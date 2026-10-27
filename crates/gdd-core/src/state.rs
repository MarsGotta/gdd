//! Where gdd keeps its state: the folder whose `.gdd/` holds the records and approvals of the
//! changes, the gate reports, the Reviewer's verdicts, the snapshots, the log, the self-test and
//! the binary. Every working tree of a repository shares the state of its main checkout, so a
//! change can run in a git worktree of its own and still read and write the same records. The
//! files of the project (`gdd/*.toml`, `features/`...) and the gate commands stay with the
//! folder gdd runs in.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::git::{Dirs, Git};

/// The name git gives the folder of the repository of a checkout.
const GIT: &str = ".git";

/// The folder whose `.gdd/` holds the state of the project at `project_root`: in a linked
/// worktree, the same folder of the main checkout; otherwise `project_root` itself, as in a
/// single checkout, the main checkout, a folder git cannot read (not a repository yet) or a
/// worktree of a bare repository, which has no main checkout. Git is asked once: work it out
/// once per command and pass it down.
pub fn root(project_root: &Path) -> PathBuf {
    match Git::new(project_root).dirs() {
        Ok(dirs) => of(project_root, &dirs),
        Err(_) => project_root.to_path_buf(),
    }
}

/// The state root of the project at `project_root`, whose working tree git keeps in `dirs`.
fn of(project_root: &Path, dirs: &Dirs) -> PathBuf {
    let linked = dirs.common != dirs.own && dirs.common.file_name() == Some(OsStr::new(GIT));
    match dirs.common.parent() {
        Some(main) if linked => main.join(&dirs.prefix),
        _ => project_root.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::{Folder, Repo};
    use std::fs;

    /// A repository with a project in `app/`, and a linked worktree of it in a folder of its own,
    /// on the branch `change/c1`.
    fn with_worktree() -> (Repo, Folder, PathBuf) {
        let repo = Repo::new();
        repo.write("app/gdd/project.toml", "");
        repo.commit();
        let folder = Folder::new();
        let worktree = folder.0.join("c1");
        let path = worktree.to_string_lossy().into_owned();
        repo.git(&["worktree", "add", "-q", "-b", "change/c1", &path]);
        (repo, folder, worktree)
    }

    #[test]
    fn a_checkout_keeps_its_own_state_at_the_root_it_was_given() {
        let repo = Repo::new();
        repo.write("app/gdd/project.toml", "");
        assert_eq!(root(repo.root()), repo.root());
        let app = repo.root().join("app");
        assert_eq!(root(&app), app);
        // Through a link, not where git says the repository really is.
        let elsewhere = Folder::new();
        let link = elsewhere.0.join("link");
        std::os::unix::fs::symlink(repo.root(), &link).unwrap();
        assert_eq!(root(&link), link);
        let plain = Folder::new();
        assert_eq!(root(&plain.0), plain.0);
    }

    #[test]
    fn a_linked_worktree_shares_the_state_of_the_main_checkout() {
        let (repo, _folder, worktree) = with_worktree();
        let main = fs::canonicalize(repo.root()).unwrap();
        assert_eq!(root(&worktree), main);
        assert_eq!(root(&worktree.join("app")), main.join("app"));
        // The main checkout keeps its own, with the worktree there.
        assert_eq!(root(repo.root()), repo.root());
    }

    #[test]
    fn a_worktree_of_a_bare_repository_keeps_its_own_state() {
        let repo = Repo::new();
        let bare = Folder::new();
        let path = bare.0.join("repo.git").to_string_lossy().into_owned();
        repo.git(&["clone", "-q", "--bare", ".", &path]);
        let worktree = bare.0.join("c1");
        let added = worktree.to_string_lossy().into_owned();
        repo.git(&["-C", &path, "worktree", "add", "-q", &added, "main"]);
        assert_eq!(root(&worktree), worktree);
    }
}
