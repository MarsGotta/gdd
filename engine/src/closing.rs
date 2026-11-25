//! Close, the fifth moment: what a change's brief said would close it, and where a change that
//! shipped stands until someone looks. It reads nothing itself: the log's progress, the blueprint
//! and what the blueprint's log says are given to it.

use crate::blueprint::Blueprint;
use crate::git::{Reverts, Undone};
use crate::log::{self, Entry, str_of};
use crate::state::{LogKind, Next, Progress};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const DAY: i64 = 86_400;

/// The three verdicts of a `close` event. Only `yes` and `no` close the change.
pub const VERDICTS: [&str; 3] = ["yes", "not-yet", "no"];

/// What to add to a brief that doesn't say how it closes.
pub const FORM: &str = "## Closing it\n\nExpected: <what changes for whoever uses it>\nSignal: <what will show it, from where the builder can't write>\nWhere: <where it will be looked at>\n\nor, when the change promises nothing to whoever uses the product, the single line:\n\nNothing to close: <why>";

/// What a brief's `## Closing it` section says: the three lines, or why there is nothing to
/// close. An error says what is missing. A section quoted inside a fenced block doesn't count.
pub fn read(body: &str) -> Result<Value, String> {
    let mut fenced = false;
    let unfenced: String = body
        .lines()
        .filter(|l| {
            let mark = l.trim_start().starts_with("```") || l.trim_start().starts_with("~~~");
            fenced ^= mark;
            !fenced && !mark
        })
        .map(|l| format!("{l}\n"))
        .collect();
    let section = crate::project::section(&unfenced, "Closing it")
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

/// Whether a closing is one `read` gives: the three lines, or why there is nothing to close.
pub fn is_form(closing: &Value) -> bool {
    let text = |k: &str| closing[k].as_str().is_some_and(|v| !v.trim().is_empty());
    closing.as_object().is_some_and(|m| match m.len() {
        1 => text("nothing"),
        3 => ["expected", "signal", "where"].iter().all(|k| text(k)),
        _ => false,
    })
}

/// What the brief expected of the change, when it promised something.
pub fn expected(p: &Progress) -> Option<&str> {
    p.closing.as_ref()?.get("expected")?.as_str()
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

/// Each time a hardening record named a change as the one that let something through, in order.
pub fn named(blueprint_log: &[Entry]) -> BTreeMap<String, Vec<i64>> {
    let mut out: BTreeMap<String, Vec<i64>> = BTreeMap::new();
    for (_, v) in log::events(blueprint_log) {
        if log::is_step(v, "harden")
            && let (Some(id), Some(t)) = (v["change"].as_str(), log::ts(v))
        {
            out.entry(id.to_string()).or_default().push(t);
        }
    }
    out
}

/// The commits a log says hold the change in the main branch: the head its gates passed on, the
/// head it shipped with, and the commit that holds its content when that head isn't there.
fn shipped_commits(entries: &[Entry]) -> Vec<String> {
    let events = log::events(entries);
    let last = |step: &str, key: &str| {
        let found = events.iter().rev().find(|(_, v)| log::is_step(v, step));
        found.and_then(|(_, v)| v[key].as_str().map(String::from))
    };
    [
        last("gates", "head"),
        last("ship", "head"),
        last("ship", "in"),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// The revert of what a log shipped, when the main branch holds one.
pub fn undone(reverts: &Reverts, entries: &[Entry]) -> Option<Undone> {
    reverts.of(&shipped_commits(entries)).cloned()
}

/// Where a change that shipped stands at Close.
#[derive(Debug, Clone, PartialEq)]
pub enum Standing {
    /// A person's verdict closed it, or it closed by itself: how.
    Closed(String),
    /// It waits for a person's verdict, and why: what its brief expected, or what was recorded
    /// against a change that promised nothing before it closed by itself.
    Waiting(String),
    /// It promised nothing: it closes by itself at this time unless something is recorded against it.
    Quiet(i64),
    /// It fixed what this change shipped, and waits for a hardening record that names it, with
    /// what should have caught it, even nothing (decision 143).
    Unhardened(String),
}

impl Standing {
    /// One word for it, as the facts and the metrics carry it.
    pub fn word(&self) -> &'static str {
        match self {
            Standing::Closed(_) => "closed",
            Standing::Waiting(_) | Standing::Unhardened(_) => "waiting",
            Standing::Quiet(_) => "quiet",
        }
    }
}

/// Where a change stands at Close, or `None` before it ships and for a log that is no change:
/// setting up checks promises nothing to anyone and has nothing to close. `named` is when a hardening record
/// first named it, and `undone` the revert of what it shipped, when the main branch holds one.
/// A log from before the layer knew Close has no closing, and a blueprint without
/// `closing.days` gives no days: that change reads as before, closed when it shipped.
pub fn standing(
    p: &Progress,
    bp: &Blueprint,
    named: &[i64],
    fix: Option<(String, bool)>,
    undone: Option<&Undone>,
    now: i64,
) -> Option<Standing> {
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
    // A fix of what shipped closes once hardening has looked at what let it through.
    if let Some((fixed, false)) = fix {
        return Some(Standing::Unhardened(fixed));
    }
    if let Some(e) = expected(p) {
        return Some(Standing::Waiting(format!("expected: {e}")));
    }
    // What promised nothing waits once someone looked at it or asked about it.
    if let Some(v) = p.verdicts.last() {
        return Some(Standing::Waiting(format!(
            "it promised nothing, and {} said not yet",
            v.who
        )));
    }
    if p.open_ask().is_some_and(|a| a.of == "close") {
        return Some(Standing::Waiting(
            "it promised nothing, and the person was asked whether it helped".into(),
        ));
    }
    let shipped = p.shipped_at.unwrap_or(now);
    let until = shipped.saturating_add(bp.closing_days().saturating_mul(DAY));
    // Undone, or named once it was in use, before its days went by: with no days, nothing is.
    Some(if until > shipped && undone.is_some_and(|u| u.at < until) {
        Standing::Waiting("what it shipped was reverted, so it no longer closes by itself".into())
    } else if named.iter().any(|t| shipped <= *t && *t < until) {
        Standing::Waiting("a hardening record names it, so it no longer closes by itself".into())
    } else if now >= until {
        Standing::Closed("by itself, with nothing recorded against it".into())
    } else {
        Standing::Quiet(until)
    })
}

/// Who looked last and what they said, when someone did.
pub fn last_look(p: &Progress, now: i64) -> Option<String> {
    p.verdicts.last().map(|v| {
        format!(
            "{} looked {} ago and said {}",
            v.who,
            crate::time::waited(now - v.ts.unwrap_or(now)),
            v.verdict
        )
    })
}

/// What the orchestrator does about a change that waits for a verdict: nothing, until asked.
pub fn waits_note(id: &str) -> String {
    format!(
        "It stops nothing, and Close never starts by itself: say in one line of your summary that it waits, with no question and no offer to look. When the person asks to look at it, load gdd-close and run gdd close {id}"
    )
}

/// The command that records the person's verdict.
pub fn verdict_command(id: &str) -> String {
    format!(
        "gdd close {id} --verdict <yes, not-yet or no> --who <person> --where <conversation, commit or pull request> --saw \"<what they saw: their words, or what you showed them>\""
    )
}

/// The next step of a change that shipped. Close never starts by itself: a change that waits
/// for a verdict names no command, and stops nothing.
pub fn next(id: &str, p: &Progress, standing: Option<&Standing>, now: i64) -> Next {
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
    match standing {
        Some(Standing::Waiting(why)) => {
            let in_use = crate::time::waited(now - p.shipped_at.unwrap_or(now));
            let looked = last_look(p, now).map_or(String::new(), |l| format!("; {l}"));
            rest(
                "in-use",
                format!(
                    "{shipped}: in use for {in_use}, with no verdict that closes it{looked}; {why}. {}",
                    waits_note(id)
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
        Some(Standing::Unhardened(fixed)) => Next {
            step: "in-use".into(),
            say: format!(
                "{shipped}: it fixed what {fixed} shipped, and it closes once a hardening record names {fixed}, with what should have caught it, even none. Propose that record to the person once, with gdd-harden: what to record is theirs"
            ),
            command: format!(
                "gdd step blueprint harden --change {fixed} --crack \"<what got through>\" --expected-check <the check that should have caught it, or none> --caught-by <who or what found it> --proposal \"<what you propose, or nothing>\" --result <proposed or approved>"
            ),
            blocked: None,
        },
        Some(Standing::Closed(how)) => rest("done", format!("{shipped}: closed, {how}")),
        None => rest("done", shipped),
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
        // A section quoted in a fence is an example, not the brief's own.
        let quoted = "## Request\n\n```\n## Closing it\n\nNothing to close: x\n```\n";
        assert!(read(quoted).is_err());
        let after = format!("{quoted}\n## Closing it\n\nExpected: a\nSignal: b\nWhere: c\n");
        assert_eq!(read(&after).unwrap()["where"], "c");
    }

    #[test]
    fn a_closing_has_one_of_the_two_forms() {
        assert!(is_form(&json!({"nothing": "a rename"})));
        assert!(is_form(
            &json!({"expected": "a", "signal": "b", "where": "c"})
        ));
        for bad in [
            json!({}),
            json!({"expected": 1}),
            json!({"nothing": " "}),
            json!("x"),
        ] {
            assert!(!is_form(&bad), "{bad}");
        }
    }
}
