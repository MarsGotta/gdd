//! Launching a role: the command and the prompt it gets, running it with its standard input closed,
//! seeing that it started, and reading its last message and the tokens its environment reports.

use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

/// Split a command line as `sh` would split plain words and quotes, without running a shell.
/// A launch without its redirection of standard input from `/dev/null`, which 0.10.1 told people
/// to write: the layer closes standard input itself, and a shell redirection is no argument.
pub fn without_stdin(launch: &str) -> String {
    let mut s = launch.to_string();
    for pat in ["0< /dev/null", "0</dev/null", "< /dev/null", "</dev/null"] {
        while let Some(i) = s.find(pat) {
            s.replace_range(i..i + pat.len(), " ");
        }
    }
    s
}

pub fn split(line: &str) -> Result<Vec<String>, String> {
    let line = &without_stdin(line);
    let mut words = vec![];
    let mut cur = String::new();
    let mut has = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                has = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(x) => cur.push(x),
                        None => return Err("an unclosed single quote".into()),
                    }
                }
            }
            '"' => {
                has = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(x) if "\"\\$`".contains(x) => cur.push(x),
                            Some(x) => {
                                cur.push('\\');
                                cur.push(x)
                            }
                            None => return Err("a backslash at the end".into()),
                        },
                        Some(x) => cur.push(x),
                        None => return Err("an unclosed double quote".into()),
                    }
                }
            }
            '\\' => match chars.next() {
                Some(x) => {
                    has = true;
                    cur.push(x)
                }
                None => return Err("a backslash at the end".into()),
            },
            c if c.is_whitespace() => {
                if has {
                    words.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            ';' | '|' | '&' | '>' | '<' if !has || cur.is_empty() => {
                // `<model>` and `<prompt>` are placeholders, not redirections.
                if c == '<' {
                    if chars.peek().is_some_and(|x| x.is_whitespace()) {
                        return Err(
                            "a `<` redirection needs a shell: a launch is one command".into()
                        );
                    }
                    let mut ph = String::from("<");
                    for x in chars.by_ref() {
                        ph.push(x);
                        if x == '>' {
                            break;
                        }
                    }
                    cur.push_str(&ph);
                    has = true;
                } else {
                    return Err(format!("{c:?} needs a shell: a launch is one command"));
                }
            }
            c => {
                has = true;
                cur.push(c)
            }
        }
    }
    if has {
        words.push(cur);
    }
    Ok(words)
}

/// Whether a `launch` of the blueprint is a command the layer can run: one command whose words
/// name the model with `<model>`. Prose such as "the Agent tool, with its model field" is not.
pub fn is_command(launch: &str) -> bool {
    launch.contains("<model>") && split(launch).is_ok_and(|w| !w.is_empty() && !w[0].contains(' '))
}

/// The words of a launch, with `<model>`, `<prompt>` and `<prompt file>` put in. Without a
/// `<prompt>` or `<prompt file>`, the prompt goes last.
pub fn argv(
    template: &str,
    model: &str,
    prompt: &str,
    prompt_file: &str,
) -> Result<Vec<String>, String> {
    let words = split(template)?;
    if words.is_empty() {
        return Err("the launch is empty".into());
    }
    let mut out = vec![];
    let mut placed = false;
    for w in words {
        if w.contains("<prompt>") || w.contains("<prompt file>") {
            placed = true;
        }
        out.push(
            w.replace("<model>", model)
                .replace("<prompt file>", prompt_file)
                .replace("<prompt>", prompt),
        );
    }
    if !placed {
        out.push(prompt.to_string());
    }
    Ok(out)
}

/// How a launched program reports its last message and its tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Adapter {
    /// `claude -p --output-format stream-json` (or `json`): a `result` event with `modelUsage`.
    Claude,
    /// `codex exec --json`: `item.completed` agent messages and `turn.completed` usage.
    Codex,
    /// Anything else: its whole output is its last message, and it reports no tokens.
    Plain,
}

impl Adapter {
    pub fn of_program(program: &str) -> Adapter {
        let base = Path::new(program)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        match base.as_str() {
            "claude" => Adapter::Claude,
            "codex" => Adapter::Codex,
            _ => Adapter::Plain,
        }
    }

    pub fn by_name(name: &str) -> Option<Adapter> {
        match name {
            "claude" => Some(Adapter::Claude),
            "codex" => Some(Adapter::Codex),
            "plain" => Some(Adapter::Plain),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Adapter::Claude => "claude",
            Adapter::Codex => "codex",
            Adapter::Plain => "plain",
        }
    }
}

/// Tokens of one model, as an environment reported them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Usage {
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cache_write: i64,
}

impl Usage {
    /// What the layer writes as a step's `tokens`: fresh input, cache writes and output, without the
    /// cache reads, which a long session repeats on every turn.
    pub fn counted(&self) -> i64 {
        self.input + self.cache_write + self.output
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Said {
    /// The role's last message, or its whole output when the environment frames none.
    pub text: String,
    /// Tokens by model; empty when the environment reported none.
    pub usage: BTreeMap<String, Usage>,
    pub error: Option<String>,
}

impl Said {
    pub fn tokens(&self) -> Option<i64> {
        (!self.usage.is_empty()).then(|| self.usage.values().map(Usage::counted).sum())
    }

    pub fn usage_json(&self) -> Value {
        let m: serde_json::Map<String, Value> = self
            .usage
            .iter()
            .map(|(k, u)| {
                (
                    k.clone(),
                    json!({"input": u.input, "output": u.output, "cache_read": u.cache_read, "cache_write": u.cache_write}),
                )
            })
            .collect();
        Value::Object(m)
    }
}

fn int(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(Value::as_i64).unwrap_or(0)
}

/// Read a launched role's output with the adapter of its program.
pub fn read_output(adapter: Adapter, raw: &str, model: &str) -> Said {
    let mut said = Said::default();
    let events: Vec<Value> = raw
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l.trim()).ok())
        .filter(|v| v.is_object())
        .collect();
    match adapter {
        Adapter::Claude => {
            let mut text: Option<String> = None;
            let mut last_assistant = None;
            for ev in &events {
                match ev.get("type").and_then(Value::as_str) {
                    Some("assistant")
                        if ev.get("parent_tool_use_id").is_none_or(Value::is_null) =>
                    {
                        if let Some(Value::Array(blocks)) = ev.pointer("/message/content") {
                            for b in blocks {
                                if b.get("type").and_then(Value::as_str) == Some("text")
                                    && let Some(t) = b.get("text").and_then(Value::as_str)
                                    && !t.trim().is_empty()
                                {
                                    last_assistant = Some(t.to_string());
                                }
                            }
                        }
                    }
                    Some("result") => {
                        text = ev.get("result").and_then(Value::as_str).map(str::to_string);
                        if let Some(Value::Object(mu)) = ev.get("modelUsage") {
                            said.usage.clear();
                            for (m, u) in mu {
                                said.usage.insert(
                                    m.clone(),
                                    Usage {
                                        input: int(u, "inputTokens"),
                                        output: int(u, "outputTokens"),
                                        cache_read: int(u, "cacheReadInputTokens"),
                                        cache_write: int(u, "cacheCreationInputTokens"),
                                    },
                                );
                            }
                        }
                        if ev.get("is_error") == Some(&Value::Bool(true)) {
                            said.error = Some(text.clone().unwrap_or_else(|| "error".into()));
                        }
                    }
                    _ => {}
                }
            }
            said.text = text.or(last_assistant).unwrap_or_else(|| raw.to_string());
        }
        Adapter::Codex => {
            let mut text = None;
            for ev in &events {
                match ev.get("type").and_then(Value::as_str) {
                    Some("turn.completed") => {
                        let u = ev.get("usage").cloned().unwrap_or(Value::Null);
                        let cached = int(&u, "cached_input_tokens");
                        // Each turn.completed carries the thread's total so far: the last one counts.
                        said.usage.insert(
                            model.to_string(),
                            Usage {
                                input: int(&u, "input_tokens") - cached,
                                output: int(&u, "output_tokens"),
                                cache_read: cached,
                                cache_write: 0,
                            },
                        );
                    }
                    Some("turn.failed") => {
                        said.error =
                            Some(ev.get("error").map(|e| e.to_string()).unwrap_or_default())
                    }
                    Some("item.completed") => {
                        let item = ev.get("item").cloned().unwrap_or(Value::Null);
                        if item.get("type").and_then(Value::as_str) == Some("agent_message")
                            && let Some(t) = item.get("text").and_then(Value::as_str)
                        {
                            text = Some(t.to_string());
                        }
                    }
                    _ => {}
                }
            }
            said.text = text.unwrap_or_else(|| raw.to_string());
        }
        Adapter::Plain => said.text = raw.to_string(),
    }
    said
}

/// What the layer saw of a launched process.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Seen {
    pub started: bool,
    pub first_output: Option<f64>,
    pub seconds: f64,
    pub timed_out: bool,
    pub exit: Option<i32>,
}

fn kill_group(pid: u32) {
    let _ = Command::new("kill")
        .args(["-TERM", "--", &format!("-{pid}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(Duration::from_millis(300));
    let _ = Command::new("kill")
        .args(["-KILL", "--", &format!("-{pid}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Run a role's command in `cwd`, standard input closed, its output to `out`. After `start_wait` it
/// must be alive or done, and have written something, or it didn't start and is stopped. It is
/// stopped too when it runs past `timeout`.
pub fn run(
    argv: &[String],
    cwd: &Path,
    out: &Path,
    start_wait: Duration,
    timeout: Duration,
) -> Result<Seen, String> {
    if argv.is_empty() {
        return Err("an empty command".into());
    }
    let file = std::fs::File::create(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let err = file.try_clone().map_err(|e| e.to_string())?;
    let mut c = Command::new(&argv[0]);
    c.args(&argv[1..])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(file)
        .stderr(err);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        c.process_group(0);
    }
    let started_at = crate::time::instant();
    let mut child = c
        .spawn()
        .map_err(|e| format!("{} could not start: {e}", argv[0]))?;
    let pid = child.id();
    let mut seen = Seen::default();
    let size = |p: &Path| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    loop {
        let elapsed = started_at.elapsed();
        if seen.first_output.is_none() && size(out) > 0 {
            seen.first_output = Some(elapsed.as_secs_f64());
            seen.started = true;
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if seen.first_output.is_none() && size(out) > 0 {
                    seen.first_output = Some(elapsed.as_secs_f64());
                    seen.started = true;
                }
                seen.exit = status.code();
                break;
            }
            Ok(None) => {}
            Err(e) => return Err(e.to_string()),
        }
        if !seen.started && elapsed >= start_wait {
            kill_group(pid);
            let _ = child.wait();
            break;
        }
        if elapsed >= timeout {
            seen.timed_out = true;
            kill_group(pid);
            let _ = child.wait();
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    seen.seconds = (started_at.elapsed().as_secs_f64() * 100.0).round() / 100.0;
    if let Some(f) = seen.first_output {
        seen.first_output = Some((f * 100.0).round() / 100.0);
    }
    Ok(seen)
}

/// The skill of a role, named both ways: as the environment has it installed, and as a plugin
/// gives it.
pub fn skill_ref(role: &str) -> String {
    let skill = if role == "reading" {
        "gdd-review".to_string()
    } else {
        format!("gdd-{role}")
    };
    format!("the `{skill}` skill (`gdd:{skill}` when it comes from a plugin)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_like_a_shell() {
        assert_eq!(
            split("codex exec -m <model> -c 'a b' \"c \\\"d\\\"\" <prompt>").unwrap(),
            vec![
                "codex", "exec", "-m", "<model>", "-c", "a b", "c \"d\"", "<prompt>"
            ]
        );
        assert!(split("a | b").is_err());
        assert_eq!(
            split("codex exec -m <model> <prompt> < /dev/null").unwrap(),
            vec!["codex", "exec", "-m", "<model>", "<prompt>"]
        );
        assert_eq!(
            split("run -m <model> </dev/null").unwrap(),
            vec!["run", "-m", "<model>"]
        );
        assert!(is_command("run -m <model> < /dev/null"));
        assert!(split("run < input.txt").is_err());
        assert!(is_command("claude -p --model <model>"));
        assert!(!is_command("the Agent tool, with its model field"));
        assert_eq!(
            argv("sh run.sh --model <model>", "m1", "do it", "/tmp/p").unwrap(),
            vec!["sh", "run.sh", "--model", "m1", "do it"]
        );
        assert_eq!(
            argv("tool --file <prompt file> -m <model>", "m1", "x", "/tmp/p").unwrap(),
            vec!["tool", "--file", "/tmp/p", "-m", "m1"]
        );
    }

    #[test]
    fn reads_a_claude_stream() {
        let raw = r#"{"type":"system","subtype":"init"}
{"type":"assistant","message":{"content":[{"type":"text","text":"working"}]}}
{"type":"result","result":"pass\nCOLOR: green\n```gdd-report\nskill: gdd-review 0.10.1\nresult: pass\ncaught: none\n```","session_id":"s1","modelUsage":{"model-a":{"inputTokens":10,"outputTokens":5,"cacheReadInputTokens":1000,"cacheCreationInputTokens":20}}}
"#;
        let s = read_output(Adapter::Claude, raw, "model-a");
        assert!(s.text.starts_with("pass"));
        assert_eq!(s.tokens(), Some(35));
        assert_eq!(s.usage["model-a"].cache_read, 1000);
    }

    #[test]
    fn reads_a_codex_stream() {
        let raw = r#"{"type":"thread.started","thread_id":"t1"}
{"type":"item.completed","item":{"type":"agent_message","text":"first"}}
{"type":"item.completed","item":{"type":"command_execution","command":"ls"}}
{"type":"item.completed","item":{"type":"agent_message","text":"last\n```gdd-report\nskill: gdd-build 0.10.1\nresult: done\ncaught: none\n```"}}
{"type":"turn.completed","usage":{"input_tokens":1200,"cached_input_tokens":1000,"output_tokens":50}}
"#;
        let s = read_output(Adapter::Codex, raw, "model-b");
        assert!(s.text.starts_with("last"));
        assert_eq!(s.usage["model-b"].input, 200);
        assert_eq!(s.tokens(), Some(250));
    }

    #[test]
    fn plain_output_reports_no_tokens() {
        let s = read_output(Adapter::Plain, "hello", "m");
        assert_eq!(s.tokens(), None);
        assert_eq!(
            Adapter::of_program("/usr/local/bin/claude"),
            Adapter::Claude
        );
        assert_eq!(Adapter::of_program("sh"), Adapter::Plain);
    }
}
