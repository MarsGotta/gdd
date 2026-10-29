//! `gdd upgrade`: what this gdd writes of the files `gdd init` wrote once, merged with what the
//! project made of them, proposed on the branch `gdd-upgrade` for a person to review and merge.
//! Those files are the project's: nothing on the main branch changes, and nothing is pushed.
//!
//! Each file is merged three ways, as Copier updates a project from its template: the base is
//! what the older gdd wrote, kept in the last commit that touched `gdd/generated.toml`; ours, the
//! file on the main branch; theirs, what this gdd renders from the answers gdd init recorded (see
//! [`init::render_once`]). The merge is git's own (`git merge-file`), through files in a fresh
//! folder under `.gdd/upgrade/` of the state root, removed afterwards. The branch gets two
//! commits: what this gdd writes, pure, with the new `gdd/generated.toml` (the base of the next
//! upgrade), then the merge with the project's changes. Both are made in a worktree of their own,
//! in the folder of the changes' worktrees, which goes whatever happens (and that folder with it
//! when the run made it and nothing else came into it); the branch is made last, on the second
//! commit, so a run that fails leaves none. Where this gdd writes a file and the project has
//! something else (a folder, a link), the upgrade stops before writing anything.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::Deserialize;

use crate::VERSION;
use crate::agents::files;
use crate::config;
use crate::gates::approve;
use crate::git::{self, Git};
use crate::guard::{self, Settings};
use crate::init::{self, Answers, GENERATED};
use crate::recipe::worktree;
use crate::stacks;

/// The branch the upgrade is proposed on.
const BRANCH: &str = "gdd-upgrade";

/// The folder of the merges' files, from the state root: a fresh folder in it for each run.
const SCRATCH: &str = ".gdd/upgrade";

/// How the conflict marks name the project's side.
const YOURS: &str = "yours";

/// Why a Builder or a Reviewer cannot upgrade.
const NOT_FOR_AGENTS: &str = "Builders and Reviewers cannot run gdd upgrade: a person runs it, \
                              then reviews and merges its branch";

/// Why a project without `gdd/generated.toml` cannot be upgraded.
const MISSING: &str = "gdd/generated.toml is missing: this project was set up before gdd recorded \
                       what it writes, so gdd upgrade has no base to merge from";

/// Why a project whose `gdd/generated.toml` has no answers cannot be upgraded.
const NO_ANSWERS: &str = "gdd/generated.toml has no [answers]: it was written before gdd \
                          recorded them, add them as gdd init writes them now";

/// Why a project whose `gdd/generated.toml` no commit holds cannot be upgraded.
const UNCOMMITTED_RECORD: &str = "gdd/generated.toml is in no commit: commit it first, gdd \
                                  upgrade merges from the commit that wrote it";

/// Why a project with uncommitted changes cannot be upgraded.
const UNCOMMITTED: &str = "commit or stash your changes first";

/// What a run that failed once it started on its branch says, before why.
const NO_BRANCH: &str = "gdd upgrade made no branch and main is as it was";

/// Why a project whose upgrade branch is there cannot be upgraded again.
const BRANCH_THERE: &str = "the branch gdd-upgrade is already there: merge it or delete it (git \
                            branch -D gdd-upgrade) first";

/// An environment: its variables, with their values.
type Env = [(String, String)];

/// What `gdd/generated.toml` says: the version of the gdd that wrote it, the files that gdd
/// writes once and, when it recorded them, what gdd init was asked.
#[derive(Debug, Deserialize)]
struct Generated {
    version: String,
    files: Vec<String>,
    answers: Option<Answers>,
}

/// What becomes of a file of the upgrade on its branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fate {
    /// This gdd writes it as the older one did: the project's file stays.
    Kept,
    /// This gdd no longer writes it: the project's file stays.
    Dropped,
    /// New in this gdd, and the project has none: this gdd's.
    Added,
    /// The project had not changed it: this gdd's.
    Updated,
    /// The project removed it: it stays out.
    LeftOut,
    /// Merged with the project's changes, with this many conflicts marked.
    Merged(usize),
}

impl Fate {
    /// What the output says of a file with this fate.
    fn said(self) -> String {
        match self {
            Fate::Kept => "yours, gdd writes it as before".to_string(),
            Fate::Dropped => format!("yours, gdd {VERSION} no longer writes it"),
            Fate::Added => format!("added (new in gdd {VERSION})"),
            Fate::Updated => "updated (you had not changed it)".to_string(),
            Fate::LeftOut => "left out: you removed it".to_string(),
            Fate::Merged(0) => "merged with your changes".to_string(),
            Fate::Merged(conflicts) => format!("conflict, settle the marks ({conflicts})"),
        }
    }
}

/// Which text a file of the upgrade takes, from what the older gdd wrote, what the project has
/// and what this gdd writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Choice {
    /// The project's, with this fate.
    Ours(Fate),
    /// This gdd's, with this fate.
    Theirs(Fate),
    /// The three merged, against an empty base when the older gdd wrote none.
    Merge,
}

/// Which text a file takes from what the older gdd wrote of it (`base`), what the project has
/// (`ours`) and what this gdd writes (`theirs`), each `None` where there is no file.
fn choose(base: Option<&[u8]>, ours: Option<&[u8]>, theirs: Option<&[u8]>) -> Choice {
    match (base, ours, theirs) {
        (base, _, theirs) if theirs == base => Choice::Ours(Fate::Kept),
        (_, _, None) => Choice::Ours(Fate::Dropped),
        (None, None, _) => Choice::Theirs(Fate::Added),
        (Some(base), Some(ours), _) if ours == base => Choice::Theirs(Fate::Updated),
        (Some(_), None, _) => Choice::Ours(Fate::LeftOut),
        _ => Choice::Merge,
    }
}

/// A file of the upgrade, settled.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Settled {
    /// Where it is, from the project root.
    path: String,
    /// What becomes of it.
    fate: Fate,
    /// Its text after the first commit: this gdd's when it writes it, else the project's; `None`
    /// where there is no file.
    first: Option<Vec<u8>>,
    /// Its text after the second commit; `None` where there is no file.
    last: Option<Vec<u8>>,
}

/// `gdd upgrade` in the project at `root`, the folder of the main checkout (and so its state root,
/// see [`crate::state::root`]), for the role `role` (`None` for a person), with git and the agents
/// on the `PATH` of `env`, the environment gdd runs in (see the module). A Builder or a Reviewer
/// is refused first; then a project whose `gdd/generated.toml` is missing or has no answers, one
/// whose checkout is not on the main branch of `gdd/rules.toml`, one with tracked changes not
/// committed and one where the branch `gdd-upgrade` is already there, in that order, with nothing
/// written. What it says: what became of each file and how to review and take the branch, or that
/// there is nothing to propose; without a line break at the end.
pub fn upgrade(root: &Path, role: Option<&str>, env: &Env) -> Result<String, String> {
    approve::may_record(role).map_err(|_| NOT_FOR_AGENTS.to_string())?;
    let generated = recorded(root)?;
    let answers = generated.answers.ok_or(NO_ANSWERS)?;
    let rules = read(root, guard::RULES)?.unwrap_or_default();
    let rules = String::from_utf8_lossy(&rules).into_owned();
    let unreadable = |e: config::Error| format!("{}: {e}", guard::RULES);
    let settings = Settings::from_toml(&rules).map_err(unreadable)?;
    let main = settings.main_branch;
    let git = Git::in_env(root, env);
    check(&git, &main)?;
    let base = git.last_commit_of(GENERATED).map_err(said)?;
    let base = base.ok_or(UNCOMMITTED_RECORD)?;
    let theirs = init::render_once(&answers, env).map_err(unrendered)?;
    let upgrade = Upgrade {
        root,
        git,
        env,
        main,
        old: generated.version,
        base,
        listed: generated.files,
        theirs,
    };
    let files = upgrade.settle()?;
    if files.iter().all(|file| file.fate == Fate::Kept) {
        let nothing = format!("Already up to date with gdd {VERSION}: nothing to propose.");
        return Ok(nothing);
    }
    let folder = worktree::folder(root, &rules).map_err(unreadable)?;
    let path = worktree::path(&folder, BRANCH)?;
    let proposed = upgrade.propose(&folder, &path, &files);
    proposed.map_err(|why| format!("{NO_BRANCH}: {why}"))?;
    Ok(upgrade.said(&files))
}

/// The checks on the checkout of `git` before an upgrade, after its record: on the branch `main`,
/// with no tracked change uncommitted, and without the branch `gdd-upgrade`.
fn check(git: &Git, main: &str) -> Result<(), String> {
    if git.current_branch().map_err(said)? != main {
        let switch = format!("git switch {main}, then run it again");
        return Err(format!("gdd upgrade runs on {main}: {switch}"));
    }
    if git.has_tracked_changes().map_err(said)? {
        return Err(UNCOMMITTED.to_string());
    }
    if git.exists(&format!("refs/heads/{BRANCH}")).map_err(said)? {
        return Err(BRANCH_THERE.to_string());
    }
    Ok(())
}

/// What an upgrade works with.
struct Upgrade<'a> {
    /// The folder of the project in the main checkout, and its state root.
    root: &'a Path,
    /// Git there, in the environment gdd runs in.
    git: Git,
    /// The environment gdd runs in.
    env: &'a Env,
    /// The main branch.
    main: String,
    /// The version of the gdd that wrote the files last.
    old: String,
    /// The commit with what it wrote: the last that touched `gdd/generated.toml`.
    base: String,
    /// The files it wrote once.
    listed: Vec<String>,
    /// What this gdd writes once, and its `gdd/generated.toml`: their paths and texts, in order.
    theirs: Vec<(String, String)>,
}

impl Upgrade<'_> {
    /// The files of the upgrade, settled, merged in a fresh folder under `.gdd/upgrade/` of the
    /// state root, which goes whatever happened.
    fn settle(&self) -> Result<Vec<Settled>, String> {
        let scratch = format!("{SCRATCH}/{}", std::process::id());
        remove(self.root, &scratch)?;
        let paths = self.paths();
        let settle = |path: &String| self.file(path, &scratch);
        let settled: Result<Vec<Settled>, String> = paths.iter().map(settle).collect();
        let removed = remove(self.root, &scratch);
        let settled = settled?;
        removed?;
        Ok(settled)
    }

    /// The paths of the upgrade: those this gdd writes once, in its order, then those only the
    /// older one wrote, each once. Never `gdd/generated.toml`: this gdd's is taken as it is.
    fn paths(&self) -> Vec<String> {
        let theirs = self.theirs.iter().map(|(path, _)| path);
        let all = theirs.chain(&self.listed);
        stacks::once(all.filter(|path| *path != GENERATED))
    }

    /// The file `path` settled, merged in the folder `scratch` of the state root when it must be.
    fn file(&self, path: &str, scratch: &str) -> Result<Settled, String> {
        let theirs = self.theirs.iter().find(|(at, _)| at == path);
        let theirs = theirs.map(|(_, text)| text.as_bytes().to_vec());
        // What the older gdd did not list it did not write, whatever its commit holds there.
        let base = if self.listed.iter().any(|at| at == path) {
            self.git.file_at(&self.base, path).map_err(said)?
        } else {
            None
        };
        let ours = self.git.file_at("HEAD", path).map_err(said)?;
        let (fate, last) = match choose(base.as_deref(), ours.as_deref(), theirs.as_deref()) {
            Choice::Ours(fate) => (fate, ours.clone()),
            Choice::Theirs(fate) => (fate, theirs.clone()),
            Choice::Merge => {
                let texts = [&ours, &base, &theirs].map(|text| text.as_deref());
                let texts = texts.map(Option::unwrap_or_default);
                let (merged, conflicts) = self.merge(texts, scratch)?;
                (Fate::Merged(conflicts), Some(merged))
            }
        };
        Ok(Settled {
            path: path.to_string(),
            fate,
            first: theirs.or(ours),
            last,
        })
    }

    /// The `texts` of the project, the older gdd and this one, in that order, merged by git
    /// through files in the folder `scratch` of the state root: the text and its conflicts.
    fn merge(&self, texts: [&[u8]; 3], scratch: &str) -> Result<(Vec<u8>, usize), String> {
        let names = [YOURS, "base", "theirs"];
        let names = names.map(|name| format!("{scratch}/{name}"));
        for (name, text) in names.iter().zip(texts) {
            write(self.root, name, text)?;
        }
        // From the root, where git runs.
        let [ours, base, theirs] = names.each_ref().map(Path::new);
        let (old, new) = (format!("gdd {}", self.old), format!("gdd {VERSION}"));
        let labels = [YOURS, old.as_str(), new.as_str()];
        let conflicts = self.git.merge_file(ours, base, theirs, labels);
        let conflicts = conflicts.map_err(said)?;
        let merged = read(self.root, &names[0])?.unwrap_or_default();
        Ok((merged, conflicts))
    }

    /// Makes the two commits of the `files` in a worktree of their own at `path`, in the folder
    /// of the worktrees `folder`, and the branch on the second. The worktree goes whatever
    /// happened, and the folder with it when this run made it and nothing else came into it.
    fn propose(&self, folder: &Path, path: &Path, files: &[Settled]) -> Result<(), String> {
        // Looked at before git makes it for the worktree: a folder that was there stays.
        let made = !folder.exists();
        let last = self.in_worktree(path, files);
        let tidied = if made { tidy(folder) } else { Ok(()) };
        let last = last?;
        tidied?;
        self.git.create_branch(BRANCH, &last).map_err(said)
    }

    /// Makes the two commits of the `files` in a worktree of their own at `path`, from the main
    /// branch as it is: the id of the second. The worktree goes whatever happened.
    fn in_worktree(&self, path: &Path, files: &[Settled]) -> Result<String, String> {
        let from = self.git.head().map_err(said)?;
        self.git.add_worktree(path, &from).map_err(said)?;
        let made = self.commit(path, files);
        let removed = self.git.remove_worktree(path);
        let removed = removed.and_then(|()| self.git.prune_worktrees());
        let last = made?;
        removed.map_err(said)?;
        Ok(last)
    }

    /// Makes, in the worktree at `path`, the commit of what this gdd writes, then the one of the
    /// `files` as they end: the id of the second. Nothing is written when one of the files would
    /// take the place of something of the project's (see [`Upgrade::free`]).
    fn commit(&self, path: &Path, files: &[Settled]) -> Result<String, String> {
        let work = worktree::project_root(self.root, path)?;
        let git = Git::in_env(&work, self.env);
        for (file, _) in &self.theirs {
            self.free(&work, file)?;
        }
        for (file, text) in &self.theirs {
            write(&work, file, text.as_bytes())?;
        }
        let written: Vec<String> = self.theirs.iter().map(|(file, _)| file.clone()).collect();
        let upgrade = format!("gdd upgrade {} to {VERSION}", self.old);
        let pure = format!("{upgrade}: what gdd {VERSION} writes");
        git.commit_files(&written, &pure).map_err(said)?;
        for file in files.iter().filter(|file| file.last != file.first) {
            match &file.last {
                Some(text) => write(&work, &file.path, text)?,
                None => remove(&work, &file.path)?,
            }
        }
        let merged = format!("{upgrade}: merged with the project's changes");
        git.commit_all_even_empty(&merged).map_err(said)?;
        git.head().map_err(said)
    }

    /// Whether this gdd may write its file `path` in the worktree at `work`, checked out from the
    /// main branch: a file is there or nothing, with only folders on the way. Anything else (a
    /// folder, a link or a submodule there, a file or a link on the way) is the project's, and
    /// writing would remove it (see [`files::replace`]).
    fn free(&self, work: &Path, path: &str) -> Result<(), String> {
        files::found(work, path).map(drop).map_err(|e| {
            let main = &self.main;
            let next = "move what is in the way, commit, then run gdd upgrade again";
            format!("{path} on {main}: {e}, where gdd {VERSION} writes a file: {next}")
        })
    }

    /// What the upgrade says of the `files` proposed.
    fn said(&self, files: &[Settled]) -> String {
        let (old, main) = (&self.old, &self.main);
        let title = format!("gdd upgrade {old} to {VERSION}, on the branch {BRANCH}:");
        let mut lines = vec![title];
        for file in files {
            lines.push(format!("  {}: {}", file.path, file.fate.said()));
        }
        let marked = |file: &Settled| matches!(file.fate, Fate::Merged(marks) if marks > 0);
        if files.iter().any(marked) {
            let settle = format!("settle the marks, commit, then git switch {main}");
            lines.push(format!("Conflicts: git switch {BRANCH}, {settle}."));
        }
        let review = format!("Review it: git diff {main} {BRANCH}");
        let merge = format!("git merge {BRANCH}, git branch -d {BRANCH}");
        let take = format!("Take it: {merge}, then bin/gdd agents install");
        lines.extend([review, take]);
        lines.join("\n")
    }
}

/// What `gdd/generated.toml` of the folder `root` says; without one, why it cannot be upgraded.
fn recorded(root: &Path) -> Result<Generated, String> {
    let Some(bytes) = read(root, GENERATED)? else {
        return Err(MISSING.to_string());
    };
    let text = String::from_utf8_lossy(&bytes);
    toml::from_str(&text).map_err(|e| format!("{GENERATED}: {e}"))
}

/// The bytes of the file `path` of the folder `root`, never through a link (see
/// [`files::read`]): `None` when there is none.
fn read(root: &Path, path: &str) -> Result<Option<Vec<u8>>, String> {
    files::read(root, path).map_err(|e| format!("cannot read {path}: {e}"))
}

/// Makes `path` of the folder `root` a new file with `bytes`, never through a link (see
/// [`files::replace`]).
fn write(root: &Path, path: &str, bytes: &[u8]) -> Result<(), String> {
    files::replace(root, path, bytes).map_err(|e| format!("cannot write {path}: {e}"))
}

/// Leaves nothing at `path` of the folder `root`, never through a link (see [`files::remove`]).
fn remove(root: &Path, path: &str) -> Result<(), String> {
    files::remove(root, path).map_err(|e| format!("cannot remove {path}: {e}"))
}

/// Removes the folder `folder` when it is empty: one with anything in it stays, and nothing there
/// is left as it is. A link there is not a folder, and is not removed.
fn tidy(folder: &Path) -> Result<(), String> {
    match fs::remove_dir(folder) {
        Ok(()) => Ok(()),
        Err(e) if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::DirectoryNotEmpty) => Ok(()),
        Err(e) => Err(format!("cannot remove {}: {e}", folder.display())),
    }
}

/// What a failure of git says.
fn said(e: git::Error) -> String {
    e.to_string()
}

/// Why the files cannot be rendered from the answers, without the `error: ` gdd init starts with.
fn unrendered(e: init::Error) -> String {
    match e {
        init::Error::Failed(why) => why,
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::{Folder, env};
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::PathBuf;
    use std::process::Command;

    const WORKFLOW: &str = ".github/workflows/gdd.yml";
    const BASICS: &str = "gdd/project.toml";
    const ZONES: &str = "gdd/zones.toml";
    const BRIEF: &str = "docs/brief.md";

    fn names(named: &[&str]) -> Vec<String> {
        named.iter().map(|name| name.to_string()).collect()
    }

    /// A project gdd init set up, for Claude Code and Codex, in the folder `project` of a folder
    /// of its own, which also holds the folder of its worktrees.
    struct Project {
        folder: Folder,
        root: PathBuf,
    }

    impl Project {
        fn new() -> Project {
            let folder = Folder::new();
            let root = folder.0.join("project");
            fs::create_dir_all(&root).unwrap();
            let options = init::Options {
                name: "Notes".to_string(),
                platform: "cli".to_string(),
                stacks: names(&["rust"]),
                agents: names(&["claude", "codex"]),
                language: "en".to_string(),
                description: String::new(),
                sensitive: Vec::new(),
                accept_same_family: false,
            };
            init::init(&root, &options, &env()).unwrap();
            Project { folder, root }
        }

        /// Runs git in the project, in the environment of the tests: what it printed, trimmed.
        fn git(&self, args: &[&str]) -> String {
            let mut git = Command::new("git");
            git.args(args).current_dir(&self.root);
            let out = git.env_clear().envs(env()).output().unwrap();
            let said = String::from_utf8_lossy(&out.stderr);
            assert!(out.status.success(), "git {args:?}: {said}");
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        }

        fn read(&self, path: &str) -> String {
            fs::read_to_string(self.root.join(path)).unwrap()
        }

        fn write(&self, path: &str, text: &str) {
            let file = self.root.join(path);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, text).unwrap();
        }

        /// Replaces the first `old` of the file `path` by `new`.
        fn replace(&self, path: &str, old: &str, new: &str) {
            let text = self.read(path);
            assert!(text.contains(old), "{path} lacks {old:?}");
            self.write(path, &text.replacen(old, new, 1));
        }

        fn remove(&self, path: &str) {
            fs::remove_file(self.root.join(path)).unwrap();
        }

        fn commit(&self, message: &str) {
            self.git(&["add", "-A"]);
            self.git(&["commit", "-q", "-m", message]);
        }

        /// Plays an older gdd `version` that wrote the project as it is now, with the files
        /// `unlisted` out of `gdd/generated.toml` and the files `listed` in it: committed as
        /// gdd init commits.
        fn older(&self, version: &str, unlisted: &[&str], listed: &[&str]) {
            let mut generated: toml::Table = toml::from_str(&self.read(GENERATED)).unwrap();
            let files = generated["files"].as_array().unwrap().iter();
            let kept = files.filter(|file| !unlisted.contains(&file.as_str().unwrap()));
            let mut files: Vec<toml::Value> = kept.cloned().collect();
            files.extend(listed.iter().map(|&file| toml::Value::from(file)));
            generated.insert("files".to_string(), toml::Value::Array(files));
            generated.insert("version".to_string(), version.into());
            self.write(GENERATED, &toml::to_string(&generated).unwrap());
            self.commit(&format!("gdd init {version}"));
        }

        fn upgrade(&self) -> Result<String, String> {
            upgrade(&self.root, None, &env())
        }

        /// The file `path` on the branch `gdd-upgrade`; `None` without one.
        fn on_branch(&self, path: &str) -> Option<String> {
            let git = Git::in_env(&self.root, &env());
            let bytes = git.file_at(BRANCH, path).unwrap();
            bytes.map(|bytes| String::from_utf8(bytes).unwrap())
        }

        fn has_branch(&self) -> bool {
            let git = Git::in_env(&self.root, &env());
            git.exists(&format!("refs/heads/{BRANCH}")).unwrap()
        }

        /// The folder of its worktrees, next to it.
        fn worktrees(&self) -> PathBuf {
            self.folder.0.join("project.worktrees")
        }

        /// Whether the upgrade left nothing behind: the worktree, the folder of the worktrees it
        /// made for it, its files of the merges.
        fn left_nothing(&self) -> bool {
            let listed = self.git(&["worktree", "list", "--porcelain"]);
            let worktrees = listed.matches("worktree ").count();
            let id = std::process::id().to_string();
            let scratch = self.root.join(SCRATCH).join(id);
            !self.worktrees().exists() && worktrees == 1 && !scratch.exists()
        }

        /// Has git run `script` before each commit of the project, its worktrees' too.
        fn before_each_commit(&self, script: &str) {
            let hooks = self.folder.0.join("hooks");
            fs::create_dir_all(&hooks).unwrap();
            let hook = hooks.join("pre-commit");
            fs::write(&hook, format!("#!/bin/sh\n{script}\n")).unwrap();
            fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
            let path = hooks.display().to_string();
            self.git(&["config", "--local", "core.hooksPath", &path]);
        }
    }

    #[test]
    fn a_file_takes_the_text_its_three_versions_choose() {
        let (a, b, c): (&[u8], &[u8], &[u8]) = (b"a", b"b", b"c");
        let cases = [
            ((Some(a), Some(b), Some(a)), Choice::Ours(Fate::Kept)),
            ((Some(a), None, Some(a)), Choice::Ours(Fate::Kept)),
            ((None, Some(b), None), Choice::Ours(Fate::Kept)),
            ((Some(a), Some(b), None), Choice::Ours(Fate::Dropped)),
            ((Some(a), None, None), Choice::Ours(Fate::Dropped)),
            ((None, None, Some(c)), Choice::Theirs(Fate::Added)),
            ((None, Some(b), Some(c)), Choice::Merge),
            ((Some(a), Some(a), Some(c)), Choice::Theirs(Fate::Updated)),
            ((Some(a), None, Some(c)), Choice::Ours(Fate::LeftOut)),
            ((Some(a), Some(b), Some(c)), Choice::Merge),
            ((Some(a), Some(c), Some(c)), Choice::Merge),
        ];
        for ((base, ours, theirs), chosen) in cases {
            let said = format!("{base:?} {ours:?} {theirs:?}");
            assert_eq!(choose(base, ours, theirs), chosen, "{said}");
        }
    }

    #[test]
    fn each_fate_says_what_became_of_the_file() {
        let says = |fate: Fate| fate.said();
        assert_eq!(says(Fate::Kept), "yours, gdd writes it as before");
        let dropped = format!("yours, gdd {VERSION} no longer writes it");
        assert_eq!(says(Fate::Dropped), dropped);
        assert_eq!(says(Fate::Added), format!("added (new in gdd {VERSION})"));
        assert_eq!(says(Fate::Updated), "updated (you had not changed it)");
        assert_eq!(says(Fate::LeftOut), "left out: you removed it");
        assert_eq!(says(Fate::Merged(0)), "merged with your changes");
        assert_eq!(says(Fate::Merged(3)), "conflict, settle the marks (3)");
    }

    #[test]
    fn the_files_are_those_this_gdd_writes_then_those_only_the_older_one_wrote() {
        let theirs = ["gdd/a.toml", GENERATED, "docs/c.md"];
        let theirs = theirs.map(|path| (path.to_string(), String::new()));
        let listed = ["docs/b.md", GENERATED, "gdd/a.toml", "x.md", "docs/b.md"];
        let upgrade = Upgrade {
            root: Path::new("."),
            git: Git::new(Path::new(".")),
            env: &[],
            main: "main".to_string(),
            old: "0.0.0".to_string(),
            base: "HEAD".to_string(),
            listed: names(&listed),
            theirs: theirs.to_vec(),
        };
        let paths = ["gdd/a.toml", "docs/c.md", "docs/b.md", "x.md"];
        assert_eq!(upgrade.paths(), paths);
    }

    #[test]
    fn this_gdd_s_text_is_proposed_merged_with_the_project_s_on_a_branch_of_its_own() {
        let project = Project::new();
        let commands = project.read("gdd/commands.toml");
        let zones = project.read(ZONES);
        let models = project.read("gdd/models.toml");
        let generated = project.read(GENERATED);
        // What the older gdd wrote.
        project.replace(WORKFLOW, "actions/checkout@v4", "actions/checkout@v3");
        project.replace(BASICS, "language = \"en\"", "language = \"es\"");
        project.replace(BRIEF, "# Project Brief", "# The Brief");
        project.write("gdd/commands.toml", &format!("{commands}# older\n"));
        project.remove("gdd/models.toml");
        project.write("docs/old.md", "old\n");
        project.older("0.0.0", &["gdd/models.toml"], &["docs/old.md"]);
        // What the project made of it.
        project.replace(BASICS, "name = \"Notes\"", "name = \"Our notes\"");
        project.replace(BRIEF, "# The Brief", "# Our Brief");
        project.replace(ZONES, "Documentation", "Documentation and notes");
        project.remove("gdd/commands.toml");
        project.commit("ours");
        let main = project.git(&["rev-parse", "main"]);
        let said = project.upgrade().unwrap();
        let expected = format!(
            "gdd upgrade 0.0.0 to {VERSION}, on the branch gdd-upgrade:\n\
             \x20 gdd/project.toml: merged with your changes\n\
             \x20 gdd/commands.toml: left out: you removed it\n\
             \x20 gdd/rules.toml: yours, gdd writes it as before\n\
             \x20 gdd/zones.toml: yours, gdd writes it as before\n\
             \x20 gdd/models.toml: added (new in gdd {VERSION})\n\
             \x20 docs/brief.md: conflict, settle the marks (1)\n\
             \x20 .github/workflows/gdd.yml: updated (you had not changed it)\n\
             \x20 docs/old.md: yours, gdd {VERSION} no longer writes it\n\
             Conflicts: git switch gdd-upgrade, settle the marks, commit, then git switch main.\n\
             Review it: git diff main gdd-upgrade\n\
             Take it: git merge gdd-upgrade, git branch -d gdd-upgrade, then bin/gdd agents install"
        );
        assert_eq!(said, expected);
        let ours = project.on_branch(BASICS).unwrap();
        assert!(ours.contains("\nname = \"Our notes\"\n"), "{ours}");
        assert!(ours.contains("\nlanguage = \"en\""), "{ours}");
        assert_eq!(project.on_branch("gdd/commands.toml"), None);
        let edited = zones.replacen("Documentation", "Documentation and notes", 1);
        assert_eq!(project.on_branch(ZONES), Some(edited));
        assert_eq!(project.on_branch("gdd/models.toml"), Some(models));
        let brief = project.on_branch(BRIEF).unwrap();
        let marked = format!(
            "<<<<<<< yours\n# Our Brief · Notes\n=======\n# Project Brief · Notes\n>>>>>>> gdd \
             {VERSION}\n"
        );
        assert!(brief.starts_with(&marked), "{brief}");
        let workflow = project.on_branch(WORKFLOW).unwrap();
        assert!(workflow.contains("actions/checkout@v4"), "{workflow}");
        assert_eq!(project.on_branch("docs/old.md").as_deref(), Some("old\n"));
        // gdd/generated.toml is this gdd's, as gdd init wrote it for the same answers.
        assert_eq!(project.on_branch(GENERATED), Some(generated));
        // First what this gdd writes, pure; then the merge, without gdd/generated.toml.
        let range = format!("main..{BRANCH}");
        let subjects = project.git(&["log", "--format=%s", &range]);
        let upgrade = format!("gdd upgrade 0.0.0 to {VERSION}");
        let both = format!(
            "{upgrade}: merged with the project's changes\n{upgrade}: what gdd {VERSION} writes"
        );
        assert_eq!(subjects, both);
        let before = format!("{BRANCH}~2");
        assert_eq!(project.git(&["rev-parse", &before]), main);
        let first = format!("{BRANCH}~1");
        let zones_first = project.git(&["show", &format!("{first}:{ZONES}")]);
        assert_eq!(zones_first, zones.trim());
        let commands_first = project.git(&["show", &format!("{first}:gdd/commands.toml")]);
        assert_eq!(commands_first, commands.trim());
        let merged = project.git(&["diff", "--name-only", &first, BRANCH]);
        let paths = "docs/brief.md\ngdd/commands.toml\ngdd/project.toml\ngdd/zones.toml";
        assert_eq!(merged, paths);
        // The main checkout stays on main, as it was.
        assert_eq!(project.git(&["rev-parse", "HEAD"]), main);
        assert_eq!(project.git(&["branch", "--show-current"]), "main");
        assert_eq!(project.git(&["status", "--porcelain"]), "");
        assert!(project.left_nothing());
    }

    #[test]
    fn a_clean_merge_names_no_conflict_and_the_next_upgrade_starts_from_the_pure_text() {
        let project = Project::new();
        project.replace(WORKFLOW, "actions/checkout@v4", "actions/checkout@v3");
        project.older("0.0.0", &[], &[]);
        project.replace(WORKFLOW, "runs-on: ubuntu-latest", "runs-on: ubuntu-24.04");
        project.commit("ours");
        let said = project.upgrade().unwrap();
        let kept = "yours, gdd writes it as before";
        let expected = format!(
            "gdd upgrade 0.0.0 to {VERSION}, on the branch gdd-upgrade:\n\
             \x20 gdd/project.toml: {kept}\n\
             \x20 gdd/commands.toml: {kept}\n\
             \x20 gdd/rules.toml: {kept}\n\
             \x20 gdd/zones.toml: {kept}\n\
             \x20 gdd/models.toml: {kept}\n\
             \x20 docs/brief.md: {kept}\n\
             \x20 .github/workflows/gdd.yml: merged with your changes\n\
             Review it: git diff main gdd-upgrade\n\
             Take it: git merge gdd-upgrade, git branch -d gdd-upgrade, then bin/gdd agents install"
        );
        assert_eq!(said, expected);
        let workflow = project.on_branch(WORKFLOW).unwrap();
        assert!(workflow.contains("actions/checkout@v4\n"), "{workflow}");
        assert!(workflow.contains("runs-on: ubuntu-24.04\n"), "{workflow}");
        // Taken, the next upgrade starts from what this gdd wrote: nothing to propose.
        project.git(&["merge", "-q", "--no-edit", BRANCH]);
        project.git(&["branch", "-q", "-d", BRANCH]);
        let base = project.git(&["log", "-1", "--format=%s", "--", GENERATED]);
        let pure = format!("gdd upgrade 0.0.0 to {VERSION}: what gdd {VERSION} writes");
        assert_eq!(base, pure);
        let nothing = format!("Already up to date with gdd {VERSION}: nothing to propose.");
        assert_eq!(project.upgrade(), Ok(nothing));
        assert!(!project.has_branch());
    }

    #[test]
    fn a_file_nobody_touched_takes_the_new_text_and_the_merge_commit_is_still_made() {
        let project = Project::new();
        project.replace(WORKFLOW, "actions/checkout@v4", "actions/checkout@v3");
        project.older("0.0.0", &[], &[]);
        let said = project.upgrade().unwrap();
        let updated = "  .github/workflows/gdd.yml: updated (you had not changed it)\n";
        assert!(said.contains(updated), "{said}");
        assert!(!said.contains("Conflicts"), "{said}");
        let workflow = project.on_branch(WORKFLOW).unwrap();
        assert!(workflow.contains("actions/checkout@v4"), "{workflow}");
        assert!(project.read(WORKFLOW).contains("actions/checkout@v3"));
        let subjects = project.git(&["log", "--format=%s", &format!("main..{BRANCH}")]);
        assert_eq!(subjects.lines().count(), 2, "{subjects}");
        assert!(subjects.starts_with("gdd upgrade 0.0.0 to "));
        let first = format!("{BRANCH}~1");
        assert_eq!(project.git(&["diff", "--name-only", &first, BRANCH]), "");
        assert!(project.left_nothing());
    }

    #[test]
    fn nothing_is_proposed_when_this_gdd_writes_what_the_older_one_did() {
        let project = Project::new();
        // The project's own changes are no reason to propose anything.
        project.replace(ZONES, "Documentation", "Docs");
        project.remove(BRIEF);
        project.commit("ours");
        let nothing = format!("Already up to date with gdd {VERSION}: nothing to propose.");
        assert_eq!(project.upgrade(), Ok(nothing));
        assert!(!project.has_branch());
        assert!(!project.root.join(SCRATCH).exists());
        assert!(project.left_nothing());
    }

    #[test]
    fn a_file_the_older_gdd_did_not_list_is_merged_against_an_empty_base() {
        let project = Project::new();
        // The older gdd did not write the models file, which the project already had.
        project.older("0.0.0", &["gdd/models.toml"], &[]);
        project.replace("gdd/models.toml", "# Models · GDD", "# Our models");
        project.commit("ours");
        let said = project.upgrade().unwrap();
        let conflict = "\n  gdd/models.toml: conflict, settle the marks (1)\n";
        assert!(said.contains(conflict), "{said}");
        let switch = "\nConflicts: git switch gdd-upgrade, settle the marks, commit, then git \
                      switch main.\n";
        assert!(said.contains(switch), "{said}");
        let models = project.on_branch("gdd/models.toml").unwrap();
        let ours = "<<<<<<< yours\n# Our models";
        assert!(models.starts_with(ours), "{models}");
        let written = format!("# Models · GDD (written by gdd init {VERSION})");
        let theirs = format!("=======\n{written}\n>>>>>>> ");
        assert!(models.contains(&theirs), "{models}");
    }

    #[test]
    fn the_checks_come_in_order_and_write_nothing() {
        let project = Project::new();
        project.replace(WORKFLOW, "actions/checkout@v4", "actions/checkout@v3");
        project.older("0.0.0", &[], &[]);
        for role in ["builder-build", "builder-test", "reviewer"] {
            let refused = upgrade(&project.root, Some(role), &env());
            assert_eq!(refused, Err(NOT_FOR_AGENTS.to_string()), "{role}");
        }
        let said = "Builders and Reviewers cannot run gdd upgrade: a person runs it, then \
                    reviews and merges its branch";
        assert_eq!(NOT_FOR_AGENTS, said);
        // The record first, even with uncommitted changes.
        let generated = project.read(GENERATED);
        project.remove(GENERATED);
        assert_eq!(project.upgrade(), Err(MISSING.to_string()));
        let missing = "gdd/generated.toml is missing: this project was set up before gdd \
                       recorded what it writes, so gdd upgrade has no base to merge from";
        assert_eq!(MISSING, missing);
        let (without, _) = generated.split_once("\n[answers]\n").unwrap();
        project.write(GENERATED, without);
        assert_eq!(project.upgrade(), Err(NO_ANSWERS.to_string()));
        let none = "gdd/generated.toml has no [answers]: it was written before gdd recorded \
                    them, add them as gdd init writes them now";
        assert_eq!(NO_ANSWERS, none);
        project.write(GENERATED, &generated);
        // Then the main branch the rules name, for a person or the Orchestrator alike.
        project.git(&["checkout", "-q", "-b", "other"]);
        let runs_on = |main: &str| -> Result<String, String> {
            let switch = format!("git switch {main}, then run it again");
            Err(format!("gdd upgrade runs on {main}: {switch}"))
        };
        assert_eq!(project.upgrade(), runs_on("main"));
        let orchestrator = upgrade(&project.root, Some("orchestrator"), &env());
        assert_eq!(orchestrator, runs_on("main"));
        project.git(&["checkout", "-q", "--detach", "main"]);
        assert_eq!(project.upgrade(), runs_on("main"));
        project.git(&["checkout", "-q", "main"]);
        let rules = project.read(guard::RULES);
        let (main, trunk) = ("main_branch = \"main\"", "main_branch = \"trunk\"");
        project.replace(guard::RULES, main, trunk);
        assert_eq!(project.upgrade(), runs_on("trunk"));
        project.write(guard::RULES, &rules);
        // Then the changes not committed; files git does not track do not count.
        project.replace(ZONES, "Documentation", "Docs");
        assert_eq!(project.upgrade(), Err(UNCOMMITTED.to_string()));
        assert_eq!(UNCOMMITTED, "commit or stash your changes first");
        project.git(&["checkout", "-q", "--", ZONES]);
        // Then the branch of an upgrade already there.
        project.git(&["branch", BRANCH]);
        assert_eq!(project.upgrade(), Err(BRANCH_THERE.to_string()));
        let there = "the branch gdd-upgrade is already there: merge it or delete it (git branch \
                     -D gdd-upgrade) first";
        assert_eq!(BRANCH_THERE, there);
        project.git(&["branch", "-q", "-D", BRANCH]);
        assert!(!project.root.join(SCRATCH).exists());
        assert!(project.left_nothing());
        project.write("notes.txt", "mine\n");
        assert!(project.upgrade().is_ok());
        assert!(project.has_branch());
    }

    #[test]
    fn a_record_that_cannot_be_read_or_that_no_commit_holds_is_refused() {
        let project = Project::new();
        project.write(GENERATED, "files = [");
        let said = project.upgrade().unwrap_err();
        assert!(said.starts_with("gdd/generated.toml: "), "{said}");
        project.remove(GENERATED);
        fs::create_dir(project.root.join(GENERATED)).unwrap();
        let said = project.upgrade().unwrap_err();
        let unreadable = "cannot read gdd/generated.toml: ";
        assert!(said.starts_with(unreadable), "{said}");
        fs::remove_dir(project.root.join(GENERATED)).unwrap();
        project.git(&["checkout", "-q", "--", GENERATED]);
        project.write(guard::RULES, "main_branch = [\n");
        let said = project.upgrade().unwrap_err();
        assert!(said.starts_with("gdd/rules.toml: "), "{said}");
        project.git(&["checkout", "-q", "--", guard::RULES]);
        // A repository where no commit ever held it.
        fs::remove_dir_all(project.root.join(".git")).unwrap();
        project.git(&["init", "-q", "-b", "main"]);
        project.git(&["add", "-A"]);
        project.git(&["reset", "-q", "--", GENERATED]);
        project.git(&["commit", "-q", "-m", "without the record"]);
        assert_eq!(project.upgrade(), Err(UNCOMMITTED_RECORD.to_string()));
        assert!(!project.has_branch());
    }

    #[test]
    fn answers_that_cannot_be_rendered_say_why() {
        let project = Project::new();
        let mut generated: toml::Table = toml::from_str(&project.read(GENERATED)).unwrap();
        let answers = generated["answers"].as_table_mut().unwrap();
        let cobol = toml::Value::Array(vec!["cobol".into()]);
        answers.insert("stacks".to_string(), cobol);
        project.write(GENERATED, &toml::to_string(&generated).unwrap());
        project.commit("cobol");
        let said = project.upgrade().unwrap_err();
        assert!(said.starts_with("unknown stack cobol (known: "), "{said}");
        assert!(!project.has_branch());
    }

    #[test]
    fn a_failure_in_the_worktree_leaves_no_branch_and_no_worktree() {
        let project = Project::new();
        project.replace(WORKFLOW, "actions/checkout@v4", "actions/checkout@v3");
        project.older("0.0.0", &[], &[]);
        let main = project.git(&["rev-parse", "main"]);
        // A hook that refuses every commit.
        project.before_each_commit("exit 1");
        let said = project.upgrade().unwrap_err();
        let commit = format!("git commit -q -m gdd upgrade 0.0.0 to {VERSION}: what ");
        let start = format!("{NO_BRANCH}: {commit}");
        assert!(said.starts_with(&start), "{said}");
        assert!(!project.has_branch());
        assert!(project.left_nothing());
        assert_eq!(project.git(&["rev-parse", "HEAD"]), main);
        assert_eq!(project.git(&["status", "--porcelain"]), "");
    }

    #[test]
    fn what_the_project_put_where_this_gdd_writes_a_file_is_never_taken() {
        let not_own = "not a file of its own (a link, a folder or a file with other names)";
        let folder: fn(&Project) = |project| {
            project.remove(BRIEF);
            project.write("docs/brief.md/goals.md", "goals\n");
        };
        let link: fn(&Project) = |project| {
            project.write("docs/goals.md", "goals\n");
            project.remove(BRIEF);
            symlink("goals.md", project.root.join(BRIEF)).unwrap();
        };
        let file_on_the_way: fn(&Project) = |project| {
            fs::remove_dir_all(project.root.join(".github")).unwrap();
            project.write(".github", "ours\n");
        };
        let cases = [
            (folder, BRIEF, not_own),
            (link, BRIEF, not_own),
            (file_on_the_way, WORKFLOW, ".github is not a folder"),
        ];
        for (ours, path, why) in cases {
            let project = Project::new();
            // This gdd writes another workflow: there is something to propose.
            project.replace(WORKFLOW, "actions/checkout@v4", "actions/checkout@v3");
            project.older("0.0.0", &[], &[]);
            ours(&project);
            project.commit("ours");
            let main = project.git(&["rev-parse", "main"]);
            let tracked = project.git(&["ls-tree", "-r", "main"]);
            let said = project.upgrade().unwrap_err();
            let next = "move what is in the way, commit, then run gdd upgrade again";
            let refused = format!(
                "{NO_BRANCH}: {path} on main: {why}, where gdd {VERSION} writes a file: {next}"
            );
            assert_eq!(said, refused);
            assert!(!project.has_branch(), "{why}");
            assert!(project.left_nothing(), "{why}");
            assert_eq!(project.git(&["rev-parse", "HEAD"]), main, "{why}");
            assert_eq!(project.git(&["ls-tree", "-r", "main"]), tracked, "{why}");
            assert_eq!(project.git(&["status", "--porcelain"]), "", "{why}");
        }
    }

    #[test]
    fn what_is_in_the_way_of_the_worktree_is_left_alone() {
        let project = Project::new();
        project.replace(WORKFLOW, "actions/checkout@v4", "actions/checkout@v3");
        project.older("0.0.0", &[], &[]);
        let worktree = project.worktrees().join(BRANCH);
        fs::create_dir_all(&worktree).unwrap();
        fs::write(worktree.join("mine.txt"), "mine\n").unwrap();
        let said = project.upgrade().unwrap_err();
        let start = format!("{NO_BRANCH}: git worktree add -q -- ");
        assert!(said.starts_with(&start), "{said}");
        let mine = fs::read_to_string(worktree.join("mine.txt")).unwrap();
        assert_eq!(mine, "mine\n");
        assert!(!project.has_branch());
    }

    #[test]
    fn the_folder_of_the_worktrees_this_run_made_goes_with_its_worktree() {
        let project = Project::new();
        project.replace(WORKFLOW, "actions/checkout@v4", "actions/checkout@v3");
        project.older("0.0.0", &[], &[]);
        assert!(!project.worktrees().exists());
        project.upgrade().unwrap();
        assert!(project.has_branch());
        assert!(!project.worktrees().exists());
        assert!(project.left_nothing());
    }

    #[test]
    fn a_folder_of_the_worktrees_that_was_there_stays() {
        let project = Project::new();
        project.replace(WORKFLOW, "actions/checkout@v4", "actions/checkout@v3");
        project.older("0.0.0", &[], &[]);
        fs::create_dir(project.worktrees()).unwrap();
        project.upgrade().unwrap();
        assert!(project.has_branch());
        let left = fs::read_dir(project.worktrees()).unwrap();
        assert_eq!(left.count(), 0);
    }

    #[test]
    fn a_folder_of_the_worktrees_that_got_something_else_meanwhile_stays() {
        let project = Project::new();
        project.replace(WORKFLOW, "actions/checkout@v4", "actions/checkout@v3");
        project.older("0.0.0", &[], &[]);
        // Something else comes into the folder while the worktree is there.
        project.before_each_commit("echo mine > ../mine.txt");
        project.upgrade().unwrap();
        assert!(project.has_branch());
        assert!(!project.worktrees().join(BRANCH).exists());
        let mine = fs::read_to_string(project.worktrees().join("mine.txt")).unwrap();
        assert_eq!(mine, "mine\n");
    }

    #[test]
    fn a_folder_goes_only_when_empty_and_what_cannot_be_removed_says_which() {
        let folder = Folder::new();
        let empty = folder.0.join("empty");
        fs::create_dir(&empty).unwrap();
        tidy(&empty).unwrap();
        assert!(!empty.exists());
        // Nothing there is left as it is.
        tidy(&empty).unwrap();
        folder.write("full/mine.txt", "mine\n");
        tidy(&folder.0.join("full")).unwrap();
        assert!(folder.0.join("full/mine.txt").exists());
        // A file or a link is no folder: it stays, and says why.
        let link = folder.0.join("link");
        symlink(folder.0.join("full"), &link).unwrap();
        let file = folder.0.join("full/mine.txt");
        for at in [&file, &link] {
            let said = tidy(at).unwrap_err();
            let why = format!("cannot remove {}: Not a directory", at.display());
            assert!(said.starts_with(&why), "{said}");
            assert!(fs::symlink_metadata(at).is_ok(), "{said}");
        }
    }

    #[test]
    fn what_cannot_be_read_written_or_removed_says_which_file() {
        let folder = Folder::new();
        let long = "x".repeat(300);
        let too_long = |verb: &str| format!("cannot {verb} {long}: File name too long");
        let said = write(&folder.0, &long, b"x").unwrap_err();
        assert!(said.starts_with(&too_long("write")), "{said}");
        let said = read(&folder.0, &long).unwrap_err();
        assert!(said.starts_with(&too_long("read")), "{said}");
        let said = remove(&folder.0, &long).unwrap_err();
        assert!(said.starts_with(&too_long("remove")), "{said}");
        // Under a file where a folder goes.
        folder.write("file", "");
        let under = folder.0.join("file");
        let not_a_folder = |verb: &str| format!("cannot {verb} a: Not a directory");
        let said = write(&under, "a", b"x").unwrap_err();
        assert!(said.starts_with(&not_a_folder("write")), "{said}");
        let said = read(&under, "a").unwrap_err();
        assert!(said.starts_with(&not_a_folder("read")), "{said}");
        let said = remove(&under, "a").unwrap_err();
        assert!(said.starts_with(&not_a_folder("remove")), "{said}");
        // What can be done is done.
        write(&folder.0, "deep/a", b"x").unwrap();
        assert_eq!(read(&folder.0, "deep/a").unwrap(), Some(b"x".to_vec()));
        remove(&folder.0, "deep/a").unwrap();
        assert_eq!(read(&folder.0, "deep/a").unwrap(), None);
    }
}
