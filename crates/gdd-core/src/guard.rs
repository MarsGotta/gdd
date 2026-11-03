//! Path Guard and the scope check, on what a change touched since its build started.
//!
//! Path Guard stops a role that touched what it must not: the protected paths of the rules
//! file, now and where the build started, and the scenarios approved in G1. The scope check
//! accepts the paths outside the plan of a change whose color is not higher than the change's,
//! and fails the others: the change would need gates it was not given.
//!
//! Both judge only the items a change touched, as its [`Blueprint`] reports them: how the work
//! is kept and listed is the blueprint's (for software, the paths of the project in its git
//! repository), what passes is the core's.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write};
use std::fs;
use std::io::{self, ErrorKind::NotADirectory, ErrorKind::NotFound};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::change::Record;
use crate::classify::{Classification, classify};
use crate::config;
use crate::layout::Layout;
use crate::rules::{CORE_ITEMS, RulesTable};
use crate::zones::{ZoneMap, glob_matches};

/// The hash of a path where there is no file.
pub const DELETED: &str = "deleted";

/// The rules file of a project, from its root, in the new names (see [`crate::layout`]).
pub const RULES: &str = "tminus/rules.toml";

/// The branch the changes leave from when the rules file names none.
pub const MAIN_BRANCH: &str = "main";

/// Hashes of the paths a change touched, at one moment: path -> SHA-256 in hex, or [`DELETED`].
pub type Hashes = BTreeMap<String, String>;

/// A stable id of an item a task touched, segments joined by `/` (software: the path from the
/// project root).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ItemId(String);

impl ItemId {
    /// The item whose id is `id`.
    pub fn new(id: impl Into<String>) -> ItemId {
        ItemId(id.into())
    }

    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A point of the work of a task the blueprint can answer about (software: a commit id).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mark(String);

impl Mark {
    /// The point whose id is `id`.
    pub fn new(id: impl Into<String>) -> Mark {
        Mark(id.into())
    }

    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Mark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a task touched: the only input the core judges risk from. Each item once, sorted. An
/// item added, changed or removed counts alike.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Touched {
    /// The items, each once, sorted.
    pub items: Vec<ItemId>,
}

impl Touched {
    /// Whether nothing was touched.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The ids of the items as text, in their order: for software, paths from the project root.
    pub fn paths(&self) -> Vec<String> {
        self.items.iter().map(|item| item.0.clone()).collect()
    }
}

impl FromIterator<String> for Touched {
    /// The items whose ids are `ids`, each once, sorted.
    fn from_iter<I: IntoIterator<Item = String>>(ids: I) -> Touched {
        let items: BTreeSet<ItemId> = ids.into_iter().map(ItemId).collect();
        Touched {
            items: items.into_iter().collect(),
        }
    }
}

/// Where the work of a task is done, as its blueprint made or reused it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    /// The project root there: where the agents and the steps of the task run.
    pub work: PathBuf,
    /// Whether `isolate` made it now: a launch refused before its build lets go of a place it
    /// made, never of one it found.
    pub made: bool,
}

/// How the main line stands for a task once it was brought up to date (see
/// [`Blueprint::refresh`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Moved {
    /// Taken: the work of the task now stands at this mark, the main line in it.
    Taken(Mark),
    /// Not taken, the work left as it was (kept first): why the task parks, as the blueprint
    /// says it (software: the items in conflict, then how to settle them by hand).
    Conflicts(String),
}

/// What the core asks a blueprint about the work of a task (ADR 0002, sections 2 and 5). Every
/// method that changes something can run twice: a relaunch calls it again.
pub trait Blueprint {
    /// Leaves the shared place on the main line before `id` gets a place of its own: the work an
    /// earlier run of `id` left there is kept as that run's; other work not kept refuses.
    fn settle(&mut self, id: &str) -> Result<(), Error>;

    /// Why the work of `id` cannot be joined to the main line now, if it cannot.
    fn blocked(&mut self, id: &str) -> Result<Option<String>, Error>;

    /// Whether the main line moved under the work of `id` since it left it.
    fn behind(&mut self, id: &str) -> Result<bool, Error>;

    /// Brings the latest main line into the work of `id` (see [`Moved`]).
    fn refresh(&mut self, id: &str) -> Result<Moved, Error>;

    /// Whether the work of `id` has anything the main line lacks.
    fn ahead(&mut self, id: &str) -> Result<bool, Error>;

    /// Joins the work of `id` to the main line, with the color and gates the core decided,
    /// `how`: what it printed. The core holds the lock and checked the gates first.
    fn integrate(&mut self, id: &str, how: &Classification) -> Result<String, Error>;

    /// A place of its own for change `id`, made or reused.
    fn isolate(&mut self, id: &str) -> Result<Place, Error>;

    /// Where the place of `id` is, as `isolate` gives it, without making it: it may not be there
    /// (`made` is false). What a task reads outside a run, as an answer to its last gate does,
    /// it reads there.
    fn place(&mut self, id: &str) -> Result<Place, Error>;

    /// Lets the place of `id` go; what it kept stays. Nothing there is no error.
    fn vacate(&mut self, id: &str) -> Result<(), Error>;

    /// The mark of the work of `id` now.
    fn mark(&mut self, id: &str) -> Result<Mark, Error>;

    /// What change `id` touched since `since`; with `None`, against the main line of work.
    fn touched(&mut self, id: &str, since: Option<&Mark>) -> Result<Touched, Error>;

    /// The bytes of `item` in the work of `id` now; `None` where there is no file. The core
    /// hashes them (see [`hash_of`]).
    fn read(&mut self, id: &str, item: &ItemId) -> Result<Option<Vec<u8>>, Error>;

    /// The bytes of `item` as they were at `mark`; `None` where there was none.
    fn rules_at(&mut self, mark: &Mark, item: &ItemId) -> Result<Option<Vec<u8>>, Error>;

    /// Keeps the work of `id` as `by` (a role or a person), saying `why`: only `items` when
    /// given (none given: nothing kept), else all of it. Nothing to keep is no error.
    fn keep(
        &mut self,
        id: &str,
        by: &str,
        why: &str,
        items: Option<&[ItemId]>,
    ) -> Result<(), Error>;

    /// Plants `content` at `item`, in place of what is there, in the place [`Blueprint::isolate`]
    /// made for `id` alone, to see a gate close; where `content` asks for what only the
    /// blueprint can make (software: a secret for each `{secret}`), it makes it now and keeps it
    /// nowhere. A blueprint plants only in a place made for a close test, never where a change
    /// works; one that plants nothing refuses.
    fn plant(&mut self, id: &str, item: &ItemId, content: &str) -> Result<(), Error> {
        let _ = (item, content);
        let why = format!("{id}: this blueprint plants no failure");
        Err(Error::Blueprint(why))
    }
}

/// What the rules file says to Path Guard: the main branch and the paths each role must not
/// change. The rest of the file is ignored.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Settings {
    /// Branch the changes leave from. Unset: [`MAIN_BRANCH`].
    #[serde(default = "default_main_branch")]
    pub main_branch: String,
    /// `[protected]`: `all` for every role, then one list per role (`builder_build` for the role
    /// `builder-build`).
    #[serde(default)]
    protected: BTreeMap<String, Vec<String>>,
}

fn default_main_branch() -> String {
    MAIN_BRANCH.to_string()
}

impl Settings {
    /// Reads `main_branch` and the `[protected]` section of a rules file.
    pub fn from_toml(text: &str) -> Result<Settings, config::Error> {
        toml::from_str(text).map_err(|e| config::Error::Toml(e.to_string()))
    }

    /// What the rules file said at `mark`, as `blueprint` has it, under the name it has in each
    /// layout (see [`crate::layout`]): the defaults where there was none. Moving the file to the
    /// name of the other layout frees no one from what it protected.
    fn committed(blueprint: &mut dyn Blueprint, mark: &Mark) -> Result<Vec<Settings>, Error> {
        let mut found = Vec::new();
        for layout in [Layout::Tminus, Layout::Gdd] {
            let file = layout.path(RULES);
            let item = ItemId::new(file.as_str());
            let bytes = blueprint.rules_at(mark, &item)?.unwrap_or_default();
            let read = String::from_utf8(bytes)
                .map_err(|e| config::Error::Toml(e.to_string()))
                .and_then(|text| Settings::from_toml(&text));
            match read {
                Ok(settings) => found.push(settings),
                Err(e) => return Err(Error::Rules(file, mark.to_string(), e)),
            }
        }
        Ok(found)
    }

    /// The patterns `role` must not touch: those of `all`, then its own, then the core's own
    /// items ([`CORE_ITEMS`]), which no role touches whatever the rules file says.
    pub fn protected(&self, role: &str) -> Vec<&str> {
        let own = role.replace('-', "_");
        ["all", own.as_str()]
            .into_iter()
            .filter_map(|key| self.protected.get(key))
            .flatten()
            .map(String::as_str)
            .chain(CORE_ITEMS)
            .collect()
    }
}

/// What can go wrong in Path Guard, the scope check or a snapshot.
#[derive(Debug)]
pub enum Error {
    /// The blueprint could not say what the change touched, or what the rules file was at a
    /// mark: what it said.
    Blueprint(String),
    /// A file of the project could not be read to hash it.
    Read(String, io::Error),
    /// The zone map or the rules table could not classify the plan.
    Config(config::Error),
    /// A rules file at a mark, the one where a build started, is not valid: the file, the mark
    /// and why.
    Rules(String, String, config::Error),
    /// The change has no starting point: it was never prepared.
    NotPrepared(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Blueprint(said) => f.write_str(said),
            Error::Read(path, e) => write!(f, "cannot read {path}: {e}"),
            Error::Config(e) => write!(f, "{e}"),
            Error::Rules(file, mark, e) => write!(f, "{file} at {mark}: {e}"),
            Error::NotPrepared(id) => write!(
                f,
                "change {id} has not been prepared: its record has no build_start"
            ),
        }
    }
}

impl std::error::Error for Error {}

impl From<config::Error> for Error {
    fn from(e: config::Error) -> Error {
        Error::Config(e)
    }
}

/// The hash of the bytes of a file: their SHA-256, in hex; [`DELETED`] for `None`, no file. One
/// formula for the files on disk (see [`file_hash`]) and for what a blueprint reads.
pub fn hash_of(bytes: Option<&[u8]>) -> String {
    let Some(bytes) = bytes else {
        return DELETED.to_string();
    };
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(bytes).iter() {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// The SHA-256 of the file at `path`, in hex; [`DELETED`] when there is no file there (nothing,
/// or a folder). See [`hash_of`].
pub fn file_hash(path: &Path) -> io::Result<String> {
    let is_file = match fs::metadata(path) {
        Ok(meta) => meta.is_file(),
        Err(e) if matches!(e.kind(), NotFound | NotADirectory) => false,
        Err(e) => return Err(e),
    };
    if !is_file {
        return Ok(hash_of(None));
    }
    Ok(hash_of(Some(&fs::read(path)?)))
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

/// Path Guard for `role` on change `id` of the project at `root`: the items `blueprint` reports
/// the change touched since its build started that `role` must not touch, then the scenarios
/// approved in G1 that changed or went away since. What `role` must not touch is what
/// `settings`, the rules file now, protects, and what the rules file protected when the build
/// started, as `blueprint` has it: deleting the file or one of its protections frees no one from
/// it. With the hashes of a snapshot, only the paths whose hash changed since it count as
/// touched. A change never prepared fails, and `blueprint` is asked nothing: there is no
/// starting point.
pub fn path_guard(
    blueprint: &mut dyn Blueprint,
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
    let start = Mark::new(start);
    let mut touched = blueprint.touched(id, Some(&start))?.paths();
    if let Some(before) = snapshot {
        touched = changed_after(root, touched, before)?;
    }
    let at_start = Settings::committed(blueprint, &start)?;
    let protected: Vec<&str> = std::iter::once(settings)
        .chain(&at_start)
        .flat_map(|rules| rules.protected(role))
        .collect();
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

/// Of the paths in `diff` and those the snapshot `before` hashed, the ones whose hash changed
/// since it. A path the snapshot hashed counts even when it left the diff: an untracked file
/// deleted, or a file put back as it was when the build started.
fn changed_after(root: &Path, diff: Vec<String>, before: &Hashes) -> Result<Vec<String>, Error> {
    let paths: BTreeSet<String> = diff.into_iter().chain(before.keys().cloned()).collect();
    let mut changed = Vec::new();
    for path in paths {
        if before.get(&path) != Some(&project_hash(root, &path)?) {
            changed.push(path);
        }
    }
    Ok(changed)
}

/// The hashes of what `blueprint` reports change `id` of the project at `root` touched since its
/// build started, as they are now: what `tminus snapshot` records. A change never prepared is an
/// error, and `blueprint` is asked nothing.
pub fn snapshot(
    blueprint: &mut dyn Blueprint,
    root: &Path,
    id: &str,
    record: &Record,
) -> Result<Hashes, Error> {
    let Some(start) = record.build_start.as_deref() else {
        return Err(Error::NotPrepared(id.to_string()));
    };
    let touched = blueprint.touched(id, Some(&Mark::new(start)))?;
    hashes(root, &touched.paths())
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

/// The scope check of change `id`: the items `blueprint` reports it touched since its build
/// started that its plan does not include (planned paths are patterns, see [`glob_matches`]).
/// One with the change's color or a lower one is accepted; a higher one is a failure. The color
/// of the change is the one decided when it was prepared, else the color of its plan. A change
/// never prepared fails, and `blueprint` is asked nothing: there is no starting point.
pub fn scope(
    blueprint: &mut dyn Blueprint,
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
    for path in blueprint.touched(id, Some(&Mark::new(start)))?.paths() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::Change;
    use crate::config::Color;
    use crate::git::tests::Folder;
    use crate::rules;

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

    /// The mark where the build of the changes of these tests started.
    const START: &str = "s1";

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

    /// A blueprint over a plain folder, with no repository: it reports the items and the rules it
    /// was given, or fails as it was told, and keeps what it was asked.
    #[derive(Default)]
    struct Fake {
        /// The items it reports touched.
        items: Vec<String>,
        /// The rules it has: (mark, item) -> bytes.
        rules: BTreeMap<(String, String), Vec<u8>>,
        /// What `touched` fails with, if it fails.
        touched_fails: Option<String>,
        /// What `rules_at` fails with, if it fails.
        rules_fail: Option<String>,
        /// What it was asked, in order.
        asked: Vec<String>,
    }

    impl Fake {
        /// A blueprint that reports `items` touched.
        fn touching(items: &[&str]) -> Fake {
            Fake {
                items: items.iter().map(|item| item.to_string()).collect(),
                ..Fake::default()
            }
        }

        /// Gives it the rules file `item` as it was at `mark`.
        fn with_rules(mut self, mark: &str, item: &str, bytes: &[u8]) -> Fake {
            let key = (mark.to_string(), item.to_string());
            self.rules.insert(key, bytes.to_vec());
            self
        }
    }

    impl Blueprint for Fake {
        fn settle(&mut self, id: &str) -> Result<(), Error> {
            Err(Error::Blueprint(format!("{id}: nothing is settled here")))
        }

        fn blocked(&mut self, id: &str) -> Result<Option<String>, Error> {
            Err(Error::Blueprint(format!("{id}: no main line here")))
        }

        fn behind(&mut self, id: &str) -> Result<bool, Error> {
            Err(Error::Blueprint(format!("{id}: no main line here")))
        }

        fn refresh(&mut self, id: &str) -> Result<Moved, Error> {
            Err(Error::Blueprint(format!("{id}: no main line here")))
        }

        fn ahead(&mut self, id: &str) -> Result<bool, Error> {
            Err(Error::Blueprint(format!("{id}: no main line here")))
        }

        fn integrate(&mut self, id: &str, _how: &Classification) -> Result<String, Error> {
            Err(Error::Blueprint(format!("{id}: no main line here")))
        }

        fn isolate(&mut self, id: &str) -> Result<Place, Error> {
            Err(Error::Blueprint(format!("{id}: no place here")))
        }

        fn place(&mut self, id: &str) -> Result<Place, Error> {
            Err(Error::Blueprint(format!("{id}: no place here")))
        }

        fn vacate(&mut self, id: &str) -> Result<(), Error> {
            Err(Error::Blueprint(format!("{id}: no place here")))
        }

        fn mark(&mut self, id: &str) -> Result<Mark, Error> {
            Err(Error::Blueprint(format!("{id}: no mark here")))
        }

        fn read(&mut self, id: &str, item: &ItemId) -> Result<Option<Vec<u8>>, Error> {
            Err(Error::Blueprint(format!("{id}: {item} is not read here")))
        }

        fn keep(
            &mut self,
            id: &str,
            _by: &str,
            _why: &str,
            _items: Option<&[ItemId]>,
        ) -> Result<(), Error> {
            Err(Error::Blueprint(format!("{id}: nothing is kept here")))
        }

        fn touched(&mut self, id: &str, since: Option<&Mark>) -> Result<Touched, Error> {
            let since = since.map_or_else(|| "main".to_string(), Mark::to_string);
            self.asked.push(format!("touched {id} since {since}"));
            match &self.touched_fails {
                Some(said) => Err(Error::Blueprint(said.clone())),
                None => Ok(self.items.iter().cloned().collect()),
            }
        }

        fn rules_at(&mut self, mark: &Mark, item: &ItemId) -> Result<Option<Vec<u8>>, Error> {
            self.asked.push(format!("rules {item} at {mark}"));
            if let Some(said) = &self.rules_fail {
                return Err(Error::Blueprint(said.clone()));
            }
            let key = (mark.to_string(), item.to_string());
            Ok(self.rules.get(&key).cloned())
        }
    }

    /// Path Guard for `role` on change `c1` of the plain folder `folder`, with what `fake`
    /// reports, the rules now `now` and the snapshot `before`, if any.
    fn judged(
        fake: &mut Fake,
        folder: &Folder,
        record: &Record,
        now: &Settings,
        role: &str,
        before: Option<&Hashes>,
    ) -> Result<Verdict, Error> {
        path_guard(fake, &folder.0, "c1", record, now, role, before)
    }

    /// The failures of Path Guard for `role` on change `c1` of the plain folder `folder`, with
    /// what `fake` reports and the rules now [`RULES`].
    fn guard(fake: &mut Fake, folder: &Folder, record: &Record, role: &str) -> Vec<String> {
        let verdict = judged(fake, folder, record, &settings(), role, None).unwrap();
        assert_eq!(verdict.ok, verdict.failures.is_empty());
        verdict.failures
    }

    /// The scope check of change `c1`, with what `fake` reports, the zone map [`ZONES`] and the
    /// default rules.
    fn check_scope(fake: &mut Fake, record: &Record) -> Scope {
        let zones = ZoneMap::from_toml(ZONES).unwrap();
        let checked = scope(fake, "c1", record, &zones, &rules::default_table()).unwrap();
        assert_eq!(checked.ok, checked.failures.is_empty());
        checked
    }

    #[test]
    fn the_core_judges_without_asking_git() {
        let sources = [
            ("guard.rs", include_str!("guard.rs")),
            ("classify.rs", include_str!("classify.rs")),
        ];
        for (name, source) in sources {
            let production = source.split("#[cfg(test)]").next().unwrap();
            assert!(production.len() < source.len(), "{name} has no tests");
            for word in ["crate::git", "Git::", "git::", "branch_at", "repo::"] {
                assert!(!production.contains(word), "{name} names {word}");
            }
        }
    }

    #[test]
    fn a_blueprint_that_plants_nothing_refuses_and_names_the_place() {
        let mut fake = Fake::default();
        let item = ItemId::new("src/bad.txt");
        let err = fake.plant("close.quality", &item, "bad").unwrap_err();
        assert!(matches!(&err, Error::Blueprint(_)), "{err:?}");
        let why = "close.quality: this blueprint plants no failure";
        assert_eq!(err.to_string(), why);
        assert!(fake.asked.is_empty());
    }

    #[test]
    fn an_item_and_a_mark_are_their_text() {
        let item = ItemId::new("src/a b.rs");
        assert_eq!(item.as_str(), "src/a b.rs");
        assert_eq!(item.to_string(), "src/a b.rs");
        assert_eq!(item, ItemId::new(String::from("src/a b.rs")));
        let mark = Mark::new("37cc864");
        assert_eq!(mark.as_str(), "37cc864");
        assert_eq!(mark.to_string(), "37cc864");
        assert_eq!(mark, Mark::new(String::from("37cc864")));
    }

    #[test]
    fn what_was_touched_holds_each_item_once_sorted() {
        let none = Touched::default();
        assert!(none.is_empty());
        assert_eq!(none.paths(), NOTHING);
        let ids = ["src/b.rs", "docs/a.md", "src/b.rs", "src/a.rs"].map(String::from);
        let touched: Touched = ids.into_iter().collect();
        assert!(!touched.is_empty());
        assert_eq!(touched.paths(), ["docs/a.md", "src/a.rs", "src/b.rs"]);
        let items = ["docs/a.md", "src/a.rs", "src/b.rs"].map(ItemId::new);
        assert_eq!(touched.items, items);
    }

    /// `patterns`, then the core's own items, which every role is kept from.
    fn and_the_core(patterns: &[&'static str]) -> Vec<&'static str> {
        patterns.iter().copied().chain(CORE_ITEMS).collect()
    }

    #[test]
    fn settings_come_from_the_rules_file() {
        let empty = Settings::from_toml("").unwrap();
        assert_eq!(empty.main_branch, "main");
        assert_eq!(MAIN_BRANCH, "main");
        assert_eq!(empty.protected("builder-build"), CORE_ITEMS);
        let text = format!("main_branch = \"trunk\"\n[color.red.full]\nmutation = true\n{RULES}");
        let read = Settings::from_toml(&text).unwrap();
        assert_eq!(read.main_branch, "trunk");
        assert_eq!(
            read.protected("builder-build"),
            and_the_core(&["tminus/**", "features/**"])
        );
        let test = and_the_core(&["tminus/**", "src/**"]);
        assert_eq!(read.protected("builder_test"), test);
        assert_eq!(read.protected("reviewer"), and_the_core(&["tminus/**"]));
        let wrong = Settings::from_toml("[protected]\nall = \"tminus/**\"\n");
        assert!(matches!(wrong, Err(config::Error::Toml(_))));
    }

    #[test]
    fn every_role_is_kept_from_the_core_items_whatever_the_rules_file_says() {
        let roles = ["builder-build", "builder-test", "reviewer"];
        let empty = Settings::from_toml("").unwrap();
        let other = "[protected]\nall = [\"bin/**\"]\nreviewer = [\"docs/**\"]\n";
        let other = Settings::from_toml(other).unwrap();
        for role in roles {
            assert_eq!(empty.protected(role), CORE_ITEMS, "{role}");
            let expected = match role {
                "reviewer" => and_the_core(&["bin/**", "docs/**"]),
                _ => and_the_core(&["bin/**"]),
            };
            assert_eq!(other.protected(role), expected, "{role}");
        }
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
    fn the_hash_of_bytes_is_the_hash_of_the_file_that_holds_them() {
        let folder = Folder::new();
        for bytes in ["abc", "", "two\nlines\n"] {
            folder.write("a.txt", bytes);
            let on_disk = file_hash(&folder.0.join("a.txt")).unwrap();
            assert_eq!(hash_of(Some(bytes.as_bytes())), on_disk, "{bytes:?}");
        }
        assert_eq!(hash_of(Some(b"abc")), ABC);
        assert_eq!(hash_of(None), DELETED);
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
    fn a_change_never_prepared_fails_path_guard_without_asking_the_blueprint() {
        let folder = Folder::new();
        let never = record(&[], None);
        let mut fake = Fake::touching(&["tminus/rules.toml"]);
        let verdict = judged(&mut fake, &folder, &never, &settings(), "reviewer", None);
        let failure = "Change c1 has not been prepared: Path Guard has no starting point.";
        assert_eq!(
            verdict.unwrap(),
            Verdict {
                ok: false,
                failures: vec![failure.to_string()],
            }
        );
        assert_eq!(fake.asked, NOTHING);
    }

    #[test]
    fn path_guard_judges_the_items_the_blueprint_reports_since_the_start() {
        let folder = Folder::new();
        let record = record(&["src/a.rs"], Some(START));
        let mut fake = Fake::default();
        let failures = guard(&mut fake, &folder, &record, "builder-build");
        assert_eq!(failures, NOTHING);
        let asked = [
            "touched c1 since s1",
            "rules tminus/rules.toml at s1",
            "rules gdd/rules.toml at s1",
        ];
        assert_eq!(fake.asked, asked);
        let touched = ["features/b.feature", "src/a.rs", "tminus/rules.toml"];
        let mut fake = Fake::touching(&touched);
        assert_eq!(
            guard(&mut fake, &folder, &record, "builder-build"),
            [
                "features/b.feature: protected path for builder-build",
                "tminus/rules.toml: protected path for builder-build",
            ]
        );
        assert_eq!(
            guard(&mut fake, &folder, &record, "builder-test"),
            [
                "src/a.rs: protected path for builder-test",
                "tminus/rules.toml: protected path for builder-test",
            ]
        );
        assert_eq!(
            guard(&mut fake, &folder, &record, "reviewer"),
            ["tminus/rules.toml: protected path for reviewer"]
        );
        // Nothing on disk was touched: only what the blueprint reports counts.
        assert!(!folder.0.join("src").exists());
    }

    #[test]
    fn the_rules_the_blueprint_had_at_the_start_protect_under_either_name() {
        let folder = Folder::new();
        let record = record(&[], Some(START));
        let now = Settings::from_toml("").unwrap();
        let docs = b"[protected]\nbuilder_build = [\"docs/**\"]\n";
        let src = b"[protected]\nall = [\"src/**\"]\n";
        for name in ["tminus/rules.toml", "gdd/rules.toml"] {
            let touched = Fake::touching(&["docs/x.md", "src/a.rs", "tests/a.rs"]);
            // Rules at another mark protect nothing here.
            let fake = touched.with_rules(START, name, docs);
            let mut fake = fake.with_rules("s0", name, src);
            let builder = "builder-build";
            let verdict = judged(&mut fake, &folder, &record, &now, builder, None);
            let failure = "docs/x.md: protected path for builder-build";
            assert_eq!(verdict.unwrap().failures, [failure], "{name}");
        }
    }

    #[test]
    fn broken_rules_at_the_start_are_an_error_that_names_them_and_the_mark() {
        let folder = Folder::new();
        let record = record(&[], Some(START));
        let broken: [&[u8]; 2] = [b"[protected]\nall = \"tminus/**\"\n", &[0xff, 0xfe]];
        let names = ["tminus/rules.toml", "gdd/rules.toml"];
        for (bytes, name) in broken.into_iter().zip(names) {
            let mut fake = Fake::default().with_rules(START, name, bytes);
            let result = judged(&mut fake, &folder, &record, &settings(), "reviewer", None);
            let err = result.unwrap_err();
            let Error::Rules(named, mark, _) = &err else {
                panic!("not the rules: {err:?}");
            };
            assert_eq!((named.as_str(), mark.as_str()), (name, START));
            let prefix = format!("{name} at {START}: invalid TOML: ");
            assert!(err.to_string().starts_with(&prefix), "{err}");
        }
    }

    #[test]
    fn a_scenario_approved_in_g1_cannot_change_or_go_away() {
        let folder = Folder::new();
        folder.write("features/a.feature", "Feature: a\n");
        let mut record = record(&[], Some(START));
        let hash = file_hash(&folder.0.join("features/a.feature")).unwrap();
        record.approved.insert("features/a.feature".into(), hash);
        let mut fake = Fake::default();
        let role = "builder-test";
        assert_eq!(guard(&mut fake, &folder, &record, role), NOTHING);
        let failure = "features/a.feature: scenario approved in G1 and modified afterwards";
        folder.write("features/a.feature", "Feature: changed\n");
        assert_eq!(guard(&mut fake, &folder, &record, role), [failure]);
        fs::remove_file(folder.0.join("features/a.feature")).unwrap();
        assert_eq!(guard(&mut fake, &folder, &record, role), [failure]);
    }

    #[test]
    fn after_a_snapshot_only_what_changed_since_counts() {
        let folder = Folder::new();
        let record = record(&[], Some(START));
        folder.write("src/a.rs", "1\n");
        folder.write("src/b.rs", "1\n");
        let mut fake = Fake::touching(&["src/a.rs", "src/b.rs"]);
        let before = snapshot(&mut fake, &folder.0, "c1", &record).unwrap();
        folder.write("src/b.rs", "2\n");
        folder.write("src/c.rs", "1\n");
        let mut fake = Fake::touching(&["src/a.rs", "src/b.rs", "src/c.rs"]);
        let (now, role) = (settings(), "builder-test");
        let since = judged(&mut fake, &folder, &record, &now, role, Some(&before));
        assert_eq!(
            since.unwrap().failures,
            [
                "src/b.rs: protected path for builder-test",
                "src/c.rs: protected path for builder-test",
            ]
        );
        assert_eq!(guard(&mut fake, &folder, &record, role).len(), 3);
    }

    #[test]
    fn the_snapshot_filter_reads_the_disk_before_the_rules_are_asked() {
        let folder = Folder::new();
        let record = record(&[], Some(START));
        let long = "x".repeat(300);
        let before = Hashes::from([(long.clone(), ABC.to_string())]);
        let mut fake = Fake {
            rules_fail: Some("no rules".into()),
            ..Fake::default()
        };
        let (now, role) = (settings(), "reviewer");
        let since = judged(&mut fake, &folder, &record, &now, role, Some(&before));
        assert!(matches!(since, Err(Error::Read(path, _)) if path == long));
        assert_eq!(fake.asked, ["touched c1 since s1"]);
        let all = judged(&mut fake, &folder, &record, &now, role, None);
        assert_eq!(all.unwrap_err().to_string(), "no rules");
    }

    #[test]
    fn an_error_of_the_blueprint_comes_back_with_its_text() {
        let folder = Folder::new();
        let record = record(&[], Some(START));
        let said = "cannot list what c1 touched";
        let mut fake = Fake {
            touched_fails: Some(said.into()),
            ..Fake::default()
        };
        let text = |err: Error| match err {
            Error::Blueprint(said) => said,
            other => panic!("not the blueprint: {other:?}"),
        };
        let now = settings();
        let guarded = judged(&mut fake, &folder, &record, &now, "reviewer", None);
        assert_eq!(text(guarded.unwrap_err()), said);
        let hashed = snapshot(&mut fake, &folder.0, "c1", &record);
        assert_eq!(text(hashed.unwrap_err()), said);
        let zones = ZoneMap::from_toml(ZONES).unwrap();
        let table = rules::default_table();
        let scoped = scope(&mut fake, "c1", &record, &zones, &table);
        assert_eq!(text(scoped.unwrap_err()), said);
        assert_eq!(fake.asked, ["touched c1 since s1"; 3]);
    }

    #[test]
    fn a_snapshot_hashes_the_items_the_blueprint_reports_as_they_are_now() {
        let folder = Folder::new();
        folder.write("src/a.rs", "abc");
        let started = record(&[], Some(START));
        let mut fake = Fake::touching(&["src/a.rs", "features/a.feature"]);
        let hashes = snapshot(&mut fake, &folder.0, "c1", &started).unwrap();
        let expected = Hashes::from([
            ("features/a.feature".to_string(), DELETED.to_string()),
            ("src/a.rs".to_string(), ABC.to_string()),
        ]);
        assert_eq!(hashes, expected);
        assert_eq!(fake.asked, ["touched c1 since s1"]);
        let never = record(&[], None);
        let err = snapshot(&mut fake, &folder.0, "c1", &never).unwrap_err();
        assert_eq!(
            err.to_string(),
            "change c1 has not been prepared: its record has no build_start"
        );
        assert_eq!(fake.asked, ["touched c1 since s1"]);
    }

    #[test]
    fn the_scope_judges_the_items_the_blueprint_reports_outside_the_plan() {
        let record = record(&["src/app.rs", "tests/**"], Some(START));
        let mut inside = Fake::touching(&["src/app.rs", "tests/deep/a.rs"]);
        let checked = check_scope(&mut inside, &record);
        assert_eq!(checked, Scope::new(vec![], vec![]));
        assert_eq!(inside.asked, ["touched c1 since s1"]);
        let touched = [
            "docs/notes.md",
            "src/app.rs",
            "src/other.rs",
            "src/storage/db.rs",
        ];
        let outside = check_scope(&mut Fake::touching(&touched), &record);
        let failure = "src/storage/db.rs: outside the package and red (the change is yellow); \
                       remove it, or the Orchestrator widens the scope";
        assert_eq!(outside.failures, [failure]);
        assert_eq!(outside.accepted, ["docs/notes.md", "src/other.rs"]);
        assert!(!outside.ok);
        let mut decided = record.clone();
        decided.decided_color = Some(Color::Red);
        let accepted = check_scope(&mut Fake::touching(&touched), &decided).accepted;
        let all = ["docs/notes.md", "src/other.rs", "src/storage/db.rs"];
        assert_eq!(accepted, all);
    }

    #[test]
    fn a_change_never_prepared_fails_the_scope_check_without_asking_the_blueprint() {
        let never = record(&[], None);
        let mut fake = Fake::touching(&["src/storage/db.rs"]);
        let checked = check_scope(&mut fake, &never);
        let failure = "Change c1 has not been prepared: the scope check has no starting point.";
        assert_eq!(
            checked,
            Scope {
                ok: false,
                failures: vec![failure.to_string()],
                accepted: vec![],
            }
        );
        assert_eq!(fake.asked, NOTHING);
    }

    #[test]
    fn as_json_the_verdicts_start_with_ok() {
        let verdict = serde_json::to_string(&Verdict::new(vec!["a".into()])).unwrap();
        assert_eq!(verdict, r#"{"ok":false,"failures":["a"]}"#);
        let checked = serde_json::to_string(&Scope::new(vec![], vec!["b".into()])).unwrap();
        assert_eq!(checked, r#"{"ok":true,"failures":[],"accepted":["b"]}"#);
    }

    #[test]
    fn errors_say_what_went_wrong() {
        let said = "cannot run git: not found";
        assert_eq!(Error::Blueprint(said.into()).to_string(), said);
        let from_config = Error::from(config::Error::UnknownColor("blue".into()));
        assert_eq!(
            from_config.to_string(),
            "unknown color \"blue\" (use green, yellow or red)"
        );
        let read = Error::Read("a.rs".into(), io::Error::other("busy"));
        assert_eq!(read.to_string(), "cannot read a.rs: busy");
        let bad = config::Error::Toml("bad".into());
        let rules = Error::Rules("gdd/rules.toml".into(), "37cc864".into(), bad);
        assert_eq!(
            rules.to_string(),
            "gdd/rules.toml at 37cc864: invalid TOML: bad"
        );
    }
}
