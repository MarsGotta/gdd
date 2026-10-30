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
    /// Where the scenario's fake programs live; first on its PATH. Outside the project folder.
    programs: Option<TempDir>,
    /// The Setup form the scenario opened.
    form: Option<OpenForm>,
    /// `form::finish` already ran on the form's outcome.
    form_finished: bool,
    /// The Board the scenario opened.
    board: Option<OpenBoard>,
    /// The sitting the scenario opened.
    sitting: Option<OpenSitting>,
    /// The G0 opened during the Builder call: whether it succeeded, and its output.
    g0_during_call: Option<std::thread::JoinHandle<(bool, String)>>,
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
    if w.project.is_some() {
        write_file(
            &project_root_of(w).join("gdd/zones.toml"),
            &format!("{}\n", w.zone_map),
        );
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
    // A change made with `gdd new-change` shows its color in the JSON the command printed.
    if w.outcome.is_none() {
        assert_eq!(stdout_json(w)["color"], color.as_str());
        return;
    }
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
    if w.project.is_none() {
        let dir = scenario_folder();
        write_file(
            &dir.0.join("project/gdd/project.toml"),
            "name = \"demo\"\nlanguage = \"en\"\n",
        );
        w.project = Some(dir);
    }
    project_root_of(w)
}

/// The scenario's own folder, with the project inside it as `project`: what hangs off the
/// project's parent (its worktrees, `../trees`...) goes with the scenario.
fn scenario_folder() -> TempDir {
    let dir = TempDir::new();
    fs::create_dir_all(dir.0.join("project")).expect("a project folder");
    dir
}

/// The default folder of the worktrees of the project at `root`: `<root>.worktrees`.
fn worktrees_folder_of(root: &Path) -> PathBuf {
    let mut name = root.as_os_str().to_os_string();
    name.push(".worktrees");
    PathBuf::from(name)
}

fn run_gdd(w: &mut GddWorld, command: &str, cwd: &Path) {
    run_gdd_as(w, command, cwd, None);
}

/// The environment gdd runs in for this scenario: the process one, minus the variables gdd's own
/// agents set. Steps never change the process environment; each scenario carries its own.
fn world_env(w: &mut GddWorld) -> &mut Vec<(String, String)> {
    w.env.get_or_insert_with(|| {
        std::env::vars()
            .filter(|(name, _)| {
                !matches!(
                    name.as_str(),
                    "GDD_ROLE" | "GDD_CHANGE" | "GDD_ROOT" | "GDD_STATE"
                )
            })
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

#[given("the log says:")]
fn the_log_says(w: &mut GddWorld, step: &cucumber::gherkin::Step) {
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
    w.exit_checked = true;
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
    w.exit_checked = true;
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
    // A run that failed (and was checked) may say it on either stream: the self-test and CI
    // print their report on stdout, a refusal goes to stderr.
    let output = if ran.succeeded {
        ran.stdout.clone()
    } else {
        format!("{}{}", ran.stdout, ran.stderr)
    };
    assert!(
        output.contains(&text),
        "the output {output:?} does not contain {text:?}"
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

#[when(expr = "the change writes {string} saying:")]
fn change_writes_saying(w: &mut GddWorld, path: String, step: &cucumber::gherkin::Step) {
    let content = docstring(step, "the step needs a doc string with the file content");
    write_file(&project_root(w).join(&path), &format!("{content}\n"));
}

#[given(expr = "the file {string} says:")]
fn file_says_docstring(w: &mut GddWorld, path: String, step: &cucumber::gherkin::Step) {
    let content = docstring(step, "the step needs a doc string with the file content");
    write_file(&project_root(w).join(&path), &format!("{content}\n"));
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
    let mut commands = docstring(step, "a commands docstring");
    // A scenario that merges says nothing about the release: it runs simulated, like `gdd init` leaves it.
    if !commands.contains("[release]") {
        commands.push_str("\n\n[release]\nsimulated = true");
    }
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
    commit_harness_file(&root, "gdd/commands.toml", "gate commands");
}

/// The command `name` of the `gate` section of the sheet `stack`, as `gdd stacks show` prints it.
fn sheet_command(stack: &str, gate: &str, name: &str) -> String {
    let sheet = gdd_core::stacks::sheet(stack).expect("a built-in sheet");
    let prefix = format!("{gate}.{name}: ");
    let shown = sheet.to_string();
    let line = shown.lines().find_map(|line| line.strip_prefix(&prefix));
    line.unwrap_or_else(|| panic!("the {stack} sheet has no command {gate}.{name}"))
        .to_string()
}

#[given(expr = "the gate command {string} {string} is the one of the {string} sheet")]
fn gate_command_is_the_sheets(w: &mut GddWorld, gate: String, name: String, stack: String) {
    let root = project_root(w);
    let path = root.join("gdd/commands.toml");
    let toml = fs::read_to_string(&path).expect("gate commands to edit");
    // Gate commands run in the process environment, not the scenario's: the command puts the
    // folder of the scenario's fake programs first on its own PATH.
    let programs = w
        .programs
        .get_or_insert_with(TempDir::new)
        .0
        .display()
        .to_string();
    // G3 fails with no scenarios to count, and these scenarios are about the mutation command.
    let feature = "features/counted.feature";
    write_file(
        &root.join(feature),
        "Feature: counted\n\n  Scenario: one\n    Given a thing\n",
    );
    commit_harness_file(&root, feature, "a scenario to count");
    let command = sheet_command(&stack, &gate, &name);
    let command = format!("PATH='{programs}':\"$PATH\"; export PATH; {command}");
    write_file(&path, &with_command(&toml, &gate, &name, &command));
    commit_harness_file(&root, "gdd/commands.toml", "gate commands");
}

/// Commits a file of the harness and moves the start of the build there: a Given on the harness
/// is state from before the build; what the change writes goes in a When.
fn commit_harness_file(root: &Path, file: &str, message: &str) {
    git(root, &["add", "--", file]);
    git(root, &["commit", "-q", "-m", message]);
    let head = git(root, &["rev-parse", "HEAD"]);
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
    let current = git(root, &["branch", "--show-current"]);
    for id in ids {
        let mut record = read_record(root, &id);
        if record.get("build_start").is_some() && current == format!("change/{id}") {
            record["build_start"] = head.clone().into();
            write_record(root, &id, &record);
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

fn log_lines_of_command(w: &mut GddWorld, command: &str, change: &str) -> Vec<serde_json::Value> {
    let text = fs::read_to_string(project_root(w).join(".gdd/log.jsonl")).unwrap_or_default();
    text.lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|r| r["role"] == "machine" && r["agent"] == command && r["change"] == change)
        .collect()
}

#[then(expr = "the log has the gate command {string} of {string} in {string} with result {string}")]
fn log_has_gate_command(
    w: &mut GddWorld,
    command: String,
    change: String,
    gate: String,
    result: String,
) {
    let lines = log_lines_of_command(w, &command, &change);
    assert!(
        lines
            .iter()
            .any(|r| r["gate"] == gate.as_str() && r["result"] == result.as_str()),
        "no {gate} line of the command {command} for {change} with result {result} in: {lines:?}"
    );
}

#[then(expr = "the log has no gate command {string} of {string}")]
fn log_has_no_gate_command(w: &mut GddWorld, command: String, change: String) {
    let lines = log_lines_of_command(w, &command, &change);
    assert!(
        lines.is_empty(),
        "the log has lines of the command {command} for {change}: {lines:?}"
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
    // Off main (a change branch), the commit must still land on main: a fast-forward.
    if git(&root, &["branch", "--show-current"]) != "main" {
        git(&root, &["branch", "-f", "main", "HEAD"]);
    }
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

/// The entries of `reviews` in the record of `id`; a record without the field has none.
fn record_reviews(w: &mut GddWorld, id: &str) -> Vec<serde_json::Value> {
    let record = read_record(&project_root(w), id);
    record["reviews"].as_array().cloned().unwrap_or_default()
}

#[then(expr = "the record of {string} lists the reviews {string}")]
fn record_lists_reviews(w: &mut GddWorld, id: String, verdicts: String) {
    let found: Vec<String> = record_reviews(w, &id)
        .iter()
        .map(|r| r["verdict"].as_str().unwrap_or("none").to_string())
        .collect();
    assert_eq!(found.join(", "), verdicts, "reviews of {id}: {found:?}");
}

#[then(expr = "review {int} of {string} lists the changed file {string}")]
fn review_lists_changed_file(w: &mut GddWorld, n: usize, id: String, file: String) {
    let reviews = record_reviews(w, &id);
    let entry = n
        .checked_sub(1)
        .and_then(|i| reviews.get(i))
        .unwrap_or_else(|| panic!("review {n} of {id} is not in the record: {reviews:?}"));
    let changed = entry["changed"].as_array().cloned().unwrap_or_default();
    assert!(
        changed.iter().any(|c| c == file.as_str()),
        "review {n} of {id} lacks {file}: {entry}"
    );
}

#[then(expr = "review {int} of {string} lists no changed files")]
fn review_lists_no_changed_files(w: &mut GddWorld, n: usize, id: String) {
    let reviews = record_reviews(w, &id);
    let entry = n
        .checked_sub(1)
        .and_then(|i| reviews.get(i))
        .unwrap_or_else(|| panic!("review {n} of {id} is not in the record: {reviews:?}"));
    let changed = entry["changed"].as_array().map_or(0, Vec::len);
    assert_eq!(changed, 0, "review {n} of {id} lists changes: {entry}");
}

#[then(expr = "the file {string} contains {string}")]
fn file_contains(w: &mut GddWorld, path: String, text: String) {
    // The step text escapes the quotes inside the text.
    let text = text.replace("\\\"", "\"");
    let content = fs::read_to_string(project_root(w).join(&path))
        .unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    assert!(content.contains(&text), "{path} lacks {text:?}: {content}");
}

#[then(expr = "the file {string} contains {string} exactly {int} times")]
fn file_contains_times(w: &mut GddWorld, path: String, text: String, times: usize) {
    // The step text escapes the quotes inside the text.
    let text = text.replace("\\\"", "\"");
    let content = fs::read_to_string(project_root(w).join(&path))
        .unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    let found = content.matches(&text).count();
    assert_eq!(
        found, times,
        "{path} has {text:?} {found} times, not {times}: {content}"
    );
}

#[then(expr = "the file {string} does not contain {string}")]
fn file_does_not_contain(w: &mut GddWorld, path: String, text: String) {
    // The step text escapes the quotes inside the text.
    let text = text.replace("\\\"", "\"");
    let content = fs::read_to_string(project_root(w).join(&path))
        .unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    assert!(
        !content.contains(&text),
        "{path} contains {text:?}: {content}"
    );
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

#[given(expr = "the tag {string} is on main")]
fn tag_is_on_main(w: &mut GddWorld, tag: String) {
    git(&project_root(w), &["tag", &tag, "main"]);
}

#[then(expr = "in the file {string}, {string} comes before {string}")]
fn file_text_comes_before(w: &mut GddWorld, path: String, first: String, second: String) {
    let content = fs::read_to_string(project_root(w).join(&path))
        .unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    let at_first = content
        .find(&first)
        .unwrap_or_else(|| panic!("{path} lacks {first:?}: {content}"));
    let at_second = content
        .find(&second)
        .unwrap_or_else(|| panic!("{path} lacks {second:?}: {content}"));
    assert!(
        at_first < at_second,
        "in {path}, {first:?} is not before {second:?}: {content}"
    );
}

#[given(expr = "in the file {string}, {string} is replaced by {string}")]
fn file_text_is_replaced(w: &mut GddWorld, path: String, old: String, new: String) {
    let file = project_root(w).join(&path);
    let content = fs::read_to_string(&file).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    assert!(content.contains(&old), "{path} lacks {old:?}: {content}");
    write_file(&file, &content.replacen(&old, &new, 1));
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

#[then(expr = "the last commit on {word} does not say {string}")]
fn last_commit_does_not_say(w: &mut GddWorld, branch: String, text: String) {
    let message = git(&project_root(w), &["log", "-1", "--format=%B", &branch]);
    assert!(
        !message.contains(&text),
        "the last commit on {branch} says {message:?}, and it should not say {text:?}"
    );
}

// ---- features/recipe.feature ---------------------------------------------------------------
//
// The agents are shell scripts named `claude`, `codex` and `opencode` in `.gdd/bin/` of the
// temporary project, which the scenario's `PATH` starts with. They check how they were started
// (the change's worktree as working directory and `GDD_ROOT`, `GDD_STATE`, `GDD_CHANGE` and the
// `GDD_ROLE` of their command set), record each call under `$GDD_STATE/.gdd/fake/`: its full
// arguments as a line of `argv`, its working directory in `cwds`, the names of its environment
// variables in `env/<call>` and `role|model|arguments` in `calls`. They print their agent's JSON output (100 input, 50 cached and 20 output
// tokens, Codex and OpenCode 5 reasoning ones too) and act on what the steps left in
// `.gdd/fake/`: `build` (`default`, `nothing <ids>`, `never` or `from <call>`), `build_path`, `replan`,
// `build_also` and `build_also_content` (a path and its text, written by every build call),
// `reviews` (one answer per line, the last one repeats), `reviewer_writes` (a path), `record`
// (`<change>/<file>` of the state to append a line to during a build call) and `log_line`
// (`<change>|<build call>`: that call appends a line of the log for the change) and `main_gets`
// (a path the first Reviewer call commits on main in the main checkout). `linger` (seconds): the
// first build-mode Builder call touches `in-call`, sleeps that long and removes `linger`.

const FAKE_AGENT: &str = r##"#!/bin/sh
integration=${0##*/}
[ -n "$GDD_ROOT" ] || exit 90
[ -n "$GDD_CHANGE" ] || exit 91
[ -n "$GDD_ROLE" ] || exit 92
[ -n "$GDD_STATE" ] || exit 97
[ "$(pwd -P)" = "$(cd "$GDD_ROOT" && pwd -P)" ] || exit 93
mkdir -p "$GDD_STATE/.gdd/fake/env"
echo "$*" >> "$GDD_STATE/.gdd/fake/argv"
pwd -P >> "$GDD_STATE/.gdd/fake/cwds"
call=$(wc -l < "$GDD_STATE/.gdd/fake/argv" | tr -d ' ')
env | sed -n 's/^\([A-Za-z_][A-Za-z0-9_]*\)=.*/\1/p' > "$GDD_STATE/.gdd/fake/env/$call"
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
fake="$GDD_STATE/.gdd/fake"
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
    if [ -f "$fake/linger" ]; then
      seconds=$(cat "$fake/linger")
      touch "$fake/in-call"
      sleep "$seconds"
      rm -f "$fake/linger"
    fi
    write=yes
    case "$mode" in
      nothing\ *) case " ${mode#nothing } " in *" $GDD_CHANGE "*) write=no ;; esac ;;
      never)
        sleep 1000 &
        echo $! >> "$fake/pids"
        echo $$ >> "$fake/pids"
        wait
        write=no
        ;;
      from\ *) [ "$n" -ge "${mode#from }" ] || write=no ;;
    esac
    also_write=yes
    if [ -f "$fake/nothing_variants" ]; then
      for v in $(cat "$fake/nothing_variants"); do
        case " $args " in *" variant=$v "*) write=no; also_write=no ;; esac
      done
    fi
    if [ "$write" = yes ]; then
      mkdir -p "$(dirname "$path")"
      echo "$n" > "$path"
    fi
    if [ "$also_write" = yes ] && [ -f "$fake/build_also" ]; then
      also=$(cat "$fake/build_also")
      mkdir -p "$(dirname "$also")"
      cp "$fake/build_also_content" "$also"
    fi
    if [ -f "$fake/delete" ]; then
      rm -rf "$(cat "$fake/delete")"
    fi
    if [ -f "$fake/swap" ]; then
      swap=$(cat "$fake/swap")
      target=${swap%%|*}
      marker=${swap#*|}
      [ -f "$fake/swap_original" ] || cp "$target" "$fake/swap_original"
      {
        echo "[g2]"
        echo "aaa_restore = \"cp '$fake/swap_original' '$target' && touch '$marker'\""
      } > "$target"
    fi
    if [ -f "$fake/plant" ] && [ "$n" = 1 ]; then
      plant=$(cat "$fake/plant")
      target=${plant%%|*}
      marker=${plant#*|}
      awk -v line="planted_check = \"touch $marker\"" '{ print } /^\[g2\]/ { print line }' "$target" > "$target.tmp"
      mv "$target.tmp" "$target"
      unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE
      git add -- "$target" || exit 1
      git -c user.name="GDD Test" -c user.email=test@example.com -c commit.gpgsign=false commit -q -m "the Builder plants a G2 command" -- "$target" || exit 1
      git show "HEAD:$target" | grep -q planted_check || exit 1
    fi
    if [ -f "$fake/record" ]; then
      record=$(cat "$fake/record")
      mkdir -p "$(dirname "$GDD_STATE/.gdd/changes/$record")"
      echo "touched by the Builder" >> "$GDD_STATE/.gdd/changes/$record"
    fi
    if [ -f "$fake/log_line" ]; then
      log_line=$(cat "$fake/log_line")
      if [ "$n" = "${log_line#*|}" ]; then
        printf '{"ts":"2026-01-01T00:00:00+00:00","change":"%s","phase":"2-build","gate":"-","role":"builder-build","agent":"claude","model":"sonnet","result":"ok"}\n' "${log_line%%|*}" >> "$GDD_STATE/.gdd/log.jsonl"
      fi
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
    if [ -f "$fake/main_gets" ] && [ "$n" = 1 ]; then
      path=$(cat "$fake/main_gets")
      mkdir -p "$(dirname "$GDD_STATE/$path")"
      echo "main moved on" > "$GDD_STATE/$path"
      unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE
      git -C "$GDD_STATE" add -- "$path"
      git -C "$GDD_STATE" -c user.name="GDD Test" -c user.email=test@example.com -c commit.gpgsign=false commit -q -m "main moves on" -- "$path"
    fi
    text=$answer
    ;;
  orchestrator)
    answer="NEXT: build"
    [ -f "$fake/replan" ] && answer=$(cat "$fake/replan")
    mkdir -p "$GDD_STATE/.gdd/changes/$GDD_CHANGE"
    printf '%s\n' "$answer" > "$GDD_STATE/.gdd/changes/$GDD_CHANGE/replan.md"
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
    w.project.as_ref().expect("a project").0.join("project")
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

#[then(expr = "agent call {int} does not include {string}")]
fn agent_call_does_not_include(w: &mut GddWorld, number: usize, text: String) {
    let call = agent_call(w, number);
    assert!(!call.contains(&text), "call {number} was {call:?}");
}

#[given(expr = "the Builder session is {string}")]
fn builder_session_is(w: &mut GddWorld, value: String) {
    assert!(
        !w.models.contains("[session]"),
        "the models have a [session] already"
    );
    w.models = format!(
        "{}\n[session]\nbuilder = \"{value}\"\n",
        w.models.trim_end()
    );
    let models = format!("{}\n", w.models);
    write_models(w, &models);
}

#[given(expr = "the yellow Builder is {string} with the model {string}")]
fn yellow_builder_is(w: &mut GddWorld, integration: String, model: String) {
    let mut in_yellow = false;
    let mut lines: Vec<String> = Vec::new();
    for line in w.models.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_yellow = trimmed == "[builder.yellow]";
            lines.push(line.to_string());
            if in_yellow {
                lines.push(format!("integration = \"{integration}\""));
                lines.push(format!("model = \"{model}\""));
            }
        } else if !in_yellow {
            lines.push(line.to_string());
        }
    }
    w.models = lines.join("\n");
    let models = format!("{}\n", w.models);
    write_models(w, &models);
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
    // Only for the changes that exist now: one that is created later builds as usual.
    let folder = project_root(w).join(".gdd/changes");
    let mut ids: Vec<String> = fs::read_dir(&folder)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    assert!(
        !ids.is_empty(),
        "the Builder writes nothing for the changes that exist: this step goes after gdd new-change"
    );
    ids.sort();
    fake_file(w, "build", &format!("nothing {}", ids.join(" ")));
}

#[given(expr = "the Builder in build mode writes nothing for variant {int}")]
fn builder_writes_nothing_for_variant(w: &mut GddWorld, variant: usize) {
    let path = project_root(w).join(".gdd/fake/nothing_variants");
    let before = fs::read_to_string(&path).unwrap_or_default();
    fake_file(w, "nothing_variants", &format!("{before}{variant}\n"));
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

#[given(expr = "the Builder in build mode also writes {string} with:")]
fn builder_also_writes(w: &mut GddWorld, path: String, step: &cucumber::gherkin::Step) {
    let text = docstring(step, "the text of the file");
    fake_file(w, "build_also", &path);
    fake_file(w, "build_also_content", &format!("{text}\n"));
}

// ---- features/regression/kit-cases.feature -------------------------------------------------
//
// The fake Builder in build mode also acts on `delete` (a path it removes from the worktree on
// each call), `swap` (`<path>|<marker>`: it replaces that file with a commands file whose first G2
// command puts the original back and creates the marker) and `plant` (`<path>|<marker>`: on its
// first call it adds a G2 command creating the marker to that file and commits it on the branch).

#[given(expr = "the Builder in build mode deletes {string}")]
fn builder_deletes(w: &mut GddWorld, path: String) {
    fake_file(w, "delete", &path);
}

#[given(
    expr = "the Builder in build mode swaps {string} for checks that put it back and write {string}"
)]
fn builder_swaps(w: &mut GddWorld, path: String, marker: String) {
    fake_file(w, "swap", &format!("{path}|{marker}"));
}

#[given(expr = "the Builder in build mode commits to {string} a G2 command that writes {string}")]
fn builder_commits_g2_command(w: &mut GddWorld, path: String, marker: String) {
    fake_file(w, "plant", &format!("{path}|{marker}"));
}

#[given(expr = "the spike budget is {string} USD")]
fn spike_budget(w: &mut GddWorld, max_usd: String) {
    let root = project_root(w);
    let path = root.join("gdd/rules.toml");
    let rules = fs::read_to_string(&path).expect("the project's rules");
    assert!(
        !rules.contains("[spike]"),
        "the rules already have a spike budget"
    );
    write_file(&path, &format!("{rules}\n[spike]\nmax_usd = {max_usd}\n"));
    // Harness state from before the change: committed, so the working tree stays clean.
    git(&root, &["add", "--", "gdd/rules.toml"]);
    git(&root, &["commit", "-q", "-m", "spike budget"]);
}

#[then(expr = "the branch {string} does not hold {string}")]
fn branch_does_not_hold(w: &mut GddWorld, branch: String, path: String) {
    let held = std::process::Command::new("git")
        .args(["cat-file", "-e", &format!("{branch}:{path}")])
        .current_dir(project_root(w))
        .output()
        .expect("git can run")
        .status
        .success();
    assert!(!held, "the branch {branch} holds {path}");
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

// ---- features/init.feature -----------------------------------------------------------------
//
// Every scenario starts in `an empty folder`, a project root without `gdd/project.toml`. gdd runs
// there through `gdd::run_with_env`, so `git commit` finds an author in the scenario environment.

#[given("an empty folder")]
fn an_empty_folder(w: &mut GddWorld) {
    w.project = Some(scenario_folder());
    for (name, value) in [
        ("GIT_AUTHOR_NAME", "GDD Test"),
        ("GIT_AUTHOR_EMAIL", "test@example.com"),
        ("GIT_COMMITTER_NAME", "GDD Test"),
        ("GIT_COMMITTER_EMAIL", "test@example.com"),
    ] {
        set_env(w, name, value);
    }
}

#[given(expr = "the folder is a git repository with the file {string}")]
fn folder_is_a_repository(w: &mut GddWorld, path: String) {
    // As the first step of a scenario there is no folder yet: start from an empty one.
    if w.project.is_none() {
        an_empty_folder(w);
    }
    let root = project_root(w);
    git(&root, &["init", "-q", "-b", "main"]);
    write_file(&root.join(&path), "some text\n");
    commit_all(&root, "existing");
}

// ---- features/stack-skeletons.feature ------------------------------------------------------
//
// Fake programs are executable scripts in a folder of their own, put first on the PATH of the
// scenario environment (outside the project, so `gdd init` still finds an empty folder).

/// Writes the executable `name` with `body` and makes sure its folder leads the PATH.
fn fake_program(w: &mut GddWorld, name: &str, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    let dir = w.programs.get_or_insert_with(TempDir::new).0.clone();
    let path = dir.join(name);
    write_file(&path, &format!("#!/bin/sh\n{body}\n"));
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("an executable");
    let leads = |(known, value): &(String, String)| {
        known == "PATH" && value.starts_with(&format!("{}:", dir.display()))
    };
    if !world_env(w).iter().any(leads) {
        let old = world_env(w)
            .iter()
            .find(|(known, _)| known == "PATH")
            .map(|(_, value)| value.clone())
            .or_else(|| std::env::var("PATH").ok())
            .unwrap_or_default();
        set_env(w, "PATH", &format!("{}:{old}", dir.display()));
    }
}

#[given(expr = "the program {string} on the PATH writes the file {string}")]
fn program_writes_file(w: &mut GddWorld, name: String, file: String) {
    fake_program(w, &name, &format!("echo generated > '{file}'\nexit 0"));
}

#[given(expr = "the program {string} on the PATH fails")]
fn program_fails(w: &mut GddWorld, name: String) {
    fake_program(w, &name, "exit 1");
}

#[given(expr = "the program {string} on the PATH does nothing")]
fn program_does_nothing(w: &mut GddWorld, name: String) {
    fake_program(w, &name, "exit 0");
}

// ---- features/mutation-speed.feature -------------------------------------------------------
//
// A counting fake program keeps its calls in `<name>.calls` next to it, one line of arguments per call.

#[given(expr = "the program {string} on the PATH exits with {int}, then {int}")]
fn program_exits_then(w: &mut GddWorld, name: String, first: u8, then: u8) {
    let body = format!(
        "calls=\"$(dirname \"$0\")/{name}.calls\"\n\
         echo \"$*\" >> \"$calls\"\n\
         n=$(wc -l < \"$calls\")\n\
         echo \"{name} call $n\"\n\
         if [ \"$n\" -eq 1 ]; then exit {first}; fi\n\
         exit {then}"
    );
    fake_program(w, &name, &body);
}

/// The arguments of each call of the fake program `name`, in order.
fn program_calls(w: &GddWorld, name: &str) -> Vec<String> {
    let dir = &w.programs.as_ref().expect("a fake program on the PATH").0;
    let calls = fs::read_to_string(dir.join(format!("{name}.calls"))).unwrap_or_default();
    calls.lines().map(str::to_string).collect()
}

#[then(expr = "the program {string} was called {int} times")]
fn program_called_times(w: &mut GddWorld, name: String, times: usize) {
    let calls = program_calls(w, &name);
    assert_eq!(calls.len(), times, "the calls of {name}: {calls:?}");
}

#[then(expr = "call {int} of the program {string} includes {string}")]
fn program_call_includes(w: &mut GddWorld, call: usize, name: String, text: String) {
    let calls = program_calls(w, &name);
    let line = calls
        .get(call.saturating_sub(1))
        .unwrap_or_else(|| panic!("no call {call} of {name}: {calls:?}"));
    assert!(
        line.contains(&text),
        "call {call} of {name} lacks {text:?}: {line:?}"
    );
}

#[given(expr = "the rules have {string} = {string}")]
fn rules_have(w: &mut GddWorld, key: String, value: String) {
    let root = project_root(w);
    let path = root.join("gdd/rules.toml");
    let rules = fs::read_to_string(&path).expect("the project's rules");
    let (table, name) = key.rsplit_once('.').expect("a dotted key");
    let value = if value.parse::<i64>().is_ok() {
        value
    } else {
        format!("\"{value}\"")
    };
    let header = format!("[{table}]\n");
    // A table already there keeps its lines.
    let rules = match rules.split_once(&header) {
        Some((before, after)) => format!("{before}{header}{name} = {value}\n{after}"),
        None => format!("{rules}\n{header}{name} = {value}\n"),
    };
    write_file(&path, &rules);
    commit_harness_file(&root, "gdd/rules.toml", "rules");
}

#[then(expr = "git tracks {string}")]
fn git_tracks(w: &mut GddWorld, path: String) {
    let tracked = git(&project_root(w), &["ls-files", "--", &path]);
    assert_eq!(tracked, path, "git does not track {path}");
}

#[then(expr = "the file {string} exists")]
fn file_exists(w: &mut GddWorld, path: String) {
    assert!(
        project_root(w).join(&path).exists(),
        "{path} does not exist, and it should"
    );
}

#[given("no coding agent is on the PATH")]
fn no_agent_on_the_path(w: &mut GddWorld) {
    set_env(w, "PATH", "/usr/bin:/bin");
}

#[then(expr = "the file {string} is executable")]
fn file_is_executable(w: &mut GddWorld, path: String) {
    use std::os::unix::fs::PermissionsExt;
    let mode = fs::metadata(project_root(w).join(&path))
        .unwrap_or_else(|e| panic!("cannot read {path}: {e}"))
        .permissions()
        .mode();
    assert_eq!(mode & 0o755, 0o755, "{path} has mode {mode:o}");
}

#[then(expr = "git does not track {string}")]
fn git_does_not_track(w: &mut GddWorld, path: String) {
    let tracked = git(&project_root(w), &["ls-files", "--", &path]);
    assert!(tracked.is_empty(), "git tracks {path}: {tracked}");
}

/// `line` without its trailing `# comment` (a `#` inside a string stays).
fn without_comment(line: &str) -> &str {
    let mut in_string = false;
    let mut escaped = false;
    for (at, c) in line.char_indices() {
        match c {
            '\\' if in_string => escaped = !escaped,
            '"' if !escaped => in_string = !in_string,
            '#' if !in_string => return line[..at].trim_end(),
            _ => escaped = false,
        }
    }
    line
}

/// The quoted strings of a TOML value: one for a string, one per item for a list.
fn toml_items(value: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current: Option<String> = None;
    let mut escaped = false;
    for c in value.chars() {
        match current.take() {
            None => current = (c == '"').then(String::new),
            Some(text) if c == '"' && !escaped => items.push(text),
            Some(mut text) => {
                if c == '\\' && !escaped {
                    escaped = true;
                } else {
                    text.push(c);
                    escaped = false;
                }
                current = Some(text);
            }
        }
    }
    items
}

/// The value of the dotted key `wanted` in a TOML file (tables, `key = value` lines, lists that
/// may span lines), lists as their items joined with ", ". Enough for the files gdd writes.
fn toml_value(text: &str, wanted: &str) -> Option<String> {
    let mut table = String::new();
    let mut lines = text.lines().map(without_comment);
    while let Some(line) = lines.next() {
        let line = line.trim();
        if let Some(header) = line.strip_prefix('[') {
            table = header.trim_end_matches(']').trim().replace('"', "");
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().replace('"', "");
        let mut value = value.trim().to_string();
        while value.starts_with('[') && !value.ends_with(']') {
            value.push_str(lines.next()?.trim());
        }
        let full = if table.is_empty() {
            key
        } else {
            format!("{table}.{key}")
        };
        if full == wanted {
            return Some(if value.contains('"') || value == "[]" {
                toml_items(&value).join(", ")
            } else {
                value
            });
        }
    }
    None
}

#[then(expr = "the TOML file {string} has {string} = {string}")]
fn toml_file_has(w: &mut GddWorld, path: String, key: String, expected: String) {
    let content = fs::read_to_string(project_root(w).join(&path))
        .unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    assert_eq!(
        toml_value(&content, &key).as_deref(),
        Some(expected.as_str()),
        "{path} has no {key} = {expected:?}:\n{content}"
    );
}

// ---- features/state-root.feature -----------------------------------------------------------
//
// A change's worktree is a sibling of the project folder: `<project>.worktrees/<id>`.

/// The folder of the worktree of change `id`.
fn worktree_of(w: &mut GddWorld, id: &str) -> PathBuf {
    worktrees_folder_of(&project_root(w)).join(id)
}

#[given(expr = "the change {string} has a worktree with the file {string}")]
fn change_has_worktree(w: &mut GddWorld, id: String, file: String) {
    let root = project_root(w);
    let folder = worktree_of(w, &id);
    // The branch of the change cannot be checked out twice: the main checkout goes back to main.
    git(&root, &["checkout", "-q", "main"]);
    let folder_text = folder.to_string_lossy().to_string();
    git(
        &root,
        &[
            "worktree",
            "add",
            "-q",
            &folder_text,
            &format!("change/{id}"),
        ],
    );
    write_file(&folder.join(&file), "text\n");
}

#[when(expr = "I run {string} in the worktree of {string}")]
fn i_run_in_worktree(w: &mut GddWorld, command: String, id: String) {
    let folder = worktree_of(w, &id);
    run_gdd(w, &command, &folder);
}

#[given(expr = "I run {string} in the worktree of {string}")]
fn given_i_run_in_worktree(w: &mut GddWorld, command: String, id: String) {
    let folder = worktree_of(w, &id);
    run_gdd(w, &command, &folder);
}

#[then(expr = "the worktree of {string} has no {string}")]
fn worktree_has_no(w: &mut GddWorld, id: String, path: String) {
    let at = worktree_of(w, &id).join(&path);
    assert!(
        fs::symlink_metadata(&at).is_err(),
        "{} exists in the worktree of {id}, and it should not",
        at.display()
    );
}

// ---- features/parallel-launch.feature ------------------------------------------------------

#[then(expr = "the current branch is {string}")]
fn current_branch_is(w: &mut GddWorld, branch: String) {
    let current = git(&project_root(w), &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_eq!(current, branch, "the main checkout is on {current}");
}

/// The worktrees git lists for the project: each folder with the branch it has checked out.
fn listed_worktrees(root: &Path) -> Vec<(PathBuf, Option<String>)> {
    let text = git(root, &["worktree", "list", "--porcelain"]);
    let mut listed: Vec<(PathBuf, Option<String>)> = Vec::new();
    for line in text.lines() {
        if let Some(folder) = line.strip_prefix("worktree ") {
            listed.push((PathBuf::from(folder), None));
        } else if let (Some(branch), Some(last)) = (line.strip_prefix("branch "), listed.last_mut())
        {
            last.1 = Some(branch.trim_start_matches("refs/heads/").to_string());
        }
    }
    listed
}

/// Whether git lists `folder` as a worktree on the branch `change/<id>`.
fn is_worktree_of(root: &Path, folder: &Path, id: &str) -> bool {
    let Ok(folder) = folder.canonicalize() else {
        return false;
    };
    let branch = format!("change/{id}");
    listed_worktrees(root).iter().any(|(listed, on)| {
        listed.canonicalize().is_ok_and(|l| l == folder) && on.as_deref() == Some(branch.as_str())
    })
}

fn assert_worktree_at(w: &mut GddWorld, id: &str, folder: &Path) {
    let root = project_root(w);
    assert!(
        is_worktree_of(&root, folder, id),
        "{} is not a worktree on change/{id}; git lists {:?}",
        folder.display(),
        listed_worktrees(&root)
    );
}

#[then(expr = "the change {string} has a worktree next to the project")]
fn change_has_worktree_next_to_project(w: &mut GddWorld, id: String) {
    let folder = worktree_of(w, &id);
    assert_worktree_at(w, &id, &folder);
}

#[then(expr = "the change {string} has a worktree in {string}")]
fn change_has_worktree_in(w: &mut GddWorld, id: String, folder: String) {
    let folder = project_root(w).join(folder);
    assert_worktree_at(w, &id, &folder);
}

#[then(expr = "the change {string} has no worktree")]
fn change_has_no_worktree(w: &mut GddWorld, id: String) {
    let root = project_root(w);
    let folder = worktree_of(w, &id);
    assert!(
        fs::symlink_metadata(&folder).is_err(),
        "{} still exists",
        folder.display()
    );
    let branch = format!("change/{id}");
    let on_branch: Vec<_> = listed_worktrees(&root)
        .into_iter()
        .filter(|(_, on)| on.as_deref() == Some(branch.as_str()))
        .collect();
    assert!(on_branch.is_empty(), "git still lists {on_branch:?}");
}

#[given(expr = "the project keeps its worktrees in {string}")]
fn project_keeps_worktrees_in(w: &mut GddWorld, folder: String) {
    let root = project_root(w);
    let path = root.join("gdd/rules.toml");
    let rules = fs::read_to_string(&path).expect("the project's rules");
    write_file(
        &path,
        &format!("{rules}\n[parallel]\nfolder = \"{folder}\"\n"),
    );
    git(&root, &["add", "--", "gdd/rules.toml"]);
    git(&root, &["commit", "-q", "-m", "worktrees folder"]);
}

#[given(expr = "the Builder in build mode changes the record file {string} of the change {string}")]
fn builder_changes_record_file(w: &mut GddWorld, file: String, id: String) {
    fake_file(w, "record", &format!("{id}/{file}"));
}

#[given(expr = "the log gets a line for the change {string} during Builder call {int}")]
fn log_gets_line_during_call(w: &mut GddWorld, id: String, call: usize) {
    fake_file(w, "log_line", &format!("{id}|{call}"));
}

#[then(expr = "the agents worked in the worktree of {string}")]
fn agents_worked_in_worktree(w: &mut GddWorld, id: String) {
    // The worktree may be gone by now: the physical folder is the project's plus its name.
    let root = project_root(w).canonicalize().expect("the project folder");
    let expected = worktrees_folder_of(&root).join(&id);
    let cwds = fs::read_to_string(project_root(w).join(".gdd/fake/cwds")).unwrap_or_default();
    assert!(!cwds.is_empty(), "no agent recorded its working folder");
    for cwd in cwds.lines() {
        assert_eq!(Path::new(cwd), expected, "an agent worked in {cwd}");
    }
}

// ---- features/merge-queue.feature ----------------------------------------------------------
//
// The fake Reviewer also acts on `main_gets` in `.gdd/fake/` (a path): on its first call it writes
// that file in the main checkout (`$GDD_STATE`) and commits it on main there.

#[given(expr = "the cap of changes building at once is {int}")]
fn cap_of_changes_building(w: &mut GddWorld, cap: usize) {
    let root = project_root(w);
    let path = root.join("gdd/rules.toml");
    let rules = fs::read_to_string(&path).expect("the project's rules");
    // A `[parallel]` table already there (the worktrees folder) keeps its lines.
    let rules = match rules.split_once("[parallel]\n") {
        Some((before, after)) => format!("{before}[parallel]\nmax = {cap}\n{after}"),
        None => format!("{rules}\n[parallel]\nmax = {cap}\n"),
    };
    write_file(&path, &rules);
    git(&root, &["add", "--", "gdd/rules.toml"]);
    git(&root, &["commit", "-q", "-m", "cap of changes building"]);
}

#[given(expr = "the change {string} is building")]
fn change_is_building(w: &mut GddWorld, id: String) {
    let root = project_root(w);
    let mut record = read_record(&root, &id);
    record["state"] = "building".into();
    write_record(&root, &id, &record);
}

#[given(expr = "the main branch gets the file {string} during the Reviewer call")]
fn main_gets_file_during_review(w: &mut GddWorld, path: String) {
    fake_file(w, "main_gets", &path);
}

#[given(expr = "another gdd holds the merge lock for {int} seconds")]
fn another_gdd_holds_merge_lock(w: &mut GddWorld, seconds: u64) {
    let file = project_root(w).join(".gdd/merge.lock");
    write_file(&file, "");
    let (taken, wait_taken) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let held = fs::OpenOptions::new().write(true).open(&file);
        let held = held.and_then(|f| f.lock().map(|()| f));
        let ok = held.is_ok();
        // The sender may be gone when the step already failed; nothing to tell then.
        let _ = taken.send(ok);
        std::thread::sleep(std::time::Duration::from_secs(seconds));
        drop(held);
    });
    assert_eq!(
        wait_taken.recv(),
        Ok(true),
        "the lock holder could not take .gdd/merge.lock"
    );
}

// ---- features/shared-writes.feature --------------------------------------------------------

#[given(expr = "an agent of another change is at work for {int} seconds")]
fn agent_of_another_change_at_work(w: &mut GddWorld, seconds: u64) {
    let file = project_root(w).join(".gdd/agents.lock");
    if !file.exists() {
        write_file(&file, "");
    }
    let (taken, wait_taken) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let held = fs::OpenOptions::new().write(true).open(&file);
        let held = held.and_then(|f| f.lock_shared().map(|()| f));
        let ok = held.is_ok();
        // The sender may be gone when the step already failed; nothing to tell then.
        let _ = taken.send(ok);
        std::thread::sleep(std::time::Duration::from_secs(seconds));
        drop(held);
    });
    assert_eq!(
        wait_taken.recv(),
        Ok(true),
        "the agent could not take .gdd/agents.lock shared"
    );
}

#[given("the machines open G0 again during the Builder call")]
fn machines_open_g0_during_the_call(w: &mut GddWorld) {
    let root = project_root(w);
    fake_file(w, "linger", "3");
    let in_call = root.join(".gdd/fake/in-call");
    let mut env = world_env(w).clone();
    env.retain(|(name, _)| name != "GDD_ROLE");
    w.g0_during_call = Some(std::thread::spawn(move || {
        for _ in 0..600 {
            if in_call.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        if !in_call.exists() {
            return (false, "the Builder call never started".to_string());
        }
        let args: Vec<String> = ["gate", "G0", "ok", "--by", "machines"]
            .iter()
            .map(|word| word.to_string())
            .collect();
        let out = gdd::run_with_env(&args, &root, &env);
        let text = format!("{}{}", out.stdout, out.stderr);
        (out.code == 0, text)
    }));
}

#[then("the G0 opened during the Builder call succeeded")]
fn g0_during_the_call_succeeded(w: &mut GddWorld) {
    let handle = w
        .g0_during_call
        .take()
        .expect("a G0 opened during the call");
    let (succeeded, text) = handle.join().expect("the G0 thread did not panic");
    assert!(succeeded, "the G0 opened during the call failed: {text:?}");
    assert!(
        text.contains("waiting for the agents at work to end their call"),
        "the G0 did not wait for the call; it said {text:?}"
    );
}

// ---- features/board.feature ----------------------------------------------------------------
//
// API of the `gdd` crate these steps expect (the Builder in build mode implements it):
// `gdd::board::Board::{load(&Path) -> Result<Board, String>, press(Key), render(w, h) -> String,
// closed() -> bool}`, with the form's `Key`.

/// The Board of the scenario, which has no `Debug` of its own.
struct OpenBoard(gdd::board::Board);

impl std::fmt::Debug for OpenBoard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OpenBoard")
    }
}

fn board_of(w: &mut GddWorld) -> &mut gdd::board::Board {
    &mut w.board.as_mut().expect("the Board is open").0
}

#[given(expr = "the change {string} is in state {string}")]
fn change_is_in_state(w: &mut GddWorld, id: String, state: String) {
    let root = project_root(w);
    let mut record = read_record(&root, &id);
    record["state"] = state.into();
    write_record(&root, &id, &record);
}

#[when("I open the board")]
fn i_open_the_board(w: &mut GddWorld) {
    let root = project_root(w);
    let board = gdd::board::Board::load(&root).unwrap_or_else(|e| panic!("cannot open: {e}"));
    w.board = Some(OpenBoard(board));
}

#[when(expr = "I press {word} on the board")]
fn i_press_on_the_board(w: &mut GddWorld, key: String) {
    board_of(w).press(key_named(&key));
}

#[then(expr = "the board shows {string}")]
fn board_shows(w: &mut GddWorld, text: String) {
    let screen = board_of(w).render(100, 40);
    assert!(
        screen.contains(&text),
        "the board lacks {text:?}:\n{screen}"
    );
}

#[then(expr = "the board does not show {string}")]
fn board_does_not_show(w: &mut GddWorld, text: String) {
    let screen = board_of(w).render(100, 40);
    assert!(
        !screen.contains(&text),
        "the board shows {text:?}:\n{screen}"
    );
}

#[then("the board is closed")]
fn the_board_is_closed(w: &mut GddWorld) {
    assert!(board_of(w).closed(), "the board is still open");
}

// ---- features/sitting.feature --------------------------------------------------------------
//
// API of the `gdd` crate these steps expect (the Builder in build mode implements it):
// `gdd::sitting::Sitting::{load(&Path, &str) -> Result<Sitting, String>, press(Key),
// render(w, h) -> String, closed() -> bool}`, with the form's `Key`.

/// The sitting of the scenario, which has no `Debug` of its own.
struct OpenSitting(gdd::sitting::Sitting);

impl std::fmt::Debug for OpenSitting {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OpenSitting")
    }
}

fn sitting_of(w: &mut GddWorld) -> &mut gdd::sitting::Sitting {
    &mut w.sitting.as_mut().expect("the sitting is open").0
}

#[when(expr = "I open the sitting of {string}")]
fn i_open_the_sitting(w: &mut GddWorld, change: String) {
    let root = project_root(w);
    let sitting = gdd::sitting::Sitting::load(&root, &change)
        .unwrap_or_else(|e| panic!("cannot open the sitting: {e}"));
    w.sitting = Some(OpenSitting(sitting));
}

#[when(expr = "I press {word} at the sitting")]
fn i_press_at_the_sitting(w: &mut GddWorld, key: String) {
    sitting_of(w).press(key_named(&key));
}

#[when(expr = "I type {string} at the sitting")]
fn i_type_at_the_sitting(w: &mut GddWorld, text: String) {
    use gdd::form::Key;
    for letter in text.chars() {
        let key = if letter == ' ' {
            Key::Space
        } else {
            Key::Char(letter)
        };
        sitting_of(w).press(key);
    }
}

#[then(expr = "the sitting shows {string}")]
fn sitting_shows(w: &mut GddWorld, text: String) {
    let screen = sitting_of(w).render(100, 40);
    assert!(
        screen.contains(&text),
        "the sitting lacks {text:?}:\n{screen}"
    );
}

#[then(expr = "the sitting does not show {string}")]
fn sitting_does_not_show(w: &mut GddWorld, text: String) {
    let screen = sitting_of(w).render(100, 40);
    assert!(
        !screen.contains(&text),
        "the sitting shows {text:?}:\n{screen}"
    );
}

#[then("the sitting is closed")]
fn the_sitting_is_closed(w: &mut GddWorld) {
    assert!(sitting_of(w).closed(), "the sitting is still open");
}

// ---- features/sitting-g4.feature -----------------------------------------------------------
//
// API of the `gdd` crate these steps expect: `Sitting::chosen(&self) -> Option<Vec<String>>`.

#[when("the sitting runs what was chosen")]
fn the_sitting_runs_what_was_chosen(w: &mut GddWorld) {
    let chosen = sitting_of(w)
        .chosen()
        .unwrap_or_else(|| panic!("the sitting chose nothing"));
    let root = project_root(w);
    let mut env = world_env(w).clone();
    env.retain(|(name, _)| name != "GDD_ROLE");
    let out = gdd::run_with_env(&chosen, &root, &env);
    w.exit_checked = false;
    w.ran = Some(Ran {
        code: out.code,
        succeeded: out.code == 0,
        stdout: out.stdout,
        stderr: out.stderr,
    });
}

#[then("the sitting chose nothing")]
fn the_sitting_chose_nothing(w: &mut GddWorld) {
    let chosen = sitting_of(w).chosen();
    assert!(chosen.is_none(), "the sitting chose {chosen:?}");
}

#[then(expr = "the log has no {string} line for {string}")]
fn log_has_no_line(w: &mut GddWorld, gate: String, change: String) {
    let text = fs::read_to_string(project_root(w).join(".gdd/log.jsonl")).unwrap_or_default();
    let found: Vec<_> = text
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|r| r["gate"] == gate.as_str() && r["change"] == change.as_str())
        .collect();
    assert!(
        found.is_empty(),
        "the log has {gate} lines for {change}: {found:?}"
    );
}

#[then("the project has no worktree besides the main checkout")]
fn project_has_no_other_worktree(w: &mut GddWorld) {
    let listed = listed_worktrees(&project_root(w));
    assert_eq!(
        listed.len(),
        1,
        "git lists more than the main checkout: {listed:?}"
    );
}

// ---- features/init-form.feature ------------------------------------------------------------
//
// API of the `gdd` crate these steps expect (the Builder in build mode implements it), as in the
// package: `gdd::form::{Form, Key, Outcome, finish}`.
// - `Form::new(agents)`, `press(Key)`, `render(width, height) -> String`, `outcome() -> Option<&Outcome>`.
// - `finish(root, &Outcome, env) -> Result<String, String>`: what the CLI does after the form.
// A space typed into a text field is `Key::Char(' ')`; `Key::Space` only toggles checklists.

/// The form of the scenario, which has no `Debug` of its own.
struct OpenForm(gdd::form::Form);

impl std::fmt::Debug for OpenForm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OpenForm")
    }
}

/// The coding agents init looks for, in the order the form lists them.
const CODING_AGENTS: [&str; 3] = ["claude", "codex", "opencode"];

/// The platforms of the form's list, in its order.
const FORM_PLATFORMS: [&str; 5] = ["mobile", "desktop", "web", "cli", "other"];

/// The stacks of the form's checklist, in catalog order; `Not sure` comes after them.
const FORM_STACKS: [&str; 4] = ["rust", "typescript", "python", "not sure"];

#[given(expr = "the coding agents on this machine are {string}")]
fn agents_on_this_machine(w: &mut GddWorld, names: String) {
    // Only these programs, and the system ones: the agents of the real machine are not found.
    set_env(w, "PATH", "/usr/bin:/bin");
    w.programs = None;
    for name in names.split_whitespace() {
        fake_program(w, name, "exit 0");
    }
}

/// The coding agents found on the `PATH` of the scenario, in the order of [`CODING_AGENTS`].
fn agents_found(w: &mut GddWorld) -> Vec<String> {
    use std::os::unix::fs::PermissionsExt;
    let path = world_env(w)
        .iter()
        .find(|(name, _)| name == "PATH")
        .map(|(_, value)| value.clone())
        .unwrap_or_default();
    let is_program = |dir: &str, name: &str| {
        fs::metadata(Path::new(dir).join(name))
            .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    };
    CODING_AGENTS
        .iter()
        .filter(|name| path.split(':').any(|dir| is_program(dir, name)))
        .map(|name| name.to_string())
        .collect()
}

fn open_form(w: &mut GddWorld) {
    let agents = agents_found(w);
    w.form = Some(OpenForm(gdd::form::Form::new(agents)));
    w.form_finished = false;
}

fn form_of(w: &mut GddWorld) -> &mut gdd::form::Form {
    &mut w.form.as_mut().expect("the Setup form is open").0
}

/// Presses `key`; the first time the form has an outcome, does what the CLI does with it and
/// keeps its output or its error as the last run.
fn press_key(w: &mut GddWorld, key: gdd::form::Key) {
    let root = project_root(w);
    let env = world_env(w).clone();
    let finished = w.form_finished;
    let form = form_of(w);
    form.press(key);
    let result = match form.outcome() {
        Some(outcome) if !finished => Some(gdd::form::finish(&root, outcome, &env)),
        _ => None,
    };
    let Some(result) = result else { return };
    w.form_finished = true;
    w.exit_checked = false;
    w.ran = Some(match result {
        Ok(stdout) => Ran {
            code: 0,
            succeeded: true,
            stdout: format!("{stdout}\n"),
            stderr: String::new(),
        },
        Err(stderr) => Ran {
            code: 1,
            succeeded: false,
            stdout: String::new(),
            stderr: format!("{stderr}\n"),
        },
    });
}

fn press_text(w: &mut GddWorld, text: &str) {
    for letter in text.chars() {
        press_key(w, gdd::form::Key::Char(letter));
    }
}

fn key_named(name: &str) -> gdd::form::Key {
    use gdd::form::Key;
    match name {
        "Enter" => Key::Enter,
        "Esc" => Key::Esc,
        "Up" => Key::Up,
        "Down" => Key::Down,
        "Space" => Key::Space,
        "Backspace" => Key::Backspace,
        letter if letter.chars().count() == 1 => Key::Char(letter.chars().next().unwrap_or(' ')),
        other => panic!("unknown key {other:?}"),
    }
}

#[when("I open the Setup form")]
fn i_open_the_form(w: &mut GddWorld) {
    open_form(w);
}

#[when(expr = "I type {string}")]
fn i_type(w: &mut GddWorld, text: String) {
    press_text(w, &text);
}

#[when(expr = "I press {word}")]
fn i_press(w: &mut GddWorld, key: String) {
    press_key(w, key_named(&key));
}

#[then(expr = "the form shows {string}")]
fn form_shows(w: &mut GddWorld, text: String) {
    let screen = form_of(w).render(100, 40);
    assert!(screen.contains(&text), "the form lacks {text:?}:\n{screen}");
}

#[then(expr = "the form does not show {string}")]
fn form_does_not_show(w: &mut GddWorld, text: String) {
    let screen = form_of(w).render(100, 40);
    assert!(
        !screen.contains(&text),
        "the form shows {text:?}:\n{screen}"
    );
}

/// What the form is filled with: the rows of the step's table, `field | answer`.
struct Answers(Vec<(String, String)>);

impl Answers {
    fn of(step: &cucumber::gherkin::Step) -> Answers {
        let table = step.table.as_ref().expect("a table of field | answer");
        let rows = table.rows.iter().skip(1);
        Answers(
            rows.filter(|row| row.len() >= 2)
                .map(|row| (row[0].trim().to_string(), row[1].trim().to_string()))
                .collect(),
        )
    }

    fn get(&self, field: &str) -> Option<&str> {
        let (_, answer) = self.0.iter().find(|(name, _)| name == field)?;
        Some(answer.as_str())
    }
}

/// Presses Down until the cursor of a list goes from `from` to `to`.
fn move_down(w: &mut GddWorld, from: usize, to: usize) {
    for _ in from..to {
        press_key(w, gdd::form::Key::Down);
    }
}

fn fill_name_and_description(w: &mut GddWorld, answers: &Answers) {
    press_text(w, answers.get("name").expect("a name in the table"));
    press_key(w, gdd::form::Key::Enter);
    press_text(w, answers.get("description").unwrap_or_default());
    press_key(w, gdd::form::Key::Enter);
}

fn fill_platform(w: &mut GddWorld, answers: &Answers) {
    let platform = answers.get("platform").expect("a platform in the table");
    let index = FORM_PLATFORMS.iter().position(|p| *p == platform);
    move_down(
        w,
        0,
        index.unwrap_or_else(|| panic!("no platform {platform:?}")),
    );
    press_key(w, gdd::form::Key::Enter);
}

/// Checks each stack of the comma-separated `stacks` (`not sure` too), without pressing Enter.
fn check_stacks(w: &mut GddWorld, stacks: &str) {
    let mut picked: Vec<usize> = stacks
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .map(|s| FORM_STACKS.iter().position(|known| *known == s))
        .map(|index| index.unwrap_or_else(|| panic!("unknown stack in {stacks:?}")))
        .collect();
    picked.sort_unstable();
    let mut at = 0;
    for index in picked {
        move_down(w, at, index);
        at = index;
        press_key(w, gdd::form::Key::Space);
    }
}

fn fill_stacks(w: &mut GddWorld, answers: &Answers) {
    check_stacks(w, answers.get("stacks").expect("stacks in the table"));
    press_key(w, gdd::form::Key::Enter);
}

/// Types the language over the default one (the backspaces change nothing when it is a hint).
fn fill_language(w: &mut GddWorld, answers: &Answers) {
    if let Some(language) = answers.get("language") {
        for _ in 0..10 {
            press_key(w, gdd::form::Key::Backspace);
        }
        press_text(w, language);
    }
    press_key(w, gdd::form::Key::Enter);
}

/// One sensitive area per line, then Enter on an empty line to go on.
fn fill_sensitive(w: &mut GddWorld, answers: &Answers) {
    let areas = answers.get("sensitive").unwrap_or_default();
    for area in areas.split(';').map(str::trim).filter(|a| !a.is_empty()) {
        press_text(w, area);
        press_key(w, gdd::form::Key::Enter);
    }
    press_key(w, gdd::form::Key::Enter);
}

/// Opens the form and answers up to `last`: `stacks`, `agents` or `summary`.
fn fill_form(w: &mut GddWorld, step: &cucumber::gherkin::Step, last: &str) {
    let answers = Answers::of(step);
    open_form(w);
    fill_name_and_description(w, &answers);
    fill_platform(w, &answers);
    if last == "stacks" {
        return;
    }
    fill_stacks(w, &answers);
    if last == "agents" {
        return;
    }
    // The agents found on the machine stay as they come.
    press_key(w, gdd::form::Key::Enter);
    fill_language(w, &answers);
    fill_sensitive(w, &answers);
}

#[given("the form is filled with:")]
fn form_filled(w: &mut GddWorld, step: &cucumber::gherkin::Step) {
    fill_form(w, step, "summary");
}

#[given("the form is filled up to the stacks with:")]
fn form_filled_to_stacks(w: &mut GddWorld, step: &cucumber::gherkin::Step) {
    fill_form(w, step, "stacks");
}

#[given("the form is filled up to the agents with:")]
fn form_filled_to_agents(w: &mut GddWorld, step: &cucumber::gherkin::Step) {
    fill_form(w, step, "agents");
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

// ---- features/versions.feature -------------------------------------------------------------

/// Whether the project has a tag with this exact name.
fn tag_exists(root: &Path, tag: &str) -> bool {
    git(root, &["tag", "--list", tag]) == tag
}

#[given("the versions are released by hand")]
fn versions_released_by_hand(w: &mut GddWorld) {
    let root = project_root(w);
    let path = root.join("gdd/rules.toml");
    let rules = fs::read_to_string(&path).expect("the project's rules");
    write_file(&path, &format!("{rules}\n[versions]\nmode = \"manual\"\n"));
    git(&root, &["add", "--", "gdd/rules.toml"]);
    git(&root, &["commit", "-q", "-m", "versions by hand"]);
}

#[then(expr = "main has the tag {string}")]
fn main_has_the_tag(w: &mut GddWorld, tag: String) {
    let root = project_root(w);
    assert!(tag_exists(&root, &tag), "no tag {tag}");
    // `merge-base` fails the step when the tag is not reachable from main.
    git(&root, &["merge-base", "--is-ancestor", &tag, "main"]);
}

#[then(expr = "main does not have the tag {string}")]
fn main_does_not_have_the_tag(w: &mut GddWorld, tag: String) {
    assert!(
        !tag_exists(&project_root(w), &tag),
        "the tag {tag} exists, and it should not"
    );
}

#[then("main has no version tag")]
fn main_has_no_version_tag(w: &mut GddWorld) {
    let tags = git(&project_root(w), &["tag", "--list", "v*"]);
    assert!(tags.is_empty(), "there are version tags: {tags}");
}

#[then(expr = "the tag {string} names the last commit on main")]
fn tag_names_last_commit(w: &mut GddWorld, tag: String) {
    let root = project_root(w);
    let named = git(&root, &["rev-parse", &format!("{tag}^{{commit}}")]);
    let last = git(&root, &["rev-parse", "main"]);
    assert_eq!(named, last, "the tag {tag} does not name main's head");
}

#[given(expr = "main has the file {string} with:")]
fn main_has_the_file(w: &mut GddWorld, step: &cucumber::gherkin::Step, path: String) {
    let root = project_root(w);
    let text = docstring(step, "the file's text");
    write_file(&root.join(&path), &format!("{text}\n"));
    // Only this file: files the Background left aside are not swept in.
    git(&root, &["add", "--", &path]);
    git(
        &root,
        &["commit", "-q", "-m", &format!("add {path}"), "--", &path],
    );
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
    // One scenario at a time: the steps are synchronous on one thread, so running several only
    // interleaves their steps, and the clock of a lock taken in a `Given` keeps running while
    // another scenario runs a long step before the `When`.
    futures::executor::block_on(
        GddWorld::cucumber()
            .max_concurrent_scenarios(1)
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

// ---- features/upgrade.feature --------------------------------------------------------------
//
// An older gdd is played by editing what `gdd init` wrote and committing it as `gdd init
// <version>`. Git runs in the scenario's project with the scenario's environment.

/// Runs git in the project of the scenario, with its environment and the author on the command
/// line; any failure is a failed step.
fn scenario_git(w: &mut GddWorld, args: &[&str]) -> String {
    let root = project_root(w);
    let env = world_env(w).clone();
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
        .env_clear()
        .envs(env)
        .current_dir(&root)
        .output()
        .expect("git can run");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// The text of `gdd/generated.toml` with its `version` line set to `version`.
fn generated_with_version(text: &str, version: &str) -> String {
    let mut found = false;
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            if line.starts_with("version = ") {
                found = true;
                format!("version = \"{version}\"")
            } else {
                line.to_string()
            }
        })
        .collect();
    assert!(found, "gdd/generated.toml has no version line: {text}");
    format!("{}\n", lines.join("\n"))
}

/// The text of `gdd/generated.toml` with `path` taken out of its `files` list.
fn generated_without_file(text: &str, path: &str) -> String {
    let quoted = format!("\"{path}\"");
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            if line.starts_with("files = ") {
                assert!(line.contains(&quoted), "files lacks {path}: {line}");
                line.replacen(&format!("{quoted}, "), "", 1)
                    .replacen(&format!(", {quoted}"), "", 1)
                    .replacen(&quoted, "", 1)
            } else {
                line.to_string()
            }
        })
        .collect();
    format!("{}\n", lines.join("\n"))
}

fn commit_older_gdd(w: &mut GddWorld, version: &str) {
    scenario_git(w, &["add", "-A"]);
    let message = format!("gdd init {version}");
    scenario_git(w, &["commit", "-q", "--allow-empty", "-m", &message]);
}

#[given(expr = "an older gdd {string} wrote {string} with {string} where this one writes {string}")]
fn an_older_gdd_wrote(w: &mut GddWorld, version: String, path: String, old: String, new: String) {
    let root = project_root(w);
    let file = root.join(&path);
    let content = fs::read_to_string(&file).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    assert!(content.contains(&new), "{path} lacks {new:?}: {content}");
    write_file(&file, &content.replacen(&new, &old, 1));
    let generated = root.join("gdd/generated.toml");
    let text = fs::read_to_string(&generated).expect("gdd/generated.toml");
    write_file(&generated, &generated_with_version(&text, &version));
    commit_older_gdd(w, &version);
}

#[given(expr = "an older gdd {string} did not write {string}")]
fn an_older_gdd_did_not_write(w: &mut GddWorld, version: String, path: String) {
    let root = project_root(w);
    fs::remove_file(root.join(&path)).unwrap_or_else(|e| panic!("cannot delete {path}: {e}"));
    if path != "gdd/generated.toml" {
        let generated = root.join("gdd/generated.toml");
        let text = fs::read_to_string(&generated).expect("gdd/generated.toml");
        let text = generated_without_file(&text, &path);
        write_file(&generated, &generated_with_version(&text, &version));
    }
    commit_older_gdd(w, &version);
}

#[then(expr = "the file {string} on the branch {string} contains {string}")]
fn file_on_branch_contains(w: &mut GddWorld, path: String, branch: String, text: String) {
    let text = text.replace("\\\"", "\"");
    let content = scenario_git(w, &["show", &format!("{branch}:{path}")]);
    assert!(
        content.contains(&text),
        "{path} on {branch} lacks {text:?}: {content}"
    );
}

#[then(expr = "the file {string} on the branch {string} does not contain {string}")]
fn file_on_branch_lacks(w: &mut GddWorld, path: String, branch: String, text: String) {
    let text = text.replace("\\\"", "\"");
    let content = scenario_git(w, &["show", &format!("{branch}:{path}")]);
    assert!(
        !content.contains(&text),
        "{path} on {branch} contains {text:?}: {content}"
    );
}
