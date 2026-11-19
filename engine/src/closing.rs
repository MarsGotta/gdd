//! Close, the fifth moment: what a change's brief said would close it, and where a change that
//! shipped stands until someone looks. It reads nothing itself: the log's progress, the blueprint
//! and what the blueprint's log says are given to it.

use crate::blueprint::Blueprint;
use crate::log::{self, Entry, str_of};
use crate::state::{LogKind, Next, Progress};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const DAY: i64 = 86_400;

/// The three verdicts of a `close` event. Only `yes` and `no` close the change.
pub const VERDICTS: [&str; 3] = ["yes", "not-yet", "no"];

/// What to add to a brief that doesn't say how it closes.
pub const FORM: &str = "## Closing it\n\nExpected: <what changes for whoever uses it>\nSignal: <what will show it, from where the builder can't write>\nWhere: <where it will be looked at>\n\nor, when the change promises nothing to whoever uses the product, the single line:\n\nNothing to close: <why>";

/// What a brief's `## Closing it` section says: the three lines, or why there is nothing to
/// close. An error says what is missing.
pub fn read(body: &str) -> Result<Value, String> {
    let section = crate::project::section(body, "Closing it")
        .ok_or("it has no `## Closing it` section, or the section is empty")?;
    let value = |label: &str| {
        section
            .lines()
            .find_map(|l| l.trim().strip_prefix(label))
            .map(str::trim)
            .filter(|v| !v.is_empty() && !(v.starts_with('<') && v.ends_with('>')))
    };
    let three = ["Expected:", "Signal:", "Where:"].map(value);
    match (value("Nothing to close:"), three) {
        (Some(why), [None, None, None]) => Ok(json!({"nothing": why})),
        (None, [Some(e), Some(s), Some(w)]) => Ok(json!({"expected": e, "signal": s, "where": w})),
        (Some(_), _) => {
            Err("it gives both the three lines and `Nothing to close:`: keep one of them".into())
        }
        (None, _) => Err(
            "its `## Closing it` needs Expected:, Signal: and Where:, each filled in, or the single line `Nothing to close: <why>`".into(),
        ),
    }
}

/// The line of a brief's closing a person approves with the rest, or `None` when it has none.
pub fn approval_line(closing: &Value) -> Option<String> {
    if let Some(why) = closing["nothing"].as_str() {
        return Some(format!("closes: nothing to close, {why}"));
    }
    closing["expected"].as_str().map(|e| {
        format!(
            "closes: expected {e}; signal {}; looked at {}",
            str_of(closing, "signal"),
            str_of(closing, "where")
        )
    })
}

/// When a hardening record first named each change as the one that let something through.
pub fn named(blueprint_log: &[Entry]) -> BTreeMap<String, i64> {
    let mut out = BTreeMap::new();
    for (_, v) in log::events(blueprint_log) {
        if log::is_step(v, "harden")
            && let (Some(id), Some(t)) = (v["change"].as_str(), log::ts(v))
        {
            out.entry(id.to_string()).or_insert(t);
        }
    }
    out
}

/// Where a change that shipped stands at Close.
#[derive(Debug, Clone, PartialEq)]
pub enum Standing {
    /// A person's verdict closed it, or it closed by itself: how.
    Closed(String),
    /// It waits for a person's verdict: it promised something, or (`named`) a hardening record
    /// named it before it closed by itself.
    Waiting { named: bool },
    /// It promised nothing: it closes by itself at this time unless something is recorded against it.
    Quiet(i64),
}

impl Standing {
    /// One word for it, as the facts and the metrics carry it.
    pub fn word(&self) -> &'static str {
        match self {
            Standing::Closed(_) => "closed",
            Standing::Waiting { .. } => "waiting",
            Standing::Quiet(_) => "quiet",
        }
    }
}

/// Where a change stands at Close, or `None` before it ships and for a log that is no change:
/// setting up checks promises nothing to anyone and has nothing to close. `named` is when a hardening record
/// first named it. A log from before the layer knew Close has no closing, and a blueprint without
/// `closing.days` gives no days: that change reads as before, closed when it shipped.
pub fn standing(p: &Progress, bp: &Blueprint, named: Option<i64>, now: i64) -> Option<Standing> {
    p.shipped.as_ref()?;
    if p.kind != Some(LogKind::Change) {
        return None;
    }
    if let Some(v) = p.closed_by() {
        return Some(Standing::Closed(format!(
            "{} said {}, line {}",
            v.who, v.verdict, v.line
        )));
    }
    if p.closing
        .as_ref()
        .is_some_and(|c| c["expected"].is_string())
    {
        return Some(Standing::Waiting { named: false });
    }
    let until = p
        .shipped_at
        .unwrap_or(now)
        .saturating_add(bp.closing_days().saturating_mul(DAY));
    // Named before its days went by: with no days, nothing is.
    Some(if named.is_some_and(|t| t < until) {
        Standing::Waiting { named: true }
    } else if now >= until {
        Standing::Closed("by itself, with nothing recorded against it".into())
    } else {
        Standing::Quiet(until)
    })
}

/// The command that records the person's verdict.
pub fn verdict_command(id: &str) -> String {
    format!(
        "gdd close {id} --verdict <yes, not-yet or no> --who <person> --where <conversation, commit or pull request> --saw \"<what they saw, in their words>\""
    )
}

/// What the brief expected, or why the change waits without having promised anything.
pub fn expected(p: &Progress) -> String {
    match p.closing.as_ref().and_then(|c| c["expected"].as_str()) {
        Some(e) => format!("expected: {e}"),
        None => "a hardening record names it, so it no longer closes by itself".into(),
    }
}

/// The next step of a change that shipped. Close never starts by itself: a change that waits
/// for a verdict names no command, and stops nothing.
pub fn next(id: &str, bp: &Blueprint, p: &Progress, named: Option<i64>, now: i64) -> Next {
    let (line, place) = p.shipped.clone().unwrap_or_default();
    let shipped = format!("it shipped ({place}), line {line}");
    let rest = |step: &str, say: String| Next {
        step: step.into(),
        say,
        command: String::new(),
        blocked: None,
    };
    if let Some(a) = p.open_ask().filter(|a| a.of == "close") {
        let waited =
            a.ts.map(|t| crate::time::waited(now - t))
                .unwrap_or_default();
        return Next {
            step: "wait".into(),
            say: format!(
                "waiting for the person's verdict since line {} ({waited}). It stops nothing, and only their words are a verdict: {}",
                a.line, a.what
            ),
            command: verdict_command(id),
            blocked: Some("needs-person".into()),
        };
    }
    match standing(p, bp, named, now) {
        Some(Standing::Waiting { .. }) => {
            let in_use = crate::time::waited(now - p.shipped_at.unwrap_or(now));
            let looked = match p.verdicts.last() {
                Some(v) => format!(
                    "; {} looked {} ago and said {}",
                    v.who,
                    crate::time::waited(now - v.ts.unwrap_or(now)),
                    v.verdict
                ),
                None => String::new(),
            };
            rest(
                "close",
                format!(
                    "{shipped}: in use for {in_use}, with no verdict that closes it{looked}; {}. It stops nothing, and Close never starts by itself: say in one line of your summary that it waits. When the person asks to look, load gdd-close and run gdd close {id}",
                    expected(p)
                ),
            )
        }
        Some(Standing::Quiet(until)) => rest(
            "done",
            format!(
                "{shipped}: nothing to close, and it closes by itself in {} unless something is recorded against it",
                crate::time::waited(until - now)
            ),
        ),
        Some(Standing::Closed(how)) => rest("done", format!("{shipped}: closed, {how}")),
        None => rest("done", format!("{shipped}: its log is closed")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_three_lines_or_nothing() {
        let three = "## Request\n\nx\n\n## Closing it\n\nExpected: fewer calls\nSignal: the call log\nWhere: the weekly report\n\n## Rules\n";
        assert_eq!(
            read(three).unwrap(),
            json!({"expected": "fewer calls", "signal": "the call log", "where": "the weekly report"})
        );
        assert_eq!(
            read("## Closing it\n\nNothing to close: it corrects a spelling\n").unwrap(),
            json!({"nothing": "it corrects a spelling"})
        );
    }

    #[test]
    fn refuses_what_is_missing_mixed_or_left_as_the_template() {
        assert!(read("## Request\n\nx\n").is_err());
        assert!(read("## Closing it\n\nExpected: a\nSignal: b\n").is_err());
        assert!(read("## Closing it\n\nExpected: a\nSignal: b\nWhere: <where>\n").is_err());
        assert!(
            read("## Closing it\n\nExpected: a\nSignal: b\nWhere: c\nNothing to close: d\n")
                .is_err()
        );
        assert!(read("## Closing it\n\nNothing to close:\n").is_err());
    }
}
