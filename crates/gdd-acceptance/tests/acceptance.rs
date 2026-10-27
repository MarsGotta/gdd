//! Runs the Gherkin scenarios in `features/` (strict: undefined or skipped steps fail).
//!
//! - `GDD_FEATURES`: the change's `.feature` files, one per line. Empty = every scenario.
//! - `GDD_HIDDEN=1`: also run `features/hidden/` (G3).
//!
//! Steps talk to `gdd-core`, never to the terminal UI.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use cucumber::{World, given, then, when};

#[derive(Debug, Default, World)]
pub struct GddWorld {
    output: String,
    zone_map: String,
    models: String,
    outcome: Option<Outcome>,
    project: Option<TempDir>,
    ran: Option<Ran>,
    /// The environment gdd runs in for this scenario, built on first use.
    env: Option<Vec<(String, String)>>,
    /// The scenario's own HOME, where its Claude Code transcripts live.
    home: Option<TempDir>,
    /// The scenario looked at the exit code itself, so a failed run may still be read.
    exit_checked: bool,
}

#[given("the gdd tool")]
fn the_tool(_w: &mut GddWorld) {}

#[when("I ask for its version")]
fn ask_version(w: &mut GddWorld) {
    w.output = gdd_core::version_line();
}

#[then(expr = "the answer starts with {string}")]
fn answer_starts(w: &mut GddWorld, prefix: String) {
    assert!(
        w.output.starts_with(&prefix),
        "{:?} does not start with {prefix:?}",
        w.output
    );
}

// ---- features/classify.feature -------------------------------------------------------------
//
// API of `gdd-core` these steps expect (the Builder in build mode implements it):
// - `gdd_core::zones::ZoneMap::from_toml(&str) -> Result<ZoneMap, E>`
// - `gdd_core::rules::default_table() -> RulesTable`
// - `gdd_core::classify(&ZoneMap, &RulesTable, &Change) -> Result<Classification, E>`
// - `gdd_core::Change { paths: Vec<String>, min_color: Option<Color>, resumable: bool }`
// - `Color: FromStr + Display` ("green" | "yellow" | "red"); `Lane: Display` ("fast" | "full")
// - `Classification { color, lane, g1, g4, human_plan, reviewer, mutation, tests_first }`,
//   the last six are `bool`; `E: Display` and names the unknown color.

/// Outcome of the last classification: the result, or the error message.
type Outcome = Result<gdd_core::Classification, String>;

fn classify_with(zone_map: &str, change: &gdd_core::Change) -> Outcome {
    let zones = gdd_core::zones::ZoneMap::from_toml(zone_map).map_err(|e| e.to_string())?;
    let table = gdd_core::rules::default_table();
    gdd_core::classify(&zones, &table, change).map_err(|e| e.to_string())
}

/// Reads a step text like `"a" and the scenario file "b" with minimum color "red" and is resumable`.
fn parse_change(text: &str) -> gdd_core::Change {
    let mut paths = Vec::new();
    let mut min_color = None;
    let mut parts = text.split('"');
    let mut tail = parts.next().unwrap_or_default();
    while let (Some(value), Some(after)) = (parts.next(), parts.next()) {
        if tail.contains("minimum color") {
            min_color = Some(value.parse().expect("a known minimum color"));
        } else {
            paths.push(value.to_string());
        }
        tail = after;
    }
    gdd_core::Change {
        paths,
        min_color,
        resumable: !tail.contains("not resumable"),
    }
}

#[given(expr = "the zone map:")]
fn the_zone_map(w: &mut GddWorld, step: &cucumber::gherkin::Step) {
    w.zone_map = docstring(step, "a zone map docstring");
    // In a project (guard and scope features) the map also lives where the tool reads it.
    if let Some(dir) = &w.project {
        write_file(&dir.0.join("gdd/zones.toml"), &format!("{}\n", w.zone_map));
    }
}

#[given("the default rules table")]
fn the_default_rules(_w: &mut GddWorld) {}

#[when(regex = r"^a change touches (.*)$")]
fn a_change_touches(w: &mut GddWorld, text: String) {
    w.outcome = Some(classify_with(&w.zone_map, &parse_change(&text)));
}

fn classified(w: &GddWorld) -> &gdd_core::Classification {
    match w.outcome.as_ref().expect("a change was classified") {
        Ok(c) => c,
        Err(e) => panic!("classifying failed: {e}"),
    }
}

#[then(expr = "its color is {string}")]
fn its_color(w: &mut GddWorld, color: String) {
    assert_eq!(classified(w).color.to_string(), color);
}

#[then(expr = "its lane is {string}")]
fn its_lane(w: &mut GddWorld, lane: String) {
    assert_eq!(classified(w).lane.to_string(), lane);
}

#[then(expr = "classifying fails with a message that names {string}")]
fn classifying_fails(w: &mut GddWorld, name: String) {
    match w.outcome.as_ref().expect("a change was classified") {
        Ok(c) => panic!("classifying should fail, got color {}", c.color),
        Err(message) => assert!(
            message.contains(&name),
            "the message {message:?} does not name {name:?}"
        ),
    }
}

#[then(regex = r"^it (needs|does not need) (.+)$")]
fn it_needs(w: &mut GddWorld, verb: String, what: String) {
    let c = classified(w);
    let actual = match what.as_str() {
        "scenario approval (G1)" => c.g1,
        "the human plan" => c.human_plan,
        "the human release gate (G4)" => c.g4,
        "the Reviewer" => c.reviewer,
        "mutation testing" => c.mutation,
        "tests first" => c.tests_first,
        other => panic!("unknown requirement {other:?}"),
    };
    assert_eq!(actual, verb == "needs", "it {verb} {what}");
}

// ---- features/report.feature and features/cli-classify.feature -----------------------------
//
// API of the `gdd` crate these steps expect (the Builder in build mode implements it):
// - `gdd::run(args: &[String], cwd: &Path) -> gdd::Outcome`, where `args` are the words after
//   the program name (`["report", "--by", "role"]`) and `Outcome { code, stdout, stderr }`
//   has `code` comparable to an integer and `stdout`/`stderr` as `String`.
// Every project is a temporary folder: `gdd/project.toml` marks its root.

/// A temporary folder, removed when dropped.
#[derive(Debug)]
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "gdd-acceptance-{}-{nanos}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).expect("a temporary folder");
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// What the last `gdd` run did.
#[derive(Debug)]
struct Ran {
    code: u8,
    succeeded: bool,
    stdout: String,
    stderr: String,
}

fn write_file(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().expect("a file inside a folder")).expect("a folder");
    fs::write(path, content).expect("a written file");
}

/// The project root of the scenario, created on first use with its `gdd/project.toml`.
fn project_root(w: &mut GddWorld) -> PathBuf {
    let dir = w.project.get_or_insert_with(|| {
        let dir = TempDir::new();
        write_file(
            &dir.0.join("gdd/project.toml"),
            "name = \"demo\"\nlanguage = \"en\"\n",
        );
        dir
    });
    dir.0.clone()
}

fn run_gdd(w: &mut GddWorld, command: &str, cwd: &Path) {
    run_gdd_as(w, command, cwd, None);
}

/// The environment gdd runs in for this scenario: the process one, minus the variables gdd's own
/// agents set. Steps never change the process environment, scenarios run concurrently.
fn world_env(w: &mut GddWorld) -> &mut Vec<(String, String)> {
    w.env.get_or_insert_with(|| {
        std::env::vars()
            .filter(|(name, _)| !matches!(name.as_str(), "GDD_ROLE" | "GDD_CHANGE" | "GDD_ROOT"))
            .collect()
    })
}

fn set_env(w: &mut GddWorld, name: &str, value: &str) {
    let env = world_env(w);
    env.retain(|(known, _)| known != name);
    env.push((name.to_string(), value.to_string()));
}

/// Runs `command` as `role`, in the environment of the scenario.
fn run_gdd_as(w: &mut GddWorld, command: &str, cwd: &Path, role: Option<&str>) {
    let args: Vec<String> = command
        .split_whitespace()
        .skip_while(|word| *word == "gdd")
        .map(str::to_string)
        .collect();
    let mut env = world_env(w).clone();
    env.retain(|(name, _)| name != "GDD_ROLE");
    if let Some(role) = role {
        env.push(("GDD_ROLE".to_string(), role.to_string()));
    }
    let out = gdd::run_with_env(&args, cwd, &env);
    // A check on the exit code belongs to the run it followed, never to a later one.
    w.exit_checked = false;
    w.ran = Some(Ran {
        code: out.code,
        succeeded: out.code == 0,
        stdout: out.stdout,
        stderr: out.stderr,
    });
}

fn last_run(w: &GddWorld) -> &Ran {
    w.ran.as_ref().expect("gdd was run")
}

fn stdout_json(w: &GddWorld) -> serde_json::Value {
    let ran = last_run(w);
    serde_json::from_str(&ran.stdout)
        .unwrap_or_else(|e| panic!("the output is not JSON ({e}): {:?}", ran.stdout))
}

#[given(expr = "a GDD project with the log:")]
fn project_with_log(w: &mut GddWorld, step: &cucumber::gherkin::Step) {
    let root = project_root(w);
    let log = docstring(step, "a log docstring");
    write_file(&root.join(".gdd/log.jsonl"), &format!("{log}\n"));
}

#[given(regex = r#"^the log line (\d+) is replaced by "(.*)"$"#)]
fn replace_log_line(w: &mut GddWorld, number: usize, text: String) {
    let path = project_root(w).join(".gdd/log.jsonl");
    let log = fs::read_to_string(&path).expect("a log to edit");
    let mut lines: Vec<&str> = log.lines().collect();
    assert!(number >= 1 && number <= lines.len(), "no log line {number}");
    lines[number - 1] = &text;
    write_file(&path, &format!("{}\n", lines.join("\n")));
}

#[given(expr = "a GDD project with the zone map:")]
fn project_with_zone_map(w: &mut GddWorld, step: &cucumber::gherkin::Step) {
    let root = project_root(w);
    let zones = docstring(step, "a zone map docstring");
    write_file(&root.join("gdd/zones.toml"), &format!("{zones}\n"));
}

#[given("the project's rules table is the default one")]
fn project_default_rules(w: &mut GddWorld) {
    let root = project_root(w);
    write_file(
        &root.join("gdd/rules.toml"),
        &fs::read_to_string(repo_root().join("gdd/rules.toml")).expect("the repository's rules"),
    );
}

#[given(regex = r#"^the planned change "([^"]+)" touches (.*)$"#)]
fn planned_change(w: &mut GddWorld, id: String, text: String) {
    let change = parse_change(&text);
    let record = serde_json::json!({
        "id": id,
        "title": id,
        "paths": change.paths,
        "resumable": change.resumable,
        "min_color": change.min_color.map(|c| c.to_string()),
        "state": "open",
        "gates": {},
        "inbox": [],
    });
    let path = project_root(w).join(format!(".gdd/changes/{id}/change.json"));
    write_file(&path, &record.to_string());
}

#[when(expr = "I run {string}")]
fn i_run(w: &mut GddWorld, command: String) {
    let root = project_root(w);
    run_gdd(w, &command, &root);
}

#[given(expr = "I run {string}")]
fn given_i_run(w: &mut GddWorld, command: String) {
    let root = project_root(w);
    run_gdd(w, &command, &root);
}

#[when(expr = "I run {string} from the folder {string}")]
fn i_run_from(w: &mut GddWorld, command: String, folder: String) {
    let cwd = project_root(w).join(folder);
    fs::create_dir_all(&cwd).expect("the folder");
    run_gdd(w, &command, &cwd);
}

#[when(expr = "I run {string} outside any GDD project")]
fn i_run_outside(w: &mut GddWorld, command: String) {
    let outside = TempDir::new();
    run_gdd(w, &command, &outside.0);
}

#[then("it succeeds")]
fn it_succeeds(w: &mut GddWorld) {
    let ran = last_run(w);
    assert!(ran.succeeded, "gdd failed: {:?}", ran.stderr);
}

#[then(expr = "it fails with a message that contains {string}")]
fn it_fails(w: &mut GddWorld, text: String) {
    let ran = last_run(w);
    assert!(
        !ran.succeeded,
        "gdd should fail, it printed {:?}",
        ran.stdout
    );
    assert!(
        ran.stderr.contains(&text),
        "the message {:?} does not contain {text:?}",
        ran.stderr
    );
}

#[then("the output is:")]
fn output_is(w: &mut GddWorld, step: &cucumber::gherkin::Step) {
    let ran = last_run(w);
    assert!(ran.succeeded, "gdd failed: {:?}", ran.stderr);
    let expected = docstring(step, "an output docstring");
    assert_eq!(ran.stdout.trim_end(), expected.as_str().trim_end());
}

#[then(expr = "the output contains {string}")]
fn output_contains(w: &mut GddWorld, text: String) {
    let ran = last_run(w);
    assert!(
        ran.succeeded || w.exit_checked,
        "gdd failed: {:?}",
        ran.stderr
    );
    assert!(
        ran.stdout.contains(&text),
        "the output {:?} does not contain {text:?}",
        ran.stdout
    );
}

#[then(expr = "the output is a JSON list of {int} entries")]
fn output_json_list(w: &mut GddWorld, count: usize) {
    let value = stdout_json(w);
    let list = value.as_array().expect("a JSON list");
    assert_eq!(list.len(), count, "entries in {value}");
}

#[then(expr = "the entry with {string} {string} has {string} {int} and {string} {int}")]
fn entry_has(
    w: &mut GddWorld,
    key: String,
    value: String,
    first: String,
    first_value: i64,
    second: String,
    second_value: i64,
) {
    let json = stdout_json(w);
    let entry = json
        .as_array()
        .expect("a JSON list")
        .iter()
        .find(|e| e[&key].as_str() == Some(value.as_str()))
        .unwrap_or_else(|| panic!("no entry with {key} {value:?} in {json}"));
    for (field, expected) in [(first, first_value), (second, second_value)] {
        assert_eq!(entry[&field].as_i64(), Some(expected), "{field} in {entry}");
    }
}

#[then(regex = r#"^the JSON field "([^"]+)" is (.+)$"#)]
fn json_field_is(w: &mut GddWorld, field: String, literal: String) {
    let ran = last_run(w);
    assert!(ran.succeeded, "gdd failed: {:?}", ran.stderr);
    let expected: serde_json::Value =
        serde_json::from_str(&literal).expect("a JSON literal in the step");
    let json = stdout_json(w);
    assert_eq!(json.get(&field), Some(&expected), "field {field} in {json}");
}

// ---- features/guard.feature and features/scope.feature -------------------------------------
//
// Projects here are temporary git repositories: `main` holds the project files, the change lives
// on `change/<id>`. Git runs with its author set on the command line, never from the user's config.

/// Runs git in `root` and returns what it printed; any failure is a failed step.
fn git(root: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=GDD Test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(root)
        .output()
        .expect("git can run");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn commit_all(root: &Path, message: &str) {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "--allow-empty", "-m", message]);
}

/// The record of change `id`, read and written back as JSON.
fn read_record(root: &Path, id: &str) -> serde_json::Value {
    let text = fs::read_to_string(root.join(format!(".gdd/changes/{id}/change.json")))
        .expect("a change record");
    serde_json::from_str(&text).expect("a JSON record")
}

fn write_record(root: &Path, id: &str, record: &serde_json::Value) {
    write_file(
        &root.join(format!(".gdd/changes/{id}/change.json")),
        &record.to_string(),
    );
}

/// Touches `path`: creates it, or appends a comment line when it exists, so config files
/// keep parsing.
fn touch(root: &Path, path: &str) {
    let file = root.join(path);
    let old = fs::read_to_string(&file).unwrap_or_default();
    write_file(&file, &format!("{old}\n# touched\n"));
}

#[given(expr = "a GDD project in a git repository with the protected paths:")]
fn git_project(w: &mut GddWorld, step: &cucumber::gherkin::Step) {
    let root = project_root(w);
    let rules = docstring(step, "a rules docstring");
    write_file(&root.join("gdd/rules.toml"), &format!("{rules}\n"));
    git(&root, &["init", "-q", "-b", "main"]);
    // Steps that make gdd commit need an author: repo-local, gdd never sets one.
    git(&root, &["config", "--local", "user.name", "GDD Test"]);
    git(
        &root,
        &["config", "--local", "user.email", "test@example.com"],
    );
    git(&root, &["config", "--local", "commit.gpgsign", "false"]);
}

#[given(regex = r#"^the change "([^"]+)" plans (.*) and its build has started on its branch$"#)]
fn change_started(w: &mut GddWorld, id: String, text: String) {
    let root = project_root(w);
    let paths = parse_change(&text).paths;
    commit_all(&root, "project");
    // A scenario may plan the same change again: the branch is already there.
    let branch = format!("change/{id}");
    let exists = std::process::Command::new("git")
        .args([
            "rev-parse",
            "-q",
            "--verify",
            &format!("refs/heads/{branch}"),
        ])
        .current_dir(&root)
        .output()
        .expect("git can run")
        .status
        .success();
    let flags: &[&str] = if exists { &[] } else { &["-b"] };
    git(
        &root,
        &[&["checkout", "-q"], flags, &[branch.as_str()]].concat(),
    );
    let record = serde_json::json!({
        "id": id,
        "title": id,
        "paths": paths,
        "resumable": false,
        "min_color": null,
        "state": "building",
        "gates": {},
        "inbox": [],
        "build_start": git(&root, &["rev-parse", "HEAD"]),
    });
    write_record(&root, &id, &record);
}

#[given(regex = r#"^the change "([^"]+)" plans (.*) and its build has not started$"#)]
fn change_not_started(w: &mut GddWorld, id: String, text: String) {
    let root = project_root(w);
    let paths = parse_change(&text).paths;
    let record = serde_json::json!({
        "id": id,
        "title": id,
        "paths": paths,
        "resumable": false,
        "min_color": null,
        "state": "open",
        "gates": {},
        "inbox": [],
    });
    write_record(&root, &id, &record);
}

#[given(expr = "the scenario {string} was approved in G1")]
fn scenario_approved(w: &mut GddWorld, file: String) {
    let root = project_root(w);
    write_file(&root.join(&file), "Feature: approved\n");
    commit_all(&root, "approved scenario");
    let hash = std::process::Command::new("sha256sum")
        .arg(&file)
        .current_dir(&root)
        .output()
        .expect("sha256sum can run");
    let hash = String::from_utf8_lossy(&hash.stdout)
        .split_whitespace()
        .next()
        .expect("a hash")
        .to_string();
    let mut record = read_record(&root, "c1");
    record["gates"]["G1"] = serde_json::json!({"decision": "ok", "hashes": {file: hash}});
    // The build starts after the approval: the scenario itself is not part of the change.
    record["build_start"] = git(&root, &["rev-parse", "HEAD"]).into();
    write_record(&root, "c1", &record);
}

fn change_writes(w: &mut GddWorld, path: &str) {
    let root = project_root(w);
    touch(&root, path);
}

#[given(expr = "the change wrote {string}")]
fn change_wrote(w: &mut GddWorld, path: String) {
    change_writes(w, &path);
}

#[when(expr = "the change writes {string}")]
fn change_writes_step(w: &mut GddWorld, path: String) {
    change_writes(w, &path);
}

#[when(expr = "the change modifies {string}")]
fn change_modifies(w: &mut GddWorld, path: String) {
    let root = project_root(w);
    assert!(root.join(&path).is_file(), "{path} should exist already");
    touch(&root, &path);
}

#[when(expr = "the change deletes {string}")]
fn change_deletes(w: &mut GddWorld, path: String) {
    let root = project_root(w);
    std::fs::remove_file(root.join(&path)).expect("the file to delete exists");
}

#[when(expr = "the change commits {string}")]
fn change_commits(w: &mut GddWorld, path: String) {
    let root = project_root(w);
    touch(&root, &path);
    git(&root, &["add", "--", &path]);
    git(&root, &["commit", "-q", "-m", "change commit"]);
}

/// The list of strings in `field` of the last JSON output.
fn json_strings(w: &GddWorld, field: &str) -> Vec<String> {
    let json = stdout_json(w);
    json[field]
        .as_array()
        .unwrap_or_else(|| panic!("no list {field:?} in {json}"))
        .iter()
        .map(|v| v.as_str().expect("a string").to_string())
        .collect()
}

fn ok_flag(w: &GddWorld) -> bool {
    let json = stdout_json(w);
    json["ok"]
        .as_bool()
        .unwrap_or_else(|| panic!("no boolean \"ok\" in {json}"))
}

#[then("Path Guard passes")]
fn guard_passes(w: &mut GddWorld) {
    assert!(ok_flag(w), "Path Guard failed: {}", last_run(w).stdout);
    assert_eq!(json_strings(w, "failures"), Vec::<String>::new());
}

/// The quoted names of a step text like `"a" and "b"`.
fn quoted(text: &str) -> Vec<&str> {
    text.split('"').skip(1).step_by(2).collect()
}

#[then(regex = r#"^Path Guard fails naming ("[^"]+"(?: and "[^"]+")?)$"#)]
fn guard_fails(w: &mut GddWorld, names: String) {
    assert!(!ok_flag(w), "Path Guard passed: {}", last_run(w).stdout);
    let failures = json_strings(w, "failures");
    let names = quoted(&names);
    assert!(!names.is_empty(), "the step names nothing");
    for name in names {
        assert!(
            failures.iter().any(|f| f.contains(name)),
            "no failure names {name:?}: {failures:?}"
        );
    }
}

#[then(regex = r"^the exit code (is|is not) 0$")]
fn exit_code(w: &mut GddWorld, verb: String) {
    w.exit_checked = true;
    let ran = last_run(w);
    assert_eq!(ran.code == 0, verb == "is", "exit code {}", ran.code);
}

#[then("the scope passes with nothing accepted")]
fn scope_passes_empty(w: &mut GddWorld) {
    assert!(ok_flag(w), "the scope failed: {}", last_run(w).stdout);
    assert_eq!(json_strings(w, "accepted"), Vec::<String>::new());
}

#[then(expr = "the scope passes accepting {string}")]
fn scope_passes_accepting(w: &mut GddWorld, path: String) {
    assert!(ok_flag(w), "the scope failed: {}", last_run(w).stdout);
    assert_eq!(json_strings(w, "accepted"), vec![path]);
}

#[then(expr = "the scope fails naming {string} and {string}")]
fn scope_fails(w: &mut GddWorld, path: String, color: String) {
    assert!(!ok_flag(w), "the scope passed: {}", last_run(w).stdout);
    let failures = json_strings(w, "failures");
    assert!(
        failures
            .iter()
            .any(|f| f.contains(&path) && f.contains(&color)),
        "no failure names {path:?} and {color:?}: {failures:?}"
    );
}

// ---- features/gates.feature ----------------------------------------------------------------
//
// The gate commands live in `gdd/commands.toml`; `gdd g2 <id>` and `gdd g3 <id>` print the result
// as JSON (`ok`, `passed`, `skipped`, `failures` with `name` and `tail`) and save it in
// `.gdd/changes/<id>/<gate>.json`.

#[given(expr = "the gate commands:")]
fn gate_commands(w: &mut GddWorld, step: &cucumber::gherkin::Step) {
    let root = project_root(w);
    let commands = docstring(step, "a commands docstring");
    write_file(&root.join("gdd/commands.toml"), &format!("{commands}\n"));
}

/// Replaces the command `name` of the `[gate]` section of a commands file.
fn with_command(toml: &str, gate: &str, name: &str, command: &str) -> String {
    let mut section = String::new();
    let mut replaced = false;
    let lines: Vec<String> = toml
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if let Some(header) = trimmed.strip_prefix('[') {
                section = header.trim_end_matches(']').to_string();
            }
            let key = trimmed.split('=').next().unwrap_or_default().trim();
            if section == gate && key == name && trimmed.contains('=') {
                replaced = true;
                let escaped = command.replace('\\', "\\\\").replace('"', "\\\"");
                return format!("{name} = \"{escaped}\"");
            }
            line.to_string()
        })
        .collect();
    assert!(replaced, "no command {name:?} in the [{gate}] section");
    format!("{}\n", lines.join("\n"))
}

#[given(regex = r#"^the gate command "([^"]+)" "([^"]+)" is "((?:[^"\\]|\\.)*)"$"#)]
fn gate_command_is(w: &mut GddWorld, gate: String, name: String, command: String) {
    let root = project_root(w);
    let path = root.join("gdd/commands.toml");
    let toml = fs::read_to_string(&path).expect("gate commands to edit");
    // The step text escapes the quotes inside the command.
    let command = command.replace("\\\"", "\"");
    write_file(&path, &with_command(&toml, &gate, &name, &command));
    // A Given on the harness is state from before the build; what the change writes goes in a When.
    git(&root, &["add", "--", "gdd/commands.toml"]);
    git(&root, &["commit", "-q", "-m", "gate commands"]);
    let head = git(&root, &["rev-parse", "HEAD"]);
    let changes = root.join(".gdd/changes");
    let ids: Vec<String> = fs::read_dir(&changes)
        .map(|dir| {
            dir.flatten()
                .filter_map(|entry| entry.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    // Only the change whose branch got the commit: a commit on main (after a launch, say) is not
    // in another change's branch, and a build start outside its branch blinds Path Guard.
    let current = git(&root, &["branch", "--show-current"]);
    for id in ids {
        let mut record = read_record(&root, &id);
        if record.get("build_start").is_some() && current == format!("change/{id}") {
            record["build_start"] = head.clone().into();
            write_record(&root, &id, &record);
        }
    }
}

#[given(expr = "the scenario {string} with {int} scenarios")]
fn scenario_file(w: &mut GddWorld, file: String, count: usize) {
    let root = project_root(w);
    let mut text = String::from("Feature: counted\n");
    for n in 1..=count {
        text.push_str(&format!("\n  Scenario: number {n}\n    Given a thing\n"));
    }
    write_file(&root.join(file), &text);
}

/// The `failures` of the last gate output: `(name, tail)` of each.
fn gate_failures(w: &GddWorld) -> Vec<(String, String)> {
    let json = stdout_json(w);
    json["failures"]
        .as_array()
        .unwrap_or_else(|| panic!("no list \"failures\" in {json}"))
        .iter()
        .map(|f| {
            let text = |key: &str| f[key].as_str().unwrap_or_default().to_string();
            (text("name"), text("tail"))
        })
        .collect()
}

#[then("the gate passes")]
fn gate_passes(w: &mut GddWorld) {
    assert!(ok_flag(w), "the gate failed: {}", last_run(w).stdout);
    assert!(gate_failures(w).is_empty(), "{}", last_run(w).stdout);
}

#[then(regex = r#"^the gate passed ("[^"]+"(?:, "[^"]+")*(?: and "[^"]+")?)$"#)]
fn gate_passed(w: &mut GddWorld, names: String) {
    let passed = json_strings(w, "passed");
    let names = quoted(&names);
    assert!(!names.is_empty(), "the step names nothing");
    for name in names {
        assert!(
            passed.iter().any(|p| p == name),
            "{name:?} is not among the passed: {passed:?}"
        );
    }
}

#[then(expr = "the gate skipped {string}")]
fn gate_skipped(w: &mut GddWorld, name: String) {
    let skipped = json_strings(w, "skipped");
    assert!(skipped.contains(&name), "not skipped: {skipped:?}");
    assert!(!json_strings(w, "passed").contains(&name));
}

#[then(expr = "the gate did not run {string}")]
fn gate_did_not_run(w: &mut GddWorld, name: String) {
    let failed: Vec<String> = gate_failures(w).into_iter().map(|(n, _)| n).collect();
    for (list, names) in [
        ("passed", json_strings(w, "passed")),
        ("skipped", json_strings(w, "skipped")),
        ("failures", failed),
    ] {
        assert!(!names.contains(&name), "{name:?} is among the {list}");
    }
}

#[then(expr = "the gate fails on {string}")]
fn gate_fails_on(w: &mut GddWorld, name: String) {
    assert!(!ok_flag(w), "the gate passed: {}", last_run(w).stdout);
    let failures = gate_failures(w);
    assert!(
        failures.iter().any(|(n, _)| n.contains(&name)),
        "no failure is named {name:?}: {failures:?}"
    );
}

#[then(expr = "the failure of {string} shows {string}")]
fn failure_shows(w: &mut GddWorld, name: String, text: String) {
    let failures = gate_failures(w);
    let (_, tail) = failures
        .iter()
        .find(|(n, _)| *n == name)
        .unwrap_or_else(|| panic!("no failure of {name:?}: {failures:?}"));
    assert!(tail.contains(&text), "the tail {tail:?} lacks {text:?}");
}

#[then(expr = "the file {string} says the gate passed")]
fn file_says_passed(w: &mut GddWorld, path: String) {
    let text = fs::read_to_string(project_root(w).join(&path))
        .unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    let saved: serde_json::Value = serde_json::from_str(&text).expect("a JSON result");
    assert_eq!(saved["ok"], true, "{saved}");
    assert_eq!(saved, stdout_json(w), "the file differs from the output");
}

#[then(expr = "the log has a {string} line for {string} with result {string}")]
fn log_has_line(w: &mut GddWorld, gate: String, change: String, result: String) {
    let text = fs::read_to_string(project_root(w).join(".gdd/log.jsonl")).unwrap_or_default();
    let found = text
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .any(|r| {
            r["gate"] == gate.as_str()
                && r["change"] == change.as_str()
                && r["result"] == result.as_str()
        });
    assert!(
        found,
        "no {gate} line for {change} with result {result} in:\n{text}"
    );
}

// ---- features/changes.feature and features/approvals.feature -------------------------------
//
// API of the `gdd` crate these steps expect (the Builder in build mode implements it):
// - `gdd::run_as(args: &[String], cwd: &Path, role: Option<&str>) -> gdd::Outcome`: `run` with
//   the role given instead of read from `GDD_ROLE`.

#[given("everything is committed on main")]
fn everything_committed(w: &mut GddWorld) {
    let root = project_root(w);
    commit_all(&root, "project");
}

fn branch_exists(root: &Path, branch: &str) -> bool {
    std::process::Command::new("git")
        .args(["rev-parse", "-q", "--verify"])
        .arg(format!("refs/heads/{branch}"))
        .current_dir(root)
        .output()
        .expect("git can run")
        .status
        .success()
}

#[then(expr = "the branch {string} exists")]
fn branch_is_there(w: &mut GddWorld, branch: String) {
    assert!(
        branch_exists(&project_root(w), &branch),
        "no branch {branch}"
    );
}

#[then(expr = "the branch {string} does not exist")]
fn branch_is_not_there(w: &mut GddWorld, branch: String) {
    assert!(
        !branch_exists(&project_root(w), &branch),
        "the branch {branch} exists"
    );
}

#[then(expr = "the output does not contain {string}")]
fn output_lacks(w: &mut GddWorld, text: String) {
    let ran = last_run(w);
    assert!(
        ran.succeeded || w.exit_checked,
        "gdd failed: {:?}",
        ran.stderr
    );
    assert!(
        !ran.stdout.contains(&text),
        "the output {:?} contains {text:?}",
        ran.stdout
    );
}

#[then(expr = "the output contains {string} exactly {int} times")]
fn output_contains_times(w: &mut GddWorld, text: String, times: usize) {
    let ran = last_run(w);
    assert!(
        ran.succeeded || w.exit_checked,
        "gdd failed: {:?}",
        ran.stderr
    );
    let found = ran.stdout.matches(&text).count();
    assert_eq!(
        found, times,
        "the output {:?} has {text:?} {found} times",
        ran.stdout
    );
}

/// Rewrites `gdd/project.toml` with `stack_line` first and the other lines kept, but for any
/// earlier `stack` key.
fn set_project_stack(w: &mut GddWorld, stack_line: &str) {
    let path = project_root(w).join("gdd/project.toml");
    let current = fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path:?}: {e}"));
    let kept: Vec<&str> = current
        .lines()
        .filter(|line| {
            let key = line.split('=').next().unwrap_or("").trim();
            !line.contains('=') || key != "stack"
        })
        .collect();
    write_file(&path, &format!("{stack_line}\n{}\n", kept.join("\n")));
}

#[given(expr = "the project uses the stacks {string}")]
fn project_uses_stacks(w: &mut GddWorld, names: String) {
    let list: Vec<String> = names
        .split_whitespace()
        .map(|n| format!("\"{n}\""))
        .collect();
    set_project_stack(w, &format!("stack = [{}]", list.join(", ")));
}

#[given(expr = "the project uses the stack {string}")]
fn project_uses_stack(w: &mut GddWorld, name: String) {
    set_project_stack(w, &format!("stack = \"{name}\""));
}

#[given(expr = "the change {string} has a resolved request {string}")]
fn resolved_request(w: &mut GddWorld, id: String, text: String) {
    let root = project_root(w);
    let mut record = read_record(&root, &id);
    let inbox = record["inbox"].as_array_mut().expect("an inbox list");
    inbox.push(serde_json::json!({
        "kind": "needs",
        "text": text,
        "ts": "2026-01-01T00:00:00+00:00",
        "resolved": "2026-01-02T00:00:00+00:00",
    }));
    write_record(&root, &id, &record);
}

#[then(expr = "the file {string} holds the same text as the output")]
fn file_holds_output(w: &mut GddWorld, path: String) {
    let text = fs::read_to_string(project_root(w).join(&path))
        .unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    let ran = last_run(w);
    assert!(ran.succeeded, "gdd failed: {:?}", ran.stderr);
    assert!(!text.trim().is_empty(), "{path} is empty");
    assert_eq!(text.trim_end(), ran.stdout.trim_end());
}

#[given(expr = "the red zone paths become {string} and {string}")]
fn red_zone_paths_become(w: &mut GddWorld, first: String, second: String) {
    let root = project_root(w);
    let old = r#"paths = ["src/storage/**"]"#;
    assert!(w.zone_map.contains(old), "no red zone to change");
    let zones = w
        .zone_map
        .replace(old, &format!("paths = [\"{first}\", \"{second}\"]"));
    write_file(&root.join("gdd/zones.toml"), &format!("{zones}\n"));
}

#[given("the self-test passed")]
fn self_test_passed(w: &mut GddWorld) {
    let root = project_root(w);
    write_file(&root.join(".gdd/selftest.json"), r#"{"ok": true}"#);
}

/// Commits `content` as `path` on the branch of change `id`, and goes back to the branch we
/// were on: the file is on the branch, not in the working tree.
fn commit_on_branch(root: &Path, id: &str, path: &str, content: &str) {
    let back = git(root, &["rev-parse", "--abbrev-ref", "HEAD"]);
    git(root, &["checkout", "-q", &format!("change/{id}")]);
    write_file(&root.join(path), content);
    git(root, &["add", "--", path]);
    git(root, &["commit", "-q", "-m", &format!("add {path}")]);
    git(root, &["checkout", "-q", &back]);
}

#[given(expr = "the change {string} has the scenario {string} on its branch")]
fn scenario_on_branch(w: &mut GddWorld, id: String, file: String) {
    let root = project_root(w);
    let text = "Feature: Drafts\n\n  Scenario: One\n    Given a draft\n    And a name\n    Then it is saved\n";
    commit_on_branch(&root, &id, &file, text);
}

#[given(expr = "the change {string} has the hidden example {string} with the step {string}")]
fn hidden_example_on_branch(w: &mut GddWorld, id: String, file: String, sentence: String) {
    let root = project_root(w);
    let text = format!("Feature: Hidden drafts\n\n  Scenario: Other\n    {sentence}\n");
    commit_on_branch(&root, &id, &file, &text);
}

#[when(expr = "the role {string} runs {string}")]
fn role_runs(w: &mut GddWorld, role: String, command: String) {
    let root = project_root(w);
    run_gdd_as(w, &command, &root, Some(&role));
}

/// SHA-256 of `bytes`, as `sha256sum` prints it.
fn sha256(bytes: &[u8]) -> String {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("sha256sum can run");
    child
        .stdin
        .take()
        .expect("a pipe to sha256sum")
        .write_all(bytes)
        .expect("bytes for sha256sum");
    let out = child.wait_with_output().expect("sha256sum finishes");
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .expect("a hash")
        .to_string()
}

#[then(expr = "the record of {string} seals {string} with its current SHA-256")]
fn record_seals(w: &mut GddWorld, id: String, file: String) {
    let root = project_root(w);
    let shown = std::process::Command::new("git")
        .args(["show", &format!("change/{id}:{file}")])
        .current_dir(&root)
        .output()
        .expect("git can run");
    assert!(shown.status.success(), "{file} is not on the branch");
    let record = read_record(&root, &id);
    assert_eq!(
        record["gates"]["G1"]["hashes"][&file].as_str(),
        Some(sha256(&shown.stdout).as_str()),
        "hashes in {record}"
    );
}

#[then(expr = "the record of {string} has the gate {string} with decision {string} by {string}")]
fn record_has_gate(w: &mut GddWorld, id: String, gate: String, decision: String, by: String) {
    let record = read_record(&project_root(w), &id);
    let entry = &record["gates"][&gate];
    assert_eq!(
        entry["decision"],
        decision.as_str(),
        "gate {gate} in {record}"
    );
    assert_eq!(entry["by"], by.as_str(), "gate {gate} in {record}");
}

// ---- features/run-steps.feature ------------------------------------------------------------
//
// The recipe steps run on temporary git repositories. The models live in `gdd/models.toml`; the
// Setup and the red zones are approved through `gdd gate`, as a person would.

#[given(expr = "the models:")]
fn the_models(w: &mut GddWorld, step: &cucumber::gherkin::Step) {
    w.models = docstring(step, "a models docstring");
    let root = project_root(w);
    write_file(&root.join("gdd/models.toml"), &format!("{}\n", w.models));
}

/// `models` with the `[reviewer]` table replaced by the given integration, model and effort.
fn with_reviewer(models: &str, integration: &str, model: &str, effort: Option<&str>) -> String {
    let mut in_reviewer = false;
    let mut lines: Vec<String> = Vec::new();
    for line in models.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_reviewer = trimmed == "[reviewer]";
            lines.push(line.to_string());
            if in_reviewer {
                lines.push(format!("integration = \"{integration}\""));
                lines.push(format!("model = \"{model}\""));
                if let Some(effort) = effort {
                    lines.push(format!("effort = \"{effort}\""));
                }
            }
        } else if !in_reviewer {
            lines.push(line.to_string());
        }
    }
    format!("{}\n", lines.join("\n"))
}

/// Writes `gdd/models.toml`. Harness state, as in the Background: when the project is committed
/// already, commit it too.
fn write_models(w: &mut GddWorld, models: &str) {
    let root = project_root(w);
    write_file(&root.join("gdd/models.toml"), models);
    let changed = git(&root, &["status", "--porcelain", "--", "gdd/models.toml"]);
    let known = git(&root, &["log", "--oneline", "-1", "--", "gdd/models.toml"]);
    if !changed.trim().is_empty() && !known.trim().is_empty() {
        git(&root, &["add", "--", "gdd/models.toml"]);
        git(&root, &["commit", "-q", "-m", "models"]);
    }
}

fn set_reviewer(w: &mut GddWorld, integration: &str, model: &str, effort: Option<&str>) {
    assert!(w.models.contains("[reviewer]"), "no reviewer to change");
    let models = with_reviewer(&w.models, integration, model, effort);
    write_models(w, &models);
}

#[given(expr = "the reviewer is {string} {string}")]
fn reviewer_is(w: &mut GddWorld, integration: String, model: String) {
    set_reviewer(w, &integration, &model, None);
}

#[given(expr = "the reviewer is {string} {string} with effort {string}")]
fn reviewer_is_with_effort(w: &mut GddWorld, integration: String, model: String, effort: String) {
    set_reviewer(w, &integration, &model, Some(&effort));
}

#[given(expr = "the models pass {string} to the agents")]
fn models_pass(w: &mut GddWorld, name: String) {
    assert!(
        !w.models.contains("[env]"),
        "the models have an [env] already"
    );
    w.models = format!("{}\n[env]\npass = [\"{name}\"]\n", w.models.trim_end());
    let models = format!("{}\n", w.models);
    write_models(w, &models);
}

#[given(expr = "the environment has {string} set to {string}")]
fn environment_has(w: &mut GddWorld, name: String, value: String) {
    set_env(w, &name, &value);
}

#[given("the local state is gone")]
fn local_state_gone(w: &mut GddWorld) {
    let state = project_root(w).join(".gdd");
    fs::remove_dir_all(&state).expect("the project had local state");
    assert!(!state.exists(), "the local state is still there");
}

#[given(expr = "the Claude Code session {string} used {string} with {int} output tokens")]
fn claude_session_used(w: &mut GddWorld, session: String, model: String, output: u64) {
    static REQUESTS: AtomicUsize = AtomicUsize::new(0);
    let root = project_root(w)
        .canonicalize()
        .expect("the canonical project root");
    let home = w.home.get_or_insert_with(TempDir::new).0.clone();
    set_env(w, "HOME", &home.to_string_lossy());
    let folder: String = root
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let path = home
        .join(".claude/projects")
        .join(folder)
        .join(format!("{session}.jsonl"));
    let line = serde_json::json!({
        "requestId": format!("req-{}", REQUESTS.fetch_add(1, Ordering::Relaxed)),
        "message": {
            "model": model,
            "usage": {
                "input_tokens": 10,
                "cache_creation_input_tokens": 0,
                "cache_read_input_tokens": 5,
                "output_tokens": output,
            },
        },
    });
    let old = fs::read_to_string(&path).unwrap_or_default();
    write_file(&path, &format!("{old}{line}\n"));
}

#[given("the Setup is done")]
fn setup_done(w: &mut GddWorld) {
    let root = project_root(w);
    write_file(&root.join("docs/brief.md"), "# Project Brief\n");
    write_file(&root.join(".gdd/selftest.json"), r#"{"ok": true}"#);
    run_gdd(w, "gdd gate G0 ok --by machines", &root);
    assert!(last_run(w).succeeded, "{}", last_run(w).stderr);
}

#[given("the red zones are approved")]
fn red_zones_approved(w: &mut GddWorld) {
    let root = project_root(w);
    run_gdd(w, "gdd gate zones ok --by human", &root);
    assert!(last_run(w).succeeded, "{}", last_run(w).stderr);
}

#[given(expr = "I am on the branch {string}")]
fn on_the_branch(w: &mut GddWorld, branch: String) {
    git(&project_root(w), &["checkout", "-q", &branch]);
}

#[given(expr = "the log has a Builder call in build mode for {string}")]
fn log_has_builder_call(w: &mut GddWorld, change: String) {
    let path = project_root(w).join(".gdd/log.jsonl");
    let old = fs::read_to_string(&path).unwrap_or_default();
    let line = serde_json::json!({
        "ts": "2026-01-01T00:00:00+00:00",
        "change": change,
        "phase": "2-build",
        "gate": "-",
        "role": "builder-build",
        "agent": "claude",
        "model": "sonnet",
        "result": "ok",
    });
    write_file(&path, &format!("{old}{line}\n"));
}

#[then(expr = "the record of {string} is in state {string}")]
fn record_state(w: &mut GddWorld, id: String, state: String) {
    let record = read_record(&project_root(w), &id);
    assert_eq!(record["state"], state.as_str(), "record {record}");
}

#[then(expr = "the file {string} contains {string}")]
fn file_contains(w: &mut GddWorld, path: String, text: String) {
    let content = fs::read_to_string(project_root(w).join(&path))
        .unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    assert!(content.contains(&text), "{path} lacks {text:?}: {content}");
}

#[given(expr = "the file {string} says {string}")]
fn file_says(w: &mut GddWorld, path: String, text: String) {
    let root = project_root(w);
    write_file(&root.join(&path), &format!("{text}\n"));
}

#[then(expr = "the file {string} does not exist")]
fn file_does_not_exist(w: &mut GddWorld, path: String) {
    assert!(
        !project_root(w).join(&path).exists(),
        "{path} exists, and it should not"
    );
}

#[then(expr = "agent call {int} was {string}")]
fn agent_call_was(w: &mut GddWorld, number: usize, arguments: String) {
    let calls = fs::read_to_string(project_root(w).join(".gdd/fake/argv")).unwrap_or_default();
    let call = calls.lines().nth(number.saturating_sub(1));
    assert_eq!(call, Some(arguments.as_str()), "the calls were:\n{calls}");
}

#[then(expr = "the branch {string} holds {string}")]
fn branch_holds(w: &mut GddWorld, branch: String, path: String) {
    // `git` fails the step when the branch has no such file.
    git(
        &project_root(w),
        &["cat-file", "-e", &format!("{branch}:{path}")],
    );
}

#[then(expr = "the last commit on {word} says {string}")]
fn last_commit_says(w: &mut GddWorld, branch: String, text: String) {
    let message = git(&project_root(w), &["log", "-1", "--format=%B", &branch]);
    assert!(
        message.contains(&text),
        "the last commit on {branch} says {message:?}, not {text:?}"
    );
}

// ---- features/recipe.feature ---------------------------------------------------------------
//
// The agents are shell scripts named `claude`, `codex` and `opencode` in `.gdd/bin/` of the
// temporary project, which the scenario's `PATH` starts with. They check how they were started
// (project root as working directory, `GDD_CHANGE`, `GDD_ROOT` and the `GDD_ROLE` of their
// command set), record each call: its full arguments as a line of `.gdd/fake/argv`, the names of
// its environment variables in `.gdd/fake/env/<call>` and `role|model|arguments` in
// `.gdd/fake/calls`. They print their agent's JSON output (100 input, 50 cached and 20 output
// tokens, Codex and OpenCode 5 reasoning ones too) and act on what the steps left in
// `.gdd/fake/`: `build` (`default`, `nothing`, `never` or `from <call>`), `build_path`, `replan`,
// `reviews` (one answer per line, the last one repeats) and `reviewer_writes` (a path).

const FAKE_AGENT: &str = r##"#!/bin/sh
integration=${0##*/}
[ -n "$GDD_ROOT" ] || exit 90
[ -n "$GDD_CHANGE" ] || exit 91
[ -n "$GDD_ROLE" ] || exit 92
[ "$(pwd -P)" = "$(cd "$GDD_ROOT" && pwd -P)" ] || exit 93
mkdir -p "$GDD_ROOT/.gdd/fake/env"
echo "$*" >> "$GDD_ROOT/.gdd/fake/argv"
call=$(wc -l < "$GDD_ROOT/.gdd/fake/argv" | tr -d ' ')
env | sed -n 's/^\([A-Za-z_][A-Za-z0-9_]*\)=.*/\1/p' > "$GDD_ROOT/.gdd/fake/env/$call"
prev=
prompt=
model=
cmd=
last=
for a in "$@"; do
  case "$prev" in
    -p) prompt=$a ;;
    --model|-m) model=$a ;;
    --command) cmd=${a#gdd-} ;;
  esac
  case "$a" in
    '$gdd-'*) prompt=$a ;;
  esac
  prev=$a
  last=$a
done
[ -n "$model" ] || exit 94
case "$integration" in
  claude) cmd=${prompt#/gdd-} ;;
  codex) cmd=${prompt#'$gdd-'} ;;
  opencode) args=$last ;;
  *) exit 94 ;;
esac
if [ "$integration" != opencode ]; then
  args=${cmd#* }
  [ "$args" != "$cmd" ] || exit 95
  cmd=${cmd%% *}
fi
case "$cmd" in
  build-test) role=builder-test ;;
  build) role=builder-build ;;
  review) role=reviewer ;;
  replan) role=orchestrator ;;
  *) exit 96 ;;
esac
[ "$GDD_ROLE" = "$role" ] || exit 92
fake="$GDD_ROOT/.gdd/fake"
echo "$role|$model|$args" >> "$fake/calls"
n=$(grep -c "^$role|" "$fake/calls")
text=done
case "$role" in
  builder-test)
    mkdir -p tests/steps
    echo "$n" > tests/steps/app.txt
    ;;
  builder-build)
    mode=default
    [ -f "$fake/build" ] && mode=$(cat "$fake/build")
    path=src/app.txt
    [ -f "$fake/build_path" ] && path=$(cat "$fake/build_path")
    write=yes
    case "$mode" in
      nothing) write=no ;;
      never)
        sleep 1000 &
        echo $! >> "$fake/pids"
        echo $$ >> "$fake/pids"
        wait
        write=no
        ;;
      from\ *) [ "$n" -ge "${mode#from }" ] || write=no ;;
    esac
    if [ "$write" = yes ]; then
      mkdir -p "$(dirname "$path")"
      echo "$n" > "$path"
    fi
    ;;
  reviewer)
    answer="VERDICT: OK"
    if [ -f "$fake/reviews" ]; then
      answer=$(sed -n "${n}p" "$fake/reviews")
      [ -n "$answer" ] || answer=$(tail -n 1 "$fake/reviews")
    fi
    if [ -f "$fake/reviewer_writes" ]; then
      path=$(cat "$fake/reviewer_writes")
      mkdir -p "$(dirname "$path")"
      echo "the Reviewer was here" > "$path"
    fi
    text=$answer
    ;;
  orchestrator)
    answer="NEXT: build"
    [ -f "$fake/replan" ] && answer=$(cat "$fake/replan")
    mkdir -p "$GDD_ROOT/.gdd/changes/$GDD_CHANGE"
    printf '%s\n' "$answer" > "$GDD_ROOT/.gdd/changes/$GDD_CHANGE/replan.md"
    text=$answer
    ;;
esac
text=$(printf '%s' "$text" | sed 's/\\/\\\\/g; s/"/\\"/g')
case "$integration" in
  claude)
    printf '{"type":"result","result":"%s","session_id":"fake-session","total_cost_usd":0.01,"usage":{"input_tokens":100,"cache_creation_input_tokens":0,"cache_read_input_tokens":50,"output_tokens":20}}\n' "$text"
    ;;
  codex)
    printf '{"type":"turn.completed","usage":{"input_tokens":100,"cached_input_tokens":50,"output_tokens":20,"reasoning_output_tokens":5}}\n'
    printf '{"type":"item.completed","item":{"type":"agent_message","text":"%s"}}\n' "$text"
    ;;
  opencode)
    printf '{"type":"step_finish","part":{"tokens":{"input":100,"output":20,"reasoning":5,"cache":{"read":50}},"cost":0}}\n'
    printf '{"type":"text","part":{"text":"%s"}}\n' "$text"
    ;;
esac
exit 0
"##;

fn fake_file(w: &mut GddWorld, name: &str, content: &str) {
    let path = project_root(w).join(".gdd/fake").join(name);
    write_file(&path, content);
}

#[given("the agents are fakes")]
fn agents_are_fakes(w: &mut GddWorld) {
    use std::os::unix::fs::PermissionsExt;
    let root = project_root(w);
    for integration in ["claude", "codex", "opencode"] {
        let path = root.join(".gdd/bin").join(integration);
        write_file(&path, FAKE_AGENT);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("an executable");
    }
    // The fakes come first on the PATH of the environment gdd runs in.
    let path = std::env::var("PATH").unwrap_or_default();
    let with_fakes = format!("{}:{path}", root.join(".gdd/bin").display());
    set_env(w, "PATH", &with_fakes);
}

#[given(expr = "the Reviewer writes {string}")]
fn reviewer_writes(w: &mut GddWorld, path: String) {
    fake_file(w, "reviewer_writes", &path);
}

/// The full arguments of agent call `number` (from 1), as the fakes recorded them.
fn agent_call(w: &GddWorld, number: usize) -> String {
    let calls = fs::read_to_string(project_root_of(w).join(".gdd/fake/argv")).unwrap_or_default();
    calls
        .lines()
        .nth(number.saturating_sub(1))
        .unwrap_or_else(|| panic!("no agent call {number}; the calls were:\n{calls}"))
        .to_string()
}

fn project_root_of(w: &GddWorld) -> PathBuf {
    w.project.as_ref().expect("a project").0.clone()
}

#[then(expr = "agent call {int} starts with {string}")]
fn agent_call_starts(w: &mut GddWorld, number: usize, start: String) {
    let call = agent_call(w, number);
    assert!(call.starts_with(&start), "call {number} was {call:?}");
}

#[then(expr = "agent call {int} includes {string}")]
fn agent_call_includes(w: &mut GddWorld, number: usize, text: String) {
    let call = agent_call(w, number);
    assert!(call.contains(&text), "call {number} was {call:?}");
}

/// Whether agent call `number` had the environment variable `name`.
fn agent_got(w: &GddWorld, number: usize, name: &str) -> bool {
    agent_call(w, number); // fails when there is no such call
    let path = project_root_of(w).join(format!(".gdd/fake/env/{number}"));
    let names = fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path:?}: {e}"));
    names.lines().any(|line| line == name)
}

#[then(expr = "agent call {int} got {string}")]
fn agent_call_got(w: &mut GddWorld, number: usize, name: String) {
    assert!(
        agent_got(w, number, &name),
        "call {number} did not get {name}"
    );
}

#[then(expr = "agent call {int} did not get {string}")]
fn agent_call_did_not_get(w: &mut GddWorld, number: usize, name: String) {
    assert!(!agent_got(w, number, &name), "call {number} got {name}");
}

#[then(expr = "the log line of agent call {int} has {string} {int}")]
fn log_line_of_call_has(w: &mut GddWorld, number: usize, field: String, value: i64) {
    let text = fs::read_to_string(project_root(w).join(".gdd/log.jsonl")).unwrap_or_default();
    // The lines of agent calls say which command they ran in their note.
    let lines: Vec<serde_json::Value> = text
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|r| r["note"].as_str().is_some_and(|n| n.starts_with("/gdd-")))
        .collect();
    let line = lines
        .get(number.saturating_sub(1))
        .unwrap_or_else(|| panic!("no log line for agent call {number} in:\n{text}"));
    assert_eq!(line[&field].as_i64(), Some(value), "{field} in {line}");
}

#[given("the Builder in build mode writes nothing")]
fn builder_writes_nothing(w: &mut GddWorld) {
    fake_file(w, "build", "nothing");
}

#[given("the Builder in build mode never answers")]
fn builder_never_answers(w: &mut GddWorld) {
    fake_file(w, "build", "never");
}

#[given(expr = "the Builder in build mode writes {string} from call {int}")]
fn builder_writes_from(w: &mut GddWorld, path: String, call: usize) {
    fake_file(w, "build_path", &path);
    fake_file(w, "build", &format!("from {call}"));
}

#[given(expr = "the replan answers {string}")]
fn replan_answers(w: &mut GddWorld, answer: String) {
    fake_file(w, "replan", &answer);
}

#[given(expr = "the Reviewer answers {string} and then {string}")]
fn reviewer_answers_then(w: &mut GddWorld, first: String, second: String) {
    fake_file(w, "reviews", &format!("{first}\n{second}\n"));
}

#[given(expr = "the Reviewer answers {string} every time")]
fn reviewer_answers_always(w: &mut GddWorld, answer: String) {
    fake_file(w, "reviews", &format!("{answer}\n"));
}

#[given(expr = "the agent time limit is {int} seconds")]
fn agent_time_limit(w: &mut GddWorld, seconds: u64) {
    let root = project_root(w);
    let path = root.join("gdd/rules.toml");
    let rules = fs::read_to_string(&path).expect("the project's rules");
    assert!(!rules.contains("[limits]"), "the rules already have limits");
    write_file(
        &path,
        &format!("{rules}\n[limits]\nagent_timeout_s = {seconds}\n"),
    );
    // Harness state from before the change: committed, so the working tree stays clean.
    git(&root, &["add", "--", "gdd/rules.toml"]);
    git(&root, &["commit", "-q", "-m", "agent time limit"]);
}

#[then("the agents were called in this order:")]
fn agents_called_in_order(w: &mut GddWorld, step: &cucumber::gherkin::Step) {
    let table = step.table.as_ref().expect("a table of calls");
    let expected: Vec<String> = table
        .rows
        .iter()
        .skip(1)
        .map(|row| {
            let cells: Vec<&str> = row.iter().map(|c| c.trim()).collect();
            cells.join("|")
        })
        .collect();
    let calls = fs::read_to_string(project_root(w).join(".gdd/fake/calls")).unwrap_or_default();
    let actual: Vec<String> = calls.lines().map(str::to_string).collect();
    assert_eq!(actual, expected, "the calls the agents recorded");
}

/// Whether process `pid` is gone (or a zombie that only waits to be collected).
fn process_is_gone(pid: &str) -> bool {
    match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Err(_) => true,
        Ok(stat) => {
            stat.rsplit_once(')')
                .and_then(|(_, rest)| rest.split_whitespace().next())
                == Some("Z")
        }
    }
}

#[then("no process the agent started is still running")]
fn no_agent_process_left(w: &mut GddWorld) {
    let pids = fs::read_to_string(project_root(w).join(".gdd/fake/pids"))
        .expect("the agent recorded the processes it started");
    let pids: Vec<&str> = pids.split_whitespace().collect();
    assert!(!pids.is_empty(), "the agent recorded no process");
    for pid in pids {
        assert!(process_is_gone(pid), "process {pid} is still running");
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root")
}

/// The doc string of a step without the line break that follows the opening quotes:
/// the gherkin parser keeps it, so every doc string would otherwise start with an empty line.
fn docstring(step: &cucumber::gherkin::Step, what: &str) -> String {
    let text = step.docstring.as_deref().expect(what);
    text.strip_prefix('\n').unwrap_or(text).to_string()
}

fn main() {
    let root = repo_root();
    let listed: HashSet<PathBuf> = std::env::var("GDD_FEATURES")
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter_map(|l| root.join(l).canonicalize().ok())
        .collect();
    let hidden = std::env::var("GDD_HIDDEN").as_deref() == Ok("1");
    let hidden_dir = root.join("features").join("hidden");
    futures::executor::block_on(
        GddWorld::cucumber()
            .fail_on_skipped()
            .with_default_cli()
            .filter_run_and_exit(root.join("features"), move |feature, _, _| {
                let Some(path) = feature.path.as_ref().and_then(|p| p.canonicalize().ok()) else {
                    return false;
                };
                if path.starts_with(&hidden_dir) {
                    return hidden;
                }
                listed.is_empty() || listed.contains(&path)
            }),
    );
}
