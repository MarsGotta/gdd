//! Reading blueprints, verifying logs that reproduce each anomaly, and the metrics tables.

mod common;
use common::*;
use gdd::blueprint::Blueprint;
use serde_json::Value;
use std::process::{Command, Stdio};

fn fixture(v: &str, dir: &std::path::Path) {
    let ok = Command::new("bash")
        .arg(repo_root().join("evals/fixtures/shop.sh"))
        .arg(v)
        .arg(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success();
    assert!(ok, "fixture {v}");
}

#[test]
fn every_fixture_blueprint_reads() {
    let tmp = tempfile::tempdir().unwrap();
    let t = std::fs::read_to_string(repo_root().join("skills/gdd-prepare/assets/blueprint.md"))
        .unwrap();
    let template = Blueprint::parse(&t).unwrap();
    assert_eq!(template.zones().len(), 1);
    let mut read = 0;
    for v in [
        "base",
        "gated",
        "prepared",
        "redrule",
        "timed",
        "logged",
        "slow",
        "catalog",
        "spent",
        "finished",
        "briefed",
        "redreview",
    ] {
        let dir = tmp.path().join(v);
        fixture(v, &dir);
        let text = std::fs::read_to_string(dir.join("gdd/blueprint.md")).unwrap();
        let b = Blueprint::parse(&text).unwrap_or_else(|e| panic!("{v}: {e}"));
        assert!(b.problems().is_empty(), "{v}: {:?}", b.problems());
        assert!(b.environments()[0].models.of("standard").is_some());
        match v {
            "gated" => {
                assert_eq!(b.check("prices").unwrap().when, vec![":(glob)src/pay/**"]);
            }
            "slow" => assert!(b.check("e2e").unwrap().before_review()),
            "redrule" => assert!(b.uncovered_is_red()),
            "base" => {
                assert_eq!(
                    b.cap("yellow", "rounds_per_change"),
                    Some(6),
                    "an old blueprint's caps hold for every color"
                );
                assert!(!b.uncovered_is_red());
            }
            "catalog" => assert_eq!(b.cap("yellow", "rounds_per_change"), Some(3)),
            _ => {}
        }
        read += 1;
    }
    assert_eq!(read, 12);
}

fn ev(sec: u32, rest: &str) -> String {
    format!(
        "{{\"contract\":\"gdd/1\",\"ts\":\"2026-11-10T10:{:02}:{:02}Z\",\"brief\":\"x\",{rest}}}",
        sec / 60,
        sec % 60
    )
}

fn verify_codes(r: &Repo, lines: &[String]) -> Vec<String> {
    r.write("gdd/log/x.jsonl", &(lines.join("\n") + "\n"));
    let v = r.gdd(&["verify", "x", "--no-git"]);
    v.facts["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["code"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn verify_finds_each_anomaly() {
    let r = Repo::new();
    let brief = r#""event":"step","step":"brief","report":{"skill":"gdd-brief","result":"yellow"},"color":{"base":"b0","color":"yellow"},"files":{"features/x.feature":"2222222222222222222222222222222222222222"},"seal":"s1""#;
    let approval = r#""event":"approval","who":"P","of":"brief","version":1,"files":{"features/x.feature":"2222222222222222222222222222222222222222"},"where":"conversation""#;
    let checks = r#""event":"step","step":"checks","report":{"skill":"gdd-checks","result":"sealed"},"seal":{"commit":"s2","checks":{}}"#;
    let build = r#""event":"step","step":"build","report":{"skill":"gdd-build","result":"done"}"#;
    let pass = r#""event":"step","step":"gates","start":"s2","gates":[{"check":"tests","seconds":1,"result":"pass"}]"#;
    let fail = r#""event":"step","step":"gates","start":"s2","gates":[{"check":"tests","seconds":1,"result":"fail"}]"#;
    let review = r#""event":"step","step":"review","report":{"skill":"gdd-review","result":"pass"},"verdict":"pass""#;
    let ship = r#""event":"step","step":"ship","where":"abc1234","color":"yellow""#;
    let good = vec![
        ev(0, brief),
        ev(30, r#""event":"ask","of":"brief","what":"ok?""#),
        ev(60, approval),
        ev(120, checks),
        ev(300, build),
        ev(330, pass),
        ev(600, review),
        ev(700, ship),
    ];
    assert!(
        verify_codes(&r, &good).is_empty(),
        "{:?}",
        verify_codes(&r, &good)
    );
    let has = |lines: Vec<String>, code: &str| {
        let c = verify_codes(&r, &lines);
        assert!(c.iter().any(|x| x == code), "{code} not in {c:?}");
    };
    has(vec![ev(0, brief), "{not json".into()], "json");
    has(
        vec![ev(0, brief), ev(10, r#""event":"note","what":"x""#)],
        "shape",
    );
    has(
        vec![ev(0, brief), ev(120, checks), ev(300, build)],
        "needs-approval",
    );
    has(
        vec![
            ev(0, brief),
            ev(60, approval),
            ev(120, checks),
            ev(300, build),
            ev(600, review),
        ],
        "order",
    );
    has(
        vec![
            ev(0, brief),
            ev(60, approval),
            ev(120, checks),
            ev(300, build),
            ev(330, pass),
            ev(700, ship),
        ],
        "order",
    );
    has(
        vec![
            ev(0, brief),
            ev(60, approval),
            ev(120, checks),
            ev(300, build),
            ev(290, pass),
        ],
        "time-backwards",
    );
    has(
        vec![
            ev(0, brief),
            ev(60, approval),
            ev(120, checks),
            ev(300, build),
            ev(300, pass),
        ],
        "same-second",
    );
    let mut many = vec![ev(0, brief), ev(60, approval), ev(120, checks)];
    for i in 0..5 {
        many.push(ev(200 + i * 20, build));
        many.push(ev(210 + i * 20, fail));
    }
    has(many, "cap-exceeded");
    has(
        vec![
            ev(0, brief),
            ev(30, r#""event":"ask","of":"brief","what":"ok?""#),
            ev(60, checks),
        ],
        "ask-unanswered",
    );
    has(
        vec![
            ev(0, brief),
            ev(30, r#""event":"ask","of":"decision","what":"which?""#),
        ],
        "ask-open",
    );
    has(
        vec![ev(0, brief), ev(60, approval), ev(61, approval)],
        "approval-twice",
    );
    has(
        vec![
            ev(0, brief),
            ev(
                60,
                r#""event":"waiver","who":"P","check":"minutes_per_change","why":"more minutes","backlog":"b""#,
            ),
        ],
        "waiver-unseen",
    );
    has(
        vec![ev(
            0,
            r#""event":"step","step":"brief","report":{"skill":"gdd-brief","result":"yellow"},"color":{"base":"b0","color":"yellow"},"files":{},"brief":"y""#,
        )],
        "wrong-log",
    );
}

#[test]
fn metrics_tables_on_a_logged_project() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("logged");
    fixture("logged", &dir);
    let m = run_gdd(
        &dir,
        &["metrics", "--all", "--format", "json"],
        &[("GDD_NO_JOURNAL", "1")],
    );
    assert_eq!(m.code, 0, "{}", m.out);
    let body: String = m
        .out
        .lines()
        .take(m.out.lines().count() - 1)
        .collect::<Vec<_>>()
        .join("\n");
    let all: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(all["schema"], "gdd-metrics/1");
    let changes = all["tables"]["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 13, "twelve changes and the standing checks");
    let c4 = changes.iter().find(|c| c["change"] == "change-04").unwrap();
    assert_eq!(c4["gates_first_try"], false);
    assert_eq!(c4["gates_runs"], 2);
    assert_eq!(c4["sent_back_checks_used"], 1.0);
    assert_eq!(c4["ended"], "shipped");
    assert_eq!(c4["color"], "green");
    let c1 = changes.iter().find(|c| c["change"] == "change-01").unwrap();
    assert_eq!(c1["gates_first_try"], true);
    assert_eq!(c1["review_first_try"], true);
    assert_eq!(c1["verify_errors"], 0);
    let checks = all["tables"]["checks"].as_array().unwrap();
    assert_eq!(
        checks.len(),
        14 * 3,
        "three checks in each of fourteen runs"
    );
    assert!(checks.iter().all(|c| c["tier"] == "each round"));
    let steps = all["tables"]["steps"].as_array().unwrap();
    assert!(
        steps
            .iter()
            .any(|s| s["step"] == "gates" && s["seconds_since_previous"] == 60)
    );
    // CSV to a folder, one file per table, and the summary for a person.
    let out = tmp.path().join("csv");
    let o = out.to_string_lossy().to_string();
    let c = run_gdd(
        &dir,
        &[
            "metrics",
            "--all",
            "--format",
            "csv",
            "--out",
            &o,
            "--summary",
        ],
        &[("GDD_NO_JOURNAL", "1")],
    );
    assert_eq!(c.code, 0, "{}", c.out);
    for t in ["changes", "steps", "checks", "asks", "findings", "engine"] {
        assert!(out.join(format!("{t}.csv")).exists(), "{t}.csv");
    }
    let csv = std::fs::read_to_string(out.join("changes.csv")).unwrap();
    assert!(csv.starts_with("change,kind,color_planned,"));
    assert_eq!(csv.lines().count(), 14);
    assert!(c.has("first try: gates 83% (10 of 12)"), "{}", c.out);
    assert!(c.has("slowest checks"), "{}", c.out);
    let j = run_gdd(
        &dir,
        &["metrics", "change-04", "--format", "jsonl"],
        &[("GDD_NO_JOURNAL", "1")],
    );
    let first: Value = serde_json::from_str(j.out.lines().next().unwrap()).unwrap();
    assert_eq!(first["schema"], "gdd-metrics/1");
    assert_eq!(first["table"], "changes");
}

#[test]
fn metrics_join_the_journal() {
    let r = Repo::new();
    r.gdd(&["brief", "m1", "--paths", "src/ui/page.txt"]);
    r.gdd(&["gates", "m1"]);
    r.gdd(&["ask", "m1", "--of", "decision", "Which page?"]);
    r.gdd(&["answer", "m1", "the first"]);
    let m = r.gdd_env(
        &["metrics", "m1", "--format", "json"],
        &[("GDD_NO_JOURNAL", "1")],
    );
    let body: String = m
        .out
        .lines()
        .take(m.out.lines().count() - 1)
        .collect::<Vec<_>>()
        .join("\n");
    let all: Value = serde_json::from_str(&body).unwrap();
    let engine = all["tables"]["engine"].as_array().unwrap();
    assert_eq!(engine.len(), 4, "{engine:?}");
    assert!(
        engine
            .iter()
            .any(|e| e["command"] == "gates" && e["refused"] == "out-of-order")
    );
    let change = &all["tables"]["changes"][0];
    assert_eq!(change["invocations"], 4);
    assert_eq!(change["refusals"], 1);
    let asks = all["tables"]["asks"].as_array().unwrap();
    assert_eq!(asks[0]["answered"], true);
    assert_eq!(asks[0]["answered_by"], "answer");
    assert_eq!(asks[0]["kind"], "decision");
}

#[test]
fn next_for_the_project() {
    let r = Repo::new();
    r.gdd(&["brief", "p1", "--paths", "src/ui/page.txt"]);
    r.gdd(&["ask", "p1", "--of", "decision", "Which?"]);
    let n = r.gdd(&["next"]);
    assert_eq!(n.code, 0, "{}", n.out);
    assert_eq!(n.facts["standing_checks"], "done");
    assert_eq!(n.facts["waiting"][0]["id"], "p1");
    assert_eq!(n.facts["backlog_next"], "g1");
    assert!(n.has("no approval of the blueprint"));
    let b = r.gdd(&["backlog", "list"]);
    assert_eq!(b.facts["items"].as_array().unwrap().len(), 4);
    assert_eq!(
        r.gdd(&[
            "backlog", "add", "p2", "--what", "Another", "--color", "green"
        ])
        .code,
        0
    );
    assert_eq!(r.gdd(&["backlog", "done", "p2"]).code, 0);
    assert!(
        r.read("gdd/backlog.md")
            .contains("| p2 | Another | green | | | done |")
    );
}

/// The backlog's order is the order of the work. In one run of the eval cases, the trials of the
/// checks that wait for code were added before the first item that brings any: the next thing to
/// do was to try the tests of a project that had none.
#[test]
fn a_checks_trial_is_added_after_an_item_of_the_product() {
    let r = Repo::new();
    let head =
        "# Backlog\n\n| Id | What | Color | Phase | Brief | State |\n|---|---|---|---|---|---|\n";
    r.write("gdd/backlog.md", head);
    let add = |id: &str| {
        r.gdd(&[
            "backlog",
            "add",
            id,
            "--what",
            "Something",
            "--color",
            "red",
        ])
    };
    assert_eq!(add("standing-checks").code, 0);
    let early = add("standing-checks-tests");
    assert_eq!(early.facts["refused"], "out-of-order", "{}", early.out);
    assert!(early.has("add first the item"), "{}", early.out);
    assert!(!r.read("gdd/backlog.md").contains("standing-checks-tests"));
    assert_eq!(add("catalog-page").code, 0);
    let after = add("standing-checks-tests");
    assert_eq!(after.code, 0, "{}", after.out);
    let listed = r.gdd(&["backlog", "list"]);
    let ids: Vec<&str> = listed.facts["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| i["id"].as_str())
        .collect();
    assert_eq!(
        ids,
        ["standing-checks", "catalog-page", "standing-checks-tests"],
        "{}",
        listed.out
    );
}
