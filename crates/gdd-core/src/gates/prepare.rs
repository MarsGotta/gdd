//! Preparing a change (`tminus prepare`), the first step of the recipe: it checks that the change
//! may enter Build, in this order: G0, the red zones (only a spike starts before a human
//! approves them), G1 for new or changed scenarios, the plan of a red change, a Reviewer of
//! another model family than the Builder (or of the same one, when the human accepted it at the
//! Setup: the Decision Inbox says so), its own branch checked out, and, for a change in G1, its
//! scenario files still the ones G1 sealed (see [`approve::unsealed`]). Then it starts the build:
//! its decision in the record, state `building`, the build starting at the commit checked out,
//! the requests of the earlier run resolved and the snapshots of the earlier build removed.
//!
//! A relaunch whose code already passes G2, and whose work Path Guard for build mode passes from
//! the new start of its build, skips the Builder (`resume_green`): it would only spend tokens.
//! Work left uncommitted that build mode may not touch (test mode's steps) runs the Builder
//! instead, as G3 judges build mode from that start. A relaunch whose code touched protected
//! paths starts its build where its branch left the main branch instead: starting at the commit
//! checked out would hide them.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::agents::models::{self, Models};
use crate::change::{self, Decision, Record};
use crate::gates::approve::{self, ZonesState};
use crate::gates::{self, BUILD_ROLE, Gate, verdict};
use crate::git::Git;
use crate::guard;
use crate::layout::{self, Layout};
use crate::log;
use crate::repo::touched::Software;

/// Why a change does not start before the Setup, in the new names: a project of the old layout
/// names its agent command as it does (see [`Layout::localize`]).
const G0_PENDING: &str = "G0 pending: run the Setup (the tminus-setup command).";

/// How a human approves the red zones.
const APPROVE_ZONES: &str = "(tminus gate zones ok --by human)";

/// Why a change with new or changed scenarios waits for G1, in the new names (see
/// [`G0_PENDING`]).
const G1_PENDING: &str =
    "New or changed scenarios without G1: a human must approve them (the tminus-change command).";

/// What a refusal for scenarios that are no longer the ones G1 sealed says before naming them.
const SCENARIOS_CHANGED: &str = "Scenarios changed after G1:";

/// What a refusal for scenarios that are no longer the ones G1 sealed asks for.
const APPROVE_AGAIN: &str = "A human must approve the scenarios again";

/// Why a red change waits for its plan.
const PLAN_PENDING: &str = "Red change: a human must approve the plan (gate 'plan').";

/// Why the Reviewer must be of another family, in the new names: a project of the old layout
/// names its models file as it does.
const SAME_FAMILY: &str =
    "Reviewer and Builder are the same model family: edit tminus/models.toml.";

/// The role Path Guard judges the code of a relaunch for: no role may touch what `all` protects.
const RELAUNCH_ROLE: &str = "builder";

/// What preparing a change reads from the project.
#[derive(Debug, Clone, Copy)]
pub struct Setup<'a> {
    /// What the gates read: a relaunch runs G2.
    pub project: &'a gates::Project<'a>,
    /// The text of the zone map, whose red zones a human must have approved.
    pub zones: &'a str,
    /// The models of the project, for the family of the Builder and the Reviewer.
    pub models: &'a Models,
}

/// What the record of a prepared change keeps as its decision.
#[derive(Serialize)]
struct Started<'a> {
    #[serde(flatten)]
    decision: &'a Decision<'a>,
    resume_green: bool,
}

/// What `tminus prepare` prints.
#[derive(Serialize)]
struct Prepared<'a> {
    ok: bool,
    #[serde(flatten)]
    started: &'a Started<'a>,
}

/// Prepares change `id`, whose decision is `decision` (see the module). A refusal is an error
/// that says why, naming the files and the agent commands of the project as it names them (see
/// [`crate::layout`]). Once started, the record keeps the decision with `resume_green`, the start
/// is logged, and what it prints, as JSON on one line, is `ok` and that decision.
pub fn prepare(setup: &Setup, id: &str, decision: &Decision) -> Result<String, String> {
    let state = setup.project.state;
    let layout = Layout::at(setup.project.root);
    if !approve::g0_open(state).map_err(|e| e.to_string())? {
        return Err(layout.localize(G0_PENDING));
    }
    let named = |e: approve::Error| e.named_in(setup.project.root).to_string();
    let zones = approve::zones_state(state, setup.zones).map_err(named)?;
    let spike = zones == ZonesState::NotApproved && decision.kind == "Spike";
    if !zones.approved() && !spike {
        let why = layout.localize(zones.message());
        return Err(format!("{why} {APPROVE_ZONES}"));
    }
    let text = change::read_record(state, id)?;
    let path = verdict::change_file(state, id, "change.json");
    let invalid = |e: serde_json::Error| format!("{path}: {e}");
    let record: Value = serde_json::from_str(&text).map_err(invalid)?;
    let approved = |gate: &str| record["gates"][gate]["decision"] == "ok";
    let classification = decision.classification;
    if classification.g1 && !approved("G1") {
        return Err(layout.localize(G1_PENDING));
    }
    if classification.human_plan && !approved("plan") {
        return Err(PLAN_PENDING.to_string());
    }
    let accepted = same_family(setup.models, decision);
    let accepted = accepted.map_err(|why| layout.localize(&why))?;
    let git = Git::new(setup.project.root);
    let branch = change::branch(id);
    if git.current_branch().map_err(|e| e.to_string())? != branch {
        return Err(format!(
            "Not on branch {branch}. Launch the change with tminus launch {id}."
        ));
    }
    if classification.g1 {
        let record = Record::from_json(&text).map_err(invalid)?;
        sealed(setup.project.root, id, &record, &classification.features)?;
    }
    let (resume_green, start) = if built_before(state, id)? {
        let record = Record::from_json(&text).map_err(invalid)?;
        relaunched(setup.project, id, &record, decision.kind, &git)?
    } else {
        (false, git.head().map_err(|e| e.to_string())?)
    };
    let started = Started {
        decision,
        resume_green,
    };
    let stored = serde_json::to_value(&started).map_err(|e| e.to_string())?;
    let fields = vec![
        ("decision", stored),
        ("state", Value::from("building")),
        ("build_start", Value::from(start)),
    ];
    let ts = log::now();
    let updated = change::with_requests_resolved(&text, &ts)
        .and_then(|text| change::with_fields(&text, fields))
        .and_then(|text| with_decided(text, accepted.as_deref(), &record, &ts))
        .map_err(invalid)?;
    change::write(state, &change::record_path(id), &updated)?;
    // The snapshots belong to the earlier build.
    for snapshot in [
        "snapshot.json",
        "snapshot-build.json",
        "snapshot-parked.json",
    ] {
        remove(state, &verdict::change_file(state, id, snapshot))?;
    }
    let mut note = format!("{} · {}", classification.color, classification.lane);
    if resume_green {
        note.push_str(" · G2 already green: Builder skipped");
    }
    let entry = log::Entry {
        change: id.to_string(),
        phase: "2-build".to_string(),
        gate: "-".to_string(),
        role: "machine".to_string(),
        agent: "-".to_string(),
        result: "prepared".to_string(),
        duration_s: 0.0,
        note,
    };
    change::logged(state, &entry)?;
    let prepared = Prepared {
        ok: true,
        started: &started,
    };
    serde_json::to_string(&prepared).map_err(|e| e.to_string())
}

/// `Ok` when the scenario files of change `id` of the project at `root`, whose record is `record`
/// and whose scenario files are `features`, are still the ones G1 sealed (see
/// [`approve::unsealed`]). Otherwise a refusal that names each file and what happened to it, and
/// asks a human to approve the scenarios again.
fn sealed(root: &Path, id: &str, record: &Record, features: &[String]) -> Result<(), String> {
    let drifted = approve::unsealed(root, id, record, features).map_err(|e| e.to_string())?;
    if drifted.is_empty() {
        return Ok(());
    }
    let named: Vec<String> = drifted
        .iter()
        .map(|(file, drift)| format!("{file} ({drift})"))
        .collect();
    Err(format!(
        "{SCENARIOS_CHANGED} {}. {APPROVE_AGAIN} (tminus gate G1 ok {id} --by human).",
        named.join(", ")
    ))
}

/// The `decided` note for the Decision Inbox when the Reviewer of `decision` is of the Builder's
/// model family, as `models` says, and the human accepted it at the Setup. `None` when the change
/// has no Reviewer or its Reviewer is of another family. A Reviewer of the Builder's family that
/// nobody accepted is a refusal.
fn same_family(models: &Models, decision: &Decision) -> Result<Option<String>, String> {
    let roles = decision.roles;
    let same = models.family(&roles.reviewer_model) == models.family(&roles.builder);
    if !(decision.classification.reviewer && same) {
        return Ok(None);
    }
    let Some(day) = models::same_family_accepted(&roles.reviewer_model) else {
        return Err(SAME_FAMILY.to_string());
    };
    let note = format!(
        "Reviewer of the same model family as the Builder (accepted at the Setup on {day})."
    );
    Ok(Some(note))
}

/// The record `text` with the `decided` note `note` left at `ts`, when there is one and
/// `record`, the record before this start, does not have it yet: a relaunch says it once.
fn with_decided(
    text: String,
    note: Option<&str>,
    record: &Value,
    ts: &str,
) -> Result<String, serde_json::Error> {
    let notes = record["inbox"].as_array();
    let said = |note: &str| {
        let decided = |n: &Value| n["kind"] == "decided" && n["text"] == note;
        notes.is_some_and(|notes| notes.iter().any(decided))
    };
    match note {
        Some(note) if !said(note) => {
            let decided = change::Note {
                kind: "decided",
                text: note,
                ts,
            };
            change::with_note(&text, &decided)
        }
        _ => Ok(text),
    }
}

/// Whether the log of the project whose state root is `state` has a call to the Builder in build
/// mode for change `id`: the change ran before. No log, no call.
fn built_before(state: &Path, id: &str) -> Result<bool, String> {
    let path = layout::named(state, log::PATH);
    let text = match fs::read_to_string(state.join(&path)) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(format!("cannot read {path}: {e}")),
    };
    let records = log::parse(&text).map_err(|e| format!("{path}, {e}"))?;
    let (change, role) = (Value::from(id), Value::from(BUILD_ROLE));
    let built = |record: &log::Record| {
        record.fields.get("change") == Some(&change) && record.fields.get("role") == Some(&role)
    };
    Ok(records.iter().any(built))
}

/// How the relaunch of change `id`, whose record is `record` and whose kind of work is `kind`,
/// starts: whether its code already passes G2, the one of its kind (see
/// [`gates::run_of_kind`]), and the commit its build starts at. Path Guard judges the code as a
/// Builder's from where its branch left the main branch, before anything runs: gate commands a
/// Builder rewrote never run. The run is saved and logged like any G2. The build starts at the
/// commit checked out, unless Path Guard failed: then it starts where the branch left the main
/// branch, so the next G2 still sees the protected paths the code touched, even the ones
/// committed. The code passes only when Path Guard for build mode from that start passes too
/// (see [`builder_may`]).
fn relaunched(
    project: &gates::Project,
    id: &str,
    record: &Record,
    kind: &str,
    git: &Git,
) -> Result<(bool, String), String> {
    let (main, branch) = (&project.settings.main_branch, change::branch(id));
    let left = git.merge_base(main, &branch).map_err(|e| e.to_string())?;
    let judged = Record {
        build_start: Some(left.clone()),
        ..record.clone()
    };
    let ran = gates::run_of_kind(project, Gate::G2, id, &judged, kind, RELAUNCH_ROLE, None);
    let report = ran.map_err(|e| e.to_string())?;
    gates::save(project.state, id, &report)?;
    let guarded = report.failures.iter().any(|f| f.name == gates::PATH_GUARD);
    let start = if guarded {
        left
    } else {
        git.head().map_err(|e| e.to_string())?
    };
    let green = report.ok && builder_may(project, id, record, &start)?;
    Ok((green, start))
}

/// Whether Path Guard for build mode, without a snapshot, passes on change `id`, whose record is
/// `record`, from `start`, where its build starts again: G3 judges build mode from there, so work
/// left uncommitted that build mode may not touch (test mode's steps) runs the Builder instead of
/// parking at G3.
fn builder_may(
    project: &gates::Project,
    id: &str,
    record: &Record,
    start: &str,
) -> Result<bool, String> {
    let from_start = Record {
        build_start: Some(start.to_string()),
        ..record.clone()
    };
    let (root, settings) = (project.root, project.settings);
    let mut software = Software::new(root, &settings.main_branch);
    let role = BUILD_ROLE;
    let guarded = guard::path_guard(&mut software, root, id, &from_start, settings, role, None);
    Ok(guarded.map_err(|e| e.to_string())?.ok)
}

/// Removes the file `path`, as the project names it, under the state root `state`, if there is
/// one.
fn remove(state: &Path, path: &str) -> Result<(), String> {
    match fs::remove_file(state.join(path)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("cannot remove {path}: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::{classify_with, of_kind};
    use crate::gates::{Commands, Rules};
    use crate::git::tests::Repo;
    use crate::guard::Settings;
    use crate::zones::ZoneMap;

    const ZONES: &str = "default = \"yellow\"\n\
                         [[zone]]\ncolor = \"green\"\npaths = [\"docs/**\"]\n\
                         [[zone]]\ncolor = \"red\"\npaths = [\"src/storage/**\"]\n\
                         reason = \"Drafts\"\n";

    const RULES: &str = "[protected]\nall = [\"tminus/**\"]\n";

    const MODELS: &str = "[builder.yellow]\nintegration = \"claude\"\nmodel = \"sonnet\"\n\
                          [builder.red]\nintegration = \"claude\"\nmodel = \"opus\"\n\
                          [builder_test]\nintegration = \"claude\"\nmodel = \"sonnet\"\n\
                          [reviewer]\nintegration = \"codex\"\nmodel = \"gpt-6\"\n\
                          [orchestrator]\nintegration = \"claude\"\nmodel = \"opus\"\n\
                          [families]\n\"opus\" = \"anthropic\"\n\"sonnet\" = \"anthropic\"\n\
                          \"haiku\" = \"anthropic\"\n\"gpt\" = \"openai\"\n";

    const RECORD: &str = r#"{"id": "c1", "title": "App", "type": "Feature",
                             "paths": ["src/app.rs"], "state": "open", "gates": {}, "x": 1,
                             "inbox": [{"kind": "needs", "text": "Which format"}]}"#;

    /// A project in a git repository, its configuration committed on `main`, with change `c1`
    /// and its branch, which is not checked out.
    struct Fixture {
        repo: Repo,
        zones: ZoneMap,
        rules: Rules,
        settings: Settings,
        commands: Commands,
        models: Models,
    }

    fn fixture(record: &str) -> Fixture {
        fixture_with(record, "[g2]\nlint = \"true\"\n", MODELS)
    }

    fn fixture_with(record: &str, commands: &str, models: &str) -> Fixture {
        let repo = Repo::new();
        repo.write("tminus/rules.toml", RULES);
        repo.write("tminus/zones.toml", ZONES);
        repo.write("tminus/commands.toml", commands);
        repo.commit();
        repo.git(&["branch", "change/c1"]);
        repo.write(".tminus/changes/c1/change.json", record);
        Fixture {
            repo,
            zones: ZoneMap::from_toml(ZONES).unwrap(),
            rules: Rules::from_toml(RULES).unwrap(),
            settings: Settings::from_toml(RULES).unwrap(),
            commands: Commands::from_toml(commands).unwrap(),
            models: Models::from_toml(models).unwrap(),
        }
    }

    impl Fixture {
        fn root(&self) -> &Path {
            self.repo.root()
        }

        /// G0 open, the red zones approved, and the branch of the change checked out.
        fn ready(&self) {
            self.setup_done();
            self.zones_approved();
            self.repo.git(&["checkout", "-q", "change/c1"]);
        }

        fn setup_done(&self) {
            self.repo.write(approve::G0_PATH, r#"{"decision": "ok"}"#);
        }

        fn zones_approved(&self) {
            let hash = approve::red_hash(&approve::red_zones(ZONES).unwrap());
            let approval = format!(r#"{{"decision": "ok", "red_hash": "{hash}"}}"#);
            self.repo.write(approve::ZONES_PATH, &approval);
        }

        /// Writes `text` to `path` and commits that file alone on the branch checked out.
        fn committed(&self, path: &str, text: &str) {
            self.repo.write(path, text);
            self.repo.git(&["add", path]);
            self.repo.git(&["commit", "-q", "-m", "work"]);
        }

        /// Records G1 ok on `c1`, by a human, sealing its scenario files as they are.
        fn g1_approved(&self) {
            let read = approve::Project {
                root: self.root(),
                state: self.root(),
                zones: ZONES,
                settings: &self.settings,
                acceptance: &self.rules.table.acceptance,
            };
            let request = approve::Request {
                gate: approve::Gate::G1,
                decision: "ok",
                change: Some("c1"),
                by: "human",
                note: "",
            };
            if let Err(e) = approve::record(&read, &request, None, |_| {}) {
                panic!("G1 not recorded: {e}");
            }
        }

        /// What the gates read from this project.
        fn gates(&self) -> gates::Project<'_> {
            gates::Project {
                root: self.root(),
                state: self.root(),
                zones: &self.zones,
                rules: &self.rules,
                settings: &self.settings,
                commands: &self.commands,
            }
        }

        fn prepare(&self) -> Result<String, String> {
            let text = change::read_record(self.root(), "c1").unwrap();
            let record = Record::from_json(&text).unwrap();
            let table = &self.rules.table;
            let classification =
                classify_with(&self.zones, table, &record.plan, &record.features).unwrap();
            let kind = change::kind(&text).unwrap();
            let classification = of_kind(classification, &kind);
            let roles = self.models.roles(classification.color);
            let decision = Decision {
                change: "c1",
                classification: &classification,
                kind: &kind,
                roles: &roles,
                builder_session: self.models.builder_session(classification.color),
                retries: 3,
            };
            let project = self.gates();
            let setup = Setup {
                project: &project,
                zones: ZONES,
                models: &self.models,
            };
            prepare(&setup, "c1", &decision)
        }

        fn refused(&self) -> String {
            match self.prepare() {
                Ok(out) => panic!("prepared: {out}"),
                Err(e) => e,
            }
        }

        fn prepared(&self) -> Value {
            match self.prepare() {
                Ok(out) => serde_json::from_str(&out).unwrap(),
                Err(e) => panic!("refused: {e}"),
            }
        }

        fn record(&self) -> Value {
            let text = change::read_record(self.root(), "c1").unwrap();
            serde_json::from_str(&text).unwrap()
        }

        fn log(&self) -> Vec<log::Record> {
            let text = fs::read_to_string(self.root().join(log::PATH)).unwrap_or_default();
            log::parse(&text).unwrap()
        }

        fn builder_called(&self, id: &str) {
            let line = format!(r#"{{"change": "{id}", "role": "builder-build"}}"#);
            log::append(self.root(), &line).unwrap();
        }
    }

    #[test]
    fn without_g0_no_change_starts() {
        let project = fixture(RECORD);
        project.zones_approved();
        project.repo.git(&["checkout", "-q", "change/c1"]);
        assert_eq!(project.refused(), G0_PENDING);
        assert_eq!(
            G0_PENDING,
            "G0 pending: run the Setup (the tminus-setup command)."
        );
        let closed = r#"{"decision": "ko"}"#;
        project.repo.write(approve::G0_PATH, closed);
        assert_eq!(project.refused(), G0_PENDING);
        assert_eq!(project.record()["state"], "open");
    }

    #[test]
    fn without_approved_red_zones_only_a_spike_starts() {
        let project = fixture(RECORD);
        project.setup_done();
        project.repo.git(&["checkout", "-q", "change/c1"]);
        let not_approved = format!("{} {APPROVE_ZONES}", ZonesState::NotApproved.message());
        assert_eq!(project.refused(), not_approved);
        assert!(not_approved.starts_with("Red zones not approved"));
        assert_eq!(APPROVE_ZONES, "(tminus gate zones ok --by human)");
        let spike = RECORD.replace("\"Feature\"", "\"Spike\"");
        project.repo.write(".tminus/changes/c1/change.json", &spike);
        assert_eq!(project.prepared()["type"], "Spike");
        // Red zones that changed after their approval stop spikes too.
        let changed = r#"{"decision": "ok", "red_hash": "x"}"#;
        project.repo.write(approve::ZONES_PATH, changed);
        let changed = format!("{} {APPROVE_ZONES}", ZonesState::Changed.message());
        assert_eq!(project.refused(), changed);
    }

    #[test]
    fn new_or_changed_scenarios_wait_for_g1() {
        let planned = RECORD.replace("\"src/app.rs\"", "\"src/app.rs\", \"features/a.feature\"");
        let project = fixture(&planned);
        project.ready();
        assert_eq!(project.refused(), G1_PENDING);
        let ko = planned.replace("\"gates\": {}", r#""gates": {"G1": {"decision": "ko"}}"#);
        project.repo.write(".tminus/changes/c1/change.json", &ko);
        assert_eq!(project.refused(), G1_PENDING);
        let ok = planned.replace("\"gates\": {}", r#""gates": {"G1": {"decision": "ok"}}"#);
        project.repo.write(".tminus/changes/c1/change.json", &ok);
        assert_eq!(project.prepared()["g1"], true);
        // Scenarios sealed in G1 count even when the paths do not show them.
        let features = r#""features": ["features/hidden/a.feature"], "gates": {}"#;
        let sealed = RECORD.replace("\"gates\": {}", features);
        project
            .repo
            .write(".tminus/changes/c1/change.json", &sealed);
        assert_eq!(project.refused(), G1_PENDING);
    }

    /// A G1 approved that sealed `features/a.feature` and `features/c.feature` with texts no file
    /// has.
    const SEALED: &str = concat!(
        r#""features": ["features/c.feature"], "gates": {"G1": {"decision": "ok", "#,
        r#""hashes": {"features/a.feature": "x", "features/c.feature": "y"}}}"#
    );

    #[test]
    fn scenarios_changed_after_g1_stop_the_start_and_leave_the_record_untouched() {
        let paths = "\"src/app.rs\", \"features/a.feature\", \"features/b.feature\"";
        let record = RECORD
            .replace("\"src/app.rs\"", paths)
            .replace("\"gates\": {}", SEALED);
        let project = fixture(&record);
        project.ready();
        project.repo.write("features/a.feature", "Feature: A\n");
        project.repo.write("features/b.feature", "Feature: B\n");
        let snapshot = change::snapshot_path("c1");
        project.repo.write(&snapshot, "{}");
        let before = change::read_record(project.root(), "c1").unwrap();
        let expected = "Scenarios changed after G1: features/a.feature (edited), \
                        features/b.feature (added), features/c.feature (removed). A human must \
                        approve the scenarios again (tminus gate G1 ok c1 --by human).";
        assert_eq!(project.refused(), expected);
        assert_eq!(change::read_record(project.root(), "c1").unwrap(), before);
        assert!(project.log().is_empty());
        assert!(project.root().join(&snapshot).exists());
        // A new G1 seals them as they are, and the change starts as before.
        project.g1_approved();
        assert_eq!(project.prepared()["g1"], true);
        assert_eq!(project.record()["state"], "building");
    }

    #[test]
    fn a_scenario_main_has_that_the_change_plans_but_did_not_touch_holds_its_approval() {
        let paths = "\"src/app.rs\", \"features/base.feature\"";
        let project = fixture(&RECORD.replace("\"src/app.rs\"", paths));
        // Main has it before the branch leaves it, and the branch adds another scenario.
        project.committed("features/base.feature", "Feature: Base\n");
        project.repo.git(&["branch", "-f", "change/c1"]);
        project.ready();
        project.committed("features/app.feature", "Feature: App\n");
        project.g1_approved();
        let out = project.prepared();
        assert_eq!(out["g1"], true);
        assert_eq!(project.record()["state"], "building");
        // Edited after G1, it no longer holds.
        project.committed("features/base.feature", "Feature: Base edited\n");
        let expected = "Scenarios changed after G1: features/base.feature (edited).";
        assert!(project.refused().starts_with(expected));
    }

    #[test]
    fn a_change_without_g1_starts_whatever_its_scenarios_say() {
        // The seal names files, but the change has no scenarios of its own.
        let sealed = SEALED.replace(r#""features": ["features/c.feature"], "#, "");
        let project = fixture(&RECORD.replace("\"gates\": {}", &sealed));
        project.ready();
        project.repo.write("features/a.feature", "Feature: A\n");
        assert_eq!(project.prepared()["g1"], false);
        // A spike has no G1.
        let planned = "\"src/app.rs\", \"features/a.feature\", \"features/b.feature\"";
        let spike = RECORD
            .replace("\"Feature\"", "\"Spike\"")
            .replace("\"src/app.rs\"", planned)
            .replace("\"gates\": {}", SEALED);
        project.repo.write(".tminus/changes/c1/change.json", &spike);
        project.repo.write("features/b.feature", "Feature: B\n");
        let out = project.prepared();
        assert_eq!(out["type"], "Spike");
        assert_eq!(out["g1"], false);
        assert_eq!(project.record()["state"], "building");
    }

    #[test]
    fn a_red_change_waits_for_its_plan_and_gets_the_strong_builder() {
        let red = RECORD.replace("src/app.rs", "src/storage/db.rs");
        let project = fixture(&red);
        project.ready();
        assert_eq!(project.refused(), PLAN_PENDING);
        assert_eq!(
            PLAN_PENDING,
            "Red change: a human must approve the plan (gate 'plan')."
        );
        let ko = red.replace("\"gates\": {}", r#""gates": {"plan": {"decision": "ko"}}"#);
        project.repo.write(".tminus/changes/c1/change.json", &ko);
        assert_eq!(project.refused(), PLAN_PENDING);
        let ok = red.replace("\"gates\": {}", r#""gates": {"plan": {"decision": "ok"}}"#);
        project.repo.write(".tminus/changes/c1/change.json", &ok);
        let out = project.prepared();
        assert_eq!(out["color"], "red");
        assert_eq!(out["review"], "deep");
        let opus = serde_json::json!({"integration": "claude", "model": "opus"});
        assert_eq!(out["builder"], opus);
    }

    #[test]
    fn the_reviewer_must_be_of_another_family_when_there_is_one() {
        let reviewer = "integration = \"claude\"\nmodel = \"haiku\"";
        let haiku = MODELS.replace("integration = \"codex\"\nmodel = \"gpt-6\"", reviewer);
        let project = fixture_with(RECORD, "[g2]\nlint = \"true\"\n", &haiku);
        project.ready();
        assert_eq!(project.refused(), SAME_FAMILY);
        assert_eq!(
            SAME_FAMILY,
            "Reviewer and Builder are the same model family: edit tminus/models.toml."
        );
        // A change without a Reviewer does not care.
        let docs = RECORD.replace("\"src/app.rs\"", "\"docs/a.md\"");
        let docs = docs.replace("\"x\": 1", "\"resumable\": true");
        project.repo.write(".tminus/changes/c1/change.json", &docs);
        let out = project.prepared();
        assert_eq!(out["color"], "green");
        assert_eq!(out["reviewer"], false);
    }

    /// The `decided` notes of the record of `c1`.
    fn decided(project: &Fixture) -> Vec<Value> {
        let record = project.record();
        let notes = record["inbox"].as_array().unwrap().iter();
        let decided = notes.filter(|n| n["kind"] == "decided");
        decided.cloned().collect()
    }

    #[test]
    fn a_reviewer_of_the_same_family_the_human_accepted_starts_and_the_inbox_says_so() {
        let reviewer = "integration = \"claude\"\nmodel = \"haiku\"\n\
                        same_family_accepted = \"2026-10-27\"";
        let haiku = MODELS.replace("integration = \"codex\"\nmodel = \"gpt-6\"", reviewer);
        let other = r#"{"kind": "decided", "text": "Other", "ts": "x"}"#;
        let record = RECORD.replace("[{", &format!("[{other}, {{"));
        let project = fixture_with(&record, "[g2]\nlint = \"true\"\n", &haiku);
        project.ready();
        let out = project.prepared();
        let reviewer = &out["reviewer_model"];
        assert_eq!(reviewer["same_family_accepted"], "2026-10-27");
        let note = "Reviewer of the same model family as the Builder (accepted at the Setup on \
                    2026-10-27).";
        let notes = decided(&project);
        assert_eq!(notes.len(), 2, "{notes:?}");
        assert_eq!(notes[1]["text"], note);
        assert!(notes[1]["ts"].as_str().unwrap().ends_with("+00:00"));
        // A relaunch says it once.
        project.prepared();
        assert_eq!(decided(&project), notes);
        // A Reviewer of another family needs no note, accepted or not.
        let gpt = "model = \"gpt-6\"\nsame_family_accepted = \"2026-10-27\"";
        let accepted = MODELS.replace("model = \"gpt-6\"", gpt);
        let fresh = fixture_with(&record, "[g2]\nlint = \"true\"\n", &accepted);
        fresh.ready();
        fresh.prepared();
        assert_eq!(decided(&fresh).len(), 1);
    }

    #[test]
    fn a_project_set_up_before_the_rename_is_refused_in_its_own_names() {
        let planned = RECORD.replace("\"src/app.rs\"", "\"src/app.rs\", \"features/a.feature\"");
        let project = fixture(&planned);
        project.repo.write("gdd/project.toml", "");
        project.repo.write(".gdd/changes/c1/change.json", &planned);
        let g0 = "G0 pending: run the Setup (the gdd-setup command).";
        assert_eq!(project.refused(), g0);
        project.repo.write(".gdd/g0.json", r#"{"decision": "ok"}"#);
        let zones = "Red zones not approved yet: the Setup asks you to approve them \
                     (the gdd-setup command). (tminus gate zones ok --by human)";
        assert_eq!(project.refused(), zones);
        let hash = approve::red_hash(&approve::red_zones(ZONES).unwrap());
        let approval = format!(r#"{{"decision": "ok", "red_hash": "{hash}"}}"#);
        project.repo.write(".gdd/zones.json", &approval);
        project.repo.git(&["checkout", "-q", "change/c1"]);
        let g1 = "New or changed scenarios without G1: a human must approve them \
                  (the gdd-change command).";
        assert_eq!(project.refused(), g1);
        assert!(!project.root().join(".tminus/g0.json").exists());
        // The log and the snapshots are read and removed in .gdd.
        let ok = planned.replace("\"gates\": {}", r#""gates": {"G1": {"decision": "ok"}}"#);
        project.repo.write(".gdd/changes/c1/change.json", &ok);
        let snapshot = ".gdd/changes/c1/snapshot.json";
        project.repo.write(snapshot, "{}");
        assert_eq!(project.prepared()["ok"], true);
        assert!(!project.root().join(snapshot).exists());
        let log = fs::read_to_string(project.root().join(".gdd/log.jsonl")).unwrap();
        assert_eq!(log.lines().count(), 1);
    }

    #[test]
    fn a_change_starts_on_its_own_branch() {
        let project = fixture(RECORD);
        project.ready();
        project.repo.git(&["checkout", "-q", "main"]);
        let expected = "Not on branch change/c1. Launch the change with tminus launch c1.";
        assert_eq!(project.refused(), expected);
        assert_eq!(project.record()["state"], "open");
        assert!(project.log().is_empty());
    }

    #[test]
    fn prepare_starts_the_build_and_names_the_model_of_each_role() {
        let project = fixture(RECORD);
        project.ready();
        project.repo.write(&change::snapshot_path("c1"), "{}");
        let build = change::build_snapshot_path("c1");
        project.repo.write(&build, "{}");
        let out = project.prepared();
        assert_eq!(out["ok"], true);
        assert_eq!(out["change"], "c1");
        assert_eq!(out["type"], "Feature");
        assert_eq!(out["color"], "yellow");
        assert_eq!(out["lane"], "full");
        assert_eq!(out["review"], "quick");
        let sonnet = serde_json::json!({"integration": "claude", "model": "sonnet"});
        assert_eq!(out["builder"], sonnet);
        assert_eq!(out["builder_test"], sonnet);
        let gpt = serde_json::json!({"integration": "codex", "model": "gpt-6"});
        assert_eq!(out["reviewer_model"], gpt);
        assert_eq!(out["orchestrator"]["model"], "opus");
        assert_eq!(out["retries"], 3);
        assert_eq!(out["resume_green"], false);
        let record = project.record();
        assert_eq!(record["state"], "building");
        assert_eq!(record["x"], 1);
        let head = project.repo.git(&["rev-parse", "HEAD"]);
        assert_eq!(record["build_start"], head.as_str());
        let mut stored = out.clone();
        stored.as_object_mut().unwrap().remove("ok");
        assert_eq!(record["decision"], stored);
        let request = &record["inbox"][0];
        assert_eq!(request["text"], "Which format");
        assert!(request["resolved"].as_str().unwrap().ends_with("+00:00"));
        let folder = project.root().join(".tminus/changes/c1");
        assert!(!folder.join("snapshot.json").exists());
        assert!(!folder.join("snapshot-build.json").exists());
        assert!(!folder.join("g2.json").exists(), "no Builder ran before");
        let log = project.log();
        let [line] = log.as_slice() else {
            panic!("one log line: {log:?}");
        };
        for (field, value) in [
            ("change", "c1"),
            ("phase", "2-build"),
            ("gate", "-"),
            ("role", "machine"),
            ("agent", "-"),
            ("result", "prepared"),
            ("note", "yellow · full"),
        ] {
            assert_eq!(line.text(field), value, "{field}");
        }
        // The record now says where the build started.
        let prepared = Record::from_json(&change::read_record(project.root(), "c1").unwrap());
        assert_eq!(prepared.unwrap().build_start, Some(head));
    }

    #[test]
    fn a_relaunch_whose_code_already_passes_g2_skips_the_builder() {
        let project = fixture(RECORD);
        project.ready();
        project.builder_called("c2");
        assert_eq!(project.prepared()["resume_green"], false);
        assert!(!project.root().join(".tminus/changes/c1/g2.json").exists());
        project.builder_called("c1");
        project.repo.write("src/app.rs", "fn main() {}\n");
        let out = project.prepared();
        assert_eq!(out["resume_green"], true);
        assert_eq!(project.record()["decision"]["resume_green"], true);
        let saved = fs::read_to_string(project.root().join(".tminus/changes/c1/g2.json")).unwrap();
        let saved: Value = serde_json::from_str(&saved).unwrap();
        assert_eq!(saved["ok"], true);
        assert_eq!(saved["passed"], serde_json::json!(["lint", "path-guard"]));
        let log = project.log();
        let gates: Vec<String> = log.iter().map(|line| line.text("gate")).collect();
        assert_eq!(gates[gates.len() - 2..], ["G2", "-"]);
        let note = log[log.len() - 1].text("note");
        assert_eq!(note, "yellow · full · G2 already green: Builder skipped");
    }

    #[test]
    fn the_snapshot_of_the_earlier_park_goes_with_the_earlier_build() {
        let project = fixture(RECORD);
        project.ready();
        let parked = change::parked_snapshot_path("c1");
        project.repo.write(&parked, "{}");
        assert_eq!(project.prepared()["ok"], true);
        assert!(!project.root().join(&parked).exists());
        // One that cannot go stops the start.
        fs::create_dir_all(project.root().join(&parked).join("x")).unwrap();
        let err = project.refused();
        let start = format!("cannot remove {parked}: ");
        assert!(err.starts_with(&start), "{err}");
    }

    #[test]
    fn a_relaunch_skips_the_builder_only_when_build_mode_may_have_written_its_work() {
        let project = fixture(RECORD);
        project.ready();
        project.builder_called("c1");
        let steps = "[protected]\nall = [\"tminus/**\"]\nbuilder_build = [\"tests/steps/**\"]\n";
        let project = Fixture {
            settings: Settings::from_toml(steps).unwrap(),
            ..project
        };
        // Work left uncommitted that build mode may write: G2 green is enough.
        project.repo.write("docs/brief.md", "a brief\n");
        assert_eq!(project.prepared()["resume_green"], true);
        // A step left uncommitted, which build mode may not touch, runs the Builder.
        project.repo.write("tests/steps/a.txt", "a step\n");
        assert_eq!(project.prepared()["resume_green"], false);
        let saved = fs::read_to_string(project.root().join(".tminus/changes/c1/g2.json")).unwrap();
        let saved: Value = serde_json::from_str(&saved).unwrap();
        assert_eq!(saved["ok"], true, "the G2 of the relaunch passed");
        let head = project.repo.git(&["rev-parse", "HEAD"]);
        assert_eq!(project.record()["build_start"], head.as_str());
    }

    #[test]
    fn a_relaunched_spike_is_judged_by_the_g2_of_a_spike() {
        let spike = RECORD.replace("\"Feature\"", "\"Spike\"");
        let project = fixture_with(&spike, "[g2]\nlint = \"touch ran\"\n", MODELS);
        project.ready();
        project.builder_called("c1");
        project.repo.write("src/app.rs", "fn main() {}\n");
        assert_eq!(project.prepared()["resume_green"], false);
        project.repo.write("docs/adr/0002-x.md", "## Decision\n");
        assert_eq!(project.prepared()["resume_green"], true);
        assert!(!project.root().join("ran").exists(), "a G2 command ran");
        let saved = fs::read_to_string(project.root().join(".tminus/changes/c1/g2.json")).unwrap();
        let saved: Value = serde_json::from_str(&saved).unwrap();
        assert_eq!(saved["passed"], serde_json::json!(["path-guard", "adr"]));
    }

    #[test]
    fn a_relaunch_whose_code_fails_g2_goes_back_to_the_builder() {
        let project = fixture_with(RECORD, "[g2]\nlint = \"exit 1\"\n", MODELS);
        project.ready();
        project.builder_called("c1");
        project.committed("src/app.rs", "fn main() {}\n");
        assert_eq!(project.prepared()["resume_green"], false);
        let saved = fs::read_to_string(project.root().join(".tminus/changes/c1/g2.json")).unwrap();
        assert!(saved.contains("\"ok\": false"), "{saved}");
        let note = project.log().pop().unwrap().text("note");
        assert_eq!(note, "yellow · full");
        // Nothing protected was touched: the build starts at the commit checked out.
        let head = project.repo.git(&["rev-parse", "HEAD"]);
        assert_eq!(project.record()["build_start"], head.as_str());
    }

    #[test]
    fn a_relaunch_keeps_in_sight_the_protected_paths_a_builder_committed() {
        let project = fixture(RECORD);
        project.ready();
        project.builder_called("c1");
        let left = project.repo.git(&["rev-parse", "HEAD"]);
        let rewritten = "[g2]\nlint = \"touch ran\"\n";
        project.committed("tminus/commands.toml", rewritten);
        let project = Fixture {
            commands: Commands::from_toml(rewritten).unwrap(),
            ..project
        };
        assert_eq!(project.prepared()["resume_green"], false);
        assert_eq!(project.record()["build_start"], left.as_str());
        // The next G2 still sees the rewrite, and runs nothing.
        let text = change::read_record(project.root(), "c1").unwrap();
        let record = Record::from_json(&text).unwrap();
        let read = project.gates();
        let report = gates::run(&read, Gate::G2, "c1", &record, BUILD_ROLE, None).unwrap();
        assert!(!report.ok);
        let guarded = &report.failures[0];
        assert_eq!(guarded.name, gates::PATH_GUARD);
        let start = "tminus/commands.toml: protected path for builder-build";
        assert!(guarded.tail.starts_with(start), "{}", guarded.tail);
        assert!(!project.root().join("ran").exists());
    }

    #[test]
    fn a_relaunch_never_runs_gate_commands_a_builder_rewrote() {
        let project = fixture(RECORD);
        project.ready();
        project.builder_called("c1");
        // The tool reads the commands from the working tree, where the Builder rewrote them.
        let rewritten = "[g2]\nlint = \"touch ran\"\n";
        project.repo.write("tminus/commands.toml", rewritten);
        let project = Fixture {
            commands: Commands::from_toml(rewritten).unwrap(),
            ..project
        };
        assert_eq!(project.prepared()["resume_green"], false);
        assert!(!project.root().join("ran").exists());
        let saved = fs::read_to_string(project.root().join(".tminus/changes/c1/g2.json")).unwrap();
        let guarded = "tminus/commands.toml: protected path for builder";
        assert!(saved.contains(guarded), "{saved}");
    }

    #[test]
    fn what_cannot_be_read_or_written_stops_the_start() {
        let project = fixture(RECORD);
        project.ready();
        project.repo.write(log::PATH, "{not json\n");
        let err = project.refused();
        assert!(err.starts_with(".tminus/log.jsonl, line 1: "), "{err}");
        fs::remove_file(project.root().join(log::PATH)).unwrap();
        // A log that cannot be read is not a log without calls.
        fs::create_dir_all(project.root().join(log::PATH)).unwrap();
        let err = project.refused();
        assert!(err.starts_with("cannot read .tminus/log.jsonl: "), "{err}");
        assert_eq!(project.record()["state"], "open");
        fs::remove_dir(project.root().join(log::PATH)).unwrap();
        fs::create_dir_all(project.root().join(".tminus/changes/c1/snapshot.json/x")).unwrap();
        let err = project.refused();
        let start = "cannot remove .tminus/changes/c1/snapshot.json: ";
        assert!(err.starts_with(start), "{err}");
        fs::remove_dir_all(project.root().join(".tminus/changes/c1/snapshot.json")).unwrap();
        project.repo.write(".tminus/changes/c1/change.json", "[]");
        let err = project.refused();
        let start = ".tminus/changes/c1/change.json: ";
        assert!(err.starts_with(start), "{err}");
    }
}
