//! Several changes at once, merged one at a time. Each change runs in its own worktree, so
//! several can run at once, and two rules keep them from stepping on each other. A change does
//! not launch while a change in flight plans a path that overlaps one of its own, nor while
//! `[parallel] max` changes build already: the agents' subscriptions are shared (see
//! [`may_launch`]). Launches are judged one at a time, under the launch lock (see
//! [`admission`]). And merges go one at a time, under the merge lock (see [`lock`]): a change
//! whose main branch moved since it was built takes the latest main first (see [`main_moved`]
//! and [`take_main`]), and its build starts again there (see [`restarted`]). A person's write to
//! the GDD state every change shares waits for the agent calls under way, and no call starts
//! while it waits (see [`calling`] and [`shared_write`]).

use std::fs::{self, File, TryLockError};
use std::io;
use std::num::NonZeroUsize;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use crate::agents::files;
use crate::change;
use crate::config;
use crate::git::{self, Git};
use crate::guard::RULES;

/// The states of a change in flight: building, parked, or waiting at G4.
const IN_FLIGHT: [&str; 5] = ["prepared", "building", "polishing", "parked", "in-pre"];

/// The states of a change in flight that count toward the cap: building.
const BUILDING: [&str; 3] = ["prepared", "building", "polishing"];

/// What a segment of a planned path holds that makes the path a pattern from there on.
const WILDCARDS: [char; 3] = ['*', '?', '['];

/// The changes that build at once at most when the rules file says nothing.
const DEFAULT_MAX: usize = 2;

/// What lets a change launch, after its id, when it overlaps changes in flight.
const AFTER_OVERLAPS: &str = "can launch once the changes it overlaps close or are rejected.";

/// What lets a change launch, after its id, when the cap is reached: the rules file follows.
const AFTER_CAP: &str = "can launch once one of those building reaches G4 or is parked, or with \
                         a higher [parallel] max in";

/// Where the records of the changes live, from the state root.
const CHANGES: &str = ".gdd/changes";

/// The merge lock, from the state root: a file of its own, made when it is missing.
pub const LOCK: &str = ".gdd/merge.lock";

/// The launch lock, from the state root: a file of its own, made when it is missing.
pub const ADMISSION: &str = ".gdd/launch.lock";

/// The lock of the agent calls, from the state root: every call holds it shared, and a person's
/// write to the GDD state every change shares holds it alone. A file of its own, made when it is
/// missing.
pub const AGENTS: &str = ".gdd/agents.lock";

/// The turn to take the lock of the agent calls, from the state root: a person who waits for the
/// calls under way holds it, so no other call starts meanwhile. A file of its own, made when it
/// is missing.
pub const TURN: &str = ".gdd/state.lock";

/// The locks, which gdd runs of other changes, and people, make and take at any time.
pub const LOCKS: [&str; 4] = [LOCK, ADMISSION, AGENTS, TURN];

/// What a merge that waits for the merge of another change says.
pub const WAITING: &str = "waiting for the merge of another change";

/// What a person's write to the GDD state every change shares says when it waits for the agent
/// calls under way.
pub const WAITING_AGENTS: &str = "waiting for the agents at work to end their call";

/// How many changes build at once at most, from the text of the rules file `rules`: `[parallel]
/// max`, an integer from 1 on; 2 when it says nothing. The rest of the file is ignored.
pub fn cap(rules: &str) -> Result<usize, config::Error> {
    #[derive(Default, Deserialize)]
    #[serde(default)]
    struct Raw {
        parallel: Parallel,
    }
    #[derive(Default, Deserialize)]
    #[serde(default)]
    struct Parallel {
        max: Option<NonZeroUsize>,
    }
    let raw: Raw = toml::from_str(rules).map_err(|e| config::Error::Toml(e.to_string()))?;
    Ok(raw.parallel.max.map_or(DEFAULT_MAX, NonZeroUsize::get))
}

/// `Ok` when change `id`, whose record is `record`, may launch in the project whose state root
/// is `state`, where `max` changes build at once at most: no other change in flight plans a path
/// that overlaps one of its own (see [`overlaps`]), and fewer than `max` others build. Else why
/// not: a line for each change in flight it overlaps, then one for the cap; then what lets it
/// launch. A record that cannot be read is an error that names it, never a change out of flight.
pub fn may_launch(state: &Path, id: &str, record: &str, max: usize) -> Result<(), String> {
    let own = Flight::read(id, record.as_bytes())?;
    let others = in_flight(state, id)?;
    let overlapping = others.iter().filter_map(|other| overlap(&own, other));
    let mut refused: Vec<String> = overlapping.collect();
    let mut next = Vec::new();
    if !refused.is_empty() {
        next.push(format!("{id} {AFTER_OVERLAPS}"));
    }
    let building: Vec<&str> = others
        .iter()
        .filter(|other| BUILDING.contains(&other.state.as_str()))
        .map(|other| other.id.as_str())
        .collect();
    if building.len() >= max {
        let (count, ids) = (building.len(), building.join(", "));
        let cap = format!("{count} change(s) building, the cap is {max}: {ids}");
        refused.push(cap);
        next.push(format!("{id} {AFTER_CAP} {RULES}."));
    }
    if refused.is_empty() {
        return Ok(());
    }
    // gdd prints the first line as an error: so is each of the others. What to do comes last.
    let (refused, next) = (refused.join("\nerror: "), next.join("\n"));
    Err(format!("{refused}\n{next}"))
}

/// A change as the launch of another one reads it from its record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct Flight {
    /// Its id.
    #[serde(skip)]
    id: String,
    /// The state its record is in.
    #[serde(default)]
    state: String,
    /// The paths it plans.
    #[serde(default)]
    paths: Vec<String>,
}

impl Flight {
    /// Change `id`, whose record holds `bytes`.
    fn read(id: &str, bytes: &[u8]) -> Result<Flight, String> {
        let path = change::record_path(id);
        let invalid = |e: serde_json::Error| format!("{path}: {e}");
        let mut flight: Flight = serde_json::from_slice(bytes).map_err(invalid)?;
        flight.id = id.to_string();
        Ok(flight)
    }
}

/// The changes in flight of the project whose state root is `state` but change `id`, by id: the
/// ones whose record is in one of [`IN_FLIGHT`]. A folder without a record holds no change.
fn in_flight(state: &Path, id: &str) -> Result<Vec<Flight>, String> {
    let mut flights = Vec::new();
    for other in others(state, id)? {
        let path = change::record_path(&other);
        let unread = |e: io::Error| format!("cannot read {path}: {e}");
        let Some(bytes) = files::read(state, &path).map_err(unread)? else {
            continue;
        };
        let flight = Flight::read(&other, &bytes)?;
        if IN_FLIGHT.contains(&flight.state.as_str()) {
            flights.push(flight);
        }
    }
    Ok(flights)
}

/// The names in the folder of the changes of the project whose state root is `state` that can
/// name a change (see [`change::valid_id`]), but `id`, sorted.
fn others(state: &Path, id: &str) -> Result<Vec<String>, String> {
    let unread = |e: io::Error| format!("cannot read {CHANGES}: {e}");
    let mut ids = Vec::new();
    for entry in fs::read_dir(state.join(CHANGES)).map_err(unread)? {
        let name = entry.map_err(unread)?.file_name();
        let name = name.to_string_lossy();
        if name != id && change::valid_id(&name) {
            ids.push(name.into_owned());
        }
    }
    ids.sort();
    Ok(ids)
}

/// Why change `own` cannot launch next to `other`, when a path each plans overlaps: every such
/// pair, its own path first.
fn overlap(own: &Flight, other: &Flight) -> Option<String> {
    let mut pairs = Vec::new();
    for path in &own.paths {
        for theirs in other.paths.iter().filter(|theirs| overlaps(path, theirs)) {
            pairs.push(format!("{path} with {theirs}"));
        }
    }
    if pairs.is_empty() {
        return None;
    }
    let (id, other_id, state) = (&own.id, &other.id, &other.state);
    let pairs = pairs.join(", ");
    Some(format!("{id} overlaps {other_id} ({state}): {pairs}"))
}

/// Whether the planned paths `a` and `b` overlap: compared segment by segment, split on `/`, one
/// is a prefix of the other up to the first segment that holds a wildcard (`*`, `**`, `?`, `[`).
/// A path that ends with `/` stands for its whole folder. Two plain paths overlap only when they
/// are the same.
pub fn overlaps(a: &str, b: &str) -> bool {
    let (a, b) = (Planned::of(a), Planned::of(b));
    if a.plain && b.plain {
        return a.fixed == b.fixed;
    }
    a.fixed.iter().zip(&b.fixed).all(|(x, y)| x == y)
}

/// A planned path, as [`overlaps`] compares it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Planned<'a> {
    /// Its segments before the first one that holds a wildcard: all of them in a plain path.
    fixed: Vec<&'a str>,
    /// Whether it names one path only: no wildcard, and no `/` at its end.
    plain: bool,
}

impl<'a> Planned<'a> {
    fn of(path: &'a str) -> Planned<'a> {
        let inside = path.trim_end_matches('/');
        let segments: Vec<&str> = inside.split('/').collect();
        let wild = segments.iter().position(|part| part.contains(WILDCARDS));
        Planned {
            fixed: segments[..wild.unwrap_or(segments.len())].to_vec(),
            plain: wild.is_none() && inside.len() == path.len(),
        }
    }
}

/// A lock, held: whoever wants it waits until it is dropped.
#[derive(Debug)]
pub struct Lock {
    _file: File,
}

/// Takes the merge lock of the project whose state root is `state` (see [`LOCK`]). While the
/// merge of another change holds it, calls `waiting` once, then waits until it is free.
pub fn lock(state: &Path, waiting: impl FnOnce()) -> Result<Lock, String> {
    held(state, LOCK, waiting)
}

/// Takes the launch lock of the project whose state root is `state` (see [`ADMISSION`]), which
/// the launch of a change holds from the moment it is judged (see [`may_launch`]) until it is
/// building: so no other launch is judged next to a change that is about to build. While
/// another launch holds it, waits until it is free.
pub fn admission(state: &Path) -> Result<Lock, String> {
    held(state, ADMISSION, || {})
}

/// An agent call under way, which holds the lock of the agent calls shared (see [`calling`]):
/// until it is dropped, no person writes the GDD state every change shares.
#[derive(Debug)]
pub struct Calling {
    _agents: File,
}

/// A person's write to the GDD state every change shares, which holds the lock of the agent
/// calls and its turn (see [`shared_write`]): until it is dropped, no agent call starts.
#[derive(Debug)]
pub struct SharedWrite {
    _agents: Lock,
    _turn: Lock,
}

/// Takes the lock of the agent calls of the project whose state root is `state` shared, for an
/// agent call (see [`AGENTS`]): the calls of several changes hold it at once. Its turn comes
/// first (see [`TURN`]), and goes once the lock is held: a call never starts while a person waits
/// to write.
pub fn calling(state: &Path) -> Result<Calling, String> {
    let turn = held(state, TURN, || {})?;
    let agents = opened(state, AGENTS)?;
    agents.lock_shared().map_err(|e| unlocked(AGENTS, e))?;
    drop(turn);
    Ok(Calling { _agents: agents })
}

/// Takes the lock of the agent calls of the project whose state root is `state` alone, for a
/// person's write to the GDD state every change shares, once its turn is taken (see [`TURN`]).
/// While agent calls are under way, calls `say` once, then waits until they end; meanwhile no
/// other call starts.
pub fn shared_write(state: &Path, say: impl FnOnce()) -> Result<SharedWrite, String> {
    let turn = held(state, TURN, || {})?;
    let agents = held(state, AGENTS, say)?;
    Ok(SharedWrite {
        _agents: agents,
        _turn: turn,
    })
}

/// Takes the lock `path` of the project whose state root is `state` alone (see [`opened`]).
/// While someone else holds it, calls `waiting` once, then waits until it is free.
fn held(state: &Path, path: &str, waiting: impl FnOnce()) -> Result<Lock, String> {
    let file = opened(state, path)?;
    match file.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => {
            waiting();
            file.lock().map_err(|e| unlocked(path, e))?;
        }
        Err(TryLockError::Error(e)) => return Err(unlocked(path, e)),
    }
    Ok(Lock { _file: file })
}

/// The lock `path` of the project whose state root is `state`, open, never through a link or
/// another name of the file (see [`files::open`]).
fn opened(state: &Path, path: &str) -> Result<File, String> {
    files::open(state, path).map_err(|e| format!("cannot open {path}: {e}"))
}

/// Why the lock `path` could not be taken.
fn unlocked(path: &str, e: io::Error) -> String {
    format!("cannot lock {path}: {e}")
}

/// Whether the main branch `main` moved since the change checked out at `work` was last built:
/// its commits since they parted changed something anywhere in the repository, also outside the
/// project folder, as the merge brings it all. The merge commit of the change itself, with
/// nothing else, is no move.
pub fn main_moved(work: &Path, main: &str) -> Result<bool, git::Error> {
    Git::new(work).changed_anywhere("HEAD", main)
}

/// Takes the latest main branch `main` into the branch of change `id`, checked out at `work`:
/// its work committed first, then main merged into it with a merge commit, never a rebase. A
/// merge with conflicts is given up, the branch as it was with its work committed: the paths in
/// conflict, which are none when it merged.
pub fn take_main(work: &Path, main: &str, id: &str) -> Result<Vec<String>, git::Error> {
    let git = Git::new(work);
    if git.has_work()? {
        git.commit_work(&format!("gdd({id}): change work"))?;
    }
    let message = format!("gdd({id}): take the latest main");
    let Err(failed) = git.merge(main, &message) else {
        return Ok(Vec::new());
    };
    let conflicts = git.conflicts()?;
    if conflicts.is_empty() {
        return Err(failed);
    }
    git.abort_merge()?;
    Ok(conflicts)
}

/// Starts the build of change `id`, under the state root `state`, again at the commit `start`,
/// where it took the latest main: what came with main is not its doing, and what it did before
/// passed its gates. Its snapshots belong to the earlier start, so they go.
pub fn restarted(state: &Path, id: &str, start: &str) -> Result<(), String> {
    let text = change::read_record(state, id)?;
    let path = change::record_path(id);
    let fields = vec![("build_start", Value::from(start))];
    let record = change::with_fields(&text, fields).map_err(|e| format!("{path}: {e}"))?;
    change::write(state, &path, &record)?;
    for snapshot in [change::snapshot_path(id), change::build_snapshot_path(id)] {
        files::remove(state, &snapshot).map_err(|e| format!("cannot remove {snapshot}: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::git::tests::{Folder, Repo};
    use std::os::unix::fs::symlink;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    /// Whether someone holds the lock `path` of the state root `state` now.
    pub(crate) fn taken(state: &Path, path: &str) -> bool {
        let file = files::open(state, path).unwrap();
        match file.try_lock() {
            Ok(()) => false,
            Err(TryLockError::WouldBlock) => true,
            Err(TryLockError::Error(e)) => panic!("cannot lock {path}: {e}"),
        }
    }

    /// Whether the lock `path` of the state root `state` is free within a second: a process
    /// another test starts keeps a lock let go until it starts its program (see `File::lock`).
    pub(crate) fn freed(state: &Path, path: &str) -> bool {
        (0..100).any(|_| {
            let free = !taken(state, path);
            if !free {
                thread::sleep(Duration::from_millis(10));
            }
            free
        })
    }

    #[test]
    fn the_cap_is_parallel_max_and_two_without_it() {
        assert_eq!(cap(""), Ok(2));
        let folder = "main_branch = \"main\"\n[parallel]\nfolder = \"../t\"\n";
        assert_eq!(cap(folder), Ok(2));
        assert_eq!(cap("[parallel]\nmax = 1\n"), Ok(1));
        assert_eq!(cap("[parallel]\nfolder = \"t\"\nmax = 5\n"), Ok(5));
        for wrong in ["max = 0", "max = -1", "max = \"2\"", "max = 1.5"] {
            let rules = format!("[parallel]\n{wrong}\n");
            let Err(config::Error::Toml(why)) = cap(&rules) else {
                panic!("{wrong} is no cap");
            };
            assert!(why.contains("max"), "{why}");
        }
    }

    #[test]
    fn a_folder_pattern_overlaps_what_is_inside_it() {
        assert!(overlaps("src/lib/**", "src/lib/parse.txt"));
        assert!(overlaps("src/lib/parse.txt", "src/lib/**"));
        assert!(overlaps("src/lib/**", "src/lib/a/b"));
        assert!(!overlaps("src/lib/**", "src/library.txt"));
        assert!(!overlaps("src/lib/**", "docs/lib/a"));
        assert!(overlaps("src/*.txt", "src/app.txt"));
        assert!(overlaps("src/lib/?.rs", "src/lib/a.rs"));
        assert!(overlaps("src/[ab].rs", "src/c.rs"));
        // Up to the first wildcard of either: what follows it is not compared.
        assert!(overlaps("src/*/x.rs", "src/lib/**"));
        assert!(!overlaps("docs/**", "src/**"));
    }

    #[test]
    fn two_plain_paths_overlap_only_when_they_are_the_same() {
        assert!(overlaps("src/app.txt", "src/app.txt"));
        assert!(!overlaps("src/app.txt", "src/other.txt"));
        assert!(!overlaps("src", "src/app.txt"));
        assert!(!overlaps("src/app.txt", "src"));
        assert!(!overlaps("a/b", "a/c"));
    }

    #[test]
    fn patterns_at_the_root_overlap_everything() {
        for path in ["README.md", "src/app.txt", "src/lib/**", "docs/*.md"] {
            assert!(overlaps("**", path), "{path}");
            assert!(overlaps(path, "**"), "{path}");
            assert!(overlaps("*.md", path), "{path}");
        }
    }

    #[test]
    fn a_path_that_ends_with_a_slash_stands_for_its_folder() {
        assert!(overlaps("src/lib/", "src/lib/parse.txt"));
        assert!(overlaps("src/lib/a/b", "src/lib/"));
        assert!(overlaps("src/lib/", "src/lib"));
        assert!(overlaps("src/lib/", "src/lib/"));
        assert!(!overlaps("src/lib/", "src/library.txt"));
        let folder = Planned {
            fixed: vec!["src", "lib"],
            plain: false,
        };
        assert_eq!(Planned::of("src/lib/"), folder);
        let file = Planned {
            plain: true,
            ..folder.clone()
        };
        assert_eq!(Planned::of("src/lib"), file);
    }

    /// A state root with the records `records`: each change id with its state and the paths it
    /// plans.
    fn state_with(records: &[(&str, &str, &[&str])]) -> Folder {
        let state = Folder::new();
        for (id, at, paths) in records {
            let record = serde_json::json!({"id": id, "state": at, "paths": paths});
            state.write(&change::record_path(id), &record.to_string());
        }
        state
    }

    /// The record of a change that plans `paths`.
    fn planning(paths: &[&str]) -> String {
        serde_json::json!({"state": "open", "paths": paths}).to_string()
    }

    /// What lets change `c9` launch when it overlaps changes in flight.
    const C9_AFTER_OVERLAPS: &str =
        "c9 can launch once the changes it overlaps close or are rejected.";

    /// What lets change `id` launch when the cap is reached.
    fn after_cap(id: &str) -> String {
        format!(
            "{id} can launch once one of those building reaches G4 or is parked, or with a \
             higher [parallel] max in gdd/rules.toml."
        )
    }

    #[test]
    fn a_change_that_overlaps_one_in_flight_does_not_launch() {
        let state = state_with(&[
            ("c1", "parked", &["src/app.txt", "src/lib/**"]),
            ("c2", "in-pre", &["docs/a.md"]),
            ("c3", "building", &["src/other.txt"]),
        ]);
        let mine = planning(&["src/lib/parse.txt", "src/app.txt", "docs/a.md"]);
        let err = may_launch(&state.0, "c9", &mine, 9).unwrap_err();
        let c1 = "c9 overlaps c1 (parked): src/lib/parse.txt with src/lib/**, \
                  src/app.txt with src/app.txt";
        let c2 = "c9 overlaps c2 (in-pre): docs/a.md with docs/a.md";
        assert_eq!(err, format!("{c1}\nerror: {c2}\n{C9_AFTER_OVERLAPS}"));
        let elsewhere = planning(&["src/x.txt"]);
        assert_eq!(may_launch(&state.0, "c9", &elsewhere, 9), Ok(()));
        // A change never overlaps itself: relaunching a parked one is fine.
        let own = planning(&["src/app.txt"]);
        assert_eq!(may_launch(&state.0, "c1", &own, 9), Ok(()));
    }

    #[test]
    fn only_changes_in_flight_count() {
        let mine = planning(&["src/app.txt"]);
        let out_of_flight = [
            "open",
            "merged",
            "promoting",
            "in-production",
            "closed",
            "rejected",
            "rolled-back",
        ];
        for at in out_of_flight {
            let state = state_with(&[("c1", at, &["src/app.txt"])]);
            assert_eq!(may_launch(&state.0, "c9", &mine, 1), Ok(()), "{at}");
        }
        for at in IN_FLIGHT {
            let state = state_with(&[("c1", at, &["src/app.txt"])]);
            let err = may_launch(&state.0, "c9", &mine, 9).unwrap_err();
            let start = format!("c9 overlaps c1 ({at}): ");
            assert!(err.starts_with(&start), "{err}");
        }
    }

    #[test]
    fn no_more_changes_build_at_once_than_the_cap() {
        let state = state_with(&[
            ("c1", "building", &[]),
            ("c2", "prepared", &[]),
            ("c3", "polishing", &[]),
            ("c4", "parked", &[]),
            ("c5", "in-pre", &[]),
        ]);
        let mine = planning(&["src/new.txt"]);
        let err = may_launch(&state.0, "c9", &mine, 3).unwrap_err();
        let cap = "3 change(s) building, the cap is 3: c1, c2, c3";
        assert_eq!(err, format!("{cap}\n{}", after_cap("c9")));
        assert_eq!(may_launch(&state.0, "c9", &mine, 4), Ok(()));
        // The change that launches is not one of the others.
        let err = may_launch(&state.0, "c1", &mine, 2).unwrap_err();
        let cap = "2 change(s) building, the cap is 2: c2, c3";
        assert_eq!(err, format!("{cap}\n{}", after_cap("c1")));
        assert_eq!(may_launch(&state.0, "c1", &mine, 3), Ok(()));
        let overlapped = state_with(&[("c1", "building", &["src/new.txt"])]);
        let err = may_launch(&overlapped.0, "c9", &mine, 1).unwrap_err();
        let overlap = "c9 overlaps c1 (building): src/new.txt with src/new.txt";
        let cap = "1 change(s) building, the cap is 1: c1";
        let next = format!("{C9_AFTER_OVERLAPS}\n{}", after_cap("c9"));
        assert_eq!(err, format!("{overlap}\nerror: {cap}\n{next}"));
    }

    #[test]
    fn what_holds_no_change_is_left_out_and_a_record_that_cannot_be_read_is_an_error() {
        let state = state_with(&[("c1", "building", &["src/app.txt"])]);
        let mine = planning(&["src/app.txt"]);
        // A folder without a record, and names no change can have.
        fs::create_dir_all(state.0.join(".gdd/changes/c2")).unwrap();
        state.write(".gdd/changes/.DS_Store", "");
        state.write(".gdd/changes/Notes/change.json", "{");
        assert!(may_launch(&state.0, "c9", &mine, 9).is_err());
        assert_eq!(may_launch(&state.0, "c1", &mine, 9), Ok(()));
        state.write(".gdd/changes/c3/change.json", "{");
        let err = may_launch(&state.0, "c1", &mine, 9).unwrap_err();
        assert!(err.starts_with(".gdd/changes/c3/change.json: "), "{err}");
        fs::remove_dir_all(state.0.join(".gdd/changes/c3")).unwrap();
        // A file where the folder of a change goes: no folder, not a missing record.
        state.write(".gdd/changes/c4", "");
        let err = may_launch(&state.0, "c1", &mine, 9).unwrap_err();
        assert_eq!(
            err,
            "cannot read .gdd/changes/c4/change.json: .gdd/changes/c4 is not a folder"
        );
        fs::remove_file(state.0.join(".gdd/changes/c4")).unwrap();
        let outside = state_with(&[("c5", "building", &["src/app.txt"])]);
        let link = state.0.join(".gdd/changes/c5");
        symlink(outside.0.join(".gdd/changes/c5"), &link).unwrap();
        let err = may_launch(&state.0, "c1", &mine, 9).unwrap_err();
        assert!(err.ends_with(".gdd/changes/c5 is not a folder"), "{err}");
        fs::remove_file(&link).unwrap();
        fs::create_dir_all(state.0.join(".gdd/changes/c6/change.json")).unwrap();
        let err = may_launch(&state.0, "c1", &mine, 9).unwrap_err();
        let unread = "cannot read .gdd/changes/c6/change.json: ";
        assert!(err.starts_with(unread), "{err}");
        fs::remove_dir_all(state.0.join(".gdd/changes/c6")).unwrap();
        let err = may_launch(&state.0, "c1", "3", 9).unwrap_err();
        assert!(err.starts_with(".gdd/changes/c1/change.json: "), "{err}");
        let nowhere = Folder::new();
        let err = may_launch(&nowhere.0, "c1", &mine, 9).unwrap_err();
        assert!(err.starts_with("cannot read .gdd/changes: "), "{err}");
    }

    #[test]
    fn the_merge_lock_waits_for_the_merge_that_holds_it() {
        let state = Folder::new();
        state.write(".gdd/log.jsonl", "");
        let mut waited = 0;
        let held = lock(&state.0, || waited += 1).unwrap();
        assert_eq!(waited, 0);
        assert!(state.0.join(LOCK).is_file());
        let (taken, wait_taken) = mpsc::channel();
        let (released, wait_released) = mpsc::channel();
        let root = state.0.clone();
        let other = thread::spawn(move || {
            let mut waited = 0;
            taken.send(()).unwrap();
            let mine = lock(&root, || waited += 1).unwrap();
            released.send(()).unwrap();
            drop(mine);
            waited
        });
        wait_taken.recv().unwrap();
        thread::sleep(Duration::from_millis(100));
        assert!(wait_released.try_recv().is_err(), "the lock is held");
        drop(held);
        wait_released.recv().unwrap();
        assert_eq!(other.join().unwrap(), 1);
    }

    #[test]
    fn a_launch_waits_for_the_launch_that_holds_the_launch_lock_and_never_for_a_merge() {
        let state = Folder::new();
        let merging = lock(&state.0, || {}).unwrap();
        let held = admission(&state.0).unwrap();
        assert!(state.0.join(ADMISSION).is_file());
        let (admitted, wait_admitted) = mpsc::channel();
        let root = state.0.clone();
        let other = thread::spawn(move || {
            let mine = admission(&root).unwrap();
            admitted.send(()).unwrap();
            drop(mine);
        });
        thread::sleep(Duration::from_millis(100));
        assert!(wait_admitted.try_recv().is_err(), "it is held");
        drop(held);
        wait_admitted.recv().unwrap();
        other.join().unwrap();
        drop(merging);
        let file = Folder::new();
        file.write(".gdd", "");
        let err = admission(&file.0).unwrap_err();
        assert_eq!(err, "cannot open .gdd/launch.lock: .gdd is not a folder");
    }

    #[test]
    fn a_merge_lock_that_is_not_a_file_of_its_own_is_refused() {
        let (state, outside) = (Folder::new(), Folder::new());
        outside.write("f", "");
        state.write(".gdd/log.jsonl", "");
        symlink(outside.0.join("f"), state.0.join(LOCK)).unwrap();
        let err = lock(&state.0, || {}).unwrap_err();
        let refused = "cannot open .gdd/merge.lock: not a file of its own";
        assert!(err.starts_with(refused), "{err}");
        fs::remove_file(state.0.join(LOCK)).unwrap();
        fs::hard_link(outside.0.join("f"), state.0.join(LOCK)).unwrap();
        let err = lock(&state.0, || {}).unwrap_err();
        assert!(err.starts_with(refused), "{err}");
        // A state root whose `.gdd` is a file: not a folder, never a missing lock.
        let file = Folder::new();
        file.write(".gdd", "");
        let err = lock(&file.0, || {}).unwrap_err();
        assert_eq!(err, "cannot open .gdd/merge.lock: .gdd is not a folder");
        // A state root without `.gdd` yet gets it.
        let fresh = Folder::new();
        assert!(lock(&fresh.0, || {}).is_ok());
        assert!(fresh.0.join(LOCK).is_file());
    }

    #[test]
    fn the_calls_of_several_changes_hold_the_lock_of_the_calls_at_once_and_let_their_turn_go() {
        let state = Folder::new();
        let first = calling(&state.0).unwrap();
        let second = calling(&state.0).unwrap();
        assert!(state.0.join(AGENTS).is_file());
        assert!(taken(&state.0, AGENTS));
        assert!(freed(&state.0, TURN));
        drop(first);
        assert!(taken(&state.0, AGENTS));
        drop(second);
        assert!(freed(&state.0, AGENTS));
    }

    #[test]
    fn a_shared_write_waits_for_the_calls_under_way_and_says_so() {
        let state = Folder::new();
        let mut said = 0;
        let alone = shared_write(&state.0, || said += 1).unwrap();
        assert_eq!(said, 0);
        assert!(taken(&state.0, AGENTS));
        assert!(taken(&state.0, TURN));
        drop(alone);
        let call = calling(&state.0).unwrap();
        let (waiting, wait_waiting) = mpsc::channel();
        let (written, wait_written) = mpsc::channel();
        let root = state.0.clone();
        let person = thread::spawn(move || {
            let write = shared_write(&root, || waiting.send(()).unwrap()).unwrap();
            written.send(()).unwrap();
            drop(write);
        });
        wait_waiting.recv().unwrap();
        thread::sleep(Duration::from_millis(100));
        assert!(wait_written.try_recv().is_err(), "the call is under way");
        drop(call);
        wait_written.recv().unwrap();
        person.join().unwrap();
    }

    #[test]
    fn a_person_waiting_to_write_keeps_the_next_call_from_starting() {
        let state = Folder::new();
        let call = calling(&state.0).unwrap();
        let (said, wait_said) = mpsc::channel();
        let (events, wait_events) = mpsc::channel();
        let (root, person_events) = (state.0.clone(), events.clone());
        let person = thread::spawn(move || {
            let write = shared_write(&root, || said.send(()).unwrap()).unwrap();
            person_events.send("written").unwrap();
            thread::sleep(Duration::from_millis(100));
            drop(write);
        });
        wait_said.recv().unwrap();
        let root = state.0.clone();
        let next = thread::spawn(move || {
            let call = calling(&root).unwrap();
            events.send("next call").unwrap();
            drop(call);
        });
        thread::sleep(Duration::from_millis(200));
        assert!(wait_events.try_recv().is_err(), "the next call waits");
        drop(call);
        let order: Vec<&str> = wait_events.iter().take(2).collect();
        assert_eq!(order, ["written", "next call"]);
        person.join().unwrap();
        next.join().unwrap();
    }

    #[test]
    fn the_locks_of_the_calls_that_are_not_files_of_their_own_are_refused() {
        let file = Folder::new();
        file.write(".gdd", "");
        let not_a_folder = "cannot open .gdd/state.lock: .gdd is not a folder";
        assert_eq!(calling(&file.0).unwrap_err(), not_a_folder);
        assert_eq!(shared_write(&file.0, || {}).unwrap_err(), not_a_folder);
        let (state, outside) = (Folder::new(), Folder::new());
        outside.write("f", "");
        state.write(TURN, "");
        symlink(outside.0.join("f"), state.0.join(AGENTS)).unwrap();
        let refused = "cannot open .gdd/agents.lock: not a file of its own";
        assert!(calling(&state.0).unwrap_err().starts_with(refused));
        let err = shared_write(&state.0, || {}).unwrap_err();
        assert!(err.starts_with(refused), "{err}");
        assert!(freed(&state.0, TURN), "the turn goes with the refusal");
    }

    /// A repository on `main` with the branch of change `c1`, which adds `app.txt`, checked out
    /// in a worktree, and main one commit ahead of where the branch left it, with `news.txt`.
    fn moved() -> (Repo, Folder) {
        let repo = Repo::new();
        repo.identify();
        repo.start("c1");
        repo.write("app.txt", "change\n");
        repo.commit();
        repo.git(&["checkout", "-q", "main"]);
        let trees = Folder::new();
        let work = trees.0.join("c1").to_string_lossy().into_owned();
        repo.git(&["worktree", "add", "-q", &work, "change/c1"]);
        repo.write("news.txt", "news\n");
        repo.commit();
        (repo, trees)
    }

    #[test]
    fn main_moved_when_its_last_commit_is_not_in_the_change() {
        let (repo, trees) = moved();
        let work = trees.0.join("c1");
        assert!(main_moved(&work, "main").unwrap());
        assert!(!main_moved(repo.root(), "main").unwrap());
        assert!(take_main(&work, "main", "c1").unwrap().is_empty());
        assert!(!main_moved(&work, "main").unwrap());
        // The change merged into main, with its own merge commit: main moved nowhere new.
        repo.git(&["merge", "--no-ff", "-q", "-m", "merged", "change/c1"]);
        assert!(!main_moved(&work, "main").unwrap());
        repo.write("later.txt", "later\n");
        repo.commit();
        assert!(main_moved(&work, "main").unwrap());
        assert!(main_moved(&work, "trunk").is_err());
    }

    #[test]
    fn main_moved_when_it_changed_only_what_is_outside_the_project_folder() {
        // The project lives in `app/` of the repository; main changes only `shared/`.
        let repo = Repo::new();
        repo.identify();
        repo.write("app/gdd/project.toml", "name = \"app\"\n");
        repo.write("shared/lib.txt", "old\n");
        repo.commit();
        repo.start("c1");
        repo.write("app/app.txt", "change\n");
        repo.commit();
        repo.git(&["checkout", "-q", "main"]);
        let trees = Folder::new();
        let tree = trees.0.join("c1");
        let path = tree.to_string_lossy().into_owned();
        repo.git(&["worktree", "add", "-q", &path, "change/c1"]);
        repo.write("shared/lib.txt", "new\n");
        repo.commit();
        let work = tree.join("app");
        assert!(main_moved(&work, "main").unwrap());
        assert!(take_main(&work, "main", "c1").unwrap().is_empty());
        let shared = fs::read_to_string(tree.join("shared/lib.txt")).unwrap();
        assert_eq!(shared, "new\n");
        assert!(!main_moved(&work, "main").unwrap());
    }

    #[test]
    fn the_change_takes_the_latest_main_with_a_merge_commit_after_its_work() {
        let (repo, trees) = moved();
        let work = trees.0.join("c1");
        fs::write(work.join("work.txt"), "uncommitted\n").unwrap();
        let before = Git::new(&work).head().unwrap();
        assert!(take_main(&work, "main", "c1").unwrap().is_empty());
        let log = repo.git(&["log", "--format=%s", "-2", "change/c1"]);
        assert_eq!(log, "gdd(c1): take the latest main\ngdd(c1): change work");
        let parents = repo.git(&["log", "-1", "--format=%P", "change/c1"]);
        let main = repo.git(&["rev-parse", "main"]);
        assert!(parents.ends_with(&main), "{parents}");
        assert!(work.join("news.txt").is_file());
        repo.git(&["merge-base", "--is-ancestor", &before, "change/c1"]);
        assert!(!Git::new(&work).has_changes().unwrap());
        // Nothing to commit and main where the change has it: nothing happens.
        let head = Git::new(&work).head().unwrap();
        assert!(take_main(&work, "main", "c1").unwrap().is_empty());
        assert_eq!(Git::new(&work).head().unwrap(), head);
    }

    #[test]
    fn a_conflict_with_the_latest_main_is_given_up_and_named() {
        let (repo, trees) = moved();
        let work = trees.0.join("c1");
        repo.write("app.txt", "main\n");
        repo.write("other.txt", "main\n");
        repo.commit();
        fs::write(work.join("other.txt"), "change\n").unwrap();
        let conflicts = take_main(&work, "main", "c1").unwrap();
        assert_eq!(conflicts, ["app.txt", "other.txt"]);
        let git = Git::new(&work);
        let app = fs::read_to_string(work.join("app.txt")).unwrap();
        assert_eq!(app, "change\n");
        assert!(!work.join("news.txt").exists());
        assert!(!git.has_changes().unwrap());
        assert!(git.conflicts().unwrap().is_empty());
        let subject = repo.git(&["log", "-1", "--format=%s", "change/c1"]);
        assert_eq!(subject, "gdd(c1): change work");
        // A merge that fails for another reason is an error, the branch as it was.
        let err = take_main(&work, "trunk", "c1").unwrap_err().to_string();
        let failed = "git merge --no-ff --no-edit -q -m gdd(c1): take the latest main trunk";
        assert!(err.starts_with(failed), "{err}");
        assert_eq!(git.head().unwrap(), repo.git(&["rev-parse", "change/c1"]));
        let plain = Folder::new();
        assert!(take_main(&plain.0, "main", "c1").is_err());
    }

    #[test]
    fn a_build_restarted_starts_where_the_change_took_main_without_its_snapshots() {
        let record = r#"{"build_start": "abc", "state": "building", "paths": ["a"]}"#;
        let state = Folder::new();
        state.write(&change::record_path("c1"), record);
        state.write(&change::snapshot_path("c1"), "{}");
        state.write(&change::build_snapshot_path("c1"), "{}");
        restarted(&state.0, "c1", "def").unwrap();
        let text = change::read_record(&state.0, "c1").unwrap();
        let record: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(record["build_start"], "def");
        assert_eq!(record["state"], "building");
        assert_eq!(record["paths"][0], "a");
        assert!(!state.0.join(change::snapshot_path("c1")).exists());
        assert!(!state.0.join(change::build_snapshot_path("c1")).exists());
        // Again, without snapshots.
        assert_eq!(restarted(&state.0, "c1", "def"), Ok(()));
        let err = restarted(&state.0, "c9", "def").unwrap_err();
        assert!(err.starts_with("change c9 does not exist"), "{err}");
        state.write(&change::record_path("c2"), "[]");
        let err = restarted(&state.0, "c2", "def").unwrap_err();
        assert!(err.starts_with(".gdd/changes/c2/change.json: "), "{err}");
        // What stands where a snapshot goes goes, never what a link points to.
        state.write(&change::record_path("c3"), "{}");
        state.write(".gdd/changes/c3/snapshot.json/x", "");
        let outside = Folder::new();
        let link = state.0.join(".gdd/changes/c3/snapshot-build.json");
        symlink(&outside.0, &link).unwrap();
        assert_eq!(restarted(&state.0, "c3", "def"), Ok(()));
        assert!(!state.0.join(".gdd/changes/c3/snapshot.json").exists());
        assert!(fs::symlink_metadata(&link).is_err());
        assert!(outside.0.is_dir());
    }
}
