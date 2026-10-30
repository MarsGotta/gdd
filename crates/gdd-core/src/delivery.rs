//! The delivery report: DORA's five software delivery metrics (dora.dev) and GDD's own signals,
//! from the log and the records of the changes.
//!
//! A deployment is a promotion: the `promote` step of the release, `ok` or `simulated`. It failed
//! when its observation failed or it was rolled back. DORA's metrics are throughput (change lead
//! time, deployment frequency, failed deployment recovery time) and instability (change fail rate,
//! deployment rework rate). GDD's signals are signs to read, never targets.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Serialize, Serializer};

use crate::flow::{DAY, Line, duration, human_at, is, step, whole};
use crate::log::Record;

/// What a Reviewer decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The change may merge.
    Ok,
    /// The change may not.
    Ko,
}

/// The verdict of a review file: the `OK` or `KO` after the last `VERDICT:` that has one; `None`
/// when no line has one (the file is not a review).
pub fn verdict(text: &str) -> Option<Verdict> {
    text.lines().rev().find_map(|line| {
        let rest = line.strip_prefix("VERDICT:")?.trim_start();
        if rest.starts_with("OK") {
            Some(Verdict::Ok)
        } else if rest.starts_with("KO") {
            Some(Verdict::Ko)
        } else {
            None
        }
    })
}

/// What the report reads besides the log, by change id.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Records {
    /// The type of the change, from its record: `Feature`, `Hotfix`...
    pub types: BTreeMap<String, String>,
    /// The verdicts of its review files, in the order of their numbers.
    pub reviews: BTreeMap<String, Vec<Verdict>>,
}

/// A promotion of a change.
#[derive(Debug, Clone, PartialEq)]
struct Deployment {
    /// When: seconds since 1970-01-01T00:00:00 UTC.
    at: i64,
    /// Where its line is in the log: what orders deployments of the same second.
    order: usize,
    /// It was logged as `simulated`, not `ok`.
    simulated: bool,
    /// Its observation failed or it was rolled back.
    failed: bool,
    /// Its change is a Hotfix.
    hotfix: bool,
    /// Seconds from the first `prepared` line of its change.
    lead: Option<i64>,
    /// When its change was rolled back for good, if it was, after it.
    rolled_back: Option<i64>,
    /// Seconds until a failed deployment was recovered.
    recovery: Option<i64>,
}

/// Whether `record` is a promotion, and if so whether it was simulated.
fn deployed(record: &Record) -> Option<bool> {
    if step(record).as_deref() != Some("promote") {
        return None;
    }
    if is(record, "result", "ok") {
        Some(false)
    } else {
        is(record, "result", "simulated").then_some(true)
    }
}

/// Whether `record` says a deployment failed: its observation did, or it was rolled back.
fn failure(record: &Record) -> bool {
    match step(record).as_deref() {
        Some("observe") => is(record, "result", "ko"),
        Some("rollback") => true,
        _ => false,
    }
}

/// Whether `record` is a rollback that went through.
fn rolled_back(record: &Record) -> bool {
    let passed = is(record, "result", "ok") || is(record, "result", "simulated");
    step(record).as_deref() == Some("rollback") && passed
}

/// The deployments of one change from its lines, in order. A deployment fails by what follows it
/// up to the next deployment of the change. `log` is the whole log `lines` come from.
fn deployments(log: &[Line], lines: &[&Line], hotfix: bool) -> Vec<Deployment> {
    let prepared = lines
        .iter()
        .find(|line| is(&line.record, "result", "prepared"))
        .map(|line| line.at);
    let mut found = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let Some(simulated) = deployed(&line.record) else {
            continue;
        };
        let after = &lines[i + 1..];
        let next = after.iter().position(|l| deployed(&l.record).is_some());
        let window = &after[..next.unwrap_or(after.len())];
        found.push(Deployment {
            at: line.at,
            order: log.iter().position(|l| std::ptr::eq(l, *line)).unwrap_or(0),
            simulated,
            failed: window.iter().any(|l| failure(&l.record)),
            hotfix,
            lead: prepared.map(|start| line.at - start),
            rolled_back: after.iter().find(|l| rolled_back(&l.record)).map(|l| l.at),
            recovery: None,
        });
    }
    found
}

/// Gives each failed deployment its recovery time: to the rollback of its change, else to the
/// next deployment of any change. `deployed` is in time order.
fn recover(deployed: &mut [Deployment]) {
    let times: Vec<i64> = deployed.iter().map(|d| d.at).collect();
    for (i, deployment) in deployed.iter_mut().enumerate() {
        let until = deployment.rolled_back.or(times.get(i + 1).copied());
        deployment.recovery = until
            .filter(|_| deployment.failed)
            .map(|at| at - deployment.at);
    }
}

/// Whether `record` is a run of mutation testing in G3.
fn mutation(record: &Record) -> bool {
    is(record, "role", "machine")
        && is(record, "gate", "G3")
        && record.text("agent").starts_with("mutation")
}

/// Whether a human spoke before the change was first prepared (all its lines, if never).
fn sat(records: &[&Record]) -> bool {
    let start = records.iter().position(|r| is(r, "result", "prepared"));
    let before = &records[..start.unwrap_or(records.len())];
    before.iter().any(|r| is(r, "role", "human"))
}

/// The tokens out of a change, when it had an agent call.
fn tokens(records: &[&Record]) -> Option<f64> {
    let called = records.iter().any(|r| r.text("model") != "-");
    called.then(|| records.iter().map(|r| r.tokens_out).sum::<u64>() as f64)
}

/// GDD's signals, added up over the changes counted.
#[derive(Debug, Clone, Default, PartialEq)]
struct Signals {
    /// Human G4 decisions, and how many asked for polish.
    decisions: usize,
    polish: usize,
    /// The changes, and the `parked` lines of them.
    changes: usize,
    parks: usize,
    /// Verdicts of reviews, and how many were KO.
    reviews: usize,
    ko: usize,
    /// The reviews of each change with one, and how many of those changes passed their first.
    passes: Vec<f64>,
    first_ok: usize,
    /// Runs of mutation testing in G3, and how many were red.
    runs: usize,
    red: usize,
    /// Times a human had to step in.
    stops: usize,
    /// The tokens out of each change that called an agent.
    tokens: Vec<f64>,
}

impl Signals {
    /// Adds a change: its lines in log order and its verdicts.
    fn add(&mut self, lines: &[&Line], reviews: &[Verdict]) {
        let records: Vec<&Record> = lines.iter().map(|line| &line.record).collect();
        let decisions = count(&records, |r| human_at(r, "G4"));
        self.decisions += decisions;
        self.polish += count(&records, |r| human_at(r, "G4") && is(r, "result", "polish"));
        self.changes += 1;
        self.parks += count(&records, |r| is(r, "result", "parked"));
        self.reviews += reviews.len();
        self.ko += reviews.iter().filter(|v| **v == Verdict::Ko).count();
        if let Some(first) = reviews.first() {
            self.passes.push(reviews.len() as f64);
            self.first_ok += usize::from(*first == Verdict::Ok);
        }
        self.runs += count(&records, mutation);
        self.red += count(&records, |r| mutation(r) && is(r, "result", "ko"));
        self.stops += decisions + usize::from(sat(&records));
        self.tokens.extend(tokens(&records));
    }
}

/// How many of `records` `keep` keeps.
fn count(records: &[&Record], keep: impl Fn(&Record) -> bool) -> usize {
    records.iter().filter(|r| keep(r)).count()
}

/// The median of `values`: the middle one, or the mean of the two middle ones.
fn median(mut values: Vec<f64>) -> Option<f64> {
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    let high = *values.get(mid)?;
    if !values.len().is_multiple_of(2) {
        return Some(high);
    }
    let low = *values.get(mid - 1)?;
    Some((low + high) / 2.0)
}

/// `n` over `d`; `None` without a `d`.
fn ratio(n: usize, d: usize) -> Option<f64> {
    (d > 0).then(|| n as f64 / d as f64)
}

/// `n` of `d` as a whole percent, rounded half up.
fn percent(n: usize, d: usize) -> usize {
    (n * 200 + d) / (2 * d)
}

/// `n` over `d` with one decimal, rounded half up: `1.3`.
fn tenths(n: usize, d: usize) -> String {
    let tenths = (n * 20 + d) / (2 * d);
    format!("{}.{}", tenths / 10, tenths % 10)
}

/// `value` with one decimal, or none when it is whole: `2.5`, `2`.
fn decimal(value: f64) -> String {
    let text = format!("{value:.1}");
    match text.strip_suffix(".0") {
        Some(whole) => whole.to_string(),
        None => text,
    }
}

/// `n` of `d` as a whole percent and the counts, with `noun` after them; `no data yet` without
/// a `d`: `25% (1 of 4 are Hotfixes)`.
fn share(n: usize, d: usize, noun: &str) -> String {
    if d == 0 {
        return NO_DATA.to_string();
    }
    format!("{}% ({n} of {d}{noun})", percent(n, d))
}

/// `total` over `changes` with one decimal and the counts: `1.2 per change (6 in 5)`.
fn per_change(total: usize, changes: usize) -> String {
    if changes == 0 {
        return NO_DATA.to_string();
    }
    format!(
        "{} per change ({total} in {changes})",
        tenths(total, changes)
    )
}

/// What a signal without data says.
const NO_DATA: &str = "no data yet";

/// What follows the median lead time.
const LEAD_FROM: &str = ", from the build's start to the promotion";

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

/// The date `days` days after 1970-01-01 as `YYYY-MM-DD`. Howard Hinnant's `civil_from_days`, on
/// eras of 400 years that start on March 1st.
fn date(days: i64) -> String {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 {
        month_from_march + 3
    } else {
        month_from_march - 9
    };
    let year = era * 400 + year_of_era + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// The delivery of the changes of a log.
#[derive(Debug, Clone, PartialEq)]
pub struct Delivery {
    /// Every deployment, by time.
    deployed: Vec<Deployment>,
    signals: Signals,
}

impl Delivery {
    /// The delivery of the changes of `lines` (see [`crate::flow::read`]), with their `records`:
    /// only the changes whose first line is on or after the day `since` (see [`crate::flow::day`])
    /// when given. Lines of no change (`-`) are left out.
    pub fn new(lines: &[Line], records: &Records, since: Option<i64>) -> Delivery {
        let mut groups: Vec<(String, Vec<&Line>)> = Vec::new();
        for line in lines {
            let id = line.record.text("change");
            if id == "-" {
                continue;
            }
            match groups.iter_mut().find(|(known, _)| *known == id) {
                Some((_, group)) => group.push(line),
                None => groups.push((id, vec![line])),
            }
        }
        let mut deployed = Vec::new();
        let mut signals = Signals::default();
        for (id, group) in &groups {
            let first = group.first().map_or(0, |line| line.at);
            if since.is_some_and(|day| first < day * DAY) {
                continue;
            }
            let hotfix = records.types.get(id).is_some_and(|kind| kind == "Hotfix");
            deployed.extend(deployments(lines, group, hotfix));
            let reviews = records.reviews.get(id).map_or(&[][..], Vec::as_slice);
            signals.add(group, reviews);
        }
        deployed.sort_by_key(|d| (d.at, d.order));
        recover(&mut deployed);
        Delivery { deployed, signals }
    }

    /// How many deployments `keep` keeps.
    fn tally(&self, keep: impl Fn(&Deployment) -> bool) -> usize {
        self.deployed.iter().filter(|d| keep(d)).count()
    }

    /// The days of the first and the last deployment.
    fn span(&self) -> Option<(i64, i64)> {
        let day = |d: &Deployment| d.at.div_euclid(DAY);
        Some((day(self.deployed.first()?), day(self.deployed.last()?)))
    }

    /// Deployments per day, from the first deployment's day to the last's, both counted.
    fn per_day(&self) -> Option<(usize, usize)> {
        let (first, last) = self.span()?;
        let days = usize::try_from(last - first + 1).ok()?;
        Some((self.deployed.len(), days))
    }

    /// The median of the seconds `of` gives for the deployments that have them.
    fn middle(&self, of: impl Fn(&Deployment) -> Option<i64>) -> Option<f64> {
        let seconds = self.deployed.iter().filter_map(of);
        median(seconds.map(|s| s as f64).collect())
    }

    /// The median lead time, in seconds.
    fn lead(&self) -> Option<f64> {
        self.middle(|d| d.lead)
    }

    /// The median recovery time, in seconds.
    fn recovery(&self) -> Option<f64> {
        self.middle(|d| d.recovery)
    }

    /// The median time between consecutive deployments, in seconds.
    fn between(&self) -> Option<f64> {
        let times: Vec<i64> = self.deployed.iter().map(|d| d.at).collect();
        let gaps = times.windows(2).map(|pair| (pair[1] - pair[0]) as f64);
        median(gaps.collect())
    }

    /// The first line of the report, or the line that says there is no deployment.
    fn headline(&self) -> String {
        let Some((first, last)) = self.span() else {
            return "No deployment in the log yet.".to_string();
        };
        let (n, from, to) = (self.deployed.len(), date(first), date(last));
        let mut line = format!("Delivery · {n} deployment(s) from {from} to {to}");
        let simulated = self.tally(|d| d.simulated);
        if simulated > 0 {
            line.push_str(&format!(" · {simulated} simulated"));
        }
        line
    }

    fn lead_text(&self) -> String {
        match self.lead() {
            Some(s) => format!("median {}{LEAD_FROM}", duration(s)),
            None => NO_DATA.to_string(),
        }
    }

    fn frequency_text(&self) -> String {
        let Some((n, days)) = self.per_day() else {
            return NO_DATA.to_string();
        };
        let mut text = format!("{} per day", tenths(n, days));
        if let Some(gap) = self.between() {
            text.push_str(&format!(" · median {} between deployments", duration(gap)));
        }
        text
    }

    fn recovery_text(&self) -> String {
        let failed = self.tally(|d| d.failed);
        if failed == 0 {
            return "none failed".to_string();
        }
        let lost = self.tally(|d| d.failed && d.recovery.is_none());
        let lost = if lost > 0 {
            format!(", {lost} not recovered")
        } else {
            String::new()
        };
        match self.recovery() {
            Some(s) => format!("median {} ({failed} failed{lost})", duration(s)),
            None => format!("no recovery yet ({failed} failed{lost})"),
        }
    }

    /// The Throughput and Instability blocks: none without a deployment.
    fn dora(&self) -> Vec<String> {
        let n = self.deployed.len();
        if n == 0 {
            return Vec::new();
        }
        let (lead, frequency) = (self.lead_text(), self.frequency_text());
        let recovery = self.recovery_text();
        let failing = share(self.tally(|d| d.failed), n, "");
        let rework = share(self.tally(|d| d.hotfix), n, " are Hotfixes");
        vec![
            "Throughput".to_string(),
            format!("  Change lead time: {lead}"),
            format!("  Deployment frequency: {frequency}"),
            format!("  Failed deployment recovery time: {recovery}"),
            "Instability".to_string(),
            format!("  Change fail rate: {failing}"),
            format!("  Deployment rework rate: {rework}"),
        ]
    }

    /// The block of GDD's signals.
    fn signs(&self) -> Vec<String> {
        let s = &self.signals;
        let polish = share(s.polish, s.decisions, " decisions");
        let parks = per_change(s.parks, s.changes);
        let reviews = share(s.ko, s.reviews, " reviews");
        let passes = match median(s.passes.clone()) {
            Some(m) => format!(
                "median {} per change · first review OK in {} of {} changes",
                decimal(m),
                s.first_ok,
                s.passes.len()
            ),
            None => NO_DATA.to_string(),
        };
        let mutation = match s.runs {
            0 => NO_DATA.to_string(),
            runs => format!("{} of {runs} runs", s.red),
        };
        let stops = per_change(s.stops, s.changes);
        let tokens = match median(s.tokens.clone()) {
            Some(m) => format!("median {} per change", thousands(whole(m))),
            None => NO_DATA.to_string(),
        };
        vec![
            "GDD signals (signs to read, never targets)".to_string(),
            format!("  Polish at G4: {polish}"),
            format!("  Parks: {parks}"),
            format!("  Reviewer KO: {reviews}"),
            format!("  Reviews to pass: {passes}"),
            format!("  Mutation testing red in G3: {mutation}"),
            format!("  Human stops: {stops}"),
            format!("  Tokens out: {tokens}"),
        ]
    }
}

/// The report as the human reads it: the deployments, DORA's five, then GDD's signals.
impl fmt::Display for Delivery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut lines = vec![self.headline()];
        lines.extend(self.dora());
        lines.extend(self.signs());
        write!(f, "{}", lines.join("\n"))
    }
}

/// The numbers as JSON: seconds and ratios, `null` where there is no data.
#[derive(Serialize)]
struct Numbers {
    deployments: usize,
    simulated: usize,
    lead_time_s: Option<f64>,
    frequency_per_day: Option<f64>,
    between_s: Option<f64>,
    recovery_s: Option<f64>,
    failed: usize,
    change_fail_rate: Option<f64>,
    rework_rate: Option<f64>,
    polish_rate: Option<f64>,
    parks_per_change: Option<f64>,
    reviewer_ko_rate: Option<f64>,
    reviews_to_pass_median: Option<f64>,
    first_review_ok: usize,
    reviewed_changes: usize,
    mutation_red: Option<usize>,
    mutation_runs: usize,
    stops_per_change: Option<f64>,
    tokens_out_median: Option<f64>,
}

impl Serialize for Delivery {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let (s, n) = (&self.signals, self.deployed.len());
        let failed = self.tally(|d| d.failed);
        Numbers {
            deployments: n,
            simulated: self.tally(|d| d.simulated),
            lead_time_s: self.lead(),
            frequency_per_day: self.per_day().and_then(|(n, days)| ratio(n, days)),
            between_s: self.between(),
            recovery_s: self.recovery(),
            failed,
            change_fail_rate: ratio(failed, n),
            rework_rate: ratio(self.tally(|d| d.hotfix), n),
            polish_rate: ratio(s.polish, s.decisions),
            parks_per_change: ratio(s.parks, s.changes),
            reviewer_ko_rate: ratio(s.ko, s.reviews),
            reviews_to_pass_median: median(s.passes.clone()),
            first_review_ok: s.first_ok,
            reviewed_changes: s.passes.len(),
            mutation_red: (s.runs > 0).then_some(s.red),
            mutation_runs: s.runs,
            stops_per_change: ratio(s.stops, s.changes),
            tokens_out_median: median(s.tokens.clone()),
        }
        .serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow;

    #[test]
    fn a_ratio_without_a_denominator_has_no_value() {
        assert_eq!(ratio(0, 0), None);
        assert_eq!(ratio(1, 4), Some(0.25));
    }

    #[test]
    fn dates_hold_over_the_whole_cycle_of_400_years() {
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(date(11_016), "2000-02-29");
        assert_eq!(date(47_540), "2100-02-28");
        assert_eq!(date(47_541), "2100-03-01");
        assert_eq!(date(157_113), "2400-02-29");
        assert_eq!(date(157_114), "2400-03-01");
    }

    /// A line of the log: `at` is `DDTHH:MM` of October 2026.
    fn row(at: &str, change: &str, role: &str, gate: &str, agent: &str, result: &str) -> String {
        format!(
            r#"{{"ts": "2026-10-{at}:00+00:00", "change": "{change}", "role": "{role}", "gate": "{gate}", "agent": "{agent}", "model": "-", "result": "{result}"}}"#
        )
    }

    /// A release step of a change.
    fn release(at: &str, change: &str, agent: &str, result: &str) -> String {
        row(at, change, "machine", "-", agent, result)
    }

    /// A line of a change of a role at a gate.
    fn by(at: &str, change: &str, role: &str, gate: &str, result: &str) -> String {
        row(at, change, role, gate, "-", result)
    }

    fn prepared(at: &str, change: &str) -> String {
        by(at, change, "machine", "-", "prepared")
    }

    fn delivery_of(rows: &[String], records: &Records, since: Option<&str>) -> Delivery {
        let log = flow::read(&rows.join("\n"));
        assert_eq!(log.left_out, 0, "{rows:?}");
        Delivery::new(
            &log.lines,
            records,
            since.map(|text| flow::day(text).unwrap()),
        )
    }

    fn delivery(rows: &[String]) -> Delivery {
        delivery_of(rows, &Records::default(), None)
    }

    fn shown(rows: &[String]) -> String {
        delivery(rows).to_string()
    }

    fn hotfixes(ids: &[&str]) -> Records {
        let types = ids.iter().map(|id| (id.to_string(), "Hotfix".to_string()));
        Records {
            types: types.collect(),
            ..Records::default()
        }
    }

    fn json(rows: &[String]) -> serde_json::Value {
        serde_json::to_value(delivery(rows)).unwrap()
    }

    /// Asserts that the report of `rows` has `line`, whole.
    fn has(rows: &[String], line: &str) {
        let text = shown(rows);
        assert!(text.lines().any(|l| l == line), "{line:?} not in\n{text}");
    }

    #[test]
    fn a_deployment_is_a_machine_promote_that_passed_outside_the_gates() {
        let rows = [
            release("20T10:00", "a", "promote", "ok"),
            release("20T11:00", "a", "promote", "simulated"),
            release("20T12:00", "a", "promote", "ko"),
            row("20T13:00", "a", "machine", "G2", "promote", "ok"),
            row("20T14:00", "a", "human", "-", "promote", "ok"),
            release("20T15:00", "a", "observe", "ok"),
        ];
        let delivery = delivery(&rows);
        assert_eq!(delivery.deployed.len(), 2);
        assert_eq!(delivery.tally(|d| d.simulated), 1);
        let headline = "Delivery · 2 deployment(s) from 2026-10-20 to 2026-10-20 · 1 simulated";
        has(&rows, headline);
    }

    #[test]
    fn without_a_deployment_the_log_says_so_and_the_signals_still_show() {
        let rows = [by("20T10:00", "a", "orchestrator", "-", "change-created")];
        let text = shown(&rows);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "No deployment in the log yet.");
        assert_eq!(lines[1], "GDD signals (signs to read, never targets)");
        assert_eq!(lines.len(), 9);
        assert_eq!(
            shown(&[]).lines().next(),
            Some("No deployment in the log yet.")
        );
        let numbers = json(&rows);
        assert_eq!(numbers["deployments"], 0);
        let empty = [
            "lead_time_s",
            "frequency_per_day",
            "between_s",
            "recovery_s",
            "change_fail_rate",
            "rework_rate",
        ];
        for key in empty {
            assert!(numbers[key].is_null(), "{key}");
        }
    }

    #[test]
    fn lead_time_runs_from_the_first_prepared_line_to_the_promotion() {
        let rows = [
            prepared("20T09:00", "a"),
            prepared("20T09:30", "a"),
            release("20T10:00", "a", "promote", "ok"),
            release("21T09:00", "b", "promote", "ok"),
        ];
        assert_eq!(delivery(&rows).lead(), Some(3600.0));
        has(
            &rows,
            "  Change lead time: median 1h 00m, from the build's start to the promotion",
        );
        let unprepared = [release("21T09:00", "b", "promote", "ok")];
        assert_eq!(delivery(&unprepared).lead(), None);
        has(&unprepared, "  Change lead time: no data yet");
    }

    #[test]
    fn medians_take_the_middle_or_the_mean_of_the_two_middle_ones() {
        assert_eq!(median(vec![]), None);
        assert_eq!(median(vec![5.0]), Some(5.0));
        assert_eq!(median(vec![9.0, 1.0, 5.0]), Some(5.0));
        assert_eq!(median(vec![4.0, 1.0, 2.0, 9.0]), Some(3.0));
        assert_eq!(median(vec![1.0, 2.0]), Some(1.5));
    }

    #[test]
    fn frequency_counts_both_days_and_the_gaps_are_between_all_changes() {
        let rows = [
            release("20T11:00", "a", "promote", "ok"),
            release("22T10:00", "c", "promote", "ok"),
            release("21T09:10", "b", "promote", "ok"),
            release("22T10:30", "c", "promote", "ok"),
        ];
        let delivery = delivery(&rows);
        assert_eq!(delivery.per_day(), Some((4, 3)));
        // 22h10m, 24h50m and 30m: the middle one.
        assert_eq!(delivery.between(), Some(79_800.0));
        let frequency = "  Deployment frequency: 1.3 per day · median 22h 10m between deployments";
        has(&rows, frequency);
    }

    #[test]
    fn one_deployment_has_no_time_between_deployments() {
        let rows = [release("20T11:00", "a", "promote", "ok")];
        has(&rows, "  Deployment frequency: 1.0 per day");
        assert_eq!(delivery(&rows).between(), None);
        assert_eq!(json(&rows)["between_s"], serde_json::Value::Null);
    }

    #[test]
    fn a_deployment_fails_when_its_observation_fails_or_it_is_rolled_back() {
        let rows = [
            release("20T10:00", "a", "promote", "ok"),
            release("20T10:05", "a", "observe", "ko"),
            release("20T11:00", "b", "promote", "ok"),
            release("20T11:05", "b", "rollback", "ko"),
            release("20T12:00", "c", "promote", "ok"),
            release("20T12:05", "c", "observe", "ok"),
            release("20T13:00", "d", "observe", "ko"),
            release("20T13:05", "d", "promote", "ok"),
            row("20T14:00", "e", "machine", "G3", "observe", "ko"),
            release("20T14:05", "e", "promote", "ok"),
        ];
        assert_eq!(failed(&rows), [true, true, false, false, false]);
        has(&rows, "  Change fail rate: 40% (2 of 5)");
    }

    /// Whether each deployment of `rows`, by time, failed.
    fn failed(rows: &[String]) -> Vec<bool> {
        delivery(rows).deployed.iter().map(|d| d.failed).collect()
    }

    #[test]
    fn a_failure_belongs_to_the_deployment_it_follows() {
        let rows = [
            release("20T10:00", "a", "promote", "ok"),
            release("20T11:00", "a", "promote", "ok"),
            release("20T11:05", "a", "observe", "ko"),
        ];
        assert_eq!(failed(&rows), [false, true]);
    }

    #[test]
    fn a_failed_deployment_recovers_by_a_rollback_that_went_through() {
        let rows = [
            release("20T10:00", "a", "promote", "ok"),
            release("20T10:05", "a", "observe", "ko"),
            release("20T10:08", "a", "rollback", "ko"),
            release("20T10:20", "a", "rollback", "simulated"),
            release("20T11:00", "b", "promote", "ok"),
        ];
        assert_eq!(delivery(&rows).recovery(), Some(1200.0));
        let recovery = "  Failed deployment recovery time: median 20m (1 failed)";
        has(&rows, recovery);
    }

    #[test]
    fn deployments_of_the_same_second_keep_the_order_of_the_log() {
        // `b` is the first change in the log, but `a` is promoted before it in the same second.
        let rows = [
            prepared("20T09:00", "b"),
            release("20T10:00", "a", "promote", "ok"),
            release("20T10:00", "b", "promote", "ok"),
            release("20T10:05", "b", "observe", "ko"),
        ];
        let delivery = delivery(&rows);
        assert_eq!(delivery.recovery(), None);
        has(
            &rows,
            "  Failed deployment recovery time: no recovery yet (1 failed, 1 not recovered)",
        );
    }

    #[test]
    fn without_a_rollback_it_recovers_with_the_next_deployment_of_any_change() {
        let rows = [
            release("20T10:00", "a", "promote", "ok"),
            release("20T10:05", "a", "observe", "ko"),
            release("20T10:30", "b", "promote", "ok"),
            release("20T11:00", "c", "promote", "ok"),
        ];
        assert_eq!(delivery(&rows).recovery(), Some(1800.0));
    }

    #[test]
    fn a_failed_deployment_nothing_follows_is_not_recovered() {
        let rows = [
            release("20T10:00", "a", "promote", "ok"),
            release("20T10:05", "a", "observe", "ko"),
            release("20T11:00", "b", "promote", "ok"),
            release("20T11:05", "b", "observe", "ko"),
        ];
        assert_eq!(delivery(&rows).recovery(), Some(3600.0));
        let recovery = "  Failed deployment recovery time: ";
        let median = format!("{recovery}median 1h 00m (2 failed, 1 not recovered)");
        has(&rows, &median);
        let last = [rows[0].clone(), rows[1].clone()];
        let none = format!("{recovery}no recovery yet (1 failed, 1 not recovered)");
        has(&last, &none);
        assert_eq!(json(&last)["recovery_s"], serde_json::Value::Null);
        assert_eq!(json(&last)["failed"], 1);
    }

    #[test]
    fn none_failed_says_so() {
        let rows = [release("20T10:00", "a", "promote", "ok")];
        has(&rows, "  Failed deployment recovery time: none failed");
        has(&rows, "  Change fail rate: 0% (0 of 1)");
    }

    #[test]
    fn rework_is_the_deployments_of_hotfixes() {
        let rows = [
            release("20T10:00", "a", "promote", "ok"),
            release("20T11:00", "b", "promote", "simulated"),
            release("20T12:00", "c", "promote", "ok"),
            release("20T13:00", "b", "promote", "ok"),
        ];
        let delivery = delivery_of(&rows, &hotfixes(&["b"]), None);
        assert_eq!(delivery.tally(|d| d.hotfix), 2);
        let text = delivery.to_string();
        assert!(text.contains("  Deployment rework rate: 50% (2 of 4 are Hotfixes)\n"));
        let other = Records {
            types: [("b".to_string(), "Feature".to_string())].into(),
            ..Records::default()
        };
        assert_eq!(delivery_of(&rows, &other, None).tally(|d| d.hotfix), 0);
    }

    #[test]
    fn percentages_and_decimals_round_half_up() {
        assert_eq!([percent(1, 8), percent(1, 3), percent(2, 3)], [13, 33, 67]);
        assert_eq!([percent(1, 2), percent(0, 5), percent(5, 5)], [50, 0, 100]);
        assert_eq!(percent(1, 200), 1);
        assert_eq!((percent(1, 400), percent(399, 400)), (0, 100));
        assert_eq!(tenths(1, 8), "0.1");
        assert_eq!(tenths(1, 40), "0.0");
        assert_eq!(tenths(3, 40), "0.1");
        assert_eq!(tenths(4, 3), "1.3");
        assert_eq!(tenths(1, 3), "0.3");
        assert_eq!(tenths(5, 1), "5.0");
        assert_eq!(tenths(1, 20), "0.1");
        assert_eq!(tenths(19, 20), "1.0");
    }

    #[test]
    fn polish_is_the_human_g4_decisions_that_asked_for_it() {
        let rows = [
            by("20T10:00", "a", "human", "G4", "polish"),
            by("20T11:00", "a", "human", "G4", "ok"),
            by("20T12:00", "b", "human", "G4", "ok"),
            by("20T12:30", "b", "machine", "G4", "polish"),
            by("20T12:40", "b", "human", "G3", "polish"),
        ];
        has(&rows, "  Polish at G4: 33% (1 of 3 decisions)");
        let none = [by("20T12:40", "b", "human", "G3", "ok")];
        has(&none, "  Polish at G4: no data yet");
        assert_eq!(json(&none)["polish_rate"], serde_json::Value::Null);
    }

    #[test]
    fn parks_are_counted_over_the_changes() {
        let rows = [
            by("20T10:00", "a", "machine", "-", "parked"),
            by("20T11:00", "a", "machine", "-", "parked"),
            by("20T12:00", "b", "machine", "-", "parked"),
            by("20T13:00", "c", "orchestrator", "-", "change-created"),
            by("20T13:00", "-", "machine", "-", "parked"),
        ];
        has(&rows, "  Parks: 1.0 per change (3 in 3)");
        has(&[], "  Parks: no data yet");
    }

    #[test]
    fn a_review_file_counts_by_its_last_verdict_and_one_without_it_is_no_review() {
        assert_eq!(verdict("VERDICT: OK"), Some(Verdict::Ok));
        assert_eq!(verdict("VERDICT: KO"), Some(Verdict::Ko));
        let two = "x\nVERDICT: KO\ny\nVERDICT:   OK\n";
        assert_eq!(verdict(two), Some(Verdict::Ok));
        assert_eq!(verdict("VERDICT: OK\nVERDICT: KO"), Some(Verdict::Ko));
        assert_eq!(verdict("VERDICT: OK\nVERDICT: maybe"), Some(Verdict::Ok));
        assert_eq!(verdict("OK"), None);
        assert_eq!(verdict("a VERDICT: OK"), None);
        assert_eq!(verdict("VERDICT: maybe"), None);
        assert_eq!(verdict(""), None);
    }

    #[test]
    fn reviewer_ko_counts_the_verdicts_of_the_changes_counted() {
        let rows = [
            by("20T10:00", "a", "orchestrator", "-", "change-created"),
            by("22T10:00", "b", "orchestrator", "-", "change-created"),
        ];
        let reviews = [
            ("a".to_string(), vec![Verdict::Ko, Verdict::Ok]),
            ("b".to_string(), vec![Verdict::Ok]),
        ];
        let records = Records {
            reviews: reviews.into(),
            ..Records::default()
        };
        let line = |since: Option<&str>| delivery_of(&rows, &records, since).to_string();
        assert!(line(None).contains("  Reviewer KO: 33% (1 of 3 reviews)\n"));
        let since = line(Some("2026-10-21"));
        assert!(since.contains("  Reviewer KO: 0% (0 of 1 reviews)\n"));
        has(&rows, "  Reviewer KO: no data yet");
        let json = serde_json::to_value(delivery_of(&rows, &records, None)).unwrap();
        assert_eq!(json["reviewer_ko_rate"], 1.0 / 3.0);
    }

    /// For each of `changes`, its id, a day `DD` of October 2026 and its verdicts: a line of it
    /// on that day, and the verdicts of its reviews in the records.
    fn reviewed(changes: &[(&str, &str, Vec<Verdict>)]) -> (Vec<String>, Records) {
        let created = |(id, day, _): &(&str, &str, Vec<Verdict>)| {
            let at = format!("{day}T10:00");
            by(&at, id, "orchestrator", "-", "change-created")
        };
        let kept =
            |(id, _, verdicts): &(&str, &str, Vec<Verdict>)| (id.to_string(), verdicts.clone());
        let records = Records {
            reviews: changes.iter().map(kept).collect(),
            ..Records::default()
        };
        (changes.iter().map(created).collect(), records)
    }

    /// The line on reviews to pass of the report of `rows` with `records`, since the day
    /// `since` when given.
    fn passes(rows: &[String], records: &Records, since: Option<&str>) -> String {
        let text = delivery_of(rows, records, since).to_string();
        let line = text
            .lines()
            .find(|line| line.starts_with("  Reviews to pass: "));
        line.unwrap_or_default().to_string()
    }

    #[test]
    fn reviews_to_pass_is_the_median_over_the_changes_reviewed_and_their_first_verdict() {
        let (ok, ko) = (Verdict::Ok, Verdict::Ko);
        let (rows, records) = reviewed(&[
            ("a", "20", vec![ko, ok]),
            ("b", "21", vec![ok]),
            ("c", "22", vec![ko, ko, ko, ok]),
            ("d", "22", vec![]),
        ]);
        let odd = "  Reviews to pass: median 2 per change · first review OK in 1 of 3 changes";
        assert_eq!(passes(&rows, &records, None), odd);
        let even = "  Reviews to pass: median 2.5 per change · first review OK in 1 of 2 changes";
        assert_eq!(passes(&rows, &records, Some("2026-10-21")), even);
        let numbers = serde_json::to_value(delivery_of(&rows, &records, None)).unwrap();
        assert_eq!(numbers["reviews_to_pass_median"], 2.0);
        assert_eq!(numbers["first_review_ok"], 1);
        assert_eq!(numbers["reviewed_changes"], 3);
        let (rows, records) = reviewed(&[("a", "20", vec![ok, ko]), ("b", "21", vec![ko])]);
        let half = "  Reviews to pass: median 1.5 per change · first review OK in 1 of 2 changes";
        assert_eq!(passes(&rows, &records, None), half);
        let one = "  Reviews to pass: median 1 per change · first review OK in 0 of 1 changes";
        assert_eq!(passes(&rows, &records, Some("2026-10-21")), one);
    }

    #[test]
    fn without_a_change_reviewed_reviews_to_pass_has_no_data() {
        let none = "  Reviews to pass: no data yet";
        let (rows, records) = reviewed(&[("a", "20", vec![])]);
        assert_eq!(passes(&rows, &records, None), none);
        let numbers = serde_json::to_value(delivery_of(&rows, &records, None)).unwrap();
        assert_eq!(numbers["reviews_to_pass_median"], serde_json::Value::Null);
        assert_eq!(numbers["first_review_ok"], 0);
        assert_eq!(numbers["reviewed_changes"], 0);
        let (rows, records) = reviewed(&[("a", "20", vec![Verdict::Ok])]);
        assert_eq!(passes(&rows, &records, Some("2026-10-21")), none);
    }

    #[test]
    fn a_median_shows_one_decimal_only_when_it_is_not_whole() {
        assert_eq!(decimal(2.0), "2");
        assert_eq!(decimal(2.5), "2.5");
        assert_eq!(decimal(0.0), "0");
        assert_eq!(decimal(10.5), "10.5");
    }

    #[test]
    fn mutation_testing_red_counts_the_runs_of_g3_not_other_commands() {
        let rows = [
            row("20T10:00", "a", "machine", "G3", "mutation", "ko"),
            row("20T10:10", "a", "machine", "G3", "mutation", "ok"),
            row("20T10:20", "a", "machine", "G3", "mutation-testing", "ko"),
            row("20T10:30", "a", "machine", "G2", "mutation", "ko"),
            row("20T10:40", "a", "machine", "G3", "checks", "ko"),
            row("20T10:50", "a", "human", "G3", "mutation", "ko"),
            row("20T10:55", "-", "machine", "G3", "mutation", "ko"),
        ];
        has(&rows, "  Mutation testing red in G3: 2 of 3 runs");
        has(&[], "  Mutation testing red in G3: no data yet");
        let numbers = json(&rows);
        assert_eq!(numbers["mutation_red"].as_u64(), Some(2));
        assert_eq!(numbers["mutation_runs"].as_u64(), Some(3));
        assert_eq!(json(&[])["mutation_red"], serde_json::Value::Null);
    }

    #[test]
    fn a_human_stops_before_the_build_once_and_at_every_g4_decision() {
        let rows = [
            by("20T09:00", "a", "human", "G1", "ok"),
            by("20T09:10", "a", "human", "zones", "ok"),
            prepared("20T09:30", "a"),
            by("20T09:40", "a", "human", "G3", "ok"),
            by("20T10:00", "a", "human", "G4", "polish"),
            by("20T11:00", "a", "human", "G4", "ok"),
            by("20T12:00", "b", "orchestrator", "-", "change-created"),
            prepared("20T12:10", "b"),
            by("20T12:20", "b", "human", "G4", "ok"),
            by("20T13:00", "c", "human", "G1", "ok"),
        ];
        has(&rows, "  Human stops: 1.7 per change (5 in 3)");
        has(&[], "  Human stops: no data yet");
        assert_eq!(json(&rows)["stops_per_change"], 5.0 / 3.0);
    }

    fn call(at: &str, change: &str, model: &str, tokens_out: u64) -> String {
        let line = row(at, change, "builder-build", "G2", "claude", "ok");
        let called = format!(r#""model": "{model}", "tokens_out": {tokens_out}"#);
        line.replace(r#""model": "-""#, &called)
    }

    #[test]
    fn tokens_out_is_the_median_over_the_changes_that_called_an_agent() {
        let rows = [
            call("20T10:00", "a", "opus", 800),
            call("20T10:10", "a", "opus", 200),
            call("20T11:00", "b", "sonnet", 300),
            call("20T12:00", "c", "sonnet", 2_000),
            row("20T13:00", "d", "machine", "G2", "unit", "ok"),
            call("20T14:00", "-", "sonnet", 99_999),
        ];
        has(&rows, "  Tokens out: median 1,000 per change");
        assert_eq!(json(&rows)["tokens_out_median"], 1000.0);
        let even = [call("20T10:00", "a", "o", 3), call("20T11:00", "b", "o", 4)];
        has(&even, "  Tokens out: median 4 per change");
        let odd = [call("20T10:00", "a", "opus", 999)];
        has(&odd, "  Tokens out: median 999 per change");
        let none = [row("20T13:00", "d", "machine", "G2", "unit", "ok")];
        has(&none, "  Tokens out: no data yet");
        assert_eq!(json(&none)["tokens_out_median"], serde_json::Value::Null);
    }

    #[test]
    fn only_the_changes_whose_first_line_is_on_or_after_the_day_count() {
        let rows = [
            by("20T23:59", "a", "orchestrator", "-", "change-created"),
            release("21T10:00", "a", "promote", "ok"),
            by("21T00:00", "b", "orchestrator", "-", "change-created"),
            release("21T11:00", "b", "promote", "ok"),
        ];
        let since = |day: &str| delivery_of(&rows, &Records::default(), Some(day));
        assert_eq!(since("2026-10-20").deployed.len(), 2);
        assert_eq!(since("2026-10-21").deployed.len(), 1);
        assert_eq!(since("2026-10-22").deployed.len(), 0);
        let headline = "Delivery · 1 deployment(s) from 2026-10-21 to 2026-10-21";
        assert_eq!(since("2026-10-21").headline(), headline);
    }

    #[test]
    fn thousands_get_a_comma_from_a_thousand() {
        let shown = [(0, "0"), (999, "999"), (1_000, "1,000"), (12_345, "12,345")];
        for (n, text) in shown {
            assert_eq!(thousands(n), text);
        }
        assert_eq!(thousands(1_234_567), "1,234,567");
    }

    #[test]
    fn days_are_shown_as_dates() {
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(date(-1), "1969-12-31");
        assert_eq!(date(11_016), "2000-02-29");
        assert_eq!(date(20_754), "2026-10-28");
        assert_eq!(date(20_818), "2026-12-31");
        assert_eq!(date(20_819), "2027-01-01");
    }

    #[test]
    fn the_report_has_its_blocks_in_order() {
        let rows = [
            prepared("20T09:00", "a"),
            release("20T10:00", "a", "promote", "ok"),
        ];
        let text = shown(&rows);
        let lines: Vec<&str> = text.lines().collect();
        let unindented = |line: &&str| !line.starts_with(' ');
        let heads: Vec<&str> = lines.iter().copied().filter(unindented).collect();
        assert_eq!(
            heads,
            [
                "Delivery · 1 deployment(s) from 2026-10-20 to 2026-10-20",
                "Throughput",
                "Instability",
                "GDD signals (signs to read, never targets)",
            ]
        );
        assert_eq!(lines.len(), 16);
    }
}
