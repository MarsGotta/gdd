//! Integrity of the logs, for CI: what makes a log wrong (errors) and what makes it suspect
//! (warnings). The log-only part is pure; the git part asks git about the commits a log names.

use crate::blueprint::Blueprint;
use crate::limits;
use crate::log::{self, Entry, kind, step, str_of, ts};
use crate::state::{LogKind, Progress, log_kind};
use serde_json::{Value, json};
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub log: String,
    pub line: usize,
    /// `error` or `warning`.
    pub level: &'static str,
    /// A short code: `shape`, `order`, `needs-approval`, `time-backwards`, `same-second`, `cap-exceeded`, `void`...
    pub code: &'static str,
    pub what: String,
}

impl Finding {
    pub fn to_json(&self) -> Value {
        json!({"log": self.log, "line": self.line, "level": self.level, "code": self.code, "what": self.what})
    }
}

const TIMED: [&str; 5] = ["checks", "build", "gates", "review", "reading"];

/// Everything wrong or suspect in one log, from the log alone.
/// `layer` holds the lines a call of the layer wrote, by its journal ([`crate::journal::lines_written`]):
/// two of them in the same second, or a start the layer took from git, are no sign of a log
/// written afterwards.
pub fn check_log(
    id: &str,
    bp: Option<&Blueprint>,
    entries: &[Entry],
    layer: &std::collections::BTreeSet<usize>,
) -> Vec<Finding> {
    let mut f = vec![];
    let mut push = |line: usize, level: &'static str, code: &'static str, what: String| {
        f.push(Finding {
            log: id.into(),
            line,
            level,
            code,
            what,
        })
    };
    for e in entries {
        if e.blank {
            push(e.line, "warning", "blank", "an empty line".into());
        } else if let Some(err) = &e.error {
            push(e.line, "error", "json", err.clone());
        } else if let Some(v) = e.event() {
            let (errors, warnings) = log::shape(v, false);
            for x in errors {
                push(e.line, "error", "shape", x);
            }
            for x in warnings {
                push(e.line, "warning", "shape", x);
            }
            let b = str_of(v, "brief");
            if !b.is_empty() && b != id {
                push(
                    e.line,
                    "error",
                    "wrong-log",
                    format!("its brief is {b:?}, but it is in the log of {id}"),
                );
            }
        }
    }
    // A void takes a line out of the change's state. One of a line that judged the change is
    // said, for whoever reads the log. One of a line that sent the change back is the way around
    // that `gdd correct` refuses: in a log, it was written around the layer.
    for e in entries {
        let Some(c) = e.event() else { continue };
        if kind(c) != "correction" || c["void"] != json!(true) {
            continue;
        }
        let Some(n) = c["line"].as_u64().map(|n| n as usize) else {
            continue;
        };
        let Some(target) = entries.iter().find(|x| x.line == n).and_then(|x| x.event()) else {
            continue;
        };
        if let Some(why) = log::never_voided(target) {
            push(e.line, "error", "void", format!("it voids line {n}, {why}"));
        } else if matches!(step(target), "gates" | "review") {
            push(
                e.line,
                "warning",
                "void",
                format!(
                    "it voids line {n}, a {} step that judged the change: {}",
                    step(target),
                    str_of(c, "what")
                ),
            );
        }
    }
    let events = log::events(entries);
    let voided = log::voided_events(entries);
    let mut p = Progress::new(id);
    let mut prev_ts: Option<(usize, i64)> = None;
    let mut prev_step: Option<(usize, String, Option<i64>)> = None;
    let mut approvals: Vec<(usize, String)> = vec![];
    for (i, (line, v)) in events.iter().enumerate() {
        let (line, v) = (*line, *v);
        let t = ts(v);
        if let (Some(t), Some((pl, pt))) = (t, prev_ts)
            && t < pt
        {
            push(
                line,
                "warning",
                "time-backwards",
                format!("its time is before line {pl}'s"),
            );
        }
        if let Some(t) = t {
            prev_ts = Some((line, t));
        }
        let st = step(v);
        if TIMED.contains(&st)
            && let Some((pl, ps, Some(pt))) = &prev_step
            && t == Some(*pt)
            && !(layer.contains(&line) && layer.contains(pl))
        {
            push(
                line,
                "warning",
                "same-second",
                format!(
                    "the {st} step is written in the same second as the {ps} step before it (line {pl}): a sign the log was written afterwards"
                ),
            );
        }
        if !st.is_empty() {
            prev_step = Some((line, st.to_string(), t));
        }
        if let Err((code, why)) = p.allows(bp, v) {
            push(
                line,
                "error",
                if code == "needs-approval" {
                    "needs-approval"
                } else {
                    "order"
                },
                why,
            );
        }
        // A new round that started while a cap had no room and no waiver gave it more.
        if let Some(bp) = bp
            && (st == "build" || st == "checks")
            && log_kind(id) == LogKind::Change
        {
            let before: Vec<&Value> = events[..i].iter().map(|(_, v)| *v).collect();
            let at = events[..i].iter().rev().find_map(|(_, v)| ts(v)).or(t);
            // What a voided line used before this one still counts.
            let void: Vec<&Value> = voided
                .iter()
                .filter(|(n, _)| *n < line)
                .map(|(_, v)| *v)
                .collect();
            if let Some(at) = at {
                let lim = limits::count_with(bp, &before, &void, at);
                let full = lim.no_room();
                if !full.is_empty() {
                    push(
                        line,
                        "warning",
                        "cap-exceeded",
                        format!(
                            "a new {st} round started with {} spent, and no waiver raised it",
                            full.join(", ")
                        ),
                    );
                }
            }
        }
        if st == "gates" {
            let start = str_of(v, "start");
            if let Some(expected) = &p.start
                && !start.is_empty()
                && start != expected
                && !layer.contains(&line)
            {
                push(
                    line,
                    "warning",
                    "start",
                    format!("its start {start} is not the seal's tip the log had then, {expected}"),
                );
            }
        }
        match kind(v) {
            "approval" => {
                let key = format!("{}|{}|{}", str_of(v, "of"), v["version"], v["files"]);
                if let Some((first, _)) = approvals.iter().find(|(_, k)| *k == key) {
                    push(
                        line,
                        "warning",
                        "approval-twice",
                        format!("the same approval as line {first}, written twice"),
                    );
                }
                approvals.push((line, key));
            }
            "waiver" => {
                let limit = v.get("limit").and_then(Value::as_str);
                let check = str_of(v, "check");
                let is_check = bp.is_some_and(|b| b.check(check).is_some());
                let words = format!("{} {}", check, str_of(v, "why")).to_lowercase();
                let about_a_limit = crate::blueprint::CAP_KEYS.iter().any(|k| words.contains(k))
                    || ["limit", "minutes", "rounds", "tokens", "cap"]
                        .iter()
                        .any(|w| words.contains(w));
                match limit {
                    None if !is_check && about_a_limit => push(
                        line,
                        "warning",
                        "waiver-unseen",
                        format!(
                            "a waiver of {check:?} without \"limit\" and \"to\": the counter can't see what it raised"
                        ),
                    ),
                    Some(l) if v.get("to").and_then(log::number_like).is_none() => push(
                        line,
                        "warning",
                        "waiver-unseen",
                        format!(
                            "a waiver of {l} without a number in \"to\": the counter can't see it"
                        ),
                    ),
                    Some(l)
                        if !crate::blueprint::CAP_KEYS.contains(&l) && l != "lines_per_change" =>
                    {
                        push(
                            line,
                            "warning",
                            "waiver-unseen",
                            format!("a waiver of {l:?}, which is not a cap of the blueprint"),
                        )
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        p.apply(line, v);
    }
    // Questions that never got what they asked for.
    for a in &p.asks {
        let wanted = match a.of.as_str() {
            "brief" | "change" | "blueprint" | "gates" | "debt" => Some("approval"),
            "waiver" => Some("waiver"),
            _ => None,
        };
        match (&a.ended_by, wanted) {
            (None, _) => push(
                a.line,
                "warning",
                "ask-open",
                format!("an ask of {} still waits for its answer", a.of),
            ),
            (Some(_), Some(w)) => {
                let got = events.iter().any(|(l, v)| {
                    *l > a.line && kind(v) == w && (w == "waiver" || str_of(v, "of") == a.of)
                });
                if !got {
                    push(
                        a.line,
                        "warning",
                        "ask-unanswered",
                        format!("an ask of {} that no {w} ever answered", a.of),
                    );
                }
            }
            _ => {}
        }
    }
    f
}

/// What git says against a log: sealed blobs, the commit that shipped, the commits gates judged.
pub fn check_git(root: &Path, id: &str, entries: &[Entry], main: &str) -> Vec<Finding> {
    use crate::git;
    let mut f = vec![];
    let mut push = |line: usize, level: &'static str, code: &'static str, what: String| {
        f.push(Finding {
            log: id.into(),
            line,
            level,
            code,
            what,
        })
    };
    let main_tip = git::commit_of(root, main);
    let mut p = Progress::new(id);
    for (line, v) in log::events(entries) {
        let st = step(v);
        let sealed =
            |commit: &str,
             files: &Value,
             push: &mut dyn FnMut(usize, &'static str, &'static str, String)| {
                if git::commit_of(root, commit).is_none() {
                    push(
                        line,
                        "error",
                        "seal-missing",
                        format!("the seal's commit {commit} isn't in this repository"),
                    );
                    return;
                }
                if let Some(m) = files.as_object() {
                    for (path, h) in m {
                        let Some(h) = h.as_str() else { continue };
                        match git::blob_at(root, commit, path) {
                            Some(b) if b == h => {}
                            Some(b) if h.len() >= 7 && b.starts_with(h) => push(
                                line,
                                "warning",
                                "short-hash",
                                format!(
                                    "{path} is logged with the abbreviated hash {h}; git hash-object gives {b}"
                                ),
                            ),
                            Some(b) => push(
                                line,
                                "error",
                                "seal-hash",
                                format!("{path} at {commit} is {b}, not the logged {h}"),
                            ),
                            None => push(
                                line,
                                "error",
                                "seal-hash",
                                format!(
                                    "{path} isn't at {commit}, where the log says it was sealed"
                                ),
                            ),
                        }
                    }
                }
            };
        match st {
            "brief" => {
                if let Some(s) = v["seal"].as_str() {
                    sealed(s, &v["files"], &mut push);
                }
                if let Some(b) = v["color"]["base"].as_str()
                    && git::commit_of(root, b).is_none()
                {
                    push(
                        line,
                        "error",
                        "base-missing",
                        format!("its base {b} isn't in this repository"),
                    );
                }
            }
            "checks" => {
                if let Some(c) = v["seal"]["commit"].as_str() {
                    sealed(c, &v["seal"]["checks"], &mut push);
                }
            }
            "gates" => {
                let start = str_of(v, "start");
                if !start.is_empty() && git::commit_of(root, start).is_none() {
                    push(
                        line,
                        "error",
                        "start-missing",
                        format!("its start {start} isn't in this repository"),
                    );
                } else if let Some(expected) = &p.start
                    && !start.is_empty()
                    && start != expected
                    && !git::is_ancestor(root, expected, start)
                {
                    push(
                        line,
                        "error",
                        "start",
                        format!(
                            "its start {start} is not the seal's tip at that time, {expected}, nor a later one"
                        ),
                    );
                }
                if let Some(h) = v["head"].as_str()
                    && !start.is_empty()
                    && git::commit_of(root, h).is_some()
                    && !git::is_ancestor(root, start, h)
                {
                    push(
                        line,
                        "error",
                        "start",
                        format!("the head it judged, {h}, doesn't start from {start}"),
                    );
                }
            }
            "ship" => {
                let w = str_of(v, "where");
                let first = w
                    .split(|c: char| c.is_whitespace() || c == ',' || c == '(')
                    .next()
                    .unwrap_or("");
                let head = v["head"].as_str().map(str::to_string).or_else(|| {
                    (first.len() >= 7
                        && first.len() <= 64
                        && first.chars().all(|c| c.is_ascii_hexdigit()))
                    .then(|| first.to_string())
                });
                // A change merged by squash or rebase: up to the commit of the main branch it names,
                // the commits that brought it hold exactly what was judged, and nothing else.
                let by_content = |h: &str, m: &str| -> Result<(), String> {
                    let at = v["in"].as_str().unwrap_or("");
                    if git::commit_of(root, at).is_none() || !git::is_ancestor(root, at, m) {
                        return Err(format!("{at:?} is not a commit of {main}"));
                    }
                    let start = if p.kind == Some(crate::state::LogKind::Setup) {
                        p.last_review.as_ref().and_then(|r| r.start.clone())
                    } else {
                        p.last_gates.as_ref().map(|g| g.start.clone())
                    };
                    crate::ship::by_content(root, id, main, at, &start.unwrap_or_default(), h)
                        .map(|_| ())
                        .map_err(|n| n.why)
                };
                match (head, &main_tip) {
                    (Some(h), Some(m)) => match git::commit_of(root, &h) {
                        None => push(
                            line,
                            "error",
                            "not-merged",
                            format!("the commit it shipped, {h}, isn't in this repository"),
                        ),
                        Some(c) if v["matched"] == "content" => {
                            if let Err(why) = by_content(&c, m) {
                                push(
                                    line,
                                    "error",
                                    "not-merged",
                                    format!(
                                        "it shipped {h} matched by content, and {main} doesn't hold it: {why}"
                                    ),
                                );
                            }
                        }
                        Some(c) if !git::is_ancestor(root, &c, m) => push(
                            line,
                            "error",
                            "not-merged",
                            format!("the commit it shipped, {h}, is not in {main}"),
                        ),
                        _ => {}
                    },
                    (None, _) => push(
                        line,
                        "warning",
                        "ship-unknown",
                        format!("it shipped at {w:?}, which names no commit git can check"),
                    ),
                    (_, None) => push(
                        line,
                        "warning",
                        "ship-unknown",
                        format!("there is no {main} branch to check the ship against"),
                    ),
                }
                // What shipped is what was judged, or differs from it only in the bookkeeping;
                // a review before the last gates holds only when nothing after it touched the
                // change's files.
                let g = p.last_gates.as_ref().and_then(|g| g.head.clone());
                if let (Some(h), Some(g)) = (v["head"].as_str(), &g)
                    && h != g
                    && !crate::judged::only_bookkeeping(root, id, g, h)
                {
                    push(
                        line,
                        "error",
                        "unjudged",
                        format!(
                            "it shipped {h}, which differs from {g}, the commit its gates judged, in more than the bookkeeping"
                        ),
                    );
                }
                if let (Some(r), Some(g)) = (p.last_review.as_ref(), &g)
                    && let Some(rh) = &r.head
                    && rh != g
                    && !crate::judged::only_bookkeeping(root, id, rh, g)
                    && !crate::judged::review_carries(
                        root,
                        rh,
                        g,
                        r.start.as_deref(),
                        &crate::judged::GitView::of(root, id, None, None),
                    )
                {
                    push(
                        line,
                        "error",
                        "unjudged",
                        format!(
                            "its review judged {rh}, and what came after it, up to {g}, touched the files the change touches: it needs a review of its own"
                        ),
                    );
                }
            }
            _ => {}
        }
        p.apply(line, v);
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bp() -> Blueprint {
        Blueprint::parse("---\ncontract: gdd/1\nchecks:\n  - id: tests\n    type: machine\n    command: t\ncolors:\n  yellow:\n    checks: [brief-approval, tests, review]\n    limits:\n      rounds_per_change: 1\n      minutes_per_change: 45\n      fix_rounds_per_gate: 2\n---\n").unwrap()
    }

    fn line(min_sec: &str, rest: &str) -> String {
        format!(
            "{{\"contract\":\"gdd/1\",\"ts\":\"2026-11-10T10:{min_sec}Z\",\"brief\":\"x\",{rest}}}"
        )
    }

    const R: &str = r#""report":{"skill":"s","result":"done"}"#;

    fn brief() -> String {
        line(
            "00:00",
            r#""event":"step","step":"brief","report":{"skill":"gdd-brief","result":"yellow"},"color":{"base":"b0","color":"yellow"},"files":{"gdd/briefs/x.md":"1111111111111111111111111111111111111111","features/x.feature":"2222222222222222222222222222222222222222"},"seal":"s1""#,
        )
    }

    fn codes(text: &str) -> Vec<&'static str> {
        check_log("x", Some(&bp()), &log::parse(text), &Default::default())
            .iter()
            .map(|f| f.code)
            .collect()
    }

    #[test]
    fn a_build_before_its_approval() {
        let t = [
            brief(),
            line(
                "01:00",
                &format!(
                    r#""event":"step","step":"checks",{R},"seal":{{"commit":"s2","checks":{{}}}}"#
                ),
            ),
            line("02:00", &format!(r#""event":"step","step":"build",{R}"#)),
        ]
        .join("\n");
        assert!(codes(&t).contains(&"needs-approval"));
    }

    #[test]
    fn same_second_and_backwards_and_unknown() {
        let approval = line(
            "00:30",
            r#""event":"approval","who":"p","of":"brief","version":1,"files":{"features/x.feature":"2222222222222222222222222222222222222222"},"where":"conversation""#,
        );
        let t = [
            brief(),
            approval.clone(),
            approval,
            line("01:00", &format!(r#""event":"step","step":"checks",{R},"seal":{{"commit":"s2","checks":{{}}}}"#)),
            line("02:00", &format!(r#""event":"step","step":"build",{R}"#)),
            line("02:00", r#""event":"step","step":"gates","start":"s2","gates":[{"check":"tests","seconds":1,"result":"pass"}]"#),
            line("01:59", r#""event":"step","step":"review","report":{"skill":"r","result":"pass"},"verdict":"pass""#),
            line("03:00", r#""event":"note""#),
            "not json".into(),
        ]
        .join("\n");
        let c = codes(&t);
        assert!(c.contains(&"same-second"), "{c:?}");
        assert!(c.contains(&"time-backwards"), "{c:?}");
        assert!(c.contains(&"approval-twice"), "{c:?}");
        assert!(c.contains(&"shape"), "{c:?}");
        assert!(c.contains(&"json"), "{c:?}");
    }

    #[test]
    fn ship_before_gates_and_a_round_past_its_cap() {
        let approval = line(
            "00:30",
            r#""event":"approval","who":"p","of":"brief","version":1,"files":{"features/x.feature":"2222222222222222222222222222222222222222"},"where":"conversation""#,
        );
        let t = [
            brief(),
            approval,
            line("01:00", &format!(r#""event":"step","step":"checks",{R},"seal":{{"commit":"s2","checks":{{}}}}"#)),
            line("02:00", &format!(r#""event":"step","step":"build",{R}"#)),
            line("03:00", r#""event":"step","step":"gates","start":"s2","gates":[{"check":"tests","seconds":1,"result":"fail"}]"#),
            line("04:00", &format!(r#""event":"step","step":"build",{R}"#)),
            line("05:00", r#""event":"step","step":"gates","start":"s2","gates":[{"check":"tests","seconds":1,"result":"fail"}]"#),
            line("06:00", &format!(r#""event":"step","step":"build",{R}"#)),
            line("07:00", r#""event":"step","step":"ship","where":"x","color":"yellow""#),
            line("08:00", r#""event":"ask","of":"brief","what":"y""#),
        ]
        .join("\n");
        let c = codes(&t);
        assert!(c.contains(&"cap-exceeded"), "{c:?}");
        assert!(c.contains(&"order"), "{c:?}");
        assert!(c.contains(&"ask-open"), "{c:?}");
    }
}
