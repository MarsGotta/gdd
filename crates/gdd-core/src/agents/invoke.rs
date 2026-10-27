//! Starting an agent of the recipe: the program its agent sheet names, found on the `PATH` of the
//! environment gdd runs in, with the arguments the sheet gives its command and its role, and never
//! through a shell. It runs from the project root, with that environment without its secrets (see
//! [`Env`]) plus `GDD_CHANGE`, `GDD_ROOT` and `GDD_ROLE`. What it prints is read with the reader
//! of its sheet (see [`usage`]): its final text and its output are kept next to the change, the
//! Reviewer's text is its review, and the call goes to the log. The tool prints none of it, and
//! writes none of it through a link the agent could have left (see [`files`]).
//!
//! Each call runs in a process group of its own and has a time limit. When the call ends, or its
//! time runs out, the whole group is killed, never a process read from a file: nothing the agent
//! started outlives the call. The file where the Reviewer or the replan answers goes away before
//! the call, so an old answer never counts.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::{self, ErrorKind, Read};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agents::env::Env;
use crate::agents::files;
use crate::agents::models::{self, Model, Roles};
use crate::agents::sheets::{self, Sheet};
use crate::agents::usage::{self, Price, Said};
use crate::gates::verdict;
use crate::{config, exec, log};

/// How often a running agent is checked.
const POLL: Duration = Duration::from_millis(50);

/// Where the sessions of the Claude Code calls are kept, one per line, from the project root.
const SESSIONS: &str = ".gdd/headless_sessions.txt";

/// Who the recipe calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Who {
    /// The Builder in test mode: acceptance steps and tests.
    BuilderTest,
    /// The Builder in build mode: the code.
    BuilderBuild,
    /// The Reviewer, of another model family than the Builder.
    Reviewer,
    /// The Orchestrator, replanning a change stuck in G2.
    Orchestrator,
}

impl Who {
    /// The GDD command it runs, `gdd-<command>` in every agent.
    fn command(self) -> &'static str {
        match self {
            Who::BuilderTest => "build-test",
            Who::BuilderBuild => "build",
            Who::Reviewer => "review",
            Who::Orchestrator => "replan",
        }
    }

    /// Its role, as the sheets, the log and `GDD_ROLE` name it.
    fn role(self) -> &'static str {
        match self {
            Who::BuilderTest => "builder-test",
            Who::BuilderBuild => "builder-build",
            Who::Reviewer => "reviewer",
            Who::Orchestrator => "orchestrator",
        }
    }

    /// The phase and the gate of its calls in the log.
    fn stage(self) -> (&'static str, &'static str) {
        match self {
            Who::Reviewer => ("3-integrate", "G3"),
            Who::BuilderTest | Who::BuilderBuild | Who::Orchestrator => ("2-build", "G2"),
        }
    }

    /// The file where it answers on change `id`, from the project root, if it answers in one.
    fn answer(self, id: &str) -> Option<String> {
        match self {
            Who::Reviewer => Some(verdict::review_path(id)),
            Who::Orchestrator => Some(verdict::replan_path(id)),
            Who::BuilderTest | Who::BuilderBuild => None,
        }
    }
}

impl fmt::Display for Who {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Who::BuilderTest => "Builder in test mode",
            Who::BuilderBuild => "Builder in build mode",
            Who::Reviewer => "Reviewer",
            Who::Orchestrator => "Orchestrator",
        })
    }
}

/// A call to an agent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Call<'a> {
    /// Who.
    pub who: Who,
    /// Its model, as `gdd/models.toml` writes it: the integration, the model name and the effort
    /// count.
    pub model: &'a Model,
    /// The arguments of its command after `change=<id>`: `mode=build fix=g3`...
    pub args: &'a str,
}

/// What `gdd/models.toml` says of the agents besides the model of each role: the variables they
/// get on purpose (`[env] pass`) and the price of each model (`[prices.<model>]`), for the agents
/// that report no cost.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Settings {
    env: Env,
    prices: BTreeMap<String, Price>,
}

impl Settings {
    /// Reads them from the text of a models file; the rest of the file is ignored.
    pub fn from_toml(text: &str) -> Result<Settings, config::Error> {
        toml::from_str(text).map_err(|e| config::Error::Toml(e.to_string()))
    }
}

/// What a call reads from the project.
#[derive(Debug, Clone, Copy)]
pub struct Project<'a> {
    /// The project root.
    pub root: &'a Path,
    /// The change.
    pub id: &'a str,
    /// The environment gdd runs in.
    pub env: &'a [(String, String)],
    /// What `gdd/models.toml` says of the agents.
    pub settings: &'a Settings,
    /// The time an agent has to answer.
    pub timeout: Duration,
}

/// Why a call to an agent did not end well.
#[derive(Debug)]
pub enum Error {
    /// The model of the role cannot be started: who, and why.
    Model(Who, String),
    /// The program of an agent is not on the `PATH`: the program, and the agent's name.
    Missing(String, String),
    /// A file could not be removed before the call: which, and why.
    Remove(String, io::Error),
    /// A file could not be read: which, and why.
    Read(String, io::Error),
    /// What the call left could not be kept: the file, and why.
    Write(String, io::Error),
    /// The agent could not be started, read or waited for: its program, and why.
    Start(String, io::Error),
    /// The agent did not answer within its time limit, in seconds: its process group was killed.
    Timeout(Who, u64),
    /// The agent ended with an error: its exit code, or minus the signal that killed it.
    Failed(Who, i32),
}

impl Error {
    /// Whether the change parks for it: the agent ran and did not do its job. The other errors
    /// are failures of the project.
    pub fn parks(&self) -> bool {
        matches!(self, Error::Timeout(..) | Error::Failed(..))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Model(who, why) => write!(f, "{}: the {who} {why}", models::PATH),
            Error::Missing(program, agent) => {
                write!(f, "cannot find the {program} program of {agent} on PATH")
            }
            Error::Remove(path, e) => write!(f, "cannot remove {path}: {e}"),
            Error::Read(path, e) => write!(f, "cannot read {path}: {e}"),
            Error::Write(path, e) => write!(f, "cannot write {path}: {e}"),
            Error::Start(program, e) => write!(f, "cannot start {program}: {e}"),
            Error::Timeout(who, seconds) => {
                write!(f, "the {who} did not answer within {seconds} seconds")
            }
            Error::Failed(who, code) => write!(f, "the {who} failed (exit {code})"),
        }
    }
}

impl std::error::Error for Error {}

/// Checks that the model of each role in `roles` can be started, in the order Orchestrator,
/// Builder, Builder in test mode, Reviewer: first that each has an agent sheet (see
/// [`startable`]), then that the program of each is on the `PATH` of `env`, the environment gdd
/// runs in. The recipe does it before anything runs.
pub fn check(roles: &Roles, env: &[(String, String)]) -> Result<(), Error> {
    let each = [
        (Who::Orchestrator, &roles.orchestrator),
        (Who::BuilderBuild, &roles.builder),
        (Who::BuilderTest, &roles.builder_test),
        (Who::Reviewer, &roles.reviewer_model),
    ];
    let mut sheets = Vec::new();
    for (who, model) in each {
        let (_, sheet, _) = startable(who, model)?;
        sheets.push(sheet);
    }
    for sheet in &sheets {
        program(sheet, env)?;
    }
    Ok(())
}

/// The integration of `model`, the model of `who`, its agent sheet and its model name. A model
/// without a name, or whose integration has no sheet, cannot be started.
fn startable(who: Who, model: &Model) -> Result<(&str, Sheet, &str), Error> {
    let text = |key| model.get(key).and_then(Value::as_str).unwrap_or_default();
    let (integration, name) = (text("integration"), text("model"));
    if name.is_empty() {
        return Err(Error::Model(who, "has no model".to_string()));
    }
    let sheet = sheets::sheet(integration).map_err(|e| {
        let why = match e {
            sheets::Error::Missing(_) => format!(", which has no agent sheet ({})", sheets::NAMES),
            invalid @ sheets::Error::Invalid(..) => format!(": {invalid}"),
        };
        Error::Model(who, format!("uses {integration:?}{why}"))
    })?;
    Ok((integration, sheet, name))
}

/// The program of the agent of `sheet`: the first file of its name that can be run, in the
/// folders of the `PATH` of `env`, the environment gdd runs in. Folders that are not absolute are
/// left out: what they find would depend on the folder gdd runs from.
pub(crate) fn program(sheet: &Sheet, env: &[(String, String)]) -> Result<PathBuf, Error> {
    let path = env.iter().rev().find(|(name, _)| name == "PATH");
    let folders = path.map_or("", |(_, folders)| folders.as_str());
    folders
        .split(':')
        .map(Path::new)
        .filter(|folder| folder.is_absolute())
        .map(|folder| folder.join(sheet.binary()))
        .find(|file| runnable(file))
        .ok_or_else(|| Error::Missing(sheet.binary().to_string(), sheet.title().to_string()))
}

/// Whether `file` is a file someone may run.
fn runnable(file: &Path) -> bool {
    let meta = fs::metadata(file);
    meta.is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// What starts a call: the integration, the agent sheet and the model name of its model, and its
/// arguments.
struct Start<'a> {
    integration: &'a str,
    sheet: Sheet,
    model: &'a str,
    args: Vec<String>,
}

/// What starts `call` on change `id`: the `launch.args` of the sheet of its integration (see
/// [`Sheet::launch`]) for its command and its role, with `change=<id> <args>` as the words of
/// the command and the `effort` of its model, if it has one.
fn start<'a>(call: &Call<'a>, id: &str) -> Result<Start<'a>, Error> {
    let (integration, sheet, model) = startable(call.who, call.model)?;
    let words = format!("change={id} {}", call.args);
    let effort = call.model.get("effort").and_then(Value::as_str);
    let (command, role) = (call.who.command(), call.who.role());
    let args = sheet.launch(command, &words, model, role, effort);
    Ok(Start {
        integration,
        sheet,
        model,
        args,
    })
}

/// How a call ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ended {
    /// With its exit code, or minus the signal that killed it.
    Code(i32),
    /// Out of time: its process group was killed.
    Timeout,
}

/// What a call did: how it ended, what it printed and how long it took.
struct Ran {
    ended: Ended,
    out: Vec<u8>,
    duration: Duration,
}

/// Calls the agent of `call` on the change of `project` (see the module) and waits for it, its
/// time limit at most. Its answer file goes away first. What it said is kept and logged even when
/// it fails or runs out of time.
pub fn run(project: &Project, call: &Call) -> Result<(), Error> {
    let start = start(call, project.id)?;
    let program = program(&start.sheet, project.env)?;
    if let Some(answer) = call.who.answer(project.id) {
        remove(project.root, &answer)?;
    }
    let ran = spawned(project, call.who, &program, &start.args)?;
    let said = usage::read(start.sheet.usage(), &String::from_utf8_lossy(&ran.out));
    keep(project, call.who, &start, &ran, said)?;
    match ran.ended {
        Ended::Code(0) => Ok(()),
        Ended::Code(code) => Err(Error::Failed(call.who, code)),
        Ended::Timeout => Err(Error::Timeout(call.who, project.timeout.as_secs())),
    }
}

/// Starts `program` with `args` for `who` on the change of `project` and waits for it to end and
/// close its output, the time limit at most; then its process group is killed, with whatever it
/// left running.
fn spawned(project: &Project, who: Who, program: &Path, args: &[String]) -> Result<Ran, Error> {
    let shown = program.display().to_string();
    let failed = |e| Error::Start(shown.clone(), e);
    let mut child = Command::new(program)
        .args(args)
        .current_dir(project.root)
        .env_clear()
        .envs(project.settings.env.of_agent(project.env))
        .env("GDD_CHANGE", project.id)
        .env("GDD_ROOT", project.root)
        .env("GDD_ROLE", who.role())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(failed)?;
    let stdout = child.stdout.take();
    let reader = thread::spawn(move || {
        let mut out = Vec::new();
        if let Some(mut pipe) = stdout {
            pipe.read_to_end(&mut out)?;
        }
        Ok::<_, io::Error>(out)
    });
    let start = Instant::now();
    // A process the agent started may keep its output open after it ends: the time limit counts
    // until the output closes too.
    let timed_out = loop {
        let exited = child.try_wait().map_err(failed)?.is_some();
        if exited && reader.is_finished() {
            break false;
        }
        if start.elapsed() >= project.timeout {
            break true;
        }
        thread::sleep(POLL);
    };
    // Whatever is left of its group, its output elsewhere, could still write after gdd checks
    // what the call did.
    exec::kill_group(&mut child);
    let status = child.wait().map_err(failed)?;
    let read = reader.join();
    let out = read.unwrap_or_else(|_| Err(io::Error::other("the output reader stopped")));
    let ended = if timed_out {
        Ended::Timeout
    } else {
        Ended::Code(code(status).map_err(failed)?)
    };
    Ok(Ran {
        ended,
        out: out.map_err(failed)?,
        duration: start.elapsed(),
    })
}

/// The exit code of `status`, or minus the signal that killed it. On Unix a process that ended
/// either exited or was killed by a signal: no code is made up for anything else.
fn code(status: ExitStatus) -> io::Result<i32> {
    let code = status.code().or_else(|| status.signal().map(|n| -n));
    code.ok_or_else(|| io::Error::other(format!("unknown exit status: {status}")))
}

/// A line of the log for an agent call, with every field in the order the log writes them.
#[derive(Serialize)]
struct Line<'a> {
    ts: &'a str,
    change: &'a str,
    phase: &'a str,
    gate: &'a str,
    role: &'a str,
    agent: &'a str,
    model: &'a str,
    tokens_in: u64,
    tokens_cache: u64,
    tokens_out: u64,
    cost_usd: f64,
    duration_s: f64,
    result: &'a str,
    retry: usize,
    note: &'a str,
}

/// Keeps what the call of `who`, started as `start`, did (`ran`) and said (`said`): its final text
/// and its output as `outputs/<role>-<n>.md` and `.raw` next to the change, `n` after the earlier
/// lines of its role on the change in the log; the Reviewer's text as its review; the session of
/// a Claude Code call; and one line in the log, priced with the model's price when the agent
/// reports no cost. A log that is not a file of its own stops it before it writes anything.
fn keep(project: &Project, who: Who, start: &Start, ran: &Ran, said: Said) -> Result<(), Error> {
    let (root, id, role) = (project.root, project.id, who.role());
    let earlier = earlier_lines(root, id, role)?;
    let outputs = format!(".gdd/changes/{id}/outputs/{role}-{}", earlier + 1);
    save(root, &format!("{outputs}.md"), said.text.as_bytes())?;
    save(root, &format!("{outputs}.raw"), &ran.out)?;
    if who == Who::Reviewer {
        save(root, &verdict::review_path(id), said.text.as_bytes())?;
    }
    if let Some(session) = &said.session {
        append(root, SESSIONS, &format!("{session}\n"))?;
    }
    let mut usage = said.usage;
    if usage.cost_usd == 0.0 {
        let price = project.settings.prices.get(start.model);
        usage.cost_usd = price.map_or(0.0, |price| usage.priced(price));
    }
    let result = match ran.ended {
        Ended::Code(0) => "ok".to_string(),
        Ended::Code(code) => format!("error-{code}"),
        Ended::Timeout => "timeout".to_string(),
    };
    let (phase, gate) = who.stage();
    let (ts, note) = (log::now(), format!("/{}", sheets::name(who.command())));
    let line = Line {
        ts: &ts,
        change: id,
        phase,
        gate,
        role,
        agent: start.integration,
        model: start.model,
        tokens_in: usage.tokens_in,
        tokens_cache: usage.tokens_cache,
        tokens_out: usage.tokens_out,
        cost_usd: usage.cost_usd,
        duration_s: seconds(ran.duration),
        result: &result,
        retry: earlier,
        note: &note,
    };
    let unwritten = |e| Error::Write(log::PATH.to_string(), e);
    let line = serde_json::to_string(&line).map_err(|e| unwritten(e.into()))?;
    log::append(root, &line).map_err(unwritten)
}

/// `duration` in seconds, to the tenth, as the log writes it.
fn seconds(duration: Duration) -> f64 {
    (duration.as_secs_f64() * 10.0).round() / 10.0
}

/// How many lines of the log of the project at `root` are of `role` on change `id`. Lines that
/// are not JSON objects are left out. A log that is not a file of its own (see [`files::read`])
/// is an error.
fn earlier_lines(root: &Path, id: &str, role: &str) -> Result<usize, Error> {
    let unread = |e| Error::Read(log::PATH.to_string(), e);
    let bytes = files::read(root, log::PATH).map_err(unread)?;
    let text = String::from_utf8_lossy(&bytes.unwrap_or_default()).into_owned();
    let says =
        |line: &Value, key: &str, value: &str| line.get(key).and_then(Value::as_str) == Some(value);
    let parsed = |line: &str| serde_json::from_str::<Value>(line).ok();
    let lines = text.lines().filter_map(parsed);
    let theirs = lines.filter(|line| says(line, "change", id) && says(line, "role", role));
    Ok(theirs.count())
}

/// Makes `path` of the project at `root` a new file with `bytes` (see [`files::replace`]).
fn save(root: &Path, path: &str, bytes: &[u8]) -> Result<(), Error> {
    files::replace(root, path, bytes).map_err(|e| Error::Write(path.to_string(), e))
}

/// Adds `text` at the end of the file `path` of the project at `root`, which must be a file of
/// its own if there is one (see [`files::read`]).
fn append(root: &Path, path: &str, text: &str) -> Result<(), Error> {
    let unwritten = |e| Error::Write(path.to_string(), e);
    let earlier = files::read(root, path).map_err(unwritten)?;
    let mut bytes = earlier.unwrap_or_default();
    bytes.extend_from_slice(text.as_bytes());
    files::replace(root, path, &bytes).map_err(unwritten)
}

/// Removes the file `path` of the project at `root`, if there is one.
fn remove(root: &Path, path: &str) -> Result<(), Error> {
    match fs::remove_file(root.join(path)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::Remove(path.to_string(), e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::Folder;

    fn model(integration: &str, name: &str) -> Model {
        let table = serde_json::json!({
            "integration": integration,
            "model": name,
            "effort": "high",
        });
        table.as_object().cloned().unwrap()
    }

    /// The arguments that start `who` with `args` on change `c1`, through `integration`.
    fn line(who: Who, integration: &str, args: &str) -> Vec<String> {
        line_of(who, &model(integration, "m-1"), args)
    }

    /// The arguments that start `who` with `args` on change `c1`, with the model `table`.
    fn line_of(who: Who, table: &Model, args: &str) -> Vec<String> {
        let call = Call {
            who,
            model: table,
            args,
        };
        let started = start(&call, "c1");
        started.map(|start| start.args).unwrap_or_default()
    }

    #[test]
    fn each_integration_is_started_as_its_sheet_says_for_the_role() {
        let args = line(Who::BuilderBuild, "claude", "mode=build fix=g3");
        let expected = [
            "-p",
            "/gdd-build change=c1 mode=build fix=g3",
            "--model",
            "m-1",
            "--output-format",
            "json",
            "--strict-mcp-config",
            "--permission-mode",
            "acceptEdits",
        ];
        assert_eq!(args[..9], expected);
        let args = line(Who::Reviewer, "codex", "round=2");
        let expected = [
            "exec",
            "--sandbox",
            "read-only",
            "-c",
            "mcp_servers={}",
            "-c",
            "model_reasoning_effort=high",
            "$gdd-review change=c1 round=2",
            "--model",
            "m-1",
            "--json",
        ];
        assert_eq!(args, expected);
        let args = line(Who::Orchestrator, "opencode", "reason=g2");
        let expected = [
            "run",
            "--command",
            "gdd-replan",
            "-m",
            "m-1",
            "--format",
            "json",
            "change=c1 reason=g2",
        ];
        assert_eq!(args, expected);
        let test = line(Who::BuilderTest, "claude", "mode=test");
        assert_eq!(test[1], "/gdd-build-test change=c1 mode=test");
        assert_eq!(line(Who::Reviewer, "claude", "round=1")[8], "plan");
        let sandbox = &line(Who::BuilderTest, "codex", "mode=test")[2];
        assert_eq!(sandbox, "workspace-write");
        // A model without an effort, or whose effort is not text, takes none.
        let mut table = model("codex", "m-1");
        table.remove("effort");
        let none = line_of(Who::Reviewer, &table, "round=1");
        table.insert("effort".into(), 3.into());
        assert_eq!(line_of(Who::Reviewer, &table, "round=1"), none);
        assert_eq!(none[5], "$gdd-review change=c1 round=1");
    }

    #[test]
    fn a_model_that_cannot_be_started_names_the_role() {
        let gemini = "gdd/models.toml: the Reviewer uses \"gemini\", which has no agent sheet \
                      (claude, codex, opencode)";
        let nameless = "gdd/models.toml: the Reviewer has no model";
        let none = "gdd/models.toml: the Reviewer uses \"\", which has no agent sheet (claude, \
                    codex, opencode)";
        let mut number = model("claude", "x");
        number.insert("integration".into(), 7.into());
        for (table, why) in [
            (model("claude", ""), nameless),
            (model("gemini", "g"), gemini),
            (model("", "g"), none),
            (number, none),
            (Model::new(), nameless),
        ] {
            let call = Call {
                who: Who::Reviewer,
                model: &table,
                args: "round=1",
            };
            let err = start(&call, "c1").err().unwrap();
            assert!(!err.parks());
            assert_eq!(err.to_string(), why);
        }
    }

    /// Roles whose models are all `claude` `m-1`.
    fn roles() -> Roles {
        Roles {
            builder: model("claude", "m-1"),
            builder_test: model("claude", "m-1"),
            reviewer_model: model("claude", "m-1"),
            orchestrator: model("claude", "m-1"),
        }
    }

    #[test]
    fn every_role_must_be_startable_before_anything_runs() {
        let faked = Faked::new("exit 0");
        // Only the folder of the fake: the tests may have a real Codex on their PATH.
        let bin = faked.folder.0.join("bin").display().to_string();
        let env = &[("PATH".to_string(), bin)];
        assert!(check(&roles(), env).is_ok());
        let said = |all: &Roles| check(all, env).unwrap_err().to_string();
        let mut one = roles();
        one.orchestrator = model("gemini", "g");
        let gemini = "gdd/models.toml: the Orchestrator uses \"gemini\", which has no agent \
                      sheet (claude, codex, opencode)";
        assert_eq!(said(&one), gemini);
        let start = |who: Who| format!("gdd/models.toml: the {who} uses \"gemini\"");
        let mut one = roles();
        one.builder = model("gemini", "g");
        assert!(said(&one).starts_with(&start(Who::BuilderBuild)));
        let mut one = roles();
        one.builder_test = model("gemini", "g");
        assert!(said(&one).starts_with(&start(Who::BuilderTest)));
        // The first role that cannot be started is the one named.
        one.reviewer_model = model("gemini", "g");
        assert!(said(&one).starts_with(&start(Who::BuilderTest)));
        let mut one = roles();
        one.reviewer_model = Model::new();
        assert_eq!(said(&one), "gdd/models.toml: the Reviewer has no model");
        // Every sheet first, then every program.
        let missing = check(&roles(), &[]).unwrap_err();
        assert!(!missing.parks());
        let claude = "cannot find the claude program of Claude Code on PATH";
        assert_eq!(missing.to_string(), claude);
        let mut one = roles();
        one.reviewer_model = model("gemini", "g");
        let err = check(&one, &[]).unwrap_err().to_string();
        assert!(err.starts_with(&start(Who::Reviewer)), "{err}");
        let mut one = roles();
        one.reviewer_model = model("codex", "g");
        let codex = "cannot find the codex program of Codex on PATH";
        assert_eq!(said(&one), codex);
    }

    #[test]
    fn the_program_is_found_on_the_path_of_the_environment() {
        let folder = Folder::new();
        let runnable = |path: &str| {
            folder.write(path, "#!/bin/sh\n");
            let mode = fs::Permissions::from_mode(0o700);
            fs::set_permissions(folder.0.join(path), mode).unwrap();
        };
        folder.write("a/claude", "#!/bin/sh\n");
        folder.write("b/claude/inside", "");
        runnable("c/claude");
        runnable("d/claude");
        let dir = |name: &str| folder.0.join(name).display().to_string();
        let sheet = sheets::sheet("claude").unwrap();
        let folders = ["a", "b", "c", "d"].map(dir).join(":");
        let env = [("PATH".to_string(), format!("relative::{folders}"))];
        assert_eq!(program(&sheet, &env).unwrap(), folder.0.join("c/claude"));
        // The last PATH counts.
        let path = |folder: &str| ("PATH".to_string(), dir(folder));
        let twice = [path("c"), path("d")];
        assert_eq!(program(&sheet, &twice).unwrap(), folder.0.join("d/claude"));
        let other = [("HOME".to_string(), dir("c"))];
        let none: [(String, String); 0] = [];
        let not_runnable = [path("a")];
        for env in [&other[..], &none[..], &not_runnable[..]] {
            let err = program(&sheet, env).unwrap_err();
            assert!(matches!(err, Error::Missing(..)), "{err}");
        }
    }

    #[test]
    fn errors_say_who_and_what() {
        let timeout = Error::Timeout(Who::BuilderBuild, 2);
        let said = "the Builder in build mode did not answer within 2 seconds";
        assert_eq!(timeout.to_string(), said);
        assert!(timeout.parks());
        let failed = Error::Failed(Who::Orchestrator, 3);
        assert_eq!(failed.to_string(), "the Orchestrator failed (exit 3)");
        assert!(failed.parks());
        let test = Error::Failed(Who::BuilderTest, -9);
        let said = "the Builder in test mode failed (exit -9)";
        assert_eq!(test.to_string(), said);
        let missing = Error::Missing("codex".into(), "Codex".into());
        let said = "cannot find the codex program of Codex on PATH";
        assert_eq!(missing.to_string(), said);
        assert!(!missing.parks());
        let no = || io::Error::other("no");
        for (error, said) in [
            (Error::Start("x".into(), no()), "start x"),
            (Error::Remove("r.md".into(), no()), "remove r.md"),
            (Error::Read("l".into(), no()), "read l"),
            (Error::Write("o".into(), no()), "write o"),
        ] {
            assert_eq!(error.to_string(), format!("cannot {said}: no"));
            assert!(!error.parks(), "{said}");
        }
    }

    #[test]
    fn the_log_writes_seconds_to_the_tenth() {
        let ms = Duration::from_millis;
        assert_eq!(seconds(ms(1234)), 1.2);
        assert_eq!(seconds(ms(1250)), 1.3);
        assert_eq!(seconds(ms(61_060)), 61.1);
        assert_eq!(seconds(Duration::ZERO), 0.0);
    }

    #[test]
    fn the_settings_of_the_agents_are_their_env_and_prices() {
        let text = "[reviewer]\nmodel = \"x\"\n[env]\npass = [\"A_TOKEN\"]\n\
                    [prices.opus]\ninput = 3\ncache = 0.3\noutput = 15\n";
        let settings = Settings::from_toml(text).unwrap();
        assert_eq!(settings.env.pass, ["A_TOKEN"]);
        let opus = Price {
            input: 3.0,
            cache: 0.3,
            output: 15.0,
        };
        assert_eq!(settings.prices.get("opus"), Some(&opus));
        assert_eq!(Settings::from_toml(""), Ok(Settings::default()));
        for wrong in ["[env]\npass = \"A\"\n", "[prices]\nopus = 3\n", "[env"] {
            let result = Settings::from_toml(wrong);
            assert!(matches!(result, Err(config::Error::Toml(_))), "{wrong:?}");
        }
    }

    /// A project whose `claude` is the shell script `script`, in its folder `bin`, the
    /// environment gdd runs in, with that folder first on the `PATH`, and the settings of its
    /// agents.
    struct Faked {
        folder: Folder,
        env: Vec<(String, String)>,
        settings: Settings,
    }

    impl Faked {
        fn new(script: &str) -> Faked {
            let folder = Folder::new();
            folder.write("bin/claude", &format!("#!/bin/sh\n{script}\n"));
            let mode = fs::Permissions::from_mode(0o755);
            fs::set_permissions(folder.0.join("bin/claude"), mode).unwrap();
            let bin = folder.0.join("bin");
            let tests = std::env::var("PATH").unwrap_or_default();
            let path = format!("{}:{tests}", bin.display());
            Faked {
                folder,
                env: vec![("PATH".into(), path)],
                settings: Settings::default(),
            }
        }

        /// Calls `who`, model `claude` `opus`, on change `c1`, with `timeout`. A process another
        /// test is starting can still hold the program just written open for writing, and until
        /// it lets it go the program cannot start ("text file busy"): then the call is tried
        /// again, a few times.
        fn call(&self, who: Who, timeout: Duration) -> Result<(), Error> {
            let table = model("claude", "opus");
            let call = Call {
                who,
                model: &table,
                args: "round=1",
            };
            let project = Project {
                root: &self.folder.0,
                id: "c1",
                env: &self.env,
                settings: &self.settings,
                timeout,
            };
            let mut tries = 1;
            loop {
                match run(&project, &call) {
                    Err(Error::Start(_, e))
                        if e.kind() == ErrorKind::ExecutableFileBusy && tries < 20 =>
                    {
                        tries += 1;
                        thread::sleep(POLL);
                    }
                    result => return result,
                }
            }
        }

        fn read(&self, path: &str) -> String {
            fs::read_to_string(self.folder.0.join(path)).unwrap_or_default()
        }

        fn write(&self, path: &str, text: &str) {
            self.folder.write(path, text);
        }

        /// The output `name` the calls left next to change `c1`.
        fn output(&self, name: &str) -> String {
            self.read(&format!(".gdd/changes/c1/outputs/{name}"))
        }

        fn has(&self, path: &str) -> bool {
            self.folder.0.join(path).exists()
        }

        /// The last line of the log.
        fn logged(&self) -> log::Record {
            let log = self.read(log::PATH);
            let lines = log::parse(log.lines().last().unwrap_or_default()).unwrap();
            lines.first().cloned().unwrap_or_default()
        }
    }

    const LONG: Duration = Duration::from_secs(60);

    #[test]
    fn the_agent_runs_from_the_root_with_its_role_and_without_secrets() {
        let script = "{ pwd -P; echo \"$GDD_CHANGE|$GDD_ROOT|$GDD_ROLE|${EDITOR-}|\
                      ${OPENAI_API_KEY-none}|${HOME-none}\"; printf '%s;' \"$@\"; } > seen; \
                      read x && exit 7; echo err >&2";
        let mut faked = Faked::new(script);
        for (name, value) in [
            ("EDITOR", "vim"),
            ("OPENAI_API_KEY", "sk"),
            ("GDD_ROLE", "x"),
        ] {
            faked.env.push((name.into(), value.into()));
        }
        faked.call(Who::BuilderBuild, LONG).unwrap();
        let seen = faked.read("seen");
        let cwd = fs::canonicalize(&faked.folder.0).unwrap();
        let (cwd, root) = (cwd.display(), faked.folder.0.display());
        let args = "-p;/gdd-build change=c1 round=1;--model;opus;--output-format;json;\
                    --strict-mcp-config;--permission-mode;acceptEdits;--allowedTools;";
        let start = format!("{cwd}\nc1|{root}|builder-build|vim|none|none\n{args}");
        assert!(seen.starts_with(&start), "{seen}");
        // A project passes a secret on purpose.
        faked.settings = Settings::from_toml("[env]\npass = [\"OPENAI_API_KEY\"]\n").unwrap();
        faked.call(Who::Reviewer, LONG).unwrap();
        let seen = faked.read("seen");
        assert!(seen.contains("|reviewer|vim|sk|none\n"), "{seen}");
    }

    /// What the fake agent prints: the file `said` of the project.
    const SAYS: &str = "cat said 2>/dev/null";

    /// A Claude Code result.
    const RESULT: &str = concat!(
        r#"{"result":"all done","session_id":"s-9","total_cost_usd":0.5,"#,
        r#""usage":{"input_tokens":100,"cache_creation_input_tokens":0,"#,
        r#""cache_read_input_tokens":50,"output_tokens":20}}"#
    );

    #[test]
    fn what_an_agent_says_is_kept_and_logged() {
        let faked = Faked::new(SAYS);
        faked.write("said", &format!("progress\n{RESULT}\n"));
        let earlier = "{\"change\": \"c1\", \"role\": \"builder-build\"}\nnot json\n[1]\n\
                       {\"change\": \"c2\", \"role\": \"builder-build\"}\n\
                       {\"change\": \"c1\", \"role\": \"reviewer\"}\n";
        faked.write(log::PATH, earlier);
        faked.call(Who::BuilderBuild, LONG).unwrap();
        // One earlier line of its role on the change: this is its second call.
        assert_eq!(faked.output("builder-build-2.md"), "all done");
        let raw = faked.output("builder-build-2.raw");
        assert_eq!(raw, format!("progress\n{RESULT}\n"));
        assert_eq!(faked.read(SESSIONS), "s-9\n");
        assert!(!faked.has(&verdict::review_path("c1")));
        let log = faked.read(log::PATH);
        let line = log.lines().last().unwrap_or_default();
        let fields = concat!(
            r#""change":"c1","phase":"2-build","gate":"G2","role":"builder-build","#,
            r#""agent":"claude","model":"opus","tokens_in":100,"tokens_cache":50,"#,
            r#""tokens_out":20,"cost_usd":0.5,"duration_s":"#
        );
        assert!(line.starts_with("{\"ts\":\""), "{line}");
        assert!(line.contains(fields), "{line}");
        let tail = r#","result":"ok","retry":1,"note":"/gdd-build"}"#;
        assert!(line.ends_with(tail), "{line}");
        assert!(faked.logged().duration_s < 10.0);
        // The Reviewer's text is its review.
        faked.call(Who::Reviewer, LONG).unwrap();
        assert_eq!(faked.read(&verdict::review_path("c1")), "all done");
        assert_eq!(faked.output("reviewer-2.md"), "all done");
        let reviewed = faked.logged();
        for (field, value) in [
            ("phase", "3-integrate"),
            ("gate", "G3"),
            ("role", "reviewer"),
            ("retry", "1"),
            ("note", "/gdd-review"),
        ] {
            assert_eq!(reviewed.text(field), value, "{field}");
        }
        assert_eq!(faked.read(SESSIONS), "s-9\ns-9\n");
        faked.call(Who::BuilderTest, LONG).unwrap();
        assert_eq!(faked.output("builder-test-1.md"), "all done");
        let tested = faked.logged();
        assert_eq!(tested.text("note"), "/gdd-build-test");
        assert_eq!(tested.text("retry"), "0");
        assert_eq!(tested.text("gate"), "G2");
        faked.call(Who::Orchestrator, LONG).unwrap();
        assert_eq!(faked.logged().text("phase"), "2-build");
        assert_eq!(faked.output("orchestrator-1.md"), "all done");
    }

    #[test]
    fn an_agent_that_reports_no_cost_is_priced_with_its_model() {
        let mut faked = Faked::new(SAYS);
        let used = concat!(
            r#"{"usage":{"input_tokens":1000000,"cache_read_input_tokens":2000000,"#,
            r#""output_tokens":100000}}"#
        );
        faked.write("said", used);
        let prices = "[prices.opus]\ninput = 3\ncache = 0.3\noutput = 15\n\
                      [prices.other]\ninput = 100\n";
        faked.settings = Settings::from_toml(prices).unwrap();
        faked.call(Who::BuilderBuild, LONG).unwrap();
        let cost = faked.logged().cost_usd;
        assert!((cost - 5.1).abs() < 1e-9, "{cost}");
        // A cost the agent reports stays.
        faked.write("said", RESULT);
        faked.call(Who::BuilderBuild, LONG).unwrap();
        assert_eq!(faked.logged().cost_usd, 0.5);
        // A model without a price costs nothing.
        faked.settings = Settings::default();
        faked.write("said", used);
        faked.call(Who::BuilderBuild, LONG).unwrap();
        assert_eq!(faked.logged().cost_usd, 0.0);
        assert_eq!(faked.logged().tokens_in, 1_000_000);
    }

    #[test]
    fn output_that_is_not_json_is_the_final_text_and_nothing_was_used() {
        let faked = Faked::new("echo plain words");
        faked.call(Who::Reviewer, LONG).unwrap();
        assert_eq!(faked.read(&verdict::review_path("c1")), "plain words\n");
        let logged = faked.logged();
        assert_eq!((logged.tokens_in, logged.tokens_out), (0, 0));
        assert!(!faked.has(SESSIONS));
    }

    #[test]
    fn the_output_is_read_while_the_agent_runs() {
        let faked = Faked::new("seq 1 200000; echo '{\"result\": \"end\"}'");
        faked.call(Who::BuilderBuild, LONG).unwrap();
        assert_eq!(faked.output("builder-build-1.md"), "end");
    }

    #[test]
    fn the_answer_file_goes_away_before_the_call() {
        let script = "case $GDD_ROLE in orchestrator) f=replan.md ;; *) f=review.md ;; esac; \
                      test -f .gdd/changes/c1/$f && exit 4; echo '{\"result\": \"new\"}'";
        let faked = Faked::new(script);
        faked.write(".gdd/changes/c1/review.md", "VERDICT: OK\n");
        faked.write(".gdd/changes/c1/replan.md", "NEXT: test\n");
        assert!(faked.call(Who::Reviewer, LONG).is_ok());
        assert_eq!(faked.read(".gdd/changes/c1/review.md"), "new");
        assert!(faked.has(".gdd/changes/c1/replan.md"));
        assert!(faked.call(Who::Orchestrator, LONG).is_ok());
        assert!(!faked.has(".gdd/changes/c1/replan.md"));
        assert!(faked.call(Who::BuilderBuild, LONG).is_err());
        assert_eq!(faked.read(".gdd/changes/c1/review.md"), "new");
        fs::remove_file(faked.folder.0.join(".gdd/changes/c1/review.md")).unwrap();
        fs::create_dir_all(faked.folder.0.join(".gdd/changes/c1/review.md/x")).unwrap();
        let err = faked.call(Who::Reviewer, LONG).unwrap_err();
        let said = "cannot remove .gdd/changes/c1/review.md: ";
        assert!(err.to_string().starts_with(said), "{err}");
    }

    #[test]
    fn an_agent_that_fails_says_how_and_is_logged() {
        let faked = Faked::new("exit 3");
        let err = faked.call(Who::BuilderBuild, LONG).unwrap_err();
        let said = "the Builder in build mode failed (exit 3)";
        assert_eq!(err.to_string(), said);
        assert_eq!(faked.logged().text("result"), "error-3");
        let faked = Faked::new("kill -TERM $$");
        let err = faked.call(Who::Reviewer, LONG).unwrap_err();
        assert_eq!(err.to_string(), "the Reviewer failed (exit -15)");
        assert_eq!(faked.logged().text("result"), "error--15");
        let mut faked = Faked::new("exit 0");
        faked.env = vec![("PATH".into(), "/nonexistent".into())];
        let err = faked.call(Who::BuilderTest, LONG).unwrap_err();
        let said = "cannot find the claude program of Claude Code on PATH";
        assert_eq!(err.to_string(), said);
        assert!(!err.parks());
        assert!(!faked.has(log::PATH));
    }

    #[test]
    fn what_a_call_keeps_never_goes_through_a_link() {
        use std::os::unix::fs::symlink;
        let faked = Faked::new(SAYS);
        faked.write("said", RESULT);
        let outside = Folder::new();
        outside.write("f", "outside");
        let (f, root) = (outside.0.join("f"), &faked.folder.0);
        let outputs = root.join(".gdd/changes/c1/outputs");
        fs::create_dir_all(&outputs).unwrap();
        symlink(&f, outputs.join("reviewer-1.md")).unwrap();
        fs::hard_link(&f, outputs.join("reviewer-1.raw")).unwrap();
        faked.call(Who::Reviewer, LONG).unwrap();
        assert_eq!(faked.output("reviewer-1.md"), "all done");
        assert_eq!(faked.output("reviewer-1.raw"), RESULT);
        // A folder on the way that links elsewhere.
        fs::rename(&outputs, root.join("moved")).unwrap();
        symlink(&outside.0, &outputs).unwrap();
        faked.call(Who::BuilderBuild, LONG).unwrap();
        assert_eq!(faked.output("builder-build-1.md"), "all done");
        assert!(!outside.0.join("builder-build-1.md").exists());
        // The sessions and the log are added to only when they are files of their own.
        fs::remove_file(root.join(SESSIONS)).unwrap();
        symlink(&f, root.join(SESSIONS)).unwrap();
        let err = faked.call(Who::BuilderBuild, LONG).unwrap_err();
        let said = format!("cannot write {SESSIONS}: not a file of its own");
        assert!(err.to_string().starts_with(&said), "{err}");
        fs::remove_file(root.join(SESSIONS)).unwrap();
        fs::remove_file(root.join(log::PATH)).unwrap();
        fs::hard_link(&f, root.join(log::PATH)).unwrap();
        let err = faked.call(Who::BuilderBuild, LONG).unwrap_err();
        let said = format!("cannot read {}: not a file of its own", log::PATH);
        assert!(err.to_string().starts_with(&said), "{err}");
        assert_eq!(fs::read_to_string(&f).unwrap(), "outside");
    }

    #[test]
    fn a_log_that_cannot_be_read_stops_the_call() {
        let faked = Faked::new(SAYS);
        faked.write("said", RESULT);
        fs::create_dir_all(faked.folder.0.join(log::PATH)).unwrap();
        let err = faked.call(Who::BuilderBuild, LONG).unwrap_err();
        let said = format!("cannot read {}: ", log::PATH);
        assert!(err.to_string().starts_with(&said), "{err}");
        assert!(!err.parks());
    }

    /// Whether process `pid` is gone, or a zombie that only waits to be collected.
    fn gone(pid: &str) -> bool {
        match fs::read_to_string(format!("/proc/{pid}/stat")) {
            Err(_) => true,
            Ok(stat) => stat
                .rsplit_once(')')
                .is_some_and(|(_, state)| state.trim_start().starts_with('Z')),
        }
    }

    /// Whether process `pid` goes within 5 seconds: a killed process takes a moment to go.
    fn goes(pid: &str) -> bool {
        let start = Instant::now();
        while !gone(pid) && start.elapsed() < Duration::from_secs(5) {
            thread::sleep(POLL);
        }
        gone(pid)
    }

    #[test]
    fn an_agent_out_of_time_is_stopped_with_everything_it_started() {
        let faked = Faked::new("sleep 30 & echo $! > child; wait");
        let start = Instant::now();
        let limit = Duration::from_millis(300);
        let err = faked.call(Who::BuilderBuild, limit).unwrap_err();
        assert!(start.elapsed() < Duration::from_secs(10), "not stopped");
        let said = "the Builder in build mode did not answer within 0 seconds";
        assert_eq!(err.to_string(), said);
        assert_eq!(faked.logged().text("result"), "timeout");
        let child = faked.read("child");
        assert!(goes(child.trim()), "the agent's child still runs");
    }

    #[test]
    fn nothing_an_agent_started_writes_after_its_call() {
        // The Reviewer ends at once and well; what it left behind, its output elsewhere, would
        // write a second later, after gdd checked that it only read.
        let faked = Faked::new("(sleep 1; echo late > late) >/dev/null 2>&1 & echo $! > child");
        faked.call(Who::Reviewer, LONG).unwrap();
        let child = faked.read("child");
        assert!(goes(child.trim()), "the agent's child still runs");
        assert!(!faked.has("late"), "the agent's child wrote after the call");
    }

    #[test]
    fn an_agent_that_leaves_its_output_open_runs_out_of_time() {
        // The agent ends at once; what it left behind keeps its output open for a few seconds,
        // so a call that waited for it would end well, not hang the tests.
        let faked = Faked::new("sleep 5 &");
        let (start, limit) = (Instant::now(), Duration::from_millis(300));
        let err = faked.call(Who::BuilderBuild, limit).unwrap_err();
        assert!(start.elapsed() < Duration::from_secs(4), "not stopped");
        assert!(matches!(err, Error::Timeout(Who::BuilderBuild, 0)), "{err}");
    }
}
