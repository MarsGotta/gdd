//! Running a gate command: the only place where the tool runs a shell.
//!
//! The commands come from `gdd/commands.toml`, which is harness: a human approves it and Path
//! Guard protects it. Text written by an agent never reaches a shell. A command runs with
//! `sh -c` from the project root, in a process group of its own, so a timeout kills it with
//! everything it started.

use std::io::{self, Read};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// How long a command may run, with its output, when the rules file says nothing: the gate and
/// release commands take their limit from it (see [`crate::rules::command_seconds`]).
pub const TIMEOUT: Duration = Duration::from_secs(1800);

/// The exit code of a command that ran out of time, as `timeout(1)` gives it.
pub const TIMED_OUT: i32 = 124;

/// How often a running command is checked.
const POLL: Duration = Duration::from_millis(20);

/// What a gate or release command that ran out of time says after how long it had: how to give
/// it more.
const RAISE: &str = ": raise limits.command_seconds in gdd/rules.toml";

/// What a command did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran {
    /// Its exit code: [`TIMED_OUT`] when it ran out of time, minus the signal that killed it
    /// when one did.
    pub code: i32,
    /// What it wrote on its standard output, then on its standard error; after a timeout, a
    /// last line that says so.
    pub output: String,
    /// How long it ran.
    pub duration: Duration,
}

impl Ran {
    /// Whether the command passed: exit code 0.
    pub fn ok(&self) -> bool {
        self.code == 0
    }
}

/// Runs `command` with `sh -c` from `root`, with the variables `env` added to the environment
/// and nothing on its standard input. The command has `timeout` to end and close its output;
/// then its process group is killed: the shell and everything it started. Not being able to
/// start the shell, or to tell how it ended, is an error.
pub fn run(command: &str, root: &Path, env: &[(&str, &str)], timeout: Duration) -> io::Result<Ran> {
    run_saying(command, root, env, timeout, "")
}

/// Runs a gate or release command as [`run`] does, with `limit`, the time limit the rules file
/// gives those commands (see [`crate::rules::command_seconds`]): when it runs out of time, its
/// last line also says how to raise the limit.
pub fn run_limited(
    command: &str,
    root: &Path,
    env: &[(&str, &str)],
    limit: Duration,
) -> io::Result<Ran> {
    run_saying(command, root, env, limit, RAISE)
}

/// Runs `command` (see [`run`]); after a timeout, its last line is `timed out (<n>s)` followed by
/// `after`.
fn run_saying(
    command: &str,
    root: &Path,
    env: &[(&str, &str)],
    timeout: Duration,
    after: &str,
) -> io::Result<Ran> {
    let start = Instant::now();
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(root)
        .envs(env.iter().copied())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()?;
    let stdout = read_all(child.stdout.take());
    let stderr = read_all(child.stderr.take());
    // A process the command started may keep its output open after the shell ends: the timeout
    // counts until the output closes too.
    let timed_out = loop {
        let exited = child.try_wait()?.is_some();
        if exited && stdout.is_finished() && stderr.is_finished() {
            break false;
        }
        if start.elapsed() >= timeout {
            break true;
        }
        thread::sleep(POLL);
    };
    if timed_out {
        kill_group(&mut child);
    }
    let status = child.wait()?;
    let mut output = String::from_utf8_lossy(&joined(stdout)?).into_owned();
    output.push_str(&String::from_utf8_lossy(&joined(stderr)?));
    let code = if timed_out {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(&format!("timed out ({}s){after}", timeout.as_secs_f64()));
        TIMED_OUT
    } else {
        // On Unix a process that ended either exited or was killed by a signal: no code is
        // made up for anything else.
        status
            .code()
            .or_else(|| status.signal().map(|signal| -signal))
            .ok_or_else(|| io::Error::other(format!("unknown exit status: {status}")))?
    };
    Ok(Ran {
        code,
        output,
        duration: start.elapsed(),
    })
}

/// Reads `pipe`, an output of the command, to its end in a thread of its own, so a full pipe
/// never stops the command.
fn read_all<R: Read + Send + 'static>(pipe: Option<R>) -> JoinHandle<io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            pipe.read_to_end(&mut bytes)?;
        }
        Ok(bytes)
    })
}

/// What a reader thread read.
fn joined(reader: JoinHandle<io::Result<Vec<u8>>>) -> io::Result<Vec<u8>> {
    reader
        .join()
        .unwrap_or_else(|_| Err(io::Error::other("the output reader stopped")))
}

/// Kills the process group `child` leads, started with a process group of its own: the child
/// and everything it started. The standard library signals one process, not a group, so the
/// program `kill` does it; the child is killed directly too, so it dies even when `kill` cannot
/// run. (A child already ended and waited for is not signalled again.)
pub(crate) fn kill_group(child: &mut Child) {
    let group = format!("-{}", child.id());
    let _ = Command::new("kill")
        .args(["-KILL", "--", &group])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.kill();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::Folder;

    const SHORT: Duration = Duration::from_millis(300);

    fn sh(command: &str, env: &[(&str, &str)]) -> Ran {
        let folder = Folder::new();
        run(command, &folder.0, env, TIMEOUT).unwrap()
    }

    /// Runs `command`, which would go on for 5 s, with the timeout [`SHORT`]: it must be
    /// stopped long before. A run that fails to stop it still ends in seconds, so mutation
    /// testing sees a failure and not a timeout.
    fn timed_out(command: &str) -> Ran {
        let folder = Folder::new();
        let start = Instant::now();
        let ran = run(command, &folder.0, &[], SHORT).unwrap();
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "not stopped: {command}"
        );
        assert_eq!(ran.code, TIMED_OUT);
        ran
    }

    #[test]
    fn a_command_gives_its_code_and_its_output_then_its_errors() {
        let ran = sh("echo err >&2; echo out; exit 3", &[]);
        assert_eq!((ran.code, ran.output.as_str()), (3, "out\nerr\n"));
        assert!(!ran.ok());
        let ran = sh("true", &[]);
        assert_eq!((ran.code, ran.output.as_str()), (0, ""));
        assert!(ran.ok());
        assert!(ran.duration < Duration::from_secs(10));
    }

    #[test]
    fn a_command_runs_from_the_root_with_the_variables_given() {
        let folder = Folder::new();
        folder.write("marker", "x");
        let env = [("GDD_X", "a b"), ("GDD_Y", "1\n2")];
        let check = "test -f marker && test \"$GDD_X\" = 'a b' && \
                     test \"$(echo \"$GDD_Y\" | wc -l)\" -eq 2";
        let ran = run(check, &folder.0, &env, TIMEOUT).unwrap();
        assert_eq!(ran.code, 0, "{}", ran.output);
        let ran = run("read line", &folder.0, &[], TIMEOUT).unwrap();
        assert_eq!(ran.code, 1, "nothing to read on the standard input");
    }

    #[test]
    fn a_command_killed_by_a_signal_gives_minus_the_signal() {
        assert_eq!(sh("kill -TERM $$", &[]).code, -15);
    }

    #[test]
    fn a_command_out_of_time_is_killed_and_says_so() {
        let ran = timed_out("echo started; sleep 5");
        assert_eq!(ran.output, "started\ntimed out (0.3s)");
        assert!(ran.duration >= SHORT);
        let ran = timed_out("printf started; sleep 5");
        assert_eq!(ran.output, "started\ntimed out (0.3s)");
        let ran = timed_out("sleep 5");
        assert_eq!(ran.output, "timed out (0.3s)");
    }

    #[test]
    fn a_gate_command_out_of_time_says_how_to_raise_its_limit() {
        let folder = Folder::new();
        let ran = run_limited("echo started; sleep 5", &folder.0, &[], SHORT).unwrap();
        assert_eq!(ran.code, TIMED_OUT);
        let said = "started\ntimed out (0.3s): raise limits.command_seconds in gdd/rules.toml";
        assert_eq!(ran.output, said);
        // A whole number of seconds is shown without decimals.
        let ran = run_limited("sleep 5", &folder.0, &[], Duration::from_secs(1)).unwrap();
        let said = "timed out (1s): raise limits.command_seconds in gdd/rules.toml";
        assert_eq!((ran.code, ran.output.as_str()), (TIMED_OUT, said));
        // In time, it is the command as it ran.
        let ran = run_limited("echo done; exit 2", &folder.0, &[], TIMEOUT).unwrap();
        assert_eq!((ran.code, ran.output.as_str()), (2, "done\n"));
    }

    #[test]
    fn a_timeout_kills_what_the_command_started_too() {
        // The shell ends at once; the sleep it left behind keeps the output open.
        let ran = timed_out("sleep 5 & echo started");
        assert_eq!(ran.output, "started\ntimed out (0.3s)");
    }

    #[test]
    fn a_command_that_closes_its_output_still_runs_out_of_time() {
        // The output ends at once; the shell goes on.
        let ran = timed_out("exec >&- 2>&-; sleep 5");
        assert_eq!(ran.output, "timed out (0.3s)");
        let ran = timed_out("exec 2>&-; echo started; sleep 5");
        assert_eq!(ran.output, "started\ntimed out (0.3s)");
    }

    #[test]
    fn the_output_is_read_while_the_command_runs() {
        let ran = sh("seq 1 200000", &[]);
        assert_eq!(ran.code, 0);
        assert!(ran.output.ends_with("\n200000\n"));
    }

    #[test]
    fn output_that_is_not_utf8_is_kept_readable() {
        let ran = sh("printf 'a\\377b'", &[]);
        assert_eq!(ran.output, "a\u{fffd}b");
    }
}
