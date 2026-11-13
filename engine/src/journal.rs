//! The layer's journal of its own use: one line per invocation in `gdd/log/<id>.engine.jsonl`, one
//! file per change so that changes in parallel never write the same file, and
//! `gdd/log/blueprint.engine.jsonl` for what isn't about one change.

use crate::out::Out;
use serde_json::{Map, Value, json};
use std::io::Write;
use std::path::Path;

pub const SCHEMA: &str = "gdd-engine/1";

/// Whether an argument can be kept as it is: a flag, an id, a path, a number or a commit. Anything
/// else, free text above all, is written as `<text>`, so nothing that could be a secret is kept.
pub fn keepable(arg: &str) -> bool {
    if arg.starts_with("--") {
        return arg.len() <= 40 && !arg.contains(' ');
    }
    if arg.len() >= 7 && arg.len() <= 64 && arg.chars().all(|c| c.is_ascii_hexdigit()) {
        return true;
    }
    if arg.is_empty() || arg.len() > 64 {
        return false;
    }
    let ok = arg
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._/:@=+-".contains(c));
    let mut run = 0;
    let mut longest = 0;
    for c in arg.chars() {
        if c.is_ascii_alphanumeric() {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    ok && longest < 24
}

/// The arguments of an invocation as the journal keeps them. Flags keep their names; the value of
/// a flag in `free` is always `<text>`, whatever it looks like.
pub fn sanitize(args: &[String], free: &[&str]) -> Vec<String> {
    let mut out = vec![];
    let mut hide_next = false;
    for a in args {
        if hide_next {
            out.push("<text>".into());
            hide_next = false;
            continue;
        }
        if let Some((flag, _)) = a.split_once('=').filter(|(f, _)| f.starts_with("--")) {
            out.push(format!("{flag}=<text>"));
            continue;
        }
        if a.starts_with("--") {
            hide_next = free.contains(&a.as_str());
            out.push(a.clone());
            continue;
        }
        out.push(if keepable(a) {
            a.clone()
        } else {
            "<text>".into()
        });
    }
    out
}

pub fn disabled() -> bool {
    std::env::var("GDD_NO_JOURNAL").is_ok_and(|v| !v.is_empty() && v != "0")
}

/// Append one invocation to the journal of `id`. Nothing is written when the project has no
/// `gdd/log` folder or the journal is turned off; a journal that can't be written fails nothing.
pub fn record(root: &Path, id: Option<&str>, command: &str, args: &[String], ms: u128, out: &Out) {
    if disabled() {
        return;
    }
    let dir = root.join("gdd/log");
    if !dir.is_dir() {
        return;
    }
    let id = id.filter(|i| crate::valid_id(i)).unwrap_or("blueprint");
    let mut m = Map::new();
    m.insert("schema".into(), json!(SCHEMA));
    m.insert("ts".into(), json!(crate::time::now_string()));
    m.insert("brief".into(), json!(id));
    m.insert("command".into(), json!(command));
    m.insert("args".into(), json!(args));
    m.insert("exit".into(), json!(out.code));
    m.insert("ms".into(), json!(ms as u64));
    m.insert("refused".into(), json!(out.refused));
    if let Some(Value::Object(extra)) = &out.journal {
        for (k, v) in extra {
            m.insert(k.clone(), v.clone());
        }
    }
    // The log line this call wrote, so verify can tell it from a line written by hand.
    if (out.code == 0 || out.code == 1)
        && let Some(n) = out
            .facts
            .get("line")
            .and_then(Value::as_u64)
            .filter(|n| *n > 0)
    {
        let written = std::fs::read_to_string(root.join(format!("gdd/log/{id}.jsonl")))
            .ok()
            .and_then(|t| t.lines().nth(n as usize - 1).map(str::to_string))
            .and_then(|l| serde_json::from_str::<Value>(&l).ok());
        if let Some(ev) = written {
            m.insert(
                "wrote".into(),
                json!({"line": n, "ts": ev["ts"], "event": ev["event"], "step": ev.get("step")}),
            );
        }
    }
    let line = Value::Object(m).to_string();
    let path = dir.join(format!("{id}.engine.jsonl"));
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{line}");
    }
}

/// The lines of a log that a call of the layer wrote, by its journal: a journal line names the
/// line it wrote, and the log still holds that event there, with the same time and kind.
pub fn lines_written(
    journal: &[Value],
    entries: &[crate::log::Entry],
) -> std::collections::BTreeSet<usize> {
    let mut out = std::collections::BTreeSet::new();
    for j in journal {
        let w = &j["wrote"];
        let Some(n) = w["line"].as_u64().map(|n| n as usize) else {
            continue;
        };
        let Some(ev) = entries.iter().find(|e| e.line == n).and_then(|e| e.event()) else {
            continue;
        };
        if ev["ts"] == w["ts"]
            && ev["event"] == w["event"]
            && ev.get("step") == w.get("step").filter(|s| !s.is_null())
        {
            out.insert(n);
        }
    }
    out
}

/// Read a journal's lines, those that are JSON objects.
pub fn read(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .map(|t| {
            t.lines()
                .filter_map(|l| serde_json::from_str::<Value>(l).ok())
                .filter(|v| v.is_object())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_ids_and_hides_text() {
        let a: Vec<String> = [
            "ask",
            "empty-cart",
            "--of",
            "brief",
            "Approve it?",
            "--who",
            "Person",
            "gdd/briefs/x.md",
            "3f2a1c9",
            "sk-abcdefghijklmnopqrstuvwxyz0123",
            "--why=because",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(
            sanitize(&a, &["--who"]),
            vec![
                "ask",
                "empty-cart",
                "--of",
                "brief",
                "<text>",
                "--who",
                "<text>",
                "gdd/briefs/x.md",
                "3f2a1c9",
                "<text>",
                "--why=<text>"
            ]
        );
    }
}
