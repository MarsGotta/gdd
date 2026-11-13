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
    let rv = r.report(
        "c1",
        "review",
        &format!("pass\nCOLOR: green\n{}", report_block("gdd-review", "pass")),
        &[],
    );
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
    assert_eq!(
        r.gdd(&["next", "c1"]).facts["state"]["next"]["step"],
        "brief"
    );
    // Judgment can't lower a color.
    let b = r.gdd(&[
        "brief",
        "c2",
        "--paths",
        "src/pay/pay.txt",
        "--color",
        "green",
    ]);
    assert_eq!(b.facts["color"], "red", "{}", b.out);
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
fn the_seal_leaves_the_checkout_and_its_index_untouched() {
    let r = Repo::new();
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
    r.report("g3", "build", &report_block("gdd-build", "done"), &[]);
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
