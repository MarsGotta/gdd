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
    outcome: Option<Outcome>,
    project: Option<TempDir>,
    ran: Option<Ran>,
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
    let args: Vec<String> = command
        .split_whitespace()
        .skip_while(|word| *word == "gdd")
        .map(str::to_string)
        .collect();
    let out = gdd::run(&args, cwd);
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
    assert!(ran.succeeded, "gdd failed: {:?}", ran.stderr);
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
}

#[given(regex = r#"^the change "([^"]+)" plans (.*) and its build has started on its branch$"#)]
fn change_started(w: &mut GddWorld, id: String, text: String) {
    let root = project_root(w);
    let paths = parse_change(&text).paths;
    commit_all(&root, "project");
    git(&root, &["checkout", "-q", "-b", &format!("change/{id}")]);
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
