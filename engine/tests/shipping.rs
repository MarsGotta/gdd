//! Shipping with the bookkeeping, the main branch merged in, readings, setup logs, the
//! orchestrator's own steps, and what the journal tells verify and the metrics.

mod common;
use common::*;
use serde_json::Value;

fn review_text(verdict: &str, color: &str, extra: &str) -> String {
    format!(
        "{verdict}\nCOLOR: {color}\nI read the change.\n{extra}\n{}",
        report_block("gdd-review", verdict)
    )
}

/// A project whose logs are tracked, as a real one's are, not ignored as in the other tests.
fn tracked_logs_repo() -> Repo {
    let r = Repo::new();
    r.write(".gitignore", "");
    r.commit("Track the logs");
    r
}

fn yellow_brief(r: &Repo, id: &str) {
    r.write(
        &format!("gdd/briefs/{id}.md"),
        &format!("---\ncontract: gdd/1\nid: {id}\nversion: 1\nscenarios: [{id}-1]\nbacklog: y1\n---\n\n# A yellow change\n\n## What changes\n\nThe app says ok.\n\n## What it won't do yet\n\nNothing.\n\n## Undoing it\n\nRevert the merge.\n"),
    );
    r.write(
        &format!("features/{id}.feature"),
        &format!("Feature: Ok\n\n  @brief:{id} @id:{id}-1\n  Scenario: It says ok\n"),
    );
    let files = [
        format!("gdd/briefs/{id}.md"),
        format!("features/{id}.feature"),
    ];
    assert_eq!(r.gdd(&["seal", id, &files[0], &files[1]]).code, 0);
    let b = r.gdd(&[
        "brief",
        id,
        &files[0],
        &files[1],
        "--paths",
        "src/core/app.txt",
    ]);
    assert_eq!(b.code, 0, "{}", b.out);
}

/// A yellow change approved, sealed with its test and its proofs, built and through its gates.
fn yellow_through_gates(r: &Repo, id: &str) -> String {
    yellow_brief(r, id);
    // Checks before the approval are refused, launched or reported.
    let early = r.gdd(&["launch", id, "checks"]);
    assert_eq!(early.facts["refused"], "needs-approval", "{}", early.out);
    let early = r.report(id, "checks", &report_block("gdd-checks", "sealed"), &[]);
    assert_eq!(early.facts["refused"], "needs-approval", "{}", early.out);
    let a = r.gdd(&[
        "approve",
        id,
        "--of",
        "brief",
        "--who",
        "Person",
        "--where",
        "conversation",
        &format!("features/{id}.feature"),
    ]);
    assert_eq!(a.code, 0, "{}", a.out);
    r.commit_on(
        &format!("gdd-seal/{id}"),
        None,
        &[(&format!("tests/{id}.sh"), "true\n")],
        "Seal its test",
    );
    let proofs = "```json\n[{\"check\":\"tests\",\"case\":\"rejects\",\"command\":\"sh tests/run.sh\",\"exit\":1,\"tail\":\"FAILED\"}]\n```\nseen rejecting, not seen passing\n";
    let c = r.report(
        id,
        "checks",
        &format!("Sealed.\n{proofs}{}", report_block("gdd-checks", "sealed")),
        &[],
    );
    assert_eq!(c.code, 0, "{}", c.out);
    let start = r.git(&["rev-parse", &format!("gdd-seal/{id}")]);
    r.commit_on(
        &format!("work/{id}"),
        Some(&start),
        &[("src/core/app.txt", "ok, built\n")],
        "Build",
    );
    assert_eq!(
        r.report(id, "build", &report_block("gdd-build", "done"), &[])
            .code,
        0
    );
    let g = r.gdd(&["gates", id]);
    assert_eq!(g.code, 0, "{}", g.out);
    start
}

#[test]
fn the_checks_proofs_come_from_their_json_block() {
    let r = Repo::new();
    yellow_through_gates(&r, "p1");
    let checks = r
        .log_lines("p1")
        .into_iter()
        .find(|e| e["step"] == "checks")
        .unwrap();
    assert_eq!(checks["seal"]["proofs"][0]["case"], "rejects");
    assert_eq!(checks["seal"]["proofs"][0]["exit"], 1);
    // A json block that isn't a list of proofs is refused, with the override named.
    yellow_brief(&r, "p2");
    r.gdd(&[
        "approve",
        "p2",
        "--of",
        "brief",
        "--who",
        "Person",
        "--where",
        "conversation",
        "features/p2.feature",
    ]);
    r.commit_on("gdd-seal/p2", None, &[("tests/p2.sh", "true\n")], "Seal");
    let bad = r.report(
        "p2",
        "checks",
        &format!(
            "```json\n{{\"not\": \"a list\"}}\n```\n{}",
            report_block("gdd-checks", "sealed")
        ),
        &[],
    );
    assert_eq!(bad.facts["refused"], "bad-proofs", "{}", bad.out);
    assert!(bad.has("--proofs"));
}

#[test]
fn bookkeeping_and_a_local_merge() {
    let r = tracked_logs_repo();
    let id = "y1";
    yellow_through_gates(&r, id);
    assert_eq!(
        r.report(
            id,
            "review",
            &review_text("pass", "yellow", "LATER:\n- the text could be shorter\n"),
            &[]
        )
        .code,
        0
    );
    let n = r.gdd(&["next", id]);
    assert_eq!(n.facts["state"]["next"]["step"], "ship", "{}", n.out);
    let cmd = n.facts["state"]["next"]["command"].as_str().unwrap();
    assert!(
        cmd.starts_with(&format!("gdd ship {id} --merge")),
        "no remote: the merge first: {cmd}"
    );
    // The description lists what was left for later, without a "Nothing." before it.
    let d = r.gdd(&["ship", id, "--describe"]);
    assert!(d.has("- the text could be shorter"), "{}", d.out);
    assert!(!d.has("Nothing."), "{}", d.out);
    // The bookkeeping goes on top of the branch without touching the checkout or its index.
    let status = r.git(&["status", "--porcelain"]);
    let index = r.git(&["ls-files", "--stage"]);
    let tip = r.git(&["rev-parse", "work/y1"]);
    let b = r.gdd(&["ship", id, "--bookkeeping"]);
    assert_eq!(b.code, 0, "{}", b.out);
    assert_eq!(r.git(&["ls-files", "--stage"]), index);
    assert_eq!(
        r.git(&["status", "--porcelain"])
            .lines()
            .filter(|l| !l.contains(".engine.jsonl"))
            .collect::<Vec<_>>(),
        status
            .lines()
            .filter(|l| !l.contains(".engine.jsonl"))
            .collect::<Vec<_>>()
    );
    let after = r.git(&["rev-parse", "work/y1"]);
    assert_ne!(after, tip);
    assert_eq!(r.git(&["rev-parse", "work/y1^"]), tip);
    let names = r.git(&["diff", "--name-only", &tip, &after]);
    assert!(
        names
            .lines()
            .all(|p| p.starts_with("gdd/log/") || p == "gdd/backlog.md"),
        "{names}"
    );
    // Bookkeeping doesn't un-judge the change: it is still ready to ship, without --head.
    let n = r.gdd(&["next", id]);
    assert_eq!(n.facts["state"]["next"]["step"], "ship", "{}", n.out);
    assert_eq!(r.gdd(&["ship", id, "--describe"]).code, 0);
    // Not on the main branch: refused.
    r.git(&["checkout", "-q", "-b", "elsewhere"]);
    let off = r.gdd(&["ship", id, "--merge"]);
    assert_eq!(off.facts["refused"], "not-on-main", "{}", off.out);
    r.git(&["checkout", "-q", "main"]);
    // Something else pending: refused.
    r.write("src/ui/page.txt", "an edit nobody shipped\n");
    let dirty = r.gdd(&["ship", id, "--merge"]);
    assert_eq!(dirty.facts["refused"], "dirty-tree", "{}", dirty.out);
    r.git(&["checkout", "--", "src/ui/page.txt"]);
    // The merge: the copies of the brief and the logs go aside, the merge brings them back.
    let m = r.gdd(&["ship", id, "--merge"]);
    assert_eq!(m.code, 0, "{}", m.out);
    // The merge, then one commit of the ship's bookkeeping on top of it.
    let main_tip = r.git(&["rev-parse", "main^"]);
    assert_eq!(m.facts["merge"], main_tip.as_str());
    assert_eq!(m.facts["logged"], r.git(&["rev-parse", "main"]).as_str());
    assert_eq!(
        r.git(&["log", "-1", "--format=%s", "main"]),
        "Log the ship of y1"
    );
    let logged = r.git(&["diff", "--name-only", "main^", "main"]);
    assert!(
        logged
            .lines()
            .all(|p| p.starts_with("gdd/log/") || p == "gdd/backlog.md"),
        "{logged}"
    );
    assert!(git_ok(
        &r.root,
        &["merge-base", "--is-ancestor", "work/y1", "main"]
    ));
    assert_eq!(
        r.git(&["rev-list", "--count", "--merges", "-1", "main^"]),
        "1"
    );
    assert!(
        r.path("gdd/briefs/y1.md").is_file(),
        "the merge brought the brief"
    );
    let ship = r
        .log_lines(id)
        .into_iter()
        .find(|e| e["step"] == "ship")
        .unwrap();
    assert_eq!(ship["head"], r.git(&["rev-parse", "work/y1"]).as_str());
    assert_eq!(ship["where"], main_tip.as_str());
    assert!(
        r.read("gdd/backlog.md")
            .contains("| y1 | A yellow change | yellow | 1 | | done |")
    );
    // Nothing of the layer's is left in the checkout.
    assert_eq!(r.git(&["status", "--porcelain"]), "");
    let v = r.gdd(&["verify", "--all"]);
    assert_eq!(v.facts["errors"], 0, "{}", v.out);
    assert!(!v.has("(start)") && !v.has("(same-second)"), "{}", v.out);
}

#[test]
fn a_merge_of_main_that_touches_none_of_its_files_asks_for_the_gates_not_a_review() {
    let r = Repo::new();
    let id = "m1";
    yellow_through_gates(&r, id);
    assert_eq!(
        r.report(id, "review", &review_text("pass", "yellow", ""), &[])
            .code,
        0
    );
    let seal_before = r.git(&["rev-parse", "gdd-seal/m1"]);
    // Main moves with a file the change doesn't touch.
    r.write("src/core/other.txt", "another change shipped\n");
    r.commit("Another change");
    let s = r.gdd(&["seal", id, "--merge-main"]);
    assert_eq!(s.code, 0, "{}", s.out);
    let tip = r.git(&["rev-parse", "gdd-seal/m1"]);
    let moved = r
        .log_lines(id)
        .into_iter()
        .rev()
        .find(|e| e["event"] == "seal")
        .unwrap();
    assert_eq!(moved["ref"], "gdd-seal/m1");
    assert_eq!(moved["from"], seal_before.as_str());
    assert_eq!(moved["to"], tip.as_str());
    assert_eq!(moved["why"], "merged main");
    // The orchestrator merges the seal into the change's branch.
    let wt = r.dir.path().join("wt-merge");
    let w = wt.to_string_lossy().to_string();
    git(&r.root, &["worktree", "add", "-q", &w, "work/m1"]);
    git(
        &wt,
        &[
            "-c",
            "user.name=T",
            "-c",
            "user.email=t@example.com",
            "merge",
            "-q",
            "--no-edit",
            "gdd-seal/m1",
        ],
    );
    git(&r.root, &["worktree", "remove", "--force", &w]);
    assert_eq!(r.gdd(&["next", id]).facts["state"]["next"]["step"], "gates");
    let g = r.gdd(&["gates", id]);
    assert_eq!(g.code, 0, "{}", g.out);
    assert_eq!(g.facts["start"], tip.as_str());
    let n = r.gdd(&["next", id]);
    assert_eq!(
        n.facts["state"]["next"]["step"], "ship",
        "the review still holds: {}",
        n.out
    );
    let v = r.gdd(&["verify", id]);
    assert_eq!(v.facts["errors"], 0, "{}", v.out);
    assert!(
        !v.has("(start)"),
        "the layer's own seal move is logged: {}",
        v.out
    );
}

#[test]
fn a_merge_of_main_that_touches_its_files_asks_for_a_review_again() {
    let r = Repo::new();
    r.write("src/ui/three.txt", "one\ntwo\nthree\n");
    r.commit("Three lines");
    r.gdd(&["brief", "m2", "--paths", "src/ui/three.txt"]);
    let base = r.git(&["rev-parse", "main"]);
    r.commit_on(
        "work/m2",
        Some(&base),
        &[("src/ui/three.txt", "one\ntwo\nTHREE\n")],
        "Build",
    );
    r.report("m2", "build", &report_block("gdd-build", "done"), &[]);
    assert_eq!(r.gdd(&["gates", "m2"]).code, 0);
    r.report("m2", "review", &review_text("pass", "green", ""), &[]);
    // Main changes the first line of the same file: it merges cleanly, and touches the change's file.
    r.write("src/ui/three.txt", "ONE\ntwo\nthree\n");
    r.commit("Another change, same file");
    let wt = r.dir.path().join("wt-m2");
    let w = wt.to_string_lossy().to_string();
    git(&r.root, &["worktree", "add", "-q", &w, "work/m2"]);
    git(
        &wt,
        &[
            "-c",
            "user.name=T",
            "-c",
            "user.email=t@example.com",
            "merge",
            "-q",
            "--no-edit",
            "main",
        ],
    );
    git(&r.root, &["worktree", "remove", "--force", &w]);
    assert_eq!(r.gdd(&["gates", "m2"]).code, 0);
    let n = r.gdd(&["next", "m2"]);
    assert_eq!(n.facts["state"]["next"]["step"], "review", "{}", n.out);
}

#[test]
fn a_red_brief_is_read_before_the_person_is_asked() {
    let r = Repo::new();
    r.write("gdd/briefs/r1.md", "---\nversion: 1\n---\n# Pay\n");
    r.write("features/r1.feature", "Feature: Pay\n");
    r.gdd(&["seal", "r1", "gdd/briefs/r1.md", "features/r1.feature"]);
    r.gdd(&[
        "brief",
        "r1",
        "gdd/briefs/r1.md",
        "features/r1.feature",
        "--paths",
        "src/pay/pay.txt",
    ]);
    let n = r.gdd(&["next", "r1"]);
    assert_eq!(n.facts["state"]["next"]["step"], "reading", "{}", n.out);
    assert_eq!(n.facts["state"]["next"]["command"], "gdd launch r1 reading");
    let l = r.gdd(&["launch", "r1", "reading"]);
    assert!(
        l.facts["prompt"]
            .as_str()
            .unwrap()
            .contains("Load the `gdd-review` skill (`gdd:gdd-review` when it comes from a plugin)"),
        "{}",
        l.out
    );
    let rd = r.report(
        "r1",
        "reading",
        &format!(
            "pass\nf1 gdd/briefs/r1.md: what if it is paid twice?\n{}",
            report_block("gdd-review", "pass")
        ),
        &["--went", "f1=brief"],
    );
    assert_eq!(rd.code, 0, "{}", rd.out);
    assert_eq!(r.gdd(&["next", "r1"]).facts["state"]["next"]["step"], "ask");
    // A yellow brief names the reading only as a choice.
    yellow_brief(&r, "y5");
    let n = r.gdd(&["next", "y5"]);
    assert_eq!(n.facts["state"]["next"]["step"], "ask");
    assert!(n.has("gdd launch y5 reading"), "{}", n.out);
}

#[test]
fn ask_names_the_command_its_answer_takes() {
    let r = Repo::new();
    r.gdd(&["brief", "a1", "--paths", "src/ui/page.txt"]);
    for of in ["blueprint", "gates", "debt", "brief", "change"] {
        let a = r.gdd(&["ask", "a1", "--of", of, "Yes?"]);
        assert!(
            a.facts["then"]
                .as_str()
                .unwrap()
                .starts_with("gdd approve a1"),
            "{of}: {}",
            a.out
        );
    }
    for of in ["decision", "stop"] {
        let a = r.gdd(&["ask", "a1", "--of", of, "Yes?"]);
        assert!(
            a.facts["then"]
                .as_str()
                .unwrap()
                .starts_with("gdd answer a1"),
            "{of}: {}",
            a.out
        );
    }
    let a = r.gdd(&["ask", "a1", "--of", "waiver", "Yes?"]);
    assert!(
        a.facts["then"]
            .as_str()
            .unwrap()
            .starts_with("gdd waiver a1"),
        "{}",
        a.out
    );
}

#[test]
fn a_change_with_no_brief_keeps_the_request_word_for_word() {
    let r = Repo::new();
    let words = "Fix the page: it says \"Copyrigth\", please";
    let b = r.gdd(&[
        "brief",
        "g1",
        "--paths",
        "src/ui/page.txt",
        "--request",
        words,
    ]);
    assert_eq!(b.code, 0, "{}", b.out);
    assert_eq!(r.log_lines("g1")[0]["request"], words);
    let l = r.gdd(&["launch", "g1", "build"]);
    let prompt = l.facts["prompt"].as_str().unwrap();
    assert!(
        prompt.contains(&format!("The person's request, word for word: \"{words}\"")),
        "{prompt}"
    );
    assert!(
        prompt.contains("Load the `gdd-build` skill (`gdd:gdd-build` when it comes from a plugin)")
    );
    let base = r.git(&["rev-parse", "main"]);
    r.commit_on(
        "work/g1",
        Some(&base),
        &[("src/ui/page.txt", "Copyright\n")],
        "Fix",
    );
    r.report("g1", "build", &report_block("gdd-build", "done"), &[]);
    r.gdd(&["gates", "g1"]);
    let rv = r.gdd(&["launch", "g1", "review"]);
    let prompt = rv.facts["prompt"].as_str().unwrap();
    assert!(prompt.contains(words), "{prompt}");
    let log = std::fs::canonicalize(r.path("gdd/log/g1.jsonl")).unwrap();
    assert!(
        prompt.contains(&log.to_string_lossy().to_string()),
        "the log's absolute path: {prompt}"
    );
    // The journal never keeps the request's words.
    assert!(!r.read("gdd/log/g1.engine.jsonl").contains("Copyrigth"));
}

#[test]
fn a_setup_log_says_what_is_set_up() {
    let r = Repo::new();
    let n = r.gdd(&["next", "standing-checks"]);
    assert_eq!(n.facts["state"]["next"]["step"], "checks", "{}", n.out);
    let l = r.gdd(&["launch", "standing-checks", "checks"]);
    let prompt = l.facts["prompt"].as_str().unwrap();
    assert!(
        prompt.contains("Set up the standing checks the blueprint's `checks` name"),
        "{prompt}"
    );
    assert!(
        !prompt.contains("There is no brief") && !prompt.contains("not set"),
        "{prompt}"
    );
    let l = r.gdd(&["launch", "gates-pay", "checks"]);
    assert!(
        l.facts["prompt"]
            .as_str()
            .unwrap()
            .contains("gdd/gates/pay.feature")
    );
    let l = r.gdd(&["launch", "standing-checks-secrets", "checks"]);
    assert!(
        l.facts["prompt"]
            .as_str()
            .unwrap()
            .contains("the check secrets")
    );
}

#[test]
fn a_launch_that_closes_its_own_standard_input() {
    let scripts = tempfile::tempdir().unwrap();
    let s = scripts.path().join("role.sh");
    std::fs::write(&s, "#!/bin/sh\necho \"$1\" > \"$(dirname \"$0\")/model.txt\"\nprintf 'pass\\nCOLOR: green\\n```gdd-report\\nskill: gdd-review 0.11.0\\nresult: pass\\ncaught: none\\n```\\n'\n").unwrap();
    let bp = BLUEPRINT.replace(
        "launch: the Agent tool, with its model field",
        &format!("launch: sh {} <model> < /dev/null", s.display()),
    );
    let r = Repo::with(&bp);
    r.gdd(&["brief", "l1", "--paths", "src/ui/page.txt"]);
    let base = r.git(&["rev-parse", "main"]);
    r.commit_on(
        "work/l1",
        Some(&base),
        &[("src/ui/page.txt", "p\n")],
        "Page",
    );
    r.report("l1", "build", &report_block("gdd-build", "done"), &[]);
    r.gdd(&["gates", "l1"]);
    let l = r.gdd(&["launch", "l1", "review", "--run", "--start-wait", "5"]);
    assert_eq!(l.code, 0, "{}", l.out);
    assert!(
        !l.facts["argv"].to_string().contains("/dev/null"),
        "{}",
        l.facts["argv"]
    );
    assert_eq!(
        std::fs::read_to_string(scripts.path().join("model.txt"))
            .unwrap()
            .trim(),
        "model-a"
    );
}

#[test]
fn the_orchestrators_own_steps() {
    let r = Repo::new();
    let p = r.gdd(&[
        "step",
        "blueprint",
        "prepare",
        "--result",
        "proposed",
        "--file",
        "gdd/blueprint.md",
        "--file",
        "gdd/backlog.md",
        "--probe",
        r#"[{"try":"push to main","command":"git push origin main","exit":1,"tail":"protected"}]"#,
    ]);
    assert_eq!(p.code, 0, "{}", p.out);
    let h = r.gdd(&[
        "step",
        "blueprint",
        "harden",
        "--change",
        "y1",
        "--crack",
        "a test asserted less than its scenario",
        "--scenario",
        "none",
        "--proposal",
        "add mutation to yellow",
    ]);
    assert_eq!(h.code, 0, "{}", h.out);
    let e = r.gdd(&[
        "step",
        "blueprint",
        "exam",
        "--level",
        "standard",
        "--model",
        "model-b",
        "--asked",
        "3",
        "--caught",
        "2",
        "--missed",
        "q2",
    ]);
    assert_eq!(e.code, 0, "{}", e.out);
    let lines = r.log_lines("blueprint");
    assert_eq!(lines[0]["report"]["skill"], "gdd-prepare");
    assert_eq!(lines[0]["report"]["result"], "proposed");
    assert_eq!(
        lines[0]["files"],
        serde_json::json!(["gdd/blueprint.md", "gdd/backlog.md"])
    );
    assert_eq!(lines[0]["probe"][0]["exit"], 1);
    assert_eq!(lines[1]["change"], "y1");
    assert_eq!(lines[1]["scenario"], Value::Null);
    assert_eq!(lines[2]["reviewer"]["model"], "model-b");
    assert_eq!(lines[2]["caught"], 2);
    assert_eq!(lines[2]["missed"], serde_json::json!(["q2"]));
    let v = r.gdd(&["verify", "blueprint"]);
    assert_eq!(v.facts["errors"], 0, "{}", v.out);
    assert_eq!(
        r.gdd(&["step", "y1", "prepare", "--result", "proposed"])
            .code,
        2
    );
    assert_eq!(
        r.gdd(&["step", "blueprint", "prepare", "--result", "maybe"])
            .code,
        2
    );
    assert_eq!(
        r.gdd(&[
            "step",
            "blueprint",
            "exam",
            "--model",
            "m",
            "--asked",
            "1",
            "--caught",
            "2"
        ])
        .code,
        2
    );
}

#[test]
fn verify_trusts_the_layers_own_lines_in_the_same_second() {
    let r = Repo::new();
    let now = [("GDD_NOW", "2026-11-13T10:00:00Z")];
    r.gdd_env(&["brief", "s1", "--paths", "src/ui/page.txt"], &now);
    let base = r.git(&["rev-parse", "main"]);
    r.commit_on(
        "work/s1",
        Some(&base),
        &[("src/ui/page.txt", "p\n")],
        "Page",
    );
    let f = r.dir.path().join("build.txt");
    std::fs::write(&f, report_block("gdd-build", "done")).unwrap();
    let fs = f.to_string_lossy().to_string();
    r.gdd_env(&["report", "s1", "build", &fs], &now);
    r.gdd_env(&["gates", "s1"], &now);
    let v = r.gdd(&["verify", "s1"]);
    assert!(
        !v.has("(same-second)"),
        "every line has its call: {}",
        v.out
    );
    // A line with no call behind it, in the same second: a sign of a log written afterwards.
    let line = r#"{"contract":"gdd/1","ts":"2026-11-13T10:00:00Z","brief":"s1","event":"step","step":"review","report":{"skill":"gdd-review","result":"pass"},"verdict":"pass"}"#;
    let log = r.read("gdd/log/s1.jsonl");
    r.write("gdd/log/s1.jsonl", &format!("{log}{line}\n"));
    let v = r.gdd(&["verify", "s1"]);
    assert!(v.has("(same-second)"), "{}", v.out);
}

#[test]
fn every_call_is_in_the_metrics_without_its_id_and_without_git() {
    let r = Repo::new();
    r.gdd(&["next", "zz"]);
    r.gdd(&["brief", "g1", "--paths", "src/ui/page.txt"]);
    r.gdd(&["gates", "g1"]);
    // A copy of the project without .git, and without the blueprint.
    let copy = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(copy.path().join("gdd/log")).unwrap();
    for e in std::fs::read_dir(r.path("gdd/log")).unwrap() {
        let e = e.unwrap();
        std::fs::copy(e.path(), copy.path().join("gdd/log").join(e.file_name())).unwrap();
    }
    let m = run_gdd(
        copy.path(),
        &["metrics", "--all", "--format", "jsonl"],
        &[("GDD_NO_JOURNAL", "1")],
    );
    assert_eq!(m.code, 0, "{}", m.out);
    let engine: Vec<Value> = m
        .out
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["table"] == "engine")
        .collect();
    assert!(
        engine
            .iter()
            .any(|e| e["change"] == "zz" && e["command"] == "next"),
        "{engine:?}"
    );
    assert!(
        engine
            .iter()
            .any(|e| e["change"] == "g1" && e["command"] == "gates")
    );
    assert_eq!(m.facts["git"], false);
}

/// Two green changes built from the same base, each through its gates and its review.
fn two_green_changes(r: &Repo) {
    let base = r.git(&["rev-parse", "main"]);
    for (id, file) in [("ga", "src/ui/a.txt"), ("gb", "src/ui/b.txt")] {
        r.gdd(&["brief", id, "--paths", file]);
        r.commit_on(
            &format!("work/{id}"),
            Some(&base),
            &[(file, "new\n")],
            "Build",
        );
        r.report(id, "build", &report_block("gdd-build", "done"), &[]);
        assert_eq!(r.gdd(&["gates", id]).code, 0);
        r.report(id, "review", &review_text("pass", "green", ""), &[]);
    }
}

#[test]
fn after_a_local_merge_the_next_change_bookkeeping_applies_cleanly() {
    let r = tracked_logs_repo();
    two_green_changes(&r);
    let a = r.gdd(&["ship", "ga", "--merge"]);
    assert_eq!(a.code, 0, "{}", a.out);
    assert_eq!(r.git(&["status", "--porcelain"]), "", "{}", a.out);
    // The next one ships by pull request: its bookkeeping, a merge on the platform, then --done.
    let b = r.gdd(&["ship", "gb", "--bookkeeping"]);
    assert_eq!(b.code, 0, "{}", b.out);
    assert!(
        git_ok(
            &r.root,
            &[
                "-c",
                "user.name=T",
                "-c",
                "user.email=t@example.com",
                "merge",
                "-q",
                "--no-ff",
                "--no-edit",
                "work/gb"
            ]
        ),
        "the bookkeeping applies cleanly"
    );
    let d = r.gdd(&["ship", "gb", "--done", "main"]);
    assert_eq!(d.code, 0, "{}", d.out);
    assert!(d.has("travels with the next change's bookkeeping: it is not something to report to the person"), "{}", d.out);
    let v = r.gdd(&["verify", "--all"]);
    assert_eq!(v.facts["errors"], 0, "{}", v.out);
}

#[test]
fn a_local_merge_after_a_pull_request_takes_the_copys_bookkeeping() {
    let r = tracked_logs_repo();
    two_green_changes(&r);
    // The first ships by pull request: its ship line stays in the copy. The copy's untracked logs
    // go aside while main takes the merge, as the platform's merge reaches the copy, and come back.
    r.gdd(&["ship", "ga", "--bookkeeping"]);
    let aside = r.dir.path().join("aside-log");
    std::fs::rename(r.path("gdd/log"), &aside).unwrap();
    assert!(git_ok(
        &r.root,
        &[
            "-c",
            "user.name=T",
            "-c",
            "user.email=t@example.com",
            "merge",
            "-q",
            "--no-ff",
            "--no-edit",
            "work/ga"
        ]
    ));
    for e in std::fs::read_dir(&aside).unwrap() {
        let e = e.unwrap();
        std::fs::copy(e.path(), r.path("gdd/log").join(e.file_name())).unwrap();
    }
    assert_eq!(r.gdd(&["ship", "ga", "--done", "main"]).code, 0);
    // The second one's bookkeeping holds the first one's log, newer than what the main branch has.
    let b = r.gdd(&["ship", "gb", "--merge"]);
    assert_eq!(b.code, 0, "{}", b.out);
    assert!(b.has("the copy's version went in"), "{}", b.out);
    assert!(git_ok(
        &r.root,
        &["merge-base", "--is-ancestor", "work/ga", "main"]
    ));
    assert!(git_ok(
        &r.root,
        &["merge-base", "--is-ancestor", "work/gb", "main"]
    ));
    let in_main = r.git(&["show", "main:gdd/log/ga.jsonl"]);
    assert!(
        in_main.contains("\"step\":\"ship\""),
        "the first ship travelled with the second change"
    );
    assert_eq!(r.git(&["status", "--porcelain"]), "");
    let v = r.gdd(&["verify", "--all"]);
    assert_eq!(v.facts["errors"], 0, "{}", v.out);
}
