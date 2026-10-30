//! The recipe of a change: what `gdd launch` and `gdd resume` run, in the order of the workflow
//! of a change ported from the bootstrap kit (the pilot repository keeps it).
//!
//! 1. Prepare. Test mode, when the change needs tests first and its code does not pass G2
//!    already: the Builder in test mode, Path Guard for it (a failure parks) and the build
//!    snapshot.
//! 2. Unless its code passes G2 already: rounds of the Builder in build mode and G2. Still red,
//!    the replans: the Orchestrator, test mode again when it says so, and one more round. Still
//!    red, the change parks.
//! 3. G3. Red: one fix by the Builder, G2, and G3 again, which decides. Still red, it parks.
//! 4. When the change has a Reviewer: its review, which the record of the change keeps with the
//!    files that changed since the review before, relaunches included. KO: one fix, G2, G3 and a
//!    second review. KO, or G3 red, it parks.
//! 5. G3 opened by the machines, merge, pre. Merges go one at a time; when main moved since the
//!    change was built, the change takes the latest main and passes G3 again on top of it before
//!    it merges: a conflict, or G3 red, parks it.
//! 6. A change that needs the human G4 waits for `gdd resume`. Any other is promoted and
//!    observed: closed.
//!
//! A spike has a recipe of its own (see [`Run::spiked`]): rounds of build mode in `mode=spike`
//! against its own G2 (see [`crate::gates::spike`]), within a budget checked after each of its
//! agent calls (see [`Run::within_budget`]); green, it pauses at G4 without G3, Reviewer, merge
//! or pre, and OK brings only its ADR to main (see [`spike_ok`]).
//!
//! The recipe decides which steps run, in what order, and when the change stops; the steps that
//! read the configuration of the project are [`Steps`]. Its agents and gates run in the
//! worktree of the change (see [`worktree`]), made from the main checkout, which stays on the
//! main branch and takes the merge: the worktree is kept while the change is parked or waits at
//! G4, and goes when the change closes or is rejected. Several changes run at once, each in its
//! worktree, as many as the cap allows; one that plans a path that overlaps one of a change in
//! flight runs with a warning, and git settles the overlap when it merges (see [`queue`]).
//!
//! Around every agent call, its own checks: no agent changes the GDD state of its change or the
//! shared one, links included (see [`State`]), or leaves a state folder in its worktree (see
//! [`state_left`]), and the Reviewer only reads: the working tree outside `.gdd/` is the same
//! after its call, down to the kind of each path, its permissions and where its links point (see
//! [`Tree`]). Either parks the change.

pub mod queue;
pub mod state;
pub mod worktree;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, ErrorKind};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::str::FromStr;

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::agents::files::{self, Found};
use crate::agents::invoke::{self, Call, Who};
use crate::agents::models::{Model, Session};
use crate::change::{self, Note};
use crate::classify::SPIKE;
use crate::gates::spike::ADR_FOLDER;
use crate::gates::{Gate, approve, release, verdict};
use crate::git::{self, Git};
use crate::guard;
use crate::repo::merge;
use crate::{inbox, log};

/// The arguments of a round of build mode.
const BUILD: &str = "mode=build";

/// The arguments of a round of build mode on a spike.
const SPIKE_BUILD: &str = "mode=spike";

/// The state of a change in pre, where it waits for the human G4.
const IN_PRE: &str = "in-pre";

/// What a spike may have spent below its budget and still count as having reached it: what is
/// left of adding up costs that binary numbers cannot hold exactly (0.7 + 0.1 is not 0.8).
const SPENT_ERROR: f64 = 1e-9;

/// The answers to G4, by name.
pub const ANSWERS: [&str; 3] = ["ok", "ko", "polish"];

/// Why a launch does not leave a working tree with uncommitted changes.
const UNCOMMITTED: &str =
    "The working tree has uncommitted changes. Commit them before launching a change.";

/// Why a change parks when G3 is still red after its fix.
const G3_RED: &str = "G3 still red after one fix";

/// Why a change parks when the Reviewer still says KO, or G3 is red, after its fix.
const REVIEW_KO: &str = "Reviewer KO or G3 red after one fix";

/// Why a change parks when G3 is red once it took the latest main.
const G3_RED_ON_MAIN: &str = "G3 red on top of the latest main";

/// Why a change parks when the latest main does not merge into it: the paths in conflict follow,
/// then the way out (see [`conflicts`]).
const CONFLICTS: &str = "main moved under it and the merge has conflicts in";

/// The note on G3 closed when main moved under the change.
const MAIN_MOVED: &str = "main moved under it: G3 again on top of the latest main";

/// The note on G3 opened again once it passed on top of the latest main.
const ON_LATEST_MAIN: &str = "automatic chain on top of the latest main";

/// What the note on a promotion that failed after the human's OK starts with.
const PROMOTION_FAILED: &str = "Promotion failed: ";

/// The state of a change in production (see [`release::promote`]).
const IN_PRODUCTION: &str = "in-production";

/// Local state of the tool, which the Reviewer's read-only check leaves out.
const LOCAL_STATE: &str = ".gdd/";

/// The folder of the local state of the tool, which only the state root holds.
const STATE_FOLDER: &str = ".gdd";

/// The fields of the record of a change the gdd commands an agent may run write: its notes and
/// the paths the scope check accepted.
const AGENT_FIELDS: [&str; 2] = ["inbox", "out_of_scope"];

/// The files of GDD state gdd keeps for every change: the approvals of G0 and of the red zones,
/// the self-test, the log and the sessions of the calls.
const SHARED: [&str; 5] = [
    approve::G0_PATH,
    approve::ZONES_PATH,
    approve::SELFTEST_PATH,
    log::PATH,
    invoke::SESSIONS,
];

/// The folders of GDD state an agent call is checked on at any depth, whatever the change: the
/// harness binary and the agents' programs, and the files the self-test sets aside.
const HARNESS: [&str; 2] = [".gdd/bin", ".gdd/selftest"];

/// What prepare decided for a change, as the recipe reads it: from what `gdd prepare` printed,
/// or from the record of a prepared change. Every field must be there, but the Builder session: a
/// missing one would skip a step.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Decided {
    /// Test mode comes before build mode.
    pub tests_first: bool,
    /// The code of a relaunch passes G2 already: no Builder.
    pub resume_green: bool,
    /// The change has a Reviewer.
    pub reviewer: bool,
    /// The change waits for the human at G4.
    pub g4: bool,
    /// Rounds of build mode against G2.
    pub retries: u32,
    /// The model of the Builder.
    pub builder: Model,
    /// The model of the Builder in test mode.
    pub builder_test: Model,
    /// The model of the Reviewer.
    pub reviewer_model: Model,
    /// The model of the Orchestrator.
    pub orchestrator: Model,
    /// The kind of work of the change (`Feature`, `Spike`...): a spike has a recipe of its own.
    #[serde(rename = "type")]
    pub kind: String,
    /// The Builder session of the change, which each Builder call gets (see [`invoke::run`]). A
    /// change prepared before gdd decided it has none: separate, as it ran then.
    #[serde(default)]
    pub builder_session: Session,
}

impl Decided {
    /// Whether the change is a spike.
    fn spike(&self) -> bool {
        self.kind == SPIKE
    }

    /// The model of `who`.
    fn model(&self, who: Who) -> &Model {
        match who {
            Who::BuilderTest => &self.builder_test,
            Who::BuilderBuild => &self.builder,
            Who::Reviewer => &self.reviewer_model,
            Who::Orchestrator => &self.orchestrator,
        }
    }
}

/// A snapshot of what the change touched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Snapshot {
    /// The next test round is judged from it.
    Round,
    /// Build mode is judged from it: taken after test mode.
    Build,
}

/// The steps of the recipe that read the configuration of the project, on the change it runs.
/// An error stops the recipe.
pub trait Steps {
    /// Checks that the change may enter Build and starts it (`gdd prepare`): what it decided.
    fn prepare(&mut self) -> Result<Decided, String>;
    /// Calls an agent and waits for it (see [`invoke::run`]).
    fn agent(&mut self, call: &Call) -> Result<(), invoke::Error>;
    /// Path Guard for test mode on what the change touched since its build started or, with
    /// `since_snapshot`, since the round snapshot: its failures.
    fn guard_test(&mut self, since_snapshot: bool) -> Result<Vec<String>, String>;
    /// Records what the change touched, as it is now, in `snapshot`.
    fn snapshot(&mut self, snapshot: Snapshot) -> Result<(), String>;
    /// Before a relaunch prepares the change again, Path Guard for build mode on what it touched
    /// since its build started, judged from the build snapshot, and the scope check: their
    /// failures, a check that cannot judge failing with the reason. `None` without a build
    /// snapshot: test mode never passed its Path Guard, or did not run.
    fn guard_earlier(&mut self) -> Result<Option<Vec<String>>, String>;
    /// The hashes of what the change touched, as they are now: what a snapshot records (see
    /// [`guard::snapshot`]).
    fn hashes(&mut self) -> Result<guard::Hashes, String>;
    /// Runs `gate` for build mode, saved and logged: whether it passed.
    fn gate(&mut self, gate: Gate) -> Result<bool, String>;
    /// Records the decision `decision` (`ok` or `ko`) on `gate` of the change, by `by`, with
    /// `note` (`gdd gate`).
    fn decide(&mut self, gate: &str, decision: &str, by: &str, note: &str) -> Result<(), String>;
    /// Merges the change into the main branch and deploys it to pre: what it printed (see
    /// [`merge::merge`]).
    fn merge(&mut self) -> Result<String, String>;
    /// The release step `step`: `pre`, `promote` or `observe` (`gdd release`).
    fn release(&mut self, step: &str) -> Result<(), String>;
    /// Tells the person who runs the recipe `line` on the way, before what the run says at its
    /// end.
    fn say(&mut self, line: &str);
}

/// What the recipe reads from the project itself.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Project<'a> {
    /// The project root, where gdd runs: the recipe only runs from the main checkout.
    pub root: &'a Path,
    /// The state root, whose `.gdd/` holds the record of the change (see
    /// [`crate::state::root`]): the project root itself, but in a linked worktree.
    pub state: &'a Path,
    /// Where the worktree of the change goes (see [`worktree::path`]).
    pub worktree: &'a Path,
    /// The project root in that worktree (see [`worktree::project_root`]): the project root of
    /// its agents and its steps.
    pub work: &'a Path,
    /// The branch changes merge into.
    pub main: &'a str,
    /// Replans of a change still red in G2 before it parks.
    pub replans: u32,
    /// Changes that build at once at most (see [`queue::cap`]).
    pub max: usize,
    /// What the agent calls of a spike may cost, in USD, before it parks (see
    /// [`crate::rules::spike_budget`]).
    pub spike_usd: f64,
}

/// What a run of the recipe tells the person who ran it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Said {
    /// It went as far as it could: what to read, for the standard output.
    Done(String),
    /// It parked the change: why, for the standard error.
    Parked(String),
}

/// The human's answer to G4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Promote the change to production.
    Ok,
    /// Reject it: production stays as it is.
    Ko,
    /// One more Builder round with the human's notes, then G4 again.
    Polish,
}

impl FromStr for Answer {
    type Err = String;

    fn from_str(word: &str) -> Result<Answer, String> {
        match word {
            "ok" => Ok(Answer::Ok),
            "ko" => Ok(Answer::Ko),
            "polish" => Ok(Answer::Polish),
            other => Err(format!("G4 is ok, ko or polish, not {other:?}")),
        }
    }
}

/// Why the recipe stopped before its end.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Stop {
    /// The change parks: why, for the human.
    Park(String),
    /// A step failed: what went wrong.
    Fail(String),
}

impl From<String> for Stop {
    fn from(e: String) -> Stop {
        Stop::Fail(e)
    }
}

impl From<invoke::Error> for Stop {
    fn from(e: invoke::Error) -> Stop {
        if e.parks() {
            Stop::Park(e.to_string())
        } else {
            Stop::Fail(e.to_string())
        }
    }
}

/// Where the recipe ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ended {
    /// In pre, waiting for the human G4.
    Paused,
    /// Promoted and observed.
    Closed,
}

/// `gdd launch`: leaves the main checkout on the main branch (see [`to_main`]), makes the
/// worktree of change `id` or reuses it (see [`worktree::open`]), with the work of an earlier run
/// there committed once the gates accept it (see [`earlier_work`]), prepares the change and runs
/// the recipe there. Only from the main checkout (see [`from_main_checkout`]), and only while the
/// cap allows it (see [`queue::may_launch`]): a refusal touches nothing. From that judgement until
/// prepare makes the change building, it holds the launch lock (see [`queue::admission`]): two
/// launches at once never both pass it. Once prepared, it warns of the changes in flight the
/// change overlaps (see [`warned`]).
pub fn launch(project: &Project, id: &str, steps: &mut impl Steps) -> Result<Said, String> {
    from_main_checkout(project, "launch")?;
    let record = change::read_record(project.state, id)?;
    let admitted = queue::admission(project.state)?;
    let warnings = queue::may_launch(project.state, id, &record, project.max)?;
    to_main(project, id)?;
    worktree::open(project.root, project.worktree, id)?;
    earlier_work(project.work, id, steps)?;
    state::clear(project.state, id)?;
    let decided = steps.prepare()?;
    drop(admitted);
    warned(project.state, id, &warnings, steps)?;
    let mut run = Run {
        project,
        id,
        steps,
        decided,
    };
    let ended = run.launched();
    finish(project, id, ended)
}

/// Says each of `warnings`, one for each change in flight change `id` overlaps (see
/// [`queue::may_launch`]), and keeps it in the record of the change, under the state root
/// `state`, as a decision taken without the human, which G4 shows: once, however often the change
/// launches.
fn warned(
    state: &Path,
    id: &str,
    warnings: &[String],
    steps: &mut impl Steps,
) -> Result<(), String> {
    for warning in warnings {
        steps.say(warning);
    }
    let text = change::read_record(state, id)?;
    let path = change::record_path(id);
    let invalid = |e: serde_json::Error| format!("{path}: {e}");
    let record: Value = serde_json::from_str(&text).map_err(invalid)?;
    let notes = record["inbox"].as_array();
    let kept = |warning: &str| {
        let same = |note: &Value| note["kind"] == "decided" && note["text"] == warning;
        notes.is_some_and(|notes| notes.iter().any(same))
    };
    let ts = log::now();
    let mut noted = text;
    for warning in warnings.iter().filter(|warning| !kept(warning)) {
        let note = Note {
            kind: "decided",
            text: warning,
            ts: &ts,
        };
        noted = change::with_note(&noted, &note).map_err(invalid)?;
    }
    change::write(state, &path, &noted)
}

/// `Ok` when `gdd <command>` runs in a project whose state is its own; else, before anything
/// is read or done, why not: a linked worktree shares the state of the main checkout, and the
/// recipe makes and removes the worktrees of the changes from there.
fn from_main_checkout(project: &Project, command: &str) -> Result<(), String> {
    if project.root == project.state {
        return Ok(());
    }
    Err(format!(
        "run gdd {command} from the main checkout ({}): the worktrees of the changes are made \
         and removed from there",
        project.state.display()
    ))
}

/// Leaves the main checkout of `project` on the main branch before change `id` runs in its
/// worktree. On the branch of the change, its work there is the work of an earlier run,
/// committed first (see [`earlier_run`]): a person's, as the agents work in the worktree; on any
/// other branch, uncommitted changes in tracked files are refused.
fn to_main(project: &Project, id: &str) -> Result<(), String> {
    let git = Git::new(project.root);
    let said = |e: git::Error| e.to_string();
    if git.current_branch().map_err(said)? == change::branch(id) {
        earlier_run(project.root, id).map_err(said)?;
    } else if git.has_tracked_changes().map_err(said)? {
        return Err(UNCOMMITTED.to_string());
    }
    git.checkout(project.main).map_err(said)
}

/// Commits the work of an earlier run of change `id` left in the project at `root`, on the
/// branch of the change, when there is any, without the state (see [`Git::commit_work`]): a
/// relaunch judges it as it judges committed work, and its build starts after it, so what test
/// mode wrote before is not build mode's doing.
fn earlier_run(root: &Path, id: &str) -> Result<(), git::Error> {
    let message = format!("gdd({id}): work of the earlier run, before relaunch");
    work_committed(root, &message)
}

/// Commits the work left in the project at `root`, on the branch checked out, with `message`,
/// when there is any, without the state (see [`Git::commit_work`]).
fn work_committed(root: &Path, message: &str) -> Result<(), git::Error> {
    let git = Git::new(root);
    if git.has_work()? {
        git.commit_work(message)?;
    }
    Ok(())
}

/// The work the agents of an earlier run of change `id` left in its worktree, at `work`,
/// committed (see [`earlier_run`]) only once the gates of that run accept it, judged as they
/// judged it, from its build snapshot (see [`Steps::guard_earlier`]). Otherwise it stays as it
/// is, and the gates of the relaunch judge it from where its build starts: committed, it would
/// fall behind that start, and a protected path no gate passed would leave their sight.
fn earlier_work(work: &Path, id: &str, steps: &mut impl Steps) -> Result<(), String> {
    let said = |e: git::Error| e.to_string();
    if !Git::new(work).has_work().map_err(said)? {
        return Ok(());
    }
    match steps.guard_earlier()? {
        Some(failures) if failures.is_empty() => earlier_run(work, id).map_err(said),
        Some(_) | None => Ok(()),
    }
}

/// A change at its merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Merging<'a> {
    /// The state root, whose main checkout takes the merge.
    pub state: &'a Path,
    /// Where the branch of the change is checked out, with its last work.
    pub work: &'a Path,
    /// The branch changes merge into.
    pub main: &'a str,
    /// The id of the change.
    pub id: &'a str,
}

/// `gdd merge`: the merge of the change `at`, as the recipe merges (see [`Run::merged`]): one at
/// a time, under the merge lock, which the merge of another change makes it wait for, saying so
/// (see [`queue::lock`]); then, once it may merge (see [`merge::may_merge`]), on top of the
/// latest main (see [`on_latest_main`]), which parks it on a conflict or G3 red. What the merge
/// printed.
pub fn merge(at: &Merging, steps: &mut impl Steps) -> Result<Said, String> {
    let _lock = queue::lock(at.state, || steps.say(queue::WAITING))?;
    let project = merge::Project {
        root: at.work,
        state: at.state,
        main: at.main,
    };
    merge::may_merge(&project, at.id)?;
    match on_latest_main(at, steps) {
        Ok(()) => steps.merge().map(Said::Done),
        Err(Stop::Park(reason)) => change::park(at.state, at.id, &reason).map(Said::Parked),
        Err(Stop::Fail(e)) => Err(e),
    }
}

/// When the main branch moved since the change `at` was built (see [`queue::main_moved`]): G3
/// closes, as what passed it is no longer what would merge, and the change takes the latest main
/// (see [`queue::take_main`]), where a conflict parks it (see [`conflicts`]). Its build starts
/// again there (see [`queue::restarted`]), and it passes G3 again on top of it: green opens G3
/// again, red parks it, G3 still closed.
fn on_latest_main(at: &Merging, steps: &mut impl Steps) -> Result<(), Stop> {
    let (work, main, id) = (at.work, at.main, at.id);
    let said = |e: git::Error| Stop::Fail(e.to_string());
    if !queue::main_moved(work, main).map_err(said)? {
        return Ok(());
    }
    steps.decide("G3", "ko", "machines", MAIN_MOVED)?;
    let paths = queue::take_main(work, main, id).map_err(said)?;
    if !paths.is_empty() {
        return Err(Stop::Park(conflicts(at, &paths)));
    }
    let start = Git::new(work).head().map_err(said)?;
    queue::restarted(at.state, id, &start)?;
    steps.say(&format!("main moved under {id}: G3 again on top of it"));
    if !steps.gate(Gate::G3)? {
        return Err(Stop::Park(G3_RED_ON_MAIN.to_string()));
    }
    Ok(steps.decide("G3", "ok", "machines", ON_LATEST_MAIN)?)
}

/// Why change `at` parks when the latest main does not merge into it, with `paths` in conflict:
/// them, then the way out, by hand in its worktree. The launch after it takes the change on top
/// of the latest main, as its branch then holds it.
fn conflicts(at: &Merging, paths: &[String]) -> String {
    let (main, id, work) = (at.main, at.id, at.work.display());
    let (paths, branch) = (paths.join(", "), change::branch(id));
    format!(
        "{CONFLICTS} {paths}: merge {main} into {branch} in its worktree ({work}), settle the \
         conflicts, commit, then gdd launch {id}"
    )
}

/// `gdd resume`: the answer of the role `role` to the G4 of change `id`, which must be paused at
/// it, with `note` (which `polish` needs). Builders and Reviewers cannot answer. The answer
/// resolves the request of G4 and the notes on promotions that failed. `ok` promotes the change
/// (see [`ok_at_g4`]), or brings only the ADR of a spike to main (see [`spike_ok`]); `ko` rejects
/// it, production untouched; either way its worktree goes once it ends. `polish` runs a polish
/// pass in its worktree from the commit it is at, made again from the branch of the change when
/// it went away (see [`polish_pass`]). A change that an earlier
/// `ok` promoted takes only `ok`, which observes it; one whose promotion did not end parks,
/// whatever the answer (see [`unsure`]). Only from the main checkout (see
/// [`from_main_checkout`]).
pub fn resume(
    project: &Project,
    id: &str,
    answer: Answer,
    note: &str,
    role: Option<&str>,
    steps: &mut impl Steps,
) -> Result<Said, String> {
    from_main_checkout(project, "resume")?;
    let state_root = project.state;
    approve::may_record(role).map_err(|e| e.to_string())?;
    let (text, record) = release::read(state_root, id)?;
    if answer == Answer::Polish && note.trim().is_empty() {
        return Err(format!(
            "polish needs your notes: gdd resume {id} --g4 polish --note ..."
        ));
    }
    if !state::paused_at_g4(state_root, id)? {
        return Err(format!("Change {id} is not paused at G4."));
    }
    if record["state"] == release::PROMOTING {
        answered(state_root, id, text, &record)?;
        return unsure(
            state_root,
            id,
            "an earlier answer stopped while promoting it",
        );
    }
    let in_production = record["state"] == IN_PRODUCTION;
    if in_production && answer != Answer::Ok {
        return Err(format!(
            "Change {id} was promoted already: production has it, so it can no longer be \
             rejected or polished. To observe it: gdd resume {id} --g4 ok."
        ));
    }
    match answer {
        Answer::Ok if record["type"] == SPIKE => spike_ok(project, id, note, text, &record, steps),
        Answer::Ok => {
            answered(state_root, id, text, &record)?;
            ok_at_g4(project, id, note, in_production, steps)
        }
        Answer::Ko => {
            answered(state_root, id, text, &record)?;
            state::clear(state_root, id)?;
            steps.decide("G4", "ko", "human", note)?;
            saved_in_state(state_root, id, "rejected")?;
            let said = format!("Change {id} rejected at G4: production untouched.");
            without_worktree(project, said)
        }
        Answer::Polish => {
            // First, so a worktree that cannot be made leaves the change paused.
            let start = to_polish(project, id)?;
            answered(state_root, id, text, &record)?;
            state::clear(state_root, id)?;
            polish_pass(project, id, note, &start, steps)
        }
    }
}

/// Writes `text`, the record of change `id` under the state root `root`, whose JSON is
/// `record`, with the requests an answer to its G4 resolves resolved (see [`g4_requests`]).
fn answered(root: &Path, id: &str, text: String, record: &Value) -> Result<(), String> {
    let path = change::record_path(id);
    let ts = log::now();
    let answered = g4_requests(id, record)
        .iter()
        .try_fold(text, |text, request| {
            change::with_request_resolved(&text, request, &ts)
        })
        .map_err(|e| format!("{path}: {e}"))?;
    change::write(root, &path, &answered)
}

/// The worktree of change `id` of `project`, ready for a polish pass (see [`worktree::open`]):
/// the commit it is at, where the pass starts.
fn to_polish(project: &Project, id: &str) -> Result<String, String> {
    worktree::open(project.root, project.worktree, id)?;
    Git::new(project.work).head().map_err(|e| e.to_string())
}

/// The requests to the human that an answer to the G4 of change `id`, whose record is `record`,
/// resolves: the request of G4, and the notes on promotions that failed.
fn g4_requests(id: &str, record: &Value) -> Vec<String> {
    let notes = record["inbox"].as_array().into_iter().flatten();
    let failed = notes
        .filter_map(|note| note["text"].as_str())
        .filter(|text| text.starts_with(PROMOTION_FAILED));
    let kind = record["type"].as_str().unwrap_or_default();
    let mut requests = vec![g4_request(id, kind)];
    requests.extend(failed.map(str::to_string));
    requests
}

/// OK at G4 of change `id` of `project`, by the human with `note`: the decision recorded, the
/// change promoted unless it is `in_production` already (an earlier OK promoted it and stopped
/// before observing it: never twice), then observed. Until the promotion passed, it stays paused
/// at G4 unless production may have it (see [`not_promoted`]); a failed observation rolls it
/// back, as it does without G4.
fn ok_at_g4(
    project: &Project,
    id: &str,
    note: &str,
    in_production: bool,
    steps: &mut impl Steps,
) -> Result<Said, String> {
    let root = project.state;
    let mut promotion = steps.decide("G4", "ok", "human", note);
    if promotion.is_ok() && !in_production {
        promotion = steps.release("promote");
    }
    if let Err(failure) = promotion {
        return not_promoted(root, id, &failure);
    }
    state::clear(root, id)?;
    finish(project, id, observed(steps))
}

/// OK at G4 of spike `id` of `project`, by the human with `note`; its record is `record`, whose
/// text is `text`. Under the merge lock, which the merge of another change makes it wait for,
/// saying so (see [`queue::lock`]), its ADR reaches main from the main checkout, alone (see
/// [`merge::adr_only`]); then the answer resolves the requests of G4, the decision is recorded
/// and the change is closed: its worktree goes, its branch stays. No promotion, no observation.
/// When its ADR cannot reach main, nothing changed: the change stays paused at G4.
fn spike_ok(
    project: &Project,
    id: &str,
    note: &str,
    text: String,
    record: &Value,
    steps: &mut impl Steps,
) -> Result<Said, String> {
    let state_root = project.state;
    let title = record["title"].as_str().unwrap_or_default();
    let lock = queue::lock(state_root, || steps.say(queue::WAITING))?;
    merge::adr_only(project.root, project.main, id, title)?;
    drop(lock);
    answered(state_root, id, text, record)?;
    steps.decide("G4", "ok", "human", note)?;
    state::clear(state_root, id)?;
    saved_in_state(state_root, id, "closed")?;
    finish(project, id, Ok(Ended::Closed))
}

/// What the human reads when the OK at G4 of change `id`, under the state root `root`, failed
/// with `failure` before the change was promoted and observed. Its record says what production
/// has: nothing of the change, which stays paused at G4 (see [`promotion_failed`]); or maybe the
/// change, promoted now or before, which parks it (see [`unsure`]). A record that cannot be read
/// keeps the pause.
fn not_promoted(root: &Path, id: &str, failure: &str) -> Result<Said, String> {
    let unread = |e: String| {
        format!(
            "Promotion of {id} failed: {failure}. Its record cannot say whether production has \
             it: {e}"
        )
    };
    let (_, record) = release::read(root, id).map_err(unread)?;
    let state = &record["state"];
    if state == release::PROMOTING || state == IN_PRODUCTION {
        return unsure(root, id, failure);
    }
    Err(promotion_failed(root, id, failure))
}

/// What the human reads when production may have change `id`, under the state root `root`, and
/// the recipe cannot go on with it (`why`): the change parks, no longer paused at G4, so that
/// no answer promotes it twice (see [`change::park`]). When it cannot park, it says why too.
fn unsure(root: &Path, id: &str, why: &str) -> Result<Said, String> {
    let reason = format!(
        "production may have it, but gdd cannot go on with it ({why}). Check production \
         before anything else: promoting it again could deploy it twice"
    );
    // The pause goes first: a parked change still paused could be promoted again.
    let parked = state::clear(root, id).and_then(|()| change::park(root, id, &reason));
    parked
        .map(Said::Parked)
        .map_err(|e| format!("Change {id} cannot park: {reason}.\n{e}"))
}

/// What the human reads when the promotion of change `id`, under the state root `root`, failed
/// with `failure`, or could not run: production is untouched, so the change stays paused at G4,
/// with a note on why and its request of G4 back in the Decision Inbox. When they cannot be left
/// there, it says why too.
fn promotion_failed(root: &Path, id: &str, failure: &str) -> String {
    let why = format!(
        "{PROMOTION_FAILED}{failure}. Fix the cause and answer again: gdd resume {id} --g4 ok, \
         or --g4 ko."
    );
    let said = format!(
        "Promotion of {id} failed: {failure}. Production untouched: the change is still paused \
         at G4. To retry: gdd resume {id} --g4 ok (or --g4 ko to reject)."
    );
    match pause_at_g4(root, id, Some(&why)) {
        Ok(()) => said,
        Err(e) => format!("{said}\n{e}"),
    }
}

/// A polish pass of change `id`, on its branch, from the commit `start` it is at: `note`, the
/// human's notes at G4, kept for the Builder; then the recipe again from build mode, the notes
/// in its first round, until it pauses at G4 again.
fn polish_pass(
    project: &Project,
    id: &str,
    note: &str,
    start: &str,
    steps: &mut impl Steps,
) -> Result<Said, String> {
    let state_root = project.state;
    polish_noted(state_root, id, note, start)?;
    let mut run = Run {
        project,
        id,
        steps,
        decided: decided_of(state_root, id)?,
    };
    let ended = run.polished();
    finish(project, id, ended)
}

/// Keeps `note`, the human's notes at G4, in the record of change `id` under the state root
/// `root` for the Builder (see [`change::POLISH`]); logs it as the answer to G4, which stays
/// pending; and puts the change back in Build, its build starting at the commit `start`: what the
/// change did before passed every gate and merged, so Path Guard judges what the pass touches.
fn polish_noted(root: &Path, id: &str, note: &str, start: &str) -> Result<(), String> {
    let text = change::read_record(root, id)?;
    let path = change::record_path(id);
    let ts = log::now();
    let polish = Note {
        kind: change::POLISH,
        text: note,
        ts: &ts,
    };
    let fields = vec![
        ("state", Value::from("building")),
        ("build_start", Value::from(start)),
    ];
    let noted = change::with_note(&text, &polish)
        .and_then(|text| change::with_fields(&text, fields))
        .map_err(|e| format!("{path}: {e}"))?;
    change::write(root, &path, &noted)?;
    let entry = log::Entry {
        change: id.to_string(),
        phase: "4-release".to_string(),
        gate: "G4".to_string(),
        role: "human".to_string(),
        agent: "-".to_string(),
        result: "polish".to_string(),
        duration_s: 0.0,
        note: note.to_string(),
    };
    change::logged(root, &entry)?;
    inbox::save(root, id, &noted).map(drop)
}

/// What prepare decided for change `id`, as its record under the state root `root` keeps it.
fn decided_of(root: &Path, id: &str) -> Result<Decided, String> {
    #[derive(Deserialize)]
    struct Raw {
        decision: Decided,
    }
    let text = change::read_record(root, id)?;
    let path = change::record_path(id);
    let raw: Raw = serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    Ok(raw.decision)
}

/// Writes the record of change `id`, under the state root `root`, in state `state`, and saves
/// its Decision Inbox.
fn saved_in_state(root: &Path, id: &str, state: &str) -> Result<(), String> {
    let text = change::read_record(root, id)?;
    let path = change::record_path(id);
    let fields = vec![("state", Value::from(state))];
    let saved = change::with_fields(&text, fields).map_err(|e| format!("{path}: {e}"))?;
    change::write(root, &path, &saved)?;
    inbox::save(root, id, &saved).map(drop)
}

/// The request to the human at G4 of change `id`, whose kind of work is `kind`, in its Decision
/// Inbox. A spike has no demo and is never promoted: its request is for its ADR files.
fn g4_request(id: &str, kind: &str) -> String {
    if kind == SPIKE {
        let branch = change::branch(id);
        return format!(
            "G4 · Release Gate: read the ADR files the spike left under {ADR_FOLDER} on its \
             branch {branch}, then answer with gdd resume {id} --g4 ok (bring only those files \
             to main, in a commit of their own: its code stays on its branch), --g4 ko (reject: \
             main untouched) or --g4 polish --note <what to polish> (one more Builder round)."
        );
    }
    format!(
        "G4 · Release Gate: check the pre demo and the change message, then answer with \
         gdd resume {id} --g4 ok (promote to production), --g4 ko (reject: production \
         untouched) or --g4 polish --note <what to polish> (one more Builder round)."
    )
}

/// What the person who ran the recipe of change `id` of `project` reads, from how it ended. A
/// closed change loses its worktree (see [`without_worktree`]); a pause at G4 leaves the request
/// in the Decision Inbox and the run state paused; a stop that parks parks the change (see
/// [`change::park`]).
fn finish(project: &Project, id: &str, ended: Result<Ended, Stop>) -> Result<Said, String> {
    let root = project.state;
    let inbox = format!("Message: {}", change::inbox_path(id));
    match ended {
        Ok(Ended::Closed) => {
            let tokens = format!("Tokens: gdd report --change {id}");
            let said = format!("Change {id} closed. {inbox} · {tokens}");
            without_worktree(project, said)
        }
        Ok(Ended::Paused) => {
            pause_at_g4(root, id, None)?;
            let next = format!("To continue: gdd resume {id} --g4 ok|ko|polish");
            Ok(Said::Done(format!("Paused at G4. {inbox} · {next}")))
        }
        Err(Stop::Park(reason)) => change::park(root, id, &reason).map(Said::Parked),
        Err(Stop::Fail(e)) => Err(e),
    }
}

/// What the human reads of a change of `project` that ended (`said`), once its worktree is
/// removed (see [`worktree::remove`]). When it cannot be, the change stays as it ended, and the
/// human reads why too.
fn without_worktree(project: &Project, said: String) -> Result<Said, String> {
    match worktree::remove(project.root, project.worktree) {
        Ok(()) => Ok(Said::Done(said)),
        Err(e) => Err(format!("{said}\nIts worktree stays: {e}")),
    }
}

/// Leaves the request of G4 in the Decision Inbox of change `id`, under the state root `root`,
/// after `why` when there is one, and its run state paused at G4.
fn pause_at_g4(root: &Path, id: &str, why: Option<&str>) -> Result<(), String> {
    let mut noted = change::read_record(root, id)?;
    let path = change::record_path(id);
    let ts = log::now();
    let kind = change::kind(&noted).map_err(|e| format!("{path}: {e}"))?;
    let request = g4_request(id, &kind);
    for text in why.into_iter().chain([request.as_str()]) {
        let note = Note {
            kind: "needs",
            text,
            ts: &ts,
        };
        noted = change::with_note(&noted, &note).map_err(|e| format!("{path}: {e}"))?;
    }
    change::write(root, &path, &noted)?;
    inbox::save(root, id, &noted)?;
    state::pause_at_g4(root, id)
}

/// Promotes the change and observes it in production.
fn promoted(steps: &mut impl Steps) -> Result<Ended, Stop> {
    steps.release("promote")?;
    observed(steps)
}

/// Observes the change in production.
fn observed(steps: &mut impl Steps) -> Result<Ended, Stop> {
    steps.release("observe")?;
    Ok(Ended::Closed)
}

/// Whether the branch of change `id` of the project at `root`, checked out, has nothing `main`
/// lacks: no work to commit, and no commit `main` does not have.
fn nothing_new(root: &Path, id: &str, main: &str) -> Result<bool, git::Error> {
    let git = Git::new(root);
    if git.has_changes()? {
        return Ok(false);
    }
    Ok(git.merge_base(main, &change::branch(id))? == git.head()?)
}

/// A run of the recipe on a change.
struct Run<'a, S> {
    project: &'a Project<'a>,
    id: &'a str,
    steps: &'a mut S,
    decided: Decided,
}

impl<S: Steps> Run<'_, S> {
    /// The recipe of a launch, once prepared; a spike's has no test mode and ends at
    /// [`Run::spiked`].
    fn launched(&mut self) -> Result<Ended, Stop> {
        let green = self.decided.resume_green;
        if self.decided.spike() {
            if !green {
                self.build(SPIKE_BUILD, SPIKE_BUILD)?;
            }
            return self.spiked();
        }
        if self.decided.tests_first && !green {
            self.test_mode("mode=test", false)?;
        }
        if !green {
            self.build(BUILD, BUILD)?;
        }
        self.integrate(false)
    }

    /// The recipe of a polish pass: from build mode, the human's notes first. A spike's is one
    /// round, with no retry or replan, then G2: green, it ends at [`Run::spiked`], never merging;
    /// still red, the change parks.
    fn polished(&mut self) -> Result<Ended, Stop> {
        if self.decided.spike() {
            self.call(Who::BuilderBuild, "mode=spike fix=polish")?;
            if !self.steps.gate(Gate::G2)? {
                return Err(self.still_red("the polish round"));
            }
            return self.spiked();
        }
        self.build("mode=build fix=polish", BUILD)?;
        self.integrate(true)
    }

    /// The end of a spike, green: its work committed on its branch, which keeps it whatever the
    /// human answers (see [`work_committed`]); the ADR files OK would bring to main said (see
    /// [`merge::adr_files`]); and the change in pre, waiting at G4. No G3, no Reviewer, no merge
    /// and no deploy to pre: its code never reaches main.
    fn spiked(&mut self) -> Result<Ended, Stop> {
        let Project { state, work, .. } = *self.project;
        let (main, id) = (self.project.main, self.id);
        let said = |e: git::Error| Stop::Fail(e.to_string());
        work_committed(work, &format!("gdd({id}): spike work")).map_err(said)?;
        let adrs = merge::adr_files(work, main, id).map_err(said)?;
        self.steps.say(&format!("ADR: {}", adrs.join(", ")));
        saved_in_state(state, id, IN_PRE)?;
        Ok(Ended::Paused)
    }

    /// Calls `who` with `args`, then checks what it did: GDD state it changed goes back as it was
    /// (see [`State`]), and a state folder it left in the worktree goes (see [`state_left`]);
    /// either parks the change, as a Reviewer that changed the working tree does. From the state
    /// as it was until it is checked, the call holds the lock of the agent calls (see
    /// [`queue::calling`]): what a person writes to the state every change shares lands before
    /// or after it, never during it. After a call of a spike that ended, its budget (see
    /// [`Run::within_budget`]): a call is never cut.
    fn call(&mut self, who: Who, args: &str) -> Result<(), Stop> {
        let Project { state, work, .. } = *self.project;
        let call = Call {
            who,
            model: self.decided.model(who),
            args,
            builder_session: self.decided.builder_session,
        };
        let calling = queue::calling(state)?;
        let before = State::of(state, self.id, who)?;
        let tree = match who {
            Who::Reviewer => Some(working_tree(work)?),
            Who::BuilderTest | Who::BuilderBuild | Who::Orchestrator => None,
        };
        let called = self.steps.agent(&call);
        let mut changed = before.put_back(state)?;
        changed.extend(state_left(work, state)?);
        drop(calling);
        if !changed.is_empty() {
            let files = changed.join(", ");
            return Err(Stop::Park(format!("the {who} changed GDD state: {files}")));
        }
        if let Some(before) = tree {
            let changed = differ(&before, &working_tree(work)?);
            if !changed.is_empty() {
                let paths = changed.join(", ");
                let why = format!("the Reviewer changed {paths}: Reviewers only read");
                return Err(Stop::Park(why));
            }
        }
        called?;
        if self.decided.spike() {
            self.within_budget()?;
        }
        Ok(())
    }

    /// What the agent calls of the change spent so far (see [`spent`]), against the budget of a
    /// spike: once it reaches it, the change parks, saying both to the cent.
    fn within_budget(&self) -> Result<(), Stop> {
        let max = self.project.spike_usd;
        let spent = spent(self.project.state, self.id)?;
        if spent + SPENT_ERROR < max {
            return Ok(());
        }
        let why = format!("the spike spent {spent:.2} USD of its budget of {max:.2} USD");
        Err(Stop::Park(why))
    }

    /// The Builder in test mode with `args`, then Path Guard for it, since the round snapshot
    /// with `since_snapshot`: a failure parks. Then the build snapshot.
    fn test_mode(&mut self, args: &str, since_snapshot: bool) -> Result<(), Stop> {
        self.call(Who::BuilderTest, args)?;
        let failures = self.steps.guard_test(since_snapshot)?;
        if !failures.is_empty() {
            let failures = failures.join("; ");
            let why = format!("Path Guard failed after test mode: {failures}");
            return Err(Stop::Park(why));
        }
        Ok(self.steps.snapshot(Snapshot::Build)?)
    }

    /// Build mode until G2 passes: the rounds in `mode`, the first with `first`, then the
    /// replans, test mode again when one says so, but on a spike. Still red, the change parks
    /// (see [`Run::still_red`]).
    fn build(&mut self, first: &str, mode: &str) -> Result<(), Stop> {
        if self.rounds(first, mode)? {
            return Ok(());
        }
        let replans = self.project.replans;
        let (state, id) = (self.project.state, self.id);
        for _ in 0..replans {
            self.call(Who::Orchestrator, "reason=g2")?;
            if !self.decided.spike() && verdict::replanned_to_test(state, id)? {
                self.steps.snapshot(Snapshot::Round)?;
                self.test_mode("mode=test after=replan", true)?;
            }
            self.call(Who::BuilderBuild, &format!("{mode} after=replan"))?;
            if self.steps.gate(Gate::G2)? {
                return Ok(());
            }
        }
        let retries = self.decided.retries;
        Err(self.still_red(&format!("{retries} retries and {replans} replan")))
    }

    /// Why the change parks with G2 still red after `after`; a spike says what its last G2 failed
    /// on too (see [`g2_failures`]), and stops when that cannot be read.
    fn still_red(&self, after: &str) -> Stop {
        let why = format!("G2 still red after {after}");
        if !self.decided.spike() {
            return Stop::Park(why);
        }
        match g2_failures(self.project.state, self.id) {
            Ok(failed) if failed.is_empty() => Stop::Park(why),
            Ok(failed) => Stop::Park(format!("{why}: {failed}")),
            Err(e) => Stop::Fail(e),
        }
    }

    /// The rounds of build mode in `mode` and G2, at least one, the first with `first`: whether
    /// G2 passed.
    fn rounds(&mut self, first: &str, mode: &str) -> Result<bool, Stop> {
        let mut args = first;
        for _ in 0..self.decided.retries.max(1) {
            self.call(Who::BuilderBuild, args)?;
            if self.steps.gate(Gate::G2)? {
                return Ok(true);
            }
            args = mode;
        }
        Ok(false)
    }

    /// G3, the review, the merge and pre; then the pause at G4, or production. A polish pass
    /// (`polishing`) with nothing new for the main branch skips the merge (see [`Run::merged`]).
    fn integrate(&mut self, polishing: bool) -> Result<Ended, Stop> {
        self.g3()?;
        if self.decided.reviewer {
            self.review()?;
        }
        let note = "automatic chain and Reviewer";
        self.steps.decide("G3", "ok", "machines", note)?;
        let Project { work, main, .. } = *self.project;
        let said = |e: git::Error| Stop::Fail(e.to_string());
        if !(polishing && nothing_new(work, self.id, main).map_err(said)?) {
            self.merged()?;
        }
        self.steps.release("pre")?;
        if self.decided.g4 {
            return Ok(Ended::Paused);
        }
        promoted(&mut *self.steps)
    }

    /// The merge, one at a time: under the merge lock, which the merge of another change makes
    /// it wait for, saying so (see [`queue::lock`]). The change merges in the main checkout: one
    /// that cannot take it parks the change (see [`merge::not_ready`]). When main moved since the
    /// change was built, it takes the latest main first (see [`on_latest_main`]).
    fn merged(&mut self) -> Result<(), Stop> {
        let project = *self.project;
        let said = |e: git::Error| Stop::Fail(e.to_string());
        let _lock = queue::lock(project.state, || self.steps.say(queue::WAITING))?;
        if let Some(why) = merge::not_ready(project.root, project.main).map_err(said)? {
            return Err(Stop::Park(why));
        }
        let at = Merging {
            state: project.state,
            work: project.work,
            main: project.main,
            id: self.id,
        };
        on_latest_main(&at, &mut *self.steps)?;
        self.steps.merge()?;
        Ok(())
    }

    /// G3; red, one fix by the Builder, G2, and G3 again: G3 decides, as it runs the whole
    /// automatic chain. Still red, the change parks.
    fn g3(&mut self) -> Result<(), Stop> {
        if self.steps.gate(Gate::G3)? {
            return Ok(());
        }
        self.call(Who::BuilderBuild, "mode=build fix=g3")?;
        self.steps.gate(Gate::G2)?;
        if self.steps.gate(Gate::G3)? {
            Ok(())
        } else {
            Err(Stop::Park(G3_RED.to_string()))
        }
    }

    /// The Reviewer; KO, one fix by the Builder, G2, G3 and a second review. KO, or G3 red, the
    /// change parks.
    fn review(&mut self) -> Result<(), Stop> {
        if self.reviewed("round=1")? {
            return Ok(());
        }
        self.call(Who::BuilderBuild, "mode=build fix=review")?;
        self.steps.gate(Gate::G2)?;
        let g3 = self.steps.gate(Gate::G3)?;
        if self.reviewed("round=2")? && g3 {
            Ok(())
        } else {
            Err(Stop::Park(REVIEW_KO.to_string()))
        }
    }

    /// The Reviewer with `args`, kept in the record of the change: its review right before the
    /// call (see [`review_begun`]), its verdict right after it (see [`review_ended`]). A call that
    /// stops the recipe leaves its review without a verdict. Whether the Reviewer said OK.
    fn reviewed(&mut self, args: &str) -> Result<bool, Stop> {
        let (state, id) = (self.project.state, self.id);
        let hashes = self.steps.hashes()?;
        review_begun(state, id, hashes)?;
        self.call(Who::Reviewer, args)?;
        let ok = verdict::review(state, id)?;
        review_ended(state, id, ok)?;
        Ok(ok)
    }
}

/// Adds to the reviews of change `id`, in its record under the state root `state`, the review of
/// the call of the Reviewer about to start (see [`change::Review`]), with no verdict yet: the
/// output the call keeps (see [`review_output`]), `hashes`, what the change touched as it is now,
/// and, after a first review, the paths whose hash changed since the review before (see
/// [`differ`]).
fn review_begun(state: &Path, id: &str, hashes: guard::Hashes) -> Result<(), String> {
    let text = change::read_record(state, id)?;
    let path = change::record_path(id);
    let invalid = |e: serde_json::Error| format!("{path}: {e}");
    let mut reviews = change::reviews(&text).map_err(invalid)?;
    let changed = reviews.last().map(|last| differ(&last.hashes, &hashes));
    reviews.push(change::Review {
        output: review_output(state, id)?,
        verdict: None,
        changed,
        hashes,
    });
    let written = change::with_reviews(&text, &reviews).map_err(invalid)?;
    change::write(state, &path, &written)
}

/// Gives the last review of change `id`, in its record under the state root `state`, the
/// verdict of its call: `OK` when it is `ok`, else `KO`.
fn review_ended(state: &Path, id: &str, ok: bool) -> Result<(), String> {
    let text = change::read_record(state, id)?;
    let path = change::record_path(id);
    let invalid = |e: serde_json::Error| format!("{path}: {e}");
    let mut reviews = change::reviews(&text).map_err(invalid)?;
    if let Some(last) = reviews.last_mut() {
        last.verdict = Some(if ok { "OK" } else { "KO" }.to_string());
    }
    let written = change::with_reviews(&text, &reviews).map_err(invalid)?;
    change::write(state, &path, &written)
}

/// The output the next call of the Reviewer on change `id` keeps, from the folder of the change:
/// `outputs/reviewer-<n>.md`, `n` after the lines of the Reviewer on the change in the log under
/// the state root `state` (see [`log_lines`]), as the call numbers it (see [`invoke::run`]).
fn review_output(state: &Path, id: &str) -> Result<String, String> {
    let role = Who::Reviewer.role();
    let theirs = |line: &&Value| line["change"] == id && line["role"] == role;
    let earlier = log_lines(state)?.iter().filter(theirs).count();
    Ok(format!("outputs/{role}-{}.md", earlier + 1))
}

/// What the agent calls of change `id` cost so far, in USD: the `cost_usd` of its lines in the
/// log under the state root `state` (see [`log_lines`]), 0 where a line has none.
fn spent(state: &Path, id: &str) -> Result<f64, String> {
    let lines = log_lines(state)?;
    let theirs = lines.iter().filter(|line| line["change"] == id);
    let costs = theirs.map(|line| line["cost_usd"].as_f64().unwrap_or(0.0));
    Ok(costs.sum())
}

/// The lines of the log under the state root `state`, in order: no log, none. Lines that are not
/// JSON are left out; a log that is not a file of its own is an error.
fn log_lines(state: &Path) -> Result<Vec<Value>, String> {
    let unread = |e: io::Error| format!("cannot read {}: {e}", log::PATH);
    let bytes = files::read(state, log::PATH).map_err(unread)?;
    let text = String::from_utf8_lossy(&bytes.unwrap_or_default()).into_owned();
    let parsed = |line: &str| serde_json::from_str::<Value>(line).ok();
    Ok(text.lines().filter_map(parsed).collect())
}

/// What the last G2 of change `id` failed on, from its report under the state root `state`: the
/// lines of the text of each failure that are not blank, `; ` between them. Nothing without a
/// report; one that cannot be read, or is not JSON, is an error.
fn g2_failures(state: &Path, id: &str) -> Result<String, String> {
    let path = Gate::G2.report_path(id);
    let bytes = files::read(state, &path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let Some(bytes) = bytes else {
        return Ok(String::new());
    };
    let report: Value = serde_json::from_slice(&bytes).map_err(|e| format!("{path}: {e}"))?;
    let failures = report["failures"].as_array().into_iter().flatten();
    let tails = failures.filter_map(|failure| failure["tail"].as_str());
    let lines = tails.flat_map(str::lines);
    let kept: Vec<&str> = lines.filter(|line| !line.trim().is_empty()).collect();
    Ok(kept.join("; "))
}

/// What a file of GDD state is during an agent call, which says how it may change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// The record of the change: only in [`AGENT_FIELDS`].
    Record,
    /// The log, and the sessions of the calls: only by lines added at their end.
    Log,
    /// An output of the role that is called, which gdd keeps for each call (see
    /// [`call_output`]): it may be added, never changed.
    Output,
    /// The report of the G2 a Builder runs: only into a report of G2.
    Report,
    /// The answer of the Reviewer or of the Orchestrator: anything, as long as it stays a file
    /// of its own.
    Answer,
    /// Anything else, from the approvals to the snapshots Path Guard judges from: never.
    Kept,
}

impl Kind {
    /// The kind of the file `path` of GDD state during a call of `who` on change `id`.
    fn of(path: &str, id: &str, who: Who) -> Kind {
        let own = format!(".gdd/changes/{id}/");
        let Some(inside) = path.strip_prefix(&own) else {
            let logs = [log::PATH, invoke::SESSIONS];
            return if logs.contains(&path) {
                Kind::Log
            } else {
                Kind::Kept
            };
        };
        let builds = matches!(who, Who::BuilderTest | Who::BuilderBuild);
        let output = inside.strip_prefix(&format!("outputs/{}-", who.role()));
        if path == change::record_path(id) {
            Kind::Record
        } else if builds && path == Gate::G2.report_path(id) {
            Kind::Report
        } else if who.answer(id).as_deref() == Some(path) {
            Kind::Answer
        } else if output.is_some_and(call_output) {
            Kind::Output
        } else {
            Kind::Kept
        }
    }
}

/// Whether `name`, what follows `outputs/<role>-` in the folder of a change, names an output gdd
/// keeps of a call of that role (see [`invoke::run`]): the number of the call, then `.md` or
/// `.raw`. Nothing else an agent leaves there, a `.session` say, is one.
fn call_output(name: &str) -> bool {
    let raw = || name.strip_suffix(".raw");
    let number = name.strip_suffix(".md").or_else(raw);
    number.is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// GDD state as it was before a call of `who` on change `id`: every file of it (see
/// [`state_files`]) with its bytes and its permissions, read as a file of its own (see
/// [`files::found`]). Agents change it only through gdd.
struct State<'a> {
    id: &'a str,
    who: Who,
    files: BTreeMap<String, Found>,
}

impl<'a> State<'a> {
    /// The GDD state of a call of `who` on change `id`, under the state root `root`, as it is
    /// now. What is not a file of its own cannot be read.
    fn of(root: &Path, id: &'a str, who: Who) -> Result<State<'a>, String> {
        let mut files = BTreeMap::new();
        for path in state_files(root, id)? {
            let unread = |e: io::Error| format!("cannot read {path}: {e}");
            if let Some(found) = files::found(root, &path).map_err(unread)? {
                files.insert(path, found);
            }
        }
        Ok(State { id, who, files })
    }

    /// Puts back as it was, with its permissions, each file of this state whose bytes changed in
    /// a way its kind does not allow, or that is no longer a file of its own, and removes what was
    /// not there: their paths, sorted. Every file is checked and put back before a file that
    /// could not be put back stops the recipe.
    fn put_back(&self, root: &Path) -> Result<Vec<String>, String> {
        let after = state_files(root, self.id)?;
        // Sorted, a folder that is no longer one goes before what was in it comes back.
        let paths: BTreeSet<&String> = self.files.keys().chain(&after).collect();
        let (mut changed, mut stuck) = (Vec::new(), Vec::new());
        for path in paths {
            let kind = Kind::of(path, self.id, self.who);
            let found = self.files.get(path);
            let before = found.map(|file| file.bytes.as_slice());
            let now = files::read(root, path).ok();
            if now.is_some_and(|now| allowed(kind, before, now.as_deref())) {
                continue;
            }
            let restored = match found {
                Some(file) => files::put_back(root, path, file),
                None => files::remove(root, path),
            };
            if let Err(e) = restored {
                stuck.push(format!("cannot put back {path}: {e}"));
            }
            changed.push(path.clone());
        }
        if stuck.is_empty() {
            Ok(changed)
        } else {
            Err(stuck.join("; "))
        }
    }
}

/// The files of GDD state an agent call is checked on, under the state root `root`, as paths
/// from it: the ones gdd keeps for every change ([`SHARED`]), whatever is at their path, the
/// others right in `.gdd/`, and every one in the folders of [`HARNESS`] and in the folder of
/// change `id`, at any depth. The other folders of `.gdd/`, other changes' or ones gdd decides
/// nothing from, are left out, and so are the locks, which the launches, merges and calls of
/// other changes, and people, make and take while the call runs (see [`queue::LOCKS`]). Links
/// are never followed: one where a folder was is a file here.
fn state_files(root: &Path, id: &str) -> Result<BTreeSet<String>, String> {
    let mut paths = BTreeSet::from(SHARED.map(String::from));
    listed(root, STATE_FOLDER, false, &mut paths)?;
    for lock in queue::LOCKS {
        paths.remove(lock);
    }
    let own = format!(".gdd/changes/{id}");
    for folder in HARNESS.into_iter().chain([own.as_str()]) {
        listed(root, folder, true, &mut paths)?;
    }
    Ok(paths)
}

/// Adds to `paths` what is in the folder `folder` of the state root `root` and is not a folder,
/// and, `deep`, what is in its folders too. A `folder` that is not one is added itself; nothing
/// there, or a file on the way to it, adds nothing.
fn listed(
    root: &Path,
    folder: &str,
    deep: bool,
    paths: &mut BTreeSet<String>,
) -> Result<(), String> {
    let unread = |e: io::Error| format!("cannot read {folder}: {e}");
    match fs::symlink_metadata(root.join(folder)) {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => {
            paths.insert(folder.to_string());
            return Ok(());
        }
        Err(e) if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => {
            return Ok(());
        }
        Err(e) => return Err(unread(e)),
    }
    for entry in fs::read_dir(root.join(folder)).map_err(unread)? {
        let entry = entry.map_err(unread)?;
        let path = format!("{folder}/{}", entry.file_name().to_string_lossy());
        if !entry.file_type().map_err(unread)?.is_dir() {
            paths.insert(path);
        } else if deep {
            listed(root, &path, deep, paths)?;
        }
    }
    Ok(())
}

/// Whether a file of GDD state of `kind` may go from `before` to `now` during an agent call
/// (`None`: no file).
fn allowed(kind: Kind, before: Option<&[u8]>, now: Option<&[u8]>) -> bool {
    match kind {
        Kind::Record => match (before.and_then(own_fields), now.and_then(own_fields)) {
            (Some(was), Some(is)) => was == is,
            _ => before == now,
        },
        Kind::Log => {
            let (before, now) = (before.unwrap_or_default(), now.unwrap_or_default());
            now.starts_with(before)
        }
        Kind::Output => before.is_none() || before == now,
        Kind::Report => before == now || now.is_some_and(g2_report),
        Kind::Answer => true,
        Kind::Kept => before == now,
    }
}

/// Whether `bytes` are a report of G2, a JSON object of that gate, as `gdd g2` saves it.
fn g2_report(bytes: &[u8]) -> bool {
    let report = serde_json::from_slice::<Map<String, Value>>(bytes);
    let gate = Value::from(Gate::G2.to_string());
    report.is_ok_and(|report| report.get("gate") == Some(&gate))
}

/// The fields of the record `bytes` that no agent may change, if it is a JSON object.
fn own_fields(bytes: &[u8]) -> Option<Map<String, Value>> {
    let mut record: Map<String, Value> = serde_json::from_slice(bytes).ok()?;
    for field in AGENT_FIELDS {
        record.remove(field);
    }
    Some(record)
}

/// What an agent left in the worktree `work` that only the state root `state` holds: a state
/// folder, `.gdd`, which goes, never what a link there points to. Its path, when there was one.
/// Nothing is looked at when the worktree is the state root itself.
fn state_left(work: &Path, state: &Path) -> Result<Vec<String>, String> {
    let path = work.join(STATE_FOLDER);
    let shown = path.display();
    if work == state {
        return Ok(Vec::new());
    }
    match fs::symlink_metadata(&path) {
        Ok(_) => {}
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("cannot look at {shown}: {e}")),
    }
    let removed = files::remove(work, STATE_FOLDER);
    removed.map_err(|e| format!("cannot remove {shown}: {e}"))?;
    Ok(vec![shown.to_string()])
}

/// The working tree of a project at one moment: each path, and what is there (see [`entry`]).
type Tree = BTreeMap<String, String>;

/// The working tree of the project at `root` outside `.gdd/`: each file git tracks, or does not
/// track and does not ignore.
fn working_tree(root: &Path) -> Result<Tree, String> {
    let git = Git::new(root);
    let said = |e: git::Error| e.to_string();
    let head = git.head().map_err(said)?;
    let branch = git.current_branch().map_err(said)?;
    let mut paths = git.files_at(&head, ".").map_err(said)?;
    paths.extend(git.changed_since(&branch, &head).map_err(said)?);
    paths.retain(|path| !path.starts_with(LOCAL_STATE));
    let seen = |path: String| {
        let what = entry(root, &path);
        (path, what)
    };
    Ok(paths.into_iter().map(seen).collect())
}

/// What is at `path` of the project at `root`, without following a link there: its
/// permissions and its kind, with the hash of a file or where a link points;
/// [`guard::DELETED`] for nothing. What cannot be read says why.
fn entry(root: &Path, path: &str) -> String {
    let at = root.join(path);
    let seen = fs::symlink_metadata(&at).and_then(|meta| {
        let kind = meta.file_type();
        let what = if kind.is_symlink() {
            format!("link to {:?}", fs::read_link(&at)?)
        } else if kind.is_file() {
            format!("file {}", guard::file_hash(&at)?)
        } else if kind.is_dir() {
            "folder".to_string()
        } else {
            "other".to_string()
        };
        Ok(format!("{:o} {what}", meta.permissions().mode() & 0o7777))
    });
    match seen {
        Ok(seen) => seen,
        Err(e) if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => {
            guard::DELETED.to_string()
        }
        Err(e) => format!("unreadable: {e}"),
    }
}

/// The paths with something else in `before` and in `after`, sorted.
fn differ(before: &Tree, after: &Tree) -> Vec<String> {
    let paths: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    let differs = |path: &&String| before.get(*path) != after.get(*path);
    paths.into_iter().filter(differs).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::{Folder, Repo};
    use std::collections::VecDeque;
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    /// A git repository with one commit and the record of change `c1`, as a folder.
    fn repo_folder() -> Folder {
        let folder = Repo::new().0;
        folder.write(&change::record_path("c1"), "{}\n");
        folder
    }

    /// A model named `name`.
    fn model(name: &str) -> Model {
        let table = serde_json::json!({"integration": "claude", "model": name});
        table.as_object().cloned().unwrap()
    }

    /// A yellow change with tests first, a Reviewer, no G4 and three rounds; each role has a
    /// model of its own.
    fn decided() -> Decided {
        Decided {
            tests_first: true,
            resume_green: false,
            reviewer: true,
            g4: false,
            retries: 3,
            builder: model("b"),
            builder_test: model("t"),
            reviewer_model: model("r"),
            orchestrator: model("o"),
            kind: "Feature".to_string(),
            builder_session: Session::Separate,
        }
    }

    /// The steps of the recipe, played: what ran, in order, with scripted answers, and the
    /// Builder session each agent call got. A gate passes once its answers run out; the Reviewer
    /// says OK, and the replan build mode; what the change touched hashes to nothing; the work of
    /// an earlier run passes the gates. Each call of an agent does what `acts` gives it to do, on
    /// the project root, and a release step what `released` gives it. A release step, or a
    /// decision on a gate, named in `fails` fails with what it gives.
    struct Played<'a> {
        root: &'a Path,
        ran: Vec<String>,
        sessions: Vec<Session>,
        decided: Decided,
        g2: VecDeque<bool>,
        g3: VecDeque<bool>,
        reviews: VecDeque<&'static str>,
        hashes: VecDeque<guard::Hashes>,
        replan: &'static str,
        guard: Vec<String>,
        earlier: Option<Vec<String>>,
        stuck: Option<Who>,
        acts: Vec<(Who, Act)>,
        released: Vec<(&'static str, Act)>,
        fails: Vec<(&'static str, &'static str)>,
    }

    /// What an agent does on the project root when it is called.
    type Act = fn(&Path);

    fn played(root: &Path) -> Played<'_> {
        Played {
            root,
            ran: Vec::new(),
            sessions: Vec::new(),
            decided: decided(),
            g2: VecDeque::new(),
            g3: VecDeque::new(),
            reviews: VecDeque::new(),
            hashes: VecDeque::new(),
            replan: "NEXT: build",
            guard: Vec::new(),
            earlier: Some(Vec::new()),
            stuck: None,
            acts: Vec::new(),
            released: Vec::new(),
            fails: Vec::new(),
        }
    }

    /// The short name of `who` in what ran.
    fn short(who: Who) -> &'static str {
        match who {
            Who::BuilderTest => "test",
            Who::BuilderBuild => "build",
            Who::Reviewer => "review",
            Who::Orchestrator => "replan",
        }
    }

    /// The last step that ran.
    fn last(steps: &Played) -> String {
        steps.ran.last().cloned().unwrap_or_default()
    }

    /// What ran, but waiting for the merge lock. A run that takes it right after another one let
    /// it go may wait a moment: a process another test starts meanwhile keeps the lock of the run
    /// before until it starts its program (a lock belongs to the open file, see `File::lock`).
    fn unwaited(steps: &Played) -> Vec<String> {
        let waited = format!("say {}", queue::WAITING);
        let mut ran = steps.ran.clone();
        ran.retain(|step| *step != waited);
        ran
    }

    impl Played<'_> {
        /// Writes `text` as the answer of an agent in `path`.
        fn answer(&self, path: &str, text: &str) {
            change::write(self.root, path, &format!("{text}\n")).unwrap();
        }

        /// How the step `name` ends: with what `fails` gives it, if anything.
        fn ended(&self, name: &str) -> Result<(), String> {
            match self.fails.iter().find(|(step, _)| *step == name) {
                Some((_, why)) => Err(why.to_string()),
                None => Ok(()),
            }
        }
    }

    impl Steps for Played<'_> {
        fn prepare(&mut self) -> Result<Decided, String> {
            self.ran.push("prepare".into());
            Ok(self.decided.clone())
        }

        fn agent(&mut self, call: &Call) -> Result<(), invoke::Error> {
            let name = call.model.get("model").and_then(Value::as_str);
            let name = name.unwrap_or_default();
            let step = format!("{} {name} {}", short(call.who), call.args);
            self.ran.push(step);
            self.sessions.push(call.builder_session);
            let acts = self.acts.iter().filter(|(who, _)| *who == call.who);
            acts.for_each(|(_, act)| act(self.root));
            if self.stuck == Some(call.who) {
                return Err(invoke::Error::Timeout(call.who, 2));
            }
            match call.who {
                Who::Reviewer => {
                    let said = self.reviews.pop_front().unwrap_or("VERDICT: OK");
                    self.answer(&verdict::review_path("c1"), said);
                }
                Who::Orchestrator => self.answer(&verdict::replan_path("c1"), self.replan),
                Who::BuilderTest | Who::BuilderBuild => {}
            }
            Ok(())
        }

        fn guard_test(&mut self, since_snapshot: bool) -> Result<Vec<String>, String> {
            self.ran.push(format!("guard since={since_snapshot}"));
            Ok(self.guard.clone())
        }

        fn snapshot(&mut self, snapshot: Snapshot) -> Result<(), String> {
            self.ran.push(format!("snapshot {snapshot:?}"));
            Ok(())
        }

        fn guard_earlier(&mut self) -> Result<Option<Vec<String>>, String> {
            self.ran.push("guard earlier".into());
            Ok(self.earlier.clone())
        }

        fn hashes(&mut self) -> Result<guard::Hashes, String> {
            Ok(self.hashes.pop_front().unwrap_or_default())
        }

        fn gate(&mut self, gate: Gate) -> Result<bool, String> {
            self.ran.push(gate.to_string());
            let answers = match gate {
                Gate::G2 => &mut self.g2,
                Gate::G3 => &mut self.g3,
            };
            Ok(answers.pop_front().unwrap_or(true))
        }

        fn decide(
            &mut self,
            gate: &str,
            decision: &str,
            by: &str,
            note: &str,
        ) -> Result<(), String> {
            self.ran.push(format!("{gate} {decision} {by}: {note}"));
            self.ended(gate)
        }

        fn merge(&mut self) -> Result<String, String> {
            self.ran.push("merge".into());
            self.ended("merge").map(|()| MERGED.to_string())
        }

        fn release(&mut self, step: &str) -> Result<(), String> {
            self.ran.push(step.into());
            let acts = self.released.iter().filter(|(name, _)| *name == step);
            acts.for_each(|(_, act)| act(self.root));
            self.ended(step)
        }

        fn say(&mut self, line: &str) {
            self.ran.push(format!("say {line}"));
        }
    }

    /// Runs the recipe of a launch on change `c1`, once prepared, with `replans` replans, all in
    /// one folder: the main checkout is the worktree of the change.
    fn launched(steps: &mut Played, replans: u32) -> Result<Ended, Stop> {
        launched_within(steps, replans, 5.0)
    }

    /// Runs the recipe of a launch as [`launched`] does, with `spike_usd` as the budget of a
    /// spike.
    fn launched_within(steps: &mut Played, replans: u32, spike_usd: f64) -> Result<Ended, Stop> {
        let project = Project {
            root: steps.root,
            state: steps.root,
            worktree: steps.root,
            work: steps.root,
            main: "main",
            replans,
            max: 2,
            spike_usd,
        };
        let decided = steps.decided.clone();
        let mut run = Run {
            project: &project,
            id: "c1",
            steps,
            decided,
        };
        run.launched()
    }

    const OPENED: &str = "G3 ok machines: automatic chain and Reviewer";

    /// What runs from G3 on when everything passes and the change needs no G4.
    fn to_production() -> Vec<&'static str> {
        vec![
            "G3",
            "review r round=1",
            OPENED,
            "merge",
            "pre",
            "promote",
            "observe",
        ]
    }

    fn park(why: &str) -> Result<Ended, Stop> {
        Err(Stop::Park(why.to_string()))
    }

    #[test]
    fn a_change_without_g4_goes_through_every_step_to_production() {
        let folder = repo_folder();
        let mut steps = played(&folder.0);
        assert_eq!(launched(&mut steps, 1), Ok(Ended::Closed));
        let mut expected = vec![
            "test t mode=test",
            "guard since=false",
            "snapshot Build",
            "build b mode=build",
            "G2",
        ];
        expected.extend(to_production());
        assert_eq!(steps.ran, expected);
    }

    #[test]
    fn every_agent_call_gets_the_builder_session_of_the_change() {
        for session in [Session::Separate, Session::Shared] {
            let folder = repo_folder();
            let mut steps = played(&folder.0);
            steps.decided.builder_session = session;
            assert_eq!(launched(&mut steps, 1), Ok(Ended::Closed));
            // Test mode, build mode and the Reviewer, who ignores it.
            assert_eq!(steps.sessions, [session; 3], "{session:?}");
        }
    }

    #[test]
    fn a_decision_kept_without_a_builder_session_keeps_the_sessions_separate() {
        let record: Value = serde_json::from_str(RECORD).unwrap();
        let mut decision = record["decision"].clone();
        let kept: Decided = serde_json::from_value(decision.clone()).unwrap();
        assert_eq!(kept.builder_session, Session::Separate);
        decision["builder_session"] = "shared".into();
        let shared: Decided = serde_json::from_value(decision).unwrap();
        assert_eq!(shared.builder_session, Session::Shared);
    }

    #[test]
    fn test_mode_runs_only_for_changes_with_tests_first() {
        let folder = repo_folder();
        let mut steps = played(&folder.0);
        steps.decided.tests_first = false;
        assert_eq!(launched(&mut steps, 1), Ok(Ended::Closed));
        let mut expected = vec!["build b mode=build", "G2"];
        expected.extend(to_production());
        assert_eq!(steps.ran, expected);
    }

    #[test]
    fn code_that_passes_g2_already_skips_the_builder() {
        let folder = repo_folder();
        let mut steps = played(&folder.0);
        steps.decided.resume_green = true;
        assert_eq!(launched(&mut steps, 1), Ok(Ended::Closed));
        assert_eq!(steps.ran, to_production());
    }

    #[test]
    fn a_change_that_needs_g4_stops_in_pre() {
        let folder = repo_folder();
        let mut steps = played(&folder.0);
        steps.decided.g4 = true;
        steps.decided.reviewer = false;
        assert_eq!(launched(&mut steps, 1), Ok(Ended::Paused));
        let expected = [
            "test t mode=test",
            "guard since=false",
            "snapshot Build",
            "build b mode=build",
            "G2",
            "G3",
            OPENED,
            "merge",
            "pre",
        ];
        assert_eq!(steps.ran, expected);
    }

    #[test]
    fn test_mode_that_touched_what_it_must_not_parks_the_change() {
        let folder = repo_folder();
        let mut steps = played(&folder.0);
        steps.guard = vec!["src/a.rs: x".into(), "src/b.rs: y".into()];
        let why = "Path Guard failed after test mode: src/a.rs: x; src/b.rs: y";
        assert_eq!(launched(&mut steps, 1), park(why));
        assert_eq!(steps.ran, ["test t mode=test", "guard since=false"]);
    }

    #[test]
    fn g2_red_gets_its_rounds_then_the_replans_then_the_change_parks() {
        let folder = repo_folder();
        let mut steps = played(&folder.0);
        steps.decided.tests_first = false;
        steps.g2 = VecDeque::from([false; 9]);
        let why = "G2 still red after 3 retries and 1 replan";
        assert_eq!(launched(&mut steps, 1), park(why));
        let round = ["build b mode=build", "G2"];
        let replan = [
            "replan o reason=g2",
            "build b mode=build after=replan",
            "G2",
        ];
        let expected = [&round[..], &round, &round, &replan].concat();
        assert_eq!(steps.ran, expected);
        assert!(folder.0.join(".gdd/changes/c1/replan.md").exists());
        let mut steps = played(&folder.0);
        steps.decided.tests_first = false;
        steps.decided.retries = 2;
        steps.g2 = VecDeque::from([false; 9]);
        let why = "G2 still red after 2 retries and 2 replan";
        assert_eq!(launched(&mut steps, 2), park(why));
        let expected = [&round[..], &round, &replan, &replan].concat();
        assert_eq!(steps.ran, expected);
    }

    #[test]
    fn without_replans_a_change_still_red_parks_after_its_rounds() {
        let folder = repo_folder();
        let mut steps = played(&folder.0);
        steps.decided.tests_first = false;
        steps.decided.retries = 0;
        steps.g2 = VecDeque::from([false]);
        let why = "G2 still red after 0 retries and 0 replan";
        assert_eq!(launched(&mut steps, 0), park(why));
        assert_eq!(steps.ran, ["build b mode=build", "G2"]);
    }

    #[test]
    fn the_rounds_stop_when_g2_passes() {
        let folder = repo_folder();
        let mut steps = played(&folder.0);
        steps.decided.tests_first = false;
        steps.decided.reviewer = false;
        steps.g2 = VecDeque::from([false, true]);
        assert_eq!(launched(&mut steps, 1), Ok(Ended::Closed));
        let expected = [
            "build b mode=build",
            "G2",
            "build b mode=build",
            "G2",
            "G3",
            OPENED,
        ];
        assert_eq!(steps.ran[..6], expected);
    }

    #[test]
    fn the_replan_can_send_the_change_back_to_test_mode() {
        let folder = repo_folder();
        let mut steps = played(&folder.0);
        steps.decided.retries = 1;
        steps.replan = "NEXT: test";
        steps.g2 = VecDeque::from([false]);
        assert_eq!(launched(&mut steps, 1), Ok(Ended::Closed));
        let mut expected = vec![
            "test t mode=test",
            "guard since=false",
            "snapshot Build",
            "build b mode=build",
            "G2",
            "replan o reason=g2",
            "snapshot Round",
            "test t mode=test after=replan",
            "guard since=true",
            "snapshot Build",
            "build b mode=build after=replan",
            "G2",
        ];
        expected.extend(to_production());
        assert_eq!(steps.ran, expected);
        let mut steps = played(&folder.0);
        steps.decided.tests_first = false;
        steps.decided.retries = 1;
        steps.replan = "NEXT: test";
        steps.g2 = VecDeque::from([false]);
        steps.guard = vec!["src/a.rs: protected".into()];
        let why = "Path Guard failed after test mode: src/a.rs: protected";
        assert_eq!(launched(&mut steps, 1), park(why));
    }

    #[test]
    fn g3_red_gets_one_fix_and_then_g3_decides() {
        let folder = repo_folder();
        let mut steps = played(&folder.0);
        steps.decided.resume_green = true;
        steps.g2 = VecDeque::from([false]);
        steps.g3 = VecDeque::from([false]);
        assert_eq!(launched(&mut steps, 1), Ok(Ended::Closed));
        let mut expected = vec!["G3", "build b mode=build fix=g3", "G2"];
        expected.extend(to_production());
        assert_eq!(steps.ran, expected);
        let mut steps = played(&folder.0);
        steps.decided.resume_green = true;
        steps.g3 = VecDeque::from([false, false]);
        assert_eq!(launched(&mut steps, 1), park(G3_RED));
        let fixed = ["G3", "build b mode=build fix=g3", "G2", "G3"];
        assert_eq!(steps.ran, fixed);
    }

    #[test]
    fn a_reviewer_ko_gets_one_fix_and_a_second_review() {
        let folder = repo_folder();
        let mut steps = played(&folder.0);
        steps.decided.resume_green = true;
        steps.reviews = VecDeque::from(["VERDICT: KO"]);
        steps.g2 = VecDeque::from([false]);
        assert_eq!(launched(&mut steps, 1), Ok(Ended::Closed));
        let expected = [
            "G3",
            "review r round=1",
            "build b mode=build fix=review",
            "G2",
            "G3",
            "review r round=2",
            OPENED,
        ];
        assert_eq!(steps.ran[..7], expected);
        let log = fs::read_to_string(folder.0.join(log::PATH)).unwrap();
        let verdicts = log::parse(&log).unwrap();
        let results: Vec<String> = verdicts.iter().map(|r| r.text("result")).collect();
        assert_eq!(results, ["ko", "ok"]);
    }

    #[test]
    fn a_second_ko_or_g3_red_after_the_fix_parks_the_change() {
        for (reviews, g3) in [
            (vec!["VERDICT: KO", "VERDICT: KO"], vec![true]),
            (vec!["VERDICT: KO", "VERDICT: OK"], vec![true, false]),
            (vec!["no verdict", "VERDICT: KO"], vec![true, true]),
        ] {
            let folder = repo_folder();
            let mut steps = played(&folder.0);
            steps.decided.resume_green = true;
            steps.reviews = VecDeque::from(reviews);
            steps.g3 = VecDeque::from(g3);
            assert_eq!(launched(&mut steps, 1), park(REVIEW_KO));
            assert_eq!(last(&steps), "review r round=2");
        }
    }

    /// The reviews the record of change `c1` of the project at `root` keeps.
    fn reviews_of(root: &Path) -> Vec<change::Review> {
        change::reviews(&change::read_record(root, "c1").unwrap()).unwrap()
    }

    /// The hashes of `pairs`: each path with its hash.
    fn hashed(pairs: &[(&str, &str)]) -> guard::Hashes {
        let owned = |(path, hash): &(&str, &str)| (path.to_string(), hash.to_string());
        pairs.iter().map(owned).collect()
    }

    /// A review of the record: its output, `outputs/reviewer-<n>.md`, its verdict, what changed
    /// and its hashes.
    fn kept(
        n: usize,
        verdict: Option<&str>,
        changed: Option<&[&str]>,
        hashes: &guard::Hashes,
    ) -> change::Review {
        change::Review {
            output: format!("outputs/reviewer-{n}.md"),
            verdict: verdict.map(str::to_string),
            changed: changed.map(|paths| paths.iter().map(|path| path.to_string()).collect()),
            hashes: hashes.clone(),
        }
    }

    /// What a Reviewer call does besides its answer: its line in the log, as gdd writes it, and
    /// a look at its review in the record, which has no verdict yet.
    fn reviewing(root: &Path) {
        let reviews = reviews_of(root);
        let review = reviews.last().expect("its review");
        assert_eq!(review.verdict, None, "{review:?}");
        log::append(root, r#"{"change": "c1", "role": "reviewer"}"#).unwrap();
    }

    #[test]
    fn every_review_is_kept_with_its_verdict_and_what_changed_since_the_one_before() {
        let folder = repo_folder();
        let mut steps = played(&folder.0);
        steps.decided.resume_green = true;
        steps.reviews = VecDeque::from(["VERDICT: KO"]);
        steps.acts = vec![(Who::Reviewer, reviewing as Act)];
        let first = hashed(&[("src/a.rs", "1"), ("tests/a.rs", "1")]);
        let second = hashed(&[("src/a.rs", "2"), ("tests/a.rs", "1")]);
        steps.hashes = VecDeque::from([first.clone(), second.clone()]);
        assert_eq!(launched(&mut steps, 1), Ok(Ended::Closed));
        let expected = [
            kept(1, Some("KO"), None, &first),
            kept(2, Some("OK"), Some(&["src/a.rs"]), &second),
        ];
        assert_eq!(reviews_of(&folder.0), expected);
        // New and gone files changed too; one with the same hash did not.
        let folder = repo_folder();
        let mut steps = played(&folder.0);
        steps.decided.resume_green = true;
        steps.reviews = VecDeque::from(["VERDICT: KO"]);
        let first = hashed(&[("a", "1"), ("b", "1"), ("d", guard::DELETED)]);
        let second = hashed(&[("b", "1"), ("c", "1"), ("d", guard::DELETED)]);
        steps.hashes = VecDeque::from([first, second]);
        assert_eq!(launched(&mut steps, 1), Ok(Ended::Closed));
        let changed = reviews_of(&folder.0)
            .pop()
            .and_then(|review| review.changed);
        assert_eq!(changed, Some(vec!["a".to_string(), "c".to_string()]));
    }

    #[test]
    fn the_reviews_of_a_relaunch_follow_the_earlier_ones() {
        let folder = repo_folder();
        let hashes = [
            hashed(&[("a", "1")]),
            hashed(&[("a", "2")]),
            hashed(&[("a", "3")]),
        ];
        for scripted in [&hashes[..2], &hashes[1..]] {
            let mut steps = played(&folder.0);
            steps.decided.resume_green = true;
            steps.reviews = VecDeque::from(["VERDICT: KO", "VERDICT: KO"]);
            steps.acts = vec![(Who::Reviewer, reviewing as Act)];
            steps.hashes = scripted.iter().cloned().collect();
            assert_eq!(launched(&mut steps, 1), park(REVIEW_KO));
        }
        let ko = Some("KO");
        let expected = [
            kept(1, ko, None, &hashes[0]),
            kept(2, ko, Some(&["a"]), &hashes[1]),
            kept(3, ko, Some(&[]), &hashes[1]),
            kept(4, ko, Some(&["a"]), &hashes[2]),
        ];
        assert_eq!(reviews_of(&folder.0), expected);
    }

    #[test]
    fn a_review_whose_call_stops_the_recipe_has_no_verdict() {
        let folder = repo_folder();
        let mut steps = played(&folder.0);
        steps.decided.resume_green = true;
        steps.stuck = Some(Who::Reviewer);
        let why = "the Reviewer did not answer within 2 seconds";
        assert_eq!(launched(&mut steps, 1), park(why));
        let none = guard::Hashes::new();
        assert_eq!(reviews_of(&folder.0), [kept(1, None, None, &none)]);
        // A record that cannot be read stops the recipe before the call.
        let folder = repo_folder();
        fs::write(folder.0.join(change::record_path("c1")), "{\"reviews\": 1}").unwrap();
        let mut steps = played(&folder.0);
        steps.decided.resume_green = true;
        let ended = launched(&mut steps, 1);
        let start = ".gdd/changes/c1/change.json: ";
        assert!(
            matches!(&ended, Err(Stop::Fail(e)) if e.starts_with(start)),
            "{ended:?}"
        );
        assert_eq!(last(&steps), "G3");
    }

    #[test]
    fn the_output_of_a_review_follows_the_calls_of_the_reviewer_on_the_change_in_the_log() {
        let folder = Folder::new();
        assert_eq!(
            review_output(&folder.0, "c1"),
            Ok("outputs/reviewer-1.md".into())
        );
        let lines = [
            r#"{"change": "c1", "role": "reviewer"}"#,
            r#"{"change": "c2", "role": "reviewer"}"#,
            r#"{"change": "c1", "role": "builder-build"}"#,
            "not json",
            "[1]",
            r#"{"change": "c1", "role": "reviewer", "result": "timeout"}"#,
        ];
        folder.write(log::PATH, &lines.join("\n"));
        assert_eq!(
            review_output(&folder.0, "c1"),
            Ok("outputs/reviewer-3.md".into())
        );
        let folder = Folder::new();
        fs::create_dir_all(folder.0.join(log::PATH)).unwrap();
        let err = review_output(&folder.0, "c1").unwrap_err();
        assert!(err.starts_with("cannot read .gdd/log.jsonl: "), "{err}");
    }

    #[test]
    fn a_change_without_a_reviewer_is_not_reviewed() {
        let folder = repo_folder();
        let mut steps = played(&folder.0);
        steps.decided.resume_green = true;
        steps.decided.reviewer = false;
        assert_eq!(launched(&mut steps, 1), Ok(Ended::Closed));
        let expected = ["G3", OPENED, "merge", "pre", "promote", "observe"];
        assert_eq!(steps.ran, expected);
    }

    #[test]
    fn an_agent_that_does_not_answer_parks_and_a_failed_step_stops_the_recipe() {
        let folder = repo_folder();
        let mut steps = played(&folder.0);
        steps.stuck = Some(Who::BuilderBuild);
        let why = "the Builder in build mode did not answer within 2 seconds";
        assert_eq!(launched(&mut steps, 1), park(why));
        assert_eq!(last(&steps), "build b mode=build");
        let unknown = invoke::Error::Model(Who::Reviewer, "has no model".into());
        let failed = Stop::Fail("gdd/models.toml: the Reviewer has no model".into());
        assert_eq!(Stop::from(unknown), failed);
        let mut steps = played(&folder.0);
        steps.decided.resume_green = true;
        steps.fails = vec![("observe", "release.observe failed")];
        let failed = Stop::Fail("release.observe failed".into());
        assert_eq!(launched(&mut steps, 1), Err(failed));
    }

    /// Runs the recipe of a launch on change `c1` of the project at `folder`, where each call of
    /// `who` does `act`.
    fn acting(folder: &Folder, who: Who, act: Act) -> Result<Ended, Stop> {
        let mut steps = played(&folder.0);
        steps.acts = vec![(who, act)];
        launched(&mut steps, 1)
    }

    /// A repository with the record of change `c1` and a log of one line.
    fn with_state() -> Folder {
        let folder = repo_folder();
        folder.write(".gdd/changes/c1/change.json", RECORD);
        folder.write(log::PATH, LOG_LINE);
        folder
    }

    const LOG_LINE: &str = "{\"change\": \"c1\"}\n";

    fn text(folder: &Folder, path: &str) -> String {
        fs::read_to_string(folder.0.join(path)).unwrap_or_default()
    }

    #[test]
    fn an_agent_that_changes_gdd_state_parks_the_change_and_the_state_goes_back() {
        let record = change::record_path("c1");
        let folder = with_state();
        let ended = acting(&folder, Who::BuilderBuild, |root| {
            let path = change::record_path("c1");
            let text = fs::read_to_string(root.join(&path)).unwrap();
            change::write(root, &path, &text.replace("in-pre", "closed")).unwrap();
        });
        let why = format!("the Builder in build mode changed GDD state: {record}");
        assert_eq!(ended, park(&why));
        assert_eq!(text(&folder, &record), RECORD);
        // The approvals at all, and the log other than at its end, are the same.
        let folder = with_state();
        folder.write(approve::ZONES_PATH, "ok");
        let ended = acting(&folder, Who::BuilderTest, |root| {
            change::write(root, approve::G0_PATH, "ok").unwrap();
            change::write(root, approve::ZONES_PATH, "ko").unwrap();
            change::write(root, log::PATH, "{}\n").unwrap();
        });
        let why = "the Builder in test mode changed GDD state: .gdd/g0.json, .gdd/log.jsonl, \
                   .gdd/zones.json";
        assert_eq!(ended, park(why));
        assert!(!folder.0.join(approve::G0_PATH).exists());
        assert_eq!(text(&folder, approve::ZONES_PATH), "ok");
        assert_eq!(text(&folder, log::PATH), LOG_LINE);
        // A record that went away comes back, even with its folder, as gdd left it before the
        // call: with its review, without a verdict.
        let folder = with_state();
        let ended = acting(&folder, Who::Reviewer, |root| {
            fs::remove_file(root.join(change::record_path("c1"))).unwrap();
        });
        let why = format!("the Reviewer changed GDD state: {record}");
        assert_eq!(ended, park(&why));
        let review = change::Review {
            output: "outputs/reviewer-1.md".to_string(),
            verdict: None,
            changed: None,
            hashes: guard::Hashes::new(),
        };
        let reviewed = change::with_reviews(RECORD, &[review]).unwrap();
        assert_eq!(text(&folder, &record), reviewed);
        let folder = with_state();
        let ended = acting(&folder, Who::BuilderBuild, |root| {
            fs::remove_dir_all(root.join(".gdd/changes")).unwrap();
        });
        assert!(matches!(ended, Err(Stop::Park(_))), "{ended:?}");
        assert_eq!(text(&folder, &record), RECORD);
        // A file where the folders of the changes were goes before the record comes back.
        let folder = with_state();
        let ended = acting(&folder, Who::BuilderBuild, |root| {
            fs::remove_dir_all(root.join(".gdd/changes")).unwrap();
            change::write(root, ".gdd/changes", "x").unwrap();
        });
        let changed = format!(".gdd/changes, {record}");
        let why = format!("the Builder in build mode changed GDD state: {changed}");
        assert_eq!(ended, park(&why));
        assert_eq!(text(&folder, &record), RECORD);
    }

    #[test]
    fn every_file_of_the_change_and_of_the_shared_state_is_checked() {
        let folder = with_state();
        folder.write(&change::build_snapshot_path("c1"), "{}");
        folder.write(".gdd/changes/c1/review.md", "VERDICT: KO\n");
        folder.write(".gdd/changes/c1/outputs/reviewer-1.md", "VERDICT: KO\n");
        folder.write(".gdd/imported_requests.json", "[]");
        let ended = acting(&folder, Who::BuilderBuild, |root| {
            // What Path Guard judges from, a snapshot it would judge from, an answer and an
            // output of another role, and shared state with no name of its own here.
            let changed = [
                change::build_snapshot_path("c1"),
                change::snapshot_path("c1"),
                verdict::review_path("c1"),
                ".gdd/changes/c1/outputs/reviewer-1.md".to_string(),
                ".gdd/changes/c1/outputs/reviewer-2.md".to_string(),
                ".gdd/imported_requests.json".to_string(),
            ];
            for path in changed {
                change::write(root, &path, "{\"files\": {}}").unwrap();
            }
        });
        let why = "the Builder in build mode changed GDD state: .gdd/changes/c1/outputs/\
                   reviewer-1.md, .gdd/changes/c1/outputs/reviewer-2.md, .gdd/changes/c1/review.md, \
                   .gdd/changes/c1/snapshot-build.json, .gdd/changes/c1/snapshot.json, \
                   .gdd/imported_requests.json";
        assert_eq!(ended, park(why));
        assert_eq!(text(&folder, &change::build_snapshot_path("c1")), "{}");
        assert!(!folder.0.join(change::snapshot_path("c1")).exists());
        assert_eq!(text(&folder, ".gdd/changes/c1/review.md"), "VERDICT: KO\n");
        assert_eq!(text(&folder, ".gdd/imported_requests.json"), "[]");
    }

    #[test]
    fn the_harness_and_what_the_selftest_set_aside_come_back_as_they_were() {
        let folder = with_state();
        let (harness, program) = (".gdd/bin/gdd-rust", ".gdd/bin/agents/codex");
        let kept = ".gdd/selftest/kept/tests/failing.txt";
        for path in [harness, program, kept] {
            folder.write(path, "mine");
        }
        let executable = fs::Permissions::from_mode(0o755);
        fs::set_permissions(folder.0.join(harness), executable).unwrap();
        let ended = acting(&folder, Who::BuilderBuild, |root| {
            change::write(root, ".gdd/bin/gdd-rust", "the Builder's own judge").unwrap();
            fs::remove_dir_all(root.join(".gdd/bin/agents")).unwrap();
            change::write(root, ".gdd/bin/new", "x").unwrap();
            change::write(root, ".gdd/selftest/kept/tests/failing.txt", "x").unwrap();
        });
        let why = format!(
            "the Builder in build mode changed GDD state: .gdd/bin/agents/codex, \
             .gdd/bin/gdd-rust, .gdd/bin/new, {kept}"
        );
        assert_eq!(ended, park(&why));
        for path in [harness, program, kept] {
            assert_eq!(text(&folder, path), "mine", "{path}");
        }
        assert!(!folder.0.join(".gdd/bin/new").exists());
        let meta = fs::metadata(folder.0.join(harness)).unwrap();
        assert_eq!(meta.permissions().mode() & 0o777, 0o755);
    }

    /// A report of G2, as `gdd g2` saves it.
    const G2_REPORT: &str = "{\"ok\": true, \"gate\": \"G2\"}\n";

    #[test]
    fn a_session_file_an_agent_leaves_next_to_the_outputs_goes_back_and_parks_the_change() {
        // No file holds a Builder session: gdd keeps it in memory for one run (`invoke::Ongoing`).
        let outputs = ".gdd/changes/c1/outputs";
        let plants: [(Who, Act); 3] = [
            (Who::BuilderTest, |root| {
                let session = ".gdd/changes/c1/outputs/builder-test-1.session";
                change::write(root, session, "s-evil").unwrap();
            }),
            (Who::BuilderBuild, |root| {
                let session = ".gdd/changes/c1/outputs/builder-build-1.session";
                change::write(root, session, "s-evil").unwrap();
            }),
            (Who::Reviewer, |root| {
                let session = ".gdd/changes/c1/outputs/reviewer-1.session";
                change::write(root, session, "s-evil").unwrap();
            }),
        ];
        for (who, plant) in plants {
            let folder = with_state();
            let session = format!("{outputs}/{}-1.session", who.role());
            let why = format!("the {who} changed GDD state: {session}");
            assert_eq!(acting(&folder, who, plant), park(&why), "{who}");
            assert!(!folder.0.join(&session).exists(), "{who}");
        }
    }

    #[test]
    fn agents_may_change_gdd_state_through_gdd() {
        let folder = with_state();
        let ended = acting(&folder, Who::BuilderBuild, |root| {
            let path = change::record_path("c1");
            let text = fs::read_to_string(root.join(&path)).unwrap();
            let ts = log::now();
            let note = Note {
                kind: "decided",
                text: "kept it",
                ts: &ts,
            };
            let noted = change::with_note(&text, &note).unwrap();
            let paths = ["docs/x.md".to_string()];
            let scoped = change::with_out_of_scope(&noted, &paths).unwrap();
            change::write(root, &path, &scoped.unwrap()).unwrap();
            log::append(root, "{\"change\": \"c1\", \"role\": \"machine\"}").unwrap();
            // Its G2, what gdd keeps of its call, and what is neither its change's nor shared.
            change::write(root, ".gdd/changes/c1/g2.json", G2_REPORT).unwrap();
            change::write(root, ".gdd/changes/c1/outputs/builder-build-1.md", "done").unwrap();
            change::write(root, invoke::SESSIONS, "s-1\n").unwrap();
            change::write(root, ".gdd/changes/c2/change.json", "{}").unwrap();
            change::write(root, ".gdd/fake/calls", "builder-build\n").unwrap();
        });
        assert_eq!(ended, Ok(Ended::Closed));
        assert!(text(&folder, &change::record_path("c1")).contains("kept it"));
        let log = text(&folder, log::PATH);
        let appended = log.starts_with(LOG_LINE) && log.contains("machine");
        assert!(appended, "{log}");
        assert_eq!(text(&folder, ".gdd/changes/c1/g2.json"), G2_REPORT);
        // The answer of each role is its own.
        let folder = with_state();
        let mut steps = played(&folder.0);
        steps.decided.reviewer = false;
        steps.g2 = VecDeque::from([false, false, false]);
        steps.replan = "NEXT: test";
        assert_eq!(launched(&mut steps, 1), Ok(Ended::Closed));
        assert_eq!(text(&folder, &verdict::replan_path("c1")), "NEXT: test\n");
        // Without a record or a log before the call, nothing counts but what the call wrote. A
        // review would write the record.
        let folder = repo_folder();
        fs::remove_file(folder.0.join(change::record_path("c1"))).unwrap();
        let mut steps = played(&folder.0);
        steps.decided.reviewer = false;
        let logs: Act = |root| log::append(root, "{}").unwrap();
        steps.acts = vec![(Who::BuilderBuild, logs)];
        assert_eq!(launched(&mut steps, 1), Ok(Ended::Closed));
        let ended = acting(&folder, Who::BuilderBuild, |root| {
            change::write(root, &change::record_path("c1"), "{}").unwrap();
        });
        let why = "the Builder in build mode changed GDD state: .gdd/changes/c1/change.json";
        assert_eq!(ended, park(why));
        assert!(!folder.0.join(change::record_path("c1")).exists());
    }

    #[test]
    fn gdd_state_is_checked_even_when_the_agent_fails() {
        let folder = with_state();
        let mut steps = played(&folder.0);
        steps.stuck = Some(Who::BuilderTest);
        let act: Act = |root| change::write(root, approve::G0_PATH, "{}").unwrap();
        steps.acts = vec![(Who::BuilderTest, act)];
        let why = "the Builder in test mode changed GDD state: .gdd/g0.json";
        assert_eq!(launched(&mut steps, 1), park(why));
        assert!(!folder.0.join(approve::G0_PATH).exists());
    }

    #[test]
    fn gdd_state_that_cannot_be_read_before_the_call_stops_the_recipe() {
        let unread = |ended: &Result<Ended, Stop>, path: &str| match ended {
            Err(Stop::Fail(e)) => e.starts_with(&format!("cannot read {path}: ")),
            _ => false,
        };
        let folder = with_state();
        fs::create_dir_all(folder.0.join(approve::ZONES_PATH)).unwrap();
        let mut steps = played(&folder.0);
        let ended = launched(&mut steps, 1);
        assert!(unread(&ended, approve::ZONES_PATH), "{ended:?}");
        assert!(steps.ran.is_empty(), "{:?}", steps.ran);
        // A link is not read: what it points to is not GDD state.
        let folder = with_state();
        let root = &folder.0;
        symlink(root.join("README.md"), root.join(approve::G0_PATH)).unwrap();
        let ended = launched(&mut played(root), 1);
        assert!(unread(&ended, approve::G0_PATH), "{ended:?}");
    }

    #[test]
    fn gdd_state_that_is_no_longer_a_file_goes_back_and_parks_the_change() {
        let folder = with_state();
        folder.write(approve::ZONES_PATH, "ok");
        let ended = acting(&folder, Who::BuilderTest, |root| {
            fs::create_dir_all(root.join(approve::G0_PATH).join("x")).unwrap();
            for path in [approve::ZONES_PATH, log::PATH] {
                fs::remove_file(root.join(path)).unwrap();
                fs::create_dir(root.join(path)).unwrap();
            }
        });
        let why = "the Builder in test mode changed GDD state: .gdd/g0.json, .gdd/log.jsonl, \
                   .gdd/zones.json";
        assert_eq!(ended, park(why));
        assert!(!folder.0.join(approve::G0_PATH).exists());
        assert_eq!(text(&folder, approve::ZONES_PATH), "ok");
        assert_eq!(text(&folder, log::PATH), LOG_LINE);
    }

    #[test]
    fn gdd_state_is_never_read_or_put_back_through_a_link() {
        let record = change::record_path("c1");
        let folder = with_state();
        folder.write(approve::ZONES_PATH, "ok");
        let ended = acting(&folder, Who::BuilderBuild, |root| {
            // The same bytes, through a link; and a link to other bytes.
            let record = root.join(change::record_path("c1"));
            fs::rename(&record, root.join("record.json")).unwrap();
            symlink(root.join("record.json"), &record).unwrap();
            change::write(root, "zones.json", "ko").unwrap();
            fs::remove_file(root.join(approve::ZONES_PATH)).unwrap();
            symlink(root.join("zones.json"), root.join(approve::ZONES_PATH)).unwrap();
            // A second name of the log.
            fs::hard_link(root.join(log::PATH), root.join("log.jsonl")).unwrap();
        });
        let changed = [record.as_str(), log::PATH, approve::ZONES_PATH].join(", ");
        let why = format!("the Builder in build mode changed GDD state: {changed}");
        assert_eq!(ended, park(&why));
        assert_eq!(own(&folder, &record), RECORD);
        assert_eq!(own(&folder, approve::ZONES_PATH), "ok");
        assert_eq!(own(&folder, log::PATH), LOG_LINE);
        assert_eq!(text(&folder, "zones.json"), "ko");
        // A folder on the way that links elsewhere: the link goes, not what it points to.
        let folder = with_state();
        let ended = acting(&folder, Who::BuilderBuild, |root| {
            let c1 = root.join(".gdd/changes/c1");
            fs::create_dir(root.join("elsewhere")).unwrap();
            fs::rename(&c1, root.join("elsewhere/c1")).unwrap();
            symlink(root.join("elsewhere/c1"), &c1).unwrap();
        });
        let changed = format!(".gdd/changes/c1, {record}");
        let why = format!("the Builder in build mode changed GDD state: {changed}");
        assert_eq!(ended, park(&why));
        assert_eq!(own(&folder, &record), RECORD);
        assert_eq!(text(&folder, "elsewhere/c1/change.json"), RECORD);
    }

    #[test]
    fn links_and_files_left_in_the_folder_of_the_change_go_and_park_it() {
        let folder = with_state();
        folder.write("target.json", "outside");
        let ended = acting(&folder, Who::BuilderBuild, |root| {
            let c1 = root.join(".gdd/changes/c1");
            symlink(root.join("target.json"), c1.join("g2.json")).unwrap();
            fs::create_dir_all(c1.join("outputs")).unwrap();
            fs::hard_link(root.join("target.json"), c1.join("outputs/x.md")).unwrap();
            symlink(root.join("src"), c1.join("outputs/src")).unwrap();
            change::write(root, ".gdd/changes/c1/notes.md", "mine\n").unwrap();
        });
        let why = "the Builder in build mode changed GDD state: .gdd/changes/c1/g2.json, \
                   .gdd/changes/c1/notes.md, .gdd/changes/c1/outputs/src, \
                   .gdd/changes/c1/outputs/x.md";
        assert_eq!(ended, park(why));
        let c1 = folder.0.join(".gdd/changes/c1");
        for path in ["g2.json", "notes.md", "outputs/x.md", "outputs/src"] {
            assert!(fs::symlink_metadata(c1.join(path)).is_err(), "{path}");
        }
        assert_eq!(text(&folder, "target.json"), "outside");
        assert_eq!(text(&folder, &change::record_path("c1")), RECORD);
    }

    /// `paths`, and the files gdd keeps for every change.
    fn with_shared(paths: &[&str]) -> BTreeSet<String> {
        let mut all = BTreeSet::from(SHARED.map(String::from));
        all.extend(paths.iter().map(|path| path.to_string()));
        all
    }

    #[test]
    fn the_state_of_a_call_is_what_is_right_in_its_folder_and_in_the_folder_of_its_change() {
        let folder = Folder::new();
        assert_eq!(state_files(&folder.0, "c1"), Ok(with_shared(&[])));
        for path in [
            "imported_requests.json",
            "changes/c1/notes.md",
            "changes/c1/outputs/a/b.md",
            "changes/c2/change.json",
            "changes/c10/change.json",
            "bin/gdd-rust",
            "bin/agents/codex",
            "selftest/kept/tests/failing.txt",
            "fake/argv",
            "pre/x",
            "adoption/x",
            "merge.lock",
            "launch.lock",
            "agents.lock",
            "state.lock",
        ] {
            folder.write(&format!(".gdd/{path}"), "x");
        }
        let listed = [
            ".gdd/imported_requests.json",
            ".gdd/changes/c1/notes.md",
            ".gdd/changes/c1/outputs/a/b.md",
            ".gdd/bin/gdd-rust",
            ".gdd/bin/agents/codex",
            ".gdd/selftest/kept/tests/failing.txt",
        ];
        assert_eq!(state_files(&folder.0, "c1"), Ok(with_shared(&listed)));
        // A folder that cannot be looked at is an error, not an empty one.
        let long = "x".repeat(300);
        let err = state_files(&folder.0, &long).unwrap_err();
        let start = format!("cannot read .gdd/changes/{long}: ");
        assert!(err.starts_with(&start), "{err}");
        // What is no longer a folder is a file, and a file on the way to one lists nothing.
        let folder = Folder::new();
        folder.write(".gdd/changes/c1", "x");
        let listed = with_shared(&[".gdd/changes/c1"]);
        assert_eq!(state_files(&folder.0, "c1"), Ok(listed));
        let folder = Folder::new();
        folder.write(".gdd/changes", "x");
        let listed = with_shared(&[".gdd/changes"]);
        assert_eq!(state_files(&folder.0, "c1"), Ok(listed));
    }

    #[test]
    fn what_a_call_may_write_depends_on_who_is_called() {
        let of = |path: &str, who: Who| Kind::of(path, "c1", who);
        let everyone = [
            Who::BuilderTest,
            Who::BuilderBuild,
            Who::Reviewer,
            Who::Orchestrator,
        ];
        for who in everyone {
            assert_eq!(of(log::PATH, who), Kind::Log, "{who}");
            assert_eq!(of(invoke::SESSIONS, who), Kind::Log, "{who}");
            assert_eq!(of(&change::record_path("c1"), who), Kind::Record, "{who}");
            for kept in [
                approve::G0_PATH,
                ".gdd/changes/c2/change.json",
                ".gdd/changes/c1/package.md",
                ".gdd/changes/c1/g3.json",
                ".gdd/changes/c1/outputs/x.md",
            ] {
                assert_eq!(of(kept, who), Kind::Kept, "{who}: {kept}");
            }
        }
        let own = |file: &str, who: Who| of(&format!(".gdd/changes/c1/{file}"), who);
        assert_eq!(own("g2.json", Who::BuilderTest), Kind::Report);
        assert_eq!(own("g2.json", Who::BuilderBuild), Kind::Report);
        assert_eq!(own("review.md", Who::Reviewer), Kind::Answer);
        assert_eq!(own("replan.md", Who::Orchestrator), Kind::Answer);
        for (who, file) in [
            (Who::Reviewer, "g2.json"),
            (Who::Orchestrator, "g2.json"),
            (Who::BuilderBuild, "review.md"),
            (Who::Reviewer, "replan.md"),
        ] {
            assert_eq!(own(file, who), Kind::Kept, "{who}");
        }
        let output = |name: &str, who: Who| own(&format!("outputs/{name}"), who);
        let build = Who::BuilderBuild;
        assert_eq!(output("builder-build-2.md", build), Kind::Output);
        assert_eq!(output("builder-test-1.raw", Who::BuilderTest), Kind::Output);
        assert_eq!(output("reviewer-1.md", Who::Reviewer), Kind::Output);
        assert_eq!(output("orchestrator-1.md", Who::Orchestrator), Kind::Output);
        assert_eq!(output("builder-test-1.md", build), Kind::Kept);
        assert_eq!(output("reviewer-1/x.md", Who::Reviewer), Kind::Kept);
        assert_eq!(of(".gdd/changes/c10/g2.json", build), Kind::Kept);
        // Only what gdd keeps of a call: its number, then `.md` or `.raw`.
        assert_eq!(output("builder-build-12.raw", build), Kind::Output);
        for other in [
            "builder-build-1.session",
            "builder-build-1.json",
            "builder-build-1.md.session",
            "builder-build-.md",
            "builder-build-x.md",
            "builder-build-1a.raw",
            "builder-build-1",
        ] {
            assert_eq!(output(other, build), Kind::Kept, "{other}");
        }
        assert_eq!(output("reviewer-1.session", Who::Reviewer), Kind::Kept);
    }

    /// The text of `path` of the project at `folder`, read as a file of its own.
    fn own(folder: &Folder, path: &str) -> String {
        let bytes = files::read(&folder.0, path).unwrap().unwrap_or_default();
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn what_may_change_in_each_file_of_gdd_state() {
        let record = br#"{"id": "c1", "inbox": [], "state": "building"}"#.as_slice();
        let noted = br#"{"state": "building", "id": "c1", "inbox": [1], "out_of_scope": ["x"]}"#;
        assert!(allowed(Kind::Record, Some(record), Some(noted)));
        let other = br#"{"id": "c1", "inbox": [], "state": "closed"}"#.as_slice();
        assert!(!allowed(Kind::Record, Some(record), Some(other)));
        assert!(!allowed(Kind::Record, Some(record), Some(b"x")));
        assert!(!allowed(Kind::Record, Some(record), None));
        assert!(!allowed(Kind::Record, None, Some(record)));
        assert!(allowed(Kind::Record, Some(b"x"), Some(b"x")));
        assert!(!allowed(Kind::Record, Some(b"x"), Some(b"y")));
        assert!(allowed(Kind::Record, None, None));
        assert!(allowed(Kind::Kept, Some(b"a"), Some(b"a")));
        assert!(!allowed(Kind::Kept, Some(b"a"), Some(b"a ")));
        assert!(!allowed(Kind::Kept, None, Some(b"a")));
        assert!(!allowed(Kind::Kept, Some(b"a"), None));
        assert!(allowed(Kind::Kept, None, None));
        assert!(allowed(Kind::Log, Some(b"a\n"), Some(b"a\nb\n")));
        assert!(allowed(Kind::Log, None, Some(b"a\n")));
        assert!(allowed(Kind::Log, Some(b""), None));
        assert!(!allowed(Kind::Log, Some(b"a\n"), Some(b"b\na\n")));
        assert!(!allowed(Kind::Log, Some(b"a\n"), None));
    }

    #[test]
    fn what_a_call_writes_may_change_only_as_gdd_writes_it() {
        assert!(allowed(Kind::Output, None, Some(b"a")));
        assert!(allowed(Kind::Output, Some(b"a"), Some(b"a")));
        assert!(!allowed(Kind::Output, Some(b"a"), Some(b"b")));
        assert!(!allowed(Kind::Output, Some(b"a"), None));
        let rewritten = [
            (None, Some(b"a")),
            (Some(b"a"), Some(b"b")),
            (Some(b"a"), None),
        ];
        for (before, now) in rewritten {
            let (before, now) = (before.map(|b| &b[..]), now.map(|n| &n[..]));
            assert!(allowed(Kind::Answer, before, now));
        }
        let report = G2_REPORT.as_bytes();
        assert!(allowed(Kind::Report, Some(b"x"), Some(report)));
        assert!(allowed(Kind::Report, None, Some(report)));
        assert!(allowed(Kind::Report, Some(b"x"), Some(b"x")));
        assert!(allowed(Kind::Report, None, None));
        let wrong: [&[u8]; 4] = [br#"{"gate": "G3"}"#, br#"{"ok": true}"#, b"[1]", b"x"];
        for now in wrong {
            assert!(!allowed(Kind::Report, Some(report), Some(now)), "{now:?}");
        }
        assert!(!allowed(Kind::Report, Some(report), None));
    }

    #[test]
    fn a_reviewer_that_changes_the_working_tree_parks_the_change() {
        let folder = with_state();
        let ended = acting(&folder, Who::Reviewer, |root| {
            change::write(root, "src/new.rs", "x\n").unwrap();
            change::write(root, "README.md", "changed\n").unwrap();
        });
        let why = "the Reviewer changed README.md, src/new.rs: Reviewers only read";
        assert_eq!(ended, park(why));
        let folder = with_state();
        let ended = acting(&folder, Who::Reviewer, |root| {
            fs::remove_file(root.join("README.md")).unwrap();
        });
        let why = "the Reviewer changed README.md: Reviewers only read";
        assert_eq!(ended, park(why));
        // Work in the tree before the review counts as it was then.
        let folder = with_state();
        folder.write("src/app.rs", "builder\n");
        let ended = acting(&folder, Who::Reviewer, |root| {
            change::write(root, "src/app.rs", "reviewer\n").unwrap();
        });
        let why = "the Reviewer changed src/app.rs: Reviewers only read";
        assert_eq!(ended, park(why));
        let folder = with_state();
        folder.write("src/app.rs", "builder\n");
        let ended = acting(&folder, Who::Reviewer, |root| {
            change::write(root, ".gdd/elsewhere/notes.md", "read it all\n").unwrap();
        });
        assert_eq!(ended, Ok(Ended::Closed));
    }

    #[test]
    fn only_the_reviewer_must_leave_the_working_tree_as_it_was() {
        for who in [Who::BuilderTest, Who::BuilderBuild] {
            let folder = with_state();
            let ended = acting(&folder, who, |root| {
                change::write(root, "src/new.rs", "x\n").unwrap();
            });
            assert_eq!(ended, Ok(Ended::Closed), "{who}");
        }
        fn tree(pairs: &[(&str, &str)]) -> Tree {
            let owned = |(path, what): &(&str, &str)| (path.to_string(), what.to_string());
            pairs.iter().map(owned).collect()
        }
        let before = tree(&[("a", "1"), ("b", "2")]);
        assert!(differ(&before, &tree(&[("b", "2"), ("a", "1")])).is_empty());
        let after = tree(&[("a", "3"), ("c", "4")]);
        assert_eq!(differ(&before, &after), ["a", "b", "c"]);
    }

    #[test]
    fn a_reviewer_may_not_change_what_a_path_is_its_permissions_or_its_link() {
        let why = |path: &str| format!("the Reviewer changed {path}: Reviewers only read");
        // A link to a copy with the same bytes is not the file.
        let folder = with_state();
        let ended = acting(&folder, Who::Reviewer, |root| {
            fs::create_dir_all(root.join(".gdd/elsewhere")).unwrap();
            fs::rename(root.join("README.md"), root.join(".gdd/elsewhere/readme")).unwrap();
            symlink(root.join(".gdd/elsewhere/readme"), root.join("README.md")).unwrap();
        });
        assert_eq!(ended, park(&why("README.md")));
        // Nor is a script that can no longer run.
        let folder = with_state();
        folder.write("run.sh", "#!/bin/sh\n");
        let runnable = fs::Permissions::from_mode(0o755);
        fs::set_permissions(folder.0.join("run.sh"), runnable).unwrap();
        let ended = acting(&folder, Who::Reviewer, |root| {
            let mode = fs::Permissions::from_mode(0o644);
            fs::set_permissions(root.join("run.sh"), mode).unwrap();
        });
        assert_eq!(ended, park(&why("run.sh")));
        // Nor a link to another file with the same bytes.
        let folder = with_state();
        symlink("README.md", folder.0.join("link")).unwrap();
        let ended = acting(&folder, Who::Reviewer, |root| {
            fs::create_dir_all(root.join(".gdd/elsewhere")).unwrap();
            fs::copy(root.join("README.md"), root.join(".gdd/elsewhere/readme")).unwrap();
            fs::remove_file(root.join("link")).unwrap();
            symlink(".gdd/elsewhere/readme", root.join("link")).unwrap();
        });
        assert_eq!(ended, park(&why("link")));
        assert_eq!(entry(&folder.0, "nothing"), guard::DELETED);
        assert_eq!(entry(&folder.0, "README.md/inside"), guard::DELETED);
        let unreadable = entry(&folder.0, &"x".repeat(300));
        assert!(unreadable.starts_with("unreadable: "), "{unreadable}");
    }

    const RECORD: &str = r#"{"id": "c1", "title": "App", "state": "in-pre", "inbox": [],
        "decision": {"tests_first": true, "resume_green": true, "reviewer": true, "g4": true,
                     "retries": 2, "builder": {"model": "b"}, "builder_test": {"model": "t"},
                     "reviewer_model": {"model": "r"}, "orchestrator": {"model": "o"},
                     "type": "Feature"}}"#;

    fn record(root: &Path) -> Value {
        let text = change::read_record(root, "c1").unwrap();
        serde_json::from_str(&text).unwrap()
    }

    fn inbox_of(root: &Path) -> String {
        fs::read_to_string(root.join(".gdd/changes/c1/inbox.md")).unwrap()
    }

    /// What the recipe said when it went as far as it could.
    fn done(said: Result<Said, String>) -> String {
        match said {
            Ok(Said::Done(text)) => text,
            other => panic!("not done: {other:?}"),
        }
    }

    /// What the recipe said when it parked the change.
    fn parked(said: Result<Said, String>) -> String {
        match said {
            Ok(Said::Parked(text)) => text,
            other => panic!("not parked: {other:?}"),
        }
    }

    const CLOSED: &str = "Change c1 closed. Message: .gdd/changes/c1/inbox.md · Tokens: gdd \
                          report --change c1";

    #[test]
    fn the_end_of_a_run_says_what_happened_and_what_to_do() {
        let repo = repo_with_change(RECORD);
        let (root, here) = (repo.root(), repo.project());
        let said = finish(&here, "c1", Ok(Ended::Closed));
        assert_eq!(said, Ok(Said::Done(CLOSED.into())));
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(false));
        let paused = "Paused at G4. Message: .gdd/changes/c1/inbox.md · To continue: gdd resume \
                      c1 --g4 ok|ko|polish";
        let said = finish(&here, "c1", Ok(Ended::Paused));
        assert_eq!(said, Ok(Said::Done(paused.into())));
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(true));
        let request = &record(root)["inbox"][0];
        assert_eq!(request["kind"], "needs");
        assert_eq!(request["text"], g4_request("c1", "Feature"));
        let inbox = inbox_of(root);
        assert!(inbox.contains("G4 · Release Gate"), "{inbox}");
        for answer in ["--g4 ok", "--g4 ko", "--g4 polish --note"] {
            assert!(inbox.contains(answer), "{answer}: {inbox}");
        }
        let said = finish(&here, "c1", park(G3_RED));
        let message = "Change c1 parked: G3 still red after one fix. Message in \
                       .gdd/changes/c1/inbox.md";
        assert_eq!(said, Ok(Said::Parked(message.into())));
        assert_eq!(record(root)["state"], "parked");
        let failed = finish(&here, "c1", Err(Stop::Fail("boom".into())));
        assert_eq!(failed, Err("boom".into()));
    }

    #[test]
    fn a_change_that_ends_says_so_even_when_its_worktree_stays() {
        let repo = repo_with_change(RECORD);
        fs::create_dir_all(repo.work.join("mine")).unwrap();
        let err = finish(&repo.project(), "c1", Ok(Ended::Closed)).unwrap_err();
        let stays = "\nIts worktree stays: git worktree remove --force -- ";
        assert!(err.starts_with(&format!("{CLOSED}{stays}")), "{err}");
        assert!(repo.work.join("mine").is_dir());
    }

    /// A repository with change `c1` (see [`repo_with_change`]), from its main checkout, and
    /// where the worktree of the change goes.
    struct Checkout {
        repo: Repo,
        work: PathBuf,
        _trees: Folder,
    }

    impl Checkout {
        /// What the recipe reads of it: one replan, and two changes building at once.
        fn project(&self) -> Project<'_> {
            Project {
                root: self.repo.root(),
                state: self.repo.root(),
                worktree: &self.work,
                work: &self.work,
                main: "main",
                replans: 1,
                max: 2,
                spike_usd: 5.0,
            }
        }
    }

    impl std::ops::Deref for Checkout {
        type Target = Repo;

        fn deref(&self) -> &Repo {
            &self.repo
        }
    }

    /// A repository with an author, that ignores `.gdd/`, with change `c1` and its branch, not
    /// checked out, and a folder for the worktree of the change.
    fn repo_with_change(record: &str) -> Checkout {
        let repo = Repo::new();
        repo.identify();
        repo.write(".gitignore", ".gdd/\n");
        repo.commit();
        repo.git(&["branch", "change/c1"]);
        repo.write(".gdd/changes/c1/change.json", record);
        let trees = Folder::new();
        Checkout {
            repo,
            work: trees.0.join("c1"),
            _trees: trees,
        }
    }

    /// The branch the worktree of `repo` has checked out; an error when there is none.
    fn worktree_branch(repo: &Checkout) -> Result<String, git::Error> {
        Git::new(&repo.work).current_branch()
    }

    #[test]
    fn launch_runs_the_recipe_in_the_worktree_of_the_change() {
        let repo = repo_with_change(RECORD);
        let root = repo.root();
        state::pause_at_g4(root, "c1").unwrap();
        let mut steps = played(root);
        steps.decided.reviewer = false;
        let said = launch(&repo.project(), "c1", &mut steps);
        assert!(done(said).starts_with("Change c1 closed."));
        assert_eq!(repo.tool().current_branch().unwrap(), "main");
        assert!(!repo.work.exists(), "the worktree stays");
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(false));
        assert_eq!(steps.ran[..2], ["prepare", "test t mode=test"]);
        let mut steps = played(root);
        steps.decided.reviewer = false;
        steps.decided.g4 = true;
        let said = launch(&repo.project(), "c1", &mut steps);
        assert!(done(said).starts_with("Paused at G4."));
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(true));
        assert_eq!(worktree_branch(&repo).unwrap(), "change/c1");
        assert_eq!(repo.tool().current_branch().unwrap(), "main");
    }

    #[test]
    fn launch_refuses_uncommitted_work_outside_the_branch_of_the_change() {
        let repo = repo_with_change(RECORD);
        let root = repo.root();
        repo.write("README.md", "changed\n");
        let mut steps = played(root);
        let refused = launch(&repo.project(), "c1", &mut steps);
        assert_eq!(refused, Err(UNCOMMITTED.to_string()));
        assert!(steps.ran.is_empty());
        assert_eq!(repo.tool().current_branch().unwrap(), "main");
        assert!(worktree_branch(&repo).is_err());
        let missing = launch(&repo.project(), "c9", &mut steps).unwrap_err();
        let start = "change c9 does not exist";
        assert!(missing.starts_with(start), "{missing}");
    }

    #[test]
    fn the_recipe_runs_only_where_the_state_is_the_project_s_own() {
        let repo = repo_with_change(RECORD);
        let root = repo.root();
        state::pause_at_g4(root, "c1").unwrap();
        let worktree = Folder::new();
        let elsewhere = Project {
            root: &worktree.0,
            ..repo.project()
        };
        let mut steps = played(root);
        let why = |command: &str| {
            format!(
                "run gdd {command} from the main checkout ({}): the worktrees of the changes are \
                 made and removed from there",
                root.display()
            )
        };
        let refused = launch(&elsewhere, "c1", &mut steps);
        assert_eq!(refused, Err(why("launch")));
        let refused = resume(&elsewhere, "c1", Answer::Ok, "", None, &mut steps);
        assert_eq!(refused, Err(why("resume")));
        assert!(steps.ran.is_empty());
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(true));
        assert_eq!(repo.tool().current_branch().unwrap(), "main");
    }

    #[test]
    fn a_relaunch_on_the_branch_keeps_the_earlier_work_as_a_commit() {
        let repo = repo_with_change(RECORD);
        let root = repo.root();
        repo.git(&["checkout", "-q", "change/c1"]);
        repo.write("README.md", "earlier run\n");
        repo.write("src/new.rs", "x\n");
        let mut steps = played(root);
        steps.decided.resume_green = true;
        assert!(launch(&repo.project(), "c1", &mut steps).is_ok());
        let subject = repo.git(&["log", "-1", "--format=%s", "change/c1"]);
        let relaunch = "gdd(c1): work of the earlier run, before relaunch";
        assert_eq!(subject, relaunch);
        assert_eq!(repo.tool().current_branch().unwrap(), "main");
        assert!(!repo.tool().has_changes().unwrap());
        assert_eq!(steps.ran[0], "prepare");
        // Files git does not track yet are work of the earlier run too.
        repo.git(&["checkout", "-q", "change/c1"]);
        repo.write("src/only.rs", "y\n");
        let mut steps = played(root);
        steps.stuck = Some(Who::Reviewer);
        let said = launch(&repo.project(), "c1", &mut steps);
        let why = "Change c1 parked: the Reviewer did not answer within 2 seconds.";
        assert!(parked(said).starts_with(why));
        let committed = repo.git(&["show", "--name-only", "--format=%s", "change/c1"]);
        assert_eq!(committed, format!("{relaunch}\n\nsrc/only.rs"));
        assert_eq!(worktree_branch(&repo).unwrap(), "change/c1");
        assert!(repo.work.join("src/only.rs").is_file());
    }

    #[test]
    fn a_relaunch_on_the_branch_never_commits_the_state() {
        let repo = repo_with_change(RECORD);
        let root = repo.root();
        repo.git(&["checkout", "-q", "change/c1"]);
        // From here on, the project does not ignore the state.
        repo.git(&["rm", "-q", ".gitignore"]);
        let mut steps = played(root);
        steps.stuck = Some(Who::BuilderTest);
        let said = launch(&repo.project(), "c1", &mut steps);
        let why = "Change c1 parked: the Builder in test mode did not answer within 2 seconds.";
        assert!(parked(said).starts_with(why));
        let committed = repo.git(&["show", "--name-only", "--format=", "change/c1"]);
        assert_eq!(committed, ".gitignore");
        assert_eq!(record(root)["state"], "parked");
        assert_eq!(repo.tool().current_branch().unwrap(), "main");
    }

    /// Launches change `c1` of `repo` again, with code that passes G2 already and a Reviewer that
    /// never answers, after the gates judged the work of the earlier run in its worktree with
    /// `earlier` (see [`Steps::guard_earlier`]): what ran.
    fn relaunched(repo: &Checkout, earlier: Option<Vec<String>>) -> Vec<String> {
        let mut steps = played(repo.root());
        steps.decided.resume_green = true;
        steps.stuck = Some(Who::Reviewer);
        steps.earlier = earlier;
        let said = launch(&repo.project(), "c1", &mut steps);
        assert!(parked(said).starts_with("Change c1 parked: "));
        steps.ran
    }

    #[test]
    fn a_relaunch_keeps_the_earlier_work_in_the_worktree_as_a_commit() {
        let repo = repo_with_change(RECORD);
        let root = repo.root();
        worktree::open(root, &repo.work, "c1").unwrap();
        change::write(&repo.work, "tests/steps/a.txt", "test mode\n").unwrap();
        let ran = relaunched(&repo, Some(Vec::new()));
        assert_eq!(ran[..2], ["guard earlier", "prepare"]);
        let committed = repo.git(&["show", "--name-only", "--format=%s", "change/c1"]);
        let subject = "gdd(c1): work of the earlier run, before relaunch";
        assert_eq!(committed, format!("{subject}\n\ntests/steps/a.txt"));
        let work = Git::new(&repo.work);
        assert!(!work.has_changes().unwrap());
        // Without work left there, nothing is judged or committed.
        let head = work.head().unwrap();
        assert_eq!(relaunched(&repo, Some(Vec::new()))[0], "prepare");
        assert_eq!(work.head().unwrap(), head);
    }

    #[test]
    fn a_relaunch_leaves_uncommitted_the_earlier_work_the_gates_did_not_pass() {
        let repo = repo_with_change(RECORD);
        let root = repo.root();
        worktree::open(root, &repo.work, "c1").unwrap();
        let work = Git::new(&repo.work);
        let head = work.head().unwrap();
        change::write(&repo.work, "gdd/commands.toml", "[g2]\nunit = \"true\"\n").unwrap();
        // Path Guard or the scope check failed on it, or test mode never passed its Path Guard.
        let failed = vec!["gdd/commands.toml: protected path for builder-build".to_string()];
        for earlier in [Some(failed), None] {
            let ran = relaunched(&repo, earlier);
            assert_eq!(ran[..2], ["guard earlier", "prepare"]);
            assert_eq!(work.head().unwrap(), head);
            assert!(work.has_changes().unwrap());
        }
    }

    #[test]
    fn a_main_checkout_that_cannot_take_the_merge_parks_the_change() {
        let repo = repo_with_change(RECORD);
        let root = repo.root();
        let mut steps = played(root);
        steps.decided.reviewer = false;
        // Someone edits a tracked file of the main checkout while the change runs.
        let edits: Act = |root| change::write(root, "README.md", "edited\n").unwrap();
        steps.acts = vec![(Who::BuilderBuild, edits)];
        let said = launch(&repo.project(), "c1", &mut steps);
        let why = format!(
            "Change c1 parked: the merge needs the main checkout ({}) without uncommitted \
             changes in tracked files: commit or stash them.",
            root.display()
        );
        assert!(parked(said).starts_with(&why));
        assert_eq!(last(&steps), OPENED);
        assert_eq!(record(root)["state"], "parked");
        assert_eq!(worktree_branch(&repo).unwrap(), "change/c1");
    }

    /// The record of change `c1` planning `src/a.rs`, in state `in-pre`.
    fn planning_a() -> String {
        RECORD.replace("\"title\"", "\"paths\": [\"src/a.rs\"], \"title\"")
    }

    /// The warning of change `c1`, which plans `src/a.rs`, next to change `c2`, parked, which
    /// plans `src/**`.
    const OVERLAPS_C2: &str =
        "c1 overlaps c2 (parked): src/a.rs with src/**; git settles it when c1 merges.";

    #[test]
    fn a_launch_next_to_a_change_it_overlaps_warns_first_and_keeps_the_warning_once() {
        let other = r#""inbox": [{"kind": "decided", "text": "Other"}]"#;
        let repo = repo_with_change(&planning_a().replace(r#""inbox": []"#, other));
        let root = repo.root();
        let parked = r#"{"state": "parked", "paths": ["src/**"]}"#;
        repo.write(".gdd/changes/c2/change.json", parked);
        let line = format!("say {OVERLAPS_C2}");
        let warned = line.as_str();
        let mut steps = played(root);
        let said = launch(&repo.project(), "c1", &mut steps);
        assert!(done(said).starts_with("Change c1 closed."));
        assert_eq!(steps.ran[..3], ["prepare", warned, "test t mode=test"]);
        // Launched again, it warns again; the Decision Inbox G4 shows keeps the warning once.
        let mut steps = played(root);
        steps.decided.g4 = true;
        let said = launch(&repo.project(), "c1", &mut steps);
        assert!(done(said).starts_with("Paused at G4."));
        assert_eq!(steps.ran[..2], ["prepare", warned]);
        let mut decided = record(root)["inbox"].as_array().cloned().unwrap();
        decided.retain(|note| note["kind"] == "decided");
        assert_eq!(decided.len(), 2, "{decided:?}");
        assert_eq!(decided[1]["text"], OVERLAPS_C2);
        let inbox = inbox_of(root);
        let shown = format!("## Decided without you\n- Other\n- {OVERLAPS_C2}\n");
        assert!(inbox.contains(&shown), "{inbox}");
    }

    #[test]
    fn a_launch_past_the_cap_touches_nothing_and_warns_of_nothing() {
        let repo = repo_with_change(&planning_a());
        let root = repo.root();
        let building = r#"{"state": "building", "paths": ["src/**"]}"#;
        repo.write(".gdd/changes/c2/change.json", building);
        repo.git(&["checkout", "-q", "change/c1"]);
        let capped = Project {
            max: 1,
            ..repo.project()
        };
        let mut steps = played(root);
        let refused = launch(&capped, "c1", &mut steps).unwrap_err();
        let cap = "1 change(s) building, the cap is 1: c2\n";
        assert!(refused.starts_with(cap), "{refused}");
        assert!(!refused.contains("overlaps"), "{refused}");
        assert!(steps.ran.is_empty());
        assert_eq!(record(root)["inbox"], serde_json::json!([]));
        assert_eq!(repo.tool().current_branch().unwrap(), "change/c1");
        assert!(worktree_branch(&repo).is_err());
    }

    /// What a Reviewer call does while the main branch moves on: `docs/news.md` committed on it
    /// in the main checkout at `root`.
    fn news_on_main(root: &Path) {
        change::write(root, "docs/news.md", "news\n").unwrap();
        let news = ["docs/news.md".to_string()];
        Git::new(root).commit_files(&news, "news").unwrap();
    }

    /// What runs of a launch of a change with tests first, up to G3 opened.
    const TO_OPENED: [&str; 8] = [
        "prepare",
        "test t mode=test",
        "guard since=false",
        "snapshot Build",
        "build b mode=build",
        "G2",
        "G3",
        "review r round=1",
    ];

    const MOVED: &str = "say main moved under c1: G3 again on top of it";

    /// G3 closed when main moved under the change, before it takes the latest main.
    const UNOPENED: &str =
        "G3 ko machines: main moved under it: G3 again on top of the latest main";

    /// G3 opened again once it passed on top of the latest main.
    const REOPENED: &str = "G3 ok machines: automatic chain on top of the latest main";

    /// What the merge of the played steps prints.
    const MERGED: &str = r#"{"ok":true}"#;

    #[test]
    fn when_main_moved_the_change_takes_it_and_passes_g3_again_before_it_merges() {
        let repo = repo_with_change(RECORD);
        let root = repo.root();
        let mut steps = played(root);
        steps.acts = vec![(Who::Reviewer, news_on_main as Act)];
        let said = launch(&repo.project(), "c1", &mut steps);
        assert!(done(said).starts_with("Change c1 closed."));
        let mut expected = TO_OPENED.to_vec();
        let merged = [OPENED, UNOPENED, MOVED, "G3", REOPENED, "merge", "pre"];
        expected.extend(merged);
        expected.extend(["promote", "observe"]);
        assert_eq!(steps.ran, expected);
        let subject = repo.git(&["log", "-1", "--format=%s", "change/c1"]);
        assert_eq!(subject, "gdd(c1): take the latest main");
        repo.git(&["cat-file", "-e", "change/c1:docs/news.md"]);
        let took_main = repo.git(&["rev-parse", "change/c1"]);
        assert_eq!(record(root)["build_start"], took_main.as_str());
    }

    #[test]
    fn g3_red_on_top_of_the_latest_main_parks_the_change() {
        let repo = repo_with_change(RECORD);
        let root = repo.root();
        let mut steps = played(root);
        steps.acts = vec![(Who::Reviewer, news_on_main as Act)];
        steps.g3 = VecDeque::from([true, false]);
        let said = launch(&repo.project(), "c1", &mut steps);
        let why = "Change c1 parked: G3 red on top of the latest main.";
        assert!(parked(said).starts_with(why));
        // G3 stays closed: the change no longer merges as it passed it.
        assert_eq!(
            steps.ran[TO_OPENED.len()..],
            [OPENED, UNOPENED, MOVED, "G3"]
        );
        assert_eq!(record(root)["state"], "parked");
        assert_eq!(worktree_branch(&repo).unwrap(), "change/c1");
        assert!(repo.work.join("docs/news.md").is_file());
    }

    #[test]
    fn a_conflict_with_the_latest_main_parks_the_change_with_its_paths() {
        let repo = repo_with_change(RECORD);
        let root = repo.root();
        repo.git(&["checkout", "-q", "change/c1"]);
        repo.write("app.txt", "change\n");
        repo.commit();
        let branch = repo.git(&["rev-parse", "HEAD"]);
        repo.git(&["checkout", "-q", "main"]);
        let mut steps = played(root);
        let conflicting: Act = |root| {
            change::write(root, "app.txt", "main\n").unwrap();
            let app = ["app.txt".to_string()];
            Git::new(root).commit_files(&app, "app").unwrap();
        };
        steps.acts = vec![(Who::Reviewer, conflicting)];
        let said = launch(&repo.project(), "c1", &mut steps);
        let why = format!(
            "Change c1 parked: main moved under it and the merge has conflicts in app.txt: merge \
             main into change/c1 in its worktree ({}), settle the conflicts, commit, then gdd \
             launch c1.",
            repo.work.display()
        );
        assert!(parked(said).starts_with(&why));
        assert_eq!(steps.ran[TO_OPENED.len()..], [OPENED, UNOPENED]);
        assert_eq!(record(root)["state"], "parked");
        let work = Git::new(&repo.work);
        assert_eq!(work.head().unwrap(), branch);
        assert!(work.conflicts().unwrap().is_empty());
        assert!(!work.has_changes().unwrap());
        // Settled as the message says, the next launch merges the change on top of main.
        assert!(work.merge("main", "settled").is_err());
        fs::write(repo.work.join("app.txt"), "settled\n").unwrap();
        let tree = repo.work.to_string_lossy().into_owned();
        repo.git(&["-C", &tree, "add", "app.txt"]);
        repo.git(&["-C", &tree, "commit", "-q", "--no-edit"]);
        let mut steps = played(root);
        let said = launch(&repo.project(), "c1", &mut steps);
        assert!(done(said).starts_with("Change c1 closed."));
        let mut expected = TO_OPENED.to_vec();
        expected.extend([OPENED, "merge", "pre", "promote", "observe"]);
        assert_eq!(steps.ran, expected);
    }

    #[test]
    fn a_merge_waits_for_the_merge_of_another_change_and_says_so() {
        let repo = repo_with_change(RECORD);
        let root = repo.root();
        let mut steps = played(root);
        steps.decided.tests_first = false;
        // Another merge takes the lock while the Reviewer runs: its call does not make it state.
        let another: Act = |root| {
            let file = files::open(root, queue::LOCK).unwrap();
            file.lock().unwrap();
            thread::spawn(move || {
                thread::sleep(Duration::from_secs(1));
                drop(file);
            });
        };
        steps.acts = vec![(Who::Reviewer, another)];
        let said = launch(&repo.project(), "c1", &mut steps);
        assert!(done(said).starts_with("Change c1 closed."));
        let waited = format!("say {}", queue::WAITING);
        assert_eq!(steps.ran[5..7], [OPENED, waited.as_str()]);
        assert_eq!(steps.ran[7..], ["merge", "pre", "promote", "observe"]);
    }

    #[test]
    fn every_kind_of_call_holds_the_lock_of_the_agent_calls_and_lets_its_turn_go() {
        let folder = repo_folder();
        let mut steps = played(&folder.0);
        steps.decided.retries = 1;
        steps.g2 = VecDeque::from([false]);
        let at_work: Act = |root| {
            assert!(queue::tests::taken(root, queue::AGENTS), "no one writes");
            assert!(queue::tests::freed(root, queue::TURN), "a person may wait");
        };
        let everyone = [
            Who::BuilderTest,
            Who::BuilderBuild,
            Who::Reviewer,
            Who::Orchestrator,
        ];
        steps.acts = everyone.map(|who| (who, at_work)).to_vec();
        assert_eq!(launched(&mut steps, 1), Ok(Ended::Closed));
        for who in everyone {
            let called = steps.ran.iter().any(|step| step.starts_with(short(who)));
            assert!(called, "{who}: {:?}", steps.ran);
        }
        assert!(queue::tests::freed(&folder.0, queue::AGENTS));
    }

    #[test]
    fn a_person_who_writes_shared_state_during_a_call_writes_before_the_next_one_starts() {
        let folder = repo_folder();
        let mut steps = played(&folder.0);
        // During test mode a person opens G0, and waits for the call to end.
        let person: Act = |root| {
            let (state, (waiting, wait_waiting)) = (root.to_path_buf(), mpsc::channel());
            thread::spawn(move || {
                let write = queue::shared_write(&state, || waiting.send(()).unwrap()).unwrap();
                change::write(&state, approve::G0_PATH, "{}\n").unwrap();
                thread::sleep(Duration::from_millis(100));
                drop(write);
            });
            wait_waiting.recv().unwrap();
            assert!(!root.join(approve::G0_PATH).exists());
        };
        let next: Act = |root| assert!(root.join(approve::G0_PATH).is_file(), "written");
        steps.acts = vec![(Who::BuilderTest, person), (Who::BuilderBuild, next)];
        assert_eq!(launched(&mut steps, 1), Ok(Ended::Closed));
        assert_eq!(text(&folder, approve::G0_PATH), "{}\n");
    }

    #[test]
    fn a_launch_is_judged_once_the_launch_before_it_made_its_change_building() {
        let repo = repo_with_change(&planning_a());
        let root = repo.root();
        let c2 = ".gdd/changes/c2/change.json";
        repo.write(c2, r#"{"state": "open"}"#);
        // Another launch holds the launch lock, and its change builds before it lets go.
        let held = queue::admission(root).unwrap();
        let state = root.to_path_buf();
        let other = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            change::write(&state, c2, r#"{"state": "building"}"#).unwrap();
            drop(held);
        });
        let capped = Project {
            max: 1,
            ..repo.project()
        };
        let mut steps = played(root);
        let refused = launch(&capped, "c1", &mut steps).unwrap_err();
        other.join().unwrap();
        let cap = "1 change(s) building, the cap is 1: c2\n";
        assert!(refused.starts_with(cap), "{refused}");
        assert!(steps.ran.is_empty());
        assert!(worktree_branch(&repo).is_err());
    }

    /// The record of change `c1` with G3 open.
    fn g3_open() -> String {
        RECORD.replace(
            "\"title\"",
            "\"gates\": {\"G3\": {\"decision\": \"ok\"}}, \"title\"",
        )
    }

    /// Change `c1` of `repo` at its merge, from its worktree.
    fn merging(repo: &Checkout) -> Merging<'_> {
        Merging {
            state: repo.root(),
            work: &repo.work,
            main: "main",
            id: "c1",
        }
    }

    #[test]
    fn gdd_merge_takes_the_latest_main_and_passes_g3_again_before_it_merges() {
        let repo = repo_with_change(&g3_open());
        let root = repo.root();
        worktree::open(root, &repo.work, "c1").unwrap();
        let at = merging(&repo);
        // Main did not move: the change merges as it is.
        let mut steps = played(root);
        assert_eq!(merge(&at, &mut steps), Ok(Said::Done(MERGED.into())));
        assert_eq!(steps.ran, ["merge"]);
        news_on_main(root);
        let mut steps = played(root);
        assert_eq!(merge(&at, &mut steps), Ok(Said::Done(MERGED.into())));
        let ran = unwaited(&steps);
        assert_eq!(ran, [UNOPENED, MOVED, "G3", REOPENED, "merge"]);
        let subject = repo.git(&["log", "-1", "--format=%s", "change/c1"]);
        assert_eq!(subject, "gdd(c1): take the latest main");
        repo.git(&["cat-file", "-e", "change/c1:docs/news.md"]);
        let took_main = repo.git(&["rev-parse", "change/c1"]);
        assert_eq!(record(root)["build_start"], took_main.as_str());
    }

    #[test]
    fn gdd_merge_parks_a_change_whose_g3_is_red_on_top_of_the_latest_main() {
        let repo = repo_with_change(&g3_open());
        let root = repo.root();
        worktree::open(root, &repo.work, "c1").unwrap();
        news_on_main(root);
        let mut steps = played(root);
        steps.g3 = VecDeque::from([false]);
        let said = merge(&merging(&repo), &mut steps);
        let why = "Change c1 parked: G3 red on top of the latest main.";
        assert!(parked(said).starts_with(why));
        assert_eq!(steps.ran, [UNOPENED, MOVED, "G3"]);
        assert_eq!(record(root)["state"], "parked");
        assert!(repo.work.join("docs/news.md").is_file());
    }

    #[test]
    fn gdd_merge_refuses_a_change_that_may_not_merge_before_anything_is_touched() {
        let repo = repo_with_change(RECORD);
        let root = repo.root();
        worktree::open(root, &repo.work, "c1").unwrap();
        news_on_main(root);
        let head = Git::new(&repo.work).head().unwrap();
        let at = merging(&repo);
        let mut steps = played(root);
        assert_eq!(merge(&at, &mut steps).unwrap_err(), "G3 is not open.");
        repo.write(".gdd/changes/c1/change.json", &g3_open());
        let elsewhere = Merging { work: root, ..at };
        let refused = merge(&elsewhere, &mut steps).unwrap_err();
        let off = "Not on branch change/c1: ";
        assert!(refused.starts_with(off), "{refused}");
        assert!(unwaited(&steps).is_empty());
        assert_eq!(Git::new(&repo.work).head().unwrap(), head);
        // A step that fails stops it, and says why.
        steps.fails = vec![("G3", "cannot record G3")];
        assert_eq!(merge(&at, &mut steps).unwrap_err(), "cannot record G3");
        assert_eq!(unwaited(&steps), [UNOPENED]);
        assert_eq!(Git::new(&repo.work).head().unwrap(), head);
        assert_eq!(record(root)["state"], "in-pre");
    }

    #[test]
    fn an_agent_that_leaves_a_state_folder_in_its_worktree_parks_the_change() {
        let repo = repo_with_change(RECORD);
        // The agents act in the worktree.
        let mut steps = played(&repo.work);
        steps.decided.reviewer = false;
        let writes: Act = |work| change::write(work, ".gdd/changes/c1/change.json", "{}").unwrap();
        steps.acts = vec![(Who::BuilderBuild, writes)];
        let said = launch(&repo.project(), "c1", &mut steps);
        let left = repo.work.join(".gdd");
        let why = format!(
            "Change c1 parked: the Builder in build mode changed GDD state: {}.",
            left.display()
        );
        assert!(parked(said).starts_with(&why));
        assert!(fs::symlink_metadata(&left).is_err());
        assert_eq!(record(repo.root())["state"], "parked");
        assert_eq!(worktree_branch(&repo).unwrap(), "change/c1");
    }

    #[test]
    fn a_project_in_a_folder_of_its_repository_runs_in_that_folder_of_its_worktree() {
        let repo = Repo::new();
        repo.identify();
        repo.write("app/.gitignore", ".gdd/\n");
        repo.commit();
        repo.git(&["branch", "change/c1"]);
        let root = repo.root().join("app");
        change::write(&root, ".gdd/changes/c1/change.json", RECORD).unwrap();
        let trees = Folder::new();
        let worktree = trees.0.join("c1");
        let work = worktree.join("app");
        let project = Project {
            root: &root,
            state: &root,
            worktree: &worktree,
            work: &work,
            main: "main",
            replans: 1,
            max: 2,
            spike_usd: 5.0,
        };
        // The agents act in the project's folder of the worktree.
        let mut steps = played(&work);
        steps.decided.reviewer = false;
        let writes: Act = |work| change::write(work, ".gdd/changes/c1/change.json", "{}").unwrap();
        steps.acts = vec![(Who::BuilderBuild, writes)];
        let said = launch(&project, "c1", &mut steps);
        let left = work.join(".gdd");
        let why = format!(
            "Change c1 parked: the Builder in build mode changed GDD state: {}.",
            left.display()
        );
        assert!(parked(said).starts_with(&why));
        assert!(fs::symlink_metadata(&left).is_err());
        assert!(work.join(".gitignore").is_file());
        assert_eq!(Git::new(&worktree).current_branch().unwrap(), "change/c1");
        assert_eq!(record(&root)["state"], "parked");
    }

    #[test]
    fn a_state_folder_left_in_a_worktree_goes_but_never_what_it_links_to() {
        let (work, state) = (Folder::new(), Folder::new());
        assert_eq!(state_left(&work.0, &state.0), Ok(Vec::new()));
        state.write(".gdd/log.jsonl", "{}\n");
        symlink(state.0.join(".gdd"), work.0.join(".gdd")).unwrap();
        let left = work.0.join(".gdd").display().to_string();
        assert_eq!(state_left(&work.0, &state.0), Ok(vec![left]));
        assert!(fs::symlink_metadata(work.0.join(".gdd")).is_err());
        assert_eq!(text(&state, ".gdd/log.jsonl"), "{}\n");
        // The state root keeps its own.
        assert_eq!(state_left(&state.0, &state.0), Ok(Vec::new()));
        assert_eq!(text(&state, ".gdd/log.jsonl"), "{}\n");
        // A worktree that cannot be looked at is an error, not one without a state folder.
        work.write("file", "");
        let under = work.0.join("file");
        let err = state_left(&under, &state.0).unwrap_err();
        let start = format!("cannot look at {}: ", under.join(".gdd").display());
        assert!(err.starts_with(&start), "{err}");
    }

    #[test]
    fn only_a_person_answers_a_change_paused_at_g4() {
        let repo = repo_with_change(RECORD);
        let root = repo.root();
        let here = repo.project();
        let mut steps = played(root);
        let said = resume(&here, "c1", Answer::Ok, "", None, &mut steps);
        assert_eq!(said, Err("Change c1 is not paused at G4.".into()));
        state::pause_at_g4(root, "c1").unwrap();
        let refused = "A Builder or Reviewer cannot open or close gates.";
        for role in ["builder-build", "reviewer"] {
            let said = resume(&here, "c1", Answer::Ok, "", Some(role), &mut steps);
            assert_eq!(said, Err(refused.into()), "{role}");
        }
        let polish = "polish needs your notes: gdd resume c1 --g4 polish --note ...";
        let said = resume(&here, "c1", Answer::Polish, " ", None, &mut steps);
        assert_eq!(said, Err(polish.into()));
        assert!(steps.ran.is_empty());
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(true));
        assert_eq!(record(root)["state"], "in-pre");
        let missing = resume(&here, "c9", Answer::Ok, "", None, &mut steps);
        let start = "change c9 does not exist";
        assert!(missing.unwrap_err().starts_with(start));
    }

    /// The repository with change `c1` paused at G4.
    fn paused() -> Checkout {
        paused_in("in-pre")
    }

    /// The repository with change `c1` in state `state`, paused at G4.
    fn paused_in(state: &str) -> Checkout {
        let repo = repo_with_change(&RECORD.replace("in-pre", state));
        finish(&repo.project(), "c1", Ok(Ended::Paused)).unwrap();
        repo
    }

    #[test]
    fn ok_at_g4_promotes_the_change() {
        let repo = paused();
        let root = repo.root();
        let here = repo.project();
        worktree::open(root, &repo.work, "c1").unwrap();
        let mut steps = played(root);
        let role = Some("orchestrator");
        let said = resume(&here, "c1", Answer::Ok, "Fine", role, &mut steps);
        assert!(done(said).starts_with("Change c1 closed."));
        assert_eq!(steps.ran, ["G4 ok human: Fine", "promote", "observe"]);
        assert!(!repo.work.exists(), "the worktree stays");
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(false));
        let request = &record(root)["inbox"][0];
        assert!(request["resolved"].is_string(), "{request}");
    }

    /// The texts of the requests to the human still open in the record of change `c1`.
    fn open_requests(root: &Path) -> Vec<String> {
        let record = record(root);
        let mut open = Vec::new();
        for note in record["inbox"].as_array().into_iter().flatten() {
            if note["kind"] == "needs" && note["resolved"].is_null() {
                open.extend(note["text"].as_str().map(str::to_string));
            }
        }
        open
    }

    const PROMOTE_FAILED: &str = "release.promote failed (exit code 1):\nregistry down";

    #[test]
    fn a_failed_promotion_leaves_the_change_paused_and_the_same_answer_retries_it() {
        let repo = paused();
        let root = repo.root();
        let here = repo.project();
        let mut steps = played(root);
        steps.fails = vec![("promote", PROMOTE_FAILED)];
        let said = resume(&here, "c1", Answer::Ok, "Fine", None, &mut steps);
        let failed = format!(
            "Promotion of c1 failed: {PROMOTE_FAILED}. Production untouched: the change is \
             still paused at G4. To retry: gdd resume c1 --g4 ok (or --g4 ko to reject)."
        );
        assert_eq!(said, Err(failed));
        assert_eq!(steps.ran, ["G4 ok human: Fine", "promote"]);
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(true));
        let why = format!(
            "Promotion failed: {PROMOTE_FAILED}. Fix the cause and answer again: gdd resume c1 \
             --g4 ok, or --g4 ko."
        );
        let request = g4_request("c1", "Feature");
        assert_eq!(open_requests(root), [why.clone(), request.clone()]);
        let inbox = inbox_of(root);
        assert!(inbox.contains(&why), "{inbox}");
        assert!(inbox.contains("G4 · Release Gate"), "{inbox}");
        // Failed again: only the last failure is open.
        let mut steps = played(root);
        steps.fails = vec![("promote", "release.promote failed")];
        assert!(resume(&here, "c1", Answer::Ok, "Fine", None, &mut steps).is_err());
        let why = "Promotion failed: release.promote failed. Fix the cause and answer again: \
                   gdd resume c1 --g4 ok, or --g4 ko.";
        assert_eq!(open_requests(root), [why.to_string(), request]);
        // Fixed, the same answer promotes it.
        let mut steps = played(root);
        let said = resume(&here, "c1", Answer::Ok, "Fine", None, &mut steps);
        assert!(done(said).starts_with("Change c1 closed."));
        assert_eq!(steps.ran, ["G4 ok human: Fine", "promote", "observe"]);
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(false));
        assert!(open_requests(root).is_empty(), "{:?}", open_requests(root));
    }

    #[test]
    fn after_a_failed_promotion_the_human_can_still_reject_the_change() {
        let repo = paused();
        let root = repo.root();
        let here = repo.project();
        let mut steps = played(root);
        steps.fails = vec![("promote", PROMOTE_FAILED)];
        assert!(resume(&here, "c1", Answer::Ok, "Fine", None, &mut steps).is_err());
        let mut steps = played(root);
        let said = resume(&here, "c1", Answer::Ko, "No", None, &mut steps);
        assert!(done(said).starts_with("Change c1 rejected at G4"));
        assert_eq!(steps.ran, ["G4 ko human: No"]);
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(false));
        assert!(open_requests(root).is_empty(), "{:?}", open_requests(root));
        assert!(!inbox_of(root).contains("Promotion failed"));
    }

    #[test]
    fn after_a_failed_promotion_the_human_can_still_polish_the_change() {
        let repo = paused();
        let root = repo.root();
        let here = repo.project();
        let mut steps = played(root);
        steps.fails = vec![("promote", PROMOTE_FAILED)];
        assert!(resume(&here, "c1", Answer::Ok, "Fine", None, &mut steps).is_err());
        let mut steps = played(root);
        let said = resume(&here, "c1", Answer::Polish, "Shorter", None, &mut steps);
        assert!(done(said).starts_with("Paused at G4."));
        assert_eq!(steps.ran[0], "build b mode=build fix=polish");
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(true));
        assert_eq!(open_requests(root), [g4_request("c1", "Feature")]);
        assert!(!inbox_of(root).contains("Promotion failed"));
    }

    #[test]
    fn a_change_promoted_already_is_only_observed() {
        let repo = paused_in(IN_PRODUCTION);
        let root = repo.root();
        let mut steps = played(root);
        let said = resume(&repo.project(), "c1", Answer::Ok, "Fine", None, &mut steps);
        assert!(done(said).starts_with("Change c1 closed."));
        assert_eq!(steps.ran, ["G4 ok human: Fine", "observe"]);
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(false));
    }

    #[test]
    fn a_failed_observation_after_g4_does_not_leave_the_change_paused() {
        let repo = paused();
        let root = repo.root();
        let mut steps = played(root);
        steps.fails = vec![("observe", "release.observe failed")];
        let said = resume(&repo.project(), "c1", Answer::Ok, "Fine", None, &mut steps);
        assert_eq!(said, Err("release.observe failed".into()));
        assert_eq!(steps.ran, ["G4 ok human: Fine", "promote", "observe"]);
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(false));
    }

    /// What the release step `promote` does before it runs its command: the record of change
    /// `c1` of the project at `root` says it is promoting.
    fn promoting(root: &Path) {
        let text = change::read_record(root, "c1").unwrap();
        let fields = vec![("state", Value::from(release::PROMOTING))];
        let promoting = change::with_fields(&text, fields).unwrap();
        change::write(root, &change::record_path("c1"), &promoting).unwrap();
    }

    const UNLOGGED: &str = "cannot write .gdd/log.jsonl: Is a directory (os error 21)";

    #[test]
    fn a_promotion_that_may_have_reached_production_parks_the_change() {
        let repo = paused();
        let root = repo.root();
        let here = repo.project();
        let mut steps = played(root);
        steps.released = vec![("promote", promoting)];
        steps.fails = vec![("promote", UNLOGGED)];
        let said = resume(&here, "c1", Answer::Ok, "Fine", None, &mut steps);
        let why = format!(
            "Change c1 parked: production may have it, but gdd cannot go on with it \
             ({UNLOGGED}). Check production before anything else: promoting it again could \
             deploy it twice. Message in .gdd/changes/c1/inbox.md"
        );
        assert_eq!(parked(said), why);
        assert_eq!(steps.ran, ["G4 ok human: Fine", "promote"]);
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(false));
        assert_eq!(record(root)["state"], "parked");
        assert!(open_requests(root).is_empty(), "{:?}", open_requests(root));
        // No answer promotes it again.
        let mut steps = played(root);
        let said = resume(&here, "c1", Answer::Ok, "Fine", None, &mut steps);
        assert_eq!(said, Err("Change c1 is not paused at G4.".into()));
        assert!(steps.ran.is_empty());
    }

    #[test]
    fn a_change_left_promoting_parks_whatever_the_answer() {
        for (answer, note) in [(Answer::Ok, ""), (Answer::Ko, "No"), (Answer::Polish, "x")] {
            let repo = paused_in(release::PROMOTING);
            let root = repo.root();
            let mut steps = played(root);
            let said = resume(&repo.project(), "c1", answer, note, None, &mut steps);
            let why = "Change c1 parked: production may have it, but gdd cannot go on with it \
                       (an earlier answer stopped while promoting it). Check production";
            assert!(parked(said).starts_with(why), "{answer:?}");
            assert!(steps.ran.is_empty(), "{answer:?}: {:?}", steps.ran);
            assert_eq!(state::paused_at_g4(root, "c1"), Ok(false));
            assert!(open_requests(root).is_empty(), "{:?}", open_requests(root));
        }
    }

    #[test]
    fn a_change_that_cannot_park_says_why_and_is_no_longer_paused() {
        let repo = paused_in(release::PROMOTING);
        let root = repo.root();
        fs::create_dir_all(root.join(log::PATH)).unwrap();
        let mut steps = played(root);
        let err = resume(&repo.project(), "c1", Answer::Ok, "", None, &mut steps).unwrap_err();
        let start = "Change c1 cannot park: production may have it, but gdd cannot go on with it";
        assert!(err.starts_with(start), "{err}");
        assert!(err.ends_with(&format!("twice.\n{UNLOGGED}")), "{err}");
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(false));
    }

    #[test]
    fn a_change_promoted_already_can_no_longer_be_rejected_or_polished() {
        let repo = paused_in(IN_PRODUCTION);
        let root = repo.root();
        let refused = "Change c1 was promoted already: production has it, so it can no longer \
                       be rejected or polished. To observe it: gdd resume c1 --g4 ok.";
        for (answer, note) in [(Answer::Ko, "No"), (Answer::Polish, "x")] {
            let mut steps = played(root);
            let said = resume(&repo.project(), "c1", answer, note, None, &mut steps);
            assert_eq!(said, Err(refused.into()), "{answer:?}");
            assert!(steps.ran.is_empty(), "{answer:?}: {:?}", steps.ran);
        }
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(true));
        assert_eq!(record(root)["state"], IN_PRODUCTION);
        assert_eq!(open_requests(root), [g4_request("c1", "Feature")]);
        assert_eq!(repo.tool().current_branch().unwrap(), "main");
    }

    #[test]
    fn a_change_promoted_already_whose_ok_cannot_be_recorded_parks() {
        let repo = paused_in(IN_PRODUCTION);
        let root = repo.root();
        let mut steps = played(root);
        let unwritten = "cannot write .gdd/changes/c1/change.json: Is a directory (os error 21)";
        steps.fails = vec![("G4", unwritten)];
        let said = resume(&repo.project(), "c1", Answer::Ok, "Fine", None, &mut steps);
        let why = format!(
            "Change c1 parked: production may have it, but gdd cannot go on with it \
             ({unwritten})."
        );
        assert!(parked(said).starts_with(&why));
        assert_eq!(steps.ran, ["G4 ok human: Fine"]);
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(false));
    }

    #[test]
    fn a_failed_promotion_whose_record_cannot_be_read_keeps_the_pause() {
        let repo = paused();
        let root = repo.root();
        let mut steps = played(root);
        let unreadable: Act = |root| {
            let record = root.join(change::record_path("c1"));
            fs::remove_file(&record).unwrap();
            fs::create_dir(&record).unwrap();
        };
        steps.released = vec![("promote", unreadable)];
        steps.fails = vec![("promote", "release.promote failed")];
        let said = resume(&repo.project(), "c1", Answer::Ok, "Fine", None, &mut steps);
        let start = "Promotion of c1 failed: release.promote failed. Its record cannot say \
                     whether production has it: cannot read .gdd/changes/c1/change.json: ";
        let err = said.unwrap_err();
        assert!(err.starts_with(start), "{err}");
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(true));
    }

    #[test]
    fn ok_at_g4_keeps_the_pause_when_the_record_cannot_be_written() {
        let repo = paused();
        let root = repo.root();
        let here = repo.project();
        // The decision of G4 is not written: nothing is promoted.
        let mut steps = played(root);
        let unwritten = "cannot write .gdd/changes/c1/change.json: Is a directory (os error 21)";
        steps.fails = vec![("G4", unwritten)];
        let said = resume(&here, "c1", Answer::Ok, "Fine", None, &mut steps);
        let start = format!("Promotion of c1 failed: {unwritten}. Production untouched");
        let err = said.unwrap_err();
        assert!(err.starts_with(&start), "{err}");
        assert_eq!(steps.ran, ["G4 ok human: Fine"]);
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(true));
        assert_eq!(open_requests(root)[1], g4_request("c1", "Feature"));
        // The note on the failure cannot be left: the error says both.
        let inbox = root.join(change::inbox_path("c1"));
        fs::remove_file(&inbox).unwrap();
        fs::create_dir(&inbox).unwrap();
        let mut steps = played(root);
        steps.fails = vec![("promote", "release.promote failed")];
        let err = resume(&here, "c1", Answer::Ok, "Fine", None, &mut steps).unwrap_err();
        let start = "Promotion of c1 failed: release.promote failed. Production untouched";
        assert!(err.starts_with(start), "{err}");
        let unsaved = "\ncannot write .gdd/changes/c1/inbox.md: ";
        assert!(err.contains(unsaved), "{err}");
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(true));
    }

    #[test]
    fn ko_at_g4_rejects_the_change_and_production_stays_as_it_is() {
        let repo = paused();
        let root = repo.root();
        let here = repo.project();
        worktree::open(root, &repo.work, "c1").unwrap();
        let mut steps = played(root);
        let said = resume(&here, "c1", Answer::Ko, "No", None, &mut steps);
        let rejected = "Change c1 rejected at G4: production untouched.";
        assert_eq!(said, Ok(Said::Done(rejected.into())));
        assert!(!repo.work.exists(), "the worktree stays");
        assert_eq!(steps.ran, ["G4 ko human: No"]);
        assert_eq!(record(root)["state"], "rejected");
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(false));
        let inbox = inbox_of(root);
        assert!(inbox.contains("state rejected"), "{inbox}");
        assert!(!inbox.contains("Release Gate"), "{inbox}");
    }

    #[test]
    fn polish_runs_one_more_round_with_the_notes_and_pauses_again() {
        let repo = paused();
        let root = repo.root();
        // The branch has work main lacks: it merges.
        repo.git(&["checkout", "-q", "change/c1"]);
        repo.write("src/app.rs", "x\n");
        repo.commit();
        let start = repo.git(&["rev-parse", "HEAD"]);
        repo.git(&["checkout", "-q", "main"]);
        // What main got since is not the pass's: its build starts on the branch, then again
        // where the branch takes the latest main.
        repo.write("gdd/commands.toml", "[release]\n");
        repo.commit();
        let here = repo.project();
        let mut steps = played(root);
        let note = "Shorter error messages";
        let said = resume(&here, "c1", Answer::Polish, note, None, &mut steps);
        assert!(done(said).starts_with("Paused at G4."));
        let expected = [
            "build b mode=build fix=polish",
            "G2",
            "G3",
            "review r round=1",
            OPENED,
            UNOPENED,
            MOVED,
            "G3",
            REOPENED,
            "merge",
            "pre",
        ];
        assert_eq!(steps.ran, expected);
        assert_eq!(repo.tool().current_branch().unwrap(), "main");
        assert_eq!(worktree_branch(&repo).unwrap(), "change/c1");
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(true));
        let record = record(root);
        let notes = record["inbox"].as_array().unwrap();
        let kinds: Vec<&str> = notes.iter().filter_map(|n| n["kind"].as_str()).collect();
        assert_eq!(kinds, ["needs", "polish", "needs"]);
        assert_eq!(notes[1]["text"], note);
        assert!(notes[0]["resolved"].is_string());
        assert!(notes[2].get("resolved").is_none());
        assert_eq!(record["state"], "building");
        let took_main = repo.git(&["rev-parse", "change/c1"]);
        assert_eq!(record["build_start"], took_main.as_str());
        assert_eq!(repo.git(&["rev-parse", "change/c1^1"]), start);
        assert!(inbox_of(root).contains(&format!("## Polish\n- {note}")));
        let log = fs::read_to_string(root.join(log::PATH)).unwrap();
        let lines = log::parse(&log).unwrap();
        let g4 = lines.iter().find(|l| l.text("gate") == "G4").unwrap();
        for (field, value) in [
            ("change", "c1"),
            ("phase", "4-release"),
            ("role", "human"),
            ("result", "polish"),
            ("note", note),
        ] {
            assert_eq!(g4.text(field), value, "{field}");
        }
    }

    #[test]
    fn a_polish_pass_with_nothing_new_for_main_skips_the_merge() {
        let repo = paused();
        let root = repo.root();
        let here = repo.project();
        let mut steps = played(root);
        steps.g2 = VecDeque::from([false, false]);
        let said = resume(&here, "c1", Answer::Polish, "x", None, &mut steps);
        assert!(done(said).starts_with("Paused at G4."));
        let expected = [
            "build b mode=build fix=polish",
            "G2",
            "build b mode=build",
            "G2",
            "replan o reason=g2",
            "build b mode=build after=replan",
            "G2",
            "G3",
            "review r round=1",
            OPENED,
            "pre",
        ];
        assert_eq!(steps.ran, expected);
        assert_eq!(repo.tool().current_branch().unwrap(), "main");
    }

    #[test]
    fn a_polish_pass_that_cannot_reach_its_branch_leaves_the_change_paused() {
        let repo = paused();
        let root = repo.root();
        repo.git(&["branch", "-D", "change/c1"]);
        let here = repo.project();
        let mut steps = played(root);
        let said = resume(&here, "c1", Answer::Polish, "x", None, &mut steps);
        let err = said.unwrap_err();
        assert!(err.starts_with("git worktree add -q -- "), "{err}");
        assert!(steps.ran.is_empty());
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(true));
        assert_eq!(record(root)["state"], "in-pre");
    }

    #[test]
    fn a_polish_pass_that_parks_leaves_the_change_parked() {
        let repo = paused();
        let root = repo.root();
        let here = repo.project();
        let mut steps = played(root);
        steps.g3 = VecDeque::from([false, false]);
        let said = resume(&here, "c1", Answer::Polish, "x", None, &mut steps);
        let why = "Change c1 parked: G3 still red after one fix.";
        assert!(parked(said).starts_with(why));
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(false));
        assert_eq!(record(root)["state"], "parked");
        let broken = repo_with_change(r#"{"id": "c1", "decision": {"g4": true}}"#);
        let root = broken.root();
        state::pause_at_g4(root, "c1").unwrap();
        let here = broken.project();
        let mut steps = played(root);
        let said = resume(&here, "c1", Answer::Polish, "x", None, &mut steps);
        let start = ".gdd/changes/c1/change.json: missing field";
        assert!(said.unwrap_err().starts_with(start));
        assert!(steps.ran.is_empty());
    }

    /// A repository with an author that ignores `.gdd/`, with spike `c1` started on its branch,
    /// checked out, and its record.
    fn spike_folder() -> Repo {
        let repo = Repo::new();
        repo.identify();
        repo.write(".gitignore", ".gdd/\n");
        repo.commit();
        repo.start("c1");
        let record = r#"{"id": "c1", "type": "Spike", "state": "building", "inbox": []}"#;
        repo.write(&change::record_path("c1"), record);
        repo
    }

    /// The steps of spike `c1` (see [`played`]).
    fn spike_played(root: &Path) -> Played<'_> {
        let mut steps = played(root);
        steps.decided.kind = SPIKE.to_string();
        steps
    }

    /// What the Builder of a spike leaves: its code and its ADR.
    fn decides(root: &Path) {
        change::write(root, "src/app.txt", "spike code\n").unwrap();
        let adr = "# 0002 · Where the notes live\n## Decision\nSQLite.\n";
        change::write(root, "docs/adr/0002-storage.md", adr).unwrap();
    }

    /// What an agent call costs, in the log of change `c1`.
    fn costs(root: &Path) {
        log::append(root, r#"{"change": "c1", "cost_usd": 0.01}"#).unwrap();
    }

    const ADR_SAID: &str = "say ADR: docs/adr/0002-storage.md";

    #[test]
    fn a_spike_builds_in_spike_mode_and_waits_at_g4_with_its_work_on_its_branch() {
        let repo = spike_folder();
        let root = repo.root();
        let mut steps = spike_played(root);
        steps.acts = vec![(Who::BuilderBuild, decides)];
        assert_eq!(launched(&mut steps, 1), Ok(Ended::Paused));
        // No test mode, G3, Reviewer, merge or pre, whatever its decision says of them.
        assert_eq!(steps.ran, ["build b mode=spike", "G2", ADR_SAID]);
        assert_eq!(record(root)["state"], "in-pre");
        let last = repo.git(&["log", "-1", "--format=%s"]);
        assert_eq!(last, "gdd(c1): spike work");
        repo.git(&["cat-file", "-e", "change/c1:src/app.txt"]);
        assert_eq!(repo.git(&["status", "--porcelain"]), "");
        assert_eq!(repo.git(&["rev-list", "--count", "main"]), "2");
        // A relaunch whose code passes G2 already skips the Builder, and has nothing to commit.
        let head = repo.git(&["rev-parse", "HEAD"]);
        let mut steps = spike_played(root);
        steps.decided.resume_green = true;
        assert_eq!(launched(&mut steps, 1), Ok(Ended::Paused));
        assert_eq!(steps.ran, [ADR_SAID]);
        assert_eq!(repo.git(&["rev-parse", "HEAD"]), head);
    }

    #[test]
    fn a_spike_still_red_replans_without_test_mode_then_parks_saying_why() {
        let repo = spike_folder();
        let root = repo.root();
        let path_guard = "src/a: protected\n\n  gdd/b: protected\n";
        let failures = serde_json::json!({"gate": "G2", "failures": [
            {"name": "path-guard", "tail": path_guard},
            {"name": "adr", "tail": crate::gates::spike::NO_ADR},
            {"name": "other"},
        ]});
        change::write(root, &Gate::G2.report_path("c1"), &failures.to_string()).unwrap();
        let mut steps = spike_played(root);
        steps.g2 = VecDeque::from([false; 9]);
        steps.replan = "NEXT: test";
        let why = "G2 still red after 3 retries and 1 replan: src/a: protected;   gdd/b: \
                   protected; a Spike leaves its decision in a new file under docs/adr/ with a \
                   ## Decision section";
        assert_eq!(launched(&mut steps, 1), park(why));
        let round = ["build b mode=spike", "G2"];
        let replan = [
            "replan o reason=g2",
            "build b mode=spike after=replan",
            "G2",
        ];
        assert_eq!(steps.ran, [&round[..], &round, &round, &replan].concat());
        // Other kinds park as they always did.
        let mut steps = played(root);
        steps.decided.tests_first = false;
        steps.g2 = VecDeque::from([false; 9]);
        let plain = "G2 still red after 3 retries and 1 replan";
        assert_eq!(launched(&mut steps, 1), park(plain));
        // Without a report, a spike has nothing more to say.
        fs::remove_file(root.join(Gate::G2.report_path("c1"))).unwrap();
        let mut steps = spike_played(root);
        steps.decided.retries = 1;
        steps.g2 = VecDeque::from([false]);
        let alone = "G2 still red after 1 retries and 0 replan";
        assert_eq!(launched(&mut steps, 0), park(alone));
        // A report that is not JSON cannot say why.
        change::write(root, &Gate::G2.report_path("c1"), "{not json").unwrap();
        let mut steps = spike_played(root);
        steps.g2 = VecDeque::from([false; 9]);
        let Err(Stop::Fail(e)) = launched(&mut steps, 0) else {
            panic!("a report that cannot be read stops the recipe");
        };
        assert!(e.starts_with(".gdd/changes/c1/g2.json: "), "{e}");
    }

    #[test]
    fn a_spike_parks_once_its_agent_calls_reach_its_budget() {
        let repo = spike_folder();
        let root = repo.root();
        // Other changes, and lines that are not JSON, do not count; a line without a cost is free.
        for line in [
            r#"{"change": "c2", "cost_usd": 3}"#,
            r#"{"change": "c1"}"#,
            "x",
        ] {
            log::append(root, line).unwrap();
        }
        let mut steps = spike_played(root);
        steps.g2 = VecDeque::from([false; 9]);
        steps.acts = vec![(Who::BuilderBuild, costs)];
        let why = "the spike spent 0.02 USD of its budget of 0.02 USD";
        assert_eq!(launched_within(&mut steps, 1, 0.02), park(why));
        let calls = ["build b mode=spike", "G2", "build b mode=spike"];
        assert_eq!(steps.ran, calls);
        // The calls of the Orchestrator count too, and so do those of the earlier runs.
        let mut steps = spike_played(root);
        steps.decided.retries = 1;
        steps.g2 = VecDeque::from([false; 9]);
        steps.acts = vec![(Who::BuilderBuild, costs), (Who::Orchestrator, costs)];
        let why = "the spike spent 0.04 USD of its budget of 0.04 USD";
        assert_eq!(launched_within(&mut steps, 1, 0.04), park(why));
        let calls = ["build b mode=spike", "G2", "replan o reason=g2"];
        assert_eq!(steps.ran, calls);
        // Other kinds have no budget.
        let folder = repo_folder();
        let mut steps = played(&folder.0);
        steps.decided.reviewer = false;
        steps.acts = vec![(Who::BuilderTest, costs), (Who::BuilderBuild, costs)];
        assert_eq!(launched_within(&mut steps, 1, 0.01), Ok(Ended::Closed));
        assert_eq!(spent(&folder.0, "c1"), Ok(0.02));
    }

    /// Whether spike `c1`, whose state root is `root`, is still within the budget `spike_usd`
    /// (see [`Run::within_budget`]).
    fn within(root: &Path, spike_usd: f64) -> Result<(), Stop> {
        let project = Project {
            root,
            state: root,
            worktree: root,
            work: root,
            main: "main",
            replans: 0,
            max: 1,
            spike_usd,
        };
        let mut steps = spike_played(root);
        let decided = steps.decided.clone();
        let run = Run {
            project: &project,
            id: "c1",
            steps: &mut steps,
            decided,
        };
        run.within_budget()
    }

    /// A folder whose log says change `c1` spent each of `costs`, in USD.
    fn spending(costs: &[&str]) -> Folder {
        let folder = Folder::new();
        for cost in costs {
            let line = format!(r#"{{"change": "c1", "cost_usd": {cost}}}"#);
            log::append(&folder.0, &line).unwrap();
        }
        folder
    }

    #[test]
    fn a_budget_reached_but_for_what_adding_up_costs_loses_is_reached() {
        let folder = spending(&["0.7", "0.1"]);
        assert!(spent(&folder.0, "c1").unwrap() < 0.8);
        let why = "the spike spent 0.80 USD of its budget of 0.80 USD";
        assert_eq!(within(&folder.0, 0.8), Err(Stop::Park(why.into())));
        assert_eq!(within(&folder.0, 0.81), Ok(()));
        // What it loses is far below a cent, never more than SPENT_ERROR.
        let folder = spending(&["0.5"]);
        let why = "the spike spent 0.50 USD of its budget of 0.50 USD";
        assert_eq!(within(&folder.0, 0.5), Err(Stop::Park(why.into())));
        let reached = within(&folder.0, 0.5 + SPENT_ERROR);
        assert!(matches!(reached, Err(Stop::Park(_))), "{reached:?}");
        assert_eq!(within(&folder.0, 0.5 + 2.0 * SPENT_ERROR), Ok(()));
        // Above it, the change has spent more than its budget, and says so.
        let why = "the spike spent 0.50 USD of its budget of 0.30 USD";
        assert_eq!(within(&folder.0, 0.3), Err(Stop::Park(why.into())));
    }

    #[test]
    fn what_a_change_spent_adds_up_the_cost_of_its_lines_in_the_log() {
        let folder = Folder::new();
        assert_eq!(spent(&folder.0, "c1"), Ok(0.0));
        for line in [
            r#"{"change": "c1", "cost_usd": 0.5}"#,
            r#"{"change": "c2", "cost_usd": 7}"#,
            r#"{"change": "c1"}"#,
            r#"{"change": "c1", "cost_usd": null}"#,
            r#"{"change": "c1", "cost_usd": "1"}"#,
            "not json",
            r#"[{"change": "c1", "cost_usd": 9}]"#,
            r#"{"change": "c1", "cost_usd": 0.25}"#,
        ] {
            log::append(&folder.0, line).unwrap();
        }
        assert_eq!(spent(&folder.0, "c1"), Ok(0.75));
        assert_eq!(spent(&folder.0, "c2"), Ok(7.0));
        assert_eq!(spent(&folder.0, "c3"), Ok(0.0));
        // A log that is not a file of its own cannot be read.
        fs::remove_file(folder.0.join(log::PATH)).unwrap();
        fs::create_dir_all(folder.0.join(log::PATH)).unwrap();
        let err = spent(&folder.0, "c1").unwrap_err();
        assert!(err.starts_with("cannot read .gdd/log.jsonl: "), "{err}");
    }

    #[test]
    fn what_the_last_g2_failed_on_is_each_line_of_its_failures() {
        let folder = Folder::new();
        assert_eq!(g2_failures(&folder.0, "c1"), Ok(String::new()));
        let report = r#"{"failures": [{"tail": "a\n \nb"}, {"tail": 3}, {"tail": "c"}]}"#;
        folder.write(&Gate::G2.report_path("c1"), report);
        assert_eq!(g2_failures(&folder.0, "c1"), Ok("a; b; c".to_string()));
        folder.write(&Gate::G2.report_path("c1"), r#"{"failures": "x"}"#);
        assert_eq!(g2_failures(&folder.0, "c1"), Ok(String::new()));
        fs::remove_file(folder.0.join(Gate::G2.report_path("c1"))).unwrap();
        fs::create_dir_all(folder.0.join(Gate::G2.report_path("c1"))).unwrap();
        let err = g2_failures(&folder.0, "c1").unwrap_err();
        let start = "cannot read .gdd/changes/c1/g2.json: ";
        assert!(err.starts_with(start), "{err}");
    }

    /// The record of spike `c1`, titled `Storage`, in pre.
    fn spike_record() -> String {
        let titled = r#""title": "Storage", "type": "Spike""#;
        let record = RECORD.replace(r#""title": "App""#, titled);
        record.replace(r#""type": "Feature""#, r#""type": "Spike""#)
    }

    /// The repository with spike `c1` paused at G4, whose branch holds its code and its ADR.
    fn spike_paused() -> Checkout {
        let repo = repo_with_change(&spike_record());
        repo.git(&["checkout", "-q", "change/c1"]);
        decides(repo.root());
        repo.commit();
        repo.git(&["checkout", "-q", "main"]);
        finish(&repo.project(), "c1", Ok(Ended::Paused)).unwrap();
        repo
    }

    #[test]
    fn ok_at_g4_brings_only_the_adr_of_a_spike_to_main_and_closes_it() {
        let repo = spike_paused();
        let root = repo.root();
        worktree::open(root, &repo.work, "c1").unwrap();
        let here = repo.project();
        let mut steps = played(root);
        let said = resume(&here, "c1", Answer::Ok, "Go", None, &mut steps);
        assert_eq!(done(said), CLOSED);
        // No promotion, no observation.
        assert_eq!(unwaited(&steps), ["G4 ok human: Go"]);
        let message = repo.git(&["log", "-1", "--format=%s", "main"]);
        assert_eq!(message, "gdd(c1): Spike ADR · Storage");
        repo.git(&["cat-file", "-e", "main:docs/adr/0002-storage.md"]);
        assert_eq!(repo.tool().file_at("main", "src/app.txt").unwrap(), None);
        repo.git(&["cat-file", "-e", "change/c1:src/app.txt"]);
        assert_eq!(record(root)["state"], "closed");
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(false));
        assert!(open_requests(root).is_empty());
        assert!(!repo.work.exists(), "the worktree stays");
    }

    #[test]
    fn ok_at_g4_to_a_spike_main_cannot_take_leaves_it_paused() {
        let repo = spike_paused();
        let root = repo.root();
        repo.write("README.md", "uncommitted\n");
        let before = repo.git(&["rev-parse", "main"]);
        let here = repo.project();
        let mut steps = played(root);
        let said = resume(&here, "c1", Answer::Ok, "Go", None, &mut steps);
        let err = said.unwrap_err();
        assert!(err.contains("without uncommitted changes"), "{err}");
        assert!(unwaited(&steps).is_empty());
        assert_eq!(repo.git(&["rev-parse", "main"]), before);
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(true));
        assert_eq!(record(root)["state"], "in-pre");
        assert_eq!(open_requests(root), [g4_request("c1", SPIKE)]);
    }

    #[test]
    fn the_g4_of_a_spike_asks_for_its_adr_and_promises_no_demo_or_promotion() {
        let repo = spike_paused();
        assert_eq!(open_requests(repo.root()), [g4_request("c1", SPIKE)]);
        let request = g4_request("c1", SPIKE);
        let start = "G4 · Release Gate: read the ADR files the spike left under docs/adr/ on its \
                     branch change/c1, then answer with gdd resume c1 --g4 ok (bring only those \
                     files to main, in a commit of their own: its code stays on its branch), ";
        assert!(request.starts_with(start), "{request}");
        for promised in ["demo", " pre", "production", "promot"] {
            assert!(!request.contains(promised), "{promised}: {request}");
        }
        let inbox = inbox_of(repo.root());
        assert!(inbox.contains("the ADR files the spike left"), "{inbox}");
        // Any other kind keeps the request of a release.
        let feature = g4_request("c1", "Feature");
        assert!(feature.contains("check the pre demo"), "{feature}");
        assert_eq!(g4_request("c1", ""), feature);
    }

    #[test]
    fn polishing_a_spike_still_red_parks_after_one_round() {
        use crate::gates::spike::NO_ADR;
        let repo = spike_paused();
        let root = repo.root();
        let before = repo.git(&["rev-parse", "main"]);
        let failures = serde_json::json!({"failures": [{"tail": NO_ADR}]});
        change::write(root, &Gate::G2.report_path("c1"), &failures.to_string()).unwrap();
        let here = repo.project();
        let mut steps = played(root);
        steps.g2 = VecDeque::from([false; 9]);
        let said = resume(&here, "c1", Answer::Polish, "More", None, &mut steps);
        let Ok(Said::Parked(parked)) = &said else {
            panic!("a red polish round parks: {said:?}");
        };
        let why = format!("G2 still red after the polish round: {NO_ADR}");
        assert!(parked.contains(&why), "{parked}");
        // No retry, no replan: one agent call.
        assert_eq!(steps.ran, ["build b mode=spike fix=polish", "G2"]);
        assert_eq!(repo.git(&["rev-parse", "main"]), before);
        assert_eq!(record(root)["state"], "parked");
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(false));
    }

    #[test]
    fn polishing_a_spike_builds_it_again_and_waits_at_g4_without_a_merge() {
        let repo = spike_paused();
        let root = repo.root();
        let before = repo.git(&["rev-parse", "main"]);
        // What prepare decided comes from the record of the spike.
        let here = repo.project();
        let mut steps = played(root);
        let said = resume(&here, "c1", Answer::Polish, "More", None, &mut steps);
        assert!(done(said).starts_with("Paused at G4."));
        let polished = ["build b mode=spike fix=polish", "G2", ADR_SAID];
        assert_eq!(steps.ran, polished);
        assert_eq!(repo.git(&["rev-parse", "main"]), before);
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(true));
        assert_eq!(record(root)["state"], "in-pre");
        // KO then leaves main as it was, and the branch keeps the spike.
        let said = resume(&here, "c1", Answer::Ko, "No", None, &mut steps);
        let rejected = "Change c1 rejected at G4: production untouched.";
        assert_eq!(said, Ok(Said::Done(rejected.into())));
        assert_eq!(repo.git(&["rev-parse", "main"]), before);
        repo.git(&["cat-file", "-e", "change/c1:docs/adr/0002-storage.md"]);
        assert_eq!(record(root)["state"], "rejected");
    }

    #[test]
    fn the_answers_to_g4_are_ok_ko_and_polish() {
        for (word, answer) in [
            ("ok", Answer::Ok),
            ("ko", Answer::Ko),
            ("polish", Answer::Polish),
        ] {
            assert_eq!(word.parse(), Ok(answer));
            assert!(ANSWERS.contains(&word));
        }
        let maybe = "maybe".parse::<Answer>();
        let why = "G4 is ok, ko or polish, not \"maybe\"";
        assert_eq!(maybe, Err(why.to_string()));
    }
}
