//! Where gdd keeps its state: the folder whose `.gdd/` holds the records and approvals of the
//! changes, the gate reports, the Reviewer's verdicts, the snapshots, the log, the self-test and
//! the binary. Every working tree of a repository shares the state of its main checkout, so a
//! change can run in a git worktree of its own and still read and write the same records. The
//! files of the project (`gdd/*.toml`, `features/`...) and the gate commands stay with the
//! folder gdd runs in.

use std::path::{Path, PathBuf};

use crate::git::{Dirs, Git};

/// The folder whose `.gdd/` holds the state of the project at `project_root`: in a linked
/// worktree, the same folder of the main checkout git names; otherwise `project_root` itself,
/// as in a single checkout, the main checkout or a folder git cannot read (not a repository
/// yet). A linked worktree of a bare repository, or of one git cannot name the checkout of, is
/// refused. Git is asked once: work it out once per command and pass it down.
pub fn root(project_root: &Path) -> Result<PathBuf, String> {
    let git = Git::new(project_root);
    match git.dirs() {
        Ok(dirs) if dirs.common != dirs.own => shared(&git, project_root, &dirs),
        _ => Ok(project_root.to_path_buf()),
    }
}

/// The state root of the project at `project_root`, in a linked worktree of the repository
/// `git` reads, kept in `dirs`: the same folder of the main checkout. Refused when git names no
/// checkout: the repository is bare, or git names the repository itself because it does not
/// know where its checkout is (a repository kept apart, `git init --separate-git-dir`).
fn shared(git: &Git, project_root: &Path, dirs: &Dirs) -> Result<PathBuf, String> {
    let main = git.main_worktree().map_err(|e| e.to_string())?;
    let worktree = project_root.display();
    let repository = main.path.display();
    if main.bare {
        return Err(format!(
            "{worktree} is a worktree of the bare repository {repository}, which has no main \
             checkout to share the state of gdd with: run gdd from a clone that is not bare"
        ));
    }
    if main.path == dirs.common {
        return Err(format!(
            "{worktree} cannot share the state of gdd: git does not know where the main checkout \
             of the repository {repository} is; run gdd from the main checkout"
        ));
    }
    // Through its components: joining an empty prefix would end the path with a slash.
    Ok(main.path.join(&dirs.prefix).components().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::{Folder, Repo};
    use std::fs;

    /// A linked worktree of `repo` in `folder`, on the new branch `change/c1`.
    fn worktree_of(repo: &Repo, folder: &Folder) -> PathBuf {
        let worktree = folder.0.join("c1");
        let path = worktree.to_string_lossy().into_owned();
        repo.git(&["worktree", "add", "-q", "-b", "change/c1", &path]);
        worktree
    }

    #[test]
    fn a_checkout_keeps_its_own_state_at_the_root_it_was_given() {
        let repo = Repo::new();
        repo.write("app/gdd/project.toml", "");
        assert_eq!(root(repo.root()).unwrap(), repo.root());
        let app = repo.root().join("app");
        assert_eq!(root(&app).unwrap(), app);
        // Through a link, not where git says the repository really is.
        let elsewhere = Folder::new();
        let link = elsewhere.0.join("link");
        std::os::unix::fs::symlink(repo.root(), &link).unwrap();
        assert_eq!(root(&link).unwrap(), link);
        let plain = Folder::new();
        assert_eq!(root(&plain.0).unwrap(), plain.0);
    }

    #[test]
    fn a_linked_worktree_shares_the_state_of_the_main_checkout() {
        let repo = Repo::new();
        repo.write("app/gdd/project.toml", "");
        repo.commit();
        let folder = Folder::new();
        let worktree = worktree_of(&repo, &folder);
        let main = fs::canonicalize(repo.root()).unwrap();
        assert_eq!(root(&worktree).unwrap(), main);
        // As gdd prints it: without a slash at the end.
        let shown = root(&worktree).unwrap().display().to_string();
        assert_eq!(shown, main.display().to_string());
        assert_eq!(root(&worktree.join("app")).unwrap(), main.join("app"));
        // The main checkout keeps its own, with the worktree there.
        assert_eq!(root(repo.root()).unwrap(), repo.root());
    }

    #[test]
    fn a_worktree_of_a_repository_kept_apart_from_its_checkout_is_refused() {
        let folder = Folder::new();
        let checkout = folder.0.join("main");
        fs::create_dir_all(&checkout).unwrap();
        let repo = Repo(Folder(checkout));
        let apart = folder.0.join("repo.git").to_string_lossy().into_owned();
        repo.git(&["init", "-q", "-b", "main", "--separate-git-dir", &apart]);
        repo.write("app/gdd/project.toml", "");
        repo.commit();
        let worktrees = Folder::new();
        let worktree = worktree_of(&repo, &worktrees);
        // Git names the repository as the main checkout: it does not know where the checkout is.
        let repository = fs::canonicalize(&apart).unwrap();
        let listed = Git::new(&worktree).main_worktree().unwrap();
        assert_eq!((listed.path, listed.bare), (repository.clone(), false));
        let err = root(&worktree.join("app")).unwrap_err();
        let named = format!(
            "{} cannot share the state of gdd: git does not know where the main checkout of the \
             repository {} is",
            worktree.join("app").display(),
            repository.display()
        );
        assert!(err.starts_with(&named), "{err}");
        // The main checkout keeps its own.
        assert_eq!(root(repo.root()).unwrap(), repo.root());
        let app = repo.root().join("app");
        assert_eq!(root(&app).unwrap(), app);
    }

    #[test]
    fn a_worktree_of_a_bare_repository_is_refused() {
        let repo = Repo::new();
        let bare = Folder::new();
        let path = bare.0.join("repo.git").to_string_lossy().into_owned();
        repo.git(&["clone", "-q", "--bare", ".", &path]);
        let worktree = bare.0.join("c1");
        let added = worktree.to_string_lossy().into_owned();
        repo.git(&["-C", &path, "worktree", "add", "-q", &added, "main"]);
        let err = root(&worktree).unwrap_err();
        let repository = fs::canonicalize(&path).unwrap();
        let named = format!(
            "{} is a worktree of the bare repository {}, which has no main checkout",
            worktree.display(),
            repository.display()
        );
        assert!(err.starts_with(&named), "{err}");
    }
}
