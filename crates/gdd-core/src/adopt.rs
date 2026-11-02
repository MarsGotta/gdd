//! Adopting a project that already has code (`tminus adopt`): every gate command that judges the
//! whole project runs once on it, and the ones that fail become its known debt (see
//! [`crate::gates::debt`]), committed in `tminus/debt.toml`, with a Refactor proposed for each,
//! riskiest first. What each failing command said stays in `.tminus/adoption/` for that
//! Refactor. A project of the old layout keeps both in `gdd/` and `.gdd/` (see
//! [`crate::layout`]).

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::time::Duration;

use crate::agents::files;
use crate::exec;
use crate::gates::debt::{self, Debt, Known, Listed, PATH};
use crate::gates::{self, Commands, Gate, exceptions};
use crate::git::Git;
use crate::layout::{self, Layout};

/// Where what a failing command said is kept, from the state root, in the new names (see
/// [`crate::layout`]).
pub const ADOPTION: &str = ".tminus/adoption";

/// The variables the commands get, empty: the ones of the gates, without a change. In the new
/// names: a project of the old layout gets them as it names them, a project of the new one under
/// both names (see [`exec::run_limited`]).
const ENV: [&str; 4] = [
    "TMINUS_CHANGE",
    "TMINUS_COLOR",
    "TMINUS_DIFF_FILES",
    "TMINUS_FEATURES",
];

/// How to get the Refactor that pays a debt, in the new names: a project of the old layout names
/// its agents' commands its own way (see [`Layout::localize`]).
const CREATE: &str = "Ask your agent for each one: /tminus-change pay the debt of <name> \
                      ($tminus-change in Codex).";

/// What `tminus adopt` found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adopted {
    /// The names of the commands that passed, in the order they ran.
    pub passed: Vec<String>,
    /// The known debt it recorded, riskiest first.
    pub debt: Vec<Debt>,
    /// The layout of the project, whose agents' commands it names.
    project: Layout,
    /// The layout of the state root, where what each command said is kept.
    state: Layout,
}

impl fmt::Display for Adopted {
    /// What `tminus adopt` prints, without a line break at the end: the commands that passed and
    /// the known debt, each with its counts when a gate has one (see [`Listed::said`]); when
    /// there is, a Refactor per command, riskiest first, with where what it said is kept, and how
    /// to create one, as the project names things.
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
            writeln!(f, "  {}. {refactor} · {}", n + 1, kept(self.state, name))?;
        }
        write!(f, "{}", self.project.localize(CREATE))
    }
}

/// Where what the command `name` said is kept, from a state root of `layout`.
fn kept(layout: Layout, name: &str) -> String {
    format!("{}/{name}.txt", layout.path(ADOPTION))
}

/// Adopts the project at `root`, whose state root is `state` and whose gate commands are
/// `commands`. Refused before anything runs once the project has its file of known debt
/// (`tminus/debt.toml`), or when a command that can become debt has a name that cannot name a
/// file. Each command of G2, then of G3, that can become debt (see [`debt::can_owe`]) runs once
/// from `root`, with the variables of the gates, as the project names them, and no change, within
/// `limit`, the time limit of the rules (see [`exec::run_limited`]); it passes when it exits with
/// 0. What each one that failed said, as a gate keeps it (see [`gates::kept`]), goes to
/// `.tminus/adoption/<name>.txt` under the state root; its debt gets the count of what it said
/// when `[count]` has texts for it and they count more than 0 (see [`debt::count`]). Then, when
/// one failed, the known debt goes to the file of known debt, committed alone on the current
/// branch (see [`record`]); when none did, nothing is written. Paths, as each root names them
/// (see [`crate::layout`]).
pub fn adopt(
    root: &Path,
    state: &Path,
    commands: &Commands,
    limit: Duration,
) -> Result<Adopted, String> {
    if let Some(known) = debt::read(root)? {
        let day = known.adopted;
        let path = layout::named(root, PATH);
        let said = format!("this project was already adopted on {day} ({path})");
        return Err(said);
    }
    let candidates = candidates(root, commands)?;
    let today = exceptions::today().to_string();
    let (project, kept_in) = (Layout::at(root), Layout::at(state));
    let names = ENV.map(|name| project.localize(name));
    let env: Vec<(&str, &str)> = names.iter().map(|name| (name.as_str(), "")).collect();
    let mut passed = Vec::new();
    let mut owed = Vec::new();
    let mut said: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for (gate, name, command) in candidates {
        match exec::run_limited(command, root, &env, limit) {
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
        let path = kept(kept_in, name);
        debt::write_new(state, &path, outputs.join("\n").as_bytes())?;
    }
    owed.sort_by(|a, b| debt::risk(&a.command).cmp(&debt::risk(&b.command)));
    if !owed.is_empty() {
        let known = Known {
            adopted: today,
            debt: owed.clone(),
        };
        record(root, &known)?;
    }
    Ok(Adopted {
        passed,
        debt: owed,
        project,
        state: kept_in,
    })
}

/// The commands of `commands`, the gate commands of the project at `root`, that can become debt
/// (see [`debt::can_owe`]): G2's, then G3's, each with its gate. One whose name cannot name a
/// file of its own is refused, naming the commands file as the project names it.
fn candidates<'a>(
    root: &Path,
    commands: &'a Commands,
) -> Result<Vec<(Gate, &'a str, &'a str)>, String> {
    let mut found = Vec::new();
    for gate in [Gate::G2, Gate::G3] {
        let all = commands.of(gate).iter();
        for (name, command) in all.filter(|(name, _)| debt::can_owe(name)) {
            if !names_a_file(name) {
                return Err(format!(
                    "{}: [{}] {name:?}: tminus adopt keeps what a command says in a file named \
                     after it, so its name takes only letters, digits, - and _",
                    layout::named(root, gates::COMMANDS),
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

/// Writes the known debt `known` to the file of known debt of the project at `root`, as the
/// project names it (see [`debt::write`]), and commits that file alone on the current branch,
/// `tminus adopt: ...` (`gdd adopt: ...` in a project of the old layout). When git cannot commit
/// it, the file goes, so the adoption can run again.
fn record(root: &Path, known: &Known) -> Result<(), String> {
    debt::write(root, known)?;
    let path = layout::named(root, PATH);
    let names = debt::by_risk(known.debt.iter().map(|debt| debt.command.as_str()));
    let tool = Layout::at(root).name();
    let message = format!("{tool} adopt: known debt in {}", names.join(", "));
    let paths = [path.clone()];
    match Git::new(root).commit_files(&paths, &message) {
        Ok(()) => Ok(()),
        Err(e) => {
            files::remove(root, &path).map_err(|e| format!("cannot remove {path}: {e}"))?;
            Err(format!(
                "cannot commit {path}: {e}. It was removed: run tminus adopt again once git can \
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
types = 'test -f README.md && test -z "$TMINUS_CHANGE$TMINUS_COLOR$TMINUS_DIFF_FILES$TMINUS_FEATURES"'
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
        adopt(repo.root(), &state.0, &commands(text), exec::TIMEOUT)
    }

    /// Where what the command `name` said is kept, from a state root of the new layout.
    fn new_kept(name: &str) -> String {
        kept(Layout::Tminus, name)
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
            project: Layout::Tminus,
            state: Layout::Tminus,
        };
        assert_eq!(adopted, expected);
        let printed = concat!(
            "Passed: types, unit\n",
            "Known debt: security, lint\n",
            "Proposed Refactors, riskiest first:\n",
            "  1. debt-security · Pay the known debt of security · ",
            ".tminus/adoption/security.txt\n",
            "  2. debt-lint · Pay the known debt of lint · .tminus/adoption/lint.txt\n",
            "Ask your agent for each one: /tminus-change pay the debt of <name> ",
            "($tminus-change in Codex)."
        );
        assert_eq!(adopted.to_string(), printed);
        let said = |name: &str| fs::read_to_string(state.0.join(new_kept(name))).unwrap();
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
        assert_eq!(subject, "tminus adopt: known debt in security, lint");
        let files = repo.git(&["show", "--name-only", "--format=", "HEAD"]);
        assert_eq!(files, PATH);
        assert_eq!(repo.git(&["branch", "--show-current"]), "main");
        assert_eq!(repo.git(&["status", "--porcelain"]), "?? loose.txt");
    }

    #[test]
    fn a_project_of_the_old_layout_is_adopted_as_it_names_things() {
        let (repo, state) = project();
        repo.write("gdd/project.toml", "name = \"x\"\n");
        state.write("gdd/project.toml", "name = \"x\"\n");
        // The variables of the gates come as the project names them, without their twins.
        let text = "[g2]\nlint = 'echo \"${GDD_CHANGE-unset}|${TMINUS_CHANGE-none}\"; exit 1'\n";
        let done = adopted(&repo, &state, text).unwrap();
        let said = fs::read_to_string(state.0.join(".gdd/adoption/lint.txt")).unwrap();
        assert_eq!(said, "|none\n");
        assert!(!state.0.join(".tminus").exists());
        let printed = done.to_string();
        let refactor = "  1. debt-lint · Pay the known debt of lint · .gdd/adoption/lint.txt\n";
        assert!(printed.contains(refactor), "{printed}");
        let create = "Ask your agent for each one: /gdd-change pay the debt of <name> \
                      ($gdd-change in Codex).";
        assert!(printed.ends_with(create), "{printed}");
        let written = fs::read_to_string(repo.root().join("gdd/debt.toml")).unwrap();
        let header = "# Known debt recorded by gdd adopt.";
        assert!(written.starts_with(header), "{written}");
        assert!(!repo.root().join(PATH).exists());
        let subject = repo.git(&["log", "-1", "--format=%s"]);
        assert_eq!(subject, "gdd adopt: known debt in lint");
        let files = repo.git(&["show", "--name-only", "--format=", "HEAD"]);
        assert_eq!(files, "gdd/debt.toml");
        let err = adopted(&repo, &state, text).unwrap_err();
        let day = exceptions::today();
        let again = format!("this project was already adopted on {day} (gdd/debt.toml)");
        assert_eq!(err, again);
        let other = Repo::new();
        other.write("gdd/project.toml", "");
        let wrong = "[g3]\n\"x.y\" = \"exit 1\"\n";
        let err = adopted(&other, &state, wrong).unwrap_err();
        let start = "gdd/commands.toml: [g3] \"x.y\": tminus adopt keeps";
        assert!(err.starts_with(start), "{err}");
    }

    #[test]
    fn a_new_project_gets_the_variables_of_the_gates_under_both_names() {
        let (repo, state) = project();
        let text = "[g2]\nlint = 'echo \"${TMINUS_CHANGE+new}|${GDD_CHANGE+old}\"; exit 1'\n";
        adopted(&repo, &state, text).unwrap();
        let said = fs::read_to_string(state.0.join(new_kept("lint"))).unwrap();
        assert_eq!(said, "new|old\n");
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
        let refactor =
            "\n  4. debt-lint · Pay the known debt of lint · .tminus/adoption/lint.txt\n";
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
        let said = "this project was already adopted on 2026-10-01 (tminus/debt.toml)";
        assert_eq!(err, said);
        repo.write(PATH, "[[debt]");
        let err = adopted(&repo, &state, "[g2]\nlint = \"touch ran\"\n").unwrap_err();
        assert!(err.starts_with("tminus/debt.toml: invalid TOML: "), "{err}");
        assert!(!repo.root().join("ran").exists());
    }

    #[test]
    fn without_failures_nothing_is_written_nor_committed() {
        let (repo, state) = project();
        let head = repo.git(&["rev-parse", "HEAD"]);
        let only_change = "[g2]\nacceptance = \"exit 1\"\n[g3]\nmutation = \"exit 1\"\n";
        let adopted = adopted(&repo, &state, only_change).unwrap();
        assert_eq!(adopted.to_string(), "Passed: none\nKnown debt: none");
        let lint = commands("[g2]\nlint = \"true\"\n");
        let adopted = adopt(repo.root(), &state.0, &lint, exec::TIMEOUT);
        let printed = adopted.unwrap().to_string();
        assert_eq!(printed, "Passed: lint\nKnown debt: none");
        assert!(!repo.root().join(PATH).exists());
        assert!(!state.0.join(ADOPTION).exists());
        assert_eq!(repo.git(&["rev-parse", "HEAD"]), head);
    }

    #[test]
    fn a_command_out_of_the_time_limit_becomes_debt_saying_how_to_raise_it() {
        let (repo, state) = project();
        let slow = commands("[g2]\nlint = \"sleep 5\"\nunit = \"true\"\n");
        let adopted = adopt(repo.root(), &state.0, &slow, Duration::from_millis(300)).unwrap();
        let today = exceptions::today().to_string();
        assert_eq!(adopted.debt, [debt("lint", Gate::G2, &today)]);
        assert_eq!(adopted.passed, ["unit"]);
        let said = fs::read_to_string(state.0.join(new_kept("lint"))).unwrap();
        let raise = "timed out (0.3s): raise limits.command_seconds in tminus/rules.toml";
        assert_eq!(said, raise);
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
                project: Layout::Tminus,
                state: Layout::Tminus,
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
        let said = fs::read_to_string(state.0.join(new_kept("lint"))).unwrap();
        assert_eq!(said, "two\n\nthree\n");
        let subject = repo.git(&["log", "-1", "--format=%s"]);
        assert_eq!(subject, "tminus adopt: known debt in lint");
    }

    #[test]
    fn a_name_that_cannot_name_a_file_is_refused_before_anything_runs() {
        let (repo, state) = project();
        for name in ["\"a/b\"", "\"\"", "\"x.y\""] {
            let text = format!("[g2]\nlint = \"touch ran\"\n[g3]\n{name} = \"exit 1\"\n");
            let err = adopted(&repo, &state, &text).unwrap_err();
            let start = format!("tminus/commands.toml: [g3] {name}: tminus adopt keeps");
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
        fs::create_dir_all(state.0.join(".tminus")).unwrap();
        symlink(&outside.0, state.0.join(ADOPTION)).unwrap();
        let err = adopted(&repo, &state, "[g2]\nlint = \"exit 1\"\n").unwrap_err();
        let said = "cannot write .tminus/adoption/lint.txt: .tminus/adoption is not a folder";
        assert_eq!(err, said);
        assert!(!outside.0.join("lint.txt").exists());
        assert!(!repo.root().join(PATH).exists());
    }

    #[test]
    fn when_git_cannot_commit_the_debt_file_goes() {
        let (plain, state) = (Folder::new(), Folder::new());
        let lint = commands("[g2]\nlint = \"exit 1\"\n");
        let err = adopt(&plain.0, &state.0, &lint, exec::TIMEOUT).unwrap_err();
        let start = "cannot commit tminus/debt.toml: git add ";
        assert!(err.starts_with(start), "{err}");
        let end = "It was removed: run tminus adopt again once git can commit.";
        assert!(err.ends_with(end), "{err}");
        assert!(!plain.0.join(PATH).exists());
        assert!(state.0.join(new_kept("lint")).exists());
    }
}
