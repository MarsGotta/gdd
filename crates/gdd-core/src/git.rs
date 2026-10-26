//! The project's git repository: which paths a change touched, what its branch holds, the
//! branch a new change gets, and the commits and merge that close it.
//!
//! Git always runs as `git` with a fixed list of arguments, in the project root, never through
//! a shell. A name read from a file never reaches git as an option, and a failure of git is an
//! [`Error`], never an empty answer. The tool never sets who commits: git takes the author from
//! the repository's configuration.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Local state of the tool: it never counts as part of a change.
const LOCAL_STATE: &str = ".gdd/";

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

/// The git repository of a project, read from the project root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Git {
    root: PathBuf,
}

impl Git {
    /// Git for the project at `root`. The paths it gives are relative to `root`, and paths
    /// outside it are left out.
    pub fn new(root: &Path) -> Git {
        Git {
            root: root.to_path_buf(),
        }
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

    /// The file `path`, from the project root, as it was at the commit `commit`: `None` when
    /// there was no file there.
    pub fn file_at(&self, commit: &str, path: &str) -> Result<Option<Vec<u8>>, Error> {
        let listing = self.read(&["ls-tree", "-z", revision(commit)?, "--", path])?;
        let listing = String::from_utf8_lossy(&listing);
        // One entry per NUL byte: "<mode> <type> <object>\t<path>".
        let object = listing.split('\0').find_map(|entry| {
            let (meta, name) = entry.split_once('\t')?;
            let mut fields = meta.split(' ');
            let (_, kind, object) = (fields.next()?, fields.next()?, fields.next()?);
            (name == path && kind == "blob").then_some(object)
        });
        match object {
            Some(object) => self.read(&["cat-file", "blob", object]).map(Some),
            None => Ok(None),
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

    /// Commits, on the current branch, everything the working tree changed except what git
    /// ignores, with `message`.
    pub fn commit_all(&self, message: &str) -> Result<(), Error> {
        self.read(&["add", "-A"])?;
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

    /// Whether the repository has a remote.
    pub fn has_remote(&self) -> Result<bool, Error> {
        Ok(!self.read(&["remote"])?.trim_ascii().is_empty())
    }

    /// Pushes the branch `name` to the remote `origin`.
    pub fn push(&self, name: &str) -> Result<(), Error> {
        self.read(&["push", "-q", "origin", revision(name)?])?;
        Ok(())
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
    fn read(&self, args: &[&str]) -> Result<Vec<u8>, Error> {
        let out = self.run(args)?;
        if out.status.success() {
            Ok(out.stdout)
        } else {
            Err(failed(args, &out))
        }
    }

    fn run(&self, args: &[&str]) -> Result<Output, Error> {
        Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .output()
            .map_err(Error::Start)
    }
}

/// The error of a git command that ran and failed.
fn failed(args: &[&str], out: &Output) -> Error {
    Error::Failed(format!(
        "git {} failed ({}): {}",
        args.join(" "),
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
