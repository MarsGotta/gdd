//! Metrics, derived only from the logs, the journals, the blueprint and git, so they can always be
//! recomputed. Six tidy tables, each a list of rows with the same columns, under one `schema`.

use crate::blueprint::Blueprint;
use crate::limits;
use crate::log::{self, Entry, kind, step, str_of, ts};
use crate::state::{GitView, LogKind, log_kind};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

pub const SCHEMA: &str = "gdd-metrics/1";
pub const TABLES: [&str; 6] = ["changes", "steps", "checks", "asks", "findings", "engine"];

pub type Row = Map<String, Value>;

/// What git adds to a change's row, when there is git.
#[derive(Debug, Clone, Default)]
pub struct ChangeGit {
    pub files: Option<i64>,
    pub added: Option<i64>,
    pub deleted: Option<i64>,
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

/// Build every table from the changes given.
pub fn tables(bp: &Blueprint, inputs: &[Input], now: i64) -> Tables {
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
        let values: Vec<&Value> = events.iter().map(|(_, v)| *v).collect();
        let last_ts = values.iter().rev().find_map(|v| ts(v));
        let first_ts = values.iter().find_map(|v| ts(v));
        let lim = limits::count(bp, &values, last_ts.unwrap_or(now));
        let state = crate::state::derive(id, bp, input.entries, GitView::default(), now);
        let p = &state.progress;

        // steps
        let mut prev_t: Option<i64> = None;
        let mut prev_step_t: Option<i64> = None;
        let mut prev_step_line: Option<usize> = None;
        let mut suspect = 0;
        let mut role_seconds = 0f64;
        let mut tokens_total = 0i64;
        let mut steps = 0;
        for (line, v) in &events {
            let t_ = ts(v);
            let st = step(v);
            if !st.is_empty() {
                steps += 1;
                let r = &v["report"];
                let since = match (t_, prev_t) {
                    (Some(a), Some(b)) => json!(a - b),
                    _ => Value::Null,
                };
                let sus = TIMED.contains(&st)
                    && t_.is_some()
                    && t_ == prev_step_t
                    && !(layer.contains(line)
                        && prev_step_line.is_some_and(|l| layer.contains(&l)));
                // Written by the layer, by its journal.
                let by_layer = layer.contains(line);
                if sus {
                    suspect += 1;
                }
                if let Some(s) = r["seconds"].as_f64() {
                    role_seconds += s;
                }
                if let Some(k) = r["tokens"].as_i64() {
                    tokens_total += k;
                }
                t.steps.push(row(vec![
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
                    ("seconds", r["seconds"].clone()),
                    ("tokens", r["tokens"].clone()),
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

        // checks
        let mut run = 0;
        let mut first_gates: Option<bool> = None;
        for (line, v) in &events {
            if step(v) != "gates" {
                continue;
            }
            run += 1;
            let failed = log::any_kv(v, "result", "fail");
            if first_gates.is_none() {
                first_gates = Some(!failed);
            }
            for g in v["gates"].as_array().cloned().unwrap_or_default() {
                let c = str_of(&g, "check").to_string();
                t.checks.push(row(vec![
                    ("change", json!(id)),
                    ("run", json!(run)),
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
                    ("result", g["result"].clone()),
                ]));
            }
        }

        // asks
        let mut asked = 0;
        let mut answered = 0;
        for a in &p.asks {
            asked += 1;
            if a.ended_by.is_some() {
                answered += 1;
            }
            let wait = match (a.ts, a.ended_at) {
                (Some(x), Some(y)) => minutes(y - x),
                _ => Value::Null,
            };
            t.asks.push(row(vec![
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

        // findings
        let mut outcome: BTreeMap<String, String> = BTreeMap::new();
        for (_, v) in &events {
            if step(v) == "review" {
                for c in v["closed"].as_array().cloned().unwrap_or_default() {
                    outcome.insert(str_of(&c, "id").into(), str_of(&c, "outcome").into());
                }
            }
        }
        for (line, v) in &events {
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
                t.findings.push(row(vec![
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
                t.findings.push(row(vec![
                    ("change", json!(id)),
                    ("line", json!(line)),
                    ("source", json!(st)),
                    ("round", round.clone()),
                    ("finding", Value::Null),
                    ("kind", json!("later")),
                    ("what", l.clone()),
                    ("where", Value::Null),
                    ("went", json!("backlog")),
                ]));
            }
        }

        // engine
        let (mut inv, mut refusals, mut launches, mut not_started, mut timed_out) = (0, 0, 0, 0, 0);
        for j in input.journal {
            inv += 1;
            if !j["refused"].is_null() {
                refusals += 1;
            }
            let l = &j["launch"];
            if l.is_object() && l["ran"] == json!(true) {
                launches += 1;
                if l["started"] == json!(false) {
                    not_started += 1;
                }
                if l["timed_out"] == json!(true) {
                    timed_out += 1;
                }
            }
            t.engine.push(row(vec![
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

        // the change
        if log_kind(id) == LogKind::Blueprint {
            continue;
        }
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
        let verify = crate::verify::check_log(id, Some(bp), input.entries, &layer);
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
            ("steps", json!(steps)),
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
        let waivers = values.iter().filter(|v| kind(v) == "waiver").count();
        let extra = vec![
            ("waivers", json!(waivers)),
            ("asks", json!(asked)),
            ("asks_answered", json!(answered)),
            ("gates_runs", json!(run)),
            ("gates_first_try", json!(first_gates)),
            ("reviews", json!(p.reviews)),
            ("review_first_try", json!(review_first)),
            ("ended", json!(ended)),
            (
                "stop_reason",
                json!(if ended == "stopped" {
                    state.next.blocked.clone()
                } else {
                    None
                }),
            ),
            ("next", json!(state.next.step)),
            ("tokens", json!(tokens_total)),
            (
                "role_seconds",
                json!((role_seconds * 100.0).round() / 100.0),
            ),
            ("suspect_times", json!(suspect)),
            (
                "verify_errors",
                json!(verify.iter().filter(|f| f.level == "error").count()),
            ),
            (
                "verify_warnings",
                json!(verify.iter().filter(|f| f.level == "warning").count()),
            ),
            ("invocations", json!(inv)),
            ("refusals", json!(refusals)),
            ("launches", json!(launches)),
            ("launches_not_started", json!(not_started)),
            ("launches_timed_out", json!(timed_out)),
        ];
        for (k, v) in extra {
            r.insert(k.into(), v);
        }
        t.changes.push(r);
    }
    t
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
    let untimed = t.checks.iter().filter(|c| c["seconds"].is_null()).count();
    if untimed > 0 {
        out.push(format!("{untimed} check runs without seconds"));
    }
    let mut asks: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for a in &t.asks {
        asks.entry(a["kind"].as_str().unwrap_or("").to_string())
            .or_default()
            .push(a["minutes_until_answered"].as_f64().unwrap_or(f64::NAN));
    }
    if !asks.is_empty() {
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
    let mut tokens: BTreeMap<String, i64> = BTreeMap::new();
    for s in &t.steps {
        if let Some(k) = s["tokens"].as_i64() {
            *tokens
                .entry(s["role"].as_str().unwrap_or("").to_string())
                .or_default() += k;
        }
    }
    if !tokens.is_empty() {
        out.push(format!(
            "tokens by role: {}",
            tokens
                .iter()
                .map(|(k, v)| format!("{k} {v}"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let mut refused: BTreeMap<String, usize> = BTreeMap::new();
    for e in &t.engine {
        if let Some(r) = e["refused"].as_str() {
            *refused.entry(r.to_string()).or_default() += 1;
        }
    }
    out.push(format!(
        "the layer: {} invocations, {} refused{}",
        t.engine.len(),
        refused.values().sum::<usize>(),
        if refused.is_empty() {
            String::new()
        } else {
            format!(
                " ({})",
                refused
                    .iter()
                    .map(|(k, v)| format!("{k} {v}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    ));
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
    out
}
