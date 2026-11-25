//! What state a check is in at the gates (decision 145): it blocks, it reports, or it is in
//! quarantine because it passed and failed on the same commit. The blueprint's `state: reports`
//! has it report; `quarantine: <n>` is the person's rule, approved once, that a check that changed
//! its mind on one commit reports for the next n changes. On red every check blocks.

use super::*;
use std::collections::BTreeMap;

/// The checks whose result on this head differs from an earlier run on the same head, in this
/// change's log: the same commit, judged twice, gave two answers.
pub(super) fn flaky_now(entries: &[Entry], head: &str, ran: &[crate::gates::Ran]) -> Vec<String> {
    let mut before: BTreeMap<String, bool> = BTreeMap::new();
    for (_, v) in log::events(entries) {
        if step(v) == "gates" && v["head"].as_str() == Some(head) {
            for g in v["gates"].as_array().into_iter().flatten() {
                before.insert(str_of(g, "check").into(), str_of(g, "result") == "pass");
            }
        }
    }
    // A reported failure is a failure too: what matters is whether the answer changed.
    ran.iter()
        .filter(|r| before.get(&r.check).is_some_and(|passed| *passed != r.passed))
        .map(|r| r.check.clone())
        .collect()
}

/// The checks in quarantine for a change: one the gates of another change found flaky, while fewer
/// changes than the blueprint's `quarantine` started after that run, and no hardening record named
/// it since.
pub(super) fn quarantined(project: &Project, bp: &Blueprint, id: &str) -> Vec<String> {
    let Some(n) = bp.quarantine() else {
        return vec![];
    };
    let mut found: BTreeMap<String, i64> = BTreeMap::new();
    let mut started: Vec<i64> = vec![];
    for other in project.log_ids().into_iter().filter(|i| log_kind(i) == LogKind::Change) {
        let entries = project.read_log(&other);
        let events = log::events(&entries);
        if other != id
            && let Some(t) = events.iter().find_map(|(_, v)| log::ts(v))
        {
            started.push(t);
        }
        // What a change's own gates found flaky never quarantines a check for that change: its
        // builder could make a check change its answer to get past it.
        for (_, v) in events.iter().filter(|(_, v)| other != id && step(v) == "gates") {
            let t = log::ts(v).unwrap_or(0);
            for c in v["flaky"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                let e = found.entry(c.to_string()).or_insert(t);
                *e = (*e).max(t);
            }
        }
    }
    let looked: Vec<(String, i64)> = log::events(&project.read_log("blueprint"))
        .into_iter()
        .filter(|(_, v)| log::is_step(v, "harden"))
        .filter_map(|(_, v)| Some((str_of(v, "expected_check").to_string(), log::ts(v)?)))
        .collect();
    found
        .into_iter()
        .filter(|(check, at)| {
            if !bp.check(check).is_some_and(|c| c.quarantinable()) {
                return false;
            }
            let since = started.iter().filter(|t| *t > at).count();
            since < n && !looked.iter().any(|(c, t)| c == check && t > at)
        })
        .map(|(check, _)| check)
        .collect()
}
