//! The worktree of a change: a second working folder of the repository, on the branch of the
//! change, where its recipe runs while the main checkout stays on the main branch. The worktrees
//! live in one folder, `[parallel] folder` of the rules file, from the project root;
//! `../<project folder>.worktrees` by default. Their state stays in the main checkout (see
//! [`crate::state::root`]). A project in a folder of its repository is in that same folder of
//! each worktree. gdd makes, reuses and removes them with git from the main checkout, and never
//! through a link: a worktree is a folder of its own.

use std::fs::{self, Metadata};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::change;
use crate::config;
use crate::git::{self, Git};

/// The folder of the worktrees of the project at `root`, from the text of its rules file
/// `rules`: `[parallel] folder`, from the project root, or `../<project folder>.worktrees` when
/// it says nothing. The rest of the file is ignored.
pub fn folder(root: &Path, rules: &str) -> Result<PathBuf, config::Error> {
    #[derive(Default, Deserialize)]
    #[serde(default)]
    struct Raw {
        parallel: Parallel,
    }
    #[derive(Default, Deserialize)]
    #[serde(default)]
    struct Parallel {
        folder: Option<String>,
    }
    let raw: Raw = toml::from_str(rules).map_err(|e| config::Error::Toml(e.to_string()))?;
    let folder = raw.parallel.folder.map_or_else(
        || {
            let mut name = root.file_name().unwrap_or_default().to_os_string();
            name.push(".worktrees");
            Path::new("..").join(name)
        },
        PathBuf::from,
    );
    Ok(root.join(folder))
}

/// Where the worktree of change `id` goes in the folder of the worktrees `folder`:
/// `<folder>/<id>`. A link at either is refused.
pub fn path(folder: &Path, id: &str) -> Result<PathBuf, String> {
    seen(folder)?;
    let path = folder.join(id);
    seen(&path)?;
    Ok(path)
}

/// The project at `root`, a folder of the main checkout, in the worktree `path`: the same folder
/// from the top of the working tree, where the agents and the steps of the change run.
pub fn project_root(root: &Path, path: &Path) -> Result<PathBuf, String> {
    let prefix = Git::new(root).dirs().map_err(|e| e.to_string())?.prefix;
    // Through its components: joining an empty prefix would end the path with a slash.
    Ok(path.join(prefix).components().collect())
}

/// Leaves the worktree `path` of change `id` ready, from the main checkout `checkout`: the one
/// there when git lists it on the branch of the change; otherwise a new one on that branch, once
/// git forgets the worktrees whose folders are gone. Anything else at `path` is refused, named.
pub fn open(checkout: &Path, path: &Path, id: &str) -> Result<(), String> {
    let branch = change::branch(id);
    opened(checkout, path, &format!("change {id}"), &branch)
}

/// Leaves the worktree `path` of variant `number` of spike `id` ready, from the main checkout
/// `checkout`, as [`open`] leaves the one of a change, on the branch of the variant (see
/// [`change::variant_id`]): made from the main branch `main` when there is none, kept when there
/// is one.
pub fn open_variant(
    checkout: &Path,
    path: &Path,
    id: &str,
    number: usize,
    main: &str,
) -> Result<(), String> {
    let git = Git::new(checkout);
    let branch = change::branch(&change::variant_id(id, number));
    let said = |e: git::Error| e.to_string();
    if !git.exists(&branch).map_err(said)? {
        git.create_branch(&branch, main).map_err(said)?;
    }
    let what = format!("variant {number} of {id}");
    opened(checkout, path, &what, &branch)
}

/// Leaves the worktree `path` of `what` ready on `branch`, from the main checkout `checkout` (see
/// [`open`]).
fn opened(checkout: &Path, path: &Path, what: &str, branch: &str) -> Result<(), String> {
    let git = Git::new(checkout);
    let said = |e: git::Error| e.to_string();
    if seen(path)?.is_none() {
        git.prune_worktrees().map_err(said)?;
        return git.add_worktree(path, branch).map_err(said);
    }
    if is_worktree_on(&git, path, branch).map_err(said)? {
        return Ok(());
    }
    Err(format!(
        "{} is in the way of the worktree of {what}: it is not a worktree on {branch}. Move it \
         away, then run gdd again.",
        path.display()
    ))
}

/// Removes the worktree `path`, with what it has that git does not, from the main checkout
/// `checkout`, and has git forget the worktrees whose folders are gone.
pub fn remove(checkout: &Path, path: &Path) -> Result<(), String> {
    let git = Git::new(checkout);
    let said = |e: git::Error| e.to_string();
    if seen(path)?.is_some() {
        git.remove_worktree(path).map_err(said)?;
    }
    git.prune_worktrees().map_err(said)
}

/// What is at `path`, without following a link: `None` when there is nothing. A link is refused.
fn seen(path: &Path) -> Result<Option<Metadata>, String> {
    let shown = path.display();
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(format!(
            "{shown} is a link: gdd keeps the worktrees of the changes in folders of their own. \
             Remove the link, then run gdd again."
        )),
        Ok(meta) => Ok(Some(meta)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("cannot look at {shown}: {e}")),
    }
}

/// Whether git lists `path` as a worktree of the repository of `git` on `branch`: the same
/// folder, however each names it.
fn is_worktree_on(git: &Git, path: &Path, branch: &str) -> Result<bool, git::Error> {
    let Ok(here) = fs::canonicalize(path) else {
        return Ok(false);
    };
    let same = |at: &Path| fs::canonicalize(at).is_ok_and(|at| at == here);
    let listed = git.worktrees()?;
    Ok(listed.iter().any(|(at, on)| on == branch && same(at)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::{Folder, Repo};
    use std::os::unix::fs::symlink;

    #[test]
    fn the_worktrees_live_next_to_the_project_unless_the_rules_say_where() {
        let root = Path::new("/work/app");
        let next = folder(root, "").unwrap();
        assert_eq!(next, Path::new("/work/app/../app.worktrees"));
        let rules = "main_branch = \"main\"\n[parallel]\nfolder = \"../trees\"\n[limits]\n";
        let trees = folder(root, rules).unwrap();
        assert_eq!(trees, Path::new("/work/app/../trees"));
        let absolute = "[parallel]\nfolder = \"/srv/trees\"\n";
        assert_eq!(folder(root, absolute).unwrap(), Path::new("/srv/trees"));
        for wrong in ["[parallel]\nfolder = 3\n", "parallel = 1\n", "[parallel"] {
            let result = folder(root, wrong);
            assert!(matches!(result, Err(config::Error::Toml(_))), "{wrong:?}");
        }
    }

    #[test]
    fn the_worktree_path_is_the_change_in_the_folder_and_never_a_link() {
        let trees = Folder::new();
        assert_eq!(path(&trees.0, "c1").unwrap(), trees.0.join("c1"));
        let missing = trees.0.join("none");
        assert_eq!(path(&missing, "c1").unwrap(), missing.join("c1"));
        let elsewhere = Folder::new();
        symlink(&elsewhere.0, trees.0.join("c2")).unwrap();
        let err = path(&trees.0, "c2").unwrap_err();
        let link = format!("{} is a link: ", trees.0.join("c2").display());
        assert!(err.starts_with(&link), "{err}");
        let linked = trees.0.join("linked");
        symlink(&elsewhere.0, &linked).unwrap();
        let err = path(&linked, "c1").unwrap_err();
        let link = format!("{} is a link: ", linked.display());
        assert!(err.starts_with(&link), "{err}");
        // A folder that cannot be looked at is an error, not a missing one.
        trees.write("file", "");
        let under = trees.0.join("file/trees");
        let err = path(&under, "c1").unwrap_err();
        let start = format!("cannot look at {}: ", under.display());
        assert!(err.starts_with(&start), "{err}");
    }

    #[test]
    fn a_project_in_a_folder_of_its_repository_is_in_that_folder_of_the_worktree() {
        let repo = Repo::new();
        repo.write("app/gdd/project.toml", "");
        let path = Path::new("/trees/c1");
        // At the top: the worktree itself, as gdd prints it, without a slash at the end.
        let top = project_root(repo.root(), path).unwrap();
        assert_eq!(top.display().to_string(), "/trees/c1");
        let app = project_root(&repo.root().join("app"), path).unwrap();
        assert_eq!(app.display().to_string(), "/trees/c1/app");
        let plain = Folder::new();
        let err = project_root(&plain.0, path).unwrap_err();
        assert!(err.starts_with("git rev-parse "), "{err}");
    }

    /// A repository with the branch `change/c1`, and a folder for its worktrees.
    fn with_branch() -> (Repo, Folder) {
        let repo = Repo::new();
        repo.git(&["branch", "change/c1"]);
        (repo, Folder::new())
    }

    /// Whether git lists `path` as a worktree of `repo` on `change/c1`.
    fn listed(repo: &Repo, path: &Path) -> bool {
        is_worktree_on(&repo.tool(), path, "change/c1").unwrap()
    }

    #[test]
    fn a_worktree_is_made_on_the_branch_of_the_change_and_reused() {
        let (repo, trees) = with_branch();
        let path = trees.0.join("deep/c1");
        assert!(!listed(&repo, &path));
        open(repo.root(), &path, "c1").unwrap();
        assert!(listed(&repo, &path));
        assert_eq!(Git::new(&path).current_branch().unwrap(), "change/c1");
        // The work of an earlier run stays.
        let work = path.join("work.txt");
        fs::write(&work, "earlier\n").unwrap();
        open(repo.root(), &path, "c1").unwrap();
        assert_eq!(fs::read_to_string(&work).unwrap(), "earlier\n");
        // Gone, it is made again.
        fs::remove_dir_all(&path).unwrap();
        open(repo.root(), &path, "c1").unwrap();
        assert!(listed(&repo, &path));
        let missing = open(repo.root(), &trees.0.join("c9"), "c9").unwrap_err();
        assert!(missing.starts_with("git worktree add -q -- "), "{missing}");
    }

    #[test]
    fn what_is_in_the_way_of_a_worktree_is_refused_by_name() {
        let (repo, trees) = with_branch();
        let path = trees.0.join("c1");
        fs::create_dir_all(&path).unwrap();
        let err = open(repo.root(), &path, "c1").unwrap_err();
        let named = format!(
            "{} is in the way of the worktree of change c1: it is not a worktree on change/c1.",
            path.display()
        );
        assert!(err.starts_with(&named), "{err}");
        // A worktree on another branch is in the way too.
        fs::remove_dir(&path).unwrap();
        repo.git(&["branch", "change/c2"]);
        repo.tool().add_worktree(&path, "change/c2").unwrap();
        let err = open(repo.root(), &path, "c1").unwrap_err();
        assert!(err.starts_with(&named), "{err}");
        assert!(!listed(&repo, &path));
        let under = trees.0.join("c1/README.md/c1");
        let err = open(repo.root(), &under, "c1").unwrap_err();
        let start = format!("cannot look at {}: ", under.display());
        assert!(err.starts_with(&start), "{err}");
    }

    #[test]
    fn the_worktree_of_a_variant_is_on_its_branch_made_from_main_and_kept() {
        let (repo, trees) = with_branch();
        repo.identify();
        let path = trees.0.join("c1-v2");
        open_variant(repo.root(), &path, "c1", 2, "main").unwrap();
        assert_eq!(Git::new(&path).current_branch().unwrap(), "change/c1-v2");
        let main = repo.git(&["rev-parse", "main"]);
        assert_eq!(Git::new(&path).head().unwrap(), main);
        // Its branch moves on and stays: gone, the worktree is made again on it.
        fs::write(path.join("work.txt"), "variant\n").unwrap();
        Git::new(&path).commit_all("variant work").unwrap();
        let moved = repo.git(&["rev-parse", "change/c1-v2"]);
        fs::remove_dir_all(&path).unwrap();
        open_variant(repo.root(), &path, "c1", 2, "main").unwrap();
        assert_eq!(Git::new(&path).head().unwrap(), moved);
        // What is in the way is refused by name.
        let other = trees.0.join("other");
        fs::create_dir_all(&other).unwrap();
        let err = open_variant(repo.root(), &other, "c1", 3, "main").unwrap_err();
        let named = format!(
            "{} is in the way of the worktree of variant 3 of c1: it is not a worktree on \
             change/c1-v3.",
            other.display()
        );
        assert!(err.starts_with(&named), "{err}");
        let err = open_variant(repo.root(), &trees.0.join("c1-v4"), "c1", 4, "trunk");
        assert!(err.unwrap_err().starts_with("git branch "));
    }

    #[test]
    fn a_removed_worktree_goes_with_its_work_and_git_forgets_it() {
        let (repo, trees) = with_branch();
        let path = trees.0.join("c1");
        open(repo.root(), &path, "c1").unwrap();
        fs::write(path.join("loose.txt"), "x\n").unwrap();
        remove(repo.root(), &path).unwrap();
        assert!(fs::symlink_metadata(&path).is_err());
        assert_eq!(repo.tool().worktrees().unwrap().len(), 1);
        // Nothing there: git only forgets what went away.
        open(repo.root(), &path, "c1").unwrap();
        fs::remove_dir_all(&path).unwrap();
        remove(repo.root(), &path).unwrap();
        assert_eq!(repo.tool().worktrees().unwrap().len(), 1);
        // A folder that is not a worktree is not removed.
        fs::create_dir_all(path.join("mine")).unwrap();
        let err = remove(repo.root(), &path).unwrap_err();
        assert!(err.starts_with("git worktree remove --force -- "), "{err}");
        assert!(path.join("mine").is_dir());
        let under = path.join("mine/x/c1");
        fs::write(path.join("mine/x"), "").unwrap();
        let err = remove(repo.root(), &under).unwrap_err();
        let start = format!("cannot look at {}: ", under.display());
        assert!(err.starts_with(&start), "{err}");
        let link = trees.0.join("link");
        symlink(&path, &link).unwrap();
        let err = remove(repo.root(), &link).unwrap_err();
        let refused = format!("{} is a link: ", link.display());
        assert!(err.starts_with(&refused), "{err}");
        assert!(path.join("mine").is_dir());
    }
}
