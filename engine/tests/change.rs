//! Whole changes driven only through the tool, in throwaway repositories whose checks are trivial
//! commands, and the refusals on the way.

mod common;
use common::*;

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
    // A closed log is closed: a correction can still say what was wrong, and voids nothing.
    let line = r.log_lines("g1").len().to_string();
    let void = r.gdd(&["correct", "g1", "--line", &line, "--void", "reopen it"]);
    assert_eq!(void.facts["refused"], "out-of-order", "{}", void.out);
    let note = r.gdd(&["correct", "g1", "--line", &line, "a typo in its place"]);
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
            "f1 src/core/app.txt:1 it says more than ok | @id:y1-1\nNOTES FOR THE BUILDER:\nSay only ok.\n",
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
    r.report(id, "checks", &sealed_report(&["tests"]), &[])
}

/// With no checks sealed yet, the role's word that nothing had to change is still nothing sealed.
#[test]
fn a_first_checks_round_that_seals_nothing_is_refused() {
    let r = Repo::new();
    yellow_brief(&r, "y6", "src/core/app.txt");
    approved(&r, "y6");
    assert_eq!(sealed_nothing(&r, "y6").facts["refused"], "not-sealed");
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
    let checks = &lines.last().unwrap()["seal"]["checks"];
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
    let r = Repo::new();
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
    r.report("y7", "build", &report_block("gdd-build", "done"), &[]);
    assert_eq!(r.gdd(&["gates", "y7"]).code, 0);
    let found = "f1 tests/y7.sh:1 the test passes whatever the app says | @id:y7-1\n";
    let rv = r.report("y7", "review", &review_text("fix", "yellow", found), &[]);
    assert_eq!(rv.code, 0, "{}", rv.out);
    // The checks role is told what the reviewer found: a better check is its to write.
    let c = r.gdd(&["launch", "y7", "checks"]);
    let prompt = c.facts["prompt"].as_str().unwrap();
    assert!(prompt.contains("The reviewer said fix"), "{prompt}");
    assert!(prompt.contains("f1 tests/y7.sh:1"), "{prompt}");
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
    assert!(
        prompt.contains("sealed again after that review"),
        "{prompt}"
    );
    assert!(prompt.contains("merge <start> into it"), "{prompt}");
    assert!(!prompt.contains("which starts at <start>"), "{prompt}");
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
    for vague in [
        "CANNOT: I could not prove the checks for all eleven scenarios",
        "CANNOT: gdd-seal/yx stays at 4575d37, version 0.13.1",
    ] {
        let v = r.report(
            "yx",
            "checks",
            &report_text("gdd-checks", "stopped", vague),
            &[],
        );
        assert_eq!(v.facts["refused"], "vague-stop", "{vague}: {}", v.out);
    }
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
    assert_eq!(limits.facts["stop"].as_array().unwrap().len(), 0);
    assert_eq!(r.gdd(&["gates", id]).code, 0);
    let review = r.gdd(&["launch", id, "review"]);
    let note = "540 lines, above the expected 400";
    assert!(review.facts["prompt"].as_str().unwrap().contains(note));
    assert_eq!(
        r.report(id, "review", &review_text("pass", "green", ""), &[])
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
