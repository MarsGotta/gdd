//! The gates the machines run: G2, the Loop Gate of every Builder round, and G3, the Merge
//! Gate. A gate runs its commands from `gdd/commands.toml` once Path Guard clears them, then
//! Path Guard again, the scope check in build mode and, in G3, the scenario count and the
//! security exceptions ([`exceptions`]). It passes only if all of them pass.
//!
//! The gates a person or the machines decide are recorded by [`approve`]; [`phrasing`] tells
//! whether hidden examples only reuse visible sentences. [`prepare`] checks that a change may
//! enter Build, and [`release`] takes a merged change to pre and production. [`verdict`] reads
//! what the Reviewer and the replan answered. [`selftest`] checks that the harness can fail when
//! it must, and [`ci`] is what CI runs on a change branch.

pub mod approve;
pub mod ci;
pub mod exceptions;
pub mod phrasing;
pub mod prepare;
pub mod release;
pub mod selftest;
pub mod verdict;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::Path;

use serde::{Deserialize, Serialize, Serializer};

use crate::change::{self, Record};
use crate::classify::{Classification, classify};
use crate::config::{self, Color};
use crate::exec;
use crate::git;
use crate::guard::{self, Hashes, Settings};
use crate::log;
use crate::rules::RulesTable;
use crate::zones::ZoneMap;

/// The role of build mode: in a gate, the scope check judges it too.
pub const BUILD_ROLE: &str = "builder-build";

/// The gate and release commands of a project, from its root.
pub const COMMANDS: &str = "gdd/commands.toml";

/// The name of the Path Guard check in a report.
pub(crate) const PATH_GUARD: &str = "path-guard";

/// Why a gate without commands fails.
const NOT_CONFIGURED: &str =
    "gdd/commands.toml has no commands for this gate. The Setup fills them in.";

/// Why G3 fails without scenarios.
const NO_SCENARIOS: &str = "Zero scenarios counts as a failure.";

/// Why the commands of a gate did not run.
const NOT_RUN: &str = "The gate commands did not run: Path Guard must pass first.";

/// Lines that report a failure a failed command keeps from its output, at most.
const KEPT_LINES: usize = 40;

/// Characters a failed command keeps from the end of its output.
const KEPT_END: usize = 2_500;

/// Characters of the log note that names the failures, at most.
const NOTE_LENGTH: usize = 300;

/// Marks of a line of output that reports a failure.
const FAILURE_MARKS: [&str; 7] = [
    "✘",
    "FAILED",
    "Failed",
    "failed",
    "panicked",
    "Error:",
    "Traceback",
];

/// Keywords that start a scenario, in English and in Spanish.
const SCENARIO_KEYWORDS: [&str; 4] = [
    "Scenario",
    "Scenario Outline",
    "Escenario",
    "Esquema del escenario",
];

/// A gate the machines run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// The Loop Gate: every Builder round.
    G2,
    /// The Merge Gate: the full automatic chain.
    G3,
}

impl Gate {
    /// Its section of `gdd/commands.toml`, and the name of its report.
    fn key(self) -> &'static str {
        match self {
            Gate::G2 => "g2",
            Gate::G3 => "g3",
        }
    }

    /// The phase of the method it belongs to.
    pub fn phase(self) -> &'static str {
        match self {
            Gate::G2 => "2-build",
            Gate::G3 => "3-integrate",
        }
    }

    /// Where the report of its last run on change `id` lives, from the state root (see
    /// [`crate::state::root`]).
    pub fn report_path(self, id: &str) -> String {
        format!(".gdd/changes/{id}/{}.json", self.key())
    }
}

impl fmt::Display for Gate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Gate::G2 => "G2",
            Gate::G3 => "G3",
        })
    }
}

/// The gate commands of a project, `gdd/commands.toml`: lines of shell, by name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Commands {
    g2: BTreeMap<String, String>,
    g3: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct RawCommands {
    #[serde(default)]
    g2: BTreeMap<String, toml::Value>,
    #[serde(default)]
    g3: BTreeMap<String, toml::Value>,
}

impl Commands {
    /// Reads the `[g2]` and `[g3]` sections of a commands file; the rest is ignored. A command is
    /// text that is not blank: an empty one is not configured, and a value that is not text is
    /// not a command.
    pub fn from_toml(text: &str) -> Result<Commands, config::Error> {
        let raw: RawCommands =
            toml::from_str(text).map_err(|e| config::Error::Toml(e.to_string()))?;
        Ok(Commands {
            g2: configured(raw.g2),
            g3: configured(raw.g3),
        })
    }

    /// The commands of `gate`, by name.
    pub fn of(&self, gate: Gate) -> &BTreeMap<String, String> {
        match gate {
            Gate::G2 => &self.g2,
            Gate::G3 => &self.g3,
        }
    }
}

/// The values of a section that are commands: text that is not blank.
pub(crate) fn configured(section: BTreeMap<String, toml::Value>) -> BTreeMap<String, String> {
    section
        .into_iter()
        .filter_map(|(name, value)| match value {
            toml::Value::String(command) if !command.trim().is_empty() => Some((name, command)),
            _ => None,
        })
        .collect()
}

/// What the rules file says to a gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rules {
    /// The rules table the gate classifies the change with, the one `gdd classify` uses (see
    /// [`RulesTable::of_project`]).
    pub table: RulesTable,
    /// Whether zero scenarios fail G3 even when the change has no scenarios of its own:
    /// `[acceptance] required`. Unset: yes.
    pub scenarios_required: bool,
    /// The harness: the paths no role may change, `[protected] all`.
    pub harness: Vec<String>,
}

#[derive(Deserialize)]
struct RawRules {
    #[serde(default)]
    acceptance: RawAcceptance,
    #[serde(default)]
    protected: RawProtected,
}

#[derive(Default, Deserialize)]
struct RawProtected {
    #[serde(default)]
    all: Vec<String>,
}

#[derive(Deserialize)]
#[serde(default)]
struct RawAcceptance {
    required: bool,
}

impl Default for RawAcceptance {
    fn default() -> RawAcceptance {
        RawAcceptance { required: true }
    }
}

impl Rules {
    /// Reads what a gate needs from a rules file; see [`RulesTable::from_toml`].
    pub fn from_toml(text: &str) -> Result<Rules, config::Error> {
        let raw: RawRules = toml::from_str(text).map_err(|e| config::Error::Toml(e.to_string()))?;
        Ok(Rules {
            table: RulesTable::of_project(text)?,
            scenarios_required: raw.acceptance.required,
            harness: raw.protected.all,
        })
    }
}

/// What a gate reads from the project: its root, its state root and its configuration.
#[derive(Debug, Clone, Copy)]
pub struct Project<'a> {
    /// The project root: the commands run from it.
    pub root: &'a Path,
    /// The state root, whose `.gdd/` holds the records and the reports (see
    /// [`crate::state::root`]).
    pub state: &'a Path,
    /// The zone map.
    pub zones: &'a ZoneMap,
    /// What the rules file says to the gates.
    pub rules: &'a Rules,
    /// What the rules file says to Path Guard.
    pub settings: &'a Settings,
    /// The gate commands.
    pub commands: &'a Commands,
}

/// What stops a gate before it can judge the change.
#[derive(Debug)]
pub enum Error {
    /// Reading what the change touched in git failed.
    Git(git::Error),
    /// The change could not be classified.
    Config(config::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Git(e) => write!(f, "{e}"),
            Error::Config(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<git::Error> for Error {
    fn from(e: git::Error) -> Error {
        Error::Git(e)
    }
}

impl From<config::Error> for Error {
    fn from(e: config::Error) -> Error {
        Error::Config(e)
    }
}

/// The report of a gate run. As JSON, the gate and the color are their names.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// Whether the gate passed: nothing failed.
    pub ok: bool,
    /// The gate.
    #[serde(serialize_with = "as_text")]
    pub gate: Gate,
    /// The color of the change.
    #[serde(serialize_with = "as_text")]
    pub color: Color,
    /// Names of the checks that passed, in the order they ran.
    pub passed: Vec<String>,
    /// Names of the commands the change does not need.
    pub skipped: Vec<String>,
    /// The checks that failed, in the order they ran.
    pub failures: Vec<Failure>,
    /// In build mode, the paths outside the plan the scope check accepted.
    pub out_of_scope: Vec<String>,
    /// In G3, a line for each security exception the change adds (see [`exceptions::added`]);
    /// left out of the JSON when there is none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub added_exceptions: Vec<String>,
    /// Seconds the commands took, added up.
    #[serde(skip)]
    pub duration_s: f64,
}

/// A check of a gate that failed.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Failure {
    /// The name of the command, or of the check.
    pub name: String,
    /// How its command ended; nothing for a check that is not a command.
    #[serde(flatten)]
    pub exit: Option<Exit>,
    /// What to read: for a command, what its output keeps (see [`kept`]).
    pub tail: String,
}

/// How a command of a gate ended.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Exit {
    /// Whether it passed.
    pub ok: bool,
    /// Its exit code (see [`exec::Ran::code`]).
    pub code: i32,
    /// Seconds it took, to a tenth.
    pub duration_s: f64,
}

fn as_text<T: fmt::Display, S: Serializer>(value: &T, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(value)
}

impl Report {
    fn fail(&mut self, name: &str, tail: String) {
        self.failures.push(Failure {
            name: name.to_string(),
            exit: None,
            tail,
        });
    }

    /// Records a check by its failures: it passes without any.
    fn judge(&mut self, name: &str, failures: Vec<String>) {
        if failures.is_empty() {
            self.passed.push(name.to_string());
        } else {
            self.fail(name, failures.join("\n"));
        }
    }

    /// Records what the command `name` did.
    fn command(&mut self, name: &str, ran: io::Result<exec::Ran>) {
        let ran = match ran {
            Ok(ran) => ran,
            Err(e) => return self.fail(name, format!("cannot run sh: {e}")),
        };
        let duration_s = tenths(ran.duration.as_secs_f64());
        self.duration_s += duration_s;
        if ran.ok() {
            self.passed.push(name.to_string());
        } else {
            self.failures.push(Failure {
                name: name.to_string(),
                exit: Some(Exit {
                    ok: false,
                    code: ran.code,
                    duration_s,
                }),
                tail: kept(&ran.output),
            });
        }
    }

    /// The line of the log for this run on change `id`: a machine step, `ok` or `ko`, with the
    /// time of the commands and a note that names what failed.
    pub fn log_entry(&self, id: &str) -> log::Entry {
        let failed: Vec<&str> = self.failures.iter().map(|f| f.name.as_str()).collect();
        log::Entry {
            change: id.to_string(),
            phase: self.gate.phase().to_string(),
            gate: self.gate.to_string(),
            role: "machine".to_string(),
            agent: "checks".to_string(),
            result: if self.ok { "ok" } else { "ko" }.to_string(),
            duration_s: tenths(self.duration_s),
            note: failed.join(", ").chars().take(NOTE_LENGTH).collect(),
        }
    }
}

/// `seconds` to a tenth of a second.
pub(crate) fn tenths(seconds: f64) -> f64 {
    (seconds * 10.0).round() / 10.0
}

/// Saves `report`, of a run on change `id`, next to the record of the change under the state root
/// `state` (see [`Gate::report_path`]), as indented JSON, and logs the run.
pub fn save(state: &Path, id: &str, report: &Report) -> Result<(), String> {
    let saved = serde_json::to_string_pretty(report).map_err(|e| e.to_string())?;
    change::write(state, &report.gate.report_path(id), &format!("{saved}\n"))?;
    change::logged(state, &report.log_entry(id))
}

/// Runs `gate` on change `id`, whose record is `record`, for `role`. The change is classified
/// from what it plans and what it touched against the main branch; its scenario files are the
/// ones it plans or touched and the ones its record names. First Path Guard judges `role`, from
/// the snapshot `snapshot` when there is one: the commands run only if it passes, since a role
/// that touched the gate configuration could make them its own and put the files back as they
/// ran. Each configured command of the gate runs from the project root (see [`exec::run`]),
/// except the `mutation*` ones when the change does not need mutation testing. Each gets
/// `GDD_CHANGE`, `GDD_COLOR`, `GDD_FEATURES` (the change's scenario files) and `GDD_DIFF_FILES`
/// (what it touched, or what it plans while it touched nothing), one path per line. Then Path
/// Guard judges again; build mode also gets the scope check, and G3 counts the scenarios: none
/// fails it when they are required or the change has its own. G3 also refuses a security
/// exception without a reason and names the ones the change adds against the main branch (see
/// [`exceptions`]). A gate without commands fails. A check that cannot judge fails with the
/// reason.
pub fn run(
    project: &Project,
    gate: Gate,
    id: &str,
    record: &Record,
    role: &str,
    snapshot: Option<&Hashes>,
) -> Result<Report, Error> {
    let root = project.root;
    let touched = guard::touched(root, id, project.settings)?;
    let mut plan = record.plan.clone();
    plan.paths.extend(touched.iter().cloned());
    // Scenarios of its own put the change in G1, and a change in G1 takes the full lane.
    plan.resumable &= record.features.is_empty();
    let decision = classify(project.zones, &project.rules.table, &plan)?;
    let features: BTreeSet<&String> = decision.features.iter().chain(&record.features).collect();
    let features = features.into_iter().cloned().collect();
    let diff = if touched.is_empty() {
        &record.plan.paths
    } else {
        &touched
    };
    let judged = Judged {
        id,
        decision,
        diff,
        features,
    };
    let commands = project.commands.of(gate);
    let mut report = judged.report(gate, commands);
    let guard = || {
        let verdict = guard::path_guard(root, id, record, project.settings, role, snapshot);
        verdict.map_or_else(|e| vec![e.to_string()], |verdict| verdict.failures)
    };
    let mut guarded = guard();
    if guarded.is_empty() {
        judged.run_commands(&mut report, root, commands);
        guarded = guard();
    } else {
        guarded.push(NOT_RUN.to_string());
    }
    report.judge(PATH_GUARD, guarded);
    if role == BUILD_ROLE {
        let failures = match guard::scope(root, id, record, project.zones, &project.rules.table) {
            Ok(scope) => {
                report.out_of_scope = scope.accepted;
                scope.failures
            }
            Err(e) => vec![e.to_string()],
        };
        report.judge("scope", failures);
    }
    if gate == Gate::G3 {
        judged.scenarios(&mut report, project);
        let found = exceptions::read(root);
        exceptions::judge(&mut report, &found);
        if let Ok(found) = &found {
            let main = &project.settings.main_branch;
            let added = exceptions::added(root, main, found, exceptions::today())?;
            report.added_exceptions = added;
        }
    }
    report.ok = report.failures.is_empty();
    Ok(report)
}

/// What a gate judges: a change, how it is classified, what it touched and its scenario files.
pub(crate) struct Judged<'a> {
    /// The id of the change.
    pub(crate) id: &'a str,
    /// How it is classified.
    pub(crate) decision: Classification,
    /// What it touched.
    pub(crate) diff: &'a [String],
    /// Its scenario files, sorted.
    pub(crate) features: Vec<String>,
}

impl Judged<'_> {
    /// The report of `gate`, whose commands are `commands`, before anything runs: a gate
    /// without commands fails already.
    pub(crate) fn report(&self, gate: Gate, commands: &BTreeMap<String, String>) -> Report {
        let mut report = Report {
            ok: false,
            gate,
            color: self.decision.color,
            passed: Vec::new(),
            skipped: Vec::new(),
            failures: Vec::new(),
            out_of_scope: Vec::new(),
            added_exceptions: Vec::new(),
            duration_s: 0.0,
        };
        if commands.is_empty() {
            let name = format!("{gate} not configured");
            report.fail(&name, NOT_CONFIGURED.to_string());
        }
        report
    }

    /// Runs each of `commands` from `root` (see [`exec::run`]) into `report`, except the
    /// `mutation*` ones when the change does not need mutation testing. Each gets `GDD_CHANGE`,
    /// `GDD_COLOR`, `GDD_DIFF_FILES` and `GDD_FEATURES`, one path per line.
    pub(crate) fn run_commands(
        &self,
        report: &mut Report,
        root: &Path,
        commands: &BTreeMap<String, String>,
    ) {
        let color = self.decision.color.to_string();
        let (diff, features) = (self.diff.join("\n"), self.features.join("\n"));
        let env = [
            ("GDD_CHANGE", self.id),
            ("GDD_COLOR", color.as_str()),
            ("GDD_DIFF_FILES", diff.as_str()),
            ("GDD_FEATURES", features.as_str()),
        ];
        for (name, command) in commands {
            if name.starts_with("mutation") && !self.decision.mutation {
                report.skipped.push(name.clone());
            } else {
                report.command(name, exec::run(command, root, &env, exec::TIMEOUT));
            }
        }
    }

    /// G3's scenario count into `report`: none fails it when they are required or the change
    /// has its own.
    pub(crate) fn scenarios(&self, report: &mut Report, project: &Project) {
        let folder = &project.rules.table.acceptance.dir;
        match count_scenarios(&project.root.join(folder)) {
            Ok(0) if !self.features.is_empty() || project.rules.scenarios_required => {
                report.fail("scenarios", NO_SCENARIOS.to_string());
            }
            Ok(count) => report.passed.push(format!("scenarios ({count})")),
            Err(e) => {
                let reason = format!("cannot count the scenarios in {folder}: {e}");
                report.fail("scenarios", reason);
            }
        }
    }
}

/// What a failed command keeps of its output. Runners print failures early and their summary
/// late: the first lines that report a failure, then the end of the output.
pub fn kept(output: &str) -> String {
    let failures: Vec<&str> = output
        .lines()
        .filter(|line| reports_failure(line))
        .take(KEPT_LINES)
        .collect();
    let end = match output.char_indices().nth_back(KEPT_END - 1) {
        Some((start, _)) => &output[start..],
        None => output,
    };
    if failures.is_empty() {
        end.to_string()
    } else {
        format!("{}\n--- end of output ---\n{end}", failures.join("\n"))
    }
}

/// Whether a line of output reports a failure: it starts with `error` or has a mark of one.
fn reports_failure(line: &str) -> bool {
    line.starts_with("error") || FAILURE_MARKS.iter().any(|mark| line.contains(mark))
}

/// The scenarios in the `.feature` files inside `folder` and its subfolders: lines that start
/// with a scenario keyword and a colon. No folder, no scenarios.
pub fn count_scenarios(folder: &Path) -> io::Result<usize> {
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    let mut count = 0;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            count += count_scenarios(&path)?;
        } else if path.extension().is_some_and(|ext| ext == "feature") {
            let text = fs::read(&path)?;
            let text = String::from_utf8_lossy(&text);
            count += text.lines().filter(|line| starts_scenario(line)).count();
        }
    }
    Ok(count)
}

/// Whether a line of a `.feature` file starts a scenario.
fn starts_scenario(line: &str) -> bool {
    let line = line.trim_start();
    SCENARIO_KEYWORDS
        .iter()
        .filter_map(|keyword| line.strip_prefix(keyword))
        .any(|rest| rest.trim_start().starts_with(':'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::Change;
    use crate::config::Lane;
    use crate::git::tests::{Folder, Repo};
    use crate::rules;

    const PROTECTED: &str = "[protected]\nall = [\"gdd/**\"]\nbuilder_build = [\"features/**\"]\n";

    const ZONES: &str = r#"
default = "yellow"
[[zone]]
color = "green"
paths = ["docs/**"]
[[zone]]
color = "red"
paths = ["src/storage/**"]
"#;

    const NO_FAILURES: [Failure; 0] = [];

    /// A project in a git repository, its rules on `main`, and change `c1` started on its
    /// branch; with what a gate reads.
    struct Started {
        repo: Repo,
        record: Record,
        zones: ZoneMap,
        rules: Rules,
        settings: Settings,
        commands: Commands,
    }

    /// The project with change `c1`, which plans `paths`, and the gate commands `commands`.
    fn started(paths: &[&str], commands: &str) -> Started {
        let repo = Repo::new();
        repo.write("gdd/rules.toml", PROTECTED);
        repo.commit();
        let start = repo.start("c1");
        let record = Record {
            plan: Change {
                paths: paths.iter().map(|p| p.to_string()).collect(),
                min_color: None,
                resumable: false,
            },
            build_start: Some(start),
            decided_color: None,
            approved: Hashes::new(),
            features: vec![],
        };
        Started {
            repo,
            record,
            zones: ZoneMap::from_toml(ZONES).unwrap(),
            rules: Rules::from_toml(PROTECTED).unwrap(),
            settings: Settings::from_toml(PROTECTED).unwrap(),
            commands: Commands::from_toml(commands).unwrap(),
        }
    }

    impl Started {
        fn project(&self) -> Project<'_> {
            Project {
                root: self.repo.root(),
                state: self.repo.root(),
                zones: &self.zones,
                rules: &self.rules,
                settings: &self.settings,
                commands: &self.commands,
            }
        }

        fn run_with(&self, gate: Gate, role: &str, snapshot: Option<&Hashes>) -> Report {
            let report = run(&self.project(), gate, "c1", &self.record, role, snapshot).unwrap();
            assert_eq!(report.ok, report.failures.is_empty(), "{report:?}");
            report
        }

        fn run(&self, gate: Gate, role: &str) -> Report {
            self.run_with(gate, role, None)
        }
    }

    fn listed(commands: &Commands, gate: Gate) -> Vec<(&str, &str)> {
        let pairs = commands.of(gate).iter();
        pairs.map(|(n, c)| (n.as_str(), c.as_str())).collect()
    }

    fn names(report: &Report) -> Vec<&str> {
        report.failures.iter().map(|f| f.name.as_str()).collect()
    }

    fn failure(name: &str, tail: &str) -> Failure {
        Failure {
            name: name.to_string(),
            exit: None,
            tail: tail.to_string(),
        }
    }

    #[test]
    fn each_gate_has_its_name_its_phase_and_its_report() {
        assert_eq!(Gate::G2.to_string(), "G2");
        assert_eq!(Gate::G3.to_string(), "G3");
        assert_eq!(Gate::G2.phase(), "2-build");
        assert_eq!(Gate::G3.phase(), "3-integrate");
        assert_eq!(Gate::G2.report_path("c1"), ".gdd/changes/c1/g2.json");
        assert_eq!(Gate::G3.report_path("c1"), ".gdd/changes/c1/g3.json");
    }

    #[test]
    fn commands_are_the_text_of_the_gate_sections() {
        let commands = Commands::from_toml(
            "install = \"x\"\n[g2]\nunit = \"cargo test\"\nlint = \"true\"\ntypes = \"\"\n\
             blank = \"  \"\nflag = true\n[g3]\nmutation = \"m\"\n[release]\nbuild = \"b\"\n",
        )
        .unwrap();
        let g2 = [("lint", "true"), ("unit", "cargo test")];
        assert_eq!(listed(&commands, Gate::G2), g2);
        assert_eq!(listed(&commands, Gate::G3), [("mutation", "m")]);
        assert_eq!(Commands::from_toml("").unwrap(), Commands::default());
        for wrong in ["g2 = \"x\"", "[g2", "[g3]\nunit = "] {
            let result = Commands::from_toml(wrong);
            assert!(matches!(result, Err(config::Error::Toml(_))), "{wrong:?}");
        }
    }

    #[test]
    fn rules_without_colors_take_the_colors_of_the_tool() {
        let rules = Rules::from_toml(PROTECTED).unwrap();
        let expected = Rules {
            table: rules::default_table(),
            scenarios_required: true,
            harness: vec!["gdd/**".into()],
        };
        assert_eq!(rules, expected);
        let rules = Rules::from_toml("[acceptance]\ndir = \"specs\"\nrequired = false\n").unwrap();
        assert_eq!(rules.table.acceptance.dir, "specs");
        assert!(!rules.scenarios_required);
        assert_eq!(rules.harness, Vec::<String>::new());
        let shipped = rules::default_table().rule(Color::Yellow, Lane::Full);
        assert_eq!(rules.table.rule(Color::Yellow, Lane::Full), shipped);
        let text = "[color.red.full]\nmutation = false\n[acceptance]\nrequired = true\n";
        let rules = Rules::from_toml(text).unwrap();
        assert_eq!(rules.table, RulesTable::from_toml(text).unwrap());
        assert!(rules.scenarios_required);
        assert!(!rules.table.rule(Color::Yellow, Lane::Full).mutation);
        let unknown = Rules::from_toml("[color.purple]\n");
        assert_eq!(unknown, Err(config::Error::UnknownColor("purple".into())));
        for wrong in [
            "[acceptance]\nrequired = 1\n",
            "x = ",
            "color = 1\n",
            "[protected]\nall = \"gdd/**\"\n",
        ] {
            let result = Rules::from_toml(wrong);
            assert!(matches!(result, Err(config::Error::Toml(_))), "{wrong:?}");
        }
    }

    #[test]
    fn a_gate_passes_when_its_commands_and_path_guard_pass() {
        let commands = "[g2]\nunit = \"echo unit-ran\"\nlint = \"true\"\ntypes = \"\"\n\
                        [g3]\nacceptance = \"false\"\n";
        let project = started(&["src/app.rs"], commands);
        let report = project.run(Gate::G2, "builder");
        let expected = Report {
            ok: true,
            gate: Gate::G2,
            color: Color::Yellow,
            passed: vec!["lint".into(), "unit".into(), "path-guard".into()],
            skipped: vec![],
            failures: vec![],
            out_of_scope: vec![],
            added_exceptions: vec![],
            duration_s: report.duration_s,
        };
        assert_eq!(report, expected);
    }

    #[test]
    fn a_failing_command_fails_the_gate_with_its_code_and_its_output() {
        let commands = "[g2]\nlint = \"true\"\nunit = \"sleep 0.2; echo boom-42; exit 3\"\n";
        let project = started(&["src/app.rs"], commands);
        let report = project.run(Gate::G2, "builder");
        assert!(!report.ok);
        assert_eq!(report.passed, ["lint", "path-guard"]);
        let [failure] = report.failures.as_slice() else {
            panic!("one failure: {report:?}");
        };
        assert_eq!(failure.name, "unit");
        assert_eq!(failure.tail, "boom-42\n");
        let exit = failure.exit.expect("how the command ended");
        assert_eq!((exit.ok, exit.code), (false, 3));
        assert!((0.2..10.0).contains(&exit.duration_s), "{exit:?}");
        assert!(report.duration_s >= exit.duration_s, "{report:?}");
    }

    #[test]
    fn a_command_that_cannot_start_fails_with_the_reason() {
        let mut report = started(&[], "").run(Gate::G2, "builder");
        report.failures.clear();
        report.command("unit", Err(io::Error::other("no shell")));
        let expected = failure("unit", "cannot run sh: no shell");
        assert_eq!(report.failures, [expected]);
        assert_eq!(report.duration_s, 0.0);
    }

    #[test]
    fn a_gate_without_commands_is_not_configured() {
        let commands = "[g2]\nlint = \"\"\nunit = \"  \"\nother = 1\n";
        let project = started(&["src/app.rs"], commands);
        let report = project.run(Gate::G2, "builder");
        assert_eq!(report.passed, ["path-guard"]);
        assert_eq!(
            report.failures,
            [failure("G2 not configured", NOT_CONFIGURED)]
        );
        let report = started(&["src/app.rs"], "").run(Gate::G3, "builder");
        assert_eq!(names(&report), ["G3 not configured", "scenarios"]);
    }

    #[test]
    fn commands_know_the_change_its_color_its_scenarios_and_what_it_touched() {
        let show = r#"[g2]
show = 'printf "%s|%s|%s|%s" "$GDD_CHANGE" "$GDD_COLOR" "$GDD_DIFF_FILES" "$GDD_FEATURES"; exit 1'
"#;
        let project = started(&["src/app.rs", "docs/b.md"], show);
        let shown = |project: &Started| project.run(Gate::G2, "builder").failures[0].tail.clone();
        assert_eq!(shown(&project), "c1|yellow|src/app.rs\ndocs/b.md|");
        project.repo.write("src/storage/db.rs", "x\n");
        project.repo.write("features/a.feature", "Feature: a\n");
        assert_eq!(
            shown(&project),
            "c1|red|features/a.feature\nsrc/storage/db.rs|features/a.feature"
        );
    }

    #[test]
    fn mutation_testing_runs_only_when_the_change_needs_it() {
        let commands = "[g3]\nacceptance = \"true\"\n\
                        mutation = \"true\"\nmutation_slow = \"exit 1\"\n";
        let green = started(&["docs/a.md"], commands);
        let report = green.run(Gate::G3, "builder");
        assert_eq!(report.color, Color::Green);
        assert_eq!(report.skipped, ["mutation", "mutation_slow"]);
        assert_eq!(report.passed[0], "acceptance");
        let mut yellow = started(&["src/app.rs"], commands);
        let report = yellow.run(Gate::G3, "builder");
        assert_eq!(report.skipped, Vec::<String>::new());
        assert_eq!(report.passed[..2], ["acceptance", "mutation"]);
        assert_eq!(names(&report), ["mutation_slow", "scenarios"]);
        // With colors of its own, the rules file decides.
        yellow.rules = Rules::from_toml("[color.yellow.full]\nmutation = false\n").unwrap();
        let report = yellow.run(Gate::G3, "builder");
        assert_eq!(report.skipped, ["mutation", "mutation_slow"]);
    }

    #[test]
    fn g3_counts_the_scenarios_and_fails_without_them() {
        let commands = "[g2]\nunit = \"true\"\n[g3]\nacceptance = \"true\"\n";
        let project = started(&["src/app.rs"], commands);
        let report = project.run(Gate::G3, "builder");
        assert_eq!(report.failures, [failure("scenarios", NO_SCENARIOS)]);
        let feature = "Feature: a\n  Scenario: one\n  Scenario: two\n";
        project.repo.write("features/a.feature", feature);
        let report = project.run(Gate::G3, "builder");
        let passed = ["acceptance", "path-guard", "scenarios (2)", "exceptions"];
        assert_eq!(report.passed, passed);
        assert_eq!(report.failures, NO_FAILURES);
        let report = project.run(Gate::G2, "builder");
        assert_eq!(report.passed, ["unit", "path-guard"]);
    }

    #[test]
    fn zero_scenarios_pass_g3_only_when_not_required_and_the_change_has_none() {
        let mut project = started(&["src/app.rs"], "[g3]\nacceptance = \"true\"\n");
        project.rules = Rules::from_toml("[acceptance]\nrequired = false\n").unwrap();
        let report = project.run(Gate::G3, "builder");
        let passed = ["acceptance", "path-guard", "scenarios (0)", "exceptions"];
        assert_eq!(report.passed, passed);
        project.record.features = vec!["features/hidden/a.feature".into()];
        assert_eq!(names(&project.run(Gate::G3, "builder")), ["scenarios"]);
        project.record.features.clear();
        let feature = "features/a.feature".to_string();
        project.record.plan.paths.push(feature);
        assert_eq!(names(&project.run(Gate::G3, "builder")), ["scenarios"]);
    }

    #[test]
    fn the_scenarios_of_the_change_include_the_ones_its_record_names() {
        let show = "[g2]\nshow = 'printf \"%s\" \"$GDD_FEATURES\"; exit 1'\n";
        let mut project = started(&["src/app.rs"], show);
        let named = ["features/hidden/b.feature", "features/a.feature"];
        project.record.features = named.map(String::from).to_vec();
        project.repo.write("features/a.feature", "Feature: a\n");
        project.repo.write("features/c.feature", "Feature: c\n");
        let shown = &project.run(Gate::G2, "builder").failures[0].tail;
        let features = "features/a.feature\nfeatures/c.feature\nfeatures/hidden/b.feature";
        assert_eq!(shown, features);
    }

    #[test]
    fn a_change_with_scenarios_of_its_own_takes_the_full_lane() {
        let commands = "[g3]\nacceptance = \"true\"\nmutation = \"true\"\n";
        let mut project = started(&["src/app.rs"], commands);
        let lanes = "[color.yellow.fast]\nmutation = false\n[color.yellow.full]\nmutation = true\n";
        project.rules = Rules::from_toml(lanes).unwrap();
        project.record.plan.resumable = true;
        assert_eq!(project.run(Gate::G3, "builder").skipped, ["mutation"]);
        project.record.features = vec!["features/hidden/a.feature".into()];
        let report = project.run(Gate::G3, "builder");
        assert_eq!(report.skipped, Vec::<String>::new());
        assert_eq!(report.passed[..2], ["acceptance", "mutation"]);
    }

    #[test]
    fn the_scenarios_are_counted_where_the_rules_say() {
        let mut project = started(&["src/app.rs"], "[g3]\nacceptance = \"true\"\n");
        project.rules = Rules::from_toml("[acceptance]\ndir = \"specs\"\n").unwrap();
        project.repo.write("features/a.feature", "Scenario: a\n");
        project.repo.write("specs/b.feature", "Scenario: b\n");
        let report = project.run(Gate::G3, "builder");
        assert_eq!(report.passed[2], "scenarios (1)");
        project.rules = Rules::from_toml("[acceptance]\ndir = \"specs/b.feature\"\n").unwrap();
        let report = project.run(Gate::G3, "builder");
        assert_eq!(names(&report), ["scenarios"]);
        let reason = "cannot count the scenarios in specs/b.feature: ";
        assert!(report.failures[0].tail.starts_with(reason), "{report:?}");
    }

    #[test]
    fn g3_refuses_an_exception_without_a_reason_and_names_the_ones_the_change_adds() {
        let commands = "[g2]\nunit = \"true\"\n[g3]\nacceptance = \"true\"\n";
        let project = started(&["src/app.rs"], commands);
        project.repo.write("features/a.feature", "Scenario: a\n");
        let osv = "[[IgnoredVulns]]\nid = \"A\"\nreason = \"r\"\n";
        project.repo.write(exceptions::OSV, osv);
        let report = project.run(Gate::G3, "builder");
        let passed = ["acceptance", "path-guard", "scenarios (1)", "exceptions"];
        assert_eq!(report.passed, passed);
        let added = ["exception added: A · osv-scanner.toml · no end date · r"];
        assert_eq!(report.added_exceptions, added);
        let report = project.run(Gate::G2, "builder");
        assert_eq!(report.passed, ["unit", "path-guard"]);
        assert_eq!(report.added_exceptions, Vec::<String>::new());
        let deny = "[advisories]\nignore = [\"B\"]\n";
        project.repo.write(exceptions::DENY, deny);
        let report = project.run(Gate::G3, "builder");
        let refused = failure("exceptions", "deny.toml: B has no reason");
        assert_eq!(report.failures, [refused]);
        assert_eq!(report.added_exceptions.len(), 2, "{report:?}");
        project.repo.write(exceptions::OSV, "[[IgnoredVulns]\n");
        let report = project.run(Gate::G3, "builder");
        assert_eq!(names(&report), ["exceptions"]);
        let invalid = "osv-scanner.toml: invalid TOML: ";
        assert!(report.failures[0].tail.starts_with(invalid), "{report:?}");
        assert_eq!(report.added_exceptions, Vec::<String>::new());
    }

    #[test]
    fn path_guard_runs_inside_the_gate_for_the_role() {
        let project = started(&["src/app.rs"], "[g2]\nlint = \"touch ran\"\n");
        project.repo.write("gdd/rules.toml", "# touched\n");
        let report = project.run(Gate::G2, "builder");
        let guarded = format!("gdd/rules.toml: protected path for builder\n{NOT_RUN}");
        assert_eq!(report.failures, [failure("path-guard", &guarded)]);
        project.repo.write("features/a.feature", "Feature: a\n");
        let report = project.run(Gate::G2, BUILD_ROLE);
        let guarded = format!(
            "features/a.feature: protected path for builder-build\n\
             gdd/rules.toml: protected path for builder-build\n{NOT_RUN}"
        );
        assert_eq!(report.failures, [failure("path-guard", &guarded)]);
        assert_eq!(report.passed, ["scope"]);
        assert!(!project.repo.root().join("ran").exists());
    }

    #[test]
    fn the_commands_run_only_once_path_guard_clears_them() {
        // Rewritten commands could put their file back as they ran: Path Guard comes first.
        let commands = "[g2]\nunit = \"rm gdd/commands.toml\"\n";
        let project = started(&["src/app.rs"], commands);
        project.repo.write("gdd/commands.toml", commands);
        let report = project.run(Gate::G2, "builder");
        let guarded = format!("gdd/commands.toml: protected path for builder\n{NOT_RUN}");
        assert_eq!(report.failures, [failure("path-guard", &guarded)]);
        assert_eq!(report.passed, Vec::<String>::new());
        assert!(project.repo.root().join("gdd/commands.toml").exists());
    }

    #[test]
    fn path_guard_judges_again_what_the_commands_wrote() {
        let commands = "[g2]\nunit = \"echo x > gdd/rules.toml\"\n";
        let project = started(&["src/app.rs"], commands);
        let report = project.run(Gate::G2, "builder");
        assert_eq!(report.passed, ["unit"]);
        let guarded = "gdd/rules.toml: protected path for builder";
        assert_eq!(report.failures, [failure("path-guard", guarded)]);
    }

    #[test]
    fn path_guard_judges_from_the_snapshot_it_gets() {
        let project = started(&["src/app.rs"], "[g2]\nlint = \"true\"\n");
        project.repo.write("features/a.feature", "Feature: a\n");
        let root = project.repo.root();
        let before = guard::snapshot(root, "c1", &project.record).unwrap();
        assert!(project.run_with(Gate::G2, BUILD_ROLE, Some(&before)).ok);
        assert_eq!(names(&project.run(Gate::G2, BUILD_ROLE)), ["path-guard"]);
    }

    #[test]
    fn build_mode_also_checks_the_scope_and_keeps_what_it_accepted() {
        let project = started(&["src/app.rs"], "[g2]\nlint = \"true\"\n");
        project.repo.write("docs/x.md", "x\n");
        let report = project.run(Gate::G2, BUILD_ROLE);
        assert!(report.ok, "{report:?}");
        assert_eq!(report.passed, ["lint", "path-guard", "scope"]);
        assert_eq!(report.out_of_scope, ["docs/x.md"]);
        project.repo.write("src/storage/db.rs", "x\n");
        let report = project.run(Gate::G2, BUILD_ROLE);
        assert_eq!(report.color, Color::Red);
        assert_eq!(names(&report), ["scope"]);
        let outside = "src/storage/db.rs: outside the package and red (the change is yellow)";
        assert!(report.failures[0].tail.starts_with(outside), "{report:?}");
        assert_eq!(report.out_of_scope, ["docs/x.md"]);
        let report = project.run(Gate::G2, "builder");
        assert!(report.ok, "{report:?}");
        assert_eq!(report.passed, ["lint", "path-guard"]);
        assert_eq!(report.out_of_scope, Vec::<String>::new());
    }

    #[test]
    fn a_check_that_cannot_judge_fails_with_the_reason() {
        let mut project = started(&["src/app.rs"], "[g2]\nlint = \"true\"\n");
        project.record.build_start = None;
        let report = project.run(Gate::G2, BUILD_ROLE);
        assert_eq!(names(&report), ["path-guard", "scope"]);
        let reason = "Change c1 has not been prepared: Path Guard has no starting point.";
        assert_eq!(report.failures[0].tail, format!("{reason}\n{NOT_RUN}"));
        project.record.build_start = Some("0123abcd".into());
        let report = project.run(Gate::G2, BUILD_ROLE);
        assert_eq!(names(&report), ["path-guard", "scope"]);
        for failure in &report.failures {
            assert!(failure.tail.starts_with("git diff "), "{failure:?}");
        }
    }

    #[test]
    fn a_gate_that_cannot_read_what_the_change_touched_is_an_error() {
        let mut project = started(&["src/app.rs"], "[g2]\nlint = \"touch ran\"\n");
        project.settings = Settings::from_toml("main_branch = \"trunk\"").unwrap();
        let result = run(
            &project.project(),
            Gate::G2,
            "c1",
            &project.record,
            "builder",
            None,
        );
        assert!(matches!(result, Err(Error::Git(git::Error::Failed(_)))));
        assert!(!project.repo.root().join("ran").exists());
    }

    #[test]
    fn as_json_a_report_names_its_gate_and_color_and_only_commands_have_an_exit() {
        let report = Report {
            ok: false,
            gate: Gate::G3,
            color: Color::Red,
            passed: vec!["lint".into()],
            skipped: vec!["mutation".into()],
            failures: vec![
                Failure {
                    name: "unit".into(),
                    exit: Some(Exit {
                        ok: false,
                        code: 3,
                        duration_s: 1.5,
                    }),
                    tail: "boom".into(),
                },
                failure("scope", "x"),
            ],
            out_of_scope: vec!["docs/a.md".into()],
            added_exceptions: vec![],
            duration_s: 9.0,
        };
        assert_eq!(
            serde_json::to_string(&report).unwrap(),
            concat!(
                r#"{"ok":false,"gate":"G3","color":"red","passed":["lint"],"#,
                r#""skipped":["mutation"],"failures":["#,
                r#"{"name":"unit","ok":false,"code":3,"duration_s":1.5,"tail":"boom"},"#,
                r#"{"name":"scope","tail":"x"}],"out_of_scope":["docs/a.md"]}"#
            )
        );
        let added = Report {
            failures: vec![],
            added_exceptions: vec!["exception added: A · deny.toml".into()],
            ..report
        };
        let json = serde_json::to_string(&added).unwrap();
        let end = concat!(
            r#""out_of_scope":["docs/a.md"],"#,
            r#""added_exceptions":["exception added: A · deny.toml"]}"#
        );
        assert!(json.ends_with(end), "{json}");
    }

    #[test]
    fn the_log_line_says_how_the_gate_ended_and_names_what_failed() {
        let mut report = Report {
            ok: true,
            gate: Gate::G2,
            color: Color::Yellow,
            passed: vec!["lint".into()],
            skipped: vec![],
            failures: vec![],
            out_of_scope: vec![],
            added_exceptions: vec![],
            duration_s: 0.1 + 0.2,
        };
        let entry = log::Entry {
            change: "c1".into(),
            phase: "2-build".into(),
            gate: "G2".into(),
            role: "machine".into(),
            agent: "checks".into(),
            result: "ok".into(),
            duration_s: 0.3,
            note: String::new(),
        };
        assert_eq!(report.log_entry("c1"), entry);
        report.ok = false;
        report.gate = Gate::G3;
        report.failures = vec![failure("unit", "x"), failure("path-guard", "y")];
        let entry = report.log_entry("c2");
        assert_eq!(entry.change, "c2");
        assert_eq!(entry.result, "ko");
        assert_eq!(entry.phase, "3-integrate");
        assert_eq!(entry.gate, "G3");
        assert_eq!(entry.note, "unit, path-guard");
        report.failures = vec![failure(&"é".repeat(400), "x")];
        assert_eq!(report.log_entry("c2").note, "é".repeat(300));
    }

    #[test]
    fn a_report_is_saved_next_to_the_record_and_logged() {
        let project = started(&["src/app.rs"], "[g2]\nlint = \"true\"\n");
        let root = project.repo.root();
        let report = project.run(Gate::G2, "builder");
        save(root, "c1", &report).unwrap();
        let saved = fs::read_to_string(root.join(".gdd/changes/c1/g2.json")).unwrap();
        let pretty = serde_json::to_string_pretty(&report).unwrap();
        assert_eq!(saved, format!("{pretty}\n"));
        let text = fs::read_to_string(root.join(log::PATH)).unwrap();
        let records = log::parse(&text).unwrap();
        let [line] = records.as_slice() else {
            panic!("one log line: {records:?}");
        };
        assert_eq!(line.text("gate"), "G2");
        assert_eq!(line.text("agent"), "checks");
        assert_eq!(line.text("result"), "ok");
        let report = project.run(Gate::G3, "builder");
        fs::create_dir_all(root.join(".gdd/changes/c1/g3.json")).unwrap();
        let err = save(root, "c1", &report).unwrap_err();
        let start = "cannot write .gdd/changes/c1/g3.json: ";
        assert!(err.starts_with(start), "{err}");
        let text = fs::read_to_string(root.join(log::PATH)).unwrap();
        assert_eq!(log::parse(&text).unwrap().len(), 1);
    }

    #[test]
    fn durations_are_kept_to_a_tenth_of_a_second() {
        assert_eq!(tenths(0.0), 0.0);
        assert_eq!(tenths(0.24), 0.2);
        assert_eq!(tenths(0.25), 0.3);
        assert_eq!(tenths(12.345), 12.3);
        assert_eq!(tenths(0.1 + 0.2), 0.3);
    }

    #[test]
    fn a_failure_keeps_the_lines_that_report_failures_then_the_end() {
        assert_eq!(kept("short\noutput\n"), "short\noutput\n");
        assert_eq!(kept(""), "");
        let numbers: String = (1..=5_000).map(|n| format!("{n}\n")).collect();
        let output = format!("FAILED step-7\n{numbers}");
        let tail = kept(&output);
        let (head, end) = tail.split_once("\n--- end of output ---\n").unwrap();
        assert_eq!(head, "FAILED step-7");
        assert_eq!(end, &output[output.len() - 2_500..]);
        assert!(end.ends_with("\n5000\n"));
    }

    #[test]
    fn each_kind_of_failure_line_is_kept() {
        let filler = "ok\n".repeat(1_000);
        for line in [
            "   ✘  Then the gate passes",
            "test a ... FAILED",
            "Failed to compile",
            "1 failed",
            "thread 'main' panicked at src/a.rs:1:1:",
            "error[E0425]: cannot find value",
            "Error: bad",
            "Traceback (most recent call last):",
        ] {
            let tail = kept(&format!("{line}\n{filler}"));
            let head = format!("{line}\n--- end of output ---\n");
            assert!(tail.starts_with(&head), "{line:?}");
        }
        for line in ["all good", "  error: indented", "FAIL 1", "no errors"] {
            let output = format!("{line}\n{filler}");
            assert_eq!(kept(&output), output[output.len() - 2_500..], "{line:?}");
        }
    }

    #[test]
    fn at_most_forty_failure_lines_are_kept_and_the_end_counts_characters() {
        let output: String = (1..=50).map(|n| format!("FAILED {n}\n")).collect();
        let tail = kept(&output);
        let (head, end) = tail.split_once("\n--- end of output ---\n").unwrap();
        assert_eq!(head.lines().count(), 40);
        assert!(head.ends_with("\nFAILED 40"), "{head}");
        assert_eq!(end, output);
        assert_eq!(kept(&"é".repeat(3_000)), "é".repeat(2_500));
        assert_eq!(kept(&"é".repeat(2_500)), "é".repeat(2_500));
    }

    #[test]
    fn scenarios_are_counted_in_every_feature_file_of_the_folder() {
        let folder = Folder::new();
        let features = folder.0.join("features");
        assert_eq!(count_scenarios(&features).unwrap(), 0);
        folder.write(
            "features/a.feature",
            "Feature: a\n  Scenario: one\n  Scenario Outline: two\n    Scenarios:\n  \
             # Scenario: not one\n  Scenario : three\n",
        );
        folder.write(
            "features/deep/b.feature",
            "Característica: b\n  Escenario: uno\n\tEsquema del escenario: dos\n",
        );
        folder.write("features/c.txt", "Scenario: not a feature file\n");
        folder.write("features/d.feature.bak", "Scenario: nor this\n");
        assert_eq!(count_scenarios(&features).unwrap(), 5);
        assert!(count_scenarios(&features.join("a.feature")).is_err());
    }

    #[test]
    fn errors_say_what_went_wrong() {
        let from_git = Error::from(git::Error::Revision("-x".into()));
        let revision = "\"-x\" is not a revision git can take";
        assert_eq!(from_git.to_string(), revision);
        let from_config = Error::from(config::Error::UnknownColor("blue".into()));
        assert_eq!(
            from_config.to_string(),
            "unknown color \"blue\" (use green, yellow or red)"
        );
    }
}
