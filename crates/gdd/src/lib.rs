//! The `gdd` command line: it reads the words of a command, runs it on the project and says
//! what happened. The method itself lives in `gdd-core`.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use clap::{Args, CommandFactory, Parser, Subcommand};
use gdd_core::agents::{self, models::Models, models::Roles};
use gdd_core::change::{self, Decision, Record};
use gdd_core::classify::classify_with;
use gdd_core::gates::{self, Gate, approve, prepare, release};
use gdd_core::git::Git;
use gdd_core::guard::{self, Hashes, Settings, Verdict};
use gdd_core::repo::merge;
use gdd_core::status::{self, Status};
use gdd_core::{Classification, Color, Report, RulesTable, ZoneMap, inbox, log};

/// File that marks the root of a GDD project.
const PROJECT: &str = "gdd/project.toml";
/// Zone map of the project.
const ZONES: &str = "gdd/zones.toml";
/// Rules table of the project, with the protected paths of Path Guard.
const RULES: &str = guard::RULES;
/// Gate commands of the project.
const COMMANDS: &str = gates::COMMANDS;
/// Where the changes keep their records, from the project root.
const CHANGES: &str = ".gdd/changes";
/// The kinds of work a change can be.
const TYPES: [&str; 6] = [
    "Feature",
    "Hotfix",
    "Refactor",
    "Migration",
    "Spike",
    "No-code",
];

/// What a run of `gdd` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// Exit code: 0 when the command did its job, 1 when it failed.
    pub code: u8,
    /// What it printed for the user.
    pub stdout: String,
    /// Why it failed.
    pub stderr: String,
}

impl Outcome {
    fn success(stdout: String) -> Outcome {
        Outcome {
            code: 0,
            stdout,
            stderr: String::new(),
        }
    }

    fn failure(stderr: String) -> Outcome {
        Outcome {
            code: 1,
            stdout: String::new(),
            stderr,
        }
    }

    /// What a command printed for the user, on a line of its own, with its exit code.
    fn printed(text: String, code: u8) -> Outcome {
        Outcome {
            code,
            stdout: format!("{text}\n"),
            stderr: String::new(),
        }
    }
}

/// GDD · Gate-Driven Development: phases, gates and coding agents, from the terminal.
#[derive(Debug, Parser)]
#[command(name = "gdd", version = gdd_core::VERSION)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Tokens, cost and time of the agent calls in the log.
    Report {
        /// Fields to group the calls by, separated by commas (change, phase, model, role, agent...).
        #[arg(long, default_value = "role")]
        by: String,
        /// Count only the records of this change.
        #[arg(long)]
        change: Option<String>,
        /// Print the groups as a JSON list.
        #[arg(long)]
        json: bool,
    },
    /// Color, lane and gates of a change, from its plan and what it touched, as JSON.
    Classify {
        /// Id of the change: its record is .gdd/changes/<id>/change.json.
        change: String,
    },
    /// Path Guard: what a change touched since its build started that a role must not touch,
    /// as JSON.
    Guard {
        /// Id of the change: its record is .gdd/changes/<id>/change.json.
        change: String,
        /// Role whose protected paths count (builder-build, builder-test, reviewer...).
        #[arg(long, default_value = "builder")]
        role: String,
        /// Exit with code 1 when Path Guard fails.
        #[arg(long)]
        strict: bool,
        /// Count only what changed since the last `gdd snapshot`.
        #[arg(long)]
        since_snapshot: bool,
    },
    /// Records what a change touched as it is now, so `gdd guard --since-snapshot` judges only
    /// what changes after.
    Snapshot {
        /// Id of the change: its record is .gdd/changes/<id>/change.json.
        change: String,
    },
    /// Paths a change touched outside its plan: accepted when their color is not higher than
    /// the change's, as JSON.
    Scope {
        /// Id of the change: its record is .gdd/changes/<id>/change.json.
        change: String,
    },
    /// Loop Gate: the G2 commands of gdd/commands.toml, Path Guard and, in build mode, the
    /// scope. Saves the result next to the change, logs it and prints it as JSON.
    G2(GateArgs),
    /// Merge Gate: the G3 commands of gdd/commands.toml, Path Guard and the scenario count.
    /// Saves the result next to the change, logs it and prints it as JSON.
    G3(GateArgs),
    /// Creates a change: its record and, when it has none, its branch change/<id> from the main
    /// branch. Logs it and prints its classification as JSON.
    NewChange(NewChangeArgs),
    /// Records a gate decision: G0 (by the machines), the red zones (by a human), or the plan,
    /// G1, G2, G3 or G4 of a change. Approving G1 seals the scenario files of the change.
    /// Builders and Reviewers cannot record anything.
    Gate(DecisionArgs),
    /// Leaves a note on a change for its Decision Inbox.
    Note(NoteArgs),
    /// The Decision Inbox of a change: what is open for the human. Also saved in
    /// .gdd/changes/<id>/inbox.md.
    Inbox {
        /// Id of the change: its record is .gdd/changes/<id>/change.json.
        change: String,
    },
    /// Where the project stands: G0, red zones, self-test, scenarios, changes and log.
    Status,
    /// Checks that a change may enter Build (G0, red zones, G1, plan, model families, its
    /// branch) and starts it. Prints its decision, with the model of each role, as JSON.
    Prepare {
        /// Id of the change: its record is .gdd/changes/<id>/change.json.
        change: String,
    },
    /// Parks a change: stops it with a message for the human in its Decision Inbox. Always
    /// exits with 1.
    Park {
        /// Id of the change: its record is .gdd/changes/<id>/change.json.
        change: String,
        /// Why; its words are joined with single spaces.
        #[arg(required = true, trailing_var_arg = true)]
        reason: Vec<String>,
    },
    /// Merges a change whose G3 is open into the main branch, with its record in the merge
    /// commit, then builds the artifact and deploys it to pre. Prints the deploy as JSON.
    Merge {
        /// Id of the change: its record is .gdd/changes/<id>/change.json.
        change: String,
    },
    /// A release step of gdd/commands.toml: pre (tests and demo), promote (to production, after
    /// G4) or observe (rolls the change back when production gets worse). Prints it as JSON.
    Release {
        /// The step.
        #[arg(value_parser = release::STEPS)]
        step: String,
        /// Id of the change: its record is .gdd/changes/<id>/change.json.
        change: String,
    },
}

/// What `gdd g2` and `gdd g3` take.
#[derive(Debug, Args)]
struct GateArgs {
    /// Id of the change: its record is .gdd/changes/<id>/change.json.
    change: String,
    /// Role Path Guard judges (builder-build also gets the scope check).
    #[arg(long, default_value = "builder")]
    role: String,
    /// Exit with code 1 when the gate fails.
    #[arg(long)]
    strict: bool,
}

/// What `gdd new-change` takes.
#[derive(Debug, Args)]
struct NewChangeArgs {
    /// Id of the change: lowercase letters, digits and hyphens.
    change: String,
    /// What the change is about, in a few words.
    #[arg(long)]
    title: String,
    /// The kind of work.
    #[arg(long = "type", default_value = "Feature", value_parser = TYPES)]
    kind: String,
    /// Paths the change plans to touch; patterns are allowed.
    #[arg(long, num_args = 0..)]
    paths: Vec<String>,
    /// The change can be resumed after an interruption (a condition of the fast lane).
    #[arg(long)]
    resumable: bool,
    /// Declared minimum color: it raises the color, never lowers it.
    #[arg(long, value_parser = parse_color)]
    min_color: Option<Color>,
}

/// What `gdd gate` takes.
#[derive(Debug, Args)]
struct DecisionArgs {
    /// The gate.
    #[arg(value_parser = approve::GATES)]
    gate: String,
    /// ok or ko; approve and reject say the same.
    decision: String,
    /// Id of the change, for the plan, G1, G2, G3 and G4.
    change: Option<String>,
    /// Who decides: human or machines.
    #[arg(long, default_value = "human")]
    by: String,
    /// What to read about the decision.
    #[arg(long, default_value = "")]
    note: String,
}

/// What `gdd note` takes.
#[derive(Debug, Args)]
struct NoteArgs {
    /// Id of the change: its record is .gdd/changes/<id>/change.json.
    change: String,
    /// needs (a decision for the human), decided (taken without them) or parked.
    #[arg(value_parser = change::NOTE_KINDS)]
    kind: String,
    /// What the note says; its words are joined with single spaces.
    #[arg(required = true, trailing_var_arg = true)]
    text: Vec<String>,
}

/// A color named on the command line.
fn parse_color(name: &str) -> Result<Color, String> {
    name.parse().map_err(|e: gdd_core::Error| e.to_string())
}

impl Command {
    /// Runs the command on the project at `root`, for the role `role`: what it prints and its
    /// exit code.
    fn run(self, root: &Path, role: Option<&str>) -> Result<Outcome, String> {
        let mut code = 0;
        let text = match self {
            Command::G2(args) => {
                let (text, code) = run_gate(root, Gate::G2, &args)?;
                return Ok(Outcome::printed(text, code));
            }
            Command::G3(args) => {
                let (text, code) = run_gate(root, Gate::G3, &args)?;
                return Ok(Outcome::printed(text, code));
            }
            Command::Park { change, reason } => {
                let message = change::park(root, &change, &reason.join(" "))?;
                return Ok(Outcome::failure(format!("{message}\n")));
            }
            Command::Report { by, change, json } => report(root, &by, change.as_deref(), json)?,
            Command::Classify { change } => classify(root, &change)?,
            Command::Guard {
                change,
                role,
                strict,
                since_snapshot,
            } => {
                let verdict = path_guard(root, &change, &role, since_snapshot)?;
                code = u8::from(strict && !verdict.ok);
                serde_json::to_string(&verdict).map_err(|e| e.to_string())?
            }
            Command::Snapshot { change } => snapshot(root, &change)?,
            Command::Scope { change } => scope(root, &change)?,
            Command::NewChange(args) => new_change(root, &args)?,
            Command::Gate(args) => gate(root, &args, role)?,
            Command::Note(args) => note(root, &args)?,
            Command::Inbox { change } => decision_inbox(root, &change)?,
            Command::Status => project_status(root)?,
            Command::Prepare { change } => prepare_change(root, &change)?,
            Command::Merge { change } => merge_change(root, &change)?,
            Command::Release { step, change } => release_change(root, &step, &change)?,
        };
        Ok(Outcome::printed(text, code))
    }
}

/// Runs `gdd` with `args`, the words after the program name, from the folder `cwd`, for the
/// role in the environment variable `GDD_ROLE` (see [`run_as`]).
pub fn run(args: &[String], cwd: &Path) -> Outcome {
    let role = std::env::var("GDD_ROLE").ok();
    run_as(args, cwd, role.as_deref())
}

/// Runs `gdd` with `args`, the words after the program name, from the folder `cwd`, for the
/// role `role` (`None` for a person): a Builder or a Reviewer cannot record gate decisions.
/// Help and version go to the standard output; every error goes to the standard error, with
/// exit code 1. `gdd guard --strict` also exits with 1 when Path Guard fails, and
/// `gdd g2 --strict` and `gdd g3 --strict` when the gate fails. `gdd park` always exits with 1,
/// its message on the standard error.
pub fn run_as(args: &[String], cwd: &Path, role: Option<&str>) -> Outcome {
    let words = std::iter::once("gdd").chain(args.iter().map(String::as_str));
    let cli = match Cli::try_parse_from(words) {
        Ok(cli) => cli,
        Err(e) if e.use_stderr() => return Outcome::failure(e.to_string()),
        Err(e) => return Outcome::success(e.to_string()),
    };
    let Some(command) = cli.command else {
        return Outcome::success(Cli::command().render_help().to_string());
    };
    match project_root(cwd).and_then(|root| command.run(&root, role)) {
        Ok(outcome) => outcome,
        Err(message) => Outcome::failure(format!("error: {message}\n")),
    }
}

/// The root of the project: the nearest folder, from `cwd` upwards, with `gdd/project.toml`.
fn project_root(cwd: &Path) -> Result<PathBuf, String> {
    cwd.ancestors()
        .find(|folder| folder.join(PROJECT).is_file())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            format!(
                "not a GDD project: no {PROJECT} in {} or the folders above it",
                cwd.display()
            )
        })
}

/// `gdd report`: the agent calls of the log added up by the fields `by`.
fn report(root: &Path, by: &str, change: Option<&str>, json: bool) -> Result<String, String> {
    let text = read(root, log::PATH)?.unwrap_or_default();
    let records = log::parse(&text).map_err(|e| format!("{}, {e}", log::PATH))?;
    let by: Vec<String> = by.split(',').map(str::to_string).collect();
    let report = Report::new(&records, &by, change);
    if json {
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())
    } else {
        Ok(report.to_string())
    }
}

/// `gdd classify`: the decision for a change (see [`decide`]).
fn classify(root: &Path, id: &str) -> Result<String, String> {
    let decided = decide(root, id)?;
    serde_json::to_string_pretty(&decided.decision(id)).map_err(|e| e.to_string())
}

/// What the tool decides for a change: its classification, its kind of work, the model of each
/// role and the Builder rounds against G2.
struct Decided {
    classification: Classification,
    kind: String,
    models: Models,
    roles: Roles,
    retries: u32,
}

impl Decided {
    /// The decision for change `id`, as `gdd classify` prints it.
    fn decision<'a>(&'a self, id: &'a str) -> Decision<'a> {
        Decision {
            change: id,
            classification: &self.classification,
            kind: &self.kind,
            roles: &self.roles,
            retries: self.retries,
        }
    }
}

/// The decision for change `id`, from its record, the zone map, the rules table and the models.
/// Its planned paths count, the ones it touched in git (see [`guard::touched`]) and the
/// scenario files its record names. A missing zone map, rules table or models file says nothing.
fn decide(root: &Path, id: &str) -> Result<Decided, String> {
    let text = change::read_record(root, id)?;
    let path = change::record_path(id);
    let invalid = |e: serde_json::Error| format!("{path}: {e}");
    let record = Record::from_json(&text).map_err(invalid)?;
    let kind = change::kind(&text).map_err(invalid)?;
    let zones = load_zones(root)?;
    let table = load_table(root)?;
    let touched = guard::touched(root, id, &load_settings(root)?).map_err(|e| e.to_string())?;
    let mut plan = record.plan;
    plan.paths.extend(touched);
    let classification =
        classify_with(&zones, &table, &plan, &record.features).map_err(|e| e.to_string())?;
    let models = load_models(root)?;
    let roles = models.roles(classification.color);
    let rules = read(root, RULES)?.unwrap_or_default();
    let retries = agents::builder_retries(&rules).map_err(|e| format!("{RULES}: {e}"))?;
    Ok(Decided {
        classification,
        kind,
        models,
        roles,
        retries,
    })
}

/// `gdd guard`: Path Guard for `role` on change `id`, with the protected paths of the rules
/// table, now and where the build started, judged from the snapshot of [`guard_snapshot`].
fn path_guard(root: &Path, id: &str, role: &str, since_snapshot: bool) -> Result<Verdict, String> {
    let record = load_record(root, id)?;
    let settings = load_settings(root)?;
    let before = guard_snapshot(root, id, role, since_snapshot)?;
    guard::path_guard(root, id, &record, &settings, role, before.as_ref())
        .map_err(|e| e.to_string())
}

/// The snapshot Path Guard judges `role` from on change `id`, if there is one: build mode is
/// judged from the one taken after test mode (what test mode wrote is not build mode's doing);
/// with `since_snapshot`, other roles from the last `gdd snapshot`. Otherwise, none: everything
/// the change touched since its build started counts.
fn guard_snapshot(
    root: &Path,
    id: &str,
    role: &str,
    since_snapshot: bool,
) -> Result<Option<Hashes>, String> {
    if role == gates::BUILD_ROLE {
        read_snapshot(root, &change::build_snapshot_path(id))
    } else if since_snapshot {
        read_snapshot(root, &change::snapshot_path(id))
    } else {
        Ok(None)
    }
}

/// `gdd g2` and `gdd g3`: runs `gate` on the change, writes in its record the paths the scope
/// check accepted, saves the report next to it and logs the run. The report as JSON, and exit
/// code 1 when the gate fails and the run is strict.
fn run_gate(root: &Path, gate: Gate, args: &GateArgs) -> Result<(String, u8), String> {
    let id = args.change.as_str();
    let record = load_record(root, id)?;
    let config = GateConfig::load(root)?;
    let snapshot = guard_snapshot(root, id, &args.role, false)?;
    let project = config.project(root);
    let report = gates::run(&project, gate, id, &record, &args.role, snapshot.as_ref())
        .map_err(|e| e.to_string())?;
    save_out_of_scope(root, id, &report.out_of_scope)?;
    gates::save(root, id, &report)?;
    let printed = serde_json::to_string(&report).map_err(|e| e.to_string())?;
    Ok((printed, u8::from(args.strict && !report.ok)))
}

/// What the gates read from a project: the zone map, the rules file and the gate commands.
struct GateConfig {
    zones: ZoneMap,
    settings: Settings,
    rules: gates::Rules,
    commands: gates::Commands,
}

impl GateConfig {
    /// The configuration of the project at `root`; a missing file says nothing.
    fn load(root: &Path) -> Result<GateConfig, String> {
        let zones = load_zones(root)?;
        let settings = load_settings(root)?;
        let rules = read(root, RULES)?.unwrap_or_default();
        let rules = gates::Rules::from_toml(&rules).map_err(|e| format!("{RULES}: {e}"))?;
        let commands = read(root, COMMANDS)?.unwrap_or_default();
        let commands =
            gates::Commands::from_toml(&commands).map_err(|e| format!("{COMMANDS}: {e}"))?;
        Ok(GateConfig {
            zones,
            settings,
            rules,
            commands,
        })
    }

    /// What a gate reads from the project at `root`.
    fn project<'a>(&'a self, root: &'a Path) -> gates::Project<'a> {
        gates::Project {
            root,
            zones: &self.zones,
            rules: &self.rules,
            settings: &self.settings,
            commands: &self.commands,
        }
    }
}

/// `gdd prepare`: checks that change `id` may enter Build and starts it (see
/// [`prepare::prepare`]).
fn prepare_change(root: &Path, id: &str) -> Result<String, String> {
    let decided = decide(root, id)?;
    let config = GateConfig::load(root)?;
    let project = config.project(root);
    let zones = read(root, ZONES)?.unwrap_or_default();
    let setup = prepare::Setup {
        project: &project,
        zones: &zones,
        models: &decided.models,
    };
    prepare::prepare(&setup, id, &decided.decision(id))
}

/// `gdd merge`: merges change `id`, classified as it is now, into the main branch (see
/// [`merge::merge`]). What follows the merge reads its configuration as the merge left it.
fn merge_change(root: &Path, id: &str) -> Result<String, String> {
    let decided = decide(root, id)?;
    let settings = load_settings(root)?;
    let project = merge::Project {
        root,
        main: &settings.main_branch,
    };
    merge::merge(&project, id, &decided.classification)
}

/// `gdd release`: the release step `step` of change `id` (see [`release`]). Promoting asks
/// whether the change needs the human G4 as it is classified now.
fn release_change(root: &Path, step: &str, id: &str) -> Result<String, String> {
    let steps = load_steps(root)?;
    match step {
        "pre" => release::pre(root, id, &steps),
        "promote" => {
            let decided = decide(root, id)?;
            release::promote(root, id, &steps, decided.classification.g4)
        }
        "observe" => release::observe(root, id, &steps),
        other => Err(format!("unknown release step {other:?}")),
    }
}

/// Writes `accepted`, the paths outside the plan the scope check accepted, in the record of
/// change `id`, when it does not say them already.
fn save_out_of_scope(root: &Path, id: &str, accepted: &[String]) -> Result<(), String> {
    let path = change::record_path(id);
    let text = read(root, &path)?.unwrap_or_default();
    match change::with_out_of_scope(&text, accepted) {
        Ok(Some(record)) => write(root, &path, &record),
        Ok(None) => Ok(()),
        Err(e) => Err(format!("{path}: {e}")),
    }
}

/// `gdd snapshot`: records the hashes of what change `id` touched since its build started.
fn snapshot(root: &Path, id: &str) -> Result<String, String> {
    let record = load_record(root, id)?;
    let hashes = guard::snapshot(root, id, &record).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(&hashes).map_err(|e| e.to_string())?;
    write(root, &change::snapshot_path(id), &format!("{text}\n"))?;
    Ok(r#"{"ok":true}"#.to_string())
}

/// `gdd scope`: the scope check of change `id`, with the zone map and the rules table.
fn scope(root: &Path, id: &str) -> Result<String, String> {
    let record = load_record(root, id)?;
    let zones = load_zones(root)?;
    let table = load_table(root)?;
    let checked = guard::scope(root, id, &record, &zones, &table).map_err(|e| e.to_string())?;
    serde_json::to_string(&checked).map_err(|e| e.to_string())
}

/// `gdd new-change`: the record of a new change `id` and, when it has none, its branch from the
/// main branch; logged. Its classification as JSON. A change that exists already is an error.
fn new_change(root: &Path, args: &NewChangeArgs) -> Result<String, String> {
    let id = args.change.as_str();
    change::check_id(id)?;
    let path = change::record_path(id);
    if root.join(&path).exists() {
        return Err(format!("Change {id} already exists."));
    }
    let main = load_settings(root)?.main_branch;
    let git = Git::new(root);
    let branch = change::branch(id);
    let created = match git.exists(&branch) {
        Ok(true) => Ok(()),
        Ok(false) => git.create_branch(&branch, &main),
        Err(e) => Err(e),
    };
    created.map_err(|e| e.to_string())?;
    let ts = log::now();
    let new = change::New {
        id,
        title: &args.title,
        kind: &args.kind,
        paths: &args.paths,
        resumable: args.resumable,
        min_color: args.min_color,
        created: &ts,
    };
    let record = change::new_record(&new).map_err(|e| e.to_string())?;
    write(root, &path, &record)?;
    let entry = log::Entry {
        change: id.to_string(),
        phase: "1-discover".to_string(),
        gate: "-".to_string(),
        role: "orchestrator".to_string(),
        agent: "-".to_string(),
        result: "change-created".to_string(),
        duration_s: 0.0,
        note: args.title.clone(),
    };
    let line = entry.line(&ts).map_err(|e| e.to_string())?;
    log::append(root, &line).map_err(|e| format!("cannot write {}: {e}", log::PATH))?;
    classify(root, id)
}

/// `gdd gate`: records a gate decision for the role `role` (see [`approve::record`]).
fn gate(root: &Path, args: &DecisionArgs, role: Option<&str>) -> Result<String, String> {
    let gate = match args.gate.parse::<approve::Gate>() {
        Ok(gate) => gate,
        Err(e) => return Err(e.to_string()),
    };
    let zones = read(root, ZONES)?.unwrap_or_default();
    let settings = load_settings(root)?;
    let table = load_table(root)?;
    let project = approve::Project {
        root,
        zones: &zones,
        settings: &settings,
        acceptance: &table.acceptance,
    };
    let request = approve::Request {
        gate,
        decision: &args.decision,
        change: args.change.as_deref(),
        by: &args.by,
        note: &args.note,
    };
    approve::record(&project, &request, role).map_err(|e| e.to_string())
}

/// `gdd note`: adds a note to the record of a change, for its Decision Inbox.
fn note(root: &Path, args: &NoteArgs) -> Result<String, String> {
    let id = args.change.as_str();
    let text = change::read_record(root, id)?;
    let ts = log::now();
    let words = args.text.join(" ");
    let note = change::Note {
        kind: &args.kind,
        text: &words,
        ts: &ts,
    };
    let path = change::record_path(id);
    let record = change::with_note(&text, &note).map_err(|e| format!("{path}: {e}"))?;
    write(root, &path, &record)?;
    Ok("noted".to_string())
}

/// `gdd inbox`: the Decision Inbox of change `id`, also saved next to its record exactly as it
/// is printed.
fn decision_inbox(root: &Path, id: &str) -> Result<String, String> {
    let text = change::read_record(root, id)?;
    inbox::save(root, id, &text)
}

/// `gdd status`: where the project stands, and its changes.
fn project_status(root: &Path) -> Result<String, String> {
    let project = read(root, PROJECT)?.unwrap_or_default();
    let basics = status::Basics::from_toml(&project).map_err(|e| format!("{PROJECT}: {e}"))?;
    let zones = read(root, ZONES)?.unwrap_or_default();
    let dir = load_table(root)?.acceptance.dir;
    let scenarios = gates::count_scenarios(&root.join(&dir)).map_err(|e| e.to_string());
    let selftest = read(root, approve::SELFTEST_PATH)?;
    let log_lines = read(root, log::PATH)?.map(|text| text.lines().count());
    let status = Status {
        basics,
        brief: root.join(approve::BRIEF).is_file(),
        g0: approve::g0_open(root).map_err(|e| e.to_string())?,
        zones: approve::zones_state(root, &zones).map_err(|e| e.to_string())?,
        selftest: selftest.and_then(|text| status::selftest_summary(&text)),
        scenarios_dir: dir,
        scenarios,
        changes: change_lines(root)?,
        log_lines,
    };
    Ok(status.to_string())
}

/// The line of each change in the status, in the order of their folders; a folder without a
/// record is not a change.
fn change_lines(root: &Path) -> Result<Vec<String>, String> {
    let entries = match fs::read_dir(root.join(CHANGES)) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("cannot read {CHANGES}: {e}")),
    };
    let mut ids: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    ids.sort();
    let mut lines = Vec::new();
    for id in ids {
        if let Some(text) = read(root, &change::record_path(&id))? {
            lines.push(status::change_line(&id, &text));
        }
    }
    Ok(lines)
}

/// The record of change `id` (see [`change::read_record`]).
fn load_record(root: &Path, id: &str) -> Result<Record, String> {
    let text = change::read_record(root, id)?;
    let path = change::record_path(id);
    Record::from_json(&text).map_err(|e| format!("{path}: {e}"))
}

/// The hashes of the snapshot at `path`; `None` when there is none.
fn read_snapshot(root: &Path, path: &str) -> Result<Option<Hashes>, String> {
    match read(root, path)? {
        Some(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| format!("{path}: {e}")),
        None => Ok(None),
    }
}

/// The zone map of the project.
fn load_zones(root: &Path) -> Result<ZoneMap, String> {
    let text = read(root, ZONES)?.unwrap_or_default();
    ZoneMap::from_toml(&text).map_err(|e| format!("{ZONES}: {e}"))
}

/// The rules table the project classifies its changes with, the one the gates use (see
/// [`RulesTable::of_project`]).
fn load_table(root: &Path) -> Result<RulesTable, String> {
    let text = read(root, RULES)?.unwrap_or_default();
    RulesTable::of_project(&text).map_err(|e| format!("{RULES}: {e}"))
}

/// The models of the project: who plays each role.
fn load_models(root: &Path) -> Result<Models, String> {
    let path = agents::models::PATH;
    let text = read(root, path)?.unwrap_or_default();
    Models::from_toml(&text).map_err(|e| format!("{path}: {e}"))
}

/// The release commands of the project.
fn load_steps(root: &Path) -> Result<release::Steps, String> {
    let text = read(root, COMMANDS)?.unwrap_or_default();
    release::Steps::from_toml(&text).map_err(|e| format!("{COMMANDS}: {e}"))
}

/// What the rules file says to Path Guard: the main branch and the protected paths.
fn load_settings(root: &Path) -> Result<Settings, String> {
    let text = read(root, RULES)?.unwrap_or_default();
    Settings::from_toml(&text).map_err(|e| format!("{RULES}: {e}"))
}

/// The text of a file of the project, or `None` when it does not exist.
fn read(root: &Path, path: &str) -> Result<Option<String>, String> {
    match fs::read_to_string(root.join(path)) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("cannot read {path}: {e}")),
    }
}

/// Writes `text` to a file of the project, creating its folder.
fn write(root: &Path, path: &str, text: &str) -> Result<(), String> {
    let file = root.join(path);
    file.parent()
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| fs::write(&file, text))
        .map_err(|e| format!("cannot write {path}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A project in a temporary folder, removed when dropped.
    struct Project(PathBuf);

    impl Project {
        fn new() -> Project {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "gdd-cli-test-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&path);
            let project = Project(path);
            project.write(PROJECT, "name = \"demo\"\n");
            project
        }

        fn write(&self, path: &str, text: &str) {
            let path = self.0.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }

        fn run(&self, command: &str) -> Outcome {
            run_in(&self.0, command)
        }
    }

    impl Drop for Project {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn words(command: &str) -> Vec<String> {
        command.split_whitespace().map(str::to_string).collect()
    }

    /// Runs `command` from `cwd` as a person, whatever `GDD_ROLE` says.
    fn run_in(cwd: &Path, command: &str) -> Outcome {
        run_as(&words(command), cwd, None)
    }

    fn json(out: &Outcome) -> serde_json::Value {
        assert_eq!(out.code, 0, "{out:?}");
        serde_json::from_str(&out.stdout).unwrap()
    }

    const LOG: &str = concat!(
        r#"{"change": "a", "role": "builder-build", "model": "opus", "tokens_in": 1200, "tokens_out": 20, "cost_usd": 0.5, "duration_s": 4}"#,
        "\n",
        r#"{"change": "b", "role": "reviewer", "model": "gpt-x", "tokens_in": 10, "tokens_out": 2}"#,
        "\n",
        r#"{"change": "a", "gate": "G1", "role": "human", "result": "ok"}"#,
        "\n"
    );

    #[test]
    fn the_version_names_the_tool() {
        let out = run_in(Path::new("/"), "--version");
        assert_eq!(
            out,
            Outcome::success(format!("{}\n", gdd_core::version_line()))
        );
    }

    #[test]
    fn help_goes_to_the_standard_output() {
        for command in ["--help", "", "report --help"] {
            let out = run_in(Path::new("/"), command);
            assert_eq!((out.code, out.stderr.as_str()), (0, ""), "{command:?}");
            assert!(out.stdout.contains("Usage: gdd"), "{command:?}: {out:?}");
        }
        let out = run_in(Path::new("/"), "");
        for command in ["report", "classify", "guard", "snapshot", "scope"] {
            assert!(out.stdout.contains(command), "{command}: {out:?}");
        }
        for command in ["new-change", "gate", "note", "inbox", "status"] {
            assert!(out.stdout.contains(command), "{command}: {out:?}");
        }
        for command in ["prepare", "park", "merge", "release"] {
            assert!(out.stdout.contains(command), "{command}: {out:?}");
        }
        for gate in ["g2", "Loop Gate", "g3", "Merge Gate"] {
            assert!(out.stdout.contains(gate), "{gate}: {out:?}");
        }
    }

    #[test]
    fn a_wrong_command_is_an_error() {
        for command in [
            "frobnicate",
            "classify",
            "report --by",
            "report --nope",
            "guard",
            "guard save --role",
            "snapshot",
            "scope",
            "g2",
            "g3 --strict",
            "g2 save --role",
            "new-change c1",
            "new-change c1 --title",
            "new-change c1 --title x --type Other",
            "new-change c1 --title x --min-color purple",
            "gate",
            "gate G9 ok",
            "gate G1",
            "note c1 needs",
            "note c1 maybe x",
            "inbox",
            "status extra",
            "prepare",
            "park",
            "park c1",
            "merge",
            "release",
            "release pre",
            "release deploy c1",
        ] {
            let out = run_in(Path::new("/"), command);
            assert_eq!((out.code, out.stdout.as_str()), (1, ""), "{command:?}");
            assert!(out.stderr.starts_with("error: "), "{command:?}: {out:?}");
        }
    }

    #[test]
    fn outside_a_project_commands_fail() {
        let folder = Project::new();
        fs::remove_file(folder.0.join(PROJECT)).unwrap();
        let out = folder.run("report");
        assert_eq!(out.code, 1);
        assert!(out.stdout.is_empty());
        assert!(
            out.stderr
                .starts_with("error: not a GDD project: no gdd/project.toml in "),
            "{out:?}"
        );
        assert!(
            out.stderr.ends_with(" or the folders above it\n"),
            "{out:?}"
        );
    }

    #[test]
    fn the_root_is_the_nearest_folder_with_the_project_file() {
        let outer = Project::new();
        outer.write(log::PATH, LOG);
        outer.write("inner/gdd/project.toml", "");
        fs::create_dir_all(outer.0.join("inner/deep/er")).unwrap();
        assert_eq!(
            project_root(&outer.0.join("inner/deep/er")),
            Ok(outer.0.join("inner"))
        );
        assert_eq!(project_root(&outer.0), Ok(outer.0.clone()));
        let out = run_in(&outer.0.join("inner/deep"), "report");
        assert_eq!(out.stdout, "No agent calls in the log yet.\n");
        // A folder named like the project file is not a project.
        fs::create_dir_all(outer.0.join("dir/gdd/project.toml")).unwrap();
        assert_eq!(project_root(&outer.0.join("dir")), Ok(outer.0.clone()));
    }

    #[test]
    fn report_prints_the_text_report() {
        let project = Project::new();
        project.write(log::PATH, LOG);
        let out = project.run("report");
        assert_eq!(
            out,
            Outcome::success(
                "role -> calls · tokens in · cache · out · cost USD · seconds\n\
                 builder-build -> 1 · 1,200 · 0 · 20 · 0.5000 · 4\n\
                 reviewer -> 1 · 10 · 0 · 2 · 0.0000 · 0\n\
                 TOTAL -> in 1,210 · cache 0 · out 22 · 0.5000 USD\n\
                 Human stops logged: 1\n"
                    .to_string()
            )
        );
    }

    #[test]
    fn report_groups_by_the_fields_asked_and_one_change() {
        let project = Project::new();
        project.write(log::PATH, LOG);
        let out = project.run("report --by change,model --change b");
        assert_eq!(out.code, 0);
        assert!(out.stdout.starts_with("change · model -> calls"), "{out:?}");
        assert!(out.stdout.contains("\nb · gpt-x -> 1 · 10 · 0 · 2 · "));
        assert!(!out.stdout.contains("opus"));
        assert!(out.stdout.ends_with("Human stops logged: 0\n"));
    }

    #[test]
    fn report_as_json() {
        let project = Project::new();
        project.write(log::PATH, LOG);
        let entries = json(&project.run("report --json --by model"));
        assert_eq!(
            entries,
            serde_json::json!([
                {"model": "gpt-x", "calls": 1, "tokens_in": 10, "tokens_cache": 0, "tokens_out": 2, "cost_usd": 0.0, "duration_s": 0.0},
                {"model": "opus", "calls": 1, "tokens_in": 1200, "tokens_cache": 0, "tokens_out": 20, "cost_usd": 0.5, "duration_s": 4.0},
            ])
        );
        assert_eq!(
            json(&project.run("report --json --change zzz")),
            serde_json::json!([])
        );
    }

    #[test]
    fn report_without_a_log_has_no_calls() {
        let project = Project::new();
        let out = project.run("report");
        assert_eq!(
            out,
            Outcome::success("No agent calls in the log yet.\n".into())
        );
    }

    #[test]
    fn a_broken_log_line_is_an_error_that_names_it() {
        let project = Project::new();
        project.write(log::PATH, &format!("{LOG}{{\"tokens_in\": \"x\"}}\n"));
        let out = project.run("report");
        assert_eq!(out.code, 1);
        assert!(out.stdout.is_empty());
        assert!(
            out.stderr
                .starts_with("error: .gdd/log.jsonl, line 4: invalid type"),
            "{out:?}"
        );
    }

    #[test]
    fn a_log_that_cannot_be_read_is_an_error() {
        let project = Project::new();
        fs::create_dir_all(project.0.join(log::PATH)).unwrap();
        let out = project.run("report");
        assert_eq!(out.code, 1);
        assert!(
            out.stderr
                .starts_with("error: cannot read .gdd/log.jsonl: "),
            "{out:?}"
        );
    }

    const ZONE_MAP: &str = "default = \"yellow\"\n[[zone]]\ncolor = \"red\"\npaths = [\"src/storage/**\"]\nreason = \"Storage\"\n";

    fn project_with_change(record: &str) -> Project {
        let project = Project::new();
        project.write(ZONES, ZONE_MAP);
        project.write(".gdd/changes/save/change.json", record);
        project
    }

    #[test]
    fn classify_prints_the_decision_as_json() {
        let project = project_with_change(
            r#"{"id": "save", "paths": ["src/storage/a.rs", "docs/x.md"], "resumable": true, "min_color": null}"#,
        );
        project.write(RULES, "[color.red.full]\nmutation = false\n");
        let out = project.run("classify save");
        assert_eq!(out.code, 0, "{out:?}");
        assert!(
            out.stdout
                .starts_with("{\n  \"change\": \"save\",\n  \"color\": \"red\",\n")
        );
        assert!(out.stdout.ends_with("}\n"));
        assert_eq!(
            json(&out),
            serde_json::json!({
                "change": "save",
                "color": "red",
                "lane": "full",
                "reasons": [
                    "docs/x.md -> yellow (no zone: default color)",
                    "src/storage/a.rs -> red (Storage)",
                ],
                "g1": false,
                "features": [],
                "tests_first": false,
                "mutation": false,
                "reviewer": true,
                "g4": true,
                "human_plan": true,
                "type": "Feature",
                "builder": {},
                "builder_test": {},
                "reviewer_model": {},
                "orchestrator": {},
                "retries": 3,
            })
        );
    }

    const MODELS: &str = "[builder.yellow]\nintegration = \"claude\"\nmodel = \"sonnet\"\n\
                          [builder.red]\nintegration = \"claude\"\nmodel = \"opus\"\n\
                          [reviewer]\nintegration = \"codex\"\nmodel = \"gpt-6\"\n\
                          [families]\n\"opus\" = \"anthropic\"\n\"sonnet\" = \"anthropic\"\n\
                          \"gpt\" = \"openai\"\n";

    #[test]
    fn classify_names_the_model_of_each_role_and_the_retries() {
        let project = project_with_change(
            r#"{"type": "Spike", "paths": ["src/storage/a.rs"], "features": ["features/x.feature"]}"#,
        );
        project.write("gdd/models.toml", MODELS);
        project.write(RULES, "[limits]\nbuilder_retries = 5\n");
        let decision = json(&project.run("classify save"));
        assert_eq!(decision["type"], "Spike");
        let opus = serde_json::json!({"integration": "claude", "model": "opus"});
        assert_eq!(decision["builder"], opus);
        assert_eq!(decision["reviewer_model"]["model"], "gpt-6");
        assert_eq!(decision["builder_test"], serde_json::json!({}));
        assert_eq!(decision["retries"], 5);
        // The scenarios the record names put the change in G1.
        assert_eq!(decision["g1"], true);
        let features = serde_json::json!(["features/x.feature"]);
        assert_eq!(decision["features"], features);
        project.write("gdd/models.toml", "[reviewer\n");
        let out = project.run("classify save");
        let start = "error: gdd/models.toml: invalid TOML";
        assert!(out.stderr.starts_with(start), "{out:?}");
        project.write("gdd/models.toml", MODELS);
        project.write(RULES, "[limits]\nbuilder_retries = \"5\"\n");
        let out = project.run("classify save");
        let start = "error: gdd/rules.toml: invalid TOML";
        assert!(out.stderr.starts_with(start), "{out:?}");
        project.write(RULES, "");
        project.write(".gdd/changes/save/change.json", r#"{"type": 3}"#);
        let out = project.run("classify save");
        let start = "error: .gdd/changes/save/change.json: invalid type";
        assert!(out.stderr.starts_with(start), "{out:?}");
    }

    #[test]
    fn classify_from_a_subfolder_with_a_minimum_color() {
        let project = project_with_change(r#"{"paths": ["docs/x.md"], "min_color": "red"}"#);
        let cwd = project.0.join("docs/deep");
        fs::create_dir_all(&cwd).unwrap();
        let decision = json(&run_in(&cwd, "classify save"));
        assert_eq!(decision["color"], "red");
        assert_eq!(decision["reasons"][1], "declared minimum color: red");
    }

    #[test]
    fn classify_without_zone_map_or_rules_takes_the_tables_of_the_tool() {
        let project = Project::new();
        project.write(
            ".gdd/changes/save/change.json",
            r#"{"paths": ["a.rs"], "resumable": true}"#,
        );
        let decision = json(&project.run("classify save"));
        assert_eq!(decision["color"], "yellow");
        assert_eq!(decision["lane"], "fast");
        assert_eq!(decision["tests_first"], true);
        assert_eq!(decision["mutation"], true);
        assert_eq!(decision["reviewer"], true);
        assert_eq!(decision["g4"], false);
        // Rules without colors say nothing about them either.
        project.write(RULES, PROTECTED);
        assert_eq!(json(&project.run("classify save")), decision);
    }

    #[test]
    fn classify_an_unknown_change_names_it() {
        let project = Project::new();
        let out = project.run("classify nope");
        assert_eq!(
            out,
            Outcome::failure(
                "error: change nope does not exist: there is no .gdd/changes/nope/change.json\n"
                    .into()
            )
        );
    }

    #[test]
    fn classify_rejects_ids_that_are_not_change_ids() {
        let project = Project::new();
        project.write("x/change.json", "{}");
        for id in ["../x", "Save", "a/b"] {
            let out = project.run(&format!("classify {id}"));
            assert_eq!(
                out,
                Outcome::failure(format!(
                    "error: invalid change id {id:?}: use lowercase letters, digits and hyphens\n"
                ))
            );
        }
    }

    #[test]
    fn classify_names_the_file_that_is_wrong() {
        let project = project_with_change(r#"{"min_color": "purple"}"#);
        let out = project.run("classify save");
        assert_eq!(out.code, 1);
        assert!(
            out.stderr
                .starts_with("error: .gdd/changes/save/change.json: unknown color \"purple\""),
            "{out:?}"
        );

        let project = project_with_change("{}");
        project.write(ZONES, "default = \"blue\"\n");
        let out = project.run("classify save");
        assert_eq!(
            out.stderr,
            "error: gdd/zones.toml: unknown color \"blue\" (use green, yellow or red)\n"
        );

        let project = project_with_change("{}");
        project.write(RULES, "[color.purple]\n");
        let out = project.run("classify save");
        assert_eq!(
            out.stderr,
            "error: gdd/rules.toml: unknown color \"purple\" (use green, yellow or red)\n"
        );
    }

    #[test]
    fn classify_a_record_that_cannot_be_read_is_an_error() {
        let project = Project::new();
        fs::create_dir_all(project.0.join(".gdd/changes/save/change.json")).unwrap();
        let out = project.run("classify save");
        assert!(
            out.stderr
                .starts_with("error: cannot read .gdd/changes/save/change.json: "),
            "{out:?}"
        );
    }

    /// Git settings of the tests: a fixed author, and no hooks or signing from the user's config.
    const GIT_CONFIG: [&str; 8] = [
        "-c",
        "user.name=GDD Test",
        "-c",
        "user.email=test@example.com",
        "-c",
        "commit.gpgsign=false",
        "-c",
        "core.hooksPath=/dev/null",
    ];

    fn git(project: &Project, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .args(GIT_CONFIG)
            .args(args)
            .current_dir(&project.0)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(out.status.success(), "git {args:?}: {stderr}");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    const PROTECTED: &str = "[protected]\nall = [\"gdd/**\"]\nbuilder_test = [\"src/**\"]\n";

    /// A project in a git repository: its files on `main`, then change `save`, which plans
    /// `src/a.rs`, started on its branch.
    fn started_project() -> Project {
        started_with_commands("")
    }

    /// The started project, with the gate commands `commands` on `main`.
    fn started_with_commands(commands: &str) -> Project {
        let project = Project::new();
        project.write(RULES, PROTECTED);
        project.write(ZONES, ZONE_MAP);
        project.write(COMMANDS, commands);
        git(&project, &["init", "-q", "-b", "main"]);
        git(&project, &["add", "-A"]);
        git(&project, &["commit", "-q", "-m", "project"]);
        git(&project, &["checkout", "-q", "-b", "change/save"]);
        let start = git(&project, &["rev-parse", "HEAD"]);
        let record = format!(r#"{{"paths": ["src/a.rs"], "build_start": "{start}"}}"#);
        project.write(".gdd/changes/save/change.json", &record);
        project
    }

    #[test]
    fn classify_counts_the_paths_the_change_touched() {
        let project = started_project();
        assert_eq!(json(&project.run("classify save"))["color"], "yellow");
        project.write("src/storage/db.rs", "x\n");
        let decision = json(&project.run("classify save"));
        assert_eq!(decision["color"], "red");
        assert_eq!(
            decision["reasons"],
            serde_json::json!([
                "src/a.rs -> yellow (no zone: default color)",
                "src/storage/db.rs -> red (Storage)",
            ])
        );
    }

    #[test]
    fn guard_prints_its_verdict_and_fails_the_run_only_when_strict() {
        let project = started_project();
        let out = project.run("guard save --role builder-test --strict");
        assert_eq!(out.code, 0);
        assert_eq!(out.stdout, "{\"ok\":true,\"failures\":[]}\n");
        project.write("src/a.rs", "x\n");
        project.write(ZONES, "default = \"red\"\n");
        let verdict = serde_json::json!({
            "ok": false,
            "failures": [
                "gdd/zones.toml: protected path for builder-test",
                "src/a.rs: protected path for builder-test",
            ],
        });
        assert_eq!(
            json(&project.run("guard save --role builder-test")),
            verdict
        );
        let strict = project.run("guard save --role builder-test --strict");
        assert_eq!((strict.code, strict.stderr.as_str()), (1, ""));
        let printed: serde_json::Value = serde_json::from_str(&strict.stdout).unwrap();
        assert_eq!(printed, verdict);
        let builder = json(&project.run("guard save"));
        assert_eq!(
            builder["failures"],
            serde_json::json!(["gdd/zones.toml: protected path for builder"])
        );
    }

    #[test]
    fn snapshot_records_the_round_and_guard_then_judges_only_what_follows() {
        let project = started_project();
        project.write("src/a.rs", "1\n");
        let out = project.run("snapshot save");
        assert_eq!(out, Outcome::success("{\"ok\":true}\n".into()));
        let path = project.0.join(".gdd/changes/save/snapshot.json");
        let saved = fs::read_to_string(path).unwrap();
        let hash = guard::file_hash(&project.0.join("src/a.rs")).unwrap();
        assert_eq!(saved, format!("{{\n  \"src/a.rs\": \"{hash}\"\n}}\n"));
        project.write("src/b.rs", "1\n");
        let since = json(&project.run("guard save --role builder-test --since-snapshot"));
        assert_eq!(
            since["failures"],
            serde_json::json!(["src/b.rs: protected path for builder-test"])
        );
        let all = json(&project.run("guard save --role builder-test"));
        assert_eq!(all["failures"].as_array().map(Vec::len), Some(2));
        project.write(".gdd/changes/save/snapshot.json", "{not json");
        let out = project.run("guard save --role builder-test --since-snapshot");
        let expected = "error: .gdd/changes/save/snapshot.json: ";
        assert_eq!(out.code, 1);
        assert!(out.stderr.starts_with(expected), "{out:?}");
        assert_eq!(project.run("guard save --role builder-test").code, 0);
    }

    #[test]
    fn guard_holds_when_the_rules_go_away_or_a_file_leaves_the_diff() {
        let project = started_project();
        fs::remove_file(project.0.join(RULES)).unwrap();
        let out = project.run("guard save --role builder-build --strict");
        assert_eq!(out.code, 1, "{out:?}");
        let printed: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
        let failure = "gdd/rules.toml: protected path for builder-build";
        assert_eq!(printed["failures"], serde_json::json!([failure]));
        project.write(RULES, PROTECTED);
        project.write("src/steps.rs", "x\n");
        assert_eq!(project.run("snapshot save").code, 0);
        fs::remove_file(project.0.join("src/steps.rs")).unwrap();
        let out = project.run("guard save --role builder-test --since-snapshot --strict");
        assert_eq!(out.code, 1, "{out:?}");
        let printed: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
        let failure = "src/steps.rs: protected path for builder-test";
        assert_eq!(printed["failures"], serde_json::json!([failure]));
    }

    #[test]
    fn scope_prints_its_verdict_and_never_fails_the_run() {
        let project = started_project();
        let empty = serde_json::json!({"ok": true, "failures": [], "accepted": []});
        assert_eq!(json(&project.run("scope save")), empty);
        project.write("docs/x.md", "x\n");
        project.write("src/storage/db.rs", "x\n");
        let out = project.run("scope save");
        assert_eq!(out.code, 0);
        assert!(out.stdout.starts_with("{\"ok\":false,"), "{out:?}");
        let failure = "src/storage/db.rs: outside the package and red (the change is yellow); \
                       remove it, or the Orchestrator widens the scope";
        let checked = serde_json::json!({
            "ok": false,
            "failures": [failure],
            "accepted": ["docs/x.md"],
        });
        assert_eq!(json(&out), checked);
    }

    #[test]
    fn a_change_never_prepared_fails_guard_and_scope_and_has_no_snapshot() {
        let project = project_with_change(r#"{"paths": ["a.rs"]}"#);
        let guarded = project.run("guard save --strict");
        assert_eq!(guarded.code, 1);
        let printed: serde_json::Value = serde_json::from_str(&guarded.stdout).unwrap();
        let failure = "Change save has not been prepared: Path Guard has no starting point.";
        assert_eq!(
            printed,
            serde_json::json!({"ok": false, "failures": [failure]})
        );
        assert_eq!(json(&project.run("scope save"))["ok"], false);
        assert_eq!(
            project.run("snapshot save"),
            Outcome::failure(
                "error: change save has not been prepared: its record has no build_start\n".into()
            )
        );
    }

    #[test]
    fn guard_scope_and_snapshot_need_a_change_that_exists() {
        let project = Project::new();
        let invalid =
            "error: invalid change id \"Save\": use lowercase letters, digits and hyphens\n";
        let missing =
            "error: change nope does not exist: there is no .gdd/changes/nope/change.json\n";
        for command in ["guard", "scope", "snapshot"] {
            let out = project.run(&format!("{command} Save"));
            assert_eq!(out, Outcome::failure(invalid.into()), "{command}");
            let out = project.run(&format!("{command} nope"));
            assert_eq!(out, Outcome::failure(missing.into()), "{command}");
        }
    }

    #[test]
    fn a_git_failure_fails_the_run() {
        let project = project_with_change(r#"{"paths": [], "build_start": "abc123"}"#);
        let expected = "error: git rev-parse --abbrev-ref HEAD failed";
        for command in ["guard save --strict", "scope save", "snapshot save"] {
            let out = project.run(command);
            assert_eq!((out.code, out.stdout.as_str()), (1, ""), "{command}");
            assert!(out.stderr.starts_with(expected), "{command}: {out:?}");
        }
    }

    #[test]
    fn guard_names_a_rules_file_that_is_wrong() {
        let project = started_project();
        project.write(RULES, "[protected]\nall = \"gdd/**\"\n");
        for command in ["guard save", "classify save", "g2 save"] {
            let out = project.run(command);
            assert_eq!(out.code, 1, "{command}");
            assert!(out.stderr.starts_with("error: gdd/rules.toml: "), "{out:?}");
        }
    }

    /// The records of the log of `project`.
    fn log_records(project: &Project) -> Vec<log::Record> {
        let text = fs::read_to_string(project.0.join(log::PATH)).unwrap();
        log::parse(&text).unwrap()
    }

    #[test]
    fn a_gate_prints_saves_and_logs_its_report() {
        let commands = "[g2]\nlint = \"true\"\nunit = \"echo boom; exit 2\"\n";
        let project = started_with_commands(commands);
        let out = project.run("g2 save");
        assert_eq!((out.code, out.stderr.as_str()), (0, ""), "{out:?}");
        let start = "{\"ok\":false,\"gate\":\"G2\",\"color\":\"yellow\",\"passed\":[\"lint\",";
        assert!(out.stdout.starts_with(start), "{out:?}");
        let printed = json(&out);
        let passed = serde_json::json!(["lint", "path-guard"]);
        assert_eq!(printed["passed"], passed);
        let failure = &printed["failures"][0];
        assert_eq!(failure["name"], "unit");
        assert_eq!(failure["code"], 2);
        assert_eq!(failure["tail"], "boom\n");
        let saved = fs::read_to_string(project.0.join(".gdd/changes/save/g2.json")).unwrap();
        let head = "{\n  \"ok\": false,\n  \"gate\": \"G2\",\n";
        assert!(saved.starts_with(head), "{saved}");
        assert!(saved.ends_with("}\n"));
        let saved: serde_json::Value = serde_json::from_str(&saved).unwrap();
        assert_eq!(saved, printed);
        let records = log_records(&project);
        let [line] = records.as_slice() else {
            panic!("one log line: {records:?}");
        };
        for (field, value) in [
            ("change", "save"),
            ("phase", "2-build"),
            ("gate", "G2"),
            ("role", "machine"),
            ("agent", "checks"),
            ("model", "-"),
            ("result", "ko"),
            ("note", "unit"),
        ] {
            assert_eq!(line.text(field), value, "{field}");
        }
        assert!(line.text("ts").ends_with("+00:00"), "{line:?}");
        let strict = project.run("g2 save --strict");
        assert_eq!((strict.code, strict.stderr.as_str()), (1, ""));
        let printed: serde_json::Value = serde_json::from_str(&strict.stdout).unwrap();
        assert_eq!(printed["ok"], false);
        assert_eq!(log_records(&project).len(), 2);
    }

    #[test]
    fn a_gate_that_passes_exits_with_0_even_when_strict() {
        let project = started_with_commands("[g3]\nacceptance = \"true\"\n");
        project.write("features/a.feature", "Feature: a\n  Scenario: one\n");
        let out = project.run("g3 save --strict");
        assert_eq!(out.code, 0, "{out:?}");
        let printed = json(&out);
        let passed = serde_json::json!(["acceptance", "path-guard", "scenarios (1)"]);
        assert_eq!(printed["passed"], passed);
        let saved = fs::read_to_string(project.0.join(".gdd/changes/save/g3.json")).unwrap();
        let saved: serde_json::Value = serde_json::from_str(&saved).unwrap();
        assert_eq!(saved, printed);
        let records = log_records(&project);
        assert_eq!(records[0].text("gate"), "G3");
        assert_eq!(records[0].text("phase"), "3-integrate");
        assert_eq!(records[0].text("result"), "ok");
        assert_eq!(records[0].text("note"), "");
    }

    #[test]
    fn a_gate_without_a_commands_file_is_not_configured() {
        let project = started_project();
        fs::remove_file(project.0.join(COMMANDS)).unwrap();
        let printed = json(&project.run("g2 save"));
        assert_eq!(printed["failures"][0]["name"], "G2 not configured");
    }

    #[test]
    fn build_mode_writes_what_the_scope_accepted_in_the_record() {
        let project = started_with_commands("[g2]\nlint = \"true\"\n");
        project.write("docs/x.md", "x\n");
        let printed = json(&project.run("g2 save --role builder-build"));
        assert_eq!(printed["ok"], true, "{printed}");
        assert_eq!(printed["out_of_scope"], serde_json::json!(["docs/x.md"]));
        let path = project.0.join(".gdd/changes/save/change.json");
        let text = fs::read_to_string(&path).unwrap();
        let record: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(record["out_of_scope"], serde_json::json!(["docs/x.md"]));
        assert_eq!(Record::from_json(&text).unwrap().plan.paths, ["src/a.rs"]);
        assert!(Record::from_json(&text).unwrap().build_start.is_some());
        let printed = json(&project.run("g2 save"));
        assert_eq!(printed["out_of_scope"], serde_json::json!([]));
        assert_eq!(fs::read_to_string(&path).unwrap(), text);
    }

    #[test]
    fn build_mode_is_judged_from_the_snapshot_taken_after_test_mode() {
        let project = started_with_commands("[g2]\nlint = \"true\"\n");
        project.write("gdd/notes.toml", "x\n");
        let guarded = "gdd/notes.toml: protected path for builder-build";
        let printed = json(&project.run("g2 save --role builder-build"));
        let tail = format!("{guarded}\nThe gate commands did not run: Path Guard must pass first.");
        assert_eq!(printed["failures"][0]["tail"], tail);
        let verdict = json(&project.run("guard save --role builder-build"));
        assert_eq!(verdict["failures"], serde_json::json!([guarded]));
        let record = load_record(&project.0, "save").unwrap();
        let hashes = guard::snapshot(&project.0, "save", &record).unwrap();
        let build = serde_json::to_string(&hashes).unwrap();
        project.write(".gdd/changes/save/snapshot-build.json", &build);
        let printed = json(&project.run("g2 save --role builder-build"));
        assert_eq!(printed["ok"], true, "{printed}");
        let verdict = json(&project.run("guard save --role builder-build"));
        assert_eq!(verdict["ok"], true, "{verdict}");
        let other = json(&project.run("guard save --role builder-test --since-snapshot"));
        assert_eq!(other["ok"], false, "{other}");
        project.write(".gdd/changes/save/snapshot-build.json", "{not json");
        let out = project.run("g2 save --role builder-build");
        let expected = "error: .gdd/changes/save/snapshot-build.json: ";
        assert!(out.stderr.starts_with(expected), "{out:?}");
    }

    #[test]
    fn a_gate_names_what_it_cannot_read_or_write() {
        let project = started_with_commands("[g2\n");
        let out = project.run("g2 save");
        assert_eq!(out.code, 1);
        let expected = "error: gdd/commands.toml: invalid TOML: ";
        assert!(out.stderr.starts_with(expected), "{out:?}");
        let project = started_with_commands("[g2]\nlint = \"true\"\n");
        project.write(RULES, "[color.purple]\n");
        let out = project.run("g2 save");
        let expected = "error: gdd/rules.toml: unknown color";
        assert!(out.stderr.starts_with(expected), "{out:?}");
        let project = started_with_commands("[g2]\nlint = \"true\"\n");
        fs::create_dir_all(project.0.join(".gdd/changes/save/g2.json")).unwrap();
        let out = project.run("g2 save");
        let expected = "error: cannot write .gdd/changes/save/g2.json: ";
        assert!(out.stderr.starts_with(expected), "{out:?}");
        let project = started_with_commands("[g2]\nlint = \"true\"\n");
        fs::create_dir_all(project.0.join(log::PATH)).unwrap();
        let out = project.run("g2 save");
        let expected = "error: cannot write .gdd/log.jsonl: ";
        assert!(out.stderr.starts_with(expected), "{out:?}");
        let project = started_with_commands("[g2]\nlint = \"true\"\n");
        let main = "main_branch = \"trunk\"\n[protected]\nall = [\"gdd/**\"]\n";
        project.write(RULES, main);
        let out = project.run("g3 save");
        assert!(out.stderr.starts_with("error: git diff "), "{out:?}");
    }

    #[test]
    fn a_gate_needs_a_change_that_exists() {
        let project = Project::new();
        for gate in ["g2", "g3"] {
            let out = project.run(&format!("{gate} nope"));
            let missing =
                "error: change nope does not exist: there is no .gdd/changes/nope/change.json\n";
            assert_eq!(out, Outcome::failure(missing.into()), "{gate}");
            let out = project.run(&format!("{gate} Save"));
            assert!(out.stderr.starts_with("error: invalid change id"), "{gate}");
        }
    }

    /// A project in a git repository, its files committed on `main`, without changes.
    fn committed_project() -> Project {
        let project = Project::new();
        project.write(RULES, PROTECTED);
        project.write(ZONES, ZONE_MAP);
        git(&project, &["init", "-q", "-b", "main"]);
        git(&project, &["add", "-A"]);
        git(&project, &["commit", "-q", "-m", "project"]);
        project
    }

    fn record_of(project: &Project, id: &str) -> serde_json::Value {
        let path = project.0.join(change::record_path(id));
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn new_change_creates_its_record_and_its_branch_and_logs_it() {
        let project = committed_project();
        let main = git(&project, &["rev-parse", "main"]);
        let out = project.run("new-change c1 --title Drafts --paths src/storage/db.rs");
        let decision = json(&out);
        assert_eq!(decision["change"], "c1");
        assert_eq!(decision["color"], "red");
        let start = "{\n  \"change\": \"c1\",\n";
        assert!(out.stdout.starts_with(start), "{out:?}");
        assert_eq!(git(&project, &["rev-parse", "change/c1"]), main);
        assert_eq!(git(&project, &["branch", "--show-current"]), "main");
        let record = record_of(&project, "c1");
        assert_eq!(record["id"], "c1");
        assert_eq!(record["title"], "Drafts");
        assert_eq!(record["type"], "Feature");
        let paths = serde_json::json!(["src/storage/db.rs"]);
        assert_eq!(record["paths"], paths);
        assert_eq!(record["resumable"], false);
        assert_eq!(record["min_color"], serde_json::Value::Null);
        assert_eq!(record["state"], "open");
        assert_eq!(record["gates"], serde_json::json!({}));
        assert_eq!(record["inbox"], serde_json::json!([]));
        let records = log_records(&project);
        let [line] = records.as_slice() else {
            panic!("one log line: {records:?}");
        };
        for (field, value) in [
            ("change", "c1"),
            ("phase", "1-discover"),
            ("gate", "-"),
            ("role", "orchestrator"),
            ("agent", "-"),
            ("result", "change-created"),
            ("note", "Drafts"),
        ] {
            assert_eq!(line.text(field), value, "{field}");
        }
        assert_eq!(line.text("ts"), record["created"].as_str().unwrap());
    }

    #[test]
    fn new_change_takes_a_type_paths_a_minimum_color_and_resumable() {
        let project = committed_project();
        let flags = "--type Spike --paths docs/a.md b.md --resumable --min-color red";
        let out = project.run(&format!("new-change c1 --title Docs {flags}"));
        let decision = json(&out);
        assert_eq!(decision["color"], "red");
        let reasons = decision["reasons"].as_array().unwrap();
        assert_eq!(reasons.last().unwrap(), "declared minimum color: red");
        let record = record_of(&project, "c1");
        assert_eq!(record["type"], "Spike");
        let paths = serde_json::json!(["docs/a.md", "b.md"]);
        assert_eq!(record["paths"], paths);
        assert_eq!(record["resumable"], true);
        assert_eq!(record["min_color"], "red");
    }

    #[test]
    fn new_change_never_overwrites_a_change_and_keeps_its_branch() {
        let project = committed_project();
        assert_eq!(project.run("new-change c1 --title First").code, 0);
        let again = project.run("new-change c1 --title Second");
        let exists = Outcome::failure("error: Change c1 already exists.\n".into());
        assert_eq!(again, exists);
        assert_eq!(record_of(&project, "c1")["title"], "First");
        // The branch of a change whose record went away is kept as it is.
        fs::remove_dir_all(project.0.join(".gdd/changes/c1")).unwrap();
        git(&project, &["checkout", "-q", "change/c1"]);
        project.write("x.rs", "x\n");
        git(&project, &["add", "--", "x.rs"]);
        git(&project, &["commit", "-q", "-m", "x"]);
        let tip = git(&project, &["rev-parse", "HEAD"]);
        assert_eq!(project.run("new-change c1 --title Again").code, 0);
        assert_eq!(git(&project, &["rev-parse", "change/c1"]), tip);
        assert_eq!(record_of(&project, "c1")["title"], "Again");
    }

    #[test]
    fn new_change_needs_an_id_that_names_a_change_and_a_repository() {
        let project = committed_project();
        let out = project.run("new-change ../c1 --title Drafts");
        let invalid = change::check_id("../c1").unwrap_err();
        assert_eq!(out, Outcome::failure(format!("error: {invalid}\n")));
        assert_eq!(git(&project, &["branch", "--list"]), "* main");
        assert!(!project.0.join(CHANGES).exists());
        let plain = Project::new();
        let out = plain.run("new-change c1 --title Drafts");
        let start = "error: git rev-parse --verify --quiet change/c1 failed";
        assert!(out.stderr.starts_with(start), "{out:?}");
        assert!(!plain.0.join(CHANGES).exists());
    }

    #[test]
    fn gate_records_the_decisions_on_a_change_but_not_for_builders() {
        let project = committed_project();
        project.run("new-change c1 --title Drafts");
        let out = project.run("gate plan ok c1 --note Looks-fine");
        assert_eq!(out, Outcome::success("plan ok on c1 (human).\n".into()));
        let record = record_of(&project, "c1");
        let plan = &record["gates"]["plan"];
        assert_eq!(plan["decision"], "ok");
        assert_eq!(plan["by"], "human");
        assert_eq!(plan["note"], "Looks-fine");
        let g4 = project.run("gate G4 reject c1 --by machines");
        assert_eq!(g4.stdout, "G4 ko on c1 (machines).\n");
        let maybe = project.run("gate G4 maybe c1");
        let expected = Outcome::failure("error: The decision is ok or ko.\n".into());
        assert_eq!(maybe, expected);
        let refused = "error: A Builder or Reviewer cannot open or close gates.\n";
        let args = words("gate G4 ok c1");
        let builder = run_as(&args, &project.0, Some("builder-build"));
        assert_eq!(builder, Outcome::failure(refused.into()));
        let orchestrator = run_as(&args, &project.0, Some("orchestrator"));
        assert_eq!(orchestrator.stdout, "G4 ok on c1 (human).\n");
        assert_eq!(log_records(&project).len(), 4);
    }

    #[test]
    fn gate_and_status_show_g0_and_the_red_zones() {
        let project = committed_project();
        let selftest = r#"{"ok": true, "summary": "harness 4/4"}"#;
        project.write(".gdd/selftest.json", selftest);
        project.write("docs/brief.md", "# Brief\n");
        let g0 = project.run("gate G0 ok --by machines");
        assert_eq!(g0.stdout, "G0 ok (machines).\n");
        let zones = project.run("gate zones ok");
        assert_eq!(zones.stdout, "Red zones ok (1 red zone(s)).\n");
        project.run("new-change c1 --title Drafts --paths src/a.rs");
        project.run("gate plan ok c1");
        let expected = concat!(
            "Project: demo · language en · platform ?\n",
            "Brief: docs/brief.md\n",
            "G0: ok · G0 ok\n",
            "Red zones: approved · red zones approved\n",
            "Harness self-test: harness 4/4\n",
            "Scenarios in features/: 0\n",
            "Changes:\n",
            "  - c1 · Drafts · open · plan:ok\n",
            "Log: 4 lines in .gdd/log.jsonl (gdd report)\n"
        );
        assert_eq!(project.run("status"), Outcome::success(expected.into()));
        let wider = "[[zone]]\ncolor = \"red\"\npaths = [\"src/**\"]\n";
        project.write(ZONES, wider);
        let changed = project.run("status").stdout;
        let line = "\nRed zones: pending · Red zones changed after their approval";
        assert!(changed.contains(line), "{changed}");
    }

    #[test]
    fn a_new_project_is_pending_everywhere() {
        let project = Project::new();
        let expected = concat!(
            "Project: demo · language en · platform ?\n",
            "Brief: missing (Setup writes it)\n",
            "G0: pending · G0 pending: run the Setup (/speckit-gdd-setup).\n",
            "Red zones: pending · Red zones not approved yet: the Orchestrator proposes them at ",
            "the end of Discover.\n",
            "Harness self-test: not run\n",
            "Scenarios in features/: 0\n",
            "Changes:\n"
        );
        assert_eq!(project.run("status"), Outcome::success(expected.into()));
    }

    #[test]
    fn status_lists_the_change_folders_with_a_record_in_order() {
        let project = Project::new();
        let second = r#"{"id": "b2", "title": "Two"}"#;
        project.write(".gdd/changes/b2/change.json", second);
        let first = r#"{"id": "a1", "state": "parked"}"#;
        project.write(".gdd/changes/a1/change.json", first);
        project.write(".gdd/changes/c3/notes.md", "no record\n");
        project.write(".gdd/changes/README", "not a change\n");
        let out = project.run("status");
        let a1 = "  - a1 ·  · parked · no gates yet";
        let b2 = "  - b2 · Two · open · no gates yet";
        let changes = format!("Changes:\n{a1}\n{b2}\n");
        assert!(out.stdout.ends_with(&changes), "{out:?}");
        project.write("features", "not a folder\n");
        let out = project.run("status");
        let scenarios = "\nScenarios in features/: cannot count them: ";
        assert!(out.stdout.contains(scenarios), "{out:?}");
    }

    #[test]
    fn status_names_what_it_cannot_read() {
        let project = Project::new();
        project.write(CHANGES, "not a folder\n");
        let out = project.run("status");
        let start = "error: cannot read .gdd/changes: ";
        assert!(out.stderr.starts_with(start), "{out:?}");
        project.write(PROJECT, "name = ");
        let out = project.run("status");
        let start = "error: gdd/project.toml: invalid TOML: ";
        assert!(out.stderr.starts_with(start), "{out:?}");
    }

    #[test]
    fn notes_reach_the_decision_inbox_which_is_saved_as_printed() {
        let project = committed_project();
        project.run("new-change c1 --title Drafts");
        let noted = project.run("note c1 needs Pick  a --name");
        assert_eq!(noted, Outcome::success("noted\n".into()));
        project.run("note c1 decided Kept it");
        let out = project.run("inbox c1");
        let expected = concat!(
            "# Decision Inbox · c1 · Drafts\n\n",
            "Color ? · lane ? · state open\n\n",
            "## Needs your decision\n- Pick a --name\n\n",
            "## Decided without you\n- Kept it\n\n",
            "## Parked\n- Nothing.\n"
        );
        assert_eq!(out, Outcome::success(expected.into()));
        let saved = fs::read_to_string(project.0.join(".gdd/changes/c1/inbox.md")).unwrap();
        assert_eq!(saved, expected);
        let record = record_of(&project, "c1");
        assert_eq!(record["inbox"][1]["kind"], "decided");
        let ts = record["inbox"][1]["ts"].as_str().unwrap();
        assert!(ts.ends_with("+00:00"), "{ts}");
    }

    #[test]
    fn notes_and_the_inbox_need_a_record_they_can_read() {
        let project = Project::new();
        let path = ".gdd/changes/c9/change.json";
        let missing = format!("error: change c9 does not exist: there is no {path}\n");
        for command in ["note c9 needs Something", "inbox c9"] {
            let out = project.run(command);
            assert_eq!(out, Outcome::failure(missing.clone()), "{command}");
        }
        project.write(".gdd/changes/c1/change.json", r#"{"inbox": {}}"#);
        let out = project.run("note c1 needs Something");
        let start = "error: .gdd/changes/c1/change.json: the field \"inbox\"";
        assert!(out.stderr.starts_with(start), "{out:?}");
        project.write(".gdd/changes/c1/change.json", "{not json");
        let out = project.run("inbox c1");
        let start = "error: .gdd/changes/c1/change.json: ";
        assert!(out.stderr.starts_with(start), "{out:?}");
    }

    #[test]
    fn park_stops_the_change_and_says_so_on_the_standard_error() {
        let project = committed_project();
        project.run("new-change c1 --title App");
        let out = project.run("park c1 G2 still  red --after 3");
        let message =
            "Change c1 parked: G2 still red --after 3. Message in .gdd/changes/c1/inbox.md\n";
        assert_eq!(out, Outcome::failure(message.into()));
        assert_eq!(record_of(&project, "c1")["state"], "parked");
        let inbox = fs::read_to_string(project.0.join(".gdd/changes/c1/inbox.md")).unwrap();
        let parked = "## Parked\n- G2 still red --after 3";
        assert!(inbox.contains(parked), "{inbox}");
        let out = project.run("park c9 Why");
        let missing = "error: change c9 does not exist: there is no .gdd/changes/c9/change.json\n";
        assert_eq!(out, Outcome::failure(missing.into()));
    }

    /// Gives the repository of `project` an author of its own, without signing or hooks: gdd
    /// commits as the repository's configuration says.
    fn identify(project: &Project) {
        for (key, value) in [
            ("user.name", "GDD Test"),
            ("user.email", "test@example.com"),
            ("commit.gpgsign", "false"),
            ("core.hooksPath", "/dev/null"),
        ] {
            git(project, &["config", "--local", key, value]);
        }
    }

    /// A project in a git repository with an author, its configuration committed on `main`,
    /// G0 open and the red zones approved; change `c1`, which plans `src/a.rs`, checked out on
    /// its branch.
    fn launched_project() -> Project {
        let project = Project::new();
        project.write(RULES, PROTECTED);
        project.write(ZONES, ZONE_MAP);
        project.write("gdd/models.toml", MODELS);
        let commands = "[g2]\nlint = \"true\"\n[release]\nsimulated = true\nbuild = \"true\"\n";
        project.write(COMMANDS, commands);
        project.write(".gitignore", ".gdd/*\n");
        git(&project, &["init", "-q", "-b", "main"]);
        identify(&project);
        git(&project, &["add", "-A"]);
        git(&project, &["commit", "-q", "-m", "project"]);
        project.write(approve::G0_PATH, r#"{"decision": "ok"}"#);
        assert_eq!(project.run("gate zones ok").code, 0);
        let created = project.run("new-change c1 --title App --paths src/a.rs");
        assert_eq!(created.code, 0, "{created:?}");
        git(&project, &["checkout", "-q", "change/c1"]);
        project
    }

    #[test]
    fn prepare_starts_the_change_and_names_the_model_of_each_role() {
        let project = launched_project();
        let out = project.run("prepare c1");
        assert_eq!(out.code, 0, "{out:?}");
        let start = "{\"ok\":true,\"change\":\"c1\",";
        assert!(out.stdout.starts_with(start), "{out:?}");
        let prepared = json(&out);
        let sonnet = serde_json::json!({"integration": "claude", "model": "sonnet"});
        assert_eq!(prepared["builder"], sonnet);
        assert_eq!(prepared["retries"], 3);
        assert_eq!(prepared["resume_green"], false);
        let record = record_of(&project, "c1");
        assert_eq!(record["state"], "building");
        assert_eq!(record["decision"]["builder"], sonnet);
        git(&project, &["checkout", "-q", "main"]);
        let refused = "error: Not on branch change/c1. Launch the change with bin/gdd launch c1.\n";
        assert_eq!(project.run("prepare c1"), Outcome::failure(refused.into()));
        let missing = "error: change c9 does not exist: there is no .gdd/changes/c9/change.json\n";
        assert_eq!(project.run("prepare c9"), Outcome::failure(missing.into()));
        project.write(COMMANDS, "[g2\n");
        let out = project.run("prepare c1");
        let start = "error: gdd/commands.toml: invalid TOML";
        assert!(out.stderr.starts_with(start), "{out:?}");
    }

    #[test]
    fn merge_and_release_take_the_change_to_production() {
        let project = launched_project();
        assert_eq!(project.run("gate G3 ok c1 --by machines").code, 0);
        project.write("src/a.rs", "x\n");
        let out = project.run("merge c1");
        assert_eq!(out.code, 0, "{out:?}");
        let merged = json(&out);
        assert_eq!(merged["simulated"], true);
        assert_eq!(merged["detail"]["name"], "deploy_pre");
        assert_eq!(git(&project, &["branch", "--show-current"]), "main");
        let message = git(&project, &["log", "-1", "--format=%B"]);
        let start = "gdd(c1): G3 Merge Gate · App\n\nColor: yellow · lane full · type Feature\n";
        assert!(message.starts_with(start), "{message}");
        assert_eq!(record_of(&project, "c1")["state"], "merged");
        for (step, state) in [
            ("pre", "in-pre"),
            ("promote", "in-production"),
            ("observe", "closed"),
        ] {
            let out = project.run(&format!("release {step} c1"));
            assert_eq!(out.code, 0, "{step}: {out:?}");
            assert_eq!(json(&out)["ok"], true, "{step}");
            assert_eq!(record_of(&project, "c1")["state"], state, "{step}");
        }
        assert_eq!(record_of(&project, "c1")["gates"]["G4"]["by"], "machines");
    }

    #[test]
    fn a_change_that_needs_g4_waits_and_production_getting_worse_rolls_it_back() {
        let project = committed_project();
        let commands = "[release]\nsimulated = true\nobserve = \"exit 1\"\n";
        project.write(COMMANDS, commands);
        project.run("new-change c1 --title Save --paths src/storage/db.rs");
        let refused = "error: This change needs a human G4 before promotion.\n";
        let out = project.run("release promote c1");
        assert_eq!(out, Outcome::failure(refused.into()));
        let closed = Outcome::failure("error: G3 is not open.\n".into());
        assert_eq!(project.run("merge c1"), closed);
        let out = project.run("release observe c1");
        assert_eq!(out.code, 1);
        let start = "error: release.observe failed (exit code 1)\nRollback: simulated\n";
        assert!(out.stderr.starts_with(start), "{out:?}");
        assert_eq!(record_of(&project, "c1")["state"], "rolled-back");
        project.write(COMMANDS, "[release\n");
        let out = project.run("release pre c1");
        let start = "error: gdd/commands.toml: invalid TOML";
        assert!(out.stderr.starts_with(start), "{out:?}");
        // Merge reads its release commands once merged: G3 comes first.
        assert_eq!(project.run("merge c1"), closed);
    }
}
