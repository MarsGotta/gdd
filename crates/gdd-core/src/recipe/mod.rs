//! The recipe of a change: what `gdd launch` and `gdd resume` run, in the order of the kit's
//! workflow (`gdd-kit/workflows/gdd-change/workflow.yml`).
//!
//! 1. Prepare. Test mode, when the change needs tests first and its code does not pass G2
//!    already: the Builder in test mode, Path Guard for it (a failure parks) and the build
//!    snapshot.
//! 2. Unless its code passes G2 already: rounds of the Builder in build mode and G2. Still red,
//!    the replans: the Orchestrator, test mode again when it says so, and one more round. Still
//!    red, the change parks.
//! 3. G3. Red: one fix by the Builder, G2, and G3 again, which decides. Still red, it parks.
//! 4. When the change has a Reviewer: its review. KO: one fix, G2, G3 and a second review. KO,
//!    or G3 red, it parks.
//! 5. G3 opened by the machines, merge, pre.
//! 6. A change that needs the human G4 waits for `gdd resume`. Any other is promoted and
//!    observed: closed.
//!
//! The recipe decides which steps run, in what order, and when the change stops; the steps that
//! read the configuration of the project are [`Steps`].
//!
//! Around every agent call, its own checks: no agent changes GDD state (see [`State`]) or leaves
//! links where gdd writes next (see [`links_left`]), and the Reviewer only reads: the working
//! tree outside `.gdd/` is the same after its call, down to the kind of each path, its
//! permissions and where its links point (see [`Tree`]). Either parks the change.

pub mod state;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, ErrorKind};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::str::FromStr;

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::agents::files;
use crate::agents::invoke::{self, Call, Who};
use crate::agents::models::Model;
use crate::change::{self, Note};
use crate::gates::{Gate, approve, verdict};
use crate::git::{self, Git};
use crate::guard;
use crate::{inbox, log};

/// The arguments of a round of build mode.
const BUILD: &str = "mode=build";

/// The answers to G4, by name.
pub const ANSWERS: [&str; 3] = ["ok", "ko", "polish"];

/// Why a launch does not leave a working tree with uncommitted changes.
const UNCOMMITTED: &str =
    "The working tree has uncommitted changes. Commit them before launching a change.";

/// Why a change parks when G3 is still red after its fix.
const G3_RED: &str = "G3 still red after one fix";

/// Why a change parks when the Reviewer still says KO, or G3 is red, after its fix.
const REVIEW_KO: &str = "Reviewer KO or G3 red after one fix";

/// Local state of the tool, which the Reviewer's read-only check leaves out.
const LOCAL_STATE: &str = ".gdd/";

/// The fields of the record of a change the gdd commands an agent may run write: its notes and
/// the paths the scope check accepted.
const AGENT_FIELDS: [&str; 2] = ["inbox", "out_of_scope"];

/// What prepare decided for a change, as the recipe reads it: from what `gdd prepare` printed,
/// or from the record of a prepared change. Every field must be there: a missing one would skip
/// a step.
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
}

impl Decided {
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
    /// Runs `gate` for build mode, saved and logged: whether it passed.
    fn gate(&mut self, gate: Gate) -> Result<bool, String>;
    /// Records the decision `decision` (`ok` or `ko`) on `gate` of the change, by `by`, with
    /// `note` (`gdd gate`).
    fn decide(&mut self, gate: &str, decision: &str, by: &str, note: &str) -> Result<(), String>;
    /// Merges the change into the main branch and deploys it to pre (`gdd merge`).
    fn merge(&mut self) -> Result<(), String>;
    /// The release step `step`: `pre`, `promote` or `observe` (`gdd release`).
    fn release(&mut self, step: &str) -> Result<(), String>;
}

/// What the recipe reads from the project itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Project<'a> {
    /// The project root.
    pub root: &'a Path,
    /// The branch changes merge into.
    pub main: &'a str,
    /// Replans of a change still red in G2 before it parks.
    pub replans: u32,
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

/// `gdd launch`: checks out the branch of change `id` (uncommitted work in tracked files is
/// refused, unless it is on that branch: then it is the work of an earlier run, committed
/// first), prepares it and runs the recipe.
pub fn launch(project: &Project, id: &str, steps: &mut impl Steps) -> Result<Said, String> {
    let root = project.root;
    change::read_record(root, id)?;
    to_branch(root, id)?;
    state::clear(root, id)?;
    let decided = steps.prepare()?;
    let mut run = Run {
        project,
        id,
        steps,
        decided,
    };
    let ended = run.launched();
    finish(root, id, ended)
}

/// Checks out the branch of change `id` of the project at `root`; see [`launch`].
fn to_branch(root: &Path, id: &str) -> Result<(), String> {
    let git = Git::new(root);
    let branch = change::branch(id);
    let said = |e: git::Error| e.to_string();
    if git.has_tracked_changes().map_err(said)? {
        if git.current_branch().map_err(said)? != branch {
            return Err(UNCOMMITTED.to_string());
        }
        let message = format!("gdd({id}): work of the earlier run, before relaunch");
        git.commit_all(&message).map_err(said)?;
    }
    git.checkout(&branch).map_err(said)
}

/// `gdd resume`: the answer of the role `role` to the G4 of change `id`, which must be paused at
/// it, with `note` (which `polish` needs). Builders and Reviewers cannot answer. `ok` promotes
/// the change; `ko` rejects it, production untouched; `polish` checks out the branch of the
/// change and runs a polish pass (see [`polish_pass`]).
pub fn resume(
    project: &Project,
    id: &str,
    answer: Answer,
    note: &str,
    role: Option<&str>,
    steps: &mut impl Steps,
) -> Result<Said, String> {
    let root = project.root;
    approve::may_record(role).map_err(|e| e.to_string())?;
    let text = change::read_record(root, id)?;
    if answer == Answer::Polish && note.trim().is_empty() {
        return Err(format!(
            "polish needs your notes: gdd resume {id} --g4 polish --note ..."
        ));
    }
    if !state::paused_at_g4(root, id)? {
        return Err(format!("Change {id} is not paused at G4."));
    }
    if answer == Answer::Polish {
        // First, so a checkout that fails leaves the change paused.
        let (git, branch) = (Git::new(root), change::branch(id));
        git.checkout(&branch).map_err(|e| e.to_string())?;
    }
    state::clear(root, id)?;
    let path = change::record_path(id);
    let answered = change::with_request_resolved(&text, &g4_request(id), &log::now())
        .map_err(|e| format!("{path}: {e}"))?;
    change::write(root, &path, &answered)?;
    match answer {
        Answer::Ok => {
            steps.decide("G4", "ok", "human", note)?;
            finish(root, id, promoted(steps))
        }
        Answer::Ko => {
            steps.decide("G4", "ko", "human", note)?;
            saved_in_state(root, id, "rejected")?;
            let said = format!("Change {id} rejected at G4: production untouched.");
            Ok(Said::Done(said))
        }
        Answer::Polish => polish_pass(project, id, note, steps),
    }
}

/// A polish pass of change `id`, on its branch: `note`, the human's notes at G4, kept for the
/// Builder; then the recipe again from build mode, the notes in its first round, until it
/// pauses at G4 again.
fn polish_pass(
    project: &Project,
    id: &str,
    note: &str,
    steps: &mut impl Steps,
) -> Result<Said, String> {
    let root = project.root;
    polish_noted(root, id, note)?;
    let mut run = Run {
        project,
        id,
        steps,
        decided: decided_of(root, id)?,
    };
    let ended = run.polished();
    finish(root, id, ended)
}

/// Keeps `note`, the human's notes at G4, in the record of change `id` of the project at `root`
/// for the Builder (see [`change::POLISH`]); logs it as the answer to G4, which stays pending;
/// and puts the change back in Build.
fn polish_noted(root: &Path, id: &str, note: &str) -> Result<(), String> {
    let text = change::read_record(root, id)?;
    let path = change::record_path(id);
    let ts = log::now();
    let polish = Note {
        kind: change::POLISH,
        text: note,
        ts: &ts,
    };
    let fields = vec![("state", Value::from("building"))];
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

/// What prepare decided for change `id` of the project at `root`, as its record keeps it.
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

/// Writes the record of change `id` of the project at `root` in state `state`, and saves its
/// Decision Inbox.
fn saved_in_state(root: &Path, id: &str, state: &str) -> Result<(), String> {
    let text = change::read_record(root, id)?;
    let path = change::record_path(id);
    let fields = vec![("state", Value::from(state))];
    let saved = change::with_fields(&text, fields).map_err(|e| format!("{path}: {e}"))?;
    change::write(root, &path, &saved)?;
    inbox::save(root, id, &saved).map(drop)
}

/// The request to the human at G4 of change `id`, in its Decision Inbox.
fn g4_request(id: &str) -> String {
    format!(
        "G4 · Release Gate: check the pre demo and the change message, then answer with \
         gdd resume {id} --g4 ok (promote to production), --g4 ko (reject: production \
         untouched) or --g4 polish --note <what to polish> (one more Builder round)."
    )
}

/// What the person who ran the recipe of change `id` of the project at `root` reads, from how
/// it ended. A pause at G4 leaves the request in the Decision Inbox and the run state paused; a
/// stop that parks parks the change (see [`change::park`]).
fn finish(root: &Path, id: &str, ended: Result<Ended, Stop>) -> Result<Said, String> {
    let inbox = format!("Message: {}", change::inbox_path(id));
    match ended {
        Ok(Ended::Closed) => {
            let tokens = format!("Tokens: gdd report --change {id}");
            let said = format!("Change {id} closed. {inbox} · {tokens}");
            Ok(Said::Done(said))
        }
        Ok(Ended::Paused) => {
            pause_at_g4(root, id)?;
            let next = format!("To continue: gdd resume {id} --g4 ok|ko|polish");
            Ok(Said::Done(format!("Paused at G4. {inbox} · {next}")))
        }
        Err(Stop::Park(reason)) => change::park(root, id, &reason).map(Said::Parked),
        Err(Stop::Fail(e)) => Err(e),
    }
}

/// Leaves the request of G4 in the Decision Inbox of change `id` of the project at `root`, and
/// its run state paused at G4.
fn pause_at_g4(root: &Path, id: &str) -> Result<(), String> {
    let text = change::read_record(root, id)?;
    let path = change::record_path(id);
    let ts = log::now();
    let request = Note {
        kind: "needs",
        text: &g4_request(id),
        ts: &ts,
    };
    let noted = change::with_note(&text, &request).map_err(|e| format!("{path}: {e}"))?;
    change::write(root, &path, &noted)?;
    inbox::save(root, id, &noted)?;
    state::pause_at_g4(root, id)
}

/// Promotes the change and observes it in production.
fn promoted(steps: &mut impl Steps) -> Result<Ended, Stop> {
    steps.release("promote")?;
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
    /// The recipe of a launch, once prepared.
    fn launched(&mut self) -> Result<Ended, Stop> {
        let green = self.decided.resume_green;
        if self.decided.tests_first && !green {
            self.test_mode("mode=test", false)?;
        }
        if !green {
            self.build(BUILD)?;
        }
        self.integrate(false)
    }

    /// The recipe of a polish pass: from build mode, the human's notes first.
    fn polished(&mut self) -> Result<Ended, Stop> {
        self.build("mode=build fix=polish")?;
        self.integrate(true)
    }

    /// Calls `who` with `args`, then checks what it did: GDD state it changed goes back as it was,
    /// what it left in the folder of the change that gdd would write through goes (see
    /// [`links_left`]), and either parks the change, as a Reviewer that changed the working tree
    /// does.
    fn call(&mut self, who: Who, args: &str) -> Result<(), Stop> {
        let root = self.project.root;
        let call = Call {
            who,
            model: self.decided.model(who),
            args,
        };
        let state = State::of(root, self.id)?;
        let tree = match who {
            Who::Reviewer => Some(working_tree(root)?),
            Who::BuilderTest | Who::BuilderBuild | Who::Orchestrator => None,
        };
        let called = self.steps.agent(&call);
        // The state first: it leaves real folders on the way to the folder of the change.
        let mut changed = state.put_back(root)?;
        changed.extend(links_left(root, self.id)?);
        if !changed.is_empty() {
            let files = changed.join(", ");
            return Err(Stop::Park(format!("the {who} changed GDD state: {files}")));
        }
        if let Some(before) = tree {
            let changed = differ(&before, &working_tree(root)?);
            if !changed.is_empty() {
                let paths = changed.join(", ");
                let why = format!("the Reviewer changed {paths}: Reviewers only read");
                return Err(Stop::Park(why));
            }
        }
        Ok(called?)
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

    /// Build mode until G2 passes: the rounds, the first with `first`, then the replans. Still
    /// red, the change parks.
    fn build(&mut self, first: &str) -> Result<(), Stop> {
        if self.rounds(first)? {
            return Ok(());
        }
        let replans = self.project.replans;
        for _ in 0..replans {
            self.call(Who::Orchestrator, "reason=g2")?;
            if verdict::replanned_to_test(self.project.root, self.id)? {
                self.steps.snapshot(Snapshot::Round)?;
                self.test_mode("mode=test after=replan", true)?;
            }
            self.call(Who::BuilderBuild, "mode=build after=replan")?;
            if self.steps.gate(Gate::G2)? {
                return Ok(());
            }
        }
        let retries = self.decided.retries;
        let why = format!("G2 still red after {retries} retries and {replans} replan");
        Err(Stop::Park(why))
    }

    /// The rounds of build mode and G2, at least one, the first with `first`: whether G2
    /// passed.
    fn rounds(&mut self, first: &str) -> Result<bool, Stop> {
        let mut args = first;
        for _ in 0..self.decided.retries.max(1) {
            self.call(Who::BuilderBuild, args)?;
            if self.steps.gate(Gate::G2)? {
                return Ok(true);
            }
            args = BUILD;
        }
        Ok(false)
    }

    /// G3, the review, the merge and pre; then the pause at G4, or production. A polish pass
    /// (`polishing`) with nothing new for the main branch skips the merge.
    fn integrate(&mut self, polishing: bool) -> Result<Ended, Stop> {
        self.g3()?;
        if self.decided.reviewer {
            self.review()?;
        }
        let note = "automatic chain and Reviewer";
        self.steps.decide("G3", "ok", "machines", note)?;
        let Project { root, main, .. } = *self.project;
        let said = |e: git::Error| Stop::Fail(e.to_string());
        if polishing && nothing_new(root, self.id, main).map_err(said)? {
            Git::new(root).checkout(main).map_err(said)?;
        } else {
            self.steps.merge()?;
        }
        self.steps.release("pre")?;
        if self.decided.g4 {
            return Ok(Ended::Paused);
        }
        promoted(&mut *self.steps)
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
        let (root, id) = (self.project.root, self.id);
        self.call(Who::Reviewer, "round=1")?;
        if verdict::review(root, id)? {
            return Ok(());
        }
        self.call(Who::BuilderBuild, "mode=build fix=review")?;
        self.steps.gate(Gate::G2)?;
        let g3 = self.steps.gate(Gate::G3)?;
        self.call(Who::Reviewer, "round=2")?;
        if verdict::review(root, id)? && g3 {
            Ok(())
        } else {
            Err(Stop::Park(REVIEW_KO.to_string()))
        }
    }
}

/// What a file of GDD state is, which says how it may change during an agent call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// The record of the change: only in [`AGENT_FIELDS`].
    Record,
    /// An approval of G0 or of the red zones: never.
    Approval,
    /// The log: only by lines added at its end.
    Log,
}

/// GDD state as it was before an agent call: the record of the change, the approvals of G0 and
/// of the red zones, and the log, each with its bytes (`None` when it did not exist), read as
/// files of their own (see [`files::read`]). Agents change it only through gdd.
struct State {
    files: Vec<(Kind, String, Option<Vec<u8>>)>,
}

impl State {
    /// The GDD state of change `id` of the project at `root`, as it is now.
    fn of(root: &Path, id: &str) -> Result<State, String> {
        let paths = [
            (Kind::Record, change::record_path(id)),
            (Kind::Approval, approve::G0_PATH.to_string()),
            (Kind::Approval, approve::ZONES_PATH.to_string()),
            (Kind::Log, log::PATH.to_string()),
        ];
        let mut state = State { files: Vec::new() };
        for (kind, path) in paths {
            let unread = |e: io::Error| format!("cannot read {path}: {e}");
            let bytes = files::read(root, &path).map_err(unread)?;
            state.files.push((kind, path, bytes));
        }
        Ok(state)
    }

    /// Puts back as it was each file of this state that changed in a way its kind does not
    /// allow, or that is no longer a file of its own: those files, in order. Every file is
    /// checked and put back before a file that could not be put back stops the recipe.
    fn put_back(&self, root: &Path) -> Result<Vec<String>, String> {
        let (mut changed, mut stuck) = (Vec::new(), Vec::new());
        for (kind, path, before) in &self.files {
            let now = files::read(root, path).ok();
            if now.is_some_and(|now| allowed(*kind, before.as_deref(), now.as_deref())) {
                continue;
            }
            let restored = match before {
                Some(bytes) => files::replace(root, path, bytes),
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

/// Whether a file of GDD state of `kind` may go from `before` to `now` during an agent call
/// (`None`: no file).
fn allowed(kind: Kind, before: Option<&[u8]>, now: Option<&[u8]>) -> bool {
    match kind {
        Kind::Record => match (before.and_then(own_fields), now.and_then(own_fields)) {
            (Some(was), Some(is)) => was == is,
            _ => before == now,
        },
        Kind::Approval => before == now,
        Kind::Log => {
            let (before, now) = (before.unwrap_or_default(), now.unwrap_or_default());
            now.starts_with(before)
        }
    }
}

/// The fields of the record `bytes` that no agent may change, if it is a JSON object.
fn own_fields(bytes: &[u8]) -> Option<Map<String, Value>> {
    let mut record: Map<String, Value> = serde_json::from_slice(bytes).ok()?;
    for field in AGENT_FIELDS {
        record.remove(field);
    }
    Some(record)
}

/// What an agent left in the folder of change `id` of the project at `root` that gdd would
/// write through: links, files with other names, and anything else that is neither a file nor a
/// folder. They go, never what they point to; their paths, sorted.
fn links_left(root: &Path, id: &str) -> Result<Vec<String>, String> {
    let folder = format!(".gdd/changes/{id}");
    let mut left = Vec::new();
    let removed = remove_links(root, &root.join(&folder), &mut left);
    removed.map_err(|e| format!("cannot read {folder}: {e}"))?;
    left.sort();
    Ok(left)
}

/// Removes what is at `path` of the project at `root`, or inside it when it is a folder, that is
/// neither a folder nor a file of its own, without following links: the paths go to `left`.
fn remove_links(root: &Path, path: &Path, left: &mut Vec<String>) -> io::Result<()> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    if meta.is_dir() {
        for inside in fs::read_dir(path)? {
            remove_links(root, &inside?.path(), left)?;
        }
    } else if !meta.is_file() || meta.nlink() > 1 {
        fs::remove_file(path)?;
        let shown = path.strip_prefix(root).unwrap_or(path);
        left.push(shown.display().to_string());
    }
    Ok(())
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

    /// A git repository with one commit, as a folder.
    fn repo_folder() -> Folder {
        Repo::new().0
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
        }
    }

    /// The steps of the recipe, played: what ran, in order, with scripted answers. A gate
    /// passes once its answers run out; the Reviewer says OK, and the replan build mode. Each
    /// call of an agent does what `acts` gives it to do, on the project root.
    struct Played<'a> {
        root: &'a Path,
        ran: Vec<String>,
        decided: Decided,
        g2: VecDeque<bool>,
        g3: VecDeque<bool>,
        reviews: VecDeque<&'static str>,
        replan: &'static str,
        guard: Vec<String>,
        stuck: Option<Who>,
        acts: Vec<(Who, Act)>,
    }

    /// What an agent does on the project root when it is called.
    type Act = fn(&Path);

    fn played(root: &Path) -> Played<'_> {
        Played {
            root,
            ran: Vec::new(),
            decided: decided(),
            g2: VecDeque::new(),
            g3: VecDeque::new(),
            reviews: VecDeque::new(),
            replan: "NEXT: build",
            guard: Vec::new(),
            stuck: None,
            acts: Vec::new(),
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

    impl Played<'_> {
        /// Writes `text` as the answer of an agent in `path`.
        fn answer(&self, path: &str, text: &str) {
            change::write(self.root, path, &format!("{text}\n")).unwrap();
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
            Ok(())
        }

        fn merge(&mut self) -> Result<(), String> {
            self.ran.push("merge".into());
            Ok(())
        }

        fn release(&mut self, step: &str) -> Result<(), String> {
            self.ran.push(step.into());
            if step == "observe" && self.root.join("observe-fails").exists() {
                return Err("release.observe failed".into());
            }
            Ok(())
        }
    }

    /// Runs the recipe of a launch on change `c1`, once prepared, with `replans` replans.
    fn launched(steps: &mut Played, replans: u32) -> Result<Ended, Stop> {
        let project = Project {
            root: steps.root,
            main: "main",
            replans,
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
        folder.write("observe-fails", "");
        let mut steps = played(&folder.0);
        steps.decided.resume_green = true;
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
        let why = "the Builder in test mode changed GDD state: .gdd/g0.json, .gdd/zones.json, \
                   .gdd/log.jsonl";
        assert_eq!(ended, park(why));
        assert!(!folder.0.join(approve::G0_PATH).exists());
        assert_eq!(text(&folder, approve::ZONES_PATH), "ok");
        assert_eq!(text(&folder, log::PATH), LOG_LINE);
        // A record that went away comes back, even with its folder.
        let folder = with_state();
        let ended = acting(&folder, Who::Reviewer, |root| {
            fs::remove_file(root.join(change::record_path("c1"))).unwrap();
        });
        let why = format!("the Reviewer changed GDD state: {record}");
        assert_eq!(ended, park(&why));
        assert_eq!(text(&folder, &record), RECORD);
        let folder = with_state();
        let ended = acting(&folder, Who::BuilderBuild, |root| {
            fs::remove_dir_all(root.join(".gdd/changes")).unwrap();
        });
        assert!(matches!(ended, Err(Stop::Park(_))), "{ended:?}");
        assert_eq!(text(&folder, &record), RECORD);
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
        });
        assert_eq!(ended, Ok(Ended::Closed));
        assert!(text(&folder, &change::record_path("c1")).contains("kept it"));
        let log = text(&folder, log::PATH);
        let appended = log.starts_with(LOG_LINE) && log.contains("machine");
        assert!(appended, "{log}");
        // Without a record or a log before the call, nothing counts but what the call wrote.
        let folder = repo_folder();
        let ended = acting(&folder, Who::BuilderBuild, |root| {
            log::append(root, "{}").unwrap();
        });
        assert_eq!(ended, Ok(Ended::Closed));
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
        let why = "the Builder in test mode changed GDD state: .gdd/g0.json, .gdd/zones.json, \
                   .gdd/log.jsonl";
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
        let changed = [record.as_str(), approve::ZONES_PATH, log::PATH].join(", ");
        let why = format!("the Builder in build mode changed GDD state: {changed}");
        assert_eq!(ended, park(&why));
        assert_eq!(own(&folder, &record), RECORD);
        assert_eq!(own(&folder, approve::ZONES_PATH), "ok");
        assert_eq!(own(&folder, log::PATH), LOG_LINE);
        assert_eq!(text(&folder, "zones.json"), "ko");
        // A folder on the way that links elsewhere.
        let folder = with_state();
        let ended = acting(&folder, Who::BuilderBuild, |root| {
            let c1 = root.join(".gdd/changes/c1");
            fs::create_dir(root.join("elsewhere")).unwrap();
            fs::rename(&c1, root.join("elsewhere/c1")).unwrap();
            symlink(root.join("elsewhere/c1"), &c1).unwrap();
        });
        let why = format!("the Builder in build mode changed GDD state: {record}");
        assert_eq!(ended, park(&why));
        assert_eq!(own(&folder, &record), RECORD);
        assert_eq!(text(&folder, "elsewhere/c1/change.json"), RECORD);
    }

    #[test]
    fn links_left_where_gdd_writes_next_go_and_park_the_change() {
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
                   .gdd/changes/c1/outputs/src, .gdd/changes/c1/outputs/x.md";
        assert_eq!(ended, park(why));
        let c1 = folder.0.join(".gdd/changes/c1");
        for path in ["g2.json", "outputs/x.md", "outputs/src"] {
            assert!(fs::symlink_metadata(c1.join(path)).is_err(), "{path}");
        }
        assert_eq!(text(&folder, "target.json"), "outside");
        assert_eq!(text(&folder, ".gdd/changes/c1/notes.md"), "mine\n");
        assert_eq!(text(&folder, &change::record_path("c1")), RECORD);
    }

    #[test]
    fn a_change_folder_that_cannot_be_looked_at_is_an_error_not_an_empty_one() {
        let folder = Folder::new();
        assert_eq!(links_left(&folder.0, "c1"), Ok(Vec::new()));
        folder.write(".gdd/changes/c1/notes.md", "mine\n");
        assert_eq!(links_left(&folder.0, "c1"), Ok(Vec::new()));
        let long = "x".repeat(300);
        let err = links_left(&folder.0, &long).unwrap_err();
        let start = format!("cannot read .gdd/changes/{long}: ");
        assert!(err.starts_with(&start), "{err}");
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
        assert!(allowed(Kind::Approval, Some(b"a"), Some(b"a")));
        assert!(!allowed(Kind::Approval, Some(b"a"), Some(b"a ")));
        assert!(!allowed(Kind::Approval, None, Some(b"a")));
        assert!(!allowed(Kind::Approval, Some(b"a"), None));
        assert!(allowed(Kind::Approval, None, None));
        assert!(allowed(Kind::Log, Some(b"a\n"), Some(b"a\nb\n")));
        assert!(allowed(Kind::Log, None, Some(b"a\n")));
        assert!(allowed(Kind::Log, Some(b""), None));
        assert!(!allowed(Kind::Log, Some(b"a\n"), Some(b"b\na\n")));
        assert!(!allowed(Kind::Log, Some(b"a\n"), None));
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
            change::write(root, ".gdd/changes/c1/notes.md", "read it all\n").unwrap();
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
            fs::rename(root.join("README.md"), root.join(".gdd/readme")).unwrap();
            symlink(root.join(".gdd/readme"), root.join("README.md")).unwrap();
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
            fs::copy(root.join("README.md"), root.join(".gdd/readme")).unwrap();
            fs::remove_file(root.join("link")).unwrap();
            symlink(".gdd/readme", root.join("link")).unwrap();
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
                     "reviewer_model": {"model": "r"}, "orchestrator": {"model": "o"}}}"#;

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

    #[test]
    fn the_end_of_a_run_says_what_happened_and_what_to_do() {
        let folder = repo_folder();
        folder.write(".gdd/changes/c1/change.json", RECORD);
        let root = &folder.0;
        let closed = "Change c1 closed. Message: .gdd/changes/c1/inbox.md · Tokens: gdd report \
                      --change c1";
        let said = finish(root, "c1", Ok(Ended::Closed));
        assert_eq!(said, Ok(Said::Done(closed.into())));
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(false));
        let paused = "Paused at G4. Message: .gdd/changes/c1/inbox.md · To continue: gdd resume \
                      c1 --g4 ok|ko|polish";
        let said = finish(root, "c1", Ok(Ended::Paused));
        assert_eq!(said, Ok(Said::Done(paused.into())));
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(true));
        let request = &record(root)["inbox"][0];
        assert_eq!(request["kind"], "needs");
        assert_eq!(request["text"], g4_request("c1"));
        let inbox = inbox_of(root);
        assert!(inbox.contains("G4 · Release Gate"), "{inbox}");
        for answer in ["--g4 ok", "--g4 ko", "--g4 polish --note"] {
            assert!(inbox.contains(answer), "{answer}: {inbox}");
        }
        let said = finish(root, "c1", park(G3_RED));
        let message = "Change c1 parked: G3 still red after one fix. Message in \
                       .gdd/changes/c1/inbox.md";
        assert_eq!(said, Ok(Said::Parked(message.into())));
        assert_eq!(record(root)["state"], "parked");
        let failed = finish(root, "c1", Err(Stop::Fail("boom".into())));
        assert_eq!(failed, Err("boom".into()));
    }

    /// A repository with an author, that ignores `.gdd/`, with change `c1` and its branch, not
    /// checked out.
    fn repo_with_change(record: &str) -> Repo {
        let repo = Repo::new();
        repo.identify();
        repo.write(".gitignore", ".gdd/\n");
        repo.commit();
        repo.git(&["branch", "change/c1"]);
        repo.write(".gdd/changes/c1/change.json", record);
        repo
    }

    fn project(root: &Path) -> Project<'_> {
        Project {
            root,
            main: "main",
            replans: 1,
        }
    }

    #[test]
    fn launch_goes_to_the_branch_of_the_change_and_runs_the_recipe() {
        let repo = repo_with_change(RECORD);
        let root = repo.root();
        state::pause_at_g4(root, "c1").unwrap();
        let mut steps = played(root);
        steps.decided.reviewer = false;
        let said = launch(&project(root), "c1", &mut steps);
        assert!(done(said).starts_with("Change c1 closed."));
        assert_eq!(repo.tool().current_branch().unwrap(), "change/c1");
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(false));
        assert_eq!(steps.ran[..2], ["prepare", "test t mode=test"]);
        let mut steps = played(root);
        steps.decided.reviewer = false;
        steps.decided.g4 = true;
        let said = launch(&project(root), "c1", &mut steps);
        assert!(done(said).starts_with("Paused at G4."));
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(true));
    }

    #[test]
    fn launch_refuses_uncommitted_work_outside_the_branch_of_the_change() {
        let repo = repo_with_change(RECORD);
        let root = repo.root();
        repo.write("README.md", "changed\n");
        let mut steps = played(root);
        let refused = launch(&project(root), "c1", &mut steps);
        assert_eq!(refused, Err(UNCOMMITTED.to_string()));
        assert!(steps.ran.is_empty());
        assert_eq!(repo.tool().current_branch().unwrap(), "main");
        let missing = launch(&project(root), "c9", &mut steps).unwrap_err();
        let start = "change c9 does not exist";
        assert!(missing.starts_with(start), "{missing}");
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
        assert!(launch(&project(root), "c1", &mut steps).is_ok());
        let subject = repo.git(&["log", "-1", "--format=%s", "change/c1"]);
        let relaunch = "gdd(c1): work of the earlier run, before relaunch";
        assert_eq!(subject, relaunch);
        assert!(!repo.tool().has_changes().unwrap());
        assert_eq!(steps.ran[0], "prepare");
        let mut steps = played(root);
        steps.stuck = Some(Who::Reviewer);
        let said = launch(&project(root), "c1", &mut steps);
        let why = "Change c1 parked: the Reviewer did not answer within 2 seconds.";
        assert!(parked(said).starts_with(why));
    }

    #[test]
    fn only_a_person_answers_a_change_paused_at_g4() {
        let repo = repo_with_change(RECORD);
        let root = repo.root();
        let here = project(root);
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
    fn paused() -> Repo {
        let repo = repo_with_change(RECORD);
        finish(repo.root(), "c1", Ok(Ended::Paused)).unwrap();
        repo
    }

    #[test]
    fn ok_at_g4_promotes_the_change() {
        let repo = paused();
        let root = repo.root();
        let here = project(root);
        let mut steps = played(root);
        let role = Some("orchestrator");
        let said = resume(&here, "c1", Answer::Ok, "Fine", role, &mut steps);
        assert!(done(said).starts_with("Change c1 closed."));
        assert_eq!(steps.ran, ["G4 ok human: Fine", "promote", "observe"]);
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(false));
        let request = &record(root)["inbox"][0];
        assert!(request["resolved"].is_string(), "{request}");
    }

    #[test]
    fn ko_at_g4_rejects_the_change_and_production_stays_as_it_is() {
        let repo = paused();
        let root = repo.root();
        let here = project(root);
        let mut steps = played(root);
        let said = resume(&here, "c1", Answer::Ko, "No", None, &mut steps);
        let rejected = "Change c1 rejected at G4: production untouched.";
        assert_eq!(said, Ok(Said::Done(rejected.into())));
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
        repo.git(&["checkout", "-q", "main"]);
        let here = project(root);
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
            "merge",
            "pre",
        ];
        assert_eq!(steps.ran, expected);
        assert_eq!(repo.tool().current_branch().unwrap(), "change/c1");
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(true));
        let record = record(root);
        let notes = record["inbox"].as_array().unwrap();
        let kinds: Vec<&str> = notes.iter().filter_map(|n| n["kind"].as_str()).collect();
        assert_eq!(kinds, ["needs", "polish", "needs"]);
        assert_eq!(notes[1]["text"], note);
        assert!(notes[0]["resolved"].is_string());
        assert!(notes[2].get("resolved").is_none());
        assert_eq!(record["state"], "building");
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
        let here = project(root);
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
        let here = project(root);
        let mut steps = played(root);
        let said = resume(&here, "c1", Answer::Polish, "x", None, &mut steps);
        let start = "git checkout -q change/c1 -- failed";
        assert!(said.unwrap_err().starts_with(start));
        assert!(steps.ran.is_empty());
        assert_eq!(state::paused_at_g4(root, "c1"), Ok(true));
        assert_eq!(record(root)["state"], "in-pre");
    }

    #[test]
    fn a_polish_pass_that_parks_leaves_the_change_parked() {
        let repo = paused();
        let root = repo.root();
        let here = project(root);
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
        let here = project(root);
        let mut steps = played(root);
        let said = resume(&here, "c1", Answer::Polish, "x", None, &mut steps);
        let start = ".gdd/changes/c1/change.json: missing field";
        assert!(said.unwrap_err().starts_with(start));
        assert!(steps.ran.is_empty());
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
