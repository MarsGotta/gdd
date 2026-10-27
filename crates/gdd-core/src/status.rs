//! The status of a project: where it stands in the method, its security exceptions, its known
//! debt, its changes, its log and, when it accepted one model family only, the agents of another
//! family on the machine.

use std::fmt;

use serde_json::{Map, Value};

use crate::agents::models::{self, Models};
use crate::config::{self, Color};
use crate::gates::approve::{BRIEF, ZonesState};
use crate::gates::debt::{self, Debt};
use crate::{init, log};

/// What the status says of a missing Project Brief.
const NO_BRIEF: &str = "missing (Setup writes it)";

/// What the status says of G0 once the machines opened it.
const G0_OK: &str = "ok · G0 ok";

/// What the status says of G0 until the machines open it.
const G0_PENDING: &str = "pending · G0 pending: run the Setup (the gdd-setup command).";

/// What the status says when an agent of another model family could review a project that
/// accepted one family only.
const SECOND_FAMILY: &str = "A second model family is on this machine";

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

/// The lines of the known debt `known` in the status: `Known debt: ` and the names of its
/// commands, riskiest first, then a line per command with the day it was recorded and the
/// Refactor that pays it; `none` without debt, or why it could not be read.
fn known_debt(f: &mut fmt::Formatter<'_>, known: &Result<Vec<Debt>, String>) -> fmt::Result {
    let debts = match known {
        Ok(debts) if debts.is_empty() => return writeln!(f, "Known debt: none"),
        Ok(debts) => debt::listed(debts),
        Err(why) => return writeln!(f, "Known debt: {why}"),
    };
    let names: Vec<&str> = debts.iter().map(|debt| debt.command.as_str()).collect();
    writeln!(f, "Known debt: {}", names.join(", "))?;
    for Debt { command, since, .. } in debts {
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
        Err(e) => return format!("  - {folder} · change.json is not valid: {e}"),
    };
    let text = |key: &str, default: &str| {
        let value = record.get(key).and_then(Value::as_str);
        value.unwrap_or(default).to_string()
    };
    let gates: Vec<String> = match record.get("gates").and_then(Value::as_object) {
        Some(gates) => gates.iter().map(gate_text).collect(),
        None => Vec::new(),
    };
    let gates = if gates.is_empty() {
        "no gates yet".to_string()
    } else {
        gates.join(", ")
    };
    let (id, title) = (text("id", folder), text("title", ""));
    let state = text("state", "open");
    format!("  - {id} · {title} · {state} · {gates}")
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
    use crate::gates::Gate;

    fn debt(command: &str, gate: Gate, since: &str) -> Debt {
        Debt {
            command: command.into(),
            gate,
            since: since.into(),
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
                debt("lint", Gate::G2, "2026-10-26"),
                debt("security", Gate::G3, "2026-10-27"),
                debt("lint", Gate::G3, "2026-10-27"),
            ]),
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
            "Known debt: security, lint\n",
            "  - security · since 2026-10-27 · Refactor debt-security\n",
            "  - lint · since 2026-10-26 · Refactor debt-lint\n",
            "Changes:\n",
            "  - c1 · Drafts · open · no gates yet\n",
            "Log: 3 lines in .gdd/log.jsonl (gdd report)"
        );
        assert_eq!(status().to_string(), expected);
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
