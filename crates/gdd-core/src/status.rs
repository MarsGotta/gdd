//! The status of a project: where it stands in the method, its security exceptions, its known
//! debt, its changes, its log and, when it accepted one model family only, the agents of another
//! family on the machine.

use std::fmt;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::Path;

use serde_json::{Map, Value};

use crate::agents::models::{self, Models};
use crate::change::Record;
use crate::classify::{Change, classify};
use crate::config::{self, Color};
use crate::gates::approve::{BRIEF, ZonesState};
use crate::gates::debt::{self, Debt, Listed};
use crate::gates::{COMMANDS, Commands, Gate};
use crate::rules::RulesTable;
use crate::zones::ZoneMap;
use crate::{guard, init, log};

/// Where the changes keep their records, from the state root.
const CHANGES: &str = ".gdd/changes";

/// The record of a change, from its folder.
const RECORD: &str = "change.json";

/// The zone map of the project, from its root.
const ZONES: &str = "gdd/zones.toml";

/// What the status says of a missing Project Brief.
const NO_BRIEF: &str = "missing (Setup writes it)";

/// What the status says of G0 once the machines opened it.
const G0_OK: &str = "ok · G0 ok";

/// What the status says of G0 until the machines open it.
const G0_PENDING: &str = "pending · G0 pending: run the Setup (the gdd-setup command).";

/// What the status says when an agent of another model family could review a project that
/// accepted one family only.
const SECOND_FAMILY: &str = "A second model family is on this machine";

/// What the status says of known debt without a count in some gate, after its counts.
const UNSEEN: &str = "a change can add to it unseen";

/// What the status says when the gate commands have no mutation testing for red changes.
const NO_MUTATION: &str =
    "no mutation testing command in [g3] of gdd/commands.toml: red changes go without it";

/// What `gdd status` says of a project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    /// Its name, language and platform.
    pub basics: Basics,
    /// Whether its Project Brief is written.
    pub brief: bool,
    /// Whether the machines opened G0.
    pub g0: bool,
    /// Whether a human approved its red zones as they are.
    pub zones: ZonesState,
    /// What its last harness self-test said; `None` when it never ran.
    pub selftest: Option<String>,
    /// The folder of its scenarios.
    pub scenarios_dir: String,
    /// How many scenarios it has, or why they could not be counted.
    pub scenarios: Result<usize, String>,
    /// How each of its security exceptions shows (see [`Exception::line`]), or why they could
    /// not be read.
    ///
    /// [`Exception::line`]: crate::gates::exceptions::Exception::line
    pub exceptions: Result<Vec<String>, String>,
    /// Its known debt (see [`debt`]), or why it could not be read.
    pub debt: Result<Vec<Debt>, String>,
    /// Whether its red changes go without mutation testing (see [`without_mutation`]).
    pub without_mutation: bool,
    /// One line per change (see [`change_line`]).
    pub changes: Vec<String>,
    /// How many lines its log has; `None` when there is no log.
    pub log_lines: Option<usize>,
    /// The coding agents on the machine that could review it (see [`second_family`]).
    pub second_family: Vec<String>,
}

impl fmt::Display for Status {
    /// One line per fact and one per change, without a line break at the end.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let basics = &self.basics;
        write!(f, "Project: {}", basics.name)?;
        write!(f, " · language {}", basics.language)?;
        writeln!(f, " · platform {}", basics.platform)?;
        let brief = if self.brief { BRIEF } else { NO_BRIEF };
        writeln!(f, "Brief: {brief}")?;
        let g0 = if self.g0 { G0_OK } else { G0_PENDING };
        writeln!(f, "G0: {g0}")?;
        let zones = if self.zones.approved() {
            "approved"
        } else {
            "pending"
        };
        writeln!(f, "Red zones: {zones} · {}", self.zones.message())?;
        let selftest = self.selftest.as_deref().unwrap_or("not run");
        writeln!(f, "Harness self-test: {selftest}")?;
        write!(f, "Scenarios in {}/: ", self.scenarios_dir)?;
        match &self.scenarios {
            Ok(count) => writeln!(f, "{count}")?,
            Err(why) => writeln!(f, "cannot count them: {why}")?,
        }
        match &self.exceptions {
            Ok(lines) if lines.is_empty() => writeln!(f, "Security exceptions: none")?,
            Ok(lines) => {
                writeln!(f, "Security exceptions: {}", lines.len())?;
                for line in lines {
                    writeln!(f, "  - {line}")?;
                }
            }
            Err(why) => writeln!(f, "Security exceptions: {why}")?,
        }
        known_debt(f, &self.debt)?;
        if self.without_mutation {
            writeln!(f, "{NO_MUTATION}")?;
        }
        write!(f, "Changes:")?;
        for line in &self.changes {
            write!(f, "\n{line}")?;
        }
        if let Some(lines) = self.log_lines {
            write!(f, "\nLog: {lines} lines in {} (gdd report)", log::PATH)?;
        }
        if !self.second_family.is_empty() {
            let agents = self.second_family.join(", ");
            let reviewer = format!("set it as the Reviewer in {}", models::PATH);
            write!(f, "\n{SECOND_FAMILY} ({agents}): {reviewer}")?;
        }
        Ok(())
    }
}

/// The coding agents `found` on the machine whose strong model (see [`init::strong_model`]) is
/// of another model family than the red Builder's in `models`, when the human accepted a Reviewer
/// of the Builder's family: any of them could review. None otherwise.
pub fn second_family(models: &Models, found: &[String]) -> Vec<String> {
    let roles = models.roles(Color::Red);
    let accepted = models::same_family_accepted(&roles.reviewer_model);
    if accepted.is_none() {
        return Vec::new();
    }
    let builder = models.family(&roles.builder);
    let other = |name: &&String| {
        let model = init::strong_model(name);
        model.is_some_and(|model| models.family(&model) != builder)
    };
    found.iter().filter(other).cloned().collect()
}

/// Whether the red changes of the project at `root` go without mutation testing: `[g3]` of its
/// gate commands has no command whose name starts with `mutation`. A commands file that is not
/// there or cannot be read says nothing.
pub fn without_mutation(root: &Path) -> bool {
    let Ok(text) = fs::read_to_string(root.join(COMMANDS)) else {
        return false;
    };
    let Ok(commands) = Commands::from_toml(&text) else {
        return false;
    };
    let mut names = commands.of(Gate::G3).keys();
    !names.any(|name| name.starts_with("mutation"))
}

/// The lines of the known debt `known` in the status: `Known debt: ` and the names of its
/// commands, riskiest first, each with its counts (see [`Listed::said`]) and, when a gate holds
/// it to no count, [`UNSEEN`]; then a line per command with the day it was recorded and the
/// Refactor that pays it; `none` without debt, or why it could not be read.
fn known_debt(f: &mut fmt::Formatter<'_>, known: &Result<Vec<Debt>, String>) -> fmt::Result {
    let debts = match known {
        Ok(debts) if debts.is_empty() => return writeln!(f, "Known debt: none"),
        Ok(debts) => debt::listed(debts),
        Err(why) => return writeln!(f, "Known debt: {why}"),
    };
    let counted = |listed: &Listed| {
        let (command, said) = (listed.command, listed.said());
        if listed.uncounted() {
            format!("{command} ({said}: {UNSEEN})")
        } else {
            format!("{command} ({said})")
        }
    };
    let names: Vec<String> = debts.iter().map(counted).collect();
    writeln!(f, "Known debt: {}", names.join(", "))?;
    for Listed { command, since, .. } in debts {
        writeln!(f, "  - {command} · since {since} · Refactor debt-{command}")?;
    }
    Ok(())
}

/// The name, language and platform of a project, from `gdd/project.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Basics {
    /// Its name; `?` when the file does not say.
    pub name: String,
    /// The language the agents use with the human; `en` when the file does not say.
    pub language: String,
    /// Its platform; `?` when the file does not say.
    pub platform: String,
}

impl Basics {
    /// Reads them from the text of `gdd/project.toml`; one the file leaves out, or does not
    /// give as text, takes its default.
    pub fn from_toml(text: &str) -> Result<Basics, config::Error> {
        let table: toml::Table =
            toml::from_str(text).map_err(|e| config::Error::Toml(e.to_string()))?;
        let field = |key: &str, default: &str| {
            let value = table.get(key).and_then(toml::Value::as_str);
            value.unwrap_or(default).to_string()
        };
        Ok(Basics {
            name: field("name", "?"),
            language: field("language", "en"),
            platform: field("platform", "?"),
        })
    }
}

/// The line of a change in the status, from the name of its folder and the text of its record:
/// its id, title, state and gates with their decisions; or why its record cannot be read.
pub fn change_line(folder: &str, record: &str) -> String {
    let record: Map<String, Value> = match serde_json::from_str(record) {
        Ok(record) => record,
        Err(e) => return format!("  - {folder} · {}", not_valid(&e)),
    };
    let id = text_field(&record, "id", folder);
    let title = text_field(&record, "title", "");
    let state = text_field(&record, "state", "open");
    let gates = gates_text(&record);
    format!("  - {id} · {title} · {state} · {gates}")
}

/// Why a record that cannot be read is shown as it is.
fn not_valid(why: &dyn fmt::Display) -> String {
    format!("change.json is not valid: {why}")
}

/// The text field `key` of a record; `default` when it is missing or not text.
fn text_field(record: &Map<String, Value>, key: &str, default: &str) -> String {
    let value = record.get(key).and_then(Value::as_str);
    value.unwrap_or(default).to_string()
}

/// The gates of a record as the status shows them, `G1:ok, plan:ok`; `no gates yet` without any.
fn gates_text(record: &Map<String, Value>) -> String {
    let gates: Vec<String> = match record.get("gates").and_then(Value::as_object) {
        Some(gates) => gates.iter().map(gate_text).collect(),
        None => Vec::new(),
    };
    if gates.is_empty() {
        "no gates yet".to_string()
    } else {
        gates.join(", ")
    }
}

/// A change as the Board shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    /// Its id; the name of its folder when its record cannot be read.
    pub id: String,
    /// What it is about.
    pub title: String,
    /// Its kind of work: `Feature`, `Spike`...
    pub kind: String,
    /// Where it stands: `open`, `building`, `in-pre`, `closed`...
    pub state: String,
    /// Its risk color: the one prepare stored, else the one its planned paths give; `?` when the
    /// project's zones cannot be read.
    pub color: String,
    /// Its gates and their decisions, like the status shows them.
    pub gates: String,
    /// The paths it plans to touch.
    pub paths: Vec<String>,
    /// Why its record cannot be read; `None` when it can.
    pub problem: Option<String>,
}

impl Card {
    /// The card of a record that cannot be read, known by the name of its folder.
    fn unreadable(folder: &str, problem: String) -> Card {
        Card {
            id: folder.to_string(),
            title: String::new(),
            kind: String::new(),
            state: String::new(),
            color: String::new(),
            gates: String::new(),
            paths: Vec::new(),
            problem: Some(problem),
        }
    }
}

/// The zone map and the rules table that give a change its color.
struct Colors {
    zones: ZoneMap,
    table: RulesTable,
}

impl Colors {
    /// Those of the project at `root`; a missing file says nothing, one that cannot be read gives
    /// none.
    fn load(root: &Path) -> Option<Colors> {
        let zones = fs::read_to_string(root.join(ZONES));
        let zones = ZoneMap::from_toml(&optional(zones)?).ok()?;
        let rules = fs::read_to_string(root.join(guard::RULES));
        let table = RulesTable::of_project(&optional(rules)?).ok()?;
        Some(Colors { zones, table })
    }

    /// The color of the plan `change`, as `gdd classify` gives it.
    fn of(&self, change: &Change) -> String {
        let classified = classify(&self.zones, &self.table, change);
        classified.map(|c| c.color.to_string()).unwrap_or_default()
    }
}

/// The text of a file read, or `None` when it could not be read for another reason than its
/// absence, which is empty text.
fn optional(read: io::Result<String>) -> Option<String> {
    match read {
        Ok(text) => Some(text),
        Err(e) if e.kind() == ErrorKind::NotFound => Some(String::new()),
        Err(_) => None,
    }
}

/// One card per folder of `.gdd/changes/` under the state root `state`, sorted by when the change
/// was created, then by id. Its color is the one prepare stored, else the one the zone map of the
/// project at `root` gives its planned paths. It reads, and writes nothing. A folder without a
/// record is not a change; one whose record cannot be read has a card with its `problem`.
pub fn cards(root: &Path, state: &Path) -> Vec<Card> {
    let dir = state.join(CHANGES);
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Vec::new(),
        Err(e) => {
            let problem = format!("cannot read {CHANGES}: {e}");
            return vec![Card::unreadable(CHANGES, problem)];
        }
    };
    let folders = entries.flatten().filter(|entry| entry.path().is_dir());
    let colors = Colors::load(root);
    let mut found: Vec<(String, Card)> = Vec::new();
    for folder in folders {
        let name = folder.file_name().to_string_lossy().into_owned();
        match fs::read_to_string(folder.path().join(RECORD)) {
            Ok(record) => found.push(card_of(&name, &record, colors.as_ref())),
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => {
                let problem = format!("cannot read {RECORD}: {e}");
                found.push((String::new(), Card::unreadable(&name, problem)));
            }
        }
    }
    found.sort_by(|(a, card), (b, other)| (a, &card.id).cmp(&(b, &other.id)));
    found.into_iter().map(|(_, card)| card).collect()
}

/// The card of the record `text` in the folder `folder`, with when it was created.
fn card_of(folder: &str, text: &str, colors: Option<&Colors>) -> (String, Card) {
    let record: Map<String, Value> = match serde_json::from_str(text) {
        Ok(record) => record,
        Err(e) => return unreadable(folder, &e),
    };
    let plan = match Record::from_json(text) {
        Ok(plan) => plan,
        Err(e) => return unreadable(folder, &e),
    };
    let color = match (plan.decided_color, colors) {
        (Some(color), _) => color.to_string(),
        (None, Some(colors)) => colors.of(&plan.plan),
        (None, None) => "?".to_string(),
    };
    let card = Card {
        id: text_field(&record, "id", folder),
        title: text_field(&record, "title", ""),
        kind: text_field(&record, "type", "Feature"),
        state: text_field(&record, "state", "open"),
        color,
        gates: gates_text(&record),
        paths: plan.plan.paths,
        problem: None,
    };
    (text_field(&record, "created", ""), card)
}

/// The card of the record in `folder` that `why` makes unreadable, with no creation time.
fn unreadable(folder: &str, why: &dyn fmt::Display) -> (String, Card) {
    (String::new(), Card::unreadable(folder, not_valid(why)))
}

/// A gate of a record as the status shows it: its name and its decision.
fn gate_text((gate, entry): (&String, &Value)) -> String {
    let decision = entry.get("decision").and_then(Value::as_str);
    format!("{gate}:{}", decision.unwrap_or("?"))
}

/// What the harness self-test said, from the text of its result: its summary, else whether it
/// passed; one that did not pass says `failed` before its summary. `None` when the text is not a
/// result.
pub fn selftest_summary(text: &str) -> Option<String> {
    let result: Map<String, Value> = serde_json::from_str(text).ok()?;
    let passed = result.get("ok") == Some(&Value::Bool(true));
    let summary = result.get("summary").and_then(Value::as_str);
    let said = match (passed, summary) {
        (true, Some(summary)) => summary.to_string(),
        (true, None) => "passed".to_string(),
        (false, Some(summary)) => format!("failed · {summary}"),
        (false, None) => "failed".to_string(),
    };
    Some(said)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn debt(command: &str, gate: Gate, since: &str) -> Debt {
        Debt {
            command: command.into(),
            gate,
            since: since.into(),
            count: None,
        }
    }

    fn status() -> Status {
        Status {
            basics: Basics {
                name: "demo".into(),
                language: "es".into(),
                platform: "cli".into(),
            },
            brief: true,
            g0: true,
            zones: ZonesState::Approved,
            selftest: Some("harness 4/4".into()),
            scenarios_dir: "features".into(),
            scenarios: Ok(12),
            exceptions: Ok(vec![
                "A · deny.toml · no end date · r".into(),
                "B · deny.toml · no end date · no reason".into(),
            ]),
            debt: Ok(vec![
                Debt {
                    count: Some(5),
                    ..debt("lint", Gate::G2, "2026-10-26")
                },
                debt("security", Gate::G3, "2026-10-27"),
                debt("lint", Gate::G3, "2026-10-27"),
            ]),
            without_mutation: false,
            changes: vec!["  - c1 · Drafts · open · no gates yet".into()],
            log_lines: Some(3),
            second_family: vec![],
        }
    }

    #[test]
    fn the_status_says_where_the_project_stands() {
        let expected = concat!(
            "Project: demo · language es · platform cli\n",
            "Brief: docs/brief.md\n",
            "G0: ok · G0 ok\n",
            "Red zones: approved · red zones approved\n",
            "Harness self-test: harness 4/4\n",
            "Scenarios in features/: 12\n",
            "Security exceptions: 2\n",
            "  - A · deny.toml · no end date · r\n",
            "  - B · deny.toml · no end date · no reason\n",
            "Known debt: security (no count: a change can add to it unseen), ",
            "lint (5 in G2, no count in G3: a change can add to it unseen)\n",
            "  - security · since 2026-10-27 · Refactor debt-security\n",
            "  - lint · since 2026-10-26 · Refactor debt-lint\n",
            "Changes:\n",
            "  - c1 · Drafts · open · no gates yet\n",
            "Log: 3 lines in .gdd/log.jsonl (gdd report)"
        );
        assert_eq!(status().to_string(), expected);
    }

    #[test]
    fn known_debt_counted_in_every_gate_shows_its_counts_without_the_warning() {
        let counted = |command: &str, gate: Gate, count: u64| Debt {
            count: Some(count),
            ..debt(command, gate, "2026-10-26")
        };
        let lines = |debts: Vec<Debt>| {
            let status = Status {
                debt: Ok(debts),
                ..status()
            };
            let (shown, start) = (status.to_string(), "Known debt: ");
            let line = shown.lines().find(|line| line.starts_with(start));
            line.unwrap_or_default().to_string()
        };
        let both = vec![
            counted("types", Gate::G2, 9),
            counted("lint", Gate::G3, 5),
            counted("lint", Gate::G2, 5),
        ];
        assert_eq!(lines(both), "Known debt: types (9), lint (5)");
        let apart = vec![counted("lint", Gate::G3, 3), counted("lint", Gate::G2, 5)];
        assert_eq!(lines(apart), "Known debt: lint (5 in G2, 3 in G3)");
    }

    #[test]
    fn a_new_project_is_pending_everywhere() {
        let new = Status {
            brief: false,
            g0: false,
            zones: ZonesState::NotApproved,
            selftest: None,
            scenarios: Err("not a folder".into()),
            exceptions: Ok(vec![]),
            debt: Ok(vec![]),
            changes: vec![],
            log_lines: None,
            ..status()
        };
        let expected = concat!(
            "Project: demo · language es · platform cli\n",
            "Brief: missing (Setup writes it)\n",
            "G0: pending · G0 pending: run the Setup (the gdd-setup command).\n",
            "Red zones: pending · Red zones not approved yet: the Orchestrator proposes them ",
            "at the end of Discover.\n",
            "Harness self-test: not run\n",
            "Scenarios in features/: cannot count them: not a folder\n",
            "Security exceptions: none\n",
            "Known debt: none\n",
            "Changes:"
        );
        assert_eq!(new.to_string(), expected);
        let unreadable = Status {
            exceptions: Err("deny.toml: invalid TOML: x".into()),
            debt: Err("gdd/debt.toml: invalid TOML: y".into()),
            ..status()
        };
        let lines = "\nSecurity exceptions: deny.toml: invalid TOML: x\n\
                     Known debt: gdd/debt.toml: invalid TOML: y\nChanges:\n";
        assert!(unreadable.to_string().contains(lines), "{unreadable}");
        let changed = Status {
            zones: ZonesState::Changed,
            ..status()
        };
        let line = "Red zones: pending · Red zones changed after their approval: a human must \
                    approve them again.\n";
        assert!(changed.to_string().contains(line), "{changed}");
    }

    #[test]
    fn the_status_warns_with_the_other_lists_when_red_changes_go_without_mutation_testing() {
        let without = Status {
            without_mutation: true,
            ..status()
        };
        let lines = "\n  - lint · since 2026-10-26 · Refactor debt-lint\nno mutation testing command \
                     in [g3] of gdd/commands.toml: red changes go without it\nChanges:\n";
        assert!(without.to_string().contains(lines), "{without}");
        assert!(!status().to_string().contains(NO_MUTATION));
    }

    #[test]
    fn the_status_names_the_agents_of_a_second_family_last() {
        let two = Status {
            second_family: vec!["codex".into(), "opencode".into()],
            log_lines: None,
            ..status()
        };
        let last = "\n  - c1 · Drafts · open · no gates yet\nA second model family is on this \
                    machine (codex, opencode): set it as the Reviewer in gdd/models.toml";
        assert!(two.to_string().ends_with(last), "{two}");
    }

    /// A models file whose Builder is `builder` with `model`, of the family of `[families]`, and
    /// whose Reviewer is the same, `accepted` by the human or not.
    fn models(builder: &str, model: &str, families: &str, accepted: bool) -> Models {
        let table = format!("integration = \"{builder}\"\nmodel = \"{model}\"\n");
        let day = if accepted {
            "same_family_accepted = \"2026-10-27\"\n"
        } else {
            ""
        };
        let roles = format!("[builder.red]\n{table}[reviewer]\n{table}{day}");
        let text = format!("{roles}[families]\n{families}");
        Models::from_toml(&text).unwrap()
    }

    fn names(named: &[&str]) -> Vec<String> {
        named.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn an_agent_of_another_family_could_review_a_project_that_accepted_one() {
        let found = names(&["claude", "codex", "gemini", "opencode"]);
        let anthropic = "\"claude\" = \"anthropic\"\n";
        let accepted = models("claude", "opus", anthropic, true);
        assert_eq!(second_family(&accepted, &found), ["codex", "opencode"]);
        let not_accepted = models("claude", "opus", anthropic, false);
        assert_eq!(second_family(&not_accepted, &found), Vec::<String>::new());
        // The families of the file count: by model name or by integration.
        let by_model = "\"opus\" = \"anthropic\"\n\"gpt\" = \"anthropic\"\n";
        let accepted = models("claude", "opus", by_model, true);
        let found = names(&["codex", "claude", "opencode"]);
        assert_eq!(second_family(&accepted, &found), ["opencode"]);
        let codex = models("codex", "gpt-6-astra", "\"codex\" = \"openai\"\n", true);
        assert_eq!(second_family(&codex, &found), ["claude", "opencode"]);
    }

    #[test]
    fn a_change_shows_its_id_title_state_and_gates() {
        let record = r#"{"id": "c1", "title": "Drafts", "state": "building",
            "gates": {"plan": {"decision": "ok"}, "G1": {"decision": "ko"}, "G4": {}}}"#;
        let line = "  - c1 · Drafts · building · G1:ko, G4:?, plan:ok";
        assert_eq!(change_line("x", record), line);
        let empty = "  - x ·  · open · no gates yet";
        assert_eq!(change_line("x", "{}"), empty);
        let odd = r#"{"id": 3, "title": null, "gates": []}"#;
        assert_eq!(change_line("x", odd), empty);
        let broken = change_line("x", "{not json");
        let start = "  - x · change.json is not valid: ";
        assert!(broken.starts_with(start), "{broken}");
    }

    /// A folder in the temporary directory, removed when dropped.
    struct Folder(std::path::PathBuf);

    impl Folder {
        fn new(name: &str) -> Folder {
            let unique = format!("gdd-cards-{}-{name}", std::process::id());
            let path = std::env::temp_dir().join(unique);
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Folder(path)
        }

        fn write(&self, path: &str, text: &str) {
            let file = self.0.join(path);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, text).unwrap();
        }

        fn change(&self, id: &str, record: &str) {
            self.write(&format!("{CHANGES}/{id}/{RECORD}"), record);
        }
    }

    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const ZONE_MAP: &str = "default = \"yellow\"\n[[zone]]\ncolor = \"red\"\npaths = [\"db/**\"]\n";

    #[test]
    fn the_cards_come_by_creation_then_id_with_their_color_and_gates() {
        let folder = Folder::new("sorted");
        folder.write(ZONES, ZONE_MAP);
        let plan = r#""paths": ["db/a.rs", "b.rs"], "gates": {"plan": {"decision": "ok"}}"#;
        let two = format!(r#"{{"created": "2", "title": "Two", {plan}}}"#);
        folder.change("b", &two);
        let spike = r#"{"created": "2", "type": "Spike", "state": "building"}"#;
        folder.change("a", spike);
        let prepared = r#"{"created": "1", "id": "z", "decision": {"color": "green"}}"#;
        folder.change("z", prepared);
        let all = cards(&folder.0, &folder.0);
        let ids: Vec<&str> = all.iter().map(|card| card.id.as_str()).collect();
        assert_eq!(ids, ["z", "a", "b"]);
        let (z, a, b) = (&all[0], &all[1], &all[2]);
        assert_eq!((z.color.as_str(), z.kind.as_str()), ("green", "Feature"));
        assert_eq!((a.color.as_str(), a.kind.as_str()), ("yellow", "Spike"));
        let (state, gates) = (a.state.as_str(), a.gates.as_str());
        assert_eq!((state, gates), ("building", "no gates yet"));
        assert_eq!((b.color.as_str(), b.title.as_str()), ("red", "Two"));
        assert_eq!((b.state.as_str(), b.gates.as_str()), ("open", "plan:ok"));
        assert_eq!(b.paths, ["db/a.rs", "b.rs"]);
        assert!(all.iter().all(|card| card.problem.is_none()));
    }

    #[test]
    fn a_project_without_changes_has_no_cards() {
        let folder = Folder::new("none");
        assert_eq!(cards(&folder.0, &folder.0), Vec::new());
        folder.write(".gdd/changes/x/inbox.md", "no record here");
        assert_eq!(cards(&folder.0, &folder.0), Vec::new());
    }

    #[test]
    fn a_record_that_cannot_be_read_has_a_card_with_its_problem() {
        let folder = Folder::new("unreadable");
        folder.change("bad", "{not json");
        folder.change("blue", r#"{"decision": {"color": "blue"}}"#);
        fs::create_dir_all(folder.0.join(".gdd/changes/dir/change.json")).unwrap();
        folder.change("ok", r#"{"title": "Fine"}"#);
        let all = cards(&folder.0, &folder.0);
        let ids: Vec<&str> = all.iter().map(|card| card.id.as_str()).collect();
        assert_eq!(ids, ["bad", "blue", "dir", "ok"]);
        let problem = |card: &Card| card.problem.clone().unwrap_or_default();
        assert!(problem(&all[0]).starts_with("change.json is not valid: "));
        let blue = problem(&all[1]);
        assert!(blue.contains("unknown color"), "{blue}");
        assert!(problem(&all[2]).starts_with("cannot read change.json: "));
        assert_eq!(all[3].problem, None);
    }

    #[test]
    fn a_changes_folder_that_cannot_be_read_is_a_card_with_its_problem() {
        let folder = Folder::new("file");
        folder.write(CHANGES, "not a folder");
        let all = cards(&folder.0, &folder.0);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, CHANGES);
        let problem = all[0].problem.clone().unwrap_or_default();
        let start = "cannot read .gdd/changes: ";
        assert!(problem.starts_with(start), "{problem}");
    }

    #[test]
    fn the_color_needs_the_zone_map_but_not_that_it_exists() {
        let folder = Folder::new("colors");
        folder.change("a", r#"{"paths": ["db/a.rs"]}"#);
        let color = |folder: &Folder| cards(&folder.0, &folder.0)[0].color.clone();
        assert_eq!(color(&folder), "yellow");
        folder.write(ZONES, ZONE_MAP);
        assert_eq!(color(&folder), "red");
        folder.write(ZONES, "default = \"blue\"");
        assert_eq!(color(&folder), "?");
        fs::remove_file(folder.0.join(ZONES)).unwrap();
        folder.write("gdd/rules.toml", "not = [toml");
        assert_eq!(color(&folder), "?");
        fs::remove_file(folder.0.join("gdd/rules.toml")).unwrap();
        fs::create_dir_all(folder.0.join(ZONES)).unwrap();
        assert_eq!(color(&folder), "?");
    }

    #[test]
    fn red_changes_go_without_mutation_testing_when_g3_has_no_mutation_command() {
        let folder = Folder::new("mutation");
        let without = |commands: &str| {
            folder.write(COMMANDS, commands);
            without_mutation(&folder.0)
        };
        assert!(without("[g2]\nunit = \"true\"\n"));
        let in_g2 = "[g2]\nmutation = \"cargo mutants\"\n[g3]\nacceptance = \"true\"\n";
        assert!(without(in_g2));
        assert!(without("[g3]\nmutation = \"\"\nmutation-rust = \"  \"\n"));
        assert!(without("[g3]\nrun-mutation = \"cargo mutants\"\n"));
        assert!(!without("[g3]\nmutation-rust = \"cargo mutants\"\n"));
        assert!(!without("[g3]\nmutation = \"cargo mutants\"\n"));
    }

    #[test]
    fn a_commands_file_that_is_not_there_or_cannot_be_read_says_nothing_of_mutation() {
        let folder = Folder::new("no-commands");
        assert!(!without_mutation(&folder.0));
        folder.write(COMMANDS, "[g3");
        assert!(!without_mutation(&folder.0));
        fs::remove_file(folder.0.join(COMMANDS)).unwrap();
        fs::create_dir_all(folder.0.join(COMMANDS)).unwrap();
        assert!(!without_mutation(&folder.0));
    }

    #[test]
    fn the_basics_come_from_the_project_file() {
        let basics = Basics::from_toml("name = \"gdd\"\nplatform = \"cli\"\n").unwrap();
        let expected = Basics {
            name: "gdd".into(),
            language: "en".into(),
            platform: "cli".into(),
        };
        assert_eq!(basics, expected);
        let unknown = Basics::from_toml("name = 3\n[language]\n").unwrap();
        assert_eq!(unknown.name, "?");
        assert_eq!(unknown.language, "en");
        assert!(Basics::from_toml("name = ").is_err());
    }

    #[test]
    fn the_self_test_says_its_summary_or_whether_it_passed() {
        let summary = selftest_summary(r#"{"ok": true, "summary": "harness 4/4"}"#);
        assert_eq!(summary.as_deref(), Some("harness 4/4"));
        let passed = selftest_summary(r#"{"ok": true}"#);
        assert_eq!(passed.as_deref(), Some("passed"));
        for failed in [r#"{"ok": false}"#, r#"{"ok": "yes"}"#, "{}"] {
            assert_eq!(selftest_summary(failed).as_deref(), Some("failed"));
        }
        let with_summary = [
            r#"{"ok": false, "summary": "harness 3/4"}"#,
            r#"{"summary": "harness 3/4"}"#,
        ];
        for failed in with_summary {
            let said = selftest_summary(failed);
            assert_eq!(said.as_deref(), Some("failed · harness 3/4"), "{failed}");
        }
        let odd = selftest_summary(r#"{"ok": true, "summary": 4}"#);
        assert_eq!(odd.as_deref(), Some("passed"));
        for nothing in ["{not json", "[]", "true"] {
            assert_eq!(selftest_summary(nothing), None, "{nothing}");
        }
    }
}
