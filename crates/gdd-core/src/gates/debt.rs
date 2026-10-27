//! The known debt of a project: the gate commands that failed when `gdd adopt` first ran them on
//! the code the project already had (see [`crate::adopt`]), in `gdd/debt.toml`. A gate still runs
//! a command with known debt, but its failure does not fail the gate: the report keeps it apart.
//! Once a change makes the command pass, the gate names it as paid, and the merge of that change
//! takes it off the list, so from then on it blocks. Only `gdd adopt` writes the list, once; the
//! merge only takes from it.

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::agents::files;
use crate::config;
use crate::gates::{Gate, Report};

/// The file of the known debt, from the project root.
pub const PATH: &str = "gdd/debt.toml";

/// The name of the check that fails a gate when the known debt cannot be read.
pub const CHECK: &str = "debt";

/// What the file of the known debt says first: what it is and how it empties.
const HEADER: &str = "# Known debt recorded by gdd adopt. These commands run and warn but do not \
                      block,\n# until a change makes each pass; the merge then takes it off this \
                      list for good.\n";

/// The kinds of command, riskiest first: the part of a name before its first `-`.
const RISK: [&str; 8] = [
    "security",
    "dependencies",
    "types",
    "unit",
    "architecture",
    "quality",
    "lint",
    "lockfile",
];

/// What the name of a command starts with when it judges the change or its scenarios, not the
/// code that was there before: it never becomes debt.
const NEVER_DEBT: [&str; 3] = ["acceptance", "mutation", "secrets"];

/// The known debt of a project, as `gdd/debt.toml` holds it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Known {
    /// The day `gdd adopt` recorded it.
    pub adopted: String,
    /// The commands with known debt.
    #[serde(default)]
    pub debt: Vec<Debt>,
}

/// A gate command with known debt.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Debt {
    /// Its name in `gdd/commands.toml`.
    pub command: String,
    /// The gate whose section has it.
    pub gate: Gate,
    /// The day it was recorded.
    pub since: String,
}

/// A command with known debt that failed in a gate: its name and what its output keeps (see
/// [`super::kept`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Owed {
    /// The name of the command.
    pub name: String,
    /// What to read.
    pub tail: String,
}

/// What the latest G2 and G3 of a change said of the known debt, each with the gate that said it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reported {
    /// The commands with known debt still failing.
    pub owed: Vec<(Gate, Owed)>,
    /// The commands with known debt that passed.
    pub paid: Vec<(Gate, String)>,
}

impl Known {
    /// The text of `gdd/debt.toml` for it: what the file is, the day of the adoption, then each
    /// debt.
    pub fn to_toml(&self) -> String {
        let mut text = format!("{HEADER}adopted = {}\n", quoted(&self.adopted));
        for debt in &self.debt {
            text.push_str(&format!(
                "\n[[debt]]\ncommand = {}\ngate = {}\nsince = {}\n",
                quoted(&debt.command),
                quoted(debt.gate.key()),
                quoted(&debt.since)
            ));
        }
        text
    }
}

/// `text` as a TOML string.
fn quoted(text: &str) -> String {
    toml::Value::from(text).to_string()
}

/// Whether the gate command `name` can become known debt: it does not judge the change or its
/// scenarios.
pub fn can_owe(name: &str) -> bool {
    !NEVER_DEBT.iter().any(|start| name.starts_with(start))
}

/// The risk of the command `name`, to sort by: the place of its kind in [`RISK`] (a kind not
/// there goes after them), then the name.
pub fn risk(name: &str) -> (usize, &str) {
    let kind = name.split_once('-').map_or(name, |(kind, _)| kind);
    let place = RISK.iter().position(|known| *known == kind);
    (place.unwrap_or(RISK.len()), name)
}

/// `names` riskiest first (see [`risk`]), each once.
pub fn by_risk<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<&'a str> {
    let mut names: Vec<&str> = names.into_iter().collect();
    names.sort_by_key(|&name| risk(name));
    names.dedup();
    names
}

/// `debts` riskiest first (see [`risk`]), one per command name: the first of its gates.
pub fn listed(debts: &[Debt]) -> Vec<&Debt> {
    let mut listed: Vec<&Debt> = debts.iter().collect();
    listed.sort_by_key(|debt| risk(&debt.command));
    listed.dedup_by(|later, first| later.command == first.command);
    listed
}

/// The known debt of the project at `root`; `None` without the file. A file that cannot be
/// read (a link, a folder, a path through a file), that is not shaped as known debt, or that
/// names a command that can never become debt (see [`can_owe`]) is an error that names it.
pub fn read(root: &Path) -> Result<Option<Known>, String> {
    let bytes = files::read(root, PATH).map_err(|e| format!("{PATH}: cannot read: {e}"))?;
    let Some(bytes) = bytes else {
        return Ok(None);
    };
    let text = String::from_utf8_lossy(&bytes);
    let known: Known = toml::from_str(&text)
        .map_err(|e| format!("{PATH}: {}", config::Error::Toml(e.to_string())))?;
    if let Some(debt) = known.debt.iter().find(|debt| !can_owe(&debt.command)) {
        return Err(format!(
            "{PATH}: {:?} can never be known debt: it judges the change or its scenarios, so it \
             always blocks. Take it off the list.",
            debt.command
        ));
    }
    Ok(Some(known))
}

/// Writes `bytes` to the file `path` under `root`, a new file (see [`files::replace`]): a link
/// on the way, or a file there with other names, is refused first (see [`files::read`]).
pub(crate) fn write_new(root: &Path, path: &str, bytes: &[u8]) -> Result<(), String> {
    let cannot = |e: std::io::Error| format!("cannot write {path}: {e}");
    files::read(root, path).map_err(cannot)?;
    files::replace(root, path, bytes).map_err(cannot)
}

/// The names of the commands of `gate` with known debt in the project at `root`. When the file
/// cannot be read there are none, and `report` fails the check `debt` with why.
pub(crate) fn owed(report: &mut Report, root: &Path, gate: Gate) -> BTreeSet<String> {
    match read(root) {
        Ok(known) => known
            .into_iter()
            .flat_map(|known| known.debt)
            .filter(|debt| debt.gate == gate)
            .map(|debt| debt.command)
            .collect(),
        Err(why) => {
            report.fail(CHECK, why);
            BTreeSet::new()
        }
    }
}

/// What the latest G2 and G3 of change `id`, under the state root `state`, said of the known
/// debt: nothing from a gate that has not run.
pub fn reported(state: &Path, id: &str) -> Result<Reported, String> {
    #[derive(Deserialize)]
    struct Saved {
        #[serde(default)]
        known_debt: Vec<Owed>,
        #[serde(default)]
        paid_debt: Vec<String>,
    }
    let mut reported = Reported::default();
    for gate in [Gate::G2, Gate::G3] {
        let path = gate.report_path(id);
        let bytes = files::read(state, &path).map_err(|e| format!("cannot read {path}: {e}"))?;
        let Some(bytes) = bytes else {
            continue;
        };
        let saved: Saved = serde_json::from_slice(&bytes).map_err(|e| format!("{path}: {e}"))?;
        let owed = saved.known_debt.into_iter().map(|owed| (gate, owed));
        reported.owed.extend(owed);
        let paid = saved.paid_debt.into_iter().map(|name| (gate, name));
        reported.paid.extend(paid);
    }
    Ok(reported)
}

/// Takes the commands `paid`, each with its gate, off the known debt of the project at `root`:
/// the file is written again without them, or removed when no debt is left. Nothing without the
/// file.
pub fn pay(root: &Path, paid: &[(Gate, String)]) -> Result<(), String> {
    let Some(mut known) = read(root)? else {
        return Ok(());
    };
    let owed = |debt: &Debt| !paid.contains(&(debt.gate, debt.command.clone()));
    known.debt.retain(owed);
    if known.debt.is_empty() {
        files::remove(root, PATH).map_err(|e| format!("cannot remove {PATH}: {e}"))
    } else {
        write_new(root, PATH, known.to_toml().as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Color;
    use crate::git::tests::Folder;
    use std::fs;
    use std::os::unix::fs::symlink;

    const KNOWN: &str = "adopted = \"2026-10-27\"\n\n\
                         [[debt]]\ncommand = \"security\"\ngate = \"g3\"\nsince = \"2026-10-27\"\n\n\
                         [[debt]]\ncommand = \"lint\"\ngate = \"g2\"\nsince = \"2026-10-28\"\n";

    fn debt(command: &str, gate: Gate, since: &str) -> Debt {
        Debt {
            command: command.to_string(),
            gate,
            since: since.to_string(),
        }
    }

    fn report() -> Report {
        Report {
            ok: false,
            gate: Gate::G2,
            color: Color::Yellow,
            passed: vec![],
            skipped: vec![],
            failures: vec![],
            out_of_scope: vec![],
            added_exceptions: vec![],
            known_debt: vec![],
            paid_debt: vec![],
            duration_s: 0.0,
        }
    }

    fn paid(list: &[(Gate, &str)]) -> Vec<(Gate, String)> {
        list.iter().map(|(g, n)| (*g, n.to_string())).collect()
    }

    #[test]
    fn the_known_debt_is_read_from_its_file() {
        let folder = Folder::new();
        assert_eq!(read(&folder.0), Ok(None));
        folder.write(PATH, KNOWN);
        let expected = Known {
            adopted: "2026-10-27".into(),
            debt: vec![
                debt("security", Gate::G3, "2026-10-27"),
                debt("lint", Gate::G2, "2026-10-28"),
            ],
        };
        assert_eq!(read(&folder.0), Ok(Some(expected)));
        folder.write(PATH, "adopted = \"2026-10-27\"\n");
        let empty = Known {
            adopted: "2026-10-27".into(),
            debt: vec![],
        };
        assert_eq!(read(&folder.0), Ok(Some(empty)));
    }

    #[test]
    fn a_file_not_shaped_as_known_debt_is_invalid() {
        let folder = Folder::new();
        for text in [
            "[[debt]",
            "[[debt]]\ncommand = \"lint\"\ngate = \"g2\"\nsince = \"x\"\n",
            "adopted = \"x\"\n[[debt]]\ncommand = \"lint\"\ngate = \"g4\"\nsince = \"x\"\n",
            "adopted = \"x\"\n[[debt]]\ngate = \"g2\"\nsince = \"x\"\n",
            "adopted = 1\n",
        ] {
            folder.write(PATH, text);
            let err = read(&folder.0).unwrap_err();
            let start = "gdd/debt.toml: invalid TOML: ";
            assert!(err.starts_with(start), "{text:?}: {err}");
        }
    }

    #[test]
    fn a_file_with_a_command_that_never_becomes_debt_is_invalid() {
        let folder = Folder::new();
        for (name, gate) in [
            ("secrets", "g3"),
            ("acceptance", "g2"),
            ("acceptance_full", "g3"),
            ("mutation-slow", "g3"),
        ] {
            let entry = format!("[[debt]]\ncommand = {name:?}\ngate = {gate:?}\nsince = \"x\"\n");
            folder.write(PATH, &format!("{KNOWN}\n{entry}"));
            let said = format!(
                "gdd/debt.toml: {name:?} can never be known debt: it judges the change or its \
                 scenarios, so it always blocks. Take it off the list."
            );
            assert_eq!(read(&folder.0), Err(said.clone()));
            // A gate owes nothing then, and fails the check `debt` with why.
            let mut report = report();
            let owed = owed(&mut report, &folder.0, Gate::G3);
            assert_eq!(owed, BTreeSet::new());
            let [failure] = report.failures.as_slice() else {
                panic!("one failure: {report:?}");
            };
            assert_eq!((failure.name.as_str(), &failure.tail), (CHECK, &said));
        }
    }

    #[test]
    fn a_file_that_cannot_be_read_is_an_error_not_no_debt() {
        let (folder, outside) = (Folder::new(), Folder::new());
        outside.write("debt.toml", KNOWN);
        fs::create_dir_all(folder.0.join("gdd")).unwrap();
        symlink(outside.0.join("debt.toml"), folder.0.join(PATH)).unwrap();
        let err = read(&folder.0).unwrap_err();
        let start = "gdd/debt.toml: cannot read: not a file";
        assert!(err.starts_with(start), "{err}");
        // A path under a regular file: not a folder on the way, then NotADirectory.
        let plain = Folder::new();
        plain.write("gdd", "a file\n");
        let err = read(&plain.0).unwrap_err();
        assert_eq!(err, "gdd/debt.toml: cannot read: gdd is not a folder");
        let err = read(&plain.0.join("gdd")).unwrap_err();
        assert!(err.starts_with("gdd/debt.toml: cannot read: "), "{err}");
        assert!(err.contains("Not a directory"), "{err}");
    }

    #[test]
    fn the_file_is_written_as_it_is_read() {
        let folder = Folder::new();
        let known = Known {
            adopted: "2026-10-27".into(),
            debt: vec![
                debt("security", Gate::G3, "2026-10-27"),
                debt("lint \"x\"", Gate::G2, "2026-10-27"),
            ],
        };
        let text = known.to_toml();
        let start = "# Known debt recorded by gdd adopt. These commands run and warn but do not \
                     block,\n# until a change makes each pass; the merge then takes it off this \
                     list for good.\nadopted = \"2026-10-27\"\n\n[[debt]]\ncommand = \"security\"\n\
                     gate = \"g3\"\nsince = \"2026-10-27\"\n\n[[debt]]\n";
        assert!(text.starts_with(start), "{text}");
        write_new(&folder.0, PATH, text.as_bytes()).unwrap();
        assert_eq!(read(&folder.0), Ok(Some(known)));
    }

    #[test]
    fn a_file_is_never_written_through_a_link_or_another_name() {
        let (folder, outside) = (Folder::new(), Folder::new());
        outside.write("debt.toml", "outside\n");
        write_new(&folder.0, PATH, b"one").unwrap();
        write_new(&folder.0, PATH, b"two").unwrap();
        assert_eq!(fs::read_to_string(folder.0.join(PATH)).unwrap(), "two");
        fs::remove_file(folder.0.join(PATH)).unwrap();
        fs::hard_link(outside.0.join("debt.toml"), folder.0.join(PATH)).unwrap();
        let err = write_new(&folder.0, PATH, b"x").unwrap_err();
        let start = "cannot write gdd/debt.toml: not a file";
        assert!(err.starts_with(start), "{err}");
        fs::remove_file(folder.0.join(PATH)).unwrap();
        fs::remove_dir(folder.0.join("gdd")).unwrap();
        symlink(&outside.0, folder.0.join("gdd")).unwrap();
        let err = write_new(&folder.0, PATH, b"x").unwrap_err();
        assert_eq!(err, "cannot write gdd/debt.toml: gdd is not a folder");
        let text = fs::read_to_string(outside.0.join("debt.toml")).unwrap();
        assert_eq!(text, "outside\n");
    }

    #[test]
    fn mutation_secrets_and_the_scenarios_never_become_debt() {
        for name in ["lint", "unit", "security", "types-ts", "coverage"] {
            assert!(can_owe(name), "{name}");
        }
        for name in [
            "acceptance",
            "acceptance_full",
            "mutation",
            "mutation-slow",
            "secrets",
        ] {
            assert!(!can_owe(name), "{name}");
        }
    }

    #[test]
    fn the_riskiest_kind_comes_first_and_ties_go_by_name() {
        let names = [
            "lockfile",
            "lint-typescript",
            "zeta",
            "lint",
            "quality",
            "architecture",
            "unit",
            "types",
            "dependencies",
            "alpha",
            "security",
            "lint-rust",
            "lint",
        ];
        let expected = [
            "security",
            "dependencies",
            "types",
            "unit",
            "architecture",
            "quality",
            "lint",
            "lint-rust",
            "lint-typescript",
            "lockfile",
            "alpha",
            "zeta",
        ];
        assert_eq!(by_risk(names), expected);
        assert_eq!(risk("security-extra"), (0, "security-extra"));
        assert_eq!(risk("other"), (8, "other"));
    }

    #[test]
    fn the_listed_debt_has_one_line_per_command() {
        let debts = [
            debt("lint", Gate::G2, "a"),
            debt("security", Gate::G3, "b"),
            debt("lint", Gate::G3, "c"),
            debt("unit", Gate::G2, "d"),
        ];
        let listed: Vec<(&str, &str)> = listed(&debts)
            .iter()
            .map(|d| (d.command.as_str(), d.since.as_str()))
            .collect();
        assert_eq!(listed, [("security", "b"), ("unit", "d"), ("lint", "a")]);
    }

    #[test]
    fn a_gate_owes_only_its_own_commands_and_fails_on_a_file_it_cannot_read() {
        let folder = Folder::new();
        let mut report = report();
        assert_eq!(owed(&mut report, &folder.0, Gate::G2), BTreeSet::new());
        folder.write(PATH, KNOWN);
        let g2 = owed(&mut report, &folder.0, Gate::G2);
        assert_eq!(g2, BTreeSet::from(["lint".to_string()]));
        let g3 = owed(&mut report, &folder.0, Gate::G3);
        assert_eq!(g3, BTreeSet::from(["security".to_string()]));
        assert!(report.failures.is_empty(), "{report:?}");
        assert!(report.passed.is_empty(), "{report:?}");
        folder.write(PATH, "[[debt]");
        assert_eq!(owed(&mut report, &folder.0, Gate::G2), BTreeSet::new());
        let [failure] = report.failures.as_slice() else {
            panic!("one failure: {report:?}");
        };
        assert_eq!(failure.name, CHECK);
        let start = "gdd/debt.toml: invalid TOML";
        assert!(failure.tail.starts_with(start), "{failure:?}");
    }

    #[test]
    fn the_latest_reports_say_what_is_owed_and_what_is_paid() {
        let folder = Folder::new();
        assert_eq!(reported(&folder.0, "c1"), Ok(Reported::default()));
        let g2 = r#"{"ok": true, "known_debt": [{"name": "lint", "tail": "x"}],
                     "paid_debt": ["unit"]}"#;
        folder.write(".gdd/changes/c1/g2.json", g2);
        folder.write(".gdd/changes/c1/g3.json", r#"{"paid_debt": ["security"]}"#);
        let owed = Owed {
            name: "lint".into(),
            tail: "x".into(),
        };
        let expected = Reported {
            owed: vec![(Gate::G2, owed)],
            paid: paid(&[(Gate::G2, "unit"), (Gate::G3, "security")]),
        };
        assert_eq!(reported(&folder.0, "c1"), Ok(expected));
        folder.write(".gdd/changes/c1/g3.json", "{}");
        assert_eq!(reported(&folder.0, "c1").unwrap().paid.len(), 1);
        folder.write(".gdd/changes/c1/g3.json", "{not json");
        let err = reported(&folder.0, "c1").unwrap_err();
        assert!(err.starts_with(".gdd/changes/c1/g3.json: "), "{err}");
        fs::remove_file(folder.0.join(".gdd/changes/c1/g3.json")).unwrap();
        fs::create_dir_all(folder.0.join(".gdd/changes/c1/g3.json")).unwrap();
        let err = reported(&folder.0, "c1").unwrap_err();
        let start = "cannot read .gdd/changes/c1/g3.json: ";
        assert!(err.starts_with(start), "{err}");
    }

    #[test]
    fn paying_takes_the_commands_of_their_gate_off_and_the_last_removes_the_file() {
        let folder = Folder::new();
        assert_eq!(pay(&folder.0, &paid(&[(Gate::G2, "lint")])), Ok(()));
        assert!(!folder.0.join(PATH).exists());
        folder.write(PATH, KNOWN);
        // A name of the other gate is not paid.
        let other_gate = paid(&[(Gate::G3, "lint"), (Gate::G2, "security")]);
        pay(&folder.0, &other_gate).unwrap();
        assert_eq!(read(&folder.0).unwrap().unwrap().debt.len(), 2);
        pay(&folder.0, &paid(&[(Gate::G2, "lint")])).unwrap();
        let left = read(&folder.0).unwrap().unwrap();
        assert_eq!(left.adopted, "2026-10-27");
        assert_eq!(left.debt, [debt("security", Gate::G3, "2026-10-27")]);
        pay(&folder.0, &paid(&[(Gate::G3, "security")])).unwrap();
        assert!(!folder.0.join(PATH).exists());
    }

    #[test]
    fn paying_a_file_that_cannot_be_read_is_an_error() {
        let folder = Folder::new();
        folder.write(PATH, "[[debt]");
        let err = pay(&folder.0, &paid(&[(Gate::G2, "lint")])).unwrap_err();
        assert!(err.starts_with("gdd/debt.toml: invalid TOML"), "{err}");
        assert_eq!(fs::read_to_string(folder.0.join(PATH)).unwrap(), "[[debt]");
    }
}
