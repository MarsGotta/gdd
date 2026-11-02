//! What a change touched, for software: the paths of the project its branch and its working tree
//! changed, as git lists them, and the rules file as a commit had it. This is the blueprint the
//! core asks (see [`Blueprint`]); it reports items and bytes, never a verdict, a color or a hash:
//! judging them is the core's.

use std::path::Path;

use crate::change;
use crate::git::{self, Git};
use crate::guard::{Blueprint, Error, ItemId, Mark, Touched};

/// The blueprint of software: the project at a root, in its git repository, whose changes leave
/// from a main branch. An item is a path from the project root, a mark a commit id.
#[derive(Debug)]
pub struct Software<'a> {
    /// The project root.
    root: &'a Path,
    /// The branch the changes leave from.
    main: &'a str,
}

impl<'a> Software<'a> {
    /// The project at `root`, whose changes leave from the branch `main`.
    pub fn new(root: &'a Path, main: &'a str) -> Software<'a> {
        Software { root, main }
    }
}

impl Blueprint for Software<'_> {
    /// What change `id` touched since the commit `since`: what its branch changed since then
    /// and, while it is checked out, what the working tree changed, added or removed (see
    /// [`Git::changed_since`]). Without `since`, against the main branch (see [`against_main`]).
    /// In the worktree of a variant of a spike, what the variant's branch touched (see
    /// [`change::branch_at`]).
    fn touched(&mut self, id: &str, since: Option<&Mark>) -> Result<Touched, Error> {
        let Some(since) = since else {
            return Ok(against_main(self.root, id, self.main)?);
        };
        let git = Git::new(self.root);
        let branch = change::branch_at(&git, id)?;
        let paths = git.changed_since(&branch, since.as_str())?;
        Ok(paths.into_iter().collect())
    }

    /// The file `item` as the commit `mark` had it; `None` where it had none (see
    /// [`Git::file_at`]).
    fn rules_at(&mut self, mark: &Mark, item: &ItemId) -> Result<Option<Vec<u8>>, Error> {
        let git = Git::new(self.root);
        Ok(git.file_at(mark.as_str(), item.as_str())?)
    }
}

/// What change `id` of the project at `root` touched against the main branch `main`, which
/// counts when classifying it: what its branch changed since it left `main` and, while it is
/// checked out, what the working tree changed, added or removed (see [`Git::changed_from`]).
/// Nothing when the project is not in a git repository or the change has no branch yet. In the
/// worktree of a variant of a spike, what the variant's branch touched (see
/// [`change::branch_at`]).
pub fn against_main(root: &Path, id: &str, main: &str) -> Result<Touched, git::Error> {
    if !git::is_repository(root) {
        return Ok(Touched::default());
    }
    let git = Git::new(root);
    let paths = git.changed_from(main, &change::branch_at(&git, id)?)?;
    Ok(paths.into_iter().collect())
}

impl From<git::Error> for Error {
    /// What git said, as an error of the blueprint: its text, as it is.
    fn from(e: git::Error) -> Error {
        Error::Blueprint(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use crate::change::Record;
    use crate::classify::Change;
    use crate::config::Color;
    use crate::git::tests::{Folder, Repo};
    use crate::guard::{self, DELETED, Hashes, Scope, Settings, Verdict, file_hash};
    use crate::rules;
    use crate::zones::ZoneMap;

    const RULES: &str = r#"
[protected]
all = ["tminus/**"]
builder_build = ["features/**"]
builder_test = ["src/**"]
"#;

    const ZONES: &str = r#"
default = "yellow"
[[zone]]
color = "green"
paths = ["docs/**"]
[[zone]]
color = "red"
paths = ["src/storage/**"]
"#;

    /// The SHA-256 of `abc`.
    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    const NOTHING: [&str; 0] = [];

    fn settings() -> Settings {
        Settings::from_toml(RULES).unwrap()
    }

    /// The record of a change that plans `paths` and started at `start` (`None`: never prepared).
    fn record(paths: &[&str], start: Option<&str>) -> Record {
        Record {
            plan: Change {
                paths: paths.iter().map(|p| p.to_string()).collect(),
                min_color: None,
                resumable: false,
            },
            build_start: start.map(str::to_string),
            decided_color: None,
            approved: Hashes::new(),
            features: vec![],
        }
    }

    /// A repository whose `main` holds the rules and a scenario, with change `c1` started on its
    /// branch; and the commit it started at.
    fn started() -> (Repo, String) {
        let repo = Repo::new();
        repo.write("tminus/rules.toml", RULES);
        repo.write("features/a.feature", "Feature: a\n");
        repo.commit();
        let start = repo.start("c1");
        (repo, start)
    }

    /// Path Guard for `role` on change `c1` of `repo`, through its software blueprint, with the
    /// rules now `now` and the snapshot `before`, if any.
    fn judged(
        repo: &Repo,
        record: &Record,
        now: &Settings,
        role: &str,
        before: Option<&Hashes>,
    ) -> Result<Verdict, Error> {
        let mut software = Software::new(repo.root(), "main");
        guard::path_guard(&mut software, repo.root(), "c1", record, now, role, before)
    }

    /// The failures of Path Guard for `role` on change `c1`, with the rules now [`RULES`].
    fn guarded(repo: &Repo, record: &Record, role: &str, before: Option<&Hashes>) -> Vec<String> {
        let verdict = judged(repo, record, &settings(), role, before).unwrap();
        assert_eq!(verdict.ok, verdict.failures.is_empty());
        verdict.failures
    }

    /// What `tminus snapshot` records for change `c1` of `repo`.
    fn snapshot_of(repo: &Repo, record: &Record) -> Result<Hashes, Error> {
        let mut software = Software::new(repo.root(), "main");
        guard::snapshot(&mut software, repo.root(), "c1", record)
    }

    /// The scope check of change `c1`, with the zone map [`ZONES`] and the default rules.
    fn check_scope(repo: &Repo, record: &Record) -> Scope {
        let zones = ZoneMap::from_toml(ZONES).unwrap();
        let table = rules::default_table();
        let mut software = Software::new(repo.root(), "main");
        let checked = guard::scope(&mut software, "c1", record, &zones, &table).unwrap();
        assert_eq!(checked.ok, checked.failures.is_empty());
        checked
    }

    #[test]
    fn since_its_start_a_change_touched_what_it_committed_changed_added_or_removed() {
        let (repo, start) = started();
        let mut software = Software::new(repo.root(), "main");
        let since = Mark::new(start.as_str());
        let nothing = software.touched("c1", Some(&since)).unwrap();
        assert_eq!(nothing, Touched::default());
        repo.write("src/committed.rs", "x\n");
        repo.commit();
        repo.write("tminus/rules.toml", "# changed\n");
        fs::remove_file(repo.root().join("features/a.feature")).unwrap();
        repo.write("src/new.rs", "y\n");
        repo.write(".tminus/changes/c1/change.json", "{}");
        let touched = software.touched("c1", Some(&since)).unwrap();
        let all = [
            "features/a.feature",
            "src/committed.rs",
            "src/new.rs",
            "tminus/rules.toml",
        ];
        assert_eq!(touched.paths(), all);
        // Away from its branch, only what it committed counts.
        repo.git(&["checkout", "-q", "main"]);
        let away = software.touched("c1", Some(&since)).unwrap();
        assert_eq!(away.paths(), ["src/committed.rs"]);
    }

    #[test]
    fn in_the_worktree_of_a_variant_what_its_branch_touched_counts() {
        let repo = Repo::new();
        let start = repo.git(&["rev-parse", "HEAD"]);
        repo.git(&["checkout", "-q", "-b", "change/c1-v2"]);
        repo.write("docs/adr/0009-x.md", "x\n");
        let mut software = Software::new(repo.root(), "main");
        let since = software.touched("c1", Some(&Mark::new(start))).unwrap();
        assert_eq!(since.paths(), ["docs/adr/0009-x.md"]);
        let from_main = software.touched("c1", None).unwrap();
        assert_eq!(from_main, since);
    }

    #[test]
    fn against_main_a_change_touched_what_its_branch_changed_or_removed() {
        let plain = Folder::new();
        let outside = against_main(&plain.0, "c1", "main").unwrap();
        assert_eq!(outside, Touched::default());
        let (repo, _) = started();
        repo.write("src/storage/db.rs", "x\n");
        repo.write(".tminus/changes/c1/change.json", "{}");
        let touched = |id: &str, main: &str| against_main(repo.root(), id, main);
        let db = touched("c1", "main").unwrap();
        assert_eq!(db.paths(), ["src/storage/db.rs"]);
        let mut software = Software::new(repo.root(), "main");
        assert_eq!(software.touched("c1", None).unwrap(), db);
        fs::remove_file(repo.root().join("README.md")).unwrap();
        let removed = touched("c1", "main").unwrap();
        assert_eq!(removed.paths(), ["README.md", "src/storage/db.rs"]);
        assert_eq!(touched("c2", "main").unwrap(), Touched::default());
        let result = touched("c1", "trunk");
        assert!(matches!(result, Err(git::Error::Failed(_))));
    }

    #[test]
    fn the_rules_at_a_mark_are_the_bytes_the_commit_had() {
        let (repo, start) = started();
        repo.write("tminus/rules.toml", "# now\n");
        repo.commit();
        let mut software = Software::new(repo.root(), "main");
        let mark = Mark::new(start.as_str());
        let rules = ItemId::new("tminus/rules.toml");
        let then = software.rules_at(&mark, &rules).unwrap();
        assert_eq!(then, Some(RULES.as_bytes().to_vec()));
        let gdd = ItemId::new("gdd/rules.toml");
        assert_eq!(software.rules_at(&mark, &gdd).unwrap(), None);
        let unknown = Mark::new("0123abcd");
        let err = software.rules_at(&unknown, &rules).unwrap_err();
        assert!(err.to_string().starts_with("git ls-tree "), "{err}");
    }

    #[test]
    fn what_git_said_is_the_text_of_the_error() {
        let from_git = Error::from(git::Error::Revision("-x".into()));
        assert!(matches!(&from_git, Error::Blueprint(_)));
        assert_eq!(
            from_git.to_string(),
            "\"-x\" is not a revision git can take"
        );
        let plain = Folder::new();
        let mut software = Software::new(&plain.0, "main");
        let err = software.touched("c1", Some(&Mark::new("s1"))).unwrap_err();
        let failed = "git rev-parse --abbrev-ref HEAD failed";
        assert!(err.to_string().starts_with(failed), "{err}");
    }

    #[test]
    fn path_guard_names_what_the_role_must_not_touch() {
        let (repo, start) = started();
        let record = record(&["src/a.rs"], Some(&start));
        assert_eq!(guarded(&repo, &record, "builder-build", None), NOTHING);
        repo.write("src/a.rs", "x\n");
        repo.write("features/b.feature", "Feature: b\n");
        repo.commit();
        repo.write("tminus/rules.toml", "# touched\n");
        assert_eq!(
            guarded(&repo, &record, "builder-build", None),
            [
                "features/b.feature: protected path for builder-build",
                "tminus/rules.toml: protected path for builder-build",
            ]
        );
        assert_eq!(
            guarded(&repo, &record, "builder-test", None),
            [
                "src/a.rs: protected path for builder-test",
                "tminus/rules.toml: protected path for builder-test",
            ]
        );
        assert_eq!(
            guarded(&repo, &record, "reviewer", None),
            ["tminus/rules.toml: protected path for reviewer"]
        );
    }

    #[test]
    fn a_scenario_approved_in_g1_cannot_change_or_go_away() {
        let (repo, start) = started();
        let mut record = record(&[], Some(&start));
        let hash = file_hash(&repo.root().join("features/a.feature")).unwrap();
        record.approved.insert("features/a.feature".into(), hash);
        assert_eq!(guarded(&repo, &record, "builder-test", None), NOTHING);
        let failure = "features/a.feature: scenario approved in G1 and modified afterwards";
        repo.write("features/a.feature", "Feature: changed\n");
        assert_eq!(guarded(&repo, &record, "builder-test", None), [failure]);
        fs::remove_file(repo.root().join("features/a.feature")).unwrap();
        assert_eq!(guarded(&repo, &record, "builder-test", None), [failure]);
        record.approved.insert("x".repeat(300), ABC.into());
        let result = judged(&repo, &record, &settings(), "reviewer", None);
        assert!(matches!(result, Err(Error::Read(..))));
    }

    #[test]
    fn after_a_snapshot_only_what_changed_since_counts() {
        let (repo, start) = started();
        let record = record(&[], Some(&start));
        repo.write("src/a.rs", "1\n");
        repo.write("src/b.rs", "1\n");
        let before = snapshot_of(&repo, &record).unwrap();
        repo.write("src/b.rs", "2\n");
        repo.write("src/c.rs", "1\n");
        assert_eq!(
            guarded(&repo, &record, "builder-test", Some(&before)),
            [
                "src/b.rs: protected path for builder-test",
                "src/c.rs: protected path for builder-test",
            ]
        );
        assert_eq!(guarded(&repo, &record, "builder-test", None).len(), 3);
    }

    #[test]
    fn a_path_the_snapshot_hashed_counts_after_it_leaves_the_diff() {
        let (repo, start) = started();
        let record = record(&[], Some(&start));
        repo.write("features/a.feature", "Feature: changed\n");
        repo.write("features/b.feature", "Feature: b\n");
        let before = snapshot_of(&repo, &record).unwrap();
        assert_eq!(
            guarded(&repo, &record, "builder-build", Some(&before)),
            NOTHING
        );
        repo.write("features/a.feature", "Feature: a\n");
        fs::remove_file(repo.root().join("features/b.feature")).unwrap();
        assert_eq!(guarded(&repo, &record, "builder-build", None), NOTHING);
        assert_eq!(
            guarded(&repo, &record, "builder-build", Some(&before)),
            [
                "features/a.feature: protected path for builder-build",
                "features/b.feature: protected path for builder-build",
            ]
        );
    }

    #[test]
    fn deleting_the_rules_or_a_protection_frees_no_one() {
        let (repo, start) = started();
        let record = record(&[], Some(&start));
        let failures = |now: &Settings| {
            let verdict = judged(&repo, &record, now, "builder-build", None);
            verdict.unwrap().failures
        };
        fs::remove_file(repo.root().join("tminus/rules.toml")).unwrap();
        repo.write("features/b.feature", "Feature: b\n");
        repo.write("docs/x.md", "x\n");
        let empty = Settings::from_toml("").unwrap();
        assert_eq!(
            failures(&empty),
            [
                "features/b.feature: protected path for builder-build",
                "tminus/rules.toml: protected path for builder-build",
            ]
        );
        let added = Settings::from_toml("[protected]\nbuilder_build = [\"docs/**\"]\n").unwrap();
        assert_eq!(
            failures(&added),
            [
                "docs/x.md: protected path for builder-build",
                "features/b.feature: protected path for builder-build",
                "tminus/rules.toml: protected path for builder-build",
            ]
        );
    }

    #[test]
    fn moving_the_rules_to_the_name_of_the_other_layout_frees_no_one() {
        for (from, to) in [("gdd", "tminus"), ("tminus", "gdd")] {
            // The rules where the build started, under the name of one layout.
            let repo = Repo::new();
            let rules =
                format!("[protected]\nall = [\"{from}/**\"]\nbuilder_build = [\"features/**\"]\n");
            repo.write(&format!("{from}/project.toml"), "");
            repo.write(&format!("{from}/rules.toml"), &rules);
            repo.commit();
            let start = repo.start("c1");
            let record = record(&[], Some(&start));
            // Moved to the other one, without any protection.
            fs::remove_dir_all(repo.root().join(from)).unwrap();
            repo.write(&format!("{to}/project.toml"), "");
            repo.write(&format!("{to}/rules.toml"), "");
            repo.write("features/b.feature", "Feature: b\n");
            let empty = Settings::from_toml("").unwrap();
            let verdict = judged(&repo, &record, &empty, "builder-build", None);
            let failures = verdict.unwrap().failures;
            let guarded = "features/b.feature: protected path for builder-build";
            assert_eq!(failures[0], guarded);
            let left = format!("{from}/");
            let moved = failures.iter().filter(|f| f.starts_with(&left)).count();
            // Where it went is one of the core's own items too: protected all the same.
            assert_eq!((failures.len(), moved), (5, 2), "{from}: {failures:?}");
        }
    }

    #[test]
    fn without_rules_where_the_build_started_the_rules_now_count() {
        let repo = Repo::new();
        let start = repo.start("c1");
        repo.write("src/a.rs", "x\n");
        let failure = "src/a.rs: protected path for builder-test";
        let record = record(&[], Some(&start));
        assert_eq!(guarded(&repo, &record, "builder-test", None), [failure]);
    }

    #[test]
    fn broken_rules_where_the_build_started_are_an_error_that_names_them() {
        let broken: [&[u8]; 2] = [b"[protected]\nall = \"tminus/**\"\n", &[0xff, 0xfe]];
        for (text, folder) in broken.into_iter().zip(["tminus", "gdd"]) {
            let repo = Repo::new();
            fs::create_dir_all(repo.root().join(folder)).unwrap();
            fs::write(repo.root().join(folder).join("rules.toml"), text).unwrap();
            repo.commit();
            let start = repo.start("c1");
            let record = record(&[], Some(&start));
            let result = judged(&repo, &record, &settings(), "reviewer", None);
            let err = result.unwrap_err();
            let file = format!("{folder}/rules.toml");
            let Error::Rules(named, commit, _) = &err else {
                panic!("not the rules: {err:?}");
            };
            assert_eq!((named, commit), (&file, &start));
            let prefix = format!("{file} at {start}: invalid TOML: ");
            assert!(err.to_string().starts_with(&prefix), "{err}");
        }
    }

    #[test]
    fn a_snapshot_hashes_what_the_change_touched_as_it_is_now() {
        let (repo, start) = started();
        repo.write("src/a.rs", "abc");
        fs::remove_file(repo.root().join("features/a.feature")).unwrap();
        let hashes = snapshot_of(&repo, &record(&[], Some(&start))).unwrap();
        let expected = Hashes::from([
            ("features/a.feature".to_string(), DELETED.to_string()),
            ("src/a.rs".to_string(), ABC.to_string()),
        ]);
        assert_eq!(hashes, expected);
        let err = snapshot_of(&repo, &record(&[], None)).unwrap_err();
        assert_eq!(
            err.to_string(),
            "change c1 has not been prepared: its record has no build_start"
        );
    }

    #[test]
    fn the_scope_accepts_lower_colors_outside_the_plan_and_fails_higher_ones() {
        let (repo, start) = started();
        let record = record(&["src/app.rs", "tests/**"], Some(&start));
        repo.write("src/app.rs", "x\n");
        repo.write("tests/deep/a.rs", "x\n");
        let inside = check_scope(&repo, &record);
        let nothing_outside = Scope {
            ok: true,
            failures: vec![],
            accepted: vec![],
        };
        assert_eq!(inside, nothing_outside);
        repo.write("docs/notes.md", "x\n");
        repo.write("src/other.rs", "x\n");
        repo.write("src/storage/db.rs", "x\n");
        let outside = check_scope(&repo, &record);
        let failure = "src/storage/db.rs: outside the package and red (the change is yellow); \
                       remove it, or the Orchestrator widens the scope";
        assert_eq!(outside.failures, [failure]);
        assert_eq!(outside.accepted, ["docs/notes.md", "src/other.rs"]);
        assert!(!outside.ok);
    }

    #[test]
    fn the_color_of_the_change_is_the_decided_one_else_the_color_of_its_plan() {
        let (repo, start) = started();
        repo.write("src/storage/db.rs", "x\n");
        let mut record = record(&["docs/a.md"], Some(&start));
        let failure = "src/storage/db.rs: outside the package and red (the change is green); \
                       remove it, or the Orchestrator widens the scope";
        assert_eq!(check_scope(&repo, &record).failures, [failure]);
        record.plan.min_color = Some(Color::Red);
        assert_eq!(check_scope(&repo, &record).accepted, ["src/storage/db.rs"]);
        record.plan.min_color = None;
        record.decided_color = Some(Color::Red);
        assert_eq!(check_scope(&repo, &record).accepted, ["src/storage/db.rs"]);
        record.decided_color = Some(Color::Green);
        record.plan.min_color = Some(Color::Red);
        assert_eq!(check_scope(&repo, &record).failures, [failure]);
    }
}
