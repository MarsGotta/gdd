//! Starting an agent of the recipe: always the wrapper of its integration,
//! `<root>/.gdd/bin/gdd-<integration>`, with fixed arguments and never through a shell, from the
//! project root, with `GDD_CHANGE` and `GDD_ROOT` and without `GDD_ROLE`. The wrapper logs and
//! keeps what the agent says; the tool prints none of it.
//!
//! Each call runs in a process group of its own and has a time limit: when it runs out, the whole
//! group is killed, never a process read from a file. The file where the Reviewer or the replan
//! answers goes away before the call, so an old answer never counts.

use std::fmt;
use std::fs;
use std::io::{self, ErrorKind};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::agents::models::{self, Model};
use crate::exec;
use crate::gates::verdict;

/// How often a running agent is checked.
const POLL: Duration = Duration::from_millis(50);

/// The integrations the tool starts, as the error of another one says.
const INTEGRATIONS: &str = "use claude, codex or opencode";

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
    /// The command of the method it runs: `speckit-gdd-<command>`.
    fn command(self) -> &'static str {
        match self {
            Who::BuilderTest => "build-test",
            Who::BuilderBuild => "build",
            Who::Reviewer => "review",
            Who::Orchestrator => "replan",
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
    /// Its model, as `gdd/models.toml` writes it: the integration and the model name count.
    pub model: &'a Model,
    /// The arguments of its command after `change=<id>`: `mode=build fix=g3`...
    pub args: &'a str,
}

/// Why a call to an agent did not end well.
#[derive(Debug)]
pub enum Error {
    /// The model of the role cannot be started: who, and why.
    Model(Who, String),
    /// A file could not be removed before the call: which, and why.
    Remove(String, io::Error),
    /// The wrapper could not be started or waited for: which, and why.
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
            Error::Remove(path, e) => write!(f, "cannot remove {path}: {e}"),
            Error::Start(program, e) => write!(f, "cannot start {program}: {e}"),
            Error::Timeout(who, seconds) => {
                write!(f, "the {who} did not answer within {seconds} seconds")
            }
            Error::Failed(who, code) => write!(f, "the {who} failed (exit {code})"),
        }
    }
}

impl std::error::Error for Error {}

/// The wrapper that starts `call` on change `id`, from the project root, and its arguments, by
/// integration:
/// `claude -p "/speckit-gdd-<cmd> change=<id> <args>" --model <m> --output-format json`,
/// `codex exec "$speckit-gdd-<cmd> change=<id> <args>" --model <m> --json`, or
/// `opencode run --command speckit.gdd.<cmd> -m <m> --format json "change=<id> <args>"`.
fn command_line(call: &Call, id: &str) -> Result<(String, Vec<String>), Error> {
    let table = call.model;
    let text = |key| table.get(key).and_then(Value::as_str).unwrap_or_default();
    let (integration, model) = (text("integration"), text("model").to_string());
    if model.is_empty() {
        return Err(Error::Model(call.who, "has no model".to_string()));
    }
    let cmd = call.who.command();
    let words = format!("change={id} {}", call.args);
    let args = match integration {
        "claude" => vec![
            "-p".to_string(),
            format!("/speckit-gdd-{cmd} {words}"),
            "--model".to_string(),
            model,
            "--output-format".to_string(),
            "json".to_string(),
        ],
        "codex" => vec![
            "exec".to_string(),
            format!("$speckit-gdd-{cmd} {words}"),
            "--model".to_string(),
            model,
            "--json".to_string(),
        ],
        "opencode" => vec![
            "run".to_string(),
            "--command".to_string(),
            format!("speckit.gdd.{cmd}"),
            "-m".to_string(),
            model,
            "--format".to_string(),
            "json".to_string(),
            words,
        ],
        other => {
            let why = format!("has the integration {other:?}: {INTEGRATIONS}");
            return Err(Error::Model(call.who, why));
        }
    };
    Ok((format!(".gdd/bin/gdd-{integration}"), args))
}

/// Calls the agent of `call` on change `id` of the project at `root` (see the module) and waits
/// for it, `timeout` at most. Its answer file goes away first.
pub fn run(root: &Path, id: &str, call: &Call, timeout: Duration) -> Result<(), Error> {
    let (program, args) = command_line(call, id)?;
    if let Some(answer) = call.who.answer(id) {
        remove(root, &answer)?;
    }
    let started = |e| Error::Start(program.clone(), e);
    let mut child = Command::new(root.join(&program))
        .args(&args)
        .current_dir(root)
        .env("GDD_CHANGE", id)
        .env("GDD_ROOT", root)
        .env_remove("GDD_ROLE")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(started)?;
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(started)? {
            break status;
        }
        if start.elapsed() >= timeout {
            exec::kill_group(&mut child);
            child.wait().map_err(started)?;
            return Err(Error::Timeout(call.who, timeout.as_secs()));
        }
        thread::sleep(POLL);
    };
    let code = status.code().or_else(|| status.signal().map(|n| -n));
    match code {
        Some(0) => Ok(()),
        Some(code) => Err(Error::Failed(call.who, code)),
        None => {
            let why = format!("unknown exit status: {status}");
            Err(started(io::Error::other(why)))
        }
    }
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
    use std::os::unix::fs::PermissionsExt;

    fn model(integration: &str, name: &str) -> Model {
        let table = serde_json::json!({
            "integration": integration,
            "model": name,
            "effort": "high",
        });
        table.as_object().cloned().unwrap()
    }

    /// The wrapper and the arguments that start `who` with `args` on change `c1`, through
    /// `integration`.
    fn line(who: Who, integration: &str, args: &str) -> (String, Vec<String>) {
        let model = model(integration, "m-1");
        let call = Call {
            who,
            model: &model,
            args,
        };
        command_line(&call, "c1").unwrap()
    }

    #[test]
    fn each_integration_gets_its_fixed_arguments() {
        let (program, args) = line(Who::BuilderBuild, "claude", "mode=build fix=g3");
        assert_eq!(program, ".gdd/bin/gdd-claude");
        let expected = [
            "-p",
            "/speckit-gdd-build change=c1 mode=build fix=g3",
            "--model",
            "m-1",
            "--output-format",
            "json",
        ];
        assert_eq!(args, expected);
        let (program, args) = line(Who::Reviewer, "codex", "round=2");
        assert_eq!(program, ".gdd/bin/gdd-codex");
        let expected = [
            "exec",
            "$speckit-gdd-review change=c1 round=2",
            "--model",
            "m-1",
            "--json",
        ];
        assert_eq!(args, expected);
        let (program, args) = line(Who::Orchestrator, "opencode", "reason=g2");
        assert_eq!(program, ".gdd/bin/gdd-opencode");
        let expected = [
            "run",
            "--command",
            "speckit.gdd.replan",
            "-m",
            "m-1",
            "--format",
            "json",
            "change=c1 reason=g2",
        ];
        assert_eq!(args, expected);
        let (_, args) = line(Who::BuilderTest, "claude", "mode=test");
        assert_eq!(args[1], "/speckit-gdd-build-test change=c1 mode=test");
    }

    #[test]
    fn a_model_that_cannot_be_started_names_the_role() {
        let gemini = "gdd/models.toml: the Reviewer has the integration \"gemini\": use \
                      claude, codex or opencode";
        let nameless = "gdd/models.toml: the Reviewer has no model";
        for (table, why) in [
            (model("claude", ""), nameless),
            (model("gemini", "g"), gemini),
            (Model::new(), nameless),
        ] {
            let call = Call {
                who: Who::Reviewer,
                model: &table,
                args: "round=1",
            };
            let err = command_line(&call, "c1").unwrap_err();
            assert!(!err.parks());
            assert_eq!(err.to_string(), why);
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
        let start = Error::Start("x".into(), io::Error::other("no"));
        assert_eq!(start.to_string(), "cannot start x: no");
        assert!(!start.parks());
        let remove = Error::Remove("r.md".into(), io::Error::other("no"));
        assert_eq!(remove.to_string(), "cannot remove r.md: no");
        assert!(!remove.parks());
    }

    /// A project whose `gdd-claude` wrapper is the shell script `script`.
    fn wrapped(script: &str) -> Folder {
        let folder = Folder::new();
        let path = ".gdd/bin/gdd-claude";
        folder.write(path, &format!("#!/bin/sh\n{script}\n"));
        let mode = fs::Permissions::from_mode(0o755);
        fs::set_permissions(folder.0.join(path), mode).unwrap();
        folder
    }

    /// Calls `who` through the wrapper of `folder`, with `timeout`.
    fn call_in(folder: &Folder, who: Who, timeout: Duration) -> Result<(), Error> {
        let table = model("claude", "opus");
        let call = Call {
            who,
            model: &table,
            args: "round=1",
        };
        run(&folder.0, "c1", &call, timeout)
    }

    const LONG: Duration = Duration::from_secs(60);

    #[test]
    fn the_wrapper_runs_from_the_root_with_the_change_and_without_a_role() {
        let script = "{ pwd -P; echo \"$GDD_CHANGE|$GDD_ROOT|${GDD_ROLE-unset}\"; \
                      printf '%s;' \"$@\"; } > seen; read x && exit 7; echo out; echo err >&2";
        let folder = wrapped(script);
        assert!(call_in(&folder, Who::BuilderBuild, LONG).is_ok());
        let seen = fs::read_to_string(folder.0.join("seen")).unwrap();
        let cwd = fs::canonicalize(&folder.0).unwrap();
        let (cwd, root) = (cwd.display(), folder.0.display());
        let args = "-p;/speckit-gdd-build change=c1 round=1;--model;opus;--output-format;json;";
        let expected = format!("{cwd}\nc1|{root}|unset\n{args}");
        assert_eq!(seen.trim_end(), expected);
    }

    #[test]
    fn the_answer_file_goes_away_before_the_call() {
        let folder = wrapped("test -f .gdd/changes/c1/review.md && exit 4; exit 0");
        folder.write(".gdd/changes/c1/review.md", "VERDICT: OK\n");
        folder.write(".gdd/changes/c1/replan.md", "NEXT: test\n");
        let gone = |path: &str| !folder.0.join(path).exists();
        assert!(call_in(&folder, Who::Reviewer, LONG).is_ok());
        assert!(gone(".gdd/changes/c1/review.md"));
        assert!(!gone(".gdd/changes/c1/replan.md"));
        assert!(call_in(&folder, Who::Orchestrator, LONG).is_ok());
        assert!(gone(".gdd/changes/c1/replan.md"));
        folder.write(".gdd/changes/c1/review.md", "x");
        assert!(call_in(&folder, Who::BuilderBuild, LONG).is_err());
        assert!(!gone(".gdd/changes/c1/review.md"));
        fs::remove_file(folder.0.join(".gdd/changes/c1/review.md")).unwrap();
        fs::create_dir_all(folder.0.join(".gdd/changes/c1/review.md/x")).unwrap();
        let err = call_in(&folder, Who::Reviewer, LONG).unwrap_err();
        let said = "cannot remove .gdd/changes/c1/review.md: ";
        assert!(err.to_string().starts_with(said), "{err}");
    }

    #[test]
    fn an_agent_that_fails_says_how() {
        let folder = wrapped("exit 3");
        let err = call_in(&folder, Who::BuilderBuild, LONG).unwrap_err();
        let said = "the Builder in build mode failed (exit 3)";
        assert_eq!(err.to_string(), said);
        let folder = wrapped("kill -TERM $$");
        let err = call_in(&folder, Who::Reviewer, LONG).unwrap_err();
        assert_eq!(err.to_string(), "the Reviewer failed (exit -15)");
        let folder = Folder::new();
        let err = call_in(&folder, Who::BuilderTest, LONG).unwrap_err();
        let said = "cannot start .gdd/bin/gdd-claude: ";
        assert!(err.to_string().starts_with(said), "{err}");
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

    #[test]
    fn an_agent_out_of_time_is_stopped_with_everything_it_started() {
        let folder = wrapped("sleep 30 & echo $! > child; wait");
        let start = Instant::now();
        let limit = Duration::from_millis(300);
        let err = call_in(&folder, Who::BuilderBuild, limit).unwrap_err();
        assert!(start.elapsed() < Duration::from_secs(10), "not stopped");
        let said = "the Builder in build mode did not answer within 0 seconds";
        assert_eq!(err.to_string(), said);
        let pid = fs::read_to_string(folder.0.join("child")).unwrap();
        // A killed process takes a moment to go.
        let killed = Instant::now();
        while !gone(pid.trim()) && killed.elapsed() < Duration::from_secs(5) {
            thread::sleep(POLL);
        }
        assert!(gone(pid.trim()), "the agent's child still runs");
    }
}
