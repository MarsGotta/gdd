//! Delivering a change once it joined the main branch, for the blueprint of software (see
//! [`super::touched::Software`]): the release commands and the rules they read ([`steps`],
//! [`command_limit`], [`version_rules`]), and each stage the core asks for ([`at`]): pre, then
//! production with the version the change gets, then observation, which rolls production back by
//! itself when it gets worse. A change is delivered from the main checkout, on the main branch,
//! with the commands and the rules the main checkout has: never from its own worktree. The
//! commands are read first, before anything else of a stage.

use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;

use crate::gates::{COMMANDS, release};
use crate::guard::{RULES, Stage};
use crate::layout::{join, named};
use crate::rules;
use crate::versions::{self, Promotion};

/// Change `id` delivered at `stage` with the release commands `steps` (see [`steps`]), from the
/// project root `root`, whose state root is `state` and whose main branch is `main`: in pre, its
/// tests and its demo (see [`release::pre`]); in production, refused without the approval of its
/// record when `needs_approval`, approved by the machines otherwise, with the version the rules
/// of the main checkout give it (see [`version_rules`] and [`versions::promote`]), saying with
/// `say` that it waits for the merge lock and the version it released; observed, rolled back
/// when production gets worse (see [`release::observe`]). Only production reads the rules and
/// says anything. What it printed, its JSON, or why it was refused or failed.
pub fn at(
    steps: &release::Steps,
    root: &Path,
    state: &Path,
    main: &str,
    id: &str,
    stage: Stage,
    say: &mut dyn FnMut(&str),
) -> Result<String, String> {
    match stage {
        Stage::Pre => release::pre(root, state, id, steps),
        Stage::Production { needs_approval } => {
            let promotion = Promotion {
                root,
                state,
                main,
                id,
                steps,
                needs_g4: needs_approval,
                rules: version_rules(state)?,
            };
            versions::promote(&promotion, say)
        }
        Stage::Observe => release::observe(root, state, id, steps),
    }
}

/// The release commands of the project at `root`, the `[release]` section of its commands file,
/// each within the time limit of its rules file (see [`command_limit`]). A file that cannot be
/// read, or is not valid, is an error that names it as the project names it.
pub fn steps(root: &Path) -> Result<release::Steps, String> {
    let limit = command_limit(root)?;
    let text = read(root, COMMANDS)?.unwrap_or_default();
    release::Steps::from_toml(&text, limit).map_err(|e| in_file(root, COMMANDS, e))
}

/// How long each gate and release command of the project at `root` may run, as its rules file
/// says (see [`rules::command_seconds`]).
pub fn command_limit(root: &Path) -> Result<Duration, String> {
    let text = read(root, RULES)?.unwrap_or_default();
    rules::command_seconds(&text).map_err(|e| in_file(root, RULES, e))
}

/// What the rules file of the main checkout, under the state root `state`, says of versions (see
/// [`versions::Rules`]): as the main branch has it, as the merge reads `push_main`. Its errors
/// name the rules file as the project names it.
pub fn version_rules(state: &Path) -> Result<versions::Rules, String> {
    let text = read(state, RULES)?.unwrap_or_default();
    versions::Rules::from_toml(&text).map_err(|e| match e.strip_prefix(RULES) {
        Some(rest) => format!("{}{rest}", named(state, RULES)),
        None => e,
    })
}

/// The text of the file `path`, written in the new names, under the folder `root`, where the
/// project keeps it (see [`join`]); `None` when there is none.
fn read(root: &Path, path: &str) -> Result<Option<String>, String> {
    match fs::read_to_string(join(root, path)) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("cannot read {}: {e}", named(root, path))),
    }
}

/// Why the file `path`, written in the new names, under the folder `root` is wrong, as the
/// project names it (see [`named`]): `<path>: <why>`.
fn in_file(root: &Path, path: &str, why: impl fmt::Display) -> String {
    format!("{}: {why}", named(root, path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    use crate::change;
    use crate::exec;
    use crate::git::Git;
    use crate::git::tests::{Folder, Repo};
    use crate::guard::{Blueprint, Error};
    use crate::log;
    use crate::recipe::queue;
    use crate::repo::touched::Software;
    use crate::versions::Mode;

    /// The record of change `c1`, joined to the main branch, a Feature with no gate decided.
    const RECORD: &str = r#"{"id": "c1", "title": "App", "type": "Feature", "state": "merged",
                             "gates": {}, "inbox": []}"#;

    /// The record of change `c1` with a changelog line: production gives it a version.
    const LINED: &str = r#"{"id": "c1", "title": "App", "type": "Feature", "state": "merged",
                            "gates": {}, "inbox": [],
                            "changelog": {"section": "Fixed", "line": "Crash"}}"#;

    /// What the main checkout runs in pre: its tests pass only on the main branch, its demo
    /// leaves a file where it ran.
    const ON_MAIN: &str = r#"[release]
simulated = true
tests_pre = "test \"$(git rev-parse --abbrev-ref HEAD)\" = main"
demo = "touch demo-ran"
"#;

    /// What the branch of a change runs in pre: tests that fail.
    const ON_ITS_BRANCH: &str = "[release]\nsimulated = true\ntests_pre = \"exit 7\"\n";

    /// Every step simulated.
    const SIMULATED: &str = "[release]\nsimulated = true\n";

    /// Tests in pre that fail, saying so.
    const RED_TESTS: &str = "[release]\nsimulated = true\ntests_pre = \"echo red; exit 1\"\n";

    /// A production that gets worse while it is observed.
    const WORSE: &str = "[release]\nsimulated = true\nobserve = \"echo worse; exit 1\"\n";

    /// A repository with an author that ignores `.tminus/`, whose main branch has the commands
    /// `commands`, with the record of change `c1`, `record`, in its main checkout.
    fn joined(commands: &str, record: &str) -> Repo {
        let repo = Repo::new();
        repo.identify();
        repo.write(".gitignore", ".tminus/\n");
        repo.write(COMMANDS, commands);
        repo.commit();
        repo.write(&change::record_path("c1"), record);
        repo
    }

    fn record(root: &Path) -> Value {
        let text = change::read_record(root, "c1").unwrap();
        serde_json::from_str(&text).unwrap()
    }

    /// What ran for the changes of the project whose state root is `root`, as its log keeps it.
    fn logged(root: &Path) -> Vec<String> {
        let text = fs::read_to_string(root.join(log::PATH)).unwrap_or_default();
        let lines = log::parse(&text).unwrap();
        lines.iter().map(|line| line.text("agent")).collect()
    }

    /// Change `c1` of the project at `root` delivered at `stage` by its software blueprint, with
    /// what it said on its way.
    fn delivered(root: &Path, stage: Stage) -> (Result<(), Error>, Vec<String>) {
        let mut said = Vec::new();
        let mut say = |line: &str| said.push(line.to_string());
        let done = Software::new(root, "main").deliver("c1", stage, &mut say);
        (done, said)
    }

    /// Whether `said` says nothing but that it waited for the merge lock, which another test may
    /// hold a moment.
    fn only_waited(said: &[String]) -> bool {
        said.iter().all(|line| line.as_str() == queue::WAITING)
    }

    #[test]
    fn pre_runs_its_tests_and_demo_from_the_main_checkout_with_its_commands() {
        let repo = joined(ON_MAIN, RECORD);
        // The branch of the change has commands of its own, checked out in its worktree.
        repo.git(&["checkout", "-q", "-b", "change/c1"]);
        repo.write(COMMANDS, ON_ITS_BRANCH);
        repo.commit();
        repo.git(&["checkout", "-q", "main"]);
        let places = Folder::new();
        let mut software = Software::with_places(repo.root(), &places.0, "main");
        let work = software.isolate("c1").unwrap().work;
        assert_eq!(Git::new(&work).current_branch().unwrap(), "change/c1");
        let mut said = Vec::new();
        let mut say = |line: &str| said.push(line.to_string());
        software.deliver("c1", Stage::Pre, &mut say).unwrap();
        assert!(said.is_empty(), "{said:?}");
        assert_eq!(record(repo.root())["state"], "in-pre");
        assert_eq!(logged(repo.root()), ["tests_pre", "demo"]);
        assert!(repo.root().join("demo-ran").is_file());
        assert!(!work.join("demo-ran").exists());
        assert!(!work.join(".tminus").exists());
    }

    #[test]
    fn what_pre_failed_on_is_the_error_as_it_said_it_and_the_change_is_in_pre() {
        let repo = joined(RED_TESTS, RECORD);
        let (done, said) = delivered(repo.root(), Stage::Pre);
        let err = done.unwrap_err();
        assert!(matches!(&err, Error::Blueprint(_)), "{err:?}");
        let why = "release.tests_pre failed (exit code 1):\nred";
        assert_eq!(err.to_string(), why);
        assert!(said.is_empty(), "{said:?}");
        assert_eq!(record(repo.root())["state"], "in-pre");
    }

    #[test]
    fn commands_the_main_checkout_cannot_take_deliver_nothing() {
        let repo = joined("[release", RECORD);
        let (done, said) = delivered(repo.root(), Stage::Pre);
        let err = done.unwrap_err().to_string();
        let invalid = "tminus/commands.toml: invalid TOML: ";
        assert!(err.starts_with(invalid), "{err}");
        assert!(said.is_empty(), "{said:?}");
        assert_eq!(record(repo.root())["state"], "merged");
        assert!(logged(repo.root()).is_empty());
    }

    #[test]
    fn delivered_at_pre_it_prints_what_its_steps_did() {
        let repo = joined(ON_MAIN, RECORD);
        let root = repo.root();
        let read = steps(root).unwrap();
        let mut quiet = |_: &str| {};
        let printed = at(&read, root, root, "main", "c1", Stage::Pre, &mut quiet);
        let printed: Value = serde_json::from_str(&printed.unwrap()).unwrap();
        assert_eq!(printed["ok"], true);
        assert_eq!(printed["steps"][0]["name"], "tests_pre");
        assert_eq!(printed["steps"][1]["name"], "demo");
    }

    #[test]
    fn production_without_the_need_of_an_approval_is_approved_by_the_machines() {
        let repo = joined(SIMULATED, RECORD);
        let root = repo.root();
        let machines = Stage::Production {
            needs_approval: false,
        };
        let (done, said) = delivered(root, machines);
        done.unwrap();
        assert!(only_waited(&said), "{said:?}");
        let approved = record(root);
        assert_eq!(approved["state"], "in-production");
        assert_eq!(approved["gates"]["G4"]["by"], "machines");
        assert_eq!(approved["version"], Value::Null);
        assert_eq!(logged(root), ["-", "promote"]);
    }

    #[test]
    fn production_that_needs_an_approval_is_refused_without_one() {
        let repo = joined(SIMULATED, RECORD);
        let root = repo.root();
        let approval = Stage::Production {
            needs_approval: true,
        };
        let (done, _) = delivered(root, approval);
        let err = done.unwrap_err();
        assert!(matches!(&err, Error::Blueprint(_)), "{err:?}");
        let refused = "This change needs a human G4 before promotion.";
        assert_eq!(err.to_string(), refused);
        assert_eq!(record(root)["state"], "merged");
        assert!(logged(root).is_empty());
        // With the person's approval in its record, it goes.
        let gate = r#""gates": {"G4": {"decision": "ok", "by": "human"}}"#;
        let approved = RECORD.replace(r#""gates": {}"#, gate);
        repo.write(&change::record_path("c1"), &approved);
        let (done, _) = delivered(root, approval);
        done.unwrap();
        assert_eq!(record(root)["state"], "in-production");
        assert_eq!(record(root)["gates"]["G4"]["by"], "human");
    }

    /// The merge commit of change `c1` on the main branch of `repo`, with a changelog line.
    fn merged_with_a_line(repo: &Repo) {
        let message = "tminus(c1): G3 Merge Gate · App\n\nRecord: x\n\n\
                       Changelog: Fixed · Crash\nBreaking: no";
        repo.git(&["commit", "-q", "--allow-empty", "-m", message]);
    }

    #[test]
    fn production_says_that_it_waits_for_the_merge_lock_then_the_version_it_released() {
        let repo = joined(SIMULATED, LINED);
        merged_with_a_line(&repo);
        let held = queue::lock(repo.root(), || {}).unwrap();
        let root = repo.root().to_path_buf();
        let waited = std::thread::spawn(move || {
            let machines = Stage::Production {
                needs_approval: false,
            };
            let (done, said) = delivered(&root, machines);
            done.map(|()| said)
        });
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(repo.git(&["tag", "--list"]), "");
        drop(held);
        let said = waited.join().unwrap().unwrap();
        assert_eq!(said, [queue::WAITING, "Released v0.1.0"]);
        assert_eq!(repo.git(&["tag", "--list"]), "v0.1.0");
        assert_eq!(record(repo.root())["version"], "0.1.0");
    }

    #[test]
    fn production_takes_no_version_when_the_rules_of_the_main_checkout_say_manual() {
        let repo = joined(SIMULATED, LINED);
        repo.write(RULES, "[versions]\nmode = \"manual\"\n");
        merged_with_a_line(&repo);
        let machines = Stage::Production {
            needs_approval: false,
        };
        let (done, said) = delivered(repo.root(), machines);
        done.unwrap();
        assert!(only_waited(&said), "{said:?}");
        assert_eq!(repo.git(&["tag", "--list"]), "");
        assert_eq!(record(repo.root())["version"], Value::Null);
        assert_eq!(record(repo.root())["state"], "in-production");
    }

    #[test]
    fn a_worse_production_is_rolled_back_by_itself_and_said() {
        let repo = joined(WORSE, RECORD);
        let root = repo.root();
        let (done, said) = delivered(root, Stage::Observe);
        let err = done.unwrap_err();
        assert!(matches!(&err, Error::Blueprint(_)), "{err:?}");
        let why = "release.observe failed (exit code 1):\nworse\nRollback: simulated\n\
                   Production failed while observing: rolled back to the previous version. A \
                   hotfix is needed. Message in .tminus/changes/c1/inbox.md";
        assert_eq!(err.to_string(), why);
        assert!(said.is_empty(), "{said:?}");
        assert_eq!(record(root)["state"], "rolled-back");
        assert_eq!(logged(root), ["observe", "rollback"]);
        // Production that holds closes it.
        let repo = joined(SIMULATED, RECORD);
        let (done, _) = delivered(repo.root(), Stage::Observe);
        done.unwrap();
        assert_eq!(record(repo.root())["state"], "closed");
    }

    #[test]
    fn the_commands_are_read_with_the_time_limit_of_the_rules() {
        let project = Folder::new();
        assert_eq!(command_limit(&project.0), Ok(exec::TIMEOUT));
        let none = release::Steps::from_toml("", exec::TIMEOUT).unwrap();
        assert_eq!(steps(&project.0), Ok(none));
        let commands = "[release]\nsimulated = true\ndemo = \"true\"\n";
        project.write(COMMANDS, commands);
        project.write(RULES, "[limits]\ncommand_seconds = 7\n");
        let seven = Duration::from_secs(7);
        assert_eq!(command_limit(&project.0), Ok(seven));
        let read = release::Steps::from_toml(commands, seven).unwrap();
        assert_eq!(steps(&project.0), Ok(read));
    }

    #[test]
    fn a_reader_names_the_file_it_cannot_read_or_take_as_the_project_names_it() {
        let project = Folder::new();
        project.write(COMMANDS, "[release");
        let err = steps(&project.0).unwrap_err();
        let invalid = "tminus/commands.toml: invalid TOML: ";
        assert!(err.starts_with(invalid), "{err}");
        project.write(RULES, "[limits]\ncommand_seconds = 0\n");
        let why = "tminus/rules.toml: invalid TOML: limits.command_seconds must be a whole \
                   number of seconds above 0, not 0";
        assert_eq!(command_limit(&project.0), Err(why.to_string()));
        assert_eq!(steps(&project.0), Err(why.to_string()));
        // A folder where the file goes is not a missing file.
        fs::remove_file(project.0.join(RULES)).unwrap();
        fs::create_dir_all(project.0.join(RULES)).unwrap();
        let unread = "cannot read tminus/rules.toml: Is a directory (os error 21)";
        assert_eq!(command_limit(&project.0), Err(unread.to_string()));
        assert_eq!(version_rules(&project.0), Err(unread.to_string()));
        let commands = Folder::new();
        fs::create_dir_all(commands.0.join(COMMANDS)).unwrap();
        let unread = "cannot read tminus/commands.toml: Is a directory (os error 21)";
        assert_eq!(steps(&commands.0), Err(unread.to_string()));
        // A project set up before the rename has them in gdd/.
        let old = Folder::new();
        old.write("gdd/project.toml", "");
        old.write("gdd/commands.toml", "[release");
        let err = steps(&old.0).unwrap_err();
        let invalid = "gdd/commands.toml: invalid TOML: ";
        assert!(err.starts_with(invalid), "{err}");
    }

    #[test]
    fn the_rules_of_versions_come_from_the_rules_file_of_the_main_checkout() {
        let state = Folder::new();
        assert_eq!(version_rules(&state.0), Ok(versions::Rules::default()));
        let manual = "push_main = true\n[versions]\nmode = \"manual\"\n";
        state.write(RULES, manual);
        let rules = versions::Rules {
            mode: Mode::Manual,
            push: true,
        };
        assert_eq!(version_rules(&state.0), Ok(rules));
        state.write(RULES, "[versions]\nmode = \"weekly\"\n");
        let err = version_rules(&state.0).unwrap_err();
        assert!(err.starts_with("tminus/rules.toml: "), "{err}");
        // A project set up before the rename names its own rules file.
        let old = Folder::new();
        old.write("gdd/project.toml", "");
        old.write("gdd/rules.toml", "[versions]\nmode = \"weekly\"\n");
        let err = version_rules(&old.0).unwrap_err();
        let rest = err.strip_prefix("gdd/rules.toml: ").unwrap_or_default();
        assert!(!rest.is_empty() && !rest.contains("tminus/"), "{err}");
    }

    #[test]
    fn the_recipe_leaves_delivering_a_change_to_the_blueprint() {
        let source = include_str!("../recipe/mod.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        assert!(production.len() < source.len(), "the recipe has no tests");
        for word in ["fn release(&mut self", ".release(", "versions::"] {
            assert!(!production.contains(word), "the recipe names {word}");
        }
    }
}
