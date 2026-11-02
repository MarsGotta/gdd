//! The Decision Inbox of a change: how the change works, as its Reviewer says; what each variant
//! of a spike did; what the human must decide, what was decided without them and what was parked,
//! from the notes of its record; the security exceptions its last G3 named as added; the known
//! debt its latest G2 and G3 found still failing or paid; and, while it waits for the G4 of the
//! human, the gates whose green proved nothing in them.

use std::collections::BTreeMap;
use std::fmt::Write;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::agents::files;
use crate::change::{self, NOTE_KINDS, POLISH, Variant};
use crate::gates::{Gate, debt, verdict};
use crate::git::Git;
use crate::guard;
use crate::layout::{self, Layout};
use crate::recipe::worktree;
use crate::rules::RulesTable;

/// The heading of the section of a review that says how the change works, and of its group.
pub const HOW_IT_WORKS: &str = "How it works";

/// The title of the group of the variants of a spike.
pub const VARIANTS: &str = "Variants";

/// The most lines of the section that says how the change works that are kept.
const MOST_EXPLAINED: usize = 12;

/// What a line of the verdict of a review starts with.
const VERDICT: &str = "VERDICT:";

/// The state in which a change waits for the G4 of the human.
const IN_PRE: &str = "in-pre";

/// What the inbox of a change that waits for its G4 says when its review does not say how the
/// change works.
const UNEXPLAINED: &str = "The Reviewer did not explain the change.";

/// The title of the group of each kind of note, in the order of [`NOTE_KINDS`].
const HEADINGS: [&str; 3] = ["Needs your decision", "Decided without you", "Parked"];

/// The title of the group of the security exceptions the change adds.
const EXCEPTIONS: &str = "Security exceptions this change adds";

/// The title of the group of the commands with known debt still failing.
const KNOWN_DEBT: &str = "Known debt";

/// The title of the group of the commands with known debt the change paid.
const DEBT_PAID: &str = "Known debt paid";

/// The title of the group of the gates whose green proved nothing in the latest runs of the
/// change, while it waits for the G4 of the human.
const UNPROVEN: &str = "Gates not seen to close";

/// What follows the name of a gate in that group.
const NO_PROOF: &str = "not seen to close; its green proves nothing (tminus selftest)";

/// A group of the inbox from the gate reports: its title and one item per entry.
pub type Group = (&'static str, Vec<String>);

/// What the header says of a change that has neither a color nor a lane yet.
const UNDECIDED: &str = "Color and lane not decided yet (tminus decides them at launch)";

/// What the paths outside the package that the scope check accepted say among the decisions.
const ACCEPTED: &str = "Paths outside the package, same or lower color, accepted by rule:";

/// What follows the number of requests resolved.
const RESOLVED: &str = "earlier request(s) were resolved (kept in change.json).";

/// What follows the number of requests still open when the change ended.
const LEFT_OPEN: &str = "request(s) were still open when the change ended (kept in change.json).";

/// The states in which a change has ended: nothing waits for the human any more.
const ENDED: [&str; 2] = ["closed", "rejected"];

/// The state of a change that was never launched.
const OPEN: &str = "open";

/// What follows the number of reasons to park that a later launch settled.
const SETTLED: &str =
    "earlier reason(s) to park were settled by a later launch (kept in change.json).";

/// The kind of the notes that say why a change parked.
const PARKED: &str = "parked";

/// The title of the group of the KOs a human gave at a gate.
const YOUR_DECISIONS: &str = "Your decisions";

/// The gates of a change that keep a decision in its record, in the order of the method.
const DECIDED_GATES: [&str; 5] = ["plan", "G1", "G2", "G3", "G4"];

#[derive(Default, Deserialize)]
#[serde(default)]
struct Raw {
    title: Option<String>,
    state: Option<String>,
    decision: Option<RawDecision>,
    gates: Option<BTreeMap<String, Value>>,
    inbox: Option<Vec<RawNote>>,
    out_of_scope: Option<Vec<String>>,
    variants: Option<Vec<Variant>>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct RawDecision {
    color: Option<String>,
    lane: Option<String>,
}

/// What a gate keeps of a decision: each field apart, so an odd one is left out.
#[derive(Default)]
struct RawGate {
    decision: Option<String>,
    by: Option<String>,
    note: Option<String>,
}

impl RawGate {
    /// The gate `value` says, or nothing for a value that is not an object.
    fn of(value: &Value) -> Option<RawGate> {
        let fields = value.as_object()?;
        let text = |key: &str| fields.get(key)?.as_str().map(str::to_string);
        Some(RawGate {
            decision: text("decision"),
            by: text("by"),
            note: text("note"),
        })
    }

    /// The line of a KO a human gave at the gate `name`; nothing for any other decision.
    fn line(&self, name: &str) -> Option<String> {
        if self.decision.as_deref() != Some("ko") || self.by.as_deref() != Some("human") {
            return None;
        }
        let note = self.note.as_deref().unwrap_or_default().trim();
        Some(if note.is_empty() {
            format!("{name} ko")
        } else {
            format!("{name} ko: {note}")
        })
    }
}

/// The line of each KO a human gave at a gate of `gates`, in the order of [`DECIDED_GATES`].
fn human_kos(gates: &BTreeMap<String, Value>) -> Vec<String> {
    let line = |name: &&str| RawGate::of(gates.get(*name)?)?.line(name);
    DECIDED_GATES.iter().filter_map(line).collect()
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct RawNote {
    kind: String,
    text: String,
    resolved: Value,
}

/// The Decision Inbox of change `id`, from the text of its record, the text of its `review`
/// (empty when there is none, `None` when it cannot be read), `asked`, the requests tminus adds
/// first under `Needs your decision` (see [`g1_request`]), and `reported`, the groups from its
/// gate reports (see [`save`]), without a line break at the end: its title, color, lane and
/// state; then how the change works, when the review says it (see [`how_it_works`]), or that the
/// Reviewer did not explain it while the change waits for its G4; then, for a spike with
/// variants, the line of each (see [`Variant::line`]); then, when a human gave a KO at a gate
/// (`plan`, `G1`, `G2`, `G3`, `G4`, in that order), `Your decisions` with `<gate> ko: <note>` for
/// each (`<gate> ko` without a note); then one group per kind of
/// note with the notes still open (`Nothing.` when there are none) and, among the decisions, the
/// paths outside the package the scope check accepted; then each group of `reported` that has
/// entries; then the human's notes at G4, when there are, under `Polish`; last, how many requests
/// were resolved, if any, how many reasons to park a later launch settled, if any, and how many
/// requests were still open when the change ended (state `closed` or `rejected`, whose open
/// requests are not listed), if any. Notes of other kinds are left out.
pub fn render(
    id: &str,
    record: &str,
    review: Option<&str>,
    asked: &[String],
    reported: &[Group],
) -> Result<String, serde_json::Error> {
    let raw: Raw = serde_json::from_str(record)?;
    let title = raw.title.unwrap_or_default();
    let decision = raw.decision.unwrap_or_default();
    let state = raw.state.unwrap_or_else(|| "?".into());
    let decided = match (decision.color, decision.lane) {
        (None, None) => format!("{UNDECIDED} · state {state}"),
        (color, lane) => {
            let color = color.unwrap_or_else(|| "?".into());
            let lane = lane.unwrap_or_else(|| "?".into());
            format!("Color {color} · lane {lane} · state {state}")
        }
    };
    let explained = review.and_then(|review| explained(review, &state));
    let mut groups: [Vec<String>; 3] = Default::default();
    groups[0].extend(asked.iter().cloned());
    let ended = ENDED.contains(&state.as_str());
    let mut polish = Vec::new();
    let (mut resolved, mut settled, mut left) = (0, 0, 0);
    for note in raw.inbox.unwrap_or_default() {
        let group = NOTE_KINDS.iter().position(|kind| *kind == note.kind);
        if ended && group == Some(0) && !is_resolved(&note.resolved) {
            left += 1;
        } else if is_resolved(&note.resolved) {
            if note.kind == PARKED {
                settled += 1;
            } else {
                resolved += 1;
            }
        } else if let Some(group) = group {
            groups[group].push(note.text);
        } else if note.kind == POLISH {
            polish.push(note.text);
        }
    }
    let accepted = raw.out_of_scope.unwrap_or_default();
    if !accepted.is_empty() {
        groups[1].push(format!("{ACCEPTED} {}", accepted.join(", ")));
    }
    let mut blocks = vec![format!("# Decision Inbox · {id} · {title}"), decided];
    blocks.extend(explained);
    let variants = variant_lines(&raw.variants.unwrap_or_default());
    blocks.extend(listed(VARIANTS, &variants));
    blocks.extend(listed(
        YOUR_DECISIONS,
        &human_kos(&raw.gates.unwrap_or_default()),
    ));
    for (heading, notes) in HEADINGS.iter().zip(groups) {
        let mut block = format!("## {heading}");
        if notes.is_empty() {
            block.push_str("\n- Nothing.");
        }
        for note in notes {
            block.push_str(&format!("\n- {note}"));
        }
        blocks.push(block);
    }
    for (heading, lines) in reported {
        blocks.extend(listed(heading, lines));
    }
    blocks.extend(listed("Polish", &polish));
    if resolved > 0 {
        blocks.push(format!("_{resolved} {RESOLVED}_"));
    }
    if settled > 0 {
        blocks.push(format!("_{settled} {SETTLED}_"));
    }
    if left > 0 {
        blocks.push(format!("_{left} {LEFT_OPEN}_"));
    }
    Ok(blocks.join("\n\n"))
}

/// The line of each of `variants`, numbered from 1 (see [`Variant::line`]).
pub fn variant_lines(variants: &[Variant]) -> Vec<String> {
    let line = |(variant, number): (&Variant, usize)| variant.line(number);
    variants.iter().zip(1..).map(line).collect()
}

/// The group `heading` with one item per line of `lines`; nothing without lines.
fn listed(heading: &str, lines: &[String]) -> Option<String> {
    if lines.is_empty() {
        return None;
    }
    let items: Vec<String> = lines.iter().map(|line| format!("- {line}")).collect();
    Some(format!("## {heading}\n{}", items.join("\n")))
}

/// The group that says how the change works, from the text of its `review`: the lines of its
/// section as they are; without them, that the Reviewer did not explain the change, only while
/// the `state` of the change is the one where it waits for its G4.
fn explained(review: &str, state: &str) -> Option<String> {
    let (lines, _) = how_it_works(review);
    if !lines.is_empty() {
        return Some(format!("## {HOW_IT_WORKS}\n{}", lines.join("\n")));
    }
    (state == IN_PRE).then(|| format!("## {HOW_IT_WORKS}\n- {UNEXPLAINED}"))
}

/// The section of the review `text` that says how the change works, and the lines of `text`
/// outside that section and its heading. The section starts at the first line that, trimmed, is
/// one or more `#`, spaces, then `How it works`, in any case; it holds the lines after it up to
/// the next one that starts with `#` or with `VERDICT:`, or the end. Its lines come without the
/// blank ones at their start and end, at most [`MOST_EXPLAINED`]; none without such a heading or
/// with nothing under it.
pub fn how_it_works(text: &str) -> (Vec<&str>, Vec<&str>) {
    let lines: Vec<&str> = text.lines().collect();
    let Some(heading) = lines.iter().position(|line| is_how_it_works(line)) else {
        return (Vec::new(), lines);
    };
    let start = heading + 1;
    let held = lines[start..].iter().position(|line| ends_section(line));
    let end = held.map_or(lines.len(), |held| start + held);
    let mut rest = lines[..heading].to_vec();
    rest.extend_from_slice(&lines[end..]);
    (kept(&lines[start..end]), rest)
}

/// Whether `line`, trimmed, is one or more `#`, spaces, then `How it works`, in any case.
fn is_how_it_works(line: &str) -> bool {
    let Some(marked) = line.trim().strip_prefix('#') else {
        return false;
    };
    let marked = marked.trim_start_matches('#');
    let words = marked.trim_start();
    words.len() < marked.len() && words.eq_ignore_ascii_case(HOW_IT_WORKS)
}

/// Whether `line` ends the section that says how the change works: it starts with `#` or with
/// `VERDICT:`.
fn ends_section(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with('#') || line.starts_with(VERDICT)
}

/// `lines` without the blank ones at their start and end, at most [`MOST_EXPLAINED`].
fn kept<'a>(lines: &[&'a str]) -> Vec<&'a str> {
    let blank = |line: &&str| line.trim().is_empty();
    let filled = lines.iter().copied().skip_while(blank);
    let mut kept: Vec<&str> = filled.take(MOST_EXPLAINED).collect();
    while kept.last().is_some_and(blank) {
        kept.pop();
    }
    kept
}

/// Renders the Decision Inbox of change `id` of the project whose state root is `state` from the
/// text of its record and its review (see [`render`]) and its gate reports: the security
/// exceptions its last G3 named as added, then the commands with known debt its latest G2 and G3
/// found still failing, with what their output keeps, then the ones they found paid (see
/// [`debt::reported`]), then, while the change waits for its G4, the gates whose green proved
/// nothing (see [`unproven_gates`]). It saves it next to the record, with a line break at the
/// end. What it rendered.
pub fn save(state: &Path, id: &str, record: &str) -> Result<String, String> {
    let added = added_exceptions(state, id)?;
    let known = debt::reported(state, id)?;
    let unproven = if waits_for_g4(record) {
        unproven_gates(state, id)?
    } else {
        Vec::new()
    };
    let reported = [
        (EXCEPTIONS, added),
        (KNOWN_DEBT, known.owed.iter().map(still_failing).collect()),
        (DEBT_PAID, known.paid.iter().map(paid_in).collect()),
        (UNPROVEN, unproven),
    ];
    let review = review_of(state, id).map(|review| unlinked(&review, &roots(state, id)));
    let asked: Vec<String> = g1_request(state, id, record).into_iter().collect();
    let path = layout::named(state, &change::record_path(id));
    let rendered = render(id, record, review.as_deref(), &asked, &reported)
        .map_err(|e| format!("{path}: {e}"))?;
    change::write(state, &change::inbox_path(id), &format!("{rendered}\n"))?;
    Ok(rendered)
}

/// Whether the change whose record is `record` waits for the G4 of the human: its state is
/// `in-pre`. A record that is not JSON does not.
fn waits_for_g4(record: &str) -> bool {
    let value: Option<Value> = serde_json::from_str(record).ok();
    value.is_some_and(|value| value["state"] == IN_PRE)
}

/// A line for each gate whose key, in the latest G2 or G3 report of change `id` under the state
/// root `state`, does not count as proof (see [`crate::gates::decl::Key`]): its name and
/// [`NO_PROOF`], each gate once, G2's first. A key that does not say counts as no proof. A report
/// that is missing gives none; one that cannot be read, or is not JSON, is an error that names
/// it.
fn unproven_gates(state: &Path, id: &str) -> Result<Vec<String>, String> {
    #[derive(Deserialize)]
    struct Saved {
        #[serde(default)]
        keys: Vec<Value>,
    }
    let mut gates: Vec<String> = Vec::new();
    for gate in [Gate::G2, Gate::G3] {
        let path = verdict::change_file(state, id, &format!("{}.json", gate.key()));
        let bytes = files::read(state, &path).map_err(|e| format!("cannot read {path}: {e}"))?;
        let Some(bytes) = bytes else {
            continue;
        };
        let saved: Saved = serde_json::from_slice(&bytes).map_err(|e| format!("{path}: {e}"))?;
        for key in saved.keys {
            let name = key["gate"].as_str().unwrap_or("?").to_string();
            if key["proven"] != true && !gates.contains(&name) {
                gates.push(name);
            }
        }
    }
    let lines = gates.iter().map(|gate| format!("{gate}: {NO_PROOF}"));
    Ok(lines.collect())
}

/// The request for G1 of change `id` of the project whose state root is `state`, whose record is
/// `record`: only while the change was never launched (state `open`), when its branch adds or
/// changes scenario files that G1 has not sealed as they are (same files, same SHA-256). What
/// keeps them from being read, in its place. A record that is not JSON asks nothing.
pub fn g1_request(state: &Path, id: &str, record: &str) -> Option<String> {
    let value: Value = serde_json::from_str(record).ok()?;
    if value["state"].as_str() != Some(OPEN) {
        return None;
    }
    match g1_waits(state, id, &value) {
        Ok(false) => None,
        Ok(true) => Some(format!(
            "G1 · Scenario Gate: read the scenarios of change/{id}, then answer your agent (or the \
             sitting screen, tminus sitting {id}): OK approves and seals them, KO with your words \
             sends them back to be rewritten."
        )),
        Err(why) => Some(format!(
            "G1 · Scenario Gate: the scenarios of change/{id} cannot be read: {why}"
        )),
    }
}

/// Whether the branch of change `id` adds or changes scenario files that G1 has not sealed as
/// they are in `record`. A change without a branch, or without such files, waits for nothing.
fn g1_waits(state: &Path, id: &str, record: &Value) -> Result<bool, String> {
    let path = layout::named(state, guard::RULES);
    let rules = match fs::read_to_string(state.join(&path)) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("cannot read {path}: {e}")),
    };
    // The file, and the rules file the error names, as the project names them.
    let in_rules = |e: crate::Error| format!("{path}: {}", e.in_layout(Layout::at(state)));
    let table = RulesTable::of_project(&rules).map_err(in_rules)?;
    let settings = guard::Settings::from_toml(&rules).map_err(in_rules)?;
    let git = Git::new(state);
    let branch = change::branch(id);
    if !git.exists(&branch).map_err(|e| e.to_string())? {
        return Ok(false);
    }
    let mut current = BTreeMap::new();
    for path in git
        .committed_from(&settings.main_branch, &branch)
        .map_err(|e| e.to_string())?
    {
        if !table.acceptance.needs_approval(&path) {
            continue;
        }
        // A file the branch deleted has nothing to seal.
        let Some(bytes) = git.file_at(&branch, &path).map_err(|e| e.to_string())? else {
            continue;
        };
        current.insert(path, sha256(&bytes));
    }
    let sealed = record["gates"]["G1"]["hashes"].as_object();
    let hash = |(path, hash): (&String, &Value)| Some((path.clone(), hash.as_str()?.to_string()));
    let sealed: BTreeMap<String, String> = sealed.into_iter().flatten().filter_map(hash).collect();
    let approved = record["gates"]["G1"]["decision"] == "ok";
    Ok(!current.is_empty() && !(approved && current == sealed))
}

/// The SHA-256 of `bytes`, in lowercase hex.
pub(crate) fn sha256(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(bytes).iter() {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// Where a link of a review of change `id` may point to a file of the project whose state root
/// is `state`: the project folder inside the worktree of the change, then the project itself.
pub fn roots(state: &Path, id: &str) -> Vec<PathBuf> {
    let rules = fs::read_to_string(layout::join(state, guard::RULES)).unwrap_or_default();
    let mut roots = Vec::new();
    if let Ok(folder) = worktree::folder(state, &rules) {
        let tree = folder.join(id);
        roots.push(worktree::project_root(state, &tree).unwrap_or(tree));
    }
    roots.push(state.to_path_buf());
    roots
}

/// `text` with each Markdown link `[label](target)`, with a label and a closing `)`, replaced:
/// by the path of its target from the first of `roots` that holds it (a `:<line>` after the path
/// stays), when the target is an absolute path in one of them; else by its label. A `[` with no
/// `](` after it, and `[](x)`, stay as written.
pub fn unlinked(text: &str, roots: &[PathBuf]) -> String {
    let prefixes: Vec<String> = roots
        .iter()
        .map(|root| format!("{}/", root.display()))
        .collect();
    let mut out = String::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match link_at(after) {
            Some((label, target, tail)) => {
                let inside = prefixes
                    .iter()
                    .find_map(|prefix| target.strip_prefix(prefix.as_str()));
                out.push_str(inside.unwrap_or(label));
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

/// The label, the target and what follows the link whose text starts at `after`, the text after
/// its `[`: none without a label (`[](x)`), a `[` inside the label (the one inside is the link),
/// a `](` or a closing `)`.
fn link_at(after: &str) -> Option<(&str, &str, &str)> {
    let close = after.find("](")?;
    let label = &after[..close];
    if label.is_empty() || label.contains('[') {
        return None;
    }
    let target = &after[close + 2..];
    let end = target.find(')')?;
    Some((label, &target[..end], &target[end + 1..]))
}

/// The text of the review of change `id` of the project whose state root is `state`: empty when
/// there is none, `None` when it cannot be read. Bytes that are not UTF-8 are replaced.
fn review_of(state: &Path, id: &str) -> Option<String> {
    match fs::read(layout::join(state, &verdict::review_path(id))) {
        Ok(bytes) => Some(String::from_utf8_lossy(&bytes).into_owned()),
        Err(e) if e.kind() == ErrorKind::NotFound => Some(String::new()),
        Err(_) => None,
    }
}

/// The entry of a command with known debt still failing in its gate: its name and gate, what it
/// reported against the count of its debt when it has one, then what its output keeps, as a
/// block of code inside the entry.
fn still_failing((gate, owed): &(Gate, debt::Owed)) -> String {
    let lines = owed.tail.lines().map(|line| format!("\n  {line}"));
    let tail: String = lines.collect();
    let counted = match (owed.count, owed.known) {
        (Some(count), Some(known)) => format!(", reports {count} (the known debt is {known})"),
        _ => String::new(),
    };
    let (name, code) = (&owed.name, format!("  ```{tail}\n  ```"));
    format!("{name} ({gate}){counted}, still failing:\n{code}")
}

/// The entry of a command with known debt paid in its gate: its name and gate.
fn paid_in((gate, name): &(Gate, String)) -> String {
    format!("{name} ({gate})")
}

/// The security exceptions the last G3 of change `id` of the project whose state root is `state`
/// named as added, from its report: none before G3 ran.
fn added_exceptions(state: &Path, id: &str) -> Result<Vec<String>, String> {
    #[derive(Deserialize)]
    struct Saved {
        #[serde(default)]
        added_exceptions: Vec<String>,
    }
    let path = layout::named(state, &Gate::G3.report_path(id));
    let text = match fs::read_to_string(state.join(&path)) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("cannot read {path}: {e}")),
    };
    let saved: Saved = serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    Ok(saved.added_exceptions)
}

/// Whether a note was resolved: its `resolved` says when, or says so.
pub(crate) fn is_resolved(resolved: &Value) -> bool {
    match resolved {
        Value::Null | Value::Bool(false) => false,
        Value::String(when) => !when.is_empty(),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::{Folder, Repo};

    const NONE: [Group; 0] = [];

    /// The review of a change that has none.
    const NO_REVIEW: Option<&str> = Some("");

    const RECORD: &str = r#"{
        "id": "c1", "title": "Drafts", "state": "building",
        "decision": {"color": "red", "lane": "full"},
        "inbox": [
            {"kind": "needs", "text": "Old question", "resolved": "2026-01-02"},
            {"kind": "needs", "text": "Pick a name", "ts": "2026-01-03T00:00:00+00:00"},
            {"kind": "decided", "text": "Kept the old format"},
            {"kind": "other", "text": "Not shown"},
            {"kind": "decided", "text": "Took the short path", "resolved": null}
        ],
        "out_of_scope": ["docs/a.md", "docs/b.md"]
    }"#;

    #[test]
    fn the_inbox_groups_the_open_notes_by_kind() {
        let expected = concat!(
            "# Decision Inbox · c1 · Drafts\n\n",
            "Color red · lane full · state building\n\n",
            "## Needs your decision\n- Pick a name\n\n",
            "## Decided without you\n- Kept the old format\n- Took the short path\n",
            "- Paths outside the package, same or lower color, accepted by rule: ",
            "docs/a.md, docs/b.md\n\n",
            "## Parked\n- Nothing.\n\n",
            "_1 earlier request(s) were resolved (kept in change.json)._"
        );
        assert_eq!(
            render("c1", RECORD, NO_REVIEW, &[], &NONE).unwrap(),
            expected
        );
    }

    #[test]
    fn a_record_that_says_little_gets_an_inbox_with_nothing() {
        let expected = concat!(
            "# Decision Inbox · c2 · \n\n",
            "Color and lane not decided yet (tminus decides them at launch) · state ?\n\n",
            "## Needs your decision\n- Nothing.\n\n",
            "## Decided without you\n- Nothing.\n\n",
            "## Parked\n- Nothing."
        );
        assert_eq!(render("c2", "{}", NO_REVIEW, &[], &NONE).unwrap(), expected);
        let one = r#"{"decision": {"color": "red"}}"#;
        let text = render("c2", one, NO_REVIEW, &[], &NONE).unwrap();
        assert!(text.contains("\nColor red · lane ? · state ?\n"), "{text}");
        let one = r#"{"decision": {"lane": "full"}}"#;
        let text = render("c2", one, NO_REVIEW, &[], &NONE).unwrap();
        assert!(text.contains("\nColor ? · lane full · state ?\n"), "{text}");
        let nulls = r#"{"decision": null, "inbox": null, "out_of_scope": null}"#;
        assert_eq!(
            render("c2", nulls, NO_REVIEW, &[], &NONE).unwrap(),
            expected
        );
        let empty = r#"{"decision": {}, "inbox": [], "out_of_scope": []}"#;
        assert_eq!(
            render("c2", empty, NO_REVIEW, &[], &NONE).unwrap(),
            expected
        );
    }

    #[test]
    fn a_spike_with_variants_lists_what_each_did_before_the_notes() {
        let record = r#"{"state": "in-pre", "inbox": [{"kind": "needs", "text": "G4?"}],
            "variants": [
                {"option": "SQLite", "branch": "change/c1-v1", "state": "parked",
                 "reason": "G2 still red", "files": 1, "minutes": 0.5, "usd": 0.1},
                {"option": null, "branch": "change/c1-v2", "state": "ready",
                 "adrs": ["docs/adr/0002-storage.md"], "decision": "Postgres.", "files": 2,
                 "minutes": 1.26, "usd": 0.5},
                {"option": "Files", "branch": "change/c1-v3"}
            ]}"#;
        let text = render("c1", record, Some("VERDICT: OK"), &[], &NONE).unwrap();
        let group = concat!(
            "## How it works\n- The Reviewer did not explain the change.\n\n",
            "## Variants\n",
            "- Variant 1 · SQLite · parked: G2 still red · ADR: none · decision: none · ",
            "1 files · 0.5 min · 0.10 USD\n",
            "- Variant 2 · same brief · ready · ADR: docs/adr/0002-storage.md · ",
            "decision: Postgres. · 2 files · 1.3 min · 0.50 USD\n",
            "- Variant 3 · Files · not built yet\n\n",
            "## Needs your decision\n- G4?"
        );
        assert!(text.contains(group), "{text}");
        assert_eq!(VARIANTS, "Variants");
        for plain in ["{}", r#"{"variants": null}"#, r#"{"variants": []}"#] {
            let text = render("c1", plain, NO_REVIEW, &[], &NONE).unwrap();
            assert!(!text.contains("## Variants"), "{text}");
        }
        let wrong = r#"{"variants": [{"option": "A"}]}"#;
        assert!(render("c1", wrong, NO_REVIEW, &[], &NONE).is_err());
    }

    #[test]
    fn parked_notes_have_their_group() {
        let record = r#"{"inbox": [{"kind": "parked", "text": "Waiting for the API"}]}"#;
        let text = render("c1", record, NO_REVIEW, &[], &NONE).unwrap();
        let end = "## Parked\n- Waiting for the API";
        assert!(text.ends_with(end), "{text}");
    }

    #[test]
    fn a_human_ko_at_a_gate_shows_under_your_decisions_in_the_order_of_the_gates() {
        let record = r#"{"gates": {
            "G4": {"decision": "ko", "by": "human", "note": "  not yet  "},
            "G1": {"decision": "ko", "by": "human", "note": "too vague"},
            "plan": {"decision": "ko", "by": "human"},
            "G2": {"decision": "ko", "by": "machines", "note": "x"},
            "G3": {"decision": "ok", "by": "human", "note": "y"},
            "zones": {"decision": "ko", "by": "human", "note": "z"}
        }}"#;
        let text = render("c1", record, NO_REVIEW, &[], &NONE).unwrap();
        let group = "## Your decisions\n- plan ko\n- G1 ko: too vague\n- G4 ko: not yet\n\n\
                     ## Needs your decision";
        assert!(text.contains(group), "{text}");
        let odd = r#"{"gates": {"G1": "ko", "G2": null, "G3": {"decision": 1, "by": "human"},
            "G4": {"decision": "ko", "by": "human", "note": 3, "extra": true}}}"#;
        let text = render("c1", odd, NO_REVIEW, &[], &NONE).unwrap();
        assert!(text.contains("## Your decisions\n- G4 ko\n\n"), "{text}");
        for plain in ["{}", r#"{"gates": null}"#, r#"{"gates": {}}"#] {
            let text = render("c1", plain, NO_REVIEW, &[], &NONE).unwrap();
            assert!(!text.contains(YOUR_DECISIONS), "{text}");
        }
        assert!(render("c1", r#"{"gates": []}"#, NO_REVIEW, &[], &NONE).is_err());
    }

    #[test]
    fn the_reasons_to_park_a_launch_settled_count_apart_from_the_requests() {
        let record = r#"{"inbox": [
            {"kind": "parked", "text": "Old", "resolved": "2026-01-02"},
            {"kind": "parked", "text": "Older", "resolved": "2026-01-01"},
            {"kind": "needs", "text": "Asked", "resolved": "2026-01-02"},
            {"kind": "parked", "text": "Waiting"}
        ]}"#;
        let text = render("c1", record, NO_REVIEW, &[], &NONE).unwrap();
        let end = concat!(
            "## Parked\n- Waiting\n\n",
            "_1 earlier request(s) were resolved (kept in change.json)._\n\n",
            "_2 earlier reason(s) to park were settled by a later launch (kept in change.json)._"
        );
        assert!(text.ends_with(end), "{text}");
        let only = r#"{"inbox": [{"kind": "parked", "text": "Old", "resolved": "x"}]}"#;
        let text = render("c1", only, NO_REVIEW, &[], &NONE).unwrap();
        assert!(!text.contains("earlier request"), "{text}");
        assert!(text.contains("_1 earlier reason(s) to park"), "{text}");
    }

    #[test]
    fn the_human_notes_at_g4_have_their_group_only_when_there_are() {
        let record = r#"{"inbox": [
            {"kind": "polish", "text": "Shorter errors"},
            {"kind": "parked", "text": "Waiting"},
            {"kind": "polish", "text": "Old", "resolved": "2026-01-02"},
            {"kind": "polish", "text": "Plainer words"}
        ]}"#;
        let text = render("c1", record, NO_REVIEW, &[], &NONE).unwrap();
        let end = concat!(
            "## Parked\n- Waiting\n\n",
            "## Polish\n- Shorter errors\n- Plainer words\n\n",
            "_1 earlier request(s) were resolved (kept in change.json)._"
        );
        assert!(text.ends_with(end), "{text}");
        let text = render("c1", "{}", NO_REVIEW, &[], &NONE).unwrap();
        assert!(!text.contains("Polish"), "{text}");
    }

    #[test]
    fn the_exceptions_the_change_adds_have_their_group_only_when_there_are() {
        let record = r#"{"inbox": [{"kind": "polish", "text": "Shorter errors"}]}"#;
        let line = "exception added: A · deny.toml · no end date · r";
        let added = (EXCEPTIONS, vec![line.to_string()]);
        let text = render("c1", record, NO_REVIEW, &[], &[added, (KNOWN_DEBT, vec![])]).unwrap();
        let end = concat!(
            "## Parked\n- Nothing.\n\n",
            "## Security exceptions this change adds\n",
            "- exception added: A · deny.toml · no end date · r\n\n",
            "## Polish\n- Shorter errors"
        );
        assert!(text.ends_with(end), "{text}");
        let text = render("c1", record, NO_REVIEW, &[], &NONE).unwrap();
        assert!(!text.contains("Security exceptions"), "{text}");
    }

    #[test]
    fn the_saved_inbox_shows_the_known_debt_still_failing_and_paid() {
        let folder = Folder::new();
        let g2 = r#"{"known_debt": [{"name": "lint", "tail": "unused import\nin a.rs\n"},
                     {"name": "types", "tail": "x", "count": 3, "known": 5},
                     {"name": "quality", "tail": "y", "count": 3}],
                     "paid_debt": ["unit"]}"#;
        folder.write(".tminus/changes/c1/g2.json", g2);
        let g3 = r#"{"added_exceptions": ["exception added: A"], "paid_debt": ["security"]}"#;
        folder.write(".tminus/changes/c1/g3.json", g3);
        let saved = save(&folder.0, "c1", "{}").unwrap();
        let end = concat!(
            "## Parked\n- Nothing.\n\n",
            "## Security exceptions this change adds\n- exception added: A\n\n",
            "## Known debt\n- lint (G2), still failing:\n",
            "  ```\n  unused import\n  in a.rs\n  ```\n",
            "- types (G2), reports 3 (the known debt is 5), still failing:\n  ```\n  x\n  ```\n",
            "- quality (G2), still failing:\n  ```\n  y\n  ```\n\n",
            "## Known debt paid\n- unit (G2)\n- security (G3)"
        );
        assert!(saved.ends_with(end), "{saved}");
        folder.write(".tminus/changes/c1/g2.json", "{not json");
        let err = save(&folder.0, "c1", "{}").unwrap_err();
        assert!(err.starts_with(".tminus/changes/c1/g2.json: "), "{err}");
    }

    #[test]
    fn the_saved_inbox_shows_the_exceptions_the_last_g3_named_as_added() {
        let folder = Folder::new();
        let report = ".tminus/changes/c1/g3.json";
        let added = r#"{"ok": true, "added_exceptions": ["exception added: A", "added: B"]}"#;
        folder.write(report, added);
        let saved = save(&folder.0, "c1", "{}").unwrap();
        let group = "## Security exceptions this change adds\n- exception added: A\n- added: B";
        assert!(saved.ends_with(group), "{saved}");
        folder.write(report, r#"{"ok": true}"#);
        let saved = save(&folder.0, "c1", "{}").unwrap();
        assert_eq!(saved, render("c1", "{}", NO_REVIEW, &[], &NONE).unwrap());
        folder.write(report, "{not json");
        let err = save(&folder.0, "c1", "{}").unwrap_err();
        assert!(err.starts_with(".tminus/changes/c1/g3.json: "), "{err}");
        fs::remove_file(folder.0.join(report)).unwrap();
        fs::create_dir_all(folder.0.join(report)).unwrap();
        let err = save(&folder.0, "c1", "{}").unwrap_err();
        let start = "cannot read .tminus/changes/c1/g3.json: ";
        assert!(err.starts_with(start), "{err}");
    }

    #[test]
    fn the_request_of_g4_names_the_gates_whose_green_proved_nothing() {
        let folder = Folder::new();
        let g2 = r#"{"keys": [{"gate": "works", "proven": true},
                     {"gate": "quality", "proven": false}]}"#;
        folder.write(".tminus/changes/c1/g2.json", g2);
        let g3 = r#"{"keys": [{"gate": "works", "proven": true}, {"gate": "quality"},
                     {"gate": "security", "proven": "yes"}]}"#;
        folder.write(".tminus/changes/c1/g3.json", g3);
        let saved = save(&folder.0, "c1", r#"{"state": "in-pre"}"#).unwrap();
        let end = concat!(
            "## Gates not seen to close\n",
            "- quality: not seen to close; its green proves nothing (tminus selftest)\n",
            "- security: not seen to close; its green proves nothing (tminus selftest)"
        );
        assert!(saved.ends_with(end), "{saved}");
        // Only while the change waits for G4.
        for record in [r#"{"state": "building"}"#, "{}"] {
            let saved = save(&folder.0, "c1", record).unwrap();
            assert!(!saved.contains(UNPROVEN), "{saved}");
        }
        // A missing report gives none.
        fs::remove_file(folder.0.join(".tminus/changes/c1/g3.json")).unwrap();
        let saved = save(&folder.0, "c1", r#"{"state": "in-pre"}"#).unwrap();
        let end = "## Gates not seen to close\n- quality: not seen to close; its green proves \
                   nothing (tminus selftest)";
        assert!(saved.ends_with(end), "{saved}");
        fs::remove_file(folder.0.join(".tminus/changes/c1/g2.json")).unwrap();
        let saved = save(&folder.0, "c1", r#"{"state": "in-pre"}"#).unwrap();
        assert!(!saved.contains(UNPROVEN), "{saved}");
    }

    #[test]
    fn a_report_that_cannot_be_read_is_named_when_looking_for_the_gates_never_seen_to_close() {
        let folder = Folder::new();
        assert_eq!(unproven_gates(&folder.0, "c1"), Ok(vec![]));
        let report = ".tminus/changes/c1/g2.json";
        folder.write(report, "{not json");
        let err = unproven_gates(&folder.0, "c1").unwrap_err();
        assert!(err.starts_with(".tminus/changes/c1/g2.json: "), "{err}");
        fs::remove_file(folder.0.join(report)).unwrap();
        fs::create_dir_all(folder.0.join(report)).unwrap();
        let err = unproven_gates(&folder.0, "c1").unwrap_err();
        let start = "cannot read .tminus/changes/c1/g2.json: ";
        assert!(err.starts_with(start), "{err}");
        let err = save(&folder.0, "c1", r#"{"state": "in-pre"}"#).unwrap_err();
        assert!(err.starts_with(start), "{err}");
    }

    #[test]
    fn a_change_waits_for_g4_only_in_pre() {
        assert!(waits_for_g4(r#"{"state": "in-pre"}"#));
        let building = r#"{"state": "building"}"#;
        for record in [building, r#"{"state": 3}"#, "{}", "{not json"] {
            assert!(!waits_for_g4(record), "{record}");
        }
    }

    /// A project of the old layout: `gdd/project.toml`, its files in `gdd/` and its state in
    /// `.gdd/`.
    fn old_layout() -> Folder {
        let folder = Folder::new();
        folder.write("gdd/project.toml", "name = \"x\"\n");
        folder
    }

    #[test]
    fn the_inbox_of_a_project_of_the_old_layout_reads_and_writes_its_gdd_folder() {
        let folder = old_layout();
        let added = r#"{"added_exceptions": ["exception added: A"]}"#;
        folder.write(".gdd/changes/c1/g3.json", added);
        let review = "## How it works\nOne pure function.\nVERDICT: OK\n";
        folder.write(".gdd/changes/c1/review.md", review);
        let saved = save(&folder.0, "c1", r#"{"state": "in-pre"}"#).unwrap();
        assert!(saved.contains("\n- exception added: A"), "{saved}");
        let explained = "## How it works\nOne pure function.\n";
        assert!(saved.contains(explained), "{saved}");
        let text = fs::read_to_string(folder.0.join(".gdd/changes/c1/inbox.md")).unwrap();
        assert_eq!(text, format!("{saved}\n"));
        let err = save(&folder.0, "c1", "{not json").unwrap_err();
        assert!(err.starts_with(".gdd/changes/c1/change.json: "), "{err}");
        folder.write(".gdd/changes/c1/g3.json", "{not json");
        let err = save(&folder.0, "c1", "{}").unwrap_err();
        assert!(err.starts_with(".gdd/changes/c1/g3.json: "), "{err}");
        fs::remove_file(folder.0.join(".gdd/changes/c1/g3.json")).unwrap();
        fs::create_dir_all(folder.0.join(".gdd/changes/c1/g3.json")).unwrap();
        let err = save(&folder.0, "c1", "{}").unwrap_err();
        let start = "cannot read .gdd/changes/c1/g3.json: ";
        assert!(err.starts_with(start), "{err}");
        assert!(!folder.0.join(".tminus").exists());
    }

    #[test]
    fn the_rules_of_a_project_of_the_old_layout_are_in_its_gdd_folder() {
        let folder = old_layout();
        // The worktree folder the rules name, and only those of gdd/.
        folder.write("tminus/rules.toml", "[parallel]\nfolder = \"/other\"\n");
        folder.write("gdd/rules.toml", "[parallel]\nfolder = \"/trees\"\n");
        let trees = [PathBuf::from("/trees/c1"), folder.0.clone()];
        assert_eq!(roots(&folder.0, "c1"), trees);
        let on_branch = [("features/a.feature", Some(SCENARIO))];
        let repo = repo_with(&[("gdd/project.toml", "")], &on_branch);
        let record = record_of("open", "none", &[]);
        fs::create_dir_all(repo.root().join("gdd/rules.toml")).unwrap();
        let text = g1_request(repo.root(), "c1", &record).unwrap();
        let start = "G1 · Scenario Gate: the scenarios of change/c1 cannot be read: cannot read \
                     gdd/rules.toml: ";
        assert!(text.starts_with(start), "{text}");
    }

    #[test]
    fn the_section_starts_at_a_how_it_works_heading_of_any_form() {
        let headings = [
            "## How it works",
            "# how it works",
            "###   HOW IT WORKS   ",
            "  ## How It Works",
            "#\tHow it works",
        ];
        let expected = (vec!["One line."], vec!["Fine."]);
        for heading in headings {
            let text = format!("Fine.\n{heading}\nOne line.\n");
            assert_eq!(how_it_works(&text), expected, "{text:?}");
        }
        let not_headings = [
            "How it works",
            "##How it works",
            "## How it works:",
            "## How it works now",
            "- ## How it works",
            "**How it works**",
        ];
        for line in not_headings {
            let text = format!("{line}\nOne line.");
            let (section, rest) = how_it_works(&text);
            assert!(section.is_empty(), "{text:?}");
            assert_eq!(rest, vec![line, "One line."]);
        }
    }

    #[test]
    fn the_section_ends_at_the_next_heading_or_the_verdict() {
        let text = "Intro\n## How it works\none\ntwo\nthree\n### Notes\nfour\nVERDICT: OK\n";
        let rest = vec!["Intro", "### Notes", "four", "VERDICT: OK"];
        assert_eq!(how_it_works(text), (vec!["one", "two", "three"], rest));
        let text = "## How it works\none\ntwo\n  #x\nthree";
        let rest = vec!["  #x", "three"];
        assert_eq!(how_it_works(text), (vec!["one", "two"], rest));
        let text = "## How it works\none\n  VERDICT: OK\n";
        assert_eq!(how_it_works(text), (vec!["one"], vec!["  VERDICT: OK"]));
        let text = "Fine.\n## How it works\none\ntwo";
        assert_eq!(how_it_works(text), (vec!["one", "two"], vec!["Fine."]));
    }

    #[test]
    fn blank_lines_at_the_start_and_end_of_the_section_are_dropped() {
        let text = "## How it works\n\n  \none\n\n  two\n \n\nVERDICT: OK";
        let section = vec!["one", "", "  two"];
        assert_eq!(how_it_works(text), (section, vec!["VERDICT: OK"]));
    }

    #[test]
    fn the_section_keeps_twelve_lines_at_most() {
        let lines: Vec<String> = (1..=20).map(|n| format!("line {n}")).collect();
        let first: Vec<&str> = lines.iter().map(String::as_str).collect();
        let text = format!("## How it works\n{}\nVERDICT: OK", lines.join("\n"));
        let (section, rest) = how_it_works(&text);
        assert_eq!(section, first[..12]);
        assert_eq!(rest, vec!["VERDICT: OK"]);
        let text = format!("## How it works\n\n{}\n \nline 13", first[..11].join("\n"));
        let (section, _) = how_it_works(&text);
        assert_eq!(section, first[..11]);
    }

    #[test]
    fn a_review_without_a_section_or_with_an_empty_one_explains_nothing() {
        assert_eq!(how_it_works(""), (vec![], vec![]));
        let text = "Fine.\nVERDICT: OK\n";
        assert_eq!(how_it_works(text), (vec![], vec!["Fine.", "VERDICT: OK"]));
        let text = "Fine.\n## How it works\n\n \n## Notes\nVERDICT: OK\n";
        let rest = vec!["Fine.", "## Notes", "VERDICT: OK"];
        assert_eq!(how_it_works(text), (vec![], rest));
    }

    #[test]
    fn the_inbox_says_how_the_change_works_right_after_its_title() {
        let review = "Fine.\n## How it works\nThe export reads each note.\n  - As it is\n\n\
                      VERDICT: OK\n";
        for state in [IN_PRE, "building", "closed"] {
            let record = format!(r#"{{"state": "{state}"}}"#);
            let text = render("c1", &record, Some(review), &[], &NONE).unwrap();
            let start = format!(
                "# Decision Inbox · c1 · \n\nColor and lane not decided yet (tminus decides them \
                 at launch) · state {state}\n\n\
                 ## How it works\nThe export reads each note.\n  - As it is\n\n\
                 ## Needs your decision\n"
            );
            assert!(text.starts_with(&start), "{text}");
        }
    }

    #[test]
    fn without_a_section_the_inbox_says_so_only_while_the_change_waits_for_its_g4() {
        let review = "Fine.\nVERDICT: OK\n";
        let waiting = r#"{"state": "in-pre"}"#;
        let group = format!("in-pre\n\n## How it works\n- {UNEXPLAINED}\n\n## Needs");
        for said in [Some(review), NO_REVIEW] {
            let text = render("c1", waiting, said, &[], &NONE).unwrap();
            assert!(text.contains(&group), "{text}");
        }
        for record in [r#"{"state": "building"}"#, r#"{"state": "closed"}"#, "{}"] {
            let text = render("c1", record, Some(review), &[], &NONE).unwrap();
            assert!(!text.contains(HOW_IT_WORKS), "{text}");
        }
        let text = render("c1", waiting, None, &[], &NONE).unwrap();
        assert!(!text.contains(HOW_IT_WORKS), "{text}");
    }

    #[test]
    fn the_saved_inbox_reads_the_review_and_leaves_out_one_that_cannot_be_read() {
        let folder = Folder::new();
        let record = r#"{"state": "in-pre"}"#;
        let saved = save(&folder.0, "c1", record).unwrap();
        assert!(saved.contains(UNEXPLAINED), "{saved}");
        let review = ".tminus/changes/c1/review.md";
        folder.write(review, "## How it works\nOne pure function.\nVERDICT: OK\n");
        let saved = save(&folder.0, "c1", record).unwrap();
        let group = "## How it works\nOne pure function.\n\n";
        assert!(saved.contains(group), "{saved}");
        fs::write(folder.0.join(review), b"## How it works\n\xff bytes\n").unwrap();
        let saved = save(&folder.0, "c1", record).unwrap();
        let group = "## How it works\n\u{fffd} bytes\n\n";
        assert!(saved.contains(group), "{saved}");
        fs::remove_file(folder.0.join(review)).unwrap();
        fs::create_dir_all(folder.0.join(review)).unwrap();
        let saved = save(&folder.0, "c1", record).unwrap();
        assert_eq!(saved, render("c1", record, None, &[], &NONE).unwrap());
        assert!(!saved.contains(HOW_IT_WORKS), "{saved}");
    }

    #[test]
    fn a_note_is_resolved_when_its_resolved_says_when_or_says_so() {
        for open in [Value::Null, Value::Bool(false), Value::from("")] {
            assert!(!is_resolved(&open), "{open}");
        }
        for done in [Value::from("x"), Value::Bool(true), Value::from(1)] {
            assert!(is_resolved(&done), "{done}");
        }
    }

    #[test]
    fn the_inbox_is_saved_next_to_the_record_as_it_is_rendered() {
        let folder = Folder::new();
        let saved = save(&folder.0, "c1", RECORD).unwrap();
        assert_eq!(saved, render("c1", RECORD, NO_REVIEW, &[], &NONE).unwrap());
        let text = fs::read_to_string(folder.0.join(".tminus/changes/c1/inbox.md")).unwrap();
        assert_eq!(text, format!("{saved}\n"));
        let err = save(&folder.0, "c1", "{not json").unwrap_err();
        let start = ".tminus/changes/c1/change.json: ";
        assert!(err.starts_with(start), "{err}");
        fs::remove_file(folder.0.join(".tminus/changes/c1/inbox.md")).unwrap();
        fs::create_dir_all(folder.0.join(".tminus/changes/c1/inbox.md")).unwrap();
        let err = save(&folder.0, "c1", RECORD).unwrap_err();
        let start = "cannot write .tminus/changes/c1/inbox.md: ";
        assert!(err.starts_with(start), "{err}");
    }

    #[test]
    fn a_record_that_is_not_valid_is_an_error() {
        for record in [
            "{not json",
            "3",
            r#"{"inbox": [{"text": 3}]}"#,
            r#"{"title": 1}"#,
        ] {
            assert!(
                render("c1", record, NO_REVIEW, &[], &NONE).is_err(),
                "{record}"
            );
        }
    }

    const NEEDS: &str = r#""inbox": [
        {"kind": "needs", "text": "Pick"},
        {"kind": "needs", "text": "Old", "resolved": "x"},
        {"kind": "needs", "text": "Two"},
        {"kind": "decided", "text": "Kept"}
    ]"#;

    #[test]
    fn the_requests_of_a_change_that_ended_are_not_listed_but_counted() {
        for state in ["closed", "rejected"] {
            let record = format!(r#"{{"state": "{state}", {NEEDS}}}"#);
            let text = render("c1", &record, NO_REVIEW, &[], &NONE).unwrap();
            let group = "## Needs your decision\n- Nothing.\n\n## Decided without you\n- Kept\n";
            assert!(text.contains(group), "{text}");
            let end = concat!(
                "_1 earlier request(s) were resolved (kept in change.json)._\n\n",
                "_2 request(s) were still open when the change ended (kept in change.json)._"
            );
            assert!(text.ends_with(end), "{text}");
        }
        for state in ["rolled-back", "building"] {
            let record = format!(r#"{{"state": "{state}", {NEEDS}}}"#);
            let text = render("c1", &record, NO_REVIEW, &[], &NONE).unwrap();
            assert!(
                text.contains("## Needs your decision\n- Pick\n- Two\n\n"),
                "{text}"
            );
            assert!(!text.contains("still open when"), "{text}");
        }
        let parked = r#"{"state": "closed", "inbox": [{"kind": "parked", "text": "Waiting"}]}"#;
        let text = render("c1", parked, NO_REVIEW, &[], &NONE).unwrap();
        assert!(text.contains("## Parked\n- Waiting"), "{text}");
    }

    #[test]
    fn the_requests_tminus_adds_come_first() {
        let record = format!(r#"{{"state": "open", {NEEDS}}}"#);
        let asked = ["Read it".to_string()];
        let text = render("c1", &record, NO_REVIEW, &asked, &NONE).unwrap();
        assert!(
            text.contains("## Needs your decision\n- Read it\n- Pick\n- Two\n"),
            "{text}"
        );
    }

    const SCENARIO: &str = "Feature: A\n";

    /// A repository on `main` with the files `on_main`, and the branch of change `c1` with the
    /// files `on_branch`, a text of `None` deleting the file; back on `main`.
    fn repo_with(on_main: &[(&str, &str)], on_branch: &[(&str, Option<&str>)]) -> Repo {
        let repo = Repo::new();
        for (path, text) in on_main {
            repo.write(path, text);
        }
        repo.commit();
        repo.start("c1");
        for (path, text) in on_branch {
            match text {
                Some(text) => repo.write(path, text),
                None => fs::remove_file(repo.root().join(path)).unwrap(),
            }
        }
        repo.commit();
        repo.git(&["checkout", "-q", "main"]);
        repo
    }

    /// The record of a change in `state` whose G1 says `decision` for the files `sealed`.
    fn record_of(state: &str, decision: &str, sealed: &[(&str, &str)]) -> String {
        let hashes: serde_json::Map<String, Value> = sealed
            .iter()
            .map(|(path, text)| (path.to_string(), Value::from(sha256(text.as_bytes()))))
            .collect();
        let g1 = serde_json::json!({"decision": decision, "hashes": hashes});
        serde_json::json!({"state": state, "gates": {"G1": g1}}).to_string()
    }

    fn asks_g1(repo: &Repo, record: &str) -> bool {
        let asked = g1_request(repo.root(), "c1", record).unwrap_or_default();
        asked.contains("read the scenarios")
    }

    #[test]
    fn g1_is_asked_for_the_scenarios_it_has_not_sealed_as_they_are() {
        let on_branch = [("features/a.feature", Some(SCENARIO))];
        let repo = repo_with(&[], &on_branch);
        let sealed = [("features/a.feature", SCENARIO)];
        let text = g1_request(repo.root(), "c1", &record_of("open", "none", &[])).unwrap();
        let expected = "G1 · Scenario Gate: read the scenarios of change/c1, then answer your \
                        agent (or the sitting screen, tminus sitting c1): OK approves and seals \
                        them, KO with your words sends them back to be rewritten.";
        assert_eq!(text, expected);
        assert!(!asks_g1(&repo, &record_of("open", "ok", &sealed)));
        assert!(asks_g1(&repo, &record_of("open", "ko", &sealed)));
        let other = [("features/a.feature", "Other")];
        assert!(asks_g1(&repo, &record_of("open", "ok", &other)));
        let more = [
            ("features/a.feature", SCENARIO),
            ("features/b.feature", SCENARIO),
        ];
        assert!(asks_g1(&repo, &record_of("open", "ok", &more)));
        assert!(asks_g1(&repo, r#"{"state": "open"}"#));
    }

    #[test]
    fn g1_is_asked_only_of_a_change_that_was_never_launched() {
        let on_branch = [("features/a.feature", Some(SCENARIO))];
        let repo = repo_with(&[], &on_branch);
        for state in ["building", "in-pre", "closed"] {
            let record = record_of(state, "none", &[]);
            assert_eq!(g1_request(repo.root(), "c1", &record), None);
        }
        assert_eq!(g1_request(repo.root(), "c1", r#"{"gates": {}}"#), None);
        assert_eq!(g1_request(repo.root(), "c1", "{not json"), None);
    }

    #[test]
    fn g1_waits_for_nothing_without_a_branch_or_scenarios_on_it() {
        let record = record_of("open", "none", &[]);
        assert_eq!(g1_request(Repo::new().root(), "c1", &record), None);
        let code = ("src/a.rs", Some("fn a() {}\n"));
        let regression = ("features/regression/r.feature", Some(SCENARIO));
        let repo = repo_with(&[], &[code, regression]);
        assert_eq!(g1_request(repo.root(), "c1", &record), None);
        let old = [("features/old.feature", SCENARIO)];
        let repo = repo_with(&old, &[("features/old.feature", None)]);
        assert_eq!(g1_request(repo.root(), "c1", &record), None);
    }

    #[test]
    fn g1_says_what_keeps_the_scenarios_from_being_read() {
        let on_branch = [("features/a.feature", Some(SCENARIO))];
        let repo = repo_with(&[], &on_branch);
        let record = record_of("open", "none", &[]);
        repo.write("tminus/rules.toml", "[acceptance]\ndir = \"specs\"\n");
        assert_eq!(g1_request(repo.root(), "c1", &record), None);
        fs::write(repo.root().join("tminus/rules.toml"), "[acceptance").unwrap();
        let text = g1_request(repo.root(), "c1", &record).unwrap();
        let start = "G1 · Scenario Gate: the scenarios of change/c1 cannot be read: ";
        assert!(text.starts_with(start), "{text}");
        fs::remove_file(repo.root().join("tminus/rules.toml")).unwrap();
        fs::create_dir_all(repo.root().join("tminus/rules.toml")).unwrap();
        let text = g1_request(repo.root(), "c1", &record).unwrap();
        let start = "G1 · Scenario Gate: the scenarios of change/c1 cannot be read: cannot read \
                     tminus/rules.toml: ";
        assert!(text.starts_with(start), "{text}");
    }

    #[test]
    fn a_rules_file_that_switches_off_the_core_is_named_as_the_project_names_it() {
        let on_branch = [("features/a.feature", Some(SCENARIO))];
        let record = record_of("open", "none", &[]);
        let core = "[color.red]\nreviewer = false\n";
        let cannot = "G1 · Scenario Gate: the scenarios of change/c1 cannot be read: ";
        let remove = "remove color.red.reviewer = false from";
        for file in ["tminus", "gdd"] {
            let project = format!("{file}/project.toml");
            let repo = repo_with(&[(project.as_str(), "")], &on_branch);
            repo.write(&format!("{file}/rules.toml"), core);
            let text = g1_request(repo.root(), "c1", &record).unwrap();
            let start = format!("{cannot}{file}/rules.toml: red changes always get");
            assert!(text.starts_with(&start), "{text}");
            assert!(
                text.ends_with(&format!("{remove} {file}/rules.toml")),
                "{text}"
            );
            repo.write(&format!("{file}/rules.toml"), "main_branch = 1\n");
            let text = g1_request(repo.root(), "c1", &record).unwrap();
            let start = format!("{cannot}{file}/rules.toml: invalid TOML: ");
            assert!(text.starts_with(&start), "{text}");
        }
    }

    #[test]
    fn the_request_for_g1_is_saved_first_among_the_needs() {
        let on_branch = [("features/a.feature", Some(SCENARIO))];
        let repo = repo_with(&[], &on_branch);
        let record = r#"{"state": "open", "inbox": [{"kind": "needs", "text": "Pick"}]}"#;
        let saved = save(repo.root(), "c1", record).unwrap();
        let group = "## Needs your decision\n- G1 · Scenario Gate: read the scenarios of change/c1";
        assert!(saved.contains(group), "{saved}");
        assert!(saved.contains(".\n- Pick\n"), "{saved}");
    }

    fn roots_of(paths: &[&str]) -> Vec<PathBuf> {
        paths.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn a_link_inside_the_project_becomes_its_path_and_any_other_its_label() {
        let roots = roots_of(&["/w/c1", "/p"]);
        let cases = [
            ("[a.rs:3](/w/c1/src/a.rs:3): x", "src/a.rs:3: x"),
            ("see [the doc](/p/docs/a.md) now", "see docs/a.md now"),
            ("[a](/other/a.md)", "a"),
            ("[a](/w/c1x/a.md)", "a"),
            ("[a](docs/a.md)", "a"),
            ("[a](/w/c1)", "a"),
            ("[a](/p/x) and [b](/w/c1/y) and [c](z)", "x and y and c"),
            ("[a [b](/p/c)", "[a c"),
            ("[](x) [a](/p/y)", "[](x) y"),
            ("[a](x", "[a](x"),
            ("[a] (x) [b", "[a] (x) [b"),
            ("no links", "no links"),
            ("", ""),
        ];
        for (text, expected) in cases {
            assert_eq!(unlinked(text, &roots), expected, "{text}");
        }
        assert_eq!(unlinked("[a](/p/x)", &[]), "a");
        let nested = roots_of(&["/p/wt/c1", "/p"]);
        assert_eq!(unlinked("[a](/p/wt/c1/x) [b](/p/wt/x)", &nested), "x wt/x");
    }

    #[test]
    fn links_are_looked_for_in_the_worktree_then_in_the_project() {
        let folder = Folder::new();
        let tree = worktree::folder(&folder.0, "").unwrap().join("c1");
        assert_eq!(roots(&folder.0, "c1"), [tree, folder.0.clone()]);
        folder.write("tminus/rules.toml", "[parallel]\nfolder = \"/trees\"\n");
        let trees = [PathBuf::from("/trees/c1"), folder.0.clone()];
        assert_eq!(roots(&folder.0, "c1"), trees);
        folder.write("tminus/rules.toml", "[parallel");
        assert_eq!(roots(&folder.0, "c1"), std::slice::from_ref(&folder.0));
    }

    #[test]
    fn the_review_is_saved_with_its_links_as_project_paths() {
        let folder = Folder::new();
        let tree = worktree::folder(&folder.0, "").unwrap().join("c1");
        let review = format!(
            "## How it works\n- [a.rs:3]({}/src/a.rs:3) checks it\n\nVERDICT: OK\n",
            tree.display()
        );
        folder.write(".tminus/changes/c1/review.md", &review);
        let saved = save(&folder.0, "c1", r#"{"state": "in-pre"}"#).unwrap();
        let shown = "## How it works\n- src/a.rs:3 checks it\n";
        assert!(saved.contains(shown), "{saved}");
    }
}
