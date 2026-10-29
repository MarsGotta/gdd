//! The project's git repository: which paths a change touched, what its branch holds, the
//! branch a new change gets, the commits and merge that close it, the history and tags of the
//! main branch, and the repository and first commit of a new project.
//!
//! Git always runs as `git` with a fixed list of arguments, in the project root, never through
//! a shell. A name read from a file never reaches git as an option, and a failure of git is an
//! [`Error`], never an empty answer. The tool never sets who commits: git takes the author from
//! the repository's configuration and the environment it runs in.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fmt;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Local state of the tool: it never counts as part of a change.
const LOCAL_STATE: &str = ".gdd/";

/// The folder of the tool's local state, from the project root.
const STATE_FOLDER: &str = ".gdd";

/// A pathspec for everything but the tool's local state.
const WITHOUT_STATE: &str = ":(exclude).gdd";

/// The mode git gives a symbolic link in a tree.
const LINK_MODE: &str = "120000";

/// What can go wrong when reading the repository.
#[derive(Debug)]
pub enum Error {
    /// Git could not be started.
    Start(std::io::Error),
    /// Git ran and failed: the command, how it ended and what it said.
    Failed(String),
    /// A name git would read as an option, not as a revision.
    Revision(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Start(e) => write!(f, "cannot run git: {e}"),
            Error::Failed(message) => f.write_str(message),
            Error::Revision(name) => write!(f, "{name:?} is not a revision git can take"),
        }
    }
}

impl std::error::Error for Error {}

/// Whether the project at `root` is inside a git repository: `root` or a folder above it has
/// `.git`.
pub fn is_repository(root: &Path) -> bool {
    root.ancestors().any(|folder| folder.join(".git").exists())
}

/// Where git keeps the repository of the working tree the project is in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dirs {
    /// What every working tree of the repository shares (`--git-common-dir`), absolute.
    pub common: PathBuf,
    /// What this working tree keeps of its own (`--git-dir`), absolute: the same folder in the
    /// main checkout.
    pub own: PathBuf,
    /// The project root from the top of its working tree (`--show-prefix`): empty at the top.
    pub prefix: PathBuf,
}

/// The first working tree git lists for a repository: its main checkout, or the repository
/// itself when it is bare.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MainWorktree {
    /// Where it is, absolute.
    pub path: PathBuf,
    /// Whether the repository is bare: then `path` is the repository, not a checkout.
    pub bare: bool,
}

/// A commit as [`Git::first_parents`] reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Logged {
    /// Its full id.
    pub commit: String,
    /// The day it was committed: `YYYY-MM-DD`.
    pub day: String,
    /// The values of each trailer asked for, in the order asked: each in the order of the
    /// message.
    pub trailers: Vec<Vec<String>>,
}

/// The git repository of a project, read from the project root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Git {
    root: PathBuf,
    /// The whole environment git runs in; `None`: the one of the process.
    env: Option<Vec<(String, String)>>,
}

impl Git {
    /// Git for the project at `root`. The paths it gives are relative to `root`, and paths
    /// outside it are left out.
    pub fn new(root: &Path) -> Git {
        Git {
            root: root.to_path_buf(),
            env: None,
        }
    }

    /// Git for the project at `root` (see [`Git::new`]), run with `env` as its whole
    /// environment: the one gdd runs in, which also says who commits.
    pub fn in_env(root: &Path, env: &[(String, String)]) -> Git {
        Git {
            root: root.to_path_buf(),
            env: Some(env.to_vec()),
        }
    }

    /// Makes the project folder a git repository whose first branch is `branch`.
    pub fn init(&self, branch: &str) -> Result<(), Error> {
        self.read(&["init", "-q", "-b", revision(branch)?])?;
        Ok(())
    }

    /// The branch checked out, even before its first commit; an error when none is.
    pub fn checked_out(&self) -> Result<String, Error> {
        self.line(&["symbolic-ref", "--short", "HEAD"])
    }

    /// Where git keeps the repository of the working tree the project is in (see [`Dirs`]).
    pub fn dirs(&self) -> Result<Dirs, Error> {
        let asked = [
            "rev-parse",
            "--path-format=absolute",
            "--git-common-dir",
            "--git-dir",
            "--show-prefix",
        ];
        let out = self.read(&asked)?;
        let mut lines = out.split(|&byte| byte == b'\n');
        // One line each, in the order asked; components drop the slash a prefix ends with.
        let mut next = || {
            let line = OsStr::from_bytes(lines.next().unwrap_or_default());
            Path::new(line).components().collect::<PathBuf>()
        };
        Ok(Dirs {
            common: next(),
            own: next(),
            prefix: next(),
        })
    }

    /// The main working tree of the repository (see [`MainWorktree`]): the first entry of
    /// `git worktree list`, whatever working tree the project is in.
    pub fn main_worktree(&self) -> Result<MainWorktree, Error> {
        let args = ["worktree", "list", "--porcelain", "-z"];
        let out = self.read(&args)?;
        // One attribute per NUL byte, "worktree <path>" first; an empty one ends the entry.
        let mut first = out
            .split(|&byte| byte == 0)
            .take_while(|line| !line.is_empty());
        let path = first
            .next()
            .and_then(|line| line.strip_prefix(b"worktree "));
        let Some(path) = path else {
            let listed = String::from_utf8_lossy(&out).into_owned();
            return Err(Error::Failed(format!(
                "git {} listed no working tree first: {listed:?}",
                args.join(" ")
            )));
        };
        Ok(MainWorktree {
            path: PathBuf::from(OsStr::from_bytes(path)),
            bare: first.any(|line| line == b"bare"),
        })
    }

    /// The working trees of the repository with a branch checked out, as `git worktree list`
    /// names them: where each is, absolute, and its branch.
    pub fn worktrees(&self) -> Result<Vec<(PathBuf, String)>, Error> {
        let out = self.read(&["worktree", "list", "--porcelain", "-z"])?;
        let mut listed = Vec::new();
        let mut path = None;
        // One attribute per NUL byte: "worktree <path>" opens an entry, "branch <ref>" names its
        // branch; a detached or bare one has none.
        for attribute in out.split(|&byte| byte == 0) {
            if let Some(at) = attribute.strip_prefix(b"worktree ") {
                path = Some(PathBuf::from(OsStr::from_bytes(at)));
            } else if let Some(name) = attribute.strip_prefix(b"branch refs/heads/")
                && let Some(path) = path.take()
            {
                listed.push((path, String::from_utf8_lossy(name).into_owned()));
            }
        }
        Ok(listed)
    }

    /// Adds the working tree `path` to the repository, with the branch `branch` checked out
    /// there; git makes the folders on the way.
    pub fn add_worktree(&self, path: &Path, branch: &str) -> Result<(), Error> {
        let branch = OsStr::new(revision(branch)?);
        let args = ["worktree", "add", "-q", "--"].map(OsStr::new);
        self.read(&[&args[..], &[path.as_os_str(), branch]].concat())?;
        Ok(())
    }

    /// Removes the working tree `path` of the repository, with what it has that git does not.
    pub fn remove_worktree(&self, path: &Path) -> Result<(), Error> {
        let args = ["worktree", "remove", "--force", "--"].map(OsStr::new);
        self.read(&[&args[..], &[path.as_os_str()]].concat())?;
        Ok(())
    }

    /// Forgets the working trees of the repository whose folders are gone.
    pub fn prune_worktrees(&self) -> Result<(), Error> {
        self.read(&["worktree", "prune"])?;
        Ok(())
    }

    /// Commits the files `paths`, from the project root, and nothing else of the working tree,
    /// with `message`; the new ones are added first.
    pub fn commit_files(&self, paths: &[String], message: &str) -> Result<(), Error> {
        let paths = paths.iter().map(String::as_str);
        let mut add = vec!["add", "--"];
        add.extend(paths.clone());
        self.read(&add)?;
        let mut commit = vec!["commit", "-q", "-m", message, "--"];
        commit.extend(paths);
        self.read(&commit)?;
        Ok(())
    }

    /// The current branch; `HEAD` when no branch is checked out.
    pub fn current_branch(&self) -> Result<String, Error> {
        self.line(&["rev-parse", "--abbrev-ref", "HEAD"])
    }

    /// Whether `name` names a commit: a branch, a tag or a commit id.
    pub fn exists(&self, name: &str) -> Result<bool, Error> {
        let args = ["rev-parse", "--verify", "--quiet", revision(name)?];
        let out = self.run(&args)?;
        match out.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(failed(&args, &out)),
        }
    }

    /// Paths the change on `branch` touched since the commit `since`: what the branch changed
    /// since then and, when it is the current branch, what the working tree changed or added.
    /// Sorted, without repeats and without the tool's local state (`.gdd/`).
    pub fn changed_since(&self, branch: &str, since: &str) -> Result<Vec<String>, Error> {
        let paths = if self.current_branch()? == branch {
            [self.diff(&[since])?, self.untracked()?].concat()
        } else {
            self.diff(&[since, branch])?
        };
        Ok(counted(paths))
    }

    /// Paths the change on `branch` touched against the main branch `main`: what the branch
    /// changed since it left `main` and, when it is the current branch, what the working tree
    /// changed or added. Nothing while the branch does not exist. Sorted, without repeats and
    /// without the tool's local state (`.gdd/`).
    pub fn changed_from(&self, main: &str, branch: &str) -> Result<Vec<String>, Error> {
        if !self.exists(branch)? {
            return Ok(Vec::new());
        }
        let mut paths = self.diff(&[&format!("{}...{branch}", revision(main)?)])?;
        if self.current_branch()? == branch {
            paths.extend(self.diff(&["HEAD"])?);
            paths.extend(self.untracked()?);
        }
        Ok(counted(paths))
    }

    /// Paths the commits of `head` changed since it left `base`, and nothing else: what CI sees
    /// of a branch, in git's order (sorted, each once). What the commits hold under `.gdd/`
    /// counts too: it is no longer local state once it is committed.
    pub fn committed_from(&self, base: &str, head: &str) -> Result<Vec<String>, Error> {
        let range = format!("{}...{}", revision(base)?, revision(head)?);
        self.diff(&[&range])
    }

    /// Whether the commits of `head` changed anything since it left `base`, anywhere in the
    /// repository: also outside the project folder, which [`Git::committed_from`] leaves out,
    /// whatever the repository's configuration says.
    pub fn changed_anywhere(&self, base: &str, head: &str) -> Result<bool, Error> {
        let range = format!("{}...{}", revision(base)?, revision(head)?);
        let args = ["diff", "--name-only", "-z", "--no-relative", &range, "--"];
        Ok(!self.paths(&args)?.is_empty())
    }

    /// The file `path`, from the project root, as it was at the commit `commit`: `None` when
    /// there was no file there.
    pub fn file_at(&self, commit: &str, path: &str) -> Result<Option<Vec<u8>>, Error> {
        self.blob_at(commit, path, true)
    }

    /// The file `path`, from the project root, as it was at the commit `commit`, when it was a
    /// file of its own there: `None` when there was none, or a symbolic link.
    pub fn regular_file_at(&self, commit: &str, path: &str) -> Result<Option<Vec<u8>>, Error> {
        self.blob_at(commit, path, false)
    }

    /// What git keeps of the file `path`, from the project root, at the commit `commit`: `None`
    /// when there was no file there, or a symbolic link and not `links`.
    fn blob_at(&self, commit: &str, path: &str, links: bool) -> Result<Option<Vec<u8>>, Error> {
        let listing = self.read(&["ls-tree", "-z", revision(commit)?, "--", path])?;
        let listing = String::from_utf8_lossy(&listing);
        // One entry per NUL byte: "<mode> <type> <object>\t<path>".
        let object = listing.split('\0').find_map(|entry| {
            let (meta, name) = entry.split_once('\t')?;
            let mut fields = meta.split(' ');
            let (mode, kind, object) = (fields.next()?, fields.next()?, fields.next()?);
            let kept = links || mode != LINK_MODE;
            (name == path && kind == "blob" && kept).then_some(object)
        });
        match object {
            Some(object) => self.read(&["cat-file", "blob", object]).map(Some),
            None => Ok(None),
        }
    }

    /// The last commit of the one checked out that touched the file `path`, from the project
    /// root, as its full id: `None` when none did.
    pub fn last_commit_of(&self, path: &str) -> Result<Option<String>, Error> {
        let commit = self.line(&["log", "-1", "--format=%H", "HEAD", "--", path])?;
        Ok(Some(commit).filter(|commit| !commit.is_empty()))
    }

    /// Merges into the file `ours` what changed from the file `base` to the file `theirs`, each
    /// from the project root or absolute, as `git merge-file` does, the three named in the
    /// conflict marks by `labels` in that order: the number of conflicts, each marked in `ours`.
    /// A failure of git is an error.
    pub fn merge_file(
        &self,
        ours: &Path,
        base: &Path,
        theirs: &Path,
        labels: [&str; 3],
    ) -> Result<usize, Error> {
        let [yours, old, new] = labels.map(OsStr::new);
        let label = OsStr::new("-L");
        let (merge, files) = (OsStr::new("merge-file"), OsStr::new("--"));
        let paths = [ours, base, theirs].map(Path::as_os_str);
        let options = [merge, label, yours, label, old, label, new, files];
        let args = [&options[..], &paths[..]].concat();
        let out = self.run(&args)?;
        // Git exits with the number of conflicts, 127 at most; on a failure, with a negative
        // code (255) or 128.
        match out.status.code().map(usize::try_from) {
            Some(Ok(conflicts)) if conflicts <= 127 => Ok(conflicts),
            _ => Err(failed(&args, &out)),
        }
    }

    /// The files inside `folder`, from the project root, at the commit `commit`: their paths
    /// from the project root, in git's order.
    pub fn files_at(&self, commit: &str, folder: &str) -> Result<Vec<String>, Error> {
        let at = revision(commit)?;
        self.paths(&["ls-tree", "-r", "-z", "--name-only", at, "--", folder])
    }

    /// Creates the branch `name` at the commit `from`, without checking it out.
    pub fn create_branch(&self, name: &str, from: &str) -> Result<(), Error> {
        self.read(&["branch", revision(name)?, revision(from)?])?;
        Ok(())
    }

    /// The commit checked out, as its full id.
    pub fn head(&self) -> Result<String, Error> {
        self.line(&["rev-parse", "HEAD"])
    }

    /// The commit where `branch` left `main`: their best common ancestor, as its full id.
    pub fn merge_base(&self, main: &str, branch: &str) -> Result<String, Error> {
        self.line(&["merge-base", revision(main)?, revision(branch)?])
    }

    /// The paths a merge in progress left in conflict, from the top of the working tree: git's
    /// order, each once.
    pub fn conflicts(&self) -> Result<Vec<String>, Error> {
        self.paths(&["diff", "--name-only", "--diff-filter=U", "-z"])
    }

    /// Whether the working tree has something to commit: a file changed, deleted or added that
    /// git does not ignore.
    pub fn has_changes(&self) -> Result<bool, Error> {
        Ok(!self.read(&["status", "--porcelain"])?.is_empty())
    }

    /// Whether a file git tracks changed or went away in the working tree; files git does not
    /// track yet do not count.
    pub fn has_tracked_changes(&self) -> Result<bool, Error> {
        let status = self.read(&["status", "--porcelain", "--untracked-files=no"])?;
        Ok(!status.is_empty())
    }

    /// Whether the index holds the file `path`, from the project root, otherwise than the commit
    /// checked out: what was added to it, or taken out of it, and not committed yet. The working
    /// tree does not count.
    pub fn has_staged(&self, path: &str) -> Result<bool, Error> {
        let args = ["diff", "--cached", "--name-only", "-z", "--", path];
        Ok(!self.paths(&args)?.is_empty())
    }

    /// Takes the files `paths`, from the project root, back out of the index: it holds them again
    /// as the commit checked out does, or not at all when it has none. The working tree does not
    /// change.
    pub fn unstage(&self, paths: &[String]) -> Result<(), Error> {
        let mut args = vec!["reset", "-q", "--"];
        args.extend(paths.iter().map(String::as_str));
        self.read(&args)?;
        Ok(())
    }

    /// Commits, on the current branch, everything the working tree changed except what git
    /// ignores, with `message`.
    pub fn commit_all(&self, message: &str) -> Result<(), Error> {
        self.read(&["add", "-A"])?;
        self.read(&["commit", "-q", "-m", message])?;
        Ok(())
    }

    /// Commits, on the current branch, everything the working tree changed except what git
    /// ignores, with `message`, as [`Git::commit_all`] does; with nothing changed, the commit is
    /// empty.
    pub fn commit_all_even_empty(&self, message: &str) -> Result<(), Error> {
        self.read(&["add", "-A"])?;
        self.read(&["commit", "-q", "--allow-empty", "-m", message])?;
        Ok(())
    }

    /// Whether the project has work to commit: a file changed, deleted or added that git does
    /// not ignore, outside the tool's local state (`.gdd/`).
    pub fn has_work(&self) -> Result<bool, Error> {
        let status = self.read(&["status", "--porcelain", "--", ".", WITHOUT_STATE])?;
        Ok(!status.is_empty())
    }

    /// Commits, on the current branch, the work of the project (see [`Git::has_work`]) with
    /// `message`: never the tool's local state, which stays where it is whatever is checked out
    /// next.
    pub fn commit_work(&self, message: &str) -> Result<(), Error> {
        self.read(&["add", "-A", "--", "."])?;
        // The state goes back out of the commit. An excluding pathspec in `git add` would fail
        // where git ignores the state.
        self.read(&["reset", "-q", "--", STATE_FOLDER])?;
        self.read(&["commit", "-q", "-m", message])?;
        Ok(())
    }

    /// Checks out the branch `name`.
    pub fn checkout(&self, name: &str) -> Result<(), Error> {
        self.read(&["checkout", "-q", revision(name)?, "--"])?;
        Ok(())
    }

    /// Merges `branch` into the current branch with a merge commit whose message is `message`,
    /// even when the current branch could just move forward to it.
    pub fn merge(&self, branch: &str, message: &str) -> Result<(), Error> {
        let args = ["merge", "--no-ff", "--no-edit", "-q", "-m", message];
        self.read(&[&args[..], &[revision(branch)?]].concat())?;
        Ok(())
    }

    /// Merges `branch` into the current branch as [`Git::merge`] does, but stops before the merge
    /// commit: [`Git::commit_all`] makes it, with what the working tree has then.
    pub fn merge_uncommitted(&self, branch: &str) -> Result<(), Error> {
        let args = ["merge", "--no-ff", "--no-commit", "-q"];
        self.read(&[&args[..], &[revision(branch)?]].concat())?;
        Ok(())
    }

    /// Gives up the merge in progress: the current branch and its files go back to how they were
    /// before it.
    pub fn abort_merge(&self) -> Result<(), Error> {
        self.read(&["merge", "--abort"])?;
        Ok(())
    }

    /// Whether the repository has a remote.
    pub fn has_remote(&self) -> Result<bool, Error> {
        Ok(!self.read(&["remote"])?.trim_ascii().is_empty())
    }

    /// Pushes `name`, a branch or a full ref such as `refs/tags/v1.0.0`, to the remote `origin`.
    pub fn push(&self, name: &str) -> Result<(), Error> {
        self.read(&["push", "-q", "origin", revision(name)?])?;
        Ok(())
    }

    /// Makes the annotated tag `name`, whose message is `message`, on the commit `at`. A tag of
    /// that name that exists already is an error: it is never moved.
    pub fn tag(&self, name: &str, message: &str, at: &str) -> Result<(), Error> {
        let args = ["tag", "-a", "-m", message, revision(name)?, revision(at)?];
        self.read(&args)?;
        Ok(())
    }

    /// The commits of `branch` along its first parents, newest first: on a main branch, the merge
    /// commits of the changes and what was committed on it directly. Each with the values of its
    /// trailers `keys` (names of letters, digits and hyphens), unfolded.
    pub fn first_parents(&self, branch: &str, keys: &[&str]) -> Result<Vec<Logged>, Error> {
        let trailers: String = keys
            .iter()
            .map(|key| format!("%x00%(trailers:key={key},valueonly,unfold,separator=%x1f)"))
            .collect();
        let format = format!("--format=%H%x00%cs{trailers}");
        let at = revision(branch)?;
        let out = self.read(&["log", "-z", "--first-parent", &format, at, "--"])?;
        let text = String::from_utf8_lossy(&out);
        // One field per NUL byte, each commit's in the order asked; each trailer's values, one
        // per unit separator.
        let fields: Vec<&str> = text.split('\0').collect();
        let values = |field: &str| -> Vec<String> {
            let values = field.split('\u{1f}').filter(|value| !value.is_empty());
            values.map(str::to_string).collect()
        };
        let logged = |commit: &[&str]| Logged {
            commit: commit[0].to_string(),
            day: commit[1].to_string(),
            trailers: commit[2..].iter().copied().map(&values).collect(),
        };
        Ok(fields.chunks_exact(2 + keys.len()).map(logged).collect())
    }

    /// The tags of the repository, by name, each with the commit it names: for an annotated tag,
    /// the commit the tag points to.
    pub fn tags(&self) -> Result<Vec<(String, String)>, Error> {
        let format = "--format=%(refname:strip=2)%00%(objectname)%00%(*objectname)";
        let out = self.read(&["for-each-ref", format, "refs/tags"])?;
        let text = String::from_utf8_lossy(&out);
        // One tag per line: its name, its object and, for an annotated tag, what it points to.
        let tag = |line: &str| {
            let mut fields = line.split('\0');
            let (name, object) = (fields.next()?, fields.next()?);
            let commit = fields.next().filter(|peeled| !peeled.is_empty());
            Some((name.to_string(), commit.unwrap_or(object).to_string()))
        };
        Ok(text.lines().filter_map(tag).collect())
    }

    /// The one line `git args` printed, without the line break.
    fn line(&self, args: &[&str]) -> Result<String, Error> {
        let out = self.read(args)?;
        Ok(String::from_utf8_lossy(&out).trim().to_string())
    }

    /// Paths that differ between two revisions, or between one and the working tree. Both
    /// sides of a rename count.
    fn diff(&self, revisions: &[&str]) -> Result<Vec<String>, Error> {
        let mut args = vec!["diff", "--name-only", "-z", "--no-renames", "--relative"];
        for &name in revisions {
            args.push(revision(name)?);
        }
        args.push("--");
        self.paths(&args)
    }

    /// Files of the working tree that git does not track yet; ignored files are left out.
    fn untracked(&self) -> Result<Vec<String>, Error> {
        self.paths(&["ls-files", "--others", "--exclude-standard", "-z"])
    }

    /// The paths `git args` printed, one per NUL byte (`-z`): verbatim, never quoted.
    fn paths(&self, args: &[&str]) -> Result<Vec<String>, Error> {
        let out = self.read(args)?;
        Ok(out
            .split(|&byte| byte == 0)
            .filter(|path| !path.is_empty())
            .map(|path| String::from_utf8_lossy(path).into_owned())
            .collect())
    }

    /// What `git args` printed; a failure is an error.
    fn read<S: AsRef<OsStr>>(&self, args: &[S]) -> Result<Vec<u8>, Error> {
        let out = self.run(args)?;
        if out.status.success() {
            Ok(out.stdout)
        } else {
            Err(failed(args, &out))
        }
    }

    fn run<S: AsRef<OsStr>>(&self, args: &[S]) -> Result<Output, Error> {
        let mut git = Command::new("git");
        git.args(args).current_dir(&self.root);
        if let Some(env) = &self.env {
            let vars = env.iter().map(|(name, value)| (name, value));
            git.env_clear().envs(vars);
        }
        git.output().map_err(Error::Start)
    }
}

/// The error of a git command that ran and failed.
fn failed<S: AsRef<OsStr>>(args: &[S], out: &Output) -> Error {
    let lossy = |arg: &S| arg.as_ref().to_string_lossy().into_owned();
    let shown: Vec<String> = args.iter().map(lossy).collect();
    Error::Failed(format!(
        "git {} failed ({}): {}",
        shown.join(" "),
        out.status,
        String::from_utf8_lossy(&out.stderr).trim()
    ))
}

/// Sorted, without repeats and without the tool's local state.
fn counted(paths: Vec<String>) -> Vec<String> {
    let paths: BTreeSet<String> = paths
        .into_iter()
        .filter(|path| !path.starts_with(LOCAL_STATE))
        .collect();
    paths.into_iter().collect()
}

/// `name` as an argument for git: an empty name, or one git would read as an option, is an
/// error.
fn revision(name: &str) -> Result<&str, Error> {
    if name.is_empty() || name.starts_with('-') {
        Err(Error::Revision(name.to_string()))
    } else {
        Ok(name)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A folder in the temporary folder, removed when dropped.
    pub(crate) struct Folder(pub(crate) PathBuf);

    impl Folder {
        pub(crate) fn new() -> Folder {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "gdd-core-test-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Folder(path)
        }

        /// Writes `text` to the file `path`, creating its folders.
        pub(crate) fn write(&self, path: &str, text: &str) {
            let path = self.0.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
    }

    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Git settings of the tests: a fixed author, and no hooks or signing from the user's config.
    const CONFIG: [&str; 8] = [
        "-c",
        "user.name=GDD Test",
        "-c",
        "user.email=test@example.com",
        "-c",
        "commit.gpgsign=false",
        "-c",
        "core.hooksPath=/dev/null",
    ];

    /// A git repository in a temporary folder: `main` with one commit.
    pub(crate) struct Repo(pub(crate) Folder);

    impl Repo {
        pub(crate) fn new() -> Repo {
            let repo = Repo(Folder::new());
            repo.git(&["init", "-q", "-b", "main"]);
            repo.write("README.md", "demo\n");
            repo.commit();
            repo
        }

        pub(crate) fn root(&self) -> &Path {
            &self.0.0
        }

        /// Runs git with a fixed author, and without hooks or signing from the user's config.
        pub(crate) fn git(&self, args: &[&str]) -> String {
            let out = Command::new("git")
                .args(CONFIG)
                .args(args)
                .current_dir(self.root())
                .output()
                .unwrap();
            let stderr = String::from_utf8_lossy(&out.stderr);
            assert!(out.status.success(), "git {args:?}: {stderr}");
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        }

        pub(crate) fn write(&self, path: &str, text: &str) {
            self.0.write(path, text);
        }

        /// Commits the whole working tree.
        pub(crate) fn commit(&self) {
            self.git(&["add", "-A"]);
            self.git(&["commit", "-q", "--allow-empty", "-m", "commit"]);
        }

        /// Starts change `id`: its branch from here, checked out. Returns the commit it starts at.
        pub(crate) fn start(&self, id: &str) -> String {
            self.git(&["checkout", "-q", "-b", &format!("change/{id}")]);
            self.git(&["rev-parse", "HEAD"])
        }

        /// Gives the repository an author of its own, without signing or hooks: the tool
        /// commits as the repository's configuration says.
        pub(crate) fn identify(&self) {
            for (key, value) in [
                ("user.name", "GDD Test"),
                ("user.email", "test@example.com"),
                ("commit.gpgsign", "false"),
                ("tag.gpgsign", "false"),
                ("core.hooksPath", "/dev/null"),
            ] {
                self.git(&["config", "--local", key, value]);
            }
        }

        pub(crate) fn tool(&self) -> Git {
            Git::new(self.root())
        }
    }

    const NOTHING: [&str; 0] = [];

    #[test]
    fn a_folder_is_in_a_repository_when_it_or_one_above_it_has_git() {
        let plain = Folder::new();
        assert!(!is_repository(&plain.0));
        let repo = Repo::new();
        repo.write("app/gdd/project.toml", "");
        assert!(is_repository(repo.root()));
        assert!(is_repository(&repo.root().join("app")));
    }

    #[test]
    fn where_git_keeps_the_repository_of_a_checkout_and_of_a_linked_worktree() {
        let repo = Repo::new();
        repo.write("app/gdd/project.toml", "");
        let top = fs::canonicalize(repo.root()).unwrap();
        let git = top.join(".git");
        let checkout = Dirs {
            common: git.clone(),
            own: git.clone(),
            prefix: PathBuf::new(),
        };
        assert_eq!(repo.tool().dirs().unwrap(), checkout);
        let app = Git::new(&repo.root().join("app")).dirs().unwrap();
        assert_eq!(app.prefix, Path::new("app"));
        let linked = Folder::new();
        let worktree = linked.0.join("c1");
        let path = worktree.to_string_lossy().into_owned();
        repo.git(&["worktree", "add", "-q", "-b", "change/c1", &path]);
        fs::create_dir_all(worktree.join("app")).unwrap();
        let dirs = Git::new(&worktree.join("app")).dirs().unwrap();
        let own = git.join("worktrees/c1");
        assert_eq!((dirs.common, dirs.own), (git, own));
        assert_eq!(dirs.prefix, Path::new("app"));
        let plain = Folder::new();
        let err = Git::new(&plain.0).dirs().unwrap_err().to_string();
        let start = "git rev-parse --path-format=absolute ";
        assert!(err.starts_with(start), "{err}");
    }

    #[test]
    fn the_main_worktree_is_the_first_one_git_lists() {
        let repo = Repo::new();
        let main = MainWorktree {
            path: fs::canonicalize(repo.root()).unwrap(),
            bare: false,
        };
        assert_eq!(repo.tool().main_worktree().unwrap(), main);
        let linked = Folder::new();
        let worktree = linked.0.join("c \"1\"");
        let path = worktree.to_string_lossy().into_owned();
        repo.git(&["worktree", "add", "-q", "-b", "change/c1", &path]);
        assert_eq!(Git::new(&worktree).main_worktree().unwrap(), main);
        let bare = Folder::new();
        let path = bare.0.join("repo.git").to_string_lossy().into_owned();
        repo.git(&["clone", "-q", "--bare", ".", &path]);
        let repository = fs::canonicalize(&path).unwrap();
        let listed = Git::new(&repository).main_worktree().unwrap();
        let bare = MainWorktree {
            path: repository,
            bare: true,
        };
        assert_eq!(listed, bare);
        let plain = Folder::new();
        let err = Git::new(&plain.0).main_worktree().unwrap_err().to_string();
        let failed = "git worktree list --porcelain -z failed";
        assert!(err.starts_with(failed), "{err}");
    }

    #[test]
    fn worktrees_are_added_listed_with_their_branch_removed_and_pruned() {
        let repo = Repo::new();
        repo.git(&["branch", "change/c1"]);
        let git = repo.tool();
        let main = (fs::canonicalize(repo.root()).unwrap(), "main".to_string());
        let only_main = std::slice::from_ref(&main);
        assert_eq!(git.worktrees().unwrap(), only_main);
        let linked = Folder::new();
        let worktree = linked.0.join("deep/c \"1\"");
        git.add_worktree(&worktree, "change/c1").unwrap();
        assert!(worktree.join("README.md").is_file());
        let path = fs::canonicalize(&worktree).unwrap();
        let added = (path, "change/c1".to_string());
        assert_eq!(git.worktrees().unwrap(), [main.clone(), added]);
        // A detached one has no branch to name.
        let detached = linked.0.join("detached").to_string_lossy().into_owned();
        repo.git(&["worktree", "add", "-q", "--detach", &detached]);
        assert_eq!(git.worktrees().unwrap().len(), 2);
        fs::write(worktree.join("loose.txt"), "x\n").unwrap();
        git.remove_worktree(&worktree).unwrap();
        assert!(!worktree.exists());
        assert_eq!(git.worktrees().unwrap(), only_main);
        // A folder that went away leaves its worktree listed until it is pruned.
        git.add_worktree(&worktree, "change/c1").unwrap();
        fs::remove_dir_all(&worktree).unwrap();
        assert_eq!(git.worktrees().unwrap().len(), 2);
        git.prune_worktrees().unwrap();
        assert_eq!(git.worktrees().unwrap(), [main]);
    }

    #[test]
    fn what_git_refuses_of_a_worktree_is_an_error() {
        let repo = Repo::new();
        let git = repo.tool();
        let linked = Folder::new();
        let worktree = linked.0.join("c1");
        let err = git.add_worktree(&worktree, "change/c9").unwrap_err();
        let shown = worktree.display();
        let start = format!("git worktree add -q -- {shown} change/c9 failed");
        assert!(err.to_string().starts_with(&start), "{err}");
        let option = git.add_worktree(&worktree, "-x");
        assert!(matches!(option, Err(Error::Revision(n)) if n == "-x"));
        let err = git.remove_worktree(&linked.0).unwrap_err().to_string();
        assert!(err.starts_with("git worktree remove --force -- "), "{err}");
        let plain = Folder::new();
        assert!(Git::new(&plain.0).worktrees().is_err());
        assert!(Git::new(&plain.0).prune_worktrees().is_err());
    }

    #[test]
    fn the_current_branch_and_the_names_that_exist() {
        let repo = Repo::new();
        let git = repo.tool();
        assert_eq!(git.current_branch().unwrap(), "main");
        assert!(git.exists("main").unwrap());
        assert!(!git.exists("change/c1").unwrap());
        repo.start("c1");
        assert_eq!(git.current_branch().unwrap(), "change/c1");
        assert!(git.exists("change/c1").unwrap());
    }

    #[test]
    fn on_its_branch_a_change_touched_its_commits_and_its_working_tree() {
        let repo = Repo::new();
        repo.write("gdd/rules.toml", "a\n");
        repo.write("src/old.rs", "old\n");
        repo.commit();
        let start = repo.start("c1");
        let git = repo.tool();
        assert_eq!(git.changed_since("change/c1", &start).unwrap(), NOTHING);
        repo.write("src/committed.rs", "x\n");
        repo.commit();
        repo.write("gdd/rules.toml", "b\n");
        repo.git(&["rm", "-q", "src/old.rs"]);
        repo.write("src/new.rs", "new\n");
        repo.write(".gdd/changes/c1/change.json", "{}");
        assert_eq!(
            git.changed_since("change/c1", &start).unwrap(),
            [
                "gdd/rules.toml",
                "src/committed.rs",
                "src/new.rs",
                "src/old.rs",
            ]
        );
    }

    #[test]
    fn away_from_its_branch_only_the_commits_of_a_change_count() {
        let repo = Repo::new();
        let start = repo.start("c1");
        repo.write("src/committed.rs", "x\n");
        repo.commit();
        repo.git(&["checkout", "-q", "main"]);
        repo.write("src/loose.rs", "y\n");
        repo.write("README.md", "changed\n");
        assert_eq!(
            repo.tool().changed_since("change/c1", &start).unwrap(),
            ["src/committed.rs"]
        );
    }

    #[test]
    fn against_main_a_change_touched_what_its_branch_changed() {
        let repo = Repo::new();
        let git = repo.tool();
        assert_eq!(git.changed_from("main", "change/c1").unwrap(), NOTHING);
        repo.start("c1");
        assert_eq!(git.changed_from("main", "change/c1").unwrap(), NOTHING);
        repo.write("src/committed.rs", "x\n");
        repo.commit();
        repo.write("README.md", "changed\n");
        repo.write("src/new.rs", "y\n");
        repo.write(".gdd/log.jsonl", "{}\n");
        assert_eq!(
            git.changed_from("main", "change/c1").unwrap(),
            ["README.md", "src/committed.rs", "src/new.rs"]
        );
        repo.git(&["checkout", "-q", "main"]);
        assert_eq!(
            git.changed_from("main", "change/c1").unwrap(),
            ["src/committed.rs"]
        );
    }

    #[test]
    fn what_a_branch_committed_since_it_left_its_base() {
        let repo = Repo::new();
        let git = repo.tool();
        repo.start("c1");
        assert_eq!(git.committed_from("main", "HEAD").unwrap(), NOTHING);
        repo.write("src/committed.rs", "x\n");
        repo.write(".gdd/bin/tool", "{}\n");
        repo.commit();
        repo.write("README.md", "changed\n");
        repo.write("src/new.rs", "y\n");
        repo.write(".gdd/log.jsonl", "{}\n");
        let committed = [".gdd/bin/tool", "src/committed.rs"];
        assert_eq!(git.committed_from("main", "HEAD").unwrap(), committed);
        // What main got after the branch left it is not the branch's.
        repo.git(&["checkout", "-q", "main"]);
        repo.write("main.rs", "m\n");
        repo.commit();
        let away = git.committed_from("main", "change/c1").unwrap();
        assert_eq!(away, committed);
        for (base, head) in [("-x", "HEAD"), ("main", "--all")] {
            let option = git.committed_from(base, head);
            assert!(matches!(option, Err(Error::Revision(_))), "{base} {head}");
        }
        let err = git.committed_from("trunk", "HEAD").unwrap_err();
        assert!(err.to_string().starts_with("git diff "), "{err}");
    }

    #[test]
    fn what_a_branch_changed_outside_the_project_folder_is_a_change_anywhere() {
        let repo = Repo::new();
        repo.write("app/gdd/rules.toml", "a\n");
        repo.commit();
        repo.start("c1");
        let git = Git::new(&repo.root().join("app"));
        assert!(!git.changed_anywhere("main", "HEAD").unwrap());
        repo.write("shared/lib.rs", "x\n");
        repo.commit();
        assert_eq!(git.committed_from("main", "HEAD").unwrap(), NOTHING);
        assert!(git.changed_anywhere("main", "HEAD").unwrap());
        assert!(!git.changed_anywhere("HEAD", "main").unwrap());
        // A configuration that keeps diffs to the folder git runs in changes nothing.
        repo.git(&["config", "--local", "diff.relative", "true"]);
        assert!(git.changed_anywhere("main", "HEAD").unwrap());
        for (base, head) in [("-x", "HEAD"), ("main", "--all")] {
            let option = git.changed_anywhere(base, head);
            assert!(matches!(option, Err(Error::Revision(_))), "{base} {head}");
        }
        let err = git.changed_anywhere("trunk", "HEAD").unwrap_err();
        assert!(err.to_string().starts_with("git diff "), "{err}");
    }

    #[test]
    fn a_rename_counts_both_paths() {
        let repo = Repo::new();
        repo.write("gdd/rules.toml", "the rules of the project\n");
        repo.commit();
        let start = repo.start("c1");
        repo.git(&["mv", "gdd/rules.toml", "rules.toml"]);
        repo.commit();
        assert_eq!(
            repo.tool().changed_since("change/c1", &start).unwrap(),
            ["gdd/rules.toml", "rules.toml"]
        );
        assert_eq!(
            repo.tool().changed_from("main", "change/c1").unwrap(),
            ["gdd/rules.toml", "rules.toml"]
        );
    }

    #[test]
    fn paths_come_verbatim_whatever_their_characters() {
        let repo = Repo::new();
        repo.write("docs/a \"b\".md", "x\n");
        repo.commit();
        let start = repo.start("c1");
        repo.write("docs/a \"b\".md", "y\n");
        repo.write("docs/é ñ.md", "x\n");
        assert_eq!(
            repo.tool().changed_since("change/c1", &start).unwrap(),
            ["docs/a \"b\".md", "docs/é ñ.md"]
        );
    }

    #[test]
    fn in_a_project_inside_a_subfolder_paths_are_relative_to_it() {
        let repo = Repo::new();
        repo.write("app/gdd/rules.toml", "a\n");
        repo.commit();
        let start = repo.start("c1");
        repo.write("app/gdd/rules.toml", "b\n");
        repo.write("app/src/new.rs", "x\n");
        repo.write("other/new.rs", "x\n");
        repo.write("README.md", "changed\n");
        let git = Git::new(&repo.root().join("app"));
        assert_eq!(
            git.changed_since("change/c1", &start).unwrap(),
            ["gdd/rules.toml", "src/new.rs"]
        );
        assert_eq!(
            git.changed_from("main", "change/c1").unwrap(),
            ["gdd/rules.toml", "src/new.rs"]
        );
    }

    #[test]
    fn a_file_as_it_was_at_a_commit() {
        let repo = Repo::new();
        repo.write("app/gdd/rules.toml", "a\n");
        repo.write("app/docs/a \"b\".md", "x\n");
        repo.commit();
        let start = repo.start("c1");
        repo.write("app/gdd/rules.toml", "b\n");
        repo.git(&["rm", "-q", "app/docs/a \"b\".md"]);
        repo.commit();
        let git = Git::new(&repo.root().join("app"));
        assert_eq!(
            git.file_at(&start, "gdd/rules.toml").unwrap(),
            Some(b"a\n".to_vec())
        );
        let quoted = git.file_at(&start, "docs/a \"b\".md").unwrap();
        assert_eq!(quoted, Some(b"x\n".to_vec()));
        for nothing in ["gdd/missing.toml", "gdd", "gdd/rules.toml/x", "rules.toml"] {
            assert_eq!(git.file_at(&start, nothing).unwrap(), None, "{nothing:?}");
        }
        assert_eq!(git.file_at("HEAD", "docs/a \"b\".md").unwrap(), None);
        let err = git.file_at("0123abcd", "gdd/rules.toml").unwrap_err();
        assert!(err.to_string().starts_with("git ls-tree "), "{err}");
        let option = git.file_at("-x", "gdd/rules.toml");
        assert!(matches!(option, Err(Error::Revision(n)) if n == "-x"));
    }

    #[test]
    fn a_file_of_its_own_at_a_commit_is_not_a_link() {
        let repo = Repo::new();
        repo.write("docs/a.md", "x\n");
        std::os::unix::fs::symlink("a.md", repo.root().join("docs/link.md")).unwrap();
        repo.commit();
        let git = repo.tool();
        let own = git.regular_file_at("HEAD", "docs/a.md").unwrap();
        assert_eq!(own, Some(b"x\n".to_vec()));
        assert_eq!(git.regular_file_at("HEAD", "docs/link.md").unwrap(), None);
        assert_eq!(git.regular_file_at("HEAD", "docs").unwrap(), None);
        // The link itself is what git keeps of it.
        let link = git.file_at("HEAD", "docs/link.md").unwrap();
        assert_eq!(link, Some(b"a.md".to_vec()));
        assert_eq!(LINK_MODE, "120000");
    }

    #[test]
    fn unstaged_files_go_back_out_of_the_index_and_stay_in_the_working_tree() {
        let repo = Repo::new();
        repo.write("README.md", "changed\n");
        repo.write("docs/new.md", "new\n");
        repo.git(&["add", "README.md", "docs/new.md"]);
        let git = repo.tool();
        let paths = ["README.md", "docs/new.md", "docs/never.md"].map(String::from);
        git.unstage(&paths).unwrap();
        assert!(!git.has_staged("README.md").unwrap());
        assert!(!git.has_staged("docs/new.md").unwrap());
        let status = repo.git(&["status", "--porcelain"]);
        assert_eq!(status, "M README.md\n?? docs/");
        let new = fs::read_to_string(repo.root().join("docs/new.md")).unwrap();
        assert_eq!(new, "new\n");
        let plain = Folder::new();
        assert!(Git::new(&plain.0).unstage(&paths).is_err());
    }

    #[test]
    fn the_last_commit_that_touched_a_file_on_the_branch_checked_out() {
        let repo = Repo::new();
        repo.write("app/gdd/generated.toml", "a\n");
        repo.commit();
        let first = repo.git(&["rev-parse", "HEAD"]);
        repo.write("app/other.txt", "x\n");
        repo.commit();
        let git = Git::new(&repo.root().join("app"));
        let generated = "gdd/generated.toml";
        assert_eq!(git.last_commit_of(generated).unwrap(), Some(first.clone()));
        assert_eq!(git.last_commit_of("gdd/none.toml").unwrap(), None);
        // What is not committed, or committed on another branch, does not count.
        repo.git(&["checkout", "-q", "-b", "other"]);
        repo.write("app/gdd/generated.toml", "b\n");
        repo.commit();
        let other = repo.git(&["rev-parse", "HEAD"]);
        repo.git(&["checkout", "-q", "main"]);
        repo.write("app/gdd/generated.toml", "c\n");
        assert_eq!(git.last_commit_of(generated).unwrap(), Some(first));
        repo.git(&["checkout", "-q", "-f", "other"]);
        assert_eq!(git.last_commit_of(generated).unwrap(), Some(other));
        let plain = Folder::new();
        let err = Git::new(&plain.0).last_commit_of(generated).unwrap_err();
        assert!(err.to_string().starts_with("git log -1 "), "{err}");
    }

    /// Merges the files `ours`, `base` and `theirs` of `folder`, without the user's configuration:
    /// the conflicts, and what `ours` holds then.
    fn merged(folder: &Folder, texts: [&str; 3]) -> (usize, String) {
        let names = ["ours", "base", "theirs"];
        for (name, text) in names.iter().zip(texts) {
            folder.write(name, text);
        }
        let [ours, base, theirs] = names.map(|name| folder.0.join(name));
        let git = Git::in_env(&folder.0, &env());
        let labels = ["yours", "gdd 0.0.0", "gdd 0.0.1"];
        let conflicts = git.merge_file(&ours, &base, &theirs, labels).unwrap();
        (conflicts, fs::read_to_string(&ours).unwrap())
    }

    #[test]
    fn three_files_merge_into_ours_with_their_conflicts_marked_and_counted() {
        let folder = Folder::new();
        let base = "a\nb\nc\nd\ne\n";
        let apart = merged(&folder, ["A\nb\nc\nd\ne\n", base, "a\nb\nc\nd\nE\n"]);
        assert_eq!(apart, (0, "A\nb\nc\nd\nE\n".to_string()));
        let one = merged(&folder, ["A\nb\nc\nd\ne\n", base, "Z\nb\nc\nd\ne\n"]);
        let marked = "<<<<<<< yours\nA\n=======\nZ\n>>>>>>> gdd 0.0.1\nb\nc\nd\ne\n";
        assert_eq!(one, (1, marked.to_string()));
        // Git joins conflicts three lines or fewer apart: these are four.
        let apart = "a\nb\nc\nd\ne\nf\n";
        let texts = ["A\nb\nc\nd\ne\nF\n", apart, "Z\nb\nc\nd\ne\nY\n"];
        let two = merged(&folder, texts);
        assert_eq!(two.0, 2);
        let last = "=======\nY\n>>>>>>> gdd 0.0.1\n";
        assert!(two.1.ends_with(last), "{}", two.1);
        // Against an empty base, whatever both have differently is in conflict.
        let empty = merged(&folder, ["mine\n", "", "gdd's\n"]);
        let all = "<<<<<<< yours\nmine\n=======\ngdd's\n>>>>>>> gdd 0.0.1\n";
        assert_eq!(empty, (1, all.to_string()));
    }

    #[test]
    fn a_merge_of_a_file_that_is_not_there_is_an_error() {
        let folder = Folder::new();
        folder.write("base", "a\n");
        folder.write("theirs", "b\n");
        let at = |name: &str| folder.0.join(name);
        let git = Git::in_env(&folder.0, &env());
        let labels = ["yours", "gdd 0.0.0", "gdd 0.0.1"];
        let err = git.merge_file(&at("none"), &at("base"), &at("theirs"), labels);
        let err = err.unwrap_err();
        assert!(matches!(err, Error::Failed(_)), "{err}");
        let start = "git merge-file -L yours -L gdd 0.0.0 -L gdd 0.0.1 -- ";
        assert!(err.to_string().starts_with(start), "{err}");
        assert!(!at("none").exists());
        let nowhere = [("PATH".to_string(), "/nonexistent".to_string())];
        let lost = Git::in_env(&folder.0, &nowhere);
        let err = lost.merge_file(&at("base"), &at("base"), &at("theirs"), labels);
        assert!(matches!(err, Err(Error::Start(_))));
    }

    #[test]
    fn the_files_of_a_folder_at_a_commit() {
        let repo = Repo::new();
        repo.write("app/features/a.feature", "a\n");
        repo.write("app/features/deep/b c.feature", "b\n");
        repo.write("app/features2/x.feature", "x\n");
        repo.commit();
        repo.start("c1");
        repo.write("app/features/new.feature", "n\n");
        repo.commit();
        repo.write("app/features/loose.feature", "l\n");
        repo.git(&["checkout", "-q", "main"]);
        let git = Git::new(&repo.root().join("app"));
        let main = ["features/a.feature", "features/deep/b c.feature"];
        assert_eq!(git.files_at("main", "features").unwrap(), main);
        assert_eq!(
            git.files_at("change/c1", "features").unwrap(),
            [
                "features/a.feature",
                "features/deep/b c.feature",
                "features/new.feature",
            ]
        );
        assert_eq!(git.files_at("main", "specs").unwrap(), NOTHING);
        let err = git.files_at("change/c9", "features").unwrap_err();
        assert!(err.to_string().starts_with("git ls-tree -r "), "{err}");
        let option = git.files_at("-x", "features");
        assert!(matches!(option, Err(Error::Revision(n)) if n == "-x"));
    }

    #[test]
    fn a_branch_is_created_where_it_is_asked_without_leaving_the_current_one() {
        let repo = Repo::new();
        let main = repo.git(&["rev-parse", "main"]);
        repo.git(&["checkout", "-q", "-b", "other"]);
        repo.write("x.rs", "x\n");
        repo.commit();
        let git = repo.tool();
        git.create_branch("change/c1", "main").unwrap();
        assert_eq!(repo.git(&["rev-parse", "change/c1"]), main);
        assert_eq!(git.current_branch().unwrap(), "other");
        let again = git.create_branch("change/c1", "main").unwrap_err();
        let failed = "git branch change/c1 main failed";
        assert!(again.to_string().starts_with(failed), "{again}");
        for (name, from) in [("-x", "main"), ("change/c2", "--all")] {
            let option = git.create_branch(name, from);
            assert!(matches!(option, Err(Error::Revision(_))), "{name} {from}");
        }
        assert!(!git.exists("change/c2").unwrap());
    }

    #[test]
    fn names_git_would_read_as_options_never_reach_it() {
        let repo = Repo::new();
        repo.start("c1");
        let git = repo.tool();
        for name in ["", "-x", "--output=x"] {
            let exists = git.exists(name);
            assert!(matches!(exists, Err(Error::Revision(n)) if n == name));
            let since = git.changed_since("change/c1", name);
            assert!(matches!(since, Err(Error::Revision(n)) if n == name));
            let from = git.changed_from(name, "change/c1");
            assert!(matches!(from, Err(Error::Revision(n)) if n == name));
        }
        repo.git(&["checkout", "-q", "main"]);
        let away = git.changed_since("-x", "main");
        assert!(matches!(away, Err(Error::Revision(n)) if n == "-x"));
        assert!(!repo.root().join("x").exists());
    }

    #[test]
    fn a_git_failure_is_an_error_not_an_empty_answer() {
        let plain = Folder::new();
        let message = Git::new(&plain.0).exists("main").unwrap_err().to_string();
        assert!(
            message.starts_with("git rev-parse --verify --quiet main failed (exit status: 128): "),
            "{message}"
        );
        let message = Git::new(&plain.0).current_branch().unwrap_err().to_string();
        assert!(
            message.starts_with("git rev-parse --abbrev-ref HEAD failed (exit status: 128): "),
            "{message}"
        );
        let repo = Repo::new();
        repo.start("c1");
        let err = repo
            .tool()
            .changed_since("change/c1", "0123abcd")
            .unwrap_err();
        assert!(matches!(err, Error::Failed(_)));
        let start = "git diff --name-only -z --no-renames --relative 0123abcd -- failed";
        assert!(err.to_string().starts_with(start), "{err}");
        let err = repo.tool().changed_from("trunk", "change/c1").unwrap_err();
        assert!(err.to_string().starts_with("git diff "), "{err}");
    }

    #[test]
    fn the_commit_checked_out_and_where_a_branch_left_main() {
        let repo = Repo::new();
        let git = repo.tool();
        let main = repo.git(&["rev-parse", "HEAD"]);
        assert_eq!(git.head().unwrap(), main);
        repo.start("c1");
        repo.write("a.rs", "a\n");
        repo.commit();
        assert_eq!(git.head().unwrap(), repo.git(&["rev-parse", "HEAD"]));
        assert_ne!(git.head().unwrap(), main);
        repo.git(&["checkout", "-q", "main"]);
        repo.write("b.rs", "b\n");
        repo.commit();
        assert_eq!(git.merge_base("main", "change/c1").unwrap(), main);
        let err = git.merge_base("main", "change/c9").unwrap_err();
        let failed = "git merge-base main change/c9 failed";
        assert!(err.to_string().starts_with(failed), "{err}");
        for (main, branch) in [("-x", "change/c1"), ("main", "--all")] {
            let option = git.merge_base(main, branch);
            assert!(matches!(option, Err(Error::Revision(_))), "{branch}");
        }
        let plain = Folder::new();
        assert!(matches!(Git::new(&plain.0).head(), Err(Error::Failed(_))));
    }

    #[test]
    fn the_last_work_is_committed_then_merged_with_a_merge_commit() {
        let repo = Repo::new();
        repo.identify();
        repo.write(".gitignore", "ignored/\n");
        repo.commit();
        let git = repo.tool();
        assert!(!git.has_changes().unwrap());
        repo.start("c1");
        repo.write("src/a.rs", "a\n");
        repo.write("ignored/x", "x\n");
        assert!(git.has_changes().unwrap());
        git.commit_all("gdd(c1): change work").unwrap();
        assert!(!git.has_changes().unwrap());
        let subject = repo.git(&["log", "-1", "--format=%s"]);
        assert_eq!(subject, "gdd(c1): change work");
        let committed = repo.git(&["show", "--name-only", "--format=", "HEAD"]);
        assert_eq!(committed, "src/a.rs");
        git.checkout("main").unwrap();
        assert_eq!(git.current_branch().unwrap(), "main");
        assert!(!repo.root().join("src/a.rs").exists());
        let message = "gdd(c1): G3 Merge Gate\n\nColor: yellow";
        git.merge("change/c1", message).unwrap();
        assert_eq!(repo.git(&["log", "-1", "--format=%B"]), message);
        let parents = repo.git(&["log", "-1", "--format=%P"]);
        assert_eq!(parents.split(' ').count(), 2, "a merge commit: {parents}");
        assert!(repo.root().join("src/a.rs").is_file());
        assert!(repo.root().join("ignored/x").is_file());
        assert!(!git.has_changes().unwrap());
    }

    #[test]
    fn a_commit_of_everything_is_made_even_when_nothing_changed() {
        let repo = Repo::new();
        repo.identify();
        let git = repo.tool();
        repo.write("src/a.rs", "a\n");
        git.commit_all_even_empty("work").unwrap();
        let committed = repo.git(&["show", "--name-only", "--format=%s", "HEAD"]);
        assert_eq!(committed, "work\n\nsrc/a.rs");
        let before = git.head().unwrap();
        git.commit_all_even_empty("nothing").unwrap();
        assert_eq!(repo.git(&["rev-parse", "HEAD~1"]), before);
        let empty = repo.git(&["show", "--name-only", "--format=%s", "HEAD"]);
        assert_eq!(empty, "nothing");
        let plain = Folder::new();
        assert!(Git::new(&plain.0).commit_all_even_empty("x").is_err());
    }

    #[test]
    fn the_work_of_a_project_never_holds_the_local_state() {
        let repo = Repo::new();
        repo.identify();
        let git = repo.tool();
        repo.write(".gdd/changes/c1/change.json", "{}");
        assert!(git.has_changes().unwrap());
        assert!(!git.has_work().unwrap());
        repo.write("src/new.rs", "x\n");
        assert!(git.has_work().unwrap());
        repo.write("README.md", "changed\n");
        git.commit_work("work").unwrap();
        let committed = repo.git(&["show", "--name-only", "--format=", "HEAD"]);
        assert_eq!(committed, "README.md\nsrc/new.rs");
        assert!(!git.has_work().unwrap());
        assert!(repo.root().join(".gdd/changes/c1/change.json").is_file());
        let err = git.commit_work("nothing").unwrap_err().to_string();
        assert!(err.starts_with("git commit -q -m nothing failed"), "{err}");
        // Where git ignores the state, and where git tracks some of it.
        repo.write(".gitignore", ".gdd/\n");
        assert!(git.has_work().unwrap());
        git.commit_work("ignored").unwrap();
        repo.git(&["add", "-f", ".gdd/changes/c1/change.json"]);
        repo.git(&["commit", "-q", "-m", "tracked"]);
        repo.write(".gdd/changes/c1/change.json", "{\"state\": \"closed\"}");
        repo.write("src/new.rs", "y\n");
        git.commit_work("work").unwrap();
        let committed = repo.git(&["show", "--name-only", "--format=", "HEAD"]);
        assert_eq!(committed, "src/new.rs");
        let plain = Folder::new();
        assert!(Git::new(&plain.0).has_work().is_err());
        assert!(Git::new(&plain.0).commit_work("x").is_err());
    }

    #[test]
    fn only_tracked_files_make_tracked_changes() {
        let repo = Repo::new();
        let git = repo.tool();
        assert!(!git.has_tracked_changes().unwrap());
        repo.write("new.rs", "x\n");
        assert!(!git.has_tracked_changes().unwrap());
        assert!(git.has_changes().unwrap());
        repo.write("README.md", "changed\n");
        assert!(git.has_tracked_changes().unwrap());
        repo.git(&["checkout", "-q", "--", "README.md"]);
        fs::remove_file(repo.root().join("README.md")).unwrap();
        assert!(git.has_tracked_changes().unwrap());
        let plain = Folder::new();
        assert!(Git::new(&plain.0).has_tracked_changes().is_err());
    }

    #[test]
    fn only_what_the_index_holds_of_a_file_is_staged() {
        let repo = Repo::new();
        repo.write("app/notes.md", "a\n");
        repo.write("app/other.md", "a\n");
        repo.commit();
        let git = Git::new(&repo.root().join("app"));
        assert!(!git.has_staged("notes.md").unwrap());
        repo.write("app/notes.md", "b\n");
        assert!(!git.has_staged("notes.md").unwrap());
        repo.git(&["add", "app/notes.md"]);
        repo.write("app/notes.md", "a\n");
        assert!(git.has_staged("notes.md").unwrap());
        assert!(!git.has_staged("other.md").unwrap());
        repo.git(&["reset", "-q", "--", "app/notes.md"]);
        assert!(!git.has_staged("notes.md").unwrap());
        repo.git(&["rm", "-q", "--cached", "app/notes.md"]);
        assert!(git.has_staged("notes.md").unwrap());
        let plain = Folder::new();
        assert!(Git::new(&plain.0).has_staged("notes.md").is_err());
    }

    #[test]
    fn what_git_refuses_to_commit_check_out_or_merge_is_an_error() {
        let repo = Repo::new();
        repo.identify();
        let git = repo.tool();
        let err = git.commit_all("nothing").unwrap_err().to_string();
        assert!(err.starts_with("git commit -q -m nothing failed"), "{err}");
        let err = git.checkout("change/c9").unwrap_err().to_string();
        let failed = "git checkout -q change/c9 -- failed";
        assert!(err.starts_with(failed), "{err}");
        let err = git.merge("change/c9", "m").unwrap_err().to_string();
        let failed = "git merge --no-ff --no-edit -q -m m change/c9 failed";
        assert!(err.starts_with(failed), "{err}");
        for name in ["-x", "--all"] {
            assert!(matches!(git.checkout(name), Err(Error::Revision(_))));
            assert!(matches!(git.merge(name, "m"), Err(Error::Revision(_))));
            assert!(matches!(git.push(name), Err(Error::Revision(_))));
        }
        assert_eq!(git.current_branch().unwrap(), "main");
        let plain = Folder::new();
        assert!(Git::new(&plain.0).has_changes().is_err());
        assert!(Git::new(&plain.0).has_remote().is_err());
    }

    #[test]
    fn a_branch_is_pushed_to_origin() {
        let repo = Repo::new();
        repo.identify();
        let git = repo.tool();
        assert!(!git.has_remote().unwrap());
        let err = git.push("main").unwrap_err().to_string();
        assert!(err.starts_with("git push -q origin main failed"), "{err}");
        let remote = Folder::new();
        let path = remote.0.to_string_lossy().into_owned();
        repo.git(&["init", "-q", "--bare", &path]);
        repo.git(&["remote", "add", "origin", &path]);
        assert!(git.has_remote().unwrap());
        git.push("main").unwrap();
        let pushed = Git::new(&remote.0).line(&["rev-parse", "main"]).unwrap();
        assert_eq!(pushed, git.head().unwrap());
    }

    #[test]
    fn a_merge_can_stop_before_its_commit_and_be_given_up() {
        let repo = Repo::new();
        repo.identify();
        repo.start("c1");
        repo.write("a.txt", "a\n");
        repo.commit();
        repo.git(&["checkout", "-q", "main"]);
        let main = repo.git(&["rev-parse", "main"]);
        let git = repo.tool();
        git.merge_uncommitted("change/c1").unwrap();
        assert_eq!(git.head().unwrap(), main);
        repo.git(&["rev-parse", "-q", "--verify", "MERGE_HEAD"]);
        git.abort_merge().unwrap();
        assert!(!repo.root().join("a.txt").exists());
        let err = git.abort_merge().unwrap_err().to_string();
        assert!(err.starts_with("git merge --abort failed"), "{err}");
        git.merge_uncommitted("change/c1").unwrap();
        repo.write("b.txt", "b\n");
        git.commit_all("merged").unwrap();
        let parents = repo.git(&["log", "-1", "--format=%P"]);
        assert_eq!(parents.split(' ').count(), 2, "a merge commit");
        repo.git(&["cat-file", "-e", "HEAD:b.txt"]);
        let wrong = git.merge_uncommitted("-x");
        assert!(matches!(wrong, Err(Error::Revision(_))));
    }

    #[test]
    fn a_merge_in_conflict_names_its_paths() {
        let repo = Repo::new();
        repo.identify();
        repo.start("c1");
        repo.write("src/a.txt", "change\n");
        repo.write("same.txt", "same\n");
        repo.write("only-change.txt", "c\n");
        repo.commit();
        repo.git(&["checkout", "-q", "main"]);
        repo.write("src/a.txt", "main\n");
        repo.write("same.txt", "same\n");
        repo.write("README.md", "main\n");
        repo.commit();
        let git = repo.tool();
        assert_eq!(git.conflicts().unwrap(), Vec::<String>::new());
        assert!(git.merge("change/c1", "m").is_err());
        assert_eq!(git.conflicts().unwrap(), ["src/a.txt"]);
        // From the top of the working tree, wherever the project is.
        let inside = Git::new(&repo.root().join("src"));
        assert_eq!(inside.conflicts().unwrap(), ["src/a.txt"]);
        git.abort_merge().unwrap();
        assert_eq!(git.conflicts().unwrap(), Vec::<String>::new());
        let plain = Folder::new();
        assert!(Git::new(&plain.0).conflicts().is_err());
    }

    /// An environment with the `PATH` of the tests, an author of its own and no configuration of
    /// the user's.
    pub(crate) fn env() -> Vec<(String, String)> {
        let path = std::env::var("PATH").unwrap_or_default();
        let home = std::env::temp_dir().display().to_string();
        let vars = [
            ("PATH", path.as_str()),
            ("HOME", home.as_str()),
            ("GIT_AUTHOR_NAME", "Env Author"),
            ("GIT_AUTHOR_EMAIL", "env@example.com"),
            ("GIT_COMMITTER_NAME", "Env Author"),
            ("GIT_COMMITTER_EMAIL", "env@example.com"),
            ("GIT_CONFIG_GLOBAL", "/dev/null"),
            ("GIT_CONFIG_NOSYSTEM", "1"),
        ];
        let owned = |(name, value): (&str, &str)| (name.to_string(), value.to_string());
        vars.into_iter().map(owned).collect()
    }

    #[test]
    fn a_new_repository_commits_only_the_files_named_as_the_environment_says() {
        let folder = Folder::new();
        let git = Git::in_env(&folder.0, &env());
        git.init("trunk").unwrap();
        assert_eq!(git.checked_out().unwrap(), "trunk");
        folder.write("a.txt", "a\n");
        folder.write("b.txt", "b\n");
        folder.write("c/d e.txt", "d\n");
        let named = ["a.txt".to_string(), "c/d e.txt".to_string()];
        git.commit_files(&named, "first").unwrap();
        let last = ["log", "-1", "--format=%an <%ce> %s"];
        let commit = git.line(&last).unwrap();
        assert_eq!(commit, "Env Author <env@example.com> first");
        assert_eq!(git.files_at("HEAD", ".").unwrap(), named);
        assert_eq!(git.untracked().unwrap(), ["b.txt"]);
        let missing = git.commit_files(&["x.txt".to_string()], "second");
        let err = missing.unwrap_err().to_string();
        assert!(err.starts_with("git add -- x.txt failed"), "{err}");
        git.read(&["checkout", "-q", "--detach"]).unwrap();
        let err = git.checked_out().unwrap_err().to_string();
        let failed = "git symbolic-ref --short HEAD failed";
        assert!(err.starts_with(failed), "{err}");
        assert!(matches!(git.init("-x"), Err(Error::Revision(_))));
        let nowhere = [("PATH".to_string(), "/nonexistent".to_string())];
        let lost = Git::in_env(&folder.0, &nowhere);
        assert!(matches!(lost.checked_out(), Err(Error::Start(_))));
    }

    #[test]
    fn the_first_parents_of_a_branch_come_newest_first_with_their_trailers() {
        let repo = Repo::new();
        repo.identify();
        let git = repo.tool();
        let keys = ["Changelog", "Breaking"];
        let first = git.head().unwrap();
        repo.start("c1");
        let inside = "c1\n\nChangelog: Inside\nBreaking: no";
        repo.git(&["commit", "-q", "--allow-empty", "-m", inside]);
        repo.git(&["checkout", "-q", "main"]);
        let merge = "Merge\n\nRecord: x\n\nchangelog: Added · One\nBreaking: no\n\
                     Changelog: Fixed · Folded\n  over two lines\nOther: z";
        repo.git(&["merge", "-q", "--no-ff", "-m", merge, "change/c1"]);
        let merged = git.head().unwrap();
        let logged = git.first_parents("main", &keys).unwrap();
        let day = repo.git(&["log", "-1", "--format=%cs"]);
        let lines = ["Added · One", "Fixed · Folded over two lines"].map(String::from);
        let expected = [
            Logged {
                commit: merged,
                day: day.clone(),
                trailers: vec![lines.to_vec(), vec!["no".to_string()]],
            },
            Logged {
                commit: first.clone(),
                day: day.clone(),
                trailers: vec![vec![], vec![]],
            },
        ];
        assert_eq!(logged, expected);
        let plain = git.first_parents("main", &[]).unwrap();
        let bare: Vec<Vec<String>> = Vec::new();
        assert_eq!(plain[1].commit, first);
        assert_eq!((plain.len(), &plain[0].trailers), (2, &bare));
        assert_eq!(git.first_parents("change/c1", &keys).unwrap().len(), 2);
        assert!(matches!(
            git.first_parents("-x", &keys),
            Err(Error::Revision(_))
        ));
        let err = git.first_parents("trunk", &keys).unwrap_err().to_string();
        assert!(err.starts_with("git log -z --first-parent "), "{err}");
    }

    #[test]
    fn tags_name_their_commits_annotated_or_not() {
        let repo = Repo::new();
        repo.identify();
        let git = repo.tool();
        assert_eq!(git.tags().unwrap(), Vec::<(String, String)>::new());
        let first = git.head().unwrap();
        repo.git(&["tag", "v0.1.0"]);
        repo.commit();
        let second = git.head().unwrap();
        let annotated = [
            "-c",
            "tag.gpgsign=false",
            "tag",
            "-a",
            "-m",
            "Release",
            "v1.0.0",
        ];
        repo.git(&annotated);
        repo.git(&["tag", "rc/two", "HEAD~1"]);
        let tags = [("rc/two", &first), ("v0.1.0", &first), ("v1.0.0", &second)];
        let expected = tags.map(|(name, commit)| (name.to_string(), commit.clone()));
        assert_eq!(git.tags().unwrap(), expected);
        let plain = Folder::new();
        assert!(Git::new(&plain.0).tags().is_err());
    }

    #[test]
    fn an_annotated_tag_names_its_commit_is_never_moved_and_is_pushed_as_a_ref() {
        let repo = Repo::new();
        repo.identify();
        let git = repo.tool();
        let first = git.head().unwrap();
        repo.commit();
        git.tag("v1.0.0", "Release v1.0.0", &first).unwrap();
        assert_eq!(repo.git(&["cat-file", "-t", "v1.0.0"]), "tag");
        assert_eq!(repo.git(&["rev-parse", "v1.0.0^{commit}"]), first);
        let subject = ["tag", "-l", "--format=%(contents:subject)", "v1.0.0"];
        assert_eq!(repo.git(&subject), "Release v1.0.0");
        let err = git.tag("v1.0.0", "Again", "HEAD").unwrap_err().to_string();
        let start = "git tag -a -m Again v1.0.0 HEAD failed";
        assert!(err.starts_with(start), "{err}");
        assert_eq!(repo.git(&["rev-parse", "v1.0.0^{commit}"]), first);
        for (name, at) in [("-f", "HEAD"), ("v2.0.0", "--all"), ("", "HEAD")] {
            let err = git.tag(name, "Release", at).unwrap_err();
            assert!(matches!(err, Error::Revision(_)), "{name:?} {at:?}: {err}");
        }
        let remote = Folder::new();
        let path = remote.0.to_string_lossy().into_owned();
        repo.git(&["init", "-q", "--bare", &path]);
        repo.git(&["remote", "add", "origin", &path]);
        git.push("refs/tags/v1.0.0").unwrap();
        let pushed = Git::new(&remote.0).line(&["rev-parse", "v1.0.0^{commit}"]);
        assert_eq!(pushed.unwrap(), first);
    }

    #[test]
    fn errors_say_what_went_wrong() {
        let start = Error::Start(std::io::Error::other("no such program"));
        assert_eq!(start.to_string(), "cannot run git: no such program");
        assert_eq!(
            Error::Revision("-x".into()).to_string(),
            "\"-x\" is not a revision git can take"
        );
        let failed = Error::Failed("git x failed (exit status: 1): no".into());
        assert_eq!(failed.to_string(), "git x failed (exit status: 1): no");
    }
}
