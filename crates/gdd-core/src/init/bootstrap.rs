//! `gdd bootstrap`: the first install of each stack of a project, which writes its lock file.
//! gdd init stays offline; this is where a new project gets its tools. It runs the `bootstrap`
//! command of each sheet, in the order of the stacks, from the project root with the shell (see
//! [`exec::run`]) in the environment gdd runs in, and stops at the first that fails. Then it
//! commits, as `gdd bootstrap`, the files of the stacks' supply chain that the run created or
//! changed and git does not ignore, and nothing else of the working tree. Before the first
//! install, it refuses a symbolic link on the way to any of those files and a file of them with
//! other names (a hard link): the installs would write out of the project.

use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use super::{Env, stack_sheets};
use crate::agents::render::first_link;
use crate::exec;
use crate::git::{self, Git};
use crate::stacks::{Sheet, once};

/// The message of its commit.
const MESSAGE: &str = "gdd bootstrap";
/// What it says when the run wrote nothing to commit.
const NOTHING: &str = "Nothing new to commit.";
/// What to do once the stacks are installed.
const NEXT: &str = "Next: bin/gdd selftest";

/// Why gdd bootstrap stopped, and what it said until then.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stopped {
    /// What it said before it stopped: each install it ran, with its output.
    pub said: String,
    /// Why it stopped.
    pub why: String,
}

/// Runs the first install of each of `stacks`, the stacks of the project at `root`, in the
/// environment `env` (see the module). What it says, without a line break at the end: for each
/// stack `<stack>: <command>` and what the command printed, then the files it committed or that
/// there was nothing new to commit, then the next step.
pub fn bootstrap(root: &Path, stacks: &[String], env: &Env) -> Result<String, Stopped> {
    let mut said = String::new();
    match installed(root, stacks, env, &mut said) {
        Ok(()) => Ok(said),
        Err(why) => Err(Stopped { said, why }),
    }
}

/// Runs the first install of each of `stacks` in the project at `root` (see [`steps`]).
fn installed(root: &Path, stacks: &[String], env: &Env, said: &mut String) -> Result<(), String> {
    let sheets = stack_sheets(stacks)?;
    let commands = sheets.iter().map(Sheet::bootstrap);
    let installs: Vec<(&str, &str)> = stacks.iter().map(String::as_str).zip(commands).collect();
    let supply_chain = once(sheets.iter().flat_map(Sheet::supply_chain));
    steps(root, &installs, &supply_chain, env, said)
}

/// Runs each of `installs`, a stack and its command, in the project at `root` (see
/// [`install`]), then commits the files of `supply_chain` they created or changed and git does
/// not ignore (see [`commit`]). What it says goes to `said`. Nothing runs when the installs
/// would write through a link (see [`check_links`]).
fn steps(
    root: &Path,
    installs: &[(&str, &str)],
    supply_chain: &[String],
    env: &Env,
    said: &mut String,
) -> Result<(), String> {
    check_links(root, supply_chain)?;
    let before = contents(root, supply_chain)?;
    for (stack, command) in installs {
        install(root, stack, command, env, said)?;
    }
    let after = contents(root, supply_chain)?;
    let git = Git::in_env(root, env);
    let failed = |e: git::Error| e.to_string();
    let branch = git.current_branch().map_err(failed)?;
    let changed = git.changed_since(&branch, "HEAD").map_err(failed)?;
    let mut written = Vec::new();
    for (path, (was, is)) in supply_chain.iter().zip(before.iter().zip(&after)) {
        if was != is && changed.contains(path) {
            written.push(path.clone());
        }
    }
    commit(&git, &written, said)
}

/// Checks that the installs, which write the files of `supply_chain` in the project at `root`,
/// write through no link: no symbolic link on the way to any of them, the files themselves
/// included, and no other name (a hard link) for one of them that is a file already. Through
/// either, they would write out of the project.
fn check_links(root: &Path, supply_chain: &[String]) -> Result<(), String> {
    for path in supply_chain {
        let link = first_link(root, path).map_err(|e| format!("cannot read {path}: {e}"))?;
        if let Some(link) = link {
            let link = link.display();
            return Err(format!(
                "cannot write {path}: {link} is a symbolic link, which gdd bootstrap does not \
                 write through: remove the link and run it again"
            ));
        }
        // `first_link` has looked at the path already: one that cannot be looked at stopped
        // there, and one that is missing has no other names.
        let meta = fs::symlink_metadata(root.join(path));
        if meta.is_ok_and(|meta| meta.is_file() && meta.nlink() > 1) {
            return Err(format!(
                "{path} has other names (a hard link): gdd bootstrap never writes through one: \
                 copy it to a file of its own and run it again"
            ));
        }
    }
    Ok(())
}

/// What each of `paths` of the project at `root` holds: its bytes, or `None` when there is no
/// file.
fn contents(root: &Path, paths: &[String]) -> Result<Vec<Option<Vec<u8>>>, String> {
    let read = |path: &String| match fs::read(root.join(path)) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("cannot read {path}: {e}")),
    };
    paths.iter().map(read).collect()
}

/// Runs `command`, the first install of `stack`, from the project root `root` with the shell in
/// the environment `env`, and says so in `said`, then what it printed. A command that fails, or
/// cannot run, is an error.
fn install(
    root: &Path,
    stack: &str,
    command: &str,
    env: &Env,
    said: &mut String,
) -> Result<(), String> {
    said.push_str(&format!("{stack}: {command}\n"));
    let vars = env.iter().map(|(k, v)| (k.as_str(), v.as_str()));
    let vars: Vec<(&str, &str)> = vars.collect();
    let ran = exec::run(command, root, &vars, exec::TIMEOUT);
    let ran = ran.map_err(|e| format!("{stack}: cannot run sh: {e}"))?;
    said.push_str(&ran.output);
    if !said.ends_with('\n') {
        said.push('\n');
    }
    if ran.ok() {
        Ok(())
    } else {
        Err(format!("{stack}: {command} failed (exit {})", ran.code))
    }
}

/// Commits the files `written` with `git`, as gdd bootstrap, and says which in `said`, or that
/// there was nothing new to commit; then the next step.
fn commit(git: &Git, written: &[String], said: &mut String) -> Result<(), String> {
    if written.is_empty() {
        said.push_str(NOTHING);
    } else {
        let committed = git.commit_files(written, MESSAGE);
        committed.map_err(|e| e.to_string())?;
        said.push_str(&format!("Committed: {}", written.join(", ")));
    }
    said.push('\n');
    said.push_str(NEXT);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::tests::git;
    use super::*;
    use crate::git::tests::{Folder, env};
    use std::os::unix::fs::PermissionsExt;

    /// A project in a new repository whose first commit holds `files`.
    fn project(files: &[(&str, &str)]) -> Folder {
        let folder = Folder::new();
        let git = Git::in_env(&folder.0, &env());
        git.init("main").unwrap();
        let mut paths = vec!["README.md".to_string()];
        folder.write("README.md", "demo\n");
        for (path, text) in files {
            folder.write(path, text);
            paths.push(path.to_string());
        }
        git.commit_files(&paths, "first").unwrap();
        folder
    }

    /// Runs `installs` in `folder`, whose stacks have the supply chain `supply`: why it stopped
    /// (empty when it did not), and what it said.
    fn ran(folder: &Folder, installs: &[(&str, &str)], supply: &[&str]) -> (String, String) {
        let supply_chain: Vec<String> = supply.iter().map(|path| path.to_string()).collect();
        let mut said = String::new();
        let done = steps(&folder.0, installs, &supply_chain, &env(), &mut said);
        (done.err().unwrap_or_default(), said)
    }

    fn last_commit(folder: &Folder) -> String {
        git(folder, &["log", "-1", "--format=%s"])
    }

    #[test]
    fn each_install_runs_in_order_and_its_output_is_shown() {
        let folder = project(&[]);
        let installs = [("a", "printf one"), ("b", "echo two >&2"), ("c", "true")];
        let (why, said) = ran(&folder, &installs, &[]);
        assert_eq!(why, "");
        let expected = "a: printf one\none\nb: echo two >&2\ntwo\nc: true\n\
                        Nothing new to commit.\nNext: bin/gdd selftest";
        assert_eq!(said, expected);
        assert_eq!(last_commit(&folder), "first");
    }

    #[test]
    fn the_first_install_that_fails_stops_the_run_and_nothing_is_committed() {
        let folder = project(&[]);
        let fails = "echo out; echo 1 > a.lock; exit 3";
        let (why, said) = ran(&folder, &[("a", fails), ("b", "touch ran")], &["a.lock"]);
        assert_eq!(why, format!("a: {fails} failed (exit 3)"));
        assert_eq!(said, format!("a: {fails}\nout\n"));
        assert!(!folder.0.join("ran").exists());
        assert_eq!(last_commit(&folder), "first");
        assert_eq!(git(&folder, &["status", "--porcelain"]), "?? a.lock");
    }

    #[test]
    fn only_the_supply_chain_files_the_run_wrote_and_git_keeps_are_committed() {
        let committed = [
            (".gitignore", "c.lock\n"),
            ("b.lock", "old\n"),
            ("d.lock", "d\n"),
        ];
        let folder = project(&committed);
        folder.write("d.lock", "mine\n");
        let writes = "echo 1 > a.lock; echo 2 > b.lock; echo 3 > c.lock; echo 4 > other";
        let supply_chain = ["a.lock", "b.lock", "c.lock", "d.lock", "e.lock"];
        let (why, said) = ran(&folder, &[("x", writes)], &supply_chain);
        assert_eq!(why, "");
        let end = "\nCommitted: a.lock, b.lock\nNext: bin/gdd selftest";
        assert!(said.ends_with(end), "{said}");
        assert_eq!(last_commit(&folder), "gdd bootstrap");
        let last = git(&folder, &["show", "--name-only", "--format=", "HEAD"]);
        assert_eq!(last, "a.lock\nb.lock");
        let status = git(&folder, &["status", "--porcelain"]);
        assert_eq!(status, "M d.lock\n?? other");
    }

    #[test]
    fn a_supply_chain_file_that_cannot_be_read_stops_the_run_before_it_starts() {
        let folder = project(&[("a", "a file\n")]);
        let (why, said) = ran(&folder, &[("x", "touch ran")], &["a/b"]);
        assert!(why.starts_with("cannot read a/b: "), "{why}");
        assert_eq!(said, "");
        assert!(!folder.0.join("ran").exists());
        // A folder has more than one name, but it is not a file to read.
        fs::create_dir(folder.0.join("c.lock")).unwrap();
        let (why, said) = ran(&folder, &[("x", "touch ran")], &["c.lock"]);
        assert!(why.starts_with("cannot read c.lock: "), "{why}");
        assert_eq!(said, "");
        assert!(!folder.0.join("ran").exists());
        let none = contents(&folder.0, &["none".to_string()]);
        assert_eq!(none, Ok(vec![None]));
    }

    #[test]
    fn nothing_runs_when_the_installs_would_write_through_a_link() {
        let elsewhere = Folder::new();
        elsewhere.write("Cargo.lock", "theirs\n");
        let outside = elsewhere.0.join("Cargo.lock");
        let programs = Folder::new();
        let env = with_cargo(&programs, "echo lock > Cargo.lock; touch ran");
        let rust = ["rust".to_string()];
        let folder = project(&[("deny.toml", "")]);
        std::os::unix::fs::symlink(&outside, folder.0.join("Cargo.lock")).unwrap();
        let stopped = bootstrap(&folder.0, &rust, &env).unwrap_err();
        let why = "cannot write Cargo.lock: Cargo.lock is a symbolic link, which gdd bootstrap \
                   does not write through: remove the link and run it again";
        assert_eq!((stopped.said.as_str(), stopped.why.as_str()), ("", why));
        assert!(!folder.0.join("ran").exists());
        let folder = project(&[("deny.toml", "")]);
        fs::hard_link(&outside, folder.0.join("Cargo.lock")).unwrap();
        let stopped = bootstrap(&folder.0, &rust, &env).unwrap_err();
        let why = "Cargo.lock has other names (a hard link): gdd bootstrap never writes through \
                   one: copy it to a file of its own and run it again";
        assert_eq!((stopped.said.as_str(), stopped.why.as_str()), ("", why));
        assert!(!folder.0.join("ran").exists());
        // A folder on the way that is a link counts too.
        let folder = project(&[]);
        std::os::unix::fs::symlink(&elsewhere.0, folder.0.join("locks")).unwrap();
        let writes = [("x", "echo lock > locks/Cargo.lock")];
        let (why, said) = ran(&folder, &writes, &["locks/Cargo.lock"]);
        let start = "cannot write locks/Cargo.lock: locks is a symbolic link, ";
        assert!(why.starts_with(start), "{why}");
        assert_eq!(said, "");
        assert_eq!(fs::read_to_string(&outside).unwrap(), "theirs\n");
        assert_eq!(fs::read_dir(&elsewhere.0).unwrap().count(), 1);
    }

    /// The environment of the tests, with a program `cargo` that runs `script` first on its
    /// `PATH`; the program is in `programs`.
    fn with_cargo(programs: &Folder, script: &str) -> Vec<(String, String)> {
        programs.write("cargo", &format!("#!/bin/sh\n{script}\n"));
        let mode = fs::Permissions::from_mode(0o755);
        fs::set_permissions(programs.0.join("cargo"), mode).unwrap();
        let mut env = env();
        for (name, value) in &mut env {
            if name == "PATH" {
                *value = format!("{}:{value}", programs.0.display());
            }
        }
        env
    }

    #[test]
    fn bootstrap_runs_the_first_install_of_each_stack_of_the_project() {
        let folder = project(&[("deny.toml", "")]);
        let programs = Folder::new();
        let rust = ["rust".to_string()];
        let env = with_cargo(&programs, "echo lock > Cargo.lock");
        let said = bootstrap(&folder.0, &rust, &env).unwrap();
        let expected = "rust: cargo generate-lockfile\n\
                        Committed: Cargo.lock\nNext: bin/gdd selftest";
        assert_eq!(said, expected);
        let env = with_cargo(&programs, "echo broken; exit 1");
        let stopped = bootstrap(&folder.0, &rust, &env).unwrap_err();
        let said = "rust: cargo generate-lockfile\nbroken\n".to_string();
        let why = "rust: cargo generate-lockfile failed (exit 1)".to_string();
        assert_eq!(stopped, Stopped { said, why });
        let cobol = bootstrap(&folder.0, &["cobol".to_string()], &env).unwrap_err();
        let unknown = "unknown stack cobol (known: python, rust, typescript)";
        assert_eq!((cobol.said.as_str(), cobol.why.as_str()), ("", unknown));
    }
}
