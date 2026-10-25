//! The log: one JSON record per line, for every agent call, gate decision and machine step.

use std::fmt;

use serde::Deserialize;
use serde_json::{Map, Value};

/// Where the log lives, from the project root.
pub const PATH: &str = ".gdd/log.jsonl";

/// One line of the log. A field the line leaves out takes the value the log writes when it is
/// not given: 0 for numbers, `-` for text (see [`Record::text`]).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Record {
    /// Input tokens that were not read from the cache.
    pub tokens_in: u64,
    /// Input tokens read from the cache.
    pub tokens_cache: u64,
    /// Output tokens.
    pub tokens_out: u64,
    /// Cost in US dollars.
    pub cost_usd: f64,
    /// Time it took, in seconds.
    pub duration_s: f64,
    /// Every other field: `ts`, `change`, `phase`, `gate`, `role`, `agent`, `model`, `result`,
    /// `retry`, `note`...
    #[serde(flatten)]
    pub fields: Map<String, Value>,
}

impl Record {
    /// The value of one of the other fields as text: `-` when the line leaves it out, the text
    /// itself for a string, its JSON for anything else.
    pub fn text(&self, field: &str) -> String {
        match self.fields.get(field) {
            None => "-".to_string(),
            Some(Value::String(text)) => text.clone(),
            Some(value) => value.to_string(),
        }
    }

    /// Whether the record is an agent call: it used tokens in or out.
    /// Machine steps and human gates leave both at 0.
    pub fn is_agent_call(&self) -> bool {
        self.tokens_in > 0 || self.tokens_out > 0
    }
}

/// A line of the log that is not a record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineError {
    /// Number of the line, from 1.
    pub line: usize,
    /// What is wrong with it.
    pub message: String,
}

impl fmt::Display for LineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for LineError {}

/// Reads the text of a log: one record per line; blank lines are skipped. The first line that
/// is not a JSON object with the fields of a record is an error that names it.
pub fn parse(text: &str) -> Result<Vec<Record>, LineError> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(i, line)| {
            serde_json::from_str(line).map_err(|e| LineError {
                line: i + 1,
                message: without_position(&e),
            })
        })
        .collect()
}

/// The message of a JSON error without its position: the JSON of a record is a single line,
/// so the line that counts is the line of the log.
fn without_position(e: &serde_json::Error) -> String {
    let message = e.to_string();
    let suffix = format!(" at line {} column {}", e.line(), e.column());
    match message.strip_suffix(&suffix) {
        Some(reason) => reason.to_string(),
        None => message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_line_gives_its_numbers_and_its_fields() {
        let records = parse(
            r#"{"ts": "2026-10-25T10:00:00+00:00", "change": "a", "phase": "2-build", "gate": "-", "role": "builder-build", "agent": "claude", "model": "opus", "tokens_in": 100, "tokens_cache": 400, "tokens_out": 20, "cost_usd": 0.01, "duration_s": 5.5, "result": "ok", "retry": 1, "note": "/speckit-gdd-build"}"#,
        )
        .unwrap();
        let [r] = records.as_slice() else {
            panic!("one record, got {records:?}");
        };
        assert_eq!((r.tokens_in, r.tokens_cache, r.tokens_out), (100, 400, 20));
        assert_eq!((r.cost_usd, r.duration_s), (0.01, 5.5));
        assert_eq!(r.text("change"), "a");
        assert_eq!(r.text("model"), "opus");
        assert_eq!(r.text("note"), "/speckit-gdd-build");
        assert_eq!(r.text("retry"), "1");
    }

    #[test]
    fn fields_left_out_take_the_defaults_of_the_log() {
        let records =
            parse(r#"{"change": "a", "gate": "G1", "role": "human", "result": "ok"}"#).unwrap();
        let r = &records[0];
        assert_eq!((r.tokens_in, r.tokens_cache, r.tokens_out), (0, 0, 0));
        assert_eq!((r.cost_usd, r.duration_s), (0.0, 0.0));
        assert_eq!(r.text("model"), "-");
        assert_eq!(r.text("role"), "human");
        assert!(!r.is_agent_call());
    }

    #[test]
    fn values_that_are_not_text_are_their_json() {
        let records = parse(r#"{"note": null, "retry": 2, "x": [1, true]}"#).unwrap();
        assert_eq!(records[0].text("note"), "null");
        assert_eq!(records[0].text("retry"), "2");
        assert_eq!(records[0].text("x"), "[1,true]");
    }

    #[test]
    fn whole_numbers_are_valid_costs_and_durations() {
        let records = parse(r#"{"cost_usd": 1, "duration_s": 3}"#).unwrap();
        assert_eq!((records[0].cost_usd, records[0].duration_s), (1.0, 3.0));
    }

    #[test]
    fn an_agent_call_used_tokens_in_or_out() {
        let call = |tokens_in, tokens_cache, tokens_out| Record {
            tokens_in,
            tokens_cache,
            tokens_out,
            ..Record::default()
        };
        assert!(call(1, 0, 0).is_agent_call());
        assert!(call(0, 0, 1).is_agent_call());
        assert!(!call(0, 5, 0).is_agent_call());
        assert!(!call(0, 0, 0).is_agent_call());
    }

    #[test]
    fn one_record_per_line_in_order() {
        let records =
            parse("{\"change\": \"a\"}\n{\"change\": \"b\"}\r\n{\"change\": \"c\"}\n").unwrap();
        let changes: Vec<String> = records.iter().map(|r| r.text("change")).collect();
        assert_eq!(changes, ["a", "b", "c"]);
        assert_eq!(parse(""), Ok(vec![]));
    }

    #[test]
    fn blank_lines_are_skipped_but_counted() {
        let records = parse("\n{\"change\": \"a\"}\n  \n\t\n{\"change\": \"b\"}\n\n").unwrap();
        let changes: Vec<String> = records.iter().map(|r| r.text("change")).collect();
        assert_eq!(changes, ["a", "b"]);
        assert_eq!(parse("\n{}\n\n{x\n").map_err(|e| e.line), Err(4));
    }

    #[test]
    fn a_broken_line_is_an_error_that_names_it() {
        let err = parse("{}\n{}\n{not json\n{}\n").unwrap_err();
        assert_eq!(err.line, 3);
        assert_eq!(err.message, "key must be a string");
        assert_eq!(err.to_string(), "line 3: key must be a string");
    }

    #[test]
    fn lines_that_are_not_records_are_errors() {
        for (text, line) in [
            ("[1, 2]", 1),
            ("{}\n42", 2),
            ("{}\n\n\"text\"", 3),
            (r#"{"tokens_in": "many"}"#, 1),
            (r#"{"tokens_out": -1}"#, 1),
            (r#"{"cost_usd": null}"#, 1),
        ] {
            assert_eq!(parse(text).map_err(|e| e.line), Err(line), "{text:?}");
        }
    }

    #[test]
    fn an_error_without_a_position_keeps_its_message() {
        let e = <serde_json::Error as serde::de::Error>::custom("no position");
        assert_eq!(without_position(&e), "no position");
    }
}
