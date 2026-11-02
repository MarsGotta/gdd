//! Bringing a change up to date with the main branch, for the blueprint of software (see
//! [`super::touched::Software`]): the main checkout left on the main branch before a change gets
//! its worktree, with the work an earlier run left there kept ([`settle`]); whether the main
//! branch moved under a change ([`main_moved`]) and taking it in a merge of the change's own
//! ([`refreshed`]); and whether a change has anything the main branch lacks ([`nothing_new`]).
//! Joining it is [`super::merge::merge`].

use std::path::Path;

use crate::change;
use crate::git::{self, Git};
use crate::guard::{Error, Mark, Moved};
use crate::layout::Layout;
use crate::recipe::{self, EARLIER_RUN};

/// Why a launch does not leave a working tree with uncommitted changes.
pub(crate) const UNCOMMITTED: &str =
    "The working tree has uncommitted changes. Commit them before launching a change.";

/// Why a change parks when the latest main does not merge into it: the paths in conflict follow,
/// then the way out (see [`conflicts`]).
const CONFLICTS: &str = "main moved under it and the merge has conflicts in";

/// Leaves the main checkout at `checkout` on the main branch `main` before change `id` runs in
/// its worktree. On the branch of the change, its work there is the work of an earlier run,
/// committed first when there is any, without the state (see [`Git::commit_work`]): a person's,
/// as the agents work in the worktree. A relaunch judges it as it judges committed work, and its
/// build starts after it, so what test mode wrote before is not build mode's doing. On any other
/// branch, uncommitted changes in tracked files are refused.
pub(super) fn settle(checkout: &Path, main: &str, id: &str) -> Result<(), Error> {
    let git = Git::new(checkout);
    if git.current_branch()? == change::branch(id) {
        if git.has_work()? {
            git.commit_work(&recipe::commit_message(checkout, id, EARLIER_RUN))?;
        }
    } else if git.has_tracked_changes()? {
        return Err(Error::Blueprint(UNCOMMITTED.to_string()));
    }
    Ok(git.checkout(main)?)
}

/// Whether the main branch `main` moved since the change checked out at `work` was last built:
/// its commits since they parted changed something anywhere in the repository, also outside the
/// project folder, as the merge brings it all. The merge commit of the change itself, with
/// nothing else, is no move.
pub(super) fn main_moved(work: &Path, main: &str) -> Result<bool, git::Error> {
    Git::new(work).changed_anywhere("HEAD", main)
}

/// The latest main branch `main` taken into change `id`, checked out at `work` (see
/// [`take_main`]): at the commit that took it; or, when it has conflicts, why the change parks
/// (see [`conflicts`]), the branch as it was with its work committed.
pub(super) fn refreshed(work: &Path, main: &str, id: &str) -> Result<Moved, Error> {
    let paths = take_main(work, main, id)?;
    if !paths.is_empty() {
        return Ok(Moved::Conflicts(conflicts(work, main, id, &paths)));
    }
    Ok(Moved::Taken(Mark::new(Git::new(work).head()?)))
}

/// Takes the latest main branch `main` into the branch of change `id`, checked out at `work`:
/// its work committed first, then main merged into it with a merge commit, never a rebase. A
/// merge with conflicts is given up, the branch as it was with its work committed: the paths in
/// conflict, which are none when it merged. The commits start with `tminus(<id>): `, or with
/// `gdd(<id>): ` in a project set up before the rename (see [`Layout::name`]).
fn take_main(work: &Path, main: &str, id: &str) -> Result<Vec<String>, git::Error> {
    let git = Git::new(work);
    let tool = Layout::at(work).name();
    if git.has_work()? {
        git.commit_work(&format!("{tool}({id}): change work"))?;
    }
    let message = format!("{tool}({id}): take the latest main");
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

/// Why change `id` parks when the latest main branch `main` does not merge into it, checked out
/// at `work`, with `paths` in conflict: them, then the way out, by hand in its worktree. The
/// launch after it takes the change on top of the latest main, as its branch then holds it.
fn conflicts(work: &Path, main: &str, id: &str, paths: &[String]) -> String {
    let (paths, branch, work) = (paths.join(", "), change::branch(id), work.display());
    format!(
        "{CONFLICTS} {paths}: merge {main} into {branch} in its worktree ({work}), settle the \
         conflicts, commit, then tminus launch {id}"
    )
}

/// Whether the branch of change `id`, checked out at `work`, has nothing the main branch `main`
/// lacks: no work to commit, and no commit `main` does not have.
pub(super) fn nothing_new(work: &Path, id: &str, main: &str) -> Result<bool, git::Error> {
    let git = Git::new(work);
    if git.has_changes()? {
        return Ok(false);
    }
    Ok(git.merge_base(main, &change::branch(id))? == git.head()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use crate::classify::{Change, Classification, classify};
    use crate::gates;
    use crate::git::tests::{Folder, Repo};
    use crate::guard::Blueprint;
    use crate::repo::touched::Software;
    use crate::rules;
    use crate::zones::ZoneMap;

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
        let commits = "tminus(c1): take the latest main\ntminus(c1): change work";
        assert_eq!(log, commits);
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
        assert_eq!(subject, "tminus(c1): change work");
        // A merge that fails for another reason is an error, the branch as it was.
        let err = take_main(&work, "trunk", "c1").unwrap_err().to_string();
        let failed = "git merge --no-ff --no-edit -q -m tminus(c1): take the latest main trunk";
        assert!(err.starts_with(failed), "{err}");
        assert_eq!(git.head().unwrap(), repo.git(&["rev-parse", "change/c1"]));
        let plain = Folder::new();
        assert!(take_main(&plain.0, "main", "c1").is_err());
    }

    #[test]
    fn a_change_of_the_old_layout_takes_main_with_its_gdd_names() {
        let (repo, trees) = moved();
        let work = trees.0.join("c1");
        fs::create_dir_all(work.join("gdd")).unwrap();
        fs::write(work.join("gdd/project.toml"), "").unwrap();
        assert!(take_main(&work, "main", "c1").unwrap().is_empty());
        let log = repo.git(&["log", "--format=%s", "-2", "change/c1"]);
        assert_eq!(log, "gdd(c1): take the latest main\ngdd(c1): change work");
    }

    /// A repository with an author that ignores `.tminus/`, with the branch of change `c1`, not
    /// checked out, and a folder for the worktrees of the changes.
    fn with_places() -> (Repo, Folder) {
        let repo = Repo::new();
        repo.identify();
        repo.write(".gitignore", ".tminus/\n");
        repo.commit();
        repo.git(&["branch", "change/c1"]);
        (repo, Folder::new())
    }

    /// The commit message of work kept before a relaunch.
    const RELAUNCH: &str = "tminus(c1): work of the earlier run, before relaunch";

    #[test]
    fn settling_keeps_the_earlier_run_s_work_on_the_branch_and_goes_to_main() {
        let (repo, places) = with_places();
        let mut software = Software::with_places(repo.root(), &places.0, "main");
        repo.git(&["checkout", "-q", "change/c1"]);
        repo.write("README.md", "earlier run\n");
        repo.write("src/only.rs", "not tracked yet\n");
        repo.write(".tminus/changes/c1/change.json", "{}");
        software.settle("c1").unwrap();
        let kept = repo.git(&["show", "--name-only", "--format=%s", "change/c1"]);
        assert_eq!(kept, format!("{RELAUNCH}\n\nREADME.md\nsrc/only.rs"));
        assert_eq!(repo.tool().current_branch().unwrap(), "main");
        assert!(!repo.tool().has_changes().unwrap());
        let record = repo.root().join(".tminus/changes/c1/change.json");
        assert!(record.is_file());
        // With nothing there, no commit: only the way back to main.
        let tip = repo.git(&["rev-parse", "change/c1"]);
        repo.git(&["checkout", "-q", "change/c1"]);
        software.settle("c1").unwrap();
        assert_eq!(repo.git(&["rev-parse", "change/c1"]), tip);
        assert_eq!(repo.tool().current_branch().unwrap(), "main");
    }

    #[test]
    fn settling_never_keeps_the_state() {
        let (repo, places) = with_places();
        repo.git(&["checkout", "-q", "change/c1"]);
        // From here on, the project does not ignore the state.
        repo.git(&["rm", "-q", ".gitignore"]);
        repo.write(".tminus/changes/c1/change.json", "{}");
        let mut software = Software::with_places(repo.root(), &places.0, "main");
        software.settle("c1").unwrap();
        let kept = repo.git(&["show", "--name-only", "--format=", "change/c1"]);
        assert_eq!(kept, ".gitignore");
        assert_eq!(repo.tool().current_branch().unwrap(), "main");
    }

    #[test]
    fn settling_elsewhere_refuses_uncommitted_tracked_work_and_stays() {
        let (repo, places) = with_places();
        let mut software = Software::with_places(repo.root(), &places.0, "main");
        repo.git(&["checkout", "-q", "-b", "other"]);
        repo.write("README.md", "changed\n");
        let err = software.settle("c1").unwrap_err();
        assert!(matches!(&err, Error::Blueprint(_)), "{err:?}");
        assert_eq!(err.to_string(), UNCOMMITTED);
        assert_eq!(repo.tool().current_branch().unwrap(), "other");
        assert!(repo.tool().has_changes().unwrap());
        // Clean, or with files git does not track, it goes to main.
        repo.git(&["checkout", "-q", "--", "README.md"]);
        repo.write("notes.txt", "mine\n");
        software.settle("c1").unwrap();
        assert_eq!(repo.tool().current_branch().unwrap(), "main");
        assert!(repo.root().join("notes.txt").is_file());
        let plain = Folder::new();
        assert!(Software::new(&plain.0, "main").settle("c1").is_err());
    }

    /// The record of change `c1` with G3 open.
    const G3_OPEN: &str = r#"{"id": "c1", "title": "App", "type": "Feature", "state": "in-pre",
                              "gates": {"G3": {"decision": "ok", "by": "machines"}}}"#;

    /// The project of [`with_places`] with change `c1`, G3 open, checked out in its worktree in
    /// the folder of places: its project root there.
    fn in_its_worktree(repo: &Repo, places: &Folder) -> std::path::PathBuf {
        repo.write(".tminus/changes/c1/change.json", G3_OPEN);
        let mut software = Software::with_places(repo.root(), &places.0, "main");
        software.isolate("c1").unwrap().work
    }

    #[test]
    fn with_places_a_change_is_blocked_by_a_main_checkout_that_cannot_take_it() {
        let (repo, places) = with_places();
        in_its_worktree(&repo, &places);
        let mut software = Software::with_places(repo.root(), &places.0, "main");
        assert_eq!(software.blocked("c1").unwrap(), None);
        let shown = repo.root().display();
        repo.git(&["checkout", "-q", "-b", "other"]);
        let off = format!(
            "the merge needs the main checkout ({shown}) on main, and it is on other: switch it \
             back to main"
        );
        assert_eq!(software.blocked("c1").unwrap(), Some(off));
        repo.git(&["checkout", "-q", "main"]);
        repo.write("README.md", "changed\n");
        let dirty = format!(
            "the merge needs the main checkout ({shown}) without uncommitted changes in tracked \
             files: commit or stash them"
        );
        assert_eq!(software.blocked("c1").unwrap(), Some(dirty));
        let plain = Folder::new();
        let mut nowhere = Software::with_places(&plain.0, &places.0, "main");
        assert!(nowhere.blocked("c1").is_err());
    }

    #[test]
    fn where_it_runs_a_change_is_blocked_by_what_merge_refuses() {
        let (repo, places) = with_places();
        let work = in_its_worktree(&repo, &places);
        let mut joining = Software::joining(&work, repo.root(), "main");
        assert_eq!(joining.place("c1").unwrap().work, work);
        assert_eq!(joining.blocked("c1").unwrap(), None);
        repo.write("README.md", "changed\n");
        let dirty = joining.blocked("c1").unwrap().unwrap_or_default();
        assert!(dirty.ends_with("commit or stash them"), "{dirty}");
        repo.git(&["checkout", "-q", "--", "README.md"]);
        let off = "Not on branch change/c1: merge runs where the last work of the change is.";
        let mut at_root = Software::new(repo.root(), "main");
        assert_eq!(at_root.blocked("c1").unwrap(), Some(off.to_string()));
        repo.write(".tminus/changes/c1/change.json", r#"{"gates": {}}"#);
        let closed = Some("G3 is not open.".to_string());
        assert_eq!(joining.blocked("c1").unwrap(), closed);
        assert_eq!(at_root.blocked("c1").unwrap(), closed);
    }

    #[test]
    fn a_change_is_behind_once_main_moved_under_it() {
        let (repo, places) = with_places();
        let work = in_its_worktree(&repo, &places);
        let mut software = Software::with_places(repo.root(), &places.0, "main");
        assert!(!software.behind("c1").unwrap());
        repo.write("news.txt", "news\n");
        repo.commit();
        assert!(software.behind("c1").unwrap());
        let mut joining = Software::joining(&work, repo.root(), "main");
        assert!(joining.behind("c1").unwrap());
        let mut trunk = Software::joining(&work, repo.root(), "trunk");
        assert!(trunk.behind("c1").is_err());
    }

    #[test]
    fn refreshing_keeps_the_work_first_then_takes_main_at_a_mark_of_its_own() {
        let (repo, places) = with_places();
        let work = in_its_worktree(&repo, &places);
        repo.write("news.txt", "news\n");
        repo.commit();
        fs::write(work.join("app.txt"), "work\n").unwrap();
        let mut software = Software::with_places(repo.root(), &places.0, "main");
        let moved = software.refresh("c1").unwrap();
        let head = Git::new(&work).head().unwrap();
        assert_eq!(moved, Moved::Taken(Mark::new(head.as_str())));
        let log = repo.git(&["log", "--format=%s", "-2", "change/c1"]);
        let commits = "tminus(c1): take the latest main\ntminus(c1): change work";
        assert_eq!(log, commits);
        assert!(work.join("news.txt").is_file());
        assert!(!software.behind("c1").unwrap());
    }

    #[test]
    fn refreshing_into_a_conflict_says_why_and_leaves_the_work_kept_as_it_was() {
        let (repo, places) = with_places();
        let work = in_its_worktree(&repo, &places);
        repo.write("app.txt", "main\n");
        repo.commit();
        fs::write(work.join("app.txt"), "change\n").unwrap();
        let mut software = Software::with_places(repo.root(), &places.0, "main");
        let why = format!(
            "main moved under it and the merge has conflicts in app.txt: merge main into \
             change/c1 in its worktree ({}), settle the conflicts, commit, then tminus launch c1",
            work.display()
        );
        assert_eq!(software.refresh("c1").unwrap(), Moved::Conflicts(why));
        let subject = repo.git(&["log", "-1", "--format=%s", "change/c1"]);
        assert_eq!(subject, "tminus(c1): change work");
        let app = fs::read_to_string(work.join("app.txt")).unwrap();
        assert_eq!(app, "change\n");
        assert!(!Git::new(&work).has_changes().unwrap());
        let mut trunk = Software::joining(&work, repo.root(), "trunk");
        assert!(trunk.refresh("c1").is_err());
    }

    #[test]
    fn a_change_is_ahead_while_it_has_what_main_lacks() {
        let (repo, places) = with_places();
        let work = in_its_worktree(&repo, &places);
        let mut software = Software::with_places(repo.root(), &places.0, "main");
        assert!(!software.ahead("c1").unwrap());
        fs::write(work.join("app.txt"), "work\n").unwrap();
        assert!(software.ahead("c1").unwrap());
        let tree = work.to_string_lossy().into_owned();
        repo.git(&["-C", &tree, "add", "app.txt"]);
        repo.git(&["-C", &tree, "commit", "-q", "-m", "work"]);
        assert!(software.ahead("c1").unwrap());
        // Right after it joined main, nothing is left.
        repo.git(&["merge", "--no-ff", "-q", "-m", "joined", "change/c1"]);
        assert!(!software.ahead("c1").unwrap());
        let mut trunk = Software::joining(&work, repo.root(), "trunk");
        assert!(trunk.ahead("c1").is_err());
    }

    /// The classification of a change that plans `path`, with `src/storage/**` red.
    fn classified(path: &str) -> Classification {
        let zones = "default = \"yellow\"\n[[zone]]\ncolor = \"red\"\n\
                     paths = [\"src/storage/**\"]\n";
        let change = Change {
            paths: vec![path.to_string()],
            min_color: None,
            resumable: false,
        };
        let zones = ZoneMap::from_toml(zones).unwrap();
        classify(&zones, &rules::default_table(), &change).unwrap()
    }

    #[test]
    fn integrating_merges_the_change_with_the_color_it_was_given() {
        for (path, color) in [("src/storage/db.rs", "red"), ("src/app.rs", "yellow")] {
            let (repo, places) = with_places();
            let commands = "[release]\nsimulated = true\nbuild = \"true\"\n";
            repo.write(gates::COMMANDS, commands);
            repo.commit();
            let work = in_its_worktree(&repo, &places);
            fs::write(work.join("app.txt"), "work\n").unwrap();
            let mut software = Software::with_places(repo.root(), &places.0, "main");
            let printed = software.integrate("c1", &classified(path)).unwrap();
            let printed: serde_json::Value = serde_json::from_str(&printed).unwrap();
            assert_eq!(printed["ok"], true, "{printed}");
            let message = repo.git(&["log", "-1", "--format=%B", "main"]);
            let line = format!("\nColor: {color} · lane full · type Feature\n");
            assert!(message.contains(&line), "{message}");
            repo.git(&["cat-file", "-e", "main:app.txt"]);
        }
    }

    #[test]
    fn the_recipe_leaves_the_main_branch_to_the_blueprint() {
        let source = include_str!("../recipe/mod.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        assert!(production.len() < source.len(), "the recipe has no tests");
        for word in [
            "main_moved",
            "take_main",
            "not_ready",
            "may_merge",
            "nothing_new",
            "to_main",
            "fn merge(&mut self",
        ] {
            assert!(!production.contains(word), "the recipe names {word}");
        }
    }

    #[test]
    fn what_merge_refuses_is_the_error_of_the_integration() {
        let (repo, places) = with_places();
        let work = in_its_worktree(&repo, &places);
        repo.write(".tminus/changes/c1/change.json", r#"{"gates": {}}"#);
        let mut joining = Software::joining(&work, repo.root(), "main");
        let app = classified("src/app.rs");
        let err = joining.integrate("c1", &app).unwrap_err();
        assert!(matches!(&err, Error::Blueprint(_)), "{err:?}");
        assert_eq!(err.to_string(), "G3 is not open.");
    }
}
