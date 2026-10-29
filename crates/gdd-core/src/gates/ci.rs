//! What CI runs (`gdd ci`): on the branch of a change, no harness path changed against the main
//! branch, and G2 and G3 green. CI has only what the repository holds: the local state of the
//! tool (`.gdd/`) never travels with it, so CI reads nothing there and writes nothing there.
//! Without a change record there is no Path Guard per role: the harness paths stand for it.
//! A command with known debt only warns, as in the gates (see [`debt`]).

use std::fmt;

use crate::change;
use crate::classify::{Change, classify};
use crate::gates::{Gate, Judged, Project, Report, debt, exceptions};
use crate::git::{self, Git};
use crate::zones::glob_matches;

/// What CI found on a branch.
#[derive(Debug, Clone, PartialEq)]
pub struct Checked {
    /// The branch.
    pub branch: String,
    /// What it found when the branch is the branch of a change; nothing on any other branch.
    pub change: Option<Found>,
}

/// What CI found on the branch of a change.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    /// The paths it changed that are harness, sorted.
    pub harness: Vec<String>,
    /// G2 on it.
    pub g2: Report,
    /// G3 on it.
    pub g3: Report,
}

impl Checked {
    /// Whether CI passes: on the branch of a change, it changed no harness path and its G2 and
    /// G3 are green; any other branch passes.
    pub fn ok(&self) -> bool {
        self.change.as_ref().is_none_or(Found::ok)
    }
}

impl Found {
    /// Whether the change changed no harness path and its G2 and G3 are green.
    fn ok(&self) -> bool {
        self.harness.is_empty() && self.g2.ok && self.g3.ok
    }
}

impl fmt::Display for Checked {
    /// The lines a person reads in the CI log, without a line break at the end. On the branch
    /// of a change: one per harness path changed, how G2 and G3 ended, then whether CI passed.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some(found) = &self.change else {
            let branch = &self.branch;
            return write!(f, "Branch {branch}: nothing to check per change.");
        };
        for path in &found.harness {
            writeln!(f, "{path}: harness path changed inside a change")?;
        }
        for report in [&found.g2, &found.g3] {
            writeln!(f, "{}", ended(report))?;
        }
        let ci = if found.ok() { "passed" } else { "failed" };
        write!(f, "CI {ci}")
    }
}

/// How a gate ended: `G2 green`, or `G2 red: ` and the names of what failed; then, when some
/// command with known debt failed, ` · known debt still failing: ` and their names.
fn ended(report: &Report) -> String {
    let ended = if report.ok {
        format!("{} green", report.gate)
    } else {
        let failed: Vec<&str> = report.failures.iter().map(|f| f.name.as_str()).collect();
        format!("{} red: {}", report.gate, failed.join(", "))
    };
    if report.known_debt.is_empty() {
        return ended;
    }
    let names = report.known_debt.iter().map(|owed| owed.name.as_str());
    let owed: Vec<&str> = names.collect();
    format!("{ended} · known debt still failing: {}", owed.join(", "))
}

/// Checks the branch `head_ref` names, else the current branch, of the project `project`. On
/// the branch `change/<id>`: what `HEAD` committed since it left the main branch (`origin/<main>`
/// when the repository has it, else `<main>`), `.gdd/` included, whose harness paths fail CI;
/// then G2 and G3, run as `gdd g2` and `gdd g3` run them on a change classified from those
/// paths (see [`Judged`]), without Path Guard. Any other branch has nothing to check. An id that cannot name a change,
/// and a failure of git, are errors.
pub fn check(project: &Project, head_ref: Option<&str>) -> Result<Checked, String> {
    let git = Git::new(project.root);
    let said = |e: git::Error| e.to_string();
    let branch = match head_ref {
        Some(branch) => branch.to_string(),
        None => git.current_branch().map_err(said)?,
    };
    let Some(id) = branch.strip_prefix("change/") else {
        return Ok(Checked {
            branch,
            change: None,
        });
    };
    change::check_id(id)?;
    let main = &project.settings.main_branch;
    let remote = format!("origin/{main}");
    let base = if git.exists(&remote).map_err(said)? {
        &remote
    } else {
        main
    };
    let diff = git.committed_from(base, "HEAD").map_err(said)?;
    let rules = project.rules;
    let harness = diff
        .iter()
        .filter(|path| rules.harness.iter().any(|p| glob_matches(p, path)))
        .cloned()
        .collect();
    let plan = Change {
        paths: diff.clone(),
        min_color: None,
        resumable: false,
    };
    let decision = classify(project.zones, &rules.table, &plan).map_err(|e| e.to_string())?;
    let judged = Judged {
        id,
        features: decision.features.clone(),
        decision,
        diff: &diff,
    };
    let found = Found {
        harness,
        g2: run(project, &judged, Gate::G2),
        g3: run(project, &judged, Gate::G3),
    };
    Ok(Checked {
        branch,
        change: Some(found),
    })
}

/// `gate` on the change `judged`, as CI runs it: its commands, the ones with known debt only
/// warning, and, in G3, the scenario count and the check of the security exceptions.
fn run(project: &Project, judged: &Judged, gate: Gate) -> Report {
    let commands = project.commands.of(gate);
    let mut report = judged.report(gate, commands);
    let owed = debt::owed(&mut report, project.root, gate);
    judged.run_commands(&mut report, project.root, project.commands, &owed);
    if gate == Gate::G3 {
        judged.scenarios(&mut report, project);
        exceptions::judge(&mut report, &exceptions::read(project.root));
    }
    report.ok = report.failures.is_empty();
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Color;
    use crate::gates::{Commands, Rules};
    use crate::git::tests::{Folder, Repo};
    use crate::guard::Settings;
    use crate::zones::ZoneMap;

    const RULES: &str = "[protected]\nall = [\"gdd/**\", \"bin/**\", \".gdd/bin/**\"]\n";

    const ZONES: &str = "default = \"yellow\"\n\
                         [[zone]]\ncolor = \"green\"\npaths = [\"docs/**\"]\n";

    /// G2 checks the failing test and shows what its commands know; G3 always passes.
    const COMMANDS: &str = "[g2]\nunit = 'test ! -f tests/failing.txt'\n\
                            show = 'printf \"%s|%s|%s|%s\" \"$GDD_CHANGE\" \"$GDD_COLOR\" \
                            \"$GDD_DIFF_FILES\" \"$GDD_FEATURES\" > shown'\n\
                            [g3]\nacceptance = 'true'\nmutation = 'true'\n";

    /// A project in a git repository, its rules and one scenario committed on `main`.
    struct Ci {
        repo: Repo,
        zones: ZoneMap,
        rules: Rules,
        settings: Settings,
        commands: Commands,
    }

    impl Ci {
        fn new() -> Ci {
            let repo = Repo::new();
            repo.write("gdd/rules.toml", RULES);
            repo.write("features/a.feature", "Feature: a\n  Scenario: one\n");
            repo.commit();
            Ci {
                repo,
                zones: ZoneMap::from_toml(ZONES).unwrap(),
                rules: Rules::from_toml(RULES).unwrap(),
                settings: Settings::from_toml(RULES).unwrap(),
                commands: Commands::from_toml(COMMANDS).unwrap(),
            }
        }

        fn project<'a>(&'a self, root: &'a std::path::Path) -> Project<'a> {
            Project {
                root,
                state: root,
                zones: &self.zones,
                rules: &self.rules,
                settings: &self.settings,
                commands: &self.commands,
            }
        }

        fn check(&self, head_ref: Option<&str>) -> Result<Checked, String> {
            check(&self.project(self.repo.root()), head_ref)
        }

        /// Commits `path` on the current branch.
        fn commits(&self, path: &str) {
            self.repo.write(path, "x\n");
            self.repo.git(&["add", "--", path]);
            self.repo.git(&["commit", "-q", "-m", path]);
        }

        /// What the `show` command of G2 wrote.
        fn shown(&self) -> String {
            std::fs::read_to_string(self.repo.root().join("shown")).unwrap()
        }
    }

    /// What CI found on the branch of a change.
    fn found(checked: &Checked) -> &Found {
        checked.change.as_ref().expect("the branch of a change")
    }

    #[test]
    fn a_branch_that_is_not_a_change_has_nothing_to_check() {
        let ci = Ci::new();
        let checked = ci.check(None).unwrap();
        assert_eq!(checked.branch, "main");
        assert_eq!(checked.change, None);
        assert!(checked.ok());
        let said = "Branch main: nothing to check per change.";
        assert_eq!(checked.to_string(), said);
        let other = ci.check(Some("feature/x")).unwrap();
        let said = "Branch feature/x: nothing to check per change.";
        assert_eq!(other.to_string(), said);
        assert!(!ci.repo.root().join("shown").exists());
    }

    #[test]
    fn a_green_change_passes_and_its_commands_know_it() {
        let ci = Ci::new();
        ci.repo.start("c1");
        ci.commits("src/app.rs");
        ci.commits("features/b.feature");
        let checked = ci.check(None).unwrap();
        assert_eq!(checked.branch, "change/c1");
        let found = found(&checked);
        assert_eq!(found.harness, Vec::<String>::new());
        assert_eq!(found.g2.color, Color::Yellow);
        assert_eq!(found.g2.passed, ["show", "unit"]);
        let passed = ["acceptance", "mutation", "scenarios (1)", "exceptions"];
        assert_eq!(found.g3.passed, passed);
        let diff = "features/b.feature\nsrc/app.rs";
        let shown = format!("c1|yellow|{diff}|features/b.feature");
        assert_eq!(ci.shown(), shown);
        assert!(checked.ok());
        let said = "G2 green\nG3 green\nCI passed";
        assert_eq!(checked.to_string(), said);
        assert!(!ci.repo.root().join(".gdd").exists());
    }

    #[test]
    fn mutation_testing_runs_only_when_the_change_needs_it() {
        let mut ci = Ci::new();
        ci.commands = Commands::from_toml("[g3]\nmutation = 'exit 1'\n").unwrap();
        ci.repo.start("c1");
        ci.commits("docs/a.md");
        let checked = ci.check(None).unwrap();
        let g3 = &found(&checked).g3;
        assert_eq!((g3.color, g3.ok), (Color::Green, true));
        assert_eq!(g3.skipped, ["mutation"]);
    }

    #[test]
    fn harness_paths_fail_ci_and_only_what_head_committed_counts() {
        let ci = Ci::new();
        ci.repo.start("c1");
        ci.commits("bin/tool");
        ci.commits("gdd/zones.toml");
        ci.commits(".gdd/bin/tool");
        ci.repo.write("gdd/loose.toml", "x\n");
        let checked = ci.check(None).unwrap();
        let found = found(&checked);
        let harness = [".gdd/bin/tool", "bin/tool", "gdd/zones.toml"];
        assert_eq!(found.harness, harness);
        assert!(found.g2.ok && found.g3.ok, "{checked:?}");
        assert!(!checked.ok());
        let said = ".gdd/bin/tool: harness path changed inside a change\n\
                    bin/tool: harness path changed inside a change\n\
                    gdd/zones.toml: harness path changed inside a change\n\
                    G2 green\nG3 green\nCI failed";
        assert_eq!(checked.to_string(), said);
    }

    #[test]
    fn a_red_gate_fails_ci_naming_what_failed() {
        let mut ci = Ci::new();
        ci.repo.start("c1");
        ci.commits("tests/failing.txt");
        let checked = ci.check(None).unwrap();
        let found = found(&checked);
        assert!(!found.g2.ok && found.g3.ok, "{checked:?}");
        assert!(!checked.ok());
        let said = "G2 red: unit\nG3 green\nCI failed";
        assert_eq!(checked.to_string(), said);
        // A gate without commands, and G3 without scenarios, fail too.
        ci.commands = Commands::from_toml("[g3]\nacceptance = 'exit 1'\n").unwrap();
        let scenario = ci.repo.root().join("features/a.feature");
        std::fs::remove_file(scenario).unwrap();
        let checked = ci.check(None).unwrap();
        let said = "G2 red: G2 not configured\nG3 red: acceptance, scenarios\nCI failed";
        assert_eq!(checked.to_string(), said);
    }

    #[test]
    fn a_command_with_known_debt_warns_in_ci_without_failing_it() {
        let ci = Ci::new();
        let known = "adopted = \"2026-10-27\"\n\
                     [[debt]]\ncommand = \"unit\"\ngate = \"g2\"\nsince = \"2026-10-27\"\n";
        ci.repo.write(debt::PATH, known);
        ci.repo.git(&["add", "--", debt::PATH]);
        ci.repo.git(&["commit", "-q", "-m", "debt"]);
        ci.repo.start("c1");
        ci.commits("tests/failing.txt");
        let checked = ci.check(None).unwrap();
        let said = "G2 green · known debt still failing: unit\nG3 green\nCI passed";
        assert_eq!(checked.to_string(), said);
        assert!(checked.ok());
        let g2 = &found(&checked).g2;
        assert_eq!(g2.known_debt[0].name, "unit");
        // A debt file CI cannot read fails both gates, and the debt blocks again.
        ci.repo.write(debt::PATH, "[[debt]");
        let checked = ci.check(None).unwrap();
        let said = "G2 red: debt, unit\nG3 red: debt\nCI failed";
        assert_eq!(checked.to_string(), said);
    }

    #[test]
    fn a_command_that_judges_the_change_blocks_in_ci_even_when_the_debt_file_lists_it() {
        let mut ci = Ci::new();
        let commands = "[g2]\nunit = 'true'\n[g3]\nsecrets = 'exit 1'\n";
        ci.commands = Commands::from_toml(commands).unwrap();
        let listed = "adopted = \"2026-10-27\"\n\
                      [[debt]]\ncommand = \"secrets\"\ngate = \"g3\"\nsince = \"2026-10-27\"\n";
        ci.repo.write(debt::PATH, listed);
        ci.repo.commit();
        ci.repo.start("c1");
        ci.commits("src/app.rs");
        let checked = ci.check(None).unwrap();
        let said = "G2 red: debt\nG3 red: debt, secrets\nCI failed";
        assert_eq!(checked.to_string(), said);
        assert_eq!(found(&checked).g3.known_debt, Vec::<debt::Owed>::new());
    }

    #[test]
    fn an_exception_without_a_reason_fails_g3_in_ci() {
        let ci = Ci::new();
        ci.repo.start("c1");
        let deny = "[advisories]\nignore = [\"RUSTSEC-1\"]\n";
        ci.repo.write("deny.toml", deny);
        ci.repo.git(&["add", "--", "deny.toml"]);
        ci.repo.git(&["commit", "-q", "-m", "deny"]);
        let checked = ci.check(None).unwrap();
        let said = "G2 green\nG3 red: exceptions\nCI failed";
        assert_eq!(checked.to_string(), said);
        let g3 = &found(&checked).g3;
        assert_eq!(g3.failures[0].tail, "deny.toml: RUSTSEC-1 has no reason");
    }

    #[test]
    fn the_branch_of_a_pull_request_counts_and_its_id_must_name_a_change() {
        let ci = Ci::new();
        let checked = ci.check(Some("change/c1")).unwrap();
        assert_eq!(checked.branch, "change/c1");
        let found = found(&checked);
        assert_eq!(found.harness, Vec::<String>::new());
        assert_eq!(found.g2.color, Color::Yellow);
        assert!(checked.ok());
        assert_eq!(ci.shown(), "c1|yellow||");
        for branch in ["change/C1", "change/", "change/../x"] {
            let invalid = ci.check(Some(branch)).unwrap_err();
            assert!(invalid.starts_with("invalid change id"), "{branch}");
        }
    }

    #[test]
    fn the_base_is_the_main_branch_of_origin_when_there_is_one() {
        let ci = Ci::new();
        let pushed = ci.repo.git(&["rev-parse", "HEAD"]);
        // Main moves on locally after what origin has.
        ci.commits("bin/local");
        ci.repo.start("c1");
        ci.commits("src/app.rs");
        let checked = ci.check(None).unwrap();
        assert_eq!(found(&checked).harness, Vec::<String>::new());
        let origin = "refs/remotes/origin/main";
        ci.repo.git(&["update-ref", origin, &pushed]);
        let checked = ci.check(None).unwrap();
        assert_eq!(found(&checked).harness, ["bin/local"]);
    }

    #[test]
    fn a_failure_of_git_is_an_error() {
        let mut ci = Ci::new();
        ci.repo.start("c1");
        ci.settings = Settings::from_toml("main_branch = \"trunk\"").unwrap();
        let err = ci.check(None).unwrap_err();
        assert!(err.starts_with("git diff "), "{err}");
        let plain = Folder::new();
        let err = check(&ci.project(&plain.0), None).unwrap_err();
        let failed = "git rev-parse --abbrev-ref HEAD failed";
        assert!(err.starts_with(failed), "{err}");
    }
}
