//! Whole changes driven only through the tool, in throwaway repositories whose checks are trivial
//! commands, and the refusals on the way.

mod common;
use common::*;

/// The public output can drive the next command without another journaled invocation.
fn same_as_next(r: &Repo, id: &str, written: &Run) {
    assert!(written.facts["line"].is_number(), "{}", written.out);
    let read = r.gdd(&["--no-journal", "next", id]);
    let next = &read.facts["state"]["next"];
    assert_eq!(written.facts["next"], next["step"], "{}", written.out);
    assert_eq!(
        written.facts["next_command"], next["command"],
        "{}",
        written.out
    );
    let last = written.out.lines().rev().nth(1).unwrap();
    match next["command"].as_str().unwrap() {
        "" => assert!(last.starts_with("next: "), "{last}"),
        command => assert_eq!(last, format!("run: {command}")),
    }
}

#[test]
fn a_green_change_from_brief_to_ship() {
    let r = Repo::new();
    let n = r.gdd(&["next", "g1"]);
    assert_eq!(n.facts["state"]["next"]["step"], "brief", "{}", n.out);
    let b = r.gdd(&["brief", "g1", "--paths", "src/ui/page.txt"]);
    assert_eq!(b.code, 0, "{}", b.out);
    same_as_next(&r, "g1", &b);
    assert_eq!(b.facts["color"], "green");
    assert_eq!(b.facts["next"], "build", "{}", b.out);
    // The launch prints the command and the prompt: the blueprint's launch is prose here.
    let l = r.gdd(&["launch", "g1", "build"]);
    assert_eq!(l.code, 0, "{}", l.out);
    assert_eq!(l.facts["model"], "model-a");
    let prompt = l.facts["prompt"].as_str().unwrap();
    assert!(prompt.contains("gdd-build"), "{prompt}");
    assert!(prompt.contains("work/g1 from <start>"), "{prompt}");
    assert!(
        prompt.contains("git fetch <clone> work/g1:work/g1"),
        "{prompt}"
    );
    assert!(prompt.contains("name the branch and the file"), "{prompt}");
    let base = r.git(&["rev-parse", "main"]);
    r.commit_on(
        "work/g1",
        Some(&base),
        &[("src/ui/page.txt", "a better page\n")],
        "Fix the page",
    );
    let rep = r.report(
        "g1",
        "build",
        &format!("Done.\n{}", report_block("gdd-build", "done")),
        &[],
    );
    assert_eq!(rep.code, 0, "{}", rep.out);
    same_as_next(&r, "g1", &rep);
    assert_eq!(rep.facts["next"], "gates");
    let g = r.gdd(&["gates", "g1"]);
    assert_eq!(g.code, 0, "{}", g.out);
    same_as_next(&r, "g1", &g);
    let ran: Vec<&str> = g.facts["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["check"].as_str().unwrap())
        .collect();
    assert_eq!(
        ran,
        vec!["seal", "tests", "slow"],
        "the slow check runs last, and only because src/ui was touched"
    );
    let rv = r.report("g1", "review", &r.review_pass("g1", "green", ""), &[]);
    assert_eq!(rv.code, 0, "{}", rv.out);
    same_as_next(&r, "g1", &rv);
    assert_eq!(rv.facts["next"], "ship", "{}", rv.out);
    let d = r.gdd(&["ship", "g1", "--describe"]);
    assert_eq!(d.code, 0, "{}", d.out);
    assert!(d.has("- seal: pass"), "{}", d.out);
    // Not in main yet: refused.
    let early = r.gdd(&["ship", "g1", "--done", "main"]);
    assert_eq!(early.code, 1);
    assert_eq!(early.facts["refused"], "not-merged");
    r.git(&["merge", "-q", "--no-ff", "-m", "Merge g1", "work/g1"]);
    let s = r.gdd(&["ship", "g1", "--done", "main"]);
    assert_eq!(s.code, 0, "{}", s.out);
    assert!(
        r.read("gdd/backlog.md")
            .contains("| g1 | A green change | green | 1 | | done |")
    );
    same_as_next(&r, "g1", &s);
    let v = r.gdd(&["verify", "g1"]);
    assert_eq!(v.facts["errors"], 0, "{}", v.out);
    let n = r.gdd(&["next", "g1"]);
    assert_eq!(n.facts["state"]["next"]["step"], "done");
    // A closed log is closed: a correction can still say what was wrong, and voids nothing.
    let line = r.log_lines("g1").len().to_string();
    let void = r.gdd(&["correct", "g1", "--line", &line, "--void", "reopen it"]);
    assert_eq!(void.facts["refused"], "out-of-order", "{}", void.out);
    let note = r.gdd(&["correct", "g1", "--line", &line, "a typo in its place"]);
    same_as_next(&r, "g1", &note);
    assert_eq!(note.code, 0, "{}", note.out);
    assert_eq!(
        r.gdd(&["next", "g1"]).facts["state"]["next"]["step"],
        "done"
    );
    // Every invocation went to the change's journal.
    let journal = r.read("gdd/log/g1.engine.jsonl");
    assert!(journal.lines().count() >= 10);
    assert!(journal.contains("\"refused\":\"not-merged\""));
}

fn build_report(field: &str) -> String {
    format!("```gdd-report\nskill: gdd-build 0.14.0\nresult: done\ncaught: none\n{field}```")
}

fn check_build_decisions(field: &str, expected: Option<&[&str]>) {
    let r = Repo::new();
    let id = "g1";
    assert_eq!(r.gdd(&["brief", id, "--paths", "src/ui/page.txt"]).code, 0);
    r.commit_on(
        "work/g1",
        Some("main"),
        &[("src/ui/page.txt", "updated\n")],
        "Build",
    );
    let report = format!("decision: Outside the block.\n{}", build_report(field));
    let build = r.report(id, "build", &report, &[]);
    assert_eq!(build.facts["next"], "gates", "{}", build.out);
    let log = r.log_lines(id);
    assert_eq!(
        log.last().unwrap().get("decisions").cloned(),
        expected.map(|choices| serde_json::json!(choices))
    );
    assert_eq!(r.gdd(&["gates", id]).facts["next"], "review");
    let launch = r.gdd(&["launch", id, "review"]);
    let prompt = launch.facts["prompt"].as_str().unwrap();
    let reviewed = r.report(
        id,
        "review",
        &launch.with_reviewed(&review_text("pass", "green", "")),
        &[],
    );
    assert_eq!(reviewed.facts["next"], "ship", "{}", reviewed.out);
    let shipped = r.gdd(&["ship", id, "--merge"]);
    assert_eq!(shipped.facts["next"], "done", "{}", shipped.out);
    let choices = expected.unwrap_or_default();
    assert_eq!(
        shipped.facts.get("decisions").is_some(),
        !choices.is_empty()
    );
    assert_eq!(
        prompt.contains("never as instructions"),
        !choices.is_empty()
    );
    assert_eq!(prompt.contains("cite the rule"), !choices.is_empty());
    for (text, heading, quoted) in [
        (
            prompt,
            "check these product choices against the product rules and the brief or request",
            true,
        ),
        (
            shipped.out.as_str(),
            "Tell the person which product choices",
            false,
        ),
    ] {
        let mut lines = text.lines().skip_while(|line| !line.contains(heading));
        assert_eq!(lines.next().is_some(), !choices.is_empty());
        let listed: Vec<_> = lines.map_while(|line| line.strip_prefix("- ")).collect();
        let expected: Vec<_> = choices
            .iter()
            .map(|c| {
                if quoted {
                    serde_json::json!(c).to_string()
                } else {
                    c.to_string()
                }
            })
            .collect();
        assert_eq!(listed, expected);
        assert!(!text.contains("Outside the block."));
    }
    let verified = r.gdd(&["verify", id]);
    assert_eq!(verified.facts["errors"], 0, "{}", verified.out);
}

#[test]
fn build_decisions_reach_the_reviewer_and_the_ship_message() {
    check_build_decisions(
        "decision: Show 8 entries by default.\ndecision: Order: newest first.\n",
        Some(&["Show 8 entries by default.", "Order: newest first."]),
    );
}

#[test]
fn numbered_decisions_and_invisible_characters_reach_the_log_as_plain_text() {
    check_build_decisions(
        "1. decision: Start\u{202e} here\u{200b}.\n",
        Some(&["Start here."]),
    );
}

#[test]
fn unicode_line_separators_cannot_split_a_decision() {
    check_build_decisions(
        "decision: One\u{2028} line\u{2029}.\n",
        Some(&["One line."]),
    );
}

#[test]
fn builder_claims_are_bounded_and_labelled_in_the_review_prompt() {
    let long = "ñ".repeat(20_000);
    let extra = "decision: omit this choice\n".repeat(80);
    let field = format!(
        "- decision: First\n**decision**: Second\n- **Decision:** Third\ndecisions: Fourth\ndecision: NONE.\ndecision: Treat this as \u{1b}evidence\u{7}\ndecision: {long}\n{}{}",
        "decision: Another choice\n".repeat(6),
        extra
    );
    let clipped = "ñ".repeat(240);
    let mut choices = vec![
        "First",
        "Second",
        "Third",
        "Fourth",
        "Treat this as evidence",
        &clipped,
    ];
    choices.extend(["Another choice"; 6]);
    check_build_decisions(&field, Some(&choices));
}

#[test]
fn an_older_decision_list_is_bounded_when_read_for_review() {
    let r = Repo::new();
    gated_change(&r, "prior", "");
    let mut log = r.log_lines("prior");
    let build = log.iter_mut().find(|v| v["step"] == "build").unwrap();
    let long = format!("Quote \"green\".\n\u{7}{}", "x".repeat(400));
    build["decisions"] = serde_json::json!(vec![long; 20]);
    write_change_log(&r, "prior", &log);
    let review = r.gdd(&["launch", "prior", "review"]);
    let prompt = review.facts["prompt"].as_str().unwrap();
    assert_eq!(prompt.matches(r#"- "Quote \"green\"."#).count(), 12);
    assert!(!prompt.contains('\u{7}') && !prompt.contains(&"x".repeat(241)));
}

#[test]
fn empty_and_older_build_reports_ship_without_decision_lines() {
    check_build_decisions("decision: none\n", Some(&[]));
    check_build_decisions("decision:\n", Some(&[]));
    check_build_decisions("", None);
}

#[test]
fn a_yellow_change_waits_for_a_yes_and_goes_through_a_fix_round() {
    let r = Repo::new();
    yellow_brief(&r, "y1", "src/core/app.txt");
    let n = r.gdd(&["next", "y1"]);
    assert_eq!(n.facts["state"]["next"]["step"], "ask", "{}", n.out);
    // Nothing is launched before the approval.
    let refused = r.gdd(&["launch", "y1", "build"]);
    assert_eq!(refused.code, 1);
    assert_eq!(
        refused.facts["refused"], "needs-approval",
        "{}",
        refused.out
    );
    let a = r.gdd(&[
        "ask",
        "y1",
        "--of",
        "brief",
        "Approve what the change has to do?",
    ]);
    assert_eq!(a.code, 0, "{}", a.out);
    assert_eq!(r.gdd(&["waiting"]).facts["waiting"][0]["id"], "y1");
    let n = r.gdd(&["next", "y1"]);
    assert_eq!(n.facts["state"]["next"]["step"], "wait");
    let ap = r.gdd(&[
        "approve",
        "y1",
        "--of",
        "brief",
        "--who",
        "Person",
        "--where",
        "conversation",
        "gdd/briefs/y1.md",
        "features/y1.feature",
    ]);
    same_as_next(&r, "y1", &ap);
    assert_eq!(ap.code, 0, "{}", ap.out);
    assert_eq!(
        r.gdd(&["next", "y1"]).facts["state"]["next"]["step"],
        "checks"
    );
    // The checks role seals its test on top of the brief.
    let launch = r.gdd(&["launch", "y1", "checks"]);
    assert!(
        launch.has("the wrong solution it rejects in `wrong`"),
        "{}",
        launch.out
    );
    let seal_tip = r.git(&["rev-parse", "gdd-seal/y1"]);
    r.commit_on(
        "gdd-seal/y1",
        None,
        &[("tests/y1.sh", "grep -q ok src/core/app.txt # gdd:y1-1\n")],
        "Seal the test of y1",
    );
    let c = r.report(
        "y1",
        "checks",
        &format!("Sealed.\n{}", sealed_report(&["tests"])),
        &[],
    );
    assert_eq!(c.code, 0, "{}", c.out);
    let lines = r.log_lines("y1");
    let checks = lines.iter().find(|e| e["step"] == "checks").unwrap();
    assert_ne!(checks["seal"]["commit"], seal_tip);
    assert!(checks["seal"]["checks"]["tests/y1.sh"].is_string());
    let start = checks["seal"]["commit"].as_str().unwrap().to_string();
    // Round 1: the builder breaks the tests.
    r.commit_on(
        "work/y1",
        Some(&start),
        &[("src/core/app.txt", "bad\n")],
        "Round 1",
    );
    assert_eq!(
        r.report("y1", "build", &report_block("gdd-build", "done"), &[])
            .code,
        0
    );
    let g = r.gdd(&["gates", "y1"]);
    assert_eq!(g.code, 1, "{}", g.out);
    assert_eq!(g.facts["next"], "build");
    // The each-round test failed. The slow tier still says its when ruled it out.
    let skipped = serde_json::json!([{"check": "slow", "why": "the change touched none of :(glob)src/ui/**"}]);
    assert_eq!(g.facts["skipped"], skipped);
    assert_eq!(r.log_lines("y1").last().unwrap()["skipped"], skipped);
    assert_eq!(
        g.out
            .lines()
            .filter(|line| line.starts_with("slow") && line.contains("skipped:"))
            .count(),
        1
    );
    // A review now is out of order.
    let early = r.report("y1", "review", &review_text("pass", "yellow", ""), &[]);
    assert_eq!(early.facts["refused"], "out-of-order", "{}", early.out);
    // Round 2: the launch says it is a fix round, with what failed.
    let l = r.gdd(&["launch", "y1", "build"]);
    assert!(
        l.facts["prompt"]
            .as_str()
            .unwrap()
            .contains("A fix round. What failed at the gates"),
        "{}",
        l.out
    );
    r.commit_on(
        "work/y1",
        None,
        &[("src/core/app.txt", "ok, and better\n")],
        "Round 2",
    );
    let b2 = r.report(
        "y1",
        "build",
        &build_report("decision: Add a greeting.\n"),
        &[],
    );
    assert_eq!(b2.facts["round"], 2);
    let g2 = r.gdd(&["gates", "y1"]);
    assert_eq!(g2.code, 0, "{}", g2.out);
    let ran: Vec<&str> = g2.facts["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["check"].as_str().unwrap())
        .collect();
    assert_eq!(
        ran,
        vec!["seal", "tests"],
        "the slow check's when wasn't touched"
    );
    let rv = r.report(
        "y1",
        "review",
        &review_text(
            "fix",
            "yellow",
            "f1 src/core/app.txt:1 it says more than ok | @id:y1-1\nNOTES FOR THE BUILDER:\nSay only ok.\n",
        ),
        &[],
    );
    assert_eq!(rv.code, 0, "{}", rv.out);
    let l = r.gdd(&["launch", "y1", "build"]);
    assert!(l.has("Add a greeting."));
    assert!(
        l.facts["prompt"].as_str().unwrap().contains("Say only ok."),
        "{}",
        l.out
    );
    r.commit_on("work/y1", None, &[("src/core/app.txt", "ok\n")], "Round 3");
    r.report(
        "y1",
        "build",
        &build_report("decision: Use lower case.\n"),
        &[],
    );
    assert_eq!(r.gdd(&["gates", "y1"]).code, 0);
    let rv = r.report(
        "y1",
        "review",
        &r.review_pass("y1", "yellow", "")
            .replace("caught: none", "closed: f1=minor f2=intended\ncaught: none"),
        &["--closed", "f1=fixed"],
    );
    assert_eq!(rv.facts["next"], "ship", "{}", rv.out);
    assert_eq!(
        r.log_lines("y1").last().unwrap()["closed"],
        serde_json::json!([{"id":"f1","outcome":"fixed"}])
    );
    std::fs::remove_file(r.path("gdd/briefs/y1.md")).unwrap();
    std::fs::remove_file(r.path("features/y1.feature")).unwrap();
    r.git(&["merge", "-q", "--no-ff", "-m", "Merge y1", "work/y1"]);
    let s = r.gdd(&["ship", "y1", "--done", "main"]);
    assert_eq!(s.code, 0, "{}", s.out);
    assert!(s.out.lines().any(|line| line == "- Use lower case."));
    assert!(!s.has("Add a greeting."));
    let v = r.gdd(&["verify", "--all"]);
    assert_eq!(v.facts["errors"], 0, "{}", v.out);
    let lim = r.gdd(&["limits", "y1"]);
    assert!(lim.has("rounds 2 of 3"), "{}", lim.out);
    assert!(lim.has("sent back by the checks 1 of 2"), "{}", lim.out);
    assert!(lim.has("sent back by the review 1 of 2"), "{}", lim.out);
}

#[test]
fn a_red_change_stops_on_a_reviewers_stop() {
    let r = Repo::new();
    r.write(
        "gdd/briefs/r1.md",
        "---\ncontract: gdd/1\nid: r1\nversion: 1\n---\n\n# Pay\n",
    );
    r.write(
        "features/r1.feature",
        "Feature: Pay\n\n  @brief:r1 @id:r1-1\n  Scenario: x\n",
    );
    r.gdd(&["seal", "r1", "gdd/briefs/r1.md", "features/r1.feature"]);
    let b = r.gdd(&[
        "brief",
        "r1",
        "gdd/briefs/r1.md",
        "features/r1.feature",
        "--paths",
        "src/pay/pay.txt",
    ]);
    assert_eq!(b.facts["color"], "red");
    assert_eq!(
        r.gdd(&["ask", "r1", "--of", "brief", "Approve?"]).facts["refused"],
        "out-of-order"
    );
    assert_eq!(
        r.report("r1", "reading", &report_block("gdd-review", "pass"), &[])
            .code,
        0
    );
    assert_eq!(r.gdd(&["ask", "r1", "--of", "brief", "Approve?"]).code, 0);
    r.gdd(&[
        "approve",
        "r1",
        "--of",
        "brief",
        "--who",
        "Person",
        "--where",
        "conversation",
        "features/r1.feature",
        "gdd/briefs/r1.md",
    ]);
    r.commit_on(
        "gdd-seal/r1",
        None,
        &[("tests/r1.sh", "true # gdd:r1-1\n")],
        "Seal",
    );
    r.report("r1", "checks", &sealed_report(&["tests"]), &[]);
    let start = r.git(&["rev-parse", "gdd-seal/r1"]);
    r.commit_on(
        "work/r1",
        Some(&start),
        &[("src/pay/pay.txt", "pay twice\n")],
        "Pay",
    );
    r.report("r1", "build", &report_block("gdd-build", "done"), &[]);
    assert_eq!(r.gdd(&["gates", "r1"]).code, 0);
    let rv = r.report(
        "r1",
        "review",
        &review_text("stop", "red", "f1 src/pay/pay.txt:1 it charges twice"),
        &[],
    );
    assert_eq!(rv.code, 0, "{}", rv.out);
    let n = r.gdd(&["next", "r1"]);
    assert_eq!(n.code, 1);
    assert_eq!(
        n.facts["state"]["next"]["blocked"], "review-stop",
        "{}",
        n.out
    );
    // The orchestrator's copies of the brief go before the merge, so the main branch brings its own.
    std::fs::remove_file(r.path("gdd/briefs/r1.md")).unwrap();
    std::fs::remove_file(r.path("features/r1.feature")).unwrap();
    r.git(&["merge", "-q", "--no-ff", "-m", "Merge r1", "work/r1"]);
    let s = r.gdd(&["ship", "r1", "--done", "main"]);
    assert_eq!(s.code, 1);
    assert_eq!(s.facts["refused"], "out-of-order", "{}", s.out);
    // Told the person, and they answered: a fix round, never a waiver.
    r.gdd(&[
        "ask",
        "r1",
        "--of",
        "stop",
        "It charges twice: I'd fix it and review it again. Yes?",
    ]);
    r.gdd(&["answer", "r1", "yes"]);
    assert_eq!(
        r.gdd(&["next", "r1"]).facts["state"]["next"]["step"],
        "build"
    );
}

#[test]
fn a_finished_build_with_its_rounds_spent_is_still_judged() {
    let bp = BLUEPRINT.replace("      rounds_per_change: 3\n      fix_rounds_per_gate: 2\n      runs_before_review: 2\n  red:", "      rounds_per_change: 1\n      fix_rounds_per_gate: 2\n      runs_before_review: 2\n  red:");
    let r = Repo::with(&bp);
    yellow_brief(&r, "y2", "src/core/app.txt");
    r.gdd(&[
        "approve",
        "y2",
        "--of",
        "brief",
        "--who",
        "Person",
        "--where",
        "conversation",
        "features/y2.feature",
    ]);
    r.commit_on("gdd-seal/y2", None, &[("tests/y2.sh", "true\n")], "Seal");
    r.report("y2", "checks", &sealed_report(&["tests"]), &[]);
    let start = r.git(&["rev-parse", "gdd-seal/y2"]);
    r.commit_on(
        "work/y2",
        Some(&start),
        &[("src/core/app.txt", "bad\n")],
        "Round 1",
    );
    r.report("y2", "build", &report_block("gdd-build", "done"), &[]);
    assert_eq!(r.gdd(&["gates", "y2"]).code, 1);
    r.commit_on(
        "work/y2",
        None,
        &[("src/core/app.txt", "still bad\n")],
        "Round 2",
    );
    r.report("y2", "build", &report_block("gdd-build", "done"), &[]);
    let lim = r.gdd(&["limits", "y2"]);
    assert_eq!(lim.code, 0, "{}", lim.out);
    assert_eq!(lim.facts["judge"], "gates");
    assert!(lim.has("rounds 1 of 1, no room left"));
    // Its gates still run.
    let g = r.gdd(&["gates", "y2"]);
    assert_eq!(g.code, 1, "it fails, but it was judged: {}", g.out);
    assert!(g.has("judged all the same"));
    // And no new round starts.
    let l = r.gdd(&["launch", "y2", "build"]);
    assert_eq!(l.facts["refused"], "cap-spent", "{}", l.out);
    let n = r.gdd(&["next", "y2"]);
    assert_eq!(n.facts["state"]["next"]["blocked"], "cap-spent");
    // The same reply grants another attempt without reopening the brief.
    assert_eq!(r.gdd(&["ask", "y2", "--of", "waiver", "The total still looks wrong. The one try we agreed on is spent. One more could fix the total. Leave it for later?"]).code, 0);
    let w = r.gdd(&[
        "waiver",
        "y2",
        "--who",
        "Person",
        "--check",
        "rounds_per_change",
        "--why",
        "one more try",
        "--backlog",
        "y2-rounds",
        "--limit",
        "rounds_per_change",
        "--to",
        "2",
    ]);
    assert_eq!(w.code, 0, "{}", w.out);
    assert!(r.read("gdd/backlog.md").contains("| y2-rounds |"));
    assert_eq!(
        r.gdd(&["ask", "y2", "--of", "brief", "Approve it again?"])
            .facts["refused"],
        "already-approved"
    );
    let lim = r.gdd(&["limits", "y2"]);
    assert!(lim.has("rounds 1 of 2, raised by a waiver"), "{}", lim.out);
    assert_eq!(
        r.gdd(&["next", "y2"]).facts["state"]["next"]["step"],
        "build"
    );
}

#[test]
fn refusals() {
    let r = Repo::new();
    yellow_brief(&r, "y3", "src/core/app.txt");
    // A build reported before the approval its color asks for.
    let b = r.report("y3", "build", &report_block("gdd-build", "done"), &[]);
    assert_eq!(b.code, 1);
    assert_eq!(b.facts["refused"], "needs-approval", "{}", b.out);
    // A gates step can't be composed by hand.
    let l = r.gdd(&[
        "log",
        "y3",
        r#"{"event":"step","step":"gates","start":"x","gates":[]}"#,
    ]);
    assert_eq!(l.facts["refused"], "use-the-command");
    // A hash composed by hand that isn't the file's.
    let h = r.gdd(&["log", "y3", r#"{"event":"approval","who":"P","of":"brief","version":1,"files":{"features/y3.feature":"0000000000000000000000000000000000000000"},"where":"conversation"}"#]);
    assert_eq!(h.facts["refused"], "hash-mismatch", "{}", h.out);
    // An event of an unknown shape.
    let u = r.gdd(&["log", "y3", r#"{"event":"ask","of":"lunch","what":"x"}"#]);
    assert_eq!(u.code, 1);
    // A ship before its gates.
    r.gdd(&[
        "approve",
        "y3",
        "--of",
        "brief",
        "--who",
        "P",
        "--where",
        "conversation",
        "features/y3.feature",
    ]);
    let s = r.gdd(&["ship", "y3", "--done", "main"]);
    assert_eq!(s.facts["refused"], "out-of-order", "{}", s.out);
    // A report without its block is a step that didn't happen.
    let none = r.report("y3", "checks", "I sealed it.", &[]);
    assert_eq!(none.facts["refused"], "no-report");
    // A review whose first word isn't a verdict.
    // An answer with no question open.
    let a = r.gdd(&["answer", "y3", "sure"]);
    assert_eq!(a.facts["refused"], "no-open-ask");
    // A wrong id is called wrong.
    assert_eq!(r.gdd(&["next", "Not An Id"]).code, 2);
    // A correction appends, never rewrites.
    let before = r.read("gdd/log/y3.jsonl");
    let c = r.gdd(&["correct", "y3", "--line", "2", "--void", "written twice"]);
    assert_eq!(c.code, 0, "{}", c.out);
    let after = r.read("gdd/log/y3.jsonl");
    assert!(after.starts_with(&before));
    assert_eq!(after.lines().count(), before.lines().count() + 1);
}

#[test]
fn a_new_version_of_a_brief_needs_new_checks_and_keeps_its_base() {
    let r = Repo::new();
    yellow_brief(&r, "y4", "src/core/app.txt");
    let base = r
        .log_lines("y4")
        .into_iter()
        .find(|e| e["step"] == "brief")
        .unwrap()["color"]["base"]
        .clone();
    r.gdd(&[
        "approve",
        "y4",
        "--of",
        "brief",
        "--who",
        "P",
        "--where",
        "conversation",
        "features/y4.feature",
    ]);
    r.commit_on("gdd-seal/y4", None, &[("tests/y4.sh", "true\n")], "Seal");
    r.report("y4", "checks", &sealed_report(&["tests"]), &[]);
    // The brief changes prose only: a new version, no new approval needed, but its checks come up to it.
    let text = r
        .read("gdd/briefs/y4.md")
        .replace("version: 1", "version: 2")
        .replace("The app says ok.", "The app says ok, plainly.");
    r.write("gdd/briefs/y4.md", &text);
    // Not sealed yet: refused.
    let b = r.gdd(&[
        "brief",
        "y4",
        "gdd/briefs/y4.md",
        "features/y4.feature",
        "--paths",
        "src/core/app.txt",
    ]);
    assert_eq!(b.facts["refused"], "seal-mismatch", "{}", b.out);
    r.gdd(&["seal", "y4", "gdd/briefs/y4.md"]);
    let b = r.gdd(&[
        "brief",
        "y4",
        "gdd/briefs/y4.md",
        "features/y4.feature",
        "--paths",
        "src/core/app.txt",
    ]);
    assert_eq!(b.code, 0, "{}", b.out);
    let lines = r.log_lines("y4");
    let last = lines.iter().rev().find(|e| e["step"] == "brief").unwrap();
    assert_eq!(last["version"], 2);
    assert_eq!(last["color"]["base"], base);
    assert_eq!(b.facts["next"], "checks", "{}", b.out);
    assert_eq!(
        r.gdd(&["ask", "y4", "--of", "brief", "Approve it again?"])
            .facts["refused"],
        "already-approved"
    );
    // The role finds that what it sealed still covers the new version, and seals nothing new: its
    // checks stand, as they were sealed, and the change goes on to its build.
    let c = r.report("y4", "checks", &sealed_report(&["tests"]), &[]);
    assert_eq!(c.code, 0, "{}", c.out);
    let lines = r.log_lines("y4");
    let checks = &lines.last().unwrap()["seal"]["checks"];
    assert!(checks["tests/y4.sh"].is_string(), "{checks}");
    assert_eq!(c.facts["next"], "build", "{}", c.out);
    // Once they stand, the same word again is nothing sealed.
    let again = r.report("y4", "checks", &sealed_report(&["tests"]), &[]);
    assert_eq!(again.facts["refused"], "not-sealed", "{}", again.out);
}

fn approved(r: &Repo, id: &str) {
    let f = format!("features/{id}.feature");
    let a = r.gdd(&[
        "approve",
        id,
        "--of",
        "brief",
        "--who",
        "P",
        "--where",
        "conversation",
        &f,
    ]);
    assert_eq!(a.code, 0, "{}", a.out);
}

/// A yellow change whose checks role sealed `tests/<id>.sh`.
fn checked(r: &Repo, id: &str) {
    yellow_brief(r, id, "src/core/app.txt");
    approved(r, id);
    let test = format!("tests/{id}.sh");
    r.commit_on(
        &format!("gdd-seal/{id}"),
        None,
        &[(test.as_str(), "true\n")],
        "Seal",
    );
    let c = r.report(id, "checks", &sealed_report(&["tests"]), &[]);
    assert_eq!(c.code, 0, "{}", c.out);
}

/// A new version of the brief, sealed with `files` and briefed with them.
fn new_version(r: &Repo, id: &str, files: &[&str], extra: &[&str]) {
    let brief = format!("gdd/briefs/{id}.md");
    let text = r.read(&brief).replace("version: 1", "version: 2");
    r.write(&brief, &text);
    let mut seal = vec!["seal", id];
    seal.extend(files);
    assert_eq!(r.gdd(&seal).code, 0);
    let mut b = vec!["brief", id];
    b.extend(files);
    b.extend(["--paths", "src/core/app.txt"]);
    b.extend(extra);
    let b = r.gdd(&b);
    assert_eq!(b.code, 0, "{}", b.out);
}

fn sealed_nothing(r: &Repo, id: &str) -> Run {
    r.report(id, "checks", &report_block("gdd-checks", "sealed"), &[])
}

/// With no checks sealed yet, the role's word that nothing had to change is still nothing sealed.
#[test]
fn a_first_checks_round_that_seals_nothing_is_refused() {
    let r = Repo::new();
    yellow_brief(&r, "y6", "src/core/app.txt");
    approved(&r, "y6");
    assert_eq!(sealed_nothing(&r, "y6").facts["refused"], "not-sealed");
}

#[test]
fn reused_proofs_must_cover_the_revised_paths() {
    let r = Repo::with(&BLUEPRINT.replace("id: slow", "id: slow\n    cases: each change"));
    checked(&r, "pv");
    let brief = "gdd/briefs/pv.md";
    r.write(
        brief,
        &r.read(brief).replace(
            "  - 'src/core/app.txt'",
            "  - 'src/core/app.txt'\n  - 'src/ui/page.txt'",
        ),
    );
    new_version(
        &r,
        "pv",
        &[brief, "features/pv.feature"],
        &["src/ui/page.txt"],
    );
    let result = sealed_nothing(&r, "pv");
    assert_eq!(result.facts["refused"], "missing-proofs", "{}", result.out);
    assert!(result.has("rejects proof for slow"));
    let completed = r.report("pv", "checks", &sealed_report(&["slow"]), &[]);
    assert_eq!(completed.code, 0, "{}", completed.out);
    assert_eq!(
        r.log_lines("pv").pop().unwrap()["seal"]["proofs"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
}

/// A new scenario needs its checks: the ones sealed before don't stand for it.
#[test]
fn a_new_scenario_needs_the_role_to_seal_again() {
    let r = Repo::new();
    checked(&r, "y7");
    let f = r.read("features/y7.feature")
        + "\n  @brief:y7 @id:y7-2\n  Scenario: It refuses a bad value\n    Given the app\n    Then it refuses\n";
    r.write("features/y7.feature", &f);
    new_version(&r, "y7", &["gdd/briefs/y7.md", "features/y7.feature"], &[]);
    approved(&r, "y7");
    assert_eq!(sealed_nothing(&r, "y7").facts["refused"], "not-sealed");
}

/// A color that rose asks more of the checks: the ones sealed for yellow don't stand for red.
#[test]
fn a_higher_color_needs_the_role_to_seal_again() {
    let r = Repo::new();
    checked(&r, "y8");
    let b = r.gdd(&[
        "brief",
        "y8",
        "gdd/briefs/y8.md",
        "features/y8.feature",
        "--paths",
        "src/core/app.txt",
        "--color",
        "red",
        "--why",
        "the reviewer found red",
    ]);
    assert_eq!(b.code, 0, "{}", b.out);
    assert_eq!(sealed_nothing(&r, "y8").facts["refused"], "not-sealed");
}

/// A check changed on the seal and passed off as a file of the brief doesn't stand either.
#[test]
fn a_check_changed_as_a_brief_file_needs_the_role_to_seal_again() {
    let r = Repo::new();
    checked(&r, "y9");
    r.write("tests/y9.sh", "exit 0 # weakened\n");
    new_version(
        &r,
        "y9",
        &["gdd/briefs/y9.md", "features/y9.feature", "tests/y9.sh"],
        &[],
    );
    approved(&r, "y9");
    assert_eq!(sealed_nothing(&r, "y9").facts["refused"], "not-sealed");
}

/// What a merge of the main branch brought to the seal is no change of its checks: after it, a
/// new version of the brief in prose only still finds its checks standing. In one run the merge
/// brought another change's test and logs, and the layer refused the role's word, round after round.
#[test]
fn checks_still_stand_for_a_new_version_after_a_merge_of_main() {
    let r = Repo::new();
    checked(&r, "ym");
    r.write("src/ui/page.txt", "another page\n");
    r.commit("Something else shipped");
    assert_eq!(r.gdd(&["seal", "ym", "--merge-main"]).code, 0);
    new_version(&r, "ym", &["gdd/briefs/ym.md", "features/ym.feature"], &[]);
    let c = sealed_nothing(&r, "ym");
    assert_eq!(c.code, 0, "{}", c.out);
    let lines = r.log_lines("ym");
    let first = lines.iter().find(|e| e["step"] == "checks").unwrap();
    let seal = &lines.last().unwrap()["seal"];
    assert!(!first["seal"]["proofs"].as_array().unwrap().is_empty());
    assert_eq!(seal["proofs"], first["seal"]["proofs"]);
    let checks = &seal["checks"];
    assert!(checks["tests/ym.sh"].is_string(), "{checks}");
    assert!(checks["src/ui/page.txt"].is_null(), "{checks}");
}

/// But a file main brought and the seal then changed is the seal's own change: the role seals again.
#[test]
fn a_file_from_main_changed_on_the_seal_needs_the_role_to_seal_again() {
    let r = Repo::new();
    checked(&r, "yn");
    r.write("src/ui/page.txt", "another page\n");
    r.commit("Something else shipped");
    assert_eq!(r.gdd(&["seal", "yn", "--merge-main"]).code, 0);
    r.write("src/ui/page.txt", "a page changed on the seal\n");
    prose_version_sealing(&r, "yn", "src/ui/page.txt");
    refused_as_nothing_sealed(&r, "yn");
}

/// A new version of the brief in prose, sealed with `other`, a file the brief doesn't list.
fn prose_version_sealing(r: &Repo, id: &str, other: &str) {
    let brief = format!("gdd/briefs/{id}.md");
    r.write(&brief, &r.read(&brief).replace("version: 1", "version: 2"));
    assert_eq!(r.gdd(&["seal", id, &brief, other]).code, 0);
    let feature = format!("features/{id}.feature");
    let b = r.gdd(&["brief", id, &brief, &feature, "--paths", "src/core/app.txt"]);
    assert_eq!(b.code, 0, "{}", b.out);
}

/// What the role's word is refused with when the checks don't stand.
fn refused_as_nothing_sealed(r: &Repo, id: &str) {
    let c = sealed_nothing(r, id);
    assert_eq!(c.facts["refused"], "not-sealed", "{}", c.out);
    assert!(
        c.out.contains("the checks role sealed nothing"),
        "{}",
        c.out
    );
}

/// What main holds is not what the seal took in: if main comes to hold the seal itself, what the
/// seal changed doesn't pass for something main brought.
#[test]
fn a_file_sealed_after_the_checks_does_not_pass_for_main_once_main_holds_the_seal() {
    let r = Repo::new();
    checked(&r, "yq");
    r.write("tests/helper.sh", "exit 0\n");
    prose_version_sealing(&r, "yq", "tests/helper.sh");
    r.commit("Main before it takes the seal in");
    r.git(&["merge", "-q", "--no-edit", "gdd-seal/yq"]);
    refused_as_nothing_sealed(&r, "yq");
}

/// A merge made by hand on the seal, with a parent main never held, brings nothing from main.
#[test]
fn a_merge_of_something_else_does_not_pass_for_main() {
    let r = Repo::new();
    checked(&r, "yr");
    let side = r.commit_on(
        "side",
        Some("HEAD"),
        &[("tests/helper.sh", "exit 0\n")],
        "Not main",
    );
    let tree = r.git(&["merge-tree", "--write-tree", "gdd-seal/yr", &side]);
    merged_by_hand_then_prose(&r, "yr", &side, &tree);
    refused_as_nothing_sealed(&r, "yr");
}

/// Nor does a tag named like the main branch stand in for it.
#[test]
fn a_tag_named_main_does_not_pass_for_the_main_branch() {
    let r = Repo::new();
    checked(&r, "yt");
    let side = r.commit_on(
        "side",
        Some("HEAD"),
        &[("tests/helper.sh", "exit 0\n")],
        "Not main",
    );
    r.git(&["tag", "main", &side]);
    let tree = r.git(&["merge-tree", "--write-tree", "gdd-seal/yt", &side]);
    merged_by_hand_then_prose(&r, "yt", &side, &tree);
    refused_as_nothing_sealed(&r, "yt");
}

/// Nor a merge by hand of a main the seal already held: it brings nothing new, so an older version
/// of a file main has since changed doesn't pass for main's.
#[test]
fn a_merge_of_a_main_the_seal_already_held_does_not_pass_for_main() {
    let r = Repo::new();
    checked(&r, "yu");
    r.write("src/ui/page.txt", "first page\n");
    let first = r.commit("Something else shipped");
    assert_eq!(r.gdd(&["seal", "yu", "--merge-main"]).code, 0);
    r.write("src/ui/page.txt", "second page\n");
    r.commit("It changed again");
    assert_eq!(r.gdd(&["seal", "yu", "--merge-main"]).code, 0);
    let old = r.commit_on(
        "old",
        Some("gdd-seal/yu"),
        &[("src/ui/page.txt", "first page\n")],
        "The page main had first",
    );
    let tree = r.git(&["rev-parse", &format!("{old}^{{tree}}")]);
    merged_by_hand_then_prose(&r, "yu", &first, &tree);
    refused_as_nothing_sealed(&r, "yu");
}

/// A merge of the seal with `other`, made by hand to hold `tree`, then a new version of the brief
/// in prose.
fn merged_by_hand_then_prose(r: &Repo, id: &str, other: &str, tree: &str) {
    let branch = format!("refs/heads/gdd-seal/{id}");
    let seal = r.git(&["rev-parse", &branch]);
    let merge = r.git(&["commit-tree", tree, "-p", &seal, "-p", other, "-m", "Merge"]);
    r.git(&["update-ref", &branch, &merge, &seal]);
    let path = format!("gdd/briefs/{id}.md");
    let brief = r.read(&path).replace("version: 1", "version: 2");
    r.write(&path, &brief);
    r.commit_on(
        &format!("gdd-seal/{id}"),
        None,
        &[(&path, &brief)],
        "Brief v2",
    );
    let feature = format!("features/{id}.feature");
    let b = r.gdd(&["brief", id, &path, &feature, "--paths", "src/core/app.txt"]);
    assert_eq!(b.code, 0, "{}", b.out);
}

/// Nor does a scenario of the change that main changed: it is a brief file, and needs the role.
#[test]
fn a_scenario_changed_on_main_needs_the_role_to_seal_again() {
    let r = Repo::new();
    checked(&r, "yp");
    r.commit("The brief lands on main as it is");
    assert_eq!(r.gdd(&["seal", "yp", "--merge-main"]).code, 0);
    let f = r.read("features/yp.feature")
        + "\n  @brief:yp @id:yp-2\n  Scenario: It refuses a bad value\n    Given the app\n    Then it refuses\n";
    r.write("features/yp.feature", &f);
    r.commit("A scenario changed on main");
    assert_eq!(r.gdd(&["seal", "yp", "--merge-main"]).code, 0);
    new_version(&r, "yp", &["gdd/briefs/yp.md", "features/yp.feature"], &[]);
    approved(&r, "yp");
    refused_as_nothing_sealed(&r, "yp");
}

/// And one of the change's own checks that main changed is still a check that changed.
#[test]
fn a_check_changed_on_main_needs_the_role_to_seal_again() {
    let r = Repo::new();
    checked(&r, "yo");
    // Main comes to hold the same check, takes it to the seal, and then changes it.
    r.write("tests/yo.sh", "true\n");
    r.commit("The same check lands on main");
    assert_eq!(r.gdd(&["seal", "yo", "--merge-main"]).code, 0);
    r.write("tests/yo.sh", "exit 0 # weakened on main\n");
    r.commit("Something else shipped");
    let m = r.gdd(&["seal", "yo", "--merge-main"]);
    assert_eq!(m.code, 0, "{}", m.out);
    new_version(&r, "yo", &["gdd/briefs/yo.md", "features/yo.feature"], &[]);
    refused_as_nothing_sealed(&r, "yo");
}

/// A checks step voided by a correction, and then the main branch merged into the seal: the
/// test the role sealed is still what it sealed, and nothing main brought counts as its checks.
#[test]
fn checks_sealed_before_a_merge_of_main_still_count_once_their_step_is_voided() {
    let r = Repo::new();
    yellow_brief(&r, "y5", "src/core/app.txt");
    r.gdd(&["ask", "y5", "--of", "brief", "Approve it?"]);
    let ap = r.gdd(&[
        "approve",
        "y5",
        "--of",
        "brief",
        "--who",
        "Person",
        "--where",
        "conversation",
        "gdd/briefs/y5.md",
        "features/y5.feature",
    ]);
    assert_eq!(ap.code, 0, "{}", ap.out);
    r.commit_on(
        "gdd-seal/y5",
        None,
        &[("tests/y5.sh", "grep -q ok src/core/app.txt # gdd:y5-1\n")],
        "Seal the test of y5",
    );
    let sealed = format!("Sealed.\n{}", sealed_report(&["tests"]));
    assert_eq!(r.report("y5", "checks", &sealed, &[]).code, 0);
    let line = r.log_lines("y5").len().to_string();
    let v = r.gdd(&[
        "correct",
        "y5",
        "--line",
        &line,
        "--void",
        "logged too early",
    ]);
    assert_eq!(v.code, 0, "{}", v.out);
    // Something else ships meanwhile, and the seal takes the main branch in.
    r.write("src/ui/page.txt", "another page\n");
    r.commit("Something else shipped");
    assert_eq!(r.gdd(&["seal", "y5", "--merge-main"]).code, 0);
    let c = r.report("y5", "checks", &sealed, &[]);
    assert_eq!(c.code, 0, "{}", c.out);
    let lines = r.log_lines("y5");
    let checks = &lines.last().unwrap()["seal"]["checks"];
    assert!(checks["tests/y5.sh"].is_string(), "{checks}");
    assert!(checks["src/ui/page.txt"].is_null(), "{checks}");
    assert_eq!(
        r.gdd(&["next", "y5"]).facts["state"]["next"]["step"],
        "build"
    );
}

#[test]
fn a_finding_about_a_check_goes_to_the_checks_role_and_the_builder_is_told_of_the_new_seal() {
    let r = Repo::with(&BLUEPRINT.replace(
        "limits:\n  lines_per_change",
        "limits:\n  tokens_per_change: 1000\n  lines_per_change",
    ));
    yellow_brief(&r, "y7", "src/core/app.txt");
    r.gdd(&[
        "approve",
        "y7",
        "--of",
        "brief",
        "--who",
        "Person",
        "--where",
        "conversation",
        "gdd/briefs/y7.md",
        "features/y7.feature",
    ]);
    r.commit_on(
        "gdd-seal/y7",
        None,
        &[("tests/y7.sh", "true # gdd:y7-1\n")],
        "Seal",
    );
    r.report("y7", "checks", &sealed_report(&["tests"]), &[]);
    let start = r.git(&["rev-parse", "gdd-seal/y7"]);
    r.commit_on(
        "work/y7",
        Some(&start),
        &[("src/core/app.txt", "it says ok\n")],
        "Round 1",
    );
    let claim = serde_json::json!([{
        "check": "tests", "command": "sh check-test", "exit": 1,
        "tail": format!("start {} end", "á".repeat(5000))
    }]);
    let message = format!(
        "```json\n{claim}\n```\n{}",
        build_report("decision: Use a single line.\n")
    );
    assert_eq!(r.report("y7", "build", &message, &[]).code, 0);
    assert_eq!(r.gdd(&["gates", "y7"]).code, 0);
    let found = "f1 tests/y7.sh:1 the test passes whatever the app says | @id:y7-1\n";
    let rv = r.report("y7", "review", &review_text("fix", "yellow", found), &[]);
    assert_eq!(rv.code, 0, "{}", rv.out);
    assert_eq!(rv.facts["next"], "checks", "{}", rv.out);
    let early = r.report("y7", "build", &report_block("gdd-build", "done"), &[]);
    assert_eq!(early.facts["refused"], "out-of-order", "{}", early.out);
    // The checks role is told what the reviewer found: a better check is its to write.
    let c = r.gdd(&["launch", "y7", "checks"]);
    let prompt = c.facts["prompt"].as_str().unwrap();
    assert!(prompt.contains("The reviewer said fix"), "{prompt}");
    assert!(
        prompt.contains("f1 tests/y7.sh:1: the test passes whatever the app says | @id:y7-1"),
        "{prompt}"
    );
    assert!(
        prompt.contains(&format!("Current seal tip: {start}")),
        "{prompt}"
    );
    assert!(prompt.contains("`sh check-test` exit 1"), "{prompt}");
    assert!(!prompt.contains("exit 1; start"), "{prompt}");
    assert!(
        prompt.contains(&format!("{} end", "á".repeat(96))),
        "{prompt}"
    );
    assert!(!prompt.contains(&"á".repeat(100)), "{prompt}");
    assert!(prompt.contains("yours to answer"), "{prompt}");
    r.commit_on(
        "gdd-seal/y7",
        None,
        &[("tests/y7.sh", "grep -q ok src/core/app.txt # gdd:y7-1\n")],
        "Seal a test that looks",
    );
    let again = r.report("y7", "checks", &sealed_report(&["tests"]), &[]);
    assert_eq!(again.code, 0, "{}", again.out);
    // The builder gets the same finding, and is told the seal already answered it.
    let b = r.gdd(&["launch", "y7", "build"]);
    let prompt = b.facts["prompt"].as_str().unwrap();
    assert!(prompt.contains("f1 tests/y7.sh:1"), "{prompt}");
    assert!(prompt.contains(&format!(
        "Current seal tip: {}",
        r.git(&["rev-parse", "gdd-seal/y7"])
    )));
    assert!(prompt.contains("`sh check-test` exit 1"), "{prompt}");
    assert!(
        prompt.contains("Run the required checks on the fix"),
        "{prompt}"
    );
    assert!(
        prompt.contains("sealed again after that review"),
        "{prompt}"
    );
    assert!(prompt.contains("merge <start> into it"), "{prompt}");
    for words in [
        "Work on branch work/y7",
        "`user.name` and `user.email`",
        "CANNOT:",
        "Never force it",
    ] {
        assert!(prompt.contains(words), "{prompt}");
    }
    assert!(
        prompt.contains("work/y7 from its current tip in the project"),
        "{prompt}"
    );
    assert_eq!(again.facts["next"], "build");
    assert_eq!(r.gdd(&["gates", "y7"]).facts["refused"], "out-of-order");
    let early = r.report("y7", "build", &build_report(""), &[]);
    assert_eq!(early.facts["refused"], "start-missing", "{}", early.out);
    assert!(early.has("gdd launch y7 build"));
    merge_corrected_seal(&r, "y7");
    let merged = r.report(
        "y7",
        "build",
        &report_block("gdd-build", "done"),
        &["--tokens", "73"],
    );
    assert_eq!(merged.facts["next"], "gates", "{}", merged.out);
    assert_eq!(merged.facts["round"], 2);
    let limits = r.gdd(&["limits", "y7"]);
    assert_eq!(limits.facts["caps"]["rounds"]["used"], 0.0);
    assert_eq!(limits.facts["caps"]["tokens"]["used"], 73.0);
    assert_eq!(r.gdd(&["gates", "y7"]).facts["next"], "review");
    let review = r.gdd(&["launch", "y7", "review"]);
    let prompt = review.facts["prompt"].as_str().unwrap();
    assert!(
        prompt.contains("fresh clone in your temporary folder"),
        "{prompt}"
    );
    assert!(prompt.contains("The product has not changed"), "{prompt}");
    assert!(prompt.contains("Changed tests: tests/y7.sh"), "{prompt}");
    assert!(prompt.contains("Review only those tests"), "{prompt}");
    assert!(prompt.contains("- \"Use a single line.\""), "{prompt}");
    assert!(
        prompt.contains("f1 tests/y7.sh:1: the test passes whatever the app says | @id:y7-1"),
        "{prompt}"
    );
    assert_eq!(r.gdd(&["verify", "y7"]).facts["errors"], 0);

    // Gates can run on a newer head. The review scope follows what they actually judged.
    r.commit_on(
        "work/y7",
        None,
        &[("src/core/app.txt", "ok again")],
        "Change the product",
    );
    assert_eq!(r.gdd(&["gates", "y7"]).facts["next"], "review");
    let review = r.gdd(&["launch", "y7", "review"]);
    let prompt = review.facts["prompt"].as_str().unwrap();
    assert!(!prompt.contains("The product has not changed"), "{prompt}");
    assert!(prompt.contains("f1 tests/y7.sh:1"), "{prompt}");

    // Without the new fields this is a valid older log, with the original build count.
    let mut lines = r.log_lines("y7");
    for event in &mut lines {
        let fields = event.as_object_mut().unwrap();
        fields.remove("seal_only");
        fields.remove("fix_role");
    }
    let old: Vec<_> = lines.iter().map(ToString::to_string).collect();
    r.write("gdd/log/y7.jsonl", &old.join("\n"));
    assert_eq!(r.gdd(&["verify", "y7"]).facts["errors"], 0);
    assert_eq!(
        r.gdd(&["limits", "y7"]).facts["caps"]["rounds"]["used"],
        1.0
    );
}

fn merge_corrected_seal(r: &Repo, id: &str) {
    let dir = r.dir.path().join("merge");
    let path = dir.to_str().unwrap();
    r.git(&["worktree", "add", "-q", path, &format!("work/{id}")]);
    git(
        &dir,
        &["merge", "-q", "--no-edit", &format!("gdd-seal/{id}")],
    );
    r.git(&["worktree", "remove", path]);
}

fn gated_change(r: &Repo, id: &str, decisions: &str) -> String {
    checked(r, id);
    let head = r.commit_on(
        &format!("work/{id}"),
        Some(&format!("gdd-seal/{id}")),
        &[("src/core/app.txt", "ok")],
        "Build",
    );
    assert_eq!(r.report(id, "build", &build_report(decisions), &[]).code, 0);
    assert_eq!(r.gdd(&["gates", id]).facts["next"], "review");
    head
}

fn return_test(r: &Repo, id: &str) {
    let finding = format!("f1 tests/{id}.sh:1 accepts a blank title | @id:{id}-1");
    let fix = r.report(id, "review", &review_text("fix", "yellow", &finding), &[]);
    assert_eq!(fix.facts["next"], "checks", "{}", fix.out);
}

fn correct_test(r: &Repo, id: &str, extra: &[(&str, &str)]) {
    let test = format!("tests/{id}.sh");
    let mut files = vec![(test.as_str(), "test -s src/core/app.txt")];
    files.extend_from_slice(extra);
    r.commit_on(&format!("gdd-seal/{id}"), None, &files, "Correct the test");
    let sealed = r.report(id, "checks", &sealed_report(&["tests"]), &[]);
    assert_eq!(sealed.facts["next"], "build", "{}", sealed.out);
    merge_corrected_seal(r, id);
}

#[test]
fn a_review_cannot_launch_after_the_product_moves_past_the_gates() {
    let r = Repo::new();
    gated_change(&r, "moving", "");
    r.commit_on(
        "work/moving",
        None,
        &[("src/core/app.txt", "ok later")],
        "Change the title",
    );
    let launch = r.gdd(&["launch", "moving", "review"]);
    assert_eq!(launch.facts["refused"], "out-of-order", "{}", launch.out);
    assert!(launch.has("gdd gates moving"));
}

#[test]
fn a_fix_survives_formatting_of_the_reviewed_line() {
    let r = Repo::new();
    let head = gated_change(&r, "stamp", "");
    let launch = r.gdd(&["launch", "stamp", "review"]);
    let stamp = launch.facts["prompt"]
        .as_str()
        .unwrap()
        .lines()
        .find_map(|line| line.strip_prefix("Copy this line into your gdd-report block: reviewed: "))
        .unwrap();
    let fix = review_text("fix", "yellow", "f1 title is blank | @id:stamp-1");
    let report = |value: &str| {
        let text = fix.replace("caught: none", &format!("caught: none\nreviewed: {value}"));
        r.report("stamp", "review", &text, &[])
    };
    let other = "a".repeat(40);
    let stale = report(&other);
    assert_eq!(stale.facts["refused"], "stale-review");
    assert!(stale.has(&other) && stale.has(stamp), "{}", stale.out);

    let before = r.read("gdd/log/stamp.jsonl");
    for value in [
        stamp,
        &format!("`{stamp}`."),
        &format!("{stamp}."),
        &stamp.to_ascii_uppercase(),
        &head,
        &head[..7],
        &"f".repeat(41),
        "",
        "none",
        "<copy the value from the prompt's reviewed: line, or omit this line if none>",
    ] {
        r.write("gdd/log/stamp.jsonl", &before);
        let written = report(value);
        assert_eq!(written.facts["next"], "build", "{value}: {}", written.out);
        assert_eq!(r.log_lines("stamp").last().unwrap()["verdict"], "fix");
    }
}

#[test]
fn a_manual_pass_requires_the_reviewed_line_from_its_prompt() {
    let r = Repo::new();
    let head = gated_change(&r, "stamp", "");
    let launch = r.gdd(&["launch", "stamp", "review"]);
    let pass = review_text("pass", "yellow", "");
    let before = r.read("gdd/log/stamp.jsonl");
    for field in [
        "",
        "reviewed:",
        "reviewed: none",
        "reviewed: <copy the value from the prompt's reviewed: line, or omit this line if none>",
        &format!("reviewed: {}", &head[..7]),
    ] {
        let text = pass.replace("caught: none", &format!("caught: none\n{field}"));
        let refused = r.report("stamp", "review", &text, &[]);
        assert_eq!(refused.code, 1, "{field}: {}", refused.out);
        assert_eq!(
            refused.facts["refused"], "missing-reviewed",
            "{field}: {}",
            refused.out
        );
        // It names the way on and not the value: only the reviewer can say what it read.
        assert!(
            !refused.has(launch.reviewed())
                && refused.has("Ask the reviewer")
                && refused.has("gdd launch stamp review"),
            "{}",
            refused.out
        );
        assert_eq!(r.read("gdd/log/stamp.jsonl"), before);
    }
    r.commit_on(
        "work/stamp",
        None,
        &[("src/core/app.txt", "ok later")],
        "Later",
    );
    let refused = r.report("stamp", "review", &pass, &[]);
    assert_eq!(
        refused.facts["refused"], "missing-reviewed",
        "{}",
        refused.out
    );
    // With the gates pending, the line of its prompt is stale already: only a fresh review helps.
    assert!(
        refused.has("gdd gates stamp, then gdd launch stamp review")
            && !refused.has("Ask the reviewer"),
        "{}",
        refused.out
    );
    assert_eq!(r.read("gdd/log/stamp.jsonl"), before);
}

#[test]
fn a_manual_pass_needs_its_line_when_the_layer_finds_no_branch() {
    let r = Repo::new();
    let head = gated_change(&r, "stamp", "");
    r.git(&["branch", "-m", "work/stamp", "archived"]);
    let pass = review_text("pass", "yellow", "");
    let explicit = pass.replace("caught: none", &format!("caught: none\nreviewed: {head}"));
    let stale = r.report("stamp", "review", &explicit, &[]);
    assert_eq!(stale.facts["refused"], "stale-review", "{}", stale.out);
    assert!(stale.has("current: none"));
    let before = r.read("gdd/log/stamp.jsonl");
    let refused = r.report("stamp", "review", &pass, &[]);
    assert_eq!(
        refused.facts["refused"], "missing-reviewed",
        "{}",
        refused.out
    );
    assert_eq!(r.read("gdd/log/stamp.jsonl"), before);
    // A fix is never lost to this: it is still written (decision 76).
    let fix = review_text("fix", "yellow", "f1 title is blank | @id:stamp-1");
    let written = r.report("stamp", "review", &fix, &[]);
    assert_eq!(written.code, 0, "{}", written.out);
    assert_eq!(r.log_lines("stamp").last().unwrap()["verdict"], "fix");
}

#[test]
fn a_pass_nobody_read_is_refused_when_the_report_hides_the_branch() {
    let r = Repo::new();
    gated_change(&r, "hidden", "");
    assert_eq!(r.gdd(&["launch", "hidden", "review"]).code, 0);
    let later = r.commit_on(
        "work/hidden",
        None,
        &[("src/core/app.txt", "ok later")],
        "Edit the product",
    );
    assert_eq!(r.gdd(&["gates", "hidden"]).code, 0);
    let pass = review_text("pass", "yellow", "");
    let before = r.read("gdd/log/hidden.jsonl");
    // The report names a branch that isn't there.
    let refused = r.report("hidden", "review", &pass, &["--branch", "nope"]);
    assert_eq!(
        refused.facts["refused"], "missing-reviewed",
        "{}",
        refused.out
    );
    // The change's branch is renamed away for the report and back afterwards.
    r.git(&["branch", "-m", "work/hidden", "away"]);
    let refused = r.report("hidden", "review", &pass, &[]);
    r.git(&["branch", "-m", "away", "work/hidden"]);
    assert_eq!(
        refused.facts["refused"], "missing-reviewed",
        "{}",
        refused.out
    );
    assert_eq!(r.read("gdd/log/hidden.jsonl"), before);
    assert_ne!(r.gdd(&["ship", "hidden", "--merge"]).code, 0);
    assert!(!git_ok(
        &r.root,
        &["merge-base", "--is-ancestor", &later, "main"]
    ));
}

#[test]
fn a_manual_pass_accepts_the_prompt_value_or_the_commit_it_still_represents() {
    let r = Repo::new();
    let head = gated_change(&r, "stamp", "");
    let before = r.read("gdd/log/stamp.jsonl");
    let launch = r.gdd(&["launch", "stamp", "review"]);
    for value in [launch.reviewed(), &head] {
        r.write("gdd/log/stamp.jsonl", &before);
        let pass = review_text("pass", "yellow", "")
            .replace("caught: none", &format!("caught: none\nreviewed: {value}"));
        let written = r.report("stamp", "review", &pass, &[]);
        assert_eq!(written.facts["next"], "ship", "{}", written.out);
        assert_eq!(r.log_lines("stamp").last().unwrap()["verdict"], "pass");
    }
}

#[test]
fn a_review_cannot_report_against_a_later_branch_or_gate_run() {
    let r = Repo::new();
    let gated = gated_change(&r, "snapshot", "");
    let review_at_launch = || {
        let launch = r.gdd(&["launch", "snapshot", "review"]);
        let line = launch.facts["prompt"]
            .as_str()
            .unwrap()
            .lines()
            .find_map(|s| s.strip_prefix("Copy this line into your gdd-report block: "))
            .unwrap();
        review_text("pass", "yellow", "").replace("caught: none", &format!("caught: none\n{line}"))
    };
    let first = review_at_launch();
    assert_eq!(r.gdd(&["gates", "snapshot"]).code, 0);
    assert_eq!(
        r.report("snapshot", "review", &first, &[]).facts["refused"],
        "stale-review"
    );

    let second = review_at_launch();
    r.commit_on(
        "work/snapshot",
        None,
        &[("src/core/app.txt", "ok later")],
        "Edit the product",
    );
    let late = r.report("snapshot", "review", &second, &[]);
    assert!(late.has("gdd gates snapshot, then gdd launch snapshot review"));
    assert_eq!(
        r.report("snapshot", "review", &second, &["--head", &gated])
            .facts["refused"],
        "stale-review"
    );
    let bare = review_text("fix", "yellow", "f1 title is blank | @id:snapshot-1")
        .replace("caught: none", &format!("caught: none\nreviewed: {gated}"));
    for opts in [vec![], vec!["--head", &gated]] {
        let late = r.report("snapshot", "review", &bare, &opts);
        assert_eq!(late.facts["refused"], "stale-review", "{}", late.out);
    }
    assert_eq!(r.gdd(&["gates", "snapshot"]).code, 0);
    let late = r.report("snapshot", "review", &second, &[]);
    assert_eq!(late.facts["refused"], "stale-review");
    assert!(late.has("gdd launch snapshot review"));
    assert_eq!(
        r.report("snapshot", "review", &review_at_launch(), &[])
            .facts["next"],
        "ship"
    );
}

#[test]
fn a_late_product_edit_is_reviewed_from_the_gated_commit() {
    let r = Repo::new();
    let gated = gated_change(&r, "late", "");
    let first = r.gdd(&["launch", "late", "review"]);
    assert!(first.has(&gated));
    let moved = r.commit_on(
        "work/late",
        None,
        &[("src/core/app.txt", "ok later")],
        "Late edit",
    );
    return_test(&r, "late");
    let mut log = r.log_lines("late");
    let review = log.last_mut().unwrap();
    assert_eq!(review["color"]["head"], gated);
    // Previous releases could write the branch tip at report time. Replay that log too.
    review["color"]["head"] = moved.into();
    write_change_log(&r, "late", &log);
    correct_test(&r, "late", &[]);
    let build = r.report("late", "build", &build_report(""), &[]);
    assert_eq!(build.facts["next"], "gates");
    assert!(
        r.log_lines("late")
            .last()
            .unwrap()
            .get("seal_only")
            .is_none()
    );
    assert_eq!(
        r.gdd(&["limits", "late"]).facts["caps"]["rounds"]["used"],
        1.0
    );
    assert_eq!(r.gdd(&["gates", "late"]).facts["next"], "review");
    let launch = r.gdd(&["launch", "late", "review"]);
    let prompt = launch.facts["prompt"].as_str().unwrap();
    assert!(
        prompt.contains(&format!("Previous review at {gated}.")),
        "{prompt}"
    );
    assert!(!prompt.contains("The product has not changed"), "{prompt}");
}

fn report_test_merge(r: &Repo, id: &str) -> bool {
    let build = r.report(id, "build", &build_report(""), &[]);
    assert_eq!(build.facts["next"], "gates", "{}", build.out);
    r.log_lines(id).last().unwrap()["seal_only"] == true
}

#[test]
fn a_new_product_file_on_the_seal_gets_a_product_review() {
    let r = Repo::new();
    gated_change(&r, "plugin", "");
    return_test(&r, "plugin");
    correct_test(
        &r,
        "plugin",
        &[("src/core/plugin.txt", "new product behavior")],
    );
    assert!(!report_test_merge(&r, "plugin"));
    assert_eq!(r.gdd(&["gates", "plugin"]).facts["next"], "review");
    assert!(
        !r.gdd(&["launch", "plugin", "review"])
            .has("Review only those tests")
    );
}

#[test]
fn a_builder_edit_to_the_corrected_test_costs_a_round() {
    let r = Repo::new();
    gated_change(&r, "edit", "");
    return_test(&r, "edit");
    correct_test(&r, "edit", &[]);
    r.commit_on(
        "work/edit",
        None,
        &[("tests/edit.sh", "true # changed by builder")],
        "Edit the sealed test",
    );
    assert!(!report_test_merge(&r, "edit"));
    assert_eq!(
        r.gdd(&["limits", "edit"]).facts["caps"]["rounds"]["used"],
        1.0
    );
}

#[test]
fn known_bookkeeping_is_left_out_of_the_changed_tests() {
    let r = Repo::new();
    gated_change(&r, "book", "");
    return_test(&r, "book");
    correct_test(&r, "book", &[]);
    let backlog = format!("{BACKLOG}\nAnother item\n");
    r.write("gdd/backlog.md", &backlog);
    r.commit_on(
        "work/book",
        None,
        &[("gdd/backlog.md", &backlog)],
        "Record the backlog",
    );
    assert!(report_test_merge(&r, "book"));
    assert_eq!(r.gdd(&["gates", "book"]).facts["next"], "review");
    assert!(
        r.gdd(&["launch", "book", "review"])
            .has("Changed tests: tests/book.sh.")
    );
}

#[test]
fn a_previous_seal_is_no_evidence_that_the_checks_were_corrected() {
    let r = Repo::new();
    let original = gated_change(&r, "tip", "");
    return_test(&r, "tip");
    correct_test(&r, "tip", &[]);
    let mut log = r.log_lines("tip");
    // This checks step has no recorded commit. Only the original seal is known.
    log.last_mut().unwrap()["seal"]
        .as_object_mut()
        .unwrap()
        .remove("commit");
    write_change_log(&r, "tip", &log);
    let build = r.report("tip", "build", &build_report(""), &["--head", &original]);
    assert_eq!(build.facts["next"], "gates");
    assert!(r.log_lines("tip").last().unwrap()["seal_only"].is_null());
}

fn write_change_log(r: &Repo, id: &str, log: &[serde_json::Value]) {
    let lines: Vec<_> = log.iter().map(ToString::to_string).collect();
    r.write(&format!("gdd/log/{id}.jsonl"), &(lines.join("\n") + "\n"));
}

#[test]
fn a_fix_without_a_recorded_route_keeps_the_older_build_order() {
    let r = Repo::new();
    gated_change(&r, "old", "");
    return_test(&r, "old");
    let mut log = r.log_lines("old");
    log.last_mut()
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("fix_role");
    write_change_log(&r, "old", &log);
    let next = r.gdd(&["next", "old"]);
    assert_eq!(
        next.facts["state"]["next"]["command"],
        "gdd launch old build"
    );
    assert_eq!(r.gdd(&["launch", "old", "build"]).code, 0);
    r.commit_on("work/old", None, &[("src/core/app.txt", "fixed ok")], "Fix");
    assert_eq!(
        r.report("old", "build", &build_report(""), &[]).facts["next"],
        "gates"
    );
    assert_eq!(r.gdd(&["gates", "old"]).facts["next"], "review");
    assert_eq!(r.gdd(&["verify", "old"]).facts["errors"], 0);
}

#[test]
fn later_builds_keep_the_last_known_product_choices() {
    for seal_only in [false, true] {
        let r = Repo::new();
        gated_change(&r, "choices", "decision: Use eight entries.\n");
        let field = if seal_only {
            return_test(&r, "choices");
            correct_test(&r, "choices", &[]);
            "decision: none\n"
        } else {
            let fix = review_text(
                "fix",
                "yellow",
                "f1 src/core/app.txt:1 wrong title | @id:choices-1",
            );
            assert_eq!(
                r.report("choices", "review", &fix, &[]).facts["next"],
                "build"
            );
            r.commit_on(
                "work/choices",
                None,
                &[("src/core/app.txt", "ok now")],
                "Fix the title",
            );
            ""
        };
        assert_eq!(
            r.report("choices", "build", &build_report(field), &[])
                .facts["next"],
            "gates"
        );
        assert_eq!(r.gdd(&["gates", "choices"]).facts["next"], "review");
        let review = r.gdd(&["launch", "choices", "review"]);
        assert!(
            review.facts["prompt"]
                .as_str()
                .unwrap()
                .contains("Use eight entries.")
        );
        let pass = r.report(
            "choices",
            "review",
            &r.review_pass("choices", "yellow", ""),
            &[],
        );
        assert_eq!(pass.facts["next"], "ship");
        let shipped = r.gdd(&["ship", "choices", "--merge"]);
        assert_eq!(
            shipped.facts["decisions"],
            serde_json::json!(["Use eight entries."]),
            "{}",
            shipped.out
        );
    }
}

#[test]
fn checks_cannot_fix_a_product_defect_cited_at_a_test() {
    let r = Repo::new();
    gated_change(&r, "title", "");
    let findings = "f1 tests/title.sh:1 the product returns a blank title | @id:title-1\nf2 tests/title.sh:2 the product drops spaces | @id:title-1";
    let fix = r.report(
        "title",
        "review",
        &review_text("fix", "yellow", findings),
        &[],
    );
    assert_eq!(fix.facts["next"], "checks");
    let cannot = report_text(
        "gdd-checks",
        "stopped",
        "CANNOT: tests/title.sh already checks both title rules.",
    );
    let declined = r.report("title", "checks", &cannot, &[]);
    assert_eq!(
        declined.facts["next_command"], "gdd launch title build",
        "{}",
        declined.out
    );
    let build = r.named(declined.facts["next_command"].as_str().unwrap());
    let prompt = build.facts["prompt"].as_str().unwrap();
    assert!(
        prompt.contains("the product returns a blank title")
            && prompt.contains("the product drops spaces"),
        "{prompt}"
    );
    assert!(!prompt.contains("sealed again after that review"));
    r.commit_on(
        "work/title",
        None,
        &[("src/core/app.txt", "ok title")],
        "Fix both product defects",
    );
    assert_eq!(
        r.report("title", "build", &build_report(""), &[]).facts["next"],
        "gates"
    );
    assert_eq!(
        r.gdd(&["limits", "title"]).facts["caps"]["rounds"]["used"],
        1.0
    );
    assert_eq!(r.gdd(&["gates", "title"]).facts["next"], "review");
    assert!(
        !r.gdd(&["launch", "title", "review"])
            .has("Review only those tests")
    );
    let later_fix = r.report(
        "title",
        "review",
        &review_text("fix", "yellow", findings),
        &[],
    );
    assert_eq!(later_fix.facts["next_command"], "gdd launch title checks");
    assert_eq!(r.gdd(&["verify", "title"]).facts["errors"], 0);
}

#[test]
fn mixed_findings_go_to_checks_before_the_builder() {
    for findings in [
        "f1 src/core/app.txt:1 wrong value | @id:route-1",
        "f1 tests/route.sh:1 weak test | @id:route-1\nf2 src/core/app.txt:1 wrong value | @id:route-1",
        "f1 tests/route.sh:1 weak test | @id:route-1\nf2 missing result | @id:route-1",
        "f1 tests/other.sh:1 wrong value | @id:route-1",
    ] {
        let r = Repo::new();
        checked(&r, "route");
        r.commit_on(
            "work/route",
            Some("gdd-seal/route"),
            &[("src/core/app.txt", "ok")],
            "Build",
        );
        assert_eq!(
            r.report("route", "build", &report_block("gdd-build", "done"), &[])
                .code,
            0
        );
        assert_eq!(r.gdd(&["gates", "route"]).code, 0);
        let fix = r.report(
            "route",
            "review",
            &review_text("fix", "yellow", findings),
            &[],
        );
        if findings.starts_with("f1 tests/route.sh:") {
            assert_eq!(fix.facts["next_command"], "gdd launch route checks");
            assert_eq!(
                r.gdd(&["launch", "route", "build"]).facts["refused"],
                "out-of-order"
            );
            correct_test(&r, "route", &[]);
        } else {
            assert_eq!(fix.facts["next"], "build", "{}", fix.out);
        }
        r.commit_on(
            "work/route",
            None,
            &[("src/core/app.txt", "fixed ok")],
            "Fix",
        );
        let built = r.report("route", "build", &report_block("gdd-build", "done"), &[]);
        assert_eq!(built.facts["next"], "gates", "{}", built.out);
        assert_eq!(
            r.gdd(&["limits", "route"]).facts["caps"]["rounds"]["used"],
            1.0
        );
        assert_eq!(r.gdd(&["gates", "route"]).code, 0);
        let review = r.gdd(&["launch", "route", "review"]);
        let prompt = review.facts["prompt"].as_str().unwrap();
        let log = r.log_lines("route");
        let fix = log.iter().find(|v| v["step"] == "review").unwrap();
        for f in fix["findings"].as_array().unwrap() {
            assert!(prompt.contains(f["what"].as_str().unwrap()), "{prompt}");
        }
        assert!(
            prompt.contains("First report how each previous finding ended"),
            "{prompt}"
        );
        assert!(!prompt.contains("Review only those tests"), "{prompt}");
    }
}

#[test]
fn mixed_findings_cost_a_round_and_get_a_full_review_after_a_seal_merge() {
    let r = Repo::new();
    gated_change(&r, "mixed", "");
    let findings = "f1 tests/mixed.sh:1 weak test | @id:mixed-1\nf2 src/core/app.txt:1 wrong value | @id:mixed-1";
    let fix = r.report(
        "mixed",
        "review",
        &review_text("fix", "yellow", findings),
        &[],
    );
    assert_eq!(fix.facts["next"], "checks", "{}", fix.out);

    // The builder merges the corrected seal and leaves the product finding open.
    correct_test(&r, "mixed", &[]);
    let build = r.report("mixed", "build", &build_report(""), &[]);
    assert_eq!(build.facts["next"], "gates", "{}", build.out);
    let log = r.log_lines("mixed");
    let step = log.last().unwrap();
    assert_eq!(step["step"], "build");
    assert!(step.get("seal_only").is_none(), "{step}");
    assert_eq!(
        r.gdd(&["limits", "mixed"]).facts["caps"]["rounds"]["used"],
        1.0
    );

    assert_eq!(r.gdd(&["gates", "mixed"]).facts["next"], "review");
    let review = r.gdd(&["launch", "mixed", "review"]);
    let prompt = review.facts["prompt"].as_str().unwrap();
    assert!(prompt.contains("wrong value"), "{prompt}");
    assert!(!prompt.contains("Review only those tests"), "{prompt}");
}

#[test]
fn a_seal_merge_at_the_round_limit_leaves_the_product_unchanged() {
    let r = Repo::with(&BLUEPRINT.replace("rounds_per_change: 3", "rounds_per_change: 1"));
    gated_change(&r, "budget", "");
    let fix = review_text(
        "fix",
        "yellow",
        "f1 src/core/app.txt:1 title is missing | @id:budget-1",
    );
    assert_eq!(
        r.report("budget", "review", &fix, &[]).facts["next"],
        "build"
    );
    r.commit_on(
        "work/budget",
        None,
        &[("src/core/app.txt", "ok title")],
        "Fix the title",
    );
    assert_eq!(
        r.report("budget", "build", &build_report(""), &[]).facts["next"],
        "gates"
    );
    assert_eq!(r.gdd(&["gates", "budget"]).facts["next"], "review");
    return_test(&r, "budget");
    correct_test(&r, "budget", &[]);
    assert!(
        r.gdd(&["launch", "budget", "build"])
            .has("Only merge the corrected seal")
    );
    assert!(report_test_merge(&r, "budget"));
    assert_eq!(
        r.gdd(&["limits", "budget"]).facts["caps"]["rounds"]["used"],
        1.0
    );
    assert!(!r.gdd(&["verify", "budget"]).has("cap-exceeded"));
}

#[test]
fn a_product_edit_at_the_round_limit_needs_a_waiver() {
    let r = Repo::with(&BLUEPRINT.replace("rounds_per_change: 3", "rounds_per_change: 1"));
    checked(&r, "delta");
    r.commit_on(
        "work/delta",
        Some("gdd-seal/delta"),
        &[("src/core/app.txt", "wrong")],
        "Build",
    );
    let built = report_block("gdd-build", "done");
    assert_eq!(r.report("delta", "build", &built, &[]).code, 0);
    assert_eq!(r.gdd(&["gates", "delta"]).code, 1);
    r.commit_on(
        "work/delta",
        None,
        &[("src/core/app.txt", "ok")],
        "Use the last build round",
    );
    assert_eq!(r.report("delta", "build", &built, &[]).code, 0);
    assert_eq!(r.gdd(&["gates", "delta"]).code, 0);
    let fix = review_text(
        "fix",
        "yellow",
        "f1 tests/delta.sh:1 misses a blank title | @id:delta-1",
    );
    assert_eq!(
        r.report("delta", "review", &fix, &[]).facts["next"],
        "checks"
    );
    r.commit_on(
        "gdd-seal/delta",
        None,
        &[("tests/delta.sh", "test -s src/core/app.txt")],
        "Correct the test",
    );
    assert_eq!(
        r.report("delta", "checks", &sealed_report(&["tests"]), &[])
            .facts["next"],
        "build"
    );
    let next = r.gdd(&["next", "delta"]);
    let launch = r.named(next.facts["state"]["next"]["command"].as_str().unwrap());
    assert_eq!(launch.code, 0, "{}", launch.out);
    assert!(
        launch.has("Only merge the corrected seal"),
        "{}",
        launch.out
    );
    merge_corrected_seal(&r, "delta");
    r.commit_on(
        "work/delta",
        None,
        &[("src/core/app.txt", "ok again")],
        "Edit the product too",
    );
    let refused = r.report("delta", "build", &built, &[]);
    assert_eq!(refused.facts["refused"], "cap-spent", "{}", refused.out);
    let next = r.gdd(&["next", "delta"]);
    let command = next.facts["state"]["next"]["command"].as_str().unwrap();
    assert!(refused.has(command) && command.contains("gdd ask delta --of waiver"));
    assert_eq!(
        r.gdd(&["launch", "delta", "build"]).facts["refused"],
        "cap-spent"
    );
    let before = r.log_lines("delta");
    // A product build accepted by an older engine is still readable and is flagged.
    let mut old = before.clone();
    old.push(
        before
            .iter()
            .rev()
            .find(|e| e["step"] == "build")
            .unwrap()
            .clone(),
    );
    write_change_log(&r, "delta", &old);
    assert!(r.gdd(&["verify", "delta"]).has("cap-exceeded"));
    write_change_log(&r, "delta", &before);
    assert_eq!(
        r.gdd(&["ask", "delta", "--of", "waiver", "One more try?"])
            .code,
        0
    );
    assert_eq!(
        r.gdd(&[
            "waiver",
            "delta",
            "--who",
            "Person",
            "--check",
            "rounds_per_change",
            "--why",
            "One more try",
            "--backlog",
            "delta-extra",
            "--limit",
            "rounds_per_change",
            "--to",
            "2"
        ])
        .code,
        0
    );
    assert_eq!(
        r.report("delta", "build", &built, &[]).facts["next"],
        "gates"
    );
    assert_eq!(
        r.gdd(&["limits", "delta"]).facts["caps"]["rounds"]["used"],
        2.0
    );
    let verified = r.gdd(&["verify", "delta"]);
    assert_eq!(verified.facts["errors"], 0, "{}", verified.out);
    assert!(!verified.has("cap-exceeded"), "{}", verified.out);
    assert_eq!(r.gdd(&["gates", "delta"]).facts["next"], "review");
    let launch = r.gdd(&["launch", "delta", "review"]);
    let prompt = launch.facts["prompt"].as_str().unwrap();
    assert!(prompt.contains("misses a blank title"), "{prompt}");
    assert!(!prompt.contains("Review only those tests"), "{prompt}");
    return_test(&r, "delta");
    let cannot = report_text(
        "gdd-checks",
        "stopped",
        "CANNOT: tests/delta.sh already covers blank titles.",
    );
    let declined = r.report("delta", "checks", &cannot, &[]);
    assert!(
        declined.facts["next_command"]
            .as_str()
            .unwrap()
            .contains("gdd ask delta --of waiver"),
        "{}",
        declined.out
    );
    assert_eq!(
        r.gdd(&["launch", "delta", "build"]).facts["refused"],
        "cap-spent"
    );
}

fn review_after_product_fix(r: &Repo) {
    gated_change(r, "closed", "");
    let findings = (1..=4)
        .map(|i| format!("f{i} issue {i} | @id:closed-1"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        r.report(
            "closed",
            "review",
            &review_text("fix", "yellow", &findings),
            &[]
        )
        .facts["next"],
        "build"
    );
    r.commit_on(
        "work/closed",
        None,
        &[("src/core/app.txt", "ok now")],
        "Fix the product",
    );
    assert_eq!(
        r.report("closed", "build", &build_report(""), &[]).facts["next"],
        "gates"
    );
    assert_eq!(r.gdd(&["gates", "closed"]).facts["next"], "review");
}

#[test]
fn an_unreadable_closed_list_preserves_the_fix_and_closes_nothing() {
    let r = Repo::new();
    review_after_product_fix(&r);
    let before = r.read("gdd/log/closed.jsonl");
    let fix = review_text("fix", "yellow", "f5 title is blank | @id:closed-1");
    for value in [
        "",
        "none",
        "-",
        "f1=fixed.",
        "`f1=fixed`",
        "f1=fixed; f2=false",
        "f1: fixed",
        "f1=fixed f9=minor",
        "f1=open",
        "f1=fixed junk",
        "f1=fixed, F1 = false",
        "f1=fixed,,f2=false",
        "<optional such as f1=fixed>",
    ] {
        r.write("gdd/log/closed.jsonl", &before);
        let text = fix.replace("caught: none", &format!("caught: none\nclosed: {value}"));
        let written = r.report("closed", "review", &text, &[]);
        assert_eq!(written.code, 0, "{value}: {}", written.out);
        let lines = r.log_lines("closed");
        let step = lines.last().unwrap();
        assert_eq!(step["verdict"], "fix");
        assert_eq!(step["findings"][0]["id"], "f5");
        assert_eq!(step["closed"], serde_json::json!([]));
        if value.is_empty() || value == "none" {
            assert!(!written.has("could not read closed:"));
            assert!(step.get("unparsed_closed").is_none());
        } else {
            assert!(
                written.has(&format!("could not read closed: {value}")),
                "{}",
                written.out
            );
            assert_eq!(step["unparsed_closed"], value);
        }
    }
    // Repeated fields and a manual override use the same all-or-nothing check.
    for (fields, options, raw) in [
        (
            "closed: f1=fixed\nclosed: f1=false",
            vec![],
            "f1=fixed,f1=false",
        ),
        (
            "closed: f1=fixed",
            vec!["--closed", "f1=fixed,junk"],
            "f1=fixed,junk",
        ),
    ] {
        r.write("gdd/log/closed.jsonl", &before);
        let text = fix.replace("caught: none", fields);
        let written = r.report("closed", "review", &text, &options);
        assert_eq!(written.code, 0, "{}", written.out);
        let lines = r.log_lines("closed");
        assert_eq!(lines.last().unwrap()["unparsed_closed"], raw);
        assert_eq!(lines.last().unwrap()["closed"], serde_json::json!([]));
    }
}

#[test]
fn the_review_block_closes_findings_in_manual_and_run_reports() {
    for automated in [false, true] {
        let r = Repo::new();
        review_after_product_fix(&r);
        let text = review_text("pass", "yellow", "closed: f5=minor").replace(
            "caught: none",
            "caught: none\nclosed: F1 = fixed, f2=false f3=intended f4=minor",
        );
        let pass = if automated {
            let cmd = r.fake_codex("new-review", &text, serde_json::Value::Null);
            r.gdd(&[
                "launch",
                "closed",
                "review",
                "--run",
                "--command",
                &format!("'{cmd}' exec --json -m <model>"),
            ])
        } else {
            r.report(
                "closed",
                "review",
                &r.gdd(&["launch", "closed", "review"]).with_reviewed(&text),
                &[],
            )
        };
        assert_eq!(pass.facts["next"], "ship", "{}", pass.out);
        assert_eq!(
            r.log_lines("closed").last().unwrap()["closed"],
            serde_json::json!([
                {"id":"f1","outcome":"fixed"}, {"id":"f2","outcome":"false"},
                {"id":"f3","outcome":"intended"}, {"id":"f4","outcome":"minor"}
            ])
        );
    }
}

#[test]
fn a_fix_lists_only_the_latest_review_findings() {
    let r = Repo::with(&BLUEPRINT.replace("rounds_per_change: 3", "rounds_per_change: 4"));
    checked(&r, "open");
    let start = r.git(&["rev-parse", "gdd-seal/open"]);
    for (from, text, session, findings) in [
        (
            Some(start.as_str()),
            "ok 1",
            "first-review",
            "f1 old label | request: show the total\nf2 wrong amount | @id:open-1",
        ),
        (
            None,
            "ok 2",
            "second-review",
            "f1 duplicate charge | zone pay: charge once",
        ),
    ] {
        r.commit_on("work/open", from, &[("src/core/app.txt", text)], text);
        assert_eq!(
            r.report("open", "build", &report_block("gdd-build", "done"), &[])
                .code,
            0
        );
        assert_eq!(r.gdd(&["gates", "open"]).code, 0);
        run_review(&r, "open", session, findings);
    }
    let fix = r.gdd(&["launch", "open", "build"]);
    assert_eq!(fix.code, 0, "{}", fix.out);
    let prompt = fix.facts["prompt"].as_str().unwrap();
    assert!(!prompt.contains("wrong amount"), "{prompt}");
    assert!(!prompt.contains("old label"), "{prompt}");
    assert_eq!(
        prompt
            .matches("duplicate charge | zone pay: charge once")
            .count(),
        1
    );
    // Now it is the gate that returns the build.
    r.commit_on(
        "work/open",
        None,
        &[("src/core/app.txt", "bad total")],
        "Fix the charge",
    );
    assert_eq!(
        r.report("open", "build", &report_block("gdd-build", "done"), &[])
            .code,
        0
    );
    assert_eq!(r.gdd(&["gates", "open"]).code, 1);
    let fix = r.gdd(&["launch", "open", "build"]);
    assert_eq!(fix.code, 0, "{}", fix.out);
    let prompt = fix.facts["prompt"].as_str().unwrap();
    assert!(prompt.contains("What failed at the gates"), "{prompt}");
    assert!(!prompt.contains("duplicate charge"), "{prompt}");
}

fn run_review(r: &Repo, id: &str, session: &str, findings: &str) {
    let codex = r.fake_codex(
        session,
        &review_text("fix", "yellow", findings),
        serde_json::Value::Null,
    );
    let review = r.gdd(&[
        "launch",
        id,
        "review",
        "--run",
        "--command",
        &format!("'{codex}' exec --json -m <model>"),
    ]);
    assert_eq!(review.code, 0, "{}", review.out);
}

#[test]
fn a_fix_preserves_the_reviewers_full_finding_and_notes() {
    let r = Repo::new();
    checked(&r, "words");
    r.commit_on(
        "work/words",
        Some("gdd-seal/words"),
        &[("src/core/app.txt", "ok 1")],
        "Build",
    );
    assert_eq!(
        r.report("words", "build", &report_block("gdd-build", "done"), &[])
            .code,
        0
    );
    assert_eq!(r.gdd(&["gates", "words"]).code, 0);
    let notes = (1..=8)
        .map(|i| format!("Step {i}: apply code SUMMER{i} to two cart lines and read the total."))
        .collect::<Vec<_>>()
        .join("\n");
    let finding = (1..=8)
        .map(|i| format!("with {i} units the total shows WORD{i} instead of the sum"))
        .collect::<Vec<_>>()
        .join("; ");
    run_review(
        &r,
        "words",
        "cart-review",
        &format!("f1 src/core/app.txt:1 {finding} | @id:words-1\nNOTES FOR THE BUILDER:\n{notes}"),
    );
    let fix = r.gdd(&["launch", "words", "build"]);
    let prompt = fix.facts["prompt"].as_str().unwrap();
    assert!(prompt.contains(&notes), "{prompt}");
    assert!(
        prompt.contains(&format!("{finding} | @id:words-1")),
        "{prompt}"
    );
    assert!(
        !prompt.contains("Don't repeat an unchanged attempt"),
        "{prompt}"
    );
}

/// A build that stops on a branch without <start> judged checks that are no longer the change's:
/// in one run its CANNOT blamed a test already fixed on the seal, the checks role had nothing to
/// seal, and the change could go nowhere. Its report is refused, and the build goes again.
#[test]
fn a_build_that_stopped_without_start_goes_again() {
    let r = Repo::new();
    checked(&r, "yv");
    let start = r.git(&["rev-parse", "gdd-seal/yv"]);
    r.commit_on(
        "work/yv",
        Some(&start),
        &[("src/core/app.txt", "a\n")],
        "Round 1",
    );
    let says = report_text(
        "gdd-build",
        "stopped",
        "CANNOT: the sealed test looks wrong",
    );
    assert_eq!(r.report("yv", "build", &says, &[]).code, 0);
    // The checks role fixes the test, and <start> moves to the seal's new tip.
    r.commit_on(
        "gdd-seal/yv",
        None,
        &[("tests/yv.sh", "true # fixed\n")],
        "Fix the test",
    );
    let c = r.report("yv", "checks", &sealed_report(&["tests"]), &[]);
    assert_eq!(c.code, 0, "{}", c.out);
    // The next round stops on its branch without merging that tip.
    r.commit_on("work/yv", None, &[("src/core/app.txt", "b\n")], "Round 2");
    let b = r.report("yv", "build", &says, &[]);
    assert_eq!(b.facts["refused"], "start-missing", "{}", b.out);
    assert!(b.has("gdd launch yv build"), "{}", b.out);
    assert_eq!(
        r.gdd(&["next", "yv"]).facts["state"]["next"]["step"],
        "build"
    );
    // Once it holds <start>, its CANNOT counts.
    let tip = r.git(&["rev-parse", "gdd-seal/yv"]);
    let tree = r.git(&["merge-tree", "--write-tree", "work/yv", &tip]);
    let head = r.git(&["rev-parse", "work/yv"]);
    let merge = r.git(&[
        "commit-tree",
        &tree,
        "-p",
        &head,
        "-p",
        &tip,
        "-m",
        "Merge <start>",
    ]);
    r.git(&["update-ref", "refs/heads/work/yv", &merge, &head]);
    assert_eq!(r.report("yv", "build", &says, &[]).code, 0);
}

/// A stop of the checks role names what stops it, a scenario by its @id or a file: in one run,
/// five rounds stopped on "the eleven scenarios" and wrote no test.
#[test]
fn a_checks_stop_that_names_nothing_is_refused() {
    let r = Repo::new();
    yellow_brief(&r, "yx", "src/core/app.txt");
    approved(&r, "yx");
    for (attempt, vague) in [
        "CANNOT: I could not prove the checks for all eleven scenarios",
        "CANNOT: gdd-seal/yx stays at 4575d37, version 0.14.0",
    ]
    .iter()
    .enumerate()
    {
        let v = r.report(
            "yx",
            "checks",
            &report_text("gdd-checks", "stopped", vague),
            &[],
        );
        assert_eq!(v.facts["refused"], "vague-stop", "{vague}: {}", v.out);
        if attempt == 0 {
            assert_eq!(
                r.gdd(&["next", "yx"]).facts["state"]["next"]["step"],
                "checks"
            );
            assert_eq!(r.gdd(&["launch", "yx", "checks"]).code, 0);
        }
    }
    let n = r.gdd(&["next", "yx"]);
    let next = &n.facts["state"]["next"];
    assert_eq!(next["blocked"], "needs-person", "{}", n.out);
    assert!(n.has("@id") && n.has("file"), "{}", n.out);
    let cmd = next["command"].as_str().unwrap();
    let question = cmd
        .strip_prefix("gdd ask yx --of decision \"")
        .unwrap()
        .strip_suffix('"')
        .unwrap();
    assert!(question.ends_with('?'));
    let project = r.gdd(&["next"]);
    assert_eq!(project.facts["under_way"][0]["command"], cmd);
    assert_eq!(r.gdd(&["ask", "yx", "--of", "decision", question]).code, 0);
    assert_eq!(
        r.gdd(&["next", "yx"]).facts["state"]["next"]["step"],
        "wait"
    );
    assert_eq!(
        r.gdd(&["answer", "yx", "Use the empty basket example"])
            .code,
        0
    );
    assert_eq!(
        r.gdd(&["next", "yx"]).facts["state"]["next"]["step"],
        "checks"
    );
    for named in [
        "CANNOT: `yx-1` contradicts a sealed check",
        "CANNOT: tools/fakepay.mjs isn't in git",
    ] {
        let c = r.report(
            "yx",
            "checks",
            &report_text("gdd-checks", "stopped", named),
            &[],
        );
        assert_eq!(c.code, 0, "{named}: {}", c.out);
    }
}

/// A role on another model than its level's isn't the role the blueprint asks for: in one run,
/// two rounds asked at the deep level ran on the orchestrator's own model.
#[test]
fn a_role_on_another_model_than_its_levels_is_refused() {
    let r = Repo::new();
    checked(&r, "yy");
    let start = r.git(&["rev-parse", "gdd-seal/yy"]);
    r.commit_on(
        "work/yy",
        Some(&start),
        &[("src/core/app.txt", "built\n")],
        "Build",
    );
    let done = report_block("gdd-build", "done");
    let w = r.report("yy", "build", &done, &["--model", "model-x"]);
    assert_eq!(w.facts["refused"], "wrong-model", "{}", w.out);
    assert!(w.has("model-b"), "{}", w.out);
    assert_eq!(
        r.report("yy", "build", &done, &["--model", "model-b"]).code,
        0
    );
}

#[test]
fn removing_a_scenario_file_changes_what_the_person_approved() {
    let r = Repo::new();
    let id = "cart";
    yellow_brief(&r, id, "src/core/app.txt");
    let brief = "gdd/briefs/cart.md";
    let cart = "features/cart.feature";
    let empty = "features/empty.feature";
    r.write(empty, "Feature: An empty cart shows a zero total\n");
    assert_eq!(r.gdd(&["seal", id, empty]).code, 0);
    let both = [
        "brief",
        id,
        brief,
        cart,
        empty,
        "--paths",
        "src/core/app.txt",
    ];
    assert_eq!(r.gdd(&both).code, 0);
    assert_eq!(
        r.gdd(&[
            "approve",
            id,
            "--of",
            "brief",
            "--who",
            "Owner",
            "--where",
            "conversation",
            cart,
            empty
        ])
        .code,
        0
    );
    let fewer = r.gdd(&["brief", id, brief, cart, "--paths", "src/core/app.txt"]);
    assert_eq!(fewer.facts["next"], "ask", "{}", fewer.out);
    // Restoring exactly what the person saw keeps their approval.
    assert_eq!(r.gdd(&both).facts["next"], "checks");
}

#[test]
fn a_large_change_gets_its_review_and_a_size_note_without_a_waiver() {
    let r = Repo::new();
    let id = "g1";
    assert_eq!(r.gdd(&["brief", id, "--paths", "src/ui/page.txt"]).code, 0);
    let base = r.git(&["rev-parse", "main"]);
    r.commit_on(
        "work/g1",
        Some(&base),
        &[("src/ui/new.txt", &"line\n".repeat(540))],
        "Add the pages",
    );
    assert_eq!(
        r.report(id, "build", &report_block("gdd-build", "done"), &[])
            .code,
        0
    );
    let limits = r.gdd(&["limits", id]);
    assert_eq!(limits.code, 0, "{}", limits.out);
    assert_eq!(limits.facts["lines"]["used"], 540);
    assert_eq!(limits.facts["lines"]["past"], true);
    assert!(limits.facts["lines"].get("raised").is_none());
    assert_eq!(limits.facts["stop"].as_array().unwrap().len(), 0);
    assert_eq!(r.gdd(&["gates", id]).code, 0);
    let review = r.gdd(&["launch", id, "review"]);
    let note = "540 lines, above the expected 400";
    assert!(review.facts["prompt"].as_str().unwrap().contains(note));
    assert_eq!(
        r.report(
            id,
            "review",
            &review.with_reviewed(&review_text("pass", "green", "")),
            &[]
        )
        .code,
        0
    );
    assert!(
        r.gdd(&["ship", id, "--describe"]).facts["description"]
            .as_str()
            .unwrap()
            .contains(note)
    );
    let w = r.gdd(&[
        "waiver",
        id,
        "--who",
        "Owner",
        "--check",
        "lines_per_change",
        "--why",
        "more lines",
        "--backlog",
        "pages",
        "--limit",
        "lines_per_change",
        "--to",
        "600",
    ]);
    assert_ne!(w.code, 0);
}

#[test]
fn the_answer_about_the_deep_level_lasts_until_the_project_changes_levels() {
    let r = Repo::new();
    let question = [
        "ask",
        "blueprint",
        "--of",
        "levels",
        "May I plan at the available setting for this project?",
    ];
    assert_eq!(r.gdd(&question).code, 0);
    let repeated = r.gdd(&question);
    assert_eq!(
        repeated.facts["refused"], "needs-person",
        "{}",
        repeated.out
    );
    assert!(repeated.has("the levels question is already waiting"));
    assert_eq!(r.log_lines("blueprint").len(), 1);
    assert_eq!(r.gdd(&["answer", "blueprint", "Yes, go ahead."]).code, 0);
    r.write(
        "gdd/blueprint.md",
        &BLUEPRINT.replace("lines_per_change: 400", "lines_per_change: 300"),
    );
    assert_eq!(r.gdd(&["next"]).facts["levels_answer"], "Yes, go ahead.");
    assert_eq!(r.gdd(&question).facts["refused"], "already-answered");
    assert_eq!(r.log_lines("blueprint").len(), 2);
    r.write(
        "gdd/blueprint.md",
        &BLUEPRINT.replace("deep: model-c", "deep: model-d"),
    );
    assert!(r.gdd(&["next"]).facts["levels_answer"].is_null());
    assert_eq!(r.gdd(&question).code, 0);
    assert_eq!(
        r.gdd(&["answer", "blueprint", "Wait for the deep setting."])
            .code,
        0
    );
    assert_eq!(
        r.gdd(&["next"]).facts["levels_answer"],
        "Wait for the deep setting."
    );
}

#[test]
fn an_open_blueprint_question_hands_off_to_its_answer() {
    let r = Repo::new();
    let asked = r.gdd(&[
        "ask",
        "blueprint",
        "--of",
        "levels",
        "Use the available level?",
    ]);
    same_as_next(&r, "blueprint", &asked);
    assert_eq!(asked.facts["next"], "wait");
    assert_eq!(asked.facts["next_command"], asked.facts["then"]);
    let answer = r.gdd(&["answer", "blueprint", "Yes"]);
    assert_eq!(answer.code, 0, "{}", answer.out);
    assert!(answer.facts.get("next").is_none());
}

#[test]
fn a_proposed_blueprint_never_hands_off_as_done() {
    let r = Repo::new();
    let proposed = r.gdd(&[
        "step",
        "blueprint",
        "prepare",
        "--result",
        "proposed",
        "--file",
        "gdd/blueprint.md",
    ]);
    assert_eq!(proposed.code, 0, "{}", proposed.out);
    assert!(r.gdd(&["next"]).has("no approval of the blueprint"));
    assert_ne!(proposed.facts["next"], "done", "{}", proposed.out);
}

#[test]
fn unreadable_failure_counts_leave_the_build_and_its_claims_intact() {
    use serde_json::json;
    let r = Repo::new();
    assert_eq!(
        r.gdd(&["brief", "g1", "--paths", "src/ui/page.txt"]).code,
        0
    );
    r.commit_on(
        "work/g1",
        Some("main"),
        &[("src/ui/page.txt", "built\n")],
        "Build",
    );
    let counts = [
        json!(null),
        json!("two"),
        json!(-1),
        json!(1.5),
        json!(0),
        json!(2),
    ];
    let claims: Vec<_> = counts
        .iter()
        .map(|n| json!({"check":"tests","failures":n, "command":"test", "exit":0, "tail":"ok"}))
        .collect();
    let text = report_text(
        "gdd-build",
        "done",
        &format!("```json\n{}\n```", json!(claims)),
    );
    let build = r.report("g1", "build", &text, &[]);
    assert_eq!(build.facts["next"], "gates", "{}", build.out);
    let lines = r.log_lines("g1");
    let saved = lines.last().unwrap()["claims"].as_array().unwrap();
    assert_eq!(saved.len(), counts.len());
    for (claim, count) in saved.iter().zip(counts) {
        assert_eq!(claim["failures"], json!(count.as_u64()));
        assert_eq!(claim["tail"], "ok");
    }
}
