//! The log of a change, `gdd/log/<id>.jsonl`: one event per line, only appended.
//!
//! Reading is pure: [`parse`] turns text into entries. [`shape`] checks an event against the
//! contract's shape of its kind. [`append`] is the only writer, and it writes one line at a time.

use serde_json::{Map, Value, json};
use std::io::Write;
use std::path::Path;

/// One line of a log.
#[derive(Debug, Clone)]
pub struct Entry {
    /// The line number, from 1.
    pub line: usize,
    pub raw: String,
    pub value: Option<Value>,
    /// Why the line can't be read as an event: not JSON, or not an object.
    pub error: Option<String>,
    pub blank: bool,
}

impl Entry {
    pub fn event(&self) -> Option<&Value> {
        self.value.as_ref().filter(|v| v.is_object())
    }
}

pub fn parse(text: &str) -> Vec<Entry> {
    text.lines()
        .enumerate()
        .map(|(i, raw)| {
            let line = i + 1;
            if raw.trim().is_empty() {
                return Entry {
                    line,
                    raw: raw.into(),
                    value: None,
                    error: Some("an empty line".into()),
                    blank: true,
                };
            }
            match serde_json::from_str::<Value>(raw) {
                Ok(v) if v.is_object() => Entry {
                    line,
                    raw: raw.into(),
                    value: Some(v),
                    error: None,
                    blank: false,
                },
                Ok(_) => Entry {
                    line,
                    raw: raw.into(),
                    value: None,
                    error: Some("not a JSON object".into()),
                    blank: false,
                },
                Err(e) => Entry {
                    line,
                    raw: raw.into(),
                    value: None,
                    error: Some(format!("not valid JSON: {e}")),
                    blank: false,
                },
            }
        })
        .collect()
}

/// The lines a `correction` with `"void": true` takes out of the change's state.
pub fn voided(entries: &[Entry]) -> Vec<usize> {
    entries
        .iter()
        .filter_map(|e| e.event())
        .filter(|v| kind(v) == "correction" && v["void"] == json!(true))
        .filter_map(|v| v["line"].as_u64().map(|n| n as usize))
        .collect()
}

/// The events of a log that count: readable, and not voided by a correction.
pub fn events(entries: &[Entry]) -> Vec<(usize, &Value)> {
    let void = voided(entries);
    entries
        .iter()
        .filter(|e| !void.contains(&e.line))
        .filter_map(|e| e.event().map(|v| (e.line, v)))
        .collect()
}

pub fn str_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

/// `approval`, `waiver`, `ask`, `step`, `answer` or `correction`.
pub fn kind(v: &Value) -> &str {
    str_of(v, "event")
}

/// The step of a `step` event, or empty.
pub fn step(v: &Value) -> &str {
    if kind(v) == "step" {
        str_of(v, "step")
    } else {
        ""
    }
}

pub fn is_step(v: &Value, name: &str) -> bool {
    step(v) == name
}

pub fn ts(v: &Value) -> Option<i64> {
    v.get("ts")
        .and_then(Value::as_str)
        .and_then(crate::time::parse)
}

pub fn result(v: &Value) -> &str {
    v.get("report")
        .and_then(|r| r.get("result"))
        .and_then(Value::as_str)
        .unwrap_or("")
}

/// Whether some object inside the event has this key with this string value.
pub fn any_kv(v: &Value, key: &str, val: &str) -> bool {
    match v {
        Value::Object(m) => m
            .iter()
            .any(|(k, x)| (k == key && x.as_str() == Some(val)) || any_kv(x, key, val)),
        Value::Array(a) => a.iter().any(|x| any_kv(x, key, val)),
        _ => false,
    }
}

/// The sum of every `tokens` number inside the event, their whole part, as the counter adds them.
pub fn tokens_in(v: &Value) -> i64 {
    match v {
        Value::Object(m) => m
            .iter()
            .map(|(k, x)| {
                if k == "tokens" {
                    x.as_f64()
                        .filter(|f| *f >= 0.0)
                        .map(|f| f as i64)
                        .unwrap_or(0)
                } else {
                    tokens_in(x)
                }
            })
            .sum(),
        Value::Array(a) => a.iter().map(tokens_in).sum(),
        _ => 0,
    }
}

/// A number as the counter reads it: a JSON number, or the digits a string starts with.
pub fn number_like(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_f64().filter(|f| *f >= 0.0).map(|f| f as i64),
        Value::String(s) => {
            let d: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
            d.parse().ok()
        }
        _ => None,
    }
}

// ----- shapes -----

#[derive(Clone, Copy, PartialEq, Debug)]
enum T {
    Str,
    StrOrNull,
    Int,
    IntOrNull,
    NumOrNull,
    NumberLike,
    Bool,
    Obj,
    Arr,
    Hashes,
    OneOf(&'static [&'static str]),
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Need {
    /// Missing or of the wrong type, the event can't be read: an error.
    Must,
    /// Written by the layer every time; on reading, a wrong type is a warning and a missing one is fine.
    Writes,
    /// Optional everywhere.
    May,
}

type Field = (&'static str, T, Need);

pub const EVENTS: [&str; 7] = [
    "approval",
    "waiver",
    "ask",
    "step",
    "answer",
    "correction",
    "seal",
];
pub const STEPS: [&str; 10] = [
    "prepare", "reading", "brief", "checks", "build", "gates", "review", "ship", "harden", "exam",
];
pub const APPROVAL_OF: [&str; 5] = ["brief", "change", "blueprint", "gates", "debt"];
pub const ASK_OF: [&str; 8] = [
    "brief",
    "change",
    "blueprint",
    "gates",
    "debt",
    "waiver",
    "decision",
    "stop",
];

const REPORT: [Field; 9] = [
    ("skill", T::Str, Need::Must),
    ("result", T::Str, Need::Must),
    ("version", T::StrOrNull, Need::Writes),
    ("level", T::StrOrNull, Need::Writes),
    ("model", T::StrOrNull, Need::Writes),
    ("round", T::Int, Need::Writes),
    ("caught", T::Str, Need::Writes),
    ("seconds", T::NumOrNull, Need::Writes),
    ("tokens", T::NumOrNull, Need::Writes),
];

fn fields_of(event: &str, step: &str) -> Vec<Field> {
    use Need::*;
    use T::*;
    match (event, step) {
        ("approval", _) => vec![
            ("who", Str, Must),
            ("of", OneOf(&APPROVAL_OF), Must),
            ("version", Int, Writes),
            ("files", Hashes, Must),
            ("where", Str, Must),
        ],
        ("waiver", _) => vec![
            ("who", Str, Must),
            ("check", Str, Must),
            ("why", Str, Must),
            ("backlog", Str, Writes),
            ("limit", Str, May),
            ("to", NumberLike, May),
        ],
        ("ask", _) => vec![("of", OneOf(&ASK_OF), Must), ("what", Str, Must)],
        ("answer", _) => vec![
            ("what", Str, Must),
            ("of", Str, Writes),
            ("ask", Int, Writes),
        ],
        ("seal", _) => vec![
            ("ref", Str, Must),
            ("from", StrOrNull, Writes),
            ("to", Str, Must),
            ("why", Str, Writes),
            ("files", Hashes, May),
        ],
        ("correction", _) => vec![
            ("line", Int, Must),
            ("what", Str, Must),
            ("void", Bool, May),
        ],
        ("step", "brief") => vec![
            ("version", Int, Writes),
            ("color", Obj, Must),
            ("files", Hashes, Must),
            ("seal", StrOrNull, Writes),
            ("request", Str, May),
        ],
        ("step", "checks") => vec![("seal", Obj, Must), ("color", Obj, May)],
        ("step", "build") => vec![("claims", Arr, Writes)],
        ("step", "gates") => vec![
            ("start", Str, Must),
            ("where", Str, Writes),
            ("gates", Arr, Must),
            ("head", Str, May),
        ],
        ("step", "review") => vec![
            ("verdict", OneOf(&["pass", "fix", "stop"]), Must),
            ("color", Obj, Writes),
            ("ran", Arr, Writes),
            ("findings", Arr, Writes),
            ("closed", Arr, Writes),
        ],
        ("step", "reading") => vec![("findings", Arr, Writes)],
        ("step", "ship") => vec![
            ("where", Str, Must),
            ("color", Str, Writes),
            ("head", Str, May),
        ],
        ("step", "prepare") => vec![
            ("files", Arr, May),
            ("probe", Arr, May),
            ("levels", Arr, May),
        ],
        ("step", "harden") => vec![
            ("change", StrOrNull, Writes),
            ("crack", Str, Writes),
            ("scenario", StrOrNull, Writes),
            ("proposal", Str, Writes),
        ],
        ("step", "exam") => vec![
            ("reviewer", Obj, Writes),
            ("asked", Int, Writes),
            ("caught", Int, Writes),
            ("missed", Arr, Writes),
        ],
        _ => vec![],
    }
}

fn fits(v: &Value, t: T) -> bool {
    match t {
        T::Str => v.is_string(),
        T::StrOrNull => v.is_string() || v.is_null(),
        T::Int => v.is_i64() || v.is_u64(),
        T::IntOrNull => v.is_i64() || v.is_u64() || v.is_null(),
        T::NumOrNull => v.is_number() || v.is_null(),
        T::NumberLike => number_like(v).is_some(),
        T::Bool => v.is_boolean(),
        T::Obj => v.is_object(),
        T::Arr => v.is_array(),
        T::Hashes => v
            .as_object()
            .is_some_and(|m| m.values().all(Value::is_string)),
        T::OneOf(words) => v.as_str().is_some_and(|s| words.contains(&s)),
    }
}

fn describe(t: T) -> String {
    match t {
        T::Str => "a string".into(),
        T::StrOrNull => "a string or null".into(),
        T::Int | T::IntOrNull => "a whole number".into(),
        T::NumOrNull => "a number or null".into(),
        T::NumberLike => "a number".into(),
        T::Bool => "true or false".into(),
        T::Obj => "an object".into(),
        T::Arr => "a list".into(),
        T::Hashes => "an object of paths and blob hashes".into(),
        T::OneOf(w) => format!("one of {}", w.join(", ")),
    }
}

struct Acc {
    errors: Vec<String>,
    warnings: Vec<String>,
    writing: bool,
}

impl Acc {
    fn check(&mut self, key: &str, t: T, need: Need, x: Option<&Value>, at: &str) {
        match x {
            None if need == Need::Must || (self.writing && need == Need::Writes) => {
                self.errors.push(format!("{at}{key} is missing"))
            }
            Some(x) if !fits(x, t) => {
                let msg = format!("{at}{key} is not {}", describe(t));
                if need == Need::Must || self.writing {
                    self.errors.push(msg)
                } else {
                    self.warnings.push(msg)
                }
            }
            _ => {}
        }
    }
}

/// What is wrong with an event's shape. `writing` holds it to everything the layer writes.
/// Returns the errors, which make it unreadable as that event, and the warnings.
pub fn shape(v: &Value, writing: bool) -> (Vec<String>, Vec<String>) {
    let mut a = Acc {
        errors: vec![],
        warnings: vec![],
        writing,
    };
    let Some(obj) = v.as_object() else {
        return (vec!["not a JSON object".into()], vec![]);
    };
    for key in ["contract", "ts", "brief", "event"] {
        if !obj.get(key).is_some_and(Value::is_string) {
            a.errors.push(format!("it has no {key}"));
        }
    }
    if let Some(c) = obj.get("contract").and_then(Value::as_str)
        && c != crate::CONTRACT
    {
        a.errors
            .push(format!("its contract is {c}, not {}", crate::CONTRACT));
    }
    if let Some(t) = obj.get("ts").and_then(Value::as_str) {
        if crate::time::parse(t).is_none() {
            a.errors.push(format!("its time {t:?} is not a UTC time"));
        } else if !crate::time::is_contract_form(t) {
            a.warnings.push(format!(
                "its time {t:?} is not written as date -u +%FT%TZ writes it"
            ));
        }
    }
    let event = str_of(v, "event");
    if !event.is_empty() && !EVENTS.contains(&event) {
        a.errors
            .push(format!("{event:?} is not an event of the contract"));
        return (a.errors, a.warnings);
    }
    let step = str_of(v, "step");
    if event == "step" && !STEPS.contains(&step) {
        a.errors.push(if step.is_empty() {
            "a step event without its step".into()
        } else {
            format!("{step:?} is not a step of the contract")
        });
        return (a.errors, a.warnings);
    }
    let stopped = obj.get("stop").is_some();
    for (key, t, need) in fields_of(event, step) {
        // A role that stopped with CANNOT: leaves no seal and no verdict.
        let need = if stopped && matches!(key, "seal" | "verdict" | "claims" | "findings" | "color")
        {
            Need::May
        } else {
            need
        };
        a.check(key, t, need, obj.get(key), "");
    }
    if event == "step" && !matches!(step, "gates" | "ship" | "exam") {
        match obj.get("report") {
            Some(r) if r.is_object() => {
                for (key, t, need) in REPORT {
                    a.check(key, t, need, r.get(key), "report.");
                }
            }
            Some(_) => a.errors.push("report is not an object".into()),
            None => a.errors.push("report is missing".into()),
        }
    }
    if stopped {
        let st = &obj["stop"];
        if !st.is_object() {
            a.errors.push("stop is not an object".into());
        } else {
            a.check("reason", T::Str, Need::Must, st.get("reason"), "stop.");
            a.check("needs", T::Str, Need::Writes, st.get("needs"), "stop.");
        }
    }
    if event == "step"
        && step == "gates"
        && let Some(Value::Array(items)) = obj.get("gates")
    {
        for (i, g) in items.iter().enumerate() {
            let at = format!("gates[{i}].");
            if !g.is_object() {
                a.errors.push(format!("gates[{i}] is not an object"));
                continue;
            }
            a.check("check", T::Str, Need::Must, g.get("check"), &at);
            a.check(
                "result",
                T::OneOf(&["pass", "fail"]),
                Need::Must,
                g.get("result"),
                &at,
            );
            a.check("seconds", T::NumOrNull, Need::Writes, g.get("seconds"), &at);
            a.check("command", T::Str, Need::Writes, g.get("command"), &at);
            a.check("exit", T::IntOrNull, Need::Writes, g.get("exit"), &at);
            a.check("tail", T::Str, Need::Writes, g.get("tail"), &at);
            if g.get("seconds").is_none() && !writing {
                a.warnings.push(format!(
                    "{at}seconds is missing: write null when nothing timed it"
                ));
            }
        }
    }
    if event == "step"
        && step == "brief"
        && let Some(c) = obj.get("color").filter(|c| c.is_object())
    {
        a.check("base", T::Str, Need::Must, c.get("base"), "color.");
        a.check(
            "color",
            T::OneOf(&crate::COLORS),
            Need::Must,
            c.get("color"),
            "color.",
        );
        a.check("paths", T::Arr, Need::Writes, c.get("paths"), "color.");
        a.check("zones", T::Arr, Need::Writes, c.get("zones"), "color.");
        a.check(
            "uncovered",
            T::Arr,
            Need::Writes,
            c.get("uncovered"),
            "color.",
        );
    }
    if event == "step"
        && step == "checks"
        && !stopped
        && let Some(sl) = obj.get("seal").filter(|s| s.is_object())
    {
        a.check("ref", T::Str, Need::Writes, sl.get("ref"), "seal.");
        a.check(
            "commit",
            T::StrOrNull,
            Need::Must,
            sl.get("commit"),
            "seal.",
        );
        a.check("checks", T::Hashes, Need::Must, sl.get("checks"), "seal.");
        a.check("proofs", T::Arr, Need::Writes, sl.get("proofs"), "seal.");
    }
    if let Some(Value::Object(m)) = obj.get("files") {
        for (p, h) in m {
            if let Some(h) = h.as_str()
                && !is_hash(h)
            {
                a.warnings
                    .push(format!("files.{p} is {h:?}, not a git blob hash"));
            }
        }
    }
    (a.errors, a.warnings)
}

pub fn is_hash(h: &str) -> bool {
    (h.len() == 40 || h.len() == 64) && h.chars().all(|c| c.is_ascii_hexdigit())
}

/// A new event with the fields every event has, in the contract's order, then the rest.
pub fn new_event(brief: &str, event: &str, ts: &str, rest: Map<String, Value>) -> Value {
    let mut m = Map::new();
    m.insert("contract".into(), json!(crate::CONTRACT));
    m.insert("ts".into(), json!(ts));
    m.insert("brief".into(), json!(brief));
    m.insert("event".into(), json!(event));
    for (k, v) in rest {
        m.insert(k, v);
    }
    Value::Object(m)
}

/// Append one event to a log, as one line. Refuses an event whose shape isn't the contract's.
pub fn append(path: &Path, event: &Value) -> Result<usize, String> {
    let (errors, _) = shape(event, true);
    if !errors.is_empty() {
        return Err(format!(
            "the event isn't the contract's shape: {}",
            errors.join("; ")
        ));
    }
    let line = serde_json::to_string(event).map_err(|e| e.to_string())?;
    if line.contains('\n') {
        return Err("an event must fit on one line".into());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    // A log that doesn't end with a newline gets one first, so the new event is a line of its own.
    let existing = std::fs::read(path).unwrap_or_default();
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    if !existing.is_empty() && !existing.ends_with(b"\n") {
        f.write_all(b"\n").map_err(|e| e.to_string())?;
    }
    f.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
    f.write_all(b"\n").map_err(|e| e.to_string())?;
    let n = existing.iter().filter(|b| **b == b'\n').count()
        + usize::from(!existing.is_empty() && !existing.ends_with(b"\n"))
        + 1;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template_events() -> Vec<Value> {
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../skills/gdd/assets/log-events.jsonl"),
        )
        .unwrap();
        parse(&text).into_iter().filter_map(|e| e.value).collect()
    }

    #[test]
    fn every_template_event_is_known() {
        for v in template_events() {
            let (errors, _) = shape(&v, false);
            // The template holds placeholders, such as "<UTC time>": only those may fail.
            for e in &errors {
                assert!(
                    e.contains("time")
                        || e.contains("one of")
                        || e.contains("not an event")
                        || e.contains("not a step")
                        || e.contains("is not a number")
                        || e.contains("not a whole number"),
                    "{e} in {v}"
                );
            }
        }
    }

    #[test]
    fn reads_and_flags() {
        let e = parse("{\"a\":1}\n\nnot json\n[1]\n");
        assert!(e[0].event().is_some());
        assert!(e[1].blank);
        assert!(e[2].error.as_ref().unwrap().contains("not valid JSON"));
        assert_eq!(e[3].error.as_deref(), Some("not a JSON object"));
    }

    #[test]
    fn shapes() {
        let ok = json!({"contract":"gdd/1","ts":"2026-11-10T10:00:00Z","brief":"x","event":"ask","of":"brief","what":"Approve it"});
        assert!(shape(&ok, true).0.is_empty());
        let bad = json!({"contract":"gdd/1","ts":"2026-11-10T10:00:00Z","brief":"x","event":"ask","of":"lunch","what":"x"});
        assert!(!shape(&bad, false).0.is_empty());
        let unknown =
            json!({"contract":"gdd/1","ts":"2026-11-10T10:00:00Z","brief":"x","event":"note"});
        assert!(shape(&unknown, false).0[0].contains("not an event"));
        let gates = json!({"contract":"gdd/1","ts":"2026-11-10T10:00:00Z","brief":"x","event":"step","step":"gates","start":"abc","gates":[{"check":"seal","result":"maybe"}]});
        let (errors, warnings) = shape(&gates, false);
        assert!(errors.iter().any(|e| e.contains("gates[0].result")));
        assert!(warnings.iter().any(|w| w.contains("seconds is missing")));
    }

    #[test]
    fn counts_like_the_counter() {
        let v = json!({"report":{"tokens":12.7},"x":[{"tokens":3}],"tokens":"9"});
        assert_eq!(tokens_in(&v), 15);
        assert_eq!(number_like(&json!("60")), Some(60));
        assert_eq!(number_like(&json!(45)), Some(45));
        assert!(any_kv(
            &json!({"gates":[{"result":"fail"}]}),
            "result",
            "fail"
        ));
    }

    #[test]
    fn appends_one_line() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("gdd/log/x.jsonl");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, "{\"a\":1}").unwrap();
        let ev = json!({"contract":"gdd/1","ts":"2026-11-10T10:00:00Z","brief":"x","event":"ask","of":"brief","what":"y"});
        assert_eq!(append(&p, &ev).unwrap(), 2);
        let text = std::fs::read_to_string(&p).unwrap();
        assert_eq!(text.lines().count(), 2);
        assert!(text.ends_with("}\n"));
        let bad = json!({"contract":"gdd/1","ts":"2026-11-10T10:00:00Z","brief":"x","event":"ask"});
        assert!(append(&p, &bad).is_err());
    }
}
