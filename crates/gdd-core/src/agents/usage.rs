//! What an agent says when it ends, read from its JSON output as the kit's wrapper read it
//! (`gdd-kit/scripts/agent_wrap.py`): the tokens, the cost and its final text. Each agent sheet
//! names its reader (`usage`). An agent that reports no cost is priced with `[prices.<model>]` of
//! `gdd/models.toml`.

use serde::Deserialize;
use serde_json::Value;

/// How an agent reports what it used: the `usage` of its sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Reader {
    /// Claude Code: one JSON object, the last line, with `usage`, `total_cost_usd`, `result`
    /// and `session_id`.
    #[serde(rename = "claude-json")]
    ClaudeJson,
    /// Codex: one event per line; `turn.completed` carries the usage, the last agent message is
    /// the final text, and there is no cost.
    #[serde(rename = "codex-jsonl")]
    CodexJsonl,
    /// OpenCode: one event per line; `step_finish` carries the tokens and the cost, and the
    /// final text is every `text` part.
    #[serde(rename = "opencode-jsonl")]
    OpencodeJsonl,
}

/// What a call used.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Usage {
    /// Input tokens not read from the cache.
    pub tokens_in: u64,
    /// Input tokens read from the cache.
    pub tokens_cache: u64,
    /// Output tokens, reasoning included.
    pub tokens_out: u64,
    /// Cost in US dollars.
    pub cost_usd: f64,
}

/// What an agent said when it ended.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Said {
    /// What it used.
    pub usage: Usage,
    /// Its final text.
    pub text: String,
    /// The session of a Claude Code call, if it gave one.
    pub session: Option<String>,
}

/// The price of a model, in US dollars per million tokens: `[prices.<model>]` of
/// `gdd/models.toml`. What it leaves out costs nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Price {
    /// Per million input tokens not read from the cache.
    pub input: f64,
    /// Per million input tokens read from the cache.
    pub cache: f64,
    /// Per million output tokens.
    pub output: f64,
}

impl Usage {
    /// What these tokens cost at `price`.
    pub fn priced(&self, price: &Price) -> f64 {
        let cost = self.tokens_in as f64 * price.input
            + self.tokens_cache as f64 * price.cache
            + self.tokens_out as f64 * price.output;
        cost / 1_000_000.0
    }
}

/// What the output `out` of an agent whose sheet reads it with `reader` says.
pub fn read(reader: Reader, out: &str) -> Said {
    match reader {
        Reader::ClaudeJson => claude(out),
        Reader::CodexJsonl => codex(out),
        Reader::OpencodeJsonl => opencode(out),
    }
}

/// The number at `key` of `table`; 0 when there is none.
fn count(table: Option<&Value>, key: &str) -> u64 {
    table
        .and_then(|table| table.get(key))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

/// The text at `key` of `table`; empty when there is none.
fn text(table: Option<&Value>, key: &str) -> String {
    table
        .and_then(|table| table.get(key))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// The JSON objects among the lines of `out`; the other lines are left out.
fn events(out: &str) -> impl Iterator<Item = Value> + '_ {
    out.lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .filter(Value::is_object)
}

/// Whether `event` is of type `kind`.
fn is(event: &Value, kind: &str) -> bool {
    event.get("type").and_then(Value::as_str) == Some(kind)
}

/// Claude Code: its last line. When it is not a JSON object, nothing was used and the whole
/// output is the final text.
fn claude(out: &str) -> Said {
    let last = out.trim().lines().last().unwrap_or_default();
    let said = serde_json::from_str::<Value>(last).ok();
    let Some(said) = said.filter(Value::is_object) else {
        return Said {
            text: out.to_string(),
            ..Said::default()
        };
    };
    let usage = said.get("usage");
    let session = text(Some(&said), "session_id");
    let cost = said.get("total_cost_usd").and_then(Value::as_f64);
    Said {
        usage: Usage {
            tokens_in: count(usage, "input_tokens") + count(usage, "cache_creation_input_tokens"),
            tokens_cache: count(usage, "cache_read_input_tokens"),
            tokens_out: count(usage, "output_tokens"),
            cost_usd: cost.unwrap_or(0.0),
        },
        text: text(Some(&said), "result"),
        session: Some(session).filter(|session| !session.is_empty()),
    }
}

/// Codex: the usage of every turn, its cached input apart, and the last agent message.
fn codex(out: &str) -> Said {
    let mut said = Said::default();
    let mut input = 0;
    for event in events(out) {
        if is(&event, "turn.completed") {
            let usage = event.get("usage");
            input += count(usage, "input_tokens");
            let tokens = &mut said.usage;
            tokens.tokens_cache += count(usage, "cached_input_tokens");
            tokens.tokens_out +=
                count(usage, "output_tokens") + count(usage, "reasoning_output_tokens");
        }
        let item = event.get("item");
        if is(&event, "item.completed") && item.is_some_and(|item| is(item, "agent_message")) {
            said.text = text(item, "text");
        }
    }
    said.usage.tokens_in = input.saturating_sub(said.usage.tokens_cache);
    said
}

/// OpenCode: the tokens and the cost of every step, and every text part, a line each.
fn opencode(out: &str) -> Said {
    let mut said = Said::default();
    let mut parts = Vec::new();
    for event in events(out) {
        let part = event.get("part");
        if is(&event, "step_finish") {
            let tokens = part.and_then(|part| part.get("tokens"));
            let cache = tokens.and_then(|tokens| tokens.get("cache"));
            let usage = &mut said.usage;
            usage.tokens_in += count(tokens, "input");
            usage.tokens_cache += count(cache, "read");
            usage.tokens_out += count(tokens, "output") + count(tokens, "reasoning");
            let cost = part.and_then(|part| part.get("cost")?.as_f64());
            usage.cost_usd += cost.unwrap_or(0.0);
        }
        if is(&event, "text") {
            parts.push(text(part, "text"));
        }
    }
    said.text = parts.join("\n");
    said
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(tokens_in: u64, tokens_cache: u64, tokens_out: u64, cost_usd: f64) -> Usage {
        Usage {
            tokens_in,
            tokens_cache,
            tokens_out,
            cost_usd,
        }
    }

    #[test]
    fn claude_code_says_everything_on_its_last_line() {
        let out = concat!(
            "starting\n",
            r#"{"type":"result","result":"VERDICT: OK","session_id":"s-1","total_cost_usd":0.25,"#,
            r#""usage":{"input_tokens":100,"cache_creation_input_tokens":7,"#,
            r#""cache_read_input_tokens":50,"output_tokens":20}}"#,
            "\n\n"
        );
        let said = read(Reader::ClaudeJson, out);
        assert_eq!(said.usage, usage(107, 50, 20, 0.25));
        assert_eq!(said.text, "VERDICT: OK");
        assert_eq!(said.session.as_deref(), Some("s-1"));
        let bare = read(Reader::ClaudeJson, r#"{"usage": null, "result": 3}"#);
        assert_eq!(bare, Said::default());
        let empty_session = read(Reader::ClaudeJson, r#"{"session_id": ""}"#);
        assert_eq!(empty_session.session, None);
    }

    #[test]
    fn claude_code_output_that_is_not_json_is_the_final_text() {
        for out in ["words\n", "", "{\"broken\n", "[1, 2]", "{}\nlast line"] {
            let said = read(Reader::ClaudeJson, out);
            assert_eq!(said.usage, Usage::default(), "{out:?}");
            assert_eq!(said.text, out, "{out:?}");
            assert_eq!(said.session, None, "{out:?}");
        }
    }

    #[test]
    fn codex_counts_its_reasoning_and_leaves_out_the_cached_input() {
        let out = concat!(
            r#"{"type":"turn.completed","usage":{"input_tokens":100,"cached_input_tokens":50,"#,
            r#""output_tokens":20,"reasoning_output_tokens":5}}"#,
            "\n",
            r#"{"type":"item.completed","item":{"type":"agent_message","text":"first"}}"#,
            "\nnot json\n[3]\n",
            r#"{"type":"turn.completed","usage":{"input_tokens":10,"output_tokens":1}}"#,
            "\n",
            r#"{"type":"item.completed","item":{"type":"agent_message","text":"VERDICT: KO"}}"#,
            "\n",
            r#"{"type":"item.completed","item":{"type":"reasoning","text":"thinking"}}"#,
            "\n",
            r#"{"type":"item.started","item":{"type":"agent_message","text":"partial"}}"#,
            "\n",
            r#"{"type":"turn.completed","usage":{"input_tokens":1,"cached_input_tokens":4}}"#
        );
        let said = read(Reader::CodexJsonl, out);
        assert_eq!(said.usage, usage(57, 54, 26, 0.0));
        // More cached input than input is no input at all.
        let cached = r#"{"type":"turn.completed","usage":{"cached_input_tokens":4}}"#;
        assert_eq!(read(Reader::CodexJsonl, cached).usage, usage(0, 4, 0, 0.0));
        assert_eq!(said.text, "VERDICT: KO");
        assert_eq!(said.session, None);
        assert_eq!(read(Reader::CodexJsonl, ""), Said::default());
    }

    #[test]
    fn opencode_counts_its_reasoning_and_its_cost_and_joins_its_texts() {
        let out = concat!(
            r#"{"type":"text","part":{"text":"one"}}"#,
            "\n",
            r#"{"type":"step_finish","part":{"tokens":{"input":100,"output":20,"reasoning":5,"#,
            r#""cache":{"read":50}},"cost":0.5}}"#,
            "\n",
            r#"{"type":"step_finish","part":{"tokens":{"input":1,"cache":null},"cost":null}}"#,
            "\n",
            r#"{"type":"step_finish","part":null}"#,
            "\n",
            r#"{"type":"text","part":{"text":"two"}}"#,
            "\n",
            r#"{"type":"text"}"#
        );
        let said = read(Reader::OpencodeJsonl, out);
        assert_eq!(said.usage, usage(101, 50, 25, 0.5));
        assert_eq!(said.text, "one\ntwo\n");
        assert_eq!(read(Reader::OpencodeJsonl, "x"), Said::default());
    }

    #[test]
    fn the_tokens_are_priced_per_million() {
        let price = Price {
            input: 3.0,
            cache: 0.3,
            output: 15.0,
        };
        let used = usage(1_000_000, 2_000_000, 100_000, 0.0);
        let cost = used.priced(&price);
        assert!((cost - 5.1).abs() < 1e-9, "{cost}");
        assert_eq!(used.priced(&Price::default()), 0.0);
        let price: Price = toml::from_str("input = 2\noutput = 1.5\n").unwrap();
        assert_eq!(usage(500_000, 9, 2_000_000, 0.0).priced(&price), 4.0);
    }

    #[test]
    fn each_reader_has_the_name_of_its_sheet() {
        #[derive(Deserialize)]
        struct Sheet {
            usage: Reader,
        }
        for (name, reader) in [
            ("claude-json", Reader::ClaudeJson),
            ("codex-jsonl", Reader::CodexJsonl),
            ("opencode-jsonl", Reader::OpencodeJsonl),
        ] {
            let sheet: Sheet = toml::from_str(&format!("usage = \"{name}\"")).unwrap();
            assert_eq!(sheet.usage, reader);
        }
        assert!(toml::from_str::<Sheet>("usage = \"gemini\"").is_err());
    }
}
