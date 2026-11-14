//! `gdd`: parses the arguments, runs a command of the library, prints what it says and journals it.

use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use gdd::commands::{self as c, Ctx};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser)]
#[command(
    name = "gdd",
    version,
    about = "The deterministic layer of Gate-Driven Development (contract gdd/1): it keeps the order, the accounting and the evidence of each change; the agents keep the judgment."
)]
struct Cli {
    /// The project's root (default: the current directory).
    #[arg(long, global = true, default_value = ".")]
    root: PathBuf,
    /// The agents' environment of the blueprint's levels to launch in.
    #[arg(long, global = true)]
    env: Option<String>,
    /// The change's branch, when it isn't work/<id>, gdd-build/<id> or build/<id>.
    #[arg(long, global = true)]
    branch: Option<String>,
    /// The commit to judge, instead of the tip of the change's branch.
    #[arg(long, global = true)]
    head: Option<String>,
    /// Write nothing to the layer's journal (gdd/log/<id>.engine.jsonl).
    #[arg(long, global = true)]
    no_journal: bool,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// The single next step of a change, with the command that runs it; without an id, the project's.
    Next { id: Option<String> },
    /// Writes nothing: it refuses an event an agent composed and names the command that writes it.
    Log { id: String, event: String },
    /// Start waiting for the person: written at the moment the question goes out.
    Ask {
        id: String,
        #[arg(long)]
        of: String,
        what: String,
    },
    /// The person's answer to the open question, in their words.
    Answer { id: String, words: String },
    /// The person's approval, with each file's blob hash as they saw it.
    Approve {
        id: String,
        #[arg(long)]
        of: String,
        #[arg(long, default_value_t = 1)]
        version: i64,
        #[arg(long)]
        who: String,
        #[arg(long = "where")]
        place: String,
        files: Vec<String>,
    },
    /// A check or a limit the person waived, with who and why; a limit takes --limit and --to.
    Waiver {
        id: String,
        #[arg(long)]
        who: String,
        #[arg(long)]
        check: String,
        #[arg(long)]
        why: String,
        #[arg(long)]
        backlog: String,
        #[arg(long)]
        limit: Option<String>,
        #[arg(long)]
        to: Option<i64>,
    },
    /// Correct a line of a log with a new event; --void takes that line out of the change's state.
    Correct {
        id: String,
        #[arg(long)]
        line: usize,
        #[arg(long)]
        void: bool,
        what: String,
    },
    /// The brief step: the planned color from the paths, the base, and the sealed files' hashes.
    Brief {
        id: String,
        #[arg(long, num_args = 1.., required_unless_present = "stop")]
        paths: Vec<String>,
        /// Drop the change, saying why: a brief step whose result is stopped.
        #[arg(long)]
        stop: Option<String>,
        /// A color judgment raises it to; it never lowers the floor.
        #[arg(long)]
        color: Option<String>,
        #[arg(long, default_value = "")]
        why: String,
        #[arg(long)]
        level: Option<String>,
        #[arg(long)]
        skill_version: Option<String>,
        /// The person's request, word for word, kept in the brief step.
        #[arg(long)]
        request: Option<String>,
        /// The same, read from a file: a shell eats no quote of it.
        #[arg(long, conflicts_with = "request")]
        request_file: Option<String>,
        /// The brief and its scenarios, as sealed.
        #[arg(last = false)]
        files: Vec<String>,
    },
    /// Write a launched role's step from the report its last message ends with.
    Report {
        id: String,
        /// checks, build, review or reading.
        step: String,
        /// The role's last message: a file, or - for standard input.
        file: String,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        level: Option<String>,
        #[arg(long)]
        seconds: Option<f64>,
        #[arg(long)]
        tokens: Option<i64>,
        /// A JSON list of the checks role's proofs.
        #[arg(long)]
        proofs: Option<String>,
        /// A JSON list of the builder's claims.
        #[arg(long)]
        claims: Option<String>,
        /// Findings a review closes, as f1=fixed.
        #[arg(long, value_delimiter = ',')]
        closed: Vec<String>,
        /// Where each finding of a reading went, as f1=brief.
        #[arg(long, value_delimiter = ',')]
        went: Vec<String>,
    },
    /// What the blueprint's limits still allow a change, as gdd/limits.sh counts it.
    Limits { id: String },
    /// Integrity of the logs, for CI: exit 1 on an error.
    Verify {
        ids: Vec<String>,
        #[arg(long)]
        all: bool,
        #[arg(long)]
        no_git: bool,
    },
    /// The planned color of some paths, or the color of what a change touched.
    Color {
        id: Option<String>,
        #[arg(long, num_args = 1..)]
        paths: Vec<String>,
    },
    /// Each zone's pathspec that matches no tracked file, and the tracked files no zone covers.
    Zones,
    /// Commit files of the working copy on gdd-seal/<id>, without touching the checkout or its index.
    Seal {
        id: String,
        files: Vec<String>,
        /// Merge the main branch into the seal instead.
        #[arg(long)]
        merge_main: bool,
    },
    /// Run the checks the color asks for on the head of the change's branch, and write the gates step.
    Gates {
        id: String,
        /// A directory already at that commit with a clean tree, such as a CI checkout.
        #[arg(long)]
        at: Option<PathBuf>,
    },
    /// The exact command and prompt of a role; with --run, launch it and write its step.
    Launch {
        id: String,
        /// checks, build, review or reading.
        role: String,
        #[arg(long)]
        run: bool,
        /// A launch command with <model> and <prompt> (or <prompt file>), instead of the blueprint's.
        #[arg(long)]
        command: Option<String>,
        #[arg(long)]
        level: Option<String>,
        /// Seconds within which the role must have written something.
        #[arg(long, default_value_t = 60)]
        start_wait: u64,
        /// Seconds it may run; by default, the minutes the change has left.
        #[arg(long)]
        timeout: Option<u64>,
        /// How to read its output: claude, codex or plain (default: by its program).
        #[arg(long)]
        adapter: Option<String>,
    },
    /// Describe the merge request from the log, or record the ship once it is in the main branch.
    Ship {
        id: String,
        #[arg(long)]
        describe: bool,
        /// Record the ship once git shows the change's head in the main branch.
        #[arg(long = "done")]
        done: Option<String>,
        /// Commit the logs, the journals and the backlog on top of the change's branch.
        #[arg(long)]
        bookkeeping: bool,
        /// Ship by a local merge into the main branch, which the checkout is on.
        #[arg(long)]
        merge: bool,
    },
    /// The orchestrator's own steps in the blueprint's log: prepare, harden or exam.
    Step {
        /// blueprint
        id: String,
        /// prepare, harden or exam.
        step: String,
        /// prepare: proposed or approved; harden: proposed (the default).
        #[arg(long)]
        result: Option<String>,
        /// What it caught that others missed (prepare, harden), or how many questions (exam).
        #[arg(long)]
        caught: Option<String>,
        /// The level it ran at (the reviewer's, for an exam); deep by default.
        #[arg(long)]
        level: Option<String>,
        /// The model it ran on (the reviewer's, for an exam).
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        skill_version: Option<String>,
        #[arg(long)]
        seconds: Option<f64>,
        #[arg(long)]
        tokens: Option<i64>,
        /// prepare: a file it wrote; repeat it for each.
        #[arg(long = "file")]
        files: Vec<String>,
        /// prepare: the probe, a JSON list of {"try","command","exit","tail"}.
        #[arg(long)]
        probe: Option<String>,
        /// prepare: the levels tried, a JSON list of {"environment","level","launch","model"}.
        #[arg(long)]
        levels: Option<String>,
        /// harden: the change that let it through, or none.
        #[arg(long)]
        change: Option<String>,
        /// harden: where it got through, or what no longer adds anything.
        #[arg(long)]
        crack: Option<String>,
        /// harden: the file and Rule of its scenario, or none.
        #[arg(long)]
        scenario: Option<String>,
        /// harden: the blueprint change it proposes.
        #[arg(long)]
        proposal: Option<String>,
        /// exam: how many questions it was asked.
        #[arg(long)]
        asked: Option<i64>,
        /// exam: a question it missed, by its id; repeat it for each.
        #[arg(long)]
        missed: Vec<String>,
    },
    /// The backlog: list, add, done, briefed.
    Backlog {
        #[command(subcommand)]
        action: BacklogCmd,
    },
    /// Every open question to the person, oldest first.
    Waiting,
    /// Tables of metrics from the logs, the journals, the blueprint and git.
    Metrics {
        ids: Vec<String>,
        #[arg(long)]
        all: bool,
        #[arg(long, default_value = "json")]
        format: String,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        summary: bool,
    },
}

#[derive(Subcommand)]
enum BacklogCmd {
    List,
    Add {
        id: String,
        #[arg(long)]
        what: String,
        #[arg(long)]
        color: String,
        #[arg(long, default_value = "")]
        phase: String,
    },
    Done {
        id: String,
    },
    Briefed {
        id: String,
        #[arg(long)]
        brief: String,
    },
}

fn pairs(v: &[String]) -> Vec<(String, String)> {
    v.iter()
        .filter_map(|x| {
            x.split_once('=')
                .map(|(a, b)| (a.to_string(), b.to_string()))
        })
        .collect()
}

fn main() {
    let started = std::time::Instant::now();
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let matches = Cli::command().get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    if cli.no_journal {
        // SAFETY: set before any thread starts; the journal reads it once at the end.
        unsafe { std::env::set_var("GDD_NO_JOURNAL", "1") };
    }
    let ctx = Ctx {
        root: cli.root.clone(),
        env: cli.env.clone(),
        branch: cli.branch.clone(),
        head: cli.head.clone(),
    };
    // A repository git refuses to read is said at once, whatever the command: only a verify that
    // asked for no git goes on without it.
    let without_git = matches!(&cli.command, Cmd::Verify { no_git: true, .. });
    if !without_git && let Some(fail) = c::git_reads(&ctx) {
        let out = gdd::out::Out::new(matches.subcommand_name().unwrap_or("gdd")).with_fail(fail);
        print!("{}", out.render());
        std::process::exit(out.code);
    }
    let (name, id, out) = match &cli.command {
        Cmd::Next { id } => ("next", id.clone(), c::next(&ctx, id.as_deref())),
        Cmd::Log { id, event } => ("log", Some(id.clone()), c::log_event(&ctx, id, event)),
        Cmd::Ask { id, of, what } => ("ask", Some(id.clone()), c::ask(&ctx, id, of, what)),
        Cmd::Answer { id, words } => ("answer", Some(id.clone()), c::answer(&ctx, id, words)),
        Cmd::Approve {
            id,
            of,
            version,
            who,
            place,
            files,
        } => (
            "approve",
            Some(id.clone()),
            c::approve(
                &ctx,
                id,
                &c::Approval {
                    of,
                    version: *version,
                    who,
                    place,
                    files,
                },
            ),
        ),
        Cmd::Waiver {
            id,
            who,
            check,
            why,
            backlog,
            limit,
            to,
        } => (
            "waiver",
            Some(id.clone()),
            c::waiver(
                &ctx,
                id,
                &c::Waiver {
                    who,
                    check,
                    why,
                    backlog,
                    limit: limit.as_deref(),
                    to: *to,
                },
            ),
        ),
        Cmd::Correct {
            id,
            line,
            void,
            what,
        } => (
            "correct",
            Some(id.clone()),
            c::correct(&ctx, id, *line, what, *void),
        ),
        Cmd::Brief {
            id,
            paths,
            stop,
            color,
            why,
            level,
            skill_version,
            request,
            request_file,
            files,
        } => (
            "brief",
            Some(id.clone()),
            c::brief(
                &ctx,
                id,
                &c::BriefArgs {
                    paths,
                    color: color.as_deref(),
                    why,
                    files,
                    level: level.as_deref(),
                    skill_version: skill_version.as_deref(),
                    request: request.as_deref(),
                    request_file: request_file.as_deref(),
                    stop: stop.as_deref(),
                },
            ),
        ),
        Cmd::Report {
            id,
            step,
            file,
            model,
            level,
            seconds,
            tokens,
            proofs,
            claims,
            closed,
            went,
        } => {
            let text = if file == "-" {
                let mut s = String::new();
                let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut s);
                Ok(s)
            } else {
                std::fs::read_to_string(file)
            };
            let out = match text {
                Ok(t) => c::report(
                    &ctx,
                    id,
                    step,
                    &t,
                    &c::ReportArgs {
                        model: model.clone(),
                        level: level.clone(),
                        seconds: *seconds,
                        tokens: *tokens,
                        proofs: proofs.clone(),
                        claims: claims.clone(),
                        closed: pairs(closed),
                        went: pairs(went),
                    },
                ),
                Err(e) => gdd::Out::new("report").with_fail(gdd::Fail::missing(
                    file.clone(),
                    format!("{file}: the role's last message can't be read: {e}"),
                )),
            };
            ("report", Some(id.clone()), out)
        }
        Cmd::Limits { id } => ("limits", Some(id.clone()), c::limits_cmd(&ctx, id)),
        Cmd::Verify { ids, all, no_git } => (
            "verify",
            if ids.len() == 1 {
                Some(ids[0].clone())
            } else {
                None
            },
            c::verify_cmd(&ctx, ids, *all, *no_git),
        ),
        Cmd::Color { id, paths } => (
            "color",
            id.clone(),
            c::color_cmd(&ctx, id.as_deref(), paths),
        ),
        Cmd::Zones => ("zones", None, c::zones_cmd(&ctx)),
        Cmd::Seal {
            id,
            files,
            merge_main,
        } => (
            "seal",
            Some(id.clone()),
            c::seal_cmd(&ctx, id, files, *merge_main),
        ),
        Cmd::Gates { id, at } => (
            "gates",
            Some(id.clone()),
            c::gates_cmd(&ctx, id, at.as_deref()),
        ),
        Cmd::Launch {
            id,
            role,
            run,
            command,
            level,
            start_wait,
            timeout,
            adapter,
        } => (
            "launch",
            Some(id.clone()),
            c::launch_cmd(
                &ctx,
                id,
                role,
                &c::LaunchArgs {
                    run: *run,
                    command: command.clone(),
                    level: level.clone(),
                    start_wait: Duration::from_secs(*start_wait),
                    timeout: timeout.map(Duration::from_secs),
                    adapter: adapter.clone(),
                },
            ),
        ),
        Cmd::Ship {
            id,
            describe,
            done,
            bookkeeping,
            merge,
        } => {
            let out = match (describe, done, bookkeeping, merge) {
                (true, None, false, false) => c::ship_describe(&ctx, id),
                (false, Some(w), false, false) => c::ship_done(&ctx, id, w),
                (false, None, true, false) => c::ship_bookkeeping(&ctx, id),
                (false, None, false, true) => c::ship_merge(&ctx, id),
                _ => {
                    let mut o = gdd::Out::new("ship");
                    o.say("one of: gdd ship <id> --describe, --bookkeeping, --merge, or --done <where>");
                    o.code = 2;
                    o.fact("reason", "usage");
                    o
                }
            };
            ("ship", Some(id.clone()), out)
        }
        Cmd::Step {
            id,
            step,
            result,
            caught,
            level,
            model,
            skill_version,
            seconds,
            tokens,
            files,
            probe,
            levels,
            change,
            crack,
            scenario,
            proposal,
            asked,
            missed,
        } => (
            "step",
            Some(id.clone()),
            c::step_cmd(
                &ctx,
                id,
                step,
                &c::StepArgs {
                    result: result.clone(),
                    caught: caught.clone(),
                    level: level.clone(),
                    model: model.clone(),
                    skill_version: skill_version.clone(),
                    seconds: *seconds,
                    tokens: *tokens,
                    files: files.clone(),
                    probe: probe.clone(),
                    levels: levels.clone(),
                    change: change.clone(),
                    crack: crack.clone(),
                    scenario: scenario.clone(),
                    proposal: proposal.clone(),
                    asked: *asked,
                    missed: missed.clone(),
                },
            ),
        ),
        Cmd::Backlog { action } => match action {
            BacklogCmd::List => ("backlog", None, c::backlog_list(&ctx)),
            BacklogCmd::Add {
                id,
                what,
                color,
                phase,
            } => (
                "backlog",
                None,
                c::backlog_add(&ctx, id, what, color, phase),
            ),
            BacklogCmd::Done { id } => ("backlog", None, c::backlog_set(&ctx, id, "done", None)),
            BacklogCmd::Briefed { id, brief } => (
                "backlog",
                None,
                c::backlog_set(&ctx, id, "briefed", Some(brief)),
            ),
        },
        Cmd::Waiting => ("waiting", None, c::waiting(&ctx)),
        Cmd::Metrics {
            ids,
            all,
            format,
            out,
            summary,
        } => {
            let ids = if *all { vec![] } else { ids.clone() };
            (
                "metrics",
                None,
                c::metrics_cmd(
                    &ctx,
                    &c::MetricsArgs {
                        ids: &ids,
                        format,
                        out: out.as_deref(),
                        summary: *summary,
                    },
                ),
            )
        }
    };
    let args = gdd::journal::sanitize(
        &raw,
        &[
            "--what",
            "--who",
            "--why",
            "--where",
            "--command",
            "--done",
            "--request",
            "--stop",
            "--caught",
            "--crack",
            "--scenario",
            "--proposal",
            "--probe",
            "--levels",
        ],
    );
    gdd::journal::record(
        &ctx.root,
        id.as_deref(),
        name,
        &args,
        started.elapsed().as_millis(),
        &out,
    );
    // What waits for the journal, such as the commit of a ship's bookkeeping, then the output.
    let mut out = out;
    gdd::commands::finish(&ctx, &mut out);
    print!("{}", out.render());
    std::process::exit(out.code);
}
