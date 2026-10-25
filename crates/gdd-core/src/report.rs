//! The token report: the agent calls of the log added up by some of their fields.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Serialize, Serializer};

use crate::log::Record;

/// What a group of agent calls adds up to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct Totals {
    /// Agent calls.
    pub calls: u64,
    /// Input tokens that were not read from the cache.
    pub tokens_in: u64,
    /// Input tokens read from the cache.
    pub tokens_cache: u64,
    /// Output tokens.
    pub tokens_out: u64,
    /// Cost in US dollars.
    pub cost_usd: f64,
    /// Time, in seconds.
    pub duration_s: f64,
}

impl Totals {
    /// The totals of one agent call.
    fn of(call: &Record) -> Totals {
        Totals {
            calls: 1,
            tokens_in: call.tokens_in,
            tokens_cache: call.tokens_cache,
            tokens_out: call.tokens_out,
            cost_usd: call.cost_usd,
            duration_s: call.duration_s,
        }
    }

    /// Adds `other` to these totals. Counts stop at their maximum instead of overflowing.
    fn add(&mut self, other: &Totals) {
        self.calls = self.calls.saturating_add(other.calls);
        self.tokens_in = self.tokens_in.saturating_add(other.tokens_in);
        self.tokens_cache = self.tokens_cache.saturating_add(other.tokens_cache);
        self.tokens_out = self.tokens_out.saturating_add(other.tokens_out);
        self.cost_usd += other.cost_usd;
        self.duration_s += other.duration_s;
    }
}

/// The agent calls of a log grouped by some of their fields, and the human stops.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// Fields the calls are grouped by, in the order asked.
    pub by: Vec<String>,
    /// What the calls of each group add up to, by the values of their `by` fields (see
    /// [`Record::text`]), sorted by those values.
    pub groups: BTreeMap<Vec<String>, Totals>,
    /// Records of a human (role `human`): the stops at human gates.
    pub human_stops: usize,
}

impl Report {
    /// Adds up the agent calls of `records` (see [`Record::is_agent_call`]) grouped by the
    /// fields `by`, and counts the human stops. With `change`, only that change's records count.
    pub fn new(records: &[Record], by: &[String], change: Option<&str>) -> Report {
        let mut groups = BTreeMap::<Vec<String>, Totals>::new();
        let mut human_stops = 0;
        for record in records
            .iter()
            .filter(|r| change.is_none_or(|c| r.text("change") == c))
        {
            if record.is_agent_call() {
                let key = by.iter().map(|field| record.text(field)).collect();
                groups.entry(key).or_default().add(&Totals::of(record));
            }
            if record.text("role") == "human" {
                human_stops += 1;
            }
        }
        Report {
            by: by.to_vec(),
            groups,
            human_stops,
        }
    }

    /// What all the agent calls of the report add up to.
    pub fn total(&self) -> Totals {
        let mut total = Totals::default();
        for totals in self.groups.values() {
            total.add(totals);
        }
        total
    }
}

/// The report as the human reads it: a header, one line per group, the total and the human
/// stops. Without agent calls, one line that says so.
impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.groups.is_empty() {
            return f.write_str("No agent calls in the log yet.");
        }
        writeln!(
            f,
            "{} -> calls · tokens in · cache · out · cost USD · seconds",
            self.by.join(" · ")
        )?;
        for (key, t) in &self.groups {
            writeln!(
                f,
                "{} -> {} · {} · {} · {} · {:.4} · {:.0}",
                key.join(" · "),
                t.calls,
                thousands(t.tokens_in),
                thousands(t.tokens_cache),
                thousands(t.tokens_out),
                t.cost_usd,
                t.duration_s
            )?;
        }
        let total = self.total();
        writeln!(
            f,
            "TOTAL -> in {} · cache {} · out {} · {:.4} USD",
            thousands(total.tokens_in),
            thousands(total.tokens_cache),
            thousands(total.tokens_out),
            total.cost_usd
        )?;
        write!(f, "Human stops logged: {}", self.human_stops)
    }
}

/// As JSON, the report is a list with one object per group: the values of its `by` fields,
/// then its totals.
impl Serialize for Report {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.groups.iter().map(|(values, totals)| Entry {
            key: Key {
                by: &self.by,
                values,
            },
            totals,
        }))
    }
}

#[derive(Serialize)]
struct Entry<'a> {
    #[serde(flatten)]
    key: Key<'a>,
    #[serde(flatten)]
    totals: &'a Totals,
}

/// The values of the `by` fields of a group, by field.
struct Key<'a> {
    by: &'a [String],
    values: &'a [String],
}

impl Serialize for Key<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.by.iter().zip(self.values))
    }
}

/// A whole number with a comma every three digits: `1,234,567`.
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log;

    const LOG: &str = r#"{"change": "a", "phase": "2-build", "role": "builder-build", "model": "opus", "tokens_in": 100, "tokens_cache": 400, "tokens_out": 20, "cost_usd": 0.01, "duration_s": 5}
{"change": "a", "phase": "3-integrate", "role": "reviewer", "model": "gpt-x", "tokens_in": 1000, "tokens_out": 40, "duration_s": 9.4}
{"change": "b", "phase": "2-build", "role": "builder-build", "model": "sonnet", "tokens_in": 1500000, "tokens_cache": 2500, "tokens_out": 5, "cost_usd": 0.25, "duration_s": 2.25}
{"change": "a", "phase": "2-build", "role": "machine", "agent": "checks", "tokens_cache": 7, "duration_s": 3, "result": "ko"}
{"change": "b", "gate": "G1", "role": "human", "result": "ok"}
{"change": "a", "role": "builder-test", "tokens_out": 3}
{"change": "a", "gate": "G4", "role": "human", "result": "ok"}"#;

    fn by(fields: &str) -> Vec<String> {
        fields.split(',').map(str::to_string).collect()
    }

    fn report(fields: &str, change: Option<&str>) -> Report {
        Report::new(&log::parse(LOG).unwrap(), &by(fields), change)
    }

    fn key(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| v.to_string()).collect()
    }

    #[test]
    fn calls_are_added_up_by_their_fields() {
        let r = report("role", None);
        assert_eq!(
            r.groups.keys().cloned().collect::<Vec<_>>(),
            [
                key(&["builder-build"]),
                key(&["builder-test"]),
                key(&["reviewer"])
            ]
        );
        assert_eq!(
            r.groups[&key(&["builder-build"])],
            Totals {
                calls: 2,
                tokens_in: 1_500_100,
                tokens_cache: 2900,
                tokens_out: 25,
                cost_usd: 0.01 + 0.25,
                duration_s: 7.25,
            }
        );
        assert_eq!(r.groups[&key(&["builder-test"])].tokens_out, 3);
        assert_eq!(r.human_stops, 2);
    }

    #[test]
    fn records_without_tokens_in_or_out_are_not_calls() {
        let r = report("agent", None);
        assert!(!r.groups.contains_key(&key(&["checks"])));
        assert_eq!(r.total().calls, 4);
    }

    #[test]
    fn a_field_left_out_groups_as_a_dash() {
        let r = report("agent,role", None);
        assert!(r.groups.contains_key(&key(&["-", "reviewer"])));
    }

    #[test]
    fn one_change_counts_only_its_records() {
        let r = report("model", Some("a"));
        assert_eq!(
            r.groups.keys().cloned().collect::<Vec<_>>(),
            [key(&["-"]), key(&["gpt-x"]), key(&["opus"])]
        );
        assert_eq!(r.human_stops, 1);
        let r = report("model", Some("b"));
        assert_eq!(r.groups.len(), 1);
        assert_eq!(r.human_stops, 1);
        let r = report("model", Some("zzz"));
        assert!(r.groups.is_empty());
        assert_eq!(r.human_stops, 0);
    }

    #[test]
    fn the_total_adds_every_group() {
        let t = report("change,model", None).total();
        assert_eq!(
            (t.calls, t.tokens_in, t.tokens_cache, t.tokens_out),
            (4, 1_501_100, 2900, 68)
        );
        assert!((t.cost_usd - 0.26).abs() < 1e-12);
        assert!((t.duration_s - 16.65).abs() < 1e-12);
    }

    #[test]
    fn counts_stop_at_their_maximum() {
        let mut t = Totals {
            calls: u64::MAX,
            tokens_in: u64::MAX - 1,
            tokens_cache: u64::MAX,
            tokens_out: u64::MAX,
            ..Totals::default()
        };
        t.add(&Totals {
            calls: 1,
            tokens_in: 5,
            tokens_cache: 1,
            tokens_out: 1,
            cost_usd: 1.5,
            duration_s: 2.0,
        });
        assert_eq!(
            t,
            Totals {
                calls: u64::MAX,
                tokens_in: u64::MAX,
                tokens_cache: u64::MAX,
                tokens_out: u64::MAX,
                cost_usd: 1.5,
                duration_s: 2.0,
            }
        );
    }

    #[test]
    fn the_text_has_a_header_a_line_per_group_the_total_and_the_human_stops() {
        assert_eq!(
            report("change,role", None).to_string(),
            "change · role -> calls · tokens in · cache · out · cost USD · seconds\n\
             a · builder-build -> 1 · 100 · 400 · 20 · 0.0100 · 5\n\
             a · builder-test -> 1 · 0 · 0 · 3 · 0.0000 · 0\n\
             a · reviewer -> 1 · 1,000 · 0 · 40 · 0.0000 · 9\n\
             b · builder-build -> 1 · 1,500,000 · 2,500 · 5 · 0.2500 · 2\n\
             TOTAL -> in 1,501,100 · cache 2,900 · out 68 · 0.2600 USD\n\
             Human stops logged: 2"
        );
    }

    #[test]
    fn without_calls_the_text_says_so() {
        assert_eq!(
            report("role", Some("zzz")).to_string(),
            "No agent calls in the log yet."
        );
        let humans = log::parse(r#"{"role": "human"}"#).unwrap();
        assert_eq!(
            Report::new(&humans, &by("role"), None).to_string(),
            "No agent calls in the log yet."
        );
    }

    #[test]
    fn as_json_each_group_has_its_fields_then_its_totals() {
        let json = serde_json::to_string(&report("model,role", Some("a"))).unwrap();
        assert_eq!(
            json,
            concat!(
                r#"[{"model":"-","role":"builder-test","calls":1,"tokens_in":0,"tokens_cache":0,"tokens_out":3,"cost_usd":0.0,"duration_s":0.0},"#,
                r#"{"model":"gpt-x","role":"reviewer","calls":1,"tokens_in":1000,"tokens_cache":0,"tokens_out":40,"cost_usd":0.0,"duration_s":9.4},"#,
                r#"{"model":"opus","role":"builder-build","calls":1,"tokens_in":100,"tokens_cache":400,"tokens_out":20,"cost_usd":0.01,"duration_s":5.0}]"#
            )
        );
        let empty = serde_json::to_string(&report("role", Some("zzz"))).unwrap();
        assert_eq!(empty, "[]");
    }

    #[test]
    fn thousands_get_a_comma() {
        for (n, text) in [
            (0, "0"),
            (7, "7"),
            (100, "100"),
            (999, "999"),
            (1000, "1,000"),
            (12_345, "12,345"),
            (123_456, "123,456"),
            (1_234_567, "1,234,567"),
        ] {
            assert_eq!(thousands(n), text);
        }
    }
}
