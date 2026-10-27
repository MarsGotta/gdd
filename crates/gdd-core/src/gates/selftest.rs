//! The harness self-test (`gdd selftest`): four checks that the gates of a project fail when
//! they must. The machines open G0 only once all four pass.
//!
//! 1. A failing test turns G2 red: the file at the path of the test `[selftest]` describes is
//!    set aside, and the G2 commands must all pass without it; then the test is planted, and
//!    one G2 command at least must fail. The file is given back as it was. Neither planting nor
//!    giving back writes through a link, not even one the G2 commands made.
//! 2. Editing a protected file is blocked (Path Guard and CI): the harness covers the rules file
//!    and the CI workflow, and the workflow runs `bin/gdd ci`.
//! 3. Rollback works in pre: `[release] rollback_pre` passes or, without it, the release is
//!    simulated.
//! 4. Zero scenarios counts as a failure: G3 counts none in an empty folder, and the rules keep
//!    zero scenarios a failure.

use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File, Permissions};
use std::io::{self, ErrorKind::NotFound, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::{Deserialize, Serialize};

use crate::agents::render::first_link;
use crate::gates::{Gate, Project, approve, configured, count_scenarios};
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
/// the project root with `GDD_CHANGE=selftest`. Not being able to read, write or give back the
/// failing test, or to make the empty folder, is an error.
pub fn run(project: &Project, config: &Config) -> Result<Selftest, String> {
    let rollback = match config.rollback_pre {
        Some(_) => ROLLBACK,
        None => ROLLBACK_SIMULATED,
    };
    let checks = [
        failing_test_caught(project, config)?,
        (PROTECTED.to_string(), harness_protected(project)),
        (rollback.to_string(), rollback_works(project.root, config)),
        (ZERO_SCENARIOS.to_string(), zero_scenarios_fail(project)?),
    ];
    let checks = checks.map(|(test, ok)| Check { test, ok }).to_vec();
    Ok(Selftest { checks })
}

/// Saves `selftest` in `.gdd/selftest.json` of the project at `root`, as indented JSON with
/// `ok`, `summary`, `detail` and `ts`, and logs it: a machine step of G0.
pub fn save(root: &Path, selftest: &Selftest) -> Result<(), String> {
    let (summary, ts) = (selftest.summary(), log::now());
    let saved = Saved {
        ok: selftest.ok(),
        summary: &summary,
        detail: &selftest.checks,
        ts: &ts,
    };
    let text = serde_json::to_string_pretty(&saved).map_err(|e| e.to_string())?;
    change::write(root, approve::SELFTEST_PATH, &format!("{text}\n"))?;
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
    change::logged(root, &entry)
}

/// The first check: its name, and whether it passed. The file at the path of the failing test of
/// `config` is set aside (see [`set_aside`]) and the G2 commands run, which must all pass; then
/// the failing test is planted and they run until one fails. The file is given back in every
/// case: its earlier bytes and permissions, or no file when there was none. The check passes
/// when G2 was green without the test and red with it; when it was red without it, the name
/// says which commands failed. It fails without a test to plant or G2 commands.
fn failing_test_caught(project: &Project, config: &Config) -> Result<(String, bool), String> {
    let commands = project.commands.of(Gate::G2);
    let (Some(file), Some(content)) = (&config.file, &config.content) else {
        return Ok((FAILING_TEST.to_string(), false));
    };
    if commands.is_empty() {
        return Ok((FAILING_TEST.to_string(), false));
    }
    let root = project.root;
    let before = set_aside(root, file)?;
    let red_without = failing(commands, root);
    let red = if red_without.is_empty() {
        planted_and_red(root, file, content, commands)
    } else {
        Ok(false)
    };
    let given_back = give_back(root, file, before);
    let red = red.map_err(|e| format!("cannot write {file}: {e}"))?;
    given_back.map_err(|e| format!("cannot give {file} back: {e}"))?;
    if red_without.is_empty() {
        return Ok((FAILING_TEST.to_string(), red));
    }
    let failed = red_without.join(", ");
    let named = format!("{FAILING_TEST} (G2 is red without it: {failed})");
    Ok((named, false))
}

/// Plants `content` at `file` of the project at `root`, in place of whatever the G2 commands
/// left there (see [`cleared`]), and runs the `commands` from `root` until one fails: whether one
/// did. A test that cannot be planted is an error, and then nothing runs.
fn planted_and_red(
    root: &Path,
    file: &str,
    content: &str,
    commands: &BTreeMap<String, String>,
) -> io::Result<bool> {
    cleared(root, file)?;
    written(&root.join(file), content.as_bytes())?;
    Ok(commands.values().any(|c| fails(c, root)))
}

/// What was at `file` of the project at `root`, now set aside: its bytes and permissions, or
/// `None` when there was no file. A file behind a symbolic link, or with other names (a hard
/// link), is refused before anything is written: the self-test would write out of the project,
/// or could not give it back as it was.
fn set_aside(root: &Path, file: &str) -> Result<Option<(Vec<u8>, Permissions)>, String> {
    let cannot_read = |e: io::Error| format!("cannot read {file}: {e}");
    if let Some(link) = first_link(root, file).map_err(cannot_read)? {
        return Err(format!(
            "cannot set {file} aside: {} is a symbolic link, which the self-test does not write \
             through: remove the link and run it again",
            link.display()
        ));
    }
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
    let aside = fs::remove_file(&path);
    aside.map_err(|e| format!("cannot set {file} aside: {e}"))?;
    Ok(Some((bytes, meta.permissions())))
}

/// Gives back what was at `file` of the project at `root` before the self-test, in place of
/// whatever the G2 commands left there (see [`cleared`]): `before`, its bytes and permissions,
/// or no file.
fn give_back(root: &Path, file: &str, before: Option<(Vec<u8>, Permissions)>) -> io::Result<()> {
    cleared(root, file)?;
    match before {
        Some((bytes, permissions)) => {
            let given = written(&root.join(file), &bytes)?;
            given.set_permissions(permissions)
        }
        None => Ok(()),
    }
}

/// Removes what the G2 commands left at `file` of the project at `root`: the planted test, a
/// file or a link, which goes and is never followed. A symbolic link on the way to it is an
/// error, and then nothing is removed: through it, the self-test would write out of the project.
fn cleared(root: &Path, file: &str) -> io::Result<()> {
    match first_link(root, file)? {
        Some(link) if link != Path::new(file) => Err(io::Error::other(format!(
            "{} is a symbolic link, which the self-test does not write through",
            link.display()
        ))),
        _ => removed(&root.join(file)),
    }
}

/// The names of the `commands` that fail, run from `root` (see [`fails`]), in their order.
fn failing<'a>(commands: &'a BTreeMap<String, String>, root: &Path) -> Vec<&'a str> {
    let failed = commands.iter().filter(|(_, c)| fails(c, root));
    failed.map(|(name, _)| name.as_str()).collect()
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

/// Whether `command`, run from `root` with `GDD_CHANGE=selftest` (see [`exec::run`]), fails: it
/// ends with a code other than 0, or it cannot run.
fn fails(command: &str, root: &Path) -> bool {
    let env = [("GDD_CHANGE", "selftest")];
    let ran = exec::run(command, root, &env, exec::TIMEOUT);
    !ran.is_ok_and(|ran| ran.ok())
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

/// Whether the rollback in pre works: its command passes, from `root`; without one, the release
/// is simulated.
fn rollback_works(root: &Path, config: &Config) -> bool {
    match &config.rollback_pre {
        Some(command) => !fails(command, root),
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

        fn run(&self) -> Result<Selftest, String> {
            let project = Project {
                root: &self.folder.0,
                zones: &self.zones,
                rules: &self.rules,
                settings: &self.settings,
                commands: &self.commands,
            };
            run(&project, &self.config)
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
        let harness = Harness::new(COMMANDS);
        harness.folder.write("tests/failing.txt", "mine\n");
        let runnable = fs::Permissions::from_mode(0o751);
        fs::set_permissions(harness.planted(), runnable).unwrap();
        assert!(harness.run().unwrap().ok());
        let mine = fs::read_to_string(harness.planted()).unwrap();
        assert_eq!(mine, "mine\n");
        let back = fs::metadata(harness.planted()).unwrap();
        assert_eq!(back.permissions().mode() & 0o777, 0o751);
        // A command that removed the test leaves nothing to give back.
        let removes = "test ! -f tests/failing.txt || { rm tests/failing.txt; false; }";
        let harness = Harness::with_unit(removes);
        assert!(harness.run().unwrap().ok());
        assert!(!harness.planted().exists());
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
        // Left by G2 with the test: nothing is given back through it.
        let leaves = format!(
            "test ! -f tests/failing.txt || {{ mv tests moved; ln -s {} tests; false; }}",
            elsewhere.0.display()
        );
        let harness = Harness::with_unit(&leaves);
        harness.folder.write("tests/failing.txt", "mine\n");
        let err = harness.run().unwrap_err();
        let why = "cannot give tests/failing.txt back: tests is a symbolic link, which the \
                   self-test does not write through";
        assert_eq!(err, why);
        assert_eq!(fs::read_dir(&elsewhere.0).unwrap().count(), 0);
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
