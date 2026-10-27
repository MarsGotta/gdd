//! The conversation with the human (`gdd session`): the requests of the interactive Claude Code
//! sessions of a project go from their transcripts into the log, one line per model, and no
//! request is ever counted twice. The sessions gdd starts for its agents are logged when they
//! end, so they are left out.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, ErrorKind::NotFound};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Map, Value};

use crate::{change, log};

/// The phases of the method: a session is counted in one of them.
pub const PHASES: [&str; 6] = [
    "0-setup",
    "1-discover",
    "2-build",
    "3-integrate",
    "4-release",
    "5-learn",
];

/// The sessions gdd started for its agents, from the state root (see [`crate::state::root`]):
/// their ids, one per line.
const HEADLESS: &str = ".gdd/headless_sessions.txt";

/// The requests already in the log, from the state root: their ids, as a JSON list.
const IMPORTED: &str = ".gdd/imported_requests.json";

/// Where the new list of [`IMPORTED`] waits, from the state root, until the log has the requests
/// it adds.
const IMPORTING: &str = ".gdd/imported_requests.json.new";

/// What requests used: how many they are and their tokens, as the log counts them.
#[derive(Debug, Default)]
struct Used {
    /// How many requests.
    calls: usize,
    /// Input tokens, the ones written to the cache included.
    tokens_in: u64,
    /// Input tokens read from the cache.
    tokens_cache: u64,
    /// Output tokens.
    tokens_out: u64,
}

impl Used {
    /// What one request used, from its `usage` as Claude Code reports it.
    fn of(usage: &Map<String, Value>) -> Used {
        let count = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
        let written = count("cache_creation_input_tokens");
        Used {
            calls: 1,
            tokens_in: count("input_tokens").saturating_add(written),
            tokens_cache: count("cache_read_input_tokens"),
            tokens_out: count("output_tokens"),
        }
    }

    /// Counts what `other` used too.
    fn count(&mut self, other: &Used) {
        self.calls = self.calls.saturating_add(other.calls);
        self.tokens_in = self.tokens_in.saturating_add(other.tokens_in);
        self.tokens_cache = self.tokens_cache.saturating_add(other.tokens_cache);
        self.tokens_out = self.tokens_out.saturating_add(other.tokens_out);
    }
}

/// A request of a session: its model and what it used.
#[derive(Debug)]
struct Request {
    model: String,
    used: Used,
}

/// A line of the log for the requests of one model, with every field in the order the log
/// writes them.
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

/// Adds to the log of the project at `root`, whose state root is `state`, the requests of its
/// Claude Code sessions that are not there yet, in `phase` and for change `id` (`-` without
/// one). The transcripts are the `.jsonl` files of `<home>/.claude/projects/<root>`, `<root>`
/// being the canonical root with every character outside `A-Za-z0-9` turned into `-`, but those
/// of the sessions in `.gdd/headless_sessions.txt`. The log gets the new requests, one line per
/// model, and they join the ones in `.gdd/imported_requests.json`: a request is never counted
/// twice, and when either file cannot be written both stay as they were, so the import can run
/// again. What to tell the human, in one line. An id that cannot name a change is an error before
/// anything is read.
pub fn import(
    root: &Path,
    state: &Path,
    home: &Path,
    phase: &str,
    id: Option<&str>,
) -> Result<String, String> {
    if let Some(id) = id {
        change::check_id(id)?;
    }
    let folder = transcripts(root, home)?;
    let headless = read(state, HEADLESS)?.unwrap_or_default();
    let headless: BTreeSet<&str> = headless.split_whitespace().collect();
    let mut imported: BTreeSet<String> = match read(state, IMPORTED)? {
        Some(text) => serde_json::from_str(&text).map_err(|e| format!("{IMPORTED}: {e}"))?,
        None => BTreeSet::new(),
    };
    let new = requests(&folder, &headless, &imported)?;
    imported.extend(new.keys().cloned());
    let mut models: BTreeMap<&str, Used> = BTreeMap::new();
    for request in new.values() {
        let used = models.entry(&request.model).or_default();
        used.count(&request.used);
    }
    let lines = lines(phase, id.unwrap_or("-"), &models)?;
    kept(state, &lines, &imported)?;
    Ok(format!(
        "Session imported: {} new requests across {} model(s), phase {phase}.",
        new.len(),
        models.len()
    ))
}

/// Where Claude Code keeps the transcripts of the project at `root`, under the home folder
/// `home`: `<home>/.claude/projects/<root>`, the canonical root with every character outside
/// `A-Za-z0-9` turned into `-`.
fn transcripts(root: &Path, home: &Path) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(root);
    let canonical = canonical.map_err(|e| format!("cannot resolve {}: {e}", root.display()))?;
    let projects = home.join(".claude/projects");
    Ok(projects.join(folder_name(&canonical)))
}

/// `root` with every character outside `A-Za-z0-9` turned into `-`.
fn folder_name(root: &Path) -> String {
    let root = root.to_string_lossy();
    let named = |c: char| if c.is_ascii_alphanumeric() { c } else { '-' };
    root.chars().map(named).collect()
}

/// The text of the file `path` under the state root `state`; `None` when there is none.
fn read(state: &Path, path: &str) -> Result<Option<String>, String> {
    match fs::read_to_string(state.join(path)) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == NotFound => Ok(None),
        Err(e) => Err(format!("cannot read {path}: {e}")),
    }
}

/// The requests of the transcripts in `folder` that `imported` does not have, by id; the
/// transcripts of the sessions in `headless` are left out. Of the lines with the same id, the
/// last one counts.
fn requests(
    folder: &Path,
    headless: &BTreeSet<&str>,
    imported: &BTreeSet<String>,
) -> Result<BTreeMap<String, Request>, String> {
    let mut found = BTreeMap::new();
    for path in transcripts_in(folder)? {
        let session = path.file_stem().unwrap_or_default().to_string_lossy();
        if headless.contains(&*session) {
            continue;
        }
        let bytes = fs::read(&path).map_err(|e| unread(&path, &e))?;
        for line in String::from_utf8_lossy(&bytes).lines() {
            let new = request(line).filter(|(id, _)| !imported.contains(id));
            if let Some((id, request)) = new {
                found.insert(id, request);
            }
        }
    }
    Ok(found)
}

/// The transcripts in `folder`, sorted: its `.jsonl` files. No folder, no transcripts.
fn transcripts_in(folder: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(e) if e.kind() == NotFound => return Ok(Vec::new()),
        Err(e) => return Err(unread(folder, &e)),
    };
    let mut paths = Vec::new();
    for entry in entries {
        let path = entry.map_err(|e| unread(folder, &e))?.path();
        if path.extension().is_some_and(|ext| ext == "jsonl") && path.is_file() {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

/// Why `path` could not be read.
fn unread(path: &Path, e: &io::Error) -> String {
    format!("cannot read {}: {e}", path.display())
}

/// The request a line of a transcript records, with its id: `requestId`, else `uuid`. Its model
/// is `-` when the line names none. `None` for a line that records no request: not JSON, without
/// an id, or with a message without usage.
fn request(line: &str) -> Option<(String, Request)> {
    let entry: Value = serde_json::from_str(line).ok()?;
    let message = entry.get("message")?;
    let usage = message.get("usage")?.as_object()?;
    if usage.is_empty() {
        return None;
    }
    let id = ["requestId", "uuid"]
        .iter()
        .find_map(|key| entry.get(key)?.as_str().filter(|id| !id.is_empty()))?;
    let model = message.get("model").and_then(Value::as_str);
    let request = Request {
        model: model.unwrap_or("-").to_string(),
        used: Used::of(usage),
    };
    Some((id.to_string(), request))
}

/// Adds `lines` to the log under the state root `state`, and makes `imported` its list of the
/// requests the log has. The list waits aside until the log has the lines, so a failure leaves
/// the log and the old list as they were; the lines go in one write.
fn kept(state: &Path, lines: &[String], imported: &BTreeSet<String>) -> Result<(), String> {
    let text = serde_json::to_string_pretty(imported).map_err(|e| e.to_string())?;
    change::write(state, IMPORTING, &format!("{text}\n"))?;
    if !lines.is_empty()
        && let Err(e) = log::append(state, &lines.join("\n"))
    {
        // Its requests are not in the log: the old list stays, and the next import retries.
        let _ = fs::remove_file(state.join(IMPORTING));
        return Err(format!("cannot write {}: {e}", log::PATH));
    }
    fs::rename(state.join(IMPORTING), state.join(IMPORTED))
        .map_err(|e| format!("cannot write {IMPORTED}: {e}"))
}

/// The lines of the log for `models`, one per model with what its requests used: the
/// Orchestrator's conversation with the human, in `phase`, for `change`.
fn lines(phase: &str, change: &str, models: &BTreeMap<&str, Used>) -> Result<Vec<String>, String> {
    let ts = log::now();
    let mut lines = Vec::new();
    for (&model, used) in models {
        let note = format!("session with the human · {} requests", used.calls);
        let line = Line {
            ts: &ts,
            change,
            phase,
            gate: "-",
            role: "orchestrator",
            agent: "claude-interactive",
            model,
            tokens_in: used.tokens_in,
            tokens_cache: used.tokens_cache,
            tokens_out: used.tokens_out,
            cost_usd: 0.0,
            duration_s: 0.0,
            result: "ok",
            retry: 0,
            note: &note,
        };
        lines.push(serde_json::to_string(&line).map_err(|e| e.to_string())?);
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::Folder;

    /// A project, and a home folder for the transcripts of its Claude Code sessions.
    struct Sessions {
        root: Folder,
        home: Folder,
    }

    impl Sessions {
        fn new() -> Sessions {
            Sessions {
                root: Folder::new(),
                home: Folder::new(),
            }
        }

        /// The folder of the transcripts of the project.
        fn folder(&self) -> PathBuf {
            transcripts(&self.root.0, &self.home.0).unwrap()
        }

        /// Adds `line` to the transcript of the session `session`.
        fn said(&self, session: &str, line: &str) {
            let path = self.folder().join(format!("{session}.jsonl"));
            let old = fs::read_to_string(&path).unwrap_or_default();
            fs::create_dir_all(self.folder()).unwrap();
            fs::write(path, format!("{old}{line}\n")).unwrap();
        }

        fn import(&self, id: Option<&str>) -> Result<String, String> {
            import(&self.root.0, &self.root.0, &self.home.0, "1-discover", id)
        }

        fn log(&self) -> Vec<log::Record> {
            let text = fs::read_to_string(self.root.0.join(log::PATH));
            log::parse(&text.unwrap_or_default()).unwrap()
        }

        fn imported(&self) -> serde_json::Value {
            let text = fs::read_to_string(self.root.0.join(IMPORTED)).unwrap();
            serde_json::from_str(&text).unwrap()
        }
    }

    /// A request `id` of `model` with 10 input tokens, 3 written to the cache, 5 read from it
    /// and `output` output tokens.
    fn request_line(id: &str, model: &str, output: u64) -> String {
        let usage = serde_json::json!({
            "input_tokens": 10,
            "cache_creation_input_tokens": 3,
            "cache_read_input_tokens": 5,
            "output_tokens": output,
        });
        let message = serde_json::json!({"model": model, "usage": usage});
        serde_json::json!({"requestId": id, "message": message}).to_string()
    }

    /// What an import of `requests` new requests of `models` models says.
    fn message(requests: usize, models: usize) -> String {
        let across = format!("{requests} new requests across {models} model(s)");
        format!("Session imported: {across}, phase 1-discover.")
    }

    #[test]
    fn the_requests_go_to_the_log_one_line_per_model() {
        let sessions = Sessions::new();
        sessions.said("s1", &request_line("r1", "opus", 40));
        sessions.said("s1", &request_line("r2", "opus", 2));
        sessions.said("s1", &request_line("r3", "sonnet", 7));
        sessions.said("s2", &request_line("r4", "opus", 1));
        assert_eq!(sessions.import(None), Ok(message(4, 2)));
        let log = sessions.log();
        let models: Vec<String> = log.iter().map(|line| line.text("model")).collect();
        assert_eq!(models, ["opus", "sonnet"]);
        let opus = &log[0];
        let tokens = (opus.tokens_in, opus.tokens_cache, opus.tokens_out);
        assert_eq!(tokens, (39, 15, 43));
        for (field, value) in [
            ("change", "-"),
            ("phase", "1-discover"),
            ("gate", "-"),
            ("role", "orchestrator"),
            ("agent", "claude-interactive"),
            ("result", "ok"),
            ("note", "session with the human · 3 requests"),
        ] {
            assert_eq!(opus.text(field), value, "{field}");
        }
        assert!(opus.text("ts").ends_with("+00:00"));
        let sonnet = "session with the human · 1 requests";
        assert_eq!(log[1].text("note"), sonnet);
        let ids = serde_json::json!(["r1", "r2", "r3", "r4"]);
        assert_eq!(sessions.imported(), ids);
    }

    #[test]
    fn the_transcripts_are_the_project_s_and_the_log_is_under_its_state_root() {
        let sessions = Sessions::new();
        let state = Folder::new();
        state.write(HEADLESS, "s2\n");
        sessions.said("s1", &request_line("r1", "opus", 40));
        sessions.said("s2", &request_line("r2", "opus", 2));
        let said = import(
            &sessions.root.0,
            &state.0,
            &sessions.home.0,
            "1-discover",
            None,
        );
        assert_eq!(said, Ok(message(1, 1)));
        let text = fs::read_to_string(state.0.join(log::PATH)).unwrap();
        assert_eq!(log::parse(&text).unwrap()[0].tokens_out, 40);
        assert!(state.0.join(IMPORTED).is_file());
        assert!(!sessions.root.0.join(".gdd").exists());
    }

    #[test]
    fn the_same_request_is_never_counted_twice() {
        let sessions = Sessions::new();
        sessions.said("s1", &request_line("r1", "opus", 40));
        sessions.import(None).unwrap();
        assert_eq!(sessions.import(None), Ok(message(0, 0)));
        assert_eq!(sessions.log().len(), 1);
        sessions.said("s1", &request_line("r2", "opus", 2));
        assert_eq!(sessions.import(None), Ok(message(1, 1)));
        assert_eq!(sessions.log()[1].tokens_out, 2);
        let text = fs::read_to_string(sessions.root.0.join(IMPORTED)).unwrap();
        assert_eq!(text, "[\n  \"r1\",\n  \"r2\"\n]\n");
    }

    #[test]
    fn only_lines_with_an_id_and_usage_are_requests() {
        let sessions = Sessions::new();
        for line in [
            "not json",
            "[1]",
            r#"{"requestId": "a", "message": "words"}"#,
            r#"{"requestId": "b", "message": {"model": "m"}}"#,
            r#"{"requestId": "c", "message": {"usage": {}}}"#,
            r#"{"requestId": "d", "message": {"usage": 7}}"#,
            r#"{"message": {"usage": {"output_tokens": 1}}}"#,
            r#"{"requestId": "", "uuid": "u1", "message": {"usage": {"output_tokens": 2}}}"#,
            r#"{"requestId": "r1", "uuid": "u2", "message": {"usage": {"output_tokens": 3}}}"#,
            r#"{"requestId": "r1", "message": {"model": "m", "usage": {"output_tokens": 5}}}"#,
        ] {
            sessions.said("s1", line);
        }
        assert_eq!(sessions.import(None), Ok(message(2, 2)));
        let log = sessions.log();
        let seen: Vec<(String, u64)> = log
            .iter()
            .map(|line| (line.text("model"), line.tokens_out))
            .collect();
        let expected = [("-".to_string(), 2), ("m".to_string(), 5)];
        assert_eq!(seen, expected);
        assert_eq!(sessions.imported(), serde_json::json!(["r1", "u1"]));
    }

    #[test]
    fn sessions_gdd_started_and_other_files_are_left_out() {
        let sessions = Sessions::new();
        for session in ["s1", "s2", "s3"] {
            sessions.said(session, &request_line(session, "opus", 1));
        }
        let headless = sessions.root.0.join(HEADLESS);
        fs::create_dir_all(headless.parent().unwrap()).unwrap();
        fs::write(&headless, "s1\n  s2 \n").unwrap();
        let notes = sessions.folder().join("s4.txt");
        fs::write(notes, request_line("s4", "opus", 1)).unwrap();
        fs::create_dir_all(sessions.folder().join("s5.jsonl")).unwrap();
        assert_eq!(sessions.import(None), Ok(message(1, 1)));
        assert_eq!(sessions.imported(), serde_json::json!(["s3"]));
    }

    #[test]
    fn without_transcripts_nothing_is_imported() {
        let sessions = Sessions::new();
        assert_eq!(sessions.import(None), Ok(message(0, 0)));
        assert!(!sessions.root.0.join(log::PATH).exists());
        assert_eq!(sessions.imported(), serde_json::json!([]));
        assert!(!sessions.root.0.join(IMPORTING).exists());
    }

    #[test]
    fn a_session_can_be_counted_for_one_change() {
        let sessions = Sessions::new();
        sessions.said("s1", &request_line("r1", "opus", 40));
        sessions.import(Some("c1")).unwrap();
        assert_eq!(sessions.log()[0].text("change"), "c1");
        let invalid = change::check_id("C1").unwrap_err();
        assert_eq!(sessions.import(Some("C1")), Err(invalid));
        assert_eq!(sessions.log().len(), 1);
    }

    #[test]
    fn the_transcripts_are_in_the_folder_named_after_the_canonical_root() {
        let path = Path::new("/home/a b/é_x.y");
        assert_eq!(folder_name(path), "-home-a-b---x-y");
        let sessions = Sessions::new();
        let link = sessions.home.0.join("link");
        std::os::unix::fs::symlink(&sessions.root.0, &link).unwrap();
        let canonical = fs::canonicalize(&sessions.root.0).unwrap();
        let named = folder_name(&canonical);
        let expected = sessions.home.0.join(".claude/projects").join(named);
        assert_eq!(transcripts(&link, &sessions.home.0), Ok(expected));
        let missing = sessions.root.0.join("missing");
        let err = transcripts(&missing, &sessions.home.0).unwrap_err();
        let start = format!("cannot resolve {}: ", missing.display());
        assert!(err.starts_with(&start), "{err}");
    }

    #[test]
    fn files_that_cannot_be_read_are_errors() {
        let sessions = Sessions::new();
        for path in [HEADLESS, IMPORTED] {
            fs::create_dir_all(sessions.root.0.join(path)).unwrap();
            let err = sessions.import(None).unwrap_err();
            let start = format!("cannot read {path}: ");
            assert!(err.starts_with(&start), "{err}");
            fs::remove_dir(sessions.root.0.join(path)).unwrap();
        }
        fs::write(sessions.root.0.join(IMPORTED), "{not json").unwrap();
        let err = sessions.import(None).unwrap_err();
        assert!(err.starts_with(".gdd/imported_requests.json: "), "{err}");
        fs::remove_file(sessions.root.0.join(IMPORTED)).unwrap();
        let folder = sessions.folder();
        fs::create_dir_all(folder.parent().unwrap()).unwrap();
        fs::write(&folder, "not a folder\n").unwrap();
        let err = sessions.import(None).unwrap_err();
        let start = format!("cannot read {}: ", folder.display());
        assert!(err.starts_with(&start), "{err}");
    }

    #[test]
    fn a_log_that_cannot_be_written_loses_no_request() {
        let sessions = Sessions::new();
        sessions.said("s1", &request_line("r1", "opus", 40));
        sessions.import(None).unwrap();
        sessions.said("s1", &request_line("r2", "sonnet", 2));
        sessions.said("s1", &request_line("r3", "opus", 3));
        let log = sessions.root.0.join(log::PATH);
        let logged = fs::read_to_string(&log).unwrap();
        fs::remove_file(&log).unwrap();
        fs::create_dir(&log).unwrap();
        let err = sessions.import(None).unwrap_err();
        assert!(err.starts_with("cannot write .gdd/log.jsonl: "), "{err}");
        assert_eq!(sessions.imported(), serde_json::json!(["r1"]));
        assert!(!sessions.root.0.join(IMPORTING).exists());
        fs::remove_dir(&log).unwrap();
        fs::write(&log, logged).unwrap();
        assert_eq!(sessions.import(None), Ok(message(2, 2)));
        let outputs: Vec<u64> = sessions.log().iter().map(|l| l.tokens_out).collect();
        assert_eq!(outputs, [40, 3, 2]);
        assert_eq!(sessions.imported(), serde_json::json!(["r1", "r2", "r3"]));
    }

    #[test]
    fn a_list_that_cannot_be_written_leaves_the_log_alone() {
        let sessions = Sessions::new();
        sessions.said("s1", &request_line("r1", "opus", 40));
        fs::create_dir_all(sessions.root.0.join(IMPORTING)).unwrap();
        let err = sessions.import(None).unwrap_err();
        let start = format!("cannot write {IMPORTING}: ");
        assert!(err.starts_with(&start), "{err}");
        assert!(!sessions.root.0.join(log::PATH).exists());
        assert!(!sessions.root.0.join(IMPORTED).exists());
        fs::remove_dir(sessions.root.0.join(IMPORTING)).unwrap();
        assert_eq!(sessions.import(None), Ok(message(1, 1)));
        assert_eq!(sessions.log().len(), 1);
    }
}
