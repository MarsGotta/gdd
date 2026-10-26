//! The harness self-test (`gdd selftest`): four checks that the gates of a project fail when
//! they must. The machines open G0 only once all four pass.
//!
//! 1. A failing test turns G2 red: the test `[selftest]` describes is planted, the G2 commands
//!    run, and the file is given back as it was.
//! 2. Editing a protected file is blocked (Path Guard and CI): the harness covers the rules file
//!    and the CI workflow, and the workflow runs `bin/gdd ci`.
//! 3. Rollback works in pre: `[release] rollback_pre` passes or, without it, the release is
//!    simulated.
//! 4. Zero scenarios counts as a failure: G3 counts none in an empty folder, and the rules keep
//!    zero scenarios a failure.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::{self, ErrorKind::NotFound};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::{Deserialize, Serialize};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Check {
    /// What it checks.
    pub test: &'static str,
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
        (FAILING_TEST, failing_test_turns_g2_red(project, config)?),
        (PROTECTED, harness_protected(project)),
        (rollback, rollback_works(project.root, config)),
        (ZERO_SCENARIOS, zero_scenarios_fail(project)?),
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

/// Plants the failing test of `config`, runs the G2 commands until one fails, and gives the file
/// back: its earlier bytes, or no file when there was none. It passes when a command failed; it
/// fails without a test to plant or G2 commands.
fn failing_test_turns_g2_red(project: &Project, config: &Config) -> Result<bool, String> {
    let commands = project.commands.of(Gate::G2);
    let (Some(file), Some(content)) = (&config.file, &config.content) else {
        return Ok(false);
    };
    if commands.is_empty() {
        return Ok(false);
    }
    let path = project.root.join(file);
    let before = match fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == NotFound => None,
        Err(e) => return Err(format!("cannot read {file}: {e}")),
    };
    let planted = written(&path, content.as_bytes());
    let red = planted.is_ok() && commands.values().any(|c| fails(c, project.root));
    let given_back = match before {
        Some(bytes) => fs::write(&path, bytes),
        None => removed(&path),
    };
    planted.map_err(|e| format!("cannot write {file}: {e}"))?;
    given_back.map_err(|e| format!("cannot give {file} back: {e}"))?;
    Ok(red)
}

/// Writes `bytes` to the file at `path`, creating its folder.
fn written(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    fs::write(path, bytes)
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

        /// The third check: its name and whether it passed.
        fn rollback(&self) -> (&'static str, bool) {
            let check = self.run().unwrap().checks[2];
            (check.test, check.ok)
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
        let harness = Harness::new(COMMANDS);
        harness.folder.write("tests/failing.txt", "mine\n");
        assert!(harness.run().unwrap().ok());
        let mine = fs::read_to_string(harness.planted()).unwrap();
        assert_eq!(mine, "mine\n");
        // A command that removed the test leaves nothing to give back.
        let harness = Harness::with_unit("rm tests/failing.txt; false");
        assert!(harness.run().unwrap().ok());
        assert!(!harness.planted().exists());
    }

    #[test]
    fn a_test_that_cannot_be_read_or_given_back_is_an_error() {
        let harness = Harness::new(COMMANDS);
        fs::create_dir_all(harness.planted()).unwrap();
        let err = harness.run().unwrap_err();
        let start = "cannot read tests/failing.txt: ";
        assert!(err.starts_with(start), "{err}");
        let swapped = "rm tests/failing.txt && mkdir tests/failing.txt && false";
        let err = Harness::with_unit(swapped).run().unwrap_err();
        let start = "cannot give tests/failing.txt back: ";
        assert!(err.starts_with(start), "{err}");
    }

    #[test]
    fn a_test_that_cannot_be_planted_is_an_error_and_nothing_runs() {
        let harness = Harness::with_unit("touch ran; false");
        fs::create_dir_all(harness.folder.0.join("tests")).unwrap();
        let nowhere = harness.folder.0.join("missing/failing.txt");
        std::os::unix::fs::symlink(nowhere, harness.planted()).unwrap();
        let err = harness.run().unwrap_err();
        let start = "cannot write tests/failing.txt: ";
        assert!(err.starts_with(start), "{err}");
        assert!(!harness.folder.0.join("ran").exists());
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
