//! The commands. Each takes what it was called with and returns what it says ([`Out`]); `main.rs`
//! only parses the arguments, prints and journals.

use crate::backlog;
use crate::blueprint::Blueprint;
use crate::color;
use crate::gates;
use crate::git;
use crate::launch::{self, Adapter};
use crate::limits;
use crate::log::{self, Entry, new_event, step, str_of};
use crate::metrics;
use crate::out::{Fail, Out, R};
use crate::project::Project;
use crate::report::{self, RoleReport};
use crate::seal;
use crate::state::{self, GitView, LogKind, State, log_kind};
use crate::time;
use crate::verify;
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

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

/// Append one event the layer built, after the state allowed it. Returns its line.
fn write(l: &Loaded, id: &str, event: &Value) -> R<usize> {
    if let Err((code, why)) = l.state.progress.allows(Some(&l.bp), event) {
        return Err(Fail::refuse(code, why, way_on(l, id)));
    }
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
    let n = state::cap_spent(id, lim);
    format!("{} Run: {}", n.say, n.command)
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

fn short(h: &str) -> &str {
    &h[..h.len().min(12)]
}

fn run<F: FnOnce(&mut Out) -> R<()>>(command: &str, f: F) -> Out {
    let mut out = Out::new(command);
    match f(&mut out) {
        Ok(()) => out,
        Err(fail) => out.with_fail(fail),
    }
}

// ----- next -----

pub fn next(ctx: &Ctx, id: Option<&str>) -> Out {
    run("next", |out| {
        let Some(id) = id else {
            return next_project(ctx, out);
        };
        let mut l = load(ctx, id)?;
        // A step that seals, builds, judges or ships needs a git that can write.
        if ["brief", "checks", "build", "gates", "ship"].contains(&l.state.next.step.as_str())
            && git_writes(ctx).is_err()
        {
            l.state.next = state::Next {
                step: "stopped".into(),
                say: format!(
                    "{CANNOT_WRITE}: work where git can write (its .git folder, and its refs), then ask again"
                ),
                command: format!("gdd next {id}"),
                blocked: Some("cannot".into()),
            };
        } else if ["brief", "checks", "build", "ship"].contains(&l.state.next.step.as_str())
            && git_commits(ctx).is_err()
        {
            // The same for a step that ends in a commit: of the layer's, or of the role it launches.
            l.state.next = state::Next {
                step: "stopped".into(),
                say: format!("{NO_AUTHOR}: {SET_AUTHOR}"),
                command: format!("gdd next {id}"),
                blocked: Some("cannot".into()),
            };
        }
        let s = &l.state;
        if l.entries.is_empty() {
            out.say(format!("{id} has no log yet"));
        }
        if let Some(c) = &s.progress.color {
            out.say(format!(
                "{id}: {c}{}, {} events",
                s.progress
                    .version
                    .map(|v| format!(", brief version {v}"))
                    .unwrap_or_default(),
                s.progress.events
            ));
        }
        if let Some(st) = &s.progress.start {
            out.say(format!(
                "base {}, start {}{}",
                s.progress.base.as_deref().map(short).unwrap_or("?"),
                short(st),
                s.git
                    .head
                    .as_deref()
                    .map(|h| format!(", head {}", short(h)))
                    .unwrap_or_default()
            ));
        }
        if !s.passed_on_head.is_empty() {
            out.say(format!(
                "passed on this head: {}",
                s.passed_on_head.join(", ")
            ));
        }
        if !s.limits.stop.is_empty() {
            out.say(format!("no room left: {}", s.limits.stop.join(", ")));
        }
        out.say(format!("next: {}", s.next.say));
        if !s.next.command.is_empty() {
            out.say(format!("run: {}", s.next.command));
        }
        out.fact("state", s.facts());
        if s.next
            .blocked
            .as_deref()
            .is_some_and(|b| b != "needs-person")
        {
            out.code = 1;
            out.refused = s.next.blocked.clone();
        }
        Ok(())
    })
}

fn next_project(ctx: &Ctx, out: &mut Out) -> R<()> {
    let project = Project::open(&ctx.root);
    if !project.has_blueprint() {
        out.say("there is no gdd/blueprint.md: the next moment is Prepare, with gdd-prepare");
        out.fact("blueprint", "missing");
        return Ok(());
    }
    let bp = project.blueprint()?;
    for p in bp.problems() {
        out.say(format!("the blueprint: {p}"));
    }
    let entries = project.read_log("blueprint");
    let approvals: Vec<&Value> = log::events(&entries)
        .into_iter()
        .map(|(_, v)| v)
        .filter(|v| log::kind(v) == "approval" && str_of(v, "of") == "blueprint")
        .collect();
    let now_hash = git::hash_object(&ctx.root, "gdd/blueprint.md").ok();
    let named = approvals
        .iter()
        .rev()
        .find(|a| a["files"]["gdd/blueprint.md"].is_string())
        .or(approvals.last());
    let approved = match named {
        None => "no approval of the blueprint in gdd/log/blueprint.jsonl".to_string(),
        Some(a) => match a["files"]["gdd/blueprint.md"].as_str() {
            Some(h) if Some(h) == now_hash.as_deref() => "the blueprint is approved as it is now".into(),
            Some(_) => "the blueprint changed after its last approval: it needs the person's approval again".into(),
            None => "the blueprint is approved; the approval names no hash of it".into(),
        },
    };
    out.say(approved.clone());
    out.fact("blueprint", approved);
    let text = std::fs::read_to_string(project.path("gdd/backlog.md")).unwrap_or_default();
    let read = backlog::read(&text);
    if project.path("gdd/backlog.md").is_file() {
        for p in &read.problems {
            out.say(format!(
                "can't read {}: fix it, then gdd backlog list",
                p.say()
            ));
        }
        if !read.problems.is_empty() {
            out.fact(
                "backlog_problems",
                json!(
                    read.problems
                        .iter()
                        .map(|p| json!({"line": p.line, "what": p.what}))
                        .collect::<Vec<_>>()
                ),
            );
        }
    }
    let items = read.items;
    let standing = items
        .iter()
        .find(|i| i.id == "standing-checks")
        .map(|i| i.state.clone());
    let set_up = standing.as_deref() == Some("done");
    out.say(match standing.as_deref() {
        Some("done") => "the standing checks are set up".to_string(),
        Some(s) => format!("the standing checks are {s}: they come before any change"),
        None => "the backlog has no standing-checks item".into(),
    });
    out.fact("standing_checks", json!(standing));
    let now = time::now();
    let mut open = vec![];
    let mut under_way = vec![];
    for lid in project.log_ids() {
        let e = project.read_log(&lid);
        let mut p = state::Progress::new(&lid);
        for (line, v) in log::events(&e) {
            p.apply(line, v);
        }
        if let Some(a) = p.open_ask() {
            open.push((
                a.ts.unwrap_or(now),
                lid.clone(),
                a.of.clone(),
                a.what.clone(),
            ));
        }
        if log_kind(&lid) != LogKind::Blueprint && p.shipped.is_none() && p.events > 0 {
            let s = state::derive(
                &lid,
                &bp,
                &e,
                GitView::for_log(&ctx.root, &lid, None, None, &e),
                now,
            );
            under_way.push(json!({"id": lid, "next": s.next.step, "command": s.next.command, "blocked": s.next.blocked}));
            out.say(match &s.next.blocked {
                Some(b) => format!("under way: {lid}, stopped ({b}): {}", s.next.command),
                None => format!("under way: {lid}, next {}: {}", s.next.step, s.next.command),
            });
        }
    }
    open.sort();
    for (t, lid, of, what) in &open {
        out.say(format!(
            "waiting {} for the person: {lid}, {of}: {what}",
            time::waited(now - t)
        ));
    }
    out.fact(
        "waiting",
        json!(open.iter().map(|(t, lid, of, what)| json!({"id": lid, "of": of, "what": what, "seconds": now - t})).collect::<Vec<_>>()),
    );
    out.fact("under_way", json!(under_way));
    match backlog::next(&items) {
        Some(i) if set_up || i.id == "standing-checks" => {
            out.say(format!(
                "next in the backlog: {} ({}, {}): {}",
                i.id, i.color, i.state, i.what
            ));
            out.fact("backlog_next", json!(i.id));
        }
        Some(i) => {
            out.say(format!(
                "next in the backlog, once the standing checks hold: {}",
                i.id
            ));
            out.fact("backlog_next", json!(i.id));
        }
        None => out.say("the backlog has nothing to do"),
    }
    Ok(())
}

// ----- log and its helpers -----

/// The command that writes an event of the contract, and what to call that event. Every event
/// and every step of the contract has one, which checks what it records.
fn command_for(id: &str, ev: &str, st: &str) -> Option<(String, String)> {
    let (what, command) = match (ev, st) {
        ("approval", _) => (
            "an approval".to_string(),
            format!(
                "gdd approve {id} --of <kind> --version <n> --who <person> --where <where> <file>..."
            ),
        ),
        ("waiver", _) => (
            "a waiver".into(),
            format!(
                "gdd waiver {id} --who <person> --check <check or limit> --why \"<why>\" --backlog <item>"
            ),
        ),
        ("ask", _) => (
            "a question to the person".into(),
            format!("gdd ask {id} --of <kind> \"<the question>\""),
        ),
        ("answer", _) => (
            "an answer".into(),
            format!("gdd answer {id} \"<their words>\""),
        ),
        ("correction", _) => (
            "a correction".into(),
            format!("gdd correct {id} --line <n> \"<what was wrong>\""),
        ),
        ("seal", _) => (
            "a move of the seal".into(),
            format!("gdd seal {id} <file>..., or gdd seal {id} --merge-main"),
        ),
        ("step", "brief") => (
            "a brief step".into(),
            format!(
                "gdd brief {id} --paths <path>... [<files>], or to drop the change: gdd brief {id} --stop \"<why>\""
            ),
        ),
        ("step", "checks" | "build" | "review" | "reading") => (
            format!("a {st} step"),
            format!(
                "gdd report {id} {st} <the role's last message>, or gdd launch {id} {st} --run"
            ),
        ),
        ("step", "gates") => ("a gates step".into(), format!("gdd gates {id}")),
        ("step", "ship") => (
            "a ship step".into(),
            format!("gdd ship {id} --merge, or gdd ship {id} --done <where>"),
        ),
        ("step", "prepare" | "harden" | "exam") => (
            format!("a {st} step"),
            format!("gdd step blueprint {st} ..."),
        ),
        _ => return None,
    };
    Some((what, command))
}

/// `gdd log` writes nothing. Every event of the contract has a command of its own, which checks
/// what it records, so an event an agent composes is refused, with the command that writes it.
/// An event composed by hand could move `<start>` or lower a color where no command would.
pub fn log_event(ctx: &Ctx, id: &str, json_text: &str) -> Out {
    run("log", |_out| {
        check_id(id)?;
        let v: Value = serde_json::from_str(json_text)
            .map_err(|e| Fail::usage(format!("the event isn't JSON: {e}")))?;
        let Value::Object(m) = v else {
            return Err(Fail::usage("the event isn't a JSON object"));
        };
        let ev = m.get("event").and_then(Value::as_str).unwrap_or("");
        let st = m.get("step").and_then(Value::as_str).unwrap_or("");
        // A hash composed by hand is checked first, so the refusal says what was wrong with it too.
        if let Some(Value::Object(files)) = m.get("files") {
            for (p, h) in files {
                let now = git::hash_object(&ctx.root, p).map_err(|_| {
                    Fail::missing(p.to_string(), format!("{p} is not in the working copy"))
                })?;
                if h.as_str() != Some(now.as_str()) {
                    return Err(Fail::refuse(
                        "hash-mismatch",
                        format!("{p} is {now} in the working copy, not {h}"),
                        "leave the hashes to the layer: gdd approve and gdd brief compute them",
                    ));
                }
            }
        }
        Err(match command_for(id, ev, st) {
            Some((what, command)) => Fail::refuse(
                "use-the-command",
                format!(
                    "{what} has a command of its own, which checks what it records: gdd log writes nothing"
                ),
                command,
            ),
            None => Fail::refuse(
                "use-the-command",
                format!(
                    "{:?} is not an event of the contract, and each of its events has a command of its own: gdd log writes nothing",
                    if st.is_empty() { ev } else { st }
                ),
                format!("gdd next {id} names the command of the next step"),
            ),
        })
    })
}

pub fn ask(ctx: &Ctx, id: &str, of: &str, what: &str) -> Out {
    run("ask", |out| {
        if !log::ASK_OF.contains(&of) {
            return Err(Fail::usage(format!(
                "--of is one of {}",
                log::ASK_OF.join(", ")
            )));
        }
        if what.trim().is_empty() || what.contains('\n') {
            return Err(Fail::usage("what is one line"));
        }
        let l = load(ctx, id)?;
        // A question about a brief, or about the change, waits for a brief to be logged; one about
        // a rule of the product is how a brief starts, and goes any time.
        let kind = log_kind(id);
        if (of == "brief" || of == "change")
            && kind == LogKind::Change
            && l.state.progress.brief_line.is_none()
        {
            return Err(Fail::refuse(
                "out-of-order",
                format!("no brief is logged for {id}: run gdd brief first"),
                format!(
                    "gdd brief {id} [<brief and scenario files>] --paths <path>... [--request \"<their words>\", or --request-file <a file with them>]"
                ),
            ));
        }
        if of == "brief" && kind == LogKind::Setup {
            return Err(Fail::refuse(
                "out-of-order",
                format!(
                    "{id} sets up checks and has no brief: the person reviews it with --of change, once its trial passes"
                ),
                format!("gdd next {id}"),
            ));
        }
        let ev = new_event(
            id,
            "ask",
            &time::now_string(),
            obj(vec![("of", json!(of)), ("what", json!(what.trim()))]),
        );
        let line = write(&l, id, &ev)?;
        out.say(format!(
            "wrote the ask, line {line}: send the message now, and the wait counts from here"
        ));
        let then = state::follow_up(id, of, l.state.progress.version.unwrap_or(1));
        out.say(format!("when they answer: {then}"));
        out.fact("then", json!(then));
        out.fact("line", line);
        Ok(())
    })
}

pub fn answer(ctx: &Ctx, id: &str, words: &str) -> Out {
    run("answer", |out| {
        let l = load(ctx, id)?;
        let Some(a) = l.state.progress.open_ask().cloned() else {
            return Err(Fail::refuse(
                "no-open-ask",
                format!("{id} has no question waiting for an answer"),
                "gdd waiting",
            ));
        };
        let ev = new_event(
            id,
            "answer",
            &time::now_string(),
            obj(vec![
                ("of", json!(a.of)),
                ("what", json!(words.trim())),
                ("ask", json!(a.line)),
            ]),
        );
        let line = write(&l, id, &ev)?;
        let waited = a.ts.map(|t| time::now() - t).unwrap_or(0);
        out.say(format!(
            "wrote their answer to line {}, line {line}, after {}",
            a.line,
            time::waited(waited)
        ));
        out.fact("line", line);
        out.fact("waited_seconds", waited);
        Ok(())
    })
}

pub struct Approval<'a> {
    pub of: &'a str,
    pub version: i64,
    pub who: &'a str,
    pub place: &'a str,
    pub files: &'a [String],
}

pub fn approve(ctx: &Ctx, id: &str, a: &Approval) -> Out {
    run("approve", |out| {
        if !log::APPROVAL_OF.contains(&a.of) {
            return Err(Fail::usage(format!(
                "--of is one of {}",
                log::APPROVAL_OF.join(", ")
            )));
        }
        let l = load(ctx, id)?;
        // A change is approved as it was judged: its files as its judged head holds them, which
        // the orchestrator's copy, on the main branch, may not have at all.
        let judged = (a.of == "change")
            .then(|| {
                judged_commit(&l)
                    .map(|(_, j)| j)
                    .or(l.state.git.head.clone())
            })
            .flatten();
        let mut files = Map::new();
        for f in a.files {
            let on_head = judged.as_ref().and_then(|j| git::blob_at(&ctx.root, j, f));
            let h = match on_head {
                Some(b) => {
                    if git::hash_object(&ctx.root, f).is_ok_and(|w| w != b) {
                        out.say(format!(
                            "{f}: the change's version, at {}, not the working copy's",
                            short(judged.as_deref().unwrap_or(""))
                        ));
                    }
                    b
                }
                None => git::hash_object(&ctx.root, f).map_err(|_| {
                    Fail::missing(
                        f.to_string(),
                        format!(
                            "{f} is not in the working copy{}",
                            if a.of == "change" {
                                ", nor at the change's head"
                            } else {
                                ""
                            }
                        ),
                    )
                })?,
            };
            if a.of == "brief"
                && let Some(sealed) = l.state.progress.brief_files.get(f)
                && sealed != &h
            {
                return Err(Fail::refuse(
                    "hash-mismatch",
                    format!(
                        "{f} in the working copy ({}) is not the version sealed in the brief step ({})",
                        short(&h),
                        short(sealed)
                    ),
                    "show the person the sealed version, or seal a new version first",
                ));
            }
            files.insert(f.clone(), json!(h));
        }
        if a.of == "brief" && !l.state.progress.brief_files.is_empty() {
            let missing: Vec<&String> = l
                .state
                .progress
                .brief_files
                .keys()
                .filter(|p| p.starts_with("features/") && !a.files.contains(p))
                .collect();
            if !missing.is_empty() {
                out.say(format!(
                    "note: the approval leaves out {}, which the brief seals",
                    missing
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
        let ev = new_event(
            id,
            "approval",
            &time::now_string(),
            obj(vec![
                ("who", json!(a.who)),
                ("of", json!(a.of)),
                ("version", json!(a.version)),
                ("files", Value::Object(files.clone())),
                ("where", json!(a.place)),
            ]),
        );
        let line = write(&l, id, &ev)?;
        out.say(format!(
            "wrote the approval of {}, line {line}, with {} files as the person saw them",
            a.of,
            files.len()
        ));
        for (f, h) in &files {
            out.say(format!("  {f} {}", h.as_str().unwrap_or("")));
        }
        out.fact("line", line);
        out.fact("files", Value::Object(files));
        Ok(())
    })
}

pub struct Waiver<'a> {
    pub who: &'a str,
    pub check: &'a str,
    pub why: &'a str,
    pub backlog: &'a str,
    pub limit: Option<&'a str>,
    pub to: Option<i64>,
}

pub fn waiver(ctx: &Ctx, id: &str, w: &Waiver) -> Out {
    run("waiver", |out| {
        if w.limit.is_some() != w.to.is_some() {
            return Err(Fail::usage(
                "a waiver of a limit takes both --limit and --to",
            ));
        }
        if let Some(lim) = w.limit
            && !crate::blueprint::CAP_KEYS.contains(&lim)
            && lim != "lines_per_change"
        {
            return Err(Fail::usage(format!(
                "--limit is a cap of the blueprint: {}",
                crate::blueprint::CAP_KEYS.join(", ")
            )));
        }
        if w.check == "review" && w.limit.is_none() {
            out.say("note: a reviewer's stop is never waived: it gets a fix and a new review");
        }
        check_id(w.backlog)?;
        let l = load(ctx, id)?;
        let mut m = obj(vec![
            ("who", json!(w.who)),
            ("check", json!(w.check)),
            ("why", json!(w.why)),
            ("backlog", json!(w.backlog)),
        ]);
        if let (Some(lim), Some(to)) = (w.limit, w.to) {
            m.insert("limit".into(), json!(lim));
            m.insert("to".into(), json!(to));
        }
        let ev = new_event(id, "waiver", &time::now_string(), m);
        let line = write(&l, id, &ev)?;
        out.say(format!("wrote the waiver, line {line}"));
        let path = l.project.path("gdd/backlog.md");
        if let Ok(text) = std::fs::read_to_string(&path)
            && !backlog::items(&text).iter().any(|i| i.id == w.backlog)
        {
            let item = backlog::Item {
                id: w.backlog.into(),
                what: format!(
                    "Go through what {id} skipped: {}",
                    w.limit.unwrap_or(w.check)
                ),
                color: l.state.progress.color.clone().unwrap_or_default(),
                state: "todo".into(),
                ..backlog::Item::default()
            };
            if let Ok(t) = backlog::add(&text, &item) {
                std::fs::write(&path, t).map_err(|e| crate::out::io(&path, e))?;
                out.say(format!("added {} to the backlog, so the change goes through what it skipped afterwards", w.backlog));
            }
        }
        out.fact("line", line);
        Ok(())
    })
}

pub fn correct(ctx: &Ctx, id: &str, line: usize, what: &str, void: bool) -> Out {
    run("correct", |out| {
        let l = load(ctx, id)?;
        if !l.entries.iter().any(|e| e.line == line) {
            return Err(Fail::usage(format!(
                "gdd/log/{id}.jsonl has no line {line}"
            )));
        }
        if void {
            // A closed log is closed: no line of it stops counting once the change shipped.
            if l.state.progress.shipped.is_some() {
                return Err(Fail::refuse(
                    "out-of-order",
                    format!("{id} shipped and its log is closed: no line of it is voided now"),
                    format!(
                        "say what was wrong without --void, which changes nothing of what counted: gdd correct {id} --line {line} \"<what was wrong>\""
                    ),
                ));
            }
            // A line that sent the change back is never voided: that would waive it.
            let target = l
                .entries
                .iter()
                .find(|e| e.line == line)
                .and_then(|e| e.event());
            if let Some(why) = target.and_then(log::never_voided) {
                return Err(Fail::refuse(
                    "never-voided",
                    format!("line {line} is {why}. Voiding it would waive it"),
                    way_on(&l, id),
                ));
            }
        }
        let mut m = obj(vec![("line", json!(line)), ("what", json!(what))]);
        if void {
            m.insert("void".into(), json!(true));
        }
        let ev = new_event(id, "correction", &time::now_string(), m);
        let n = log::append(&l.project.log_path(id), &ev).map_err(|e| {
            Fail::refuse(
                "bad-shape",
                e,
                format!("gdd verify {id}: it shows the log as it is, line by line"),
            )
        })?;
        out.say(format!(
            "wrote a correction of line {line}, line {n}{}; no line was rewritten",
            if void {
                ", which takes it out of the change's state. What it used still counts against the caps"
            } else {
                ""
            }
        ));
        out.fact("line", n);
        Ok(())
    })
}

// ----- brief -----

pub struct BriefArgs<'a> {
    pub paths: &'a [String],
    pub color: Option<&'a str>,
    pub why: &'a str,
    pub files: &'a [String],
    pub level: Option<&'a str>,
    pub skill_version: Option<&'a str>,
    /// The person's request, word for word.
    pub request: Option<&'a str>,
    /// A file that holds it, for words a shell would change.
    pub request_file: Option<&'a str>,
    /// Why the change is dropped: the brief step then ends it, with its `result` `stopped`.
    pub stop: Option<&'a str>,
}

/// A dropped change: a `brief` step whose `result` is `stopped`, with why. It moves nothing: no
/// color, no seal, no `<start>`. A new brief takes the change up again.
fn brief_stopped(l: &Loaded, id: &str, why: &str, a: &BriefArgs, out: &mut Out) -> R<()> {
    if !a.paths.is_empty() || !a.files.is_empty() || a.color.is_some() {
        return Err(Fail::usage(
            "--stop drops the change: it takes no --paths, no --color and no files",
        ));
    }
    let why = why.trim();
    if why.is_empty() {
        return Err(Fail::usage("--stop says why the change is dropped"));
    }
    let version = l.state.progress.version.unwrap_or(1);
    let level = a.level.unwrap_or("standard");
    let ev = new_event(
        id,
        "step",
        &time::now_string(),
        obj(vec![
            ("step", json!("brief")),
            ("version", json!(version)),
            (
                "report",
                report_obj(
                    "gdd-brief",
                    a.skill_version.or(Some(env!("CARGO_PKG_VERSION"))),
                    Some(level),
                    None,
                    round_of(&l.state, "brief"),
                    "stopped",
                    "none",
                    None,
                    None,
                ),
            ),
            ("files", json!({})),
            ("seal", Value::Null),
            (
                "stop",
                json!({"reason": why, "needs": "a new brief, to take it up again"}),
            ),
        ]),
    );
    let line = write(l, id, &ev)?;
    out.say(format!(
        "{id} is dropped: wrote the brief step, stopped, line {line}"
    ));
    out.fact("line", line);
    out.fact("next", "stopped");
    Ok(())
}

/// The `brief` step: the planned color from the paths, the base, the files with their hashes and
/// the seal's tip, which must hold those same files.
pub fn brief(ctx: &Ctx, id: &str, a: &BriefArgs) -> Out {
    run("brief", |out| {
        let l = load(ctx, id)?;
        if log_kind(id) == LogKind::Setup {
            return Err(Fail::refuse(
                "out-of-order",
                format!(
                    "{id} sets up checks and has no brief: its seal is made by the checks role, on gdd-seal/{id}"
                ),
                format!("gdd next {id}"),
            ));
        }
        if let Some(why) = a.stop {
            return brief_stopped(&l, id, why, a, out);
        }
        if a.paths.is_empty() {
            return Err(Fail::usage(
                "--paths lists every path the change will touch",
            ));
        }
        git_writes(ctx)?;
        let main = git::main_branch(&ctx.root);
        let seal_tip = l.state.git.seal_tip.clone();
        let base = match (&l.state.progress.base, &seal_tip) {
            (Some(b), _) => b.clone(),
            (None, Some(t)) => git::merge_base(&ctx.root, t, &main).ok_or(Fail::missing(
                format!("a commit both gdd-seal/{id} and {main} hold"),
                format!("the seal and {main} share no commit"),
            ))?,
            (None, None) => git::commit_of(&ctx.root, &main).ok_or(Fail::missing(
                format!("refs/heads/{main}"),
                format!("there is no {main} branch"),
            ))?,
        };
        let logged = l.state.progress.color.clone();
        let c = color::color_of(
            &ctx.root,
            &l.bp,
            "planned",
            a.paths,
            a.color,
            logged.as_deref(),
            a.why,
        )
        .map_err(Fail::usage)?;
        if let Some(asked) = a.color
            && crate::color_rank(asked) < crate::color_rank(&c.color)
        {
            out.say(format!(
                "the color is {}, not {asked}: judgment can raise the floor, never lower it",
                c.color
            ));
        }
        let mut files = Map::new();
        for f in a.files {
            let h = git::hash_object(&ctx.root, f).map_err(|_| {
                Fail::missing(f.to_string(), format!("{f} is not in the working copy"))
            })?;
            if let Some(tip) = &seal_tip {
                match git::blob_at(&ctx.root, tip, f) {
                    Some(b) if b == h => {}
                    Some(b) => {
                        return Err(Fail::refuse(
                            "seal-mismatch",
                            format!(
                                "{f} is {} in the working copy and {} on the seal",
                                short(&h),
                                short(&b)
                            ),
                            format!("gdd seal {id} {f}"),
                        ));
                    }
                    None => {
                        return Err(Fail::refuse(
                            "seal-mismatch",
                            format!("{f} is not on gdd-seal/{id}"),
                            format!("gdd seal {id} {}", a.files.join(" ")),
                        ));
                    }
                }
            }
            files.insert(f.clone(), json!(h));
        }
        if !files.is_empty() && seal_tip.is_none() {
            return Err(Fail::refuse(
                "not-sealed",
                "the brief and its scenarios are written but not sealed",
                format!("gdd seal {id} {}", a.files.join(" ")),
            ));
        }
        let version = if l.state.progress.brief_line.is_some() {
            let prev = l.state.progress.version.unwrap_or(1);
            if files.is_empty() { prev } else { prev + 1 }
        } else {
            1
        };
        let version = l
            .project
            .brief(id)
            .and_then(|(f, _)| f["version"].as_i64())
            .filter(|_| !files.is_empty())
            .unwrap_or(version);
        let level = a.level.map(str::to_string).unwrap_or_else(|| {
            if c.color == "red" {
                "deep".into()
            } else {
                "standard".into()
            }
        });
        let colorobj = json!({
            "on": "planned",
            "base": base,
            "paths": a.paths,
            "zones": c.zones.keys().collect::<Vec<_>>(),
            "uncovered": c.uncovered,
            "floor": c.floor,
            "color": c.color,
            "why": c.why,
        });
        let seal_field = if files.is_empty() {
            Value::Null
        } else {
            json!(seal_tip)
        };
        let ev = new_event(
            id,
            "step",
            &time::now_string(),
            obj(vec![
                ("step", json!("brief")),
                ("version", json!(version)),
                (
                    "report",
                    report_obj(
                        "gdd-brief",
                        // The skills and the layer are released together: its version stands for
                        // theirs when the orchestrator names none.
                        a.skill_version.or(Some(env!("CARGO_PKG_VERSION"))),
                        Some(&level),
                        None,
                        round_of(&l.state, "brief"),
                        &c.color,
                        "none",
                        None,
                        None,
                    ),
                ),
                ("color", colorobj),
                ("files", Value::Object(files.clone())),
                ("seal", seal_field),
            ]),
        );
        let mut ev = ev;
        let from_file = match a.request_file {
            Some(f) => Some(
                std::fs::read_to_string(f)
                    .map_err(|_| Fail::missing(f.to_string(), format!("{f} is missing")))?,
            ),
            None => None,
        };
        if let Some(r) = a
            .request
            .or(from_file.as_deref())
            .map(str::trim)
            .filter(|r| !r.is_empty())
        {
            ev["request"] = json!(r);
        }
        let line = write(&l, id, &ev)?;
        out.say(format!(
            "{id} is {} (floor {}), brief version {version}, base {}",
            c.color,
            c.floor,
            short(&base)
        ));
        for (z, ps) in &c.zones {
            out.say(format!("  zone {z}: {}", ps.join(", ")));
        }
        if !c.uncovered.is_empty() {
            out.say(format!(
                "  no zone covers {}: never green; propose its zone in the backlog",
                c.uncovered.join(", ")
            ));
        }
        out.say(format!("wrote the brief step, line {line}"));
        let after = after(ctx, &l, id);
        out.say(format!("next: {}", after.next.say));
        if !after.next.command.is_empty() {
            out.say(format!("run: {}", after.next.command));
        }
        out.fact("color", json!(c.color));
        out.fact("line", line);
        out.fact("next", json!(after.next.step));
        Ok(())
    })
}

#[allow(clippy::too_many_arguments)]
fn report_obj(
    skill: &str,
    version: Option<&str>,
    level: Option<&str>,
    model: Option<&str>,
    round: i64,
    result: &str,
    caught: &str,
    seconds: Option<f64>,
    tokens: Option<i64>,
) -> Value {
    json!({
        "skill": skill,
        "version": version,
        "level": level,
        "model": model,
        "round": round,
        "result": result,
        "caught": caught,
        "seconds": seconds,
        "tokens": tokens,
    })
}

/// Which brief this is for the change: one more than the brief steps so far.
fn round_of(s: &State, _st: &str) -> i64 {
    1 + s.progress.briefs
}

// ----- report -----

#[derive(Debug, Clone, Default)]
pub struct ReportArgs {
    pub model: Option<String>,
    pub level: Option<String>,
    pub seconds: Option<f64>,
    pub tokens: Option<i64>,
    pub proofs: Option<String>,
    pub claims: Option<String>,
    pub closed: Vec<(String, String)>,
    pub went: Vec<(String, String)>,
}

fn json_file(path: &Option<String>) -> R<Value> {
    match path {
        None => Ok(json!([])),
        Some(p) => {
            let t = std::fs::read_to_string(p)
                .map_err(|_| Fail::missing(p.to_string(), format!("{p} is missing")))?;
            let v: Value = serde_json::from_str(&t)
                .map_err(|e| Fail::usage(format!("{p} isn't JSON: {e}")))?;
            if !v.is_array() {
                return Err(Fail::usage(format!("{p} isn't a JSON list")));
            }
            Ok(v)
        }
    }
}

pub fn report(ctx: &Ctx, id: &str, st: &str, text: &str, a: &ReportArgs) -> Out {
    run("report", |out| {
        let l = load(ctx, id)?;
        let (line, _) = write_report(ctx, &l, id, st, text, a, out)?;
        out.fact("line", line);
        Ok(())
    })
}

fn level_of(l: &Loaded, role: &str, asked: Option<&str>) -> String {
    if let Some(a) = asked {
        return a.to_string();
    }
    let color = l
        .state
        .progress
        .color
        .clone()
        .unwrap_or_else(|| "yellow".into());
    let base =
        l.bp.color_level(&color)
            .unwrap_or_else(|| "standard".into());
    if role == "checks" && l.state.progress.checks_stopped == 1 && !l.state.progress.checks_done {
        return "deep".into();
    }
    base
}

fn write_report(
    ctx: &Ctx,
    l: &Loaded,
    id: &str,
    st: &str,
    text: &str,
    a: &ReportArgs,
    out: &mut Out,
) -> R<(usize, RoleReport)> {
    let skill = match st {
        "checks" => "gdd-checks",
        "build" => "gdd-build",
        "review" | "reading" => "gdd-review",
        _ => return Err(Fail::usage("the step is checks, build, review or reading")),
    };
    let r = report::parse(text).map_err(|e| {
        Fail::refuse(
            "no-report",
            format!("the role's last message: {e}"),
            format!("launch it again, naming its skill: gdd launch {id} {st}"),
        )
    })?;
    if r.skill != skill {
        return Err(Fail::refuse(
            "wrong-skill",
            format!("its report names {}, not {skill}", r.skill),
            format!("gdd launch {id} {st}"),
        ));
    }
    // What the state allows comes before anything git says about the step.
    let probe = new_event(
        id,
        "step",
        &time::now_string(),
        obj(vec![
            ("step", json!(st)),
            ("report", json!({"skill": skill, "result": r.result})),
        ]),
    );
    if let Err((code, why)) = l.state.progress.allows(Some(&l.bp), &probe) {
        return Err(Fail::refuse(code, why, way_on(l, id)));
    }
    // A round `gdd launch` would not have started isn't written either: a role run without it, with
    // a cap already spent. What counts is how the caps stood at the log's last line, which is when
    // the round began, so a build that ended after its time ran out is still written and judged.
    if (st == "checks" || st == "build")
        && let Some(then) = l.state.progress.last_ts
    {
        let events = log::events(&l.entries);
        let values: Vec<&Value> = events.iter().map(|(_, v)| *v).collect();
        let void = log::voided_events(&l.entries);
        let void: Vec<&Value> = void.iter().map(|(_, v)| *v).collect();
        let lim = limits::count_with(&l.bp, &values, &void, then);
        let full = lim.no_room();
        if !full.is_empty() {
            return Err(Fail::refuse(
                "cap-spent",
                format!(
                    "no new round: {} had no room left when this one began",
                    full.join(", ")
                ),
                cap_way_on(id, &l.state.limits),
            ));
        }
    }
    let level = level_of(l, st, a.level.as_deref());
    let model = a
        .model
        .clone()
        .or_else(|| model_for(l, &ctx.env, st, &level).ok().map(|(_, m, _)| m));
    let p = &l.state.progress;
    let round = 1 + match st {
        "checks" => p.checks_launches(),
        "build" => p.builds,
        "review" => p.reviews,
        _ => p.readings(),
    };
    let stopped = r.cannot.is_some() || r.result == "stopped";
    let result = if stopped {
        "stopped".to_string()
    } else if st == "review" || st == "reading" {
        r.verdict.clone().unwrap_or(r.result.clone())
    } else {
        r.result.clone()
    };
    let mut m = obj(vec![
        ("step", json!(st)),
        (
            "report",
            report_obj(
                skill,
                r.version.as_deref(),
                Some(&level),
                model.as_deref(),
                round,
                &result,
                &r.caught,
                a.seconds,
                a.tokens,
            ),
        ),
    ]);
    if let Some((reason, needs)) = &r.cannot {
        m.insert("stop".into(), json!({"reason": reason, "needs": needs}));
    }
    match st {
        "checks" if !stopped => {
            let tip = l.state.git.seal_tip.clone().ok_or(Fail::refuse(
                "not-sealed",
                format!("gdd-seal/{id} doesn't exist: the checks role seals its checks there"),
                format!("launch the checks role again, which seals on gdd-seal/{id}: gdd launch {id} checks"),
            ))?;
            // A setup log has no brief step: its seal was cut from the main branch, and that
            // commit is where it starts.
            let main = git::main_branch(&ctx.root);
            let cut = || git::merge_base(&ctx.root, &tip, &main);
            let start = match p.start.clone() {
                Some(s) => s,
                None => cut().ok_or(Fail::missing(
                    format!("a commit both gdd-seal/{id} and {main} hold"),
                    format!(
                        "gdd-seal/{id} shares no commit with {main}: it is cut from the main branch"
                    ),
                ))?,
            };
            // After a trial that sent a setup's checks back, what counts is what the seal holds
            // that the trial didn't judge. A merge of the main branch moves <start>: the fix can
            // be below it, sealed before the merge, or have come through main itself.
            let judged = p
                .last_review
                .as_ref()
                .and_then(|r| r.head.clone())
                .filter(|h| log_kind(id) == LogKind::Setup && git::is_ancestor(&ctx.root, h, &tip));
            let from = judged.unwrap_or_else(|| start.clone());
            // And what the seal held, unrecorded, when a `seal` event moved <start> past it: a
            // checks step voided by a correction and then a merge of the main branch, say.
            let pending: Vec<&(String, String)> = p
                .unrecorded
                .iter()
                .filter(|(a, b)| {
                    git::is_ancestor(&ctx.root, a, b) && git::is_ancestor(&ctx.root, b, &tip)
                })
                .collect();
            if tip == from && pending.is_empty() {
                return Err(Fail::refuse(
                    "not-sealed",
                    format!("gdd-seal/{id} didn't move: the checks role sealed nothing"),
                    format!("gdd launch {id} checks"),
                ));
            }
            if !start.is_empty() && !git::is_ancestor(&ctx.root, &start, &tip) {
                return Err(Fail::refuse(
                    "not-sealed",
                    format!(
                        "gdd-seal/{id} no longer contains {}: a seal only moves forward",
                        short(&start)
                    ),
                    format!(
                        "what was sealed was taken off the seal: tell the person, gdd ask {id} --of stop \"<what the seal lost>\""
                    ),
                ));
            }
            let mut changed = git::diff_names(&ctx.root, &from, &tip)
                .map_err(|e| Fail::missing(format!("the changes from {from} to {tip}"), e))?;
            for (a, b) in pending {
                for f in git::diff_names(&ctx.root, a, b)
                    .map_err(|e| Fail::missing(format!("the changes from {a} to {b}"), e))?
                {
                    if !changed.contains(&f) {
                        changed.push(f);
                    }
                }
            }
            let mut checks = Map::new();
            for f in changed.iter().filter(|f| !p.brief_files.contains_key(*f)) {
                if let Some(h) = git::blob_at(&ctx.root, &tip, f) {
                    checks.insert(f.clone(), json!(h));
                }
            }
            // Its proofs: --proofs overrides the json block of its last message.
            let proofs = if a.proofs.is_some() {
                json_file(&a.proofs)?
            } else if let Some(err) = &r.proofs_error {
                return Err(Fail::refuse(
                    "bad-proofs",
                    format!("the role's proofs can't be read: {err}"),
                    format!(
                        "pass them as a file: gdd report {id} checks <file> --proofs <json file>"
                    ),
                ));
            } else {
                r.proofs.clone().unwrap_or_else(|| json!([]))
            };
            let n = proofs.as_array().map(Vec::len).unwrap_or(0);
            m.insert(
                "seal".into(),
                json!({"ref": format!("gdd-seal/{id}"), "commit": tip, "checks": checks, "hidden": null, "proofs": proofs}),
            );
            // A setup log's color is the one the paths it sealed give, from where it was cut.
            if log_kind(id) == LogKind::Setup {
                let base = p.base.clone().or_else(cut).unwrap_or_else(|| start.clone());
                let paths = git::diff_names(&ctx.root, &base, &tip)
                    .map_err(|e| Fail::missing(format!("the changes from {base} to {tip}"), e))?;
                let c = color::color_of(
                    &ctx.root,
                    &l.bp,
                    "sealed",
                    &paths,
                    None,
                    p.color.as_deref(),
                    "",
                )
                .map_err(Fail::usage)?;
                out.say(format!("what it set up is {} (floor {})", c.color, c.floor));
                m.insert(
                    "color".into(),
                    json!({"on": "sealed", "base": base, "head": tip, "paths": paths, "zones": c.zones.keys().collect::<Vec<_>>(), "uncovered": c.uncovered, "floor": c.floor, "color": c.color, "why": c.why}),
                );
            }
            out.say(format!(
                "sealed at {}: {}; {n} proofs",
                short(&tip),
                checks.keys().cloned().collect::<Vec<_>>().join(", ")
            ));
            if n == 0 {
                out.say("its last message has no json block of proofs: they aren't in the log (or pass --proofs <file>)");
            }
        }
        "build" => {
            if !stopped && l.state.git.available && l.state.git.head.is_none() {
                return Err(Fail::missing(
                    format!("the branch of {id}"),
                    format!(
                        "no branch for {id} (work/{id}, gdd-build/{id} or build/{id}, or the one --branch names): the build left nothing the gates can judge"
                    ),
                ));
            }
            if !stopped
                && let (Some(h), Some(s)) = (&l.state.git.head, &p.start)
                && h == s
            {
                return Err(Fail::refuse(
                    "nothing-built",
                    "the build ended without CANNOT: and its branch has nothing past <start>: nothing was built",
                    format!(
                        "give it the request again, word for word, once: gdd launch {id} build"
                    ),
                ));
            }
            // What it says each check gave: --claims, or the json block of its last message.
            let claims = if a.claims.is_some() {
                json_file(&a.claims)?
            } else {
                let c: Vec<Value> = r
                    .list
                    .as_ref()
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter(|x| x.get("command").is_some_and(|c| c.is_string()))
                            .cloned()
                            .collect()
                    })
                    .unwrap_or_default();
                json!(c)
            };
            m.insert("claims".into(), claims);
        }
        "review" => {
            let Some(verdict) = r.verdict.clone().or(stopped.then(|| "stop".to_string())) else {
                return Err(Fail::refuse(
                    "no-verdict",
                    "its first word isn't pass, fix or stop: that isn't a pass",
                    format!("launch it again, once: gdd launch {id} review"),
                ));
            };
            let head = l.state.git.head.clone();
            // What a setup log's trial judges is all it sealed, from where its seal was cut.
            let start = if log_kind(id) == LogKind::Setup {
                p.base.clone().or(p.start.clone()).unwrap_or_default()
            } else {
                p.start.clone().unwrap_or_default()
            };
            let raise = r.color.as_ref().map(|c| c.0.as_str());
            let colorobj = match (&head, start.is_empty()) {
                (Some(h), false) => {
                    let touched = git::diff_names(&ctx.root, &start, h).map_err(|e| {
                        Fail::missing(format!("the changes from {start} to {h}"), e)
                    })?;
                    let c = color::color_of(
                        &ctx.root,
                        &l.bp,
                        "touched",
                        &touched,
                        raise,
                        None,
                        r.color.as_ref().map(|c| c.1.as_str()).unwrap_or(""),
                    )
                    .map_err(Fail::usage)?;
                    json!({"on": "touched", "start": start, "head": h, "zones": c.zones.keys().collect::<Vec<_>>(), "uncovered": c.uncovered, "floor": c.floor, "color": c.color, "why": c.why})
                }
                _ => {
                    json!({"on": "touched", "start": start, "head": head, "zones": [], "uncovered": [], "floor": null, "color": raise, "why": "no git: the reviewer's word"})
                }
            };
            m.insert("color".into(), colorobj);
            m.insert("verdict".into(), json!(verdict));
            // What it ran, from the json block of its last message: its commands, as it gave them.
            let ran: Vec<Value> = r
                .list
                .as_ref()
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter(|x| x.get("command").is_some_and(|c| c.is_string()))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            if ran.is_empty() {
                out.say("its last message has no json block of what it ran: the log can't show it");
            }
            m.insert("ran".into(), json!(ran));
            m.insert(
                "findings".into(),
                json!(
                    r.findings
                        .iter()
                        .map(|f| json!({"id": f.id, "where": f.place, "what": f.what}))
                        .collect::<Vec<_>>()
                ),
            );
            m.insert(
                "closed".into(),
                json!(
                    a.closed
                        .iter()
                        .map(|(i, o)| json!({"id": i, "outcome": o}))
                        .collect::<Vec<_>>()
                ),
            );
            if !r.notes.is_empty() {
                m.insert("notes".into(), json!(r.notes));
            }
            if !r.later.is_empty() {
                m.insert("later".into(), json!(r.later));
            }
        }
        "reading" => {
            m.insert(
                "findings".into(),
                json!(
                    r.findings
                        .iter()
                        .enumerate()
                        .map(|(i, f)| {
                            let went = a
                                .went
                                .iter()
                                .find(|(k, _)| *k == f.id)
                                .map(|(_, w)| w.clone())
                                .unwrap_or_else(|| "log".into());
                            json!({"id": f.id, "question": i + 1, "what": f.what, "went": went})
                        })
                        .collect::<Vec<_>>()
                ),
            );
        }
        _ => {}
    }
    let ev = new_event(id, "step", &time::now_string(), m);
    let line = write(l, id, &ev)?;
    out.say(format!(
        "wrote the {st} step, line {line}: {result}, round {round}, {level}{}",
        model
            .as_deref()
            .map(|m| format!(" on {m}"))
            .unwrap_or_default()
    ));
    if let Some((reason, _)) = &r.cannot {
        out.say(format!("it stopped: CANNOT: {reason}"));
    }
    for f in &r.findings {
        out.say(format!("  {} {} {}", f.id, f.place, f.what));
    }
    if !r.later.is_empty() {
        out.say(format!("LATER, for the backlog: {}", r.later.join("; ")));
    }
    if !r.missing.is_empty() {
        out.say(format!("MISSING, for the brief: {}", r.missing.join("; ")));
    }
    let after = after(ctx, l, id);
    out.say(format!("next: {}", after.next.say));
    if !after.next.command.is_empty() {
        out.say(format!("run: {}", after.next.command));
    }
    out.fact("result", json!(result));
    out.fact("round", round);
    out.fact("next", json!(after.next.step));
    Ok((line, r))
}

// ----- limits -----

pub fn limits_cmd(ctx: &Ctx, id: &str) -> Out {
    run("limits", |out| {
        check_id(id)?;
        check_refs(ctx)?;
        check_head(ctx)?;
        let project = Project::open(&ctx.root);
        let bp = project.blueprint()?;
        if !project.has_log(id) {
            return Err(Fail::missing(
                format!("gdd/log/{id}.jsonl"),
                format!("gdd/log/{id}.jsonl is missing"),
            ));
        }
        let entries = project.read_log(id);
        let events = log::events(&entries);
        let values: Vec<&Value> = events.iter().map(|(_, v)| *v).collect();
        let void = log::voided_events(&entries);
        let void: Vec<&Value> = void.iter().map(|(_, v)| *v).collect();
        let lim = limits::count_with(&bp, &values, &void, time::now());
        for t in lim.text() {
            out.say(t);
        }
        let skipped = entries
            .iter()
            .filter(|e| e.error.is_some() && !e.blank)
            .count();
        if skipped > 0 {
            out.say(format!(
                "{skipped} lines aren't events and weren't counted: gdd verify {id}"
            ));
        }
        if !void.is_empty() {
            out.say("lines voided by a correction decide nothing, but what they used still counts: a voided round is a round");
        }
        if let Value::Object(m) = lim.facts() {
            for (k, v) in m {
                out.fact(&k, v);
            }
        }
        // The builder's lines, which the counter leaves to git: from <start> to the change's head,
        // without the bookkeeping. One more line, beside the counter's, and never in `stop`.
        let mut p = state::Progress::new(id);
        for (line, v) in &events {
            p.apply(*line, v);
        }
        let gv = GitView::of(&ctx.root, id, ctx.branch.as_deref(), ctx.head.as_deref());
        if let (Some(start), Some(head)) = (&p.start, &gv.head)
            && let Some(used) = git::changed_lines(&ctx.root, start, head)
        {
            let (cap, raised) = match lim.raised.get("lines_per_change") {
                Some(r) => (Some(*r), true),
                None => (bp.cap(&lim.color, "lines_per_change"), false),
            };
            let past = cap.is_some_and(|c| used > c);
            out.say(match cap {
                Some(c) => format!(
                    "lines {used} of {c}{}{}",
                    if raised { ", raised by a waiver" } else { "" },
                    if past {
                        ", past it: a change this big is split"
                    } else {
                        ""
                    }
                ),
                None => format!("lines {used} (no cap)"),
            });
            out.fact("lines", json!({"used": used, "cap": cap, "raised": raised, "past": past, "start": start, "head": head}));
        }
        out.code = if lim.stop.is_empty() { 0 } else { 1 };
        if out.code == 1 {
            out.refused = Some("cap-spent".into());
        }
        Ok(())
    })
}

// ----- verify -----

pub fn verify_cmd(ctx: &Ctx, ids: &[String], all: bool, no_git: bool) -> Out {
    run("verify", |out| {
        let project = Project::open(&ctx.root);
        let bp = if project.has_blueprint() {
            Some(project.blueprint()?)
        } else {
            None
        };
        if bp.is_none() {
            out.say("there is no gdd/blueprint.md: the order of colors can't be checked");
        }
        let ids: Vec<String> = if all || ids.is_empty() {
            project.log_ids()
        } else {
            ids.to_vec()
        };
        for id in &ids {
            check_id(id)?;
            if !project.has_log(id) {
                return Err(Fail::missing(
                    format!("gdd/log/{id}.jsonl"),
                    format!("gdd/log/{id}.jsonl is missing"),
                ));
            }
        }
        let use_git = !no_git && git::is_repo(&ctx.root);
        if !no_git && !use_git {
            out.say("not a git repository: checking the logs alone, as with --no-git");
        }
        let main = git::main_branch(&ctx.root);
        let mut found = vec![];
        for id in &ids {
            let entries = project.read_log(id);
            let journal =
                crate::journal::read(&project.path(&format!("gdd/log/{id}.engine.jsonl")));
            let layer = crate::journal::lines_written(&journal, &entries);
            found.extend(verify::check_log(id, bp.as_ref(), &entries, &layer));
            if use_git {
                found.extend(verify::check_git(&ctx.root, id, &entries, &main));
            }
        }
        // The same event written in two logs.
        if ids.len() > 1 {
            let mut seen: std::collections::BTreeMap<String, (String, usize)> = Default::default();
            for id in &ids {
                for e in project.read_log(id) {
                    let Some(v) = e.event() else { continue };
                    let key = format!(
                        "{}|{}|{}|{}|{}|{}",
                        str_of(v, "ts"),
                        log::kind(v),
                        step(v),
                        str_of(v, "of"),
                        v["version"],
                        v["files"]
                    );
                    if let Some((other, line)) = seen.get(&key) {
                        if other != id {
                            found.push(verify::Finding {
                                log: id.clone(),
                                line: e.line,
                                level: "warning",
                                code: "written-twice",
                                what: format!("the same event as gdd/log/{other}.jsonl:{line}, written in two logs"),
                            });
                        }
                    } else {
                        seen.insert(key, (id.clone(), e.line));
                    }
                }
            }
        }
        let errors = found.iter().filter(|f| f.level == "error").count();
        let warnings = found.len() - errors;
        for f in &found {
            out.say(format!(
                "{} gdd/log/{}.jsonl:{}: {} ({})",
                f.level, f.log, f.line, f.what, f.code
            ));
        }
        out.say(format!(
            "{} logs, {errors} errors, {warnings} warnings{}",
            ids.len(),
            if use_git { "" } else { ", without git" }
        ));
        out.fact("logs", ids.len());
        out.fact("errors", errors);
        out.fact("warnings", warnings);
        out.fact("git", use_git);
        out.fact(
            "findings",
            json!(found.iter().map(|f| f.to_json()).collect::<Vec<_>>()),
        );
        if errors > 0 {
            out.code = 1;
            out.refused = Some("verify-failed".into());
        }
        Ok(())
    })
}

// ----- color and zones -----

pub fn color_cmd(ctx: &Ctx, id: Option<&str>, paths: &[String]) -> Out {
    run("color", |out| {
        let project = Project::open(&ctx.root);
        let bp = project.blueprint()?;
        let (on, list, logged, start, head) = if !paths.is_empty() {
            let logged = match id {
                Some(i) => load(ctx, i)?.state.progress.color,
                None => None,
            };
            ("planned", paths.to_vec(), logged, None, None)
        } else {
            let id = id.ok_or(Fail::usage(
                "give an id, for what it touched, or --paths, for what it will",
            ))?;
            let l = load(ctx, id)?;
            let start = l.state.progress.start.clone().ok_or(Fail::refuse(
                "out-of-order",
                format!("{id} has no brief step: no <start>"),
                format!("gdd brief {id} ..."),
            ))?;
            let head = l.state.git.head.clone().ok_or(Fail::missing(
                format!("the branch of {id}"),
                format!("no branch for {id} (work/{id}, gdd-build/{id} or build/{id}): pass --branch or --head"),
            ))?;
            let touched = git::diff_names(&ctx.root, &start, &head)
                .map_err(|e| Fail::missing(format!("the changes from {start} to {head}"), e))?;
            (
                "touched",
                touched,
                l.state.progress.color.clone(),
                Some(start),
                Some(head),
            )
        };
        let c = color::color_of(&ctx.root, &bp, on, &list, None, logged.as_deref(), "")
            .map_err(Fail::usage)?;
        let rose = logged
            .as_deref()
            .is_some_and(|l| crate::color_rank(&c.floor) > crate::color_rank(l));
        out.say(format!(
            "{on}: {} (floor {}){}",
            c.color,
            c.floor,
            if rose {
                format!(
                    ", higher than the {} logged: back to Brief",
                    logged.clone().unwrap_or_default()
                )
            } else {
                String::new()
            }
        ));
        if let (Some(s), Some(h)) = (&start, &head) {
            out.say(format!("what changed from {} to {}", short(s), short(h)));
        }
        for (z, ps) in &c.zones {
            out.say(format!("  zone {z}: {}", ps.join(", ")));
        }
        if !c.uncovered.is_empty() {
            out.say(format!(
                "  no zone covers: {} (never green)",
                c.uncovered.join(", ")
            ));
        }
        out.fact("on", on);
        out.fact("paths", json!(c.paths));
        out.fact("zones", json!(c.zones.keys().collect::<Vec<_>>()));
        out.fact("uncovered", json!(c.uncovered));
        out.fact("floor", json!(c.floor));
        out.fact("color", json!(c.color));
        out.fact("logged", json!(logged));
        out.fact("rose", rose);
        Ok(())
    })
}

pub fn zones_cmd(ctx: &Ctx) -> Out {
    run("zones", |out| {
        let project = Project::open(&ctx.root);
        let bp = project.blueprint()?;
        if !git::is_repo(&ctx.root) {
            return Err(Fail::missing(
                "a git repository",
                "zones are matched by git: this is not a git repository",
            ));
        }
        let r = color::zones(&ctx.root, &bp)
            .map_err(|e| Fail::missing("the files the zones' pathspecs match", e))?;
        for (z, spec, n) in &r.counts {
            out.say(format!("zone {z}: {spec} matches {n} tracked files"));
        }
        for (z, spec) in &r.empty {
            out.say(format!("the zone {z}'s {spec} matches no tracked file: a path that doesn't exist, or a pattern written wrong"));
        }
        if !r.uncovered.is_empty() {
            out.say(format!(
                "{} tracked files no zone covers, never green:",
                r.uncovered.len()
            ));
            for f in r.uncovered.iter().take(30) {
                out.say(format!("  {f}"));
            }
            if r.uncovered.len() > 30 {
                out.say(format!("  and {} more", r.uncovered.len() - 30));
            }
        }
        out.fact(
            "empty",
            json!(
                r.empty
                    .iter()
                    .map(|(z, s)| json!({"zone": z, "pathspec": s}))
                    .collect::<Vec<_>>()
            ),
        );
        out.fact("uncovered", json!(r.uncovered));
        out.say(format!(
            "{} pathspecs match nothing and {} tracked files no zone covers: a zone that names a folder that doesn't exist yet covers nothing",
            r.empty.len(),
            r.uncovered.len()
        ));
        Ok(())
    })
}

// ----- seal -----

pub fn seal_cmd(ctx: &Ctx, id: &str, files: &[String], merge_main: bool) -> Out {
    run("seal", |out| {
        let l = load(ctx, id)?;
        if log_kind(id) == LogKind::Setup && !merge_main {
            return Err(Fail::refuse(
                "out-of-order",
                format!(
                    "the seal of {id} is made by the checks role, on gdd-seal/{id}: {id} has no brief, and nothing else is sealed there"
                ),
                format!(
                    "gdd next {id}. What the person approves outside the role's work, such as a change to the blueprint, lands on the main branch as the constitution says, and reaches this seal with: gdd seal {id} --merge-main"
                ),
            ));
        }
        if !git::is_repo(&ctx.root) {
            return Err(Fail::missing(
                "a git repository",
                "sealing needs git: this is not a git repository",
            ));
        }
        git_writes(ctx)?;
        git_commits(ctx)?;
        let main = git::main_branch(&ctx.root);
        if merge_main {
            let old = l.state.git.seal_tip.clone();
            let tip = seal::merge_main(&ctx.root, id, &main)
                .map_err(|e| {
                    Fail::refuse(
                        "conflict",
                        e,
                        format!("settle it on gdd-seal/{id}: the orchestrator its brief and scenarios, the checks role its checks; then gdd seal {id} --merge-main again"),
                    )
                })?;
            if old.as_deref() == Some(tip.as_str()) {
                out.say(format!("gdd-seal/{id} already holds {main}: nothing moved"));
            } else {
                let line = log_seal_move(
                    &l,
                    id,
                    old.as_deref(),
                    &tip,
                    &format!("merged {main}"),
                    None,
                )?;
                out.say(format!("gdd-seal/{id} is at {} with {main} merged in, written to the log, line {line}: merge it into the change's branch, and the next gdd gates judges from it", short(&tip)));
                out.fact("line", line);
            }
            out.fact("commit", json!(tip));
            return Ok(());
        }
        let parent = match &l.state.git.seal_tip {
            Some(t) => t.clone(),
            None => match &l.state.progress.base {
                Some(b) => b.clone(),
                None => git::commit_of(&ctx.root, &main).ok_or(Fail::missing(
                    format!("refs/heads/{main}"),
                    format!("there is no {main} branch"),
                ))?,
            },
        };
        let msg = format!("Seal {id}: what judges it, before anyone builds");
        let s = seal::seal(&ctx.root, id, &parent, files, &msg)
            .map_err(|e| Fail::refuse("seal", e, format!("gdd next {id}")))?;
        if s.unchanged {
            out.say(format!(
                "gdd-seal/{id} already holds these files as they are, at {}",
                short(&s.commit)
            ));
        } else {
            let sealed: Map<String, Value> =
                s.files.iter().map(|(f, h)| (f.clone(), json!(h))).collect();
            let line = log_seal_move(
                &l,
                id,
                Some(&s.parent),
                &s.commit,
                &format!("sealed {}", files.join(", ")),
                Some(sealed),
            )?;
            out.say(format!(
                "sealed on gdd-seal/{id}: commit {} on top of {}, written to the log, line {line}",
                s.commit,
                short(&s.parent)
            ));
            out.fact("line", line);
        }
        for (f, h) in &s.files {
            out.say(format!("  {f} {h}"));
        }
        out.say(format!(
            "the checkout and its index were not touched; next: gdd brief {id} --paths ... {}",
            files.join(" ")
        ));
        out.fact("ref", json!(format!("gdd-seal/{id}")));
        out.fact("commit", json!(s.commit));
        out.fact(
            "files",
            json!(
                s.files
                    .iter()
                    .map(|(f, h)| (f.clone(), json!(h)))
                    .collect::<Map<_, _>>()
            ),
        );
        Ok(())
    })
}

/// Write that the seal moved: its ref, the commit it was at (or was cut from), the new one and why.
fn log_seal_move(
    l: &Loaded,
    id: &str,
    from: Option<&str>,
    to: &str,
    why: &str,
    files: Option<Map<String, Value>>,
) -> R<usize> {
    let mut m = obj(vec![
        ("ref", json!(format!("gdd-seal/{id}"))),
        ("from", json!(from)),
        ("to", json!(to)),
        ("why", json!(why)),
    ]);
    if let Some(f) = files {
        m.insert("files".into(), Value::Object(f));
    }
    let ev = new_event(id, "seal", &time::now_string(), m);
    write(l, id, &ev)
}

// ----- gates -----

/// The private folder of one run's outputs (see [`crate::runs`]); when the usual one can't be
/// trusted, it says so and uses a fresh one.
fn runs_dir(root: &Path, id: &str, what: &str, out: &mut Out) -> R<PathBuf> {
    let stamp = time::now_string().replace(':', "");
    let (dir, note) = crate::runs::for_run(root, id, what, &stamp)
        .map_err(|e| Fail::missing("a private folder for the outputs", e.to_string()))?;
    if let Some(n) = note {
        out.say(n);
    }
    Ok(dir)
}

/// The tip of `gdd-seal/<id>` as the remotes hold it, and the ref's name, when every remote that
/// has it agrees on one commit.
fn remote_seal(root: &Path, id: &str) -> Option<(String, String)> {
    let listed = git::out(
        root,
        &[
            "for-each-ref",
            "--format=%(objectname) %(refname:short)",
            "refs/remotes/",
        ],
    )
    .ok()?;
    let suffix = format!("/gdd-seal/{id}");
    let found: Vec<(&str, &str)> = listed
        .lines()
        .filter_map(|l| l.split_once(' '))
        .filter(|(_, name)| name.ends_with(&suffix))
        .collect();
    let (tip, name) = found.first()?;
    found
        .iter()
        .all(|(t, _)| t == tip)
        .then(|| (tip.to_string(), name.to_string()))
}

/// The `<start>` gates judge against, from git and not from the log: the tip of `gdd-seal/<id>`
/// (a remote's, when there is no local one), or, with nothing sealed, the commit of the main
/// branch the head was last brought up to (the
/// base, or a later one). The log's `<start>` must agree with it, else nothing is judged: a log
/// line can say anything, and a `<start>` that holds checks someone loosened passes the seal check.
fn judged_start(ctx: &Ctx, l: &Loaded, head: &str, out: &mut Out) -> R<String> {
    let id = &l.state.id;
    let p = &l.state.progress;
    let logged = p.start.clone().ok_or(Fail::refuse(
        "out-of-order",
        "no brief step: no <start>",
        way_on(l, id),
    ))?;
    let main = git::main_branch(&ctx.root);
    // In CI there is often no local gdd-seal/<id>: the platform's, as its remote holds it, is the one.
    let seal = l
        .state
        .git
        .seal_tip
        .clone()
        .map(|t| (t, format!("gdd-seal/{id}")))
        .or_else(|| remote_seal(&ctx.root, id));
    if let Some((tip, name)) = &seal {
        if tip != &logged {
            return Err(Fail::refuse(
                "seal-moved",
                format!(
                    "{name} is at {}, and the log's <start> is {}: the seal moved where the layer didn't record it, so nothing judges from it",
                    short(tip),
                    short(&logged)
                ),
                format!(
                    "the seal moves only through gdd seal {id} and the checks role's report (gdd report {id} checks); if nobody you launched moved it, tell the person"
                ),
            ));
        }
        return Ok(tip.clone());
    }
    if p.sealed_any || !p.sealed.is_empty() {
        return Err(Fail::refuse(
            "seal-missing",
            format!(
                "the log says gdd-seal/{id} is at {}, and git has no gdd-seal/{id}, nor one its remotes agree on: the gates judge from the seal, never from the log alone",
                short(&logged)
            ),
            format!(
                "fetch it as a local branch (git fetch <remote> gdd-seal/{id}:gdd-seal/{id}), then gdd gates {id}"
            ),
        ));
    }
    let mb = git::merge_base(&ctx.root, head, &main).ok_or(Fail::missing(
        format!("a commit both {} and {main} hold", short(head)),
        format!("{} shares no commit with {main}", short(head)),
    ))?;
    if mb != logged && !git::is_ancestor(&ctx.root, &logged, &mb) {
        return Err(Fail::refuse(
            "start-mismatch",
            format!(
                "nothing is sealed, so <start> is where {} meets {main}, {}; the log says {}, which is not there nor before it",
                short(head),
                short(&mb),
                short(&logged)
            ),
            format!("tell the person: the log of {id} doesn't match git"),
        ));
    }
    if mb != logged {
        out.say(format!(
            "nothing sealed and {main} merged in: <start> is {}",
            short(&mb)
        ));
    }
    Ok(mb)
}

pub fn gates_cmd(ctx: &Ctx, id: &str, at: Option<&Path>) -> Out {
    run("gates", |out| {
        let l = load(ctx, id)?;
        if !git::is_repo(&ctx.root) {
            return Err(Fail::missing(
                "a git repository",
                "the gates judge a commit: this is not a git repository",
            ));
        }
        if at.is_none() {
            git_writes(ctx)?;
        }
        // The commit it would judge goes into what the state is asked: gates that failed don't
        // run again on another commit before the round that moved the branch is reported.
        let mut asked = obj(vec![
            ("step", json!("gates")),
            ("start", json!("x")),
            ("where", json!("x")),
            ("gates", json!([])),
        ]);
        if let Some(h) = &l.state.git.head {
            asked.insert("head".into(), json!(h));
        }
        let probe = new_event(id, "step", &time::now_string(), asked);
        if let Err((code, why)) = l.state.progress.allows(Some(&l.bp), &probe) {
            return Err(Fail::refuse(code, why, way_on(&l, id)));
        }
        let head = l.state.git.head.clone().ok_or(Fail::missing(
            format!("the branch of {id}"),
            format!("no branch for {id} (work/{id}, gdd-build/{id} or build/{id}): pass --branch or --head"),
        ))?;
        if !l.state.limits.stop.is_empty() {
            return Err(Fail::refuse(
                "cap-spent",
                format!(
                    "{} no room left, and no finished build waits to be judged",
                    l.state.limits.stop.join(", ")
                ),
                cap_way_on(id, &l.state.limits),
            ));
        }
        if l.state.limits.spent {
            out.say("a cap has no room left, but a finished build is judged all the same: no new round starts after it");
        }
        let color = l
            .state
            .progress
            .color
            .clone()
            .unwrap_or_else(|| "yellow".into());
        let start = judged_start(ctx, &l, &head, out)?;
        if !git::is_ancestor(&ctx.root, &start, &head) {
            return Err(Fail::refuse(
                "wrong-start",
                format!(
                    "{} doesn't start from <start> {}",
                    short(&head),
                    short(&start)
                ),
                "merge the seal's tip into the change's branch first",
            ));
        }
        let touched = git::diff_names(&ctx.root, &start, &head)
            .map_err(|e| Fail::missing(format!("the changes from {start} to {head}"), e))?;
        let touched_by = |specs: &[String]| {
            git::match_paths(&ctx.root, &touched, specs)
                .map(|m| !m.is_empty())
                .unwrap_or(true)
        };
        // The standing gates of every zone it touches.
        let mut zone_checks = vec![];
        let (hit, _) = color::sort_paths(&ctx.root, &l.bp, &touched)
            .map_err(|e| Fail::missing("the zones of the paths it touched", e))?;
        for z in l.bp.zones() {
            if let (true, Some(file)) = (hit.contains_key(&z.id), &z.gates)
                && let Some(text) = git::show(&ctx.root, &start, file)
            {
                zone_checks.extend(gates::zone_gate_checks(&text));
            }
        }
        let plan = gates::plan(&l.bp, &color, &touched_by, &zone_checks);
        // A check whose `runs` is neither tier would be left out without a word, and the gates
        // could pass without it: a slip of the pen in the blueprint is no way to drop a check.
        if let Some(p) = plan
            .iter()
            .find(|p| p.check != "seal" && p.tier != "each round" && p.tier != "before review")
        {
            return Err(Fail::refuse(
                "bad-blueprint",
                format!(
                    "the check {} runs {:?} in the blueprint, which is neither `each round` nor `before review`: as written it would never run",
                    p.check, p.tier
                ),
                format!(
                    "write it right in gdd/blueprint.md, which the person approves; then gdd gates {id}"
                ),
            ));
        }
        let sealed: Vec<String> = l.state.progress.sealed.keys().cloned().collect();
        // Every check that runs gets what the layer knows in its command: <start> and <base>,
        // hashes git resolved, and the sealed paths. A placeholder left over is refused before
        // anything runs: as written, it would fail on every change, or judge nothing.
        let base = l
            .state
            .progress
            .base
            .clone()
            .filter(|b| log::is_hash(b) && git::commit_of(&ctx.root, b).is_some());
        let fill = gates::Fill {
            start: &start,
            base: base.as_deref(),
            sealed: &sealed,
        };
        let mut commands: std::collections::BTreeMap<String, String> = Default::default();
        for p in plan.iter().filter(|p| p.skipped.is_none()) {
            let raw = if p.check == "seal" {
                gates::seal_command(&l.bp, &start, &sealed)
            } else {
                p.command.clone()
            };
            let filled = gates::fill(&raw, &fill).map_err(|held| {
                Fail::refuse(
                    "unfilled",
                    format!(
                        "the command of the check {} holds {held}, which the layer doesn't fill (it fills <start>, <base> and <sealed paths>): run as written, it would judge nothing",
                        p.check
                    ),
                    format!(
                        "write the check's real command in gdd/blueprint.md, from the constitution, and have the person approve the blueprint; then gdd gates {id}"
                    ),
                )
            })?;
            commands.insert(p.check.clone(), filled);
        }
        // Where it runs: a throwaway worktree of exactly that commit, or a directory checked first.
        let tmp;
        let (dir, place) = match at {
            Some(d) => {
                git::clean_at(d, &head).map_err(|e| {
                    Fail::refuse(
                        "wrong-commit",
                        format!("{}: {e}", d.display()),
                        "check out the change's head in a clean directory, or leave out --at",
                    )
                })?;
                (
                    d.to_path_buf(),
                    if std::env::var("CI").is_ok() {
                        "ci"
                    } else {
                        "a checked directory"
                    },
                )
            }
            None => {
                tmp = tempfile::Builder::new()
                    .prefix("gdd-gates-")
                    .tempdir()
                    .map_err(|e| {
                        Fail::missing("a temporary folder for the gates", e.to_string())
                    })?;
                let d = tmp.path().join("tree");
                git::worktree_add(&ctx.root, &d, &head)
                    .map_err(|e| Fail::missing(format!("a worktree of {head}"), e))?;
                (d, "a fresh worktree")
            }
        };
        let outputs = match runs_dir(&ctx.root, id, "gates", out) {
            Ok(d) => d,
            Err(e) => {
                if at.is_none() {
                    let _ = git::worktree_remove(&ctx.root, &dir);
                }
                return Err(e);
            }
        };
        // A fresh copy has only what git holds: the blueprint's `install` gives it what the
        // checks need. If it fails, the copy can't judge anything, and that isn't the change
        // failing: nothing is written and no round is counted.
        let fresh = at.is_none();
        let install = l.bp.install().filter(|_| fresh);
        let mut installed = None;
        if let Some(cmd) = &install {
            let r = gates::run_check(
                &dir,
                "install",
                cmd,
                "install",
                &outputs.join("install.out"),
            );
            out.say(format!(
                "{:<10} {} exit {} in {} s",
                "install",
                if r.passed { "done" } else { "FAILED" },
                r.exit
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "none".into()),
                r.seconds.unwrap_or(0.0),
            ));
            if !r.passed {
                let _ = git::worktree_remove(&ctx.root, &dir);
                return Err(Fail::refuse(
                    "environment",
                    format!(
                        "the fresh copy couldn't be prepared: the blueprint's install, `{cmd}`, ended with exit {}: {}. No check ran and nothing was written: its output is in {}",
                        r.exit
                            .map(|e| e.to_string())
                            .unwrap_or_else(|| "none".into()),
                        r.tail.lines().last().unwrap_or(""),
                        outputs.join("install.out").display()
                    ),
                    format!(
                        "if it is the environment (no network, a registry that is down), try once more, or run the gates where the dependencies are installed: gdd gates {id} --at <a directory with {} checked out, and clean>. If the change broke it, it is the builder's to fix: tell the person",
                        short(&head)
                    ),
                ));
            }
            installed = Some(json!({"command": cmd, "seconds": r.seconds, "exit": r.exit}));
        }
        let mut ran: Vec<gates::Ran> = vec![];
        let mut skipped = vec![];
        let mut stop_rest = false;
        for tier in ["seal", "each round", "before review"] {
            if stop_rest {
                break;
            }
            for p in plan.iter().filter(|p| {
                if tier == "seal" {
                    p.check == "seal"
                } else {
                    p.check != "seal" && p.tier == tier
                }
            }) {
                if let Some(why) = &p.skipped {
                    skipped.push(json!({"check": p.check, "why": why}));
                    out.say(format!("{:<10} skipped: {why}", p.check));
                    continue;
                }
                if tier == "before review" && ran.iter().any(|r| !r.passed) {
                    break;
                }
                let command = commands.get(&p.check).cloned().unwrap_or_default();
                let r = gates::run_check(
                    &dir,
                    &p.check,
                    &command,
                    &p.tier,
                    &outputs.join(format!("{}.out", p.check)),
                );
                out.say(format!(
                    "{:<10} {} exit {} in {} s{}",
                    r.check,
                    if r.passed { "pass" } else { "FAIL" },
                    r.exit
                        .map(|e| e.to_string())
                        .unwrap_or_else(|| "none".into()),
                    r.seconds.unwrap_or(0.0),
                    if r.passed || r.tail.is_empty() {
                        String::new()
                    } else {
                        format!(": {}", r.tail.lines().last().unwrap_or(""))
                    }
                ));
                let failed_seal = r.check == "seal" && !r.passed;
                ran.push(r);
                if failed_seal {
                    out.say("the seal failed: what judges the change was touched, so nothing else counts");
                    stop_rest = true;
                    break;
                }
            }
            if tier == "each round" && ran.iter().any(|r| !r.passed) {
                break;
            }
        }
        if at.is_none() {
            let _ = git::worktree_remove(&ctx.root, &dir);
        }
        // "Command not found" in a fresh copy nobody prepared is the copy lacking what the check
        // needs, not the change failing: nothing is written, and no round is counted.
        if fresh
            && install.is_none()
            && let Some(r) = ran
                .iter()
                .find(|r| r.check != "seal" && r.exit == Some(127))
        {
            return Err(Fail::refuse(
                "environment",
                format!(
                    "the check {} ended with exit 127, command not found: the fresh copy lacks what it needs, such as the project's dependencies. It judged nothing, and nothing was written",
                    r.check
                ),
                format!(
                    "run the gates where the dependencies are installed: gdd gates {id} --at <a directory with {} checked out, and clean>. For good, the blueprint names the command that installs them (install: <command>), a change to the blueprint the person approves",
                    short(&head)
                ),
            ));
        }
        let passed = ran.iter().all(|r| r.passed);
        let mut m = obj(vec![
            ("step", json!("gates")),
            ("start", json!(start)),
            ("where", json!(place)),
            ("head", json!(head)),
            (
                "gates",
                json!(ran.iter().map(|r| r.to_json()).collect::<Vec<_>>()),
            ),
        ]);
        if !skipped.is_empty() {
            m.insert("skipped".into(), json!(skipped));
        }
        if let Some(i) = installed {
            m.insert("install".into(), i);
        }
        let ev = new_event(id, "step", &time::now_string(), m);
        let line = write(&l, id, &ev)?;
        out.say(format!(
            "judged {} from {} in {place}; wrote the gates step, line {line}; outputs in {}",
            short(&head),
            short(&start),
            outputs.display()
        ));
        let after = after(ctx, &l, id);
        let lim_after = &after.limits;
        if !lim_after.slow.is_empty() {
            out.say(format!(
                "past its seconds, for the backlog: {}",
                lim_after.slow.join(", ")
            ));
        }
        out.say(format!("next: {}", after.next.say));
        if !after.next.command.is_empty() {
            out.say(format!("run: {}", after.next.command));
        }
        out.fact("head", json!(head));
        out.fact("start", json!(start));
        out.fact("passed", passed);
        out.fact("line", line);
        out.fact("checks", json!(ran.iter().map(|r| json!({"check": r.check, "result": if r.passed {"pass"} else {"fail"}, "seconds": r.seconds, "exit": r.exit})).collect::<Vec<_>>()));
        out.fact("next", json!(after.next.step));
        if !passed {
            out.code = 1;
        }
        Ok(())
    })
}

// ----- launch -----

/// The environment, the model and the launch of a role, from the blueprint's `levels`.
fn model_for(
    l: &Loaded,
    env: &Option<String>,
    role: &str,
    level: &str,
) -> R<(String, String, Option<String>)> {
    let envs = l.bp.environments();
    let wanted = env.clone().or_else(|| std::env::var("GDD_ENV").ok());
    let e = match wanted {
        Some(w) => envs.iter().find(|e| e.name == w).ok_or(Fail::refuse(
            "no-environment",
            format!(
                "the blueprint's levels don't list the environment {w}: stop and ask the person"
            ),
            "the blueprint's levels name each environment, and gdd-prepare sets them: ask the person to add this one, or pass --env with one it names",
        ))?,
        None if envs.len() == 1 => &envs[0],
        None if envs.is_empty() => {
            return Err(Fail::refuse(
                "no-environment",
                "the blueprint has no levels",
                "gdd-prepare sets the blueprint's levels, which the person approves: ask them, and stop until then",
            ));
        }
        None => {
            return Err(Fail::usage(format!(
                "say which environment with --env: {}",
                envs.iter()
                    .map(|e| e.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
    };
    let models = match (&e.reviewer, role) {
        (Some(r), "review" | "reading") => r,
        _ => &e.models,
    };
    let model = models.of(level).ok_or(Fail::refuse(
        "no-model",
        format!(
            "the blueprint gives {} no model for {level}: stop and ask, never use another",
            e.name
        ),
        "ask the person to name the model in the blueprint's levels (gdd-prepare), or pass --level with one it names",
    ))?;
    let launch = models.launch.clone().or(e.models.launch.clone());
    Ok((e.name.clone(), model.to_string(), launch))
}

/// What a setup log sets up, in words for its roles: `standing-checks`, `standing-checks-<check>`
/// or `gates-<zone>`.
fn setup_subject(id: &str) -> (String, String) {
    if let Some(check) = id.strip_prefix("standing-checks-") {
        (
            format!("the check {check}, which the color of a new zone asks for"),
            format!("the new check {check}"),
        )
    } else if let Some(zone) = id.strip_prefix("gates-") {
        let zone = match zone.rsplit_once('-') {
            Some((z, n)) if n.chars().all(|c| c.is_ascii_digit()) => z,
            _ => zone,
        };
        (
            format!(
                "the gate of the zone {zone}: the check of each new `Rule:` in gdd/gates/{zone}.feature, built from its scenarios"
            ),
            format!("the new gate of the zone {zone}"),
        )
    } else {
        (
            "the standing checks the blueprint's `checks` name, from the blueprint and the constitution".into(),
            "the new standing checks".into(),
        )
    }
}

/// Whether the project has code yet: a tracked file that is not one of its GDD files, its
/// scenarios, its agents' files, its CI, its documents or its data.
fn has_code(root: &Path) -> bool {
    let Ok(files) = git::ls_files(root, &[]) else {
        return true;
    };
    files.iter().any(|path| {
        const NOT_CODE: [&str; 8] = [
            "gdd/",
            "features/",
            "data/",
            ".agents/",
            ".claude/",
            ".codex/",
            ".gdd/",
            ".github/",
        ];
        if NOT_CODE.iter().any(|d| path.starts_with(d)) {
            return false;
        }
        let name = path.rsplit('/').next().unwrap_or(path);
        match name.rsplit_once('.') {
            // README, LICENSE, CODEOWNERS: documents, not code.
            None => !name.chars().all(|c| c.is_ascii_uppercase() || c == '_'),
            Some(("", _)) => false,
            Some((_, ext)) => !matches!(
                ext.to_ascii_lowercase().as_str(),
                "md" | "txt"
                    | "json"
                    | "csv"
                    | "tsv"
                    | "yaml"
                    | "yml"
                    | "xml"
                    | "toml"
                    | "png"
                    | "jpg"
                    | "jpeg"
                    | "gif"
                    | "svg"
                    | "webp"
                    | "pdf"
            ),
        }
    })
}

fn prompt_for(l: &Loaded, id: &str, role: &str, _env: &str, level: &str) -> String {
    let p = &l.state.progress;
    let mut s = vec![];
    let setup = log_kind(id) == LogKind::Setup;
    let root = std::fs::canonicalize(&l.project.root).unwrap_or_else(|_| l.project.root.clone());
    let log_path = root.join(Project::log_rel(id));
    // A role loads its skill from another folder, and one was seen starting its work there instead
    // of in the project: say where the project is.
    let at = format!("The project is the folder {}.", root.display());
    if setup {
        s.push(format!(
            "Load {} and do its work for the project's {id} of this GDD project (contract gdd/1), and nothing else.",
            launch::skill_ref(role)
        ));
        s.push(at);
        let (what, tried) = setup_subject(id);
        let seal = l.state.git.seal_tip.clone().or(p.start.clone());
        match role {
            "checks" => {
                let on = if l.state.git.seal_tip.is_some() {
                    format!("on gdd-seal/{id}, on top of its tip")
                } else {
                    format!("on gdd-seal/{id}, cut from the main branch")
                };
                s.push(format!(
                    "Set up {what}: each one with a tool that already exists before a script of the project's own, proved once in a throwaway clone, its bad case planted and seen failing, then seen passing without it, and sealed {on}."
                ));
                // A fix round after a trial that sent the checks back or said stop: what it found,
                // and what the person answered when they were told.
                let events = log::events(&l.entries);
                if let Some(r) = &p.last_review
                    && matches!(r.verdict.as_str(), "fix" | "stop")
                    && let Some((_, v)) = events.iter().find(|(line, _)| *line == r.line)
                {
                    s.push(format!(
                        "A fix round. The trial said {}:",
                        r.verdict
                    ));
                    for f in v["findings"].as_array().cloned().unwrap_or_default() {
                        s.push(format!(
                            "- {} {}: {}",
                            str_of(&f, "id"),
                            str_of(&f, "where"),
                            str_of(&f, "what")
                        ));
                    }
                    if let Some((_, a)) = events
                        .iter()
                        .rev()
                        .find(|(line, v)| *line > r.line && log::kind(v) == "answer")
                    {
                        s.push(format!(
                            "The person's answer, word for word: \"{}\"",
                            str_of(a, "what")
                        ));
                    }
                }
            }
            "review" => s.push(format!(
                "Try {tried} on gdd-seal/{id}{}, in a fresh clone: follow references/standing-checks.md of the skill. Each check three ways, a round each: it rejects a planted bad case, it accepts without it, and a change can't switch it off. When the blueprint names its gates, read each gate's feature against the blueprint too. Its log is {}.",
                seal.map(|c| format!(", at {c}")).unwrap_or_default(),
                log_path.display()
            )),
            _ => {}
        }
        if id == "standing-checks" && (role == "checks" || role == "review") && !has_code(&root) {
            s.push(
                "The project has no code yet: only the seal and the secret scan are set up now. Every other check waits with its `when` in the blueprint, and is set up by its own item, standing-checks-<check>, once a change brings what it watches."
                    .into(),
            );
        }
        if let Some(b) = l.state.git.main_tip.clone() {
            s.push(format!("<base> is {b}, the tip of the main branch."));
        }
        s.push(
            "It has no brief, and its color is the one its paths give: the files of the checks, of CI and of each tool's configuration."
                .into(),
        );
        s.push(format!("Work at the {level} level."));
    } else {
        s.push(format!(
            "Load {} and do its work for the change {id} of this GDD project (contract gdd/1), and nothing else.",
            launch::skill_ref(role)
        ));
        s.push(at);
        if role == "reading" {
            s.push("You are to read a brief before anyone builds it: follow references/brief.md of the skill.".into());
        }
        let brief = format!("gdd/briefs/{id}.md");
        if l.project.path(&brief).is_file() || !p.brief_files.is_empty() {
            let scen: Vec<&String> = p
                .brief_files
                .keys()
                .filter(|f| f.starts_with("features/"))
                .collect();
            s.push(format!(
                "The brief: {brief}{}{}.",
                p.version
                    .map(|v| format!(", version {v}"))
                    .unwrap_or_default(),
                if scen.is_empty() {
                    String::new()
                } else {
                    format!(
                        ", with its scenarios in {}",
                        scen.iter()
                            .map(|x| x.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
            ));
            if let Some(r) = &p.request
                && (role == "build" || role == "review")
            {
                s.push(format!("The person's request, word for word: \"{r}\""));
            }
        } else {
            match &p.request {
                Some(r) => s.push(format!("There is no brief. The person's request, word for word: \"{r}\"")),
                None => s.push("There is no brief, and the log keeps no request: ask the orchestrator for it, word for word, before you start.".into()),
            }
            if !p.paths.is_empty() {
                s.push(format!("The paths it touches: {}.", p.paths.join(", ")));
            }
        }
        if let Some(b) = &p.base {
            s.push(format!("<base> is {b}."));
        }
        if let Some(st) = &p.start {
            s.push(format!("<start> is {st}."));
        }
        match &p.color {
            Some(c) => s.push(format!("The color is {c}, at the {level} level.")),
            None => s.push(format!("Work at the {level} level.")),
        }
        let branch = l
            .state
            .git
            .branch
            .clone()
            .unwrap_or_else(|| format!("work/{id}"));
        match role {
            "checks" => s.push(format!("Seal what you write on gdd-seal/{id}, on top of its tip.")),
            "build" => s.push(format!(
                "Work on the branch {branch}, which starts at <start>, in a worktree of your own inside the project folder, and commit there."
            )),
            "review" => s.push(format!(
                "Review the change at {}, in a fresh clone. Its log is {}.",
                l.state.git.head.clone().unwrap_or_else(|| format!("the tip of {branch}")),
                log_path.display()
            )),
            _ => {}
        }
    }
    let zones: Vec<String> = p.zones.clone();
    let ctx_files: Vec<String> =
        l.bp.context()
            .into_iter()
            .filter(|c| {
                c.targets.iter().any(|t| {
                    zones
                        .iter()
                        .any(|z| t == z || t.starts_with(&format!("{z}/")))
                })
            })
            .map(|c| format!("{} {}", c.kind, c.name))
            .collect();
    if !ctx_files.is_empty() {
        s.push(format!(
            "The context the blueprint gives its gates: {}.",
            ctx_files.join(", ")
        ));
    }
    let round = 1 + match role {
        "checks" => p.checks_launches(),
        "build" => p.builds,
        "review" => p.reviews,
        _ => p.readings(),
    };
    s.push(if log_kind(id) == LogKind::Setup {
        format!("This is round {round} of {role} for {id}.")
    } else {
        format!("This is round {round} of {role} for this change.")
    });
    if role == "build" && round > 1 {
        let entries = log::events(&l.entries);
        if let Some(g) = entries.iter().rev().find(|(_, v)| step(v) == "gates")
            && log::any_kv(g.1, "result", "fail")
        {
            s.push("A fix round. What failed at the gates:".into());
            for e in g.1["gates"].as_array().cloned().unwrap_or_default() {
                if str_of(&e, "result") == "fail" {
                    s.push(format!(
                        "- {}: `{}` exit {}; last lines:\n{}",
                        str_of(&e, "check"),
                        str_of(&e, "command"),
                        e["exit"],
                        str_of(&e, "tail")
                    ));
                }
            }
        }
        if let Some(r) = entries.iter().rev().find(|(_, v)| step(v) == "review")
            && matches!(str_of(r.1, "verdict"), "fix" | "stop")
            && p.last_gates.as_ref().is_none_or(|g| g.line < r.0)
        {
            s.push(format!(
                "A fix round. The reviewer said {}:",
                str_of(r.1, "verdict")
            ));
            for f in r.1["findings"].as_array().cloned().unwrap_or_default() {
                s.push(format!(
                    "- {} {}: {}",
                    str_of(&f, "id"),
                    str_of(&f, "where"),
                    str_of(&f, "what")
                ));
            }
            if let Some(n) = r.1["notes"].as_str() {
                s.push(format!("NOTES FOR THE BUILDER:\n{n}"));
            }
        }
    }
    // What the log can show of a role's own runs comes from a json block, like the checks
    // role's proofs: without it, a claim or a trial is only prose.
    match role {
        "build" => s.push(
            "Before the gdd-report block, give a json block with what each check gave when you ran it: [{\"check\":\"<check id>\",\"command\":\"<command>\",\"exit\":0,\"tail\":\"<its last lines>\"}]."
                .into(),
        ),
        "review" => s.push(
            "Before the gdd-report block, give a json block with each command you ran: [{\"command\":\"<command>\",\"exit\":0,\"tail\":\"<its last lines>\"}]."
                .into(),
        ),
        _ => {}
    }
    s.push("End your last message with its gdd-report block, filled in.".into());
    s.join("\n")
}

#[derive(Debug, Clone)]
pub struct LaunchArgs {
    pub run: bool,
    pub command: Option<String>,
    pub level: Option<String>,
    pub start_wait: Duration,
    pub timeout: Option<Duration>,
    pub adapter: Option<String>,
}

pub fn launch_cmd(ctx: &Ctx, id: &str, role: &str, a: &LaunchArgs) -> Out {
    run("launch", |out| {
        if !["checks", "build", "review", "reading"].contains(&role) {
            return Err(Fail::usage("the role is checks, build, review or reading"));
        }
        let l = load(ctx, id)?;
        let p = &l.state.progress;
        // What the state allows: the same rules as the step this launch will write.
        if role != "reading" {
            let probe = new_event(
                id,
                "step",
                &time::now_string(),
                obj(vec![
                    ("step", json!(role)),
                    ("report", json!({"skill": "x", "result": "x"})),
                ]),
            );
            if let Err((code, why)) = p.allows(Some(&l.bp), &probe) {
                return Err(Fail::refuse(code, why, way_on(&l, id)));
            }
            if role == "review"
                && let Some(g) = p.last_gates.as_ref()
                && let (Some(gh), Some(h)) = (&g.head, &l.state.git.head)
                && gh != h
            {
                return Err(Fail::refuse(
                    "out-of-order",
                    format!(
                        "the gates passed on {}, and the head is now {}",
                        short(gh),
                        short(h)
                    ),
                    format!("gdd gates {id}"),
                ));
            }
        } else if p.brief_line.is_some() && p.last_build.is_some() {
            return Err(Fail::refuse(
                "out-of-order",
                "a brief is read before anyone builds it",
                way_on(&l, id),
            ));
        }
        if (role == "checks" || role == "build") && !l.state.limits.stop.is_empty() {
            return Err(Fail::refuse(
                "cap-spent",
                format!(
                    "no new round: {} no room left",
                    l.state.limits.stop.join(", ")
                ),
                cap_way_on(id, &l.state.limits),
            ));
        }
        if role == "build" && l.state.next.step != "build" {
            return Err(Fail::refuse(
                "out-of-order",
                format!(
                    "the next step is {}, not a build: {}",
                    l.state.next.step, l.state.next.say
                ),
                way_on(&l, id),
            ));
        }
        if role == "checks" || role == "build" {
            git_writes(ctx)?;
            git_commits(ctx)?;
        }
        let level = level_of(&l, role, a.level.as_deref());
        let (env, model, launch_text) = model_for(&l, &ctx.env, role, &level)?;
        let prompt = prompt_for(&l, id, role, &env, &level);
        let template = a
            .command
            .clone()
            .or_else(|| launch_text.clone().filter(|t| launch::is_command(t)));
        out.say(format!("{role} for {id}: {env}, {level}, model {model}"));
        out.fact("role", role);
        out.fact("environment", json!(env));
        out.fact("level", json!(level));
        out.fact("model", json!(model));
        out.fact("prompt", json!(prompt));
        let Some(template) = template else {
            out.say(format!(
                "launch it the way the blueprint says: {}",
                launch_text.clone().unwrap_or_else(|| "(no launch)".into())
            ));
            out.say(format!("with the model {model}, and this prompt:"));
            for line in prompt.lines() {
                out.say(format!("  {line}"));
            }
            out.say(format!(
                "then: gdd report {id} {role} <its last message, a file or ->"
            ));
            if a.run {
                return Err(Fail::refuse(
                    "not-a-command",
                    "the blueprint's launch is not one command with <model>: the layer can't run it",
                    "launch it yourself as printed, or pass --command \"<program> ... <model> ... <prompt>\"",
                ));
            }
            return Ok(());
        };
        let runs = runs_dir(&ctx.root, id, role, out)?;
        let prompt_file = runs.join("prompt.txt");
        crate::runs::write_new(&prompt_file, prompt.as_bytes())
            .map_err(|e| crate::out::io(&prompt_file, e))?;
        let argv = launch::argv(&template, &model, &prompt, &prompt_file.to_string_lossy())
            .map_err(Fail::usage)?;
        let shown: Vec<String> = argv
            .iter()
            .map(|w| {
                if w == &prompt {
                    "<prompt>".to_string()
                } else {
                    gates::quote(w)
                }
            })
            .collect();
        out.say(format!("command: {}", shown.join(" ")));
        out.say(format!("prompt: {}", prompt_file.display()));
        out.fact("argv", json!(shown));
        if !a.run {
            out.say(format!(
                "run it with --run, or launch it yourself and then: gdd report {id} {role} <file>"
            ));
            return Ok(());
        }
        let adapter = match &a.adapter {
            Some(n) => {
                Adapter::by_name(n).ok_or(Fail::usage("--adapter is claude, codex or plain"))?
            }
            None => Adapter::of_program(&argv[0]),
        };
        let left = l
            .state
            .limits
            .line("minutes_per_change", "minutes")
            .and_then(|m| m.cap.map(|c| (c - m.used as i64) * 60))
            .filter(|s| *s > 0);
        let timeout = a
            .timeout
            .unwrap_or(Duration::from_secs(left.unwrap_or(15 * 60) as u64));
        let output = runs.join("output.txt");
        out.say(format!("running, standard input closed, output to {}; it must show output within {} s; timeout {} s", output.display(), a.start_wait.as_secs(), timeout.as_secs()));
        let seen = launch::run(&argv, &ctx.root, &output, a.start_wait, timeout).map_err(|e| {
            Fail::refuse(
                "not-started",
                e,
                "the environment failed, not the change: retry once, or tell the person",
            )
        })?;
        let raw = std::fs::read_to_string(&output).unwrap_or_default();
        let said = launch::read_output(adapter, &raw, &model);
        let journal = json!({"launch": {
            "ran": true, "role": role, "environment": env, "level": level, "model": model, "adapter": adapter.name(),
            "started": seen.started, "first_output": seen.first_output, "seconds": seen.seconds, "timed_out": seen.timed_out,
            "exit": seen.exit, "tokens": said.tokens(), "usage": if said.usage.is_empty() { Value::Null } else { said.usage_json() },
        }});
        out.journal = Some(journal);
        out.fact("started", seen.started);
        out.fact("seconds", seen.seconds);
        out.fact("timed_out", seen.timed_out);
        out.fact("tokens", json!(said.tokens()));
        if !seen.started {
            return Err(Fail::refuse(
                "not-started",
                format!(
                    "the {role} wrote nothing within {} s{}: it didn't start, and it was stopped",
                    a.start_wait.as_secs(),
                    seen.exit
                        .map(|e| format!(" (exit {e})"))
                        .unwrap_or_default()
                ),
                "the environment failed, not the change: retry once, or tell the person",
            ));
        }
        if seen.timed_out {
            return Err(Fail::refuse(
                "timeout",
                format!(
                    "the {role} ran past {} s and was stopped",
                    timeout.as_secs()
                ),
                "the environment failed, not the change: retry once, or tell the person",
            ));
        }
        out.say(format!(
            "it ran {} s, first output after {} s, exit {}",
            seen.seconds,
            seen.first_output.unwrap_or(0.0),
            seen.exit
                .map(|e| e.to_string())
                .unwrap_or_else(|| "none".into())
        ));
        let ra = ReportArgs {
            model: Some(model.clone()),
            level: Some(level.clone()),
            seconds: Some(seen.seconds),
            tokens: said.tokens(),
            ..ReportArgs::default()
        };
        let step_name = role;
        let (line, _) = write_report(ctx, &l, id, step_name, &said.text, &ra, out)?;
        out.fact("line", line);
        Ok(())
    })
}

// ----- ship -----

pub fn ship_describe(ctx: &Ctx, id: &str) -> Out {
    run("ship", |out| {
        let l = load(ctx, id)?;
        if l.state.next.step != "ship" {
            return Err(Fail::refuse(
                "out-of-order",
                format!("{id} isn't ready to ship: {}", l.state.next.say),
                way_on(&l, id),
            ));
        }
        let events = log::events(&l.entries);
        let brief = l.project.brief(id);
        let text = crate::ship::describe(&l.state, &events, brief.as_ref());
        for line in text.lines() {
            out.say(line.to_string());
        }
        out.fact("description", json!(text));
        Ok(())
    })
}

/// The head a change ships: the one the state was derived on (named with `--head`, else the tip
/// of its branch), else, with no branch, the commit that was judged.
fn ship_head(l: &Loaded, id: &str) -> R<String> {
    l.state
        .git
        .head
        .clone()
        .or_else(|| judged_commit(l).map(|(_, j)| j))
        .ok_or(Fail::missing(
            format!("the head of {id}"),
            format!("no head for {id}: no branch and no gates step names one; pass --head"),
        ))
}

/// The `<start>` and the commit the change was judged on: the head its last passing gates judged,
/// from the start they judged it from; for a setup log, the head its trial judged.
fn judged_commit(l: &Loaded) -> Option<(String, String)> {
    let p = &l.state.progress;
    if p.kind == Some(LogKind::Setup) {
        let r = p.last_review.as_ref().filter(|r| r.verdict == "pass")?;
        Some((r.start.clone().unwrap_or_default(), r.head.clone()?))
    } else {
        let g = p.last_gates.as_ref().filter(|g| g.passed)?;
        Some((g.start.clone(), g.head.clone()?))
    }
}

/// What the state must allow before anything ships: the ship step itself, and a next step that
/// is shipping, derived on the head that ships. `--head` only names that head: it never stands in
/// for the state being ready, so a head nobody judged never goes in.
fn ready_to_ship(l: &Loaded, id: &str, place: &str) -> R<()> {
    let probe = new_event(
        id,
        "step",
        &time::now_string(),
        obj(vec![
            ("step", json!("ship")),
            ("where", json!(place)),
            ("color", json!(l.state.progress.color)),
        ]),
    );
    if let Err((code, why)) = l.state.progress.allows(Some(&l.bp), &probe) {
        return Err(Fail::refuse(code, why, way_on(l, id)));
    }
    if l.state.next.step != "ship" {
        return Err(Fail::refuse(
            "out-of-order",
            format!("{id} isn't ready to ship: {}", l.state.next.say),
            way_on(l, id),
        ));
    }
    Ok(())
}

/// The commit the change was judged on, which `head` must be, or stand for by the layer's own
/// rule (it differs only in the layer's bookkeeping); and on a change, the review judged it too.
fn judged_for(ctx: &Ctx, l: &Loaded, id: &str, head: &str) -> R<String> {
    let gv = &l.state.git;
    let unjudged = |why: String| Fail::refuse("unjudged", why, format!("gdd next {id}"));
    let Some((_, judged)) = judged_commit(l) else {
        return Err(unjudged(
            "no step the layer wrote names the commit that was judged".into(),
        ));
    };
    let stands = |a: &str, b: &str| {
        a == b || gv.stands_for(a, b) || state::only_bookkeeping(&ctx.root, id, a, b)
    };
    if !stands(&judged, head) {
        return Err(unjudged(format!(
            "{} is not {}, the commit that was judged, and differs from it in more than the layer's bookkeeping",
            short(head),
            short(&judged)
        )));
    }
    if l.state.progress.kind != Some(LogKind::Setup) {
        let reviewed = l
            .state
            .progress
            .last_review
            .as_ref()
            .and_then(|r| r.head.clone());
        if !reviewed.is_some_and(|r| stands(&r, &judged) || stands(&r, head)) {
            return Err(unjudged(format!(
                "the review didn't judge {}, the commit its gates judged",
                short(&judged)
            )));
        }
    }
    Ok(judged)
}

/// The orchestrator's bookkeeping in the working copy that travels with a change: the backlog,
/// and of the logs and journals directly in `gdd/log/` named for a valid id
/// ([`state::is_bookkeeping`]), the change's own, the blueprint's, and those of every change that
/// is over, shipped or dropped. A change still under way ships its log with itself.
fn bookkeeping_files(l: &Loaded, root: &Path, id: &str) -> Vec<String> {
    let travels = |x: &str| {
        if x == id || x == "blueprint" {
            return true;
        }
        let mut p = state::Progress::new(x);
        for (line, v) in log::events(&l.project.read_log(x)) {
            p.apply(line, v);
        }
        p.shipped.is_some() || p.brief_stopped.is_some()
    };
    let mut out = vec![];
    if let Ok(d) = std::fs::read_dir(root.join("gdd/log")) {
        for e in d.flatten() {
            if e.file_type().is_ok_and(|t| t.is_file())
                && let Ok(n) = e.file_name().into_string()
                && state::is_bookkeeping(&format!("gdd/log/{n}"))
                && n.strip_suffix(".engine.jsonl")
                    .or_else(|| n.strip_suffix(".jsonl"))
                    .is_some_and(travels)
            {
                out.push(format!("gdd/log/{n}"));
            }
        }
    }
    out.sort();
    if root.join("gdd/backlog.md").is_file() {
        out.push("gdd/backlog.md".into());
    }
    out
}

/// The change's branch and its tip, which must still be `head`, the commit the state was derived
/// on: a branch that moved since carries what nobody judged.
fn branch_at(ctx: &Ctx, l: &Loaded, id: &str, head: &str) -> R<(String, String)> {
    let branch = l.state.git.branch.clone().ok_or(Fail::missing(
        format!("the branch of {id}"),
        format!("no branch for {id} (work/{id}, gdd-build/{id}, build/{id}, or gdd-seal/{id} for a setup log): pass --branch"),
    ))?;
    let reference = format!("refs/heads/{branch}");
    let tip = git::commit_of(&ctx.root, &reference).ok_or(Fail::missing(
        reference.clone(),
        format!("{branch} is not a local branch"),
    ))?;
    if tip != head {
        return Err(branch_moved(&branch, &tip, head, id));
    }
    Ok((branch, tip))
}

fn branch_moved(branch: &str, tip: &str, head: &str, id: &str) -> Fail {
    Fail::refuse(
        "branch-moved",
        format!(
            "{branch} is at {}, and the commit that stands judged is {}: the branch moved, and what it holds now wasn't judged",
            short(tip),
            short(head)
        ),
        format!("gdd next {id}: it names what judges the branch as it is now"),
    )
}

/// Commit the bookkeeping on top of the change's branch, which must be at `head`, when its tip
/// doesn't hold it as it is. Returns the new tip, and whether it moved. The commit it makes goes
/// to the call's journal with the blob of each file it changed: that is how the layer knows later
/// that a head differs from the judged one only by its own bookkeeping.
fn commit_bookkeeping(
    ctx: &Ctx,
    l: &Loaded,
    id: &str,
    head: &str,
    out: &mut Out,
) -> R<(String, bool)> {
    let (branch, tip) = branch_at(ctx, l, id, head)?;
    let reference = format!("refs/heads/{branch}");
    let files = bookkeeping_files(l, &ctx.root, id);
    if files.is_empty() {
        return Ok((tip, false));
    }
    let worktrees = git::out(&ctx.root, &["worktree", "list", "--porcelain"]).unwrap_or_default();
    if worktrees
        .lines()
        .any(|w| w.trim() == format!("branch {reference}"))
    {
        out.say(format!(
            "note: {branch} is checked out in a worktree, which now trails it by the bookkeeping: remove it once the change is in"
        ));
    }
    let msg = format!("Bookkeeping of {id}: its log, the journals and the backlog");
    let c = seal::commit_on(&ctx.root, &reference, &tip, &files, &msg)
        .map_err(|e| Fail::refuse("bookkeeping", e, format!("gdd next {id}")))?;
    if !c.unchanged {
        let changed = git::diff_names(&ctx.root, &tip, &c.commit).unwrap_or_default();
        let blobs: Map<String, Value> = c
            .files
            .iter()
            .filter(|(f, _)| changed.contains(f))
            .map(|(f, h)| (f.clone(), json!(h)))
            .collect();
        let mut journal = match out.journal.take() {
            Some(Value::Object(m)) => m,
            _ => Map::new(),
        };
        journal.insert(
            "bookkeeping".into(),
            json!({"commit": c.commit, "parent": tip, "files": blobs}),
        );
        out.journal = Some(Value::Object(journal));
    }
    Ok((c.commit, !c.unchanged))
}

pub fn ship_bookkeeping(ctx: &Ctx, id: &str) -> Out {
    run("ship", |out| {
        let l = load(ctx, id)?;
        if !git::is_repo(&ctx.root) {
            return Err(Fail::missing(
                "a git repository",
                "the bookkeeping is committed with git: this is not a git repository",
            ));
        }
        git_writes(ctx)?;
        git_commits(ctx)?;
        ready_to_ship(&l, id, "bookkeeping")?;
        let head = ship_head(&l, id)?;
        judged_for(ctx, &l, id, &head)?;
        let (commit, moved) = commit_bookkeeping(ctx, &l, id, &head, out)?;
        if moved {
            out.say(format!(
                "committed the logs, the journals and the backlog on {} as {commit}; the checkout and its index were not touched",
                l.state.git.branch.clone().unwrap_or_default()
            ));
        } else {
            out.say(format!(
                "{} already holds the bookkeeping as it is, at {}",
                l.state.git.branch.clone().unwrap_or_default(),
                short(&commit)
            ));
        }
        out.say(format!(
            "what the layer logs from now on travels with the next change; next: open the pull request, and once it is merged: gdd ship {id} --done <where>"
        ));
        out.fact("commit", json!(commit));
        out.fact("moved", moved);
        Ok(())
    })
}

/// One entry of `git status --porcelain=v1 -z`: its two status letters and its path.
fn status_entries(root: &Path) -> R<Vec<(char, char, String)>> {
    let o = git::run(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )
    .map_err(|e| Fail::missing("the status of the checkout", e))?;
    let parts: Vec<String> = o
        .stdout
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect();
    let mut out = vec![];
    let mut i = 0;
    while i < parts.len() {
        let e = &parts[i];
        if e.len() < 4 {
            i += 1;
            continue;
        }
        let mut c = e.chars();
        let (x, y) = (c.next().unwrap_or(' '), c.next().unwrap_or(' '));
        out.push((x, y, e[3..].to_string()));
        i += if x == 'R' || x == 'C' { 2 } else { 1 };
    }
    Ok(out)
}

pub fn ship_merge(ctx: &Ctx, id: &str) -> Out {
    run("ship", |out| {
        let l = load(ctx, id)?;
        if !git::is_repo(&ctx.root) {
            return Err(Fail::missing(
                "a git repository",
                "a merge needs git: this is not a git repository",
            ));
        }
        git_writes(ctx)?;
        git_commits(ctx)?;
        let main = git::main_branch(&ctx.root);
        ready_to_ship(&l, id, "merge")?;
        let head = ship_head(&l, id)?;
        let judged = judged_for(ctx, &l, id, &head)?;
        let on = git::out(&ctx.root, &["symbolic-ref", "--quiet", "--short", "HEAD"])
            .unwrap_or_default();
        if on != main {
            return Err(Fail::refuse(
                "not-on-main",
                format!(
                    "the checkout is on {}, not on {main}",
                    if on.is_empty() {
                        "a detached head".into()
                    } else {
                        on
                    }
                ),
                format!("check out {main} in the orchestrator's copy, then: gdd ship {id} --merge"),
            ));
        }
        let branch = l.state.git.branch.clone().ok_or(Fail::missing(
            format!("the branch of {id}"),
            format!("no branch for {id}: pass --branch"),
        ))?;
        // The bookkeeping first, so the branch holds the copy's logs and backlog as they are. The
        // branch must still be at the head that stands judged, and what merges is that commit,
        // with the bookkeeping on top, by its hash.
        let (tip, moved) = commit_bookkeeping(ctx, &l, id, &head, out)?;
        if moved {
            out.say(format!(
                "committed the bookkeeping on {branch} as {}",
                short(&tip)
            ));
            if !state::only_bookkeeping(&ctx.root, id, &judged, &tip) {
                return Err(Fail::refuse(
                    "unjudged",
                    format!(
                        "{} differs from {}, the commit that was judged, in more than the layer's bookkeeping",
                        short(&tip),
                        short(&judged)
                    ),
                    format!("gdd next {id}"),
                ));
            }
        }
        // Nothing else may be pending: what the copy changed is either bookkeeping, or a file the
        // branch holds as it is (such as the brief and its scenarios), or nothing the merge touches.
        let mut aside: Vec<String> = vec![];
        let mut pending: Vec<String> = vec![];
        for (x, y, path) in status_entries(&ctx.root)? {
            let untracked = x == '?' && y == '?';
            if !untracked && x != ' ' {
                pending.push(format!("{path} (staged)"));
                continue;
            }
            if state::is_bookkeeping(&path) {
                if ctx.root.join(&path).is_file() {
                    aside.push(path);
                }
                continue;
            }
            if untracked {
                match git::blob_at(&ctx.root, &tip, &path) {
                    Some(b)
                        if git::hash_object(&ctx.root, &path).ok().as_deref()
                            == Some(b.as_str()) =>
                    {
                        aside.push(path)
                    }
                    Some(_) => {
                        pending.push(format!("{path} (your copy differs from the change's)"))
                    }
                    None => {}
                }
                continue;
            }
            pending.push(format!("{path} (changed)"));
        }
        if !pending.is_empty() {
            return Err(Fail::refuse(
                "dirty-tree",
                format!(
                    "something else is pending in the checkout: {}",
                    pending.join(", ")
                ),
                "commit or set it aside yourself; a correction of the brief is a new version",
            ));
        }
        // Move aside what the merge will bring, as it is.
        let keep = runs_dir(&ctx.root, id, "merge", out)?.join("aside");
        for p in &aside {
            let from = ctx.root.join(p);
            let to = keep.join(p);
            if let Some(d) = to.parent() {
                crate::runs::create_dirs(d).map_err(|e| crate::out::io(d, e))?;
            }
            crate::runs::copy_new(&from, &to).map_err(|e| crate::out::io(&to, e))?;
            let tracked = git::out(&ctx.root, &["ls-files", "--error-unmatch", "--", p]).is_ok();
            if tracked {
                git::out(&ctx.root, &["checkout", "--", p])
                    .map_err(|e| Fail::missing(p.clone(), e))?;
            } else {
                std::fs::remove_file(&from).map_err(|e| crate::out::io(&from, e))?;
            }
        }
        let put_back = |all: bool| -> Vec<String> {
            let mut back = vec![];
            let head = git::commit_of(&ctx.root, "HEAD");
            for p in &aside {
                let saved = keep.join(p);
                let brought = head
                    .as_ref()
                    .and_then(|h| git::blob_bytes(&ctx.root, h, p))
                    .zip(std::fs::read(&saved).ok())
                    .is_some_and(|(a, b)| a == b);
                if all || !brought {
                    let to = ctx.root.join(p);
                    if let Some(d) = to.parent() {
                        let _ = std::fs::create_dir_all(d);
                    }
                    let _ = std::fs::copy(&saved, &to);
                    back.push(p.clone());
                }
            }
            back
        };
        // What merges is the commit that stands judged, by its hash, never the branch by its name:
        // if the branch moved meanwhile, nothing merges.
        let now = git::commit_of(&ctx.root, &format!("refs/heads/{branch}"));
        if now.as_deref() != Some(tip.as_str()) {
            put_back(true);
            return Err(branch_moved(
                &branch,
                now.as_deref().unwrap_or("nothing"),
                &tip,
                id,
            ));
        }
        let msg = format!("Merge {branch}: {id}");
        let merged = git::run_env(
            &ctx.root,
            &["merge", "--no-ff", "--no-edit", "-m", &msg, &tip],
            &[("GIT_MERGE_AUTOEDIT", "no")],
        )
        .map_err(|e| Fail::missing("git merge", e))?;
        if !merged.status.success() {
            let conflicts = git::out(&ctx.root, &["diff", "--name-only", "--diff-filter=U"])
                .unwrap_or_default();
            let conflicts: Vec<&str> = conflicts.lines().filter(|l| !l.is_empty()).collect();
            if !conflicts.is_empty() && conflicts.iter().all(|p| state::is_bookkeeping(p)) {
                // The copy's bookkeeping is what reaches the main branch: the branch holds it.
                for p in &conflicts {
                    git::out(&ctx.root, &["checkout", "--theirs", "--", p])
                        .map_err(|e| Fail::missing(p.to_string(), e))?;
                    git::out(&ctx.root, &["add", "--", p])
                        .map_err(|e| Fail::missing(p.to_string(), e))?;
                }
                git::out_env(
                    &ctx.root,
                    &["commit", "--no-edit", "-m", &msg],
                    &[("GIT_MERGE_AUTOEDIT", "no")],
                )
                .map_err(|e| Fail::missing("the merge commit", e))?;
                out.say(format!(
                    "the bookkeeping conflicted in {}: the copy's version went in",
                    conflicts.join(", ")
                ));
            } else {
                let _ = git::out(&ctx.root, &["merge", "--abort"]);
                put_back(true);
                return Err(Fail::refuse(
                    "conflict",
                    format!(
                        "merging {branch} into {main} conflicts{}",
                        if conflicts.is_empty() {
                            format!(": {}", String::from_utf8_lossy(&merged.stderr).trim())
                        } else {
                            format!(" in {}", conflicts.join(", "))
                        }
                    ),
                    format!(
                        "bring the change up to date first: gdd seal {id} --merge-main, merge it into {branch}, gdd gates {id}"
                    ),
                ));
            }
        }
        let back = put_back(false);
        if !back.is_empty() {
            out.say(format!(
                "put back what the merge didn't bring: {}",
                back.join(", ")
            ));
        }
        let main_tip = git::commit_of(&ctx.root, &main).ok_or(Fail::missing(
            format!("refs/heads/{main}"),
            format!("there is no {main} branch"),
        ))?;
        if !git::is_ancestor(&ctx.root, &tip, &main_tip) {
            return Err(Fail::refuse(
                "not-merged",
                format!("{} is not in {main} after the merge", short(&tip)),
                format!(
                    "look at git log {main} and tell the person: the merge didn't take {}",
                    short(&tip)
                ),
            ));
        }
        out.say(format!(
            "merged {} ({branch}) into {main} as {}",
            short(&tip),
            short(&main_tip)
        ));
        let line = write_ship(&l, id, &main_tip, &tip, None, out)?;
        out.fact("line", line);
        out.fact("head", json!(tip));
        out.fact("merge", json!(main_tip));
        // Once this call is journaled, its ship line, its journal and the backlog go on the main
        // branch in a commit of their own, so the checkout holds nothing of the layer's.
        out.commit_log = Some(format!("Log the ship of {id}"));
        Ok(())
    })
}

/// Write the `ship` step and set the change's backlog item to done. `matched_in` is the commit of
/// the main branch that holds the judged content when the change's commits aren't in it.
fn write_ship(
    l: &Loaded,
    id: &str,
    place: &str,
    head: &str,
    matched_in: Option<&str>,
    out: &mut Out,
) -> R<usize> {
    let main = git::main_branch(&l.project.root);
    let color = l.state.progress.color.clone().unwrap_or_default();
    let mut m = obj(vec![
        ("step", json!("ship")),
        ("where", json!(place)),
        ("color", json!(color)),
        ("head", json!(head)),
    ]);
    if let Some(at) = matched_in {
        m.insert("matched".into(), json!("content"));
        m.insert("in".into(), json!(at));
    }
    let ev = new_event(id, "step", &time::now_string(), m);
    let line = write(l, id, &ev)?;
    out.say(format!(
        "{} is in {main}: wrote the ship step, line {line}; the log of {id} is closed",
        short(head)
    ));
    let path = l.project.path("gdd/backlog.md");
    if let Ok(text) = std::fs::read_to_string(&path) {
        let items = backlog::items(&text);
        let named = l
            .project
            .brief(id)
            .and_then(|(f, _)| f["backlog"].as_str().map(str::to_string));
        let item = items
            .iter()
            .find(|i| Some(&i.id) == named.as_ref())
            .or_else(|| items.iter().find(|i| i.brief == id))
            .or_else(|| items.iter().find(|i| i.id == id));
        match item {
            Some(i) if i.state != "done" => {
                let t = backlog::set(&text, &i.id, "state", "done")
                    .map_err(|e| Fail::missing("gdd/backlog.md", e))?;
                std::fs::write(&path, t).map_err(|e| crate::out::io(&path, e))?;
                out.say(format!(
                    "set {} to done in the backlog: say so only once what it builds is in use",
                    i.id
                ));
                out.fact("backlog", json!(i.id));
            }
            Some(i) => out.say(format!("{} was already done in the backlog", i.id)),
            None => out.say(format!("no backlog item names {id}")),
        }
    }
    // A zone drawn before its code existed can name a path nobody wrote, and what was built
    // for it lands in another zone, or in none. What ships is when it shows.
    if let Ok(z) = color::zones(&l.project.root, &l.bp)
        && !z.empty.is_empty()
    {
        let mut names: Vec<String> = vec![];
        for (zone, _) in &z.empty {
            if !names.contains(zone) {
                names.push(zone.clone());
            }
        }
        out.say(format!(
            "note: pathspecs of these zones match no tracked file: {}. What was built for them may be judged in another zone, or in none: gdd zones shows it, and correcting a path is a change to the blueprint, for the person",
            names.join(", ")
        ));
        out.fact("empty_zones", json!(names));
    }
    Ok(line)
}

pub fn ship_done(ctx: &Ctx, id: &str, place: &str) -> Out {
    run("ship", |out| {
        let l = load(ctx, id)?;
        if !git::is_repo(&ctx.root) {
            return Err(Fail::missing(
                "a git repository",
                "shipping is checked against git: this is not a git repository",
            ));
        }
        let main = git::main_branch(&ctx.root);
        ready_to_ship(&l, id, place)?;
        let head = ship_head(&l, id)?;
        let judged = judged_for(ctx, &l, id, &head)?;
        let main_tip = git::commit_of(&ctx.root, &main).ok_or(Fail::missing(
            format!("refs/heads/{main}"),
            format!("there is no {main} branch"),
        ))?;
        let gv = &l.state.git;
        let stands = |c: &str| {
            c == judged
                || gv.stands_for(&judged, c)
                || state::only_bookkeeping(&ctx.root, id, &judged, c)
        };
        // The change's branch is what a pull request merges. While it exists, whatever its tip
        // holds is what a merge of it brought, --head or not: past what was judged, it can't ship.
        let branch_tip = gv.branch.as_ref().and_then(|b| {
            git::commit_of(&ctx.root, &format!("refs/heads/{b}")).map(|t| (b.clone(), t))
        });
        if let Some((branch, tip)) = &branch_tip
            && !stands(tip)
        {
            return Err(Fail::refuse(
                "unjudged",
                format!(
                    "{branch} is at {}, which differs from {}, the commit that was judged, in more than the layer's bookkeeping: what a merge of it brought wasn't judged",
                    short(tip),
                    short(&judged)
                ),
                format!("gdd next {id}: it names what judges the branch as it is now"),
            ));
        }
        let bound = branch_tip
            .map(|(_, t)| t)
            .or_else(|| ctx.head.as_ref().and(gv.head.clone()));
        // What went in, from git: the merge that brought it, a fast-forward bounded by what comes
        // after it, or, by squash or rebase, exactly the content that was judged and nothing else.
        let (start, _) = judged_commit(&l).unwrap_or_default();
        let went = crate::ship::went_in(
            &ctx.root,
            id,
            &main,
            &main_tip,
            &start,
            &judged,
            bound.as_deref(),
            &stands,
        )
        .map_err(|n| {
            let instead = match n.code {
                "which-tip" => format!(
                    "name the tip that was merged: gdd ship {id} --done {place} --head <that commit>"
                ),
                "unjudged" => format!("tell the person: what went into {main} wasn't judged"),
                _ => format!(
                    "merge it as the constitution says (gdd ship {id} --merge, or a pull request after gdd ship {id} --bookkeeping), then: gdd ship {id} --done <where>"
                ),
            };
            Fail::refuse(n.code, n.why, instead)
        })?;
        let (shipped, matched_in) = match went {
            crate::ship::WentIn::Merged { tip, merge } => {
                out.say(format!(
                    "{main} took {} by the merge {}",
                    short(&tip),
                    short(&merge)
                ));
                (tip, None)
            }
            crate::ship::WentIn::FastForward { tip } => {
                out.say(format!("{main} took {} by a fast-forward", short(&tip)));
                (tip, None)
            }
            crate::ship::WentIn::Content { files, at } => {
                out.say(format!(
                    "matched by content: {main} took at {} exactly what was judged at {} in each of the {files} files the change touched, and nothing else, though not its commits (a merge by squash or rebase)",
                    short(&at),
                    short(&judged)
                ));
                out.fact("matched", "content");
                (judged.clone(), Some(at))
            }
        };
        let line = write_ship(&l, id, place, &shipped, matched_in.as_deref(), out)?;
        out.say(
            "the ship line stays in this copy and travels with the next change's bookkeeping: it is not something to report to the person",
        );
        out.fact("line", line);
        out.fact("head", json!(shipped));
        Ok(())
    })
}

/// What a command leaves for after its call is journaled: for `ship --merge`, committing the
/// bookkeeping the main branch doesn't hold yet (`gdd/log/**`, `gdd/backlog.md`), in one commit
/// that touches nothing else. A program that uses the library calls it after the journal.
pub fn finish(ctx: &Ctx, out: &mut Out) {
    let Some(msg) = out.commit_log.take() else {
        return;
    };
    let main = git::main_branch(&ctx.root);
    let not_done = |why: &str| {
        format!(
            "the ship's line and the journal stay uncommitted on {main} ({why}): that is bookkeeping, which travels with the next change, not something to report to the person"
        )
    };
    let on =
        git::out(&ctx.root, &["symbolic-ref", "--quiet", "--short", "HEAD"]).unwrap_or_default();
    if on != main {
        out.say(not_done(&format!("the checkout is on {on}")));
        return;
    }
    let paths: Vec<&str> = ["gdd/log", "gdd/backlog.md"]
        .into_iter()
        .filter(|p| ctx.root.join(p).exists())
        .collect();
    let mut add = vec!["add", "-A", "--"];
    add.extend(&paths);
    if let Err(e) = git::out(&ctx.root, &add) {
        out.say(not_done(e.lines().last().unwrap_or("git add failed")));
        return;
    }
    let mut staged = vec!["diff", "--cached", "--quiet", "--"];
    staged.extend(&paths);
    if git::run(&ctx.root, &staged).is_ok_and(|o| o.status.success()) {
        return;
    }
    let mut commit = vec!["commit", "-q", "-m", msg.as_str(), "--"];
    commit.extend(&paths);
    match git::out(&ctx.root, &commit) {
        Ok(_) => {
            let sha = git::commit_of(&ctx.root, "HEAD").unwrap_or_default();
            out.say(format!(
                "committed the ship's bookkeeping on {main} as {}: {msg}; nothing of the layer's is left in the checkout",
                short(&sha)
            ));
            out.fact("logged", json!(sha));
        }
        Err(e) => out.say(not_done(e.lines().last().unwrap_or("git commit failed"))),
    }
}

// ----- the orchestrator's own steps -----

#[derive(Debug, Clone, Default)]
pub struct StepArgs {
    pub result: Option<String>,
    pub caught: Option<String>,
    pub level: Option<String>,
    pub model: Option<String>,
    pub skill_version: Option<String>,
    pub seconds: Option<f64>,
    pub tokens: Option<i64>,
    pub files: Vec<String>,
    pub probe: Option<String>,
    pub levels: Option<String>,
    pub change: Option<String>,
    pub crack: Option<String>,
    pub scenario: Option<String>,
    pub proposal: Option<String>,
    pub asked: Option<i64>,
    pub missed: Vec<String>,
}

fn json_list(text: &Option<String>, what: &str, keys: &[&str]) -> R<Value> {
    let Some(t) = text else { return Ok(json!([])) };
    let v: Value =
        serde_json::from_str(t).map_err(|e| Fail::usage(format!("{what} isn't JSON: {e}")))?;
    let ok = v.as_array().is_some_and(|a| {
        a.iter()
            .all(|x| x.is_object() && keys.iter().all(|k| x.get(*k).is_some()))
    });
    if !ok {
        return Err(Fail::usage(format!(
            "{what} is a JSON list of objects, each with {}",
            keys.join(", ")
        )));
    }
    Ok(v)
}

/// The orchestrator's own steps, `prepare`, `harden` and `exam`, in the shapes of the contract,
/// so nobody composes a whole report by hand. They go to the blueprint's log.
pub fn step_cmd(ctx: &Ctx, id: &str, st: &str, a: &StepArgs) -> Out {
    run("step", |out| {
        if id != "blueprint" {
            return Err(Fail::usage(format!(
                "a {st} step goes to the blueprint's log: gdd step blueprint {st} ..."
            )));
        }
        let l = load(ctx, id)?;
        let round = 1 + log::events(&l.entries)
            .iter()
            .filter(|(_, v)| step(v) == st)
            .count() as i64;
        let level = a.level.clone().unwrap_or_else(|| "deep".into());
        let caught = a.caught.clone().unwrap_or_else(|| "none".into());
        let mut m = obj(vec![("step", json!(st))]);
        match st {
            "prepare" => {
                let result = a
                    .result
                    .clone()
                    .ok_or(Fail::usage("--result is proposed or approved"))?;
                if result != "proposed" && result != "approved" {
                    return Err(Fail::usage("--result is proposed or approved"));
                }
                for f in &a.files {
                    if !ctx.root.join(f).exists() {
                        return Err(Fail::missing(
                            f.to_string(),
                            format!("{f} is not in the working copy"),
                        ));
                    }
                }
                m.insert(
                    "report".into(),
                    report_obj(
                        "gdd-prepare",
                        a.skill_version
                            .as_deref()
                            .or(Some(env!("CARGO_PKG_VERSION"))),
                        Some(&level),
                        a.model.as_deref(),
                        round,
                        &result,
                        &caught,
                        a.seconds,
                        a.tokens,
                    ),
                );
                m.insert("files".into(), json!(a.files));
                m.insert(
                    "probe".into(),
                    json_list(&a.probe, "--probe", &["try", "command", "exit", "tail"])?,
                );
                m.insert(
                    "levels".into(),
                    json_list(
                        &a.levels,
                        "--levels",
                        &["environment", "level", "launch", "model"],
                    )?,
                );
            }
            "harden" => {
                let (Some(crack), Some(proposal)) = (&a.crack, &a.proposal) else {
                    return Err(Fail::usage("a harden step takes --crack and --proposal"));
                };
                let result = a.result.clone().unwrap_or_else(|| "proposed".into());
                m.insert(
                    "report".into(),
                    report_obj(
                        "gdd-harden",
                        a.skill_version
                            .as_deref()
                            .or(Some(env!("CARGO_PKG_VERSION"))),
                        Some(&level),
                        a.model.as_deref(),
                        round,
                        &result,
                        &caught,
                        a.seconds,
                        a.tokens,
                    ),
                );
                let change = a.change.clone().filter(|c| c != "none");
                if let Some(c) = &change {
                    check_id(c)?;
                }
                m.insert("change".into(), json!(change));
                m.insert("crack".into(), json!(crack));
                m.insert(
                    "scenario".into(),
                    json!(a.scenario.clone().filter(|s| s != "none")),
                );
                m.insert("proposal".into(), json!(proposal));
            }
            "exam" => {
                let (Some(asked), Some(model)) = (a.asked, &a.model) else {
                    return Err(Fail::usage(
                        "an exam step takes --asked, --caught, --level and --model (the reviewer's)",
                    ));
                };
                let caught: i64 = caught.parse().map_err(|_| {
                    Fail::usage("--caught is how many questions it caught, a number")
                })?;
                if caught > asked || caught < 0 {
                    return Err(Fail::usage("--caught is between 0 and --asked"));
                }
                m.insert("reviewer".into(), json!({"level": level, "model": model}));
                m.insert("asked".into(), json!(asked));
                m.insert("caught".into(), json!(caught));
                m.insert("missed".into(), json!(a.missed));
            }
            _ => return Err(Fail::usage("the step is prepare, harden or exam")),
        }
        let ev = new_event(id, "step", &time::now_string(), m);
        let line = write(&l, id, &ev)?;
        out.say(format!(
            "wrote the {st} step to gdd/log/blueprint.jsonl, line {line}, round {round}"
        ));
        out.fact("line", line);
        out.fact("round", round);
        Ok(())
    })
}

// ----- backlog -----

pub fn backlog_list(ctx: &Ctx) -> Out {
    run("backlog", |out| {
        let text = std::fs::read_to_string(ctx.root.join("gdd/backlog.md"))
            .map_err(|_| Fail::missing("gdd/backlog.md", "gdd/backlog.md is missing"))?;
        let b = backlog::read(&text);
        let items = &b.items;
        for i in items {
            out.say(format!(
                "{:<8} {:<20} {:<7} {}",
                i.state, i.id, i.color, i.what
            ));
        }
        if let Some(n) = backlog::next(items) {
            out.say(format!("next: {}", n.id));
        }
        out.fact("items", json!(items.iter().map(|i| json!({"id": i.id, "color": i.color, "phase": i.phase, "brief": i.brief, "state": i.state})).collect::<Vec<_>>()));
        // What can't be read is said, never skipped: an item missed is a change nobody builds.
        for p in &b.problems {
            out.say(format!("can't read {}", p.say()));
        }
        if !b.problems.is_empty() {
            out.fact(
                "problems",
                json!(
                    b.problems
                        .iter()
                        .map(|p| json!({"line": p.line, "what": p.what}))
                        .collect::<Vec<_>>()
                ),
            );
            out.say("fix those lines in gdd/backlog.md as its table asks (Id, What, Color, Phase, Brief and State), then gdd backlog list again");
            out.code = 1;
            out.refused = Some("unreadable".into());
        }
        Ok(())
    })
}

pub fn backlog_add(ctx: &Ctx, id: &str, what: &str, color: &str, phase: &str) -> Out {
    run("backlog", |out| {
        check_id(id)?;
        if crate::color_rank(color).is_none() {
            return Err(Fail::usage("--color is green, yellow or red"));
        }
        let path = ctx.root.join("gdd/backlog.md");
        let text = std::fs::read_to_string(&path)
            .map_err(|_| Fail::missing("gdd/backlog.md", "gdd/backlog.md is missing"))?;
        // The trial of a check that waits for what it watches comes after the first item that
        // brings it something to judge. The backlog's order is the order of the work: added before
        // any item of the product, the trial would be next, with nothing to try the check on.
        if id.starts_with("standing-checks-")
            && !backlog::items(&text)
                .iter()
                .any(|i| log_kind(&i.id) == LogKind::Change)
        {
            return Err(Fail::refuse(
                "out-of-order",
                format!(
                    "{id} is the trial of a check that waits for what it watches, and the backlog holds no item of the product yet: it would come next, with nothing to try the check on"
                ),
                "add first the item that brings it something to judge, then this one: gdd backlog add <that item> --what \"<what the person gets>\" --color <color>",
            ));
        }
        let item = backlog::Item {
            id: id.into(),
            what: what.into(),
            color: color.into(),
            phase: phase.into(),
            state: "todo".into(),
            ..backlog::Item::default()
        };
        let t = backlog::add(&text, &item)
            .map_err(|e| Fail::refuse("exists", e, "pick another id, or gdd backlog list"))?;
        std::fs::write(&path, t).map_err(|e| crate::out::io(&path, e))?;
        out.say(format!("added {id} to the backlog, {color}, todo"));
        Ok(())
    })
}

pub fn backlog_set(ctx: &Ctx, id: &str, state: &str, brief: Option<&str>) -> Out {
    run("backlog", |out| {
        let path = ctx.root.join("gdd/backlog.md");
        let text = std::fs::read_to_string(&path)
            .map_err(|_| Fail::missing("gdd/backlog.md", "gdd/backlog.md is missing"))?;
        let mut t = backlog::set(&text, id, "state", state)
            .map_err(|e| Fail::refuse("no-item", e, "gdd backlog list"))?;
        if let Some(b) = brief {
            t = backlog::set(&t, id, "brief", b)
                .map_err(|e| Fail::refuse("no-item", e, "gdd backlog list"))?;
        }
        std::fs::write(&path, t).map_err(|e| crate::out::io(&path, e))?;
        out.say(format!("{id} is {state} in the backlog"));
        Ok(())
    })
}

// ----- waiting -----

pub fn waiting(ctx: &Ctx) -> Out {
    run("waiting", |out| {
        let project = Project::open(&ctx.root);
        let now = time::now();
        let mut open = vec![];
        for id in project.log_ids() {
            let e = project.read_log(&id);
            let mut p = state::Progress::new(&id);
            for (line, v) in log::events(&e) {
                p.apply(line, v);
            }
            if let Some(a) = p.open_ask() {
                open.push((
                    a.ts.unwrap_or(now),
                    id.clone(),
                    a.line,
                    a.of.clone(),
                    a.what.clone(),
                ));
            }
        }
        open.sort();
        for (t, id, line, of, what) in &open {
            out.say(format!(
                "{} {id}:{line} {of}: {what}",
                time::waited(now - t)
            ));
        }
        if open.is_empty() {
            out.say("nothing waits for the person");
        }
        out.fact(
            "waiting",
            json!(open.iter().map(|(t, id, line, of, what)| json!({"id": id, "line": line, "of": of, "what": what, "since": time::format(*t), "seconds": now - t})).collect::<Vec<_>>()),
        );
        Ok(())
    })
}

// ----- metrics -----

pub struct MetricsArgs<'a> {
    pub ids: &'a [String],
    pub format: &'a str,
    pub out: Option<&'a Path>,
    pub summary: bool,
}

pub fn metrics_cmd(ctx: &Ctx, a: &MetricsArgs) -> Out {
    run("metrics", |out| {
        let project = Project::open(&ctx.root);
        // Without a blueprint, the tables still come out, with no caps and no tiers.
        let bp = if project.has_blueprint() {
            project.blueprint()?
        } else {
            out.say("there is no gdd/blueprint.md: the tables have no caps and no tiers");
            Blueprint::parse("---\n---\n").map_err(|e| Fail::missing("an empty blueprint", e))?
        };
        let ids: Vec<String> = if a.ids.is_empty() {
            project.log_ids()
        } else {
            a.ids.to_vec()
        };
        let use_git = git::is_repo(&ctx.root);
        let mut stash = vec![];
        for id in &ids {
            check_id(id)?;
            let entries = project.read_log(id);
            let journal =
                crate::journal::read(&project.path(&format!("gdd/log/{id}.engine.jsonl")));
            let mut p = state::Progress::new(id);
            for (line, v) in log::events(&entries) {
                p.apply(line, v);
            }
            let mut cg = metrics::ChangeGit::default();
            if use_git {
                let head = p
                    .last_gates
                    .as_ref()
                    .and_then(|g| g.head.clone())
                    .or_else(|| GitView::of(&ctx.root, id, None, None).head);
                if let (Some(s), Some(h)) = (&p.start, head)
                    && let Some((f, ad, de)) = git::shortstat(&ctx.root, s, &h)
                {
                    cg = metrics::ChangeGit {
                        files: Some(f),
                        added: Some(ad),
                        deleted: Some(de),
                    };
                }
            }
            let scenarios = project
                .brief(id)
                .and_then(|(f, _)| f["scenarios"].as_array().map(|a| a.len() as i64))
                .or_else(|| {
                    let n: usize = p
                        .brief_files
                        .keys()
                        .filter(|k| k.starts_with("features/"))
                        .filter_map(|k| std::fs::read_to_string(project.path(k)).ok())
                        .map(|t| t.matches(&format!("@brief:{id}")).count())
                        .sum();
                    (n > 0).then_some(n as i64)
                });
            stash.push((id.clone(), entries, journal, cg, scenarios));
        }
        let inputs: Vec<metrics::Input> = stash
            .iter()
            .map(|(id, e, j, g, s)| metrics::Input {
                id,
                entries: e,
                journal: j,
                git: g.clone(),
                scenarios: *s,
            })
            .collect();
        let mut t = metrics::tables(&bp, &inputs, time::now());
        // Journals of ids with no log of their own, such as blueprint.engine.jsonl, or a call
        // refused before its change wrote anything: every call of the layer is in `engine`.
        if a.ids.is_empty() {
            for extra in project.journal_ids() {
                let extra = extra.as_str();
                if !ids.iter().any(|i| i == extra) {
                    let j = crate::journal::read(
                        &project.path(&format!("gdd/log/{extra}.engine.jsonl")),
                    );
                    let more = metrics::tables(
                        &bp,
                        &[metrics::Input {
                            id: extra,
                            entries: &[],
                            journal: &j,
                            git: Default::default(),
                            scenarios: None,
                        }],
                        time::now(),
                    );
                    t.engine.extend(more.engine);
                }
            }
        }
        let counts: Map<String, Value> = metrics::TABLES
            .iter()
            .map(|n| (n.to_string(), json!(t.get(n).len())))
            .collect();
        match (a.format, a.out) {
            (f, Some(dir)) => {
                std::fs::create_dir_all(dir).map_err(|e| crate::out::io(dir, e))?;
                for name in metrics::TABLES {
                    let (file, text) = match f {
                        "csv" => (format!("{name}.csv"), metrics::csv(t.get(name))),
                        "jsonl" => (format!("{name}.jsonl"), metrics::jsonl(&t, name)),
                        _ => (format!("{name}.json"), serde_json::to_string_pretty(&json!({"schema": metrics::SCHEMA, "table": name, "rows": t.get(name)})).unwrap_or_default()),
                    };
                    std::fs::write(dir.join(&file), text)
                        .map_err(|e| crate::out::io(&dir.join(&file), e))?;
                }
                out.say(format!(
                    "wrote {} tables as {f} in {}",
                    metrics::TABLES.len(),
                    dir.display()
                ));
            }
            ("csv", None) => {
                if !a.summary {
                    for name in metrics::TABLES {
                        out.say(format!("# {name}"));
                        for l in metrics::csv(t.get(name)).lines() {
                            out.say(l.to_string());
                        }
                    }
                }
            }
            ("jsonl", None) => {
                if !a.summary {
                    for name in metrics::TABLES {
                        for l in metrics::jsonl(&t, name).lines() {
                            out.say(l.to_string());
                        }
                    }
                }
            }
            (_, None) => {
                if !a.summary {
                    out.say(
                        serde_json::to_string_pretty(&metrics::json_all(&t)).unwrap_or_default(),
                    );
                }
            }
        }
        if a.summary {
            for l in metrics::summary(&t) {
                out.say(l);
            }
        }
        out.fact("schema", json!(metrics::SCHEMA));
        out.fact("rows", Value::Object(counts));
        out.fact("git", use_git);
        Ok(())
    })
}
