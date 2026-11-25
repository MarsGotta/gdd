//! Metrics, derived only from the logs, the journals, the blueprint and git, so they can always be
//! recomputed. Tidy tables, each a list of rows with the same columns, under one `schema`.

use crate::blueprint::Blueprint;
use crate::judged::GitView;
use crate::limits;
use crate::log::{self, Entry, kind, step, str_of, ts};
use crate::state::{LogKind, log_kind};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const SCHEMA: &str = "gdd-metrics/1";
pub const TABLES: [&str; 8] = [
    "changes",
    "steps",
    "checks",
    "asks",
    "findings",
    "engine",
    "claims",
    "hardening",
];

pub type Row = Map<String, Value>;

/// What git adds to a change's row, when there is git.
#[derive(Debug, Clone, Default)]
pub struct ChangeGit {
    pub files: Option<i64>,
    pub added: Option<i64>,
    pub deleted: Option<i64>,
    /// Once it shipped and git was read: the revert of what it shipped, or that there is none.
    pub reverted: Option<Option<crate::git::Undone>>,
}

pub struct Input<'a> {
    pub id: &'a str,
    pub entries: &'a [Entry],
    pub journal: &'a [Value],
    pub git: ChangeGit,
    pub scenarios: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct Tables {
    pub changes: Vec<Row>,
    pub steps: Vec<Row>,
    pub checks: Vec<Row>,
    pub claims: Vec<Row>,
    pub hardening: Vec<Row>,
    pub asks: Vec<Row>,
    pub findings: Vec<Row>,
    pub engine: Vec<Row>,
}

impl Tables {
    pub fn get(&self, name: &str) -> &Vec<Row> {
        match name {
            "changes" => &self.changes,
            "steps" => &self.steps,
            "checks" => &self.checks,
            "claims" => &self.claims,
            "hardening" => &self.hardening,
            "asks" => &self.asks,
            "findings" => &self.findings,
            _ => &self.engine,
        }
    }
}

fn row(pairs: Vec<(&str, Value)>) -> Row {
    pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

fn minutes(secs: i64) -> Value {
    json!(((secs as f64) / 60.0 * 10.0).round() / 10.0)
}

/// One kind of tokens of a launch, summed over its models, or null when it reported none.
fn usage_sum(usage: &Value, key: &str) -> Value {
    match usage.as_object() {
        Some(m) if !m.is_empty() => json!(m.values().filter_map(|u| u[key].as_i64()).sum::<i64>()),
        _ => Value::Null,
    }
}

const TIMED: [&str; 5] = ["checks", "build", "gates", "review", "reading"];

fn cap_keys() -> [(&'static str, &'static str, &'static str); 6] {
    [
        ("rounds", "rounds_per_change", "rounds"),
        (
            "sent_back_checks",
            "fix_rounds_per_gate",
            "sent back by the checks",
        ),
        (
            "sent_back_review",
            "fix_rounds_per_gate",
            "sent back by the review",
        ),
        ("minutes", "minutes_per_change", "minutes"),
        (
            "runs_before_review",
            "runs_before_review",
            "runs before the review",
        ),
        ("tokens", "tokens_per_change", "tokens"),
    ]
}

/// Build every table from the changes given. `named` is when a hardening record first named
/// each change, from the blueprint's log, which may not be among the inputs.
pub fn tables(bp: &Blueprint, inputs: &[Input], named: &BTreeMap<String, Vec<i64>>, now: i64) -> Tables {
    let late: Vec<String> = bp
        .checks()
        .into_iter()
        .filter(|c| c.before_review())
        .map(|c| c.id)
        .collect();
    let mut t = Tables::default();
    for input in inputs {
        let id = input.id;
        let events = log::events(input.entries);
        let layer = crate::journal::lines_written(input.journal, input.entries);
        let state = crate::state::derive(
            id,
            bp,
            input.entries,
            GitView::default(),
            named,
            input.git.reverted.clone().flatten(),
            now,
        );
        let mut n = Totals::default();
        step_rows(&mut t.steps, &mut n, id, &events, &layer);
        check_rows(&mut t.checks, &mut n, id, &events, &late);
        claim_rows(&mut t.claims, id, &events);
        hardening_rows(&mut t.hardening, &events);
        ask_rows(&mut t.asks, &mut n, id, &state.progress.asks, now);
        finding_rows(&mut t.findings, id, &events);
        engine_rows(&mut t.engine, &mut n, input.journal);
        if log_kind(id) != LogKind::Blueprint {
            t.changes
                .push({
                    let mut row = change_row(bp, input, &events, &layer, &state, &n, now);
                    // A record counts once the fix was briefed: an older one looked at another thing.
                    row.insert(
                        "fix_hardened".into(),
                        json!(state.progress.fix_hardened(named).map(|(_, h)| h)),
                    );
                    row
                });
        }
    }
    t
}

/// What the rows of one log add up to, for its own row in `changes`.
#[derive(Default)]
struct Totals {
    steps: usize,
    suspect: usize,
    role_seconds: f64,
    tokens: i64,
    gates_runs: usize,
    gates_first_try: Option<bool>,
    asks: usize,
    asks_answered: usize,
    invocations: usize,
    refusals: usize,
    launches: usize,
    not_started: usize,
    timed_out: usize,
}

type Events<'a> = [(usize, &'a Value)];

/// One row for each step, with the seconds since the event before it and whether the layer wrote it.
fn step_rows(
    rows: &mut Vec<Row>,
    n: &mut Totals,
    id: &str,
    events: &Events,
    layer: &BTreeSet<usize>,
) {
    let mut prev_t: Option<i64> = None;
    let mut prev_step_t: Option<i64> = None;
    let mut prev_step_line: Option<usize> = None;
    for (line, v) in events {
        let t_ = ts(v);
        let st = step(v);
        if !st.is_empty() {
            n.steps += 1;
            let r = &v["report"];
            let since = match (t_, prev_t) {
                (Some(a), Some(b)) => json!(a - b),
                _ => Value::Null,
            };
            let sus = TIMED.contains(&st)
                && t_.is_some()
                && t_ == prev_step_t
                && !(layer.contains(line) && prev_step_line.is_some_and(|l| layer.contains(&l)));
            // Written by the layer, by its journal.
            let by_layer = layer.contains(line);
            let uncited = v["uncited"].as_array().map(|ids| {
                ids.iter()
                    .filter_map(Value::as_str)
                    .collect::<BTreeSet<_>>()
                    .len()
            });
            if sus {
                n.suspect += 1;
            }
            if let Some(s) = r["seconds"].as_f64() {
                n.role_seconds += s;
            }
            if let Some(k) = r["tokens"].as_i64() {
                n.tokens += k;
            }
            rows.push(row(vec![
                ("change", json!(id)),
                ("line", json!(line)),
                ("ts", v["ts"].clone()),
                ("step", json!(st)),
                ("role", r["skill"].clone()),
                ("skill_version", r["version"].clone()),
                ("level", r["level"].clone()),
                ("model", r["model"].clone()),
                ("round", r["round"].clone()),
                (
                    "result",
                    if st == "gates" {
                        json!(if log::any_kv(v, "result", "fail") {
                            "fail"
                        } else {
                            "pass"
                        })
                    } else if st == "review" {
                        v["verdict"].clone()
                    } else {
                        r["result"].clone()
                    },
                ),
                ("caught", r["caught"].clone()),
                ("uncited", json!(uncited)),
                ("seconds", r["seconds"].clone()),
                ("tokens", r["tokens"].clone()),
                ("tokens_read", r["tokens_read"].clone()),
                ("seconds_since_previous", since),
                ("suspect_time", json!(sus)),
                ("by_layer", json!(by_layer)),
            ]));
            prev_step_t = t_;
            prev_step_line = Some(*line);
        }
        if t_.is_some() {
            prev_t = t_;
        }
    }
}

/// One row for each check of each run of the gates.
fn check_rows(rows: &mut Vec<Row>, n: &mut Totals, id: &str, events: &Events, late: &[String]) {
    for (line, v) in events {
        if step(v) != "gates" {
            continue;
        }
        n.gates_runs += 1;
        let failed = log::any_kv(v, "result", "fail");
        if n.gates_first_try.is_none() {
            n.gates_first_try = Some(!failed);
        }
        let ran = v["gates"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|g| (g, false));
        let skipped = v["skipped"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|g| (g, true));
        for (g, skipped) in ran.chain(skipped) {
            let c = str_of(g, "check").to_string();
            rows.push(row(vec![
                ("change", json!(id)),
                ("run", json!(n.gates_runs)),
                ("line", json!(line)),
                ("head", v["head"].clone()),
                ("start", v["start"].clone()),
                ("check", json!(c)),
                (
                    "tier",
                    json!(if late.contains(&c) {
                        "before review"
                    } else {
                        "each round"
                    }),
                ),
                ("command", g["command"].clone()),
                ("exit", g["exit"].clone()),
                ("seconds", g["seconds"].clone()),
                (
                    "result",
                    if skipped {
                        json!("skipped")
                    } else {
                        g["result"].clone()
                    },
                ),
                ("why", g["why"].clone()),
            ]));
        }
    }
}

fn claim_rows(rows: &mut Vec<Row>, id: &str, events: &Events) {
    for (line, v) in events.iter().filter(|(_, v)| step(v) == "build") {
        for claim in v["claims"].as_array().into_iter().flatten() {
            rows.push(row(vec![
                ("change", json!(id)),
                ("line", json!(line)),
                ("round", v["report"]["round"].clone()),
                ("check", claim["check"].clone()),
                ("failures", json!(claim["failures"].as_u64())),
                ("command", claim["command"].clone()),
                ("exit", claim["exit"].clone()),
                ("tail", claim["tail"].clone()),
            ]));
        }
    }
}

fn hardening_rows(rows: &mut Vec<Row>, events: &Events) {
    for (line, v) in events.iter().filter(|(_, v)| step(v) == "harden") {
        let mut h = row(vec![
            ("line", json!(line)),
            ("result", v["report"]["result"].clone()),
        ]);
        for field in [
            "change",
            "crack",
            "expected_check",
            "caught_by",
            "action",
            "proposal",
            "catches",
            "cost",
        ] {
            h.insert(field.into(), v[field].clone());
        }
        rows.push(h);
    }
}

/// One row for each question to the person, with how long it waited.
fn ask_rows(rows: &mut Vec<Row>, n: &mut Totals, id: &str, asks: &[crate::state::Ask], now: i64) {
    for a in asks {
        n.asks += 1;
        if a.ended_by.is_some() {
            n.asks_answered += 1;
        }
        let wait = match (a.ts, a.ended_at) {
            (Some(x), Some(y)) => minutes(y - x),
            _ => Value::Null,
        };
        rows.push(row(vec![
            ("change", json!(id)),
            ("line", json!(a.line)),
            (
                "ts",
                a.ts.map(crate::time::format)
                    .map(Value::from)
                    .unwrap_or(Value::Null),
            ),
            ("kind", json!(a.of)),
            ("what", json!(a.what)),
            ("answered", json!(a.ended_by.is_some())),
            ("answered_by", json!(a.ended_by)),
            ("got_what_it_asked", json!(a.answered)),
            ("minutes_until_answered", wait),
            (
                "minutes_open",
                match (a.ended_by.is_none(), a.ts) {
                    (true, Some(x)) => minutes(now - x),
                    _ => Value::Null,
                },
            ),
        ]));
    }
}

/// One row for each finding of a review or a reading, with where it went, and for each `LATER:`.
fn finding_rows(rows: &mut Vec<Row>, id: &str, events: &Events) {
    let mut outcome: BTreeMap<String, String> = BTreeMap::new();
    for (_, v) in events {
        if step(v) == "review" {
            for c in v["closed"].as_array().cloned().unwrap_or_default() {
                outcome.insert(str_of(&c, "id").into(), str_of(&c, "outcome").into());
            }
        }
    }
    for (line, v) in events {
        let st = step(v);
        if st != "review" && st != "reading" {
            continue;
        }
        let round = v["report"]["round"].clone();
        for f in v["findings"].as_array().cloned().unwrap_or_default() {
            let fid = str_of(&f, "id").to_string();
            let went = if st == "reading" {
                f["went"].as_str().unwrap_or("").to_string()
            } else {
                outcome.get(&fid).cloned().unwrap_or_else(|| "open".into())
            };
            rows.push(row(vec![
                ("change", json!(id)),
                ("line", json!(line)),
                ("source", json!(st)),
                ("round", round.clone()),
                ("finding", json!(fid)),
                (
                    "kind",
                    json!(if st == "reading" {
                        "reading"
                    } else {
                        "finding"
                    }),
                ),
                ("what", f["what"].clone()),
                ("where", f.get("where").cloned().unwrap_or(Value::Null)),
                ("went", json!(went)),
            ]));
        }
        for l in v["later"].as_array().cloned().unwrap_or_default() {
            rows.push(row(vec![
                ("change", json!(id)),
                ("line", json!(line)),
                ("source", json!(st)),
                ("round", round.clone()),
                ("finding", Value::Null),
                ("kind", json!("later")),
                ("what", l.clone()),
                ("where", Value::Null),
                (
                    "went",
                    json!(if st == "reading" { "log" } else { "backlog" }),
                ),
            ]));
        }
    }
}

/// One row for each call of the layer its journal kept.
fn engine_rows(rows: &mut Vec<Row>, n: &mut Totals, journal: &[Value]) {
    for j in journal {
        n.invocations += 1;
        if !j["refused"].is_null() {
            n.refusals += 1;
        }
        let l = &j["launch"];
        if l.is_object() && l["ran"] == json!(true) {
            n.launches += 1;
            if l["started"] == json!(false) {
                n.not_started += 1;
            }
            if l["timed_out"] == json!(true) {
                n.timed_out += 1;
            }
        }
        rows.push(row(vec![
            ("change", j["brief"].clone()),
            ("ts", j["ts"].clone()),
            ("command", j["command"].clone()),
            (
                "args",
                json!(
                    j["args"]
                        .as_array()
                        .map(|a| a
                            .iter()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join(" "))
                        .unwrap_or_default()
                ),
            ),
            ("exit", j["exit"].clone()),
            ("ms", j["ms"].clone()),
            ("refused", j["refused"].clone()),
            ("wrote_line", j["wrote"]["line"].clone()),
            ("role", l["role"].clone()),
            ("model", l["model"].clone()),
            ("adapter", l["adapter"].clone()),
            ("started", l["started"].clone()),
            ("seconds_to_first_output", l["first_output"].clone()),
            ("seconds", l["seconds"].clone()),
            ("timed_out", l["timed_out"].clone()),
            ("tokens", l["tokens"].clone()),
            ("input_tokens", usage_sum(&l["usage"], "input")),
            ("output_tokens", usage_sum(&l["usage"], "output")),
            ("cache_read_tokens", usage_sum(&l["usage"], "cache_read")),
            ("cache_write_tokens", usage_sum(&l["usage"], "cache_write")),
        ]));
    }
}

/// The row of a change: its color, its size, its caps and what its other rows add up to.
fn change_row(
    bp: &Blueprint,
    input: &Input,
    events: &Events,
    layer: &BTreeSet<usize>,
    state: &crate::state::State,
    n: &Totals,
    now: i64,
) -> Row {
    let id = input.id;
    let p = &state.progress;
    let last_ts = events.iter().rev().find_map(|(_, v)| ts(v));
    let first_ts = events.iter().find_map(|(_, v)| ts(v));
    let lim = limits::count_entries(bp, id, input.entries, last_ts.unwrap_or(now));
    let after_gates = limits::minutes_after_gates_passed(events, last_ts.unwrap_or(now));
    let review_first = events
        .iter()
        .find(|(_, v)| step(v) == "review")
        .map(|(_, v)| str_of(v, "verdict") == "pass");
    let shipped = events.iter().find(|(_, v)| step(v) == "ship");
    let ended = if shipped.is_some() {
        "shipped"
    } else if state
        .next
        .blocked
        .as_deref()
        .is_some_and(|b| b != "needs-person")
    {
        "stopped"
    } else {
        "open"
    };
    let touched = p.last_review.as_ref().and_then(|r| r.color.clone());
    let rose = match (&p.planned, &touched) {
        (Some(a), Some(b)) => json!(crate::color_rank(b) > crate::color_rank(a)),
        _ => Value::Null,
    };
    let verify = crate::verify::check_log(id, Some(bp), input.entries, layer);
    let mut r = row(vec![
        ("change", json!(id)),
        (
            "kind",
            json!(match log_kind(id) {
                LogKind::Setup => "setup",
                _ => "change",
            }),
        ),
        ("color_planned", json!(p.planned)),
        ("color", json!(p.color)),
        ("color_touched", json!(touched)),
        ("rose", rose),
        ("zones", json!(p.zones)),
        ("uncovered", json!(p.uncovered)),
        ("scenarios", json!(input.scenarios)),
        ("files_changed", json!(input.git.files)),
        ("lines_added", json!(input.git.added)),
        ("lines_deleted", json!(input.git.deleted)),
        ("events", json!(events.len())),
        ("steps", json!(n.steps)),
        ("minutes_work", json!(lim.minutes)),
        ("minutes_waiting", minutes(lim.waited)),
        ("minutes_parked", Value::Null),
        (
            "minutes_to_ship",
            match (shipped.and_then(|(_, v)| ts(v)), first_ts) {
                (Some(s), Some(f)) => minutes(s - f),
                _ => Value::Null,
            },
        ),
        ("minutes_after_gates_passed", json!(after_gates)),
        ("builds", json!(lim.builds)),
        ("checks_launches", json!(lim.checks)),
    ]);
    for (col, key, name) in cap_keys() {
        let line = lim.lines.iter().find(|l| l.name == name);
        r.insert(format!("{col}_used"), json!(line.map(|l| l.used)));
        r.insert(
            format!("{col}_allowed"),
            json!(line.and_then(|l| l.cap).or(bp.cap(&lim.color, key))),
        );
        r.insert(
            format!("{col}_raised"),
            json!(line.is_some_and(|l| l.raised)),
        );
    }
    for (col, name) in [
        ("seconds_each_round", "seconds of the checks of a round"),
        (
            "seconds_before_review",
            "seconds of the checks before the review",
        ),
    ] {
        let line = lim.lines.iter().find(|l| l.name == name);
        r.insert(format!("{col}_most"), json!(line.map(|l| l.used)));
        r.insert(format!("{col}_allowed"), json!(line.and_then(|l| l.cap)));
    }
    let waivers = events.iter().filter(|(_, v)| kind(v) == "waiver").count();
    let extra = vec![
        ("waivers", json!(waivers)),
        ("reviews_to_checks", reviews_to_checks(id, events)),
        ("gates_to_checks", gates_to_checks(id, events)),
        ("fixes", json!(state.progress.fixes)),

        ("brief_corrections", brief_corrections(events)),
        ("asks", json!(n.asks)),
        ("asks_answered", json!(n.asks_answered)),
        ("gates_runs", json!(n.gates_runs)),
        ("gates_first_try", json!(n.gates_first_try)),
        ("reviews", json!(p.reviews)),
        ("review_first_try", json!(review_first)),
        ("ended", json!(ended)),
        ("close", json!(state.standing.as_ref().map(|s| s.word()))),
        (
            "undone",
            match &input.git.reverted {
                Some(Some(u)) => json!(u.by),
                Some(None) => json!(false),
                None => Value::Null,
            },
        ),
        (
            "verdict",
            json!(p.verdicts.last().map(|v| v.verdict.clone())),
        ),
        (
            "days_to_verdict",
            match (p.closed_by().and_then(|v| v.ts), p.shipped_at) {
                (Some(closed), Some(shipped)) => json!((closed - shipped) / crate::closing::DAY),
                _ => Value::Null,
            },
        ),
        (
            "stop_reason",
            json!(if ended == "stopped" {
                state.next.blocked.clone()
            } else {
                None
            }),
        ),
        ("next", json!(state.next.step)),
        ("tokens", json!(n.tokens)),
        (
            "role_seconds",
            json!((n.role_seconds * 100.0).round() / 100.0),
        ),
        ("suspect_times", json!(n.suspect)),
        (
            "verify_errors",
            json!(verify.iter().filter(|f| f.level == "error").count()),
        ),
        (
            "verify_warnings",
            json!(verify.iter().filter(|f| f.level == "warning").count()),
        ),
        ("invocations", json!(n.invocations)),
        ("refusals", json!(n.refusals)),
        ("launches", json!(n.launches)),
        ("launches_not_started", json!(n.not_started)),
        ("launches_timed_out", json!(n.timed_out)),
    ];
    for (k, v) in extra {
        r.insert(k.into(), v);
    }
    r
}

fn cell(v: &Value) -> String {
    let s = match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Array(a) => a
            .iter()
            .map(|x| match x {
                Value::String(s) => s.clone(),
                o => o.to_string(),
            })
            .collect::<Vec<_>>()
            .join(";"),
        o => o.to_string(),
    };
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s
    }
}

/// One table as CSV, with its header from the first row's columns.
pub fn csv(rows: &[Row]) -> String {
    let Some(first) = rows.first() else {
        return String::new();
    };
    let cols: Vec<&String> = first.keys().collect();
    let mut out = cols
        .iter()
        .map(|c| c.as_str())
        .collect::<Vec<_>>()
        .join(",");
    out.push('\n');
    for r in rows {
        let line: Vec<String> = cols
            .iter()
            .map(|c| cell(r.get(*c).unwrap_or(&Value::Null)))
            .collect();
        out.push_str(&line.join(","));
        out.push('\n');
    }
    out
}

pub fn json_all(t: &Tables) -> Value {
    let mut m = Map::new();
    m.insert("schema".into(), json!(SCHEMA));
    let mut tables = Map::new();
    for name in TABLES {
        tables.insert(
            name.into(),
            Value::Array(t.get(name).iter().cloned().map(Value::Object).collect()),
        );
    }
    m.insert("tables".into(), Value::Object(tables));
    Value::Object(m)
}

pub fn jsonl(t: &Tables, name: &str) -> String {
    let mut out = String::new();
    for r in t.get(name) {
        let mut m = Map::new();
        m.insert("schema".into(), json!(SCHEMA));
        m.insert("table".into(), json!(name));
        for (k, v) in r {
            m.insert(k.clone(), v.clone());
        }
        out.push_str(&Value::Object(m).to_string());
        out.push('\n');
    }
    out
}

fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

fn pct(a: usize, b: usize) -> String {
    match (a * 100 + b / 2).checked_div(b) {
        None => "none".into(),
        Some(p) => format!("{p}% ({a} of {b})"),
    }
}

/// What a person reads: where the time went, per color, at the first try, the slowest checks,
/// the questions and their waits, and what the layer refused.
pub fn summary(t: &Tables) -> Vec<String> {
    let mut out = vec![];
    let changes: Vec<&Row> = t
        .changes
        .iter()
        .filter(|r| r["kind"] == json!("change"))
        .collect();
    out.push(format!(
        "{} changes: {} shipped, {} stopped, {} open; {} setup logs",
        changes.len(),
        changes
            .iter()
            .filter(|r| r["ended"] == json!("shipped"))
            .count(),
        changes
            .iter()
            .filter(|r| r["ended"] == json!("stopped"))
            .count(),
        changes
            .iter()
            .filter(|r| r["ended"] == json!("open"))
            .count(),
        t.changes.len() - changes.len()
    ));
    work_lines(&mut out, t, &changes);
    gates_lines(&mut out, t, &changes);
    weak_checks_lines(&mut out, &changes);
    approval_lines(&mut out, &changes);
    escape_lines(&mut out, &changes);
    check_evidence(&mut out, t);
    hardening_lines(&mut out, t);
    ask_lines(&mut out, t);
    close_lines(&mut out, &changes);
    undone_lines(&mut out, &changes);
    trust_lines(&mut out, t);
    role_costs(&mut out, t);
    engine_costs(&mut out, t);
    out
}

/// Where the time went: the share of each step, and the minutes of work by color.
fn work_lines(out: &mut Vec<String>, t: &Tables, changes: &[&Row]) {
    // Share of the work by step: the time from the event before each step to the step.
    let mut by_step: BTreeMap<String, f64> = BTreeMap::new();
    for s in &t.steps {
        if let Some(x) = s["seconds_since_previous"].as_f64() {
            *by_step
                .entry(s["step"].as_str().unwrap_or("").to_string())
                .or_default() += x;
        }
    }
    let total: f64 = by_step.values().sum();
    if total > 0.0 {
        let mut v: Vec<(String, f64)> = by_step.into_iter().collect();
        v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        out.push("share of the logged time, by the step that closed it:".into());
        for (k, x) in v {
            out.push(format!(
                "  {k:<8} {:>5.1}%  {:.0} min",
                x * 100.0 / total,
                x / 60.0
            ));
        }
    }
    out.push("minutes of work per change, without waits for a person (median, most):".into());
    for color in crate::COLORS {
        let m: Vec<f64> = changes
            .iter()
            .filter(|r| r["color"] == json!(color))
            .filter_map(|r| r["minutes_work"].as_f64())
            .collect();
        if !m.is_empty() {
            let most = m.iter().cloned().fold(0.0, f64::max);
            out.push(format!(
                "  {color:<7} {} changes: {:.0}, {:.0}",
                m.len(),
                median(m),
                most
            ));
        }
    }
    let after: Vec<f64> = changes
        .iter()
        .filter_map(|r| r["minutes_after_gates_passed"].as_f64())
        .collect();
    if !after.is_empty() {
        let most = after.iter().cloned().fold(0.0, f64::max);
        out.push(format!(
            "minutes after the gates first all passed, without waits for a person (median, most): {} changes: {:.0}, {:.0}",
            after.len(),
            median(after),
            most
        ));
    }
}

/// How the gates and the reviews went at the first try, and what each check takes.
fn gates_lines(out: &mut Vec<String>, t: &Tables, changes: &[&Row]) {
    let g: Vec<&Row> = changes
        .iter()
        .copied()
        .filter(|r| !r["gates_first_try"].is_null())
        .collect();
    let r: Vec<&Row> = changes
        .iter()
        .copied()
        .filter(|r| !r["review_first_try"].is_null())
        .collect();
    out.push(format!(
        "first try: gates {}, review {}",
        pct(
            g.iter()
                .filter(|x| x["gates_first_try"] == json!(true))
                .count(),
            g.len()
        ),
        pct(
            r.iter()
                .filter(|x| x["review_first_try"] == json!(true))
                .count(),
            r.len()
        )
    ));
    let uncited: Vec<_> = t
        .steps
        .iter()
        .filter_map(|s| s["uncited"].as_u64())
        .collect();
    if !uncited.is_empty() {
        out.push(format!(
            "fix reviews missing citations: {}, uncited findings: {}",
            uncited.len(),
            uncited.iter().sum::<u64>()
        ));
    }
    let mut checks: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for c in &t.checks {
        if let Some(s) = c["seconds"].as_f64() {
            checks
                .entry(c["check"].as_str().unwrap_or("").to_string())
                .or_default()
                .push(s);
        }
    }
    if !checks.is_empty() {
        let mut v: Vec<(String, f64, f64, usize)> = checks
            .into_iter()
            .map(|(k, s)| {
                let most = s.iter().cloned().fold(0.0, f64::max);
                (k, most, median(s.clone()), s.len())
            })
            .collect();
        v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        out.push("slowest checks (most, median seconds, runs):".into());
        for (k, most, med, n) in v.into_iter().take(5) {
            out.push(format!("  {k:<14} {most:.1}, {med:.1}, {n}"));
        }
    }
    let untimed = t
        .checks
        .iter()
        .filter(|c| c["result"] != "skipped" && c["seconds"].is_null())
        .count();
    if untimed > 0 {
        out.push(format!("{untimed} check runs without seconds"));
    }
}

/// Count recorded gate runs and the builder's own observations independently. A build
/// can have several gate runs, or none, so joining them by round would multiply evidence.
fn check_evidence(out: &mut Vec<String>, t: &Tables) {
    #[derive(Default)]
    struct Evidence<'a> {
        runs: usize,
        failed: usize,
        seconds: Option<f64>,
        untimed: usize,
        skipped: BTreeMap<&'a str, usize>,
        builder_failures: Option<u128>,
        missing_claim_counts: usize,
    }
    let mut checks: BTreeMap<&str, Evidence> = BTreeMap::new();
    for c in &t.checks {
        let e = checks
            .entry(c["check"].as_str().unwrap_or("unknown"))
            .or_default();
        if c["result"] == "skipped" {
            *e.skipped
                .entry(c["why"].as_str().unwrap_or("unknown"))
                .or_default() += 1;
            continue;
        }
        e.runs += 1;
        e.failed += usize::from(c["result"] == "fail");
        if let Some(s) = c["seconds"].as_f64() {
            *e.seconds.get_or_insert(0.0) += s;
        } else {
            e.untimed += 1;
        }
    }
    for c in &t.claims {
        let e = checks
            .entry(c["check"].as_str().unwrap_or("unknown"))
            .or_default();
        if let Some(n) = c["failures"].as_u64() {
            *e.builder_failures.get_or_insert(0) += u128::from(n);
        } else {
            e.missing_claim_counts += 1;
        }
    }
    if !checks.is_empty() {
        out.push("check | gate failures/runs | builder-reported failures | claims missing counts | skipped | measured gate seconds | untimed runs".into());
    }
    for (check, e) in checks {
        let seconds = e
            .seconds
            .map_or_else(|| "unknown".into(), |s| format!("{s:.1}"));
        let failures = e
            .builder_failures
            .map_or_else(|| "unknown".into(), |n| n.to_string());
        out.push(format!(
            "  {check} | {}/{} | {failures} | {} | {} | {seconds} | {}",
            e.failed,
            e.runs,
            e.missing_claim_counts,
            e.skipped.values().sum::<usize>(),
            e.untimed
        ));
        for (reason, n) in e.skipped {
            out.push(format!("  {n} skipped: {reason}"));
        }
    }
    let builds = t.steps.iter().filter(|s| s["step"] == "build").count();
    let with_claims: BTreeSet<_> = t
        .claims
        .iter()
        .map(|c| (c["change"].as_str(), c["line"].as_u64()))
        .collect();
    if builds > with_claims.len() {
        out.push(format!(
            "build steps without claims: {}",
            builds - with_claims.len()
        ));
    }
}

/// Keep both sides of an attribution together. Proposed and approved steps can describe
/// the same crack, so the count is explicitly a count of records.
fn hardening_lines(out: &mut Vec<String>, t: &Tables) {
    let mut by_check = BTreeMap::new();
    for h in &t.hardening {
        let expected = h["expected_check"].as_str().unwrap_or("unknown");
        let found = h["caught_by"].as_str().unwrap_or("unknown");
        by_check
            .entry((expected, found))
            .or_insert_with(Vec::new)
            .push(h);
    }
    if !by_check.is_empty() {
        out.push("expected check | found by | Harden records".into());
    }
    for ((expected, found), records) in by_check {
        out.push(format!("  {expected} | {found} | {}", records.len()));
        for h in records {
            let field = |k: &str| h[k].as_str().unwrap_or("unknown");
            out.push(format!(
                "    blueprint line {} ({}): {}. {}.",
                h["line"],
                field("result"),
                field("crack"),
                field("proposal")
            ));
            if matches!(h["action"].as_str(), Some("relax" | "retire")) {
                out.push(format!(
                    "    {} | catches: {} | cost: {}",
                    field("action"),
                    field("catches"),
                    field("cost")
                ));
            }
        }
    }
}

/// What the person was asked, and how long each kind of question waited.
fn ask_lines(out: &mut Vec<String>, t: &Tables) {
    let mut asks: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for a in &t.asks {
        asks.entry(a["kind"].as_str().unwrap_or("").to_string())
            .or_default()
            .push(a["minutes_until_answered"].as_f64().unwrap_or(f64::NAN));
    }
    if asks.is_empty() {
        out.push("questions to the person: 0".into());
    } else {
        out.push(
            "questions to the person (asked, median and longest wait in minutes, still open):"
                .into(),
        );
        for (k, w) in asks {
            let open = w.iter().filter(|x| x.is_nan()).count();
            let closed: Vec<f64> = w.iter().cloned().filter(|x| !x.is_nan()).collect();
            let most = closed.iter().cloned().fold(0.0, f64::max);
            out.push(format!(
                "  {k:<9} {}, {:.0}, {:.0}, {open}",
                w.len(),
                median(closed),
                most
            ));
        }
    }
}

/// What is in use and what the person said of it: a close that never says no is worth a look.
fn close_lines(out: &mut Vec<String>, changes: &[&Row]) {
    let count =
        |column: &str, word: &str| changes.iter().filter(|r| r[column] == json!(word)).count();
    let verdicts = crate::closing::VERDICTS.map(|v| count("verdict", v));
    if count("close", "waiting") + count("close", "quiet") + verdicts.iter().sum::<usize>() == 0 {
        return;
    }
    out.push(format!(
        "in use: {} waiting for a verdict, {} closing by themselves; last verdicts: {} yes, {} not yet, {} no",
        count("close", "waiting"),
        count("close", "quiet"),
        verdicts[0],
        verdicts[1],
        verdicts[2]
    ));
}

/// The reviews that sent a change back to whoever wrote its checks, because a finding named a
/// sealed check, leaving out those whose color rose, which go back to Brief (decision 134). A setup
/// log has no brief to compare with: null.
fn reviews_to_checks(id: &str, events: &[(usize, &Value)]) -> Value {
    if log_kind(id) != LogKind::Change {
        return Value::Null;
    }
    let mut briefed: Option<&str> = None;
    let mut n = 0;
    for (_, v) in events {
        match step(v) {
            "brief" => briefed = v["color"]["color"].as_str().or(briefed),
            "review" if matches!(v["fix_role"].as_str(), Some("checks" | "checks-then-build")) => {
                let found = v["color"]["color"].as_str();
                let rose = matches!((briefed, found), (Some(b), Some(c)) if crate::color_rank(c) > crate::color_rank(b));
                n += usize::from(!rose);
            }
            _ => {}
        }
    }
    json!(n)
}

/// The gates runs that sent the change back to whoever wrote its checks, because a check that
/// judges its tests failed, such as one of planted faults (decision 142). Null for a setup log.
fn gates_to_checks(id: &str, events: &[(usize, &Value)]) -> Value {
    if log_kind(id) != LogKind::Change {
        return Value::Null;
    }
    let n = events
        .iter()
        .filter(|(_, v)| step(v) == "gates")
        .filter(|(_, v)| matches!(v["fix_role"].as_str(), Some("checks" | "checks-then-build")))
        .count();
    json!(n)
}

/// The questions asking the person to approve the brief that came back with a correction: they
/// answered something other than the yes, or a new version of the brief came before it, as the
/// last question asked (decision 138). Null when nobody was asked to approve a brief.
pub(crate) fn brief_corrections(events: &[(usize, &Value)]) -> Value {
    let (mut asked, mut corrected, mut open) = (0, 0, false);
    for (_, v) in events {
        let new_version = step(v) == "brief" && v["report"]["result"] != json!("stopped");
        match kind(v) {
            "ask" => {
                open = str_of(v, "of") == "brief";
                asked += usize::from(open);
            }
            "answer" if open => (corrected, open) = (corrected + 1, false),
            // A new version corrected it; a stop only ends the question.
            "step" if open && step(v) == "brief" => {
                (corrected, open) = (corrected + usize::from(new_version), false)
            }
            "approval" if str_of(v, "of") == "brief" => open = false,
            _ => {}
        }
    }
    if asked == 0 {
        Value::Null
    } else {
        json!(corrected)
    }
}

/// What got through: the fixes of what a change shipped, by the color of the change they fixed, and
/// how many still wait for a hardening record that names it (decision 143).
fn escape_lines(out: &mut Vec<String>, changes: &[&Row]) {
    let fixed: Vec<&str> = changes.iter().filter_map(|r| r["fixes"].as_str()).collect();
    if fixed.is_empty() {
        return;
    }
    let color_of = |id: &str| {
        changes
            .iter()
            .find(|r| r["change"] == json!(id))
            .and_then(|r| r["color"].as_str())
            .unwrap_or("none")
    };
    let mut by_color: BTreeMap<&str, usize> = BTreeMap::new();
    for f in &fixed {
        *by_color.entry(color_of(f)).or_default() += 1;
    }
    // A record counts once the fix was briefed: an older one looked at something else.
    let open = changes
        .iter()
        .filter(|r| r["fix_hardened"] == json!(false))
        .count();
    let each: Vec<String> = by_color.iter().map(|(c, n)| format!("{c} {n}")).collect();
    out.push(format!(
        "fixes of what shipped, by the color of what they fixed: {}; {open} still without a hardening record",
        each.join(", ")
    ));
}

/// The changes of each color asked to approve their brief, and how many came back corrected: where
/// a color's approvals keep correcting nothing, hardening may propose taking them out.
fn approval_lines(out: &mut Vec<String>, changes: &[&Row]) {
    let mut by_color: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for r in changes.iter().filter(|r| r["brief_corrections"].is_u64()) {
        let n = by_color
            .entry(r["color"].as_str().unwrap_or("none"))
            .or_default();
        n.0 += 1;
        n.1 += usize::from(r["brief_corrections"].as_u64().unwrap_or(0) > 0);
    }
    if by_color.is_empty() {
        return;
    }
    let each: Vec<String> = by_color
        .iter()
        .map(|(color, (asked, corrected))| format!("{color} {corrected} of {asked}"))
        .collect();
    out.push(format!(
        "briefs whose approval brought a correction: {}",
        each.join(", ")
    ));
}

/// The changes of each color whose review sent them back to whoever wrote their checks: where
/// that keeps happening, hardening proposes a check of planted faults for the color.
fn weak_checks_lines(out: &mut Vec<String>, changes: &[&Row]) {
    let mut by_color: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for r in changes.iter().filter(|r| r["reviews"].as_u64().unwrap_or(0) > 0) {
        let n = by_color
            .entry(r["color"].as_str().unwrap_or("none"))
            .or_default();
        n.0 += 1;
        n.1 += usize::from(r["reviews_to_checks"].as_u64().unwrap_or(0) > 0);
    }
    if by_color.values().any(|(_, back)| *back > 0) {
        let each: Vec<String> = by_color
            .iter()
            .map(|(color, (reviewed, back))| format!("{color} {back} of {reviewed}"))
            .collect();
        out.push(format!(
            "reviewed changes sent back to whoever wrote their checks: {}",
            each.join(", ")
        ));
    }
    // What a check of planted faults sent back at the gates, before any reviewer.
    let mut gated: BTreeMap<&str, usize> = BTreeMap::new();
    for r in changes.iter().filter(|r| r["gates_to_checks"].as_u64().unwrap_or(0) > 0) {
        *gated.entry(r["color"].as_str().unwrap_or("none")).or_default() += 1;
    }
    if !gated.is_empty() {
        let each: Vec<String> = gated.iter().map(|(c, n)| format!("{c} {n}")).collect();
        out.push(format!(
            "changes the gates sent back to whoever wrote their checks: {}",
            each.join(", ")
        ));
    }
}

/// What shipped and was reverted later in the main branch, by color: where it happens, a gate let
/// something through. A change git wasn't read for is in neither number.
fn undone_lines(out: &mut Vec<String>, changes: &[&Row]) {
    let shipped = |r: &&&Row| r["ended"] == json!("shipped");
    if !changes.iter().any(|r| shipped(&r)) {
        return;
    }
    let mut by_color: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for r in changes
        .iter()
        .filter(shipped)
        .filter(|r| !r["undone"].is_null())
    {
        let n = by_color
            .entry(r["color"].as_str().unwrap_or("none"))
            .or_default();
        n.0 += 1;
        n.1 += usize::from(r["undone"].is_string());
    }
    let each: Vec<String> = by_color
        .iter()
        .map(|(color, (shipped, undone))| format!("{color} {undone} of {shipped}"))
        .collect();
    out.push(format!(
        "shipped and reverted later in the main branch: {}",
        if each.is_empty() {
            "not read from git".into()
        } else {
            each.join(", ")
        }
    ));
}

/// How far the logs can be trusted: times that look written afterwards, and what verify finds.
fn trust_lines(out: &mut Vec<String>, t: &Tables) {
    let suspect = t
        .steps
        .iter()
        .filter(|s| s["suspect_time"] == json!(true))
        .count();
    out.push(format!(
        "steps written in the same second as the step before: {suspect} of {}",
        t.steps.len()
    ));
    let errors: i64 = t
        .changes
        .iter()
        .filter_map(|r| r["verify_errors"].as_i64())
        .sum();
    let warnings: i64 = t
        .changes
        .iter()
        .filter_map(|r| r["verify_warnings"].as_i64())
        .sum();
    out.push(format!(
        "logs passing verify: {} of {} ({errors} errors, {warnings} warnings in all)",
        t.changes
            .iter()
            .filter(|r| r["verify_errors"] == json!(0))
            .count(),
        t.changes.len()
    ));
}

/// Measured role costs and the steps with missing measurements.
fn role_costs(out: &mut Vec<String>, t: &Tables) {
    #[derive(Default)]
    struct RoleCost {
        seconds: Option<f64>,
        tokens: Option<u128>,
        missing_seconds: usize,
        missing_tokens: usize,
    }
    let mut roles: BTreeMap<&str, RoleCost> = BTreeMap::new();
    for s in &t.steps {
        let Some(role) = s["role"].as_str() else {
            continue;
        };
        let cost = roles.entry(role).or_default();
        if let Some(seconds) = s["seconds"].as_f64() {
            *cost.seconds.get_or_insert(0.0) += seconds;
        } else {
            cost.missing_seconds += 1;
        }
        if let Some(tokens) = s["tokens"].as_u64() {
            *cost.tokens.get_or_insert(0) += u128::from(tokens);
        } else {
            cost.missing_tokens += 1;
        }
    }
    if !roles.is_empty() {
        out.push("role | measured seconds | steps missing seconds | measured tokens | steps missing tokens".into());
    }
    for (role, cost) in roles {
        let seconds = cost
            .seconds
            .map_or_else(|| "unknown".into(), |s| format!("{s:.1}"));
        let tokens = cost
            .tokens
            .map_or_else(|| "unknown".into(), |n| n.to_string());
        out.push(format!(
            "  {role} | {seconds} | {} | {tokens} | {}",
            cost.missing_seconds, cost.missing_tokens
        ));
    }
}

/// Calls per change, refusals and launch outcomes.
fn engine_costs(out: &mut Vec<String>, t: &Tables) {
    let logged_calls: BTreeMap<_, _> = t
        .changes
        .iter()
        .filter(|c| c["kind"] == "change")
        .filter_map(|c| Some((c["change"].as_str()?, c["invocations"].as_u64()?)))
        .collect();
    let mut journal_only_calls = BTreeMap::new();
    let mut refused: BTreeMap<String, usize> = BTreeMap::new();
    for e in &t.engine {
        if let Some(id) = e["change"].as_str()
            && !logged_calls.contains_key(id)
            && log_kind(id) == LogKind::Change
        {
            *journal_only_calls.entry(id).or_default() += 1;
        }
        if let Some(r) = e["refused"].as_str() {
            *refused.entry(r.to_string()).or_default() += 1;
        }
    }
    if !logged_calls.is_empty() || !journal_only_calls.is_empty() {
        out.push("change | recorded layer calls".into());
        for (id, count) in logged_calls.into_iter().chain(journal_only_calls) {
            out.push(format!("  {id} | {count}"));
        }
    }
    let total = refused.values().sum::<usize>();
    let mut line = format!("the layer: {} invocations, {total} refused", t.engine.len());
    if !refused.is_empty() {
        let reasons: Vec<_> = refused.iter().map(|(k, n)| format!("{k} {n}")).collect();
        line.push_str(&format!(" ({})", reasons.join(", ")));
    }
    out.push(line);
    let launched: Vec<&Row> = t
        .engine
        .iter()
        .filter(|e| e["started"].is_boolean())
        .collect();
    if !launched.is_empty() {
        out.push(format!(
            "launches: {}, not started {}, timed out {}",
            launched.len(),
            launched
                .iter()
                .filter(|e| e["started"] == json!(false))
                .count(),
            launched
                .iter()
                .filter(|e| e["timed_out"] == json!(true))
                .count()
        ));
    }
}
