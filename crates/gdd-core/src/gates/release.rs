//! The release steps (`gdd release pre|promote|observe`): what takes a merged change to pre,
//! then to production, and rolls it back when production gets worse.
//!
//! Each step runs its command from the `[release]` section of `gdd/commands.toml` (see
//! [`exec::run`]), never anything else; a step without a command passes as simulated only when
//! that section says `simulated = true`. Every step is logged. A change that needs the human G4
//! is never promoted without it.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::change::{self, Approval, Note};
use crate::config;
use crate::exec;
use crate::gates::{configured, kept, tenths};
use crate::{inbox, log};

/// The steps `gdd release` runs, by name.
pub const STEPS: [&str; 3] = ["pre", "promote", "observe"];

/// Why a change that needs the human G4 is not promoted.
const NEEDS_G4: &str = "This change needs a human G4 before promotion.";

/// What the human reads when production got worse and the change was rolled back.
const ROLLED_BACK: &str = "Production failed while observing: rolled back to the previous \
                           version. A hotfix is needed.";

/// The release commands of a project: the `[release]` section of `gdd/commands.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Steps {
    commands: BTreeMap<String, String>,
    simulated: bool,
}

impl Steps {
    /// Reads the `[release]` section of a commands file; the rest is ignored. A command is text
    /// that is not blank; only `simulated = true` lets the steps without one pass.
    pub fn from_toml(text: &str) -> Result<Steps, config::Error> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            release: BTreeMap<String, toml::Value>,
        }
        let raw: Raw = toml::from_str(text).map_err(|e| config::Error::Toml(e.to_string()))?;
        let simulated = raw.release.get("simulated") == Some(&toml::Value::Boolean(true));
        Ok(Steps {
            commands: configured(raw.release),
            simulated,
        })
    }
}

/// What a release step did. As JSON, only what it has: a simulated step has no exit code and
/// no output.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Step {
    /// The name of the step: `build`, `deploy_pre`, `tests_pre`, `demo`, `promote`...
    pub name: String,
    /// Whether it passed.
    pub ok: bool,
    /// It had no command and passed because the project simulates its release.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub simulated: bool,
    /// The exit code of its command (see [`exec::Ran::code`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,
    /// Seconds its command took, to a tenth.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_s: Option<f64>,
    /// What to read: what the output of its command keeps (see [`kept`]), or why it failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tail: Option<String>,
}

impl Step {
    fn new(name: &str, ok: bool) -> Step {
        Step {
            name: name.to_string(),
            ok,
            simulated: false,
            code: None,
            duration_s: None,
            tail: None,
        }
    }

    /// The step `name` when its command did `ran`.
    fn ran(name: &str, ran: io::Result<exec::Ran>) -> Step {
        match ran {
            Ok(ran) => Step {
                code: Some(ran.code),
                duration_s: Some(tenths(ran.duration.as_secs_f64())),
                tail: Some(kept(&ran.output)),
                ..Step::new(name, ran.ok())
            },
            Err(e) => Step {
                tail: Some(format!("cannot run sh: {e}")),
                ..Step::new(name, false)
            },
        }
    }

    /// Why the step failed: its name, the exit code of its command, then what it said.
    pub fn failure(&self) -> String {
        let mut why = format!("release.{} failed", self.name);
        if let Some(code) = self.code {
            why.push_str(&format!(" (exit code {code})"));
        }
        match self.tail.as_deref().map(str::trim_end) {
            Some(tail) if !tail.is_empty() => format!("{why}:\n{tail}"),
            _ => why,
        }
    }

    /// The result the log keeps: `simulated`, `ok` or `ko`.
    fn result(&self) -> &'static str {
        if self.simulated {
            "simulated"
        } else if self.ok {
            "ok"
        } else {
            "ko"
        }
    }
}

/// What a step of the recipe that passed prints: `ok`, then what it says (as JSON, only what it
/// has).
#[derive(Debug, Serialize)]
pub(crate) struct Passed<'a> {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    simulated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    steps: Option<&'a [Step]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<&'a Step>,
}

impl<'a> Passed<'a> {
    /// The steps that ran.
    fn steps(steps: &'a [Step]) -> Passed<'a> {
        Passed {
            ok: true,
            simulated: None,
            steps: Some(steps),
            detail: None,
        }
    }

    /// The step that counts and, when `simulated` is given, whether it was simulated.
    pub(crate) fn detail(detail: &'a Step, simulated: Option<bool>) -> Passed<'a> {
        Passed {
            ok: true,
            simulated,
            steps: None,
            detail: Some(detail),
        }
    }

    /// As JSON, on one line.
    pub(crate) fn json(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|e| e.to_string())
    }
}

/// Runs the release step `name` of change `id` from the root of the project at `root`, with
/// `GDD_CHANGE`, and logs it: `build` and `deploy_pre` belong to Integrate, the others to
/// Release. A step without a command in `steps` passes as simulated when `steps` simulates the
/// release, and fails when not.
pub fn step(root: &Path, id: &str, steps: &Steps, name: &str) -> Result<Step, String> {
    let step = match steps.commands.get(name) {
        Some(command) => {
            let env = [("GDD_CHANGE", id)];
            Step::ran(name, exec::run(command, root, &env, exec::TIMEOUT))
        }
        None if steps.simulated => Step {
            simulated: true,
            ..Step::new(name, true)
        },
        None => Step {
            tail: Some(format!(
                "release.{name} not configured and simulated = false"
            )),
            ..Step::new(name, false)
        },
    };
    let phase = if matches!(name, "build" | "deploy_pre") {
        "3-integrate"
    } else {
        "4-release"
    };
    let entry = log::Entry {
        change: id.to_string(),
        phase: phase.to_string(),
        gate: "-".to_string(),
        role: "machine".to_string(),
        agent: name.to_string(),
        result: step.result().to_string(),
        duration_s: step.duration_s.unwrap_or(0.0),
        note: String::new(),
    };
    change::logged(root, &entry)?;
    Ok(step)
}

/// `gdd release pre`: the tests in pre, then the demo, for change `id` of the project at `root`.
/// The change is in pre whatever they say: its state becomes `in-pre`. Both steps as JSON, or
/// why they failed.
pub fn pre(root: &Path, id: &str, steps: &Steps) -> Result<String, String> {
    let (text, _) = read(root, id)?;
    let ran = [
        step(root, id, steps, "tests_pre")?,
        step(root, id, steps, "demo")?,
    ];
    saved_in_state(root, id, &text, "in-pre")?;
    let failures: Vec<String> = ran.iter().filter(|s| !s.ok).map(Step::failure).collect();
    if !failures.is_empty() {
        return Err(failures.join("\n"));
    }
    Passed::steps(&ran).json()
}

/// `gdd release promote`: change `id` of the project at `root` to production. A change that
/// needs the human G4 waits for it: `needs_g4` says whether it needs it as it is classified
/// now, and the decision stored when it was prepared counts too. Otherwise the machines approve
/// G4, logged. The change goes to `in-production`, or stays `in-pre` when the step fails. The
/// step as JSON, or why it was refused or failed.
pub fn promote(root: &Path, id: &str, steps: &Steps, needs_g4: bool) -> Result<String, String> {
    let (mut text, record) = read(root, id)?;
    if needs_g4 || record["decision"]["g4"] == true {
        if record["gates"]["G4"]["decision"] != "ok" {
            return Err(NEEDS_G4.to_string());
        }
    } else {
        let ts = log::now();
        let approval = Approval {
            decision: "ok",
            by: "machines",
            ts: &ts,
            note: "automatic promotion: pre is green",
            hashes: None,
        };
        let path = change::record_path(id);
        let approved = change::with_gate(&text, "G4", &approval, None);
        text = approved.map_err(|e| format!("{path}: {e}"))?;
        let entry = log::Entry {
            change: id.to_string(),
            phase: "4-release".to_string(),
            gate: "G4".to_string(),
            role: "machine".to_string(),
            agent: "-".to_string(),
            result: "ok".to_string(),
            duration_s: 0.0,
            note: "automatic".to_string(),
        };
        change::logged(root, &entry)?;
    }
    let promoted = step(root, id, steps, "promote")?;
    let state = if promoted.ok {
        "in-production"
    } else {
        "in-pre"
    };
    saved_in_state(root, id, &text, state)?;
    if !promoted.ok {
        return Err(promoted.failure());
    }
    Passed::detail(&promoted, None).json()
}

/// `gdd release observe`: watches change `id` of the project at `root` in production. When the
/// step passes, the change is `closed`. When it fails, the rollback runs and the change goes to
/// `rolled-back` with a request for a hotfix; the error says so, with what the rollback did.
/// Either way its Decision Inbox is saved (see [`inbox::save`]).
pub fn observe(root: &Path, id: &str, steps: &Steps) -> Result<String, String> {
    let (text, _) = read(root, id)?;
    let observed = step(root, id, steps, "observe")?;
    if observed.ok {
        let closed = saved_in_state(root, id, &text, "closed")?;
        inbox::save(root, id, &closed)?;
        return Passed::detail(&observed, None).json();
    }
    let rollback = step(root, id, steps, "rollback")?;
    let path = change::record_path(id);
    let ts = log::now();
    let note = Note {
        kind: "needs",
        text: ROLLED_BACK,
        ts: &ts,
    };
    let noted = change::with_note(&text, &note).map_err(|e| format!("{path}: {e}"))?;
    let rolled_back = saved_in_state(root, id, &noted, "rolled-back")?;
    inbox::save(root, id, &rolled_back)?;
    let rollback = if rollback.ok {
        rollback.result().to_string()
    } else {
        rollback.failure()
    };
    let inbox = change::inbox_path(id);
    Err(format!(
        "{}\nRollback: {rollback}\n{ROLLED_BACK} Message in {inbox}",
        observed.failure()
    ))
}

/// The record of change `id` of the project at `root`: its text, and the JSON object it holds.
/// A record that is not one is an error before any step runs.
pub(crate) fn read(root: &Path, id: &str) -> Result<(String, Value), String> {
    let text = change::read_record(root, id)?;
    let path = change::record_path(id);
    let record: Map<String, Value> =
        serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    Ok((text, Value::Object(record)))
}

/// Writes the record `text` of change `id` of the project at `root` in state `state`; every
/// other field keeps its value. The record written.
fn saved_in_state(root: &Path, id: &str, text: &str, state: &str) -> Result<String, String> {
    let path = change::record_path(id);
    let fields = vec![("state", Value::from(state))];
    let saved = change::with_fields(text, fields).map_err(|e| format!("{path}: {e}"))?;
    change::write(root, &path, &saved)?;
    Ok(saved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::Folder;
    use std::fs;
    use std::time::Duration;

    const COMMANDS: &str = r#"
[g2]
lint = "true"

[release]
simulated = true
build = "true"
deploy_pre = "printf '%s' \"$GDD_CHANGE\" > deployed"
tests_pre = "true"
demo = ""
promote = "  "
observe = ""
rollback = ""
flag = false
"#;

    /// A project with change `c1`, whose record is `record`.
    fn project(record: &str) -> Folder {
        let folder = Folder::new();
        folder.write(".gdd/changes/c1/change.json", record);
        folder
    }

    const RECORD: &str = r#"{"id": "c1", "title": "App", "state": "merged", "gates": {},
                             "inbox": [], "x": 1}"#;

    fn steps(text: &str) -> Steps {
        Steps::from_toml(text).unwrap()
    }

    /// `commands` with the release command `name` set to `command`.
    fn with(name: &str, command: &str) -> Steps {
        let mut steps = steps(COMMANDS);
        steps.commands.insert(name.into(), command.into());
        steps
    }

    fn record(folder: &Folder) -> Value {
        let text = fs::read_to_string(folder.0.join(".gdd/changes/c1/change.json")).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    fn log(folder: &Folder) -> Vec<log::Record> {
        let text = fs::read_to_string(folder.0.join(log::PATH)).unwrap_or_default();
        log::parse(&text).unwrap()
    }

    fn json(text: &str) -> Value {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn the_release_commands_are_the_text_of_the_release_section() {
        let read = steps(COMMANDS);
        let commands: Vec<&str> = read.commands.keys().map(String::as_str).collect();
        assert_eq!(commands, ["build", "deploy_pre", "tests_pre"]);
        assert!(read.simulated);
        assert_eq!(steps(""), Steps::default());
        assert!(!Steps::default().simulated);
        for text in [
            "[release]\nsimulated = false\n",
            "[release]\nsimulated = \"yes\"\n",
        ] {
            assert!(!steps(text).simulated, "{text}");
        }
        for wrong in ["release = 1\n", "[release", "[release]\nbuild = "] {
            let result = Steps::from_toml(wrong);
            assert!(matches!(result, Err(config::Error::Toml(_))), "{wrong:?}");
        }
    }

    #[test]
    fn a_step_runs_its_command_with_the_change_and_is_logged() {
        let folder = project(RECORD);
        let ran = step(&folder.0, "c1", &steps(COMMANDS), "deploy_pre").unwrap();
        assert!(ran.ok && !ran.simulated, "{ran:?}");
        assert_eq!((ran.code, ran.tail.as_deref()), (Some(0), Some("")));
        assert!(ran.duration_s.is_some_and(|d| (0.0..10.0).contains(&d)));
        let deployed = fs::read_to_string(folder.0.join("deployed")).unwrap();
        assert_eq!(deployed, "c1");
        let failing = with("tests_pre", "echo broken; exit 4");
        let failed = step(&folder.0, "c1", &failing, "tests_pre").unwrap();
        assert!(!failed.ok);
        assert_eq!(failed.code, Some(4));
        assert_eq!(failed.tail.as_deref(), Some("broken\n"));
        let log = log(&folder);
        let fields = ["phase", "agent", "result", "change"];
        let values = |line: &log::Record| fields.map(|field| line.text(field));
        let integrate = ["3-integrate", "deploy_pre", "ok", "c1"];
        assert_eq!(values(&log[0]), integrate.map(String::from));
        let release = ["4-release", "tests_pre", "ko", "c1"];
        assert_eq!(values(&log[1]), release.map(String::from));
        for line in &log {
            let fixed = ["gate", "role", "note"].map(|field| line.text(field));
            assert_eq!(fixed, ["-", "machine", ""].map(String::from));
        }
        assert_eq!(log[1].duration_s, failed.duration_s.unwrap());
    }

    #[test]
    fn a_step_without_a_command_passes_only_when_the_release_is_simulated() {
        let folder = project(RECORD);
        for name in ["demo", "promote", "observe", "flag", "missing"] {
            let simulated = step(&folder.0, "c1", &steps(COMMANDS), name).unwrap();
            let expected = Step {
                simulated: true,
                ..Step::new(name, true)
            };
            assert_eq!(simulated, expected, "{name}");
        }
        let results: Vec<String> = log(&folder).iter().map(|l| l.text("result")).collect();
        assert_eq!(results, ["simulated"; 5]);
        assert_eq!(log(&folder)[0].duration_s, 0.0);
        let real = "[release]\nsimulated = false\nbuild = \"true\"\n";
        let built = step(&folder.0, "c1", &steps(real), "build").unwrap();
        assert!(built.ok && !built.simulated);
        let refused = step(&folder.0, "c1", &steps(real), "deploy_pre").unwrap();
        let why = "release.deploy_pre not configured and simulated = false";
        let expected = Step {
            tail: Some(why.to_string()),
            ..Step::new("deploy_pre", false)
        };
        assert_eq!(refused, expected);
        let last = log(&folder).pop().unwrap();
        assert_eq!(last.text("phase"), "3-integrate");
        assert_eq!(last.text("result"), "ko");
    }

    #[test]
    fn a_step_says_why_it_failed() {
        let ran = |code, output: &str| exec::Ran {
            code,
            output: output.to_string(),
            duration: Duration::from_millis(1_250),
        };
        let failed = Step::ran("observe", Ok(ran(3, "latency rose\n\n")));
        assert_eq!(failed.duration_s, Some(1.3));
        let why = "release.observe failed (exit code 3):\nlatency rose";
        assert_eq!(failed.failure(), why);
        let quiet = Step::ran("observe", Ok(ran(1, " \n")));
        assert_eq!(quiet.failure(), "release.observe failed (exit code 1)");
        let unstarted = Step::ran("build", Err(io::Error::other("no shell")));
        assert!(!unstarted.ok && unstarted.code.is_none());
        let why = "release.build failed:\ncannot run sh: no shell";
        assert_eq!(unstarted.failure(), why);
        assert_eq!(Step::new("demo", false).failure(), "release.demo failed");
    }

    #[test]
    fn as_json_a_step_has_only_what_it_did() {
        let simulated = Step {
            simulated: true,
            ..Step::new("demo", true)
        };
        let text = serde_json::to_string(&simulated).unwrap();
        assert_eq!(text, r#"{"name":"demo","ok":true,"simulated":true}"#);
        let built = exec::Ran {
            code: 0,
            output: "built\n".into(),
            duration: Duration::from_millis(100),
        };
        let text = serde_json::to_string(&Step::ran("build", Ok(built))).unwrap();
        let expected = r#"{"name":"build","ok":true,"code":0,"duration_s":0.1,"tail":"built\n"}"#;
        assert_eq!(text, expected);
        let passed = Passed::detail(&simulated, Some(true)).json().unwrap();
        let expected = concat!(
            r#"{"ok":true,"simulated":true,"#,
            r#""detail":{"name":"demo","ok":true,"simulated":true}}"#
        );
        assert_eq!(passed, expected);
        let both = [simulated.clone(), simulated];
        let steps = Passed::steps(&both).json().unwrap();
        let start = r#"{"ok":true,"steps":[{"name":"demo""#;
        assert!(steps.starts_with(start), "{steps}");
    }

    #[test]
    fn pre_runs_the_tests_and_the_demo_and_puts_the_change_in_pre() {
        let folder = project(RECORD);
        let out = json(&pre(&folder.0, "c1", &steps(COMMANDS)).unwrap());
        assert_eq!(out["ok"], true);
        let steps = out["steps"].as_array().unwrap();
        let names: Vec<&Value> = steps.iter().map(|step| &step["name"]).collect();
        assert_eq!(names, [&Value::from("tests_pre"), &Value::from("demo")]);
        assert_eq!(out["steps"][1]["simulated"], true);
        let record = record(&folder);
        assert_eq!(record["state"], "in-pre");
        assert_eq!(record["x"], 1);
        let agents: Vec<String> = log(&folder).iter().map(|l| l.text("agent")).collect();
        assert_eq!(agents, ["tests_pre", "demo"]);
    }

    #[test]
    fn failing_tests_in_pre_fail_the_step_but_the_change_is_in_pre() {
        let folder = project(RECORD);
        let mut failing = with("tests_pre", "echo red; exit 1");
        failing.commands.insert("demo".into(), "exit 2".into());
        let err = pre(&folder.0, "c1", &failing).unwrap_err();
        let expected =
            "release.tests_pre failed (exit code 1):\nred\nrelease.demo failed (exit code 2)";
        assert_eq!(err, expected);
        assert_eq!(record(&folder)["state"], "in-pre");
        assert_eq!(log(&folder).len(), 2);
        let demo_only = with("demo", "exit 2");
        let err = pre(&folder.0, "c1", &demo_only).unwrap_err();
        assert_eq!(err, "release.demo failed (exit code 2)");
    }

    #[test]
    fn a_change_that_does_not_need_g4_is_promoted_by_the_machines() {
        let folder = project(RECORD);
        let out = promote(&folder.0, "c1", &steps(COMMANDS), false).unwrap();
        let out = json(&out);
        assert_eq!(out["ok"], true);
        assert_eq!(out["detail"]["name"], "promote");
        let record = record(&folder);
        assert_eq!(record["state"], "in-production");
        let g4 = &record["gates"]["G4"];
        assert_eq!(g4["decision"], "ok");
        assert_eq!(g4["by"], "machines");
        assert_eq!(g4["note"], "automatic promotion: pre is green");
        assert!(g4["ts"].as_str().unwrap().ends_with("+00:00"));
        assert_eq!(record["x"], 1);
        let log = log(&folder);
        let approved = &log[0];
        let fields = ["phase", "gate", "role", "agent", "result", "note"];
        let values = fields.map(|field| approved.text(field));
        let expected = ["4-release", "G4", "machine", "-", "ok", "automatic"];
        assert_eq!(values, expected.map(String::from));
        assert_eq!(log[1].text("agent"), "promote");
    }

    #[test]
    fn a_change_that_needs_g4_waits_for_the_human() {
        let folder = project(RECORD);
        let err = promote(&folder.0, "c1", &steps(COMMANDS), true).unwrap_err();
        assert_eq!(err, NEEDS_G4);
        assert_eq!(NEEDS_G4, "This change needs a human G4 before promotion.");
        assert_eq!(record(&folder)["state"], "merged");
        assert_eq!(record(&folder)["gates"], serde_json::json!({}));
        assert!(log(&folder).is_empty());
        // The decision stored when it was prepared counts too.
        let prepared = r#"{"id": "c1", "decision": {"g4": true}, "gates": {}}"#;
        let folder = project(prepared);
        let err = promote(&folder.0, "c1", &steps(COMMANDS), false).unwrap_err();
        assert_eq!(err, NEEDS_G4);
        for refused in [r#"{"decision": "ko"}"#, r#"{"by": "human"}"#] {
            let text = format!(r#"{{"id": "c1", "gates": {{"G4": {refused}}}}}"#);
            let folder = project(&text);
            let err = promote(&folder.0, "c1", &steps(COMMANDS), true).unwrap_err();
            assert_eq!(err, NEEDS_G4, "{refused}");
        }
        let approved = r#"{"id": "c1", "gates": {"G4": {"decision": "ok", "by": "human"}}}"#;
        let folder = project(approved);
        assert!(promote(&folder.0, "c1", &steps(COMMANDS), true).is_ok());
        let record = record(&folder);
        assert_eq!(record["state"], "in-production");
        assert_eq!(record["gates"]["G4"]["by"], "human");
        let agents: Vec<String> = log(&folder).iter().map(|l| l.text("agent")).collect();
        assert_eq!(agents, ["promote"]);
    }

    #[test]
    fn a_failed_promotion_leaves_the_change_in_pre() {
        let folder = project(RECORD);
        let err = promote(&folder.0, "c1", &with("promote", "exit 5"), false).unwrap_err();
        assert_eq!(err, "release.promote failed (exit code 5)");
        let record = record(&folder);
        assert_eq!(record["state"], "in-pre");
        assert_eq!(record["gates"]["G4"]["by"], "machines");
    }

    #[test]
    fn production_that_holds_closes_the_change() {
        let folder = project(RECORD);
        let out = json(&observe(&folder.0, "c1", &steps(COMMANDS)).unwrap());
        assert_eq!(out["detail"]["name"], "observe");
        assert_eq!(record(&folder)["state"], "closed");
        let inbox = fs::read_to_string(folder.0.join(".gdd/changes/c1/inbox.md")).unwrap();
        assert!(inbox.contains("state closed"), "{inbox}");
        let agents: Vec<String> = log(&folder).iter().map(|l| l.text("agent")).collect();
        assert_eq!(agents, ["observe"]);
    }

    #[test]
    fn production_getting_worse_rolls_the_change_back_and_asks_for_a_hotfix() {
        let folder = project(RECORD);
        let err = observe(&folder.0, "c1", &with("observe", "echo worse; exit 1")).unwrap_err();
        let expected = "release.observe failed (exit code 1):\nworse\nRollback: simulated\n\
                        Production failed while observing: rolled back to the previous version. \
                        A hotfix is needed. Message in .gdd/changes/c1/inbox.md";
        assert_eq!(err, expected);
        let rolled_back = record(&folder);
        assert_eq!(rolled_back["state"], "rolled-back");
        let note = &rolled_back["inbox"][0];
        assert_eq!(note["kind"], "needs");
        assert_eq!(note["text"], ROLLED_BACK);
        let inbox = fs::read_to_string(folder.0.join(".gdd/changes/c1/inbox.md")).unwrap();
        assert!(inbox.contains("A hotfix is needed"), "{inbox}");
        assert!(inbox.contains("state rolled-back"), "{inbox}");
        let agents: Vec<String> = log(&folder).iter().map(|l| l.text("agent")).collect();
        assert_eq!(agents, ["observe", "rollback"]);
        let mut both = with("observe", "exit 1");
        both.commands.insert("rollback".into(), "true".into());
        let folder = project(RECORD);
        let err = observe(&folder.0, "c1", &both).unwrap_err();
        assert!(err.contains("\nRollback: ok\n"), "{err}");
        let stuck = "echo stuck; exit 9";
        both.commands.insert("rollback".into(), stuck.into());
        let folder = project(RECORD);
        let err = observe(&folder.0, "c1", &both).unwrap_err();
        let rollback = "\nRollback: release.rollback failed (exit code 9):\nstuck\n";
        assert!(err.contains(rollback), "{err}");
        assert_eq!(record(&folder)["state"], "rolled-back");
    }

    #[test]
    fn the_steps_need_a_record_they_can_read_and_write() {
        let folder = Folder::new();
        let missing = "change c9 does not exist: there is no .gdd/changes/c9/change.json";
        let release = steps(COMMANDS);
        let missing: Result<String, String> = Err(missing.into());
        assert_eq!(pre(&folder.0, "c9", &release), missing);
        assert_eq!(promote(&folder.0, "c9", &release, false), missing);
        assert_eq!(observe(&folder.0, "c9", &release), missing);
        assert!(log(&folder).is_empty());
        for wrong in ["{not json", "[]"] {
            let broken = project(wrong);
            for err in [
                pre(&broken.0, "c1", &release).unwrap_err(),
                promote(&broken.0, "c1", &release, false).unwrap_err(),
                observe(&broken.0, "c1", &release).unwrap_err(),
            ] {
                assert!(err.starts_with(".gdd/changes/c1/change.json: "), "{err}");
            }
            assert!(log(&broken).is_empty(), "no step ran");
        }
        let unlogged = project(RECORD);
        fs::create_dir_all(unlogged.0.join(log::PATH)).unwrap();
        let err = pre(&unlogged.0, "c1", &release).unwrap_err();
        assert!(err.starts_with("cannot write .gdd/log.jsonl: "), "{err}");
        let err = promote(&unlogged.0, "c1", &release, false).unwrap_err();
        assert!(err.starts_with("cannot write .gdd/log.jsonl: "), "{err}");
        assert_eq!(record(&unlogged)["state"], "merged");
    }
}
