//! The token report: the agent calls of the log added up by some of their fields.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Serialize, Serializer};

use crate::agents::models;
use crate::delivery;
use crate::flow::{self, DAY, Line, Wait};
use crate::layout::Layout;
use crate::log::{CostBasis, Record};

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
    /// Cost in US dollars of the calls whose cost is known.
    pub cost_usd: f64,
    /// Whether an estimate from the prices of the models file (`tminus/models.toml`) is in the
    /// cost.
    pub cost_estimated: bool,
    /// Calls whose cost is unknown: their agent reported none, and their model has no price.
    pub calls_without_price: u64,
    /// Time, in seconds.
    pub duration_s: f64,
}

impl Totals {
    /// The totals of one agent call.
    fn of(call: &Record) -> Totals {
        let basis = call.cost_basis();
        Totals {
            calls: 1,
            tokens_in: call.tokens_in,
            tokens_cache: call.tokens_cache,
            tokens_out: call.tokens_out,
            cost_usd: call.cost_usd,
            cost_estimated: basis == CostBasis::Estimated,
            calls_without_price: u64::from(basis == CostBasis::Unknown),
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
        self.cost_estimated |= other.cost_estimated;
        let without = other.calls_without_price;
        self.calls_without_price = self.calls_without_price.saturating_add(without);
        self.duration_s += other.duration_s;
    }

    /// The cost of the calls whose cost is known, to the fourth decimal, with `~` first when an
    /// estimate is in it; `None` when no call has a known cost.
    fn known_cost(&self) -> Option<String> {
        let mark = if self.cost_estimated { "~" } else { "" };
        let known = self.calls > self.calls_without_price;
        known.then(|| format!("{mark}{:.4}", self.cost_usd))
    }

    /// The cost of a group as its line in the report shows it: the known cost (see
    /// [`Totals::known_cost`]), then `+ unknown` when some call has no price; `unknown` when none
    /// has a known cost.
    fn cost_cell(&self) -> String {
        match (self.known_cost(), self.calls_without_price) {
            (Some(known), 0) => known,
            (Some(known), _) => format!("{known} + unknown"),
            (None, _) => "unknown".to_string(),
        }
    }

    /// The cost of all the calls as the line of the total shows it: in USD, and how many calls
    /// have no price, when some have none.
    fn cost_total(&self) -> String {
        let without = self.calls_without_price;
        let unknown = format!("unknown ({without} call(s) with no price)");
        match (self.known_cost(), without) {
            (Some(known), 0) => format!("{known} USD"),
            (Some(known), _) => format!("{known} USD + {unknown}"),
            (None, _) => unknown,
        }
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
    /// The models of the calls whose cost is unknown, sorted.
    pub unpriced: BTreeSet<String>,
    /// The layout of the project, whose models file the report names (see
    /// [`Report::in_layout`]): the new one, unless said otherwise.
    layout: Layout,
}

impl Report {
    /// Adds up the agent calls of `records` (see [`Record::is_agent_call`]) grouped by the
    /// fields `by`, and counts the human stops. With `change`, only that change's records count.
    pub fn new(records: &[Record], by: &[String], change: Option<&str>) -> Report {
        let mut groups = BTreeMap::<Vec<String>, Totals>::new();
        let mut human_stops = 0;
        let mut unpriced = BTreeSet::new();
        for record in records
            .iter()
            .filter(|r| change.is_none_or(|c| r.text("change") == c))
        {
            if record.is_agent_call() {
                let key = by.iter().map(|field| record.text(field)).collect();
                groups.entry(key).or_default().add(&Totals::of(record));
                if record.cost_basis() == CostBasis::Unknown {
                    unpriced.insert(record.text("model"));
                }
            }
            if record.text("role") == "human" {
                human_stops += 1;
            }
        }
        Report {
            by: by.to_vec(),
            groups,
            human_stops,
            unpriced,
            layout: Layout::Tminus,
        }
    }

    /// This report for a project of `layout`: it names the models file as that project does
    /// (`gdd/models.toml` in a project set up before the rename).
    pub fn in_layout(self, layout: Layout) -> Report {
        Report { layout, ..self }
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

/// The report as the human reads it: a header, one line per group, the total, what its marks
/// mean when it has them, and the human stops. Without agent calls, one line that says so.
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
                "{} -> {} · {} · {} · {} · {} · {:.0}",
                key.join(" · "),
                t.calls,
                thousands(t.tokens_in),
                thousands(t.tokens_cache),
                thousands(t.tokens_out),
                t.cost_cell(),
                t.duration_s
            )?;
        }
        let total = self.total();
        writeln!(
            f,
            "TOTAL -> in {} · cache {} · out {} · {}",
            thousands(total.tokens_in),
            thousands(total.tokens_cache),
            thousands(total.tokens_out),
            total.cost_total()
        )?;
        let file = self.layout.path(models::PATH);
        if total.cost_estimated {
            writeln!(f, "~ estimated from the prices in {file}")?;
        }
        if !self.unpriced.is_empty() {
            let models: Vec<&str> = self.unpriced.iter().map(String::as_str).collect();
            let models = models.join(", ");
            writeln!(f, "unknown: no price in {file} for {models}")?;
        }
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

/// A moment named on the command line: the last second of a day `YYYY-MM-DD` (UTC), or a time as
/// the log writes it; seconds since 1970-01-01T00:00:00 UTC. `None` for anything else.
pub fn until(text: &str) -> Option<i64> {
    match flow::day(text) {
        Some(day) => Some(day * DAY + DAY - 1),
        None => flow::seconds(text),
    }
}

/// What one change kept of what the method promises.
#[derive(Debug, Clone)]
struct Kept {
    /// Seconds of its first line.
    start: i64,
    /// Its type, `-` without a record.
    kind: String,
    /// Its color, `-` never prepared.
    color: String,
    closed: bool,
    stops: usize,
    parks: usize,
    /// Seconds waiting for the human at the sitting.
    sitting: f64,
    /// Seconds waiting for the human at G4.
    g4: f64,
    /// Its agent calls.
    calls: Totals,
}

impl Kept {
    fn new(lines: &[&Line], types: &BTreeMap<String, String>) -> Kept {
        let records: Vec<&Record> = lines.iter().map(|line| &line.record).collect();
        let id = records
            .first()
            .map_or_else(String::new, |r| r.text("change"));
        let g4 = records.iter().filter(|r| flow::human_at(r, "G4")).count();
        let mut calls = Totals::default();
        for call in records.iter().filter(|r| r.is_agent_call()) {
            calls.add(&Totals::of(call));
        }
        let waits = flow::waits(lines);
        let waited = |wait: &Wait| waits.get(wait).copied().unwrap_or(0.0);
        Kept {
            start: lines.first().map_or(0, |line| line.at),
            kind: types.get(&id).cloned().unwrap_or_else(|| "-".to_string()),
            color: color(&records),
            closed: flow::state(&records) == "closed",
            stops: g4 + usize::from(delivery::sat(&records)),
            parks: records
                .iter()
                .filter(|r| flow::is(r, "result", "parked"))
                .count(),
            sitting: waited(&Wait::Sitting),
            g4: waited(&Wait::Human),
            calls,
        }
    }

    /// Whether it closed without stopping the human and without parking.
    fn free(&self) -> bool {
        self.closed && self.stops == 0 && self.parks == 0
    }
}

/// The color of a change: the first part of the note of its last `prepared` line; `-` if it was
/// never prepared.
fn color(records: &[&Record]) -> String {
    let prepared = records.iter().rfind(|r| flow::is(r, "result", "prepared"));
    let note = prepared.map_or_else(String::new, |r| r.text("note"));
    let first = note.split(" · ").next().unwrap_or_default().trim();
    let color = if first.is_empty() { "-" } else { first };
    color.to_string()
}

/// What a group of changes adds up to.
#[derive(Default)]
struct Block {
    changes: usize,
    closed: usize,
    free: usize,
    stops: usize,
    parks: usize,
    sitting: f64,
    g4: f64,
    calls: Totals,
}

impl Block {
    fn of(kept: &[&Kept]) -> Block {
        let mut block = Block::default();
        for k in kept {
            block.changes += 1;
            block.closed += usize::from(k.closed);
            block.free += usize::from(k.free());
            block.stops += k.stops;
            block.parks += k.parks;
            block.sitting += k.sitting;
            block.g4 += k.g4;
            block.calls.add(&k.calls);
        }
        block
    }

    /// Its five lines, under the `label` of the group.
    fn lines(&self, label: &str) -> String {
        let n = self.changes;
        let total = self.sitting + self.g4;
        let each = |t: u64| thousands((2 * t + n as u64) / (2 * n as u64));
        let t = &self.calls;
        format!(
            "{label}: {n} change(s) · {} closed · {} ended without stopping you\n  \
             Stops: {} · parked {}\n  \
             Waiting for you: {} per change ({} in {n}) · at the sitting {} · at G4 {}\n  \
             Tokens per change: in {} · cache {} · out {}\n  \
             Cost: {}",
            self.closed,
            self.free,
            delivery::per_change(self.stops, n),
            delivery::per_change(self.parks, n),
            flow::duration(total / n as f64),
            flow::duration(total),
            flow::duration(self.sitting),
            flow::duration(self.g4),
            each(t.tokens_in),
            each(t.tokens_cache),
            each(t.tokens_out),
            self.cost(),
        )
    }

    /// The cost line without its indentation.
    fn cost(&self) -> String {
        let t = &self.calls;
        if t.calls == 0 {
            return "no agent call yet".to_string();
        }
        let each = match t.known_cost() {
            Some(_) => {
                let mark = if t.cost_estimated { "~" } else { "" };
                format!("{mark}{:.4} USD", t.cost_usd / self.changes as f64)
            }
            None => "unknown".to_string(),
        };
        format!("{each} per change · in all {}", t.cost_total())
    }
}

/// What the method promises to keep low, for every change of a log and by type and color: the
/// changes that ended without stopping the human, the stops, the time each waited for the human,
/// and tokens and cost.
#[derive(Debug, Clone)]
pub struct Promises {
    /// The changes kept, in the order they were created.
    changes: Vec<Kept>,
}

impl Promises {
    /// The promises of the changes of `lines` as they were at the end of `until` (the lines after
    /// it do not exist), `types` by change id; only the changes whose first line is on or after
    /// the day `since` (see [`flow::day`]) when given. Lines of no change (`-`) are left out.
    pub fn new(
        lines: &[Line],
        types: &BTreeMap<String, String>,
        since: Option<i64>,
        until: Option<i64>,
    ) -> Promises {
        let mut groups: Vec<(String, Vec<&Line>)> = Vec::new();
        for line in lines.iter().filter(|l| until.is_none_or(|end| l.at <= end)) {
            let id = line.record.text("change");
            if id == "-" {
                continue;
            }
            match groups.iter_mut().find(|(known, _)| *known == id) {
                Some((_, group)) => group.push(line),
                None => groups.push((id, vec![line])),
            }
        }
        let changes = groups
            .iter()
            .map(|(_, group)| Kept::new(group, types))
            .filter(|kept| since.is_none_or(|day| kept.start >= day * DAY))
            .collect();
        Promises { changes }
    }
}

impl fmt::Display for Promises {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let starts = self.changes.iter().map(|kept| kept.start);
        let (Some(first), Some(last)) = (starts.clone().min(), starts.max()) else {
            return f.write_str("No change in the log yet.");
        };
        let day = |at: i64| delivery::date(at.div_euclid(DAY));
        let n = self.changes.len();
        writeln!(
            f,
            "Promises · {n} change(s) created from {} to {}",
            day(first),
            day(last)
        )?;
        let all: Vec<&Kept> = self.changes.iter().collect();
        write!(f, "{}", Block::of(&all).lines("All"))?;
        let mut groups = BTreeMap::<(&str, &str), Vec<&Kept>>::new();
        for kept in &self.changes {
            let key = (kept.kind.as_str(), kept.color.as_str());
            groups.entry(key).or_default().push(kept);
        }
        for ((kind, color), kept) in groups {
            write!(
                f,
                "\n{}",
                Block::of(&kept).lines(&format!("{kind} · {color}"))
            )?;
        }
        if let Some(warning) = mostly_red(&self.changes) {
            write!(f, "\n{warning}")?;
        }
        Ok(())
    }
}

/// How many of the last changes that closed the report looks at to say whether almost every change
/// comes out red (see [`mostly_red`]).
const LAST_CLOSED: usize = 10;

/// How many of those [`LAST_CLOSED`] changes red are almost every change.
const ALMOST_ALL: usize = 8;

/// What the report says when at least [`ALMOST_ALL`] of the last [`LAST_CLOSED`] changes of
/// `changes` that closed, in the order they were created, came out red: the blueprint marks too
/// much as delicate. `None` when fewer of them are red, or fewer than [`LAST_CLOSED`] closed. A
/// warning: it stops nothing.
fn mostly_red(changes: &[Kept]) -> Option<String> {
    let closed: Vec<&Kept> = changes.iter().filter(|kept| kept.closed).collect();
    let from = closed.len().checked_sub(LAST_CLOSED)?;
    let red = closed[from..]
        .iter()
        .filter(|kept| kept.color == "red")
        .count();
    (red >= ALMOST_ALL).then(|| {
        format!(
            "Most changes come out red: {red} of the last {LAST_CLOSED} closed. If almost \
             everything is red, the blueprint is wrong: the zone map may mark too much as delicate."
        )
    })
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
                cost_estimated: false,
                calls_without_price: 0,
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
            calls_without_price: u64::MAX,
            ..Totals::default()
        };
        t.add(&Totals {
            calls: 1,
            tokens_in: 5,
            tokens_cache: 1,
            tokens_out: 1,
            cost_usd: 1.5,
            cost_estimated: true,
            calls_without_price: 1,
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
                cost_estimated: true,
                calls_without_price: u64::MAX,
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
                r#"[{"model":"-","role":"builder-test","calls":1,"tokens_in":0,"tokens_cache":0,"tokens_out":3,"cost_usd":0.0,"cost_estimated":false,"calls_without_price":0,"duration_s":0.0},"#,
                r#"{"model":"gpt-x","role":"reviewer","calls":1,"tokens_in":1000,"tokens_cache":0,"tokens_out":40,"cost_usd":0.0,"cost_estimated":false,"calls_without_price":0,"duration_s":9.4},"#,
                r#"{"model":"opus","role":"builder-build","calls":1,"tokens_in":100,"tokens_cache":400,"tokens_out":20,"cost_usd":0.01,"cost_estimated":false,"calls_without_price":0,"duration_s":5.0}]"#
            )
        );
        let empty = serde_json::to_string(&report("role", Some("zzz"))).unwrap();
        assert_eq!(empty, "[]");
        let with_prices = Report::new(&log::parse(PRICED).unwrap(), &by("role"), None);
        let json: serde_json::Value = serde_json::to_value(&with_prices).unwrap();
        let reviewer = &json[2];
        assert_eq!(reviewer["role"], "reviewer");
        assert_eq!(reviewer["cost_estimated"], true);
        assert_eq!(reviewer["calls_without_price"], 1);
        assert_eq!(json[1]["calls_without_price"], 2);
    }

    /// A log whose agents reported some costs, some were estimated from the prices of their
    /// models, and some are unknown.
    const PRICED: &str = r#"{"change": "a", "role": "builder-build", "model": "opus", "tokens_in": 100, "tokens_out": 20, "cost_usd": 0.01, "duration_s": 5}
{"change": "a", "role": "reviewer", "model": "gpt-x", "tokens_in": 50, "tokens_out": 25, "cost_usd": 0.0021, "cost_basis": "estimated", "duration_s": 9}
{"change": "a", "role": "reviewer", "model": "gpt-y", "tokens_in": 50, "tokens_out": 25, "cost_usd": 0.0, "cost_basis": "unknown", "duration_s": 1}
{"change": "b", "role": "orchestrator", "model": "sonnet", "tokens_in": 10, "tokens_out": 4, "cost_usd": 0.0, "cost_basis": "unknown"}
{"change": "b", "role": "orchestrator", "model": "gpt-x", "tokens_in": 10, "tokens_out": 4, "cost_usd": 0.0, "cost_basis": "unknown"}
{"change": "b", "role": "machine", "model": "nothing", "tokens_in": 10, "tokens_out": 4, "cost_basis": "unknown"}
{"change": "c", "role": "reviewer", "model": "gpt-x", "tokens_in": 50, "tokens_out": 25, "cost_usd": 0.0021, "cost_basis": "estimated", "duration_s": 9}"#;

    fn priced(fields: &str, change: Option<&str>) -> String {
        Report::new(&log::parse(PRICED).unwrap(), &by(fields), change).to_string()
    }

    #[test]
    fn estimated_costs_are_marked_and_unknown_ones_are_never_0() {
        assert_eq!(
            priced("role", Some("a")),
            "role -> calls · tokens in · cache · out · cost USD · seconds\n\
             builder-build -> 1 · 100 · 0 · 20 · 0.0100 · 5\n\
             reviewer -> 2 · 100 · 0 · 50 · ~0.0021 + unknown · 10\n\
             TOTAL -> in 200 · cache 0 · out 70 · ~0.0121 USD + unknown (1 call(s) with no price)\n\
             ~ estimated from the prices in tminus/models.toml\n\
             unknown: no price in tminus/models.toml for gpt-y\n\
             Human stops logged: 0"
        );
        // No call with a known cost: unknown, and the models without a price, sorted.
        let none = priced("role", Some("b"));
        let unknown = "\norchestrator -> 2 · 20 · 0 · 8 · unknown · 0\n";
        assert!(none.contains(unknown), "{none}");
        let total = "\nTOTAL -> in 20 · cache 0 · out 8 · unknown (2 call(s) with no price)\n\
                     unknown: no price in tminus/models.toml for gpt-x, sonnet\nHuman stops";
        assert!(none.contains(total), "{none}");
        assert!(!none.contains('~'), "{none}");
        // Only estimates: marked, and nothing unknown.
        let estimated = priced("role", Some("c"));
        let total = "\nTOTAL -> in 50 · cache 0 · out 25 · ~0.0021 USD\n\
                     ~ estimated from the prices in tminus/models.toml\nHuman stops";
        assert!(estimated.contains(total), "{estimated}");
        assert!(!estimated.contains("unknown"), "{estimated}");
        // Every model without a price is named once, whatever the grouping.
        let mixed = priced("change", None);
        for line in [
            "\na -> 3 · 200 · 0 · 70 · ~0.0121 + unknown · 15\n",
            "\nb -> 2 · 20 · 0 · 8 · unknown · 0\n",
            "\nunknown: no price in tminus/models.toml for gpt-x, gpt-y, sonnet\n",
        ] {
            assert!(mixed.contains(line), "{mixed}");
        }
        // Reported costs alone carry no mark.
        let log = format!("{}\n{}", PRICED.lines().next().unwrap_or_default(), PRICED);
        let reported = Report::new(&log::parse(&log).unwrap(), &by("model"), Some("a"));
        let reported = reported.to_string();
        for line in [
            "\nopus -> 2 · 200 · 0 · 40 · 0.0200 · 10\n",
            "\ngpt-y -> 1 · 50 · 0 · 25 · unknown · 1\n",
        ] {
            assert!(reported.contains(line), "{reported}");
        }
    }

    #[test]
    fn a_reported_cost_beside_an_unknown_one_has_no_mark() {
        let log = r#"{"role": "builder-build", "tokens_out": 1, "cost_usd": 0.5}
{"role": "builder-build", "tokens_out": 1, "cost_basis": "unknown"}"#;
        let r = Report::new(&log::parse(log).unwrap(), &by("role"), None).to_string();
        let group = "\nbuilder-build -> 2 · 0 · 0 · 2 · 0.5000 + unknown · 0\n";
        assert!(r.contains(group), "{r}");
        let total = "· 0.5000 USD + unknown (1 call(s) with no price)\nunknown: no price in \
                     tminus/models.toml for -\nHuman stops logged: 0";
        assert!(r.ends_with(total), "{r}");
    }

    #[test]
    fn a_report_for_a_project_of_the_old_layout_names_its_models_file_as_it_does() {
        let records = log::parse(PRICED).unwrap();
        let report = Report::new(&records, &by("role"), Some("a"));
        assert_eq!(report.layout, Layout::Tminus);
        let old = report.clone().in_layout(Layout::Gdd).to_string();
        for line in [
            "\n~ estimated from the prices in gdd/models.toml\n",
            "\nunknown: no price in gdd/models.toml for gpt-y\n",
        ] {
            assert!(old.contains(line), "{old}");
        }
        assert!(!old.contains("tminus"), "{old}");
        let new = report.in_layout(Layout::Tminus).to_string();
        assert_eq!(new, priced("role", Some("a")));
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

    fn at(ts: &str, change: &str, fields: &str) -> String {
        format!(r#"{{"ts": "2026-10-28T{ts}+00:00", "change": "{change}", {fields}}}"#)
    }

    const CREATED: &str = r#""role": "orchestrator", "result": "change-created""#;
    const PREPARED: &str = r#""role": "machine", "result": "prepared", "note": "yellow · full""#;
    const OBSERVED: &str = r#""role": "machine", "gate": "-", "agent": "observe", "result": "ok""#;
    const G4_OK: &str = r#""role": "human", "gate": "G4", "result": "ok""#;
    const G1: &str = r#""role": "human", "gate": "G1", "result": "ok""#;
    const PARKED: &str = r#""role": "machine", "result": "parked", "note": "G3 red""#;

    /// Rows one second apart, in the order given.
    fn seq(rows: &[(&str, &str)]) -> Vec<String> {
        let at_second = |i: usize| format!("10:{:02}:{:02}", i / 60, i % 60);
        let row = |(i, (change, fields)): (usize, &(&str, &str))| at(&at_second(i), change, fields);
        rows.iter().enumerate().map(row).collect()
    }

    fn placed(rows: &[String]) -> Vec<Line> {
        flow::read(&rows.join("\n")).lines
    }

    fn typed(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        let owned = |(id, kind): &(&str, &str)| (id.to_string(), kind.to_string());
        pairs.iter().map(owned).collect()
    }

    fn promises(rows: &[String]) -> Promises {
        Promises::new(&placed(rows), &BTreeMap::new(), None, None)
    }

    fn day(text: &str) -> Option<i64> {
        flow::day(text)
    }

    #[test]
    fn until_is_the_last_second_of_a_day_or_a_time() {
        let last = flow::seconds("2026-10-28T23:59:59+00:00");
        assert_eq!(until("2026-10-28"), last);
        assert!(last.is_some());
        let east = flow::seconds("2026-10-28T08:00:00+00:00");
        assert_eq!(until("2026-10-28T10:00:00+02:00"), east);
        assert!(east.is_some());
        for bad in ["2026-13-01", "x", "2026-10-28T25:00:00+00:00", ""] {
            assert_eq!(until(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_line_at_the_end_of_the_day_is_kept_and_the_next_second_is_not() {
        let rows = [
            at("23:59:59", "a", CREATED),
            format!(r#"{{"ts": "2026-10-29T00:00:00+00:00", "change": "b", {CREATED}}}"#),
        ];
        let lines = placed(&rows);
        let none = BTreeMap::new();
        let end = until("2026-10-28");
        assert_eq!(Promises::new(&lines, &none, None, end).changes.len(), 1);
        assert_eq!(Promises::new(&lines, &none, None, None).changes.len(), 2);
        let early = until("2026-10-28T23:59:58+00:00");
        assert!(Promises::new(&lines, &none, None, early).changes.is_empty());
    }

    #[test]
    fn a_change_in_flight_shows_as_it_was_at_until() {
        let rows = [
            at("09:00:00", "a", CREATED),
            at("09:30:00", "a", PREPARED),
            at("10:00:01", "a", OBSERVED),
            at("10:00:01", "b", CREATED),
        ];
        let lines = placed(&rows);
        let none = BTreeMap::new();
        let then = until("2026-10-28T10:00:00+00:00");
        let kept = Promises::new(&lines, &none, None, then).changes;
        assert_eq!(kept.len(), 1);
        assert!(!kept[0].closed);
        let now = Promises::new(&lines, &none, None, None).changes;
        assert!(now[0].closed);
    }

    #[test]
    fn since_keeps_the_changes_created_on_or_after_the_day() {
        let rows = [
            format!(r#"{{"ts": "2026-10-27T23:59:59+00:00", "change": "a", {CREATED}}}"#),
            format!(r#"{{"ts": "2026-10-28T00:00:00+00:00", "change": "b", {CREATED}}}"#),
            format!(r#"{{"ts": "2026-10-28T00:00:01+00:00", "change": "c", {CREATED}}}"#),
        ];
        let lines = placed(&rows);
        let none = BTreeMap::new();
        let kept = Promises::new(&lines, &none, day("2026-10-28"), None).changes;
        assert_eq!(kept.len(), 2);
        let end = until("2026-10-28T00:00:00+00:00");
        let kept = Promises::new(&lines, &none, day("2026-10-28"), end).changes;
        assert_eq!(kept.len(), 1);
    }

    #[test]
    fn a_change_ends_without_stopping_you_only_when_every_part_holds() {
        let ko = r#""role": "human", "gate": "G4", "result": "ko""#;
        let machine = r#""role": "machine", "gate": "G4", "result": "ok""#;
        let rollback = r#""role": "machine", "gate": "-", "agent": "rollback", "result": "ok""#;
        let rows = seq(&[
            ("free", CREATED),
            ("free", PREPARED),
            ("free", OBSERVED),
            ("park", CREATED),
            ("park", PREPARED),
            ("park", PARKED),
            ("park", PREPARED),
            ("park", OBSERVED),
            ("sat", CREATED),
            ("sat", G1),
            ("sat", PREPARED),
            ("sat", OBSERVED),
            ("g4", CREATED),
            ("g4", PREPARED),
            ("g4", G4_OK),
            ("g4", OBSERVED),
            ("machine", CREATED),
            ("machine", PREPARED),
            ("machine", machine),
            ("machine", OBSERVED),
            ("open", CREATED),
            ("open", PREPARED),
            ("ko", CREATED),
            ("ko", PREPARED),
            ("ko", ko),
            ("rolled", CREATED),
            ("rolled", PREPARED),
            ("rolled", OBSERVED),
            ("rolled", rollback),
        ]);
        let kept = promises(&rows).changes;
        let shown: Vec<(bool, usize, usize, bool)> = kept
            .iter()
            .map(|k| (k.closed, k.stops, k.parks, k.free()))
            .collect();
        assert_eq!(
            shown,
            [
                (true, 0, 0, true),
                (true, 0, 1, false),
                (true, 1, 0, false),
                (true, 1, 0, false),
                (true, 0, 0, true),
                (false, 0, 0, false),
                (false, 1, 0, false),
                (false, 0, 0, false),
            ]
        );
        let text = promises(&rows).to_string();
        let mut lines = text.lines().skip(1);
        let all = "All: 8 change(s) · 5 closed · 2 ended without stopping you";
        assert_eq!(lines.next(), Some(all));
        let stops = "  Stops: 0.4 per change (3 in 8) · parked 0.1 per change (1 in 8)";
        assert_eq!(lines.next(), Some(stops));
    }

    /// Changes one after the other, each `(color, state)`: `-` never prepared; `closed`,
    /// `rejected` (a human KO at G4) or `open`.
    fn colored(changes: &[(&str, &str)]) -> Promises {
        let mut rows = Vec::new();
        for (n, (color, state)) in changes.iter().enumerate() {
            let id = format!("c{n}");
            let prepared =
                format!(r#""role": "machine", "result": "prepared", "note": "{color} · full""#);
            rows.push((id.clone(), CREATED.to_string()));
            if *color != "-" {
                rows.push((id.clone(), prepared));
            }
            match *state {
                "closed" => rows.push((id, OBSERVED.to_string())),
                "rejected" => rows.push((
                    id,
                    r#""role": "human", "gate": "G4", "result": "ko""#.to_string(),
                )),
                _ => {}
            }
        }
        let borrowed: Vec<(&str, &str)> =
            rows.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        promises(&seq(&borrowed))
    }

    const WARNING: &str = "Most changes come out red";

    fn closed(red: usize, other: usize) -> Vec<(&'static str, &'static str)> {
        let mut changes = vec![("red", "closed"); red];
        changes.extend(vec![("yellow", "closed"); other]);
        changes
    }

    #[test]
    fn almost_every_change_red_is_eight_of_the_last_ten_closed() {
        let eight = mostly_red(&colored(&closed(8, 2)).changes);
        let said = "Most changes come out red: 8 of the last 10 closed. If almost everything is \
                    red, the blueprint is wrong: the zone map may mark too much as delicate.";
        assert_eq!(eight.as_deref(), Some(said));
        assert_eq!(mostly_red(&colored(&closed(7, 3)).changes), None);
        let ten = mostly_red(&colored(&closed(10, 0)).changes).unwrap();
        assert!(ten.starts_with("Most changes come out red: 10 of the last 10 closed."));
    }

    #[test]
    fn fewer_than_ten_closed_changes_say_nothing() {
        assert_eq!(mostly_red(&colored(&closed(9, 0)).changes), None);
        assert_eq!(mostly_red(&[]), None);
        let mut nine = closed(9, 0);
        nine.push(("red", "open"));
        nine.push(("red", "rejected"));
        assert_eq!(mostly_red(&colored(&nine).changes), None);
    }

    #[test]
    fn only_the_last_ten_closed_changes_count() {
        let mut old_red = closed(5, 0);
        old_red.extend(closed(7, 3));
        assert_eq!(mostly_red(&colored(&old_red).changes), None);
        let mut old_yellow = closed(0, 5);
        old_yellow.extend(closed(8, 2));
        let said = mostly_red(&colored(&old_yellow).changes).unwrap();
        assert!(said.starts_with("Most changes come out red: 8 of the last 10 closed."));
        let mut open_last = closed(7, 3);
        open_last.extend([("red", "open"), ("red", "rejected"), ("red", "open")]);
        assert_eq!(mostly_red(&colored(&open_last).changes), None);
    }

    #[test]
    fn a_change_never_prepared_is_not_red() {
        let mut changes = closed(7, 0);
        changes.extend([("-", "closed"), ("green", "closed"), ("-", "closed")]);
        assert_eq!(mostly_red(&colored(&changes).changes), None);
    }

    #[test]
    fn the_report_ends_with_the_warning_and_without_it_is_as_before() {
        let text = colored(&closed(8, 2)).to_string();
        let last = text.lines().last().unwrap();
        assert!(last.starts_with(WARNING), "{text}");
        assert!(!text.ends_with('\n'));
        let quiet = colored(&closed(7, 3)).to_string();
        assert!(!quiet.contains(WARNING), "{quiet}");
        assert!(quiet.ends_with("Cost: no agent call yet"), "{quiet}");
    }

    #[test]
    fn the_color_is_the_first_part_of_the_last_prepared_note() {
        let yellow = r#""role": "machine", "result": "prepared", "note": "yellow · full""#;
        let red = r#""role": "machine", "result": "prepared", "note": "red · full · G2 already green: Builder skipped""#;
        let bare = r#""role": "machine", "result": "prepared", "note": "  green  ""#;
        let rows = seq(&[
            ("a", CREATED),
            ("a", yellow),
            ("a", red),
            ("b", CREATED),
            ("c", CREATED),
            ("c", bare),
            ("c", yellow),
            ("c", PARKED),
            ("d", CREATED),
            ("d", bare),
            ("d", r#""role": "machine", "result": "ok", "note": "red""#),
        ]);
        let colors: Vec<String> = promises(&rows)
            .changes
            .into_iter()
            .map(|k| k.color)
            .collect();
        assert_eq!(colors, ["red", "-", "yellow", "green"]);
    }

    #[test]
    fn the_type_comes_from_the_record_and_is_a_dash_without_one() {
        let rows = seq(&[("a", CREATED), ("b", CREATED)]);
        let types = typed(&[("a", "Hotfix")]);
        let kept = Promises::new(&placed(&rows), &types, None, None).changes;
        let kinds: Vec<&str> = kept.iter().map(|k| k.kind.as_str()).collect();
        assert_eq!(kinds, ["Hotfix", "-"]);
    }

    #[test]
    fn the_waits_for_the_human_are_the_sitting_and_g4_only() {
        let rows = [
            at("10:00:00", "a", CREATED),
            at("10:10:00", "a", G1),
            at("10:30:00", "a", PREPARED),
            at("11:00:00", "a", r#""role": "machine", "result": "ok""#),
            at("11:30:00", "a", G4_OK),
            at("11:31:00", "a", OBSERVED),
        ];
        let types = typed(&[("a", "Feature")]);
        let text = Promises::new(&placed(&rows), &types, None, None).to_string();
        let block = "All: 1 change(s) · 1 closed · 0 ended without stopping you\n  \
             Stops: 2.0 per change (2 in 1) · parked 0.0 per change (0 in 1)\n  \
             Waiting for you: 40m per change (40m in 1) · at the sitting 10m · at G4 30m\n  \
             Tokens per change: in 0 · cache 0 · out 0\n  \
             Cost: no agent call yet";
        let feature = block.replacen("All", "Feature · yellow", 1);
        let header = "Promises · 1 change(s) created from 2026-10-28 to 2026-10-28";
        assert_eq!(text, format!("{header}\n{block}\n{feature}"));
    }

    #[test]
    fn a_log_without_changes_says_so() {
        assert_eq!(promises(&[]).to_string(), "No change in the log yet.");
        let nobody = [at("10:00:00", "-", CREATED)];
        assert_eq!(promises(&nobody).to_string(), "No change in the log yet.");
    }

    #[test]
    fn the_first_and_last_days_are_those_of_the_first_and_last_change_by_start() {
        let rows = [
            r#"{"ts": "2026-10-30T10:00:00+00:00", "change": "b", "result": "x"}"#.to_string(),
            r#"{"ts": "2026-10-28T10:00:00+00:00", "change": "a", "result": "x"}"#.to_string(),
            r#"{"ts": "2026-10-29T10:00:00+00:00", "change": "c", "result": "x"}"#.to_string(),
        ];
        let text = promises(&rows).to_string();
        let header = "Promises · 3 change(s) created from 2026-10-28 to 2026-10-30\n";
        assert!(text.starts_with(header), "{text}");
    }

    /// The `Tokens` and `Cost` lines of the All block of two changes: `a`, with the `calls`, and `b`.
    fn all_lines(calls: &[&str]) -> (String, String) {
        let mut rows = vec![at("10:00:00", "a", CREATED), at("10:00:01", "b", CREATED)];
        rows.extend(calls.iter().map(|call| at("10:00:02", "a", call)));
        let text = promises(&rows).to_string();
        let line = |start: &str| {
            let found = text.lines().find(|l| l.trim_start().starts_with(start));
            found.unwrap_or_default().to_string()
        };
        (line("Tokens"), line("Cost"))
    }

    #[test]
    fn tokens_per_change_round_half_up() {
        let call = r#""role": "builder-build", "tokens_in": 1, "tokens_cache": 5, "tokens_out": 4"#;
        let (tokens, _) = all_lines(&[call]);
        assert_eq!(tokens, "  Tokens per change: in 1 · cache 3 · out 2");
        let more = r#""role": "builder-build", "tokens_in": 1, "tokens_cache": 1, "tokens_out": 1"#;
        let (tokens, _) = all_lines(&[call, more]);
        assert_eq!(tokens, "  Tokens per change: in 1 · cache 3 · out 3");
        let big = r#""role": "builder-build", "tokens_in": 3000000, "tokens_out": 1"#;
        let (tokens, _) = all_lines(&[big]);
        assert_eq!(
            tokens,
            "  Tokens per change: in 1,500,000 · cache 0 · out 1"
        );
        let (tokens, _) = all_lines(&[r#""role": "machine", "tokens_out": 9"#]);
        assert_eq!(tokens, "  Tokens per change: in 0 · cache 0 · out 0");
    }

    #[test]
    fn tokens_per_change_divide_by_the_number_of_changes() {
        // One change: (2*3 + 1) / 2 = 3; a divisor of 2 + n would give 2.
        let call = r#""role": "builder-build", "tokens_out": 3"#;
        let rows = [at("10:00:00", "a", CREATED), at("10:00:02", "a", call)];
        let text = promises(&rows).to_string();
        assert!(
            text.contains("Tokens per change: in 0 · cache 0 · out 3\n"),
            "{text}"
        );
        // Three changes: (2*10 + 3) / 6 = 3; a divisor of 2 + n would give 4.
        let ten = r#""role": "builder-build", "tokens_out": 10"#;
        let rows = [
            at("10:00:00", "a", CREATED),
            at("10:00:01", "b", CREATED),
            at("10:00:02", "c", CREATED),
            at("10:00:03", "a", ten),
        ];
        let text = promises(&rows).to_string();
        assert!(
            text.contains("Tokens per change: in 0 · cache 0 · out 3\n"),
            "{text}"
        );
    }

    #[test]
    fn the_cost_per_change_says_what_it_stands_on() {
        let reported = r#""role": "builder-build", "tokens_out": 1, "cost_usd": 0.01"#;
        let estimated =
            r#""role": "reviewer", "tokens_out": 1, "cost_usd": 0.01, "cost_basis": "estimated""#;
        let unknown = r#""role": "reviewer", "tokens_out": 1, "cost_basis": "unknown""#;
        let cases: [(&[&str], &str); 5] = [
            (&[], "  Cost: no agent call yet"),
            (
                &[reported],
                "  Cost: 0.0050 USD per change · in all 0.0100 USD",
            ),
            (
                &[estimated],
                "  Cost: ~0.0050 USD per change · in all ~0.0100 USD",
            ),
            (
                &[reported, unknown],
                "  Cost: 0.0050 USD per change · in all 0.0100 USD + unknown (1 call(s) with no price)",
            ),
            (
                &[unknown],
                "  Cost: unknown per change · in all unknown (1 call(s) with no price)",
            ),
        ];
        for (calls, line) in cases {
            assert_eq!(all_lines(calls).1, line, "{calls:?}");
        }
    }

    #[test]
    fn the_groups_come_sorted_by_type_and_color_and_the_all_block_adds_them_up() {
        let red = r#""role": "machine", "result": "prepared", "note": "red · full""#;
        let green = r#""role": "machine", "result": "prepared", "note": "green""#;
        let rows = seq(&[
            ("a", CREATED),
            ("a", PREPARED),
            ("a", G4_OK),
            ("b", CREATED),
            ("b", red),
            ("c", CREATED),
            ("c", green),
            ("c", G4_OK),
            ("c", G4_OK),
            ("d", CREATED),
            ("e", CREATED),
            ("e", red),
        ]);
        let types = typed(&[
            ("a", "Hotfix"),
            ("b", "Feature"),
            ("c", "Feature"),
            ("e", "Feature"),
        ]);
        let text = Promises::new(&placed(&rows), &types, None, None).to_string();
        let heads: Vec<&str> = text
            .lines()
            .skip(1)
            .filter(|l| !l.starts_with(' '))
            .collect();
        let labels: Vec<&str> = heads.iter().filter_map(|l| l.split(": ").next()).collect();
        assert_eq!(
            labels,
            [
                "All",
                "- · -",
                "Feature · green",
                "Feature · red",
                "Hotfix · yellow"
            ]
        );
        let count = |head: &str| -> usize {
            let rest = head.split(": ").nth(1).unwrap_or_default();
            rest.split(' ')
                .next()
                .unwrap_or_default()
                .parse()
                .unwrap_or(0)
        };
        let counts: Vec<usize> = heads.iter().map(|h| count(h)).collect();
        assert_eq!(counts, [5, 1, 1, 2, 1]);
        let stops: Vec<usize> = text
            .lines()
            .filter(|l| l.trim_start().starts_with("Stops"))
            .map(|l| {
                let inner = l.split('(').nth(1).unwrap_or_default();
                inner
                    .split(' ')
                    .next()
                    .unwrap_or_default()
                    .parse()
                    .unwrap_or(99)
            })
            .collect();
        assert_eq!(stops, [3, 0, 2, 0, 1]);
    }
}
