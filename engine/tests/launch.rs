//! `gdd launch --run` with fake commands: one that starts and reports, one that never writes
//! anything, one that waits on standard input, one that runs past its timeout, and one that
//! speaks the stream of an environment that reports tokens. No real agent is ever launched.

mod common;
use common::*;
use std::time::Instant;

/// A green change built and through its gates, waiting for its review.
fn ready_for_review() -> Repo {
    let r = Repo::new();
    r.gdd(&["brief", "l1", "--paths", "src/ui/page.txt"]);
    let base = r.git(&["rev-parse", "main"]);
    r.commit_on(
        "work/l1",
        Some(&base),
        &[("src/ui/page.txt", "p\n")],
        "Page",
    );
    r.report("l1", "build", &report_block("gdd-build", "done"), &[]);
    assert_eq!(r.gdd(&["gates", "l1"]).code, 0);
    r
}

fn script(r: &Repo, name: &str, body: &str) -> String {
    let p = r.dir.path().join(name);
    std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
    p.to_string_lossy().to_string()
}

const PASS: &str = "printf 'pass\\nCOLOR: green\\n```gdd-report\\nskill: gdd-review 0.10.1\\nresult: pass\\ncaught: none\\n```\\n'";

fn journal(r: &Repo) -> Vec<serde_json::Value> {
    r.read("gdd/log/l1.engine.jsonl")
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn a_fake_role_that_starts_and_reports() {
    let r = ready_for_review();
    let args = r.dir.path().join("args.txt");
    let s = script(
        &r,
        "ok.sh",
        &format!(
            "echo \"$1\" > {}\ntest -s \"$2\" || exit 9\n{PASS}",
            args.display()
        ),
    );
    let cmd = format!("sh {s} <model> <prompt file>");
    let l = r.gdd(&[
        "launch",
        "l1",
        "review",
        "--run",
        "--command",
        &cmd,
        "--start-wait",
        "5",
    ]);
    assert_eq!(l.code, 0, "{}", l.out);
    assert_eq!(
        std::fs::read_to_string(&args).unwrap().trim(),
        "model-a",
        "launched on the model of its level"
    );
    let review = r
        .log_lines("l1")
        .into_iter()
        .find(|e| e["step"] == "review")
        .unwrap();
    assert_eq!(review["verdict"], "pass");
    assert_eq!(review["report"]["model"], "model-a");
    assert_eq!(review["report"]["level"], "light");
    assert!(review["report"]["seconds"].is_number());
    assert!(
        review["report"]["tokens"].is_null(),
        "a plain command reports no tokens"
    );
    let j = journal(&r);
    let last = j.last().unwrap();
    assert_eq!(last["command"], "launch");
    assert_eq!(last["launch"]["started"], true);
    assert!(last["launch"]["first_output"].is_number());
    assert_eq!(last["launch"]["timed_out"], false);
    assert!(last["launch"]["tokens"].is_null());
    assert!(
        !last["args"].to_string().contains(&s),
        "the command's text is not journaled: {}",
        last["args"]
    );
}

#[test]
fn a_role_that_never_writes_is_reported_as_not_started() {
    let r = ready_for_review();
    let s = script(&r, "silent.sh", "sleep 30");
    let t = Instant::now();
    let l = r.gdd(&[
        "launch",
        "l1",
        "review",
        "--run",
        "--command",
        &format!("sh {s} <model>"),
        "--start-wait",
        "1",
    ]);
    assert!(t.elapsed().as_secs() < 10, "it was stopped, not waited for");
    assert_eq!(l.code, 1);
    assert_eq!(l.facts["refused"], "not-started", "{}", l.out);
    assert!(
        !r.log_lines("l1").iter().any(|e| e["step"] == "review"),
        "no step is written for a role that didn't start"
    );
    assert_eq!(journal(&r).last().unwrap()["launch"]["started"], false);
}

#[test]
fn a_role_that_waits_on_standard_input_does_not_hang() {
    let r = ready_for_review();
    let s = script(
        &r,
        "stdin.sh",
        &format!("cat > /dev/null\nread answer\n{PASS}"),
    );
    let t = Instant::now();
    let l = r.gdd(&[
        "launch",
        "l1",
        "review",
        "--run",
        "--command",
        &format!("sh {s} <model>"),
        "--start-wait",
        "5",
        "--timeout",
        "20",
    ]);
    assert!(t.elapsed().as_secs() < 10);
    assert_eq!(l.code, 0, "{}", l.out);
}

#[test]
fn a_role_past_its_timeout_is_stopped() {
    let r = ready_for_review();
    let s = script(&r, "slow.sh", "echo working\nsleep 30");
    let t = Instant::now();
    let l = r.gdd(&[
        "launch",
        "l1",
        "review",
        "--run",
        "--command",
        &format!("sh {s} <model>"),
        "--start-wait",
        "5",
        "--timeout",
        "2",
    ]);
    assert!(t.elapsed().as_secs() < 10);
    assert_eq!(l.facts["refused"], "timeout", "{}", l.out);
    assert_eq!(journal(&r).last().unwrap()["launch"]["timed_out"], true);
}

#[test]
fn tokens_are_read_from_the_stream_of_its_environment() {
    let r = ready_for_review();
    let result = serde_json::json!({
        "type": "result",
        "result": "pass\nCOLOR: green\n```gdd-report\nskill: gdd-review 0.10.1\nresult: pass\ncaught: none\n```",
        "session_id": "s1",
        "modelUsage": {"model-a": {"inputTokens": 100, "outputTokens": 40, "cacheReadInputTokens": 5000, "cacheCreationInputTokens": 60}}
    });
    let stream = r.dir.path().join("stream.jsonl");
    std::fs::write(
        &stream,
        format!("{{\"type\":\"system\",\"subtype\":\"init\"}}\n{result}\n"),
    )
    .unwrap();
    let s = script(&r, "claude-like.sh", &format!("cat {}", stream.display()));
    let l = r.gdd(&[
        "launch",
        "l1",
        "review",
        "--run",
        "--command",
        &format!("sh {s} <model>"),
        "--adapter",
        "claude",
        "--start-wait",
        "5",
    ]);
    assert_eq!(l.code, 0, "{}", l.out);
    let review = r
        .log_lines("l1")
        .into_iter()
        .find(|e| e["step"] == "review")
        .unwrap();
    assert_eq!(
        review["report"]["tokens"], 200,
        "fresh input, cache writes and output"
    );
    let j = journal(&r);
    assert_eq!(
        j.last().unwrap()["launch"]["usage"]["model-a"]["cache_read"],
        5000
    );
    // The metrics take it from the journal.
    let m = r.gdd_env(
        &["metrics", "l1", "--format", "jsonl"],
        &[("GDD_NO_JOURNAL", "1")],
    );
    let launch = m
        .out
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .find(|v| v["table"] == "engine" && v["command"] == "launch")
        .unwrap();
    assert_eq!(launch["cache_read_tokens"], 5000);
    assert_eq!(launch["input_tokens"], 100);
    assert_eq!(launch["started"], true);
}

#[test]
fn launch_refuses_what_the_state_does_not_allow_yet() {
    let r = Repo::new();
    r.gdd(&["brief", "l2", "--paths", "src/ui/page.txt"]);
    let s = script(&r, "never.sh", "touch ran-anyway");
    let l = r.gdd(&[
        "launch",
        "l2",
        "review",
        "--run",
        "--command",
        &format!("sh {s} <model>"),
    ]);
    assert_eq!(l.facts["refused"], "out-of-order", "{}", l.out);
    assert!(!r.path("ran-anyway").exists());
    // The blueprint's launch is prose: --run without a command can't run it.
    let b = r.gdd(&["launch", "l2", "build", "--run"]);
    assert_eq!(b.facts["refused"], "not-a-command", "{}", b.out);
    // Without --run it prints the prompt the role gets.
    let p = r.gdd(&["launch", "l2", "build"]);
    let prompt = p.facts["prompt"].as_str().unwrap();
    assert!(prompt.contains("<base> is"), "{prompt}");
    assert!(prompt.contains("round 1 of build"), "{prompt}");
    assert!(prompt.contains("gdd-report"), "{prompt}");
}
