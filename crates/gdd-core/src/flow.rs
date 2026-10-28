//! The time report: where the flow time of each change goes, from the lines of the log.
//!
//! The flow time of a change runs from its first line to its last; a line with a duration of `d`
//! seconds covers the `d` seconds before it was written. Part of the flow time is active: the
//! agent calls, the gate runs and the release steps. Part of it is waiting: at the sitting, until
//! the change starts, parked until relaunched, and for the human at G4. What is left is the time
//! between steps. Flow efficiency is active time over flow time (Flow Framework).

use std::collections::BTreeMap;
use std::fmt;

use serde::{Serialize, Serializer};

use crate::log::{self, LineError, Record};

/// Seconds in a day.
const DAY: i64 = 86_400;

/// The agents of the lines of the release steps.
const RELEASE_STEPS: [&str; 7] = [
    "build",
    "deploy_pre",
    "tests_pre",
    "demo",
    "promote",
    "observe",
    "rollback",
];

/// The gates whose runs leave a line per command.
const GATES: [&str; 2] = ["G2", "G3"];

/// Waits the project names, at most.
const BIGGEST: usize = 3;

/// A line of the log placed in time.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// When it was written: seconds since 1970-01-01T00:00:00 UTC.
    pub at: i64,
    /// The line itself.
    pub record: Record,
}

/// Reads the text of a log (see [`log::parse`]) and places each line in time: a line whose `ts`
/// is not a time as the log writes it is an error that names it too.
pub fn read(text: &str) -> Result<Vec<Line>, LineError> {
    let records = log::parse(text)?;
    let numbers = text
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(i, _)| i + 1);
    numbers
        .zip(records)
        .map(|(number, record)| {
            let ts = record.text("ts");
            let at = seconds(&ts).ok_or_else(|| LineError {
                line: number,
                message: format!("ts {ts:?} is not a time like 2026-10-28T10:00:00+00:00"),
            })?;
            Ok(Line { at, record })
        })
        .collect()
}

/// The day `text` names as `YYYY-MM-DD`, a date of the calendar, in days since 1970-01-01;
/// `None` when it names none.
pub fn day(text: &str) -> Option<i64> {
    let [year, month, day] = numbers(text, '-', [4, 2, 2])?;
    let valid = (1..=12).contains(&month) && (1..=days_in(year, month)).contains(&day);
    valid.then(|| days_from_civil(year, month, day))
}

/// The days of `month` of `year` in the Gregorian calendar.
fn days_in(year: i64, month: i64) -> i64 {
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Seconds since 1970-01-01T00:00:00 UTC of a time as the log writes it,
/// `2026-10-28T10:00:00+00:00`, with any offset under a day; `None` for anything else.
fn seconds(ts: &str) -> Option<i64> {
    let (date, rest) = ts.split_once('T')?;
    let (time, offset) = rest.split_at_checked(8)?;
    let [hour, minute, second] = clock(time)?;
    let (east, offset) = match offset.split_at_checked(1)? {
        ("+", offset) => (1, offset),
        ("-", offset) => (-1, offset),
        _ => return None,
    };
    let [offset_hours, offset_minutes] = clock(offset)?;
    let offset = east * (offset_hours * 3_600 + offset_minutes * 60);
    let seconds = hour * 3_600 + minute * 60 + second - offset;
    Some(day(date)? * DAY + seconds)
}

/// The hours, minutes and seconds of `text` as a clock shows them, `10:00:00` (or `01:00`
/// without seconds): two digits each, the hours under 24 and the rest under 60.
fn clock<const N: usize>(text: &str) -> Option<[i64; N]> {
    let numbers = numbers(text, ':', [2; N])?;
    let below = |(i, number): (usize, &i64)| *number < if i == 0 { 24 } else { 60 };
    numbers.iter().enumerate().all(below).then_some(numbers)
}

/// The `N` whole numbers of `text` separated by `separator`, each of digits only and as many as
/// its width in `widths`.
fn numbers<const N: usize>(text: &str, separator: char, widths: [usize; N]) -> Option<[i64; N]> {
    let mut parts = text.split(separator);
    let mut numbers = [0; N];
    for (number, width) in numbers.iter_mut().zip(widths) {
        let part = parts.next()?;
        if part.len() != width || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        *number = part.parse().ok()?;
    }
    parts.next().is_none().then_some(numbers)
}

/// Days since 1970-01-01 of a date of the Gregorian calendar. Howard Hinnant's
/// `days_from_civil`, on eras of 400 years that start on March 1st.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// A kind of waiting. Their order breaks ties between waits of the same time.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Wait {
    /// Parked until relaunched, with the note of the parking: its cause.
    Parked(String),
    /// For the human at G4.
    Human,
    /// At the sitting that created the change.
    Sitting,
    /// From the sitting until the change starts.
    Start,
}

impl Wait {
    /// Its name, without its cause.
    fn name(&self) -> &'static str {
        match self {
            Wait::Parked(_) => "parked",
            Wait::Human => "for the human at G4",
            Wait::Sitting => "at the sitting",
            Wait::Start => "to start",
        }
    }

    /// Its cause, when it has one.
    fn cause(&self) -> Option<&str> {
        match self {
            Wait::Parked(note) if !note.is_empty() => Some(note.as_str()),
            _ => None,
        }
    }

    /// Its name with its cause, as a key of the JSON: `parked (G3 still red)`.
    fn key(&self) -> String {
        match self.cause() {
            Some(cause) => format!("{} ({cause})", self.name()),
            None => self.name().to_string(),
        }
    }

    /// A wait of `seconds` of this kind as a change shows it: `parked 40m (G3 still red)`.
    fn shown(&self, seconds: f64) -> String {
        let shown = format!("{} {}", self.name(), duration(seconds));
        match self.cause() {
            Some(cause) => format!("{shown} ({cause})"),
            None => shown,
        }
    }
}

/// Where the flow time of one change went.
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    /// Its id.
    id: String,
    /// Its title: the note of its `change-created` line.
    title: String,
    /// `closed`, `rejected`, `rolled back` or `in progress`.
    state: &'static str,
    /// When its first line was written: seconds since 1970-01-01T00:00:00 UTC.
    start: i64,
    /// Seconds from its first line to its last.
    flow_s: f64,
    /// Seconds of the agent calls, by role and model, longest first.
    agents: Vec<(String, f64)>,
    /// Seconds of its gate runs: their `checks` lines.
    gates_s: f64,
    /// The same seconds by command, longest first.
    gates: Vec<(String, f64)>,
    /// Seconds of the release steps, by step, longest first.
    release: Vec<(String, f64)>,
    /// Seconds of waiting, by kind, longest first.
    waiting: Vec<(Wait, f64)>,
}

impl Change {
    /// The time of change `id` from its lines, in the order of the log.
    fn new(id: String, lines: &[&Line]) -> Change {
        let at = |line: Option<&&Line>| line.map_or(0, |line| line.at);
        let (start, end) = (at(lines.first()), at(lines.last()));
        let records: Vec<&Record> = lines.iter().map(|line| &line.record).collect();
        let (gates_s, gates) = gates(&records);
        Change {
            id,
            title: title(&records),
            state: state(&records),
            start,
            flow_s: (end - start) as f64,
            agents: sorted(agents(&records)),
            gates_s,
            gates: sorted(gates),
            release: sorted(release(&records)),
            waiting: sorted(waits(lines)),
        }
    }

    /// What its flow time adds up to. The time between steps is never below zero, even when
    /// lines overlap.
    fn totals(&self) -> Totals {
        let active_s = total(&self.agents) + self.gates_s + total(&self.release);
        let waiting_s = total(&self.waiting);
        Totals {
            flow_s: self.flow_s,
            active_s,
            waiting_s,
            between_s: (self.flow_s - active_s - waiting_s).max(0.0),
        }
    }
}

/// Whether `record` has `value` in `field` (see [`Record::text`]).
fn is(record: &Record, field: &str, value: &str) -> bool {
    record.text(field) == value
}

/// Whether `record` is a human's at `gate`.
fn human_at(record: &Record, gate: &str) -> bool {
    is(record, "role", "human") && is(record, "gate", gate)
}

/// The title of a change: the note of its `change-created` line; `-` without one.
fn title(records: &[&Record]) -> String {
    let created = records.iter().find(|r| is(r, "result", "change-created"));
    created.map_or_else(|| "-".to_string(), |r| r.text("note"))
}

/// The release step `record` is the line of, if any: a machine line of a step, outside the gates,
/// as the release logs them. A command of a gate may have the name of a step: its line is the
/// gate's.
fn step(record: &Record) -> Option<String> {
    let step = record.text("agent");
    let released = is(record, "role", "machine")
        && is(record, "gate", "-")
        && RELEASE_STEPS.contains(&step.as_str());
    released.then_some(step)
}

/// The state of a change after the last of its lines that decides one: `closed` after an
/// `observe` step that passed, `rejected` after a human G4 `ko`, `rolled back` after a
/// `rollback` step that passed; else `in progress`. A step passes `ok`, or `simulated` when the
/// release has no command for it.
fn state(records: &[&Record]) -> &'static str {
    let mut state = "in progress";
    for record in records {
        let passed = is(record, "result", "ok") || is(record, "result", "simulated");
        match step(record).as_deref() {
            Some("observe") if passed => state = "closed",
            Some("rollback") if passed => state = "rolled back",
            _ if human_at(record, "G4") && is(record, "result", "ko") => state = "rejected",
            _ => {}
        }
    }
    state
}

/// The agent calls: lines with a model and a duration, by `role · model`.
fn agents(records: &[&Record]) -> BTreeMap<String, f64> {
    let mut agents = BTreeMap::new();
    for record in records {
        let model = record.text("model");
        if model != "-" && record.duration_s > 0.0 {
            let key = format!("{} · {model}", record.text("role"));
            *agents.entry(key).or_default() += record.duration_s;
        }
    }
    agents
}

/// The release steps: the lines of a step (see [`step`]) with a duration, by step.
fn release(records: &[&Record]) -> BTreeMap<String, f64> {
    let mut steps = BTreeMap::new();
    for record in records {
        if let Some(step) = step(record)
            && record.duration_s > 0.0
        {
            *steps.entry(step).or_default() += record.duration_s;
        }
    }
    steps
}

/// The time of the gate runs, from their `checks` lines, and how it splits by command: the
/// machine lines of a gate just before its `checks` line are the commands of the run. A run
/// without them, as in older logs, counts under its gate.
fn gates(records: &[&Record]) -> (f64, BTreeMap<String, f64>) {
    let mut total = 0.0;
    let mut by_command = BTreeMap::new();
    let mut commands: Vec<&Record> = Vec::new();
    for record in records.iter().copied() {
        let gate = record.text("gate");
        if !is(record, "role", "machine") || !GATES.contains(&gate.as_str()) {
            commands.clear();
        } else if is(record, "agent", "checks") {
            total += record.duration_s;
            if commands.is_empty() {
                *by_command.entry(gate).or_default() += record.duration_s;
            }
            for command in commands.drain(..) {
                *by_command.entry(command.text("agent")).or_default() += command.duration_s;
            }
        } else {
            commands.push(record);
        }
    }
    (total, by_command)
}

/// The waits of a change, by kind, from its lines:
/// - at the sitting: from its first line to the last human line before the first `prepared`;
/// - to start: from that human line, else its first line, to the first `prepared`;
/// - parked: from each `parked` line to the next `prepared`, by the note of the parking;
/// - for the human at G4: from the line before each human G4 line to it.
fn waits(lines: &[&Line]) -> BTreeMap<Wait, f64> {
    let mut waits = BTreeMap::new();
    let mut add = |wait: Wait, from: i64, to: i64| {
        *waits.entry(wait).or_default() += (to - from) as f64;
    };
    let prepared = |line: &&&Line| is(&line.record, "result", "prepared");
    let first = lines.first().map_or(0, |line| line.at);
    let start = lines.iter().position(|line| prepared(&line));
    let before = &lines[..start.unwrap_or(lines.len())];
    let sitting = before.iter().rfind(|l| is(&l.record, "role", "human"));
    if let Some(human) = sitting {
        add(Wait::Sitting, first, human.at);
    }
    if let Some(started) = start.and_then(|i| lines.get(i)) {
        let from = sitting.map_or(first, |human| human.at);
        add(Wait::Start, from, started.at);
    }
    for (i, line) in lines.iter().enumerate() {
        let record = &line.record;
        if is(record, "result", "parked")
            && let Some(relaunched) = lines[i..].iter().find(prepared)
        {
            add(Wait::Parked(record.text("note")), line.at, relaunched.at);
        } else if human_at(record, "G4")
            && let Some(before) = i.checked_sub(1).and_then(|i| lines.get(i))
        {
            add(Wait::Human, before.at, line.at);
        }
    }
    waits
}

/// The times of `times`, longest first; ties keep the order of their keys.
fn sorted<K: Ord>(times: BTreeMap<K, f64>) -> Vec<(K, f64)> {
    let mut sorted: Vec<(K, f64)> = times.into_iter().collect();
    sorted.sort_by(|(_, a), (_, b)| b.total_cmp(a));
    sorted
}

/// The seconds of `times`, added up.
fn total<K>(times: &[(K, f64)]) -> f64 {
    times.iter().map(|(_, seconds)| seconds).sum()
}

/// Whole seconds, rounded half up; none below zero.
fn whole(seconds: f64) -> u64 {
    (seconds + 0.5).floor() as u64
}

/// A duration as the report shows it: under a minute `Ns`; under an hour `Nm`; else `Hh MMm`;
/// minutes rounded half up.
fn duration(seconds: f64) -> String {
    let seconds = whole(seconds);
    let minutes = (seconds + 30) / 60;
    if seconds < 60 {
        format!("{seconds}s")
    } else if minutes < 60 {
        format!("{minutes}m")
    } else {
        format!("{}h {:02}m", minutes / 60, minutes % 60)
    }
}

/// Each of `times` as `name duration`.
fn named(times: &[(String, f64)]) -> Vec<String> {
    let shown = |(name, s): &(String, f64)| format!("{name} {}", duration(*s));
    times.iter().map(shown).collect()
}

/// What a flow time adds up to, in seconds.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Totals {
    flow_s: f64,
    active_s: f64,
    waiting_s: f64,
    between_s: f64,
}

impl Totals {
    /// Adds `other` to these totals.
    fn add(&mut self, other: Totals) {
        self.flow_s += other.flow_s;
        self.active_s += other.active_s;
        self.waiting_s += other.waiting_s;
        self.between_s += other.between_s;
    }

    /// Active time over flow time, as a whole percent rounded half up; 0 without flow time.
    fn efficiency(&self) -> u64 {
        if self.flow_s > 0.0 {
            whole(self.active_s * 100.0 / self.flow_s)
        } else {
            0
        }
    }
}

/// The flow time and how it splits, as it follows `flow time`: `2h 30m · active 47m (31%) ·
/// waiting 1h 40m · between steps 3m`.
impl fmt::Display for Totals {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} · active {} ({}%) · waiting {} · between steps {}",
            duration(self.flow_s),
            duration(self.active_s),
            self.efficiency(),
            duration(self.waiting_s),
            duration(self.between_s)
        )
    }
}

/// A change as the human reads it: its id, title and state, its flow time and how it splits,
/// then a line for each list that is not empty.
impl fmt::Display for Change {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (id, title, state) = (&self.id, &self.title, self.state);
        let totals = self.totals();
        write!(f, "{id} · {title} · {state}\n  Flow time {totals}")?;
        let waiting = self.waiting.iter().map(|(wait, s)| wait.shown(*s));
        for (list, items) in [
            ("Agents", named(&self.agents)),
            ("Gates", named(&self.gates)),
            ("Release", named(&self.release)),
            ("Waiting", waiting.collect()),
        ] {
            if !items.is_empty() {
                write!(f, "\n  {list}: {}", items.join(", "))?;
            }
        }
        Ok(())
    }
}

/// As JSON, a change is an object: its id, title and state, its totals in whole seconds, its
/// efficiency, and each list as an object of whole seconds by name.
impl Serialize for Change {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let totals = self.totals();
        Entry {
            change: &self.id,
            title: &self.title,
            state: self.state,
            flow_s: whole(totals.flow_s),
            active_s: whole(totals.active_s),
            waiting_s: whole(totals.waiting_s),
            between_s: whole(totals.between_s),
            efficiency: totals.efficiency(),
            agents: Seconds(self.agents.clone()),
            gates: Seconds(self.gates.clone()),
            release: Seconds(self.release.clone()),
            waiting: Seconds(self.waiting.iter().map(|(w, s)| (w.key(), *s)).collect()),
        }
        .serialize(serializer)
    }
}

/// A change as JSON.
#[derive(Serialize)]
struct Entry<'a> {
    change: &'a str,
    title: &'a str,
    state: &'a str,
    flow_s: u64,
    active_s: u64,
    waiting_s: u64,
    between_s: u64,
    efficiency: u64,
    agents: Seconds,
    gates: Seconds,
    release: Seconds,
    waiting: Seconds,
}

/// Times by name, as a JSON object of whole seconds in their order.
struct Seconds(Vec<(String, f64)>);

impl Serialize for Seconds {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let times = self.0.iter().map(|(name, s)| (name, whole(*s)));
        serializer.collect_map(times)
    }
}

/// Where the flow time of the changes of a log went.
#[derive(Debug, Clone, PartialEq)]
pub struct Flow {
    /// The change asked for, if one was.
    asked: Option<String>,
    /// The changes, in the order they were created.
    changes: Vec<Change>,
}

impl Flow {
    /// The time of each change of `lines` (see [`read`]), in the order they were created: only
    /// `change` when given, and only the changes whose first line is on or after the day `since`
    /// (see [`day`]) when given. Lines of no change (`-`) are left out.
    pub fn new(lines: &[Line], change: Option<&str>, since: Option<i64>) -> Flow {
        let mut groups: Vec<(String, Vec<&Line>)> = Vec::new();
        for line in lines {
            let id = line.record.text("change");
            if id == "-" || change.is_some_and(|asked| asked != id) {
                continue;
            }
            match groups.iter_mut().find(|(known, _)| *known == id) {
                Some((_, group)) => group.push(line),
                None => groups.push((id, vec![line])),
            }
        }
        let changes = groups
            .into_iter()
            .map(|(id, group)| Change::new(id, &group))
            .filter(|change| since.is_none_or(|day| change.start >= day * DAY))
            .collect();
        Flow {
            asked: change.map(str::to_string),
            changes,
        }
    }
}

/// The report as the human reads it. For one change, that change, or a line that says the log
/// does not have it. Otherwise every change, then the project: what they add up to and the
/// biggest waits of all, a parking by its cause.
impl fmt::Display for Flow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(id) = &self.asked {
            return match self.changes.first() {
                Some(change) => write!(f, "{change}"),
                None => write!(f, "No change {id} in the log."),
            };
        }
        let mut project = Totals::default();
        let mut waits = BTreeMap::<Wait, f64>::new();
        for change in &self.changes {
            writeln!(f, "{change}\n")?;
            project.add(change.totals());
            for (wait, seconds) in &change.waiting {
                *waits.entry(wait.clone()).or_default() += seconds;
            }
        }
        let count = self.changes.len();
        write!(f, "Project: {count} change(s) · flow time {project}")?;
        let biggest: Vec<String> = sorted(waits)
            .into_iter()
            .take(BIGGEST)
            .map(|(wait, seconds)| format!("{} {}", wait.name(), duration(seconds)))
            .collect();
        if !biggest.is_empty() {
            write!(f, "\nBiggest waits: {}", biggest.join(", "))?;
        }
        Ok(())
    }
}

/// As JSON, the report is a list with one object per change (see [`Change`]).
impl Serialize for Flow {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(&self.changes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Three changes and two lines of none: `x` closed, `y` in progress and parked, `z`
    /// rejected at G4; `y` and `z` from the next day.
    const LOG: &str = r#"{"ts": "2026-10-28T07:00:00+00:00", "change": "-", "gate": "G0", "role": "machine", "agent": "selftest", "duration_s": 99}
{"ts": "2026-10-28T08:00:00+00:00", "change": "x", "role": "orchestrator", "result": "change-created", "note": "Export"}
{"ts": "2026-10-28T08:15:00+00:00", "change": "x", "gate": "zones", "role": "human", "result": "ok"}
{"ts": "2026-10-28T08:25:00+00:00", "change": "x", "gate": "G1", "role": "human", "result": "ok"}
{"ts": "2026-10-28T08:40:00+00:00", "change": "x", "role": "machine", "result": "prepared"}
{"ts": "2026-10-28T08:50:00+00:00", "change": "x", "gate": "G2", "role": "builder-test", "agent": "claude", "model": "sonnet", "tokens_out": 9, "duration_s": 300, "result": "ok"}
{"ts": "2026-10-28T09:10:00+00:00", "change": "x", "gate": "G2", "role": "builder-build", "agent": "claude", "model": "opus", "tokens_out": 9, "duration_s": 1200, "result": "ok"}
{"ts": "2026-10-28T09:10:30+00:00", "change": "x", "gate": "G2", "role": "machine", "agent": "lint", "duration_s": 30, "result": "ok"}
{"ts": "2026-10-28T09:12:00+00:00", "change": "x", "gate": "G2", "role": "machine", "agent": "unit", "duration_s": 90, "result": "ok"}
{"ts": "2026-10-28T09:12:00+00:00", "change": "x", "gate": "G2", "role": "machine", "agent": "path-guard", "duration_s": 0.0, "result": "ok"}
{"ts": "2026-10-28T09:12:00+00:00", "change": "x", "gate": "G2", "role": "machine", "agent": "checks", "duration_s": 120, "result": "ok"}
{"ts": "2026-10-28T09:20:00+00:00", "change": "x", "gate": "G3", "role": "machine", "agent": "mutation", "duration_s": 480, "result": "ko"}
{"ts": "2026-10-28T09:20:00+00:00", "change": "x", "gate": "G3", "role": "machine", "agent": "checks", "duration_s": 480, "result": "ko"}
{"ts": "2026-10-28T09:20:00+00:00", "change": "x", "role": "machine", "result": "parked", "note": "mutants survived"}
{"ts": "2026-10-28T10:20:00+00:00", "change": "x", "role": "machine", "result": "prepared"}
{"ts": "2026-10-28T10:25:00+00:00", "change": "x", "gate": "G3", "role": "machine", "agent": "checks", "duration_s": 300, "result": "ok"}
{"ts": "2026-10-28T10:27:00+00:00", "change": "x", "gate": "G3", "role": "reviewer", "agent": "codex", "model": "gpt-6", "tokens_out": 9, "duration_s": 120, "result": "ok"}
{"ts": "2026-10-28T10:27:00+00:00", "change": "x", "gate": "G3", "role": "machine", "agent": "verdict", "result": "ok"}
{"ts": "2026-10-28T10:27:00+00:00", "change": "x", "gate": "G3", "role": "machine", "agent": "merge", "result": "ok"}
{"ts": "2026-10-28T10:30:00+00:00", "change": "x", "role": "machine", "agent": "build", "duration_s": 180, "result": "ok"}
{"ts": "2026-10-28T10:31:00+00:00", "change": "x", "role": "machine", "agent": "demo", "duration_s": 60, "result": "ok"}
{"ts": "2026-10-28T11:01:00+00:00", "change": "x", "gate": "G4", "role": "human", "result": "ok"}
{"ts": "2026-10-28T11:01:00+00:00", "change": "x", "role": "machine", "agent": "promote", "result": "ok"}
{"ts": "2026-10-28T11:01:00+00:00", "change": "x", "role": "machine", "agent": "observe", "result": "ok"}

{"ts": "2026-10-29T09:00:00+00:00", "change": "y", "role": "orchestrator", "result": "change-created", "note": "Import"}
{"ts": "2026-10-29T09:10:00+00:00", "change": "y", "role": "machine", "result": "prepared"}
{"ts": "2026-10-29T09:30:00+00:00", "change": "y", "gate": "G2", "role": "builder-build", "agent": "claude", "model": "sonnet", "tokens_out": 9, "duration_s": 900, "result": "ok"}
{"ts": "2026-10-29T09:31:00+00:00", "change": "y", "gate": "G2", "role": "machine", "agent": "unit", "duration_s": 60, "result": "ko"}
{"ts": "2026-10-29T09:31:00+00:00", "change": "y", "gate": "G2", "role": "machine", "agent": "checks", "duration_s": 60, "result": "ko"}
{"ts": "2026-10-29T12:00:00+00:00", "change": "z", "role": "orchestrator", "result": "change-created", "note": "Theme"}
{"ts": "2026-10-29T09:31:00+00:00", "change": "y", "role": "machine", "result": "parked", "note": "G2 red"}
{"ts": "2026-10-29T12:05:00+00:00", "change": "z", "gate": "G1", "role": "human", "result": "ok"}
{"ts": "2026-10-29T12:10:00+00:00", "change": "z", "role": "machine", "result": "prepared"}
{"ts": "2026-10-29T12:20:00+00:00", "change": "z", "role": "machine", "agent": "demo", "duration_s": 120, "result": "ok"}
{"ts": "2026-10-29T12:50:00+00:00", "role": "machine", "agent": "selftest", "result": "ok"}
{"ts": "2026-10-29T12:50:00+00:00", "change": "z", "gate": "G4", "role": "human", "result": "ko"}
"#;

    const X: &str = "x · Export · closed\n  \
        Flow time 3h 01m · active 46m (25%) · waiting 2h 10m · between steps 5m\n  \
        Agents: builder-build · opus 20m, builder-test · sonnet 5m, reviewer · gpt-6 2m\n  \
        Gates: mutation 8m, G3 5m, unit 2m, lint 30s, path-guard 0s\n  \
        Release: build 3m, demo 1m\n  \
        Waiting: parked 1h 00m (mutants survived), for the human at G4 30m, at the sitting 25m, \
        to start 15m";

    const Y: &str = "y · Import · in progress\n  \
        Flow time 31m · active 16m (52%) · waiting 10m · between steps 5m\n  \
        Agents: builder-build · sonnet 15m\n  \
        Gates: unit 1m\n  \
        Waiting: to start 10m";

    const Z: &str = "z · Theme · rejected\n  \
        Flow time 50m · active 2m (4%) · waiting 40m · between steps 8m\n  \
        Release: demo 2m\n  \
        Waiting: for the human at G4 30m, at the sitting 5m, to start 5m";

    fn flow(change: Option<&str>, since: Option<&str>) -> Flow {
        let lines = read(LOG).unwrap();
        Flow::new(&lines, change, since.map(|text| day(text).unwrap()))
    }

    /// The lines of `text`, one JSON object per line, placed in time.
    fn lines(text: &str) -> Vec<Line> {
        read(text).unwrap()
    }

    /// The change `c` of the lines of `text`.
    fn change(text: &str) -> Change {
        let lines = lines(text);
        Change::new("c".to_string(), &lines.iter().collect::<Vec<_>>())
    }

    /// The records of `text`, one JSON object per line.
    fn records(text: &str) -> Vec<Record> {
        log::parse(text).unwrap()
    }

    fn times(pairs: &[(&str, f64)]) -> Vec<(String, f64)> {
        pairs.iter().map(|(n, s)| (n.to_string(), *s)).collect()
    }

    #[test]
    fn each_change_shows_where_its_time_went() {
        assert_eq!(flow(Some("x"), None).to_string(), X);
        assert_eq!(flow(Some("y"), None).to_string(), Y);
        assert_eq!(flow(Some("z"), None).to_string(), Z);
    }

    #[test]
    fn a_change_not_in_the_log_says_so() {
        let missing = flow(Some("zzz"), None);
        assert_eq!(missing.to_string(), "No change zzz in the log.");
        assert_eq!(serde_json::to_string(&missing).unwrap(), "[]");
        let before = flow(Some("x"), Some("2026-10-29"));
        assert_eq!(before.to_string(), "No change x in the log.");
    }

    #[test]
    fn the_project_adds_every_change_up_and_names_the_biggest_waits() {
        let project = "Project: 3 change(s) · flow time 4h 22m · active 1h 04m (24%) · \
                       waiting 3h 00m · between steps 18m\n\
                       Biggest waits: parked 1h 00m, for the human at G4 1h 00m, \
                       at the sitting 30m";
        let all = format!("{X}\n\n{Y}\n\n{Z}\n\n{project}");
        assert_eq!(flow(None, None).to_string(), all);
    }

    #[test]
    fn only_the_changes_created_since_a_day_count() {
        let project = "Project: 2 change(s) · flow time 1h 21m · active 18m (22%) · \
                       waiting 50m · between steps 13m\n\
                       Biggest waits: for the human at G4 30m, to start 15m, at the sitting 5m";
        let since = format!("{Y}\n\n{Z}\n\n{project}");
        assert_eq!(flow(None, Some("2026-10-29")).to_string(), since);
        // A change whose first line is the first second of the day counts.
        const EDGE: &str = r#"{"ts": "2026-10-28T00:00:00+00:00", "change": "a"}
{"ts": "2026-10-28T10:01:00+00:00", "change": "b"}"#;
        let lines = lines(EDGE);
        let ids = |since: &str| -> Vec<String> {
            let flow = Flow::new(&lines, None, day(since));
            flow.changes.iter().map(|c| c.id.clone()).collect()
        };
        assert_eq!(ids("2026-10-28"), ["a", "b"]);
        assert_eq!(ids("2026-10-29"), Vec::<String>::new());
        assert_eq!(ids("2026-10-27"), ["a", "b"]);
    }

    #[test]
    fn without_changes_the_project_has_nothing_to_wait_for() {
        let flow = flow(None, Some("2026-11-01"));
        let empty = "Project: 0 change(s) · flow time 0s · active 0s (0%) · waiting 0s · \
                     between steps 0s";
        assert_eq!(flow.to_string(), empty);
        assert_eq!(serde_json::to_string(&flow).unwrap(), "[]");
    }

    #[test]
    fn biggest_waits_keep_each_cause_of_parking_apart() {
        const PARKINGS: &str = r#"{"ts": "2026-10-28T10:00:00+00:00", "change": "c", "result": "parked", "note": "a"}
{"ts": "2026-10-28T10:10:00+00:00", "change": "c", "result": "prepared"}
{"ts": "2026-10-28T10:20:00+00:00", "change": "c", "result": "parked", "note": "b"}
{"ts": "2026-10-28T10:25:00+00:00", "change": "c", "result": "prepared"}
{"ts": "2026-10-28T10:30:00+00:00", "change": "d", "result": "parked", "note": "a"}
{"ts": "2026-10-28T10:32:00+00:00", "change": "d", "result": "prepared"}"#;
        let lines = lines(PARKINGS);
        let shown = Flow::new(&lines, None, None).to_string();
        let biggest = "\nBiggest waits: parked 12m, to start 12m, parked 5m";
        assert!(shown.ends_with(biggest), "{shown}");
        let one = Flow::new(&lines, Some("c"), None).to_string();
        let waiting = "\n  Waiting: parked 10m (a), to start 10m, parked 5m (b)";
        assert!(one.ends_with(waiting), "{one}");
    }

    #[test]
    fn as_json_each_change_has_its_totals_and_lists_in_whole_seconds() {
        let json = serde_json::to_string(&flow(Some("x"), None)).unwrap();
        assert_eq!(
            json,
            concat!(
                r#"[{"change":"x","title":"Export","state":"closed","flow_s":10860,"#,
                r#""active_s":2760,"waiting_s":7800,"between_s":300,"efficiency":25,"#,
                r#""agents":{"builder-build · opus":1200,"builder-test · sonnet":300,"#,
                r#""reviewer · gpt-6":120},"#,
                r#""gates":{"mutation":480,"G3":300,"unit":90,"lint":30,"path-guard":0},"#,
                r#""release":{"build":180,"demo":60},"#,
                r#""waiting":{"parked (mutants survived)":3600,"for the human at G4":1800,"#,
                r#""at the sitting":1500,"to start":900}}]"#
            )
        );
        let all = serde_json::to_value(flow(None, None)).unwrap();
        assert_eq!(all.as_array().map(Vec::len), Some(3));
        assert_eq!(all[1]["change"], "y");
        assert_eq!(all[1]["waiting"], serde_json::json!({"to start": 600}));
        assert_eq!(all[2]["release"], serde_json::json!({"demo": 120}));
        assert_eq!(all[1]["release"], serde_json::json!({}));
    }

    #[test]
    fn seconds_are_rounded_half_up_in_the_json() {
        const ROUNDED: &str = r#"{"ts": "2026-10-28T10:00:00+00:00", "role": "machine", "agent": "demo", "duration_s": 2.5}
{"ts": "2026-10-28T10:01:00+00:00", "role": "machine", "agent": "build", "duration_s": 2.4}"#;
        let json = serde_json::to_value(change(ROUNDED)).unwrap();
        assert_eq!(json["release"], serde_json::json!({"demo": 3, "build": 2}));
        assert_eq!(json["active_s"], 5);
        assert_eq!(json["between_s"], 55);
    }

    #[test]
    fn lines_of_no_change_are_left_out_and_changes_keep_the_order_they_were_created_in() {
        let flow = flow(None, None);
        let ids: Vec<&str> = flow.changes.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["x", "y", "z"]);
        assert_eq!(flow.changes[1].flow_s, 1860.0);
    }

    #[test]
    fn a_change_adds_up_its_flow_time() {
        const ADDED: &str = r#"{"ts": "2026-10-28T10:00:00+00:00", "result": "change-created"}
{"ts": "2026-10-28T10:05:00+00:00", "role": "machine", "agent": "demo", "duration_s": 60}
{"ts": "2026-10-28T11:05:00+00:00", "role": "machine", "gate": "G2", "agent": "checks", "duration_s": 120}"#;
        let totals = Totals {
            flow_s: 3900.0,
            active_s: 180.0,
            waiting_s: 0.0,
            between_s: 3720.0,
        };
        assert_eq!(change(ADDED).totals(), totals);
    }

    #[test]
    fn the_time_between_steps_is_never_below_zero() {
        const OVERLAP: &str = r#"{"ts": "2026-10-28T10:00:00+00:00", "result": "prepared"}
{"ts": "2026-10-28T10:01:00+00:00", "role": "builder-build", "model": "opus", "duration_s": 600}"#;
        let totals = change(OVERLAP).totals();
        assert_eq!((totals.flow_s, totals.active_s), (60.0, 600.0));
        assert_eq!(totals.between_s, 0.0);
        assert_eq!(totals.efficiency(), 1000);
    }

    #[test]
    fn efficiency_is_a_whole_percent_rounded_half_up() {
        let efficiency = |flow_s, active_s| {
            let totals = Totals {
                flow_s,
                active_s,
                ..Totals::default()
            };
            totals.efficiency()
        };
        assert_eq!(efficiency(8.0, 1.0), 13);
        assert_eq!(efficiency(3.0, 1.0), 33);
        assert_eq!(efficiency(9000.0, 2800.0), 31);
        assert_eq!(efficiency(10.0, 10.0), 100);
        assert_eq!(efficiency(0.0, 50.0), 0);
        assert_eq!(efficiency(0.0, 0.0), 0);
    }

    #[test]
    fn totals_add_up() {
        let mut totals = Totals {
            flow_s: 1.0,
            active_s: 2.0,
            waiting_s: 3.0,
            between_s: 4.0,
        };
        totals.add(Totals {
            flow_s: 10.0,
            active_s: 20.0,
            waiting_s: 30.0,
            between_s: 40.0,
        });
        let expected = Totals {
            flow_s: 11.0,
            active_s: 22.0,
            waiting_s: 33.0,
            between_s: 44.0,
        };
        assert_eq!(totals, expected);
    }

    #[test]
    fn agents_are_the_lines_with_a_model_and_a_duration() {
        const CALLS: &str = r#"{"role": "builder-build", "model": "opus", "duration_s": 600}
{"role": "builder-build", "model": "opus", "duration_s": 300.5}
{"role": "builder-test", "model": "sonnet", "duration_s": 120}
{"role": "reviewer", "duration_s": 50}
{"role": "orchestrator", "model": "opus", "tokens_out": 5}"#;
        let records = records(CALLS);
        let found = sorted(agents(&records.iter().collect::<Vec<_>>()));
        let expected = [
            ("builder-build · opus", 900.5),
            ("builder-test · sonnet", 120.0),
        ];
        assert_eq!(found, times(&expected));
    }

    #[test]
    fn release_steps_are_machine_lines_of_a_step_with_a_duration() {
        const STEPS: &str = r#"{"role": "machine", "agent": "demo", "duration_s": 100}
{"role": "machine", "agent": "demo", "duration_s": 20}
{"role": "machine", "agent": "build", "duration_s": 60}
{"role": "machine", "agent": "rollback", "duration_s": 7}
{"role": "machine", "agent": "promote"}
{"role": "machine", "agent": "merge", "duration_s": 10}
{"role": "human", "agent": "demo", "duration_s": 30}
{"role": "machine", "gate": "-", "agent": "build", "duration_s": 5}
{"role": "machine", "gate": "G2", "agent": "build", "duration_s": 40}
{"role": "machine", "gate": "G3", "agent": "demo", "duration_s": 50}"#;
        let records = records(STEPS);
        let found = sorted(release(&records.iter().collect::<Vec<_>>()));
        let expected = [("demo", 120.0), ("build", 65.0), ("rollback", 7.0)];
        assert_eq!(found, times(&expected));
    }

    #[test]
    fn a_gate_command_with_the_name_of_a_release_step_is_only_the_gates() {
        const NAMED: &str = r#"{"ts": "2026-10-28T10:00:00+00:00", "result": "prepared"}
{"ts": "2026-10-28T10:01:00+00:00", "role": "machine", "gate": "G2", "agent": "build", "duration_s": 30}
{"ts": "2026-10-28T10:01:00+00:00", "role": "machine", "gate": "G2", "agent": "observe", "duration_s": 0, "result": "ok"}
{"ts": "2026-10-28T10:01:00+00:00", "role": "machine", "gate": "G2", "agent": "checks", "duration_s": 30}"#;
        let change = change(NAMED);
        assert_eq!(change.totals().active_s, 30.0);
        assert_eq!(change.gates, times(&[("build", 30.0), ("observe", 0.0)]));
        assert!(change.release.is_empty());
        assert_eq!(change.state, "in progress");
    }

    #[test]
    fn gate_runs_split_by_the_commands_just_before_them() {
        const RUNS: &str = r#"{"role": "machine", "gate": "G2", "agent": "lint", "duration_s": 20}
{"role": "machine", "gate": "G2", "agent": "unit", "duration_s": 40}
{"role": "machine", "gate": "G2", "agent": "checks", "duration_s": 60}
{"role": "builder-build", "gate": "G2", "agent": "claude", "model": "opus", "duration_s": 600}
{"role": "machine", "gate": "G2", "agent": "checks", "duration_s": 30}
{"role": "machine", "gate": "G3", "agent": "merge"}
{"role": "human", "gate": "G4", "result": "ok"}
{"role": "machine", "gate": "G3", "agent": "acceptance", "duration_s": 100}
{"role": "machine", "gate": "G3", "agent": "checks", "duration_s": 100}
{"role": "machine", "agent": "demo", "duration_s": 50}
{"role": "machine", "gate": "G3", "agent": "checks", "duration_s": 10}
{"role": "machine", "gate": "G3", "agent": "verdict"}
{"role": "builder-build", "gate": "G3", "model": "opus", "duration_s": 5}
{"role": "machine", "gate": "G2", "agent": "unit", "duration_s": 4}
{"role": "machine", "gate": "G2", "agent": "checks", "duration_s": 5}"#;
        let records = records(RUNS);
        let (total, by_command) = gates(&records.iter().collect::<Vec<_>>());
        assert_eq!(total, 205.0);
        let expected = [
            ("acceptance", 100.0),
            ("unit", 44.0),
            ("G2", 30.0),
            ("lint", 20.0),
            ("G3", 10.0),
        ];
        assert_eq!(sorted(by_command), times(&expected));
    }

    #[test]
    fn the_sitting_ends_at_the_last_human_line_before_the_change_is_prepared() {
        const SITTING: &str = r#"{"ts": "2026-10-28T10:00:00+00:00", "role": "orchestrator"}
{"ts": "2026-10-28T10:15:00+00:00", "role": "human", "gate": "zones"}
{"ts": "2026-10-28T10:25:00+00:00", "role": "human", "gate": "G1"}
{"ts": "2026-10-28T10:40:00+00:00", "result": "prepared"}
{"ts": "2026-10-28T10:50:00+00:00", "role": "human", "gate": "G3"}"#;
        let expected = [(Wait::Sitting, 1500.0), (Wait::Start, 900.0)];
        assert_eq!(change(SITTING).waiting, expected);
    }

    #[test]
    fn without_a_sitting_the_change_waits_only_to_start() {
        const UNSEATED: &str = r#"{"ts": "2026-10-28T10:00:00+00:00", "role": "orchestrator"}
{"ts": "2026-10-28T10:10:00+00:00", "result": "prepared"}
{"ts": "2026-10-28T10:20:00+00:00", "role": "human", "gate": "G1"}"#;
        assert_eq!(change(UNSEATED).waiting, [(Wait::Start, 600.0)]);
    }

    #[test]
    fn a_change_never_prepared_is_all_sitting() {
        const UNPREPARED: &str = r#"{"ts": "2026-10-28T10:00:00+00:00", "role": "orchestrator"}
{"ts": "2026-10-28T10:10:00+00:00", "role": "human", "gate": "G1"}
{"ts": "2026-10-28T10:30:00+00:00", "role": "orchestrator"}"#;
        assert_eq!(change(UNPREPARED).waiting, [(Wait::Sitting, 600.0)]);
        const ALONE: &str = r#"{"ts": "2026-10-28T10:00:00+00:00", "role": "machine"}"#;
        assert!(change(ALONE).waiting.is_empty());
    }

    #[test]
    fn a_parked_change_waits_until_it_is_prepared_again() {
        const PARKED: &str = r#"{"ts": "2026-10-28T10:00:00+00:00", "result": "prepared"}
{"ts": "2026-10-28T10:10:00+00:00", "result": "parked", "note": "red"}
{"ts": "2026-10-28T10:15:00+00:00", "role": "builder-build"}
{"ts": "2026-10-28T10:40:00+00:00", "result": "prepared"}
{"ts": "2026-10-28T10:50:00+00:00", "result": "parked", "note": "red"}
{"ts": "2026-10-28T10:55:00+00:00", "result": "prepared"}"#;
        let expected = [(Wait::Parked("red".into()), 2100.0), (Wait::Start, 0.0)];
        assert_eq!(change(PARKED).waiting, expected);
        const STILL: &str = r#"{"ts": "2026-10-28T10:00:00+00:00", "result": "prepared"}
{"ts": "2026-10-28T10:10:00+00:00", "result": "parked", "note": "red"}"#;
        assert_eq!(change(STILL).waiting, [(Wait::Start, 0.0)]);
    }

    #[test]
    fn the_human_at_g4_is_waited_for_from_the_line_before() {
        const AT_G4: &str = r#"{"ts": "2026-10-28T10:00:00+00:00", "role": "human", "gate": "G4", "result": "polish"}
{"ts": "2026-10-28T10:05:00+00:00", "result": "prepared"}
{"ts": "2026-10-28T10:10:00+00:00", "role": "machine", "agent": "demo"}
{"ts": "2026-10-28T10:40:00+00:00", "role": "human", "gate": "G4", "result": "ok"}
{"ts": "2026-10-28T10:45:00+00:00", "role": "human", "gate": "G3"}"#;
        // The first line has no line before it: only the sitting, empty, ends there.
        let expected = [
            (Wait::Human, 1800.0),
            (Wait::Start, 300.0),
            (Wait::Sitting, 0.0),
        ];
        assert_eq!(change(AT_G4).waiting, expected);
    }

    #[test]
    fn waits_of_the_same_time_keep_the_order_of_their_kinds() {
        let mut waits = BTreeMap::new();
        for wait in [
            Wait::Start,
            Wait::Sitting,
            Wait::Human,
            Wait::Parked("b".into()),
        ] {
            waits.insert(wait, 60.0);
        }
        waits.insert(Wait::Parked("a".into()), 60.0);
        waits.insert(Wait::Sitting, 61.0);
        let order: Vec<Wait> = sorted(waits).into_iter().map(|(w, _)| w).collect();
        let expected = [
            Wait::Sitting,
            Wait::Parked("a".into()),
            Wait::Parked("b".into()),
            Wait::Human,
            Wait::Start,
        ];
        assert_eq!(order, expected);
    }

    #[test]
    fn a_wait_shows_its_cause_only_when_it_has_one() {
        let red = Wait::Parked("G3 red".into());
        assert_eq!(red.shown(2400.0), "parked 40m (G3 red)");
        assert_eq!(red.key(), "parked (G3 red)");
        let blank = Wait::Parked(String::new());
        assert_eq!(blank.shown(30.0), "parked 30s");
        assert_eq!(blank.key(), "parked");
        assert_eq!(Wait::Human.shown(60.0), "for the human at G4 1m");
        assert_eq!(Wait::Sitting.key(), "at the sitting");
        assert_eq!(Wait::Start.key(), "to start");
    }

    #[test]
    fn the_state_follows_the_last_line_that_decides_one() {
        let state_of = |text: &str| {
            let records = records(text);
            state(&records.iter().collect::<Vec<_>>())
        };
        let observed = r#"{"role": "machine", "agent": "observe", "result": "ok"}"#;
        let worse = r#"{"role": "machine", "agent": "observe", "result": "ko"}"#;
        let rolled = r#"{"role": "machine", "agent": "rollback", "result": "ok"}"#;
        let stuck = r#"{"role": "machine", "agent": "rollback", "result": "ko"}"#;
        let refused = r#"{"role": "human", "gate": "G4", "result": "ko"}"#;
        let accepted = r#"{"role": "human", "gate": "G4", "result": "ok"}"#;
        let by_machine = r#"{"role": "machine", "gate": "G4", "result": "ko"}"#;
        let at_g3 = r#"{"role": "human", "gate": "G3", "result": "ko"}"#;
        // Commands of a gate, and lines of no machine, with the name of a step.
        let in_g2 = r#"{"role": "machine", "gate": "G2", "agent": "observe", "result": "ok"}"#;
        let in_g3 = r#"{"role": "machine", "gate": "G3", "agent": "rollback"}"#;
        let human_observe = r#"{"role": "human", "agent": "observe", "result": "ok"}"#;
        assert_eq!(state_of(""), "in progress");
        assert_eq!(state_of(observed), "closed");
        assert_eq!(state_of(worse), "in progress");
        assert_eq!(state_of(stuck), "in progress");
        assert_eq!(state_of(rolled), "rolled back");
        assert_eq!(state_of(refused), "rejected");
        for undecided in [accepted, by_machine, at_g3, in_g2, in_g3, human_observe] {
            assert_eq!(state_of(undecided), "in progress", "{undecided}");
        }
        let closed = format!("{refused}\n{observed}");
        assert_eq!(state_of(&closed), "closed");
        let rolled_back = format!("{observed}\n{rolled}");
        assert_eq!(state_of(&rolled_back), "rolled back");
        let rejected = format!("{worse}\n{rolled}\n{refused}");
        assert_eq!(state_of(&rejected), "rejected");
        let still_rejected = format!("{refused}\n{worse}\n{stuck}");
        assert_eq!(state_of(&still_rejected), "rejected");
    }

    #[test]
    fn a_simulated_observe_closes_the_change_and_a_simulated_rollback_rolls_it_back() {
        // As a release with `simulated = true` logs its steps without a command.
        const OBSERVED: &str = r#"{"ts": "2026-10-28T10:00:00+00:00", "change": "c", "phase": "4-release", "gate": "-", "role": "machine", "agent": "promote", "model": "-", "duration_s": 0, "result": "simulated"}
{"ts": "2026-10-28T10:00:01+00:00", "change": "c", "phase": "4-release", "gate": "-", "role": "machine", "agent": "observe", "model": "-", "duration_s": 0, "result": "simulated"}"#;
        assert_eq!(change(OBSERVED).state, "closed");
        const ROLLED: &str = r#"{"ts": "2026-10-28T10:00:00+00:00", "change": "c", "phase": "4-release", "gate": "-", "role": "machine", "agent": "observe", "model": "-", "duration_s": 4, "result": "ko"}
{"ts": "2026-10-28T10:00:01+00:00", "change": "c", "phase": "4-release", "gate": "-", "role": "machine", "agent": "rollback", "model": "-", "duration_s": 0, "result": "simulated"}"#;
        assert_eq!(change(ROLLED).state, "rolled back");
    }

    #[test]
    fn the_title_is_the_note_of_the_line_that_created_the_change() {
        const TITLED: &str = r#"{"ts": "2026-10-28T10:00:00+00:00", "result": "ok", "note": "no"}
{"ts": "2026-10-28T10:01:00+00:00", "result": "change-created", "note": "Drafts"}"#;
        assert_eq!(change(TITLED).title, "Drafts");
        const UNTITLED: &str = r#"{"ts": "2026-10-28T10:00:00+00:00", "result": "ok"}"#;
        assert_eq!(change(UNTITLED).title, "-");
    }

    #[test]
    fn durations_are_seconds_minutes_or_hours_and_minutes() {
        for (seconds, shown) in [
            (0.0, "0s"),
            (0.4, "0s"),
            (0.5, "1s"),
            (59.4, "59s"),
            (59.5, "1m"),
            (89.0, "1m"),
            (90.0, "2m"),
            (3569.0, "59m"),
            (3570.0, "1h 00m"),
            (9000.0, "2h 30m"),
            (12_180.0, "3h 23m"),
            (37_000.0, "10h 17m"),
            (-5.0, "0s"),
        ] {
            assert_eq!(duration(seconds), shown, "{seconds}");
        }
    }

    #[test]
    fn times_are_read_as_the_log_writes_them() {
        for (ts, seconds) in [
            ("1970-01-01T00:00:00+00:00", 0),
            ("2000-02-28T23:59:59+00:00", 951_782_399),
            ("2000-02-29T00:00:00+00:00", 951_782_400),
            ("2000-03-01T00:00:00+00:00", 951_868_800),
            ("2023-11-14T22:13:20+00:00", 1_700_000_000),
            ("2023-11-14T23:13:20+01:00", 1_700_000_000),
            ("2023-11-14T21:13:20-01:00", 1_700_000_000),
            ("2023-11-14T22:43:20+00:30", 1_700_000_000),
            ("2023-11-15T22:12:20+23:59", 1_700_000_000),
            ("2024-02-29T00:00:00+00:00", 1_709_164_800),
            ("2024-12-31T23:59:59+00:00", 1_735_689_599),
            ("2100-03-01T00:00:00+00:00", 4_107_542_400),
        ] {
            assert_eq!(super::seconds(ts), Some(seconds), "{ts}");
        }
        for ts in [
            "-",
            "2023-11-14 22:13:20+00:00",
            "2023-11-14T22:13+00:00",
            "2023-11-14T22:13:20",
            "2023-11-14T22:13:20Z",
            "2023-11-14T22:13:20*00:00",
            "2023-11-14T22:13:20+0000",
            "2023-11-14T22:13:20+00:00:00",
            "2023-11-14T22:13:2a+00:00",
            "2023-13-14T22:13:20+00:00",
            "2023-11-14T22:13:20+00:00 ",
            "2023-02-29T22:13:20+00:00",
            "2023-11-14T24:00:00+00:00",
            "2023-11-14T22:60:20+00:00",
            "2023-11-14T22:13:60+00:00",
            "2023-11-14T22:13:20+24:00",
            "2023-11-14T22:13:20+00:60",
            "2023-11-14T2:13:200+00:00",
            "2023-11-14T+2:13:20+00:00",
            "2023-11-14T22:13:20+0:000",
            "9223372036854775807-01-01T00:00:00+00:00",
        ] {
            assert_eq!(super::seconds(ts), None, "{ts}");
        }
    }

    #[test]
    fn days_are_dates_of_the_calendar() {
        for (text, days) in [
            ("1970-01-01", 0),
            ("1969-12-31", -1),
            ("2000-01-31", 10_987),
            ("2000-02-29", 11_016),
            ("2000-03-01", 11_017),
            ("2024-02-29", 19_782),
            ("2025-01-01", 20_089),
            ("2026-04-30", 20_573),
            ("2026-10-28", 20_754),
            ("2026-12-31", 20_818),
            ("2100-03-01", 47_541),
        ] {
            assert_eq!(day(text), Some(days), "{text}");
        }
        for text in [
            "",
            "2026-10",
            "2026-10-28-1",
            "2026/10/28",
            "+2026-10-28",
            "2026-10-x",
            "2026-13-01",
            "2026-00-10",
            "2026-10-32",
            "2026-10-00",
            "99999999999999999999-01-01",
            "9223372036854775807-01-01",
            "02026-10-28",
            "2026-1-05",
            "2026-10-+1",
            "2026-02-29",
            "2026-02-30",
            "2023-02-29",
            "1900-02-29",
            "2026-04-31",
            "2026-06-31",
            "2026-09-31",
            "2026-11-31",
        ] {
            assert_eq!(day(text), None, "{text}");
        }
    }

    #[test]
    fn a_line_without_a_time_is_an_error_that_names_it() {
        let placed = "\n{\"ts\": \"2026-10-28T10:00:00+00:00\"}\n";
        let text = format!("{placed}\n{{\"ts\": \"10:00\"}}\n");
        let err = read(&text).unwrap_err();
        assert_eq!(err.line, 4);
        let said = "ts \"10:00\" is not a time like 2026-10-28T10:00:00+00:00";
        assert_eq!(err.message, said);
        let left_out = said.replacen("10:00", "-", 1);
        assert_eq!(read("{}").unwrap_err().message, left_out);
        assert_eq!(read("{}\n{x").unwrap_err().line, 2);
        let lines = lines(placed);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].at, 1_793_181_600);
    }
}
