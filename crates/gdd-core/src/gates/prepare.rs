//! Preparing a change (`gdd prepare`), the first step of the recipe: it checks that the change
//! may enter Build, in this order: G0, the red zones (only a spike starts before a human
//! approves them), G1 for new or changed scenarios, the plan of a red change, a Reviewer of
//! another model family than the Builder (or of the same one, when the human accepted it at the
//! Setup: the Decision Inbox says so), and its own branch checked out. Then it starts the build:
//! its decision in the record, state `building`, the build starting at the commit checked out,
//! the requests of the earlier run resolved and the snapshots of the earlier build removed.
//!
//! A relaunch whose code already passes G2 skips the Builder (`resume_green`): it would only
//! spend tokens. A relaunch whose code touched protected paths starts its build where its branch
//! left the main branch instead: starting at the commit checked out would hide them.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::agents::models::{self, Models};
use crate::change::{self, Decision, Record};
use crate::gates::approve::{self, ZonesState};
use crate::gates::{self, BUILD_ROLE, Gate};
use crate::git::Git;
use crate::log;

/// Why a change does not start before the Setup.
const G0_PENDING: &str = "G0 pending: run the Setup (the gdd-setup command).";

/// How a human approves the red zones.
const APPROVE_ZONES: &str = "(bin/gdd gate zones ok --by human)";

/// Why a change with new or changed scenarios waits for G1.
const G1_PENDING: &str =
    "New or changed scenarios without G1: a human must approve them (the gdd-change command).";

/// Why a red change waits for its plan.
const PLAN_PENDING: &str = "Red change: a human must approve the plan (gate 'plan').";

/// Why the Reviewer must be of another family.
const SAME_FAMILY: &str = "Reviewer and Builder are the same model family: edit gdd/models.toml.";

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

/// What `gdd prepare` prints.
#[derive(Serialize)]
struct Prepared<'a> {
    ok: bool,
    #[serde(flatten)]
    started: &'a Started<'a>,
}

/// Prepares change `id`, whose decision is `decision` (see the module). A refusal is an error
/// that says why. Once started, the record keeps the decision with `resume_green`, the start is
/// logged, and what it prints, as JSON on one line, is `ok` and that decision.
pub fn prepare(setup: &Setup, id: &str, decision: &Decision) -> Result<String, String> {
    let root = setup.project.root;
    if !approve::g0_open(root).map_err(|e| e.to_string())? {
        return Err(G0_PENDING.to_string());
    }
    let zones = approve::zones_state(root, setup.zones).map_err(|e| e.to_string())?;
    let spike = zones == ZonesState::NotApproved && decision.kind == "Spike";
    if !zones.approved() && !spike {
        return Err(format!("{} {APPROVE_ZONES}", zones.message()));
    }
    let text = change::read_record(root, id)?;
    let path = change::record_path(id);
    let invalid = |e: serde_json::Error| format!("{path}: {e}");
    let record: Value = serde_json::from_str(&text).map_err(invalid)?;
    let approved = |gate: &str| record["gates"][gate]["decision"] == "ok";
    let classification = decision.classification;
    if classification.g1 && !approved("G1") {
        return Err(G1_PENDING.to_string());
    }
    if classification.human_plan && !approved("plan") {
        return Err(PLAN_PENDING.to_string());
    }
    let accepted = same_family(setup.models, decision)?;
    let git = Git::new(root);
    let branch = change::branch(id);
    if git.current_branch().map_err(|e| e.to_string())? != branch {
        return Err(format!(
            "Not on branch {branch}. Launch the change with bin/gdd launch {id}."
        ));
    }
    let (resume_green, start) = if built_before(root, id)? {
        let record = Record::from_json(&text).map_err(invalid)?;
        relaunched(setup.project, id, &record, &git)?
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
    change::write(root, &path, &updated)?;
    // The snapshots belong to the earlier build.
    remove(root, &change::snapshot_path(id))?;
    remove(root, &change::build_snapshot_path(id))?;
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
    change::logged(root, &entry)?;
    let prepared = Prepared {
        ok: true,
        started: &started,
    };
    serde_json::to_string(&prepared).map_err(|e| e.to_string())
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

/// Whether the log of the project at `root` has a call to the Builder in build mode for change
/// `id`: the change ran before. No log, no call.
fn built_before(root: &Path, id: &str) -> Result<bool, String> {
    let text = match fs::read_to_string(root.join(log::PATH)) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(format!("cannot read {}: {e}", log::PATH)),
    };
    let records = log::parse(&text).map_err(|e| format!("{}, {e}", log::PATH))?;
    let (change, role) = (Value::from(id), Value::from(BUILD_ROLE));
    let built = |record: &log::Record| {
        record.fields.get("change") == Some(&change) && record.fields.get("role") == Some(&role)
    };
    Ok(records.iter().any(built))
}

/// How the relaunch of change `id`, whose record is `record`, starts: whether its code already
/// passes G2, and the commit its build starts at. Path Guard judges the code as a Builder's from
/// where its branch left the main branch, before anything runs: gate commands a Builder rewrote
/// never run. The run is saved and logged like any G2. The build starts at the commit checked
/// out, unless Path Guard failed: then it starts where the branch left the main branch, so the
/// next G2 still sees the protected paths the code touched, even the ones committed.
fn relaunched(
    project: &gates::Project,
    id: &str,
    record: &Record,
    git: &Git,
) -> Result<(bool, String), String> {
    let (main, branch) = (&project.settings.main_branch, change::branch(id));
    let left = git.merge_base(main, &branch).map_err(|e| e.to_string())?;
    let from_main = Record {
        build_start: Some(left.clone()),
        ..record.clone()
    };
    let report = gates::run(project, Gate::G2, id, &from_main, RELAUNCH_ROLE, None)
        .map_err(|e| e.to_string())?;
    gates::save(project.root, id, &report)?;
    let guarded = report.failures.iter().any(|f| f.name == gates::PATH_GUARD);
    let start = if guarded {
        left
    } else {
        git.head().map_err(|e| e.to_string())?
    };
    Ok((report.ok, start))
}

/// Removes the file `path` of the project at `root`, if there is one.
fn remove(root: &Path, path: &str) -> Result<(), String> {
    match fs::remove_file(root.join(path)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("cannot remove {path}: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::classify_with;
    use crate::gates::{Commands, Rules};
    use crate::git::tests::Repo;
    use crate::guard::Settings;
    use crate::zones::ZoneMap;

    const ZONES: &str = "default = \"yellow\"\n\
                         [[zone]]\ncolor = \"green\"\npaths = [\"docs/**\"]\n\
                         [[zone]]\ncolor = \"red\"\npaths = [\"src/storage/**\"]\n\
                         reason = \"Drafts\"\n";

    const RULES: &str = "[protected]\nall = [\"gdd/**\"]\n";

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
        repo.write("gdd/rules.toml", RULES);
        repo.write("gdd/zones.toml", ZONES);
        repo.write("gdd/commands.toml", commands);
        repo.commit();
        repo.git(&["branch", "change/c1"]);
        repo.write(".gdd/changes/c1/change.json", record);
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

        /// What the gates read from this project.
        fn gates(&self) -> gates::Project<'_> {
            gates::Project {
                root: self.root(),
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
            let roles = self.models.roles(classification.color);
            let kind = change::kind(&text).unwrap();
            let decision = Decision {
                change: "c1",
                classification: &classification,
                kind: &kind,
                roles: &roles,
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
            "G0 pending: run the Setup (the gdd-setup command)."
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
        assert_eq!(APPROVE_ZONES, "(bin/gdd gate zones ok --by human)");
        let spike = RECORD.replace("\"Feature\"", "\"Spike\"");
        project.repo.write(".gdd/changes/c1/change.json", &spike);
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
        project.repo.write(".gdd/changes/c1/change.json", &ko);
        assert_eq!(project.refused(), G1_PENDING);
        let ok = planned.replace("\"gates\": {}", r#""gates": {"G1": {"decision": "ok"}}"#);
        project.repo.write(".gdd/changes/c1/change.json", &ok);
        assert_eq!(project.prepared()["g1"], true);
        // Scenarios sealed in G1 count even when the paths do not show them.
        let features = r#""features": ["features/hidden/a.feature"], "gates": {}"#;
        let sealed = RECORD.replace("\"gates\": {}", features);
        project.repo.write(".gdd/changes/c1/change.json", &sealed);
        assert_eq!(project.refused(), G1_PENDING);
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
        project.repo.write(".gdd/changes/c1/change.json", &ko);
        assert_eq!(project.refused(), PLAN_PENDING);
        let ok = red.replace("\"gates\": {}", r#""gates": {"plan": {"decision": "ok"}}"#);
        project.repo.write(".gdd/changes/c1/change.json", &ok);
        let out = project.prepared();
        assert_eq!(out["color"], "red");
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
            "Reviewer and Builder are the same model family: edit gdd/models.toml."
        );
        // A change without a Reviewer does not care.
        let docs = RECORD.replace("\"src/app.rs\"", "\"docs/a.md\"");
        let docs = docs.replace("\"x\": 1", "\"resumable\": true");
        project.repo.write(".gdd/changes/c1/change.json", &docs);
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
    fn a_change_starts_on_its_own_branch() {
        let project = fixture(RECORD);
        project.ready();
        project.repo.git(&["checkout", "-q", "main"]);
        let expected = "Not on branch change/c1. Launch the change with bin/gdd launch c1.";
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
        let folder = project.root().join(".gdd/changes/c1");
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
        assert!(!project.root().join(".gdd/changes/c1/g2.json").exists());
        project.builder_called("c1");
        project.repo.write("src/app.rs", "fn main() {}\n");
        let out = project.prepared();
        assert_eq!(out["resume_green"], true);
        assert_eq!(project.record()["decision"]["resume_green"], true);
        let saved = fs::read_to_string(project.root().join(".gdd/changes/c1/g2.json")).unwrap();
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
    fn a_relaunch_whose_code_fails_g2_goes_back_to_the_builder() {
        let project = fixture_with(RECORD, "[g2]\nlint = \"exit 1\"\n", MODELS);
        project.ready();
        project.builder_called("c1");
        project.committed("src/app.rs", "fn main() {}\n");
        assert_eq!(project.prepared()["resume_green"], false);
        let saved = fs::read_to_string(project.root().join(".gdd/changes/c1/g2.json")).unwrap();
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
        project.committed("gdd/commands.toml", rewritten);
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
        let start = "gdd/commands.toml: protected path for builder-build";
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
        project.repo.write("gdd/commands.toml", rewritten);
        let project = Fixture {
            commands: Commands::from_toml(rewritten).unwrap(),
            ..project
        };
        assert_eq!(project.prepared()["resume_green"], false);
        assert!(!project.root().join("ran").exists());
        let saved = fs::read_to_string(project.root().join(".gdd/changes/c1/g2.json")).unwrap();
        let guarded = "gdd/commands.toml: protected path for builder";
        assert!(saved.contains(guarded), "{saved}");
    }

    #[test]
    fn what_cannot_be_read_or_written_stops_the_start() {
        let project = fixture(RECORD);
        project.ready();
        project.repo.write(log::PATH, "{not json\n");
        let err = project.refused();
        assert!(err.starts_with(".gdd/log.jsonl, line 1: "), "{err}");
        fs::remove_file(project.root().join(log::PATH)).unwrap();
        // A log that cannot be read is not a log without calls.
        fs::create_dir_all(project.root().join(log::PATH)).unwrap();
        let err = project.refused();
        assert!(err.starts_with("cannot read .gdd/log.jsonl: "), "{err}");
        assert_eq!(project.record()["state"], "open");
        fs::remove_dir(project.root().join(log::PATH)).unwrap();
        fs::create_dir_all(project.root().join(".gdd/changes/c1/snapshot.json/x")).unwrap();
        let err = project.refused();
        let start = "cannot remove .gdd/changes/c1/snapshot.json: ";
        assert!(err.starts_with(start), "{err}");
        fs::remove_dir_all(project.root().join(".gdd/changes/c1/snapshot.json")).unwrap();
        project.repo.write(".gdd/changes/c1/change.json", "[]");
        let err = project.refused();
        assert!(err.starts_with(".gdd/changes/c1/change.json: "), "{err}");
    }
}
