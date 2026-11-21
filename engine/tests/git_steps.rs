//! Color and zones, the seal, the gates and verify against git, each in a throwaway repository.

mod common;
use common::*;

#[test]
fn color_rises_and_what_no_zone_covers_is_never_green() {
    let r = Repo::new();
    let c = r.gdd(&["color", "--paths", "src/ui/page.txt"]);
    assert_eq!(c.facts["color"], "green", "{}", c.out);
    let c = r.gdd(&["color", "--paths", "src/ui/page.txt", "docs/new.md"]);
    assert_eq!(
        c.facts["color"], "yellow",
        "a path no zone covers is never green: {}",
        c.out
    );
    assert_eq!(c.facts["uncovered"][0], "docs/new.md");
    let c = r.gdd(&[
        "color",
        "--paths",
        "src/ui/new/deep/file.txt",
        "src/pay/x.txt",
    ]);
    assert_eq!(c.facts["color"], "red");
    assert_eq!(c.facts["zones"], serde_json::json!(["pay", "ui"]));
    // A change briefed green that also touched a yellow path: its touched color rose.
    r.gdd(&["brief", "c1", "--paths", "src/ui/page.txt"]);
    let base = r.git(&["rev-parse", "main"]);
    r.commit_on(
        "work/c1",
        Some(&base),
        &[("src/ui/page.txt", "x\n"), ("src/core/app.txt", "ok too\n")],
        "Both",
    );
    let c = r.gdd(&["color", "c1"]);
    assert_eq!(c.facts["on"], "touched");
    assert_eq!(c.facts["color"], "yellow", "{}", c.out);
    assert_eq!(c.facts["rose"], true);
    // The review step records the touched color, and the next step is back to Brief.
    r.report("c1", "build", &report_block("gdd-build", "done"), &[]);
    r.gdd(&["gates", "c1"]);
    let rv = r.report("c1", "review", &r.review_pass("c1", "green", ""), &[]);
    assert_eq!(rv.code, 0, "{}", rv.out);
    let review = r
        .log_lines("c1")
        .into_iter()
        .find(|e| e["step"] == "review")
        .unwrap();
    assert_eq!(
        review["color"]["color"], "yellow",
        "git's floor wins over the reviewer's word"
    );
    let n = r.gdd(&["next", "c1"]);
    let next = &n.facts["state"]["next"];
    assert_eq!(next["step"], "brief");
    // It passed, and it still doesn't ship at the color it was briefed at.
    let early = r.gdd(&["ship", "c1", "--merge"]);
    assert_eq!(early.facts["refused"], "out-of-order", "{}", early.out);
    assert!(early.has("higher than green: back to Brief"), "{}", early.out);
    r.write(
        "features/c1.feature",
        "Feature: The page and the app change together\n",
    );
    let md = brief_md(&r, "c1");
    assert_eq!(r.gdd(&["seal", "c1", &md, "features/c1.feature"]).code, 0);
    let command = next["command"]
        .as_str()
        .unwrap()
        .replace("<files>", "gdd/briefs/c1.md features/c1.feature")
        .replace("<path>...", "src/ui/page.txt src/core/app.txt")
        .replace("<what raised it>", "the app also changed");
    let raised = r.named(&command);
    assert_eq!(raised.code, 0, "{}", raised.out);
    assert_eq!(raised.facts["color"], "yellow");
    // Judgment can't lower a color.
    let b = r.gdd(&[
        "brief",
        "c2",
        "--paths",
        "src/pay/pay.txt",
        "--color",
        "green",
    ]);
    assert!(b.has("the color is red, not green"), "{}", b.out);
    // And a red change needs its brief: the step with no files is refused, so nothing is logged
    // at the lower color either.
    assert_eq!(b.facts["refused"], "no-brief", "{}", b.out);
    assert!(r.log_lines("c2").is_empty(), "{:?}", r.log_lines("c2"));
}

/// A change briefed green whose build also touched a yellow path, reviewed with `verdict`.
fn reviewed_at_a_higher_color(r: &Repo, verdict: &str) {
    r.gdd(&["brief", "c1", "--paths", "src/ui/page.txt"]);
    let base = r.git(&["rev-parse", "main"]);
    r.commit_on(
        "work/c1",
        Some(&base),
        &[("src/ui/page.txt", "x\n"), ("src/core/app.txt", "ok too\n")],
        "Both",
    );
    r.report("c1", "build", &report_block("gdd-build", "done"), &[]);
    r.gdd(&["gates", "c1"]);
    let launch = r.gdd(&["launch", "c1", "review"]);
    let found = if verdict == "pass" {
        ""
    } else {
        "f1 src/core/app.txt:1 the app changed and nothing covers it | request: page\n"
    };
    let text = launch.with_reviewed(&review_text(verdict, "green", found));
    let rv = r.report("c1", "review", &text, &[]);
    assert_eq!(rv.code, 0, "{verdict}: {}", rv.out);
}

#[test]
fn a_color_that_rose_goes_back_to_brief_whatever_the_verdict() {
    for verdict in ["fix", "stop"] {
        let r = Repo::new();
        reviewed_at_a_higher_color(&r, verdict);
        if verdict == "stop" {
            // A stop is told first, and the person answers it.
            let n = r.gdd(&["next", "c1"]);
            assert_eq!(n.facts["state"]["next"]["blocked"], "review-stop", "{}", n.out);
            assert_eq!(r.gdd(&["ask", "c1", "--of", "stop", "It stopped."]).code, 0);
            assert_eq!(r.gdd(&["answer", "c1", "Fix it."]).code, 0);
        }
        // Not a fix round: the color comes first.
        let n = r.gdd(&["next", "c1"]);
        let next = &n.facts["state"]["next"];
        assert_eq!(next["step"], "brief", "{verdict}: {}", n.out);
        assert!(n.has("the reviewer found yellow, higher than green"), "{}", n.out);
        // After a stop, it also says how a change the person dropped ends.
        assert_eq!(n.has("gdd brief c1 --stop"), verdict == "stop", "{}", n.out);
        // The new brief, its approval and its checks: the judging starts over at yellow.
        r.write("features/c1.feature", &ok_feature("c1"));
        let md = brief_md(&r, "c1");
        assert_eq!(r.gdd(&["seal", "c1", &md, "features/c1.feature"]).code, 0);
        let command = next["command"]
            .as_str()
            .unwrap()
            .replace("<files>", "gdd/briefs/c1.md features/c1.feature")
            .replace("<path>...", "src/ui/page.txt src/core/app.txt")
            .replace("<what raised it>", "the app also changed");
        let raised = r.named(&command);
        assert_eq!(raised.code, 0, "{verdict}: {}", raised.out);
        assert_eq!(raised.facts["color"], "yellow");
        let yes = r.gdd(&[
            "approve",
            "c1",
            "--of",
            "brief",
            "--who",
            "Person",
            "--where",
            "conversation",
            "gdd/briefs/c1.md",
            "features/c1.feature",
        ]);
        assert_eq!(yes.code, 0, "{verdict}: {}", yes.out);
        r.commit_on(
            "gdd-seal/c1",
            None,
            &[("tests/c1.sh", "true # gdd:c1-1\n")],
            "Seal",
        );
        let sealed = r.report("c1", "checks", &sealed_report(&["tests"]), &[]);
        assert_eq!(sealed.code, 0, "{verdict}: {}", sealed.out);
        let n = r.gdd(&["next", "c1"]);
        assert_eq!(n.facts["state"]["next"]["step"], "build", "{verdict}: {}", n.out);
        assert_eq!(n.facts["state"]["color"], "yellow", "{verdict}: {}", n.out);
    }
}

#[test]
fn a_color_that_rose_comes_before_a_round_for_gates_that_failed_later() {
    let r = Repo::new();
    reviewed_at_a_higher_color(&r, "pass");
    // The branch moves after the review and its gates fail: still the brief, not a fix round.
    r.commit_on("work/c1", None, &[("src/core/app.txt", "no\n")], "Break");
    let g = r.gdd(&["gates", "c1"]);
    assert!(g.has("tests      FAIL"), "{}", g.out);
    let n = r.gdd(&["next", "c1"]);
    assert_eq!(n.facts["state"]["next"]["step"], "brief", "{}", n.out);
}

#[test]
fn a_color_that_rose_comes_before_the_findings_about_a_check() {
    let r = Repo::new();
    yellow_brief(&r, "y8", "src/core/app.txt");
    r.gdd(&[
        "approve",
        "y8",
        "--of",
        "brief",
        "--who",
        "Person",
        "--where",
        "conversation",
        "gdd/briefs/y8.md",
        "features/y8.feature",
    ]);
    r.commit_on(
        "gdd-seal/y8",
        None,
        &[("tests/y8.sh", "true # gdd:y8-1\n")],
        "Seal",
    );
    r.report("y8", "checks", &sealed_report(&["tests"]), &[]);
    let start = r.git(&["rev-parse", "gdd-seal/y8"]);
    // The build also touched a red path.
    r.commit_on(
        "work/y8",
        Some(&start),
        &[("src/core/app.txt", "it says ok\n"), ("src/pay/pay.txt", "x\n")],
        "Round 1",
    );
    assert_eq!(r.report("y8", "build", &report_block("gdd-build", "done"), &[]).code, 0);
    assert_eq!(r.gdd(&["gates", "y8"]).code, 0);
    // Its only finding names a sealed test, which would go to the checks agent.
    let found = "f1 tests/y8.sh:1 the test passes whatever the app says | @id:y8-1\n";
    let rv = r.report("y8", "review", &review_text("fix", "yellow", found), &[]);
    assert_eq!(rv.code, 0, "{}", rv.out);
    let review = r.log_lines("y8").into_iter().rfind(|e| e["step"] == "review").unwrap();
    assert_eq!(review["fix_role"], "checks", "{review}");
    assert_eq!(review["color"]["color"], "red", "{review}");
    let n = r.gdd(&["next", "y8"]);
    assert_eq!(n.facts["state"]["next"]["step"], "brief", "{}", n.out);
    assert!(n.has("the reviewer found red, higher than yellow"), "{}", n.out);
}

#[test]
fn a_change_is_told_what_its_zone_still_has_to_set_up() {
    let r = Repo::new();
    let more = "| gates-core | The core's own gate | yellow | 1 | | todo |\n\
        | gates-ui-2 | A second gate for the pages | green | 1 | | todo |\n\
        | standing-checks-slow | The slow check | green | 1 | | todo |\n\
        | standing-checks-e2e | A check no color asks for | green | 1 | | todo |\n";
    r.write("gdd/backlog.md", &format!("{BACKLOG}{more}"));
    r.commit("More to set up");
    let ahead = |paths: &str| r.gdd(&["color", "--paths", paths]);
    // Its zone's own gate. The slow check only runs on the pages, so it isn't named here.
    let c = ahead("src/core/app.txt");
    assert_eq!(
        c.facts["setup_ahead"],
        serde_json::json!(["gates-core"]),
        "{}",
        c.out
    );
    assert!(
        c.has("zone core's own gate isn't set up yet (gates-core, todo)")
            && c.has("gdd next gates-core"),
        "{}",
        c.out
    );
    // On the pages: their second gate, and the check their color asks for and their files run.
    let c = ahead("src/ui/page.txt");
    assert_eq!(
        c.facts["setup_ahead"],
        serde_json::json!(["gates-ui-2", "standing-checks-slow"]),
        "{}",
        c.out
    );
    assert!(c.has("the check slow, which green asks for"), "{}", c.out);
    // Red asks for no slow check, and its zone has no gate of its own to set up.
    let c = ahead("src/pay/pay.txt");
    assert_eq!(c.facts["setup_ahead"], serde_json::json!([]), "{}", c.out);
    // A setup log goes ahead of nothing.
    let c = r.gdd(&["color", "gates-core", "--paths", "src/core/app.txt"]);
    assert!(c.facts["setup_ahead"].is_null(), "{}", c.out);
    // It is said again when the change starts, once, and the change starts all the same.
    let b = r.gdd(&["brief", "g1", "--paths", "src/ui/page.txt"]);
    assert_eq!(b.code, 0, "{}", b.out);
    assert_eq!(
        b.facts["setup_ahead"],
        serde_json::json!(["gates-ui-2", "standing-checks-slow"]),
        "{}",
        b.out
    );
    let again = r.gdd(&["brief", "g1", "--paths", "src/ui/page.txt"]);
    assert_eq!(again.code, 0, "{}", again.out);
    assert!(again.facts["setup_ahead"].is_null(), "{}", again.out);
    assert_eq!(r.log_lines("g1").len(), 2);
    // What is set up is no longer named.
    let done = more.replace("| todo |", "| done |");
    r.write("gdd/backlog.md", &format!("{BACKLOG}{done}"));
    let c = ahead("src/core/app.txt");
    assert_eq!(c.facts["setup_ahead"], serde_json::json!([]), "{}", c.out);
    // And before the standing checks hold, no change starts: nothing to go ahead of, as when
    // Prepare tries the zones.
    let before = BACKLOG.replace("| red | 0 | | done |", "| red | 0 | | todo |");
    assert_ne!(before, BACKLOG);
    r.write("gdd/backlog.md", &format!("{before}{more}"));
    let c = ahead("src/core/app.txt");
    assert_eq!(c.facts["setup_ahead"], serde_json::json!([]), "{}", c.out);
}

#[test]
fn zones_find_a_pathspec_that_matches_nothing_and_what_no_zone_covers() {
    let bp = BLUEPRINT.replace(
        "      - ':(glob)src/ui/**'\n",
        "      - ':(glob)src/ui/**'\n      - ':(glob)src/pages/**'\n",
    );
    let r = Repo::with(&bp);
    r.write("docs/readme.md", "x\n");
    r.commit("A doc");
    let z = r.gdd(&["zones"]);
    assert_eq!(z.code, 0, "{}", z.out);
    assert_eq!(z.facts["empty"][0]["pathspec"], ":(glob)src/pages/**");
    let uncovered: Vec<&str> = z.facts["uncovered"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(uncovered.contains(&"docs/readme.md"));
    assert!(uncovered.contains(&".gitignore"));
    assert!(!uncovered.contains(&"src/ui/page.txt"));
}

#[test]
fn blueprint_preview_finds_a_server_zone_its_checks_never_reach() {
    let bp = BLUEPRINT
        .replace("src/pay/**", "server/**")
        .replace("    command: sh tests/run.sh", "    when: [':(glob)src/ui/**']\n    command: sh tests/run.sh")
        .replace("    color: red\n    paths:\n      - ':(glob)server/**'", "    color: red\n    gates: gdd/gates/server.feature\n    paths:\n      - ':(glob)server/**'");
    let r = Repo::with(&bp);
    r.write("gdd/gates/server.feature", "@check:slow\nFeature: Pay\n");
    // A new project can try an item before the server or its tests exist.
    let preview = r.gdd(&["color", "--paths", "server/new.rs"]);
    assert_eq!(preview.code, 0, "{}", preview.out);
    assert_eq!(preview.facts["color"], "red");
    assert_eq!(preview.facts["checks"], serde_json::json!(["seal"]));
    assert_eq!(
        preview.facts["other_checks"],
        serde_json::json!(["review", "brief-approval", "person-review"])
    );
    let skipped = &preview.facts["skipped"];
    assert_eq!(skipped[0]["check"], "tests");
    assert_eq!(skipped[1]["check"], "slow");
    assert!(preview.out.lines().any(|l| l.contains("tests")
        && l.contains("skipped")
        && l.contains("src/ui/**")
        && !l.starts_with("gdd ")));

    // The UI makes tests apply to the mixed change. The server alone still has a gap.
    let mixed = r.gdd(&[
        "color",
        "--paths",
        "server/new.rs",
        "./src/ui/",
        "src/ui/page.txt",
    ]);
    assert_eq!(
        mixed.facts["checks"],
        serde_json::json!(["seal", "tests", "slow"])
    );
    assert_eq!(mixed.facts["skipped"], serde_json::json!([]));
    let gap = serde_json::json!({"zone": "pay", "color": "red", "check": "tests", "when": [":(glob)src/ui/**"]});
    assert_eq!(mixed.facts["gaps"], serde_json::json!([gap]));
    assert!(mixed.out.lines().any(|l| l.contains("pay")
        && l.contains("tests")
        && l.contains("red")
        && !l.starts_with("gdd ")));

    r.write("server/pay.rs", "pay\n");
    r.write("server/private/keys.rs", "private\n");
    r.commit("Add the server");
    let zones = r.gdd(&["zones"]);
    assert_eq!(zones.code, 0, "{}", zones.out);
    assert!(zones.facts["gaps"].as_array().unwrap().contains(&gap));
    // One of several when pathspecs reaches the zone. Git exclusions still apply.
    let watched = bp.replace(
        "when: [':(glob)src/ui/**']",
        "when: [':(glob)src/ui/**', ':(glob)server/**', ':(exclude,glob)server/private/**']",
    );
    r.write("gdd/blueprint.md", &watched);
    let zones = r.gdd(&["zones"]);
    assert!(
        !zones.facts["gaps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["zone"] == "pay")
    );
    let excluded = r.gdd(&["color", "--paths", "server/private/keys.rs"]);
    assert_eq!(excluded.facts["checks"], serde_json::json!(["seal"]));
}

#[cfg(unix)]
#[test]
fn a_local_gate_clone_shares_unchanged_git_objects() {
    use std::os::unix::fs::MetadataExt;
    let r = Repo::new();
    let head = r.git(&["rev-parse", "HEAD"]);
    let clone = r.dir.path().join("gate-clone");
    gdd::git::clone_at(&r.root, &clone, &head).unwrap();
    let object = format!(".git/objects/{}/{}", &head[..2], &head[2..]);
    assert_eq!(
        r.path(&object).metadata().unwrap().ino(),
        clone.join(object).metadata().unwrap().ino()
    );
}

#[test]
fn printed_brief_commands_put_the_files_before_paths() {
    let r = Repo::new();
    let path = "features/two words.feature";
    r.write(path, "Feature: Captions\n");
    assert!(
        r.gdd(&["next", "order"])
            .has("[<brief and scenario files>] --paths")
    );
    let first = r.gdd(&["seal", "order", path]);
    assert!(
        first.has("gdd brief order 'features/two words.feature' --paths"),
        "{}",
        first.out
    );
    let again = r.gdd(&["seal", "order", path]);
    assert!(
        again.has("gdd brief order 'features/two words.feature' --paths"),
        "{}",
        again.out
    );
    let early = r.gdd(&["ask", "order", "--of", "brief", "Approve?"]);
    assert!(
        early.has("[<brief and scenario files>] --paths"),
        "{}",
        early.out
    );
}

#[test]
fn a_sealed_file_cannot_be_a_change_path() {
    let r = Repo::new();
    let file = "features/title.feature";
    r.write(file, "Feature: Titles\n");
    let md = brief_md(&r, "title");
    assert_eq!(r.gdd(&["seal", "title", &md, file]).code, 0);
    let wrong = r.gdd(&[
        "brief",
        "title",
        "--paths",
        "src/core/app.txt",
        file,
        "--color",
        "red",
        "--why",
        "shared titles",
    ]);
    assert_eq!(wrong.facts["refused"], "sealed-path", "{}", wrong.out);
    assert!(wrong.has("gdd brief title features/title.feature --paths src/core/app.txt --color red --why 'shared titles'"));
    assert_eq!(r.log_lines("title").len(), 1);
    assert_eq!(
        r.gdd(&["brief", "title", &md, file, "--paths", "src/core/app.txt"])
            .code,
        0
    );
}

#[test]
fn the_seal_leaves_the_checkout_and_its_index_untouched() {
    let r = Repo::new();
    r.write(".git/worktrees", "no linked worktrees here\n");
    r.write("src/core/app.txt", "ok, edited and staged\n");
    r.git(&["add", "src/core/app.txt"]);
    r.write("src/ui/page.txt", "edited, not staged\n");
    r.write("gdd/briefs/s1.md", "---\nversion: 1\n---\n# S1\n");
    r.write("features/s1.feature", "Feature: S1\n");
    let status_before = r.git(&["status", "--porcelain"]);
    let index_before = r.git(&["ls-files", "--stage"]);
    let head_before = r.git(&["rev-parse", "HEAD"]);
    let branch_before = r.git(&["symbolic-ref", "HEAD"]);
    let s = r.gdd(&["seal", "s1", "gdd/briefs/s1.md", "features/s1.feature"]);
    assert_eq!(s.code, 0, "{}", s.out);
    assert_eq!(r.git(&["status", "--porcelain"]), status_before);
    assert_eq!(r.git(&["ls-files", "--stage"]), index_before);
    assert_eq!(r.git(&["rev-parse", "HEAD"]), head_before);
    assert_eq!(r.git(&["symbolic-ref", "HEAD"]), branch_before);
    let tip = r.git(&["rev-parse", "gdd-seal/s1"]);
    assert_eq!(s.facts["commit"], tip.as_str());
    assert_eq!(
        r.git(&["rev-parse", "gdd-seal/s1^"]),
        head_before,
        "cut from the base"
    );
    let files = r.git(&["diff", "--name-only", &head_before, &tip]);
    assert_eq!(
        files, "features/s1.feature\ngdd/briefs/s1.md",
        "the seal holds only what was named"
    );
    assert_eq!(
        s.facts["files"]["gdd/briefs/s1.md"],
        r.git(&["hash-object", "gdd/briefs/s1.md"]).as_str()
    );
    // Sealing the same files again changes nothing.
    let again = r.gdd(&["seal", "s1", "gdd/briefs/s1.md"]);
    assert!(again.has("already holds"), "{}", again.out);
    // The main branch merged into the seal, without a checkout.
    r.git(&["stash", "-q", "--include-untracked"]);
    r.write("src/core/other.txt", "more\n");
    let main2 = r.commit("Main moves");
    r.git(&["stash", "pop", "-q", "--index"]);
    let m = r.gdd(&["seal", "s1", "--merge-main"]);
    assert_eq!(m.code, 0, "{}", m.out);
    let merged = r.git(&["rev-parse", "gdd-seal/s1"]);
    assert!(git_ok(
        &r.root,
        &["merge-base", "--is-ancestor", &main2, &merged]
    ));
    assert_eq!(r.git(&["status", "--porcelain"]), status_before);
}

#[test]
fn gates_judge_the_commit_and_not_stray_files() {
    let r = Repo::new();
    r.gdd(&["brief", "g2", "--paths", "src/ui/page.txt"]);
    let base = r.git(&["rev-parse", "main"]);
    let head = r.commit_on(
        "work/g2",
        Some(&base),
        &[("src/ui/page.txt", "new page\n")],
        "Page",
    );
    r.report("g2", "build", &report_block("gdd-build", "done"), &[]);
    // A stray file in the orchestrator's copy that would break the tests: the gates don't see it.
    r.write("src/core/app.txt", "bad, and not committed\n");
    let g = r.gdd(&["gates", "g2"]);
    assert_eq!(g.code, 0, "{}", g.out);
    assert_eq!(g.facts["head"], head.as_str());
    let gates = r
        .log_lines("g2")
        .into_iter()
        .find(|e| e["step"] == "gates")
        .unwrap();
    assert_eq!(gates["head"], head.as_str());
    assert_eq!(gates["start"], base.as_str());
    assert_eq!(gates["where"], "a fresh worktree");
    for e in gates["gates"].as_array().unwrap() {
        assert!(e["seconds"].is_number());
        assert!(e["command"].is_string());
        assert_eq!(e["exit"], 0);
    }
    assert_eq!(gates["gates"][0]["check"], "seal");
    assert_eq!(gates["gates"][2]["check"], "slow");
    assert_eq!(gates["gates"][2]["tail"], "the slow check ran");
    // No worktree is left behind.
    assert_eq!(r.git(&["worktree", "list"]).lines().count(), 1);
    // --at refuses a directory that isn't at that commit, or isn't clean.
    let at = r.root.to_string_lossy().to_string();
    let bad = r.gdd(&["gates", "g2", "--at", &at]);
    assert_eq!(bad.facts["refused"], "wrong-commit", "{}", bad.out);
}

#[test]
fn a_failing_check_runs_before_review_checks_never() {
    let r = Repo::new();
    r.gdd(&["brief", "g3", "--paths", "src/ui/page.txt"]);
    let base = r.git(&["rev-parse", "main"]);
    r.commit_on(
        "work/g3",
        Some(&base),
        &[("src/ui/page.txt", "p\n"), ("src/core/app.txt", "bad\n")],
        "Breaks the tests",
    );
    let message = format!(
        "Do the work.\n```json\n[{{\"check\":\"tests\",\"command\":\"test\",\"failures\":2,\"exit\":0,\"tail\":\"OK\"}}]\n```\n{}",
        report_block("gdd-build", "done")
    );
    let report = r.report("g3", "build", &message, &[]);
    assert_eq!(report.code, 0, "{}", report.out);
    assert_eq!(
        r.log_lines("g3").last().unwrap()["claims"][0]["failures"],
        2
    );
    let g = r.gdd(&["gates", "g3"]);
    assert_eq!(g.code, 1);
    let ran: Vec<&str> = g.facts["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["check"].as_str().unwrap())
        .collect();
    assert_eq!(
        ran,
        vec!["seal", "tests"],
        "the slow check waits for the rest to pass"
    );
    assert_eq!(
        g.facts["skipped"],
        serde_json::json!([{"check":"slow","why":"tests failed first"}])
    );
}

#[test]
fn a_touched_seal_stops_everything() {
    let r = Repo::new();
    r.gdd(&["brief", "g4", "--paths", "src/ui/page.txt"]);
    let base = r.git(&["rev-parse", "main"]);
    r.commit_on(
        "work/g4",
        Some(&base),
        &[
            ("src/ui/page.txt", "p\n"),
            ("AGENTS.md", "# Rewritten by the builder\n"),
        ],
        "Touch AGENTS.md",
    );
    r.report("g4", "build", &report_block("gdd-build", "done"), &[]);
    let g = r.gdd(&["gates", "g4"]);
    assert_eq!(g.code, 1);
    assert_eq!(g.facts["checks"].as_array().unwrap().len(), 1, "{}", g.out);
    let n = r.gdd(&["next", "g4"]);
    assert_eq!(n.facts["state"]["next"]["blocked"], "seal-broken");
}

#[test]
fn verify_with_git() {
    let r = Repo::new();
    r.write("gdd/briefs/v1.md", "---\nversion: 1\n---\n# V1\n");
    r.write("features/v1.feature", "Feature: V1\n");
    r.gdd(&["seal", "v1", "gdd/briefs/v1.md", "features/v1.feature"]);
    r.gdd(&[
        "brief",
        "v1",
        "gdd/briefs/v1.md",
        "features/v1.feature",
        "--paths",
        "src/core/app.txt",
    ]);
    let clean = r.gdd(&["verify", "v1"]);
    assert_eq!(clean.facts["errors"], 0, "{}", clean.out);
    // A sealed hash composed by hand, wrong; a ship that isn't in main; a gates start that isn't the seal's.
    let base = r.git(&["rev-parse", "main"]);
    let tip = r.git(&["rev-parse", "gdd-seal/v1"]);
    let side = r.commit_on(
        "side/v1",
        Some(&base),
        &[("src/core/app.txt", "ok side\n")],
        "Side",
    );
    let log = r.read("gdd/log/v1.jsonl");
    let wrong = format!(
        "{{\"contract\":\"gdd/1\",\"ts\":\"2030-01-01T10:00:00Z\",\"brief\":\"v1\",\"event\":\"step\",\"step\":\"checks\",\"report\":{{\"skill\":\"gdd-checks\",\"result\":\"sealed\"}},\"seal\":{{\"ref\":\"gdd-seal/v1\",\"commit\":\"{tip}\",\"checks\":{{\"features/v1.feature\":\"1111111111111111111111111111111111111111\"}},\"proofs\":[]}}}}\n\
{{\"contract\":\"gdd/1\",\"ts\":\"2030-01-01T10:01:00Z\",\"brief\":\"v1\",\"event\":\"step\",\"step\":\"build\",\"report\":{{\"skill\":\"gdd-build\",\"result\":\"done\"}}}}\n\
{{\"contract\":\"gdd/1\",\"ts\":\"2030-01-01T10:02:00Z\",\"brief\":\"v1\",\"event\":\"step\",\"step\":\"gates\",\"start\":\"{side}\",\"head\":\"{side}\",\"gates\":[{{\"check\":\"tests\",\"seconds\":1,\"result\":\"pass\"}}]}}\n\
{{\"contract\":\"gdd/1\",\"ts\":\"2030-01-01T10:03:00Z\",\"brief\":\"v1\",\"event\":\"step\",\"step\":\"review\",\"report\":{{\"skill\":\"gdd-review\",\"result\":\"pass\"}},\"verdict\":\"pass\",\"color\":{{\"head\":\"{side}\"}}}}\n\
{{\"contract\":\"gdd/1\",\"ts\":\"2030-01-01T10:04:00Z\",\"brief\":\"v1\",\"event\":\"step\",\"step\":\"ship\",\"where\":\"{side}\",\"color\":\"yellow\"}}\n"
    );
    r.write("gdd/log/v1.jsonl", &format!("{log}{wrong}"));
    let v = r.gdd(&["verify", "v1"]);
    assert_eq!(v.code, 1, "{}", v.out);
    let codes: Vec<&str> = v.facts["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["code"].as_str().unwrap())
        .collect();
    assert!(codes.contains(&"seal-hash"), "{codes:?}");
    assert!(codes.contains(&"not-merged"), "{codes:?}");
    assert!(codes.contains(&"start"), "{codes:?}");
    assert!(
        codes.contains(&"needs-approval"),
        "a build before the approval yellow asks for: {codes:?}"
    );
    // Without git, only what the log alone shows.
    let n = r.gdd(&["verify", "v1", "--no-git"]);
    let codes: Vec<&str> = n.facts["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["code"].as_str().unwrap())
        .collect();
    assert!(
        !codes.contains(&"seal-hash") && !codes.contains(&"not-merged"),
        "{codes:?}"
    );
    assert!(codes.contains(&"needs-approval"));
}

/// A path as an agent writes it, with `./` in front, as a folder, or as a folder that doesn't exist
/// yet, falls in its zone all the same; a path outside the project is refused.
#[test]
fn a_path_written_any_way_falls_in_its_zone() {
    let r = Repo::new();
    let zone_of = |p: &str| {
        let run = r.gdd(&["--no-journal", "color", "--paths", p]);
        assert_eq!(run.code, 0, "{p}: {}", run.out);
        (run.facts["zones"].clone(), run.facts["uncovered"].clone())
    };
    let first = zone_of("src/ui/page.txt");
    assert!(
        first.0.as_array().is_some_and(|z| z.len() == 1),
        "{first:?}"
    );
    assert!(
        first.1.as_array().is_some_and(|u| u.is_empty()),
        "{first:?}"
    );
    for p in ["./src/ui/page.txt", "src/ui/", "src/ui", "src/ui/new/"] {
        assert_eq!(zone_of(p), first, "{p}");
    }
    for p in ["/etc/passwd", "../outside", "."] {
        let run = r.gdd(&["--no-journal", "color", "--paths", p]);
        assert_ne!(run.code, 0, "{p}");
    }
}
