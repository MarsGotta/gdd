//! Adopting a project that already has code (`gdd adopt`): every gate command that judges the
//! whole project runs once on it, and the ones that fail become its known debt (see
//! [`crate::gates::debt`]), committed in `gdd/debt.toml`, with a Refactor proposed for each,
//! riskiest first. What each failing command said stays in `.gdd/adoption/` for that Refactor.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use crate::agents::files;
use crate::exec;
use crate::gates::debt::{self, Debt, Known, Listed, PATH};
use crate::gates::{self, Commands, Gate, exceptions};
use crate::git::Git;

/// Where what a failing command said is kept, from the state root.
pub const ADOPTION: &str = ".gdd/adoption";

/// The variables the commands get: the ones of the gates, without a change.
const ENV: [(&str, &str); 4] = [
    ("GDD_CHANGE", ""),
    ("GDD_COLOR", ""),
    ("GDD_DIFF_FILES", ""),
    ("GDD_FEATURES", ""),
];

/// How to create the Refactor that pays a debt.
const CREATE: &str =
    "Create one with: bin/gdd new-change debt-<name> --type Refactor --title \"...\" --paths ...";

/// What `gdd adopt` found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adopted {
    /// The names of the commands that passed, in the order they ran.
    pub passed: Vec<String>,
    /// The known debt it recorded, riskiest first.
    pub debt: Vec<Debt>,
}

impl fmt::Display for Adopted {
    /// What `gdd adopt` prints, without a line break at the end: the commands that passed and
    /// the known debt, each with its counts when a gate has one (see [`Listed::said`]); when
    /// there is, a Refactor per command, riskiest first, with where what it said is kept, and how
    /// to create one.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let passed = if self.passed.is_empty() {
            "none".to_string()
        } else {
            self.passed.join(", ")
        };
        writeln!(f, "Passed: {passed}")?;
        let listed = debt::listed(&self.debt);
        if listed.is_empty() {
            return write!(f, "Known debt: none");
        }
        let counted = |listed: &Listed| {
            if listed.counts.iter().all(|(_, count)| count.is_none()) {
                listed.command.to_string()
            } else {
                format!("{} ({})", listed.command, listed.said())
            }
        };
        let counted: Vec<String> = listed.iter().map(counted).collect();
        writeln!(f, "Known debt: {}", counted.join(", "))?;
        let names: Vec<&str> = listed.iter().map(|listed| listed.command).collect();
        writeln!(f, "Proposed Refactors, riskiest first:")?;
        for (n, name) in names.iter().enumerate() {
            let refactor = format!("debt-{name} · Pay the known debt of {name}");
            writeln!(f, "  {}. {refactor} · {}", n + 1, kept(name))?;
        }
        write!(f, "{CREATE}")
    }
}

/// Where what the command `name` said is kept, from the state root.
fn kept(name: &str) -> String {
    format!("{ADOPTION}/{name}.txt")
}

/// Adopts the project at `root`, whose state root is `state` and whose gate commands are
/// `commands`. Refused before anything runs once the project has `gdd/debt.toml`, or when a
/// command that can become debt has a name that cannot name a file. Each command of G2, then of
/// G3, that can become debt (see [`debt::can_owe`]) runs once from `root`, with the variables of
/// the gates and no change; it passes when it exits with 0. What each one that failed said, as a
/// gate keeps it (see [`gates::kept`]), goes to `.gdd/adoption/<name>.txt` under the state root;
/// its debt gets the count of what it said when `[count]` has texts for it and they count more
/// than 0 (see [`debt::count`]). Then, when one failed, the known debt goes to `gdd/debt.toml`,
/// committed alone on the current branch (see [`record`]); when none did, nothing is written.
pub fn adopt(root: &Path, state: &Path, commands: &Commands) -> Result<Adopted, String> {
    if let Some(known) = debt::read(root)? {
        let day = known.adopted;
        let said = format!("this project was already adopted on {day} ({PATH})");
        return Err(said);
    }
    let candidates = candidates(commands)?;
    let today = exceptions::today().to_string();
    let mut passed = Vec::new();
    let mut owed = Vec::new();
    let mut said: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for (gate, name, command) in candidates {
        match exec::run(command, root, &ENV, exec::TIMEOUT) {
            Ok(ran) if ran.ok() => passed.push(name.to_string()),
            ran => {
                let output = ran.map_or_else(|e| format!("cannot run sh: {e}"), |ran| ran.output);
                let texts = commands.count(name);
                let counted = texts.map(|texts| debt::count(&output, texts));
                said.entry(name).or_default().push(gates::kept(&output));
                owed.push(Debt {
                    command: name.to_string(),
                    gate,
                    since: today.clone(),
                    count: counted.filter(|&count| count > 0),
                });
            }
        }
    }
    for (name, outputs) in said {
        debt::write_new(state, &kept(name), outputs.join("\n").as_bytes())?;
    }
    owed.sort_by(|a, b| debt::risk(&a.command).cmp(&debt::risk(&b.command)));
    if !owed.is_empty() {
        let known = Known {
            adopted: today,
            debt: owed.clone(),
        };
        record(root, &known)?;
    }
    Ok(Adopted { passed, debt: owed })
}

/// The commands of `commands` that can become debt (see [`debt::can_owe`]): G2's, then G3's,
/// each with its gate. One whose name cannot name a file of its own is refused.
fn candidates(commands: &Commands) -> Result<Vec<(Gate, &str, &str)>, String> {
    let mut found = Vec::new();
    for gate in [Gate::G2, Gate::G3] {
        let all = commands.of(gate).iter();
        for (name, command) in all.filter(|(name, _)| debt::can_owe(name)) {
            if !names_a_file(name) {
                return Err(format!(
                    "{}: [{}] {name:?}: gdd adopt keeps what a command says in a file named after \
                     it, so its name takes only letters, digits, - and _",
                    gates::COMMANDS,
                    gate.key()
                ));
            }
            found.push((gate, name.as_str(), command.as_str()));
        }
    }
    Ok(found)
}

/// Whether `name` can name a file of its own: letters, digits, `-` and `_`, as a bare TOML key.
fn names_a_file(name: &str) -> bool {
    let plain = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
    !name.is_empty() && name.chars().all(plain)
}

/// Writes the known debt `known` to `gdd/debt.toml` of the project at `root` and commits that
/// file alone on the current branch. When git cannot commit it, the file goes, so the adoption
/// can run again.
fn record(root: &Path, known: &Known) -> Result<(), String> {
    debt::write_new(root, PATH, known.to_toml().as_bytes())?;
    let names = debt::by_risk(known.debt.iter().map(|debt| debt.command.as_str()));
    let message = format!("gdd adopt: known debt in {}", names.join(", "));
    match Git::new(root).commit_files(&[PATH.to_string()], &message) {
        Ok(()) => Ok(()),
        Err(e) => {
            files::remove(root, PATH).map_err(|e| format!("cannot remove {PATH}: {e}"))?;
            Err(format!(
                "cannot commit {PATH}: {e}. It was removed: run gdd adopt again once git can \
                 commit."
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::{Folder, Repo};
    use std::fs;
    use std::os::unix::fs::symlink;

    /// Gate commands of a project with code: lint and security fail; the ones that judge the
    /// change or its scenarios fail too, but never run; `types` passes only from the project
    /// root and with no change in its variables.
    const COMMANDS: &str = r#"
[g2]
lint = "echo unused import; exit 1"
types = 'test -f README.md && test -z "$GDD_CHANGE$GDD_COLOR$GDD_DIFF_FILES$GDD_FEATURES"'
unit = "true"
acceptance = "touch ran; exit 1"
empty = ""

[g3]
acceptance_full = "touch ran; exit 1"
security = "echo vulnerable qs >&2; exit 1"
secrets = "touch ran; exit 1"
mutation = "touch ran; exit 1"
"#;

    fn commands(text: &str) -> Commands {
        Commands::from_toml(text).unwrap()
    }

    fn debt(command: &str, gate: Gate, since: &str) -> Debt {
        Debt {
            command: command.to_string(),
            gate,
            since: since.to_string(),
            count: None,
        }
    }

    /// A git repository with an author of its own, and a state root apart.
    fn project() -> (Repo, Folder) {
        let repo = Repo::new();
        repo.identify();
        (repo, Folder::new())
    }

    fn adopted(repo: &Repo, state: &Folder, text: &str) -> Result<Adopted, String> {
        adopt(repo.root(), &state.0, &commands(text))
    }

    #[test]
    fn the_commands_that_fail_become_known_debt_committed_alone() {
        let (repo, state) = project();
        repo.write("loose.txt", "not committed\n");
        let adopted = adopted(&repo, &state, COMMANDS).unwrap();
        let today = exceptions::today().to_string();
        let expected = Adopted {
            passed: vec!["types".into(), "unit".into()],
            debt: vec![
                debt("security", Gate::G3, &today),
                debt("lint", Gate::G2, &today),
            ],
        };
        assert_eq!(adopted, expected);
        let printed = concat!(
            "Passed: types, unit\n",
            "Known debt: security, lint\n",
            "Proposed Refactors, riskiest first:\n",
            "  1. debt-security · Pay the known debt of security · .gdd/adoption/security.txt\n",
            "  2. debt-lint · Pay the known debt of lint · .gdd/adoption/lint.txt\n",
            "Create one with: bin/gdd new-change debt-<name> --type Refactor --title \"...\" ",
            "--paths ..."
        );
        assert_eq!(adopted.to_string(), printed);
        let said = |name: &str| fs::read_to_string(state.0.join(kept(name))).unwrap();
        assert_eq!(said("lint"), "unused import\n");
        assert_eq!(said("security"), "vulnerable qs\n");
        assert!(!repo.root().join(ADOPTION).exists());
        assert!(!repo.root().join("ran").exists());
        let known = Known {
            adopted: today,
            debt: expected.debt,
        };
        let written = fs::read_to_string(repo.root().join(PATH)).unwrap();
        assert_eq!(written, known.to_toml());
        let subject = repo.git(&["log", "-1", "--format=%s"]);
        assert_eq!(subject, "gdd adopt: known debt in security, lint");
        let files = repo.git(&["show", "--name-only", "--format=", "HEAD"]);
        assert_eq!(files, PATH);
        assert_eq!(repo.git(&["branch", "--show-current"]), "main");
        assert_eq!(repo.git(&["status", "--porcelain"]), "?? loose.txt");
    }

    #[test]
    fn the_known_debt_gets_the_count_of_what_each_command_said_when_it_has_one() {
        let (repo, state) = project();
        let text = "[g2]\nlint = \"echo Found 5 errors.; exit 1\"\n\
                    unit = \"echo 2 failed; exit 1\"\ntypes = \"echo Found 1 error >&2; exit 1\"\n\
                    [g3]\nsecurity = \"echo vulnerable; exit 1\"\n\
                    [count]\nlint = [\"Found {n} error\"]\ntypes = [\"Found {n} error\"]\n\
                    security = [\"{n} found\"]\n";
        let adopted = adopted(&repo, &state, text).unwrap();
        let today = exceptions::today().to_string();
        let counted = |command: &str, gate: Gate, count: u64| Debt {
            count: Some(count),
            ..debt(command, gate, &today)
        };
        // Without [count] for it, or when its texts count nothing, a debt has no count.
        let expected = [
            debt("security", Gate::G3, &today),
            counted("types", Gate::G2, 1),
            debt("unit", Gate::G2, &today),
            counted("lint", Gate::G2, 5),
        ];
        assert_eq!(adopted.debt, expected);
        let printed = adopted.to_string();
        let line = "\nKnown debt: security, types (1), unit, lint (5)\n";
        assert!(printed.contains(line), "{printed}");
        let refactor = "\n  4. debt-lint · Pay the known debt of lint · .gdd/adoption/lint.txt\n";
        assert!(printed.contains(refactor), "{printed}");
        let written = fs::read_to_string(repo.root().join(PATH)).unwrap();
        let since = format!("since = \"{today}\"\ncount = 5\n");
        assert!(written.ends_with(&since), "{written}");
        assert_eq!(debt::read(repo.root()).unwrap().unwrap().debt, expected);
    }

    #[test]
    fn a_project_is_adopted_only_once() {
        let (repo, state) = project();
        let known = "adopted = \"2026-10-01\"\n";
        repo.write(PATH, known);
        let err = adopted(&repo, &state, "[g2]\nlint = \"touch ran\"\n").unwrap_err();
        let said = "this project was already adopted on 2026-10-01 (gdd/debt.toml)";
        assert_eq!(err, said);
        repo.write(PATH, "[[debt]");
        let err = adopted(&repo, &state, "[g2]\nlint = \"touch ran\"\n").unwrap_err();
        assert!(err.starts_with("gdd/debt.toml: invalid TOML: "), "{err}");
        assert!(!repo.root().join("ran").exists());
    }

    #[test]
    fn without_failures_nothing_is_written_nor_committed() {
        let (repo, state) = project();
        let head = repo.git(&["rev-parse", "HEAD"]);
        let only_change = "[g2]\nacceptance = \"exit 1\"\n[g3]\nmutation = \"exit 1\"\n";
        let adopted = adopted(&repo, &state, only_change).unwrap();
        assert_eq!(adopted.to_string(), "Passed: none\nKnown debt: none");
        let adopted = adopt(repo.root(), &state.0, &commands("[g2]\nlint = \"true\"\n"));
        let printed = adopted.unwrap().to_string();
        assert_eq!(printed, "Passed: lint\nKnown debt: none");
        assert!(!repo.root().join(PATH).exists());
        assert!(!state.0.join(ADOPTION).exists());
        assert_eq!(repo.git(&["rev-parse", "HEAD"]), head);
    }

    #[test]
    fn a_name_in_both_gates_shows_the_count_of_each_when_they_differ() {
        let lint = |gate: Gate, count: Option<u64>| Debt {
            count,
            ..debt("lint", gate, "2026-10-27")
        };
        let shown = |g2: Option<u64>, g3: Option<u64>| {
            let debt = vec![lint(Gate::G2, g2), lint(Gate::G3, g3)];
            let printed = Adopted {
                passed: vec![],
                debt,
            };
            let (printed, start) = (printed.to_string(), "Known debt: ");
            let line = printed.lines().find_map(|line| line.strip_prefix(start));
            line.unwrap_or_default().to_string()
        };
        assert_eq!(shown(Some(5), None), "lint (5 in G2, no count in G3)");
        assert_eq!(shown(Some(5), Some(3)), "lint (5 in G2, 3 in G3)");
        assert_eq!(shown(Some(5), Some(5)), "lint (5)");
        assert_eq!(shown(None, None), "lint");
    }

    #[test]
    fn a_name_in_both_gates_keeps_what_each_run_said() {
        let (repo, state) = project();
        let text = "[g2]\nlint = \"echo two; exit 1\"\n[g3]\nlint = \"echo three; exit 1\"\n";
        let adopted = adopted(&repo, &state, text).unwrap();
        let today = exceptions::today().to_string();
        let both = [
            debt("lint", Gate::G2, &today),
            debt("lint", Gate::G3, &today),
        ];
        assert_eq!(adopted.debt, both);
        let printed = adopted.to_string();
        assert!(printed.contains("\nKnown debt: lint\n"), "{printed}");
        assert!(!printed.contains("2. debt-lint"), "{printed}");
        let said = fs::read_to_string(state.0.join(kept("lint"))).unwrap();
        assert_eq!(said, "two\n\nthree\n");
        let subject = repo.git(&["log", "-1", "--format=%s"]);
        assert_eq!(subject, "gdd adopt: known debt in lint");
    }

    #[test]
    fn a_name_that_cannot_name_a_file_is_refused_before_anything_runs() {
        let (repo, state) = project();
        for name in ["\"a/b\"", "\"\"", "\"x.y\""] {
            let text = format!("[g2]\nlint = \"touch ran\"\n[g3]\n{name} = \"exit 1\"\n");
            let err = adopted(&repo, &state, &text).unwrap_err();
            let start = format!("gdd/commands.toml: [g3] {name}: gdd adopt keeps");
            assert!(err.starts_with(&start), "{name}: {err}");
        }
        assert!(!repo.root().join("ran").exists());
        // A name that cannot name a file but never becomes debt never runs: it is not refused.
        let text = "[g2]\n\"acceptance/x\" = \"exit 1\"\nlint-rust_2 = \"true\"\nLint = \"true\"\n";
        let adopted = adopted(&repo, &state, text).unwrap();
        assert_eq!(adopted.passed, ["Lint", "lint-rust_2"]);
    }

    #[test]
    fn what_a_command_said_is_never_kept_through_a_link() {
        let (repo, state) = project();
        let outside = Folder::new();
        fs::create_dir_all(state.0.join(".gdd")).unwrap();
        symlink(&outside.0, state.0.join(ADOPTION)).unwrap();
        let err = adopted(&repo, &state, "[g2]\nlint = \"exit 1\"\n").unwrap_err();
        let said = "cannot write .gdd/adoption/lint.txt: .gdd/adoption is not a folder";
        assert_eq!(err, said);
        assert!(!outside.0.join("lint.txt").exists());
        assert!(!repo.root().join(PATH).exists());
    }

    #[test]
    fn when_git_cannot_commit_the_debt_file_goes() {
        let (plain, state) = (Folder::new(), Folder::new());
        let err = adopt(&plain.0, &state.0, &commands("[g2]\nlint = \"exit 1\"\n")).unwrap_err();
        let start = "cannot commit gdd/debt.toml: git add ";
        assert!(err.starts_with(start), "{err}");
        let end = "It was removed: run gdd adopt again once git can commit.";
        assert!(err.ends_with(end), "{err}");
        assert!(!plain.0.join(PATH).exists());
        assert!(state.0.join(kept("lint")).exists());
    }
}
