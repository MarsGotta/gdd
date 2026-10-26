//! Path Guard and the scope check, on what a change touched since its build started.
//!
//! Path Guard stops a role that touched what it must not: the protected paths of the rules
//! file, and the scenarios approved in G1. The scope check accepts the paths outside the plan of
//! a change whose color is not higher than the change's, and fails the others: the change would
//! need gates it was not given.

use std::collections::BTreeMap;
use std::fmt::{self, Write};
use std::fs;
use std::io::{self, ErrorKind::NotADirectory, ErrorKind::NotFound};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::change::{self, Record};
use crate::classify::classify;
use crate::config;
use crate::git::{self, Git};
use crate::rules::RulesTable;
use crate::zones::{ZoneMap, glob_matches};

/// The hash of a path where there is no file.
pub const DELETED: &str = "deleted";

/// Hashes of the paths a change touched, at one moment: path -> SHA-256 in hex, or [`DELETED`].
pub type Hashes = BTreeMap<String, String>;

/// What the rules file says to Path Guard: the main branch and the paths each role must not
/// change. The rest of the file is ignored.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Settings {
    /// Branch the changes leave from. Unset: `main`.
    #[serde(default = "default_main_branch")]
    pub main_branch: String,
    /// `[protected]`: `all` for every role, then one list per role (`builder_build` for the role
    /// `builder-build`).
    #[serde(default)]
    protected: BTreeMap<String, Vec<String>>,
}

fn default_main_branch() -> String {
    "main".to_string()
}

impl Settings {
    /// Reads `main_branch` and the `[protected]` section of a rules file.
    pub fn from_toml(text: &str) -> Result<Settings, config::Error> {
        toml::from_str(text).map_err(|e| config::Error::Toml(e.to_string()))
    }

    /// The patterns `role` must not touch: those of `all`, then its own.
    pub fn protected(&self, role: &str) -> Vec<&str> {
        let own = role.replace('-', "_");
        ["all", own.as_str()]
            .into_iter()
            .filter_map(|key| self.protected.get(key))
            .flatten()
            .map(String::as_str)
            .collect()
    }
}

/// What can go wrong in Path Guard, the scope check or a snapshot.
#[derive(Debug)]
pub enum Error {
    /// Reading git failed.
    Git(git::Error),
    /// A file of the project could not be read to hash it.
    Read(String, io::Error),
    /// The zone map or the rules table could not classify the plan.
    Config(config::Error),
    /// The change has no starting point: it was never prepared.
    NotPrepared(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Git(e) => write!(f, "{e}"),
            Error::Read(path, e) => write!(f, "cannot read {path}: {e}"),
            Error::Config(e) => write!(f, "{e}"),
            Error::NotPrepared(id) => write!(
                f,
                "change {id} has not been prepared: its record has no build_start"
            ),
        }
    }
}

impl std::error::Error for Error {}

impl From<git::Error> for Error {
    fn from(e: git::Error) -> Error {
        Error::Git(e)
    }
}

impl From<config::Error> for Error {
    fn from(e: config::Error) -> Error {
        Error::Config(e)
    }
}

/// The SHA-256 of the file at `path`, in hex; [`DELETED`] when there is no file there (nothing,
/// or a folder).
pub fn file_hash(path: &Path) -> io::Result<String> {
    let is_file = match fs::metadata(path) {
        Ok(meta) => meta.is_file(),
        Err(e) if matches!(e.kind(), NotFound | NotADirectory) => false,
        Err(e) => return Err(e),
    };
    if !is_file {
        return Ok(DELETED.to_string());
    }
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(fs::read(path)?).iter() {
        let _ = write!(hex, "{byte:02x}");
    }
    Ok(hex)
}

/// The hash of the file `path` of the project at `root`, see [`file_hash`].
fn project_hash(root: &Path, path: &str) -> Result<String, Error> {
    file_hash(&root.join(path)).map_err(|e| Error::Read(path.to_string(), e))
}

/// The hashes of `paths`, files of the project at `root`.
pub fn hashes(root: &Path, paths: &[String]) -> Result<Hashes, Error> {
    paths
        .iter()
        .map(|path| project_hash(root, path).map(|hash| (path.clone(), hash)))
        .collect()
}

/// The failure of `check` on change `id` when it was never prepared.
fn not_prepared(id: &str, check: &str) -> String {
    format!("Change {id} has not been prepared: {check} has no starting point.")
}

/// The verdict of Path Guard: `ok` when there are no failures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Verdict {
    /// Whether Path Guard passes.
    pub ok: bool,
    /// One line per path the role must not touch, or approved scenario that changed.
    pub failures: Vec<String>,
}

impl Verdict {
    fn new(failures: Vec<String>) -> Verdict {
        Verdict {
            ok: failures.is_empty(),
            failures,
        }
    }
}

/// Path Guard for `role` on change `id` of the project at `root`: the paths the change touched
/// since its build started that `role` must not touch, then the scenarios approved in G1 that
/// changed or went away since. With the hashes of a snapshot, only the paths whose hash changed
/// since it count as touched. A change never prepared fails: there is no starting point.
pub fn path_guard(
    root: &Path,
    id: &str,
    record: &Record,
    settings: &Settings,
    role: &str,
    snapshot: Option<&Hashes>,
) -> Result<Verdict, Error> {
    let Some(start) = record.build_start.as_deref() else {
        return Ok(Verdict::new(vec![not_prepared(id, "Path Guard")]));
    };
    let mut touched = Git::new(root).changed_since(&change::branch(id), start)?;
    if let Some(before) = snapshot {
        let now = hashes(root, &touched)?;
        touched.retain(|path| now.get(path) != before.get(path));
    }
    let protected = settings.protected(role);
    let mut failures: Vec<String> = touched
        .iter()
        .filter(|path| protected.iter().any(|pattern| glob_matches(pattern, path)))
        .map(|path| format!("{path}: protected path for {role}"))
        .collect();
    for (file, approved) in &record.approved {
        if project_hash(root, file)? != *approved {
            failures.push(format!(
                "{file}: scenario approved in G1 and modified afterwards"
            ));
        }
    }
    Ok(Verdict::new(failures))
}

/// The hashes of what change `id` of the project at `root` touched since its build started, as
/// they are now: what `gdd snapshot` records. A change never prepared is an error.
pub fn snapshot(root: &Path, id: &str, record: &Record) -> Result<Hashes, Error> {
    let Some(start) = record.build_start.as_deref() else {
        return Err(Error::NotPrepared(id.to_string()));
    };
    let touched = Git::new(root).changed_since(&change::branch(id), start)?;
    hashes(root, &touched)
}

/// The verdict of the scope check: `ok` when there are no failures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Scope {
    /// Whether the scope check passes.
    pub ok: bool,
    /// One line per path outside the plan with a color higher than the change's.
    pub failures: Vec<String>,
    /// Paths outside the plan with the change's color or a lower one, sorted.
    pub accepted: Vec<String>,
}

impl Scope {
    fn new(failures: Vec<String>, accepted: Vec<String>) -> Scope {
        Scope {
            ok: failures.is_empty(),
            failures,
            accepted,
        }
    }
}

/// The scope check of change `id` of the project at `root`: the paths it touched since its build
/// started that its plan does not include (planned paths are patterns, see [`glob_matches`]).
/// One with the change's color or a lower one is accepted; a higher one is a failure. The color
/// of the change is the one decided when it was prepared, else the color of its plan. A change
/// never prepared fails: there is no starting point.
pub fn scope(
    root: &Path,
    id: &str,
    record: &Record,
    zones: &ZoneMap,
    table: &RulesTable,
) -> Result<Scope, Error> {
    let Some(start) = record.build_start.as_deref() else {
        let failure = not_prepared(id, "the scope check");
        return Ok(Scope::new(vec![failure], vec![]));
    };
    let color = match record.decided_color {
        Some(color) => color,
        None => classify(zones, table, &record.plan)?.color,
    };
    let planned = &record.plan.paths;
    let (mut failures, mut accepted) = (Vec::new(), Vec::new());
    for path in Git::new(root).changed_since(&change::branch(id), start)? {
        if planned.iter().any(|plan| glob_matches(plan, &path)) {
            continue;
        }
        let path_color = zones.color_of(&path);
        if path_color > color {
            failures.push(format!(
                "{path}: outside the package and {path_color} (the change is {color}); \
                 remove it, or the Orchestrator widens the scope"
            ));
        } else {
            accepted.push(path);
        }
    }
    Ok(Scope::new(failures, accepted))
}

/// Paths change `id` of the project at `root` touched against the main branch, which count when
/// classifying it: nothing when the project is not in a git repository or the change has no
/// branch yet.
pub fn touched(root: &Path, id: &str, settings: &Settings) -> Result<Vec<String>, git::Error> {
    if !git::is_repository(root) {
        return Ok(Vec::new());
    }
    Git::new(root).changed_from(&settings.main_branch, &change::branch(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::Change;
    use crate::config::Color;
    use crate::git::tests::{Folder, Repo};
    use crate::rules;

    const RULES: &str = r#"
[protected]
all = ["gdd/**"]
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
        }
    }

    /// A repository whose `main` holds the rules and a scenario, with change `c1` started on its
    /// branch; and the commit it started at.
    fn started() -> (Repo, String) {
        let repo = Repo::new();
        repo.write("gdd/rules.toml", RULES);
        repo.write("features/a.feature", "Feature: a\n");
        repo.commit();
        let start = repo.start("c1");
        (repo, start)
    }

    /// The failures of Path Guard for `role` on change `c1`.
    fn guard(repo: &Repo, record: &Record, role: &str, snapshot: Option<&Hashes>) -> Vec<String> {
        let verdict = path_guard(repo.root(), "c1", record, &settings(), role, snapshot).unwrap();
        assert_eq!(verdict.ok, verdict.failures.is_empty());
        verdict.failures
    }

    /// The scope check of change `c1`, with the zone map [`ZONES`] and the default rules.
    fn check_scope(repo: &Repo, record: &Record) -> Scope {
        let zones = ZoneMap::from_toml(ZONES).unwrap();
        let checked = scope(repo.root(), "c1", record, &zones, &rules::default_table()).unwrap();
        assert_eq!(checked.ok, checked.failures.is_empty());
        checked
    }

    #[test]
    fn settings_come_from_the_rules_file() {
        let empty = Settings::from_toml("").unwrap();
        assert_eq!(empty.main_branch, "main");
        assert_eq!(empty.protected("builder-build"), NOTHING);
        let text = format!("main_branch = \"trunk\"\n[color.red.full]\nmutation = true\n{RULES}");
        let read = Settings::from_toml(&text).unwrap();
        assert_eq!(read.main_branch, "trunk");
        assert_eq!(read.protected("builder-build"), ["gdd/**", "features/**"]);
        assert_eq!(read.protected("builder_test"), ["gdd/**", "src/**"]);
        assert_eq!(read.protected("reviewer"), ["gdd/**"]);
        let wrong = Settings::from_toml("[protected]\nall = \"gdd/**\"\n");
        assert!(matches!(wrong, Err(config::Error::Toml(_))));
    }

    #[test]
    fn a_file_hash_is_its_sha256_or_deleted() {
        let folder = Folder::new();
        folder.write("a.txt", "abc");
        folder.write("sub/b.txt", "b");
        assert_eq!(file_hash(&folder.0.join("a.txt")).unwrap(), ABC);
        for nothing in ["missing.txt", "a.txt/inside", "sub"] {
            let hash = file_hash(&folder.0.join(nothing)).unwrap();
            assert_eq!(hash, DELETED, "{nothing}");
        }
        assert!(file_hash(&folder.0.join("x".repeat(300))).is_err());
    }

    #[test]
    fn a_file_that_cannot_be_read_is_an_error_that_names_it() {
        let folder = Folder::new();
        folder.write("a.txt", "abc");
        let long = "x".repeat(300);
        let ok = hashes(&folder.0, &["a.txt".to_string(), "b.txt".to_string()]).unwrap();
        let expected = Hashes::from([
            ("a.txt".to_string(), ABC.to_string()),
            ("b.txt".to_string(), DELETED.to_string()),
        ]);
        assert_eq!(ok, expected);
        let err = hashes(&folder.0, &["a.txt".to_string(), long.clone()]).unwrap_err();
        assert!(matches!(&err, Error::Read(path, _) if *path == long));
        let message = err.to_string();
        assert!(message.starts_with(&format!("cannot read {long}: ")));
    }

    #[test]
    fn a_change_never_prepared_fails_path_guard() {
        let folder = Folder::new();
        let never = record(&[], None);
        let verdict = path_guard(&folder.0, "c1", &never, &settings(), "reviewer", None).unwrap();
        let failure = "Change c1 has not been prepared: Path Guard has no starting point.";
        assert_eq!(
            verdict,
            Verdict {
                ok: false,
                failures: vec![failure.to_string()],
            }
        );
    }

    #[test]
    fn path_guard_names_what_the_role_must_not_touch() {
        let (repo, start) = started();
        let record = record(&["src/a.rs"], Some(&start));
        assert_eq!(guard(&repo, &record, "builder-build", None), NOTHING);
        repo.write("src/a.rs", "x\n");
        repo.write("features/b.feature", "Feature: b\n");
        repo.commit();
        repo.write("gdd/rules.toml", "# touched\n");
        assert_eq!(
            guard(&repo, &record, "builder-build", None),
            [
                "features/b.feature: protected path for builder-build",
                "gdd/rules.toml: protected path for builder-build",
            ]
        );
        assert_eq!(
            guard(&repo, &record, "builder-test", None),
            [
                "gdd/rules.toml: protected path for builder-test",
                "src/a.rs: protected path for builder-test",
            ]
        );
        assert_eq!(
            guard(&repo, &record, "reviewer", None),
            ["gdd/rules.toml: protected path for reviewer"]
        );
    }

    #[test]
    fn a_scenario_approved_in_g1_cannot_change_or_go_away() {
        let (repo, start) = started();
        let mut record = record(&[], Some(&start));
        let hash = file_hash(&repo.root().join("features/a.feature")).unwrap();
        record.approved.insert("features/a.feature".into(), hash);
        assert_eq!(guard(&repo, &record, "builder-test", None), NOTHING);
        let failure = "features/a.feature: scenario approved in G1 and modified afterwards";
        repo.write("features/a.feature", "Feature: changed\n");
        assert_eq!(guard(&repo, &record, "builder-test", None), [failure]);
        fs::remove_file(repo.root().join("features/a.feature")).unwrap();
        assert_eq!(guard(&repo, &record, "builder-test", None), [failure]);
        record.approved.insert("x".repeat(300), ABC.into());
        let result = path_guard(repo.root(), "c1", &record, &settings(), "reviewer", None);
        assert!(matches!(result, Err(Error::Read(..))));
    }

    #[test]
    fn after_a_snapshot_only_what_changed_since_counts() {
        let (repo, start) = started();
        let record = record(&[], Some(&start));
        repo.write("src/a.rs", "1\n");
        repo.write("src/b.rs", "1\n");
        let before = snapshot(repo.root(), "c1", &record).unwrap();
        repo.write("src/b.rs", "2\n");
        repo.write("src/c.rs", "1\n");
        assert_eq!(
            guard(&repo, &record, "builder-test", Some(&before)),
            [
                "src/b.rs: protected path for builder-test",
                "src/c.rs: protected path for builder-test",
            ]
        );
        assert_eq!(guard(&repo, &record, "builder-test", None).len(), 3);
    }

    #[test]
    fn a_snapshot_hashes_what_the_change_touched_as_it_is_now() {
        let (repo, start) = started();
        repo.write("src/a.rs", "abc");
        fs::remove_file(repo.root().join("features/a.feature")).unwrap();
        let hashes = snapshot(repo.root(), "c1", &record(&[], Some(&start))).unwrap();
        let expected = Hashes::from([
            ("features/a.feature".to_string(), DELETED.to_string()),
            ("src/a.rs".to_string(), ABC.to_string()),
        ]);
        assert_eq!(hashes, expected);
        let err = snapshot(repo.root(), "c1", &record(&[], None)).unwrap_err();
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
        assert_eq!(inside, Scope::new(vec![], vec![]));
        assert!(inside.ok);
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

    #[test]
    fn a_change_never_prepared_fails_the_scope_check() {
        let folder = Folder::new();
        let zones = ZoneMap::from_toml(ZONES).unwrap();
        let never = record(&[], None);
        let checked = scope(&folder.0, "c1", &never, &zones, &rules::default_table()).unwrap();
        let failure = "Change c1 has not been prepared: the scope check has no starting point.";
        assert_eq!(
            checked,
            Scope {
                ok: false,
                failures: vec![failure.to_string()],
                accepted: vec![],
            }
        );
    }

    #[test]
    fn as_json_the_verdicts_start_with_ok() {
        let verdict = serde_json::to_string(&Verdict::new(vec!["a".into()])).unwrap();
        assert_eq!(verdict, r#"{"ok":false,"failures":["a"]}"#);
        let checked = serde_json::to_string(&Scope::new(vec![], vec!["b".into()])).unwrap();
        assert_eq!(checked, r#"{"ok":true,"failures":[],"accepted":["b"]}"#);
    }

    #[test]
    fn classifying_counts_what_the_change_touched_in_git() {
        let plain = Folder::new();
        assert_eq!(touched(&plain.0, "c1", &settings()).unwrap(), NOTHING);
        let (repo, _) = started();
        repo.write("src/storage/db.rs", "x\n");
        repo.write(".gdd/changes/c1/change.json", "{}");
        assert_eq!(
            touched(repo.root(), "c1", &settings()).unwrap(),
            ["src/storage/db.rs"]
        );
        assert_eq!(touched(repo.root(), "c2", &settings()).unwrap(), NOTHING);
        let trunk = Settings::from_toml("main_branch = \"trunk\"").unwrap();
        let result = touched(repo.root(), "c1", &trunk);
        assert!(matches!(result, Err(git::Error::Failed(_))));
    }

    #[test]
    fn errors_say_what_went_wrong() {
        let from_git = Error::from(git::Error::Revision("-x".into()));
        assert_eq!(
            from_git.to_string(),
            "\"-x\" is not a revision git can take"
        );
        let from_config = Error::from(config::Error::UnknownColor("blue".into()));
        assert_eq!(
            from_config.to_string(),
            "unknown color \"blue\" (use green, yellow or red)"
        );
        let read = Error::Read("a.rs".into(), io::Error::other("busy"));
        assert_eq!(read.to_string(), "cannot read a.rs: busy");
    }
}
