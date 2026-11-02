//! The harness self-test (`tminus selftest`): four checks that the gates of a project fail when
//! they must. The machines open G0 only once all four pass.
//!
//! 1. A failing test turns G2 red: the file at the path of the test `[selftest]` describes is
//!    set aside, and G2 must be green without it; then the test is planted, and G2 must turn
//!    red. G2 is judged as the gate judges it: a command red when it fails and is not held to its
//!    known debt (see [`debt`]). The file is given back as it was, from a copy kept in
//!    `.tminus/selftest/kept/` meanwhile. Neither planting nor giving back writes through a link,
//!    not even one the G2 commands made: when one stops the file from going back, the copy stays.
//! 2. Editing a protected file is blocked (Path Guard and CI): the harness covers the rules file
//!    and the CI workflow, and the workflow runs `tminus ci` or `gdd ci`, both this program. The
//!    `bin/tminus ci` or `bin/gdd ci` of a project set up by an earlier version contains it, so it
//!    still counts. The rules file and the workflow are the ones of the project's layout (see
//!    [`crate::layout`]).
//! 3. Rollback works in pre: `[release] rollback_pre` passes or, without it, the release is
//!    simulated.
//! 4. Zero scenarios counts as a failure: G3 counts none in an empty folder, and the rules keep
//!    zero scenarios a failure.
//!
//! Then each gate the blueprint declares is seen to close, in its order (see [`gate_closes`]):
//! the failure the commands file plants for it, in the table its close test names, must turn its
//! stage red, which is green without it. Its commands there, the ones it claims but the optional
//! ones, run without and with the failure, held to the known debt of the stage; the file at its
//! path is set aside and given back as for the first check. A gate planted from `[selftest]` in
//! G2 reads the runs of the first check, which runs every G2 command with its test, instead of
//! running them again. What the self-test saw of each gate is kept with its lock (see
//! [`Closed`]): the engine counts a gate's green as proof only while the gate closed with its
//! lock as it is now.

use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, ErrorKind::AlreadyExists, ErrorKind::NotFound, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::{Deserialize, Serialize};

use crate::agents::files;
use crate::agents::render::first_link;
use crate::gates::decl::{Closed, Declared, GateDecl};
use crate::gates::{COMMANDS, Gate, Project, approve, configured, count_scenarios, debt};
use crate::layout::{self, Layout};
use crate::stacks::blueprint;
use crate::zones::glob_matches;
use crate::{change, config, exec, guard, log};

/// The first check.
const FAILING_TEST: &str = "a failing test turns G2 red";

/// The second check.
const PROTECTED: &str = "editing a protected file is blocked (Path Guard and CI)";

/// The third check, when the project has a rollback in pre.
const ROLLBACK: &str = "rollback works in pre";

/// The third check, when the project has none.
const ROLLBACK_SIMULATED: &str = "rollback works in pre (simulated)";

/// The fourth check.
const ZERO_SCENARIOS: &str = "zero scenarios counts as a failure";

/// The table of the commands file whose failure the first check plants: a close test that plants
/// it in G2 reads the runs of the first check instead of running its own.
const SELFTEST: &str = "selftest";

/// Why a close test that reads the runs of the first check did not close when the first check
/// did not run its commands with its failure.
const FIRST_NOT_PLANTED: &str = "the first check could not plant its failure";

/// Why a gate without a close test is unproven.
const NO_CLOSE_TEST: &str = "the blueprint declares no close test for it";

/// The CI workflow, from the project root, in the new names: a project of the old layout keeps
/// it as `gdd.yml` (see [`crate::layout`]).
const WORKFLOW: &str = ".github/workflows/tminus.yml";

/// What the CI workflow must run, one of them in either layout: both names run this program.
/// `bin/tminus ci` and `bin/gdd ci`, which contain them, count too.
const RUNS_CI: [&str; 2] = ["tminus ci", "gdd ci"];

/// Where the file at the path of the failing test is kept while it is set aside, under its own
/// path, from the state root (see [`crate::state::root`]), in the new names.
const KEPT: &str = ".tminus/selftest/kept";

/// The variable the commands of the self-test get with the change, in the new names: a project
/// of the old layout gets it as it names it (see [`Layout::localize`]).
const CHANGE: &str = "TMINUS_CHANGE";

/// What the self-test reads from `tminus/commands.toml` besides the G2 commands.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    /// `[selftest] file`: where the failing test goes, from the project root.
    file: Option<String>,
    /// `[selftest] content`: the failing test.
    content: Option<String>,
    /// `[release] rollback_pre`: the command that rolls pre back.
    rollback_pre: Option<String>,
    /// `[release] simulated`: the release is simulated.
    simulated: bool,
}

impl Config {
    /// Reads the `[selftest]` and `[release]` sections of a commands file; the rest is ignored.
    /// As with commands, a value counts when it is text that is not blank; only
    /// `simulated = true` simulates the release.
    pub fn from_toml(text: &str) -> Result<Config, config::Error> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            selftest: BTreeMap<String, toml::Value>,
            #[serde(default)]
            release: BTreeMap<String, toml::Value>,
        }
        let raw: Raw = toml::from_str(text).map_err(|e| config::Error::Toml(e.to_string()))?;
        let simulated = raw.release.get("simulated") == Some(&toml::Value::Boolean(true));
        let mut selftest = configured(raw.selftest);
        Ok(Config {
            file: selftest.remove("file"),
            content: selftest.remove("content"),
            rollback_pre: configured(raw.release).remove("rollback_pre"),
            simulated,
        })
    }
}

/// One check of the self-test.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Check {
    /// What it checks, and why it failed when it says.
    pub test: String,
    /// Whether it passed.
    pub ok: bool,
}

/// What the self-test found: its checks, in order, and what it saw of each gate the blueprint
/// declares, in its order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selftest {
    /// The checks.
    pub checks: Vec<Check>,
    /// What it saw of each gate (see [`gate_closes`]).
    pub gates: Vec<Closed>,
}

impl Selftest {
    /// Whether every check passed and every gate whose close test ran closed.
    pub fn ok(&self) -> bool {
        let closed = self.gates.iter().all(|gate| gate.closed || !gate.tested);
        self.checks.iter().all(|check| check.ok) && closed
    }

    /// `harness <passed>/<checks>`.
    pub fn summary(&self) -> String {
        let passed = self.checks.iter().filter(|check| check.ok).count();
        format!("harness {passed}/{}", self.checks.len())
    }
}

impl fmt::Display for Selftest {
    /// `OK <test>` or `KO <test>`, a line each; then a line per gate, `OK <gate> closes: ...`,
    /// `KO <gate> does not close: ...` or `-- <gate> is unproven: ...`, with what the self-test
    /// said of it; then the summary, without a line break at the end.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for check in &self.checks {
            let mark = if check.ok { "OK" } else { "KO" };
            writeln!(f, "{mark} {}", check.test)?;
        }
        for gate in &self.gates {
            let (mark, seen) = match (gate.tested, gate.closed) {
                (true, true) => ("OK", "closes"),
                (true, false) => ("KO", "does not close"),
                (false, _) => ("--", "is unproven"),
            };
            writeln!(f, "{mark} {} {seen}: {}", gate.gate, gate.said)?;
        }
        f.write_str(&self.summary())
    }
}

/// What `.tminus/selftest.json` keeps.
#[derive(Serialize)]
struct Saved<'a> {
    ok: bool,
    summary: &'a str,
    detail: &'a [Check],
    gates: &'a [Closed],
    ts: &'a str,
}

/// Runs the four checks on `project`, whose commands file says `config`, then the close test of
/// each gate the blueprint declares, in its order (see [`gate_closes`]), each kept with the
/// gate's lock now (see [`Declared::lock`]). The commands run from the project root with
/// `TMINUS_CHANGE=selftest`, as the project names it (see [`CHANGE`]). Not being able to read,
/// set aside, write or give back a failing test or a planted failure, or to make the empty
/// folder, is an error; a file that a link keeps from going back fails the first check, or the
/// close test, instead (see [`failing_test_caught`]).
pub fn run(project: &Project, config: &Config) -> Result<Selftest, String> {
    let declared = blueprint::gates().map_err(|e| e.to_string())?;
    let rollback = match config.rollback_pre {
        Some(_) => ROLLBACK,
        None => ROLLBACK_SIMULATED,
    };
    let (first, first_ok, ran) = failing_test_caught(project, config)?;
    let checks = [
        (first, first_ok),
        (PROTECTED.to_string(), harness_protected(project)),
        (rollback.to_string(), rollback_works(project, config)),
        (ZERO_SCENARIOS.to_string(), zero_scenarios_fail(project)?),
    ];
    let checks = checks.map(|(test, ok)| Check { test, ok }).to_vec();
    let mut gates = Vec::new();
    for gate in declared.gates() {
        let (tested, closed, said) = gate_closes(project, &declared, gate, ran.as_ref())?;
        gates.push(Closed {
            gate: gate.id.to_string(),
            tested,
            closed,
            lock: declared.lock(gate, project.commands),
            said,
        });
    }
    Ok(Selftest { checks, gates })
}

/// What the self-test kept of each gate the last time it ran in the project whose state root is
/// `state` (see [`save`]): none when it never ran or kept none. A result that cannot be read, or
/// is not JSON, is an error that names it.
pub fn closes(state: &Path) -> Result<Vec<Closed>, String> {
    #[derive(Deserialize)]
    struct Kept {
        #[serde(default)]
        gates: Vec<Closed>,
    }
    let path = layout::named(state, approve::SELFTEST_PATH);
    let bytes = files::read(state, &path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let Some(bytes) = bytes else {
        return Ok(Vec::new());
    };
    let kept: Kept = serde_json::from_slice(&bytes).map_err(|e| format!("{path}: {e}"))?;
    Ok(kept.gates)
}

/// Saves `selftest` in `.tminus/selftest.json` under the state root `state`, where the project
/// keeps it (see [`crate::layout`]), as indented JSON with `ok`, `summary`, `detail`, `gates`
/// (see [`closes`]) and `ts`, and logs it: a machine step of G0.
pub fn save(state: &Path, selftest: &Selftest) -> Result<(), String> {
    let (summary, ts) = (selftest.summary(), log::now());
    let saved = Saved {
        ok: selftest.ok(),
        summary: &summary,
        detail: &selftest.checks,
        gates: &selftest.gates,
        ts: &ts,
    };
    let text = serde_json::to_string_pretty(&saved).map_err(|e| e.to_string())?;
    change::write(state, approve::SELFTEST_PATH, &format!("{text}\n"))?;
    let entry = log::Entry {
        change: "-".to_string(),
        phase: "0-setup".to_string(),
        gate: "G0".to_string(),
        role: "machine".to_string(),
        agent: "selftest".to_string(),
        result: if selftest.ok() { "ok" } else { "ko" }.to_string(),
        duration_s: 0.0,
        note: summary,
    };
    change::logged(state, &entry)
}

/// The first check: its name, whether it passed, and what the G2 commands did when they ran.
/// They run without and with the failing test of `config` (see [`planted_run`]), held to the
/// known debt of G2 as the gate holds them (see [`red`]): every one of them, even once one turned
/// red. The check passes when G2 was green without the test and red with it; when it was red
/// without it, the name says which commands turned it red. It fails without a test to plant or
/// G2 commands; when the known debt cannot be read, and then nothing runs; and when a link kept
/// the file from going back: then the name says where its copy is. What the commands did is
/// known only when they ran and the file went back.
fn failing_test_caught(
    project: &Project,
    config: &Config,
) -> Result<(String, bool, Option<Ran>), String> {
    let commands = project.commands.of(Gate::G2);
    let (Some(file), Some(content)) = (&config.file, &config.content) else {
        return Ok((FAILING_TEST.to_string(), false, None));
    };
    if commands.is_empty() {
        return Ok((FAILING_TEST.to_string(), false, None));
    }
    let owed = match debt::of_gate(project.root, Gate::G2) {
        Ok(owed) => owed,
        Err(why) => return Ok((format!("{FAILING_TEST} ({why})"), false, None)),
    };
    let ran = match planted_run(project, file, content, commands, &owed)? {
        Planted::Ran(ran) => ran,
        Planted::NotGivenBack(kept) => {
            let named = format!("{FAILING_TEST} ({file} was not given back: a copy is in {kept})");
            return Ok((named, false, None));
        }
    };
    if !ran.without.is_empty() {
        let failed = ran.without.join(", ");
        let named = format!("{FAILING_TEST} (G2 is red without it: {failed})");
        return Ok((named, false, Some(ran)));
    }
    let red = !ran.with.is_empty();
    Ok((FAILING_TEST.to_string(), red, Some(ran)))
}

/// What the commands of a planted failure did: the ones red without it and, when none was, the
/// ones red with it, by name, in their order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Ran {
    without: Vec<String>,
    with: Vec<String>,
}

/// How a planted failure ended: its commands ran, or a link kept the file at its path from going
/// back, and its copy stays where this says.
enum Planted {
    Ran(Ran),
    NotGivenBack(String),
}

/// The close test of `gate`, one of the gates `declared`, in `project`: whether it ran, whether
/// the gate closed, and what the self-test says of it. It does not run without a close test or
/// without a failure to plant in the table it names. Then the commands of its stage that the
/// gate claims, the optional ones left out, run without and with the failure, held to the known
/// debt of the stage (see [`planted_run`]); none is a gate that does not close. A close test that
/// plants the failure of the first check in G2 reads the runs of the first check, `first`,
/// instead: without them, the gate does not close. It closes when no command was red without the
/// failure and one of its own was red with it. A debt file that cannot be read, and a file a link
/// kept from going back, are a gate that does not close, and say why.
fn gate_closes(
    project: &Project,
    declared: &Declared,
    gate: &GateDecl,
    first: Option<&Ran>,
) -> Result<(bool, bool, String), String> {
    let not_closed = |said: String| Ok((true, false, said));
    let Some(close) = &gate.close else {
        return Ok((false, false, NO_CLOSE_TEST.to_string()));
    };
    let Some(plant) = project.commands.plant(&close.plant) else {
        let file = layout::named(project.root, COMMANDS);
        let said = format!("no failure to plant for it in [{}] of {file}", close.plant);
        return Ok((false, false, said));
    };
    let stage = close.stage;
    let claimed = |name: &str| declared.claim(name).id == gate.id;
    let ours = |name: &str| claimed(name) && declared.optional(name).is_none();
    let commands = project.commands.of(stage).iter();
    let commands = commands.filter(|(name, _)| ours(name));
    let commands = commands.map(|(name, text)| (name.clone(), text.clone()));
    let commands: BTreeMap<String, String> = commands.collect();
    if commands.is_empty() {
        return not_closed(format!("no command of it runs in {stage}"));
    }
    let ran = if close.plant == SELFTEST && stage == Gate::G2 {
        match first {
            Some(ran) => ran.clone(),
            None => return not_closed(FIRST_NOT_PLANTED.to_string()),
        }
    } else {
        let owed = match debt::of_gate(project.root, stage) {
            Ok(owed) => owed,
            Err(why) => return not_closed(why),
        };
        match planted_run(project, &plant.file, &plant.content, &commands, &owed)? {
            Planted::Ran(ran) => ran,
            Planted::NotGivenBack(kept) => {
                let file = &plant.file;
                return not_closed(format!("{file} was not given back: a copy is in {kept}"));
            }
        }
    };
    if !ran.without.is_empty() {
        let red = ran.without.join(", ");
        return not_closed(format!("{stage} is red without its planted failure: {red}"));
    }
    if !ran.with.iter().any(|name| commands.contains_key(name)) {
        return not_closed(format!("{stage} stayed green with its planted failure"));
    }
    let said = format!("its planted failure turns {stage} red");
    Ok((true, true, said))
}

/// Plants `content` at `file` of `project` to see `commands` turn red. The file there is set
/// aside (see [`set_aside`]) and the commands run, held to the known debt `owed` (see [`red`]);
/// then, when none was red, the failure is planted and they all run again (see
/// [`planted_red`]). The file is given back in every case (see [`give_back`]): its earlier bytes
/// and permissions, or no file when there was none. A link that kept it from going back is told
/// with where its copy is, before what went wrong with the failure; a refusal to set it aside, a
/// failure that cannot be planted and a file that cannot be given back are errors.
fn planted_run(
    project: &Project,
    file: &str,
    content: &str,
    commands: &BTreeMap<String, String>,
    owed: &BTreeMap<String, Option<u64>>,
) -> Result<Planted, String> {
    let (root, state) = (project.root, project.state);
    let kept = set_aside(root, state, file)?;
    let without = turning_red(commands, project, owed);
    let with = if without.is_empty() {
        planted_red(project, file, content, commands, owed)
    } else {
        Ok(Vec::new())
    };
    let given_back = give_back(root, state, file, kept.as_deref());
    if let Some(kept) = &kept {
        let given = given_back.as_ref();
        if !*given.map_err(|e| format!("cannot give {file} back: {e}: a copy is in {kept}"))? {
            return Ok(Planted::NotGivenBack(kept.clone()));
        }
    }
    let with = with.map_err(|e| format!("cannot write {file}: {e}"))?;
    given_back.map_err(|e| format!("cannot give {file} back: {e}"))?;
    Ok(Planted::Ran(Ran { without, with }))
}

/// Plants `content` at `file` of `project`, in place of whatever the commands left there (see
/// [`cleared`]), and runs every one of `commands` from its root, held to the known debt `owed`
/// (see [`red`]): the names of the ones that turned red. A failure that cannot be planted is an
/// error, and then nothing runs.
fn planted_red(
    project: &Project,
    file: &str,
    content: &str,
    commands: &BTreeMap<String, String>,
    owed: &BTreeMap<String, Option<u64>>,
) -> io::Result<Vec<String>> {
    let root = project.root;
    cleared(root, file)?;
    written(&root.join(file), content.as_bytes())?;
    Ok(turning_red(commands, project, owed))
}

/// Sets aside what was at `file` of the project at `root`: a copy of it, bytes and permissions,
/// is kept at its path under [`KEPT`] of the state root `state`, as the project names it, and it
/// goes. Where the copy is, from the state root, or `None` when there was no file. A symbolic
/// link on the way to the file or to its copy, a file with other names (a hard link) or a copy
/// already there is refused before anything is written: the self-test would write out of the
/// project, could not give the file back as it was, or would lose the copy an earlier run kept.
fn set_aside(root: &Path, state: &Path, file: &str) -> Result<Option<String>, String> {
    // Only the folder is named as the project names it: the path of the test stays as it is.
    let kept = format!("{}/{file}", layout::named(state, KEPT));
    for (base, path) in [(root, file), (state, kept.as_str())] {
        let link = first_link(base, path).map_err(|e| format!("cannot read {path}: {e}"))?;
        if let Some(link) = link {
            return Err(format!(
                "cannot set {file} aside: {} is a symbolic link, which the self-test does not \
                 write through: remove the link and run it again",
                link.display()
            ));
        }
    }
    let cannot_read = |e: io::Error| format!("cannot read {file}: {e}");
    let path = root.join(file);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == NotFound => return Ok(None),
        Err(e) => return Err(cannot_read(e)),
    };
    let meta = fs::metadata(&path).map_err(cannot_read)?;
    if meta.nlink() > 1 {
        return Err(format!(
            "cannot set {file} aside: it has other names (a hard link), which the self-test does \
             not write through: copy it to a file of its own and run it again"
        ));
    }
    let copied = written(&state.join(&kept), &bytes);
    let copied = copied.and_then(|copy| copy.set_permissions(meta.permissions()));
    copied.map_err(|e| not_kept(file, &kept, e))?;
    let aside = fs::remove_file(&path);
    aside.map_err(|e| format!("cannot set {file} aside: {e}"))?;
    Ok(Some(kept))
}

/// Why `file` could not be copied to `kept`, and what to do about it.
fn not_kept(file: &str, kept: &str, e: io::Error) -> String {
    match e.kind() {
        AlreadyExists => format!(
            "cannot set {file} aside: {kept} is already there, kept by a self-test that could \
             not give {file} back: once {file} is as it should be, remove the copy and run it \
             again"
        ),
        _ => format!("cannot keep a copy of {file} in {kept}: {e}"),
    }
}

/// Gives back what was at `file` of the project at `root` before the self-test, in place of
/// whatever the commands left there: the file whose copy is at `kept` of the state root
/// `state`, its bytes and permissions, and then the copy goes; or no file (see [`cleared`]).
/// Whether it was given back: not when a symbolic link is now on the way to the file, or to its
/// copy, itself included, which would take the self-test out of the project. Then nothing goes
/// through the link and the copy stays.
fn give_back(root: &Path, state: &Path, file: &str, kept: Option<&str>) -> io::Result<bool> {
    let Some(kept) = kept else {
        return cleared(root, file).map(|()| true);
    };
    if link_on_the_way(root, file)?.is_some() {
        return Ok(false);
    }
    removed(&root.join(file))?;
    if first_link(state, kept)?.is_some() {
        return Ok(false);
    }
    let copy = state.join(kept);
    let bytes = fs::read(&copy)?;
    let permissions = fs::metadata(&copy)?.permissions();
    let given = written(&root.join(file), &bytes)?;
    given.set_permissions(permissions)?;
    fs::remove_file(copy)?;
    Ok(true)
}

/// Removes what the commands left at `file` of the project at `root`: the planted failure, a
/// file or a link, which goes and is never followed. A symbolic link on the way to it is an
/// error, and then nothing is removed: through it, the self-test would write out of the project.
fn cleared(root: &Path, file: &str) -> io::Result<()> {
    match link_on_the_way(root, file)? {
        Some(link) => Err(io::Error::other(format!(
            "{} is a symbolic link, which the self-test does not write through",
            link.display()
        ))),
        None => removed(&root.join(file)),
    }
}

/// The first symbolic link on the way from `root` to its `file`, `file` itself left out: a link
/// there goes without being followed.
fn link_on_the_way(root: &Path, file: &str) -> io::Result<Option<PathBuf>> {
    let link = first_link(root, file)?;
    Ok(link.filter(|link| link != Path::new(file)))
}

/// The names of the `commands` of a stage that turn it red, run in `project` held to the known
/// debt `owed` of the stage (see [`red`]), in their order: every one of them runs.
fn turning_red(
    commands: &BTreeMap<String, String>,
    project: &Project,
    owed: &BTreeMap<String, Option<u64>>,
) -> Vec<String> {
    let turned = commands.iter().filter(|(n, c)| red(n, c, project, owed));
    turned.map(|(name, _)| name.clone()).collect()
}

/// Whether the command `name`, `command`, turns its stage red, run in `project` (see
/// [`failed`]): it fails, and it is not held to its known debt in `owed`, by name, the count
/// recorded for it when there is one, as the gate holds it (see [`debt::held`]).
fn red(name: &str, command: &str, project: &Project, owed: &BTreeMap<String, Option<u64>>) -> bool {
    let Some(output) = failed(command, project) else {
        return false;
    };
    let Some(&known) = owed.get(name) else {
        return true;
    };
    let texts = project.commands.count(name);
    debt::held(name, debt::Ceiling { known, texts }, &output).is_err()
}

/// Writes `bytes` as the new file at `path`, creating its folder: the file, still open.
/// Something already at `path`, even a link, is an error.
fn written(path: &Path, bytes: &[u8]) -> io::Result<File> {
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    let mut file = File::create_new(path)?;
    file.write_all(bytes)?;
    Ok(file)
}

/// Removes the file at `path`; when there is none already, there is nothing to do.
fn removed(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() == NotFound => Ok(()),
        other => other,
    }
}

/// What `command` printed, run from the root of `project` with `TMINUS_CHANGE=selftest`, as the
/// project names it, within the time limit of its rules (see [`exec::run_limited`]), when it
/// fails: it ends with a code other than 0, or it cannot run, and then it printed nothing. `None`
/// when it passes.
fn failed(command: &str, project: &Project) -> Option<String> {
    let change = Layout::at(project.root).localize(CHANGE);
    let env = [(change.as_str(), "selftest")];
    let limit = project.rules.command_limit;
    match exec::run_limited(command, project.root, &env, limit) {
        Ok(ran) if ran.ok() => None,
        Ok(ran) => Some(ran.output),
        Err(_) => Some(String::new()),
    }
}

/// Whether `command`, run in `project`, fails (see [`failed`]).
fn fails(command: &str, project: &Project) -> bool {
    failed(command, project).is_some()
}

/// Whether the harness of `project` covers the rules file and the CI workflow of its layout, and
/// the workflow runs `tminus ci` or `gdd ci` (see [`RUNS_CI`]). A workflow that cannot be read
/// runs nothing.
fn harness_protected(project: &Project) -> bool {
    let harness = &project.rules.harness;
    let protected = |path: &str| harness.iter().any(|p| glob_matches(p, path));
    let rules = layout::named(project.root, guard::RULES);
    let workflow = layout::named(project.root, WORKFLOW);
    let text = fs::read_to_string(project.root.join(&workflow));
    let runs_ci = text.is_ok_and(|text| RUNS_CI.iter().any(|ci| text.contains(ci)));
    protected(&rules) && protected(&workflow) && runs_ci
}

/// Whether the rollback in pre works: its command passes, in `project` (see [`fails`]); without
/// one, the release is simulated.
fn rollback_works(project: &Project, config: &Config) -> bool {
    match &config.rollback_pre {
        Some(command) => !fails(command, project),
        None => config.simulated,
    }
}

/// Whether zero scenarios fail G3 in `project`: G3 counts none in an empty folder, and the rules
/// keep zero scenarios a failure.
fn zero_scenarios_fail(project: &Project) -> Result<bool, String> {
    let folder = empty_folder()?;
    let counted = count_scenarios(&folder);
    let removed = fs::remove_dir(&folder);
    removed.map_err(|e| format!("cannot remove {}: {e}", folder.display()))?;
    Ok(matches!(counted, Ok(0)) && project.rules.scenarios_required)
}

/// A new, empty folder in the temporary folder of the system.
fn empty_folder() -> Result<PathBuf, String> {
    static MADE: AtomicUsize = AtomicUsize::new(0);
    let made = MADE.fetch_add(1, Ordering::Relaxed);
    let name = format!("gdd-selftest-{}-{made}", std::process::id());
    let folder = std::env::temp_dir().join(name);
    let created = fs::create_dir(&folder);
    created.map_err(|e| format!("cannot make {}: {e}", folder.display()))?;
    Ok(folder)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gates::{Commands, Rules};
    use crate::git::tests::Folder;
    use crate::guard::Settings;
    use crate::zones::ZoneMap;

    const RULES: &str = "[protected]\nall = [\"tminus/**\", \".github/**\"]\n";

    const COMMANDS: &str = "[g2]\nunit = 'test ! -f tests/failing.txt'\n\
                            [selftest]\nfile = 'tests/failing.txt'\ncontent = 'fails'\n\
                            [release]\nsimulated = true\nrollback_pre = ''\n";

    /// The G2 command of `COMMANDS`.
    const UNIT: &str = "unit = 'test ! -f tests/failing.txt'";

    /// A CI workflow that runs the gates.
    const RUNS_THE_GATES: &str = "steps:\n  - run: tminus ci\n";

    /// A project with a CI workflow that runs the gates, and what the self-test reads from it.
    struct Harness {
        folder: Folder,
        zones: ZoneMap,
        rules: Rules,
        settings: Settings,
        commands: Commands,
        config: Config,
    }

    impl Harness {
        fn new(commands: &str) -> Harness {
            let folder = Folder::new();
            folder.write(WORKFLOW, RUNS_THE_GATES);
            Harness {
                folder,
                zones: ZoneMap::from_toml("").unwrap(),
                rules: Rules::from_toml(RULES).unwrap(),
                settings: Settings::from_toml("").unwrap(),
                commands: Commands::from_toml(commands).unwrap(),
                config: Config::from_toml(commands).unwrap(),
            }
        }

        /// The harness of `COMMANDS` with `unit` as its G2 command.
        fn with_unit(unit: &str) -> Harness {
            Harness::new(&COMMANDS.replace(UNIT, &format!("unit = '{unit}'")))
        }

        /// What the self-test reads of this project, whose state root is `state`.
        fn project<'a>(&'a self, state: &'a Path) -> Project<'a> {
            Project {
                root: &self.folder.0,
                state,
                zones: &self.zones,
                rules: &self.rules,
                settings: &self.settings,
                commands: &self.commands,
            }
        }

        fn run(&self) -> Result<Selftest, String> {
            run(&self.project(&self.folder.0), &self.config)
        }

        /// Whether each check passed, in order.
        fn passed(&self) -> Vec<bool> {
            let checks = self.run().unwrap().checks;
            checks.iter().map(|check| check.ok).collect()
        }

        /// The third check: its name, one of the rollback checks, and whether it passed.
        fn rollback(&self) -> (&'static str, bool) {
            let check = self.run().unwrap().checks.swap_remove(2);
            let mut names = [ROLLBACK, ROLLBACK_SIMULATED].into_iter();
            let name = names.find(|name| check.test == *name);
            (name.unwrap_or("another check"), check.ok)
        }

        fn planted(&self) -> PathBuf {
            self.folder.0.join("tests/failing.txt")
        }

        /// Where the file at the path of the test is kept while it is set aside.
        fn kept(&self) -> PathBuf {
            self.folder
                .0
                .join(".tminus/selftest/kept/tests/failing.txt")
        }
    }

    /// The harness of `COMMANDS` with `unit` as its G2 command and `lint` too, counted by
    /// `Found {n} error`, with known debt in G2 for `lint`, with `count` recorded when there is
    /// one.
    fn owing_lint(lint: &str, unit: &str, count: Option<u64>) -> Harness {
        let both = format!("lint = '{lint}'\nunit = '{unit}'");
        let commands = COMMANDS.replace(UNIT, &both);
        let counted = format!("{commands}[count]\nlint = ['Found {{n}} error']\n");
        let harness = Harness::new(&counted);
        harness.folder.write(debt::PATH, &known("lint", count));
        harness
    }

    /// The file of the known debt with debt in G2 for `command`, with `count` recorded when
    /// there is one.
    fn known(command: &str, count: Option<u64>) -> String {
        known_in(command, Gate::G2, count)
    }

    /// The file of the known debt with debt in `gate` for `command`, with `count` recorded when
    /// there is one.
    fn known_in(command: &str, gate: Gate, count: Option<u64>) -> String {
        let owed = debt::Debt {
            command: command.to_string(),
            gate,
            since: "2026-10-30".to_string(),
            count,
        };
        let known = debt::Known {
            adopted: "2026-10-30".to_string(),
            debt: vec![owed],
        };
        known.to_toml()
    }

    /// The name of the first check of the self-test of `harness`, and whether it passed.
    fn first(harness: &Harness) -> (String, bool) {
        let check = harness.run().unwrap().checks.swap_remove(0);
        (check.test, check.ok)
    }

    /// The bytes and the permissions of the file at `path`.
    fn bytes_and_mode(path: &Path) -> (String, u32) {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(path).unwrap().permissions().mode() & 0o777;
        (fs::read_to_string(path).unwrap(), mode)
    }

    #[test]
    fn a_sound_harness_passes_its_four_checks_and_leaves_no_test_behind() {
        let harness = Harness::new(COMMANDS);
        let selftest = harness.run().unwrap();
        assert!(selftest.ok());
        assert_eq!(selftest.summary(), "harness 4/4");
        let said = "OK a failing test turns G2 red\n\
                    OK editing a protected file is blocked (Path Guard and CI)\n\
                    OK rollback works in pre (simulated)\n\
                    OK zero scenarios counts as a failure\n\
                    OK works closes: its planted failure turns G2 red\n\
                    -- quality is unproven: no failure to plant for it in [close_test.quality] of \
                    tminus/commands.toml\n\
                    -- security is unproven: the blueprint declares no close test for it\n\
                    harness 4/4";
        assert_eq!(selftest.to_string(), said);
        assert!(!harness.planted().exists());
    }

    #[test]
    fn a_g2_that_misses_the_failing_test_fails_the_first_check() {
        let harness = Harness::with_unit("true");
        let selftest = harness.run().unwrap();
        assert!(!selftest.ok());
        assert_eq!(selftest.summary(), "harness 3/4");
        let said = selftest.to_string();
        let first = "KO a failing test turns G2 red\n";
        assert!(said.starts_with(first), "{said}");
        assert!(!harness.planted().exists());
    }

    #[test]
    fn without_a_test_to_plant_or_g2_commands_the_first_check_fails() {
        for missing in ["file = 'tests/failing.txt'", "content = 'fails'", UNIT] {
            let harness = Harness::new(&COMMANDS.replace(missing, ""));
            assert_eq!(harness.passed(), [false, true, true, true], "{missing}");
            assert!(!harness.folder.0.join("tests").exists(), "{missing}");
        }
        let blank = Harness::new(&COMMANDS.replace("'fails'", "'  '"));
        assert!(!blank.passed()[0]);
    }

    #[test]
    fn the_test_goes_back_to_what_was_there() {
        use std::os::unix::fs::PermissionsExt;
        // Its copy, kept while it is set aside, goes once it is back.
        let kept = ".tminus/selftest/kept/tests/failing.txt";
        let copies = format!("test ! -f tests/failing.txt || cp -p {kept} c");
        let copy = format!("copy = '{copies}'\n{UNIT}");
        let harness = Harness::new(&COMMANDS.replace(UNIT, &copy));
        harness.folder.write("tests/failing.txt", "mine\n");
        let runnable = fs::Permissions::from_mode(0o751);
        fs::set_permissions(harness.planted(), runnable).unwrap();
        assert!(harness.run().unwrap().ok());
        let mine = ("mine\n".to_string(), 0o751);
        assert_eq!(bytes_and_mode(&harness.planted()), mine);
        assert_eq!(bytes_and_mode(&harness.folder.0.join("c")), mine);
        assert!(!harness.kept().exists());
        // A command that removed the test leaves nothing to give back.
        let removes = "test ! -f tests/failing.txt || { rm tests/failing.txt; false; }";
        let harness = Harness::with_unit(removes);
        assert!(harness.run().unwrap().ok());
        assert!(!harness.planted().exists());
    }

    #[test]
    fn the_test_is_kept_aside_under_the_state_root() {
        let state = Folder::new();
        let copy = state.0.join(".tminus/selftest/kept/tests/failing.txt");
        // Green only while the copy is there, under the state root, and the test is not.
        let unit = format!("test ! -f tests/failing.txt && test -f {}", copy.display());
        let harness = Harness::with_unit(&unit);
        harness.folder.write("tests/failing.txt", "mine\n");
        let selftest = run(&harness.project(&state.0), &harness.config).unwrap();
        assert!(selftest.ok(), "{selftest}");
        assert_eq!(fs::read_to_string(harness.planted()).unwrap(), "mine\n");
        assert!(!copy.exists());
        assert!(!harness.folder.0.join(".tminus").exists());
    }

    #[test]
    fn a_g2_already_red_without_the_test_fails_the_first_check_and_plants_nothing() {
        let red = "lint = 'test ! -f tests/failing.txt || touch planted; false'\n\
                   types = 'exit 2'\n";
        let harness = Harness::new(&COMMANDS.replace(UNIT, &format!("{red}{UNIT}")));
        harness.folder.write("tests/failing.txt", "mine\n");
        let selftest = harness.run().unwrap();
        let first = &selftest.checks[0];
        let named = "a failing test turns G2 red (G2 is red without it: lint, types)";
        assert_eq!((first.test.as_str(), first.ok), (named, false));
        let shown = selftest.to_string();
        assert!(shown.starts_with(&format!("KO {named}\n")), "{shown}");
        assert!(!harness.folder.0.join("planted").exists());
        let mine = fs::read_to_string(harness.planted()).unwrap();
        assert_eq!(mine, "mine\n");
        // Without the file there, there is nothing to give back.
        fs::remove_file(harness.planted()).unwrap();
        assert!(!harness.run().unwrap().checks[0].ok);
        assert!(!harness.planted().exists());
    }

    #[test]
    fn a_g2_command_held_to_its_known_debt_leaves_g2_green() {
        let unit = "test ! -f tests/failing.txt";
        for (lint, count) in [
            ("echo Found 5 errors.; false", Some(5)),
            ("echo Found 3 errors.; false", Some(5)),
            ("echo panicked; false", None),
        ] {
            let harness = owing_lint(lint, unit, count);
            let selftest = harness.run().unwrap();
            assert!(selftest.ok(), "{lint}: {selftest}");
            assert!(!harness.planted().exists(), "{lint}");
        }
    }

    #[test]
    fn a_g2_command_above_its_known_debt_or_without_it_turns_g2_red() {
        let unit = "test ! -f tests/failing.txt";
        let named = "a failing test turns G2 red (G2 is red without it: lint)".to_string();
        // More than its count, or nothing tminus can count.
        for lint in ["echo Found 7 errors.; false", "echo panicked; false"] {
            let harness = owing_lint(lint, unit, Some(5));
            assert_eq!(first(&harness), (named.clone(), false), "{lint}");
        }
        // Debt of another command does not hold it.
        let harness = owing_lint("echo Found 5 errors.; false", unit, Some(5));
        harness.folder.write(debt::PATH, &known("types", Some(5)));
        assert_eq!(first(&harness), (named, false));
    }

    #[test]
    fn with_the_test_a_command_held_to_its_known_debt_must_report_more() {
        // Held to its debt, lint misses the test, and nothing else catches it.
        let harness = owing_lint("echo Found 5 errors.; false", "true", Some(5));
        assert_eq!(first(&harness), (FAILING_TEST.to_string(), false));
        // A count the test raises turns G2 red.
        let rises = "test -f tests/failing.txt && echo Found 6 errors. || \
                     echo Found 5 errors.; false";
        let harness = owing_lint(rises, "true", Some(5));
        assert_eq!(first(&harness), (FAILING_TEST.to_string(), true));
        assert!(!harness.planted().exists());
    }

    #[test]
    fn known_debt_that_cannot_be_read_fails_the_first_check_and_nothing_runs() {
        let harness = Harness::with_unit("echo ran >> runs; test ! -f tests/failing.txt");
        harness.folder.write("tests/failing.txt", "mine\n");
        harness.folder.write(debt::PATH, "[[debt]");
        let (named, ok) = first(&harness);
        let start = "a failing test turns G2 red (tminus/debt.toml: invalid TOML: ";
        assert!(named.starts_with(start) && named.ends_with(')'), "{named}");
        assert!(!ok);
        assert!(!harness.folder.0.join("runs").exists());
        assert_eq!(fs::read_to_string(harness.planted()).unwrap(), "mine\n");
        assert!(!harness.kept().exists());
    }

    #[test]
    fn a_test_that_cannot_be_read_or_given_back_is_an_error() {
        let harness = Harness::new(COMMANDS);
        fs::create_dir_all(harness.planted()).unwrap();
        let err = harness.run().unwrap_err();
        let start = "cannot read tests/failing.txt: ";
        assert!(err.starts_with(start), "{err}");
        let swapped = "test ! -f tests/failing.txt || \
                       { rm tests/failing.txt && mkdir tests/failing.txt && false; }";
        let err = Harness::with_unit(swapped).run().unwrap_err();
        let start = "cannot give tests/failing.txt back: ";
        assert!(err.starts_with(start), "{err}");
        // With a file to give back, the error says where its copy stays.
        let harness = Harness::with_unit(swapped);
        harness.folder.write("tests/failing.txt", "mine\n");
        let err = harness.run().unwrap_err();
        let end = ": a copy is in .tminus/selftest/kept/tests/failing.txt";
        assert!(err.starts_with(start) && err.ends_with(end), "{err}");
        assert_eq!(fs::read_to_string(harness.kept()).unwrap(), "mine\n");
        // A path under a file is not a missing one.
        let harness = Harness::new(COMMANDS);
        harness.folder.write("tests", "a file\n");
        let err = harness.run().unwrap_err();
        let start = "cannot read tests/failing.txt: ";
        assert!(err.starts_with(start), "{err}");
    }

    #[test]
    fn a_test_that_cannot_be_planted_is_an_error_and_nothing_runs_with_it() {
        let harness = Harness::with_unit("echo ran >> runs; mkdir -p tests/failing.txt");
        let err = harness.run().unwrap_err();
        let start = "cannot write tests/failing.txt: ";
        assert!(err.starts_with(start), "{err}");
        let runs = fs::read_to_string(harness.folder.0.join("runs")).unwrap();
        assert_eq!(runs, "ran\n");
    }

    #[test]
    fn the_self_test_never_writes_through_a_link() {
        let elsewhere = Folder::new();
        elsewhere.write("failing.txt", "theirs\n");
        let outside = elsewhere.0.join("failing.txt");
        let harness = Harness::with_unit("echo ran >> runs; test ! -f tests/failing.txt");
        fs::create_dir_all(harness.folder.0.join("tests")).unwrap();
        std::os::unix::fs::symlink(&outside, harness.planted()).unwrap();
        let err = harness.run().unwrap_err();
        let why = "cannot set tests/failing.txt aside: tests/failing.txt is a symbolic link, \
                   which the self-test does not write through: remove the link and run it again";
        assert_eq!(err, why);
        fs::remove_file(harness.planted()).unwrap();
        fs::hard_link(&outside, harness.planted()).unwrap();
        let err = harness.run().unwrap_err();
        let why = "cannot set tests/failing.txt aside: it has other names (a hard link), which \
                   the self-test does not write through: copy it to a file of its own and run it \
                   again";
        assert_eq!(err, why);
        assert_eq!(fs::read_to_string(&outside).unwrap(), "theirs\n");
        assert_eq!(fs::read_to_string(harness.planted()).unwrap(), "theirs\n");
        assert!(!harness.folder.0.join("runs").exists());
        // A folder on the way that is a link counts too.
        let harness = Harness::new(COMMANDS);
        std::os::unix::fs::symlink(&elsewhere.0, harness.folder.0.join("tests")).unwrap();
        let err = harness.run().unwrap_err();
        let start = "cannot set tests/failing.txt aside: tests is a symbolic link";
        assert!(err.starts_with(start), "{err}");
        assert_eq!(fs::read_dir(&elsewhere.0).unwrap().count(), 1);
        // So does one on the way to the copy of the file.
        let harness = Harness::with_unit("echo ran >> runs; test ! -f tests/failing.txt");
        harness.folder.write("tests/failing.txt", "mine\n");
        fs::create_dir_all(harness.folder.0.join(".tminus/selftest")).unwrap();
        let kept = harness.folder.0.join(".tminus/selftest/kept");
        std::os::unix::fs::symlink(&elsewhere.0, kept).unwrap();
        let err = harness.run().unwrap_err();
        let start = "cannot set tests/failing.txt aside: .tminus/selftest/kept is a symbolic link";
        assert!(err.starts_with(start), "{err}");
        assert_eq!(fs::read_dir(&elsewhere.0).unwrap().count(), 1);
        assert_eq!(fs::read_to_string(harness.planted()).unwrap(), "mine\n");
        assert!(!harness.folder.0.join("runs").exists());
    }

    #[test]
    fn a_link_the_g2_commands_leave_at_the_test_is_removed_and_never_followed() {
        use std::os::unix::fs::PermissionsExt;
        let elsewhere = Folder::new();
        elsewhere.write("failing.txt", "theirs\n");
        let outside = elsewhere.0.join("failing.txt");
        let to = outside.display();
        for ln in ["ln -s", "ln"] {
            // Left by G2 without the test: the test is planted in its place.
            let leaves = format!(
                "grep -q fails tests/failing.txt && exit 1; mkdir -p tests; \
                 {ln} {to} tests/failing.txt"
            );
            let harness = Harness::with_unit(&leaves);
            assert_eq!(harness.passed(), [true, true, true, true], "{ln}");
            assert!(fs::symlink_metadata(harness.planted()).is_err(), "{ln}");
            // Left by G2 with the test: the file is given back in its place.
            let leaves = format!(
                "test ! -f tests/failing.txt || \
                 {{ rm tests/failing.txt; {ln} {to} tests/failing.txt; false; }}"
            );
            let harness = Harness::with_unit(&leaves);
            harness.folder.write("tests/failing.txt", "mine\n");
            let mode = fs::Permissions::from_mode(0o640);
            fs::set_permissions(harness.planted(), mode).unwrap();
            assert!(harness.run().unwrap().ok(), "{ln}");
            let back = fs::symlink_metadata(harness.planted()).unwrap();
            assert!(back.is_file(), "{ln}");
            let mode = back.permissions().mode() & 0o777;
            assert_eq!((back.nlink(), mode), (1, 0o640), "{ln}");
            let mine = fs::read_to_string(harness.planted()).unwrap();
            assert_eq!(mine, "mine\n", "{ln}");
        }
        assert_eq!(fs::read_to_string(&outside).unwrap(), "theirs\n");
        assert_eq!(fs::read_dir(&elsewhere.0).unwrap().count(), 1);
    }

    #[test]
    fn a_link_the_g2_commands_leave_on_the_way_stops_the_self_test() {
        let elsewhere = Folder::new();
        // Left by G2 without the test: nothing is planted.
        let leaves = format!(
            "test -e tests || ln -s {} tests; test ! -f tests/failing.txt",
            elsewhere.0.display()
        );
        let err = Harness::with_unit(&leaves).run().unwrap_err();
        let why = "cannot write tests/failing.txt: tests is a symbolic link, which the self-test \
                   does not write through";
        assert_eq!(err, why);
        assert_eq!(fs::read_dir(&elsewhere.0).unwrap().count(), 0);
        // Without a file there, nothing is removed through it either.
        let leaves = format!(
            "test ! -f tests/failing.txt || {{ mv tests moved; ln -s {} tests; false; }}",
            elsewhere.0.display()
        );
        let err = Harness::with_unit(&leaves).run().unwrap_err();
        let why = "cannot give tests/failing.txt back: tests is a symbolic link, which the \
                   self-test does not write through";
        assert_eq!(err, why);
        assert_eq!(fs::read_dir(&elsewhere.0).unwrap().count(), 0);
    }

    #[test]
    fn a_file_a_link_keeps_from_going_back_stays_in_its_copy() {
        use std::os::unix::fs::PermissionsExt;
        let elsewhere = Folder::new();
        let not_back = "a failing test turns G2 red (tests/failing.txt was not given back: a copy \
                        is in .tminus/selftest/kept/tests/failing.txt)";
        // A link left by G2 with the test on the way to the file, or to its copy.
        let ways = [
            ("tests", ".tminus/selftest/kept/tests/failing.txt"),
            (".tminus/selftest/kept", "moved/tests/failing.txt"),
        ];
        for (way, copy) in ways {
            let leaves = format!(
                "test ! -f tests/failing.txt || {{ mv {way} moved; ln -s {} {way}; false; }}",
                elsewhere.0.display()
            );
            let harness = Harness::with_unit(&leaves);
            harness.folder.write("tests/failing.txt", "mine\n");
            let mode = fs::Permissions::from_mode(0o640);
            fs::set_permissions(harness.planted(), mode).unwrap();
            let selftest = harness.run().unwrap();
            let first = &selftest.checks[0];
            assert_eq!((first.test.as_str(), first.ok), (not_back, false), "{way}");
            // The gate that plants the same failure cannot see whether it closes.
            let works = &selftest.gates[0];
            let seen = (works.tested, works.closed, works.said.as_str());
            assert_eq!(seen, (true, false, FIRST_NOT_PLANTED), "{way}");
            let shown = selftest.to_string();
            assert!(shown.starts_with(&format!("KO {not_back}\n")), "{shown}");
            let copy = bytes_and_mode(&harness.folder.0.join(copy));
            assert_eq!(copy, ("mine\n".to_string(), 0o640), "{way}");
            assert!(!harness.planted().exists(), "{way}");
            assert_eq!(fs::read_dir(&elsewhere.0).unwrap().count(), 0, "{way}");
        }
    }

    #[test]
    fn a_copy_an_earlier_self_test_kept_is_never_overwritten() {
        let harness = Harness::with_unit("echo ran >> runs; test ! -f tests/failing.txt");
        harness.folder.write("tests/failing.txt", "mine\n");
        let earlier = ".tminus/selftest/kept/tests/failing.txt";
        harness.folder.write(earlier, "earlier\n");
        let err = harness.run().unwrap_err();
        let why = "cannot set tests/failing.txt aside: .tminus/selftest/kept/tests/failing.txt is \
                   already there, kept by a self-test that could not give tests/failing.txt \
                   back: once tests/failing.txt is as it should be, remove the copy and run it \
                   again";
        assert_eq!(err, why);
        assert_eq!(fs::read_to_string(harness.kept()).unwrap(), "earlier\n");
        assert_eq!(fs::read_to_string(harness.planted()).unwrap(), "mine\n");
        assert!(!harness.folder.0.join("runs").exists());
    }

    #[test]
    fn the_harness_must_cover_the_rules_and_a_ci_that_runs_the_gates() {
        let mut harness = Harness::new(COMMANDS);
        // The rules of a project of the new layout are in tminus/.
        for all in [
            "[]",
            "[\"tminus/**\"]",
            "[\".github/**\"]",
            "[\"gdd/**\", \".github/**\"]",
        ] {
            let rules = format!("[protected]\nall = {all}\n");
            harness.rules = Rules::from_toml(&rules).unwrap();
            assert_eq!(harness.passed(), [true, false, true, true], "{all}");
        }
        harness.rules = Rules::from_toml(RULES).unwrap();
        let workflow = harness.folder.0.join(WORKFLOW);
        fs::write(&workflow, "steps:\n  - run: cargo test\n").unwrap();
        assert_eq!(harness.passed(), [true, false, true, true]);
        fs::remove_file(&workflow).unwrap();
        assert_eq!(harness.passed(), [true, false, true, true]);
        fs::create_dir_all(&workflow).unwrap();
        assert_eq!(harness.passed(), [true, false, true, true]);
    }

    #[test]
    fn a_ci_that_runs_tminus_ci_or_gdd_ci_or_their_bin_entry_runs_the_gates() {
        let harness = Harness::new(COMMANDS);
        let workflow = harness.folder.0.join(WORKFLOW);
        assert_eq!(WORKFLOW, ".github/workflows/tminus.yml");
        for (runs, passes) in [
            ("tminus ci", true),
            ("bin/tminus ci", true),
            ("gdd ci", true),
            ("bin/gdd ci", true),
            ("echo tminus", false),
            ("echo gdd", false),
            ("tminus selftest", false),
            ("gdd selftest", false),
        ] {
            fs::write(&workflow, format!("steps:\n  - run: {runs}\n")).unwrap();
            assert_eq!(harness.passed()[1], passes, "{runs}");
        }
    }

    /// The harness of `COMMANDS` in a project set up before the rename: its rules protect `gdd/`
    /// and its workflow, `gdd.yml`, runs `runs`.
    fn old_harness(runs: &str) -> Harness {
        let mut harness = Harness::new(COMMANDS);
        fs::remove_file(harness.folder.0.join(WORKFLOW)).unwrap();
        harness.folder.write("gdd/project.toml", "");
        let workflow = format!("steps:\n  - run: {runs}\n");
        harness.folder.write(".github/workflows/gdd.yml", &workflow);
        let rules = "[protected]\nall = [\"gdd/**\", \".github/**\"]\n";
        harness.rules = Rules::from_toml(rules).unwrap();
        harness
    }

    #[test]
    fn a_project_set_up_before_the_rename_is_tested_in_its_own_layout() {
        for runs in ["gdd ci", "bin/gdd ci", "tminus ci"] {
            assert_eq!(old_harness(runs).passed(), [true; 4], "{runs}");
        }
        assert!(!old_harness("echo gdd").passed()[1]);
        // Its rules are in gdd/: protecting tminus/ does not cover them.
        let mut harness = old_harness("gdd ci");
        let rules = "[protected]\nall = [\"tminus/**\", \".github/**\"]\n";
        harness.rules = Rules::from_toml(rules).unwrap();
        assert!(!harness.passed()[1]);
        // The file of the test is kept aside in .gdd, and the commands get GDD_CHANGE alone.
        let mut harness = old_harness("gdd ci");
        let copy = ".gdd/selftest/kept/tests/failing.txt";
        let seen = "printf \"%s|%s\" \"$GDD_CHANGE\" \"${TMINUS_CHANGE-none}\" > seen";
        let unit = format!("{seen}; test ! -f tests/failing.txt && test -f {copy}");
        let commands = COMMANDS.replace(UNIT, &format!("unit = '{unit}'"));
        harness.commands = Commands::from_toml(&commands).unwrap();
        harness.folder.write("tests/failing.txt", "mine\n");
        let selftest = harness.run().unwrap();
        assert!(selftest.ok(), "{selftest}");
        let seen = fs::read_to_string(harness.folder.0.join("seen")).unwrap();
        assert_eq!(seen, "selftest|none");
        assert!(!harness.folder.0.join(copy).exists());
        // Its commands file is the one in gdd/.
        let quality = "no failure to plant for it in [close_test.quality] of gdd/commands.toml";
        assert_eq!(selftest.gates[1].said, quality);
        save(&harness.folder.0, &selftest).unwrap();
        assert!(harness.folder.0.join(".gdd/selftest.json").is_file());
        assert_eq!(closes(&harness.folder.0), Ok(selftest.gates));
        assert!(!harness.folder.0.join(".tminus").exists());
    }

    #[test]
    fn the_rollback_in_pre_passes_or_the_release_is_simulated() {
        let with_rollback = |command: &str| {
            let empty = "rollback_pre = ''";
            let commands = COMMANDS.replace(empty, &format!("rollback_pre = '{command}'"));
            Harness::new(&commands)
        };
        let ours = with_rollback("test \"$TMINUS_CHANGE\" = selftest");
        assert_eq!(ours.rollback(), (ROLLBACK, true));
        assert_eq!(with_rollback("exit 1").rollback(), (ROLLBACK, false));
        let harness = with_rollback("test -f gdd.flag");
        assert_eq!(harness.rollback(), (ROLLBACK, false));
        harness.folder.write("gdd.flag", "");
        assert_eq!(harness.rollback(), (ROLLBACK, true), "from the root");
        let simulated = Harness::new(COMMANDS);
        assert_eq!(simulated.rollback(), (ROLLBACK_SIMULATED, true));
        for not in ["simulated = false", "simulated = 'yes'"] {
            let real = Harness::new(&COMMANDS.replace("simulated = true", not));
            assert_eq!(real.rollback(), (ROLLBACK_SIMULATED, false), "{not}");
        }
    }

    #[test]
    fn zero_scenarios_must_stay_a_failure() {
        let mut harness = Harness::new(COMMANDS);
        let optional = format!("{RULES}[acceptance]\nrequired = false\n");
        harness.rules = Rules::from_toml(&optional).unwrap();
        assert_eq!(harness.passed(), [true, true, true, false]);
    }

    #[test]
    fn the_commands_file_must_be_valid() {
        for wrong in ["[selftest", "selftest = 1", "[release]\nx = "] {
            let result = Config::from_toml(wrong);
            assert!(matches!(result, Err(config::Error::Toml(_))), "{wrong:?}");
        }
        assert_eq!(Config::from_toml("").unwrap(), Config::default());
    }

    #[test]
    fn the_result_is_saved_for_the_status_and_logged() {
        let harness = Harness::new(COMMANDS);
        let root = &harness.folder.0;
        let mut selftest = harness.run().unwrap();
        selftest.checks[3].ok = false;
        save(root, &selftest).unwrap();
        let path = root.join(approve::SELFTEST_PATH);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("{\n  \"ok\": false,\n"), "{text}");
        assert!(text.ends_with("}\n"), "{text}");
        let saved: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(saved["summary"], "harness 3/4");
        let zero = serde_json::json!({"test": ZERO_SCENARIOS, "ok": false});
        assert_eq!(saved["detail"][3], zero);
        // What it saw of the gates goes after the checks.
        let gates = serde_json::to_value(&selftest.gates).unwrap();
        assert_eq!(saved["gates"], gates);
        assert_eq!(saved["gates"][0]["gate"], "works");
        let at = |key: &str| text.find(&format!("\n  \"{key}\":")).unwrap();
        let order = [at("detail"), at("gates"), at("ts")];
        assert!(order.is_sorted(), "{text}");
        let ts = saved["ts"].as_str().unwrap();
        assert!(ts.ends_with("+00:00"), "{saved}");
        let lines = || {
            let text = fs::read_to_string(root.join(log::PATH)).unwrap();
            log::parse(&text).unwrap()
        };
        for (field, value) in [
            ("change", "-"),
            ("phase", "0-setup"),
            ("gate", "G0"),
            ("role", "machine"),
            ("agent", "selftest"),
            ("result", "ko"),
            ("note", "harness 3/4"),
        ] {
            assert_eq!(lines()[0].text(field), value, "{field}");
        }
        selftest.checks[3].ok = true;
        save(root, &selftest).unwrap();
        assert_eq!(lines()[1].text("result"), "ok");
        fs::remove_file(&path).unwrap();
        fs::create_dir_all(&path).unwrap();
        let err = save(root, &selftest).unwrap_err();
        let start = "cannot write .tminus/selftest.json: ";
        assert!(err.starts_with(start), "{err}");
    }

    /// A G2 command of quality, green until something is at `src/bad.txt`.
    const LINT: &str = "lint = 'test ! -f src/bad.txt'";

    /// The harness of `COMMANDS` with `g2` as its G2 commands, and the failure quality plants at
    /// `src/bad.txt`.
    fn with_quality(g2: &str) -> Harness {
        let planted = "[close_test.quality]\nfile = 'src/bad.txt'\ncontent = 'bad'\n";
        Harness::new(&format!("{}{planted}", COMMANDS.replace(UNIT, g2)))
    }

    /// Whether the self-test of `harness` ran the close test of the gate `id`, whether the gate
    /// closed, and what it said of it.
    fn closed(harness: &Harness, id: &str) -> (bool, bool, String) {
        let gates = harness.run().unwrap().gates;
        let found = gates.into_iter().find(|gate| gate.gate == id);
        let gate = found.unwrap_or_else(|| panic!("no gate {id}"));
        (gate.tested, gate.closed, gate.said)
    }

    /// A gate that did not close, saying `said`.
    fn open(said: &str) -> (bool, bool, String) {
        (true, false, said.to_string())
    }

    /// A gate that closed, saying `said`.
    fn shut(said: &str) -> (bool, bool, String) {
        (true, true, said.to_string())
    }

    /// A gate whose close test did not run, saying `said`.
    fn unproven(said: &str) -> (bool, bool, String) {
        (false, false, said.to_string())
    }

    /// What the self-test saw of `gate`: whether its close test ran, whether it closed, and what
    /// it said.
    fn as_seen(gate: &Closed) -> (bool, bool, String) {
        (gate.tested, gate.closed, gate.said.clone())
    }

    #[test]
    fn a_gate_closes_when_its_planted_failure_turns_one_of_its_commands_red() {
        let harness = with_quality(&format!("{LINT}\n{UNIT}"));
        let selftest = harness.run().unwrap();
        assert!(selftest.ok(), "{selftest}");
        let said = "its planted failure turns G2 red";
        let quality = &selftest.gates[1];
        assert_eq!(quality.gate, "quality");
        assert_eq!(as_seen(quality), shut(said));
        let shown = selftest.to_string();
        let line = format!("\nOK quality closes: {said}\n");
        assert!(shown.contains(&line), "{shown}");
        // Each gate is kept with its lock now, in the order of the blueprint.
        let declared = blueprint::gates().unwrap();
        for (gate, seen) in declared.gates().iter().zip(&selftest.gates) {
            assert_eq!(seen.gate, gate.id.as_str());
            let lock = declared.lock(gate, &harness.commands);
            assert_eq!(seen.lock, lock, "{}", seen.gate);
        }
        assert_eq!(selftest.gates.len(), 3);
        assert!(!harness.folder.0.join("src/bad.txt").exists());
    }

    #[test]
    fn a_gate_that_stays_green_with_its_planted_failure_does_not_close_and_fails_the_self_test() {
        let harness = with_quality(&format!("lint = 'true'\n{UNIT}"));
        let selftest = harness.run().unwrap();
        assert!(!selftest.ok());
        assert_eq!(selftest.summary(), "harness 4/4");
        let said = "G2 stayed green with its planted failure";
        assert_eq!(as_seen(&selftest.gates[1]), open(said));
        let shown = selftest.to_string();
        let line = format!("\nKO quality does not close: {said}\n-- security is unproven: ");
        assert!(shown.contains(&line), "{shown}");
        // A command of another gate that turns red with it does not close it.
        let red = "lint = 'test ! -f tests/failing.txt'\nunit = 'true'";
        let harness = Harness::new(&COMMANDS.replace(UNIT, red));
        assert_eq!(first(&harness), (FAILING_TEST.to_string(), true));
        assert_eq!(closed(&harness, "works"), open(said));
    }

    #[test]
    fn a_gate_red_without_its_planted_failure_does_not_close_and_names_what_turned_it_red() {
        let harness = with_quality(&format!("lint = 'exit 1'\ntypes = 'exit 2'\n{UNIT}"));
        let said = "G2 is red without its planted failure: lint, types";
        assert_eq!(closed(&harness, "quality"), open(said));
        // The first check did not plant its test: what turned G2 red is all works can see.
        assert_eq!(closed(&harness, "works"), open(said));
        assert!(!harness.folder.0.join("src/bad.txt").exists());
    }

    #[test]
    fn a_gate_without_a_command_of_its_own_in_its_stage_does_not_close() {
        let said = "no command of it runs in G2";
        assert_eq!(closed(&with_quality(UNIT), "quality"), open(said));
        // Its optional commands do not count.
        let optional = with_quality(&format!("mutation = 'true'\n{UNIT}"));
        assert_eq!(closed(&optional, "quality"), open(said));
    }

    #[test]
    fn the_optional_commands_of_a_gate_do_not_run_in_its_close_test() {
        let harness = with_quality(&format!("{LINT}\nmutation = 'echo ran >> mutated'\n{UNIT}"));
        let said = "its planted failure turns G2 red";
        assert_eq!(closed(&harness, "quality"), shut(said));
        // Only the first check ran it: without its test and with it.
        let mutated = fs::read_to_string(harness.folder.0.join("mutated")).unwrap();
        assert_eq!(mutated, "ran\nran\n");
    }

    #[test]
    fn a_gate_planted_from_the_failing_test_in_g2_reads_the_runs_of_the_first_check() {
        let harness = Harness::with_unit("echo ran >> runs; test ! -f tests/failing.txt");
        let said = "its planted failure turns G2 red";
        assert_eq!(closed(&harness, "works"), shut(said));
        let runs = fs::read_to_string(harness.folder.0.join("runs")).unwrap();
        assert_eq!(runs, "ran\nran\n");
        // Known debt that cannot be read stops the first check: no gate can plant its failure.
        let harness = with_quality(&format!("{LINT}\n{UNIT}"));
        harness.folder.write(debt::PATH, "[[debt]");
        assert_eq!(closed(&harness, "works"), open(FIRST_NOT_PLANTED));
        let (tested, ran, said) = closed(&harness, "quality");
        assert_eq!((tested, ran), (true, false));
        let start = "tminus/debt.toml: invalid TOML: ";
        assert!(said.starts_with(start), "{said}");
        assert!(!harness.folder.0.join("src/bad.txt").exists());
    }

    #[test]
    fn a_planted_failure_goes_back_to_what_was_there() {
        use std::os::unix::fs::PermissionsExt;
        let lint = "test ! -f src/bad.txt || grep -q mine src/bad.txt";
        let harness = with_quality(&format!("lint = '{lint}'\n{UNIT}"));
        let bad = harness.folder.0.join("src/bad.txt");
        harness.folder.write("src/bad.txt", "mine\n");
        fs::set_permissions(&bad, fs::Permissions::from_mode(0o640)).unwrap();
        let said = "its planted failure turns G2 red";
        assert_eq!(closed(&harness, "quality"), shut(said));
        assert_eq!(bytes_and_mode(&bad), ("mine\n".to_string(), 0o640));
        let kept = ".tminus/selftest/kept/src/bad.txt";
        assert!(!harness.folder.0.join(kept).exists());
    }

    /// A declaration whose one gate, `probe`, takes every command and closes before integrating
    /// from `[close_test.probe]`.
    const PROBE: &str = "format = 1\n[[gate]]\nid = \"probe\"\nchecks = \"c\"\nrest = true\n\
                         close = { stage = \"g3\", plant = \"close_test.probe\" }\n";

    /// A harness whose G3 command is `probe`, counted by `Found {n} error`, and whose failure for
    /// the gate `probe` goes at `file`.
    fn probing(probe: &str, file: &str) -> Harness {
        Harness::new(&format!(
            "[g3]\nprobe = '{probe}'\n[count]\nprobe = ['Found {{n}} error']\n\
             [close_test.probe]\nfile = '{file}'\ncontent = 'bad'\n"
        ))
    }

    /// What the close test of the gate `probe` of [`PROBE`] sees in `harness`, without runs of
    /// the first check.
    fn probe_closes(harness: &Harness) -> Result<(bool, bool, String), String> {
        let declared = Declared::from_toml(PROBE).unwrap();
        let project = harness.project(&harness.folder.0);
        gate_closes(&project, &declared, &declared.gates()[0], None)
    }

    #[test]
    fn a_close_test_before_integrating_holds_to_the_known_debt_of_that_stage() {
        let rises = "test -f bad.txt && echo Found 6 errors. || echo Found 5 errors.; false";
        let harness = probing(rises, "bad.txt");
        let owed = known_in("probe", Gate::G3, Some(5));
        harness.folder.write(debt::PATH, &owed);
        let said = "its planted failure turns G3 red";
        assert_eq!(probe_closes(&harness), Ok(shut(said)));
        assert!(!harness.folder.0.join("bad.txt").exists());
        // Debt of the other stage does not hold it.
        harness.folder.write(debt::PATH, &known("probe", Some(5)));
        let said = "G3 is red without its planted failure: probe";
        assert_eq!(probe_closes(&harness), Ok(open(said)));
        // Without the failure in the commands file, it does not run.
        let mut harness = harness;
        harness.commands = Commands::from_toml("[g3]\nprobe = 'true'\n").unwrap();
        let said = "no failure to plant for it in [close_test.probe] of tminus/commands.toml";
        assert_eq!(probe_closes(&harness), Ok(unproven(said)));
    }

    #[test]
    fn a_planted_failure_a_link_keeps_from_going_back_stays_in_its_copy() {
        let elsewhere = Folder::new();
        let leaves = format!(
            "test ! -f sub/bad.txt || {{ mv sub moved; ln -s {} sub; false; }}",
            elsewhere.0.display()
        );
        let harness = probing(&leaves, "sub/bad.txt");
        harness.folder.write("sub/bad.txt", "mine\n");
        let kept = ".tminus/selftest/kept/sub/bad.txt";
        let said = format!("sub/bad.txt was not given back: a copy is in {kept}");
        assert_eq!(probe_closes(&harness), Ok(open(&said)));
        let copy = harness.folder.0.join(kept);
        assert_eq!(fs::read_to_string(copy).unwrap(), "mine\n");
        assert_eq!(fs::read_dir(&elsewhere.0).unwrap().count(), 0);
    }

    #[test]
    fn a_gate_without_a_close_test_is_unproven() {
        let declared = Declared::from_toml(&PROBE.replace("close = ", "# close = ")).unwrap();
        let harness = probing("true", "bad.txt");
        let project = harness.project(&harness.folder.0);
        let seen = gate_closes(&project, &declared, &declared.gates()[0], None);
        let said = "the blueprint declares no close test for it";
        assert_eq!(seen, Ok(unproven(said)));
    }

    #[test]
    fn the_self_test_passes_only_when_every_check_passes_and_every_gate_tested_closed() {
        let check = |ok| Check {
            test: "c".to_string(),
            ok,
        };
        let gate = |tested, closed| Closed {
            gate: "g".to_string(),
            tested,
            closed,
            lock: String::new(),
            said: String::new(),
        };
        let selftest = |checks: &[bool], gates: Vec<Closed>| Selftest {
            checks: checks.iter().map(|&ok| check(ok)).collect(),
            gates,
        };
        let passing = [true; 4];
        assert!(selftest(&passing, vec![gate(true, true), gate(false, false)]).ok());
        assert!(!selftest(&passing, vec![gate(true, true), gate(true, false)]).ok());
        assert!(!selftest(&[true, false, true, true], vec![gate(true, true)]).ok());
        assert!(!selftest(&[false; 4], vec![]).ok());
    }

    #[test]
    fn what_the_self_test_kept_of_the_gates_is_read_back() {
        let harness = Harness::new(COMMANDS);
        let root = &harness.folder.0;
        assert_eq!(closes(root), Ok(vec![]));
        let selftest = harness.run().unwrap();
        save(root, &selftest).unwrap();
        assert_eq!(closes(root), Ok(selftest.gates));
        let path = root.join(approve::SELFTEST_PATH);
        fs::write(&path, r#"{"ok": true}"#).unwrap();
        assert_eq!(closes(root), Ok(vec![]));
        fs::write(&path, "{not json").unwrap();
        let err = closes(root).unwrap_err();
        assert!(err.starts_with(".tminus/selftest.json: "), "{err}");
        // A folder where the result goes is not a self-test that never ran.
        fs::remove_file(&path).unwrap();
        fs::create_dir_all(&path).unwrap();
        let err = closes(root).unwrap_err();
        let start = "cannot read .tminus/selftest.json: ";
        assert!(err.starts_with(start), "{err}");
    }
}
