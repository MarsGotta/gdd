//! The harness self-test (`gdd selftest`): four checks that the gates of a project fail when
//! they must. The machines open G0 only once all four pass.
//!
//! 1. A failing test turns G2 red: the file at the path of the test `[selftest]` describes is
//!    set aside, and G2 must be green without it; then the test is planted, and G2 must turn
//!    red. G2 is judged as the gate judges it: a command red when it fails and is not held to its
//!    known debt (see [`debt`]). The file is given back as it was, from a copy kept in
//!    `.gdd/selftest/kept/` meanwhile. Neither planting nor giving back writes through a link,
//!    not even one the G2 commands made: when one stops the file from going back, the copy stays.
//! 2. Editing a protected file is blocked (Path Guard and CI): the harness covers the rules file
//!    and the CI workflow, and the workflow runs `bin/gdd ci`.
//! 3. Rollback works in pre: `[release] rollback_pre` passes or, without it, the release is
//!    simulated.
//! 4. Zero scenarios counts as a failure: G3 counts none in an empty folder, and the rules keep
//!    zero scenarios a failure.

use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, ErrorKind::AlreadyExists, ErrorKind::NotFound, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::{Deserialize, Serialize};

use crate::agents::render::first_link;
use crate::gates::{Gate, Project, approve, configured, count_scenarios, debt};
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

/// The CI workflow, from the project root.
const WORKFLOW: &str = ".github/workflows/gdd.yml";

/// What the CI workflow must run.
const RUNS_CI: &str = "bin/gdd ci";

/// Where the file at the path of the failing test is kept while it is set aside, under its own
/// path, from the state root (see [`crate::state::root`]).
const KEPT: &str = ".gdd/selftest/kept";

/// What the self-test reads from `gdd/commands.toml` besides the G2 commands.
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

/// What the self-test found: its checks, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selftest {
    /// The checks.
    pub checks: Vec<Check>,
}

impl Selftest {
    /// Whether every check passed.
    pub fn ok(&self) -> bool {
        self.checks.iter().all(|check| check.ok)
    }

    /// `harness <passed>/<checks>`.
    pub fn summary(&self) -> String {
        let passed = self.checks.iter().filter(|check| check.ok).count();
        format!("harness {passed}/{}", self.checks.len())
    }
}

impl fmt::Display for Selftest {
    /// `OK <test>` or `KO <test>`, a line each, then the summary, without a line break at the
    /// end.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for check in &self.checks {
            let mark = if check.ok { "OK" } else { "KO" };
            writeln!(f, "{mark} {}", check.test)?;
        }
        f.write_str(&self.summary())
    }
}

/// What `.gdd/selftest.json` keeps.
#[derive(Serialize)]
struct Saved<'a> {
    ok: bool,
    summary: &'a str,
    detail: &'a [Check],
    ts: &'a str,
}

/// Runs the four checks on `project`, whose commands file says `config`. The commands run from
/// the project root with `GDD_CHANGE=selftest`. Not being able to read, set aside, write or give
/// back the failing test, or to make the empty folder, is an error; a file that a link keeps from
/// going back fails the first check instead (see [`failing_test_caught`]).
pub fn run(project: &Project, config: &Config) -> Result<Selftest, String> {
    let rollback = match config.rollback_pre {
        Some(_) => ROLLBACK,
        None => ROLLBACK_SIMULATED,
    };
    let checks = [
        failing_test_caught(project, config)?,
        (PROTECTED.to_string(), harness_protected(project)),
        (rollback.to_string(), rollback_works(project, config)),
        (ZERO_SCENARIOS.to_string(), zero_scenarios_fail(project)?),
    ];
    let checks = checks.map(|(test, ok)| Check { test, ok }).to_vec();
    Ok(Selftest { checks })
}

/// Saves `selftest` in `.gdd/selftest.json` under the state root `state`, as indented JSON with
/// `ok`, `summary`, `detail` and `ts`, and logs it: a machine step of G0.
pub fn save(state: &Path, selftest: &Selftest) -> Result<(), String> {
    let (summary, ts) = (selftest.summary(), log::now());
    let saved = Saved {
        ok: selftest.ok(),
        summary: &summary,
        detail: &selftest.checks,
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

/// The first check: its name, and whether it passed. The file at the path of the failing test of
/// `config` is set aside (see [`set_aside`]) and the G2 commands run, none of which may turn G2
/// red, held to the known debt of G2 as the gate holds them (see [`red`]); then the failing test
/// is planted and they run until one turns G2 red. The file is given back in every case (see
/// [`give_back`]): its earlier bytes and permissions, or no file when there was none. The check
/// passes when G2 was green without the test and red with it; when it was red without it, the
/// name says which commands turned it red. It fails without a test to plant or G2 commands; when
/// the known debt cannot be read, and then nothing runs; and when a link kept the file from going
/// back: then the name says why, or where its copy is. What goes wrong with a file to give back
/// is told before what goes wrong with the test.
fn failing_test_caught(project: &Project, config: &Config) -> Result<(String, bool), String> {
    let commands = project.commands.of(Gate::G2);
    let (Some(file), Some(content)) = (&config.file, &config.content) else {
        return Ok((FAILING_TEST.to_string(), false));
    };
    if commands.is_empty() {
        return Ok((FAILING_TEST.to_string(), false));
    }
    let owed = match debt::of_gate(project.root, Gate::G2) {
        Ok(owed) => owed,
        Err(why) => return Ok((format!("{FAILING_TEST} ({why})"), false)),
    };
    let (root, state) = (project.root, project.state);
    let kept = set_aside(root, state, file)?;
    let red_without = turning_red(commands, project, &owed);
    let red = if red_without.is_empty() {
        planted_and_red(project, file, content, &owed)
    } else {
        Ok(false)
    };
    let given_back = give_back(root, state, file, kept.as_deref());
    if let Some(kept) = &kept {
        let given = given_back.as_ref();
        if !*given.map_err(|e| format!("cannot give {file} back: {e}: a copy is in {kept}"))? {
            let named = format!("{FAILING_TEST} ({file} was not given back: a copy is in {kept})");
            return Ok((named, false));
        }
    }
    let red = red.map_err(|e| format!("cannot write {file}: {e}"))?;
    given_back.map_err(|e| format!("cannot give {file} back: {e}"))?;
    if red_without.is_empty() {
        return Ok((FAILING_TEST.to_string(), red));
    }
    let failed = red_without.join(", ");
    let named = format!("{FAILING_TEST} (G2 is red without it: {failed})");
    Ok((named, false))
}

/// Plants `content` at `file` of `project`, in place of whatever the G2 commands left there (see
/// [`cleared`]), and runs its G2 commands from its root, held to the known debt `owed`, until one
/// turns G2 red (see [`red`]): whether one did. A test that cannot be planted is an error, and
/// then nothing runs.
fn planted_and_red(
    project: &Project,
    file: &str,
    content: &str,
    owed: &BTreeMap<String, Option<u64>>,
) -> io::Result<bool> {
    let root = project.root;
    cleared(root, file)?;
    written(&root.join(file), content.as_bytes())?;
    let mut commands = project.commands.of(Gate::G2).iter();
    Ok(commands.any(|(name, c)| red(name, c, project, owed)))
}

/// Sets aside what was at `file` of the project at `root`: a copy of it, bytes and permissions,
/// is kept at its path under [`KEPT`] of the state root `state`, and it goes. Where the copy is,
/// from the state root, or `None` when there was no file. A symbolic link on the way to the file
/// or to its copy, a file with other names (a hard link) or a copy already there is refused
/// before anything is written: the self-test would write out of the project, could not give the
/// file back as it was, or would lose the copy an earlier run kept.
fn set_aside(root: &Path, state: &Path, file: &str) -> Result<Option<String>, String> {
    let kept = format!("{KEPT}/{file}");
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
/// whatever the G2 commands left there: the file whose copy is at `kept` of the state root
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

/// Removes what the G2 commands left at `file` of the project at `root`: the planted test, a
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

/// The names of the G2 `commands` that turn G2 red, run in `project` held to the known debt
/// `owed` (see [`red`]), in their order.
fn turning_red<'a>(
    commands: &'a BTreeMap<String, String>,
    project: &Project,
    owed: &BTreeMap<String, Option<u64>>,
) -> Vec<&'a str> {
    let turned = commands.iter().filter(|(n, c)| red(n, c, project, owed));
    turned.map(|(name, _)| name.as_str()).collect()
}

/// Whether the G2 command `name`, `command`, turns G2 red, run in `project` (see [`failed`]): it
/// fails, and it is not held to its known debt in `owed`, by name, the count recorded for it
/// when there is one, as G2 holds it (see [`debt::held`]).
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

/// What `command` printed, run from the root of `project` with `GDD_CHANGE=selftest`, within the
/// time limit of its rules (see [`exec::run_limited`]), when it fails: it ends with a code other
/// than 0, or it cannot run, and then it printed nothing. `None` when it passes.
fn failed(command: &str, project: &Project) -> Option<String> {
    let env = [("GDD_CHANGE", "selftest")];
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

/// Whether the harness of `project` covers the rules file and the CI workflow, and the workflow
/// runs `bin/gdd ci`. A workflow that cannot be read runs nothing.
fn harness_protected(project: &Project) -> bool {
    let harness = &project.rules.harness;
    let protected = |path: &str| harness.iter().any(|p| glob_matches(p, path));
    let workflow = fs::read_to_string(project.root.join(WORKFLOW));
    let runs_ci = workflow.is_ok_and(|text| text.contains(RUNS_CI));
    protected(guard::RULES) && protected(WORKFLOW) && runs_ci
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

    const RULES: &str = "[protected]\nall = [\"gdd/**\", \".github/**\"]\n";

    const COMMANDS: &str = "[g2]\nunit = 'test ! -f tests/failing.txt'\n\
                            [selftest]\nfile = 'tests/failing.txt'\ncontent = 'fails'\n\
                            [release]\nsimulated = true\nrollback_pre = ''\n";

    /// The G2 command of `COMMANDS`.
    const UNIT: &str = "unit = 'test ! -f tests/failing.txt'";

    /// A CI workflow that runs the gates.
    const RUNS_THE_GATES: &str = "steps:\n  - run: bin/gdd ci\n";

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
            self.folder.0.join(".gdd/selftest/kept/tests/failing.txt")
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
        let owed = debt::Debt {
            command: command.to_string(),
            gate: Gate::G2,
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
        let copies = "test ! -f tests/failing.txt || cp -p .gdd/selftest/kept/tests/failing.txt c";
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
        let copy = state.0.join(".gdd/selftest/kept/tests/failing.txt");
        // Green only while the copy is there, under the state root, and the test is not.
        let unit = format!("test ! -f tests/failing.txt && test -f {}", copy.display());
        let harness = Harness::with_unit(&unit);
        harness.folder.write("tests/failing.txt", "mine\n");
        let selftest = run(&harness.project(&state.0), &harness.config).unwrap();
        assert!(selftest.ok(), "{selftest}");
        assert_eq!(fs::read_to_string(harness.planted()).unwrap(), "mine\n");
        assert!(!copy.exists());
        assert!(!harness.folder.0.join(".gdd").exists());
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
        // More than its count, or nothing gdd can count.
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
        let start = "a failing test turns G2 red (gdd/debt.toml: invalid TOML: ";
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
        let end = ": a copy is in .gdd/selftest/kept/tests/failing.txt";
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
        fs::create_dir_all(harness.folder.0.join(".gdd/selftest")).unwrap();
        let kept = harness.folder.0.join(".gdd/selftest/kept");
        std::os::unix::fs::symlink(&elsewhere.0, kept).unwrap();
        let err = harness.run().unwrap_err();
        let start = "cannot set tests/failing.txt aside: .gdd/selftest/kept is a symbolic link";
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
                        is in .gdd/selftest/kept/tests/failing.txt)";
        // A link left by G2 with the test on the way to the file, or to its copy.
        let ways = [
            ("tests", ".gdd/selftest/kept/tests/failing.txt"),
            (".gdd/selftest/kept", "moved/tests/failing.txt"),
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
        let earlier = ".gdd/selftest/kept/tests/failing.txt";
        harness.folder.write(earlier, "earlier\n");
        let err = harness.run().unwrap_err();
        let why = "cannot set tests/failing.txt aside: .gdd/selftest/kept/tests/failing.txt is \
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
        for all in ["[]", "[\"gdd/**\"]", "[\".github/**\"]"] {
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
    fn the_rollback_in_pre_passes_or_the_release_is_simulated() {
        let with_rollback = |command: &str| {
            let empty = "rollback_pre = ''";
            let commands = COMMANDS.replace(empty, &format!("rollback_pre = '{command}'"));
            Harness::new(&commands)
        };
        let ours = with_rollback("test \"$GDD_CHANGE\" = selftest");
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
        let start = "cannot write .gdd/selftest.json: ";
        assert!(err.starts_with(start), "{err}");
    }
}
