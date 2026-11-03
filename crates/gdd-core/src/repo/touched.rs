//! The blueprint of software: where a change works (its worktree, on its branch), the marks of
//! its work (commits), what it touched (the paths of the project its branch and its working tree
//! changed, as git lists them), the bytes of its files, the rules file as a commit had it, what
//! it keeps (commits on its branch), and how it stands against the main branch: taking it (a
//! merge of the main branch into its branch, see [`super::integrate`]) and joining it (its merge
//! into the main branch, see [`merge::merge`]). This is the blueprint the core asks (see
//! [`Blueprint`]); it reports items and bytes, never a verdict, a color or a hash: judging them
//! is the core's.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::ErrorKind::{NotADirectory, NotFound};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

use super::integrate;
use super::merge;
use crate::agents::render::first_link;
use crate::change;
use crate::classify::Classification;
use crate::git::{self, Git};
use crate::guard::{self, Blueprint, Error, ItemId, Mark, Moved, Place, Settings, Touched};
use crate::layout;
use crate::recipe::{self, worktree};

/// Where a planted failure asks the blueprint for a secret it makes when it plants it.
pub const SECRET: &str = "{secret}";

/// What a made secret starts with: the shape of a GitHub token, which secret scanners know.
const SECRET_SHAPE: &str = "ghp_";

/// How many letters and digits follow [`SECRET_SHAPE`] in a made secret.
const SECRET_LENGTH: usize = 36;

/// The blueprint of software: the project at a root, in its git repository, whose changes leave
/// from a main branch and merge into it in the main checkout, the state root. An item is a path
/// from the project root, a mark a commit id. With a folder of places (see
/// [`Software::with_places`]), each change works in its worktree there; without one, every
/// change works at the root, which it cannot leave or let go. For the self-test (see
/// [`Software::for_close_tests`]), each close test works in a place of its own there instead,
/// the only places it plants a failure in.
#[derive(Debug)]
pub struct Software<'a> {
    /// The project root, in the main checkout when there is a folder of places.
    root: &'a Path,
    /// The state root, in the main checkout, where the changes merge (see
    /// [`crate::state::root`]).
    state: &'a Path,
    /// The branch the changes leave from.
    main: &'a str,
    /// The folder of the worktrees of the changes (see [`worktree::folder`]), if they have one.
    places: Option<&'a Path>,
    /// The project root in the worktree of each change asked about so far.
    works: BTreeMap<String, PathBuf>,
    /// Whether each place is made anew for one close test alone, from the main branch as it is
    /// and on no branch, the only places it plants in (see [`Software::for_close_tests`]).
    close_tests: bool,
}

impl<'a> Software<'a> {
    /// The project at `root`, its own state root, whose changes leave from the branch `main`,
    /// where every change works: what the checks that only read ask.
    pub fn new(root: &'a Path, main: &'a str) -> Software<'a> {
        Software::joining(root, root, main)
    }

    /// The project at `checkout`, the main checkout, whose changes leave from the branch `main`
    /// and work each in its worktree in the folder `folder` (see [`worktree::path`]): what the
    /// recipe asks.
    pub fn with_places(checkout: &'a Path, folder: &'a Path, main: &'a str) -> Software<'a> {
        Software {
            places: Some(folder),
            ..Software::new(checkout, main)
        }
    }

    /// The project at `checkout`, the main checkout, whose places for close tests are made in
    /// the folder `folder` (see [`places`]), each anew from the main branch `settings` name as it
    /// is, on no branch (see [`worktree::fresh`]): what the self-test asks. Only in these does it
    /// plant (see [`Blueprint::plant`]).
    pub fn for_close_tests(
        checkout: &'a Path,
        folder: &'a Path,
        settings: &'a Settings,
    ) -> Software<'a> {
        Software {
            close_tests: true,
            ..Software::with_places(checkout, folder, &settings.main_branch)
        }
    }

    /// The project at `root`, where every change works, whose state root is `state` and whose
    /// changes merge into the branch `main`: what `tminus merge` asks, run where the last work of
    /// the change is.
    pub fn joining(root: &'a Path, state: &'a Path, main: &'a str) -> Software<'a> {
        Software {
            root,
            state,
            main,
            places: None,
            works: BTreeMap::new(),
            close_tests: false,
        }
    }

    /// The folder of places, or why change `id` cannot have a place without one.
    fn folder(&self, id: &str) -> Result<&'a Path, Error> {
        let none = || Error::Blueprint(format!("{id}: no folder of places"));
        self.places.ok_or_else(none)
    }

    /// Where the worktree of change `id` goes (see [`worktree::path`]).
    fn worktree(&self, id: &str) -> Result<PathBuf, Error> {
        worktree::path(self.folder(id)?, id).map_err(Error::Blueprint)
    }

    /// The project root where change `id` works: the root without a folder of places, else the
    /// same folder in its worktree (see [`worktree::project_root`]), asked of git once.
    fn work(&mut self, id: &str) -> Result<PathBuf, Error> {
        if self.places.is_none() {
            return Ok(self.root.to_path_buf());
        }
        if let Some(work) = self.works.get(id) {
            return Ok(work.clone());
        }
        let worktree = self.worktree(id)?;
        let work = worktree::project_root(self.root, &worktree).map_err(Error::Blueprint)?;
        self.works.insert(id.to_string(), work.clone());
        Ok(work)
    }
}

impl Blueprint for Software<'_> {
    /// The worktree of change `id`, on its branch, from the main checkout (see
    /// [`worktree::open`]): made when there was nothing at its path, else the one there. For
    /// close tests, the place of close test `id`, made anew from the main branch as it is, on no
    /// branch (see [`worktree::fresh`]).
    fn isolate(&mut self, id: &str) -> Result<Place, Error> {
        let worktree = self.worktree(id)?;
        if self.close_tests {
            worktree::fresh(self.root, &worktree, self.main).map_err(Error::Blueprint)?;
            let work = self.work(id)?;
            return Ok(Place { work, made: true });
        }
        let made = fs::symlink_metadata(&worktree).is_err();
        worktree::open(self.root, &worktree, id).map_err(Error::Blueprint)?;
        let work = self.work(id)?;
        Ok(Place { work, made })
    }

    /// Where the worktree of change `id` is or goes, its project root there (see
    /// [`Software::work`]), whether there is one or not: nothing is made, and nothing is asked of
    /// the worktree. Without a folder of places, the root.
    fn place(&mut self, id: &str) -> Result<Place, Error> {
        let work = self.work(id)?;
        Ok(Place { work, made: false })
    }

    /// Removes the worktree of `id`, with what it has that git does not, from the main checkout
    /// (see [`worktree::remove`]); its branch stays.
    fn vacate(&mut self, id: &str) -> Result<(), Error> {
        let worktree = self.worktree(id)?;
        worktree::remove(self.root, &worktree).map_err(Error::Blueprint)
    }

    /// The commit checked out where `id` works.
    fn mark(&mut self, id: &str) -> Result<Mark, Error> {
        let work = self.work(id)?;
        Ok(Mark::new(Git::new(&work).head()?))
    }

    /// What change `id` touched since the commit `since`, where it works: what its branch
    /// changed since then and, while it is checked out, what the working tree changed, added or
    /// removed (see [`Git::changed_since`]). Without `since`, against the main branch (see
    /// [`against_main`]). In the worktree of a variant of a spike, what the variant's branch
    /// touched (see [`change::branch_at`]).
    fn touched(&mut self, id: &str, since: Option<&Mark>) -> Result<Touched, Error> {
        let work = self.work(id)?;
        let Some(since) = since else {
            return Ok(against_main(&work, id, self.main)?);
        };
        let git = Git::new(&work);
        let branch = change::branch_at(&git, id)?;
        let paths = git.changed_since(&branch, since.as_str())?;
        Ok(paths.into_iter().collect())
    }

    /// The bytes of the file `item` where `id` works, following a link, as
    /// [`crate::guard::file_hash`] reads it: `None` for nothing, a folder, or a path under a
    /// file; what cannot be read is an error that names it.
    fn read(&mut self, id: &str, item: &ItemId) -> Result<Option<Vec<u8>>, Error> {
        let path = self.work(id)?.join(item.as_str());
        let unread = |e| Error::Read(item.to_string(), e);
        let is_file = match fs::metadata(&path) {
            Ok(meta) => meta.is_file(),
            Err(e) if matches!(e.kind(), NotFound | NotADirectory) => false,
            Err(e) => return Err(unread(e)),
        };
        if !is_file {
            return Ok(None);
        }
        fs::read(&path).map(Some).map_err(unread)
    }

    /// The file `item` as the commit `mark` had it; `None` where it had none (see
    /// [`Git::file_at`]).
    fn rules_at(&mut self, mark: &Mark, item: &ItemId) -> Result<Option<Vec<u8>>, Error> {
        let git = Git::new(self.root);
        Ok(git.file_at(mark.as_str(), item.as_str())?)
    }

    /// Commits, on the branch checked out where `id` works, `items` and nothing else (see
    /// [`Git::commit_files`]), or all of its work but the state with no `items` (see
    /// [`Git::commit_work`]), as `tminus(<id>): <why>` (see [`recipe::commit_message`]): nothing
    /// to commit, no commit. Who keeps it is not written anywhere: the author of the commit is
    /// the one git has, as for any commit of the tool.
    fn keep(
        &mut self,
        id: &str,
        _by: &str,
        why: &str,
        items: Option<&[ItemId]>,
    ) -> Result<(), Error> {
        let work = self.work(id)?;
        let git = Git::new(&work);
        let message = recipe::commit_message(&work, id, why);
        match items {
            None if git.has_work()? => git.commit_work(&message)?,
            None | Some([]) => {}
            Some(items) => {
                let paths: Vec<String> = items.iter().map(ToString::to_string).collect();
                git.commit_files(&paths, &message)?;
            }
        }
        Ok(())
    }

    /// Leaves the main checkout on the main branch, with what an earlier run of change `id` left
    /// there on its branch kept (see [`integrate::settle`]).
    fn settle(&mut self, id: &str) -> Result<(), Error> {
        integrate::settle(self.state, self.main, id)
    }

    /// With a folder of places, why the main checkout cannot take a merge (see
    /// [`merge::not_ready`]); without one, why change `id` may not merge from where it works
    /// (see [`merge::may_merge`]).
    fn blocked(&mut self, id: &str) -> Result<Option<String>, Error> {
        if self.places.is_some() {
            return Ok(merge::not_ready(self.state, self.main)?);
        }
        let work = self.work(id)?;
        let project = merge::Project {
            root: &work,
            state: self.state,
            main: self.main,
        };
        Ok(merge::may_merge(&project, id).err())
    }

    /// Whether the main branch moved since change `id` was last built, where it works (see
    /// [`integrate::main_moved`]).
    fn behind(&mut self, id: &str) -> Result<bool, Error> {
        let work = self.work(id)?;
        Ok(integrate::main_moved(&work, self.main)?)
    }

    /// The latest main branch taken into change `id`, where it works (see
    /// [`integrate::refreshed`]).
    fn refresh(&mut self, id: &str) -> Result<Moved, Error> {
        let work = self.work(id)?;
        integrate::refreshed(&work, self.main, id)
    }

    /// Whether change `id` has, where it works, work to commit or a commit the main branch
    /// lacks (see [`integrate::nothing_new`]).
    fn ahead(&mut self, id: &str) -> Result<bool, Error> {
        let work = self.work(id)?;
        Ok(!integrate::nothing_new(&work, id, self.main)?)
    }

    /// Merges change `id`, classified as `how`, from where it works into the main branch in the
    /// main checkout, then deploys it to pre (see [`merge::merge`]): what that printed. What it
    /// refused or failed on is the error, as it said it.
    fn integrate(&mut self, id: &str, how: &Classification) -> Result<String, Error> {
        let work = self.work(id)?;
        let project = merge::Project {
            root: &work,
            state: self.state,
            main: self.main,
        };
        merge::merge(&project, id, how).map_err(Error::Blueprint)
    }

    /// Writes `content` as the file `item` where close test `id` works, in place of the file or
    /// link there, which is never followed, each [`SECRET`] in it replaced by a secret made now
    /// (see [`with_secrets`]): only in a place made for a close test (see
    /// [`Software::for_close_tests`]). A path that leaves the project (`..`, absolute, `.`), a
    /// symbolic link on the way to it, or a folder at it, is refused, and nothing is written.
    fn plant(&mut self, id: &str, item: &ItemId, content: &str) -> Result<(), Error> {
        if !self.close_tests {
            let why = format!("{id}: software plants only in the place of a close test");
            return Err(Error::Blueprint(why));
        }
        let path = Path::new(item.as_str());
        let named = |part: Component| matches!(part, Component::Normal(_));
        if !path.components().all(named) {
            let why = format!("cannot plant {item}: it is not a path inside the project");
            return Err(Error::Blueprint(why));
        }
        let work = self.work(id)?;
        let unplanted = |e: io::Error| Error::Blueprint(format!("cannot plant {item}: {e}"));
        let link = first_link(&work, item.as_str()).map_err(unplanted)?;
        if let Some(link) = link.filter(|link| link != path) {
            let shown = link.display();
            let why = format!("cannot plant {item}: {shown} is a symbolic link");
            return Err(Error::Blueprint(why));
        }
        let planted = with_secrets(content).map_err(unplanted)?;
        let at = work.join(path);
        cleared(&at).map_err(unplanted)?;
        if let Some(folder) = at.parent() {
            fs::create_dir_all(folder).map_err(unplanted)?;
        }
        let mut file = File::create_new(&at).map_err(unplanted)?;
        file.write_all(planted.as_bytes()).map_err(unplanted)
    }
}

/// Removes the file or link at `path`, never following it; nothing there is nothing to do.
fn cleared(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() == NotFound => Ok(()),
        other => other,
    }
}

/// `content` with each [`SECRET`] in it replaced by a secret of its own, made now (see
/// [`made_secret`]); without one, as it is, and no secret is made.
fn with_secrets(content: &str) -> io::Result<String> {
    let mut parts = content.split(SECRET);
    let mut planted = parts.next().unwrap_or_default().to_string();
    for part in parts {
        planted.push_str(&made_secret()?);
        planted.push_str(part);
    }
    Ok(planted)
}

/// A secret made now, from the system's random source: [`SECRET_SHAPE`] and
/// [`SECRET_LENGTH`] letters and digits, one for each byte read, so a secret scanner finds it,
/// and no two alike. It is never printed, logged or kept.
pub fn made_secret() -> io::Result<String> {
    const LETTERS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut random = [0u8; SECRET_LENGTH];
    File::open("/dev/urandom")?.read_exact(&mut random)?;
    let letter = |byte: &u8| char::from(LETTERS[usize::from(*byte) % LETTERS.len()]);
    let letters: String = random.iter().map(letter).collect();
    Ok(format!("{SECRET_SHAPE}{letters}"))
}

/// The folder where the places of the changes, and of the close tests, of the project at `root`
/// go, as its rules file says in the layout of the project (see [`worktree::folder`]): without
/// a rules file, where one that says nothing puts them. A rules file that cannot be read, or is
/// not valid, is an error that names it.
pub fn places(root: &Path) -> Result<PathBuf, String> {
    let rules = layout::named(root, guard::RULES);
    let text = match fs::read_to_string(root.join(&rules)) {
        Ok(text) => text,
        Err(e) if e.kind() == NotFound => String::new(),
        Err(e) => return Err(format!("cannot read {rules}: {e}")),
    };
    worktree::folder(root, &text).map_err(|e| format!("{rules}: {e}"))
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
    use std::collections::BTreeSet;
    use std::fs;
    use std::os::unix::fs::symlink;

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

    /// A repository with an author that ignores `.tminus/`, with change `c1` and its branch, not
    /// checked out; and a folder of places.
    fn with_places() -> (Repo, Folder) {
        let repo = Repo::new();
        repo.identify();
        repo.write(".gitignore", ".tminus/\n");
        repo.commit();
        repo.git(&["branch", "change/c1"]);
        (repo, Folder::new())
    }

    /// The place at `work`, made now or found there.
    fn place(work: &Path, made: bool) -> Place {
        let work = work.to_path_buf();
        Place { work, made }
    }

    #[test]
    fn the_place_of_a_change_is_its_worktree_made_once_and_found_after() {
        let (repo, places) = with_places();
        let mut software = Software::with_places(repo.root(), &places.0, "main");
        let work = places.0.join("c1");
        assert_eq!(software.isolate("c1").unwrap(), place(&work, true));
        assert_eq!(Git::new(&work).current_branch().unwrap(), "change/c1");
        assert_eq!(software.isolate("c1").unwrap(), place(&work, false));
        let mut fresh = Software::with_places(repo.root(), &places.0, "main");
        assert_eq!(fresh.isolate("c1").unwrap(), place(&work, false));
        // Let go, it is gone and its branch stays; nothing there to let go is no error.
        software.vacate("c1").unwrap();
        assert!(!work.exists());
        repo.git(&["rev-parse", "--verify", "change/c1"]);
        software.vacate("c1").unwrap();
        // Anything else in the way is refused, named.
        fs::create_dir_all(work.join("mine")).unwrap();
        let err = software.isolate("c1").unwrap_err().to_string();
        let way = "is in the way of the worktree of change c1";
        let shown = format!("{} {way}", work.display());
        assert!(err.starts_with(&shown), "{err}");
        let err = software.vacate("c1").unwrap_err().to_string();
        assert!(err.starts_with("git worktree remove --force -- "), "{err}");
    }

    #[test]
    fn where_the_place_of_a_change_is_is_said_without_making_it() {
        let (repo, places) = with_places();
        let mut software = Software::with_places(repo.root(), &places.0, "main");
        let work = places.0.join("c1");
        assert_eq!(software.place("c1").unwrap(), place(&work, false));
        assert!(!work.exists(), "a worktree was made");
        assert_eq!(software.isolate("c1").unwrap(), place(&work, true));
        assert_eq!(software.place("c1").unwrap(), place(&work, false));
        // Gone again, it is still where it would be.
        software.vacate("c1").unwrap();
        assert_eq!(software.place("c1").unwrap(), place(&work, false));
        // Without a folder of places, every change is at the root.
        let mut at_root = Software::new(repo.root(), "main");
        assert_eq!(at_root.place("c1").unwrap(), place(repo.root(), false));
    }

    #[test]
    fn a_project_in_a_folder_of_its_repository_works_in_that_folder_of_its_worktree() {
        let repo = Repo::new();
        repo.write("app/README.md", "app\n");
        repo.commit();
        repo.git(&["branch", "change/c1"]);
        let places = Folder::new();
        let root = repo.root().join("app");
        let mut software = Software::with_places(&root, &places.0, "main");
        let work = places.0.join("c1").join("app");
        assert_eq!(software.place("c1").unwrap(), place(&work, false));
        assert_eq!(software.isolate("c1").unwrap(), place(&work, true));
        assert!(work.join("README.md").is_file());
        fs::write(work.join("new.md"), "x\n").unwrap();
        let start = Mark::new(repo.git(&["rev-parse", "HEAD"]));
        let touched = software.touched("c1", Some(&start)).unwrap();
        assert_eq!(touched.paths(), ["new.md"]);
    }

    #[test]
    fn without_a_folder_of_places_a_change_works_at_the_root_and_cannot_leave_it() {
        let (repo, _) = with_places();
        let mut software = Software::new(repo.root(), "main");
        for err in [
            software.isolate("c1").map(drop).unwrap_err(),
            software.vacate("c1").unwrap_err(),
        ] {
            assert_eq!(err.to_string(), "c1: no folder of places");
        }
        let head = repo.git(&["rev-parse", "HEAD"]);
        assert_eq!(software.mark("c1").unwrap(), Mark::new(head));
        repo.write("a.txt", "abc");
        let read = software.read("c1", &ItemId::new("a.txt")).unwrap();
        assert_eq!(read, Some(b"abc".to_vec()));
    }

    #[test]
    fn the_mark_of_a_place_is_the_commit_checked_out_there() {
        let (repo, places) = with_places();
        let mut software = Software::with_places(repo.root(), &places.0, "main");
        let work = software.isolate("c1").unwrap().work;
        fs::write(work.join("a.txt"), "a\n").unwrap();
        software.keep("c1", "agents", "work", None).unwrap();
        let there = Git::new(&work).head().unwrap();
        assert_ne!(there, repo.git(&["rev-parse", "HEAD"]));
        assert_eq!(software.mark("c1").unwrap(), Mark::new(there));
        // A change with no place yet has no mark.
        assert!(software.mark("c2").is_err());
    }

    #[test]
    fn what_is_read_where_a_change_works_is_the_bytes_of_its_files() {
        let (repo, places) = with_places();
        let mut software = Software::with_places(repo.root(), &places.0, "main");
        let work = software.isolate("c1").unwrap().work;
        fs::write(work.join("a.txt"), "abc").unwrap();
        fs::create_dir_all(work.join("folder")).unwrap();
        std::os::unix::fs::symlink(work.join("a.txt"), work.join("link")).unwrap();
        let mut read = |item: &str| software.read("c1", &ItemId::new(item));
        assert_eq!(read("a.txt").unwrap(), Some(b"abc".to_vec()));
        assert_eq!(read("link").unwrap(), Some(b"abc".to_vec()));
        for nothing in ["folder", "missing.txt", "a.txt/inside"] {
            assert_eq!(read(nothing).unwrap(), None, "{nothing}");
        }
        let long = "x".repeat(300);
        let err = read(&long).unwrap_err();
        assert!(matches!(&err, Error::Read(item, _) if *item == long));
        let start = format!("cannot read {long}: ");
        assert!(err.to_string().starts_with(&start), "{err}");
        // Read as `file_hash` reads the disk: one formula for both.
        let bytes = read("a.txt").unwrap();
        let on_disk = guard::file_hash(&work.join("a.txt")).unwrap();
        assert_eq!(guard::hash_of(bytes.as_deref()), on_disk);
    }

    /// The commits of the branch of `c1` of `repo` since `since`, newest first: the subject of
    /// each, a blank line, then the files it changed, one a line.
    fn commits_since(repo: &Repo, since: &str) -> Vec<String> {
        let range = format!("{since}..change/c1");
        let count = repo.git(&["rev-list", "--count", &range]);
        let shown = |back: usize| {
            let commit = format!("change/c1~{back}");
            repo.git(&["show", "--name-only", "--format=%s", &commit])
        };
        (0..count.parse::<usize>().unwrap()).map(shown).collect()
    }

    /// The items whose ids are `ids`.
    fn items(ids: &[&str]) -> Vec<ItemId> {
        ids.iter().map(|id| ItemId::new(*id)).collect()
    }

    #[test]
    fn what_a_change_keeps_is_a_commit_on_its_branch_with_what_was_asked() {
        let (repo, places) = with_places();
        let mut software = Software::with_places(repo.root(), &places.0, "main");
        let work = software.isolate("c1").unwrap().work;
        let start = Git::new(&work).head().unwrap();
        // Nothing to keep, or nothing given, keeps nothing.
        software.keep("c1", "agents", "nothing", None).unwrap();
        fs::write(work.join("a.txt"), "by hand\n").unwrap();
        software.keep("c1", "human", "none", Some(&[])).unwrap();
        assert_eq!(commits_since(&repo, &start), Vec::<String>::new());
        // The person's first, only what was given; then the rest, but the state.
        fs::create_dir_all(work.join("tests")).unwrap();
        fs::write(work.join("tests/b.txt"), "agents\n").unwrap();
        fs::write(work.join("c.txt"), "by hand too\n").unwrap();
        let by_hand = "changed by hand after the change parked";
        let theirs = items(&["a.txt", "c.txt"]);
        let kept = software.keep("c1", "human", by_hand, Some(&theirs));
        kept.unwrap();
        fs::create_dir_all(work.join(".tminus")).unwrap();
        fs::write(work.join(".tminus/x.json"), "{}").unwrap();
        fs::remove_file(work.join(".gitignore")).unwrap();
        let earlier = "work of the earlier run, before relaunch";
        software.keep("c1", "agents", earlier, None).unwrap();
        let agents = format!("tminus(c1): {earlier}\n\n.gitignore\ntests/b.txt");
        let person = format!("tminus(c1): {by_hand}\n\na.txt\nc.txt");
        assert_eq!(commits_since(&repo, &start), [agents, person]);
        assert!(work.join(".tminus/x.json").is_file());
        // What a person kept already is not what is left to keep.
        let mark = software.mark("c1").unwrap();
        fs::write(work.join("d.txt"), "d\n").unwrap();
        let left = software.touched("c1", Some(&mark)).unwrap();
        assert_eq!(left.paths(), ["d.txt"]);
    }

    #[test]
    fn what_a_change_keeps_is_named_after_the_layout_of_its_project() {
        let repo = Repo::new();
        repo.identify();
        repo.write("gdd/project.toml", "");
        repo.commit();
        let start = repo.start("c1");
        repo.write("src/a.rs", "x\n");
        let mut software = Software::new(repo.root(), "main");
        let kept = software.keep("c1", "builder-build", "spike work", None);
        kept.unwrap();
        let commit = "gdd(c1): spike work\n\nsrc/a.rs";
        assert_eq!(commits_since(&repo, &start), [commit]);
        // What cannot be kept says what git said.
        let err = software.keep("c1", "human", "gone", Some(&items(&["missing.rs"])));
        let err = err.unwrap_err().to_string();
        assert!(err.starts_with("git add -- missing.rs failed"), "{err}");
    }

    #[test]
    fn the_recipe_leaves_the_places_and_what_they_keep_to_the_blueprint() {
        let source = include_str!("../recipe/mod.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        assert!(production.len() < source.len(), "the recipe has no tests");
        for word in ["commit_files", "worktree::open(", "worktree::remove("] {
            assert!(!production.contains(word), "the recipe names {word}");
        }
    }

    /// A repository whose `main` has `src/kept.txt`, a folder for the places of its close
    /// tests, and settings that say nothing: the main branch is `main`.
    fn for_close_tests() -> (Repo, Folder, Settings) {
        let repo = Repo::new();
        repo.write("src/kept.txt", "committed\n");
        repo.commit();
        (repo, Folder::new(), Settings::from_toml("").unwrap())
    }

    /// The text of the file `item` from `work`.
    fn text_of(work: &Path, item: &str) -> String {
        fs::read_to_string(work.join(item)).unwrap()
    }

    /// Whether `secret` has the shape of a made secret: `ghp_` and 36 letters and digits.
    fn is_made_secret(secret: &str) -> bool {
        let rest = secret.strip_prefix("ghp_").unwrap_or_default();
        rest.len() == 36 && rest.bytes().all(|byte| byte.is_ascii_alphanumeric())
    }

    #[test]
    fn the_place_of_a_close_test_is_made_anew_from_the_main_branch_on_no_branch() {
        let (repo, places, _) = for_close_tests();
        let trunk = repo.git(&["rev-parse", "main"]);
        repo.git(&["branch", "trunk"]);
        // The main branch the rules name stayed behind; the main checkout is on another one.
        repo.write("src/kept.txt", "on main\n");
        repo.commit();
        repo.start("c2");
        repo.write("src/kept.txt", "on c2\n");
        repo.commit();
        let settings = Settings::from_toml("main_branch = \"trunk\"\n").unwrap();
        let mut software = Software::for_close_tests(repo.root(), &places.0, &settings);
        let work = places.0.join("close.x");
        assert_eq!(software.isolate("close.x").unwrap(), place(&work, true));
        let there = Git::new(&work);
        assert_eq!(there.current_branch().unwrap(), "HEAD");
        assert_eq!(there.head().unwrap(), trunk);
        assert_eq!(text_of(&work, "src/kept.txt"), "committed\n");
        // What an earlier run left there goes: each close test starts again.
        fs::write(work.join("left.txt"), "x\n").unwrap();
        assert_eq!(software.isolate("close.x").unwrap(), place(&work, true));
        assert!(!work.join("left.txt").exists());
        // Let go, it is gone and git no longer lists it.
        software.vacate("close.x").unwrap();
        assert!(!work.exists());
        let listed = repo.git(&["worktree", "list", "--porcelain"]);
        assert!(!listed.contains("close.x"), "{listed}");
        // Anything else in the way is refused, named, and stays.
        fs::create_dir_all(work.join("mine")).unwrap();
        let err = software.isolate("close.x").unwrap_err().to_string();
        let shown = work.display();
        let way = format!("{shown} is in the way of the place of a close test");
        assert!(err.starts_with(&way), "{err}");
        assert!(work.join("mine").is_dir());
        // A change still works in its worktree, on its branch.
        repo.git(&["branch", "change/c1", "main"]);
        let mut changes = Software::with_places(repo.root(), &places.0, "main");
        let c1 = places.0.join("c1");
        assert_eq!(changes.isolate("c1").unwrap(), place(&c1, true));
        assert_eq!(Git::new(&c1).current_branch().unwrap(), "change/c1");
    }

    #[test]
    fn a_failure_is_planted_only_in_the_place_of_a_close_test() {
        let (repo, places, settings) = for_close_tests();
        let item = ItemId::new("src/bad.txt");
        // Where a change works, nothing is planted.
        let mut at_root = Software::new(repo.root(), "main");
        let mut changes = Software::with_places(repo.root(), &places.0, "main");
        let why = "c1: software plants only in the place of a close test";
        for software in [&mut at_root, &mut changes] {
            let err = software.plant("c1", &item, "bad\n").unwrap_err();
            assert_eq!(err.to_string(), why);
        }
        assert!(!repo.root().join("src/bad.txt").exists());
        let mut software = Software::for_close_tests(repo.root(), &places.0, &settings);
        let work = software.isolate("close.x").unwrap().work;
        software.plant("close.x", &item, "bad\n").unwrap();
        assert_eq!(text_of(&work, "src/bad.txt"), "bad\n");
        assert!(!repo.root().join("src/bad.txt").exists());
        // A file the main line has there is replaced; the checkout's stays as it was.
        let kept = ItemId::new("src/kept.txt");
        software.plant("close.x", &kept, "planted\n").unwrap();
        assert_eq!(text_of(&work, "src/kept.txt"), "planted\n");
        assert_eq!(text_of(repo.root(), "src/kept.txt"), "committed\n");
        // Its folders are made.
        let deep = ItemId::new("deep/new/bad.txt");
        software.plant("close.x", &deep, "x").unwrap();
        assert_eq!(text_of(&work, "deep/new/bad.txt"), "x");
        assert_eq!(repo.git(&["status", "--porcelain"]), "");
    }

    #[test]
    fn each_secret_a_planted_failure_asks_for_is_made_as_it_is_planted() {
        let (repo, places, settings) = for_close_tests();
        let mut software = Software::for_close_tests(repo.root(), &places.0, &settings);
        let work = software.isolate("close.x").unwrap().work;
        let item = ItemId::new("src/leak.txt");
        let content = format!("token = {SECRET} {SECRET}\n");
        software.plant("close.x", &item, &content).unwrap();
        let text = text_of(&work, "src/leak.txt");
        assert!(text.starts_with("token = "), "{text}");
        assert!(text.ends_with('\n'), "{text}");
        let made: Vec<&str> = text["token = ".len()..].split_whitespace().collect();
        assert_eq!(made.len(), 2, "{text}");
        assert!(made.iter().all(|secret| is_made_secret(secret)), "{text}");
        assert_ne!(made[0], made[1]);
        assert!(!text.contains(SECRET), "{text}");
        assert_eq!(SECRET, "{secret}");
    }

    #[test]
    fn a_made_secret_has_the_shape_of_a_token_scanners_know_and_is_never_the_same() {
        let one = made_secret().unwrap();
        let two = made_secret().unwrap();
        assert_eq!((one.len(), two.len()), (40, 40));
        assert!(is_made_secret(&one) && is_made_secret(&two));
        assert_ne!(one, two);
        // Spread over its letters and digits, not a few of them.
        let letters = format!("{}{}", &one[4..], &two[4..]);
        let distinct: BTreeSet<char> = letters.chars().collect();
        assert!(distinct.len() > 5, "{distinct:?}");
    }

    #[test]
    fn a_failure_is_never_planted_out_of_its_place_through_a_link_or_over_a_folder() {
        let (repo, places, settings) = for_close_tests();
        let elsewhere = Folder::new();
        elsewhere.write("theirs.txt", "theirs\n");
        let theirs = elsewhere.0.join("theirs.txt");
        symlink(&elsewhere.0, repo.root().join("linked")).unwrap();
        symlink(&theirs, repo.root().join("src/link.txt")).unwrap();
        repo.commit();
        let mut software = Software::for_close_tests(repo.root(), &places.0, &settings);
        let work = software.isolate("close.x").unwrap().work;
        let mut plant = |item: &str| software.plant("close.x", &ItemId::new(item), "bad\n");
        let absolute = elsewhere.0.join("bad.txt").display().to_string();
        for outside in [
            "../c1/src/bad.txt",
            absolute.as_str(),
            "src/../bad.txt",
            "./bad.txt",
        ] {
            let err = plant(outside).unwrap_err();
            let why = format!("cannot plant {outside}: it is not a path inside the project");
            assert_eq!(err.to_string(), why);
        }
        // A link on the way is refused; a link at the path goes, never followed.
        let err = plant("linked/bad.txt").unwrap_err();
        let why = "cannot plant linked/bad.txt: linked is a symbolic link";
        assert_eq!(err.to_string(), why);
        plant("src/link.txt").unwrap();
        // A folder there is refused, as the system says it.
        let err = plant("src").unwrap_err();
        let why = "cannot plant src: Is a directory (os error 21)";
        assert_eq!(err.to_string(), why);
        let link = fs::symlink_metadata(work.join("src/link.txt")).unwrap();
        assert!(link.is_file());
        assert_eq!(text_of(&work, "src/link.txt"), "bad\n");
        assert_eq!(fs::read_to_string(&theirs).unwrap(), "theirs\n");
        assert_eq!(fs::read_dir(&elsewhere.0).unwrap().count(), 1);
        assert!(!places.0.join("c1").exists());
        assert!(!work.join("bad.txt").exists());
        assert_eq!(text_of(&work, "src/kept.txt"), "committed\n");
    }

    #[test]
    fn the_places_go_where_the_rules_file_of_the_project_says() {
        let project = Folder::new();
        let name = project.0.file_name().unwrap().to_str().unwrap();
        let next = project.0.join("..").join(format!("{name}.worktrees"));
        assert_eq!(places(&project.0), Ok(next));
        project.write("tminus/rules.toml", "[parallel]\nfolder = \"../trees\"\n");
        assert_eq!(places(&project.0), Ok(project.0.join("../trees")));
        project.write("tminus/rules.toml", "[parallel");
        let err = places(&project.0).unwrap_err();
        let start = "tminus/rules.toml: invalid TOML: ";
        assert!(err.starts_with(start), "{err}");
        // A rules file that cannot be read is not a missing one.
        fs::remove_file(project.0.join("tminus/rules.toml")).unwrap();
        fs::create_dir_all(project.0.join("tminus/rules.toml")).unwrap();
        let err = places(&project.0).unwrap_err();
        let why = "cannot read tminus/rules.toml: Is a directory (os error 21)";
        assert_eq!(err, why);
        // A project of the old layout has its rules file in gdd/.
        let old = Folder::new();
        old.write("gdd/project.toml", "");
        old.write("gdd/rules.toml", "[parallel]\nfolder = \"/srv/trees\"\n");
        assert_eq!(places(&old.0), Ok(PathBuf::from("/srv/trees")));
    }
}
