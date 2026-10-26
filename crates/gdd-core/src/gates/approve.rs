//! Recording the gates a person or the machines decide (`gdd gate`): who may record what, and
//! what each record keeps.
//!
//! The machines open G0, once the Project Brief is written and the harness self-test passed. A
//! human approves the red zones, and the approval keeps their hash: changing them asks for a new
//! one. The plan, G1, G2, G3 and G4 are recorded on a change. Approving G1 seals the scenario
//! files of the change, as its branch has them, with their SHA-256, and refuses hidden examples
//! with sentences no visible scenario has. A Builder or a Reviewer can never record anything.

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::fs;
use std::io::{self, ErrorKind::NotADirectory, ErrorKind::NotFound};
use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::change::{self, Approval, Record};
use crate::config;
use crate::gates::phrasing;
use crate::git::{self, Git};
use crate::guard::{self, Settings};
use crate::log;
use crate::rules::Acceptance;
use crate::zones::ZoneMap;

/// Where the decision on G0 lives, from the project root.
pub const G0_PATH: &str = ".gdd/g0.json";

/// Where the approval of the red zones lives, from the project root.
pub const ZONES_PATH: &str = ".gdd/zones.json";

/// Where the result of the harness self-test lives, from the project root.
pub const SELFTEST_PATH: &str = ".gdd/selftest.json";

/// The Project Brief, from the project root.
pub const BRIEF: &str = "docs/brief.md";

/// The gates `gdd gate` records, by name.
pub const GATES: [&str; 7] = ["G0", "zones", "plan", "G1", "G2", "G3", "G4"];

/// Sentences a refusal of G1 names, at most.
const NAMED: usize = 10;

/// Why a Builder or a Reviewer records nothing.
const NOT_FOR_AGENTS: &str = "A Builder or Reviewer cannot open or close gates.";

/// Why G0 is only opened by the machines.
const G0_BY_MACHINES: &str =
    "G0 is opened by the machines (--by machines): Brief written and harness 4/4.";

/// Why G0 does not open without a passing self-test.
const G0_UNTESTED: &str = "G0 cannot open: the harness self-test is not 4/4 (bin/gdd selftest).";

/// Why G0 does not open without the Project Brief.
const G0_NO_BRIEF: &str = "G0 cannot open: docs/brief.md (Project Brief) is missing.";

/// Why the red zones are only approved by a human.
const ZONES_BY_HUMAN: &str = "Red zones are approved by a human (--by human).";

/// Why G1 refuses hidden examples with sentences no visible scenario has.
const UNKNOWN_SENTENCES: &str = "G1 refused: hidden examples use step phrasings that no visible \
                                 scenario has, so test mode can never write them. Reuse visible \
                                 sentences with new data:";

/// What red zones no human approved yet mean.
const NOT_APPROVED: &str =
    "Red zones not approved yet: the Orchestrator proposes them at the end of Discover.";

/// What red zones changed after their approval mean.
const CHANGED: &str = "Red zones changed after their approval: a human must approve them again.";

/// A gate `gdd gate` records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// The project is ready: opened by the machines.
    G0,
    /// The red zones: approved by a human.
    Zones,
    /// The plan of a red change.
    Plan,
    /// The scenarios of a change.
    G1,
    /// The Loop Gate.
    G2,
    /// The Merge Gate.
    G3,
    /// The release.
    G4,
}

impl Gate {
    const ALL: [Gate; 7] = [
        Gate::G0,
        Gate::Zones,
        Gate::Plan,
        Gate::G1,
        Gate::G2,
        Gate::G3,
        Gate::G4,
    ];

    /// The phase of the method it belongs to.
    pub fn phase(self) -> &'static str {
        match self {
            Gate::G0 => "0-setup",
            Gate::Zones | Gate::Plan | Gate::G1 => "1-discover",
            Gate::G2 => "2-build",
            Gate::G3 => "3-integrate",
            Gate::G4 => "4-release",
        }
    }
}

impl fmt::Display for Gate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Gate::G0 => "G0",
            Gate::Zones => "zones",
            Gate::Plan => "plan",
            Gate::G1 => "G1",
            Gate::G2 => "G2",
            Gate::G3 => "G3",
            Gate::G4 => "G4",
        })
    }
}

impl FromStr for Gate {
    type Err = Error;

    fn from_str(name: &str) -> Result<Gate, Error> {
        let gate = Gate::ALL.into_iter().find(|g| g.to_string() == name);
        gate.ok_or_else(|| {
            let why = format!("unknown gate {name:?}: use one of {}", GATES.join(", "));
            Error::Refused(why)
        })
    }
}

/// What a gate decision says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// The gate passes.
    Ok,
    /// The gate does not pass.
    Ko,
}

impl Decision {
    /// The decision a word says: `ok` or `ko`; `approve` and `reject` say the same.
    pub fn parse(word: &str) -> Result<Decision, Error> {
        match word {
            "ok" | "approve" => Ok(Decision::Ok),
            "ko" | "reject" => Ok(Decision::Ko),
            _ => Err(refused("The decision is ok or ko.")),
        }
    }
}

impl fmt::Display for Decision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Decision::Ok => "ok",
            Decision::Ko => "ko",
        })
    }
}

/// A decision to record, as `gdd gate` takes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request<'a> {
    /// The gate.
    pub gate: Gate,
    /// The decision as written: `ok`, `ko`, `approve` or `reject`.
    pub decision: &'a str,
    /// The change it is recorded on: needed by the plan, G1, G2, G3 and G4.
    pub change: Option<&'a str>,
    /// Who decides: `human`, `machines`...
    pub by: &'a str,
    /// What to read about it.
    pub note: &'a str,
}

/// What recording a decision reads from the project.
#[derive(Debug, Clone, Copy)]
pub struct Project<'a> {
    /// The project root.
    pub root: &'a Path,
    /// The text of the zone map; empty when there is none.
    pub zones: &'a str,
    /// What the rules file says to Path Guard: the main branch.
    pub settings: &'a Settings,
    /// Where the scenarios live.
    pub acceptance: &'a Acceptance,
}

/// Why a decision was not recorded.
#[derive(Debug)]
pub enum Error {
    /// The rules of who records what refuse it: why.
    Refused(String),
    /// A file of the project could not be read, written or understood: what went wrong.
    Project(String),
    /// Reading git failed.
    Git(git::Error),
    /// The zone map is not valid.
    Zones(config::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Refused(message) | Error::Project(message) => f.write_str(message),
            Error::Git(e) => write!(f, "{e}"),
            Error::Zones(e) => write!(f, "gdd/zones.toml: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<git::Error> for Error {
    fn from(e: git::Error) -> Error {
        Error::Git(e)
    }
}

fn refused(why: &str) -> Error {
    Error::Refused(why.to_string())
}

/// The error of a file of the project that cannot be read.
fn unreadable(path: &str, e: io::Error) -> Error {
    Error::Project(format!("cannot read {path}: {e}"))
}

/// The error of a file of the project that cannot be written.
fn unwritable(path: &str, e: io::Error) -> Error {
    Error::Project(format!("cannot write {path}: {e}"))
}

/// Records the decision `request` asks for, by the role `role` (`None` for a person or the
/// Orchestrator), and logs it. A Builder or a Reviewer is refused before anything else; then a
/// decision other than `ok` or `ko`. What it returns says what was recorded, in one line.
pub fn record(project: &Project, request: &Request, role: Option<&str>) -> Result<String, Error> {
    may_record(role)?;
    let decision = Decision::parse(request.decision)?;
    let ts = log::now();
    match request.gate {
        Gate::G0 => open_g0(project, request, decision, &ts),
        Gate::Zones => approve_zones(project, request, decision, &ts),
        _ => record_on_change(project, request, decision, &ts),
    }
}

/// `Ok` unless `role` is a Builder or a Reviewer: they can never record a decision.
fn may_record(role: Option<&str>) -> Result<(), Error> {
    let role = role.unwrap_or_default();
    if role.starts_with("builder") || role.starts_with("reviewer") {
        return Err(refused(NOT_FOR_AGENTS));
    }
    Ok(())
}

/// G0: only the machines open it, and only with the Project Brief and a passing self-test.
fn open_g0(
    project: &Project,
    request: &Request,
    decision: Decision,
    ts: &str,
) -> Result<String, Error> {
    let root = project.root;
    if request.by != "machines" {
        return Err(refused(G0_BY_MACHINES));
    }
    if decision == Decision::Ok && !self_test_passed(root)? {
        return Err(refused(G0_UNTESTED));
    }
    if decision == Decision::Ok && !root.join(BRIEF).is_file() {
        return Err(refused(G0_NO_BRIEF));
    }
    let decided = decision.to_string();
    let approval = Approval {
        decision: &decided,
        by: request.by,
        ts,
        note: request.note,
        hashes: None,
    };
    write_json(root, G0_PATH, &approval)?;
    logged(root, ts, "-", "machine", request, decision)?;
    Ok(format!("G0 {decision} ({}).", request.by))
}

/// What the approval of the red zones keeps.
#[derive(Serialize)]
struct ZonesApproval<'a> {
    decision: &'a str,
    by: &'a str,
    ts: &'a str,
    red_hash: &'a str,
    red: &'a [toml::Table],
    note: &'a str,
}

/// The red zones: only a human approves them; the approval keeps them and their hash.
fn approve_zones(
    project: &Project,
    request: &Request,
    decision: Decision,
    ts: &str,
) -> Result<String, Error> {
    if request.by != "human" {
        return Err(refused(ZONES_BY_HUMAN));
    }
    if let Some(id) = request.change {
        change::check_id(id).map_err(Error::Refused)?;
    }
    let reds = red_zones(project.zones)?;
    let decided = decision.to_string();
    let approval = ZonesApproval {
        decision: &decided,
        by: request.by,
        ts,
        red_hash: &red_hash(&reds),
        red: &reds,
        note: request.note,
    };
    write_json(project.root, ZONES_PATH, &approval)?;
    let change = request.change.unwrap_or("-");
    logged(project.root, ts, change, "human", request, decision)?;
    let count = reds.len();
    Ok(format!("Red zones {decision} ({count} red zone(s))."))
}

/// The plan, G1, G2, G3 and G4: recorded on the change. Approving G1 also seals its scenario
/// files (see [`seal`]).
fn record_on_change(
    project: &Project,
    request: &Request,
    decision: Decision,
    ts: &str,
) -> Result<String, Error> {
    let gate = request.gate;
    let Some(id) = request.change else {
        let why = format!("Missing change id: {gate} is recorded on a change.");
        return Err(Error::Refused(why));
    };
    let text = change::read_record(project.root, id).map_err(Error::Project)?;
    let path = change::record_path(id);
    let invalid = |e: serde_json::Error| Error::Project(format!("{path}: {e}"));
    let sealed = if gate == Gate::G1 && decision == Decision::Ok {
        let record = Record::from_json(&text).map_err(invalid)?;
        Some(seal(project, id, &record)?)
    } else {
        None
    };
    let decided = decision.to_string();
    let approval = Approval {
        decision: &decided,
        by: request.by,
        ts,
        note: request.note,
        hashes: sealed.as_ref().map(|s| &s.hashes),
    };
    let features = sealed.as_ref().map(|s| s.features.as_slice());
    let name = gate.to_string();
    let updated = change::with_gate(&text, &name, &approval, features).map_err(invalid)?;
    write(project.root, &path, &updated)?;
    logged(project.root, ts, id, request.by, request, decision)?;
    Ok(format!("{gate} {decision} on {id} ({}).", request.by))
}

/// What approving G1 seals: the scenario files of the change, and the SHA-256 of the ones that
/// exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sealed {
    /// The scenario files of the change.
    pub features: Vec<String>,
    /// The SHA-256 of each one that exists, in hex.
    pub hashes: BTreeMap<String, String>,
}

/// What approving G1 seals on change `id`, whose record is `record`. Its scenario files are the
/// ones it touched against the main branch (or plans, while it touched nothing) that need
/// approval; without any, the ones its record names. They are read as the change's branch has
/// them: the working tree while it is checked out, or when the change has no branch; else its
/// last commit, so approving from another branch seals them too. Hidden examples the change
/// touched whose sentences no visible scenario has are refused: test mode could never write
/// their steps.
pub fn seal(project: &Project, id: &str, record: &Record) -> Result<Sealed, Error> {
    let mut changed = guard::touched(project.root, id, project.settings)?;
    if changed.is_empty() {
        changed = record.plan.paths.clone();
        changed.sort();
        changed.dedup();
    }
    let acceptance = project.acceptance;
    let mut features: Vec<String> = changed
        .iter()
        .filter(|path| acceptance.needs_approval(path))
        .cloned()
        .collect();
    if features.is_empty() {
        features = record.features.clone();
    }
    let view = View::of(project.root, id)?;
    let mut unknown = unknown_sentences(&view, &changed, &acceptance.dir)?;
    if !unknown.is_empty() {
        unknown.truncate(NAMED);
        let why = format!("{UNKNOWN_SENTENCES}\n  - {}", unknown.join("\n  - "));
        return Err(Error::Refused(why));
    }
    let mut hashes = BTreeMap::new();
    for file in &features {
        if let Some(bytes) = view.read(file)? {
            hashes.insert(file.clone(), sha256(&bytes));
        }
    }
    Ok(Sealed { features, hashes })
}

/// The sentences of the hidden examples among `changed`, inside `<dir>/hidden/`, that no
/// visible scenario of `dir` has, as `view` shows them (see [`phrasing::unknown`]).
fn unknown_sentences(view: &View, changed: &[String], dir: &str) -> Result<Vec<String>, Error> {
    let hidden_dir = format!("{dir}/hidden/");
    let is_feature = |path: &String| path.ends_with(".feature");
    let hidden: Vec<String> = changed
        .iter()
        .filter(|path| is_feature(path) && path.starts_with(&hidden_dir))
        .cloned()
        .collect();
    let visible: Vec<String> = view
        .files(dir)?
        .into_iter()
        .filter(|path| is_feature(path) && !path.starts_with(&hidden_dir))
        .collect();
    let hidden = view.texts(&hidden)?;
    let visible = view.texts(&visible)?;
    Ok(phrasing::unknown(&hidden, &visible))
}

/// The files of a change as its branch has them: the working tree while the branch is checked
/// out, or when there is no branch to read; else the last commit of the branch.
#[derive(Debug)]
enum View<'a> {
    /// The working tree of the project at this root.
    Tree(&'a Path),
    /// This branch, through git.
    Branch(Git, String),
}

impl<'a> View<'a> {
    /// The view of change `id` of the project at `root`.
    fn of(root: &'a Path, id: &str) -> Result<View<'a>, Error> {
        let branch = change::branch(id);
        if !git::is_repository(root) {
            return Ok(View::Tree(root));
        }
        let git = Git::new(root);
        if !git.exists(&branch)? || git.current_branch()? == branch {
            return Ok(View::Tree(root));
        }
        Ok(View::Branch(git, branch))
    }

    /// The file `path`, from the project root; `None` when there is no file there.
    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, Error> {
        match self {
            View::Tree(root) => {
                let file = root.join(path);
                if !file.is_file() {
                    return Ok(None);
                }
                let bytes = fs::read(file).map_err(|e| unreadable(path, e))?;
                Ok(Some(bytes))
            }
            View::Branch(git, branch) => Ok(git.file_at(branch, path)?),
        }
    }

    /// The text of each of the files `paths` there is.
    fn texts(&self, paths: &[String]) -> Result<Vec<String>, Error> {
        let mut texts = Vec::new();
        for path in paths {
            if let Some(bytes) = self.read(path)? {
                texts.push(String::from_utf8_lossy(&bytes).into_owned());
            }
        }
        Ok(texts)
    }

    /// The files inside `folder`, from the project root.
    fn files(&self, folder: &str) -> Result<Vec<String>, Error> {
        match self {
            View::Tree(root) => {
                let mut found = Vec::new();
                walk(root, folder, &mut found).map_err(|e| unreadable(folder, e))?;
                Ok(found)
            }
            View::Branch(git, branch) => Ok(git.files_at(branch, folder)?),
        }
    }
}

/// Adds the files inside `folder` of the project at `root`, and inside its subfolders, to
/// `found`, as paths from the project root. No folder, no files.
fn walk(root: &Path, folder: &str, found: &mut Vec<String>) -> io::Result<()> {
    let entries = match fs::read_dir(root.join(folder)) {
        Ok(entries) => entries,
        Err(e) if matches!(e.kind(), NotFound | NotADirectory) => return Ok(()),
        Err(e) => return Err(e),
    };
    for entry in entries {
        let entry = entry?;
        let path = format!("{folder}/{}", entry.file_name().to_string_lossy());
        if entry.file_type()?.is_dir() {
            walk(root, &path, found)?;
        } else {
            found.push(path);
        }
    }
    Ok(())
}

/// Whether a human approved the red zones, and whether they changed since.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZonesState {
    /// Approved as they are.
    Approved,
    /// Never approved.
    NotApproved,
    /// Approved, then changed.
    Changed,
}

impl ZonesState {
    /// Whether the red zones are approved as they are.
    pub fn approved(self) -> bool {
        self == ZonesState::Approved
    }

    /// What it means, in one sentence.
    pub fn message(self) -> &'static str {
        match self {
            ZonesState::Approved => "red zones approved",
            ZonesState::NotApproved => NOT_APPROVED,
            ZonesState::Changed => CHANGED,
        }
    }
}

/// Whether a human approved the red zones of the zone map `zones`, of the project at `root`, as
/// they are.
pub fn zones_state(root: &Path, zones: &str) -> Result<ZonesState, Error> {
    let approval = saved(root, ZONES_PATH)?.filter(|a| a["decision"] == "ok");
    let Some(approval) = approval else {
        return Ok(ZonesState::NotApproved);
    };
    if approval["red_hash"] == red_hash(&red_zones(zones)?).as_str() {
        Ok(ZonesState::Approved)
    } else {
        Ok(ZonesState::Changed)
    }
}

/// Whether the machines opened G0 on the project at `root`.
pub fn g0_open(root: &Path) -> Result<bool, Error> {
    let g0 = saved(root, G0_PATH)?;
    Ok(g0.is_some_and(|g0| g0["decision"] == "ok"))
}

/// Whether the last harness self-test of the project at `root` passed.
fn self_test_passed(root: &Path) -> Result<bool, Error> {
    let test = saved(root, SELFTEST_PATH)?;
    Ok(test.is_some_and(|test| test["ok"] == true))
}

/// The red zones of the zone map `zones`: its `[[zone]]` tables whose color is `red`, in their
/// order, with every key they have. A zone map that is not valid is an error.
pub fn red_zones(zones: &str) -> Result<Vec<toml::Table>, Error> {
    #[derive(Deserialize)]
    struct Raw {
        #[serde(default)]
        zone: Vec<toml::Table>,
    }
    ZoneMap::from_toml(zones).map_err(Error::Zones)?;
    let raw: Raw =
        toml::from_str(zones).map_err(|e| Error::Zones(config::Error::Toml(e.to_string())))?;
    let red = toml::Value::from("red");
    let is_red = |zone: &toml::Table| zone.get("color") == Some(&red);
    Ok(raw.zone.into_iter().filter(is_red).collect())
}

/// The hash the approval of the red zones keeps: the first 16 hex characters of the SHA-256 of
/// `reds` as JSON, written as the bootstrap kit writes it (Python's `json.dumps(reds,
/// sort_keys=True)`), so both tools agree on it.
pub fn red_hash(reds: &[toml::Table]) -> String {
    let list = toml::Value::Array(reds.iter().cloned().map(toml::Value::Table).collect());
    let mut json = String::new();
    python_json(&list, &mut json);
    let mut hash = sha256(json.as_bytes());
    hash.truncate(16);
    hash
}

/// Writes `value` as JSON into `out` the way Python's `json.dumps(value, sort_keys=True)` does:
/// keys sorted, `", "` between items, `": "` after keys, and every character outside printable
/// ASCII escaped. A zone map holds no floats or dates: they are written as Rust and TOML write
/// them.
fn python_json(value: &toml::Value, out: &mut String) {
    match value {
        toml::Value::String(text) => python_string(text, out),
        toml::Value::Integer(n) => {
            let _ = write!(out, "{n}");
        }
        toml::Value::Float(x) => {
            let _ = write!(out, "{x:?}");
        }
        toml::Value::Boolean(b) => out.push_str(if *b { "true" } else { "false" }),
        toml::Value::Datetime(date) => python_string(&date.to_string(), out),
        toml::Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                python_json(item, out);
            }
            out.push(']');
        }
        toml::Value::Table(table) => {
            let mut entries: Vec<(&String, &toml::Value)> = table.iter().collect();
            entries.sort_by_key(|(key, _)| *key);
            out.push('{');
            for (i, (key, item)) in entries.into_iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                python_string(key, out);
                out.push_str(": ");
                python_json(item, out);
            }
            out.push('}');
        }
    }
}

/// Writes `text` into `out` as a JSON string the way Python's `json.dumps` does by default.
fn python_string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            ' '..='~' => out.push(c),
            _ => {
                for unit in c.encode_utf16(&mut [0; 2]).iter() {
                    let _ = write!(out, "\\u{unit:04x}");
                }
            }
        }
    }
    out.push('"');
}

/// The SHA-256 of `bytes`, in hex.
fn sha256(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(bytes).iter() {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// The JSON a file of the tool holds; `None` when there is no file or it is not JSON.
fn saved(root: &Path, path: &str) -> Result<Option<serde_json::Value>, Error> {
    match fs::read_to_string(root.join(path)) {
        Ok(text) => Ok(serde_json::from_str(&text).ok()),
        Err(e) if e.kind() == NotFound => Ok(None),
        Err(e) => Err(unreadable(path, e)),
    }
}

/// Writes `value` as indented JSON, with a line break at the end, to a file of the project.
fn write_json(root: &Path, path: &str, value: &impl Serialize) -> Result<(), Error> {
    match serde_json::to_string_pretty(value) {
        Ok(text) => write(root, path, &format!("{text}\n")),
        Err(e) => Err(Error::Project(format!("{path}: {e}"))),
    }
}

/// Writes `text` to a file of the project, creating its folder.
fn write(root: &Path, path: &str, text: &str) -> Result<(), Error> {
    let file = root.join(path);
    file.parent()
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| fs::write(&file, text))
        .map_err(|e| unwritable(path, e))
}

/// Logs the decision of `request`, recorded at `ts` on `change` (`-` for none) by `role`.
fn logged(
    root: &Path,
    ts: &str,
    change: &str,
    role: &str,
    request: &Request,
    decision: Decision,
) -> Result<(), Error> {
    let entry = log::Entry {
        change: change.to_string(),
        phase: request.gate.phase().to_string(),
        gate: request.gate.to_string(),
        role: role.to_string(),
        agent: "-".to_string(),
        result: decision.to_string(),
        duration_s: 0.0,
        note: request.note.to_string(),
    };
    match entry.line(ts) {
        Ok(line) => log::append(root, &line).map_err(|e| unwritable(log::PATH, e)),
        Err(e) => Err(Error::Project(format!("{}: {e}", log::PATH))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::{Folder, Repo};
    use std::path::PathBuf;

    const ZONES: &str = "default = \"yellow\"\n\
                         [[zone]]\ncolor = \"green\"\npaths = [\"docs/**\"]\n\
                         [[zone]]\ncolor = \"red\"\npaths = [\"src/storage/**\"]\n\
                         reason = \"Drafts\"\n";

    const RECORD: &str = concat!(
        r#"{"id": "c1", "title": "Drafts", "state": "open", "gates": {}, "inbox": [], "#,
        r#""paths": ["src/storage/db.rs", "features/drafts.feature"], "extra": 7}"#
    );

    const VISIBLE: &str = "Feature: Drafts\n  Scenario: One\n    Given a draft \"x\"\n    \
                           Then it is saved\n";

    /// A project with the record of change `c1`, and what recording a decision reads.
    struct Fixture {
        root: PathBuf,
        repo: Option<Repo>,
        _folder: Option<Folder>,
        settings: Settings,
        acceptance: Acceptance,
        zones: String,
    }

    /// The project in a git repository: `main` with one commit, and the branch of `c1`, which
    /// is not checked out.
    fn fixture() -> Fixture {
        let repo = Repo::new();
        repo.git(&["branch", "change/c1"]);
        repo.write(".gdd/changes/c1/change.json", RECORD);
        Fixture {
            root: repo.root().to_path_buf(),
            repo: Some(repo),
            _folder: None,
            settings: Settings::from_toml("").unwrap(),
            acceptance: Acceptance::default(),
            zones: ZONES.to_string(),
        }
    }

    /// The project without git.
    fn without_git() -> Fixture {
        let folder = Folder::new();
        folder.write(".gdd/changes/c1/change.json", RECORD);
        Fixture {
            root: folder.0.clone(),
            repo: None,
            _folder: Some(folder),
            settings: Settings::from_toml("").unwrap(),
            acceptance: Acceptance::default(),
            zones: ZONES.to_string(),
        }
    }

    impl Fixture {
        fn root(&self) -> &Path {
            &self.root
        }

        fn write(&self, path: &str, text: &str) {
            let file = self.root.join(path);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, text).unwrap();
        }

        fn git(&self, args: &[&str]) -> String {
            self.repo.as_ref().unwrap().git(args)
        }

        /// Commits `text` as `path` on the branch of `c1`, and goes back to `main`.
        fn commit_on_branch(&self, path: &str, text: &str) {
            self.git(&["checkout", "-q", "change/c1"]);
            self.write(path, text);
            self.git(&["add", "--", path]);
            self.git(&["commit", "-q", "-m", path]);
            self.git(&["checkout", "-q", "main"]);
        }

        fn project(&self) -> Project<'_> {
            Project {
                root: self.root(),
                zones: &self.zones,
                settings: &self.settings,
                acceptance: &self.acceptance,
            }
        }

        fn run_as(
            &self,
            gate: Gate,
            decision: &str,
            change: Option<&str>,
            by: &str,
            role: Option<&str>,
        ) -> Result<String, Error> {
            let request = Request {
                gate,
                decision,
                change,
                by,
                note: "Looks fine",
            };
            record(&self.project(), &request, role)
        }

        fn run(
            &self,
            gate: Gate,
            decision: &str,
            change: Option<&str>,
            by: &str,
        ) -> Result<String, Error> {
            self.run_as(gate, decision, change, by, None)
        }

        /// Records `gate` ok, by a human.
        fn ok(&self, gate: Gate, change: Option<&str>) {
            if let Err(e) = self.run(gate, "ok", change, "human") {
                panic!("not recorded: {e}");
            }
        }

        fn refused(&self, gate: Gate, decision: &str, change: Option<&str>, by: &str) -> String {
            match self.run(gate, decision, change, by) {
                Ok(done) => panic!("recorded: {done}"),
                Err(e) => e.to_string(),
            }
        }

        fn json(&self, path: &str) -> serde_json::Value {
            let text = fs::read_to_string(self.root.join(path)).unwrap();
            serde_json::from_str(&text).unwrap()
        }

        fn record_of(&self, id: &str) -> serde_json::Value {
            self.json(&change::record_path(id))
        }

        fn log(&self) -> Vec<log::Record> {
            let text = fs::read_to_string(self.root.join(log::PATH)).unwrap_or_default();
            log::parse(&text).unwrap()
        }
    }

    fn assert_logged(line: &log::Record, fields: [(&str, &str); 7]) {
        for (field, value) in fields {
            assert_eq!(line.text(field), value, "{field} in {line:?}");
        }
        assert!(line.text("ts").ends_with("+00:00"), "{line:?}");
        assert_eq!(line.tokens_in + line.tokens_out, 0);
        assert_eq!(line.duration_s, 0.0);
    }

    #[test]
    fn each_gate_has_its_name_and_its_phase() {
        assert_eq!(Gate::ALL.map(|gate| gate.to_string()), GATES);
        for gate in Gate::ALL {
            assert_eq!(gate.to_string().parse::<Gate>().unwrap(), gate);
        }
        let phases = [
            "0-setup",
            "1-discover",
            "1-discover",
            "1-discover",
            "2-build",
            "3-integrate",
            "4-release",
        ];
        assert_eq!(Gate::ALL.map(Gate::phase), phases);
        let unknown = "g1".parse::<Gate>().unwrap_err().to_string();
        let expected = "unknown gate \"g1\": use one of G0, zones, plan, G1, G2, G3, G4";
        assert_eq!(unknown, expected);
    }

    #[test]
    fn a_decision_is_ok_or_ko_and_approve_and_reject_say_the_same() {
        for (word, decision) in [
            ("ok", Decision::Ok),
            ("approve", Decision::Ok),
            ("ko", Decision::Ko),
            ("reject", Decision::Ko),
        ] {
            assert_eq!(Decision::parse(word).unwrap(), decision, "{word}");
        }
        assert_eq!(Decision::Ok.to_string(), "ok");
        assert_eq!(Decision::Ko.to_string(), "ko");
        for word in ["maybe", "OK", ""] {
            let err = Decision::parse(word).unwrap_err().to_string();
            assert_eq!(err, "The decision is ok or ko.", "{word:?}");
        }
    }

    #[test]
    fn builders_and_reviewers_record_nothing() {
        let project = fixture();
        let roles = [
            "builder-build",
            "builder-test",
            "builder",
            "reviewer",
            "reviewer-2",
        ];
        let requests = [
            (Gate::G4, Some("c1"), "human"),
            (Gate::Zones, None, "human"),
            (Gate::G0, None, "machines"),
        ];
        for role in roles {
            for (gate, change, by) in requests {
                let err = project.run_as(gate, "maybe", change, by, Some(role));
                assert_eq!(err.unwrap_err().to_string(), NOT_FOR_AGENTS, "{role}");
            }
        }
        assert_eq!(
            NOT_FOR_AGENTS,
            "A Builder or Reviewer cannot open or close gates."
        );
        assert_eq!(project.log().len(), 0);
        assert_eq!(project.record_of("c1")["gates"], serde_json::json!({}));
        for role in [None, Some("orchestrator"), Some(""), Some("human")] {
            let done = project.run_as(Gate::G4, "ok", Some("c1"), "human", role);
            assert_eq!(done.unwrap(), "G4 ok on c1 (human).", "{role:?}");
        }
    }

    #[test]
    fn g0_opens_only_by_the_machines_with_the_brief_and_a_passing_self_test() {
        let project = fixture();
        let open = |by: &str| project.refused(Gate::G0, "ok", None, by);
        assert_eq!(open("human"), G0_BY_MACHINES);
        let closed = project.refused(Gate::G0, "ko", None, "human");
        assert_eq!(closed, G0_BY_MACHINES);
        assert_eq!(open("machines"), G0_UNTESTED);
        for failed in [r#"{"ok": false}"#, r#"{"ok": "yes"}"#, "{not json"] {
            project.write(SELFTEST_PATH, failed);
            assert_eq!(open("machines"), G0_UNTESTED, "{failed}");
        }
        project.write(SELFTEST_PATH, r#"{"ok": true, "summary": "harness 4/4"}"#);
        assert_eq!(open("machines"), G0_NO_BRIEF);
        fs::create_dir_all(project.root().join(BRIEF)).unwrap();
        assert_eq!(open("machines"), G0_NO_BRIEF);
        assert!(!g0_open(project.root()).unwrap());
        assert_eq!(project.log().len(), 0);
        fs::remove_dir(project.root().join(BRIEF)).unwrap();
        project.write(BRIEF, "# Brief\n");
        let opened = project.run(Gate::G0, "approve", Some("c1"), "machines");
        assert_eq!(opened.unwrap(), "G0 ok (machines).");
        assert!(g0_open(project.root()).unwrap());
        let saved = project.json(G0_PATH);
        assert_eq!(saved["decision"], "ok");
        assert_eq!(saved["by"], "machines");
        assert_eq!(saved["note"], "Looks fine");
        assert_eq!(saved.as_object().unwrap().len(), 4);
        let log = project.log();
        assert_logged(
            &log[0],
            [
                ("change", "-"),
                ("phase", "0-setup"),
                ("gate", "G0"),
                ("role", "machine"),
                ("agent", "-"),
                ("result", "ok"),
                ("note", "Looks fine"),
            ],
        );
        assert_eq!(log[0].text("ts"), saved["ts"].as_str().unwrap());
    }

    #[test]
    fn the_messages_of_g0_are_the_ones_of_the_bootstrap_kit() {
        assert_eq!(
            G0_BY_MACHINES,
            "G0 is opened by the machines (--by machines): Brief written and harness 4/4."
        );
        assert_eq!(
            G0_UNTESTED,
            "G0 cannot open: the harness self-test is not 4/4 (bin/gdd selftest)."
        );
        assert_eq!(
            G0_NO_BRIEF,
            "G0 cannot open: docs/brief.md (Project Brief) is missing."
        );
    }

    #[test]
    fn g0_closes_by_the_machines_without_conditions() {
        let project = fixture();
        let closed = project.run(Gate::G0, "ko", None, "machines");
        assert_eq!(closed.unwrap(), "G0 ko (machines).");
        assert!(!g0_open(project.root()).unwrap());
        assert_eq!(project.json(G0_PATH)["decision"], "ko");
        assert_eq!(project.log()[0].text("result"), "ko");
    }

    #[test]
    fn the_red_zones_are_approved_by_a_human_with_their_hash() {
        let project = fixture();
        let by_machines = project.refused(Gate::Zones, "ok", None, "machines");
        assert_eq!(by_machines, ZONES_BY_HUMAN);
        assert_eq!(
            ZONES_BY_HUMAN,
            "Red zones are approved by a human (--by human)."
        );
        assert!(!project.root().join(ZONES_PATH).exists());
        let done = project.run(Gate::Zones, "ok", None, "human");
        assert_eq!(done.unwrap(), "Red zones ok (1 red zone(s)).");
        let saved = project.json(ZONES_PATH);
        assert_eq!(saved["decision"], "ok");
        assert_eq!(saved["by"], "human");
        assert_eq!(saved["note"], "Looks fine");
        assert_eq!(saved["red_hash"], red_hash(&red_zones(ZONES).unwrap()));
        let red = serde_json::json!([
            {"color": "red", "paths": ["src/storage/**"], "reason": "Drafts"}
        ]);
        assert_eq!(saved["red"], red);
        assert_eq!(saved.as_object().unwrap().len(), 6);
        let log = project.log();
        assert_logged(
            &log[0],
            [
                ("change", "-"),
                ("phase", "1-discover"),
                ("gate", "zones"),
                ("role", "human"),
                ("agent", "-"),
                ("result", "ok"),
                ("note", "Looks fine"),
            ],
        );
        assert_eq!(log[0].text("ts"), saved["ts"].as_str().unwrap());
        let rejected = project.run(Gate::Zones, "reject", Some("c1"), "human");
        assert_eq!(rejected.unwrap(), "Red zones ko (1 red zone(s)).");
        let line = project.log().pop().unwrap();
        assert_eq!(line.text("change"), "c1");
        assert_eq!(line.text("result"), "ko");
        let invalid = project.refused(Gate::Zones, "ok", Some("../c1"), "human");
        assert_eq!(invalid, change::check_id("../c1").unwrap_err());
    }

    #[test]
    fn the_red_zones_of_a_zone_map_that_is_not_valid_are_not_approved() {
        let mut project = fixture();
        project.zones = "default = \"blue\"\n".into();
        let err = project.refused(Gate::Zones, "ok", None, "human");
        let unknown = "unknown color \"blue\" (use green, yellow or red)";
        assert_eq!(err, format!("gdd/zones.toml: {unknown}"));
        project.zones = String::new();
        let done = project.run(Gate::Zones, "ok", None, "human");
        assert_eq!(done.unwrap(), "Red zones ok (0 red zone(s)).");
    }

    #[test]
    fn changing_a_red_zone_after_its_approval_asks_for_it_again() {
        let project = fixture();
        let state = |zones: &str| zones_state(project.root(), zones).unwrap();
        assert_eq!(state(ZONES), ZonesState::NotApproved);
        project.ok(Gate::Zones, None);
        assert_eq!(state(ZONES), ZonesState::Approved);
        // Only the red zones count.
        let greener = ZONES.replace("docs/**", "docs/**\", \"*.md");
        assert_eq!(state(&greener), ZonesState::Approved);
        let wider = ZONES.replace("src/storage/**", "src/storage/**\", \"src/sync/**");
        assert_eq!(state(&wider), ZonesState::Changed);
        let broken = zones_state(project.root(), "default = \"blue\"");
        assert!(matches!(broken, Err(Error::Zones(_))), "{broken:?}");
        let rejected = project.run(Gate::Zones, "ko", None, "human");
        assert!(rejected.is_ok(), "{rejected:?}");
        assert_eq!(state(ZONES), ZonesState::NotApproved);
        project.write(ZONES_PATH, "{not json");
        assert_eq!(state(ZONES), ZonesState::NotApproved);
    }

    #[test]
    fn the_state_of_the_red_zones_says_what_it_means() {
        assert!(ZonesState::Approved.approved());
        assert!(!ZonesState::NotApproved.approved());
        assert!(!ZonesState::Changed.approved());
        assert_eq!(ZonesState::Approved.message(), "red zones approved");
        assert_eq!(
            ZonesState::NotApproved.message(),
            "Red zones not approved yet: the Orchestrator proposes them at the end of Discover."
        );
        assert_eq!(
            ZonesState::Changed.message(),
            "Red zones changed after their approval: a human must approve them again."
        );
    }

    /// The zone map of this repository when the change that brought the hash started.
    const REPOSITORY_ZONES: &str = r#"
default = "yellow"

[[zone]]
color = "green"
paths = ["docs/**", "*.md"]
reason = "Documentation"

[[zone]]
color = "red"
paths = ["gdd/**", "gdd-kit/**", "bin/**", ".github/**", ".claude/**", ".specify/**", ".gdd/bin/**"]
reason = "The harness itself: touching the gates is touching the safety of the method"

[[zone]]
color = "red"
paths = ["Cargo.lock", "deny.toml", "rust-toolchain.toml"]
reason = "Dependencies and toolchain: supply chain"

[[zone]]
color = "red"
paths = ["crates/gdd-core/src/classify.rs", "crates/gdd-core/src/zones.rs", "crates/gdd-core/src/rules.rs", "crates/gdd-core/src/gates/**", "crates/gdd-core/src/guard.rs"]
reason = "Gate integrity: the color and the rules decide which gates run; gates and Path Guard decide what passes"

[[zone]]
color = "red"
paths = ["crates/gdd-core/src/git.rs", "crates/gdd-core/src/repo/**"]
reason = "The user's repository: branches, merges and commits"

[[zone]]
color = "red"
paths = ["crates/gdd-core/src/agents/**"]
reason = "Secrets and permissions when launching agents"

[[zone]]
color = "red"
paths = ["crates/gdd-core/src/exec.rs"]
reason = "Shell execution: agent text must never reach a command"

[[zone]]
color = "yellow"
paths = ["crates/gdd-core/src/log.rs", "crates/gdd-core/src/report.rs"]
reason = "The log: wrong numbers mislead, but nothing breaks and it can be rebuilt"

[[zone]]
color = "green"
paths = ["crates/gdd/src/tui/theme.rs", "crates/gdd/src/tui/text/**"]
reason = "Look and copy of the TUI"
"#;

    #[test]
    fn the_red_hash_is_the_one_of_the_bootstrap_kit() {
        let reds = red_zones(REPOSITORY_ZONES).unwrap();
        assert_eq!(reds.len(), 6);
        assert_eq!(red_hash(&reds), "bc48bf5c6ecb81d6");
        assert_eq!(red_hash(&[]), sha256(b"[]")[..16]);
    }

    #[test]
    fn red_zones_are_the_zones_that_say_red_with_every_key_they_have() {
        let zones = "default = \"red\"\n[[zone]]\npaths = [\"a/**\"]\n\
                     [[zone]]\ncolor = \"red\"\npaths = [\"b/**\"]\nowner = \"me\"\n\
                     [[zone]]\ncolor = \"yellow\"\npaths = [\"c/**\"]\n";
        let expected = "color = \"red\"\npaths = [\"b/**\"]\nowner = \"me\"\n";
        let expected: toml::Table = toml::from_str(expected).unwrap();
        assert_eq!(red_zones(zones).unwrap(), [expected]);
        let single = red_zones("[zone]\ncolor = \"red\"\n");
        assert!(matches!(single, Err(Error::Zones(_))), "{single:?}");
        assert!(matches!(red_zones("x = "), Err(Error::Zones(_))));
    }

    fn python(text: &str) -> String {
        let value = toml::Value::Table(toml::from_str(text).unwrap());
        let mut out = String::new();
        python_json(&value, &mut out);
        out
    }

    fn quoted(text: &str) -> String {
        let mut out = String::new();
        python_string(text, &mut out);
        out
    }

    /// `text` with each `%` replaced by a backslash and a `u`, as Python escapes a character.
    fn escaped(text: &str) -> String {
        text.replace('%', "\\u")
    }

    #[test]
    fn json_is_written_as_python_writes_it() {
        let mixed = "b = 1\na = true\nc = [1.5, false, -3]\nd = []\ne = {}\n\
                     [f]\nz = 1\ny = \"x\"\n";
        let expected = concat!(
            r#"{"a": true, "b": 1, "c": [1.5, false, -3], "d": [], "e": {}, "#,
            r#""f": {"y": "x", "z": 1}}"#
        );
        assert_eq!(python(mixed), expected);
        let date = python("\"é\" = 1979-05-27\n");
        assert_eq!(date, escaped(r#"{"%00e9": "1979-05-27"}"#));
        assert_eq!(quoted("q\" b\\ n\n r\r"), r#""q\" b\\ n\n r\r""#);
        assert_eq!(quoted("t\t b\u{8} f\u{c}"), r#""t\t b\b f\f""#);
        let control = quoted("u\u{1} d\u{7f} ~");
        assert_eq!(control, escaped(r#""u%0001 d%007f ~""#));
        assert_eq!(quoted("é 😀"), escaped(r#""%00e9 %d83d%de00""#));
    }

    #[test]
    fn decisions_on_a_change_are_written_in_its_record_and_logged() {
        let project = fixture();
        let done = project.run(Gate::Plan, "ok", Some("c1"), "human");
        assert_eq!(done.unwrap(), "plan ok on c1 (human).");
        let record = project.record_of("c1");
        let plan = &record["gates"]["plan"];
        assert_eq!(plan["decision"], "ok");
        assert_eq!(plan["by"], "human");
        assert_eq!(plan["note"], "Looks fine");
        assert_eq!(plan.as_object().unwrap().len(), 4);
        assert_eq!(record["extra"], 7);
        assert_eq!(record.get("features"), None);
        let log = project.log();
        assert_logged(
            &log[0],
            [
                ("change", "c1"),
                ("phase", "1-discover"),
                ("gate", "plan"),
                ("role", "human"),
                ("agent", "-"),
                ("result", "ok"),
                ("note", "Looks fine"),
            ],
        );
        assert_eq!(log[0].text("ts"), plan["ts"].as_str().unwrap());
        let later = [
            (Gate::G2, "2-build"),
            (Gate::G3, "3-integrate"),
            (Gate::G4, "4-release"),
        ];
        for (gate, phase) in later {
            let done = project.run(gate, "reject", Some("c1"), "machines");
            assert_eq!(done.unwrap(), format!("{gate} ko on c1 (machines)."));
            let record = project.record_of("c1");
            let entry = &record["gates"][gate.to_string()];
            assert_eq!(entry["decision"], "ko");
            assert_eq!(entry["by"], "machines");
            let line = project.log().pop().unwrap();
            assert_eq!(line.text("phase"), phase);
            assert_eq!(line.text("role"), "machines");
        }
        let record = project.record_of("c1");
        assert_eq!(record["gates"]["plan"]["decision"], "ok");
    }

    #[test]
    fn a_decision_on_a_change_needs_a_change_that_exists() {
        let project = fixture();
        let missing = project.refused(Gate::G4, "ok", None, "human");
        assert_eq!(missing, "Missing change id: G4 is recorded on a change.");
        let unknown = project.refused(Gate::Plan, "ok", Some("c9"), "human");
        let path = ".gdd/changes/c9/change.json";
        let expected = format!("change c9 does not exist: there is no {path}");
        assert_eq!(unknown, expected);
        let invalid = project.refused(Gate::G1, "ok", Some("C1"), "human");
        assert_eq!(invalid, change::check_id("C1").unwrap_err());
        project.write(".gdd/changes/c2/change.json", "{\"gates\": []}");
        let wrong = project.refused(Gate::Plan, "ok", Some("c2"), "human");
        let field = "the field \"gates\" of the record is not an object";
        assert_eq!(wrong, format!(".gdd/changes/c2/change.json: {field}"));
        project.write(".gdd/changes/c3/change.json", "{\"paths\": 3}");
        let broken = project.refused(Gate::G1, "ok", Some("c3"), "human");
        let start = ".gdd/changes/c3/change.json: invalid type";
        assert!(broken.starts_with(start), "{broken}");
        assert_eq!(project.log().len(), 0);
    }

    #[test]
    fn approving_g1_seals_the_scenarios_from_the_branch_of_the_change() {
        let project = fixture();
        project.commit_on_branch("features/drafts.feature", VISIBLE);
        assert!(!project.root().join("features/drafts.feature").exists());
        let done = project.run(Gate::G1, "ok", Some("c1"), "human");
        assert_eq!(done.unwrap(), "G1 ok on c1 (human).");
        let record = project.record_of("c1");
        let hash = sha256(VISIBLE.as_bytes());
        let hashes = serde_json::json!({"features/drafts.feature": hash});
        assert_eq!(record["gates"]["G1"]["hashes"], hashes);
        assert_eq!(record["gates"]["G1"]["decision"], "ok");
        let features = serde_json::json!(["features/drafts.feature"]);
        assert_eq!(record["features"], features);
        assert_eq!(record["extra"], 7);
        assert_eq!(project.log()[0].text("gate"), "G1");
    }

    #[test]
    fn on_its_branch_g1_seals_the_scenarios_as_the_working_tree_has_them() {
        let project = fixture();
        project.commit_on_branch("features/drafts.feature", VISIBLE);
        project.git(&["checkout", "-q", "change/c1"]);
        let edited = format!("{VISIBLE}    And it is listed\n");
        project.write("features/drafts.feature", &edited);
        project.write("features/new.feature", "Feature: New\n");
        project.ok(Gate::G1, Some("c1"));
        let hashes = serde_json::json!({
            "features/drafts.feature": sha256(edited.as_bytes()),
            "features/new.feature": sha256(b"Feature: New\n"),
        });
        assert_eq!(project.record_of("c1")["gates"]["G1"]["hashes"], hashes);
    }

    #[test]
    fn rejecting_g1_seals_nothing() {
        let project = fixture();
        project.commit_on_branch("features/drafts.feature", VISIBLE);
        let hidden = "Given a brand new sentence\n";
        project.commit_on_branch("features/hidden/drafts.feature", hidden);
        let done = project.run(Gate::G1, "ko", Some("c1"), "human");
        assert_eq!(done.unwrap(), "G1 ko on c1 (human).");
        let record = project.record_of("c1");
        assert_eq!(record["gates"]["G1"]["decision"], "ko");
        assert_eq!(record["gates"]["G1"].get("hashes"), None);
        assert_eq!(record.get("features"), None);
    }

    #[test]
    fn a_change_that_touched_nothing_seals_the_scenarios_it_plans_or_its_record_names() {
        let project = fixture();
        let planned = concat!(
            r#"{"paths": ["features/b.feature", "src/a.rs", "features/b.feature", "#,
            r#""features/missing.feature"]}"#
        );
        project.write(".gdd/changes/c2/change.json", planned);
        project.write("features/b.feature", "Feature: B\n");
        project.ok(Gate::G1, Some("c2"));
        let record = project.record_of("c2");
        let features = serde_json::json!(["features/b.feature", "features/missing.feature"]);
        assert_eq!(record["features"], features);
        let hashes = serde_json::json!({"features/b.feature": sha256(b"Feature: B\n")});
        assert_eq!(record["gates"]["G1"]["hashes"], hashes);
        let named = r#"{"paths": ["src/a.rs"], "features": ["features/hidden/x.feature"]}"#;
        project.write(".gdd/changes/c3/change.json", named);
        project.write("features/hidden/x.feature", "Given a draft \"y\"\n");
        project.ok(Gate::G1, Some("c3"));
        let record = project.record_of("c3");
        let features = serde_json::json!(["features/hidden/x.feature"]);
        assert_eq!(record["features"], features);
        let hash = sha256(b"Given a draft \"y\"\n");
        let hashes = serde_json::json!({"features/hidden/x.feature": hash});
        assert_eq!(record["gates"]["G1"]["hashes"], hashes);
    }

    #[test]
    fn without_git_g1_seals_the_working_tree() {
        let project = without_git();
        project.write("features/drafts.feature", VISIBLE);
        project.ok(Gate::G1, Some("c1"));
        let hash = sha256(VISIBLE.as_bytes());
        let hashes = serde_json::json!({"features/drafts.feature": hash});
        assert_eq!(project.record_of("c1")["gates"]["G1"]["hashes"], hashes);
    }

    #[test]
    fn g1_refuses_hidden_examples_with_sentences_no_visible_scenario_has() {
        let project = fixture();
        project.commit_on_branch("features/drafts.feature", VISIBLE);
        let hidden = "Feature: Hidden\n  Scenario: Other\n    Given a draft \"z\"\n    \
                      Given a brand new sentence\n";
        project.commit_on_branch("features/hidden/drafts.feature", hidden);
        let err = project.refused(Gate::G1, "ok", Some("c1"), "human");
        let expected = format!("{UNKNOWN_SENTENCES}\n  - a brand new sentence");
        assert_eq!(err, expected);
        assert_eq!(
            UNKNOWN_SENTENCES,
            "G1 refused: hidden examples use step phrasings that no visible scenario has, \
             so test mode can never write them. Reuse visible sentences with new data:"
        );
        assert_eq!(project.record_of("c1")["gates"], serde_json::json!({}));
        assert_eq!(project.log().len(), 0);
        let mut many = String::new();
        for c in 'a'..='l' {
            many.push_str(&format!("Given step {c}\n"));
        }
        project.commit_on_branch("features/hidden/drafts.feature", &many);
        let err = project.refused(Gate::G1, "ok", Some("c1"), "human");
        assert_eq!(err.matches("\n  - ").count(), 10, "{err}");
        assert!(err.ends_with("\n  - step j"), "{err}");
    }

    #[test]
    fn hidden_examples_that_reuse_visible_sentences_are_sealed_with_the_rest() {
        let project = fixture();
        project.commit_on_branch("features/drafts.feature", VISIBLE);
        let hidden = "Feature: Hidden\n  Scenario: Other\n    Given a draft \"z\"\n    \
                      Then it is saved\n";
        project.commit_on_branch("features/hidden/drafts.feature", hidden);
        // Older hidden examples were approved with their own changes.
        project.write("features/hidden/old.feature", "Given an old sentence\n");
        project.git(&["add", "--", "features/hidden/old.feature"]);
        project.git(&["commit", "-q", "-m", "old"]);
        project.ok(Gate::G1, Some("c1"));
        let record = project.record_of("c1");
        assert_eq!(record["features"][0], "features/drafts.feature");
        assert_eq!(record["features"][1], "features/hidden/drafts.feature");
        assert_eq!(record["features"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn only_scenario_files_inside_the_hidden_folder_are_hidden_examples() {
        let project = fixture();
        project.commit_on_branch("features/drafts.feature", VISIBLE);
        let new = "Given a brand new sentence\n";
        project.commit_on_branch("features/hidden/notes.md", new);
        project.commit_on_branch("docs/drafts.feature", new);
        project.ok(Gate::G1, Some("c1"));
        let features = serde_json::json!(["features/drafts.feature"]);
        assert_eq!(project.record_of("c1")["features"], features);
    }

    #[test]
    fn on_its_branch_the_hidden_examples_are_checked_in_the_working_tree() {
        let project = fixture();
        project.git(&["checkout", "-q", "change/c1"]);
        project.write("features/drafts.feature", VISIBLE);
        let step = "Given one more visible step\n";
        project.write("features/deep/more.feature", step);
        project.write("features/hidden/a.feature", step);
        project.ok(Gate::G1, Some("c1"));
        let new = "Given a brand new sentence\n";
        project.write("features/hidden/a.feature", new);
        let err = project.refused(Gate::G1, "ok", Some("c1"), "human");
        assert!(err.ends_with("\n  - a brand new sentence"), "{err}");
    }

    #[test]
    fn the_files_of_a_folder_in_the_working_tree() {
        let folder = Folder::new();
        folder.write("features/a.feature", "a");
        folder.write("features/deep/er/b.feature", "b");
        folder.write("other.feature", "o");
        let mut found = Vec::new();
        walk(&folder.0, "features", &mut found).unwrap();
        found.sort();
        let expected = ["features/a.feature", "features/deep/er/b.feature"];
        assert_eq!(found, expected);
        for nothing in ["specs", "other.feature"] {
            let mut found = Vec::new();
            walk(&folder.0, nothing, &mut found).unwrap();
            assert_eq!(found, Vec::<String>::new(), "{nothing}");
        }
        let unreadable = walk(&folder.0, &"x".repeat(300), &mut found);
        assert!(unreadable.is_err(), "{unreadable:?}");
    }

    #[test]
    fn files_that_cannot_be_written_or_read_are_errors_that_name_them() {
        let project = fixture();
        project.write(".gdd/log.jsonl/x", "");
        let err = project.refused(Gate::Zones, "ok", None, "human");
        let start = "cannot write .gdd/log.jsonl: ";
        assert!(err.starts_with(start), "{err}");
        fs::create_dir_all(project.root().join(G0_PATH)).unwrap();
        let err = project.refused(Gate::G0, "ko", None, "machines");
        let start = "cannot write .gdd/g0.json: ";
        assert!(err.starts_with(start), "{err}");
        let err = g0_open(project.root()).unwrap_err().to_string();
        assert!(err.starts_with("cannot read .gdd/g0.json: "), "{err}");
        fs::remove_file(project.root().join(ZONES_PATH)).unwrap();
        fs::create_dir_all(project.root().join(ZONES_PATH)).unwrap();
        let state = zones_state(project.root(), ZONES);
        let err = state.unwrap_err().to_string();
        assert!(err.starts_with("cannot read .gdd/zones.json: "), "{err}");
    }

    #[test]
    fn errors_say_what_went_wrong() {
        assert_eq!(Error::Refused("no".into()).to_string(), "no");
        let project = Error::Project("cannot read x".into());
        assert_eq!(project.to_string(), "cannot read x");
        let git = Error::from(git::Error::Revision("-x".into()));
        assert_eq!(git.to_string(), "\"-x\" is not a revision git can take");
        let zones = Error::Zones(config::Error::Toml("bad".into()));
        assert_eq!(zones.to_string(), "gdd/zones.toml: invalid TOML: bad");
    }
}
