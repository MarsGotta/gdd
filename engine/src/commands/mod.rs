//! The commands. Each takes what it was called with and returns what it says ([`Out`]); `main.rs`
//! only parses the arguments, prints and journals.

use crate::backlog;
use crate::blueprint::Blueprint;
use crate::color;
use crate::gates;
use crate::git;
use crate::judged::GitView;
use crate::launch::{self, Adapter};
use crate::limits;
use crate::log::{self, Entry, new_event, step, str_of};
use crate::metrics;
use crate::out::{Fail, Out, R};
use crate::project::Project;
use crate::report::{self, RoleReport};
use crate::seal;
use crate::state::{self, LogKind, State, log_kind};
use crate::time;
use crate::verify;
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

mod brief;
mod counting;
mod events;
mod gating;
mod items;
mod launching;
mod measuring;
mod next;
mod own_steps;
mod reports;
mod sealing;
mod shipping;
mod verifying;
mod waiting;
mod zones;

pub use brief::*;
pub use counting::*;
pub use events::*;
pub use gating::*;
pub use items::*;
pub use launching::*;
pub use measuring::*;
pub use next::*;
pub use own_steps::*;
pub use reports::*;
pub use sealing::*;
pub use shipping::*;
pub use verifying::*;
pub use waiting::*;
pub use zones::*;

/// What every command shares: the project's root and how to find a change's branch.
#[derive(Debug, Clone, Default)]
pub struct Ctx {
    pub root: PathBuf,
    /// The agents' environment of the blueprint's `levels` to launch in.
    pub env: Option<String>,
    /// The change's branch, when it isn't `work/<id>`, `gdd-build/<id>` or `build/<id>`.
    pub branch: Option<String>,
    /// The commit to judge, instead of the branch's tip.
    pub head: Option<String>,
}

fn check_id(id: &str) -> R<()> {
    if crate::valid_id(id) {
        Ok(())
    } else {
        Err(Fail::usage(format!(
            "{id:?} is not an id: lowercase letters, digits and hyphens"
        )))
    }
}

struct Loaded {
    project: Project,
    bp: Blueprint,
    entries: Vec<Entry>,
    state: State,
}

/// `--head` names a commit and `--branch` a branch: a value that starts with `-` would reach git
/// as one of its options, and is refused.
fn check_refs(ctx: &Ctx) -> R<()> {
    for (flag, v) in [("--head", &ctx.head), ("--branch", &ctx.branch)] {
        if let Some(v) = v
            && v.starts_with('-')
        {
            return Err(Fail::usage(format!(
                "{flag} {v:?} starts with -: it names a commit or a branch, never an option of git"
            )));
        }
    }
    Ok(())
}

/// `--head` names the commit the state is derived on: one that names nothing is never the branch.
fn check_head(ctx: &Ctx) -> R<()> {
    if let Some(h) = &ctx.head
        && git::is_repo(&ctx.root)
        && git::commit_of(&ctx.root, h).is_none()
    {
        return Err(Fail::missing(
            h.clone(),
            format!("--head {h} names no commit here"),
        ));
    }
    Ok(())
}

fn load(ctx: &Ctx, id: &str) -> R<Loaded> {
    check_id(id)?;
    check_refs(ctx)?;
    let project = Project::open(&ctx.root);
    let bp = project.blueprint()?;
    check_head(ctx)?;
    let entries = project.read_log(id);
    let gv = GitView::for_log(
        &ctx.root,
        id,
        ctx.branch.as_deref(),
        ctx.head.as_deref(),
        &entries,
    );
    let state = state::derive(id, &bp, &entries, gv, time::now());
    Ok(Loaded {
        project,
        bp,
        entries,
        state,
    })
}

/// The state of a change again, after this command wrote to its log.
fn after(ctx: &Ctx, l: &Loaded, id: &str) -> State {
    let entries = l.project.read_log(id);
    let gv = GitView::for_log(
        &ctx.root,
        id,
        ctx.branch.as_deref(),
        ctx.head.as_deref(),
        &entries,
    );
    state::derive(id, &l.bp, &entries, gv, time::now())
}

/// Refuse a step the state doesn't allow yet, with what moves the work instead. `head` is the
/// commit a gates step would judge.
fn allowed(l: &Loaded, id: &str, step: &str, head: Option<&str>) -> R<()> {
    l.state
        .progress
        .allows_step(Some(&l.bp), step, head)
        .map_err(|(code, why)| Fail::refuse(code, why, way_on(l, id)))
}

/// Append one event the layer built, after the state allowed it. Returns its line.
fn write(l: &Loaded, id: &str, event: &Value) -> R<usize> {
    l.state
        .progress
        .allows(Some(&l.bp), event)
        .map_err(|(code, why)| Fail::refuse(code, why, way_on(l, id)))?;
    log::append(&l.project.log_path(id), event).map_err(|e| Fail {
        exit: 1,
        reason: "bad-shape".into(),
        lines: vec![format!("refused: {e}")],
        what: None,
    })
}

/// What moves the work after a refusal: the command of the next step, or, when the log is closed
/// or its next step has no command, what to do instead. Never nothing.
fn way_on(l: &Loaded, id: &str) -> String {
    let n = &l.state.next;
    if !n.command.is_empty() {
        return n.command.clone();
    }
    if n.step == "done" {
        if l.state.progress.kind == Some(LogKind::Blueprint) {
            return "the blueprint's log has steps of its own: gdd step blueprint <prepare, harden or exam>, gdd ask blueprint, gdd approve blueprint".into();
        }
        return format!(
            "{id} shipped and its log is closed: anything more is a new change, with an id of its own (gdd brief <new id> --paths <path>...)"
        );
    }
    format!("gdd next {id}")
}

/// What to do when a cap is spent, as `next` says it: who gives more room, and the command.
fn cap_way_on(id: &str, lim: &limits::Limits) -> String {
    let n = crate::next::cap_spent(id, lim);
    format!("{} Run: {}", n.say, n.command)
}

/// The paths that differ between two commits, or which two git couldn't compare.
fn changes(ctx: &Ctx, a: &str, b: &str) -> R<Vec<String>> {
    git::diff_names(&ctx.root, a, b)
        .map_err(|e| Fail::missing(format!("the changes from {a} to {b}"), e))
}

/// The backlog's path and its text, or that it is missing.
fn read_backlog(ctx: &Ctx) -> R<(PathBuf, String)> {
    let path = ctx.root.join("gdd/backlog.md");
    let text = std::fs::read_to_string(&path)
        .map_err(|_| Fail::missing("gdd/backlog.md", "gdd/backlog.md is missing"))?;
    Ok((path, text))
}

/// What comes next, and the command that runs it when there is one.
fn say_next(out: &mut Out, next: &state::Next) {
    out.say(format!("next: {}", next.say));
    if !next.command.is_empty() {
        out.say(format!("run: {}", next.command));
    }
}

/// Refuse where there is no git repository, saying what needed one.
fn need_repo(ctx: &Ctx, what: &str) -> R<()> {
    if git::is_repo(&ctx.root) {
        return Ok(());
    }
    Err(Fail::missing(
        "a git repository",
        format!("{what}: this is not a git repository"),
    ))
}

/// The words of the refusal when git can't write here.
pub const CANNOT_WRITE: &str = "git can't write a ref here: nothing can be sealed or built";

/// Refuse, before anything is sealed or built, when git can't write a ref in this repository.
fn git_writes(ctx: &Ctx) -> R<()> {
    if !git::is_repo(&ctx.root) {
        return Ok(());
    }
    git::can_write_ref(&ctx.root).map_err(|e| {
        let mut f = Fail::refuse(
            "cannot",
            CANNOT_WRITE,
            "work where git can write (its .git folder, and its refs), then run it again",
        );
        f.lines
            .push(format!("git said: {}", e.lines().last().unwrap_or("")));
        f
    })
}

/// The words of the refusal when git won't read the repository that is here.
pub const WONT_READ: &str = "git won't read the repository here: nothing can be checked against it";

/// Refuse, before any command runs, where there is a repository and git won't read it, as it won't
/// one another user owns: a checkout in a CI container, most often. Taken for no repository at
/// all, `verify` would check the logs alone, without git, and pass.
pub fn git_reads(ctx: &Ctx) -> Option<Fail> {
    let (said, safe) = git::refuses(&ctx.root)?;
    let instead = match safe {
        Some(top) => format!(
            "if this copy is the one to trust, tell git so (git config --global --add safe.directory {}), then run it again",
            top.display()
        ),
        None => format!(
            "git -C {} status says why: settle it, then run it again",
            ctx.root.display()
        ),
    };
    let mut f = Fail::refuse("cannot", WONT_READ, instead);
    f.lines.push(format!("git said: {said}"));
    Some(f)
}

/// The words of the refusal when git has nobody to write a commit as.
pub const NO_AUTHOR: &str = "git has no name and email to write a commit with here: nothing can be sealed, built or shipped";

/// What moves it: the name and the email are the person's to give, never made up.
const SET_AUTHOR: &str = "ask the person whose name and email the commits of this copy carry, set them in this repository (git config user.name \"<name>\", git config user.email \"<email>\"), then run it again";

/// Refuse, before a commit is written or a role that commits is launched, when git has no author
/// here. A machine nobody set up, such as a CI runner or a fresh container, has none.
fn git_commits(ctx: &Ctx) -> R<()> {
    if !git::is_repo(&ctx.root) {
        return Ok(());
    }
    git::identity(&ctx.root).map_err(|e| {
        let mut f = Fail::refuse("cannot", NO_AUTHOR, SET_AUTHOR);
        f.lines
            .push(format!("git said: {}", e.lines().last().unwrap_or("")));
        f
    })
}

fn obj(pairs: Vec<(&str, Value)>) -> Map<String, Value> {
    pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

fn run<F: FnOnce(&mut Out) -> R<()>>(command: &str, f: F) -> Out {
    let mut out = Out::new(command);
    match f(&mut out) {
        Ok(()) => out,
        Err(fail) => out.with_fail(fail),
    }
}
