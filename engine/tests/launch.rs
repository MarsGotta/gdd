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
    assert!(review["report"]["tokens_read"].is_null());
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
        "modelUsage": {
            "model-a": {"inputTokens": 100, "outputTokens": 40, "cacheReadInputTokens": 5000, "cacheCreationInputTokens": 60},
            "model-b": {"inputTokens": 0, "outputTokens": 0, "cacheReadInputTokens": 200}
        }
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
    assert!(
        l.facts.get("session_id").is_none(),
        "reviewers always start fresh"
    );
    let review = r
        .log_lines("l1")
        .into_iter()
        .find(|e| e["step"] == "review")
        .unwrap();
    assert_eq!(
        review["report"]["tokens"], 200,
        "fresh input, cache writes and output"
    );
    assert_eq!(review["report"]["tokens_read"], 5360);
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
    assert_eq!(launch["cache_read_tokens"], 5200);
    assert_eq!(launch["input_tokens"], 100);
    assert_eq!(launch["started"], true);
    let review_step = m
        .out
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .find(|v| v["table"] == "steps" && v["step"] == "review")
        .unwrap();
    assert_eq!(review_step["tokens_read"], 5360);
}

#[test]
fn a_manual_role_reports_only_the_input_measured_for_its_round() {
    let r = Repo::new();
    r.write("tests/run.sh", "grep -q page src/ui/page.txt\n");
    r.commit("Check the page");
    r.gdd(&["brief", "l1", "--paths", "src/ui/page.txt"]);
    r.commit_on(
        "work/l1",
        Some("main"),
        &[("src/ui/page.txt", "broken")],
        "First build",
    );
    assert_eq!(
        r.report("l1", "build", &report_block("gdd-build", "done"), &[])
            .code,
        0
    );
    let gates = r.gdd(&["gates", "l1"]);
    assert_eq!(gates.facts["next"], "build", "{}", gates.out);
    r.commit_on(
        "work/l1",
        None,
        &[("src/ui/page.txt", "fixed page")],
        "Fix the page",
    );
    let build = r.report(
        "l1",
        "build",
        &report_block("gdd-build", "done"),
        &["--tokens-read", "900"],
    );
    assert_eq!(build.code, 0, "{}", build.out);
    let events = r.log_lines("l1");
    let report = &events.last().unwrap()["report"];
    assert_eq!(report["round"], 2);
    assert_eq!(report["tokens_read"], 900);
    assert!(report["tokens"].is_null());
}

#[test]
fn a_missing_input_measurement_is_distinct_from_a_measured_zero() {
    use gdd::launch::{Adapter, read_output};
    let read = |adapter, event: serde_json::Value| {
        read_output(adapter, &event.to_string(), "model-a").tokens_read
    };
    for (adapter, event) in [
        (
            Adapter::Codex,
            serde_json::json!({"type":"turn.completed","usage":{"output_tokens":7}}),
        ),
        (
            Adapter::Claude,
            serde_json::json!({"type":"result","modelUsage":{"model-a":{"outputTokens":7}}}),
        ),
    ] {
        assert_eq!(read(adapter, event), None);
    }
    assert_eq!(
        read(
            Adapter::Codex,
            serde_json::json!({
                "type":"turn.completed","usage":{"input_tokens":0,"output_tokens":7}
            })
        ),
        Some(0)
    );
    assert_eq!(
        read(
            Adapter::Claude,
            serde_json::json!({
                "type":"result","usage":{"input_tokens":8,"cache_read_input_tokens":40,"cache_creation_input_tokens":12}
            })
        ),
        Some(60)
    );
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
    assert!(prompt.contains(r#""failures":0"#), "{prompt}");
    assert!(prompt.contains("gdd-report"), "{prompt}");
}

/// A yellow change the person approved, waiting for its checks.
fn approved(r: &Repo, id: &str) {
    yellow_brief(r, id, "src/core/app.txt");
    let a = r.gdd(&[
        "approve",
        id,
        "--of",
        "brief",
        "--who",
        "Person",
        "--where",
        "conversation",
        &format!("gdd/briefs/{id}.md"),
        &format!("features/{id}.feature"),
    ]);
    assert_eq!(a.code, 0, "{}", a.out);
}

/// A fake role that commits one file on a branch, in a worktree of its own, and ends with its report.
fn committing(r: &Repo, name: &str, add: &str, file: &str, skill: &str, result: &str) -> String {
    let wt = r.dir.path().join(format!("{name}-wt"));
    let wt = wt.display();
    let report = if skill == "gdd-checks" {
        sealed_report(&["tests"])
    } else {
        report_block(skill, result)
    };
    script(
        r,
        &format!("{name}.sh"),
        &format!(
            "git worktree add -q {add} || exit 8\nmkdir -p $(dirname {wt}/{file})\necho 'by {name}' > {wt}/{file}\ngit -C {wt} add -A && git -C {wt} commit -q -m Work || exit 7\ngit worktree remove --force {wt}\ncat <<'REPORT'\n{report}REPORT"
        ),
    )
}

#[test]
fn what_a_role_sealed_or_built_while_it_ran_is_what_its_report_is_judged_against() {
    let r = Repo::new();
    approved(&r, "c1");
    let wt = |n: &str| r.dir.path().join(format!("{n}-wt")).display().to_string();
    // The checks role seals during the launch: its step is written the first time.
    let s = committing(
        &r,
        "checks",
        &format!("{} gdd-seal/c1", wt("checks")),
        "tests/c1.sh",
        "gdd-checks",
        "sealed",
    );
    let cmd = format!("sh {s} <model>");
    let c = r.gdd(&[
        "launch",
        "c1",
        "checks",
        "--run",
        "--command",
        &cmd,
        "--start-wait",
        "5",
    ]);
    assert_eq!(c.code, 0, "{}", c.out);
    assert_eq!(c.facts["result"], "sealed", "{}", c.out);
    assert_eq!(c.facts["next"], "build", "{}", c.out);
    // And the builder makes its branch during the launch.
    let s = committing(
        &r,
        "build",
        &format!("-b work/c1 {} gdd-seal/c1", wt("build")),
        "src/core/app.txt",
        "gdd-build",
        "done",
    );
    let cmd = format!("sh {s} <model>");
    let b = r.gdd(&[
        "launch",
        "c1",
        "build",
        "--run",
        "--command",
        &cmd,
        "--start-wait",
        "5",
    ]);
    assert_eq!(b.code, 0, "{}", b.out);
    assert_eq!(b.facts["next"], "gates", "{}", b.out);
}

#[test]
fn a_role_answers_on_its_standard_output_whatever_it_says_on_the_other() {
    let r = ready_for_review();
    // An environment that frames no events: its progress on standard error, its answer on the other.
    let s = script(
        &r,
        "noisy.sh",
        &format!("echo 'a banner, then all it did' >&2\n{PASS}\necho 'tokens used' >&2"),
    );
    let cmd = format!("sh {s} <model>");
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
    let review = r
        .log_lines("l1")
        .into_iter()
        .find(|e| e["step"] == "review")
        .unwrap();
    assert_eq!(review["verdict"], "pass");
}

#[test]
fn a_role_that_wrote_only_to_standard_error_still_said_it() {
    let r = ready_for_review();
    let s = script(&r, "errors.sh", &format!("( {PASS} ) >&2"));
    let cmd = format!("sh {s} <model>");
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
}

/// Both ways of reporting a role count. A move during --run belongs to that attempt.
#[test]
fn two_refusals_need_the_same_reason_seal_and_head() {
    let r = Repo::new();
    approved(&r, "c1");
    let reject = || r.report("c1", "checks", "No report block", &[]);
    let next = || r.gdd(&["next", "c1"]).facts["state"]["next"].clone();
    assert_eq!(reject().facts["refused"], "no-report");

    // The role creates the change branch during its launch, then leaves an unusable report.
    let cmd = format!(
        "sh {} <model>",
        script(
            &r,
            "try.sh",
            "git branch work/c1 gdd-seal/c1 && echo 'No report block'"
        )
    );
    let ran = r.gdd(&["launch", "c1", "checks", "--run", "--command", &cmd]);
    assert_eq!(ran.facts["refused"], "no-report", "{}", ran.out);
    assert_eq!(
        next()["step"],
        "checks",
        "the head changed during the launch"
    );
    assert_eq!(reject().facts["refused"], "no-report");
    assert_eq!(next()["blocked"], "needs-person");
    let ask = gdd::launch::split(next()["command"].as_str().unwrap()).unwrap();
    assert!(
        ask.last().unwrap().contains('<'),
        "the orchestrator supplies the proposal"
    );

    r.commit_on(
        "gdd-seal/c1",
        None,
        &[("tests/example.sh", "true\n")],
        "Seal a test",
    );
    assert_eq!(
        next()["step"],
        "checks",
        "the seal moved without a log event"
    );
    assert_eq!(reject().facts["refused"], "no-report");
    let wrong = r.report("c1", "checks", &report_block("gdd-build", "done"), &[]);
    assert_eq!(wrong.facts["refused"], "wrong-skill");
    assert_eq!(reject().facts["refused"], "no-report");
    assert_eq!(
        next()["step"],
        "checks",
        "the two matching reasons were not consecutive"
    );
    assert_eq!(reject().facts["refused"], "no-report");
    assert_eq!(next()["blocked"], "needs-person");
    let typo = r.gdd(&["report", "c1", "checks", "absent-report.txt"]);
    assert_eq!(typo.code, 2);
    assert_eq!(next()["blocked"], "needs-person");
    let later = gdd::time::format(gdd::time::now() + 601 * 60);
    let capped = r.gdd_env(&["next", "c1"], &[("GDD_NOW", &later)]);
    assert_eq!(
        capped.facts["state"]["next"]["blocked"], "cap-spent",
        "{}",
        capped.out
    );
}

#[test]
fn the_documented_fix_resumes_the_builder_that_reported() {
    let r = Repo::with(
        &BLUEPRINT.replace("the Agent tool, with its model field", "sh role.sh <model>"),
    );
    let session = "thread-of-this-builder";
    let reply = serde_json::json!({
        "type": "item.completed",
        "item": {"type": "agent_message", "text": report_block("gdd-build", "done")}
    });
    r.write(
        "role.sh",
        &format!(
            r#"set -eu
echo launched >> launches
test "$1" = model-a
input=24
if test "$#" = 3; then
    input=61
    test "$2" = {session}
    case "$3" in *"A fix round"*) ;; *) exit 1 ;; esac
fi
cat <<'END'
{{"type":"thread.started","thread_id":"{session}"}}
{reply}
END
printf '{{"type":"turn.completed","usage":{{"input_tokens":%s,"cached_input_tokens":20,"output_tokens":2}}}}\n' "$input"
"#
        ),
    );
    r.write("tests/run.sh", "grep -q page src/ui/page.txt\n");
    r.commit("Set up the fake command and the check");
    let brief = r.gdd(&["brief", "l1", "--paths", "src/ui/page.txt"]);
    assert_eq!(brief.facts["next_command"], "gdd launch l1 build --run");
    r.commit_on(
        "work/l1",
        Some("main"),
        &[("src/ui/page.txt", "broken\n")],
        "First build",
    );
    let first = r.named("gdd launch l1 build --run --adapter codex");
    assert_eq!(first.code, 0, "{}", first.out);
    let session_id = first.facts["session_id"].as_str().unwrap();
    assert_eq!(session_id, session);
    assert_eq!(first.facts["tokens"], 6);
    assert_eq!(
        r.log_lines("l1").last().unwrap()["report"]["tokens_read"],
        24
    );
    assert!(first.facts["seconds"].is_number());
    let failed = r.named(first.facts["next_command"].as_str().unwrap());
    assert_eq!(failed.code, 1, "{}", failed.out);
    assert_eq!(failed.facts["next_command"], "gdd launch l1 build");
    let before = r.log_lines("l1");
    let launches = r.read("launches");
    assert!(!launches.is_empty());
    let bare = ["launch", "l1", "build", "--run"];
    assert_eq!(r.gdd(&bare).facts["refused"], "resume-required");
    let fresh = r.gdd(&bare);
    assert_eq!(fresh.code, 1, "{}", fresh.out);
    assert!(fresh.has("--run --command") && fresh.has("gdd report l1 build"));
    assert_eq!(r.log_lines("l1"), before);
    assert_eq!(r.read("launches"), launches);
    assert_eq!(
        r.gdd(&["next", "l1"]).facts["state"]["next"]["step"],
        "build"
    );
    let codex = script(
        &r,
        "codex",
        "test \"$1 $2 $3 $4\" = 'exec resume --json -m' || exit 1\nshift 4\nexec sh role.sh \"$@\"",
    );
    assert!(
        std::process::Command::new("chmod")
            .args(["+x", &codex])
            .status()
            .unwrap()
            .success()
    );
    let example = include_str!("../../skills/gdd/references/fix-round.md")
        .lines()
        .find(|line| line.starts_with("gdd launch "))
        .unwrap()
        .replace("<id>", "l1")
        .replace("SESSION_ID", session_id)
        .replace("codex exec", &format!("{codex} exec"));
    r.commit_on(
        "work/l1",
        None,
        &[("src/ui/page.txt", "fixed page\n")],
        "Fix the page",
    );
    let fixed = r.named(&example);
    assert_eq!(fixed.code, 0, "{}", fixed.out);
    assert_eq!(fixed.facts["round"], 2);
    assert_eq!(
        r.log_lines("l1").last().unwrap()["report"]["tokens_read"],
        37
    );
    let gates = r.named(fixed.facts["next_command"].as_str().unwrap());
    assert_eq!(
        gates.facts["next_command"], "gdd launch l1 review --run",
        "{}",
        gates.out
    );
}

fn repo_for_codex_rounds() -> Repo {
    let bp = BLUEPRINT
        .replace(
            "  lines_per_change: 400",
            "  lines_per_change: 400\n  tokens_per_change: 100000",
        )
        .replace("rounds_per_change: 1", "rounds_per_change: 6")
        .replace("fix_rounds_per_gate: 1", "fix_rounds_per_gate: 6");
    let r = Repo::with(&bp);
    r.write("tests/run.sh", "grep -q page-ok src/ui/page.txt\n");
    r.commit("Check the page");
    assert_eq!(
        r.gdd(&["brief", "l1", "--paths", "src/ui/page.txt"]).code,
        0
    );
    r
}

fn codex_round(r: &Repo, round: u32, session: &str, reading: u32) -> Run {
    let resume = if round == 1 {
        ""
    } else {
        assert_eq!(r.gdd(&["gates", "l1"]).code, 1);
        session
    };
    r.commit_on(
        "work/l1",
        (round == 1).then_some("main"),
        &[("src/ui/page.txt", &format!("broken {round}"))],
        "Build",
    );
    let usage = serde_json::json!({
        "input_tokens": reading * 12000, "cached_input_tokens": reading * 2000, "output_tokens": reading * 1000
    });
    let codex = r.fake_codex(session, &report_block("gdd-build", "done"), usage);
    // The wrapper identifies the session in its output, whatever it calls the resume operation.
    let cmd = format!("sh '{codex}' <model> {resume}");
    let run = r.gdd(&[
        "launch",
        "l1",
        "build",
        "--run",
        "--command",
        &cmd,
        "--adapter",
        "codex",
    ]);
    assert_eq!(run.code, 0, "{}", run.out);
    run
}

#[test]
fn the_cap_counts_each_codex_round_once() {
    let r = repo_for_codex_rounds();
    for round in 1..=4 {
        codex_round(&r, round, "builder", round);
    }
    let limits = r.gdd(&["limits", "l1"]);
    assert!(limits.has("tokens 44000 of 100000"), "{}", limits.out);
    let builds = r
        .log_lines("l1")
        .into_iter()
        .filter(|e| e["step"] == "build")
        .collect::<Vec<_>>();
    assert_eq!(builds.len(), 4);
    for step in builds {
        assert_eq!(step["report"]["tokens"], 11000);
        assert_eq!(step["report"]["tokens_read"], 12000);
    }
}

#[test]
fn a_codex_fix_uses_the_last_reading_from_its_own_session() {
    let r = repo_for_codex_rounds();
    codex_round(&r, 1, "first", 1);
    let changed = codex_round(&r, 2, "other", 2);
    assert_eq!(changed.facts["tokens"], 22000, "{}", changed.out);
    assert_eq!(
        r.log_lines("l1").last().unwrap()["report"]["tokens_read"],
        24000
    );
    let next = codex_round(&r, 3, "first", 2);
    assert_eq!(next.facts["tokens"], 11000);
    assert_eq!(
        r.log_lines("l1").last().unwrap()["report"]["tokens_read"],
        12000
    );
    assert!(r.gdd(&["limits", "l1"]).has("tokens 44000 of 100000"));
    let reset = codex_round(&r, 4, "other", 0);
    assert!(reset.facts["tokens"].is_null(), "{}", reset.out);
    assert!(reset.facts.get("reported_input").is_none());
    let help = r.gdd(&["launch", "--help"]);
    assert_eq!(help.code, 0);
    assert!(!help.has("--tokens-read-before"));
    // A manual or older step has round counts but no thread reading.
    assert_eq!(r.gdd(&["gates", "l1"]).code, 1);
    r.commit_on(
        "work/l1",
        None,
        &[("src/ui/page.txt", "broken 5")],
        "Manual build",
    );
    let manual = r.report(
        "l1",
        "build",
        &report_block("gdd-build", "done"),
        &["--tokens", "4000", "--tokens-read", "5000"],
    );
    assert_eq!(manual.code, 0, "{}", manual.out);
    let after_manual = codex_round(&r, 6, "other", 5);
    assert!(
        after_manual.facts["tokens"].is_null(),
        "{}",
        after_manual.out
    );
    assert!(r.log_lines("l1").last().unwrap()["report"]["tokens_read"].is_null());
}

#[test]
fn the_suggested_launch_uses_the_selected_environments_route() {
    let blueprint = BLUEPRINT.replace(
        "limits:\n  lines_per_change",
        "  terminal:\n    launch: sh role.sh <model>\n    light: model-a\nlimits:\n  lines_per_change");
    let r = Repo::with(&blueprint);
    let brief = r.gdd(&[
        "brief",
        "g1",
        "--paths",
        "src/ui/page.txt",
        "--env",
        "terminal",
    ]);
    let command = "gdd launch g1 build --env terminal --run";
    assert_eq!(brief.facts["next_command"], command);
    let next = r.gdd(&["next", "g1", "--env", "terminal"]);
    assert_eq!(next.facts["state"]["next"]["command"], command);
    let project = r.gdd(&["next", "--env", "terminal"]);
    assert_eq!(project.facts["under_way"][0]["command"], command);
    let tool = r.gdd(&["next", "g1", "--env", "fake"]);
    assert_eq!(
        tool.facts["state"]["next"]["command"],
        "gdd launch g1 build --env fake"
    );
}
