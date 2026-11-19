//! What the blueprint's limits still allow one change, counted from its log.
//!
//! It counts exactly as the contract's counter, `gdd/limits.sh`, does: the same caps, the same
//! used values, the same `stop`, `slow` and `judge`, and the same lines. It is pure: the events, the
//! time it counts to and the log's id, which says whether the checks are what is built, are given.

use crate::blueprint::Blueprint;
use crate::log::{any_kv, kind, number_like, result, step, tokens_in, ts};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub name: &'static str,
    pub key: &'static str,
    /// What was used. Whole for counts and minutes, with fractions for seconds.
    pub used: f64,
    pub cap: Option<i64>,
    pub raised: bool,
    /// No room left: for a tier of seconds, past it.
    pub full: bool,
    /// A tier of seconds, which stops nothing.
    pub tier: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Limits {
    pub color: String,
    pub lines: Vec<Line>,
    pub stop: Vec<String>,
    pub slow: Vec<String>,
    /// `gates`, `review`, `ship` or empty.
    pub judge: String,
    /// A cap has no room left while a finished build waits to be judged.
    pub spent: bool,
    pub minutes: i64,
    /// Seconds spent waiting for a person, from each ask to the event after it.
    pub waited: i64,
    pub builds: i64,
    pub checks: i64,
    /// Recorded waivers, including old line waivers. Only caps on rounds, time and cost can stop work.
    pub raised: BTreeMap<String, i64>,
}

impl Limits {
    pub fn line(&self, key: &str, name: &str) -> Option<&Line> {
        self.lines.iter().find(|l| l.key == key && l.name == name)
    }

    /// The caps with no room left, those in `stop` and those held for a build being judged.
    pub fn no_room(&self) -> Vec<&'static str> {
        self.no_room_for(false)
    }

    pub(crate) fn no_room_for(&self, checks_fix: bool) -> Vec<&'static str> {
        self.lines
            .iter()
            .filter(|l| l.full && !l.tier && !(checks_fix && l.name == "rounds"))
            .map(|l| l.name)
            .collect()
    }

    /// The lines the counter prints, in its order and words.
    pub fn text(&self) -> Vec<String> {
        let mut out = vec![];
        if !self.color.is_empty() {
            out.push(format!("color {}", self.color));
        }
        for l in &self.lines {
            let used = l.used.trunc() as i64;
            match l.cap {
                None => out.push(format!("{} {} (no cap)", l.name, used)),
                Some(c) => {
                    let raised = if l.raised { ", raised by a waiver" } else { "" };
                    let full = match (l.full, l.tier) {
                        (true, true) => ", past it: to the backlog for hardening",
                        (true, false) => ", no room left",
                        _ => "",
                    };
                    out.push(format!("{} {} of {}{}{}", l.name, used, c, raised, full));
                }
            }
        }
        let after = if self.spent {
            "; a cap above has no room left, so no new round starts after it"
        } else {
            ""
        };
        match self.judge.as_str() {
            "gates" => out.push(format!(
                "the last build is waiting for its gates: run them{after}"
            )),
            "review" => out.push(format!(
                "the last build passed its gates and is waiting for its review: launch it{after}"
            )),
            "ship" => out.push(format!(
                "the last build passed its review and is waiting to ship{after}"
            )),
            _ => {}
        }
        out
    }

    /// The facts of the counter's last line, and what was used of each cap.
    pub fn facts(&self) -> Value {
        let used: serde_json::Map<String, Value> = self
            .lines
            .iter()
            .map(|l| {
                (
                    l.name.to_string(),
                    json!({"key": l.key, "used": l.used, "cap": l.cap, "raised": l.raised, "full": l.full}),
                )
            })
            .collect();
        json!({"color": self.color, "stop": self.stop, "slow": self.slow, "judge": self.judge, "caps": used, "minutes": self.minutes, "waited_seconds": self.waited})
    }
}

/// What one `gates` step took: the seconds of its checks of each round, those of its checks before
/// the review, and whether one of the latter ran.
fn gates_cost(v: &Value, late: &[String]) -> (f64, f64, bool) {
    let (mut each, mut before, mut late_ran) = (0f64, 0f64, false);
    if let Some(Value::Array(items)) = v.get("gates") {
        for g in items {
            let Some(c) = g.get("check").and_then(Value::as_str) else {
                continue;
            };
            let s = g
                .get("seconds")
                .and_then(Value::as_f64)
                .filter(|s| *s >= 0.0)
                .unwrap_or(0.0);
            if late.iter().any(|x| x == c) {
                late_ran = true;
                before += s;
            } else {
                each += s;
            }
        }
    }
    (each, before, late_ran)
}

/// What a log's events add up to, before any cap is read.
#[derive(Default)]
struct Tally {
    first: Option<i64>,
    asked: i64,
    waiting: bool,
    waited: i64,
    // A change that shipped is over: its minutes stop there, not at the clock.
    shipped: Option<i64>,
    builds: i64,
    seal_merges: i64,
    fixing_checks: bool,
    checks: i64,
    review: i64,
    gates: i64,
    late_runs: i64,
    tokens: i64,
    most_each: f64,
    most_before: f64,
    next_up: String,
    color: String,
    raised: BTreeMap<String, i64>,
}

impl Tally {
    /// An event that counts: it moves the clock, the color, the caps a waiver raised and what
    /// waits to be judged, and it uses what it uses.
    fn take(&mut self, v: &Value, late: &[String]) {
        let t = ts(v);
        if let Some(t) = t {
            if self.first.is_none() {
                self.first = Some(t);
            }
            if self.waiting && self.shipped.is_none() {
                self.waited += t - self.asked;
                self.waiting = false;
            }
        }
        if kind(v) == "ask"
            && self.shipped.is_none()
            && let Some(t) = t
        {
            self.asked = t;
            self.waiting = true;
        }
        if kind(v) == "waiver"
            && let Some(cap) = v.get("limit").and_then(Value::as_str)
            && !cap.is_empty()
            && cap.chars().all(|c| c.is_ascii_lowercase() || c == '_')
            && let Some(to) = v.get("to").and_then(number_like)
        {
            self.raised.insert(cap.to_string(), to);
        }
        let st = step(v);
        if st == "ship"
            && let Some(t) = t
        {
            self.shipped = Some(t);
        }
        if st == "brief" && crate::color_rank(result(v)).is_some() {
            self.color = result(v).to_string();
        }
        match st {
            "review" => self.fixing_checks = v["fix_role"] == "checks",
            "checks" if result(v) == "stopped" || v.get("stop").is_some() => {
                self.fixing_checks = false
            }
            "brief" | "build" | "ship" => self.fixing_checks = false,
            _ => {}
        }
        // What the last of these steps leaves waiting to be judged.
        let waits = |yes: bool, what: &str| if yes { what.into() } else { String::new() };
        match st {
            "brief" | "checks" | "ship" => self.next_up.clear(),
            "build" => self.next_up = waits(!any_kv(v, "result", "stopped"), "gates"),
            "gates" => self.next_up = waits(!any_kv(v, "result", "fail"), "review"),
            "review" => self.next_up = waits(any_kv(v, "verdict", "pass"), "ship"),
            _ => {}
        }
        self.used(v, late);
    }

    /// An event a correction voided decides nothing, but what it used was used.
    fn take_void(&mut self, v: &Value, late: &[String]) {
        if let Some(t) = ts(v) {
            self.first = Some(self.first.map_or(t, |f| f.min(t)));
        }
        self.used(v, late);
    }

    /// What an event used, voided or not: a launch, a time sent back, the seconds of its checks
    /// and its tokens.
    fn used(&mut self, v: &Value, late: &[String]) {
        match step(v) {
            "build" => {
                self.builds += 1;
                self.seal_merges += i64::from(v["seal_only"] == true);
            }
            "checks" => self.checks += 1,
            "review" if any_kv(v, "verdict", "fix") => self.review += 1,
            "gates" => {
                if any_kv(v, "result", "fail") {
                    self.gates += 1;
                }
                let (each, before, late_ran) = gates_cost(v, late);
                if late_ran {
                    self.late_runs += 1;
                }
                self.most_each = self.most_each.max(each);
                self.most_before = self.most_before.max(before);
            }
            _ => {}
        }
        self.tokens += tokens_in(v);
    }

    /// The minutes of work up to `now`, or to the ship, without the waits for a person.
    fn minutes(&self, now: i64) -> i64 {
        let end = match self.shipped {
            Some(s) => s,
            None if self.waiting => self.asked,
            None => now,
        };
        match self.first {
            None => 0,
            Some(f) => ((end - f - self.waited) as f64 / 60.0).trunc() as i64,
        }
    }
}

/// Count a change's events up to `now` (the time of the count, or of the end of a closed log).
#[cfg(test)]
pub fn count(bp: &Blueprint, events: &[&Value], now: i64) -> Limits {
    count_with(bp, "a-change", events, &[], now)
}

/// Count a log's entries up to `now`: the events that count, and what the voided ones used.
pub fn count_entries(bp: &Blueprint, id: &str, entries: &[crate::log::Entry], now: i64) -> Limits {
    let events = crate::log::events(entries);
    let values: Vec<&Value> = events.iter().map(|(_, v)| *v).collect();
    let void = crate::log::voided_events(entries);
    let void: Vec<&Value> = void.iter().map(|(_, v)| *v).collect();
    count_with(bp, id, &values, &void, now)
}

/// The same, with the events a correction voided. They decide nothing: no color, no cap raised, no
/// build waiting to be judged, no wait for a person. But what they used was used, and it still
/// counts: a round, a build sent back, a run before the review, its seconds and its tokens.
/// Otherwise voiding a round would give it back, and a cap would hold nothing.
pub fn count_with(
    bp: &Blueprint,
    id: &str,
    events: &[&Value],
    void: &[&Value],
    now: i64,
) -> Limits {
    let late: Vec<String> = bp
        .checks()
        .into_iter()
        .filter(|c| c.before_review())
        .map(|c| c.id)
        .collect();
    let mut l = Limits::default();
    let mut t = Tally::default();
    for v in events {
        t.take(v, &late);
    }
    for v in void {
        t.take_void(v, &late);
    }
    let minutes = t.minutes(now);
    let Tally {
        waited,
        builds,
        checks,
        review,
        gates,
        late_runs,
        tokens,
        most_each,
        most_before,
        mut next_up,
        color,
        raised,
        ..
    } = t;
    let cap_of = |key: &str| -> (Option<i64>, bool) {
        if let Some(r) = raised.get(key) {
            return (Some(*r), true);
        }
        (bp.cap(&color, key), false)
    };
    let judged = !next_up.is_empty();
    let mut stop = vec![];
    let mut spent = false;
    let mut lines = vec![];
    let mut report =
        |name: &'static str, used: f64, key: &'static str, past: bool, judged: bool| {
            let (cap, raised) = cap_of(key);
            let full = cap.is_some_and(|c| {
                if past {
                    used > c as f64
                } else {
                    used >= c as f64
                }
            });
            let held = full && judged;
            if held {
                spent = true;
            } else if full {
                stop.push(name.to_string());
            }
            lines.push(Line {
                name,
                key,
                used,
                cap,
                raised,
                full,
                tier: false,
            });
        };
    // A round is a launch of whoever builds, after the first: gdd-build in a change, and
    // gdd-checks in a log of standing checks, where the checks are what is built. The checks
    // sealed again for a build go with it.
    let setup = crate::state::log_kind(id) == crate::state::LogKind::Setup;
    let launches = if setup {
        checks
    } else {
        builds - t.seal_merges
    };
    let rounds = (launches - 1).max(0);
    report(
        "rounds",
        rounds as f64,
        "rounds_per_change",
        false,
        judged || t.fixing_checks,
    );
    report(
        "sent back by the checks",
        gates as f64,
        "fix_rounds_per_gate",
        true,
        judged,
    );
    report(
        "sent back by the review",
        review as f64,
        "fix_rounds_per_gate",
        true,
        judged,
    );
    report(
        "minutes",
        minutes as f64,
        "minutes_per_change",
        false,
        judged,
    );
    if cap_of("runs_before_review").0.is_some() {
        report(
            "runs before the review",
            late_runs as f64,
            "runs_before_review",
            false,
            judged && next_up != "gates",
        );
    }
    if cap_of("tokens_per_change").0.is_some() {
        report("tokens", tokens as f64, "tokens_per_change", false, judged);
    }
    let mut slow = vec![];
    for (name, used, key) in [
        (
            "seconds of the checks of a round",
            most_each,
            "seconds_each_round",
        ),
        (
            "seconds of the checks before the review",
            most_before,
            "seconds_before_review",
        ),
    ] {
        let (cap, raised) = cap_of(key);
        let Some(c) = cap else { continue };
        let past = used > c as f64;
        if past {
            slow.push(name.to_string());
        }
        lines.push(Line {
            name,
            key,
            used,
            cap,
            raised,
            full: past,
            tier: true,
        });
    }
    if !stop.is_empty() {
        next_up.clear();
    }
    l.color = color;
    l.lines = lines;
    l.stop = stop;
    l.slow = slow;
    l.judge = next_up;
    l.spent = spent;
    l.minutes = minutes;
    l.waited = waited;
    l.builds = builds;
    l.checks = checks;
    l.raised = raised;
    l
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bp() -> Blueprint {
        let t = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../skills/gdd-prepare/assets/blueprint.md"),
        )
        .unwrap();
        let t = t.replace(
            "  - id: review\n",
            "  - id: e2e\n    type: machine\n    command: sh e2e.sh\n    runs: before review\n  - id: review\n",
        );
        Blueprint::parse(&t).unwrap()
    }

    fn ev(minute: i64, rest: &str) -> Value {
        serde_json::from_str(&format!(
            "{{\"contract\":\"gdd/1\",\"ts\":\"2026-11-10T{:02}:{:02}:00Z\",\"brief\":\"x\",{}}}",
            10 + minute / 60,
            minute % 60,
            rest
        ))
        .unwrap()
    }

    const BRIEF: &str = r#""event":"step","step":"brief","report":{"skill":"gdd-brief","result":"yellow","tokens":10}"#;
    const ASK: &str = r#""event":"ask","of":"decision","what":"a rounding""#;
    const APPROVAL: &str = r#""event":"approval","who":"Person","of":"brief""#;
    const CHECKS: &str = r#""event":"step","step":"checks","report":{"result":"sealed"}"#;
    const BUILD: &str = r#""event":"step","step":"build","report":{"result":"done"}"#;
    const FAIL: &str = r#""event":"step","step":"gates","gates":[{"check":"seal","seconds":1,"result":"pass"},{"check":"tests","seconds":20,"result":"fail"}]"#;

    fn run(events: &[Value], now_minute: i64) -> Limits {
        let refs: Vec<&Value> = events.iter().collect();
        let now = crate::time::parse("2026-11-10T10:00:00Z").unwrap() + now_minute * 60;
        count(&bp(), &refs, now)
    }

    #[test]
    fn waits_for_a_person_are_left_out() {
        let l = run(
            &[
                ev(0, BRIEF),
                ev(5, ASK),
                ev(65, APPROVAL),
                ev(70, CHECKS),
                ev(75, BUILD),
                ev(80, ASK),
            ],
            200,
        );
        let text = l.text();
        assert!(text.contains(&"color yellow".to_string()));
        assert!(text.contains(&"rounds 0 of 3".to_string()));
        assert!(text.contains(&"minutes 20 of 45".to_string()), "{text:?}");
        assert!(l.stop.is_empty());
        assert_eq!(l.judge, "gates");
    }

    #[test]
    fn a_finished_build_is_still_judged() {
        let mut e = vec![ev(0, BRIEF), ev(1, CHECKS)];
        for m in [2, 4, 6, 8] {
            e.push(ev(m, BUILD));
            e.push(ev(m + 1, FAIL));
        }
        let mut waiting = e[..e.len() - 1].to_vec();
        waiting.push(ev(9, ASK));
        let l = run(&waiting, 9);
        assert!(l.stop.is_empty());
        assert_eq!(l.judge, "gates");
        assert!(l.spent);
        assert!(l.text().iter().any(|t| t == "rounds 3 of 3, no room left"));
        e.push(ev(10, ASK));
        let l = run(&e, 10);
        assert!(l.stop.contains(&"rounds".to_string()));
        assert!(l.stop.contains(&"sent back by the checks".to_string()));
        assert_eq!(l.judge, "");
    }

    #[test]
    fn seal_merges_spend_review_returns_without_spending_build_rounds() {
        let mut events = vec![ev(0, BRIEF), ev(1, CHECKS), ev(2, BUILD)];
        for round in 1..=3 {
            events.push(ev(
                3 * round,
                r#""event":"step","step":"review","verdict":"fix","fix_role":"checks""#,
            ));
            let limits = run(&events, 3 * round);
            assert_eq!(
                limits.line("rounds_per_change", "rounds").unwrap().used,
                0.0
            );
            assert_eq!(
                limits.stop.contains(&"sent back by the review".to_string()),
                round == 3
            );
            if round < 3 {
                events.push(ev(3 * round + 1, CHECKS));
                events.push(ev(
                    3 * round + 2,
                    r#""event":"step","step":"build","report":{"result":"done"},"seal_only":true"#,
                ));
            }
        }
    }

    #[test]
    fn a_waiver_raises_a_cap() {
        let w = r#""event":"waiver","who":"Person","check":"minutes_per_change","why":"x","backlog":"b","limit":"minutes_per_change","to":"60""#;
        let l = run(
            &[
                ev(0, BRIEF),
                ev(1, CHECKS),
                ev(2, BUILD),
                ev(50, FAIL),
                ev(51, w),
                ev(53, ASK),
            ],
            99,
        );
        assert!(l.stop.is_empty(), "{:?}", l.text());
        assert!(
            l.text()
                .contains(&"minutes 53 of 60, raised by a waiver".to_string())
        );
    }
}
