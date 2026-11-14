//! What the blueprint's limits still allow one change, counted from its log.
//!
//! It counts exactly as the contract's counter, `gdd/limits.sh`, does: the same caps, the same
//! used values, the same `stop`, `slow` and `judge`, and the same lines. It is pure: the events and
//! the time it counts to are given.

use crate::blueprint::Blueprint;
use crate::log::{any_kv, is_step, kind, number_like, result, step, tokens_in, ts};
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
    /// No room left, but kept out of `stop` because a finished build waits to be judged.
    pub held: bool,
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
    pub waiting: bool,
    pub builds: i64,
    pub checks: i64,
    pub raised: BTreeMap<String, i64>,
}

impl Limits {
    pub fn line(&self, key: &str, name: &str) -> Option<&Line> {
        self.lines.iter().find(|l| l.key == key && l.name == name)
    }

    /// The caps with no room left, those in `stop` and those held for a build being judged.
    pub fn no_room(&self) -> Vec<&'static str> {
        self.lines
            .iter()
            .filter(|l| l.full && !l.tier)
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

/// Count a change's events up to `now` (the time of the count, or of the end of a closed log).
pub fn count(bp: &Blueprint, events: &[&Value], now: i64) -> Limits {
    let late: Vec<String> = bp
        .checks()
        .into_iter()
        .filter(|c| c.before_review())
        .map(|c| c.id)
        .collect();
    let mut l = Limits::default();
    let (mut first, mut asked, mut waiting, mut waited) = (None::<i64>, 0i64, false, 0i64);
    // A change that shipped is over: its minutes stop there, not at the clock.
    let mut shipped = None::<i64>;
    let (mut builds, mut checks, mut review, mut gates, mut late_runs, mut tokens) =
        (0i64, 0i64, 0i64, 0i64, 0i64, 0i64);
    let (mut most_each, mut most_before) = (0f64, 0f64);
    let mut next_up = String::new();
    let mut color = String::new();
    let mut raised: BTreeMap<String, i64> = BTreeMap::new();

    for v in events {
        let t = ts(v);
        if let Some(t) = t {
            if first.is_none() {
                first = Some(t);
            }
            if waiting {
                waited += t - asked;
                waiting = false;
            }
        }
        if kind(v) == "ask"
            && let Some(t) = t
        {
            asked = t;
            waiting = true;
        }
        if kind(v) == "waiver"
            && let Some(cap) = v.get("limit").and_then(Value::as_str)
            && !cap.is_empty()
            && cap.chars().all(|c| c.is_ascii_lowercase() || c == '_')
            && let Some(to) = v.get("to").and_then(number_like)
        {
            raised.insert(cap.to_string(), to);
        }
        let st = step(v);
        if st == "ship"
            && let Some(t) = t
        {
            shipped = Some(t);
        }
        if st == "brief" && crate::color_rank(result(v)).is_some() {
            color = result(v).to_string();
        }
        if matches!(st, "brief" | "checks" | "ship") {
            next_up.clear();
        }
        if st == "build" {
            builds += 1;
            next_up = if any_kv(v, "result", "stopped") {
                String::new()
            } else {
                "gates".into()
            };
        }
        if st == "checks" {
            checks += 1;
        }
        if st == "review" {
            next_up = if any_kv(v, "verdict", "pass") {
                "ship".into()
            } else {
                String::new()
            };
            if any_kv(v, "verdict", "fix") {
                review += 1;
            }
        }
        if is_step(v, "gates") {
            let failed = any_kv(v, "result", "fail");
            next_up = if failed {
                String::new()
            } else {
                "review".into()
            };
            if failed {
                gates += 1;
            }
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
            if late_ran {
                late_runs += 1;
            }
            most_each = most_each.max(each);
            most_before = most_before.max(before);
        }
        tokens += tokens_in(v);
    }

    let end = match shipped {
        Some(s) => s,
        None if waiting => asked,
        None => now,
    };
    let minutes = match first {
        None => 0,
        Some(f) => ((end - f - waited) as f64 / 60.0).trunc() as i64,
    };
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
                held,
                tier: false,
            });
        };
    let rounds =
        (if builds > 1 { builds - 1 } else { 0 }) + (if checks > 1 { checks - 1 } else { 0 });
    report("rounds", rounds as f64, "rounds_per_change", false, judged);
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
            held: false,
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
    l.waiting = waiting;
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
