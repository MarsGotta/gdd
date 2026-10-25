//! Runs the Gherkin scenarios in `features/` (strict: undefined or skipped steps fail).
//!
//! - `GDD_FEATURES`: the change's `.feature` files, one per line. Empty = every scenario.
//! - `GDD_HIDDEN=1`: also run `features/hidden/` (G3).
//!
//! Steps talk to `gdd-core`, never to the terminal UI.

use std::collections::HashSet;
use std::path::PathBuf;

use cucumber::{World, given, then, when};

#[derive(Debug, Default, World)]
pub struct GddWorld {
    output: String,
    zone_map: String,
    outcome: Option<Outcome>,
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
    w.zone_map = step.docstring.clone().expect("a zone map docstring");
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

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root")
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
