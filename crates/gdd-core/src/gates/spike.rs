//! G2 for a spike. A spike answers one question and its code never reaches main: the project's
//! G2 commands do not run. It passes when Path Guard passes, with the rules of the role it runs
//! for, and the branch of the spike adds or changes an ADR with its decision: a file
//! `docs/adr/*.md` with a line that, trimmed, is `## Decision`. Its report is G2's. In the
//! worktree of a variant of a spike, the branch is the variant's (see [`change::branch_at`]).

use std::path::Path;
use std::time::Instant;

use crate::agents::files;
use crate::change::{self, Record};
use crate::classify::classify;
use crate::git::{self, Git};
use crate::guard::{self, Hashes, Settings};

use super::{Error, Gate, PATH_GUARD, Project, Report, tenths};

/// The folder of the ADRs of a project, from its root: what a spike leaves for main.
pub const ADR_FOLDER: &str = "docs/adr/";

/// The heading of the section of an ADR that holds its decision.
const DECISION: &str = "## Decision";

/// The name of the check of the ADR in a report.
const ADR: &str = "adr";

/// Why G2 fails for a spike whose branch leaves no ADR with its decision.
pub const NO_ADR: &str =
    "a Spike leaves its decision in a new file under docs/adr/ with a ## Decision section";

/// Runs G2 for spike `id`, whose record is `record`, for `role`: Path Guard, from the snapshot
/// `snapshot` when there is one, then the ADR, among what the branch changed (see [`changed`]
/// and [`decided`]); no command runs. The spike is classified, for the color of its report, from
/// what it plans and what it touched against the main branch. Path Guard fails with the reason
/// when it cannot judge, and keeps its own time.
pub fn run(
    project: &Project,
    id: &str,
    record: &Record,
    role: &str,
    snapshot: Option<&Hashes>,
) -> Result<Report, Error> {
    let root = project.root;
    let touched = guard::touched(root, id, project.settings)?;
    let mut plan = record.plan.clone();
    plan.paths.extend(touched.iter().cloned());
    let classified = classify(project.zones, &project.rules.table, &plan)?;
    let mut report = Report::new(Gate::G2, classified.color);
    let started = Instant::now();
    let guarded = guard::path_guard(root, id, record, project.settings, role, snapshot);
    let guarded = guarded.map_or_else(|e| vec![e.to_string()], |verdict| verdict.failures);
    let took = tenths(started.elapsed().as_secs_f64());
    report.took(PATH_GUARD, guarded.is_empty(), took);
    report.judge(PATH_GUARD, guarded);
    let adr = if decided(root, &changed(root, id, project.settings)?) {
        Vec::new()
    } else {
        vec![NO_ADR.to_string()]
    };
    report.judge(ADR, adr);
    report.ok = report.failures.is_empty();
    Ok(report)
}

/// What the branch of spike `id`, checked out at `root`, changed since it left the main branch
/// `settings` name, as its working tree has it now: only what differs from where it left, so a
/// file changed and then put back, committed or not, does not count (see
/// [`Git::changed_since`]).
fn changed(root: &Path, id: &str, settings: &Settings) -> Result<Vec<String>, git::Error> {
    let git = Git::new(root);
    let branch = change::branch_at(&git, id)?;
    let left = git.merge_base(&settings.main_branch, &branch)?;
    git.changed_since(&branch, &left)
}

/// The first line of the decision of the ADR `text`: the first line after its `## Decision`
/// heading that is not blank, trimmed, unless the next heading comes first. `None` without one.
pub fn decision_of(text: &str) -> Option<String> {
    let mut lines = text.lines().skip_while(|line| line.trim() != DECISION);
    lines.next()?;
    let first = lines.find(|line| !line.trim().is_empty())?.trim();
    (!first.starts_with('#')).then(|| first.to_string())
}

/// Whether, among `changed`, what the branch of a spike checked out at `root` changed (see
/// [`changed`]), there is an ADR with its decision: a file right in [`ADR_FOLDER`] whose name
/// ends with `.md`, as the working tree has it, with a line that, trimmed, is `## Decision`. A
/// file the branch removed, or that is no file of its own, has none (see [`has_decision`]).
fn decided(root: &Path, changed: &[String]) -> bool {
    let mut adrs = changed.iter().filter(|path| is_adr(path));
    adrs.any(|path| has_decision(root, path))
}

/// Whether `path`, from the project root, is an ADR: a Markdown file right in [`ADR_FOLDER`].
/// What else the folder holds (an image, a file in a folder of its own) is no ADR.
pub fn is_adr(path: &str) -> bool {
    let name = path.strip_prefix(ADR_FOLDER);
    name.is_some_and(|name| name.ends_with(".md") && !name.contains('/'))
}

/// Whether the file `path` of the project at `root` has a line that, trimmed, is the heading of
/// the decision. A file that is not there, or is no file of its own, has none: a link, or one
/// on its way, would say what git does not keep (see [`files::found`]).
fn has_decision(root: &Path, path: &str) -> bool {
    let bytes = files::read(root, path).ok().flatten();
    let text = bytes.map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
    text.is_some_and(|text| text.lines().any(|line| line.trim() == DECISION))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::Change;
    use crate::config::Color;
    use crate::gates::{Commands, Rules};
    use crate::git::tests::Repo;
    use crate::zones::ZoneMap;
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;

    const RULES: &str = "[protected]\nall = [\"gdd/**\"]\nbuilder_build = [\"features/**\"]\n";

    const ZONES: &str = "default = \"yellow\"\n\
                         [[zone]]\ncolor = \"red\"\npaths = [\"src/storage/**\"]\n";

    /// An ADR with its decision.
    const DECIDED: &str = "# 0002 · Where the notes live\n## Decision\nSQLite.\n";

    /// Where the spike leaves its ADR.
    const ADR_PATH: &str = "docs/adr/0002-storage.md";

    /// A project in a git repository, its rules and an ADR without a decision on `main`, gate
    /// commands that would leave a file `ran`, and spike `c1` started on its branch, which plans
    /// `src/app.txt`.
    struct Spike {
        repo: Repo,
        record: Record,
        zones: ZoneMap,
        rules: Rules,
        settings: Settings,
        commands: Commands,
    }

    fn spike() -> Spike {
        let repo = Repo::new();
        repo.write("gdd/rules.toml", RULES);
        repo.write("docs/adr/0001-old.md", "# 0001 · Old\nNo decision yet.\n");
        repo.commit();
        let start = repo.start("c1");
        let record = Record {
            plan: Change {
                paths: vec!["src/app.txt".to_string()],
                min_color: None,
                resumable: false,
            },
            build_start: Some(start),
            decided_color: None,
            approved: Hashes::new(),
            features: vec![],
        };
        Spike {
            repo,
            record,
            zones: ZoneMap::from_toml(ZONES).unwrap(),
            rules: Rules::from_toml(RULES).unwrap(),
            settings: Settings::from_toml(RULES).unwrap(),
            commands: Commands::from_toml("[g2]\nunit = \"touch ran\"\n").unwrap(),
        }
    }

    impl Spike {
        /// Writes `text` to `path` of the project.
        fn write(&self, path: &str, text: &str) {
            self.repo.write(path, text);
        }

        /// Runs git with `args` in the project.
        fn git(&self, args: &[&str]) {
            self.repo.git(args);
        }

        /// The path `path` of the project.
        fn path(&self, path: &str) -> PathBuf {
            self.repo.root().join(path)
        }

        /// Its G2, for build mode: no command runs.
        fn run(&self) -> Report {
            let project = Project {
                root: self.repo.root(),
                state: self.repo.root(),
                zones: &self.zones,
                rules: &self.rules,
                settings: &self.settings,
                commands: &self.commands,
            };
            let report = run(&project, "c1", &self.record, "builder-build", None).unwrap();
            assert_eq!(report.ok, report.failures.is_empty(), "{report:?}");
            assert!(!self.path("ran").exists(), "a command ran");
            report
        }

        /// The names of the checks of its G2 that failed.
        fn failed(&self) -> Vec<String> {
            self.run().failures.into_iter().map(|f| f.name).collect()
        }
    }

    #[test]
    fn a_spike_with_an_adr_and_its_decision_passes_without_running_commands() {
        let project = spike();
        project.write("src/app.txt", "x\n");
        project.write(ADR_PATH, DECIDED);
        let report = project.run();
        assert!(report.ok, "{report:?}");
        assert_eq!((report.gate, report.color), (Gate::G2, Color::Yellow));
        assert_eq!(report.passed, [PATH_GUARD, ADR]);
        assert_eq!(report.skipped, Vec::<String>::new());
        let [timed] = report.timed.as_slice() else {
            panic!("Path Guard alone keeps its time: {report:?}");
        };
        assert_eq!((timed.name.as_str(), timed.ok), (PATH_GUARD, true));
        assert_eq!(report.duration_s, 0.0);
        assert_eq!(ADR, "adr");
        // What it touched colors it.
        project.write("src/storage/db.txt", "x\n");
        assert_eq!(project.run().color, Color::Red);
    }

    #[test]
    fn a_spike_without_an_adr_fails_and_says_where_its_decision_goes() {
        let project = spike();
        assert_eq!(project.failed(), [ADR]);
        project.write("src/app.txt", "x\n");
        let report = project.run();
        assert_eq!(report.passed, [PATH_GUARD]);
        assert_eq!(report.failures[0].tail, NO_ADR);
        assert!(report.failures[0].exit.is_none());
        assert_eq!(
            NO_ADR,
            "a Spike leaves its decision in a new file under docs/adr/ with a ## Decision section"
        );
    }

    #[test]
    fn an_adr_without_its_decision_section_does_not_count() {
        let project = spike();
        for text in [
            "# 0002\nSQLite.\n",
            "# 0002\n### Decision\n",
            "# 0002\n## Decisions\n",
            "# 0002\n## Decision taken\n",
            "# 0002 ## Decision\n",
        ] {
            project.write(ADR_PATH, text);
            assert_eq!(project.failed(), [ADR], "{text:?}");
        }
        // Trimmed, the line is the heading.
        project.write(ADR_PATH, "# 0002\n  ## Decision \t\nx\n");
        assert_eq!(project.failed(), Vec::<String>::new());
    }

    #[test]
    fn only_markdown_files_right_in_the_adr_folder_are_adrs() {
        let project = spike();
        for path in [
            "docs/adr/0002-storage.txt",
            "docs/adr/old/0002-storage.md",
            "docs/0002-storage.md",
            "docs/adr.md",
            "adr/0002-storage.md",
        ] {
            project.write(path, DECIDED);
            assert_eq!(project.failed(), [ADR], "{path}");
            fs::remove_file(project.path(path)).unwrap();
        }
        assert!(is_adr(ADR_PATH));
        assert!(!is_adr("docs/adr/"));
        assert_eq!(ADR_FOLDER, "docs/adr/");
    }

    #[test]
    fn an_adr_the_branch_changed_or_committed_counts_and_one_it_removed_does_not() {
        let project = spike();
        project.write("docs/adr/0001-old.md", DECIDED);
        assert_eq!(project.failed(), Vec::<String>::new());
        project.repo.commit();
        assert_eq!(project.failed(), Vec::<String>::new());
        // The decision on main, removed on the branch, is not the spike's.
        project.git(&["checkout", "-q", "main"]);
        project.git(&["merge", "-q", "change/c1"]);
        project.git(&["checkout", "-q", "change/c1"]);
        project.git(&["rm", "-q", "docs/adr/0001-old.md"]);
        assert_eq!(project.failed(), [ADR]);
        // An ADR on main the branch did not touch is not the spike's either.
        project.git(&["reset", "-q", "HEAD", "--", "docs/adr/0001-old.md"]);
        project.git(&["checkout", "-q", "--", "docs/adr/0001-old.md"]);
        assert!(project.path("docs/adr/0001-old.md").is_file());
        assert_eq!(project.failed(), [ADR]);
    }

    #[test]
    fn an_adr_changed_then_put_back_as_main_has_it_is_not_the_spikes() {
        let project = spike();
        // An ADR main decided already, before the spike.
        project.git(&["checkout", "-q", "main"]);
        project.write("docs/adr/0001-old.md", DECIDED);
        project.repo.commit();
        project.git(&["checkout", "-q", "change/c1"]);
        project.git(&["merge", "-q", "main"]);
        assert_eq!(project.failed(), [ADR]);
        project.write("docs/adr/0001-old.md", "# 0001 · Old\nRewritten.\n");
        project.repo.commit();
        project.write("docs/adr/0001-old.md", DECIDED);
        assert_eq!(project.failed(), [ADR], "put back in the working tree");
        project.repo.commit();
        assert_eq!(project.failed(), [ADR], "put back in a commit");
        // Changed for good, it is the spike's.
        project.write("docs/adr/0001-old.md", &format!("{DECIDED}And why.\n"));
        assert_eq!(project.failed(), Vec::<String>::new());
    }

    #[test]
    fn an_adr_behind_a_link_does_not_count() {
        let project = spike();
        project.write("notes/decided.md", DECIDED);
        symlink("../../notes/decided.md", project.path(ADR_PATH)).unwrap();
        assert_eq!(project.failed(), [ADR]);
        fs::remove_file(project.path(ADR_PATH)).unwrap();
        // Nor one in a folder of ADRs that is a link.
        fs::remove_dir_all(project.path("docs/adr")).unwrap();
        fs::create_dir_all(project.path("notes/adr")).unwrap();
        fs::write(project.path("notes/adr/0002-storage.md"), DECIDED).unwrap();
        symlink("../notes/adr", project.path("docs/adr")).unwrap();
        assert!(!has_decision(project.repo.root(), ADR_PATH));
        assert_eq!(project.failed(), [ADR]);
    }

    #[test]
    fn path_guard_judges_a_spike_as_it_judges_build_mode() {
        let project = spike();
        project.write(ADR_PATH, DECIDED);
        project.write("features/a.feature", "Feature: a\n");
        project.write("gdd/commands.toml", "[g2]\nunit = \"touch ran\"\n");
        let report = project.run();
        assert_eq!(report.passed, [ADR]);
        let [guarded] = report.failures.as_slice() else {
            panic!("Path Guard fails alone: {report:?}");
        };
        assert_eq!(guarded.name, PATH_GUARD);
        let protected = "features/a.feature: protected path for builder-build\n\
                         gdd/commands.toml: protected path for builder-build";
        assert!(guarded.tail.starts_with(protected), "{}", guarded.tail);
        assert!(!report.timed[0].ok);
    }

    #[test]
    fn a_spike_that_was_never_prepared_fails_path_guard() {
        let mut project = spike();
        project.write(ADR_PATH, DECIDED);
        project.record.build_start = None;
        let report = project.run();
        assert_eq!(report.passed, [ADR]);
        assert_eq!(report.failures[0].name, PATH_GUARD);
        let why = "Change c1 has not been prepared: Path Guard has no starting point.";
        assert_eq!(report.failures[0].tail, why);
    }

    #[test]
    fn in_the_worktree_of_a_variant_the_gate_judges_the_branch_of_the_variant() {
        let project = spike();
        project.git(&["checkout", "-q", "-b", "change/c1-v2", "main"]);
        project.write(ADR_PATH, DECIDED);
        project.write("features/a.feature", "Feature: a\n");
        let report = project.run();
        assert_eq!(report.passed, [ADR]);
        let protected = "features/a.feature: protected path for builder-build";
        let tail = &report.failures[0].tail;
        assert!(tail.starts_with(protected), "{tail}");
        // Committed on the branch of the variant, it still counts.
        fs::remove_file(project.path("features/a.feature")).unwrap();
        project.repo.commit();
        assert!(project.run().ok);
        // On the branch of the spike, what the variant did is not the spike's.
        project.git(&["checkout", "-q", "change/c1"]);
        assert_eq!(project.failed(), [ADR]);
    }

    #[test]
    fn the_decision_of_an_adr_is_the_first_line_under_its_heading() {
        assert_eq!(decision_of(DECIDED).as_deref(), Some("SQLite."));
        let spaced = "# 0002\n  ## Decision \n\n   Postgres, for its replicas.  \nMore.\n";
        let first = decision_of(spaced);
        assert_eq!(first.as_deref(), Some("Postgres, for its replicas."));
        for none in [
            "# 0002\nSQLite.\n",
            "# 0002\n## Decision\n",
            "# 0002\n## Decision\n\n \n",
            "# 0002\n## Decision\n## Consequences\nSlower.\n",
            "# 0002\n### Decision\nSQLite.\n",
            "",
        ] {
            assert_eq!(decision_of(none), None, "{none:?}");
        }
    }

    #[test]
    fn a_file_that_cannot_be_read_has_no_decision() {
        let project = spike();
        let root = project.repo.root();
        fs::create_dir_all(project.path(ADR_PATH)).unwrap();
        assert!(!has_decision(root, ADR_PATH));
        assert!(!has_decision(root, "missing.md"));
        let bytes = "docs/adr/0003-bytes.md";
        fs::write(project.path(bytes), b"\xff\xfe\n## Decision\n").unwrap();
        assert!(has_decision(root, bytes));
        symlink("0003-bytes.md", project.path("docs/adr/0004-link.md")).unwrap();
        assert!(!has_decision(root, "docs/adr/0004-link.md"));
    }
}
