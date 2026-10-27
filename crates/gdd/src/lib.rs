//! The `gdd` command line: it reads the words of a command, runs it on the project and says
//! what happened. The method itself lives in `gdd-core`. In a terminal, `gdd init` without the
//! flags of the project asks the Setup form (see [`form`]).

pub mod form;

use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::{Arg, ArgAction, ArgMatches, Args, CommandFactory, FromArgMatches, Parser, Subcommand};
use gdd_core::agents::{
    self, Limits, invoke, models::Models, models::Roles, render, sheets::Sheet,
};
use gdd_core::change::{self, Decision, Record};
use gdd_core::classify::classify_with;
use gdd_core::gates::{self, Gate, approve, ci, debt, exceptions, prepare, release, selftest};
use gdd_core::git::Git;
use gdd_core::guard::{self, Hashes, Settings, Verdict};
use gdd_core::recipe::{self, Answer, Said, Snapshot, queue};
use gdd_core::repo::merge;
use gdd_core::status::{self, Status};
use gdd_core::{
    Classification, Color, Report, RulesTable, ZoneMap, adopt, inbox, init, log, session, stacks,
    state,
};

/// File that marks the root of a GDD project.
const PROJECT: &str = "gdd/project.toml";
/// Zone map of the project.
const ZONES: &str = "gdd/zones.toml";
/// Rules table of the project, with the protected paths of Path Guard.
const RULES: &str = guard::RULES;
/// Gate commands of the project.
const COMMANDS: &str = gates::COMMANDS;
/// Where the changes keep their records, from the state root (see [`state::root`]).
const CHANGES: &str = ".gdd/changes";
/// The role of test mode, which Path Guard judges after it.
const TEST_ROLE: &str = "builder-test";
/// Why a command that works on a change cannot run.
const MISSING_CHANGE: &str = "missing change id (argument or GDD_CHANGE)";
/// Why `gdd session` cannot find the transcripts.
const NO_HOME: &str = "HOME is not set: the Claude Code transcripts are under it";
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
    /// Exit code: 0 when the command did its job, 1 when it failed, 2 when `gdd init` has
    /// neither the flags of the project nor a terminal for its form.
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

    /// A command that could not do its job, and why: an error.
    fn error(message: &str) -> Outcome {
        Outcome::failure(format!("error: {message}\n"))
    }

    /// What a command printed for the user, on a line of its own, with its exit code.
    fn printed(text: String, code: u8) -> Outcome {
        Outcome {
            code,
            stdout: format!("{text}\n"),
            stderr: String::new(),
        }
    }

    /// What a run of the recipe said: where it got to, or why it parked the change, which
    /// fails the run.
    fn said(said: Said) -> Outcome {
        match said {
            Said::Done(text) => Outcome::printed(text, 0),
            Said::Parked(message) => Outcome::failure(format!("{message}\n")),
        }
    }
}

/// GDD · Gate-Driven Development: phases, gates and coding agents, from the terminal.
#[derive(Debug, Parser)]
#[command(name = "gdd", version = gdd_core::VERSION)]
struct Cli {
    #[command(subcommand)]
    command: Option<Run>,
}

/// What `gdd` runs: `gdd init`, which makes a project, or a command that works in one.
#[derive(Debug, Subcommand)]
enum Run {
    /// Sets up GDD in this folder, which becomes the project root: the harness (gdd/, bin/gdd,
    /// the CI workflow), the Brief, the agents' commands, AGENTS.md and CLAUDE.md when absent, the
    /// skeleton of each stack the folder has no project of, and the lines of each stack and
    /// .gdd/ in .gitignore; then commits what it wrote, and only that. Never overwrites a file,
    /// refuses inside a GDD project and installs nothing (gdd bootstrap does).
    Init(InitArgs),
    #[command(flatten)]
    InProject(Command),
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
        /// Id of the change (its record is .gdd/changes/<id>/change.json); else GDD_CHANGE.
        change: Option<String>,
    },
    /// Path Guard: what a change touched since its build started that a role must not touch,
    /// as JSON.
    Guard(GuardArgs),
    /// Records what a change touched as it is now, so `gdd guard --since-snapshot` judges only
    /// what changes after.
    Snapshot {
        /// Id of the change (its record is .gdd/changes/<id>/change.json); else GDD_CHANGE.
        change: Option<String>,
    },
    /// Paths a change touched outside its plan: accepted when their color is not higher than
    /// the change's, as JSON.
    Scope {
        /// Id of the change: its record is .gdd/changes/<id>/change.json.
        change: String,
    },
    /// Loop Gate: the G2 commands of gdd/commands.toml, Path Guard and, in build mode, the
    /// scope. Saves the result next to the change, logs it unless an agent runs it (GDD_ROLE)
    /// and prints it as JSON.
    G2(GateArgs),
    /// Merge Gate: the G3 commands of gdd/commands.toml, Path Guard and the scenario count.
    /// Saves the result next to the change, logs it unless an agent runs it (GDD_ROLE) and
    /// prints it as JSON.
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
        /// Id of the change (its record is .gdd/changes/<id>/change.json); else GDD_CHANGE.
        change: Option<String>,
    },
    /// Where the project stands: G0, red zones, self-test, scenarios, changes and log.
    Status,
    /// The first install of each stack of gdd/project.toml, in order, which writes its lock file:
    /// stops at the first that fails, then commits the files of the stacks' supply chain the run
    /// created or changed, and nothing else. Run it once, after gdd init.
    Bootstrap,
    /// Adopts a project that already has code: runs once each G2 and G3 command that judges the
    /// whole project (not acceptance*, mutation* or secrets*), keeps what each failing one said in
    /// .gdd/adoption/, records them as known debt in gdd/debt.toml, which it commits alone, and
    /// proposes a Refactor per command, riskiest first. Refused once gdd/debt.toml exists.
    Adopt,
    /// Harness self-test: a failing test turns a green G2 red, the harness is protected and CI
    /// runs the gates, rollback works in pre, and zero scenarios fail G3. Saved in
    /// .gdd/selftest.json and logged; exits with 1 unless all four pass.
    Selftest,
    /// What CI runs: on the branch change/<id> (GITHUB_HEAD_REF, else the current one), no
    /// harness path changed against the main branch, and G2 and G3 green. Reads and writes
    /// nothing under .gdd/; exits with 1 when CI fails.
    Ci,
    /// Adds the requests of the interactive Claude Code sessions of the project to the log, one
    /// line per model and each request once; the sessions gdd started for its agents are left
    /// out.
    Session {
        /// The phase of the method the conversation belongs to.
        #[arg(long, value_parser = session::PHASES)]
        phase: String,
        /// Count it for this change.
        #[arg(long)]
        change: Option<String>,
    },
    /// Checks that a change may enter Build (G0, red zones, G1, plan, model families, its
    /// branch) and starts it. Prints its decision, with the model of each role, as JSON.
    Prepare {
        /// Id of the change (its record is .gdd/changes/<id>/change.json); else GDD_CHANGE.
        change: Option<String>,
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
        /// Id of the change (its record is .gdd/changes/<id>/change.json); else GDD_CHANGE.
        change: Option<String>,
    },
    /// A release step of gdd/commands.toml: pre (tests and demo), promote (to production, after
    /// G4) or observe (rolls the change back when production gets worse). Prints it as JSON.
    Release {
        /// The step.
        #[arg(value_parser = release::STEPS)]
        step: String,
        /// Id of the change (its record is .gdd/changes/<id>/change.json); else GDD_CHANGE.
        change: Option<String>,
    },
    /// Runs the recipe of a change in its own git worktree, on its branch change/<id>, while
    /// this checkout goes to the main branch: Build, G2, G3, the Reviewer, merge and pre; then it
    /// waits for the human G4 when the change needs it, or goes to production. The worktree goes
    /// when the change closes. Refuses uncommitted changes outside that branch. A change that
    /// parks exits with 1.
    Launch {
        /// Id of the change: its record is .gdd/changes/<id>/change.json.
        change: String,
    },
    /// Answers the G4 of a change paused at it: ok takes it to production, ko rejects it, and
    /// polish runs one more Builder round with your notes and pauses at G4 again. Builders and
    /// Reviewers cannot answer.
    Resume(ResumeArgs),
    /// The GDD commands, and the instructions of each stack in `stack` of gdd/project.toml, in
    /// the folders of the coding agents (claude, codex, opencode), as their agent sheets say.
    Agents {
        #[command(subcommand)]
        action: AgentsAction,
    },
    /// The stack sheets gdd comes with (python, rust, typescript): one line per sheet.
    Stacks {
        #[command(subcommand)]
        action: Option<StacksAction>,
    },
}

/// What `gdd agents` does.
#[derive(Debug, Subcommand)]
enum AgentsAction {
    /// Writes every GDD command and the instructions of each stack of the project for each
    /// agent, and no other file. Prints the paths as JSON.
    Install(AgentArgs),
    /// Checks that each GDD command and the instructions of each stack of the project of each
    /// agent are exactly what install writes; names every file missing or edited, and exits
    /// with 1.
    Check(AgentArgs),
}

/// What `gdd stacks` does besides listing the sheets.
#[derive(Debug, Subcommand)]
enum StacksAction {
    /// One sheet: the tool for each need, the commands and the protected paths.
    Show {
        /// The stack.
        name: String,
    },
    /// What the named sheets give together, as TOML: the shape of gdd/commands.toml plus a
    /// [protected] table. Writes nothing.
    Compose {
        /// The stacks; a name given twice counts once.
        names: Vec<String>,
    },
}

/// What `gdd init` takes. A text option takes every word up to the next option. The name, the
/// platform and the stacks go together; without them, in a terminal, the Setup form asks, the
/// other flags as its first answers.
#[derive(Debug, Args)]
struct InitArgs {
    /// The name of the project.
    #[arg(long, num_args = 1.., requires_all = ["platform", "stacks"])]
    name: Vec<String>,
    /// Where it runs.
    #[arg(long, value_parser = init::PLATFORMS, requires_all = ["name", "stacks"])]
    platform: Option<String>,
    /// Its stacks (gdd stacks lists them).
    #[arg(long = "stack", num_args = 1.., requires_all = ["name", "platform"])]
    stacks: Vec<String>,
    /// Its coding agents, the first builds; without it, those on the PATH (claude, codex,
    /// opencode).
    #[arg(long, num_args = 1..)]
    agents: Vec<String>,
    /// The language the agents use with the human.
    #[arg(long, default_value = "en")]
    language: String,
    /// What it is, in words.
    #[arg(long, num_args = 1..)]
    description: Vec<String>,
    #[command(flatten)]
    sensitive: Sensitive,
    /// Accept a Reviewer of the Builder's model family when every agent is of one family.
    #[arg(long)]
    accept_same_family: bool,
}

impl InitArgs {
    /// Whether the flags name the project: its platform, and so its name and its stacks, which
    /// go with it.
    fn name_the_project(&self) -> bool {
        self.platform.is_some()
    }

    /// What the flags say of the new project; the name, the platform and the stacks are empty
    /// when they do not name it.
    fn options(self) -> init::Options {
        init::Options {
            name: self.name.join(" "),
            platform: self.platform.unwrap_or_default(),
            stacks: self.stacks,
            agents: self.agents,
            language: self.language,
            description: self.description.join(" "),
            sensitive: self.sensitive.0,
            accept_same_family: self.accept_same_family,
        }
    }
}

/// Why `gdd init` without the flags of the project did nothing.
const NO_TERMINAL: &str = "error: gdd init opens its Setup form only in a terminal; else give it \
                           the flags --name, --platform and --stack";

/// The option of the sensitive areas of `gdd init`.
const SENSITIVE: &str = "sensitive";

/// What `gdd init --sensitive` names: an area each time the option is given, its words joined
/// with single spaces.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Sensitive(Vec<String>);

impl FromArgMatches for Sensitive {
    fn from_arg_matches(matches: &ArgMatches) -> Result<Sensitive, clap::Error> {
        let given = matches.get_occurrences::<String>(SENSITIVE);
        let mut areas = Vec::new();
        for words in given.into_iter().flatten() {
            let words: Vec<&str> = words.map(String::as_str).collect();
            areas.push(words.join(" "));
        }
        Ok(Sensitive(areas))
    }

    fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<(), clap::Error> {
        *self = Sensitive::from_arg_matches(matches)?;
        Ok(())
    }
}

impl Args for Sensitive {
    fn augment_args(command: clap::Command) -> clap::Command {
        let option = Arg::new(SENSITIVE)
            .long(SENSITIVE)
            .num_args(1..)
            .action(ArgAction::Append)
            .value_parser(clap::value_parser!(String))
            .help("What would be serious if it failed: one area each time");
        command.arg(option)
    }

    fn augment_args_for_update(command: clap::Command) -> clap::Command {
        Sensitive::augment_args(command)
    }
}

/// What `gdd agents install` and `gdd agents check` take.
#[derive(Debug, Args)]
struct AgentArgs {
    /// The agents; without it, the integrations of gdd/models.toml.
    #[arg(long = "agent", num_args = 1..)]
    agents: Vec<String>,
}

/// What `gdd guard` takes.
#[derive(Debug, Args)]
struct GuardArgs {
    /// Id of the change (its record is .gdd/changes/<id>/change.json); else GDD_CHANGE.
    change: Option<String>,
    /// Role whose protected paths count (builder-build, builder-test, reviewer...).
    #[arg(long, default_value = "builder")]
    role: String,
    /// Exit with code 1 when Path Guard fails.
    #[arg(long)]
    strict: bool,
    /// Count only what changed since the last `gdd snapshot`.
    #[arg(long)]
    since_snapshot: bool,
}

/// What `gdd g2` and `gdd g3` take.
#[derive(Debug, Args)]
struct GateArgs {
    /// Id of the change (its record is .gdd/changes/<id>/change.json); else GDD_CHANGE.
    change: Option<String>,
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

/// What `gdd resume` takes.
#[derive(Debug, Args)]
struct ResumeArgs {
    /// Id of the change: its record is .gdd/changes/<id>/change.json.
    change: String,
    /// The answer to G4.
    #[arg(long, value_parser = recipe::ANSWERS)]
    g4: String,
    /// What to read about the answer: the rest of the words. Polish needs it.
    #[arg(long, num_args = 1.., allow_hyphen_values = true)]
    note: Vec<String>,
}

/// A color named on the command line.
fn parse_color(name: &str) -> Result<Color, String> {
    name.parse().map_err(|e: gdd_core::Error| e.to_string())
}

impl Command {
    /// Runs the command on the project at `root`, whose state root is `state` (see
    /// [`state::root`]), for the role `role`, in the environment `env`: what it prints and its
    /// exit code.
    fn run(
        self,
        root: &Path,
        state: &Path,
        role: Option<&str>,
        env: &Env,
    ) -> Result<Outcome, String> {
        let text = match self {
            Command::G2(args) => return run_gate(root, state, Gate::G2, args, role, env),
            Command::G3(args) => return run_gate(root, state, Gate::G3, args, role, env),
            Command::Guard(args) => return run_guard(root, state, args, env),
            Command::Bootstrap => return bootstrap(root, env),
            Command::Selftest => return run_selftest(root, state),
            Command::Ci => return run_ci(root, state, env),
            Command::Park { change, reason } => {
                let message = change::park(state, &change, &reason.join(" "))?;
                return Ok(Outcome::failure(format!("{message}\n")));
            }
            Command::Launch { change } => return launch(root, state, &change, env),
            Command::Resume(args) => return resume(root, state, &args, role, env),
            Command::Merge { change } => {
                return merge_queued(root, state, &change_id(change, env)?, env);
            }
            Command::Agents { action } => return agent_commands(root, &action),
            Command::Stacks { action } => stack_sheets(action).map_err(|e| e.to_string())?,
            Command::Report { by, change, json } => report(state, &by, change.as_deref(), json)?,
            Command::Classify { change } => classify(root, state, &change_id(change, env)?)?,
            Command::Snapshot { change } => snapshot(root, state, &change_id(change, env)?)?,
            Command::Scope { change } => scope(root, state, &change)?,
            Command::NewChange(args) => new_change(root, state, &args)?,
            Command::Gate(args) => record_gate(root, state, &args, role)?,
            Command::Note(args) => note(state, &args)?,
            Command::Inbox { change } => decision_inbox(state, &change_id(change, env)?)?,
            Command::Status => project_status(root, state, env)?,
            Command::Adopt => adopt_project(root, state)?,
            Command::Session { phase, change } => {
                import_session(root, state, &phase, change.as_deref(), env)?
            }
            Command::Prepare { change } => prepare_change(root, state, &change_id(change, env)?)?,
            Command::Release { step, change } => {
                release_change(root, state, &step, &change_id(change, env)?)?
            }
        };
        Ok(Outcome::printed(text, 0))
    }
}

/// The id of the change a command works on: `given`, else `GDD_CHANGE` of the environment `env`.
fn change_id(given: Option<String>, env: &Env) -> Result<String, String> {
    let set = given.or_else(|| variable(env, "GDD_CHANGE").map(str::to_string));
    set.ok_or_else(|| MISSING_CHANGE.to_string())
}

/// The value of the variable `name` in the environment `env`, the last one set, when it is not
/// empty.
fn variable<'a>(env: &'a Env, name: &str) -> Option<&'a str> {
    let (_, value) = env.iter().rev().find(|(known, _)| known == name)?;
    Some(value.as_str()).filter(|value| !value.is_empty())
}

/// An environment: its variables, with their values.
type Env = [(String, String)];

/// Runs `gdd` with `args`, the words after the program name, from the folder `cwd`, in the
/// environment of the process (see [`run_with_env`]).
pub fn run(args: &[String], cwd: &Path) -> Outcome {
    run_with_env(args, cwd, &process_env())
}

/// Runs `gdd` with `args`, the words after the program name, from the folder `cwd`, with `env`
/// as the whole environment it runs in: the agents it starts get it (see [`invoke`]), and its
/// `GDD_ROLE` is the role it runs for (see [`run_as`]).
pub fn run_with_env(args: &[String], cwd: &Path, env: &[(String, String)]) -> Outcome {
    let role = env.iter().rev().find(|(name, _)| name == "GDD_ROLE");
    run_in(args, cwd, role.map(|(_, role)| role.as_str()), env)
}

/// Runs `gdd` with `args`, the words after the program name, from the folder `cwd`, for the
/// role `role` (`None` for a person), in the environment of the process: a Builder or a
/// Reviewer cannot record gate decisions. Help and version go to the standard output; every
/// error goes to the standard error, with exit code 1. `gdd guard --strict` also exits with 1
/// when Path Guard fails, and `gdd g2 --strict` and `gdd g3 --strict` when the gate fails.
/// `gdd park` always exits with 1, its message on the standard error, and `gdd agents check`
/// when it finds a problem. `gdd selftest` and `gdd ci` exit with 1 when they fail, what they
/// found on the standard output. `gdd init` without the flags of the project exits with 2.
pub fn run_as(args: &[String], cwd: &Path, role: Option<&str>) -> Outcome {
    run_in(args, cwd, role, &process_env())
}

/// Runs `gdd` with `args` from the folder `cwd`, for the role `role`, in the environment `env`
/// (see [`run_as`]).
fn run_in(args: &[String], cwd: &Path, role: Option<&str>, env: &Env) -> Outcome {
    let cli = match parse(args) {
        Ok(cli) => cli,
        Err(e) if e.use_stderr() => return Outcome::failure(e.to_string()),
        Err(e) => return Outcome::success(e.to_string()),
    };
    let command = match cli.command {
        None => return Outcome::success(Cli::command().render_help().to_string()),
        Some(Run::Init(args)) => return init_project(cwd, args, env),
        Some(Run::InProject(command)) => command,
    };
    let ran = project_root(cwd).and_then(|root| {
        // Once per command, then passed down: git is asked here only.
        let state = state::root(&root)?;
        command.run(&root, &state, role, env)
    });
    ran.unwrap_or_else(|message| Outcome::error(&message))
}

/// The command `gdd` with `args`, the words after the program name, as clap reads it.
fn parse(args: &[String]) -> Result<Cli, clap::Error> {
    let words = std::iter::once("gdd").chain(args.iter().map(String::as_str));
    Cli::try_parse_from(words)
}

/// The environment of the process; a variable whose name or value is not UTF-8 is left out.
fn process_env() -> Vec<(String, String)> {
    let text = |(name, value): (OsString, OsString)| {
        Some((name.into_string().ok()?, value.into_string().ok()?))
    };
    std::env::vars_os().filter_map(text).collect()
}

/// `gdd init`: sets up GDD in the folder `cwd` (see [`init::init`]), with git and the agents on
/// the `PATH` of the environment `env`. What it says goes to the standard output; why it wrote
/// nothing, or stopped, to the standard error. Without the flags of the project, which then only
/// the form asks (see [`asks_form`]), it fails like a command missing its arguments, with exit
/// code 2.
fn init_project(cwd: &Path, args: InitArgs, env: &Env) -> Outcome {
    if !args.name_the_project() {
        let missing = Outcome::failure(format!("{NO_TERMINAL}\n"));
        return Outcome { code: 2, ..missing };
    }
    match init::init(cwd, &args.options(), env) {
        Ok(said) => Outcome::printed(said, 0),
        Err(e) => Outcome::failure(format!("{e}\n")),
    }
}

/// The first answers of the Setup form, when `gdd` with `args` asks it (see
/// [`form::in_terminal`]): it is `gdd init` without the flags of the project (`--name`,
/// `--platform` and `--stack`), whose other flags answer first, and its standard input and output
/// are terminals (`stdin` and `stdout`).
pub fn asks_form(args: &[String], stdin: bool, stdout: bool) -> Option<init::Options> {
    if !(stdin && stdout) {
        return None;
    }
    match parse(args).ok()?.command? {
        Run::Init(args) if !args.name_the_project() => Some(args.options()),
        _ => None,
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

/// `gdd report`: the agent calls of the log under the state root `state`, added up by the fields
/// `by`.
fn report(state: &Path, by: &str, change: Option<&str>, json: bool) -> Result<String, String> {
    let text = read(state, log::PATH)?.unwrap_or_default();
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
fn classify(root: &Path, state: &Path, id: &str) -> Result<String, String> {
    let decided = decide(root, state, id)?;
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

/// The decision for change `id`, from its record under the state root `state`, and the zone
/// map, the rules table and the models of the project at `root`. Its planned paths count, the
/// ones it touched in git (see [`guard::touched`]) and the scenario files its record names. A
/// missing zone map, rules table or models file says nothing.
fn decide(root: &Path, state: &Path, id: &str) -> Result<Decided, String> {
    let text = change::read_record(state, id)?;
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
    let retries = load_limits(root)?.builder_retries;
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
fn path_guard(
    root: &Path,
    state: &Path,
    id: &str,
    role: &str,
    since_snapshot: bool,
) -> Result<Verdict, String> {
    let record = load_record(state, id)?;
    let settings = load_settings(root)?;
    let before = guard_snapshot(state, id, role, since_snapshot)?;
    guard::path_guard(root, id, &record, &settings, role, before.as_ref())
        .map_err(|e| e.to_string())
}

/// The snapshot, under the state root `state`, Path Guard judges `role` from on change `id`, if
/// there is one: build mode is judged from the one taken after test mode (what test mode wrote is
/// not build mode's doing); with `since_snapshot`, other roles from the last `gdd snapshot`.
/// Otherwise, none: everything the change touched since its build started counts.
fn guard_snapshot(
    state: &Path,
    id: &str,
    role: &str,
    since_snapshot: bool,
) -> Result<Option<Hashes>, String> {
    if role == gates::BUILD_ROLE {
        read_snapshot(state, &change::build_snapshot_path(id))
    } else if since_snapshot {
        read_snapshot(state, &change::snapshot_path(id))
    } else {
        Ok(None)
    }
}

/// `gdd guard`: Path Guard on the change of `args`, else on the one `GDD_CHANGE` of the
/// environment `env` names (see [`path_guard`]). Its verdict as JSON, and exit code 1 when it
/// fails and the run is strict.
fn run_guard(root: &Path, state: &Path, args: GuardArgs, env: &Env) -> Result<Outcome, String> {
    let id = change_id(args.change, env)?;
    let verdict = path_guard(root, state, &id, &args.role, args.since_snapshot)?;
    let printed = serde_json::to_string(&verdict).map_err(|e| e.to_string())?;
    let code = u8::from(args.strict && !verdict.ok);
    Ok(Outcome::printed(printed, code))
}

/// `gdd g2` and `gdd g3`: runs `gate` on the change of `args`, else on the one `GDD_CHANGE` of
/// the environment `env` names (see [`gate_report`]). Started by an agent, for the role `role`,
/// the run is not logged: an agent writes only its change's state folder, and the recipe logs
/// its own run of the gate after the call. The report as JSON, then, for the person who reads
/// it, one line per security exception the change adds on the standard error; exit code 1 when
/// the gate fails and the run is strict.
fn run_gate(
    root: &Path,
    state: &Path,
    gate: Gate,
    args: GateArgs,
    role: Option<&str>,
    env: &Env,
) -> Result<Outcome, String> {
    let id = change_id(args.change, env)?;
    let report = gate_report(root, state, gate, &id, &args.role, role.is_none())?;
    let printed = serde_json::to_string(&report).map_err(|e| e.to_string())?;
    let code = u8::from(args.strict && !report.ok);
    let mut outcome = Outcome::printed(printed, code);
    let added = report.added_exceptions.iter();
    outcome.stderr = added.map(|line| format!("{line}\n")).collect();
    Ok(outcome)
}

/// `gdd bootstrap`: the first install of each stack of the project at `root`, in the environment
/// `env` (see [`init::bootstrap`]). What it said goes to the standard output; why it stopped, to
/// the standard error, and fails the run.
fn bootstrap(root: &Path, env: &Env) -> Result<Outcome, String> {
    let stacks = project_stacks(root)?;
    Ok(match init::bootstrap(root, &stacks, env) {
        Ok(said) => Outcome::printed(said, 0),
        Err(stopped) => Outcome {
            code: 1,
            stdout: stopped.said,
            stderr: format!("error: {}\n", stopped.why),
        },
    })
}

/// `gdd selftest`: the harness self-test of the project (see [`selftest::run`]), saved and
/// logged under the state root `state`. It fails the run unless every check passed.
fn run_selftest(root: &Path, state: &Path) -> Result<Outcome, String> {
    let gates = GateConfig::load(root)?;
    let text = read(root, COMMANDS)?.unwrap_or_default();
    let config = selftest::Config::from_toml(&text).map_err(|e| format!("{COMMANDS}: {e}"))?;
    let checked = selftest::run(&gates.project(root, state), &config)?;
    selftest::save(state, &checked)?;
    let code = u8::from(!checked.ok());
    Ok(Outcome::printed(checked.to_string(), code))
}

/// `gdd ci`: what CI runs on the branch `GITHUB_HEAD_REF` of the environment `env` names, else
/// on the current one (see [`ci::check`]). It fails the run when CI fails.
fn run_ci(root: &Path, state: &Path, env: &Env) -> Result<Outcome, String> {
    let gates = GateConfig::load(root)?;
    let head_ref = variable(env, "GITHUB_HEAD_REF");
    let checked = ci::check(&gates.project(root, state), head_ref)?;
    let code = u8::from(!checked.ok());
    Ok(Outcome::printed(checked.to_string(), code))
}

/// `gdd session`: the requests of the interactive Claude Code sessions of the project into the
/// log (see [`session::import`]), from the transcripts under the `HOME` of the environment `env`.
fn import_session(
    root: &Path,
    state: &Path,
    phase: &str,
    change: Option<&str>,
    env: &Env,
) -> Result<String, String> {
    let home = variable(env, "HOME").ok_or(NO_HOME)?;
    session::import(root, state, Path::new(home), phase, change)
}

/// Runs `gate` on change `id` for `role`, writes in its record the paths the scope check
/// accepted and saves the report next to it, under the state root `state`; `logged`, it logs the
/// run too.
fn gate_report(
    root: &Path,
    state: &Path,
    gate: Gate,
    id: &str,
    role: &str,
    logged: bool,
) -> Result<gates::Report, String> {
    let record = load_record(state, id)?;
    let config = GateConfig::load(root)?;
    let snapshot = guard_snapshot(state, id, role, false)?;
    let project = config.project(root, state);
    let report = gates::run(&project, gate, id, &record, role, snapshot.as_ref())
        .map_err(|e| e.to_string())?;
    save_out_of_scope(state, id, &report.out_of_scope)?;
    if logged {
        gates::save(state, id, &report)?;
    } else {
        gates::keep(state, id, &report)?;
    }
    Ok(report)
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

    /// What a gate reads from the project at `root`, whose state root is `state`.
    fn project<'a>(&'a self, root: &'a Path, state: &'a Path) -> gates::Project<'a> {
        gates::Project {
            root,
            state,
            zones: &self.zones,
            rules: &self.rules,
            settings: &self.settings,
            commands: &self.commands,
        }
    }
}

/// `gdd prepare`: checks that change `id` may enter Build and starts it (see
/// [`prepare::prepare`]).
fn prepare_change(root: &Path, state: &Path, id: &str) -> Result<String, String> {
    prepare_decided(root, state, id, &decide(root, state, id)?)
}

/// Prepares change `id`, for which the tool decided `decided` (see [`prepare_change`]).
fn prepare_decided(
    root: &Path,
    state: &Path,
    id: &str,
    decided: &Decided,
) -> Result<String, String> {
    let config = GateConfig::load(root)?;
    let project = config.project(root, state);
    let zones = read(root, ZONES)?.unwrap_or_default();
    let setup = prepare::Setup {
        project: &project,
        zones: &zones,
        models: &decided.models,
    };
    prepare::prepare(&setup, id, &decided.decision(id))
}

/// `gdd merge`: the merge of change `id`, whose branch is checked out at the project root `root`,
/// as the recipe merges (see [`recipe::merge`]): one merge at a time, on top of the latest main,
/// with its steps there; the gates it runs again start in the environment `env`. What it says on
/// its way goes to the standard error, before why it parked or failed: the standard output is
/// the JSON of the merge.
fn merge_queued(root: &Path, state: &Path, id: &str, env: &Env) -> Result<Outcome, String> {
    let main = load_settings(root)?.main_branch;
    let limits = load_limits(root)?;
    let settings = agent_settings(root)?;
    let mut steps = ProjectSteps::new(root, state, id, &limits, settings, env);
    let at = recipe::Merging {
        state,
        work: root,
        main: &main,
        id,
    };
    let ran = recipe::merge(&at, &mut steps);
    let mut outcome = ran.map_or_else(|e| Outcome::error(&e), Outcome::said);
    outcome.stderr.insert_str(0, &steps.said);
    Ok(outcome)
}

/// Merges change `id`, classified as it is now, into the main branch (see [`merge::merge`]).
/// What follows the merge reads its configuration as the merge left it.
fn merge_change(root: &Path, state: &Path, id: &str) -> Result<String, String> {
    let decided = decide(root, state, id)?;
    let settings = load_settings(root)?;
    let project = merge::Project {
        root,
        state,
        main: &settings.main_branch,
    };
    merge::merge(&project, id, &decided.classification)
}

/// `gdd release`: the release step `step` of change `id` (see [`release`]). Promoting asks
/// whether the change needs the human G4 as it is classified now.
fn release_change(root: &Path, state: &Path, step: &str, id: &str) -> Result<String, String> {
    let steps = load_steps(root)?;
    match step {
        "pre" => release::pre(root, state, id, &steps),
        "promote" => {
            let decided = decide(root, state, id)?;
            release::promote(root, state, id, &steps, decided.classification.g4)
        }
        "observe" => release::observe(root, state, id, &steps),
        other => Err(format!("unknown release step {other:?}")),
    }
}

/// `gdd launch`: the recipe of change `id` (see [`recipe::launch`]), whose agents start in the
/// environment `env`.
fn launch(root: &Path, state: &Path, id: &str, env: &Env) -> Result<Outcome, String> {
    in_worktree(root, state, id, env, |project, steps| {
        recipe::launch(project, id, steps)
    })
}

/// `gdd resume`: the answer of the role `role` to the G4 of a change (see [`recipe::resume`]),
/// whose agents start in the environment `env`.
fn resume(
    root: &Path,
    state: &Path,
    args: &ResumeArgs,
    role: Option<&str>,
    env: &Env,
) -> Result<Outcome, String> {
    let id = args.change.as_str();
    let answer: Answer = args.g4.parse()?;
    let note = args.note.join(" ");
    in_worktree(root, state, id, env, |project, steps| {
        recipe::resume(project, id, answer, &note, role, steps)
    })
}

/// Runs `run`, a run of the recipe, on change `id` of the project at `root`, whose state root is
/// `state`, with its steps in the project's folder of the worktree of the change, in the folder
/// of the worktrees the rules file names (see [`recipe::worktree`]), and as many changes building
/// at once as it allows (see [`queue::cap`]); the agents start in the environment `env`. What the
/// run said on its way comes first in what it prints, however it ended.
fn in_worktree(
    root: &Path,
    state: &Path,
    id: &str,
    env: &Env,
    run: impl FnOnce(&recipe::Project, &mut ProjectSteps) -> Result<Said, String>,
) -> Result<Outcome, String> {
    let main = load_settings(root)?.main_branch;
    let limits = load_limits(root)?;
    let rules = read(root, RULES)?.unwrap_or_default();
    let folder = recipe::worktree::folder(root, &rules).map_err(|e| format!("{RULES}: {e}"))?;
    let max = queue::cap(&rules).map_err(|e| format!("{RULES}: {e}"))?;
    let worktree = recipe::worktree::path(&folder, id)?;
    let work = recipe::worktree::project_root(root, &worktree)?;
    let project = recipe::Project {
        root,
        state,
        worktree: &worktree,
        work: &work,
        main: &main,
        replans: limits.replans,
        max,
    };
    let settings = agent_settings(root)?;
    let mut steps = ProjectSteps::new(&work, state, id, &limits, settings, env);
    let ran = run(&project, &mut steps);
    let mut outcome = ran.map_or_else(|e| Outcome::error(&e), Outcome::said);
    outcome.stdout.insert_str(0, &steps.said);
    Ok(outcome)
}

/// What `gdd/models.toml` of the project at `root` says of the agents besides the model of each
/// role.
fn agent_settings(root: &Path) -> Result<invoke::Settings, String> {
    let path = agents::models::PATH;
    let text = read(root, path)?.unwrap_or_default();
    invoke::Settings::from_toml(&text).map_err(|e| format!("{path}: {e}"))
}

/// `gdd agents install` and `gdd agents check` (see [`render::install`] and [`render::check`]),
/// with the stacks of the project: each problem check finds goes on a line of its own on the
/// standard error, and fails the run.
fn agent_commands(root: &Path, action: &AgentsAction) -> Result<Outcome, String> {
    let stacks = project_stacks(root)?;
    match action {
        AgentsAction::Install(args) => {
            let written = render::install(root, &agent_sheets(root, &args.agents)?, &stacks)?;
            let printed = serde_json::json!({"ok": true, "written": written});
            Ok(Outcome::printed(printed.to_string(), 0))
        }
        AgentsAction::Check(args) => {
            let problems = render::check(root, &agent_sheets(root, &args.agents)?, &stacks)?;
            if problems.is_empty() {
                return Ok(Outcome::printed(r#"{"ok":true}"#.to_string(), 0));
            }
            let lines = problems.iter().map(|line| format!("error: {line}\n"));
            Ok(Outcome::failure(lines.collect()))
        }
    }
}

/// The sheets of the agents `gdd agents` works on (see [`render::agents`]): the models of the
/// project count only when no agent is `named`.
fn agent_sheets(root: &Path, named: &[String]) -> Result<Vec<Sheet>, String> {
    let models = if named.is_empty() {
        load_models(root)?
    } else {
        Models::default()
    };
    render::agents(&models, named)
}

/// The stacks `gdd/project.toml` names (see [`stacks::of_project`]).
fn project_stacks(root: &Path) -> Result<Vec<String>, String> {
    let text = read(root, PROJECT)?.unwrap_or_default();
    stacks::of_project(&text).map_err(|e| format!("{PROJECT}: {e}"))
}

/// `gdd stacks`: the list of the sheets, one of them (see [`stacks::Sheet`]) or what several
/// give together (see [`stacks::compose`]).
fn stack_sheets(action: Option<StacksAction>) -> Result<String, stacks::Error> {
    match action {
        None => stacks::list(),
        Some(StacksAction::Show { name }) => Ok(stacks::sheet(&name)?.to_string()),
        Some(StacksAction::Compose { names }) => stacks::compose(&names),
    }
}

/// The steps of the recipe on change `id` of the project at `root`, the worktree of the change,
/// as the commands of the same name run them: each reads the configuration as it is when it
/// runs, but the agents' time limit and settings, read once.
struct ProjectSteps<'a> {
    root: &'a Path,
    /// The state root of the project (see [`state::root`]).
    state: &'a Path,
    id: &'a str,
    /// The time an agent has to answer.
    timeout: Duration,
    /// The environment gdd runs in, where the agents start.
    env: &'a Env,
    /// What `gdd/models.toml` says of the agents besides the model of each role.
    settings: invoke::Settings,
    /// What the recipe said on its way, a line each.
    said: String,
}

impl<'a> ProjectSteps<'a> {
    /// The steps on change `id` of the project at `root`, whose state root is `state` and whose
    /// rules file sets `limits`, with the agents starting in the environment `env` with
    /// `settings`.
    fn new(
        root: &'a Path,
        state: &'a Path,
        id: &'a str,
        limits: &Limits,
        settings: invoke::Settings,
        env: &'a Env,
    ) -> ProjectSteps<'a> {
        ProjectSteps {
            root,
            state,
            id,
            timeout: Duration::from_secs(limits.agent_timeout_s),
            env,
            settings,
            said: String::new(),
        }
    }
}

impl recipe::Steps for ProjectSteps<'_> {
    /// Prepares the change once every role can be started (see [`invoke::check`]).
    fn prepare(&mut self) -> Result<recipe::Decided, String> {
        let decided = decide(self.root, self.state, self.id)?;
        invoke::check(&decided.roles, self.env).map_err(|e| e.to_string())?;
        let printed = prepare_decided(self.root, self.state, self.id, &decided)?;
        serde_json::from_str(&printed).map_err(|e| format!("gdd prepare: {e}"))
    }

    fn agent(&mut self, call: &invoke::Call) -> Result<(), invoke::Error> {
        let project = invoke::Project {
            root: self.root,
            state: self.state,
            id: self.id,
            env: self.env,
            settings: &self.settings,
            timeout: self.timeout,
        };
        invoke::run(&project, call)
    }

    fn guard_test(&mut self, since_snapshot: bool) -> Result<Vec<String>, String> {
        let verdict = path_guard(self.root, self.state, self.id, TEST_ROLE, since_snapshot)?;
        Ok(verdict.failures)
    }

    fn snapshot(&mut self, snapshot: Snapshot) -> Result<(), String> {
        let path = match snapshot {
            Snapshot::Round => change::snapshot_path(self.id),
            Snapshot::Build => change::build_snapshot_path(self.id),
        };
        save_snapshot(self.root, self.state, self.id, &path)
    }

    fn gate(&mut self, gate: Gate) -> Result<bool, String> {
        let (root, state, id) = (self.root, self.state, self.id);
        let report = gate_report(root, state, gate, id, gates::BUILD_ROLE, true)?;
        Ok(report.ok)
    }

    fn decide(&mut self, gate: &str, decision: &str, by: &str, note: &str) -> Result<(), String> {
        let args = DecisionArgs {
            gate: gate.to_string(),
            decision: decision.to_string(),
            change: Some(self.id.to_string()),
            by: by.to_string(),
            note: note.to_string(),
        };
        record_gate(self.root, self.state, &args, None).map(drop)
    }

    /// The recipe holds the merge lock already.
    fn merge(&mut self) -> Result<String, String> {
        merge_change(self.root, self.state, self.id)
    }

    /// After the merge the change is on the main branch: its release runs from the main checkout,
    /// the project root of the state root, as the main branch configures it.
    fn release(&mut self, step: &str) -> Result<(), String> {
        release_change(self.state, self.state, step, self.id).map(drop)
    }

    fn say(&mut self, line: &str) {
        self.said.push_str(line);
        self.said.push('\n');
    }
}

/// Writes `accepted`, the paths outside the plan the scope check accepted, in the record of
/// change `id` under the state root `state`, when it does not say them already.
fn save_out_of_scope(state: &Path, id: &str, accepted: &[String]) -> Result<(), String> {
    let path = change::record_path(id);
    let text = read(state, &path)?.unwrap_or_default();
    match change::with_out_of_scope(&text, accepted) {
        Ok(Some(record)) => write(state, &path, &record),
        Ok(None) => Ok(()),
        Err(e) => Err(format!("{path}: {e}")),
    }
}

/// `gdd snapshot`: records the hashes of what change `id` touched since its build started.
fn snapshot(root: &Path, state: &Path, id: &str) -> Result<String, String> {
    save_snapshot(root, state, id, &change::snapshot_path(id))?;
    Ok(r#"{"ok":true}"#.to_string())
}

/// Records in the file `path`, under the state root `state`, the hashes of what change `id` of
/// the project at `root` touched since its build started.
fn save_snapshot(root: &Path, state: &Path, id: &str, path: &str) -> Result<(), String> {
    let record = load_record(state, id)?;
    let hashes = guard::snapshot(root, id, &record).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(&hashes).map_err(|e| e.to_string())?;
    write(state, path, &format!("{text}\n"))
}

/// `gdd scope`: the scope check of change `id`, with the zone map and the rules table.
fn scope(root: &Path, state: &Path, id: &str) -> Result<String, String> {
    let record = load_record(state, id)?;
    let zones = load_zones(root)?;
    let table = load_table(root)?;
    let checked = guard::scope(root, id, &record, &zones, &table).map_err(|e| e.to_string())?;
    serde_json::to_string(&checked).map_err(|e| e.to_string())
}

/// `gdd new-change`: the record of a new change `id`, under the state root `state`, and, when it
/// has none, its branch from the main branch; logged. Its classification as JSON. A change that
/// exists already is an error.
fn new_change(root: &Path, state: &Path, args: &NewChangeArgs) -> Result<String, String> {
    let id = args.change.as_str();
    change::check_id(id)?;
    let path = change::record_path(id);
    if state.join(&path).exists() {
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
    write(state, &path, &record)?;
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
    log::append(state, &line).map_err(|e| format!("cannot write {}: {e}", log::PATH))?;
    classify(root, state, id)
}

/// `gdd gate`: records a gate decision for the role `role` (see [`approve::record`]).
fn record_gate(
    root: &Path,
    state: &Path,
    args: &DecisionArgs,
    role: Option<&str>,
) -> Result<String, String> {
    let gate = match args.gate.parse::<approve::Gate>() {
        Ok(gate) => gate,
        Err(e) => return Err(e.to_string()),
    };
    let zones = read(root, ZONES)?.unwrap_or_default();
    let settings = load_settings(root)?;
    let table = load_table(root)?;
    let project = approve::Project {
        root,
        state,
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

/// `gdd note`: adds a note to the record of a change, under the state root `state`, for its
/// Decision Inbox.
fn note(state: &Path, args: &NoteArgs) -> Result<String, String> {
    let id = args.change.as_str();
    let text = change::read_record(state, id)?;
    let ts = log::now();
    let words = args.text.join(" ");
    let note = change::Note {
        kind: &args.kind,
        text: &words,
        ts: &ts,
    };
    let path = change::record_path(id);
    let record = change::with_note(&text, &note).map_err(|e| format!("{path}: {e}"))?;
    write(state, &path, &record)?;
    Ok("noted".to_string())
}

/// `gdd inbox`: the Decision Inbox of change `id`, also saved next to its record, under the state
/// root `state`, exactly as it is printed.
fn decision_inbox(state: &Path, id: &str) -> Result<String, String> {
    let text = change::read_record(state, id)?;
    inbox::save(state, id, &text)
}

/// `gdd status`: where the project at `root` stands, and its changes, from its files and what
/// the state root `state` keeps; and the coding agents on the `PATH` of the environment `env`
/// that could review it when it accepted one model family only (see [`status::second_family`]).
fn project_status(root: &Path, state: &Path, env: &Env) -> Result<String, String> {
    let project = read(root, PROJECT)?.unwrap_or_default();
    let basics = status::Basics::from_toml(&project).map_err(|e| format!("{PROJECT}: {e}"))?;
    let zones = read(root, ZONES)?.unwrap_or_default();
    let dir = load_table(root)?.acceptance.dir;
    let scenarios = gates::count_scenarios(&root.join(&dir)).map_err(|e| e.to_string());
    let today = exceptions::today();
    let found = exceptions::read(root);
    let selftest = read(state, approve::SELFTEST_PATH)?;
    let log_lines = read(state, log::PATH)?.map(|text| text.lines().count());
    let models = load_models(root)?;
    let status = Status {
        basics,
        brief: root.join(approve::BRIEF).is_file(),
        g0: approve::g0_open(state).map_err(|e| e.to_string())?,
        zones: approve::zones_state(state, &zones).map_err(|e| e.to_string())?,
        selftest: selftest.and_then(|text| status::selftest_summary(&text)),
        scenarios_dir: dir,
        scenarios,
        exceptions: found.map(|found| found.iter().map(|e| e.line(today)).collect()),
        debt: debt::read(root).map(|known| known.map(|known| known.debt).unwrap_or_default()),
        changes: change_lines(state)?,
        log_lines,
        second_family: status::second_family(&models, &init::agents_on_path(env)?),
    };
    Ok(status.to_string())
}

/// `gdd adopt`: adopts the project at `root`, whose state root is `state`, with its gate
/// commands (see [`adopt::adopt`]).
fn adopt_project(root: &Path, state: &Path) -> Result<String, String> {
    let commands = read(root, COMMANDS)?.unwrap_or_default();
    let commands = gates::Commands::from_toml(&commands).map_err(|e| format!("{COMMANDS}: {e}"))?;
    adopt::adopt(root, state, &commands).map(|adopted| adopted.to_string())
}

/// The line of each change in the status, in the order of their folders under the state root
/// `state`; a folder without a record is not a change.
fn change_lines(state: &Path) -> Result<Vec<String>, String> {
    let entries = match fs::read_dir(state.join(CHANGES)) {
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
        if let Some(text) = read(state, &change::record_path(&id))? {
            lines.push(status::change_line(&id, &text));
        }
    }
    Ok(lines)
}

/// The record of change `id`, under the state root `state` (see [`change::read_record`]).
fn load_record(state: &Path, id: &str) -> Result<Record, String> {
    let text = change::read_record(state, id)?;
    let path = change::record_path(id);
    Record::from_json(&text).map_err(|e| format!("{path}: {e}"))
}

/// The hashes of the snapshot at `path` under the state root `state`; `None` when there is none.
fn read_snapshot(state: &Path, path: &str) -> Result<Option<Hashes>, String> {
    match read(state, path)? {
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

/// The limits the rules file of the project sets on the agents.
fn load_limits(root: &Path) -> Result<Limits, String> {
    let text = read(root, RULES)?.unwrap_or_default();
    Limits::from_toml(&text).map_err(|e| format!("{RULES}: {e}"))
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

/// The text of the file `path` under the folder `root` (the project root or the state root), or
/// `None` when it does not exist.
fn read(root: &Path, path: &str) -> Result<Option<String>, String> {
    match fs::read_to_string(root.join(path)) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("cannot read {path}: {e}")),
    }
}

/// Writes `text` to the file `path` under the folder `root`, creating its folder.
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
    pub(super) struct Project(pub(super) PathBuf);

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

    impl Project {
        /// Where gdd makes the worktrees of the changes: next to the project.
        fn worktrees(&self) -> PathBuf {
            let mut folder = self.0.clone().into_os_string();
            folder.push(".worktrees");
            PathBuf::from(folder)
        }
    }

    impl Drop for Project {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
            let _ = fs::remove_dir_all(self.worktrees());
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
        for command in ["prepare", "park", "merge", "release", "stacks"] {
            assert!(out.stdout.contains(command), "{command}: {out:?}");
        }
        for command in [
            "launch", "resume", "agents", "selftest", "ci", "session", "adopt",
        ] {
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
            "classify a b",
            "report --by",
            "report --nope",
            "guard save --role",
            "scope",
            "g3 --nope",
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
            "status extra",
            "adopt extra",
            "selftest extra",
            "ci extra",
            "session",
            "session --phase 9-later",
            "session --phase 2-build --change",
            "park",
            "park c1",
            "release",
            "release deploy c1",
            "launch",
            "resume c1",
            "resume --g4 ok",
            "resume c1 --g4 maybe",
            "resume c1 --g4 ok --note",
            "agents remove",
            "agents install --agent",
            "agents check claude",
            "stacks rust",
            "stacks show",
            "stacks show rust python",
            "init --platform cli",
            "init --stack rust",
            "init --name x --stack rust",
            "init --name x --platform tv --stack rust",
            "init --name --platform cli --stack rust",
            "init --name x --platform cli",
            "init --name x --platform cli --stack rust --agents",
        ] {
            let out = run_in(Path::new("/"), command);
            assert_eq!((out.code, out.stdout.as_str()), (1, ""), "{command:?}");
            assert!(out.stderr.starts_with("error: "), "{command:?}: {out:?}");
        }
    }

    #[test]
    fn init_without_the_flags_of_the_project_asks_for_a_terminal_or_them() {
        for command in ["init", "init --language es --agents claude"] {
            let out = run_in(Path::new("/"), command);
            let failed = (2, String::new(), format!("{NO_TERMINAL}\n"));
            assert_eq!((out.code, out.stdout, out.stderr), failed, "{command:?}");
        }
        let said = "error: gdd init opens its Setup form only in a terminal; else give it the \
                    flags --name, --platform and --stack";
        assert_eq!(NO_TERMINAL, said);
    }

    #[test]
    fn gdd_init_without_the_flags_of_the_project_in_a_terminal_asks_the_form() {
        let asks = |command: &str, stdin, stdout| asks_form(&words(command), stdin, stdout);
        let alone = init::Options {
            name: String::new(),
            platform: String::new(),
            stacks: Vec::new(),
            agents: Vec::new(),
            language: "en".to_string(),
            description: String::new(),
            sensitive: Vec::new(),
            accept_same_family: false,
        };
        assert_eq!(asks("init", true, true), Some(alone.clone()));
        let command = "init --language es --agents codex claude --description A notes app \
                       --sensitive Losing notes --sensitive Leaks --accept-same-family";
        let given = init::Options {
            agents: vec!["codex".to_string(), "claude".to_string()],
            language: "es".to_string(),
            description: "A notes app".to_string(),
            sensitive: vec!["Losing notes".to_string(), "Leaks".to_string()],
            accept_same_family: true,
            ..alone
        };
        assert_eq!(asks(command, true, true), Some(given));
        assert_eq!(asks("init", false, true), None);
        assert_eq!(asks("init", true, false), None);
        for command in [
            "init --name x --platform cli --stack rust",
            "init --name x",
            "init --help",
            "status",
            "",
        ] {
            assert_eq!(asks(command, true, true), None, "{command:?}");
        }
    }

    /// An environment for `gdd init`: the `PATH` of the tests, for git, an author, and no git
    /// configuration of the user's.
    pub(super) fn init_env() -> Vec<(String, String)> {
        let path = std::env::var("PATH").unwrap_or_default();
        let home = std::env::temp_dir().display().to_string();
        let vars = [
            ("PATH", path.as_str()),
            ("HOME", home.as_str()),
            ("GIT_AUTHOR_NAME", "GDD Test"),
            ("GIT_AUTHOR_EMAIL", "test@example.com"),
            ("GIT_COMMITTER_NAME", "GDD Test"),
            ("GIT_COMMITTER_EMAIL", "test@example.com"),
            ("GIT_CONFIG_GLOBAL", "/dev/null"),
            ("GIT_CONFIG_NOSYSTEM", "1"),
        ];
        let owned = |(name, value): (&str, &str)| (name.to_string(), value.to_string());
        vars.into_iter().map(owned).collect()
    }

    /// A folder that is not a GDD project, removed when dropped.
    pub(super) fn plain_folder() -> Project {
        let folder = Project::new();
        fs::remove_dir_all(folder.0.join("gdd")).unwrap();
        folder
    }

    #[test]
    fn init_sets_up_the_folder_it_runs_in_from_the_words_of_each_option() {
        let folder = plain_folder();
        let command = "init --name My notes --platform web --stack rust --agents codex claude \
                       --description A notes app --sensitive Losing notes --sensitive Leaks \
                       --language es";
        let out = run_with_env(&words(command), &folder.0, &init_env());
        assert_eq!((out.code, out.stderr.as_str()), (0, ""), "{out:?}");
        assert!(out.stdout.starts_with("gdd/project.toml\n"), "{out:?}");
        let next = " $gdd-setup in Codex).\n";
        assert!(out.stdout.ends_with(next), "{out:?}");
        let read = |path: &str| fs::read_to_string(folder.0.join(path)).unwrap();
        let project = read(PROJECT);
        for line in [
            "name = \"My notes\"\n",
            "description = \"A notes app\"\n",
            "language = \"es\"   #",
            "platform = \"web\"   #",
        ] {
            assert!(project.contains(line), "{line}: {project}");
        }
        let brief = read("docs/brief.md");
        let sensitive = "\n1. Losing notes\n2. Leaks\n";
        assert!(brief.ends_with(sensitive), "{brief}");
        let models = read("gdd/models.toml");
        let codex = "[builder.red]\nintegration = \"codex\"\n";
        assert!(models.contains(codex), "{models}");
        let again = run_with_env(&words(command), &folder.0, &init_env());
        let project = folder.0.join(PROJECT);
        let twice = format!("error: already a GDD project: {}\n", project.display());
        assert_eq!(again, Outcome::failure(twice));
    }

    #[test]
    fn each_sensitive_option_names_one_area() {
        let words = ["x", "--sensitive", "a", "b", "--sensitive", "c"];
        let command = Sensitive::augment_args(clap::Command::new("x"));
        let matches = command.try_get_matches_from(words).unwrap();
        let areas = Sensitive::from_arg_matches(&matches).unwrap();
        let expected = Sensitive(vec!["a b".into(), "c".into()]);
        assert_eq!(areas, expected);
        let update = Sensitive::augment_args_for_update(clap::Command::new("x"));
        let matches = update.try_get_matches_from(words[..3].to_vec()).unwrap();
        let mut updated = expected;
        updated.update_from_arg_matches(&matches).unwrap();
        assert_eq!(updated, Sensitive(vec!["a".into()]));
        let none = Sensitive::augment_args(clap::Command::new("x"));
        let matches = none.try_get_matches_from(["x"]).unwrap();
        let areas = Sensitive::from_arg_matches(&matches).unwrap();
        assert_eq!(areas, Sensitive::default());
    }

    #[test]
    fn init_names_each_file_already_there_and_writes_nothing() {
        let folder = plain_folder();
        folder.write("docs/brief.md", "mine\n");
        folder.write("bin/gdd", "mine\n");
        let command = "init --name Notes --platform cli --stack rust --agents claude codex";
        let out = run_with_env(&words(command), &folder.0, &init_env());
        let said = "docs/brief.md already exists\nbin/gdd already exists\n\
                    error: gdd init never overwrites a file: move or remove these, then run it \
                    again\n";
        assert_eq!(out, Outcome::failure(said.to_string()));
        assert!(!folder.0.join("gdd").exists());
        let cobol = words("init --name N --platform cli --stack cobol");
        let out = run_with_env(&cobol, &folder.0, &[]);
        let unknown = "error: unknown stack cobol (known: python, rust, typescript)\n";
        assert_eq!(out, Outcome::failure(unknown.to_string()));
    }

    #[test]
    fn bootstrap_says_each_install_and_why_it_stopped() {
        use std::os::unix::fs::PermissionsExt;
        let folder = plain_folder();
        let command = "init --name Notes --platform cli --stack rust --agents claude codex";
        let out = run_with_env(&words(command), &folder.0, &init_env());
        assert_eq!(out.code, 0, "{out:?}");
        let cargo = ".gdd/bin/cargo";
        folder.write(cargo, "#!/bin/sh\necho lock > Cargo.lock\n");
        let mode = fs::Permissions::from_mode(0o755);
        fs::set_permissions(folder.0.join(cargo), mode).unwrap();
        let mut env = init_env();
        let bin = folder.0.join(".gdd/bin");
        for (name, value) in &mut env {
            if name == "PATH" {
                *value = format!("{}:{value}", bin.display());
            }
        }
        let out = run_with_env(&words("bootstrap"), &folder.0, &env);
        let said = "rust: cargo generate-lockfile\nCommitted: Cargo.lock\n\
                    Next: bin/gdd selftest\n";
        assert_eq!(out, Outcome::success(said.to_string()));
        folder.write(cargo, "#!/bin/sh\necho broken\nexit 2\n");
        let out = run_with_env(&words("bootstrap"), &folder.0, &env);
        let stopped = Outcome {
            code: 1,
            stdout: "rust: cargo generate-lockfile\nbroken\n".to_string(),
            stderr: "error: rust: cargo generate-lockfile failed (exit 2)\n".to_string(),
        };
        assert_eq!(out, stopped);
        folder.write(PROJECT, "stack = 3\n");
        let out = run_with_env(&words("bootstrap"), &folder.0, &env);
        let wrong = "error: gdd/project.toml: invalid TOML: stack is not a name or a list of \
                     names\n";
        assert_eq!(out, Outcome::failure(wrong.to_string()));
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
    fn a_gate_an_agent_runs_saves_its_report_and_leaves_the_log_alone() {
        let project = started_with_commands("[g2]\nlint = \"true\"\n");
        project.write(log::PATH, LOG);
        let mut env = process_env();
        env.push(("GDD_ROLE".to_string(), "builder-build".to_string()));
        for gate in ["g2", "g3"] {
            let command = words(&format!("{gate} save --role builder-build"));
            let printed = json(&run_with_env(&command, &project.0, &env));
            let path = project.0.join(format!(".gdd/changes/save/{gate}.json"));
            let saved: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
            assert_eq!(saved, printed, "{gate}");
        }
        assert_eq!(fs::read_to_string(project.0.join(log::PATH)).unwrap(), LOG);
    }

    #[test]
    fn a_gate_that_passes_exits_with_0_even_when_strict() {
        let project = started_with_commands("[g3]\nacceptance = \"true\"\n");
        project.write("features/a.feature", "Feature: a\n  Scenario: one\n");
        let out = project.run("g3 save --strict");
        assert_eq!(out.code, 0, "{out:?}");
        let printed = json(&out);
        let passed = ["acceptance", "path-guard", "scenarios (1)", "exceptions"];
        assert_eq!(printed["passed"], serde_json::json!(passed));
        assert_eq!(out.stderr, "");
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
    fn g3_names_the_exceptions_the_change_adds_and_the_inbox_shows_them() {
        let project = started_with_commands("[g3]\nacceptance = \"true\"\n");
        project.write("features/a.feature", "Feature: a\n  Scenario: one\n");
        let deny = "[advisories]\nignore = [{ id = \"RUSTSEC-1\", reason = \"r\" }]\n";
        project.write("deny.toml", deny);
        let out = project.run("g3 save");
        let line = "exception added: RUSTSEC-1 · deny.toml · no end date · r";
        assert_eq!(out.stderr, format!("{line}\n"));
        let printed = json(&out);
        assert_eq!(printed["added_exceptions"], serde_json::json!([line]));
        let saved = fs::read_to_string(project.0.join(".gdd/changes/save/g3.json")).unwrap();
        let saved: serde_json::Value = serde_json::from_str(&saved).unwrap();
        assert_eq!(saved, printed);
        let inbox = project.run("inbox save").stdout;
        let group = format!("\n## Security exceptions this change adds\n- {line}\n");
        assert!(inbox.contains(&group), "{inbox}");
        let status = project.run("status").stdout;
        let listed = "\nSecurity exceptions: 1\n  - RUSTSEC-1 · deny.toml · no end date · r\n";
        assert!(status.contains(listed), "{status}");
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

    #[test]
    fn without_an_id_the_commands_of_the_agents_take_gdd_change() {
        let project = started_with_commands("[g2]\nlint = \"true\"\n[g3]\nlint = \"true\"\n");
        let missing = Outcome::failure(format!("error: {MISSING_CHANGE}\n"));
        let unknown =
            "error: change nope does not exist: there is no .gdd/changes/nope/change.json\n";
        let invalid =
            "error: invalid change id \"Save\": use lowercase letters, digits and hyphens\n";
        for command in [
            "classify",
            "guard",
            "snapshot",
            "inbox",
            "g2",
            "g3",
            "prepare",
            "merge",
            "release pre",
        ] {
            let run = |id: &str| {
                let env = [("GDD_CHANGE".to_string(), id.to_string())];
                run_with_env(&words(command), &project.0, &env)
            };
            let unset = run_with_env(&words(command), &project.0, &[]);
            assert_eq!(unset, missing, "{command}");
            assert_eq!(run(""), missing, "{command}");
            assert_eq!(run("nope"), Outcome::failure(unknown.into()), "{command}");
            assert_eq!(run("Save"), Outcome::failure(invalid.into()), "{command}");
        }
        let env = [("GDD_CHANGE".to_string(), "save".to_string())];
        let decision = json(&run_with_env(&words("classify"), &project.0, &env));
        assert_eq!(decision["change"], "save");
        let out = run_with_env(&words("g2 --strict"), &project.0, &env);
        let passed = serde_json::json!(["lint", "path-guard"]);
        assert_eq!(json(&out)["passed"], passed);
        // An id given wins over GDD_CHANGE.
        let out = run_with_env(&words("classify nope"), &project.0, &env);
        assert_eq!(out, Outcome::failure(unknown.into()));
    }

    /// Gate commands with a failing test for the self-test, and a simulated release.
    const SELFTEST_COMMANDS: &str = "[g2]\nunit = \"test ! -f t.txt\"\n\
                                     [selftest]\nfile = \"t.txt\"\ncontent = \"x\"\n\
                                     [release]\nsimulated = true\n";

    #[test]
    fn selftest_prints_its_checks_saves_them_and_fails_unless_all_pass() {
        let project = Project::new();
        project.write(RULES, "[protected]\nall = [\"gdd/**\", \".github/**\"]\n");
        project.write(".github/workflows/gdd.yml", "run: bin/gdd ci\n");
        project.write(COMMANDS, SELFTEST_COMMANDS);
        let out = project.run("selftest");
        assert_eq!((out.code, out.stderr.as_str()), (0, ""), "{out:?}");
        let first = "OK a failing test turns G2 red\n";
        assert!(out.stdout.starts_with(first), "{out:?}");
        assert!(out.stdout.ends_with("\nharness 4/4\n"), "{out:?}");
        assert!(!project.0.join("t.txt").exists());
        let status = project.run("status").stdout;
        let line = "\nHarness self-test: harness 4/4\n";
        assert!(status.contains(line), "{status}");
        assert_eq!(log_records(&project)[0].text("agent"), "selftest");
        let passing = SELFTEST_COMMANDS.replace("test ! -f", "test -f");
        project.write(COMMANDS, &passing);
        let out = project.run("selftest");
        assert_eq!(out.code, 1, "{out:?}");
        assert!(out.stdout.ends_with("\nharness 3/4\n"), "{out:?}");
        project.write(COMMANDS, "[selftest\n");
        let out = project.run("selftest");
        let start = "error: gdd/commands.toml: invalid TOML";
        assert!(out.stderr.starts_with(start), "{out:?}");
    }

    #[test]
    fn ci_checks_the_branch_github_names_else_the_current_one() {
        let project = committed_project();
        project.write(COMMANDS, "[g2]\nunit = \"true\"\n[g3]\nunit = \"true\"\n");
        project.write("features/a.feature", "Feature: a\n  Scenario: one\n");
        git(&project, &["add", "-A"]);
        git(&project, &["commit", "-q", "-m", "gates"]);
        let ci = |head: &str| {
            let env = [("GITHUB_HEAD_REF".to_string(), head.to_string())];
            run_with_env(&words("ci"), &project.0, &env)
        };
        let main = "Branch main: nothing to check per change.\n";
        assert_eq!(ci(""), Outcome::success(main.into()));
        let passed = "G2 green\nG3 green\nCI passed\n";
        assert_eq!(ci("change/c1"), Outcome::success(passed.into()));
        git(&project, &["checkout", "-q", "-b", "change/c1"]);
        project.write("gdd/x.toml", "x\n");
        git(&project, &["add", "-A"]);
        git(&project, &["commit", "-q", "-m", "harness"]);
        let failed = "gdd/x.toml: harness path changed inside a change\n\
                      G2 green\nG3 green\nCI failed\n";
        let out = ci("");
        assert_eq!((out.code, out.stdout.as_str()), (1, failed));
        assert!(!project.0.join(".gdd").exists());
    }

    #[test]
    fn session_imports_the_transcripts_under_the_home_of_its_environment() {
        let project = Project::new();
        let session = words("session --phase 2-build --change c1");
        let out = run_with_env(&session, &project.0, &[]);
        assert_eq!(out, Outcome::failure(format!("error: {NO_HOME}\n")));
        let home = Project::new();
        let root = fs::canonicalize(&project.0).unwrap();
        let named = |c: char| if c.is_ascii_alphanumeric() { c } else { '-' };
        let folder: String = root.display().to_string().chars().map(named).collect();
        let line = r#"{"uuid": "u1", "message": {"model": "m", "usage": {"output_tokens": 4}}}"#;
        home.write(&format!(".claude/projects/{folder}/s1.jsonl"), line);
        let env = [("HOME".to_string(), home.0.display().to_string())];
        let out = run_with_env(&session, &project.0, &env);
        let said = "Session imported: 1 new requests across 1 model(s), phase 2-build.\n";
        assert_eq!(out, Outcome::success(said.into()));
        let records = log_records(&project);
        assert_eq!(records[0].text("change"), "c1");
        assert_eq!(records[0].tokens_out, 4);
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
            "Security exceptions: none\n",
            "Known debt: none\n",
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
            "G0: pending · G0 pending: run the Setup (the gdd-setup command).\n",
            "Red zones: pending · Red zones not approved yet: the Orchestrator proposes them at ",
            "the end of Discover.\n",
            "Harness self-test: not run\n",
            "Scenarios in features/: 0\n",
            "Security exceptions: none\n",
            "Known debt: none\n",
            "Changes:\n"
        );
        assert_eq!(project.run("status"), Outcome::success(expected.into()));
        project.write("osv-scanner.toml", "[[IgnoredVulns]");
        let out = project.run("status");
        let line = "\nSecurity exceptions: osv-scanner.toml: invalid TOML: ";
        assert_eq!(out.code, 0, "{out:?}");
        assert!(out.stdout.contains(line), "{out:?}");
        project.write(debt::PATH, "[[debt]");
        let out = project.run("status");
        let line = "\nKnown debt: gdd/debt.toml: invalid TOML: ";
        assert_eq!(out.code, 0, "{out:?}");
        assert!(out.stdout.contains(line), "{out:?}");
    }

    #[test]
    fn adopt_records_the_known_debt_once_and_the_status_lists_it() {
        let project = committed_project();
        identify(&project);
        let commands = "[g2]\nlint = \"echo unused import; exit 1\"\nunit = \"true\"\n";
        project.write(COMMANDS, commands);
        let out = project.run("adopt");
        assert_eq!((out.code, out.stderr.as_str()), (0, ""), "{out:?}");
        let start = "Passed: unit\nKnown debt: lint\nProposed Refactors, riskiest first:\n  \
                     1. debt-lint · Pay the known debt of lint · .gdd/adoption/lint.txt\n";
        assert!(out.stdout.starts_with(start), "{out:?}");
        assert!(out.stdout.ends_with("--paths ...\n"), "{out:?}");
        let kept = fs::read_to_string(project.0.join(".gdd/adoption/lint.txt")).unwrap();
        assert_eq!(kept, "unused import\n");
        let subject = git(&project, &["log", "-1", "--format=%s"]);
        assert_eq!(subject, "gdd adopt: known debt in lint");
        let status = project.run("status").stdout;
        let since = exceptions::today();
        let line = format!("  - lint · since {since} · Refactor debt-lint\n");
        let listed = format!("\nKnown debt: lint\n{line}");
        assert!(status.contains(&listed), "{status}");
        let again = project.run("adopt");
        let refused = format!("error: this project was already adopted on {since}");
        let refused = format!("{refused} (gdd/debt.toml)\n");
        assert_eq!(again, Outcome::failure(refused));
        project.write(COMMANDS, "[g2\n");
        let out = project.run("adopt");
        let start = "error: gdd/commands.toml: invalid TOML";
        assert!(out.stderr.starts_with(start), "{out:?}");
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
    fn status_names_the_agents_on_the_path_that_could_review_a_project_of_one_family() {
        use std::os::unix::fs::PermissionsExt;
        let project = Project::new();
        let claude = "integration = \"claude\"\nmodel = \"opus\"\n";
        let roles = format!("[builder.red]\n{claude}[reviewer]\n{claude}");
        let families = "[families]\n\"claude\" = \"anthropic\"\n";
        let accepted = "same_family_accepted = \"2026-10-27\"\n";
        let models = format!("{roles}{accepted}{families}");
        project.write(agents::models::PATH, &models);
        for agent in ["claude", "codex"] {
            let program = format!("agents/{agent}");
            project.write(&program, "#!/bin/sh\n");
            let runnable = fs::Permissions::from_mode(0o755);
            let file = project.0.join(&program);
            fs::set_permissions(file, runnable).unwrap();
        }
        let path = project.0.join("agents").display().to_string();
        let env = [("PATH".to_string(), path)];
        let out = run_with_env(&words("status"), &project.0, &env);
        let line = "\nA second model family is on this machine (codex): set it as the Reviewer \
                    in gdd/models.toml\n";
        assert!(out.stdout.ends_with(line), "{out:?}");
        let models = format!("{roles}{families}");
        project.write(agents::models::PATH, &models);
        let out = run_with_env(&words("status"), &project.0, &env);
        assert_eq!(out.code, 0, "{out:?}");
        assert!(!out.stdout.contains("second model family"), "{out:?}");
        project.write(agents::models::PATH, "[reviewer");
        let out = run_with_env(&words("status"), &project.0, &env);
        let start = "error: gdd/models.toml: invalid TOML: ";
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
        let commands = "[g2]\nlint = \"true\"\n[release]\nsimulated = true\nbuild = \"true\"\n";
        launched_with(commands, MODELS)
    }

    /// The launched project, with the gate commands `commands`, the models `models` and a
    /// scenario on `main`.
    fn launched_with(commands: &str, models: &str) -> Project {
        let project = Project::new();
        project.write(RULES, PROTECTED);
        project.write(ZONES, ZONE_MAP);
        project.write("gdd/models.toml", models);
        project.write(COMMANDS, commands);
        project.write("features/a.feature", "Feature: a\n  Scenario: one\n");
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
    fn a_merge_waits_for_the_merge_of_another_change_and_says_so_apart_from_its_json() {
        let project = launched_project();
        assert_eq!(project.run("gate G3 ok c1 --by machines").code, 0);
        let lock = project.0.join(queue::LOCK);
        fs::write(&lock, "").unwrap();
        let (taken, wait_taken) = std::sync::mpsc::channel();
        let another = std::thread::spawn(move || {
            let held = fs::File::open(&lock).unwrap();
            held.lock().unwrap();
            taken.send(()).unwrap();
            std::thread::sleep(Duration::from_millis(300));
        });
        wait_taken.recv().unwrap();
        let out = project.run("merge c1");
        another.join().unwrap();
        assert_eq!(out.stderr, format!("{}\n", queue::WAITING));
        assert_eq!(json(&out)["detail"]["name"], "deploy_pre");
        assert_eq!(record_of(&project, "c1")["state"], "merged");
    }

    /// Gate commands whose G3 fails once main has `docs/breaking.md`.
    const BREAKABLE: &str = "[g3]\nfresh = \"test ! -f docs/breaking.md\"\n\
                             [release]\nsimulated = true\nbuild = \"true\"\n";

    /// The launched project with [`BREAKABLE`] gates and G3 open on change `c1`, once main got
    /// the file `path` after the branch of the change left it.
    fn moved_under_c1(path: &str) -> Project {
        let project = launched_with(BREAKABLE, MODELS);
        assert_eq!(project.run("gate G3 ok c1 --by machines").code, 0);
        git(&project, &["checkout", "-q", "main"]);
        project.write(path, "x\n");
        git(&project, &["add", path]);
        git(&project, &["commit", "-q", "-m", path]);
        git(&project, &["checkout", "-q", "change/c1"]);
        project
    }

    const MOVED_UNDER_C1: &str = "main moved under c1: G3 again on top of it\n";

    #[test]
    fn a_merge_on_top_of_a_main_that_moved_passes_g3_again_there_first() {
        let project = moved_under_c1("docs/news.md");
        let out = project.run("merge c1");
        assert_eq!(out.stderr, MOVED_UNDER_C1);
        assert_eq!(json(&out)["detail"]["name"], "deploy_pre");
        let record = record_of(&project, "c1");
        assert_eq!(record["state"], "merged");
        let g3 = &record["gates"]["G3"];
        assert_eq!(g3["decision"], "ok");
        assert_eq!(g3["by"], "machines");
        assert_eq!(g3["note"], "automatic chain on top of the latest main");
        git(&project, &["cat-file", "-e", "main:docs/news.md"]);
        let took = git(&project, &["log", "-1", "--format=%s", "main^2"]);
        assert_eq!(took, "gdd(c1): take the latest main");
    }

    #[test]
    fn g3_red_on_top_of_the_latest_main_parks_the_change_and_leaves_g3_closed() {
        let project = moved_under_c1("docs/breaking.md");
        let out = project.run("merge c1");
        assert_eq!(out.code, 1, "{out:?}");
        let parked = "\nChange c1 parked: G3 red on top of the latest main.";
        assert!(out.stderr.starts_with(MOVED_UNDER_C1), "{out:?}");
        assert!(out.stderr.contains(parked), "{out:?}");
        let record = record_of(&project, "c1");
        assert_eq!(record["state"], "parked");
        assert_eq!(record["gates"]["G3"]["decision"], "ko");
        // Its branch has the latest main now, and still does not merge without G3.
        let main = git(&project, &["rev-parse", "main"]);
        let again = project.run("merge c1");
        let closed = "error: G3 is not open.\n";
        assert!(again.stderr.ends_with(closed), "{again:?}");
        assert_eq!(git(&project, &["rev-parse", "main"]), main);
    }

    #[test]
    fn what_the_recipe_says_on_its_way_comes_before_what_it_says_at_its_end() {
        let project = Project::new();
        let limits = Limits::default();
        let settings = invoke::Settings::default();
        let mut steps = ProjectSteps::new(&project.0, &project.0, "c1", &limits, settings, &[]);
        recipe::Steps::say(&mut steps, "one");
        recipe::Steps::say(&mut steps, "two");
        assert_eq!(steps.said, "one\ntwo\n");
    }

    #[test]
    fn a_cap_that_is_no_integer_from_one_on_is_an_error_that_names_the_rules_file() {
        let project = launched_project();
        git(&project, &["checkout", "-q", "main"]);
        let rules = format!("{PROTECTED}\n[parallel]\nmax = 0\n");
        project.write(RULES, &rules);
        let out = project.run("launch c1");
        let start = "error: gdd/rules.toml: invalid TOML: ";
        assert!(out.stderr.starts_with(start), "{out:?}");
        assert!(out.stderr.contains("max = 0"), "{out:?}");
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

    #[test]
    fn launch_and_resume_say_what_they_refuse() {
        let project = launched_project();
        let out = project.run("resume c1 --g4 ok");
        let not_paused = "error: Change c1 is not paused at G4.\n";
        assert_eq!(out, Outcome::failure(not_paused.into()));
        let out = project.run("resume c1 --g4 polish");
        let polish = "error: polish needs your notes: gdd resume c1 --g4 polish --note ...\n";
        assert_eq!(out, Outcome::failure(polish.into()));
        let args = words("resume c1 --g4 ok --note Fine by me");
        let builder = run_as(&args, &project.0, Some("builder-test"));
        let refused = "error: A Builder or Reviewer cannot open or close gates.\n";
        assert_eq!(builder, Outcome::failure(refused.into()));
        // The role of a run with an environment of its own is its GDD_ROLE.
        let role = |role: &str| ("GDD_ROLE".to_string(), role.to_string());
        let env = [role("human"), role("reviewer")];
        let reviewer = run_with_env(&args, &project.0, &env);
        assert_eq!(reviewer, Outcome::failure(refused.into()));
        let person = run_with_env(&words("resume c1 --g4 ok"), &project.0, &[]);
        let not_paused = "error: Change c1 is not paused at G4.\n";
        assert_eq!(person, Outcome::failure(not_paused.into()));
        git(&project, &["checkout", "-q", "main"]);
        project.write(ZONES, "default = \"red\"\n");
        let dirty = "error: The working tree has uncommitted changes. Commit them before \
                     launching a change.\n";
        assert_eq!(project.run("launch c1"), Outcome::failure(dirty.into()));
        assert_eq!(git(&project, &["branch", "--show-current"]), "main");
    }

    #[test]
    fn without_an_environment_of_its_own_gdd_runs_in_the_one_of_the_process() {
        let path = std::env::var("PATH").unwrap_or_default();
        assert!(process_env().contains(&("PATH".to_string(), path)));
    }

    /// The roles `MODELS` lacks: the Builder in test mode and the Orchestrator.
    const MORE_ROLES: &str = "[builder_test]\nintegration = \"claude\"\nmodel = \"sonnet\"\n\
                              [orchestrator]\nintegration = \"claude\"\nmodel = \"opus\"\n";

    #[test]
    fn launch_starts_nothing_until_every_role_can_be_started() {
        let project = launched_project();
        let before = record_of(&project, "c1")["state"].clone();
        let out = project.run("launch c1");
        let nameless = "error: gdd/models.toml: the Orchestrator has no model\n";
        assert_eq!(out, Outcome::failure(nameless.into()));
        assert_eq!(record_of(&project, "c1")["state"], before);
        // The models of the change are the ones on its branch, in its worktree.
        let worktree = project.worktrees().join("c1");
        let gemini = MODELS.replace("\"codex\"", "\"gemini\"");
        let models = format!("{gemini}{MORE_ROLES}");
        fs::write(worktree.join("gdd/models.toml"), models).unwrap();
        let committed = std::process::Command::new("git")
            .args(GIT_CONFIG)
            .args(["commit", "-q", "-am", "gemini"])
            .current_dir(&worktree)
            .status()
            .unwrap();
        assert!(committed.success());
        let out = project.run("launch c1");
        let no_sheet = "error: gdd/models.toml: the Reviewer uses \"gemini\", which has no agent \
                        sheet (claude, codex, opencode)\n";
        assert_eq!(out, Outcome::failure(no_sheet.into()));
        assert_eq!(record_of(&project, "c1")["state"], before);
    }

    #[test]
    fn an_agent_that_fails_parks_the_change_and_fails_the_run() {
        let project = launched_project();
        project.write("gdd/models.toml", &format!("{MODELS}{MORE_ROLES}"));
        let env = with_agents(&project, "#!/bin/sh\nexit 1\n");
        let out = launch_in(&project, &env);
        let parked = "Change c1 parked: the Builder in test mode failed (exit 1). Message in \
                      .gdd/changes/c1/inbox.md\n";
        assert_eq!(out, Outcome::failure(parked.into()));
        assert_eq!(record_of(&project, "c1")["state"], "parked");
        // The work of the earlier run on its branch: committed before the relaunch.
        let subject = git(&project, &["log", "-1", "--format=%s", "change/c1"]);
        let relaunch = "gdd(c1): work of the earlier run, before relaunch";
        assert_eq!(subject, relaunch);
        // The change waits in its worktree; this checkout is back on main.
        assert_eq!(git(&project, &["branch", "--show-current"]), "main");
        assert!(project.worktrees().join("c1/gdd/models.toml").is_file());
    }

    #[test]
    fn launch_starts_nothing_until_the_program_of_each_agent_is_on_the_path() {
        let project = launched_project();
        project.write("gdd/models.toml", &format!("{MODELS}{MORE_ROLES}"));
        git(&project, &["commit", "-q", "-am", "roles"]);
        let before = record_of(&project, "c1")["state"].clone();
        with_agents(&project, "#!/bin/sh\n");
        fs::remove_file(project.0.join(".gdd/bin/codex")).unwrap();
        // Only the folder of the fakes: the tests may have a real Codex on their PATH.
        let fakes = project.0.join(".gdd/bin").display().to_string();
        let mut env = vec![("PATH".to_string(), fakes)];
        let missing = "error: cannot find the codex program of Codex on PATH\n";
        assert_eq!(launch_in(&project, &env), Outcome::failure(missing.into()));
        assert_eq!(record_of(&project, "c1")["state"], before);
        // The environment gdd runs in is the one given, not the process one.
        env.clear();
        let claude = "error: cannot find the claude program of Claude Code on PATH\n";
        assert_eq!(launch_in(&project, &env), Outcome::failure(claude.into()));
        let wrong = format!("{MODELS}{MORE_ROLES}[env]\npass = \"X\"\n");
        project.write("gdd/models.toml", &wrong);
        let out = launch_in(&project, &env);
        let start = "error: gdd/models.toml: ";
        assert!(out.stderr.starts_with(start), "{out:?}");
    }

    /// Gate commands that pass once the Builder writes `src/a.rs`.
    const RECIPE_COMMANDS: &str = "[g2]\nunit = \"test -f src/a.rs\"\n[g3]\nunit = \"true\"\n\
                                   [release]\nsimulated = true\n";

    /// One agent for every integration (see [`with_agents`]): it keeps its role in
    /// `.gdd/fake/calls` of the state root, a folder that is not GDD state; test mode writes
    /// `@TEST@`, build mode `src/a.rs`, and the Reviewer, of Codex, says OK.
    const FAKE_AGENT: &str = r#"#!/bin/sh
mkdir -p "$GDD_STATE/.gdd/fake"
echo "$GDD_ROLE" >> "$GDD_STATE/.gdd/fake/calls"
case "$GDD_ROLE" in
  builder-test) mkdir -p "$(dirname @TEST@)" && echo t > @TEST@ ;;
  builder-build) mkdir -p src && echo b > src/a.rs ;;
  reviewer) echo '{"type":"item.completed","item":{"type":"agent_message","text":"VERDICT: OK"}}' ;;
esac
"#;

    /// An environment whose `PATH` finds first the agents of `project`: `script`, as `claude`
    /// and `codex` in its `.gdd/bin/`.
    fn with_agents(project: &Project, script: &str) -> Vec<(String, String)> {
        use std::os::unix::fs::PermissionsExt;
        for agent in ["claude", "codex"] {
            let path = format!(".gdd/bin/{agent}");
            project.write(&path, script);
            let mode = fs::Permissions::from_mode(0o755);
            fs::set_permissions(project.0.join(path), mode).unwrap();
        }
        let tests = std::env::var("PATH").unwrap_or_default();
        let path = format!("{}:{tests}", project.0.join(".gdd/bin").display());
        vec![("PATH".to_string(), path)]
    }

    /// Runs `gdd launch c1` on `project` in the environment `env`.
    fn launch_in(project: &Project, env: &Env) -> Outcome {
        run_with_env(&words("launch c1"), &project.0, env)
    }

    /// Launches change `c1` of `project` with the fake agents, whose test mode writes
    /// `test_writes`. Each call is kept in `.gdd/fake/calls`.
    fn launch_with_fakes(project: &Project, test_writes: &str) -> Outcome {
        let env = with_agents(project, &FAKE_AGENT.replace("@TEST@", test_writes));
        launch_in(project, &env)
    }

    #[test]
    fn launch_runs_the_recipe_to_production() {
        let models = format!("{MODELS}{MORE_ROLES}");
        let project = launched_with(RECIPE_COMMANDS, &models);
        let out = launch_with_fakes(&project, "tests/t.txt");
        let closed = "Change c1 closed. Message: .gdd/changes/c1/inbox.md · Tokens: gdd report \
                      --change c1\n";
        assert_eq!(out, Outcome::success(closed.into()));
        let calls = fs::read_to_string(project.0.join(".gdd/fake/calls")).unwrap();
        assert_eq!(calls, "builder-test\nbuilder-build\nreviewer\n");
        let review = project.0.join(".gdd/changes/c1/outputs/reviewer-1.md");
        assert_eq!(fs::read_to_string(review).unwrap(), "VERDICT: OK");
        let log = fs::read_to_string(project.0.join(log::PATH)).unwrap();
        let lines = log::parse(&log).unwrap();
        let notes = lines.iter().map(|line| line.text("note"));
        let agents: Vec<String> = notes.filter(|note| note.starts_with("/gdd-")).collect();
        assert_eq!(agents, ["/gdd-build-test", "/gdd-build", "/gdd-review"]);
        let record = record_of(&project, "c1");
        assert_eq!(record["state"], "closed");
        let opened = "automatic chain and Reviewer";
        assert_eq!(record["gates"]["G3"]["note"], opened);
        assert_eq!(record["gates"]["G4"]["by"], "machines");
        for saved in ["g2.json", "g3.json", "snapshot-build.json"] {
            let path = project.0.join(".gdd/changes/c1").join(saved);
            assert!(path.is_file(), "{saved}");
        }
        let merged = git(&project, &["log", "-1", "--format=%s", "main"]);
        assert_eq!(merged, "gdd(c1): G3 Merge Gate · App");
    }

    #[test]
    fn test_mode_that_writes_the_code_parks_the_change() {
        let models = format!("{MODELS}{MORE_ROLES}");
        let project = launched_with(RECIPE_COMMANDS, &models);
        let out = launch_with_fakes(&project, "src/x.rs");
        let parked = "Change c1 parked: Path Guard failed after test mode: src/x.rs: protected \
                      path for builder-test. Message in .gdd/changes/c1/inbox.md\n";
        assert_eq!(out, Outcome::failure(parked.into()));
    }

    #[test]
    fn resume_answers_g4_with_the_rest_of_the_words_as_the_note() {
        let project = launched_project();
        project.write(".gdd/changes/c1/run.json", r#"{"paused_at": "G4"}"#);
        let out = project.run("resume c1 --g4 ko --note Not what I asked");
        let rejected = "Change c1 rejected at G4: production untouched.\n";
        assert_eq!(out, Outcome::success(rejected.into()));
        let record = record_of(&project, "c1");
        let g4 = &record["gates"]["G4"];
        assert_eq!(g4["decision"], "ko");
        assert_eq!(g4["by"], "human");
        assert_eq!(g4["note"], "Not what I asked");
        assert_eq!(record["state"], "rejected");
        assert!(!project.0.join(".gdd/changes/c1/run.json").exists());
    }

    /// The paths `gdd agents install` said it wrote.
    fn written(out: &Outcome) -> Vec<String> {
        serde_json::from_value(json(out)["written"].clone()).unwrap()
    }

    #[test]
    fn agents_install_writes_the_commands_and_check_names_what_changed() {
        let project = Project::new();
        project.write("gdd/models.toml", MODELS);
        let out = project.run("agents install");
        let start = "{\"ok\":true,\"written\":[\".claude/skills/gdd-build/SKILL.md\",";
        assert!(out.stdout.starts_with(start), "{out:?}");
        let paths = written(&out);
        assert_eq!(paths.len(), 22);
        assert_eq!(paths[11], ".agents/skills/gdd-build/SKILL.md");
        assert!(paths.iter().all(|path| project.0.join(path).is_file()));
        let ok = Outcome::success("{\"ok\":true}\n".into());
        assert_eq!(project.run("agents check"), ok);
        let build = ".claude/skills/gdd-build/SKILL.md";
        project.write(build, "edited\n");
        let review = ".agents/skills/gdd-review/SKILL.md";
        fs::remove_file(project.0.join(review)).unwrap();
        let differs = format!("error: {build}: differs from what gdd writes\n");
        let missing = format!("error: {review}: missing\n");
        let both = Outcome::failure(format!("{differs}{missing}"));
        assert_eq!(project.run("agents check"), both);
        let out = project.run("agents check --agent codex");
        assert_eq!(out, Outcome::failure(missing));
        let out = project.run("agents install --agent opencode --agent codex opencode");
        let paths = written(&out);
        assert_eq!(paths.len(), 22);
        assert_eq!(paths[0], ".opencode/commands/gdd-build.md");
        assert_eq!(project.run("agents check --agent codex"), ok);
    }

    #[test]
    fn agents_need_a_sheet_and_someone_to_name_them() {
        let project = Project::new();
        let none = "error: gdd/models.toml names no agent: name one with --agent (claude, codex, \
                    opencode)\n";
        for command in ["agents install", "agents check"] {
            let out = project.run(command);
            assert_eq!(out, Outcome::failure(none.into()), "{command}");
        }
        let gemini = "error: no agent sheet for \"gemini\" (sheets: claude, codex, opencode)\n";
        for command in [
            "agents install --agent gemini",
            "agents install --agent claude gemini",
            "agents check --agent gemini",
        ] {
            let out = project.run(command);
            assert_eq!(out, Outcome::failure(gemini.into()), "{command}");
        }
        assert!(!project.0.join(".claude").exists());
        // The models count only when no agent is named.
        project.write("gdd/models.toml", "[reviewer\n");
        let out = project.run("agents check");
        let start = "error: gdd/models.toml: invalid TOML";
        assert!(out.stderr.starts_with(start), "{out:?}");
        let out = project.run("agents install --agent claude");
        assert_eq!(written(&out).len(), 11);
    }

    #[test]
    fn agents_install_and_check_the_instructions_of_the_stacks_of_the_project() {
        let project = Project::new();
        project.write(PROJECT, "stack = [\"rust\", \"python\"]\n");
        let paths = written(&project.run("agents install --agent claude"));
        let instructions = [
            ".claude/skills/gdd-stack-rust/SKILL.md",
            ".claude/skills/gdd-stack-python/SKILL.md",
        ];
        assert_eq!(paths[11..], instructions);
        let ok = Outcome::success("{\"ok\":true}\n".into());
        assert_eq!(project.run("agents check --agent claude"), ok);
        project.write(PROJECT, "stack = \"typescript\"\n");
        let missing = "error: .claude/skills/gdd-stack-typescript/SKILL.md: missing\n";
        let out = project.run("agents check --agent claude");
        assert_eq!(out, Outcome::failure(missing.into()));
        project.write(PROJECT, "stack = \"\"\n");
        assert_eq!(project.run("agents check --agent claude"), ok);
    }

    #[test]
    fn agents_install_nothing_for_a_project_whose_stacks_cannot_be_read() {
        let project = Project::new();
        project.write(PROJECT, "stack = [\"rust\", \"cobol\"]\n");
        let unknown = "error: unknown stack cobol (known: python, rust, typescript)\n";
        for command in [
            "agents install --agent claude",
            "agents check --agent claude",
        ] {
            let out = project.run(command);
            assert_eq!(out, Outcome::failure(unknown.into()), "{command}");
        }
        project.write(PROJECT, "stack = 3\n");
        let out = project.run("agents install --agent claude");
        let wrong = "error: gdd/project.toml: invalid TOML: stack is not a name or a list of \
                     names\n";
        assert_eq!(out, Outcome::failure(wrong.into()));
        assert!(!project.0.join(".claude").exists());
    }

    #[test]
    fn stacks_lists_shows_and_composes_the_sheets() {
        let project = Project::new();
        let listed = format!("{}\n", stacks::list().unwrap());
        assert_eq!(project.run("stacks"), Outcome::success(listed));
        let shown = format!("{}\n", stacks::sheet("python").unwrap());
        let out = project.run("stacks show python");
        assert_eq!(out, Outcome::success(shown));
        let composed = stacks::compose(&words("rust typescript")).unwrap();
        let out = project.run("stacks compose rust typescript rust");
        assert_eq!(out, Outcome::success(format!("{composed}\n")));
        let unknown = "error: unknown stack cobol (known: python, rust, typescript)\n";
        for command in ["stacks show cobol", "stacks compose rust cobol"] {
            let out = project.run(command);
            assert_eq!(out, Outcome::failure(unknown.into()), "{command}");
        }
        let none = "error: name at least one stack: gdd stacks compose <name>...\n";
        let out = project.run("stacks compose");
        assert_eq!(out, Outcome::failure(none.into()));
    }
}
