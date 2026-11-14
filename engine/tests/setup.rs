//! The whole life of a setup log through the layer, a git that can't write, questions before a
//! brief, and what a refusal names.

mod common;
use common::*;
use std::process::{Command, Stdio};

fn report_text(skill: &str, result: &str, body: &str) -> String {
    format!("{body}\n{}", report_block(skill, result))
}

/// Seal files on `gdd-seal/<id>`, cut from the main branch, as the checks role does.
fn seal_as_the_checks_role(
    dir: &std::path::Path,
    scratch: &std::path::Path,
    id: &str,
    files: &[(&str, &str)],
) {
    let wt = scratch.join(format!("wt-{id}"));
    let w = wt.to_string_lossy().to_string();
    let main = git(dir, &["rev-parse", "main"]);
    git(
        dir,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            &format!("gdd-seal/{id}"),
            &w,
            &main,
        ],
    );
    for (p, t) in files {
        let path = wt.join(p);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, t).unwrap();
    }
    git(&wt, &["add", "-A"]);
    git(&wt, &["commit", "-q", "-m", "Set them up"]);
    git(dir, &["worktree", "remove", "--force", &w]);
}

fn write(path: &std::path::Path, text: &str) -> String {
    std::fs::write(path, text).unwrap();
    path.to_string_lossy().to_string()
}

#[test]
fn the_standing_checks_of_the_prepared_shop_from_start_to_ship() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("shop");
    let ok = Command::new("bash")
        .arg(repo_root().join("evals/fixtures/shop.sh"))
        .args(["prepared"])
        .arg(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success();
    assert!(ok);
    let runs = tmp.path().join("runs");
    let runs = runs.to_string_lossy().to_string();
    let gdd = |args: &[&str]| run_gdd(&dir, args, &[("GDD_RUNS_DIR", &runs)]);
    let id = "standing-checks";
    // Nothing is set up yet, and no change starts first.
    let p = gdd(&["next"]);
    assert!(p.has("the standing checks are todo"), "{}", p.out);
    let n = gdd(&["next", id]);
    assert_eq!(n.facts["state"]["next"]["step"], "checks", "{}", n.out);
    let l = gdd(&["launch", id, "checks"]);
    assert_eq!(l.code, 0, "{}", l.out);
    let prompt = l.facts["prompt"].as_str().unwrap();
    assert!(
        prompt.contains("Set up the standing checks the blueprint's `checks` name"),
        "{prompt}"
    );
    assert!(
        prompt.contains(&format!("This is round 1 of checks for {id}.")),
        "{prompt}"
    );
    // The checks role sets them up and seals them; its last message goes to the layer.
    seal_as_the_checks_role(
        &dir,
        tmp.path(),
        id,
        &[
            (".github/workflows/checks.yml", "name: checks\n"),
            (".gitleaks.toml", "[allowlist]\n"),
        ],
    );
    let proofs = "```json\n[{\"check\":\"tests\",\"case\":\"rejects\",\"command\":\"python3 -m unittest discover -s tests -q\",\"exit\":1,\"tail\":\"FAILED (failures=1)\"},{\"check\":\"tests\",\"case\":\"passes\",\"command\":\"python3 -m unittest discover -s tests -q\",\"exit\":0,\"tail\":\"OK\"}]\n```";
    let f = write(
        &tmp.path().join("checks.txt"),
        &report_text("gdd-checks", "sealed", &format!("Set up.\n{proofs}")),
    );
    let c = gdd(&["report", id, "checks", &f]);
    assert_eq!(c.code, 0, "the bug: a setup log had no <start>: {}", c.out);
    let lines: Vec<serde_json::Value> =
        std::fs::read_to_string(dir.join("gdd/log/standing-checks.jsonl"))
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
    let checks = &lines[0];
    assert_eq!(checks["seal"]["checks"].as_object().unwrap().len(), 2);
    assert_eq!(checks["seal"]["proofs"].as_array().unwrap().len(), 2);
    assert_eq!(
        checks["color"]["color"], "yellow",
        "judges is yellow here, and .gitleaks.toml no zone covers"
    );
    assert_eq!(
        checks["color"]["base"],
        git(&dir, &["rev-parse", "main"]).as_str()
    );
    // The trial.
    assert_eq!(gdd(&["next", id]).facts["state"]["next"]["step"], "review");
    let l = gdd(&["launch", id, "review"]);
    assert!(
        l.facts["prompt"]
            .as_str()
            .unwrap()
            .contains("Try the new standing checks on gdd-seal/standing-checks")
    );
    let f = write(
        &tmp.path().join("review.txt"),
        &report_text(
            "gdd-review",
            "pass",
            "pass\nCOLOR: yellow\nEach check rejected its bad case and passed without it.\nLATER:\n- the secret scan could be faster",
        ),
    );
    let rv = gdd(&["report", id, "review", &f]);
    assert_eq!(rv.code, 0, "{}", rv.out);
    let review = rv.facts["next"].clone();
    assert_eq!(
        review, "ship",
        "yellow asks for no person's review: {}",
        rv.out
    );
    // It ships the way the shop's constitution says: a local merge.
    let d = gdd(&["ship", id, "--describe"]);
    assert!(d.has("It sets up checks of the project"), "{}", d.out);
    let s = gdd(&["ship", id, "--merge"]);
    assert_eq!(s.code, 0, "{}", s.out);
    assert_eq!(
        git(&dir, &["status", "--porcelain"]),
        "",
        "nothing of the layer's is left: {}",
        s.out
    );
    assert!(git(&dir, &["show", "main:.github/workflows/checks.yml"]).contains("checks"));
    assert_eq!(gdd(&["next", id]).facts["state"]["next"]["step"], "done");
    let p = gdd(&["next"]);
    assert_eq!(p.facts["standing_checks"], "done", "{}", p.out);
    assert_eq!(p.facts["backlog_next"], "discount-code", "{}", p.out);
    let v = gdd(&["verify", "--all"]);
    assert_eq!(v.facts["errors"], 0, "{}", v.out);
    assert_eq!(v.facts["warnings"], 0, "{}", v.out);
}

#[test]
fn a_zones_own_gate_with_the_persons_review_by_pull_request() {
    let r = Repo::new();
    let id = "gates-pay";
    let l = r.gdd(&["launch", id, "checks"]);
    assert!(
        l.facts["prompt"]
            .as_str()
            .unwrap()
            .contains("gdd/gates/pay.feature"),
        "{}",
        l.out
    );
    seal_as_the_checks_role(
        &r.root,
        r.dir.path(),
        id,
        &[(
            "gdd/checks/prices.sh",
            "grep -rn '[0-9]\\.[0-9]' src/pay && exit 1 || exit 0\n",
        )],
    );
    let c = r.report(
        id,
        "checks",
        &report_text("gdd-checks", "sealed", "Built the check of the Rule."),
        &[],
    );
    assert_eq!(c.code, 0, "{}", c.out);
    let r2 = r.report(
        id,
        "review",
        &report_text(
            "gdd-review",
            "pass",
            "pass\nCOLOR: red\nThe check rejects 9.95 in src/pay.",
        ),
        &[],
    );
    assert_eq!(r2.code, 0, "{}", r2.out);
    // Red asks for the person's review: there is no brief to ask about, only the change.
    let n = r.gdd(&["next", id]);
    assert_eq!(n.facts["state"]["next"]["step"], "ask", "{}", n.out);
    let b = r.gdd(&["ask", id, "--of", "brief", "Approve its brief?"]);
    assert_eq!(b.facts["refused"], "out-of-order", "{}", b.out);
    assert_eq!(
        r.gdd(&[
            "ask",
            id,
            "--of",
            "change",
            "The new gate of payments is ready: ship it?"
        ])
        .code,
        0
    );
    let a = r.gdd(&[
        "approve",
        id,
        "--of",
        "change",
        "--who",
        "Person",
        "--where",
        "conversation",
    ]);
    assert_eq!(a.code, 0, "{}", a.out);
    assert_eq!(r.gdd(&["next", id]).facts["state"]["next"]["step"], "ship");
    // By pull request: the bookkeeping, the platform's merge, then --done.
    let bk = r.gdd(&["ship", id, "--bookkeeping"]);
    assert_eq!(bk.code, 0, "{}", bk.out);
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
            "gdd-seal/gates-pay"
        ]
    ));
    let d = r.gdd(&["ship", id, "--done", "main"]);
    assert_eq!(d.code, 0, "{}", d.out);
    assert!(d.has("not something to report to the person"));
    assert_eq!(r.gdd(&["next", id]).facts["state"]["next"]["step"], "done");
    let v = r.gdd(&["verify", "--all"]);
    assert_eq!(v.facts["errors"], 0, "{}", v.out);
}

#[test]
fn nothing_is_sealed_or_built_where_git_cant_write() {
    let r = Repo::new();
    r.gdd(&["brief", "g1", "--paths", "src/ui/page.txt"]);
    r.write("gdd/briefs/y1.md", "---\nversion: 1\n---\n# Y1\n");
    let git_dir = r.path(".git");
    if !lock(&git_dir, "a-w") {
        return;
    }
    let words = "git can't write a ref here: nothing can be sealed or built";
    let b = r.gdd(&["brief", "g2", "--paths", "src/ui/page.txt"]);
    let s = r.gdd(&["seal", "y1", "gdd/briefs/y1.md"]);
    let n = r.gdd(&["next", "g1"]);
    let l = r.gdd(&["launch", "g1", "build"]);
    let fresh = r.gdd(&["next", "g3"]);
    lock(&git_dir, "u+w");
    for (what, run) in [("brief", &b), ("seal", &s), ("launch", &l)] {
        assert_eq!(run.code, 1, "{what}: {}", run.out);
        assert_eq!(run.facts["refused"], "cannot", "{what}: {}", run.out);
        assert!(run.has(words), "{what}: {}", run.out);
    }
    assert_eq!(n.code, 1, "{}", n.out);
    assert_eq!(n.facts["state"]["next"]["blocked"], "cannot", "{}", n.out);
    assert!(n.has(words), "{}", n.out);
    assert_eq!(
        fresh.facts["state"]["next"]["blocked"], "cannot",
        "{}",
        fresh.out
    );
    assert!(
        !r.path("gdd/log/g2.jsonl").exists(),
        "no brief step was written"
    );
    // Once git can write again, everything goes on.
    assert_eq!(
        r.gdd(&["next", "g1"]).facts["state"]["next"]["step"],
        "build"
    );
}

#[test]
fn no_question_about_a_brief_before_there_is_one() {
    let r = Repo::new();
    for of in ["brief", "change"] {
        let a = r.gdd(&["ask", "n1", "--of", of, "Approve it?"]);
        assert_eq!(a.code, 1, "{}", a.out);
        assert_eq!(a.facts["refused"], "out-of-order");
        assert!(
            a.has("no brief is logged for n1: run gdd brief first"),
            "{}",
            a.out
        );
        assert!(a.has("instead: gdd brief n1"), "{}", a.out);
    }
    // A question about a rule of the product is how a brief starts.
    for of in ["decision", "stop", "waiver"] {
        assert_eq!(r.gdd(&["ask", "n1", "--of", of, "Two decimals?"]).code, 0);
    }
    assert_eq!(
        r.gdd(&[
            "ask",
            "blueprint",
            "--of",
            "blueprint",
            "Approve the blueprint?"
        ])
        .code,
        0
    );
}

#[test]
fn a_refusal_names_what_is_missing() {
    let r = Repo::new();
    r.gdd(&["brief", "g1", "--paths", "src/ui/page.txt"]);
    let g = r.gdd(&["gates", "g1"]);
    // No build yet: the order comes first.
    assert_eq!(g.facts["refused"], "out-of-order", "{}", g.out);
    let base = r.git(&["rev-parse", "main"]);
    // A build reported with no branch to judge.
    let b = r.report("g1", "build", &report_block("gdd-build", "done"), &[]);
    assert_eq!(b.code, 2, "{}", b.out);
    assert_eq!(b.facts["reason"], "missing", "{}", b.out);
    assert_eq!(b.facts["what"], "the branch of g1", "{}", b.out);
    assert!(b.has("no branch for g1"), "{}", b.out);
    // Gates with a build logged and its branch gone.
    r.commit_on(
        "work/g1",
        Some(&base),
        &[("src/ui/page.txt", "p\n")],
        "Page",
    );
    assert_eq!(
        r.report("g1", "build", &report_block("gdd-build", "done"), &[])
            .code,
        0
    );
    let g = r.gdd(&["--branch", "nowhere", "gates", "g1"]);
    assert_eq!(g.facts["reason"], "missing", "{}", g.out);
    assert_eq!(g.facts["what"], "the branch of g1", "{}", g.out);
    let lim = r.gdd(&["limits", "nolog"]);
    assert_eq!(lim.facts["what"], "gdd/log/nolog.jsonl", "{}", lim.out);
    let rep = r.gdd(&["report", "g1", "build", "/no/such/message.txt"]);
    assert_eq!(rep.code, 2);
    assert_eq!(rep.facts["what"], "/no/such/message.txt", "{}", rep.out);
    let ap = r.gdd(&[
        "approve",
        "g1",
        "--of",
        "change",
        "--who",
        "P",
        "--where",
        "conversation",
        "no/such/file.txt",
    ]);
    assert_eq!(ap.facts["what"], "no/such/file.txt", "{}", ap.out);
}
