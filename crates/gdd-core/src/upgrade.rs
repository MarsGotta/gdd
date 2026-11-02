//! `tminus upgrade`: what this tminus writes of the files `tminus init` wrote once, merged with
//! what the project made of them, proposed on the branch `tminus-upgrade` for a person to review
//! and merge. Those files are the project's: nothing on the main branch changes, and nothing is
//! pushed.
//!
//! Each file is merged three ways, as Copier updates a project from its template: the base is
//! what the older tminus wrote, kept in the last commit that touched `tminus/generated.toml`;
//! ours, the file on the main branch; theirs, what this tminus renders from the answers tminus
//! init recorded (see [`init::render_once`]). The merge is git's own (`git merge-file`), through
//! files in a fresh folder under `.tminus/upgrade/` of the state root, removed afterwards. The
//! branch gets two commits: what this tminus writes, pure, with the new `tminus/generated.toml`
//! (the base of the next upgrade), then the merge with the project's changes. Both are made in a
//! worktree of their own, in the folder of the changes' worktrees, which goes whatever happens
//! (and that folder with it when the run made it and nothing else came into it); the branch is
//! made last, on the second commit, so a run that fails leaves none. Where this tminus writes a
//! file and the project has something else (a folder, a link), the upgrade stops before writing
//! anything.
//!
//! A project set up before the rename (see [`crate::layout`]) is moved to the new names by the
//! same command, in two runs. The first proposes the move on the same branch: the first commit,
//! what this tminus writes, pure, in the new names; the second, the merge read from `gdd/` and
//! written to `tminus/` (see [`layout::moved_names`]), the rest of `gdd/` moved as it is, the
//! agents' commands in the new names and `.tminus/` in `.gitignore`. Once the branch is merged,
//! the state cannot ride a branch: the second run renames `.gdd/` to `.tminus/` as it is. Neither
//! runs while a change is in flight.

use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::Deserialize;

use crate::VERSION;
use crate::agents::models::{self, Models};
use crate::agents::sheets::{self, Sheet};
use crate::agents::{Limits, files, render};
use crate::config;
use crate::gates::approve;
use crate::git::{self, Git};
use crate::guard::{self, Settings};
use crate::init::{self, Answers, GENERATED};
use crate::layout::{self, Layout};
use crate::recipe::{queue, worktree};
use crate::stacks;

/// The branch the upgrade is proposed on.
const BRANCH: &str = "tminus-upgrade";

/// The folder of the merges' files, from the state root, in the new names: a fresh folder in it
/// for each run.
const SCRATCH: &str = ".tminus/upgrade";

/// The project file, from the project root, in the new names.
const PROJECT: &str = "tminus/project.toml";

/// What git leaves out, from the project root.
const GITIGNORE: &str = ".gitignore";

/// What a move says of the agents' commands.
const COMMANDS_MOVED: &str = "  the agents' commands: tminus-<command> in place of gdd-<command>";

/// What the run that moves the state says.
const STATE_MOVED: &str = "Moved .gdd/ to .tminus/: the approvals, records and log of the \
                           project as they were.\nThe red zones now name tminus/: approve them \
                           again with tminus gate zones ok --by human";

/// How the conflict marks name the project's side.
const YOURS: &str = "yours";

/// Why a Builder or a Reviewer cannot upgrade.
const NOT_FOR_AGENTS: &str = "Builders and Reviewers cannot run tminus upgrade: a person runs \
                              it, then reviews and merges its branch";

/// Why a project without its generated file cannot be upgraded, after the file.
const MISSING: &str = "is missing: this project was set up before tminus recorded what it \
                       writes, so tminus upgrade has no base to merge from";

/// Why a project whose generated file has no answers cannot be upgraded, after the file.
const NO_ANSWERS: &str = "has no [answers]: it was written before tminus recorded them, add \
                          them as tminus init writes them now";

/// Why a project whose generated file no commit holds cannot be upgraded, after the file.
const UNCOMMITTED_RECORD: &str = "is in no commit: commit it first, tminus upgrade merges from \
                                  the commit that wrote it";

/// Why a project with uncommitted changes cannot be upgraded.
const UNCOMMITTED: &str = "commit or stash your changes first";

/// What a run that failed once it started on its branch says, before why.
const NO_BRANCH: &str = "tminus upgrade made no branch and main is as it was";

/// An environment: its variables, with their values.
type Env = [(String, String)];

/// What the generated file says: the version of the tminus that wrote it, the files that tminus
/// writes once and, when it recorded them, what tminus init was asked.
#[derive(Debug, Deserialize)]
struct Generated {
    version: String,
    files: Vec<String>,
    answers: Option<Answers>,
}

/// What becomes of a file of the upgrade on its branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fate {
    /// This tminus writes it as the older one did: the project's file stays.
    Kept,
    /// This tminus no longer writes it: the project's file stays.
    Dropped,
    /// New in this tminus, and the project has none: this tminus's.
    Added,
    /// The project had not changed it: this tminus's.
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
            Fate::Kept => "yours, tminus writes it as before".to_string(),
            Fate::Dropped => format!("yours, tminus {VERSION} no longer writes it"),
            Fate::Added => format!("added (new in tminus {VERSION})"),
            Fate::Updated => "updated (you had not changed it)".to_string(),
            Fate::LeftOut => "left out: you removed it".to_string(),
            Fate::Merged(0) => "merged with your changes".to_string(),
            Fate::Merged(conflicts) => format!("conflict, settle the marks ({conflicts})"),
        }
    }
}

/// Which text a file of the upgrade takes, from what the older tminus wrote, what the project
/// has and what this tminus writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Choice {
    /// The project's, with this fate.
    Ours(Fate),
    /// This tminus's, with this fate.
    Theirs(Fate),
    /// The three merged, against an empty base when the older tminus wrote none.
    Merge,
}

/// Which text a file takes from what the older tminus wrote of it (`base`), what the project has
/// (`ours`) and what this tminus writes (`theirs`), each `None` where there is no file.
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

/// What the main branch may have at a path the upgrade writes or removes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Room {
    /// A file, which the upgrade merges, writes anew or removes, or nothing.
    File,
    /// Nothing: a move puts a file there under a name the project did not use before, and a file
    /// of the project's own there would be lost under it.
    Nothing,
}

/// A file of the upgrade, settled.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Settled {
    /// Where it is, from the project root.
    path: String,
    /// What becomes of it.
    fate: Fate,
    /// Its text at its path after the first commit: this tminus's when it writes it, else what
    /// the main branch has there; `None` where there is no file.
    first: Option<Vec<u8>>,
    /// Its text after the second commit; `None` where there is no file.
    last: Option<Vec<u8>>,
    /// What the upgrade says of the limits of the project it kept in it, a line each (see
    /// [`kept_limits`]).
    kept: Vec<String>,
}

/// `tminus upgrade` in the project at `root`, the folder of the main checkout (and so its state
/// root, see [`crate::state::root`]), for the role `role` (`None` for a person), with git and the
/// agents on the `PATH` of `env`, the environment tminus runs in (see the module). A Builder or a
/// Reviewer is refused first. A project whose files took the new names and whose state did not
/// yet (see [`layout::state_to_move`]) gets its state moved (see [`move_state`]). Otherwise a
/// project whose generated file is missing or has no answers, one whose checkout is not on the
/// main branch of its rules file, one with tracked changes not committed, one where the upgrade
/// branch is already there and, in the old layout, one with changes in flight are refused, in
/// that order, with nothing written; the record, the rules file and the folder of the merges are
/// named as the project names them (see [`Layout::at`]). A project of the old layout always gets
/// the move to the new names, its state left in `.gdd/` for the second run (see [`keep_state`]).
/// What it says: what became of each file and how to review and take the branch, or that there
/// is nothing to propose; without a line break at the end.
pub fn upgrade(root: &Path, role: Option<&str>, env: &Env) -> Result<String, String> {
    approve::may_record(role).map_err(|_| NOT_FOR_AGENTS.to_string())?;
    if layout::state_to_move(root)? {
        return move_state(root);
    }
    let from = Layout::at(root);
    let record = from.path(GENERATED);
    let generated = recorded(root, &record)?;
    let no_answers = || format!("{record} {NO_ANSWERS}");
    let answers = generated.answers.ok_or_else(no_answers)?;
    let rules_file = from.path(guard::RULES);
    let rules = read(root, &rules_file)?.unwrap_or_default();
    let rules = String::from_utf8_lossy(&rules).into_owned();
    let unreadable = |e: config::Error| format!("{rules_file}: {e}");
    let settings = Settings::from_toml(&rules).map_err(unreadable)?;
    let main = settings.main_branch;
    let git = Git::in_env(root, env);
    check(&git, &main)?;
    if from == Layout::Gdd {
        none_in_flight(root)?;
    }
    let base = git.last_commit_of(&record).map_err(said)?;
    let uncommitted = || format!("{record} {UNCOMMITTED_RECORD}");
    let base = base.ok_or_else(uncommitted)?;
    let rendered = init::render_once(&answers, env, Layout::Tminus);
    let theirs = rendered.map_err(unrendered)?;
    let moving = match from {
        Layout::Gdd => Some(Move::of(root, &git)?),
        Layout::Tminus => None,
    };
    let upgrade = Upgrade {
        root,
        git,
        env,
        moving,
        main,
        old: generated.version,
        base,
        listed: generated.files,
        theirs,
    };
    let files = upgrade.settle()?;
    let kept = files.iter().all(|file| file.fate == Fate::Kept);
    if kept && upgrade.moving.is_none() {
        let nothing = format!("Already up to date with tminus {VERSION}: nothing to propose.");
        return Ok(nothing);
    }
    let folder = worktree::folder(root, &rules).map_err(unreadable)?;
    let path = worktree::path(&folder, BRANCH)?;
    let proposed = upgrade.propose(&folder, &path, &files);
    let more = proposed.map_err(|why| format!("{NO_BRANCH}: {why}"))?;
    if upgrade.moving.is_some() {
        keep_state(root)?;
    }
    Ok(upgrade.said(&files, &more))
}

/// Leaves the state folder of the old layout, `.gdd/`, at the state root `root` of a project
/// whose move is proposed, made empty when the project has none yet: once the branch is merged,
/// every other command names the run that moves it (see [`layout::check`]), which then always
/// has a state to move to `.tminus/`.
fn keep_state(root: &Path) -> Result<(), String> {
    let state = root.join(".gdd");
    if fs::symlink_metadata(&state).is_ok() {
        return Ok(());
    }
    fs::create_dir(&state).map_err(|e| format!("cannot write {}: {e}", state.display()))
}

/// The second run of a move: `.gdd/` of the state root `root` renamed `.tminus/` in one step,
/// with the approvals, records and log of the project as they are; nothing else is written, and
/// git is not asked. Refused when `.gdd` is not a folder of its own (a link, a file), then while
/// a change is in flight (see [`none_in_flight`]). What it says: what moved and that the red
/// zones, which now name `tminus/`, need their approval again.
fn move_state(root: &Path) -> Result<String, String> {
    let old = root.join(".gdd");
    if !fs::symlink_metadata(&old).is_ok_and(|meta| meta.is_dir()) {
        let shown = old.display();
        return Err(format!("{shown} is not a folder: move it by hand"));
    }
    none_in_flight(root)?;
    let new = root.join(".tminus");
    let moved = fs::rename(&old, &new);
    moved.map_err(|e| format!("cannot move {} to {}: {e}", old.display(), new.display()))?;
    Ok(STATE_MOVED.to_string())
}

/// `Ok` unless a change of the project whose state is in `.gdd/` of the state root `root` is in
/// flight (see [`queue::in_flight_all`]): then why nothing moves, naming each with its state.
/// Their worktrees hold the old names, and a change waiting at G4 resumes against main.
fn none_in_flight(root: &Path) -> Result<(), String> {
    let flights = queue::in_flight_all(root, Layout::Gdd)?;
    if flights.is_empty() {
        return Ok(());
    }
    let named: Vec<String> = flights
        .iter()
        .map(|(id, state)| format!("{id} ({state})"))
        .collect();
    Err(format!(
        "tminus upgrade moves nothing while changes are in flight: {}. Finish, close or reject \
         them, then run it again.",
        named.join(", ")
    ))
}

/// The checks on the checkout of `git` before an upgrade, after its record: on the branch `main`,
/// with no tracked change uncommitted, and without the upgrade branch.
fn check(git: &Git, main: &str) -> Result<(), String> {
    if git.current_branch().map_err(said)? != main {
        let switch = format!("git switch {main}, then run it again");
        return Err(format!("tminus upgrade runs on {main}: {switch}"));
    }
    if git.has_tracked_changes().map_err(said)? {
        return Err(UNCOMMITTED.to_string());
    }
    if git.exists(&format!("refs/heads/{BRANCH}")).map_err(said)? {
        return Err(branch_there(BRANCH));
    }
    Ok(())
}

/// Why a project whose upgrade branch `branch` is there cannot be upgraded again.
fn branch_there(branch: &str) -> String {
    format!(
        "the branch {branch} is already there: merge it or delete it (git branch -D {branch}) \
         first"
    )
}

/// What a move to the new names writes besides the files of the record, as the main branch of
/// the project names it: the agents' commands of both names, with the instructions of its stacks.
struct Move {
    /// Every agent's commands in the old names, wherever the project may have them.
    old_commands: Vec<String>,
    /// The commands of its agents in the new names, as [`render::install`] writes them.
    commands: Vec<String>,
    /// The agents of its models file, for the new names.
    agents: Vec<Sheet>,
    /// Its stacks.
    stacks: Vec<String>,
}

impl Move {
    /// The move of the project at `root`, of the old layout, read from the commit `git` has
    /// checked out there, the main branch: its models file and its project file, as `tminus
    /// agents install` reads them. Either one that cannot be read is an error that names it.
    fn of(root: &Path, git: &Git) -> Result<Move, String> {
        let models_file = Layout::Gdd.path(models::PATH);
        let models = Models::from_toml(&text_at(git, &models_file)?);
        let models = models.map_err(|e| format!("{models_file}: {e}"))?;
        let agents = render::agents_in(root, &models, &[])?;
        let agents: Vec<Sheet> = agents
            .into_iter()
            .map(|sheet| sheet.in_layout(Layout::Tminus))
            .collect();
        let project_file = Layout::Gdd.path(PROJECT);
        let stacks = stacks::of_project(&text_at(git, &project_file)?);
        let stacks = stacks.map_err(|e| format!("{project_file}: {e}"))?;
        let old = |name: &str| -> Result<Sheet, String> {
            let sheet = sheets::sheet(name).map_err(|e| e.to_string())?;
            Ok(sheet.in_layout(Layout::Gdd))
        };
        let every = sheets::NAMES.split(", ").map(old);
        let every = every.collect::<Result<Vec<Sheet>, String>>()?;
        Ok(Move {
            old_commands: render::paths(&every, &stacks)?,
            commands: render::paths(&agents, &stacks)?,
            agents,
            stacks,
        })
    }

    /// What the second commit of the move does in the worktree at `work`, where `git` runs,
    /// besides the files of the record: the old generated file goes, the `moved` files take their
    /// new names, the agents' commands of the old names give way to those of the new ones, and
    /// `.gitignore` leaves out `.tminus/` too. What the upgrade says of it, a line each.
    fn rest(
        &self,
        work: &Path,
        git: &Git,
        moved: &[(String, String)],
    ) -> Result<Vec<String>, String> {
        remove(work, &Layout::Gdd.path(GENERATED))?;
        let mut lines = Vec::new();
        for (from, to) in moved {
            move_file(work, git, from, to)?;
            lines.push(format!("  {to}: moved as it was"));
        }
        for command in &self.old_commands {
            remove(work, command)?;
        }
        render::install(work, &self.agents, &self.stacks)?;
        if ignored(work)? {
            lines.push(format!("  {GITIGNORE}: .tminus/ added"));
        }
        lines.push(COMMANDS_MOVED.to_string());
        Ok(lines)
    }
}

/// What an upgrade works with.
struct Upgrade<'a> {
    /// The folder of the project in the main checkout, and its state root.
    root: &'a Path,
    /// Git there, in the environment tminus runs in.
    git: Git,
    /// The environment tminus runs in.
    env: &'a Env,
    /// What moves a project of the old layout to the new names besides its files; `None` for a
    /// project in them already.
    moving: Option<Move>,
    /// The main branch.
    main: String,
    /// The version of the tminus that wrote the files last.
    old: String,
    /// The commit with what it wrote: the last that touched the generated file.
    base: String,
    /// The files it wrote once, as the project named them then.
    listed: Vec<String>,
    /// What this tminus writes once, and its generated file: their paths and texts, in order, in
    /// the new names.
    theirs: Vec<(String, String)>,
}

impl Upgrade<'_> {
    /// The layout of the project on its main branch: the old one when it moves.
    fn on_main(&self) -> Layout {
        if self.moving.is_some() {
            Layout::Gdd
        } else {
            Layout::Tminus
        }
    }

    /// The files of the upgrade, settled, merged in a fresh folder under the folder of the
    /// merges of the state root, which goes whatever happened.
    fn settle(&self) -> Result<Vec<Settled>, String> {
        let scratch = format!("{}/{}", self.on_main().path(SCRATCH), std::process::id());
        remove(self.root, &scratch)?;
        let paths = self.paths();
        let settle = |path: &String| self.file(path, &scratch);
        let settled: Result<Vec<Settled>, String> = paths.iter().map(settle).collect();
        let removed = remove(self.root, &scratch);
        let settled = settled?;
        removed?;
        Ok(settled)
    }

    /// The paths of the upgrade, in the new names: those this tminus writes once, in its order,
    /// then those only the older one wrote, each once. Never the generated file: this tminus's is
    /// taken as it is.
    fn paths(&self) -> Vec<String> {
        let theirs = self.theirs.iter().map(|(path, _)| path.clone());
        let listed = self.listed.iter().map(|path| self.renamed(path));
        let all: Vec<String> = theirs.chain(listed).collect();
        stacks::once(all.iter().filter(|path| path.as_str() != GENERATED))
    }

    /// `path`, as the older tminus listed it, in the new names when the project moves to them.
    fn renamed(&self, path: &str) -> String {
        match self.moving {
            Some(_) => layout::new_path(path),
            None => path.to_string(),
        }
    }

    /// The file `path` settled, merged in the folder `scratch` of the state root when it must be:
    /// read where the main branch has it (see [`Upgrade::on_main`]), written at `path`.
    fn file(&self, path: &str, scratch: &str) -> Result<Settled, String> {
        let old = self.on_main().path(path);
        let theirs = self.theirs.iter().find(|(at, _)| at == path);
        let theirs = theirs.map(|(_, text)| text.as_bytes().to_vec());
        // What the older tminus did not list it did not write, whatever its commit holds there.
        let base = if self.listed.contains(&old) {
            self.git.file_at(&self.base, &old).map_err(said)?
        } else {
            None
        };
        let ours = self.git.file_at("HEAD", &old).map_err(said)?;
        let first = theirs
            .clone()
            .or_else(|| ours.clone().filter(|_| old == path));
        let [base, ours, theirs] = self.compared([base, ours, theirs]);
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
        // Only the rules file has `[limits]`, and a text with conflict marks is no TOML: in any
        // other file there is nothing to keep.
        let (last, kept) = match last {
            Some(text) => {
                let ours = ours.as_deref().unwrap_or_default();
                let kept = kept_limits(ours, text);
                let (text, said) = kept.map_err(|e| format!("{NO_BRANCH}: {path}: {e}"))?;
                let said = said.iter().map(|line| format!("  {path}: {line}"));
                (Some(text), said.collect())
            }
            None => (None, Vec::new()),
        };
        Ok(Settled {
            path: path.to_string(),
            fate,
            first,
            last,
            kept,
        })
    }

    /// The texts of a file that the older tminus wrote, the project has and this tminus writes,
    /// in that order, as they are compared and merged: when the project moves, the first two
    /// with their folders and agents' commands in the new names (see [`layout::moved_names`]),
    /// and this tminus's with the variables in the names the project's own code reads (see
    /// [`layout::old_variables`]); as they are otherwise.
    fn compared(&self, texts: [Option<Vec<u8>>; 3]) -> [Option<Vec<u8>>; 3] {
        if self.moving.is_none() {
            return texts;
        }
        let [base, ours, theirs] = texts;
        let in_names = |text: Option<Vec<u8>>, names: fn(&str) -> String| {
            text.map(|bytes| renamed_text(bytes, names))
        };
        [
            in_names(base, layout::moved_names),
            in_names(ours, layout::moved_names),
            in_names(theirs, layout::old_variables),
        ]
    }

    /// The `texts` of the project, the older tminus and this one, in that order, merged by git
    /// through files in the folder `scratch` of the state root: the text and its conflicts. The
    /// marks name the older tool as the project knew it, and this tminus.
    fn merge(&self, texts: [&[u8]; 3], scratch: &str) -> Result<(Vec<u8>, usize), String> {
        let names = [YOURS, "base", "theirs"];
        let names = names.map(|name| format!("{scratch}/{name}"));
        for (name, text) in names.iter().zip(texts) {
            write(self.root, name, text)?;
        }
        // From the root, where git runs.
        let [ours, base, theirs] = names.each_ref().map(Path::new);
        let old = format!("{} {}", self.on_main().name(), self.old);
        let new = format!("tminus {VERSION}");
        let labels = [YOURS, old.as_str(), new.as_str()];
        let conflicts = self.git.merge_file(ours, base, theirs, labels);
        let conflicts = conflicts.map_err(said)?;
        let merged = read(self.root, &names[0])?.unwrap_or_default();
        Ok((merged, conflicts))
    }

    /// Makes the two commits of the `files` in a worktree of their own at `path`, in the folder
    /// of the worktrees `folder`, and the branch on the second. The worktree goes whatever
    /// happened, and the folder with it when this run made it and nothing else came into it.
    /// What the upgrade says of a move besides the `files` (see [`Move::rest`]).
    fn propose(
        &self,
        folder: &Path,
        path: &Path,
        files: &[Settled],
    ) -> Result<Vec<String>, String> {
        // Looked at before git makes it for the worktree: a folder that was there stays.
        let made = !folder.exists();
        let done = self.in_worktree(path, files);
        let tidied = if made { tidy(folder) } else { Ok(()) };
        let (last, more) = done?;
        tidied?;
        self.git.create_branch(BRANCH, &last).map_err(said)?;
        Ok(more)
    }

    /// Makes the two commits of the `files` in a worktree of their own at `path`, from the main
    /// branch as it is: the id of the second, and what the upgrade says of a move besides the
    /// `files`. The worktree goes whatever happened.
    fn in_worktree(&self, path: &Path, files: &[Settled]) -> Result<(String, Vec<String>), String> {
        let from = self.git.head().map_err(said)?;
        self.git.add_worktree(path, &from).map_err(said)?;
        let made = self.commit(path, files);
        let removed = self.git.remove_worktree(path);
        let removed = removed.and_then(|()| self.git.prune_worktrees());
        let done = made?;
        removed.map_err(said)?;
        Ok(done)
    }

    /// Makes, in the worktree at `path`, the commit of what this tminus writes, then the one of
    /// the `files` as they end, each at its path, the old one gone when the project moves, with
    /// the rest of the move (see [`Move::rest`]): the id of the second, and what the upgrade says
    /// of the rest. Nothing is written when one of the paths it writes or removes holds something
    /// of the project's it would take the place of (see [`Upgrade::targets`] and
    /// [`Upgrade::free`]).
    fn commit(&self, path: &Path, files: &[Settled]) -> Result<(String, Vec<String>), String> {
        let work = worktree::project_root(self.root, path)?;
        let git = Git::in_env(&work, self.env);
        let moved = self.moved(&git, files)?;
        for (target, room) in self.targets(files, &moved) {
            self.free(&work, &target, room)?;
        }
        for (file, text) in &self.theirs {
            write(&work, file, text.as_bytes())?;
        }
        let written: Vec<String> = self.theirs.iter().map(|(file, _)| file.clone()).collect();
        let upgrade = format!("tminus upgrade {} to {VERSION}", self.old);
        let pure = format!("{upgrade}: what tminus {VERSION} writes");
        git.commit_files(&written, &pure).map_err(said)?;
        for file in files {
            if file.last != file.first {
                match &file.last {
                    Some(text) => write(&work, &file.path, text)?,
                    None => remove(&work, &file.path)?,
                }
            }
            let old = self.on_main().path(&file.path);
            if old != file.path {
                remove(&work, &old)?;
            }
        }
        let more = match &self.moving {
            Some(moving) => moving.rest(&work, &git, &moved)?,
            None => Vec::new(),
        };
        let merged = format!("{upgrade}: merged with the project's changes");
        git.commit_all_even_empty(&merged).map_err(said)?;
        Ok((git.head().map_err(said)?, more))
    }

    /// The files `git` tracks under `gdd/` on the main branch but those of the record (the
    /// `files` settled and the old generated file), each with its new name, in git's order: what
    /// a move takes to `tminus/` as it is. None unless the project moves.
    fn moved(&self, git: &Git, files: &[Settled]) -> Result<Vec<(String, String)>, String> {
        if self.moving.is_none() {
            return Ok(Vec::new());
        }
        let mut record: Vec<String> = files
            .iter()
            .map(|file| self.on_main().path(&file.path))
            .collect();
        record.push(Layout::Gdd.path(GENERATED));
        let tracked = git.files_at("HEAD", "gdd").map_err(said)?;
        let rest = tracked.into_iter().filter(|path| !record.contains(path));
        let named = |from: String| {
            let to = layout::new_path(&from);
            (from, to)
        };
        Ok(rest.map(named).collect())
    }

    /// Every path the two commits write or remove, from the project root, with what the main
    /// branch may have there: what this tminus writes; each of the `files` the second commit
    /// writes or removes at its path; and, when the project moves, the old path of each of the
    /// `files` that takes a new one, the old generated file, where the `moved` files go, the
    /// agents' commands of both names and `.gitignore`. What a move writes under a new name has
    /// room for nothing there (see [`Upgrade::written`]); what it moves or installs, too.
    fn targets(&self, files: &[Settled], moved: &[(String, String)]) -> Vec<(String, Room)> {
        let theirs = self.theirs.iter().map(|(path, _)| path);
        let changed = files.iter().filter(|file| file.last != file.first);
        let written = theirs.chain(changed.map(|file| &file.path));
        let mut targets: Vec<(String, Room)> = written.map(|path| self.written(path)).collect();
        let Some(moving) = &self.moving else {
            return targets;
        };
        let paths = files.iter().map(|file| &file.path);
        let both = paths.map(|path| (self.on_main().path(path), path));
        let renamed = both.filter(|(old, path)| old != *path);
        let renamed = renamed.map(|(old, _)| old);
        let over = renamed.chain([Layout::Gdd.path(GENERATED)]);
        let over = over.chain(moving.old_commands.iter().cloned());
        let over = over.chain([GITIGNORE.to_string()]);
        targets.extend(over.map(|path| (path, Room::File)));
        let new = moved.iter().map(|(_, to)| to).chain(&moving.commands);
        targets.extend(new.map(|path| (path.clone(), Room::Nothing)));
        targets
    }

    /// The path `path` this tminus writes a file at, with what the main branch may have there:
    /// nothing when a move writes it under a new name (see [`Upgrade::on_main`]), since what the
    /// project has there is not what the move read, which it read where the main branch names
    /// it; a file or nothing otherwise.
    fn written(&self, path: &str) -> (String, Room) {
        let room = if self.on_main().path(path) == path {
            Room::File
        } else {
            Room::Nothing
        };
        (path.to_string(), room)
    }

    /// Whether this tminus may write or remove its file `path` in the worktree at `work`, checked
    /// out from the main branch, where `room` says what may be there: a file or nothing, or
    /// nothing at all, with only folders on the way. Anything else (a folder, a link or a
    /// submodule there, a file or a link on the way, a file where a move puts one under a new
    /// name) is the project's, and writing would remove it (see [`files::replace`]).
    fn free(&self, work: &Path, path: &str, room: Room) -> Result<(), String> {
        let main = &self.main;
        let next = "move what is in the way, commit, then run tminus upgrade again";
        let found = files::found(work, path).map_err(|e| {
            format!("{path} on {main}: {e}, where tminus {VERSION} writes a file: {next}")
        })?;
        if found.is_some() && room == Room::Nothing {
            return Err(format!(
                "{path} on {main}: the project has a file there, which the move to the tminus \
                 names would replace: {next}"
            ));
        }
        Ok(())
    }

    /// What the upgrade says of the `files` proposed, then of the rest of a move, `more`.
    fn said(&self, files: &[Settled], more: &[String]) -> String {
        let (old, main) = (&self.old, &self.main);
        let (names, next) = match self.moving {
            Some(_) => (
                ", in the tminus names",
                "tminus upgrade again: it moves .gdd/ to .tminus/",
            ),
            None => ("", "tminus agents install"),
        };
        let title = format!("tminus upgrade {old} to {VERSION}, on the branch {BRANCH}{names}:");
        let mut lines = vec![title];
        for file in files {
            lines.push(format!("  {}: {}", file.path, file.fate.said()));
            lines.extend_from_slice(&file.kept);
        }
        lines.extend_from_slice(more);
        let marked = |file: &Settled| matches!(file.fate, Fate::Merged(marks) if marks > 0);
        if files.iter().any(marked) {
            let settle = format!("settle the marks, commit, then git switch {main}");
            lines.push(format!("Conflicts: git switch {BRANCH}, {settle}."));
        }
        let review = format!("Review it: git diff {main} {BRANCH}");
        let merge = format!("git merge {BRANCH}, git branch -d {BRANCH}");
        let take = format!("Take it: {merge}, then {next}");
        lines.extend([review, take]);
        lines.join("\n")
    }
}

/// The text `text` of a file of an upgrade, with every limit it sets above what the project's
/// file `ours` sets (see [`Limits::raised_from`]; a file or a key `ours` leaves out counts as
/// tminus's value) put back as `ours` sets it: its own line when it has one (see [`limit_line`]),
/// `<key> = <value>` otherwise, each line with the line break it had. A higher limit loosens the
/// gate, which the team does in a commit of its own, never through an upgrade. What the upgrade
/// says of each limit kept, a line each, once the text with them read again sets each limit no
/// higher than `ours` nor `text` do. A text that is no TOML, the project's included, is left as it
/// is; a limit raised whose line cannot be found in the `[limits]` of `text`, or a text that does
/// not read as it should once kept, is an error.
fn kept_limits(ours: &[u8], text: Vec<u8>) -> Result<(Vec<u8>, Vec<String>), String> {
    let (Some((ours, before)), Some((result, after))) = (limits_in(ours), limits_in(&text)) else {
        return Ok((text, Vec::new()));
    };
    let raised = after.raised_from(&before);
    if raised.is_empty() {
        return Ok((text, Vec::new()));
    }
    let mut lines: Vec<String> = result.split_inclusive('\n').map(String::from).collect();
    let mut said = Vec::new();
    for &(key, old, new) in &raised {
        let Some(at) = limit_line(result, key) else {
            return Err(format!(
                "tminus {VERSION} raises limits.{key} from {old} to {new} and its line cannot be \
                 found in [limits] to keep yours: set it in a commit of its own"
            ));
        };
        let own = limit_line(ours, key).and_then(|line| ours.lines().nth(line));
        let own = own.map_or_else(|| format!("{key} = {old}"), str::to_string);
        let line = &lines[at];
        let end = &line[line.trim_end_matches(['\r', '\n']).len()..];
        lines[at] = format!("{own}{end}");
        said.push(format!(
            "kept your limits.{key} = {old}, tminus {VERSION} writes {new}: a higher limit \
             loosens the gate, raise it in a commit of its own"
        ));
    }
    let kept = lines.concat().into_bytes();
    if limits_in(&kept).map(|(_, limits)| limits) != Some(after.lowest(&before)) {
        let keys = raised.iter().map(|(key, ..)| format!("limits.{key}"));
        let keys: Vec<String> = keys.collect();
        return Err(format!(
            "tminus {VERSION} raises {} and the text with yours put back does not read as \
             yours: set them in a commit of their own",
            keys.join(", ")
        ));
    }
    Ok((kept, said))
}

/// The text of `bytes` and the limits it sets (see [`Limits::from_toml`]), when it is a text in
/// TOML.
fn limits_in(bytes: &[u8]) -> Option<(&str, Limits)> {
    let text = std::str::from_utf8(bytes).ok()?;
    Limits::from_toml(text).ok().map(|limits| (text, limits))
}

/// Which line of the rules text `text`, counted from 0, sets `key` in its `[limits]` table, as
/// TOML reads it: the line of the value TOML gives `limits.<key>`, so a line inside a string is
/// none. Only when that line is `<key> = <value>`, a comment after it or not, so the whole line
/// can be put in its place; `None` otherwise, as when the text is no TOML or does not set it.
fn limit_line(text: &str, key: &str) -> Option<usize> {
    #[derive(Default, Deserialize)]
    #[serde(default)]
    struct Spans {
        limits: BTreeMap<String, toml::Spanned<toml::Value>>,
    }
    let spans: Spans = toml::from_str(text).ok()?;
    let value = spans.limits.get(key)?.span();
    let start = text[..value.start].rfind('\n').map_or(0, |at| at + 1);
    let end = text[value.end..].find('\n');
    let end = end.map_or(text.len(), |at| value.end + at);
    let named = text[start..value.start].trim().strip_suffix('=')?;
    let rest = text[value.end..end].trim();
    let alone = named.trim_end() == key && (rest.is_empty() || rest.starts_with('#'));
    alone.then(|| text[..start].matches('\n').count())
}

/// The text of the file `path` at the commit `git` has checked out; empty when there is none.
fn text_at(git: &Git, path: &str) -> Result<String, String> {
    let found = git.file_at("HEAD", path).map_err(said)?;
    let bytes = found.unwrap_or_default();
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// The text `bytes` with its names changed by `names`; bytes that are no text stay as they are.
fn renamed_text(bytes: Vec<u8>, names: fn(&str) -> String) -> Vec<u8> {
    match String::from_utf8(bytes) {
        Ok(text) => names(&text).into_bytes(),
        Err(e) => e.into_bytes(),
    }
}

/// Moves the file `from` of the worktree at `work`, where `git` runs, to `to`, as git keeps it:
/// its bytes and its mode. The folders on the way to `to` are made first; nothing that is not a
/// folder is on the way (see [`Upgrade::free`]).
fn move_file(work: &Path, git: &Git, from: &str, to: &str) -> Result<(), String> {
    let folder = Path::new(to).parent().unwrap_or(Path::new(""));
    let made = fs::create_dir_all(work.join(folder));
    made.map_err(|e| format!("cannot write {to}: {e}"))?;
    git.move_path(from, to).map_err(said)
}

/// Adds `.tminus/` to the `.gitignore` of the folder `work` when no line of it is exactly that
/// (see [`init::ignore`]), as a new file with every line it had (see [`files::replace`]): whether
/// it did.
fn ignored(work: &Path) -> Result<bool, String> {
    let Some(added) = init::ignore(work, &[])? else {
        return Ok(false);
    };
    let text = read(work, GITIGNORE)?.unwrap_or_default();
    write(work, GITIGNORE, &[text, added.into_bytes()].concat())?;
    Ok(true)
}

/// What the generated file `record` of the folder `root` says; without one, why it cannot be
/// upgraded.
fn recorded(root: &Path, record: &str) -> Result<Generated, String> {
    let Some(bytes) = read(root, record)? else {
        return Err(format!("{record} {MISSING}"));
    };
    let text = String::from_utf8_lossy(&bytes);
    toml::from_str(&text).map_err(|e| format!("{record}: {e}"))
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

/// Why the files cannot be rendered from the answers, without the `error: ` tminus init starts
/// with.
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

    const WORKFLOW: &str = ".github/workflows/tminus.yml";
    const BASICS: &str = "tminus/project.toml";
    const ZONES: &str = "tminus/zones.toml";
    const BRIEF: &str = "docs/brief.md";

    fn names(named: &[&str]) -> Vec<String> {
        named.iter().map(|name| name.to_string()).collect()
    }

    /// A project tminus init set up, for Claude Code and Codex, in the folder `project` of a
    /// folder of its own, which also holds the folder of its worktrees.
    struct Project {
        folder: Folder,
        root: PathBuf,
        layout: Layout,
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
            Project {
                folder,
                root,
                layout: Layout::Tminus,
            }
        }

        /// [`Project::new`] as gdd set it up before the rename: every file it committed in the
        /// old names, with the texts it wrote, its agents' commands too, committed.
        fn old() -> Project {
            let mut project = Project::new();
            let tracked = project.git(&["ls-files"]);
            for path in tracked.lines() {
                let text = project.read(path);
                project.remove(path);
                let old = Layout::Gdd.path(path);
                project.write(&old, &Layout::Gdd.localize(&text));
            }
            fs::remove_dir(project.root.join("tminus")).unwrap();
            project.commit("gdd init");
            project.layout = Layout::Gdd;
            assert_eq!(Layout::at(&project.root), Layout::Gdd);
            project
        }

        /// Its generated file, as it names it.
        fn generated(&self) -> String {
            self.layout.path(GENERATED)
        }

        /// Leaves change `id` in the state `state`, in the state folder of the old layout.
        fn change_in(&self, id: &str, state: &str) {
            let record = serde_json::json!({"id": id, "state": state}).to_string();
            self.write(&format!(".gdd/changes/{id}/change.json"), &record);
        }

        /// Takes the branch of the upgrade into main, as a person does: its files are in the new
        /// names from then on.
        fn take(&mut self) {
            self.git(&["merge", "-q", "--no-edit", BRANCH]);
            self.git(&["branch", "-q", "-d", BRANCH]);
            self.layout = Layout::Tminus;
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

        /// Plays an older tminus `version` that wrote the project as it is now, with the files
        /// `unlisted` out of its generated file and the files `listed` in it: committed as
        /// tminus init commits.
        fn older(&self, version: &str, unlisted: &[&str], listed: &[&str]) {
            let path = self.generated();
            let mut generated: toml::Table = toml::from_str(&self.read(&path)).unwrap();
            let files = generated["files"].as_array().unwrap().iter();
            let kept = files.filter(|file| !unlisted.contains(&file.as_str().unwrap()));
            let mut files: Vec<toml::Value> = kept.cloned().collect();
            files.extend(listed.iter().map(|&file| toml::Value::from(file)));
            generated.insert("files".to_string(), toml::Value::Array(files));
            generated.insert("version".to_string(), version.into());
            self.write(&path, &toml::to_string(&generated).unwrap());
            self.commit(&format!("{} init {version}", self.layout.name()));
        }

        fn upgrade(&self) -> Result<String, String> {
            upgrade(&self.root, None, &env())
        }

        /// The file `path` on the branch of its upgrade; `None` without one.
        fn on_branch(&self, path: &str) -> Option<String> {
            let git = Git::in_env(&self.root, &env());
            let bytes = git.file_at(BRANCH, path).unwrap();
            bytes.map(|bytes| String::from_utf8(bytes).unwrap())
        }

        fn has_branch(&self) -> bool {
            let git = Git::in_env(&self.root, &env());
            let branch = format!("refs/heads/{BRANCH}");
            git.exists(&branch).unwrap()
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
            let scratch = self.root.join(self.layout.path(SCRATCH)).join(id);
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
        assert_eq!(says(Fate::Kept), "yours, tminus writes it as before");
        let dropped = format!("yours, tminus {VERSION} no longer writes it");
        assert_eq!(says(Fate::Dropped), dropped);
        assert_eq!(
            says(Fate::Added),
            format!("added (new in tminus {VERSION})")
        );
        assert_eq!(says(Fate::Updated), "updated (you had not changed it)");
        assert_eq!(says(Fate::LeftOut), "left out: you removed it");
        assert_eq!(says(Fate::Merged(0)), "merged with your changes");
        assert_eq!(says(Fate::Merged(3)), "conflict, settle the marks (3)");
    }

    #[test]
    fn the_files_are_those_this_tminus_writes_then_those_only_the_older_one_wrote() {
        let theirs = ["tminus/a.toml", GENERATED, "docs/c.md"];
        let theirs = theirs.map(|path| (path.to_string(), String::new()));
        let listed = [
            "docs/b.md",
            GENERATED,
            "tminus/a.toml",
            "x.md",
            "docs/b.md",
            "gdd/d.md",
        ];
        let mut upgrade = Upgrade {
            root: Path::new("."),
            git: Git::new(Path::new(".")),
            env: &[],
            moving: None,
            main: "main".to_string(),
            old: "0.0.0".to_string(),
            base: "HEAD".to_string(),
            listed: names(&listed),
            theirs: theirs.to_vec(),
        };
        let paths = [
            "tminus/a.toml",
            "docs/c.md",
            "docs/b.md",
            "x.md",
            "gdd/d.md",
        ];
        assert_eq!(upgrade.paths(), paths);
        // A move reads the older one's paths, its generated file's too, in the new names.
        upgrade.moving = Some(Move {
            old_commands: Vec::new(),
            commands: Vec::new(),
            agents: Vec::new(),
            stacks: Vec::new(),
        });
        let listed = [
            "gdd/a.toml",
            "gdd/generated.toml",
            "docs/b.md",
            ".github/workflows/gdd.yml",
            "gdd/e.md",
        ];
        upgrade.listed = names(&listed);
        let paths = [
            "tminus/a.toml",
            "docs/c.md",
            "docs/b.md",
            ".github/workflows/tminus.yml",
            "tminus/e.md",
        ];
        assert_eq!(upgrade.paths(), paths);
    }

    #[test]
    fn this_tminus_s_text_is_proposed_merged_with_the_project_s_on_a_branch_of_its_own() {
        let project = Project::new();
        let commands = project.read("tminus/commands.toml");
        let zones = project.read(ZONES);
        let models = project.read("tminus/models.toml");
        let generated = project.read(GENERATED);
        // What the older tminus wrote.
        project.replace(WORKFLOW, "actions/checkout@v4", "actions/checkout@v3");
        project.replace(BASICS, "language = \"en\"", "language = \"es\"");
        project.replace(BRIEF, "# Project Brief", "# The Brief");
        project.write("tminus/commands.toml", &format!("{commands}# older\n"));
        project.remove("tminus/models.toml");
        project.write("docs/old.md", "old\n");
        project.older("0.0.0", &["tminus/models.toml"], &["docs/old.md"]);
        // What the project made of it.
        project.replace(BASICS, "name = \"Notes\"", "name = \"Our notes\"");
        project.replace(BRIEF, "# The Brief", "# Our Brief");
        project.replace(ZONES, "Documentation", "Documentation and notes");
        project.remove("tminus/commands.toml");
        project.commit("ours");
        let main = project.git(&["rev-parse", "main"]);
        let said = project.upgrade().unwrap();
        let expected = format!(
            "tminus upgrade 0.0.0 to {VERSION}, on the branch tminus-upgrade:\n\
             \x20 tminus/project.toml: merged with your changes\n\
             \x20 tminus/commands.toml: left out: you removed it\n\
             \x20 tminus/rules.toml: yours, tminus writes it as before\n\
             \x20 tminus/zones.toml: yours, tminus writes it as before\n\
             \x20 tminus/models.toml: added (new in tminus {VERSION})\n\
             \x20 docs/brief.md: conflict, settle the marks (1)\n\
             \x20 .github/workflows/tminus.yml: updated (you had not changed it)\n\
             \x20 docs/old.md: yours, tminus {VERSION} no longer writes it\n\
             Conflicts: git switch tminus-upgrade, settle the marks, commit, then git switch \
             main.\n\
             Review it: git diff main tminus-upgrade\n\
             Take it: git merge tminus-upgrade, git branch -d tminus-upgrade, then tminus agents \
             install"
        );
        assert_eq!(said, expected);
        let ours = project.on_branch(BASICS).unwrap();
        assert!(ours.contains("\nname = \"Our notes\"\n"), "{ours}");
        assert!(ours.contains("\nlanguage = \"en\""), "{ours}");
        assert_eq!(project.on_branch("tminus/commands.toml"), None);
        let edited = zones.replacen("Documentation", "Documentation and notes", 1);
        assert_eq!(project.on_branch(ZONES), Some(edited));
        assert_eq!(project.on_branch("tminus/models.toml"), Some(models));
        let brief = project.on_branch(BRIEF).unwrap();
        let marked = format!(
            "<<<<<<< yours\n# Our Brief · Notes\n=======\n# Project Brief · Notes\n>>>>>>> tminus \
             {VERSION}\n"
        );
        assert!(brief.starts_with(&marked), "{brief}");
        let workflow = project.on_branch(WORKFLOW).unwrap();
        assert!(workflow.contains("actions/checkout@v4"), "{workflow}");
        assert_eq!(project.on_branch("docs/old.md").as_deref(), Some("old\n"));
        // tminus/generated.toml is this tminus's, as tminus init wrote it for the same answers.
        assert_eq!(project.on_branch(GENERATED), Some(generated));
        // First what this tminus writes, pure; then the merge, without tminus/generated.toml.
        let range = format!("main..{BRANCH}");
        let subjects = project.git(&["log", "--format=%s", &range]);
        let upgrade = format!("tminus upgrade 0.0.0 to {VERSION}");
        let both = format!(
            "{upgrade}: merged with the project's changes\n{upgrade}: what tminus {VERSION} writes"
        );
        assert_eq!(subjects, both);
        let before = format!("{BRANCH}~2");
        assert_eq!(project.git(&["rev-parse", &before]), main);
        let first = format!("{BRANCH}~1");
        let zones_first = project.git(&["show", &format!("{first}:{ZONES}")]);
        assert_eq!(zones_first, zones.trim());
        let show = format!("{first}:tminus/commands.toml");
        let commands_first = project.git(&["show", &show]);
        assert_eq!(commands_first, commands.trim());
        let merged = project.git(&["diff", "--name-only", &first, BRANCH]);
        let paths = "docs/brief.md\ntminus/commands.toml\ntminus/project.toml\ntminus/zones.toml";
        assert_eq!(merged, paths);
        // The main checkout stays on main, as it was.
        assert_eq!(project.git(&["rev-parse", "HEAD"]), main);
        assert_eq!(project.git(&["branch", "--show-current"]), "main");
        assert_eq!(project.git(&["status", "--porcelain"]), "");
        assert!(project.left_nothing());
    }

    #[test]
    fn a_project_set_up_before_the_rename_is_proposed_in_the_new_names() {
        let project = Project::old();
        // What the older gdd wrote, then what the project made of it.
        project.replace(BRIEF, "# Project Brief", "# The Brief");
        project.write("gdd/extra.md", "Written by gdd into gdd/\n");
        project.write("gdd/kept.md", "Written by the older tool\n");
        project.older("0.0.0", &[], &["gdd/extra.md", "gdd/kept.md"]);
        project.replace(BRIEF, "# The Brief", "# Our Brief");
        let zones = "gdd/zones.toml";
        project.replace(zones, "Documentation", "Documentation and notes");
        project.write("gdd/notes.md", "Our notes about gdd/ and .gdd/\n");
        project.write("gdd/bin/check.sh", "#!/bin/sh\n");
        let script = project.root.join("gdd/bin/check.sh");
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        project.commit("ours");
        // With the base in the marks, they name the older tool as the project knew it.
        project.git(&["config", "--local", "merge.conflictStyle", "diff3"]);
        let main = project.git(&["rev-parse", "main"]);
        let said = project.upgrade().unwrap();
        let updated = "updated (you had not changed it)";
        let expected = format!(
            "tminus upgrade 0.0.0 to {VERSION}, on the branch tminus-upgrade, in the tminus \
             names:\n\
             \x20 tminus/project.toml: {updated}\n\
             \x20 tminus/commands.toml: {updated}\n\
             \x20 tminus/rules.toml: {updated}\n\
             \x20 tminus/zones.toml: merged with your changes\n\
             \x20 tminus/models.toml: {updated}\n\
             \x20 docs/brief.md: conflict, settle the marks (1)\n\
             \x20 .github/workflows/tminus.yml: {updated}\n\
             \x20 tminus/extra.md: yours, tminus {VERSION} no longer writes it\n\
             \x20 tminus/kept.md: yours, tminus {VERSION} no longer writes it\n\
             \x20 tminus/bin/check.sh: moved as it was\n\
             \x20 tminus/notes.md: moved as it was\n\
             \x20 .gitignore: .tminus/ added\n\
             \x20 the agents' commands: tminus-<command> in place of gdd-<command>\n\
             Conflicts: git switch tminus-upgrade, settle the marks, commit, then git switch \
             main.\n\
             Review it: git diff main tminus-upgrade\n\
             Take it: git merge tminus-upgrade, git branch -d tminus-upgrade, then tminus \
             upgrade again: it moves .gdd/ to .tminus/"
        );
        assert_eq!(said, expected);
        let brief = project.on_branch(BRIEF).unwrap();
        let marked = format!(
            "<<<<<<< yours\n# Our Brief · Notes\n||||||| gdd 0.0.0\n# The Brief · Notes\n\
             =======\n# Project Brief · Notes\n>>>>>>> tminus {VERSION}\n"
        );
        assert!(brief.starts_with(&marked), "{brief}");
        // Nothing of the old names is left on the branch, and the new ones are all there.
        let tree = project.git(&["ls-tree", "-r", "--name-only", BRANCH]);
        for path in tree.lines() {
            let old = path.starts_with("gdd/") || path.contains("/gdd-");
            assert!(!old && !path.ends_with("gdd.yml"), "{path} in {tree}");
        }
        for new in [
            ".claude/skills/tminus-build/SKILL.md",
            ".agents/skills/tminus-stack-rust/SKILL.md",
            WORKFLOW,
            GENERATED,
        ] {
            assert!(tree.lines().any(|path| path == new), "{new} in {tree}");
        }
        // The project's line keeps its words, in the new names; the variables keep theirs.
        let zones = project.on_branch(ZONES).unwrap();
        let edited = "reason = \"Documentation and notes\"";
        assert!(zones.contains(edited), "{zones}");
        assert!(zones.contains("\".tminus/bin/**\""), "{zones}");
        assert!(!zones.contains("gdd/**"), "{zones}");
        let commands = project.on_branch("tminus/commands.toml").unwrap();
        let variables = "GDD_FEATURES= GDD_HIDDEN=1 cargo test";
        assert!(commands.contains(variables), "{commands}");
        assert!(!commands.contains("TMINUS_"), "{commands}");
        // What moved as it was keeps its bytes and its mode; what the older gdd wrote, its words.
        let notes = project.on_branch("tminus/notes.md");
        assert_eq!(notes.as_deref(), Some("Our notes about gdd/ and .gdd/\n"));
        let mode = project.git(&["ls-tree", BRANCH, "--", "tminus/bin/check.sh"]);
        assert!(mode.starts_with("100755 blob "), "{mode}");
        let extra = project.on_branch("tminus/extra.md");
        assert_eq!(extra.as_deref(), Some("Written by gdd into tminus/\n"));
        // Even with no name to change, it reaches its new path: the first commit has none there.
        let kept = project.on_branch("tminus/kept.md");
        assert_eq!(kept.as_deref(), Some("Written by the older tool\n"));
        let ignored = project.on_branch(GITIGNORE);
        assert_eq!(ignored.as_deref(), Some("/target\n.gdd/\n.tminus/\n"));
        // First what this tminus writes, pure: the base of the next upgrade.
        let range = format!("main..{BRANCH}");
        let subjects = project.git(&["log", "--format=%s", &range]);
        let upgrade = format!("tminus upgrade 0.0.0 to {VERSION}");
        let both = format!(
            "{upgrade}: merged with the project's changes\n{upgrade}: what tminus {VERSION} writes"
        );
        assert_eq!(subjects, both);
        let first = project.git(&["rev-parse", &format!("{BRANCH}~1")]);
        let base = project.git(&["log", "-1", "--format=%H", BRANCH, "--", GENERATED]);
        assert_eq!(base, first);
        let pure = project.git(&["show", &format!("{first}:tminus/commands.toml")]);
        let pure_variables = "TMINUS_FEATURES= TMINUS_HIDDEN=1 cargo test";
        assert!(pure.contains(pure_variables), "{pure}");
        // Main and the state stay as they were.
        assert_eq!(project.git(&["rev-parse", "HEAD"]), main);
        assert_eq!(project.git(&["status", "--porcelain"]), "");
        assert!(project.root.join("gdd/project.toml").is_file());
        assert!(!project.root.join(".tminus").exists());
        assert!(project.left_nothing());
    }

    #[test]
    fn once_the_branch_is_taken_the_state_moves_as_it_is_and_nothing_is_left_to_propose() {
        let mut project = Project::old();
        let log = "{\"event\": \"change-created\", \"change\": \"c1\"}\n";
        project.write(".gdd/log.jsonl", log);
        project.change_in("c1", "open");
        let record = project.read(".gdd/changes/c1/change.json");
        // A .gitignore that leaves out .tminus/ already keeps its lines.
        project.write(GITIGNORE, "/target\n.gdd/\n.tminus/\n");
        project.commit("ours");
        let said = project.upgrade().unwrap();
        assert!(!said.contains(GITIGNORE), "{said}");
        let ignored = project.on_branch(GITIGNORE);
        assert_eq!(ignored.as_deref(), Some("/target\n.gdd/\n.tminus/\n"));
        project.take();
        // Until the state moves, every other command names the way out (see `layout::check`).
        let root = &project.root;
        let refused = layout::check(root, root).unwrap_err();
        let way_out = "run tminus upgrade to move it to .tminus/";
        assert!(refused.ends_with(way_out), "{refused}");
        assert_eq!(project.upgrade(), Ok(STATE_MOVED.to_string()));
        let moved = "Moved .gdd/ to .tminus/: the approvals, records and log of the project as \
                     they were.\nThe red zones now name tminus/: approve them again with tminus \
                     gate zones ok --by human";
        assert_eq!(STATE_MOVED, moved);
        assert!(!root.join(".gdd").exists());
        assert_eq!(project.read(".tminus/log.jsonl"), log);
        assert_eq!(project.read(".tminus/changes/c1/change.json"), record);
        assert_eq!(layout::check(root, root), Ok(()));
        // The agents' commands are the ones tminus agents install writes.
        let models = Models::from_toml(&project.read(models::PATH)).unwrap();
        let agents = render::agents(&models, &[]).unwrap();
        let stacks = stacks::of_project(&project.read(PROJECT)).unwrap();
        assert_eq!(render::check(root, &agents, &stacks), Ok(Vec::new()));
        // The next upgrade starts from what this tminus wrote, pure: nothing to propose.
        let nothing = format!("Already up to date with tminus {VERSION}: nothing to propose.");
        assert_eq!(project.upgrade(), Ok(nothing));
        assert!(!project.has_branch());
        assert!(project.left_nothing());
    }

    #[test]
    fn a_change_in_flight_stops_either_run_and_nothing_moves() {
        let mut project = Project::old();
        project.change_in("c2", "in-pre");
        project.change_in("c1", "building");
        project.change_in("c3", "open");
        let main = project.git(&["rev-parse", "main"]);
        let in_flight = "tminus upgrade moves nothing while changes are in flight: c1 (building), \
                         c2 (in-pre). Finish, close or reject them, then run it again.";
        assert_eq!(project.upgrade(), Err(in_flight.to_string()));
        assert!(!project.has_branch());
        assert!(project.left_nothing());
        assert_eq!(project.git(&["rev-parse", "main"]), main);
        // Closed, rejected or rolled back, they let it run.
        project.change_in("c1", "closed");
        project.change_in("c2", "rolled-back");
        project.upgrade().unwrap();
        project.take();
        project.change_in("c1", "building");
        let one = "tminus upgrade moves nothing while changes are in flight: c1 (building). \
                   Finish, close or reject them, then run it again.";
        assert_eq!(project.upgrade(), Err(one.to_string()));
        assert!(project.root.join(".gdd/changes/c1/change.json").is_file());
        assert!(!project.root.join(".tminus").exists());
    }

    #[test]
    fn a_state_that_is_no_folder_of_its_own_is_moved_by_hand_and_agents_move_nothing() {
        let folder = Folder::new();
        let root = &folder.0;
        folder.write("tminus/project.toml", "");
        folder.write(".gdd/log.jsonl", "log\n");
        for role in ["builder-build", "reviewer"] {
            let refused = upgrade(root, Some(role), &env());
            assert_eq!(refused, Err(NOT_FOR_AGENTS.to_string()), "{role}");
        }
        assert!(root.join(".gdd/log.jsonl").is_file());
        // A link to a folder elsewhere, or a file, stays where it is.
        let elsewhere = Folder::new();
        let state = elsewhere.0.join("state");
        fs::rename(root.join(".gdd"), &state).unwrap();
        symlink(&state, root.join(".gdd")).unwrap();
        let gdd = root.join(".gdd");
        let by_hand = format!("{} is not a folder: move it by hand", gdd.display());
        assert_eq!(upgrade(root, None, &env()), Err(by_hand.clone()));
        assert!(fs::symlink_metadata(&gdd).unwrap().file_type().is_symlink());
        fs::remove_file(&gdd).unwrap();
        folder.write(".gdd", "state\n");
        assert_eq!(upgrade(root, None, &env()), Err(by_hand));
        assert!(!root.join(".tminus").exists());
        let kept = fs::read_to_string(state.join("log.jsonl")).unwrap();
        assert_eq!(kept, "log\n");
        // A folder of its own moves, and nothing else is written.
        fs::remove_file(&gdd).unwrap();
        fs::rename(&state, &gdd).unwrap();
        assert_eq!(upgrade(root, None, &env()), Ok(STATE_MOVED.to_string()));
        let log = fs::read_to_string(root.join(".tminus/log.jsonl")).unwrap();
        assert_eq!(log, "log\n");
        assert_eq!(fs::read_dir(root).unwrap().count(), 2);
    }

    /// The move of [`Project::old`], where the older gdd wrote and listed the files `listed` and
    /// the project then did `ours`, committed, refused: why, once it is sure nothing was written
    /// (no branch, no worktree, main and the state as they were).
    fn refused_move(listed: &[&str], ours: impl FnOnce(&Project)) -> String {
        let project = Project::old();
        for path in listed {
            project.write(path, "Written by the older tool\n");
        }
        project.older("0.0.0", &[], listed);
        ours(&project);
        project.commit("ours");
        let main = project.git(&["rev-parse", "main"]);
        let tracked = project.git(&["ls-tree", "-r", "main"]);
        let said = project.upgrade().unwrap_err();
        assert!(!project.has_branch(), "{said}");
        assert!(project.left_nothing(), "{said}");
        assert_eq!(project.git(&["rev-parse", "HEAD"]), main, "{said}");
        assert_eq!(project.git(&["ls-tree", "-r", "main"]), tracked, "{said}");
        assert_eq!(project.git(&["status", "--porcelain"]), "", "{said}");
        assert!(!project.root.join(".gdd").exists(), "{said}");
        said
    }

    #[test]
    fn a_move_writes_and_removes_nothing_through_what_is_not_a_file() {
        let not_own = "not a file of its own (a link, a folder or a file with other names)";
        let writes = format!("where tminus {VERSION} writes a file");
        let next = "move what is in the way, commit, then run tminus upgrade again";
        // The new name of a file only the older gdd listed: a folder there, a link on the way.
        let folder: fn(&Project) = |project| project.write("tminus/kept.md/inner.md", "ours\n");
        let link: fn(&Project) = |project| {
            fs::create_dir(project.root.join("tminus")).unwrap();
            symlink("../docs", project.root.join("tminus/sub")).unwrap();
        };
        // Its old name, which the move removes, a folder.
        let old_folder: fn(&Project) = |project| {
            project.remove("gdd/kept.md");
            project.write("gdd/kept.md/inner.md", "ours\n");
        };
        let new_folder = format!("tminus/kept.md on main: {not_own}");
        let link_on_the_way = "tminus/sub/kept.md on main: tminus/sub is not a folder";
        let old = format!("gdd/kept.md on main: {not_own}");
        let cases = [
            ("gdd/kept.md", folder, new_folder),
            ("gdd/sub/kept.md", link, link_on_the_way.to_string()),
            ("gdd/kept.md", old_folder, old),
        ];
        for (listed, ours, why) in cases {
            let said = refused_move(&[listed], ours);
            assert_eq!(said, format!("{NO_BRANCH}: {why}, {writes}: {next}"));
        }
    }

    #[test]
    fn a_move_never_takes_the_place_of_a_file_the_project_has_under_a_new_name() {
        let next = "move what is in the way, commit, then run tminus upgrade again";
        let none: &[&str] = &[];
        let cases = [
            // What this tminus writes, its record too.
            (none, none, ZONES),
            (none, none, WORKFLOW),
            (none, none, GENERATED),
            // A file only the older gdd listed, one the project added to gdd/, an agent's command.
            (&["gdd/kept.md"][..], none, "tminus/kept.md"),
            (none, &["gdd/notes.md"][..], "tminus/notes.md"),
            (none, none, ".claude/skills/tminus-build/SKILL.md"),
        ];
        for (listed, added, taken) in cases {
            let said = refused_move(listed, |project| {
                for path in added {
                    project.write(path, "Our notes\n");
                }
                project.write(taken, "Ours, under the new name\n");
            });
            let refused = format!(
                "{NO_BRANCH}: {taken} on main: the project has a file there, which the move to the \
                 tminus names would replace: {next}"
            );
            assert_eq!(said, refused);
        }
    }

    #[test]
    fn what_a_move_neither_writes_nor_removes_is_not_looked_at() {
        let project = Project::old();
        project.write("docs/old.md", "old\n");
        project.older("0.0.0", &[], &["docs/old.md"]);
        // The project made a folder where the older gdd wrote a file that keeps its name.
        project.remove("docs/old.md");
        project.write("docs/old.md/ours.md", "ours\n");
        project.commit("ours");
        let said = project.upgrade().unwrap();
        let yours = format!("\n  docs/old.md: yours, tminus {VERSION} no longer writes it\n");
        assert!(said.contains(&yours), "{said}");
        let ours = project.on_branch("docs/old.md/ours.md");
        assert_eq!(ours.as_deref(), Some("ours\n"));
        assert!(project.left_nothing());
    }

    #[test]
    fn a_move_names_a_file_of_its_agents_it_cannot_read() {
        for (file, text) in [
            ("gdd/models.toml", "[reviewer\n"),
            ("gdd/project.toml", "stack = 1\n"),
        ] {
            let project = Project::old();
            project.write(file, text);
            project.commit("ours");
            let said = project.upgrade().unwrap_err();
            assert!(said.starts_with(&format!("{file}: ")), "{said}");
            assert!(!project.has_branch());
            assert!(project.left_nothing());
            assert!(!project.root.join(".gdd").exists());
        }
    }

    #[test]
    fn a_project_with_no_state_yet_gets_an_empty_one_to_move() {
        let mut project = Project::old();
        assert!(!project.root.join(".gdd").exists());
        project.upgrade().unwrap();
        let state = fs::read_dir(project.root.join(".gdd")).unwrap();
        assert_eq!(state.count(), 0);
        project.take();
        assert_eq!(project.upgrade(), Ok(STATE_MOVED.to_string()));
        assert!(project.root.join(".tminus").is_dir());
        assert!(!project.root.join(".gdd").exists());
    }

    #[test]
    fn a_project_set_up_before_the_rename_names_its_own_record() {
        let project = Project::old();
        let generated = project.read("gdd/generated.toml");
        project.remove("gdd/generated.toml");
        let missing = "gdd/generated.toml is missing: this project was set up before tminus \
                       recorded what it writes, so tminus upgrade has no base to merge from";
        assert_eq!(project.upgrade(), Err(missing.to_string()));
        let (without, _) = generated.split_once("\n[answers]\n").unwrap();
        project.write("gdd/generated.toml", without);
        let none = format!("gdd/generated.toml {NO_ANSWERS}");
        assert_eq!(project.upgrade(), Err(none));
        project.write("gdd/generated.toml", &generated);
        project.write("gdd/rules.toml", "main_branch = [\n");
        let said = project.upgrade().unwrap_err();
        assert!(said.starts_with("gdd/rules.toml: "), "{said}");
    }

    #[test]
    fn a_clean_merge_names_no_conflict_and_the_next_upgrade_starts_from_the_pure_text() {
        let project = Project::new();
        project.replace(WORKFLOW, "actions/checkout@v4", "actions/checkout@v3");
        project.older("0.0.0", &[], &[]);
        project.replace(WORKFLOW, "runs-on: ubuntu-latest", "runs-on: ubuntu-24.04");
        project.commit("ours");
        let said = project.upgrade().unwrap();
        let kept = "yours, tminus writes it as before";
        let expected = format!(
            "tminus upgrade 0.0.0 to {VERSION}, on the branch tminus-upgrade:\n\
             \x20 tminus/project.toml: {kept}\n\
             \x20 tminus/commands.toml: {kept}\n\
             \x20 tminus/rules.toml: {kept}\n\
             \x20 tminus/zones.toml: {kept}\n\
             \x20 tminus/models.toml: {kept}\n\
             \x20 docs/brief.md: {kept}\n\
             \x20 .github/workflows/tminus.yml: merged with your changes\n\
             Review it: git diff main tminus-upgrade\n\
             Take it: git merge tminus-upgrade, git branch -d tminus-upgrade, then tminus agents \
             install"
        );
        assert_eq!(said, expected);
        let workflow = project.on_branch(WORKFLOW).unwrap();
        assert!(workflow.contains("actions/checkout@v4\n"), "{workflow}");
        assert!(workflow.contains("runs-on: ubuntu-24.04\n"), "{workflow}");
        // Taken, the next upgrade starts from what this tminus wrote: nothing to propose.
        project.git(&["merge", "-q", "--no-edit", BRANCH]);
        project.git(&["branch", "-q", "-d", BRANCH]);
        let base = project.git(&["log", "-1", "--format=%s", "--", GENERATED]);
        let pure = format!("tminus upgrade 0.0.0 to {VERSION}: what tminus {VERSION} writes");
        assert_eq!(base, pure);
        let nothing = format!("Already up to date with tminus {VERSION}: nothing to propose.");
        assert_eq!(project.upgrade(), Ok(nothing));
        assert!(!project.has_branch());
    }

    #[test]
    fn a_file_nobody_touched_takes_the_new_text_and_the_merge_commit_is_still_made() {
        let project = Project::new();
        project.replace(WORKFLOW, "actions/checkout@v4", "actions/checkout@v3");
        project.older("0.0.0", &[], &[]);
        let said = project.upgrade().unwrap();
        let updated = "  .github/workflows/tminus.yml: updated (you had not changed it)\n";
        assert!(said.contains(updated), "{said}");
        assert!(!said.contains("Conflicts"), "{said}");
        let workflow = project.on_branch(WORKFLOW).unwrap();
        assert!(workflow.contains("actions/checkout@v4"), "{workflow}");
        assert!(project.read(WORKFLOW).contains("actions/checkout@v3"));
        let subjects = project.git(&["log", "--format=%s", &format!("main..{BRANCH}")]);
        assert_eq!(subjects.lines().count(), 2, "{subjects}");
        assert!(subjects.starts_with("tminus upgrade 0.0.0 to "));
        let first = format!("{BRANCH}~1");
        assert_eq!(project.git(&["diff", "--name-only", &first, BRANCH]), "");
        assert!(project.left_nothing());
    }

    #[test]
    fn nothing_is_proposed_when_this_tminus_writes_what_the_older_one_did() {
        let project = Project::new();
        // The project's own changes are no reason to propose anything.
        project.replace(ZONES, "Documentation", "Docs");
        project.remove(BRIEF);
        project.commit("ours");
        let nothing = format!("Already up to date with tminus {VERSION}: nothing to propose.");
        assert_eq!(project.upgrade(), Ok(nothing));
        assert!(!project.has_branch());
        assert!(!project.root.join(SCRATCH).exists());
        assert!(project.left_nothing());
    }

    #[test]
    fn a_file_the_older_tminus_did_not_list_is_merged_against_an_empty_base() {
        let project = Project::new();
        // The older tminus did not write the models file, which the project already had.
        project.older("0.0.0", &["tminus/models.toml"], &[]);
        project.replace("tminus/models.toml", "# Models · GDD", "# Our models");
        project.commit("ours");
        let said = project.upgrade().unwrap();
        let conflict = "\n  tminus/models.toml: conflict, settle the marks (1)\n";
        assert!(said.contains(conflict), "{said}");
        let switch = "\nConflicts: git switch tminus-upgrade, settle the marks, commit, then git \
                      switch main.\n";
        assert!(said.contains(switch), "{said}");
        let models = project.on_branch("tminus/models.toml").unwrap();
        let ours = "<<<<<<< yours\n# Our models";
        assert!(models.starts_with(ours), "{models}");
        let written = format!("# Models · GDD (written by tminus init {VERSION})");
        let theirs = format!("=======\n{written}\n>>>>>>> ");
        assert!(models.contains(&theirs), "{models}");
    }

    #[test]
    fn the_checks_come_in_order_and_write_nothing() {
        let project = Project::new();
        project.replace(WORKFLOW, "actions/checkout@v4", "actions/checkout@v3");
        project.older("0.0.0", &[], &[]);
        for role in ["builder-build", "builder-test", "reviewer", "orchestrator"] {
            let refused = upgrade(&project.root, Some(role), &env());
            assert_eq!(refused, Err(NOT_FOR_AGENTS.to_string()), "{role}");
        }
        let said = "Builders and Reviewers cannot run tminus upgrade: a person runs it, then \
                    reviews and merges its branch";
        assert_eq!(NOT_FOR_AGENTS, said);
        // The record first, even with uncommitted changes.
        let generated = project.read(GENERATED);
        project.remove(GENERATED);
        let missing = "tminus/generated.toml is missing: this project was set up before tminus \
                       recorded what it writes, so tminus upgrade has no base to merge from";
        assert_eq!(project.upgrade(), Err(missing.to_string()));
        let (without, _) = generated.split_once("\n[answers]\n").unwrap();
        project.write(GENERATED, without);
        let none = "tminus/generated.toml has no [answers]: it was written before tminus \
                    recorded them, add them as tminus init writes them now";
        assert_eq!(project.upgrade(), Err(none.to_string()));
        project.write(GENERATED, &generated);
        // Then the main branch the rules name; the replanning agent is still refused first.
        project.git(&["checkout", "-q", "-b", "other"]);
        let runs_on = |main: &str| -> Result<String, String> {
            let switch = format!("git switch {main}, then run it again");
            Err(format!("tminus upgrade runs on {main}: {switch}"))
        };
        assert_eq!(project.upgrade(), runs_on("main"));
        let orchestrator = upgrade(&project.root, Some("orchestrator"), &env());
        assert_eq!(orchestrator, Err(NOT_FOR_AGENTS.to_string()));
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
        let there = "the branch tminus-upgrade is already there: merge it or delete it (git \
                     branch -D tminus-upgrade) first";
        assert_eq!(project.upgrade(), Err(there.to_string()));
        assert_eq!(branch_there(BRANCH), there);
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
        assert!(said.starts_with("tminus/generated.toml: "), "{said}");
        project.remove(GENERATED);
        fs::create_dir(project.root.join(GENERATED)).unwrap();
        let said = project.upgrade().unwrap_err();
        let unreadable = "cannot read tminus/generated.toml: ";
        assert!(said.starts_with(unreadable), "{said}");
        fs::remove_dir(project.root.join(GENERATED)).unwrap();
        project.git(&["checkout", "-q", "--", GENERATED]);
        project.write(guard::RULES, "main_branch = [\n");
        let said = project.upgrade().unwrap_err();
        assert!(said.starts_with("tminus/rules.toml: "), "{said}");
        project.git(&["checkout", "-q", "--", guard::RULES]);
        // A repository where no commit ever held it.
        fs::remove_dir_all(project.root.join(".git")).unwrap();
        project.git(&["init", "-q", "-b", "main"]);
        project.git(&["add", "-A"]);
        project.git(&["reset", "-q", "--", GENERATED]);
        project.git(&["commit", "-q", "-m", "without the record"]);
        let uncommitted = "tminus/generated.toml is in no commit: commit it first, tminus \
                           upgrade merges from the commit that wrote it";
        assert_eq!(project.upgrade(), Err(uncommitted.to_string()));
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
        let commit = format!("git commit -q -m tminus upgrade 0.0.0 to {VERSION}: what ");
        let start = format!("{NO_BRANCH}: {commit}");
        assert!(said.starts_with(&start), "{said}");
        assert!(!project.has_branch());
        assert!(project.left_nothing());
        assert_eq!(project.git(&["rev-parse", "HEAD"]), main);
        assert_eq!(project.git(&["status", "--porcelain"]), "");
    }

    #[test]
    fn what_the_project_put_where_this_tminus_writes_a_file_is_never_taken() {
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
            // This tminus writes another workflow: there is something to propose.
            project.replace(WORKFLOW, "actions/checkout@v4", "actions/checkout@v3");
            project.older("0.0.0", &[], &[]);
            ours(&project);
            project.commit("ours");
            let main = project.git(&["rev-parse", "main"]);
            let tracked = project.git(&["ls-tree", "-r", "main"]);
            let said = project.upgrade().unwrap_err();
            let next = "move what is in the way, commit, then run tminus upgrade again";
            let refused = format!(
                "{NO_BRANCH}: {path} on main: {why}, where tminus {VERSION} writes a file: {next}"
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

    const RULES: &str = "tminus/rules.toml";

    /// What the upgrade says of the limit `key` of the rules file it kept at `old` where this
    /// tminus writes `new`, without the file.
    fn kept_said(key: &str, old: u64, new: u64) -> String {
        format!(
            "kept your limits.{key} = {old}, tminus {VERSION} writes {new}: a higher limit \
             loosens the gate, raise it in a commit of its own"
        )
    }

    /// [`kept_limits`] of the texts `ours` and `text`, as text.
    fn kept_text(ours: &str, text: &str) -> Result<(String, Vec<String>), String> {
        let (kept, said) = kept_limits(ours.as_bytes(), text.as_bytes().to_vec())?;
        Ok((String::from_utf8(kept).unwrap(), said))
    }

    #[test]
    fn a_limit_this_tminus_raises_keeps_the_project_s_own_line() {
        let ours = "main_branch = \"main\"\n[limits]\nbuilder_retries = 3\n\
                    reviewer_fixes = 1 # ours\n";
        let text = "main_branch = \"main\"\n\n[limits]\nbuilder_retries = 3\n\
                    reviewer_fixes = 2   # new\ncommand_seconds = 1800\n";
        let kept = "main_branch = \"main\"\n\n[limits]\nbuilder_retries = 3\n\
                    reviewer_fixes = 1 # ours\ncommand_seconds = 1800\n";
        let said = vec![kept_said("reviewer_fixes", 1, 2)];
        assert_eq!(kept_text(ours, text), Ok((kept.to_string(), said)));
        // A key, or a file, the project leaves out counts as tminus's value; each line keeps its
        // line break, the last one too.
        let text = "[limits]\r\nreviewer_fixes = 2\r\nreplans = 1\r\nbuilder_retries = 4";
        let kept = "[limits]\r\nreviewer_fixes = 1\r\nreplans = 1\r\nbuilder_retries = 3";
        let said = vec![
            kept_said("builder_retries", 3, 4),
            kept_said("reviewer_fixes", 1, 2),
        ];
        for ours in ["[limits]\nbuilder_retries = 3\n", ""] {
            let expected = Ok((kept.to_string(), said.clone()));
            assert_eq!(kept_text(ours, text), expected, "{ours:?}");
        }
    }

    #[test]
    fn a_limit_this_tminus_lowers_or_leaves_and_a_text_that_is_no_toml_pass_as_they_are() {
        let ours = "[limits]\nbuilder_retries = 5\nreviewer_fixes = 2\n";
        let text = "[limits]\nbuilder_retries = 3\nreviewer_fixes = 2\n";
        assert_eq!(kept_text(ours, text), Ok((text.to_string(), Vec::new())));
        let marked = "[limits]\n<<<<<<< yours\nreviewer_fixes = 0\n=======\n\
                      reviewer_fixes = 2\n>>>>>>> tminus\n";
        assert_eq!(kept_text("", marked), Ok((marked.to_string(), Vec::new())));
        let raised = "[limits]\nreviewer_fixes = 2\n";
        let not_toml = "[limits\nreviewer_fixes = 1\n";
        let left = Ok((raised.to_string(), Vec::new()));
        assert_eq!(kept_text(not_toml, raised), left);
        let bytes = b"[limits]\nreviewer_fixes = 2 # \xff\n".to_vec();
        assert_eq!(kept_limits(b"", bytes.clone()), Ok((bytes, Vec::new())));
    }

    #[test]
    fn a_limit_raised_whose_line_is_not_its_own_is_an_error() {
        for text in [
            "limits = { reviewer_fixes = 2 }\n",
            "limits.reviewer_fixes = 2\n",
            "[limits]\n\"reviewer_fixes\" = 2\n",
            "limits = {\n  reviewer_fixes = 2, replans = 1,\n}\n",
        ] {
            let said = kept_text("[limits]\nreviewer_fixes = 1\n", text).unwrap_err();
            let why = format!(
                "tminus {VERSION} raises limits.reviewer_fixes from 1 to 2 and its line cannot be \
                 found in [limits] to keep yours: set it in a commit of its own"
            );
            assert_eq!(said, why, "{text:?}");
        }
    }

    #[test]
    fn the_line_of_a_limit_is_the_one_that_sets_it_in_the_limits_table() {
        let text = "x = 1\n[other]\nreviewer_fixes = 7\n[ limits ] # the agents\n\
                    reviewer_fixes_x = 3\n  reviewer_fixes= 2\n";
        assert_eq!(limit_line(text, "reviewer_fixes"), Some(5));
        assert_eq!(limit_line(text, "x"), None);
        let other = "[other]\nreviewer_fixes = 7\n[limits]\nreviewer_fixes = 2\n";
        let kept = "[other]\nreviewer_fixes = 7\n[limits]\nreviewer_fixes = 1\n";
        let said = vec![kept_said("reviewer_fixes", 1, 2)];
        assert_eq!(kept_text("", other), Ok((kept.to_string(), said)));
        for none in [
            "reviewer_fixes = 2\n",
            "[limits]\nreplans = 1\n[spike]\nreviewer_fixes = 2\n",
            "[limits.more]\nreviewer_fixes = 2\n",
            "[[limits]]\nreviewer_fixes = 2\n",
            "[limits]\n# reviewer_fixes = 2\n",
            "[limits]\nnote = \"\"\"\nreviewer_fixes = 2\n\"\"\"\n",
            "[limits\nreviewer_fixes = 2\n",
        ] {
            assert_eq!(limit_line(none, "reviewer_fixes"), None, "{none:?}");
        }
        // The table as TOML names it, in quotes too; a text in a string of it is no key.
        let quoted = "[\"limits\"]\nnote = '''\nreviewer_fixes = 9\n'''\nreviewer_fixes = 2\n";
        assert_eq!(limit_line(quoted, "reviewer_fixes"), Some(4));
    }

    #[test]
    fn a_limit_written_inside_a_string_is_neither_the_line_kept_nor_the_line_put_back() {
        // The text of a table in a note, before the table itself: TOML reads it as a string.
        let note = "note = \"\"\"\n[limits]\nreviewer_fixes = 99\n\"\"\"\n";
        let ours = format!("{note}[limits]\nreviewer_fixes = 1\n");
        let text = format!("{note}[limits]\nreviewer_fixes = 2\n");
        assert_eq!(limit_line(&text, "reviewer_fixes"), Some(5));
        let said = vec![kept_said("reviewer_fixes", 1, 2)];
        assert_eq!(kept_text(&ours, &text), Ok((ours.clone(), said.clone())));
        // A project that sets the limit only in a string leaves it out.
        let kept = format!("{note}[limits]\nreviewer_fixes = 1\n");
        assert_eq!(kept_text(note, &text), Ok((kept, said)));
        for (key, old, new) in [
            ("builder_retries", 3, 4),
            ("replans", 1, 2),
            ("reviewer_fixes", 1, 2),
            ("agent_timeout_s", 3600, 7200),
        ] {
            let note = format!("note = '''\n[limits]\n{key} = 99\n'''\n");
            let ours = format!("{note}[limits]\n{key} = {old} # ours\n");
            let text = format!("{note}[limits]\n{key} = {new}\n");
            let said = vec![kept_said(key, old, new)];
            assert_eq!(kept_text(&ours, &text), Ok((ours.clone(), said)), "{key}");
        }
    }

    /// The line of the limit `key` in the text `rules`.
    fn line_of(rules: &str, key: &str) -> String {
        let line = rules.lines().find(|line| line.starts_with(key));
        line.expect("its line").to_string()
    }

    #[test]
    fn an_upgrade_keeps_the_limits_the_project_had_and_then_has_nothing_to_propose() {
        let mut project = Project::new();
        // An older tminus wrote one fix round; the project left it as it was.
        project.replace(RULES, "reviewer_fixes = 2", "reviewer_fixes = 1");
        project.older("0.0.0", &[], &[]);
        let said = project.upgrade().unwrap();
        let kept = kept_said("reviewer_fixes", 1, 2);
        let lines = format!("  {RULES}: updated (you had not changed it)\n  {RULES}: {kept}\n");
        assert!(said.contains(&lines), "{said}");
        let rules = project.on_branch(RULES).unwrap();
        let ours = line_of(&project.read(RULES), "reviewer_fixes");
        assert_eq!(line_of(&rules, "reviewer_fixes"), ours);
        assert!(!rules.contains("reviewer_fixes = 2"), "{rules}");
        // The first commit is what this tminus writes, pure: the base of the next upgrade.
        let first = project.git(&["show", &format!("{BRANCH}~1:{RULES}")]);
        assert!(first.contains("reviewer_fixes = 2"), "{first}");
        project.take();
        let nothing = format!("Already up to date with tminus {VERSION}: nothing to propose.");
        assert_eq!(project.upgrade(), Ok(nothing));
    }

    #[test]
    fn a_merged_rules_file_keeps_the_limits_the_project_had_and_takes_the_lower_ones() {
        let project = Project::new();
        // An older tminus wrote one fix round and four retries; the project raised its budget.
        project.replace(RULES, "reviewer_fixes = 2", "reviewer_fixes = 1");
        project.replace(RULES, "builder_retries = 3", "builder_retries = 4");
        project.older("0.0.0", &[], &[]);
        project.replace(RULES, "max_usd = 5", "max_usd = 6");
        project.commit("ours");
        let said = project.upgrade().unwrap();
        let kept = kept_said("reviewer_fixes", 1, 2);
        let lines = format!("  {RULES}: merged with your changes\n  {RULES}: {kept}\n");
        assert!(said.contains(&lines), "{said}");
        assert!(!said.contains("limits.builder_retries"), "{said}");
        let rules = project.on_branch(RULES).unwrap();
        for line in ["max_usd = 6", "builder_retries = 3", "reviewer_fixes = 1 "] {
            assert!(rules.contains(line), "{line} in {rules}");
        }
        assert!(!rules.contains("reviewer_fixes = 2"), "{rules}");
    }

    #[test]
    fn a_limit_the_project_left_out_is_written_at_the_value_it_read() {
        let project = Project::new();
        let line = line_of(&project.read(RULES), "reviewer_fixes");
        project.replace(RULES, &format!("{line}\n"), "");
        project.older("0.0.0", &[], &[]);
        let said = project.upgrade().unwrap();
        let kept = kept_said("reviewer_fixes", 1, 2);
        let lines = format!("  {RULES}: updated (you had not changed it)\n  {RULES}: {kept}\n");
        assert!(said.contains(&lines), "{said}");
        let rules = project.on_branch(RULES).unwrap();
        assert_eq!(line_of(&rules, "reviewer_fixes"), "reviewer_fixes = 1");
    }

    #[test]
    fn a_project_that_edited_the_same_limit_settles_the_marks_itself() {
        let project = Project::new();
        project.replace(RULES, "reviewer_fixes = 2", "reviewer_fixes = 1");
        project.older("0.0.0", &[], &[]);
        project.replace(RULES, "reviewer_fixes = 1", "reviewer_fixes = 0");
        project.commit("ours");
        let said = project.upgrade().unwrap();
        let marked = format!("  {RULES}: conflict, settle the marks (1)\n");
        assert!(said.contains(&marked), "{said}");
        assert!(!said.contains("kept your"), "{said}");
        let rules = project.on_branch(RULES).unwrap();
        let yours = "<<<<<<< yours\nreviewer_fixes = 0";
        assert!(rules.contains(yours), "{rules}");
    }

    #[test]
    fn a_project_moving_to_the_new_names_keeps_its_limits_too() {
        let project = Project::old();
        project.replace("gdd/rules.toml", "reviewer_fixes = 2", "reviewer_fixes = 1");
        project.older("0.0.0", &[], &[]);
        let said = project.upgrade().unwrap();
        let kept = kept_said("reviewer_fixes", 1, 2);
        let lines = format!("  {RULES}: updated (you had not changed it)\n  {RULES}: {kept}\n");
        assert!(said.contains(&lines), "{said}");
        let rules = project.on_branch(RULES).unwrap();
        assert!(rules.contains("\nreviewer_fixes = 1 "), "{rules}");
        assert!(!rules.contains("reviewer_fixes = 2"), "{rules}");
    }

    #[test]
    fn an_upgrade_keeps_the_limit_toml_reads_not_a_text_that_looks_like_it() {
        let project = Project::new();
        project.replace(RULES, "reviewer_fixes = 2", "reviewer_fixes = 1");
        project.older("0.0.0", &[], &[]);
        // The project keeps the text of a table in a note, before its own, and names its table
        // in quotes: neither changes where TOML reads the limit.
        let note = "note = \"\"\"\n[limits]\nreviewer_fixes = 99\n\"\"\"\n";
        project.replace(RULES, "[acceptance]", &format!("{note}\n[acceptance]"));
        project.replace(RULES, "[limits]\nbuilder", "[\"limits\"]\nbuilder");
        project.commit("ours");
        let said = project.upgrade().unwrap();
        let kept = kept_said("reviewer_fixes", 1, 2);
        let lines = format!("  {RULES}: merged with your changes\n  {RULES}: {kept}\n");
        assert!(said.contains(&lines), "{said}");
        let rules = project.on_branch(RULES).unwrap();
        assert!(rules.contains(note), "{rules}");
        assert!(rules.contains("\n[\"limits\"]\n"), "{rules}");
        assert!(!rules.contains("reviewer_fixes = 2"), "{rules}");
        assert_eq!(Limits::from_toml(&rules).unwrap().reviewer_fixes, 1);
    }
}
