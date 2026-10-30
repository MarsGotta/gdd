//! Merging a change (`gdd merge`): once G3 is open, the last work of the change is committed on
//! its branch, where it is checked out, and the branch merges into the main branch, in the main
//! checkout, with a merge commit that carries the record of the change: what a pull request
//! description would hold, and its line in the changelog as trailers (see [`changelog`]). The
//! full working files stay in `.gdd/changes/<id>/`, which git does not keep. The known debt the
//! change paid goes off the list in that same merge commit, and the counts it lowered go down
//! there. Then main is pushed when the rules
//! ask for it and there is a remote, and the artifact is built and deployed to pre from the main
//! checkout: as the main branch configures them once the change merged into it, not as the
//! branch of the change had them.
//!
//! A spike never merges: once the human approves it, only its ADR reaches main, in a commit of
//! its own (see [`adr_only`]).

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::agents::files::{self, Found};
use crate::change;
use crate::changelog;
use crate::classify::Classification;
use crate::config::{self, Color};
use crate::gates::release::{self, Passed, Steps};
use crate::gates::spike::{self, ADR_FOLDER};
use crate::gates::{self, Gate, approve, debt};
use crate::git::Git;
use crate::guard;
use crate::log;
use crate::rules;

/// Why a change without G3 does not merge.
const G3_CLOSED: &str = "G3 is not open.";

/// Characters the message keeps of a line it quotes, at most.
const LINE_LENGTH: usize = 220;

/// Decisions taken without the human the message lists: the last ones.
const DECIDED: usize = 8;

/// Agents of the log lines that are no agent calls: their tokens do not count.
const NOT_AGENTS: [&str; 9] = [
    "-",
    "checks",
    "path-guard",
    "merge",
    "verdict",
    "build",
    "deploy_pre",
    "tests_pre",
    "demo",
];

/// What merging a change reads from the project.
#[derive(Debug, Clone, Copy)]
pub struct Project<'a> {
    /// The project root, where the branch of the change is checked out: the main checkout or the
    /// worktree of the change.
    pub root: &'a Path,
    /// The state root, whose `.gdd/` holds the record of the change (see
    /// [`crate::state::root`]): the project root in the main checkout, where the change merges.
    pub state: &'a Path,
    /// The branch the change merges into.
    pub main: &'a str,
}

/// Whether a rules file asks to push the main branch after the Merge Gate: `push_main`; no when
/// it says nothing. The rest of the file is ignored.
fn push_main(rules: &str) -> Result<bool, config::Error> {
    #[derive(Default, Deserialize)]
    #[serde(default)]
    struct Raw {
        push_main: bool,
    }
    let raw: Raw = toml::from_str(rules).map_err(|e| config::Error::Toml(e.to_string()))?;
    Ok(raw.push_main)
}

/// Merges change `id`, classified as `classification`, into the main branch (see the module).
/// It needs G3 open and the branch of the change checked out at the project root, where its last
/// work is. In a worktree, the main checkout must have the main branch checked out and no
/// uncommitted changes in tracked files (see [`not_ready`]); the main checkout itself on the
/// branch of the change goes to the main branch. The known debt its latest G2 and G3 named as
/// paid goes off `gdd/debt.toml` in the merge commit, and each count they saved below the one
/// recorded lowers it (see [`debt::lowered`] and [`debt::pay`]); when it cannot, the merge is
/// given up. The record goes to state `merged` with the merge commit, and the merge is
/// logged; then the rules file and the release commands are read as the merge left them, and a
/// configuration that cannot be read, or a failed push, build or deploy, is an error after that.
/// What it prints, as JSON on one line: `ok`, whether the deploy to pre was simulated, and that
/// step.
pub fn merge(
    project: &Project,
    id: &str,
    classification: &Classification,
) -> Result<String, String> {
    let (root, state) = (project.root, project.state);
    let (text, record) = release::read(state, id)?;
    ready(project, id, &record)?;
    let work = Git::new(root);
    let branch = change::branch(id);
    let said = |e: crate::git::Error| e.to_string();
    let in_worktree = root != state;
    let reported = debt::reported(state, id)?;
    let (paid, lowered) = (reported.paid, debt::lowered(state, &reported.owed)?);
    let names = debt::by_risk(paid.iter().map(|(_, name)| name.as_str()));
    let message = message(state, id, &record, classification, &names, &lowered)?;
    if work.has_changes().map_err(said)? {
        let work_done = format!("gdd({id}): change work");
        work.commit_all(&work_done).map_err(said)?;
    }
    // The main checkout, where the change merges and what follows runs.
    let git = Git::new(state);
    if !in_worktree {
        git.checkout(project.main).map_err(said)?;
    }
    if paid.is_empty() && lowered.is_empty() {
        git.merge(&branch, &message).map_err(said)?;
    } else {
        merge_paying(&git, state, &branch, &message, &paid, &lowered)?;
    }
    let commit = git.head().map_err(said)?;
    let path = change::record_path(id);
    let fields = vec![
        ("state", Value::from("merged")),
        ("commit", Value::from(commit.as_str())),
    ];
    let merged = change::with_fields(&text, fields).map_err(|e| format!("{path}: {e}"))?;
    change::write(state, &path, &merged)?;
    let entry = log::Entry {
        change: id.to_string(),
        phase: "3-integrate".to_string(),
        gate: "G3".to_string(),
        role: "machine".to_string(),
        agent: "merge".to_string(),
        result: "ok".to_string(),
        duration_s: 0.0,
        note: commit.chars().take(12).collect(),
    };
    change::logged(state, &entry)?;
    let (push, steps) = configured(state)?;
    if push && git.has_remote().map_err(said)? {
        let main = project.main;
        if let Err(e) = git.push(main) {
            return Err(format!("Merged into {main}, but the push failed: {e}"));
        }
    }
    let build = release::step(state, state, id, &steps, "build")?;
    if !build.ok {
        let why = build.failure();
        return Err(format!("Could not build the artifact: {why}"));
    }
    let pre = release::step(state, state, id, &steps, "deploy_pre")?;
    if !pre.ok {
        let why = pre.failure();
        return Err(format!("Could not deploy to pre: {why}"));
    }
    Passed::detail(&pre, Some(pre.simulated)).json()
}

/// `Ok` when change `id` of `project` may merge (see [`merge`]); else why not.
pub fn may_merge(project: &Project, id: &str) -> Result<(), String> {
    let (_, record) = release::read(project.state, id)?;
    ready(project, id, &record)
}

/// `Ok` when change `id` of `project`, whose record is `record`, may merge: G3 open, the branch
/// of the change checked out at the project root and, in a worktree, a main checkout that can
/// take the merge (see [`not_ready`]). Else why not.
fn ready(project: &Project, id: &str, record: &Value) -> Result<(), String> {
    if record["gates"]["G3"]["decision"] != "ok" {
        return Err(G3_CLOSED.to_string());
    }
    let branch = change::branch(id);
    let said = |e: crate::git::Error| e.to_string();
    if Git::new(project.root).current_branch().map_err(said)? != branch {
        return Err(format!(
            "Not on branch {branch}: merge runs where the last work of the change is."
        ));
    }
    let in_worktree = project.root != project.state;
    if in_worktree && let Some(why) = not_ready(project.state, project.main).map_err(said)? {
        return Err(why);
    }
    Ok(())
}

/// Why the main checkout at `checkout` cannot take a merge into the main branch `main`, if it
/// cannot: it is on another branch, or it has uncommitted changes in tracked files.
pub fn not_ready(checkout: &Path, main: &str) -> Result<Option<String>, crate::git::Error> {
    let git = Git::new(checkout);
    let shown = checkout.display();
    let current = git.current_branch()?;
    if current != main {
        return Ok(Some(format!(
            "the merge needs the main checkout ({shown}) on {main}, and it is on {current}: \
             switch it back to {main}"
        )));
    }
    if git.has_tracked_changes()? {
        return Ok(Some(format!(
            "the merge needs the main checkout ({shown}) without uncommitted changes in tracked \
             files: commit or stash them"
        )));
    }
    Ok(None)
}

/// The files under [`ADR_FOLDER`] the branch `branch` of a spike, or of one of its variants,
/// added or changed since it left the main branch `main`, from the project root of `git`, each
/// with what it holds on that branch, in git's order. What the branch removed, and its links, are
/// left out: only files reach main.
fn adrs(git: &Git, main: &str, branch: &str) -> Result<Vec<(String, Vec<u8>)>, crate::git::Error> {
    let mut adrs = Vec::new();
    for path in git.committed_from(main, branch)? {
        if !path.starts_with(ADR_FOLDER) {
            continue;
        }
        if let Some(bytes) = git.regular_file_at(branch, &path)? {
            adrs.push((path, bytes));
        }
    }
    Ok(adrs)
}

/// An ADR file of a spike on its way to the main checkout.
struct Adr {
    /// Its path, from the project root.
    path: String,
    /// What the branch of the spike holds in it.
    bytes: Vec<u8>,
    /// What the main checkout held at its path before: `None` when nothing was there.
    held: Option<Found>,
}

/// What OK at the G4 of a spike brings to the main branch `main` of the project at `root` from
/// `branch`, its own or the one of a variant: the paths of its ADR files (see [`adrs`]).
pub fn adr_files(root: &Path, main: &str, branch: &str) -> Result<Vec<String>, crate::git::Error> {
    let adrs = adrs(&Git::new(root), main, branch)?;
    Ok(adrs.into_iter().map(|(path, _)| path).collect())
}

/// Where the ADR files OK at the G4 of a spike brings to main come from, and what they get there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Adrs<'a> {
    /// The branch they come from: the spike's, or the one of the variant the human chose.
    pub branch: &'a str,
    /// What each ADR document gets at its end on main, after a blank line (see
    /// [`change::variants_section`]): nothing when empty. Other files get nothing.
    pub appended: &'a str,
}

/// OK at the G4 of spike `id`, titled `title`: its ADR files that main lacks, `from` its branch or
/// the one of the variant the human chose, with what `from` appends (see [`to_bring`]), written
/// in the main checkout `main_checkout`, and committed alone on the main branch `main`, with the
/// message `gdd(<id>): Spike ADR · <title>`: no changelog trailer, as spikes are not in the
/// changelog. The main checkout must be able to take a merge (see [`not_ready`]); with nothing to
/// bring, there is no commit. When a file cannot be written or the commit fails, the main
/// checkout gets back what it held (see [`put_back`]). The paths it committed.
pub fn adr_only(
    main_checkout: &Path,
    main: &str,
    id: &str,
    title: &str,
    from: &Adrs,
) -> Result<Vec<String>, String> {
    let said = |e: crate::git::Error| e.to_string();
    if let Some(why) = not_ready(main_checkout, main).map_err(said)? {
        return Err(why);
    }
    let git = Git::new(main_checkout);
    let adrs = to_bring(&git, main_checkout, main, from)?;
    if adrs.is_empty() {
        return Ok(Vec::new());
    }
    let paths: Vec<String> = adrs.iter().map(|adr| adr.path.clone()).collect();
    let written = adrs.iter().try_for_each(|adr| {
        let written = files::replace(main_checkout, &adr.path, &adr.bytes);
        written.map_err(|e| format!("cannot write {}: {e}", adr.path))
    });
    let message = format!("gdd({id}): Spike ADR · {title}");
    let committed = written.and_then(|()| git.commit_files(&paths, &message).map_err(said));
    committed.map_err(|e| put_back(&git, main_checkout, &adrs, &e))?;
    Ok(paths)
}

/// The ADR files on the branch of `from` (see [`adrs`]), each ADR document with what `from`
/// appends to it (see [`appended`]) and every other file, an image say, as the branch has it (see
/// [`spike::is_adr`]), that the main checkout at `root`, on the main branch `main` of `git`,
/// lacks: main holds none of them as they would be. Each with what the main checkout holds at
/// its path, to put it back. Before anything is written, what would be lost or written through is
/// an error: a link, or what is not a folder, on the way to a file, or what is not a file of its
/// own at its path (see [`files::found`]); or a file git does not track there.
fn to_bring(git: &Git, root: &Path, main: &str, from: &Adrs) -> Result<Vec<Adr>, String> {
    let said = |e: crate::git::Error| e.to_string();
    let shown = root.display();
    let mut brought = Vec::new();
    for (path, bytes) in adrs(git, main, from.branch).map_err(said)? {
        let bytes = if spike::is_adr(&path) {
            appended(bytes, from.appended)
        } else {
            bytes
        };
        let tracked = git.file_at(main, &path).map_err(said)?;
        if tracked.as_ref() == Some(&bytes) {
            continue;
        }
        let held = files::found(root, &path).map_err(|e| {
            format!(
                "cannot write {path} in the main checkout ({shown}): {e}. gdd writes neither \
                 through a link nor over what is not a file: move it away, then answer again"
            )
        })?;
        if held.is_some() && tracked.is_none() {
            return Err(format!(
                "{path} is in the main checkout ({shown}) and git does not track it: move it \
                 away or commit it, then answer again"
            ));
        }
        brought.push(Adr { path, bytes, held });
    }
    Ok(brought)
}

/// The ADR file `bytes` with `text` at its end, after a blank line, its last line ended first;
/// as it is when `text` is empty.
fn appended(mut bytes: Vec<u8>, text: &str) -> Vec<u8> {
    if text.is_empty() {
        return bytes;
    }
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        bytes.push(b'\n');
    }
    bytes.push(b'\n');
    bytes.extend_from_slice(text.as_bytes());
    bytes
}

/// What the human reads when bringing `adrs` to the main checkout of `git`, at `root`, failed
/// with `failure`, once the main checkout holds again what it held: each file as it was, or none
/// where there was none, and none of them in the index. When that fails too, it says so.
fn put_back(git: &Git, root: &Path, adrs: &[Adr], failure: &str) -> String {
    let restored = adrs.iter().try_for_each(|adr| match &adr.held {
        Some(held) => files::put_back(root, &adr.path, held),
        None => files::remove(root, &adr.path),
    });
    let paths: Vec<String> = adrs.iter().map(|adr| adr.path.clone()).collect();
    let index = |()| git.unstage(&paths).map_err(|e| e.to_string());
    match restored.map_err(|e| e.to_string()).and_then(index) {
        Ok(()) => format!("{failure}. The ADR files were taken back out: main is as it was."),
        Err(e) => format!("{failure}. Putting the main checkout back failed too: {e}"),
    }
}

/// Merges `branch` into the current branch with a merge commit whose message is `message`, and
/// settles the known debt of the project at `root` in that same commit: takes what was `paid`,
/// each with its gate, off the list, and lowers the counts `lowered` names (see [`debt::pay`]).
/// When the list cannot be written or the commit fails, the merge is given up and the current
/// branch stays as it was.
fn merge_paying(
    git: &Git,
    root: &Path,
    branch: &str,
    message: &str,
    paid: &[(Gate, String)],
    lowered: &[debt::Lowered],
) -> Result<(), String> {
    let said = |e: crate::git::Error| e.to_string();
    git.merge_uncommitted(branch).map_err(said)?;
    let settled = debt::pay(root, paid, lowered);
    let committed = settled.and_then(|()| git.commit_all(message).map_err(said));
    committed.map_err(|e| match git.abort_merge() {
        Ok(()) => format!("{e}. The merge was given up: main is as it was."),
        Err(abort) => format!("{e}. Giving up the merge failed too: {abort}"),
    })
}

/// Whether to push the main branch, and the release commands with the time limit of the rules
/// (see [`rules::command_seconds`]), as the project at `root` has them in its working tree:
/// after the merge, the main branch's. A missing file says nothing.
fn configured(root: &Path) -> Result<(bool, Steps), String> {
    let rules = configuration(root, guard::RULES)?;
    let in_rules = |e: config::Error| format!("{}: {e}", guard::RULES);
    let push = push_main(&rules).map_err(in_rules)?;
    let limit = rules::command_seconds(&rules).map_err(in_rules)?;
    let commands = configuration(root, gates::COMMANDS)?;
    let in_commands = |e: config::Error| format!("{}: {e}", gates::COMMANDS);
    let steps = Steps::from_toml(&commands, limit).map_err(in_commands)?;
    Ok((push, steps))
}

/// The text of the configuration file `path` of the project at `root`: empty when there is none.
fn configuration(root: &Path, path: &str) -> Result<String, String> {
    match fs::read_to_string(root.join(path)) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(format!("cannot read {path}: {e}")),
    }
}

/// The message of the merge commit of change `id` of the project whose state root is `state`,
/// whose record is `record` and whose classification is `classification`: the title, then the
/// color, lane and kind of work; the approvals; the verdicts of the Reviewer and its points; the
/// decisions taken without the human (the last ones); the tokens of its agent calls; the known
/// debt it paid, `paid`, and the counts of known debt it lowered, `lowered`, when there are; and
/// where its record lives. Then, in a paragraph of their own, the trailers of its line in the
/// changelog, when it has one (see [`changelog::of_change`]).
pub fn message(
    state: &Path,
    id: &str,
    record: &Value,
    classification: &Classification,
    paid: &[&str],
    lowered: &[debt::Lowered],
) -> Result<String, String> {
    let title = record["title"].as_str().unwrap_or_default();
    let kind = record["type"].as_str().unwrap_or("-");
    let (color, lane) = (classification.color, classification.lane);
    let mut lines = vec![
        format!("gdd({id}): G3 Merge Gate · {title}"),
        String::new(),
        format!("Color: {color} · lane {lane} · type {kind}"),
    ];
    let approvals = approvals(state, record, color)?;
    if approvals.is_empty() {
        lines.push("Approvals: none needed".to_string());
    } else {
        lines.push(format!("Approvals: {}", approvals.join("; ")));
    }
    lines.extend(reviews(state, id)?);
    let none = Vec::new();
    let notes = record["inbox"].as_array().unwrap_or(&none);
    let decided: Vec<&str> = notes
        .iter()
        .filter(|note| note["kind"] == "decided")
        .filter_map(|note| note["text"].as_str())
        .collect();
    if !decided.is_empty() {
        lines.push("Decided without the human:".to_string());
        for text in &decided[decided.len().saturating_sub(DECIDED)..] {
            lines.push(format!("  - {}", clipped(&text.replace('\n', " "))));
        }
    }
    lines.extend(tokens(state, id)?);
    if !paid.is_empty() {
        lines.push(format!("Known debt paid: {}", paid.join(", ")));
    }
    if !lowered.is_empty() {
        let lowered: Vec<String> = lowered.iter().map(debt::Lowered::to_string).collect();
        lines.push(format!("Known debt lowered: {}", lowered.join(", ")));
    }
    lines.push(format!("Record: .gdd/changes/{id}/ (local)"));
    let path = change::record_path(id);
    let line = changelog::of_change(record).map_err(|e| format!("{path}: {e}"))?;
    if let Some(line) = line {
        lines.push(String::new());
        lines.push(line.trailers());
    }
    Ok(lines.join("\n"))
}

/// The approvals the message names: the red zones for a red change, then G1 and the plan when
/// the record has them.
fn approvals(state: &Path, record: &Value, color: Color) -> Result<Vec<String>, String> {
    let mut approvals = Vec::new();
    let zones = match color {
        Color::Red => saved(state, approve::ZONES_PATH)?,
        _ => None,
    };
    if let Some(zones) = zones {
        approvals.push(format!("red zones {}", approval(&zones, "")));
    }
    for gate in ["G1", "plan"] {
        let decided = &record["gates"][gate];
        if decided.as_object().map_or(0, Map::len) == 0 {
            continue;
        }
        let sealed = decided["hashes"].as_object().map_or(0, Map::len);
        let extra = match sealed {
            0 => String::new(),
            files => format!(", {files} scenario file(s) sealed"),
        };
        approvals.push(format!("{gate} {}", approval(decided, &extra)));
    }
    Ok(approvals)
}

/// An approval in one line: `<decision> (<by>, <day><extra>)`.
fn approval(decided: &Value, extra: &str) -> String {
    let text = |key: &str, missing: &str| decided[key].as_str().unwrap_or(missing).to_string();
    let day: String = text("ts", "").chars().take(10).collect();
    let (decision, by) = (text("decision", "-"), text("by", "-"));
    format!("{decision} ({by}, {day}{extra})")
}

/// The JSON object a file of the tool under the state root `state` holds: `None` when there is no
/// file, or it holds no object or an empty one.
fn saved(state: &Path, path: &str) -> Result<Option<Value>, String> {
    let text = match fs::read_to_string(state.join(path)) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("cannot read {path}: {e}")),
    };
    let fields: Map<String, Value> = serde_json::from_str(&text).unwrap_or_default();
    Ok((!fields.is_empty()).then_some(Value::Object(fields)))
}

/// The lines of the Reviewer: its verdicts, in the order of its outputs
/// (`.gdd/changes/<id>/outputs/reviewer-*.md`), then the points of each output (its lines that
/// start with `- **`), as plain text. Nothing without outputs.
fn reviews(state: &Path, id: &str) -> Result<Vec<String>, String> {
    let folder = format!(".gdd/changes/{id}/outputs");
    let entries = match fs::read_dir(state.join(&folder)) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("cannot read {folder}: {e}")),
    };
    let mut names: Vec<String> = entries
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("reviewer-") && name.ends_with(".md"))
        .collect();
    names.sort();
    let mut outputs = Vec::new();
    for name in names {
        let path = format!("{folder}/{name}");
        let bytes = fs::read(state.join(&path)).map_err(|e| format!("cannot read {path}: {e}"))?;
        outputs.push(String::from_utf8_lossy(&bytes).into_owned());
    }
    if outputs.is_empty() {
        return Ok(Vec::new());
    }
    let verdicts: Vec<&str> = outputs.iter().map(String::as_str).map(verdict).collect();
    let mut lines = vec![format!("Reviewer: {}", verdicts.join(" -> "))];
    for output in &outputs {
        for line in output.lines().filter(|line| line.starts_with("- **")) {
            let point = unlinked(&line[2..]).replace("**", "");
            lines.push(format!("  - {}", clipped(&point)));
        }
    }
    Ok(lines)
}

/// The verdict of an output of the Reviewer: the `OK` or `KO` after the first `VERDICT:` that
/// has one; `?` when none has.
fn verdict(output: &str) -> &'static str {
    output
        .match_indices("VERDICT:")
        .find_map(|(at, mark)| {
            let rest = output[at + mark.len()..].trim_start();
            ["OK", "KO"].into_iter().find(|word| rest.starts_with(word))
        })
        .unwrap_or("?")
}

/// `text` with each Markdown link, `[label](target)` with a label, replaced by its label.
fn unlinked(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let close = after.find(']').filter(|&close| close > 0);
        let link = close.and_then(|close| {
            let target = after[close + 1..].strip_prefix('(')?;
            let end = target.find(')')?;
            Some((&after[..close], &target[end + 1..]))
        });
        match link {
            Some((label, tail)) => {
                out.push_str(label);
                rest = tail;
            }
            None => {
                out.push('[');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The tokens of the agent calls of change `id` in the log under the state root `state`, in one
/// line: nothing when there are none.
fn tokens(state: &Path, id: &str) -> Result<Option<String>, String> {
    let text = match fs::read_to_string(state.join(log::PATH)) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("cannot read {}: {e}", log::PATH)),
    };
    let records = log::parse(&text).map_err(|e| format!("{}, {e}", log::PATH))?;
    let change = Value::from(id);
    let calls: Vec<&log::Record> = records
        .iter()
        .filter(|record| record.fields.get("change") == Some(&change))
        .filter(|record| {
            let agent = record.fields.get("agent").and_then(Value::as_str);
            !agent.is_some_and(|agent| NOT_AGENTS.contains(&agent))
        })
        .collect();
    if calls.is_empty() {
        return Ok(None);
    }
    let sum = |tokens: fn(&log::Record) -> u64| calls.iter().copied().map(tokens).sum::<u64>();
    let cost: f64 = calls.iter().map(|record| record.cost_usd).sum();
    let line = format!(
        "Tokens: in {} · cache {} · out {} · {cost:.2} USD (estimate) · {} agent call(s)",
        grouped(sum(|r| r.tokens_in)),
        grouped(sum(|r| r.tokens_cache)),
        grouped(sum(|r| r.tokens_out)),
        calls.len()
    );
    Ok(Some(line))
}

/// `n` with a comma between each group of three digits: `1,234,567`.
fn grouped(n: u64) -> String {
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

/// `text` to [`LINE_LENGTH`] characters at most.
fn clipped(text: &str) -> String {
    text.chars().take(LINE_LENGTH).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::{Change, classify};
    use crate::git::tests::{Folder, Repo};
    use crate::zones::ZoneMap;

    const ZONES: &str = "default = \"yellow\"\n[[zone]]\ncolor = \"red\"\n\
                         paths = [\"src/storage/**\"]\nreason = \"Drafts\"\n";

    const COMMANDS: &str = "[release]\nsimulated = true\nbuild = \"true\"\n";

    fn classified(path: &str) -> Classification {
        let change = Change {
            paths: vec![path.to_string()],
            min_color: None,
            resumable: false,
        };
        let zones = ZoneMap::from_toml(ZONES).unwrap();
        classify(&zones, &rules::default_table(), &change).unwrap()
    }

    fn json(text: &str) -> Value {
        serde_json::from_str(text).unwrap()
    }

    /// A line of the log: a call of `agent` for change `change`, with its tokens (in, cache and
    /// out) and its cost.
    fn call(change: &str, agent: &str, tokens: [u64; 3], cost: f64) -> String {
        let line = serde_json::json!({
            "change": change, "agent": agent, "tokens_in": tokens[0],
            "tokens_cache": tokens[1], "tokens_out": tokens[2], "cost_usd": cost,
        });
        line.to_string()
    }

    #[test]
    fn the_rules_say_whether_main_is_pushed() {
        assert_eq!(push_main(""), Ok(false));
        let rules = "main_branch = \"main\"\npush_main = false\n";
        assert_eq!(push_main(rules), Ok(false));
        assert_eq!(push_main("push_main = true\n[limits]\nx = 1\n"), Ok(true));
        for wrong in ["push_main = \"yes\"\n", "push_main = "] {
            let result = push_main(wrong);
            assert!(matches!(result, Err(config::Error::Toml(_))), "{wrong:?}");
        }
    }

    #[test]
    fn a_change_without_approvals_says_none_were_needed() {
        let folder = Folder::new();
        let record = r#"{"title": "App", "type": "Feature", "gates": {"G3": {"decision": "ok"}}}"#;
        let record = json(record);
        let app = classified("src/app.rs");
        let text = message(&folder.0, "c1", &record, &app, &[], &[]).unwrap();
        let expected = "gdd(c1): G3 Merge Gate · App\n\n\
                        Color: yellow · lane full · type Feature\n\
                        Approvals: none needed\n\
                        Record: .gdd/changes/c1/ (local)\n\n\
                        Changelog: Added · App\n\
                        Breaking: no";
        assert_eq!(text, expected);
        let paid = ["security", "lint"];
        let text = message(&folder.0, "c1", &record, &app, &paid, &[]).unwrap();
        let line = "\nApprovals: none needed\nKnown debt paid: security, lint\nRecord: ";
        assert!(text.contains(line), "{text}");
        let lowered = |command: &str, from, to| debt::Lowered {
            command: command.into(),
            gate: Gate::G2,
            from,
            to,
        };
        let lowered = [lowered("lint", 5, 3), lowered("types", 9, 8)];
        let text = message(&folder.0, "c1", &record, &app, &paid, &lowered).unwrap();
        let lines = "\nKnown debt paid: security, lint\n\
                     Known debt lowered: lint from 5 to 3, types from 9 to 8\nRecord: ";
        assert!(text.contains(lines), "{text}");
        let text = message(&folder.0, "c1", &record, &app, &[], &lowered[..1]).unwrap();
        let line = "\nApprovals: none needed\nKnown debt lowered: lint from 5 to 3\nRecord: ";
        assert!(text.contains(line), "{text}");
        let bare = message(&folder.0, "c1", &json("{}"), &app, &[], &[]).unwrap();
        let start = "gdd(c1): G3 Merge Gate · \n\nColor: yellow · lane full · type -\n";
        assert!(bare.starts_with(start), "{bare}");
    }

    #[test]
    fn the_message_carries_the_record_of_the_change() {
        let folder = Folder::new();
        let zones = r#"{"decision": "ok", "by": "human", "ts": "2026-10-20T09:00:00+00:00"}"#;
        folder.write(approve::ZONES_PATH, zones);
        let first = "Notes\n- **Blocking** none\nVERDICT: KO\n\
                     - **Fix** [the guard](src/a.rs#L3) now\n";
        let outputs = ".gdd/changes/c1/outputs";
        folder.write(&format!("{outputs}/reviewer-1.md"), first);
        folder.write(&format!("{outputs}/reviewer-2.md"), "VERDICT:\n  OK\n");
        folder.write(&format!("{outputs}/reviewer-3.md"), "no verdict\n");
        let builder = "VERDICT: OK\n- **Not** a review\n";
        folder.write(&format!("{outputs}/builder-1.md"), builder);
        let log = [
            call("c1", "claude", [1200, 3000, 50], 0.5),
            call("c1", "codex", [300, 0, 7], 0.256),
            call("c1", "checks", [999_999, 0, 0], 0.0),
            call("c2", "claude", [999_999, 0, 0], 0.0),
        ];
        folder.write(log::PATH, &log.join("\n"));
        let note = |n: u32| {
            let text = format!("Choice {n}\nwith reason");
            serde_json::json!({"kind": "decided", "text": text})
        };
        let mut inbox = vec![serde_json::json!({"kind": "needs", "text": "Not this"})];
        inbox.extend((1..=9).map(note));
        let record = serde_json::json!({
            "title": "Save",
            "type": "Feature",
            "gates": {
                "G1": {
                    "decision": "ok", "by": "human", "ts": "2026-10-21T10:00:00+00:00",
                    "hashes": {"features/a.feature": "ab", "features/hidden/a.feature": "cd"}
                },
                "plan": {"decision": "ok", "by": "human", "ts": "2026-10-22T11:00:00+00:00"},
                "G3": {"decision": "ok", "by": "machines"}
            },
            "inbox": inbox
        });
        let red = classified("src/storage/db.rs");
        let text = message(&folder.0, "c1", &record, &red, &[], &[]).unwrap();
        let approvals = "Approvals: red zones ok (human, 2026-10-20); \
                         G1 ok (human, 2026-10-21, 2 scenario file(s) sealed); \
                         plan ok (human, 2026-10-22)";
        let tokens = "Tokens: in 1,500 · cache 3,000 · out 57 · \
                      0.76 USD (estimate) · 2 agent call(s)";
        let mut expected = vec![
            "gdd(c1): G3 Merge Gate · Save".to_string(),
            String::new(),
            "Color: red · lane full · type Feature".to_string(),
            approvals.to_string(),
            "Reviewer: KO -> OK -> ?".to_string(),
            "  - Blocking none".to_string(),
            "  - Fix the guard now".to_string(),
            "Decided without the human:".to_string(),
        ];
        expected.extend((2..=9).map(|n| format!("  - Choice {n} with reason")));
        expected.push(tokens.to_string());
        expected.push("Record: .gdd/changes/c1/ (local)".to_string());
        expected.push(String::new());
        expected.push("Changelog: Added · Save\nBreaking: no".to_string());
        assert_eq!(text, expected.join("\n"));
        // Only a red change names the red zones.
        let app = classified("src/app.rs");
        let yellow = message(&folder.0, "c1", &record, &app, &[], &[]).unwrap();
        let g1 = "\nApprovals: G1 ok (human, 2026-10-21, 2 scenario file(s) sealed); plan";
        assert!(yellow.contains(g1), "{yellow}");
    }

    #[test]
    fn the_line_of_the_change_ends_the_message_as_trailers_git_reads() {
        let repo = Repo::new();
        let yellow = classified("src/app.rs");
        let record = json(
            r#"{"title": "App", "type": "Refactor", "changelog":
                {"section": "Changed", "line": "Notes in one file", "breaking": true}}"#,
        );
        let text = message(repo.root(), "c1", &record, &yellow, &["lint"], &[]).unwrap();
        let end = "\nKnown debt paid: lint\nRecord: .gdd/changes/c1/ (local)\n\n\
                   Changelog: Changed · Notes in one file\nBreaking: yes";
        assert!(text.ends_with(end), "{text}");
        repo.write("message.txt", &text);
        let parsed = repo.git(&["interpret-trailers", "--parse", "message.txt"]);
        assert_eq!(
            parsed,
            "Changelog: Changed · Notes in one file\nBreaking: yes"
        );
        // Left out: no paragraph, and no trailers.
        for left_out in [
            r#"{"title": "Tidy", "type": "Refactor"}"#,
            r#"{"title": "App", "type": "Feature", "changelog": {"none": true}}"#,
        ] {
            let text = message(repo.root(), "c1", &json(left_out), &yellow, &[], &[]).unwrap();
            assert!(
                text.ends_with("\nRecord: .gdd/changes/c1/ (local)"),
                "{text}"
            );
            assert!(!text.contains("Changelog:"), "{text}");
        }
        let wrong = json(r#"{"changelog": {"section": "Improved", "line": "x"}}"#);
        let err = message(repo.root(), "c1", &wrong, &yellow, &[], &[]).unwrap_err();
        let start = ".gdd/changes/c1/change.json: the section must be one of ";
        assert!(err.starts_with(start), "{err}");
    }

    #[test]
    fn approvals_say_what_they_have() {
        let folder = Folder::new();
        let red = classified("src/storage/db.rs");
        // A red change whose zones nobody approved names no approval of them.
        let text = message(&folder.0, "c1", &json("{}"), &red, &[], &[]).unwrap();
        assert!(text.contains("\nApprovals: none needed\n"), "{text}");
        folder.write(approve::ZONES_PATH, "{}");
        let record = json(r#"{"gates": {"G1": {"decision": "ko"}, "plan": {}}}"#);
        let text = message(&folder.0, "c1", &record, &red, &[], &[]).unwrap();
        assert!(text.contains("\nApprovals: G1 ko (-, )\n"), "{text}");
        folder.write(approve::ZONES_PATH, "{not json");
        let text = message(&folder.0, "c1", &json("{}"), &red, &[], &[]).unwrap();
        assert!(text.contains("\nApprovals: none needed\n"), "{text}");
        fs::remove_file(folder.0.join(approve::ZONES_PATH)).unwrap();
        fs::create_dir_all(folder.0.join(approve::ZONES_PATH)).unwrap();
        let err = message(&folder.0, "c1", &json("{}"), &red, &[], &[]).unwrap_err();
        assert!(err.starts_with("cannot read .gdd/zones.json: "), "{err}");
    }

    #[test]
    fn what_the_message_cannot_read_is_an_error() {
        let folder = Folder::new();
        let yellow = classified("src/app.rs");
        folder.write(log::PATH, "{not json\n");
        let err = message(&folder.0, "c1", &json("{}"), &yellow, &[], &[]).unwrap_err();
        assert!(err.starts_with(".gdd/log.jsonl, line 1: "), "{err}");
        fs::remove_file(folder.0.join(log::PATH)).unwrap();
        fs::create_dir_all(folder.0.join(log::PATH)).unwrap();
        let err = message(&folder.0, "c1", &json("{}"), &yellow, &[], &[]).unwrap_err();
        assert!(err.starts_with("cannot read .gdd/log.jsonl: "), "{err}");
        fs::remove_dir(folder.0.join(log::PATH)).unwrap();
        folder.write(".gdd/changes/c1/outputs", "not a folder");
        let err = message(&folder.0, "c1", &json("{}"), &yellow, &[], &[]).unwrap_err();
        let start = "cannot read .gdd/changes/c1/outputs: ";
        assert!(err.starts_with(start), "{err}");
        fs::remove_file(folder.0.join(".gdd/changes/c1/outputs")).unwrap();
        fs::create_dir_all(folder.0.join(".gdd/changes/c1/outputs/reviewer-1.md")).unwrap();
        let err = message(&folder.0, "c1", &json("{}"), &yellow, &[], &[]).unwrap_err();
        let start = "cannot read .gdd/changes/c1/outputs/reviewer-1.md: ";
        assert!(err.starts_with(start), "{err}");
    }

    #[test]
    fn verdicts_links_and_numbers_are_read_as_the_kit_reads_them() {
        assert_eq!(verdict("VERDICT: OK"), "OK");
        assert_eq!(verdict("x VERDICT:KO y"), "KO");
        assert_eq!(verdict("VERDICT: maybe\nVERDICT: \t OK"), "OK");
        assert_eq!(verdict("verdict: OK"), "?");
        assert_eq!(verdict(""), "?");
        for (text, plain) in [
            ("[a](b)", "a"),
            ("x [the guard](a.rs) y [b]()", "x the guard y b"),
            ("[a [b](c)", "a [b"),
            ("[a] [b](c)", "[a] b"),
            ("[](x) [a](b", "[](x) [a](b"),
            ("[a]x(b)", "[a]x(b)"),
            ("no links", "no links"),
        ] {
            assert_eq!(unlinked(text), plain, "{text}");
        }
        let numbers = [(0, "0"), (999, "999"), (1_000, "1,000")];
        for (n, text) in numbers {
            assert_eq!(grouped(n), text);
        }
        assert_eq!(grouped(1_234_567), "1,234,567");
        assert_eq!(clipped(&"é".repeat(300)), "é".repeat(220));
        assert_eq!(clipped("short"), "short");
    }

    /// A project in a git repository with an author of its own, its files and its release
    /// commands committed on `main`, and change `c1` started on its branch, with G3 open.
    fn started(ignore: &str) -> Repo {
        let repo = Repo::new();
        repo.identify();
        repo.write(".gitignore", ignore);
        repo.write("src/old.rs", "old\n");
        repo.write(gates::COMMANDS, COMMANDS);
        repo.commit();
        repo.start("c1");
        let record = r#"{"id": "c1", "title": "App", "type": "Feature", "state": "building",
                         "gates": {"G3": {"decision": "ok", "by": "machines"}},
                         "inbox": [], "x": 1}"#;
        repo.write(".gdd/changes/c1/change.json", record);
        repo
    }

    fn merged(repo: &Repo) -> Result<String, String> {
        let project = Project {
            root: repo.root(),
            state: repo.root(),
            main: "main",
        };
        merge(&project, "c1", &classified("src/app.rs"))
    }

    fn record(repo: &Repo) -> Value {
        json(&change::read_record(repo.root(), "c1").unwrap())
    }

    fn agents(repo: &Repo) -> Vec<String> {
        let text = fs::read_to_string(repo.root().join(log::PATH)).unwrap();
        let records = log::parse(&text).unwrap();
        records.iter().map(|r| r.text("agent")).collect()
    }

    #[test]
    fn merge_keeps_the_last_work_and_records_the_change_in_the_merge_commit() {
        let repo = started(".gdd/*\n");
        repo.write("src/app.rs", "fn main() {}\n");
        let out = json(&merged(&repo).unwrap());
        assert_eq!(out["ok"], true);
        assert_eq!(out["simulated"], true);
        assert_eq!(out["detail"]["name"], "deploy_pre");
        assert_eq!(repo.git(&["branch", "--show-current"]), "main");
        repo.git(&["cat-file", "-e", "main:src/app.rs"]);
        let work = repo.git(&["log", "-1", "--format=%s", "change/c1"]);
        assert_eq!(work, "gdd(c1): change work");
        let message = repo.git(&["log", "-1", "--format=%B", "main"]);
        let expected = "gdd(c1): G3 Merge Gate · App\n\nColor: yellow · lane full · type Feature";
        assert!(message.starts_with(expected), "{message}");
        let line = repo.git(&["log", "-1", "--format=%(trailers:key=Changelog)", "main"]);
        assert_eq!(line, "Changelog: Added · App");
        let parents = repo.git(&["log", "-1", "--format=%P", "main"]);
        assert_eq!(parents.split(' ').count(), 2, "a merge commit");
        let record = record(&repo);
        assert_eq!(record["state"], "merged");
        let head = repo.git(&["rev-parse", "HEAD"]);
        assert_eq!(record["commit"], head.as_str());
        assert_eq!(record["x"], 1);
        assert_eq!(agents(&repo), ["merge", "build", "deploy_pre"]);
        let text = fs::read_to_string(repo.root().join(log::PATH)).unwrap();
        let line = &log::parse(&text).unwrap()[0];
        assert_eq!(line.text("gate"), "G3");
        assert_eq!(line.text("phase"), "3-integrate");
        assert_eq!(line.text("note"), head[..12]);
    }

    /// The project of [`started`] back on `main`, and a worktree on `change/c1` with work of
    /// the change to commit: the main checkout keeps the state.
    fn in_worktree() -> (Repo, Folder) {
        let repo = started(".gdd/*\n");
        repo.git(&["checkout", "-q", "main"]);
        let trees = Folder::new();
        let worktree = trees.0.join("c1");
        repo.tool().add_worktree(&worktree, "change/c1").unwrap();
        fs::create_dir_all(worktree.join("src")).unwrap();
        fs::write(worktree.join("src/app.rs"), "x\n").unwrap();
        (repo, trees)
    }

    fn merged_from(repo: &Repo, worktree: &Path) -> Result<String, String> {
        let project = Project {
            root: worktree,
            state: repo.root(),
            main: "main",
        };
        merge(&project, "c1", &classified("src/app.rs"))
    }

    #[test]
    fn a_change_in_its_worktree_merges_in_the_main_checkout() {
        let (repo, trees) = in_worktree();
        let worktree = trees.0.join("c1");
        assert!(merged_from(&repo, &worktree).is_ok());
        repo.git(&["cat-file", "-e", "main:src/app.rs"]);
        let message = repo.git(&["log", "-1", "--format=%s", "main"]);
        assert_eq!(message, "gdd(c1): G3 Merge Gate · App");
        let work = repo.git(&["log", "-1", "--format=%s", "change/c1"]);
        assert_eq!(work, "gdd(c1): change work");
        assert_eq!(repo.git(&["branch", "--show-current"]), "main");
        assert!(repo.root().join("src/app.rs").is_file());
        assert_eq!(Git::new(&worktree).current_branch().unwrap(), "change/c1");
        assert_eq!(record(&repo)["state"], "merged");
        assert_eq!(agents(&repo), ["merge", "build", "deploy_pre"]);
        assert!(!worktree.join(".gdd").exists());
    }

    #[test]
    fn a_main_checkout_off_main_or_with_uncommitted_changes_takes_no_merge() {
        let (repo, trees) = in_worktree();
        let worktree = trees.0.join("c1");
        let main = repo.git(&["rev-parse", "main"]);
        repo.git(&["checkout", "-q", "-b", "other"]);
        let err = merged_from(&repo, &worktree).unwrap_err();
        let off = format!(
            "the merge needs the main checkout ({}) on main, and it is on other: switch it back \
             to main",
            repo.root().display()
        );
        assert_eq!(err, off);
        repo.git(&["checkout", "-q", "main"]);
        repo.write("README.md", "changed\n");
        let err = merged_from(&repo, &worktree).unwrap_err();
        let dirty = "without uncommitted changes in tracked files: commit or stash them";
        assert!(err.ends_with(dirty), "{err}");
        // Nothing merged and nothing committed: the work waits in the worktree.
        assert_eq!(repo.git(&["rev-parse", "main"]), main);
        assert!(Git::new(&worktree).has_changes().unwrap());
        assert_eq!(record(&repo)["state"], "building");
        // Files git does not track do not stop it.
        repo.git(&["checkout", "-q", "--", "README.md"]);
        repo.write("notes.txt", "mine\n");
        assert_eq!(not_ready(repo.root(), "main").unwrap(), None);
        let plain = Folder::new();
        assert!(not_ready(&plain.0, "main").is_err());
    }

    #[test]
    fn may_merge_says_what_merge_refuses_and_touches_nothing() {
        let (repo, trees) = in_worktree();
        let worktree = trees.0.join("c1");
        let project = Project {
            root: &worktree,
            state: repo.root(),
            main: "main",
        };
        assert_eq!(may_merge(&project, "c1"), Ok(()));
        repo.write("README.md", "changed\n");
        let err = may_merge(&project, "c1").unwrap_err();
        assert!(err.ends_with("commit or stash them"), "{err}");
        repo.git(&["checkout", "-q", "--", "README.md"]);
        let off = Project {
            root: repo.root(),
            ..project
        };
        let expected = "Not on branch change/c1: merge runs where the last work of the change is.";
        assert_eq!(may_merge(&off, "c1"), Err(expected.to_string()));
        let closed = r#"{"gates": {"G3": {"decision": "ko"}}}"#;
        repo.write(".gdd/changes/c1/change.json", closed);
        assert_eq!(may_merge(&project, "c1"), Err(G3_CLOSED.to_string()));
        let err = may_merge(&project, "c9").unwrap_err();
        assert!(err.starts_with("change c9 does not exist"), "{err}");
        assert!(Git::new(&worktree).has_changes().unwrap());
        let last = repo.git(&["log", "-1", "--format=%s", "change/c1"]);
        assert_eq!(last, "commit");
    }

    #[test]
    fn a_branch_with_nothing_left_to_commit_merges_as_it_is() {
        let repo = started(".gdd/*\n");
        repo.write("src/app.rs", "x\n");
        repo.commit();
        let tip = repo.git(&["rev-parse", "HEAD"]);
        assert!(merged(&repo).is_ok());
        assert_eq!(repo.git(&["rev-parse", "change/c1"]), tip);
        assert_eq!(repo.git(&["rev-parse", "HEAD^2"]), tip);
    }

    #[test]
    fn local_state_that_git_does_not_ignore_merges_with_the_change() {
        let repo = started("");
        repo.write("src/app.rs", "x\n");
        assert!(merged(&repo).is_ok());
        assert_eq!(record(&repo)["state"], "merged");
        assert_eq!(agents(&repo), ["merge", "build", "deploy_pre"]);
    }

    /// Known debt of `lint` in G2 and in G3, and of `security` in G3.
    const DEBT: &str = "adopted = \"2026-10-27\"\n\
                        [[debt]]\ncommand = \"lint\"\ngate = \"g2\"\nsince = \"2026-10-27\"\n\
                        [[debt]]\ncommand = \"lint\"\ngate = \"g3\"\nsince = \"2026-10-27\"\n\
                        [[debt]]\ncommand = \"security\"\ngate = \"g3\"\nsince = \"2026-10-27\"\n";

    /// The project of [`started`] with `known` as its known debt on `main` before the change
    /// started, the latest G2 of `c1` naming `paid` as paid, and work of the change to commit.
    fn paying(known: &str, paid: &str) -> Repo {
        let repo = started(".gdd/*\n");
        repo.git(&["checkout", "-q", "main"]);
        repo.write(debt::PATH, known);
        repo.commit();
        repo.git(&["checkout", "-q", "change/c1"]);
        repo.git(&["merge", "-q", "main"]);
        let g2 = format!(r#"{{"ok": true, "paid_debt": {paid}}}"#);
        repo.write(".gdd/changes/c1/g2.json", &g2);
        repo.write("src/app.rs", "x\n");
        repo
    }

    #[test]
    fn the_merge_commit_takes_the_paid_debt_off_the_list() {
        let repo = paying(DEBT, r#"["lint"]"#);
        let g3 = r#"{"ok": true, "paid_debt": ["security"]}"#;
        repo.write(".gdd/changes/c1/g3.json", g3);
        assert!(merged(&repo).is_ok());
        let message = repo.git(&["log", "-1", "--format=%B", "main"]);
        let paid = "\nKnown debt paid: security, lint\n";
        assert!(message.contains(paid), "{message}");
        let parents = repo.git(&["log", "-1", "--format=%P", "main"]);
        assert_eq!(parents.split(' ').count(), 2, "a merge commit");
        // Only the debt of the gate that paid it goes: lint stays in G3.
        let left = repo.git(&["show", "main:gdd/debt.toml"]);
        assert_eq!(left.matches("[[debt]]").count(), 1, "{left}");
        let lint = "command = \"lint\"\ngate = \"g3\"";
        assert!(left.contains(lint), "{left}");
        // The branch of the change keeps the list as it was: the merge commit pays.
        let branch = repo.git(&["show", "change/c1:gdd/debt.toml"]);
        assert_eq!(branch, DEBT.trim_end());
        assert_eq!(repo.git(&["status", "--porcelain"]), "");
        assert_eq!(record(&repo)["state"], "merged");
    }

    #[test]
    fn the_last_debt_paid_takes_the_list_away() {
        let only = "adopted = \"2026-10-27\"\n\
                    [[debt]]\ncommand = \"lint\"\ngate = \"g2\"\nsince = \"2026-10-27\"\n";
        let repo = paying(only, r#"["lint"]"#);
        assert!(merged(&repo).is_ok());
        assert!(!repo.root().join(debt::PATH).exists());
        let listed = repo.git(&["ls-tree", "--name-only", "main", "gdd/"]);
        assert_eq!(listed, gates::COMMANDS);
        let message = repo.git(&["log", "-1", "--format=%B", "main"]);
        assert!(message.contains("\nKnown debt paid: lint\n"), "{message}");
    }

    /// Known debt of `lint` in G2 with the count 5, and of `security` in G3 with the count 2.
    const COUNTED: &str = concat!(
        "adopted = \"2026-10-27\"\n",
        "[[debt]]\ncommand = \"lint\"\ngate = \"g2\"\nsince = \"2026-10-27\"\ncount = 5\n",
        "[[debt]]\ncommand = \"security\"\ngate = \"g3\"\nsince = \"2026-10-27\"\ncount = 2\n",
    );

    /// A report of a gate that names `name`, known debt with the count `known`, as still failing
    /// with the count `count`.
    fn owing(name: &str, count: u64, known: u64) -> String {
        let owed = serde_json::json!({"name": name, "tail": "x", "count": count, "known": known});
        serde_json::json!({"ok": true, "known_debt": [owed]}).to_string()
    }

    #[test]
    fn the_merge_commit_lowers_the_counts_the_change_lowered() {
        let repo = paying(COUNTED, "[]");
        repo.write(".gdd/changes/c1/g2.json", &owing("lint", 3, 5));
        assert!(merged(&repo).is_ok());
        let message = repo.git(&["log", "-1", "--format=%B", "main"]);
        let line = "\nKnown debt lowered: lint from 5 to 3\nRecord: ";
        assert!(message.contains(line), "{message}");
        let parents = repo.git(&["log", "-1", "--format=%P", "main"]);
        assert_eq!(parents.split(' ').count(), 2, "a merge commit");
        let left = repo.git(&["show", "main:gdd/debt.toml"]);
        let lint = "command = \"lint\"\ngate = \"g2\"\nsince = \"2026-10-27\"\ncount = 3\n";
        assert!(left.contains(lint), "{left}");
        let security = "gate = \"g3\"\nsince = \"2026-10-27\"\ncount = 2";
        assert!(left.ends_with(security), "{left}");
        assert_eq!(repo.git(&["status", "--porcelain"]), "");
    }

    #[test]
    fn the_debt_paid_and_the_counts_lowered_go_in_the_same_commit() {
        let repo = paying(COUNTED, r#"["lint"]"#);
        repo.write(".gdd/changes/c1/g3.json", &owing("security", 1, 2));
        assert!(merged(&repo).is_ok());
        let message = repo.git(&["log", "-1", "--format=%B", "main"]);
        let lines = "\nKnown debt paid: lint\nKnown debt lowered: security from 2 to 1\nRecord: ";
        assert!(message.contains(lines), "{message}");
        let left = repo.git(&["show", "main:gdd/debt.toml"]);
        assert_eq!(left.matches("[[debt]]").count(), 1, "{left}");
        let security = "gate = \"g3\"\nsince = \"2026-10-27\"\ncount = 1";
        assert!(left.ends_with(security), "{left}");
    }

    #[test]
    fn a_count_kept_or_above_leaves_the_list_and_the_message_as_they_were() {
        for count in [5, 7] {
            let repo = paying(COUNTED, "[]");
            repo.write(".gdd/changes/c1/g2.json", &owing("lint", count, 5));
            assert!(merged(&repo).is_ok());
            let message = repo.git(&["log", "-1", "--format=%B", "main"]);
            assert!(!message.contains("Known debt"), "{message}");
            let left = repo.git(&["show", "main:gdd/debt.toml"]);
            assert_eq!(left, COUNTED.trim_end(), "{count}");
        }
    }

    #[test]
    fn a_debt_list_that_cannot_be_paid_gives_the_merge_up() {
        let repo = paying("[[debt]", "\"lint\"");
        let err = merged(&repo).unwrap_err();
        assert!(err.starts_with(".gdd/changes/c1/g2.json: "), "{err}");
        assert_eq!(repo.git(&["branch", "--show-current"]), "change/c1");
        repo.write(".gdd/changes/c1/g2.json", r#"{"paid_debt": ["lint"]}"#);
        let main = repo.git(&["rev-parse", "main"]);
        let err = merged(&repo).unwrap_err();
        assert!(err.starts_with("gdd/debt.toml: invalid TOML: "), "{err}");
        let given_up = "The merge was given up: main is as it was.";
        assert!(err.ends_with(given_up), "{err}");
        assert_eq!(repo.git(&["rev-parse", "main"]), main);
        assert!(!repo.tool().exists("MERGE_HEAD").unwrap());
        assert_eq!(repo.git(&["status", "--porcelain"]), "");
        assert_eq!(record(&repo)["state"], "building");
    }

    #[test]
    fn merge_needs_g3_and_the_branch_of_the_change() {
        let repo = started(".gdd/*\n");
        let main = repo.git(&["rev-parse", "main"]);
        let closed: Result<String, String> = Err(G3_CLOSED.to_string());
        for gates in ["{}", r#"{"G3": {"decision": "ko"}}"#, r#"{"G3": "ok"}"#] {
            let record = format!(r#"{{"id": "c1", "gates": {gates}}}"#);
            repo.write(".gdd/changes/c1/change.json", &record);
            assert_eq!(merged(&repo), closed, "{gates}");
        }
        assert_eq!(G3_CLOSED, "G3 is not open.");
        let open = r#"{"gates": {"G3": {"decision": "ok"}}}"#;
        repo.write(".gdd/changes/c1/change.json", open);
        repo.git(&["checkout", "-q", "main"]);
        repo.write("src/loose.rs", "x\n");
        let expected = "Not on branch change/c1: merge runs where the last work of the change is.";
        assert_eq!(merged(&repo), Err(expected.to_string()));
        assert_eq!(repo.git(&["rev-parse", "main"]), main);
        assert!(!repo.root().join(log::PATH).exists());
        let missing = Folder::new();
        let project = Project {
            root: &missing.0,
            state: &missing.0,
            main: "main",
        };
        let err = merge(&project, "c1", &classified("src/app.rs")).unwrap_err();
        assert!(err.starts_with("change c1 does not exist"), "{err}");
    }

    #[test]
    fn a_failed_build_or_deploy_fails_the_merge_after_it_is_recorded() {
        let repo = started(".gdd/*\n");
        let broken = "[release]\nsimulated = true\nbuild = \"echo no; exit 3\"\n";
        repo.write(gates::COMMANDS, broken);
        let err = merged(&repo).unwrap_err();
        let expected = "Could not build the artifact: release.build failed (exit code 3):\nno";
        assert_eq!(err, expected);
        assert_eq!(record(&repo)["state"], "merged");
        assert_eq!(agents(&repo), ["merge", "build"]);
        let repo = started(".gdd/*\n");
        repo.write(gates::COMMANDS, "[release]\nbuild = \"true\"\n");
        let err = merged(&repo).unwrap_err();
        let expected = "Could not deploy to pre: release.deploy_pre failed:\n\
                        release.deploy_pre not configured and simulated = false";
        assert_eq!(err, expected);
    }

    #[test]
    fn the_build_and_the_deploy_after_the_merge_run_within_the_time_limit_of_the_rules() {
        let repo = started(".gdd/*\n");
        let slow = "[release]\nbuild = \"true\"\ndeploy_pre = \"echo started; sleep 5\"\n";
        repo.write(gates::COMMANDS, slow);
        repo.write(guard::RULES, "[limits]\ncommand_seconds = 1\n");
        let err = merged(&repo).unwrap_err();
        let expected = "Could not deploy to pre: release.deploy_pre failed (exit code 124):\n\
                        started\ntimed out (1s): raise limits.command_seconds in gdd/rules.toml";
        assert_eq!(err, expected);
        assert_eq!(agents(&repo), ["merge", "build", "deploy_pre"]);
    }

    /// A bare repository in a temporary folder, the remote `origin` of `repo`.
    fn remote(repo: &Repo) -> Folder {
        let remote = Folder::new();
        let path = remote.0.to_string_lossy().into_owned();
        repo.git(&["init", "-q", "--bare", &path]);
        repo.git(&["remote", "add", "origin", &path]);
        remote
    }

    #[test]
    fn main_is_pushed_only_when_the_rules_ask_and_there_is_a_remote() {
        let asked = "push_main = true\n";
        let repo = started(".gdd/*\n");
        repo.write(guard::RULES, asked);
        assert!(merged(&repo).is_ok(), "no remote");
        let repo = started(".gdd/*\n");
        let remote = remote(&repo);
        assert!(merged(&repo).is_ok());
        let origin = Git::new(&remote.0);
        assert!(!origin.exists("main").unwrap(), "not asked to push");
        repo.git(&["checkout", "-q", "change/c1"]);
        repo.write(guard::RULES, asked);
        assert!(merged(&repo).is_ok());
        let pushed = origin.merge_base("main", "main").unwrap();
        assert_eq!(pushed, repo.git(&["rev-parse", "main"]));
        let repo = started(".gdd/*\n");
        repo.git(&["remote", "add", "origin", "/nonexistent/gdd-remote"]);
        repo.write(guard::RULES, asked);
        let err = merged(&repo).unwrap_err();
        let start = "Merged into main, but the push failed: ";
        assert!(err.starts_with(start), "{err}");
        assert_eq!(record(&repo)["state"], "merged");
    }

    #[test]
    fn what_follows_the_merge_runs_as_main_configures_it() {
        let repo = started(".gdd/*\n");
        let remote = remote(&repo);
        // The main branch changed its release after the branch of the change left it.
        repo.git(&["checkout", "-q", "main"]);
        let real = "[release]\nbuild = \"true\"\ndeploy_pre = \"touch deployed\"\n";
        repo.write(gates::COMMANDS, real);
        repo.write(guard::RULES, "push_main = true\n");
        repo.commit();
        repo.git(&["checkout", "-q", "change/c1"]);
        repo.write("src/app.rs", "x\n");
        let out = json(&merged(&repo).unwrap());
        assert_eq!(out["simulated"], false);
        assert!(repo.root().join("deployed").exists());
        let pushed = Git::new(&remote.0).merge_base("main", "main").unwrap();
        assert_eq!(pushed, repo.git(&["rev-parse", "main"]));
    }

    #[test]
    fn a_configuration_that_cannot_be_read_fails_the_merge_after_it_is_recorded() {
        let failed = |path: &str, text: &str| {
            let repo = started(".gdd/*\n");
            repo.write(path, text);
            let err = merged(&repo).unwrap_err();
            assert_eq!(record(&repo)["state"], "merged", "{path}");
            assert_eq!(agents(&repo), ["merge"], "{path}");
            err
        };
        let err = failed(gates::COMMANDS, "[release\n");
        let start = "gdd/commands.toml: invalid TOML";
        assert!(err.starts_with(start), "{err}");
        let err = failed(guard::RULES, "push_main = \"yes\"\n");
        let start = "gdd/rules.toml: invalid TOML";
        assert!(err.starts_with(start), "{err}");
        let err = failed(guard::RULES, "[limits]\ncommand_seconds = 0\n");
        let said = "gdd/rules.toml: invalid TOML: limits.command_seconds must be a whole number \
                    of seconds above 0, not 0";
        assert_eq!(err, said);
        // A folder where the rules file should be.
        let err = failed("gdd/rules.toml/x", "");
        let start = "cannot read gdd/rules.toml: ";
        assert!(err.starts_with(start), "{err}");
    }

    /// The ADR a spike adds.
    const STORAGE: &str = "# 0002\n## Decision\nSQLite.\n";

    /// A repository with an author whose `main` holds two ADRs, and spike `c1`, whose branch adds
    /// an ADR and an image next to it, changes one ADR, removes the other and adds code; `main`
    /// checked out, which moved on after the branch left it.
    fn spiked() -> Repo {
        let repo = Repo::new();
        repo.identify();
        repo.write(".gitignore", ".gdd/\n");
        repo.write("docs/adr/0000-gone.md", "# 0000\n");
        repo.write("docs/adr/0001-old.md", "# 0001\n");
        repo.commit();
        repo.start("c1");
        repo.git(&["rm", "-q", "docs/adr/0000-gone.md"]);
        repo.write("docs/adr/0001-old.md", "# 0001\n## Decision\nKeep it.\n");
        repo.write("docs/adr/0002-storage.md", STORAGE);
        repo.write("docs/adr/img/schema.svg", "<svg/>\n");
        repo.write("src/app.txt", "spike code\n");
        repo.write("docs/notes.md", "notes\n");
        repo.commit();
        repo.git(&["checkout", "-q", "main"]);
        repo.write("README.md", "main moved on\n");
        repo.commit();
        repo
    }

    /// OK at the G4 of spike `id` of `repo`, titled `title`: its ADR files from its own branch,
    /// as they are there.
    fn adr_ok(repo: &Repo, id: &str, title: &str) -> Result<Vec<String>, String> {
        let branch = change::branch(id);
        let from = Adrs {
            branch: &branch,
            appended: "",
        };
        adr_only(repo.root(), "main", id, title, &from)
    }

    /// The paths the last commit on `main` of `repo` touched.
    fn last_commit_paths(repo: &Repo) -> Vec<String> {
        let shown = repo.git(&["show", "--format=", "--name-only", "main"]);
        shown.lines().map(str::to_string).collect()
    }

    /// What the file `path` of `repo` holds: nothing when there is none.
    fn held(repo: &Repo, path: &str) -> String {
        fs::read_to_string(repo.root().join(path)).unwrap_or_default()
    }

    #[test]
    fn ok_to_a_spike_brings_only_its_adr_files_to_main_in_a_commit_of_their_own() {
        let repo = spiked();
        let adrs = [
            "docs/adr/0001-old.md",
            "docs/adr/0002-storage.md",
            "docs/adr/img/schema.svg",
        ];
        assert_eq!(adr_files(repo.root(), "main", "change/c1").unwrap(), adrs);
        let before = repo.git(&["rev-parse", "main"]);
        let committed = adr_ok(&repo, "c1", "Storage").unwrap();
        assert_eq!(committed, adrs);
        let message = repo.git(&["log", "-1", "--format=%B", "main"]);
        assert_eq!(message, "gdd(c1): Spike ADR · Storage");
        assert_eq!(repo.git(&["rev-parse", "main~1"]), before);
        let merges = repo.git(&["rev-list", "--count", "--merges", "main"]);
        assert_eq!(merges, "0");
        assert_eq!(last_commit_paths(&repo), adrs);
        let decided = "# 0001\n## Decision\nKeep it.\n";
        assert_eq!(held(&repo, "docs/adr/0001-old.md"), decided);
        assert_eq!(held(&repo, "docs/adr/0002-storage.md"), STORAGE);
        assert_eq!(held(&repo, "docs/adr/img/schema.svg"), "<svg/>\n");
        // What the spike removed, and its code, stay as main has them.
        assert_eq!(held(&repo, "docs/adr/0000-gone.md"), "# 0000\n");
        assert!(!repo.root().join("src/app.txt").exists());
        assert!(!repo.root().join("docs/notes.md").exists());
        assert_eq!(held(&repo, "README.md"), "main moved on\n");
        assert_eq!(repo.git(&["status", "--porcelain"]), "");
        assert_eq!(repo.git(&["rev-parse", "--abbrev-ref", "HEAD"]), "main");
    }

    #[test]
    fn a_spike_brings_to_main_only_what_main_lacks() {
        let repo = spiked();
        repo.write("docs/adr/0002-storage.md", STORAGE);
        repo.git(&["add", "docs/adr/0002-storage.md"]);
        repo.git(&["commit", "-q", "-m", "already there"]);
        let committed = adr_ok(&repo, "c1", "Storage").unwrap();
        let brought = ["docs/adr/0001-old.md", "docs/adr/img/schema.svg"];
        assert_eq!(committed, brought);
        assert_eq!(last_commit_paths(&repo), committed);
        // With nothing left to bring, no commit.
        let before = repo.git(&["rev-parse", "main"]);
        assert_eq!(adr_ok(&repo, "c1", "Storage"), Ok(vec![]));
        assert_eq!(repo.git(&["rev-parse", "main"]), before);
        // A spike without ADR files has nothing to bring.
        repo.git(&["branch", "change/c2"]);
        let none = adr_files(repo.root(), "main", "change/c2").unwrap();
        assert!(none.is_empty());
        assert_eq!(adr_ok(&repo, "c2", "None"), Ok(vec![]));
        assert_eq!(repo.git(&["rev-parse", "main"]), before);
    }

    #[test]
    fn ok_to_a_variant_brings_its_adr_files_from_its_branch_with_what_is_appended() {
        let repo = spiked();
        repo.git(&["checkout", "-q", "-b", "change/c1-v2", "main"]);
        let postgres = "# 0002\n## Decision\nPostgres.";
        repo.write("docs/adr/0002-storage.md", postgres);
        repo.write("docs/adr/0002-schema.svg", "<svg/>");
        repo.write("docs/adr/img/0002-notes.md", "# Notes\n");
        repo.write("src/app.txt", "variant code\n");
        repo.commit();
        repo.git(&["checkout", "-q", "main"]);
        let adrs = [
            "docs/adr/0002-schema.svg",
            "docs/adr/0002-storage.md",
            "docs/adr/img/0002-notes.md",
        ];
        let files = adr_files(repo.root(), "main", "change/c1-v2").unwrap();
        assert_eq!(files, adrs);
        let section = "## Variants\nChosen: variant 2 (Postgres), branch change/c1-v2.\n";
        let from = Adrs {
            branch: "change/c1-v2",
            appended: section,
        };
        let brought = adr_only(repo.root(), "main", "c1", "Storage", &from).unwrap();
        assert_eq!(brought, adrs);
        let message = repo.git(&["log", "-1", "--format=%B", "main"]);
        assert_eq!(message, "gdd(c1): Spike ADR · Storage");
        assert_eq!(last_commit_paths(&repo), adrs);
        let expected = format!("{postgres}\n\n{section}");
        assert_eq!(held(&repo, "docs/adr/0002-storage.md"), expected);
        // Only the ADR gets the section: what else it brings reaches main as the branch has it.
        assert_eq!(held(&repo, "docs/adr/0002-schema.svg"), "<svg/>");
        assert_eq!(held(&repo, "docs/adr/img/0002-notes.md"), "# Notes\n");
        assert!(!repo.root().join("src/app.txt").exists());
        assert_eq!(repo.git(&["status", "--porcelain"]), "");
        // Once main has them as they would be, there is nothing left to bring.
        let again = adr_only(repo.root(), "main", "c1", "Storage", &from);
        assert_eq!(again, Ok(vec![]));
    }

    #[test]
    fn what_is_appended_to_an_adr_comes_after_a_blank_line() {
        let section = "## Variants\nx\n";
        let ended = appended(b"# 0002\n".to_vec(), section);
        assert_eq!(ended, b"# 0002\n\n## Variants\nx\n");
        let open = appended(b"# 0002".to_vec(), section);
        assert_eq!(open, b"# 0002\n\n## Variants\nx\n");
        assert_eq!(appended(Vec::new(), section), b"\n## Variants\nx\n");
        assert_eq!(appended(b"# 0002".to_vec(), ""), b"# 0002");
    }

    #[test]
    fn a_spike_reaches_main_only_from_a_main_checkout_that_could_take_a_merge() {
        let repo = spiked();
        repo.git(&["checkout", "-q", "change/c1"]);
        let err = adr_ok(&repo, "c1", "Storage").unwrap_err();
        let shown = repo.root().display();
        let expected = format!(
            "the merge needs the main checkout ({shown}) on main, and it is on change/c1: \
             switch it back to main"
        );
        assert_eq!(err, expected);
        repo.git(&["checkout", "-q", "main"]);
        repo.write("README.md", "uncommitted\n");
        let before = repo.git(&["rev-parse", "main"]);
        let err = adr_ok(&repo, "c1", "Storage").unwrap_err();
        assert!(err.contains("without uncommitted changes"), "{err}");
        assert_eq!(repo.git(&["rev-parse", "main"]), before);
        assert!(!repo.root().join("docs/adr/0002-storage.md").exists());
        // A spike whose branch does not exist cannot be read.
        repo.git(&["checkout", "-q", "--", "README.md"]);
        let err = adr_ok(&repo, "c9", "None").unwrap_err();
        assert!(err.starts_with("git diff"), "{err}");
    }

    /// Makes `path` of `repo` a symbolic link to `target`.
    fn linked(repo: &Repo, target: &Path, path: &str) {
        std::os::unix::fs::symlink(target, repo.root().join(path)).unwrap();
    }

    /// What stands in the way of an ADR: what it is, how it gets there, given a folder elsewhere,
    /// and how the refusal starts.
    type Way = (&'static str, fn(&Repo, &Path), &'static str);

    #[test]
    fn what_stands_in_the_way_of_an_adr_stops_it_before_anything_is_written() {
        const NEW: &str = "docs/adr/0002-storage.md";
        let image = "cannot write docs/adr/img/schema.svg in the main checkout (";
        let untracked = "docs/adr/0002-storage.md is in the main checkout (";
        let ways: [Way; 5] = [
            (
                "a file where the folder of the image goes",
                |repo, _| repo.write("docs/adr/img", "in the way\n"),
                image,
            ),
            (
                "a link where the folder of the image goes",
                |repo, elsewhere| linked(repo, elsewhere, "docs/adr/img"),
                image,
            ),
            (
                "a link where an ADR goes",
                |repo, elsewhere| linked(repo, &elsewhere.join("x.md"), NEW),
                "cannot write docs/adr/0002-storage.md in the main checkout (",
            ),
            (
                "a draft git does not track where an ADR goes",
                |repo, _| repo.write(NEW, "draft\n"),
                untracked,
            ),
            (
                "the same ADR, still out of git",
                |repo, _| repo.write(NEW, STORAGE),
                untracked,
            ),
        ];
        for (way, in_the_way, start) in ways {
            let (repo, elsewhere) = (spiked(), Folder::new());
            in_the_way(&repo, &elsewhere.0);
            let draft = held(&repo, NEW);
            let main = repo.git(&["rev-parse", "main"]);
            let err = adr_ok(&repo, "c1", "Storage").unwrap_err();
            assert!(err.starts_with(start), "{way}: {err}");
            assert!(err.ends_with(", then answer again"), "{way}: {err}");
            // Nothing written: not the ADRs before it either, nor through the link.
            assert_eq!(held(&repo, "docs/adr/0001-old.md"), "# 0001\n", "{way}");
            assert_eq!(held(&repo, NEW), draft, "{way}");
            let untouched = fs::read_dir(&elsewhere.0).unwrap().next().is_none();
            assert!(untouched, "{way}");
            assert_eq!(repo.git(&["rev-parse", "main"]), main, "{way}");
        }
    }

    #[test]
    fn a_link_the_spike_left_under_docs_adr_never_reaches_main() {
        let repo = spiked();
        repo.git(&["checkout", "-q", "change/c1"]);
        let code = Path::new("../../src/app.txt");
        linked(&repo, code, "docs/adr/0003-code.md");
        repo.commit();
        repo.git(&["checkout", "-q", "main"]);
        let adrs = [
            "docs/adr/0001-old.md",
            "docs/adr/0002-storage.md",
            "docs/adr/img/schema.svg",
        ];
        assert_eq!(adr_files(repo.root(), "main", "change/c1").unwrap(), adrs);
        let brought = adr_ok(&repo, "c1", "Storage").unwrap();
        assert_eq!(brought, adrs);
        assert!(fs::symlink_metadata(repo.root().join("docs/adr/0003-code.md")).is_err());
        let kept = repo.tool().file_at("main", "docs/adr/0003-code.md");
        assert_eq!(kept.unwrap(), None);
    }

    #[test]
    fn a_commit_of_the_adr_that_fails_gives_the_main_checkout_back_what_it_held() {
        use std::os::unix::fs::PermissionsExt;
        let repo = spiked();
        let main = repo.git(&["rev-parse", "main"]);
        let hooks = Folder::new();
        let hook = hooks.0.join("pre-commit");
        fs::write(&hook, "#!/bin/sh\necho refused >&2\nexit 1\n").unwrap();
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
        let path = hooks.0.display().to_string();
        repo.git(&["config", "--local", "core.hooksPath", &path]);
        let err = adr_ok(&repo, "c1", "Storage").unwrap_err();
        assert!(err.starts_with("git commit "), "{err}");
        assert!(err.contains("refused"), "{err}");
        let end = ". The ADR files were taken back out: main is as it was.";
        assert!(err.ends_with(end), "{err}");
        // The file it changed holds what it held, and the new ones are gone, from the index too.
        assert_eq!(repo.git(&["rev-parse", "main"]), main);
        assert_eq!(held(&repo, "docs/adr/0001-old.md"), "# 0001\n");
        assert!(!repo.root().join("docs/adr/0002-storage.md").exists());
        assert!(!repo.root().join("docs/adr/img/schema.svg").exists());
        assert_eq!(repo.git(&["status", "--porcelain"]), "");
        assert!(!repo.tool().has_staged("docs/adr").unwrap());
        // Once git takes the commit, the same answer brings them.
        repo.git(&["config", "--local", "core.hooksPath", "/dev/null"]);
        assert!(adr_ok(&repo, "c1", "Storage").is_ok());
        assert_eq!(held(&repo, "docs/adr/0002-storage.md"), STORAGE);
    }

    #[test]
    fn a_main_checkout_that_cannot_be_put_back_is_said_after_the_failure() {
        let adrs = [Adr {
            path: "docs/adr/0002-storage.md".to_string(),
            bytes: STORAGE.as_bytes().to_vec(),
            held: None,
        }];
        // Outside a repository, the index cannot be put back.
        let plain = Folder::new();
        let said = put_back(&Git::new(&plain.0), &plain.0, &adrs, "it failed");
        let start = "it failed. Putting the main checkout back failed too: git reset ";
        assert!(said.starts_with(start), "{said}");
    }
}
