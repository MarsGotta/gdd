//! The differential test: `gdd limits` against the contract's counter, `gdd/limits.sh` as the
//! working tree of the skills has it, on every log of every fixture variant and on synthetic logs.

mod common;
use common::*;
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Stdio};

const NOW: &str = "2026-11-10T12:00:00Z";

pub const VARIANTS: [&str; 26] = [
    "base",
    "plain",
    "history",
    "gated",
    "prepared",
    "redrule",
    "timed",
    "logged",
    "broken",
    "review",
    "rises",
    "pressure",
    "missing",
    "build",
    "tempt",
    "stocked",
    "catalog",
    "draft",
    "slow",
    "redreview",
    "style",
    "spent",
    "finished",
    "briefed",
    "stopped",
    "base",
];

/// A `date` that always answers the same time, for the counter, which reads the clock itself.
fn fake_clock(dir: &Path) -> String {
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let date = bin.join("date");
    std::fs::write(&date, format!("#!/bin/sh\necho {NOW}\n")).unwrap();
    Command::new("chmod").arg("+x").arg(&date).status().unwrap();
    format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

struct Count {
    code: i32,
    lines: Vec<String>,
    last: Value,
}

fn counter(project: &Path, id: &str, path: &str) -> Count {
    let script = repo_root().join("skills/gdd-prepare/assets/limits.sh");
    let o = Command::new("sh")
        .arg(script)
        .arg(id)
        .current_dir(project)
        .env("PATH", path)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout).to_string();
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let last = lines.pop().unwrap_or_default();
    let json = last
        .strip_prefix("gdd-limits ")
        .map(|j| serde_json::from_str(j).unwrap())
        .unwrap_or(Value::Null);
    Count {
        code: o.status.code().unwrap_or(-1),
        lines,
        last: json,
    }
}

fn ours(project: &Path, id: &str) -> Count {
    let r = run_gdd(
        project,
        &["limits", id],
        &[("GDD_NOW", NOW), ("GDD_NO_JOURNAL", "1")],
    );
    let mut lines: Vec<String> = r.out.lines().map(str::to_string).collect();
    lines.pop();
    lines.retain(|l| !l.contains("aren't counted") && !l.contains("weren't counted"));
    // The builder's lines are the layer's own line, which the counter leaves to git.
    lines.retain(|l| !l.starts_with("lines "));
    Count {
        code: r.code,
        lines,
        last: r.facts,
    }
}

/// Compare the two counts: the same lines (each cap's used value, its cap, whether a waiver raised
/// it, whether it has room), the same exit code, and the same color, stop, slow and judge.
fn same(name: &str, a: &Count, b: &Count) -> Result<(), String> {
    if a.lines != b.lines {
        return Err(format!(
            "{name}: lines differ\n  limits.sh: {:?}\n  gdd:       {:?}",
            a.lines, b.lines
        ));
    }
    if a.code != b.code {
        return Err(format!("{name}: exit {} vs {}", a.code, b.code));
    }
    for k in ["color", "stop", "slow", "judge"] {
        if a.last[k] != b.last[k] {
            return Err(format!(
                "{name}: {k} is {} in limits.sh and {} in gdd",
                a.last[k], b.last[k]
            ));
        }
    }
    Ok(())
}

#[test]
fn every_fixture_variant_counts_the_same() {
    let tmp = tempfile::tempdir().unwrap();
    let path = fake_clock(tmp.path());
    let mut compared = vec![];
    let mut failures = vec![];
    let mut seen = std::collections::BTreeSet::new();
    for v in VARIANTS {
        if !seen.insert(v) {
            continue;
        }
        let dir = tmp.path().join(v);
        let ok = Command::new("bash")
            .arg(repo_root().join("evals/fixtures/shop.sh"))
            .arg(v)
            .arg(&dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success();
        assert!(ok, "the fixture {v} builds");
        let Ok(logs) = std::fs::read_dir(dir.join("gdd/log")) else {
            continue;
        };
        let mut ids: Vec<String> = logs
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .filter(|n| n.ends_with(".jsonl") && !n.ends_with(".engine.jsonl"))
            .map(|n| n.trim_end_matches(".jsonl").to_string())
            .collect();
        ids.sort();
        for id in ids {
            let a = counter(&dir, &id, &path);
            let b = ours(&dir, &id);
            let name = format!("{v}/{id}");
            match same(&name, &a, &b) {
                Ok(()) => compared.push(format!("{name}: {}", a.last)),
                Err(e) => failures.push(e),
            }
        }
    }
    println!("{}", compared.join("\n"));
    // Where a change has a branch, the layer also counts its lines, beside the counter's fields.
    let review = tmp.path().join("review");
    let r = run_gdd(
        &review,
        &["limits", "discount-code"],
        &[("GDD_NOW", NOW), ("GDD_NO_JOURNAL", "1")],
    );
    assert!(r.facts["lines"]["used"].as_i64().unwrap() > 0, "{}", r.out);
    assert_eq!(r.facts["lines"]["cap"], 400);
    assert!(
        r.out.contains("lines ") && r.out.contains(" of 400"),
        "{}",
        r.out
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(
        compared.len() >= 60,
        "only {} logs compared",
        compared.len()
    );
}

fn template_with_late() -> String {
    std::fs::read_to_string(repo_root().join("skills/gdd-prepare/assets/blueprint.md"))
        .unwrap()
        .replace("  - id: review\n", "  - id: e2e\n    type: machine\n    command: sh e2e.sh\n    runs: before review\n  - id: review\n")
}

fn ev(minute: i64, rest: &str, spaced: bool) -> String {
    let line = format!(
        "{{\"contract\":\"gdd/1\",\"ts\":\"2026-11-10T{:02}:{:02}:00Z\",\"brief\":\"x\",{rest}}}",
        10 + minute / 60,
        minute % 60
    );
    if spaced {
        line.replace("\":\"", "\": \"")
            .replace("\",\"", "\", \"")
            .replace(",\"", ", \"")
    } else {
        line
    }
}

#[test]
fn synthetic_logs_count_the_same() {
    let tmp = tempfile::tempdir().unwrap();
    let path = fake_clock(tmp.path());
    let old = "---\ncontract: gdd/1\nlimits:\n  fix_rounds_per_gate: 2\n  rounds_per_change: 6\n  minutes_per_change: 120  # without the time it waits for a person\n---\n".to_string();
    let brief = r#""event":"step","step":"brief","report":{"skill":"gdd-brief","result":"yellow","tokens":10}"#;
    let ask = r#""event":"ask","of":"decision","what":"a rounding""#;
    let approval = r#""event":"approval","who":"P","of":"brief""#;
    let checks = r#""event":"step","step":"checks","report":{"result":"sealed"}"#;
    let build = r#""event":"step","step":"build","report":{"result":"done","tokens":120}"#;
    let stopped =
        r#""event":"step","step":"build","report":{"result":"stopped"},"stop":{"reason":"x"}"#;
    let ok = r#""event":"step","step":"gates","gates":[{"check":"seal","seconds":1,"result":"pass"},{"check":"tests","seconds":20,"result":"pass"}]"#;
    let fail = r#""event":"step","step":"gates","gates":[{"check":"seal","seconds":1,"result":"pass"},{"check":"tests","seconds":20,"result":"fail"}]"#;
    let slow = r#""event":"step","step":"gates","gates":[{"check":"tests","seconds":130.5,"tail":"the \"check\":\"x\" text","result":"pass"},{"check":"e2e","seconds":30,"result":"pass"}]"#;
    let late = r#""event":"step","step":"gates","gates":[{"check":"tests","seconds":5,"result":"pass"},{"check":"e2e","seconds":40,"result":"fail"}]"#;
    let pass = r#""event":"step","step":"review","report":{"result":"pass"},"verdict":"pass""#;
    let fix = r#""event":"step","step":"review","report":{"result":"fix"},"verdict":"fix""#;
    let waiver = |limit: &str, to: &str| {
        format!(
            r#""event":"waiver","who":"P","check":"{limit}","why":"x","backlog":"b","limit":"{limit}","to":{to}"#
        )
    };
    let mut rounds = vec![ev(0, brief, false), ev(1, checks, false)];
    for m in [2, 4, 6, 8] {
        rounds.push(ev(m, build, false));
        rounds.push(ev(m + 1, fail, false));
    }
    let mut cases: Vec<(String, String, Vec<String>)> = vec![];
    let tpl = template_with_late();
    cases.push((
        "waits".into(),
        tpl.clone(),
        vec![
            ev(0, brief, false),
            ev(5, ask, false),
            ev(65, approval, false),
            ev(70, checks, false),
            ev(75, build, false),
            ev(80, ask, false),
        ],
    ));
    cases.push((
        "spaced".into(),
        tpl.clone(),
        vec![
            ev(0, brief, true),
            ev(5, ask, true),
            ev(65, approval, true),
            ev(70, checks, true),
            ev(75, build, true),
            ev(80, ask, true),
        ],
    ));
    let mut finished = rounds[..rounds.len() - 1].to_vec();
    finished.push(ev(9, ask, false));
    cases.push(("finished".into(), tpl.clone(), finished));
    let mut spent = rounds.clone();
    spent.push(ev(10, ask, false));
    cases.push(("spent".into(), tpl.clone(), spent));
    let mut passed = rounds[..rounds.len() - 1].to_vec();
    passed.push(ev(9, ok, false));
    cases.push(("passed".into(), tpl.clone(), passed.clone()));
    let mut reviewed = passed.clone();
    reviewed.push(ev(10, pass, false));
    cases.push(("reviewed".into(), tpl.clone(), reviewed));
    let mut fixed = passed.clone();
    fixed.push(ev(10, fix, false));
    cases.push(("fixed".into(), tpl.clone(), fixed));
    let mut stop = rounds[..rounds.len() - 2].to_vec();
    stop.push(ev(8, stopped, false));
    cases.push(("stopped".into(), tpl.clone(), stop));
    cases.push((
        "slow".into(),
        tpl.clone(),
        vec![
            ev(0, brief, false),
            ev(1, checks, false),
            ev(2, build, false),
            ev(3, slow, false),
            ev(4, ask, false),
        ],
    ));
    cases.push((
        "late".into(),
        tpl.clone(),
        vec![
            ev(0, brief, false),
            ev(1, checks, false),
            ev(2, build, false),
            ev(3, late, false),
            ev(4, build, false),
            ev(5, late, false),
            ev(6, build, false),
            ev(7, ask, false),
        ],
    ));
    let lateish = vec![
        ev(0, brief, false),
        ev(1, checks, false),
        ev(2, build, false),
        ev(50, fail, false),
    ];
    let mut w = lateish.clone();
    w.extend([
        ev(51, &waiver("minutes_per_change", "40"), false),
        ev(52, &waiver("minutes_per_change", "\"60\""), false),
        ev(53, ask, false),
    ]);
    cases.push(("waiver".into(), tpl.clone(), w));
    let mut w2 = lateish.clone();
    w2.extend([
        ev(51, &waiver("rounds_per_change", "9"), false),
        ev(53, ask, false),
    ]);
    cases.push(("other-waiver".into(), tpl.clone(), w2));
    cases.push((
        "old".into(),
        old.clone(),
        vec![
            ev(0, brief, false),
            ev(1, checks, false),
            ev(2, build, false),
            ev(3, build, false),
            ev(4, ask, false),
        ],
    ));
    cases.push((
        "tokens".into(),
        tpl.replace(
            "tokens_per_change: <n, where the environment reports them>",
            "tokens_per_change: 200",
        ),
        vec![
            ev(0, brief, false),
            ev(1, checks, false),
            ev(2, build, false),
            ev(3, build, false),
        ],
    ));
    cases.push((
        "open".into(),
        tpl.clone(),
        vec![
            ev(0, brief, false),
            ev(1, checks, false),
            ev(2, build, false),
        ],
    ));
    cases.push(("empty".into(), tpl.clone(), vec![]));
    let mut failures = vec![];
    for (name, bp, lines) in &cases {
        let dir = tmp.path().join(format!("s-{name}"));
        std::fs::create_dir_all(dir.join("gdd/log")).unwrap();
        std::fs::write(dir.join("gdd/blueprint.md"), bp).unwrap();
        let mut text = lines.join("\n");
        if !text.is_empty() {
            text.push('\n');
        }
        std::fs::write(dir.join("gdd/log/x.jsonl"), text).unwrap();
        let a = counter(&dir, "x", &path);
        let b = ours(&dir, "x");
        if let Err(e) = same(name, &a, &b) {
            failures.push(e);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn both_refuse_the_same() {
    let tmp = tempfile::tempdir().unwrap();
    let path = fake_clock(tmp.path());
    std::fs::create_dir_all(tmp.path().join("gdd/log")).unwrap();
    std::fs::write(tmp.path().join("gdd/blueprint.md"), template_with_late()).unwrap();
    assert_eq!(counter(tmp.path(), "missing", &path).code, 2);
    assert_eq!(ours(tmp.path(), "missing").code, 2);
    assert_eq!(counter(tmp.path(), "Bad Id", &path).code, 2);
    assert_eq!(ours(tmp.path(), "Bad-Id").code, 2);
}
