//! The log: one JSON record per line, for every agent call, gate decision and machine step.

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Where the log lives, from the project root.
pub const PATH: &str = ".gdd/log.jsonl";

/// A machine step for the log: what the tool did by itself. The fields of an agent call (model,
/// tokens, cost, retry) take the values the log writes when they are not given.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Id of the change.
    pub change: String,
    /// Phase of the method (`2-build`...).
    pub phase: String,
    /// Gate (`G2`...), or `-`.
    pub gate: String,
    /// Who acted: `machine` for the tool itself.
    pub role: String,
    /// What acted (`checks` for the commands of a gate).
    pub agent: String,
    /// `ok` or `ko`.
    pub result: String,
    /// Time it took, in seconds.
    pub duration_s: f64,
    /// What to read about it.
    pub note: String,
}

/// A line of the log with every field, in the order the log writes them.
#[derive(Serialize)]
struct Line<'a> {
    ts: &'a str,
    change: &'a str,
    phase: &'a str,
    gate: &'a str,
    role: &'a str,
    agent: &'a str,
    model: &'a str,
    tokens_in: u64,
    tokens_cache: u64,
    tokens_out: u64,
    cost_usd: f64,
    duration_s: f64,
    result: &'a str,
    retry: u64,
    note: &'a str,
}

impl Entry {
    /// The line of the log for this step, done at `ts` (see [`now`]): one JSON object.
    pub fn line(&self, ts: &str) -> serde_json::Result<String> {
        serde_json::to_string(&Line {
            ts,
            change: &self.change,
            phase: &self.phase,
            gate: &self.gate,
            role: &self.role,
            agent: &self.agent,
            model: "-",
            tokens_in: 0,
            tokens_cache: 0,
            tokens_out: 0,
            cost_usd: 0.0,
            duration_s: self.duration_s,
            result: &self.result,
            retry: 0,
            note: &self.note,
        })
    }
}

/// Adds `line` at the end of the log of the project at `root`, creating the log when there is
/// none.
pub fn append(root: &Path, line: &str) -> io::Result<()> {
    let path = root.join(PATH);
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    let mut log = OpenOptions::new().create(true).append(true).open(path)?;
    // One write per line, so lines written at the same time do not mix.
    log.write_all(format!("{line}\n").as_bytes())
}

/// The time now, in UTC to the second, as the log writes it: `2026-10-26T02:14:07+00:00`.
pub fn now() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    timestamp(seconds)
}

/// The UTC time `seconds` after 1970-01-01T00:00:00, as the log writes it.
fn timestamp(seconds: u64) -> String {
    let (year, month, day) = civil_date(seconds / 86_400);
    let time = seconds % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}+00:00",
        time / 3_600,
        time / 60 % 60,
        time % 60
    )
}

/// The date `days` days after 1970-01-01: year, month and day, in the Gregorian calendar.
/// Howard Hinnant's `civil_from_days`, on eras of 400 years that start on March 1st.
fn civil_date(days: u64) -> (u64, u64, u64) {
    let shifted = days + 719_468; // days since 0000-03-01
    let era = shifted / 146_097;
    let day_of_era = shifted % 146_097;
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
    let year = era * 400 + year_of_era + u64::from(month <= 2);
    (year, month, day)
}

/// One line of the log. A field the line leaves out takes the value the log writes when it is
/// not given: 0 for numbers, `-` for text (see [`Record::text`]).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Record {
    /// Input tokens that were not read from the cache.
    pub tokens_in: u64,
    /// Input tokens read from the cache.
    pub tokens_cache: u64,
    /// Output tokens.
    pub tokens_out: u64,
    /// Cost in US dollars.
    pub cost_usd: f64,
    /// Time it took, in seconds.
    pub duration_s: f64,
    /// Every other field: `ts`, `change`, `phase`, `gate`, `role`, `agent`, `model`, `result`,
    /// `retry`, `note`...
    #[serde(flatten)]
    pub fields: Map<String, Value>,
}

impl Record {
    /// The value of one of the other fields as text: `-` when the line leaves it out, the text
    /// itself for a string, its JSON for anything else.
    pub fn text(&self, field: &str) -> String {
        match self.fields.get(field) {
            None => "-".to_string(),
            Some(Value::String(text)) => text.clone(),
            Some(value) => value.to_string(),
        }
    }

    /// Whether the record is an agent call: it used tokens in or out.
    /// Machine steps and human gates leave both at 0.
    pub fn is_agent_call(&self) -> bool {
        self.tokens_in > 0 || self.tokens_out > 0
    }
}

/// A line of the log that is not a record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineError {
    /// Number of the line, from 1.
    pub line: usize,
    /// What is wrong with it.
    pub message: String,
}

impl fmt::Display for LineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for LineError {}

/// Reads the text of a log: one record per line; blank lines are skipped. The first line that
/// is not a JSON object with the fields of a record is an error that names it.
pub fn parse(text: &str) -> Result<Vec<Record>, LineError> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(i, line)| {
            serde_json::from_str(line).map_err(|e| LineError {
                line: i + 1,
                message: without_position(&e),
            })
        })
        .collect()
}

/// The message of a JSON error without its position: the JSON of a record is a single line,
/// so the line that counts is the line of the log.
fn without_position(e: &serde_json::Error) -> String {
    let message = e.to_string();
    let suffix = format!(" at line {} column {}", e.line(), e.column());
    match message.strip_suffix(&suffix) {
        Some(reason) => reason.to_string(),
        None => message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_line_gives_its_numbers_and_its_fields() {
        let records = parse(
            r#"{"ts": "2026-10-25T10:00:00+00:00", "change": "a", "phase": "2-build", "gate": "-", "role": "builder-build", "agent": "claude", "model": "opus", "tokens_in": 100, "tokens_cache": 400, "tokens_out": 20, "cost_usd": 0.01, "duration_s": 5.5, "result": "ok", "retry": 1, "note": "/speckit-gdd-build"}"#,
        )
        .unwrap();
        let [r] = records.as_slice() else {
            panic!("one record, got {records:?}");
        };
        assert_eq!((r.tokens_in, r.tokens_cache, r.tokens_out), (100, 400, 20));
        assert_eq!((r.cost_usd, r.duration_s), (0.01, 5.5));
        assert_eq!(r.text("change"), "a");
        assert_eq!(r.text("model"), "opus");
        assert_eq!(r.text("note"), "/speckit-gdd-build");
        assert_eq!(r.text("retry"), "1");
    }

    #[test]
    fn fields_left_out_take_the_defaults_of_the_log() {
        let records =
            parse(r#"{"change": "a", "gate": "G1", "role": "human", "result": "ok"}"#).unwrap();
        let r = &records[0];
        assert_eq!((r.tokens_in, r.tokens_cache, r.tokens_out), (0, 0, 0));
        assert_eq!((r.cost_usd, r.duration_s), (0.0, 0.0));
        assert_eq!(r.text("model"), "-");
        assert_eq!(r.text("role"), "human");
        assert!(!r.is_agent_call());
    }

    #[test]
    fn values_that_are_not_text_are_their_json() {
        let records = parse(r#"{"note": null, "retry": 2, "x": [1, true]}"#).unwrap();
        assert_eq!(records[0].text("note"), "null");
        assert_eq!(records[0].text("retry"), "2");
        assert_eq!(records[0].text("x"), "[1,true]");
    }

    #[test]
    fn whole_numbers_are_valid_costs_and_durations() {
        let records = parse(r#"{"cost_usd": 1, "duration_s": 3}"#).unwrap();
        assert_eq!((records[0].cost_usd, records[0].duration_s), (1.0, 3.0));
    }

    #[test]
    fn an_agent_call_used_tokens_in_or_out() {
        let call = |tokens_in, tokens_cache, tokens_out| Record {
            tokens_in,
            tokens_cache,
            tokens_out,
            ..Record::default()
        };
        assert!(call(1, 0, 0).is_agent_call());
        assert!(call(0, 0, 1).is_agent_call());
        assert!(!call(0, 5, 0).is_agent_call());
        assert!(!call(0, 0, 0).is_agent_call());
    }

    #[test]
    fn one_record_per_line_in_order() {
        let records =
            parse("{\"change\": \"a\"}\n{\"change\": \"b\"}\r\n{\"change\": \"c\"}\n").unwrap();
        let changes: Vec<String> = records.iter().map(|r| r.text("change")).collect();
        assert_eq!(changes, ["a", "b", "c"]);
        assert_eq!(parse(""), Ok(vec![]));
    }

    #[test]
    fn blank_lines_are_skipped_but_counted() {
        let records = parse("\n{\"change\": \"a\"}\n  \n\t\n{\"change\": \"b\"}\n\n").unwrap();
        let changes: Vec<String> = records.iter().map(|r| r.text("change")).collect();
        assert_eq!(changes, ["a", "b"]);
        assert_eq!(parse("\n{}\n\n{x\n").map_err(|e| e.line), Err(4));
    }

    #[test]
    fn a_broken_line_is_an_error_that_names_it() {
        let err = parse("{}\n{}\n{not json\n{}\n").unwrap_err();
        assert_eq!(err.line, 3);
        assert_eq!(err.message, "key must be a string");
        assert_eq!(err.to_string(), "line 3: key must be a string");
    }

    #[test]
    fn lines_that_are_not_records_are_errors() {
        for (text, line) in [
            ("[1, 2]", 1),
            ("{}\n42", 2),
            ("{}\n\n\"text\"", 3),
            (r#"{"tokens_in": "many"}"#, 1),
            (r#"{"tokens_out": -1}"#, 1),
            (r#"{"cost_usd": null}"#, 1),
        ] {
            assert_eq!(parse(text).map_err(|e| e.line), Err(line), "{text:?}");
        }
    }

    #[test]
    fn an_error_without_a_position_keeps_its_message() {
        let e = <serde_json::Error as serde::de::Error>::custom("no position");
        assert_eq!(without_position(&e), "no position");
    }

    fn entry() -> Entry {
        Entry {
            change: "c1".into(),
            phase: "2-build".into(),
            gate: "G2".into(),
            role: "machine".into(),
            agent: "checks".into(),
            result: "ko".into(),
            duration_s: 1.5,
            note: "unit, \"lint\" ñ".into(),
        }
    }

    #[test]
    fn a_machine_step_writes_every_field_of_the_log_in_its_order() {
        let line = entry().line("2026-10-26T02:14:07+00:00").unwrap();
        assert_eq!(
            line,
            concat!(
                r#"{"ts":"2026-10-26T02:14:07+00:00","change":"c1","phase":"2-build","gate":"G2","#,
                r#""role":"machine","agent":"checks","model":"-","tokens_in":0,"tokens_cache":0,"#,
                r#""tokens_out":0,"cost_usd":0.0,"duration_s":1.5,"result":"ko","retry":0,"#,
                r#""note":"unit, \"lint\" ñ"}"#
            )
        );
        let records = parse(&line).unwrap();
        assert_eq!(records[0].duration_s, 1.5);
        assert_eq!(records[0].text("gate"), "G2");
        assert!(!records[0].is_agent_call());
    }

    #[test]
    fn lines_are_appended_to_the_log_of_the_project() {
        let root = std::env::temp_dir().join(format!("gdd-log-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        append(&root, "{\"change\": \"a\"}").unwrap();
        append(&root, "{\"change\": \"b\"}").unwrap();
        let text = fs::read_to_string(root.join(PATH)).unwrap();
        let _ = fs::remove_dir_all(&root);
        assert_eq!(text, "{\"change\": \"a\"}\n{\"change\": \"b\"}\n");
        let file = std::env::temp_dir().join(format!("gdd-log-file-{}", std::process::id()));
        fs::write(&file, "").unwrap();
        let result = append(&file, "{}");
        let _ = fs::remove_file(&file);
        assert!(result.is_err(), "a project root that is a file");
    }

    #[test]
    fn timestamps_are_utc_dates_to_the_second() {
        assert_eq!(timestamp(0), "1970-01-01T00:00:00+00:00");
        assert_eq!(timestamp(951_782_399), "2000-02-28T23:59:59+00:00");
        assert_eq!(timestamp(951_782_400), "2000-02-29T00:00:00+00:00");
        assert_eq!(timestamp(951_868_800), "2000-03-01T00:00:00+00:00");
        assert_eq!(timestamp(1_700_000_000), "2023-11-14T22:13:20+00:00");
        assert_eq!(timestamp(1_735_689_599), "2024-12-31T23:59:59+00:00");
        assert_eq!(timestamp(4_107_542_400), "2100-03-01T00:00:00+00:00");
        let now = now();
        assert_eq!(now.len(), "2026-10-26T02:14:07+00:00".len());
        assert!(now.ends_with("+00:00") && now.as_str() > "2026", "{now}");
    }
}
