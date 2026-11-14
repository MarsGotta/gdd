//! Whole changes driven only through the tool, in throwaway repositories whose checks are trivial
//! commands, and the refusals on the way.

mod common;
use common::*;

fn review_text(verdict: &str, color: &str, extra: &str) -> String {
    format!(
        "{verdict}\nCOLOR: {color}\nI read the change.\n{extra}\n{}",
        report_block("gdd-review", verdict)
    )
}

#[test]
fn a_green_change_from_brief_to_ship() {
    let r = Repo::new();
    let n = r.gdd(&["next", "g1"]);
    assert_eq!(n.facts["state"]["next"]["step"], "brief", "{}", n.out);
    let b = r.gdd(&["brief", "g1", "--paths", "src/ui/page.txt"]);
    assert_eq!(b.code, 0, "{}", b.out);
    assert_eq!(b.facts["color"], "green");
    assert_eq!(b.facts["next"], "build", "{}", b.out);
    // The launch prints the command and the prompt: the blueprint's launch is prose here.
    let l = r.gdd(&["launch", "g1", "build"]);
    assert_eq!(l.code, 0, "{}", l.out);
    assert_eq!(l.facts["model"], "model-a");
    assert!(
        l.facts["prompt"].as_str().unwrap().contains("/gdd-build")
            || l.facts["prompt"].as_str().unwrap().contains("gdd-build")
    );
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
    assert_eq!(rep.facts["next"], "gates");
    let g = r.gdd(&["gates", "g1"]);
    assert_eq!(g.code, 0, "{}", g.out);
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
    let rv = r.report("g1", "review", &review_text("pass", "green", ""), &[]);
    assert_eq!(rv.code, 0, "{}", rv.out);
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
    let v = r.gdd(&["verify", "g1"]);
    assert_eq!(v.facts["errors"], 0, "{}", v.out);
    let n = r.gdd(&["next", "g1"]);
    assert_eq!(n.facts["state"]["next"]["step"], "done");
    // Every invocation went to the change's journal.
    let journal = r.read("gdd/log/g1.engine.jsonl");
    assert!(journal.lines().count() >= 10);
    assert!(journal.contains("\"refused\":\"not-merged\""));
}

fn yellow_brief(r: &Repo, id: &str, path: &str) {
    r.write(&format!("gdd/briefs/{id}.md"), &format!("---\ncontract: gdd/1\nid: {id}\nversion: 1\ncolor: yellow\nzones: [core]\ntouches:\n  - '{path}'\nscenarios: [{id}-1]\nbacklog: {id}\n---\n\n# A yellow change\n\n## Request\n\n\"Make it ok.\"\n\n## What changes\n\nThe app says ok.\n\n## What it won't do yet\n\nNothing else.\n\n## Undoing it\n\nRevert the merge.\n"));
    r.write(&format!("features/{id}.feature"), &format!("Feature: Ok\n\n  @brief:{id} @id:{id}-1\n  Scenario: It says ok\n    Given the app\n    Then it says ok\n"));
    let s = r.gdd(&[
        "seal",
        id,
        &format!("gdd/briefs/{id}.md"),
        &format!("features/{id}.feature"),
    ]);
    assert_eq!(s.code, 0, "{}", s.out);
    let b = r.gdd(&[
        "brief",
        id,
        &format!("gdd/briefs/{id}.md"),
        &format!("features/{id}.feature"),
        "--paths",
        path,
    ]);
    assert_eq!(b.code, 0, "{}", b.out);
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
    assert_eq!(ap.code, 0, "{}", ap.out);
    assert_eq!(
        r.gdd(&["next", "y1"]).facts["state"]["next"]["step"],
        "checks"
    );
    // The checks role seals its test on top of the brief.
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
        &format!("Sealed.\n{}", report_block("gdd-checks", "sealed")),
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
    let b2 = r.report("y1", "build", &report_block("gdd-build", "done"), &[]);
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
            "f1 src/core/app.txt:1 it says more than ok\nNOTES FOR THE BUILDER:\nSay only ok.\n",
        ),
        &[],
    );
    assert_eq!(rv.code, 0, "{}", rv.out);
    let l = r.gdd(&["launch", "y1", "build"]);
    assert!(
        l.facts["prompt"].as_str().unwrap().contains("Say only ok."),
        "{}",
        l.out
    );
    r.commit_on("work/y1", None, &[("src/core/app.txt", "ok\n")], "Round 3");
    r.report("y1", "build", &report_block("gdd-build", "done"), &[]);
    assert_eq!(r.gdd(&["gates", "y1"]).code, 0);
    let rv = r.report(
        "y1",
        "review",
        &review_text("pass", "yellow", ""),
        &["--closed", "f1=fixed"],
    );
    assert_eq!(rv.facts["next"], "ship", "{}", rv.out);
    std::fs::remove_file(r.path("gdd/briefs/y1.md")).unwrap();
    std::fs::remove_file(r.path("features/y1.feature")).unwrap();
    r.git(&["merge", "-q", "--no-ff", "-m", "Merge y1", "work/y1"]);
    let s = r.gdd(&["ship", "y1", "--done", "main"]);
    assert_eq!(s.code, 0, "{}", s.out);
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
    r.gdd(&["ask", "r1", "--of", "brief", "Approve?"]);
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
    r.report("r1", "checks", &report_block("gdd-checks", "sealed"), &[]);
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
    r.report("y2", "checks", &report_block("gdd-checks", "sealed"), &[]);
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
    // A waiver of the cap gives it room, and the counter sees it.
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
    r.report("y4", "checks", &report_block("gdd-checks", "sealed"), &[]);
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
    let sealed = format!("Sealed.\n{}", report_block("gdd-checks", "sealed"));
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
