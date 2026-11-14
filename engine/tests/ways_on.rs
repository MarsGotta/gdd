//! Every state a log stops in names a way on, and following it moves the work: a setup log's
//! fix round after its trial's stop, a spent cap the person can give room to, a seal touched and
//! told, checks that stopped twice, a builder's CANNOT, a dropped change. And only
//! `standing-checks` comes before every change.

mod common;
use common::*;
use std::process::Command;

fn text(skill: &str, result: &str, body: &str) -> String {
    format!("{body}\n{}", report_block(skill, result))
}

fn next_of(r: &Repo, id: &str) -> Run {
    r.gdd(&["next", id])
}

fn step_of(n: &Run) -> String {
    n.facts["state"]["next"]["step"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

fn blocked_of(n: &Run) -> String {
    n.facts["state"]["next"]["blocked"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

fn command_of(n: &Run) -> String {
    n.facts["state"]["next"]["command"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

/// The checks role sets up a check on `gdd-seal/<id>` (cut from main the first time) and reports.
fn set_up(r: &Repo, id: &str, file: &str, body: &str) {
    let seal = format!("gdd-seal/{id}");
    if git_ok(
        &r.root,
        &["rev-parse", "--verify", "-q", &format!("refs/heads/{seal}")],
    ) {
        r.commit_on(&seal, None, &[(file, body)], "Fix the checks");
    } else {
        let main = r.git(&["rev-parse", "main"]);
        r.commit_on(&seal, Some(&main), &[(file, body)], "Set them up");
    }
    let c = r.report(id, "checks", &text("gdd-checks", "sealed", "Set up."), &[]);
    assert_eq!(c.code, 0, "{}", c.out);
}

fn cannot(r: &Repo, id: &str, step: &str, skill: &str) -> Run {
    r.report(
        id,
        step,
        &text(
            skill,
            "stopped",
            "CANNOT: what it needs isn't there\nThe person decides.",
        ),
        &[],
    )
}

// ----- 1. a setup log's trial that said stop gets a fix round -----

#[test]
fn a_setup_log_gets_a_fix_round_after_its_trials_stop() {
    let r = Repo::new();
    let id = "standing-checks";
    set_up(&r, id, "tests/standing.sh", "true\n");
    assert_eq!(step_of(&next_of(&r, id)), "review");
    let rv = r.report(
        id,
        "review",
        &text(
            "gdd-review",
            "stop",
            "stop\nCOLOR: yellow\nf1 tests/standing.sh:1 it passes whatever the code does",
        ),
        &[],
    );
    assert_eq!(rv.code, 0, "{}", rv.out);
    let n = next_of(&r, id);
    assert_eq!(blocked_of(&n), "review-stop", "{}", n.out);
    assert!(
        command_of(&n).starts_with(&format!("gdd ask {id} --of stop")),
        "{}",
        n.out
    );
    // A stop is never waived: a waiver changes nothing.
    let w = r.gdd(&[
        "waiver",
        id,
        "--who",
        "Person",
        "--check",
        "review",
        "--why",
        "ship it anyway",
        "--backlog",
        id,
    ]);
    assert_eq!(w.code, 0, "{}", w.out);
    assert_eq!(blocked_of(&next_of(&r, id)), "review-stop");
    // Told, and answered: a fix round, within the room the caps give.
    r.gdd(&[
        "ask",
        id,
        "--of",
        "stop",
        "The scan passes anything: fix it?",
    ]);
    r.gdd(&["answer", id, "yes, fix it"]);
    let n = next_of(&r, id);
    assert_eq!(step_of(&n), "checks", "{}", n.out);
    assert_eq!(command_of(&n), format!("gdd launch {id} checks"));
    assert!(n.has("a fix round after the trial's stop"), "{}", n.out);
    assert_eq!(n.code, 0, "{}", n.out);
    // The checks role is told what the trial found and what the person answered.
    let l = r.gdd(&["launch", id, "checks"]);
    assert_eq!(l.code, 0, "{}", l.out);
    let prompt = l.facts["prompt"].as_str().unwrap();
    assert!(
        prompt.contains("it passes whatever the code does"),
        "{prompt}"
    );
    assert!(prompt.contains("yes, fix it"), "{prompt}");
    assert!(prompt.contains("on top of its tip"), "{prompt}");
    // Until a new trial passes, it doesn't ship.
    let early = r.gdd(&["ship", id, "--bookkeeping"]);
    assert_eq!(early.facts["refused"], "out-of-order", "{}", early.out);
    set_up(&r, id, "tests/standing.sh", "grep -q ok src/core/app.txt\n");
    assert_eq!(step_of(&next_of(&r, id)), "review");
    let rv = r.report(
        id,
        "review",
        &text(
            "gdd-review",
            "pass",
            "pass\nCOLOR: yellow\nIt rejects a broken app and passes the real one.",
        ),
        &[],
    );
    assert_eq!(rv.code, 0, "{}", rv.out);
    assert_eq!(step_of(&next_of(&r, id)), "ship");
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
            "gdd-seal/standing-checks"
        ]
    ));
    let d = r.gdd(&["ship", id, "--done", "main"]);
    assert_eq!(d.code, 0, "{}", d.out);
    assert_eq!(step_of(&next_of(&r, id)), "done");
    assert_eq!(r.gdd(&["next"]).facts["standing_checks"], "done");
    let v = r.gdd(&["verify", "--all"]);
    assert_eq!(v.facts["errors"], 0, "{}", v.out);
}

// ----- 2. a spent cap names the question to the person -----

#[test]
fn a_spent_cap_on_the_standing_checks_asks_the_person_at_once() {
    let bp = BLUEPRINT.replace(
        "  lines_per_change: 400\n",
        "  lines_per_change: 400\n  rounds_per_change: 1\n",
    );
    let r = Repo::with(&bp);
    let id = "standing-checks";
    for _ in 0..2 {
        let c = cannot(&r, id, "checks", "gdd-checks");
        assert_eq!(c.code, 0, "{}", c.out);
    }
    let n = next_of(&r, id);
    assert_eq!(blocked_of(&n), "cap-spent", "{}", n.out);
    assert!(
        n.has("every change waits for the standing checks"),
        "{}",
        n.out
    );
    let ask = format!(
        "gdd ask {id} --of waiver \"rounds: 1 of 1 used. One more round would <what it buys>. Yes?\""
    );
    assert_eq!(command_of(&n), ask, "{}", n.out);
    assert!(
        n.has(&format!(
            "gdd waiver {id} --who <person> --check rounds_per_change --why \"<their words>\" --backlog {id} --limit rounds_per_change --to 2"
        )),
        "{}",
        n.out
    );
    // A launch says the same.
    let l = r.gdd(&["launch", id, "checks"]);
    assert_eq!(l.facts["refused"], "cap-spent", "{}", l.out);
    assert!(l.has(&ask), "{}", l.out);
    // After the person's yes, the waiver raises the cap and it goes on.
    r.gdd(&[
        "ask",
        id,
        "--of",
        "waiver",
        "rounds: 1 of 1 used. One more round would set up the scan. Yes?",
    ]);
    let w = r.gdd(&[
        "waiver",
        id,
        "--who",
        "Person",
        "--check",
        "rounds_per_change",
        "--why",
        "one more round",
        "--backlog",
        id,
        "--limit",
        "rounds_per_change",
        "--to",
        "2",
    ]);
    assert_eq!(w.code, 0, "{}", w.out);
    let n = next_of(&r, id);
    assert_eq!(step_of(&n), "checks", "{}", n.out);
    assert_eq!(command_of(&n), format!("gdd launch {id} checks"));
}

#[test]
fn a_spent_cap_on_a_change_says_more_room_is_the_persons_to_give() {
    let bp = BLUEPRINT.replace(
        "      rounds_per_change: 1\n      fix_rounds_per_gate: 1\n",
        "      rounds_per_change: 1\n      fix_rounds_per_gate: 3\n",
    );
    let r = Repo::with(&bp);
    let id = "c9";
    assert_eq!(r.gdd(&["brief", id, "--paths", "src/ui/page.txt"]).code, 0);
    let base = r.git(&["rev-parse", "main"]);
    for (i, body) in ["bad\n", "still bad\n"].iter().enumerate() {
        r.commit_on(
            &format!("work/{id}"),
            (i == 0).then_some(base.as_str()),
            &[("src/core/app.txt", body)],
            "Build",
        );
        r.report(id, "build", &report_block("gdd-build", "done"), &[]);
        assert_eq!(r.gdd(&["gates", id]).code, 1);
    }
    let n = next_of(&r, id);
    assert_eq!(blocked_of(&n), "cap-spent", "{}", n.out);
    assert!(
        n.has("splitting it or leaving it in the backlog"),
        "{}",
        n.out
    );
    assert!(n.has("More room is the person's to give"), "{}", n.out);
    assert!(
        n.has(&format!(
            "gdd ask {id} --of waiver \"rounds: 1 of 1 used. One more round would <what it buys>. Yes?\""
        )),
        "{}",
        n.out
    );
    assert!(n.has("--limit rounds_per_change --to 2"), "{}", n.out);
    let w = r.gdd(&[
        "waiver",
        id,
        "--who",
        "Person",
        "--check",
        "rounds_per_change",
        "--why",
        "one more",
        "--backlog",
        "c9-rounds",
        "--limit",
        "rounds_per_change",
        "--to",
        "2",
    ]);
    assert_eq!(w.code, 0, "{}", w.out);
    assert_eq!(step_of(&next_of(&r, id)), "build");
}

// ----- 3. only standing-checks comes before every change -----

#[test]
fn only_the_standing_checks_come_before_every_change() {
    let r = Repo::new();
    r.write(
        "gdd/backlog.md",
        "# Backlog\n\n| Id | What | Color | Phase | Brief | State |\n|---|---|---|---|---|---|\n| standing-checks | The seal and the secret scan | red | 0 | | done |\n| catalog | The catalog | green | 1 | | todo |\n| standing-checks-slow | Set up the slow check, once there is a page | yellow | 1 | | todo |\n| gates-pay | The gate of payments | red | 1 | | todo |\n",
    );
    r.commit("The backlog of a project with no code yet");
    let p = r.gdd(&["next"]);
    assert_eq!(p.facts["standing_checks"], "done", "{}", p.out);
    assert_eq!(p.facts["backlog_next"], "catalog", "{}", p.out);
    assert!(!p.has("once the standing checks hold"), "{}", p.out);
    // The first change starts, and its gates run every check whose `when` it touched, or that
    // has none, whether its own setup item is done or not.
    let n = next_of(&r, "catalog");
    assert_eq!(step_of(&n), "brief", "{}", n.out);
    assert_eq!(
        r.gdd(&["brief", "catalog", "--paths", "src/ui/page.txt"])
            .code,
        0
    );
    let base = r.git(&["rev-parse", "main"]);
    r.commit_on(
        "work/catalog",
        Some(&base),
        &[("src/ui/page.txt", "the catalog\n")],
        "Build",
    );
    r.report("catalog", "build", &report_block("gdd-build", "done"), &[]);
    let g = r.gdd(&["gates", "catalog"]);
    assert_eq!(g.code, 0, "{}", g.out);
    let ran: Vec<&str> = g.facts["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["check"].as_str().unwrap())
        .collect();
    assert_eq!(ran, vec!["seal", "tests", "slow"], "{}", g.out);
    // Once it is done, the setup items come in the backlog's order, like any other.
    r.gdd(&["backlog", "done", "catalog"]);
    assert_eq!(
        r.gdd(&["next"]).facts["backlog_next"],
        "standing-checks-slow"
    );
}

// ----- 4. a setup log's seal is the checks role's -----

#[test]
fn a_setup_logs_seal_is_the_checks_roles() {
    let r = Repo::new();
    r.write("gdd/debt.md", "# Known debt\n");
    for id in ["standing-checks", "gates-pay", "standing-checks-slow"] {
        let s = r.gdd(&["seal", id, "gdd/debt.md"]);
        assert_eq!(s.facts["refused"], "out-of-order", "{id}: {}", s.out);
        assert!(s.has("checks role"), "{id}: {}", s.out);
        assert!(s.has(&format!("gdd next {id}")), "{id}: {}", s.out);
        let b = r.gdd(&["brief", id, "--paths", "gdd/debt.md"]);
        assert_eq!(b.facts["refused"], "out-of-order", "{id}: {}", b.out);
        assert!(b.has("has no brief"), "{id}: {}", b.out);
        let b = r.gdd(&["brief", id, "--stop", "drop it"]);
        assert_eq!(b.facts["refused"], "out-of-order", "{id}: {}", b.out);
        assert!(!r.path(&format!("gdd/log/{id}.jsonl")).exists(), "{id}");
        assert!(!git_ok(
            &r.root,
            &[
                "rev-parse",
                "--verify",
                "-q",
                &format!("refs/heads/gdd-seal/{id}")
            ]
        ));
    }
    // Bringing the seal up to date with the main branch stays.
    set_up(&r, "standing-checks", "tests/standing.sh", "true\n");
    r.write("src/core/other.txt", "the main branch moves\n");
    r.commit("Main moves");
    let m = r.gdd(&["seal", "standing-checks", "--merge-main"]);
    assert_eq!(m.code, 0, "{}", m.out);
}

// ----- 5. the backlog reader says what it can't read -----

#[test]
fn backlog_list_names_the_line_it_cannot_read() {
    let r = Repo::new();
    r.write(
        "gdd/backlog.md",
        "# Backlog\n\n## Phase 0\n\n| ID | What | Color | Brief | Status |\n|---|---|---|---|---|\n| **standing-checks** | Set up the checks | red | | `todo` |\n\n## Phase 1\n\n| Id | Qué | Color | Fase | Brief | Estado | Notes |\n|---|---|---|---|---|---|---|\n| `catalog-search` | Search | yellow | 1 | | todo | soon |\n| Classify Features | Classify | red | | | todo | |\n",
    );
    let l = r.gdd(&["backlog", "list"]);
    assert_eq!(l.code, 1, "{}", l.out);
    assert!(l.has("line 14"), "{}", l.out);
    assert!(l.has("\"Classify Features\" is not an id"), "{}", l.out);
    let ids: Vec<&str> = l.facts["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["standing-checks", "catalog-search"], "{}", l.out);
    let p = r.gdd(&["next"]);
    assert_eq!(p.facts["standing_checks"], "todo", "{}", p.out);
    assert!(p.has("line 14"), "{}", p.out);
    // The layer writes a renamed column where it is.
    assert_eq!(r.gdd(&["backlog", "done", "catalog-search"]).code, 0);
    assert!(
        r.read("gdd/backlog.md")
            .contains("| `catalog-search` | Search | yellow | 1 | | done | soon |"),
        "{}",
        r.read("gdd/backlog.md")
    );
}

// ----- 6. every stopped state has a way out -----

#[test]
fn every_stopped_state_names_a_way_out() {
    let r = Repo::new();
    let base = r.git(&["rev-parse", "main"]);

    // A dropped change: a new brief takes it up again.
    r.gdd(&["brief", "w1", "--paths", "src/ui/page.txt"]);
    r.gdd(&["brief", "w1", "--stop", "the person dropped it"]);
    let n = next_of(&r, "w1");
    assert_eq!(blocked_of(&n), "cannot", "{}", n.out);
    assert!(command_of(&n).starts_with("gdd brief w1"), "{}", n.out);
    r.gdd(&["brief", "w1", "--paths", "src/ui/page.txt"]);
    assert_eq!(step_of(&next_of(&r, "w1")), "build");

    // A builder's CANNOT: a new brief takes it up again.
    r.gdd(&["brief", "w2", "--paths", "src/ui/page.txt"]);
    r.commit_on("work/w2", Some(&base), &[("src/ui/page.txt", "x\n")], "Try");
    assert_eq!(cannot(&r, "w2", "build", "gdd-build").code, 0);
    let n = next_of(&r, "w2");
    assert_eq!(blocked_of(&n), "cannot", "{}", n.out);
    assert!(command_of(&n).starts_with("gdd brief w2"), "{}", n.out);
    r.gdd(&["brief", "w2", "--paths", "src/ui/page.txt"]);
    assert_eq!(step_of(&next_of(&r, "w2")), "build");

    // A touched seal: the person is told, then a fix round puts back what judges it.
    r.gdd(&["brief", "w3", "--paths", "src/ui/page.txt"]);
    r.commit_on(
        "work/w3",
        Some(&base),
        &[
            ("src/ui/page.txt", "p\n"),
            ("AGENTS.md", "# Anything goes\n"),
        ],
        "Build, and loosen what judges it",
    );
    r.report("w3", "build", &report_block("gdd-build", "done"), &[]);
    assert_eq!(r.gdd(&["gates", "w3"]).code, 1);
    let n = next_of(&r, "w3");
    assert_eq!(blocked_of(&n), "seal-broken", "{}", n.out);
    assert!(
        command_of(&n).starts_with("gdd ask w3 --of stop"),
        "{}",
        n.out
    );
    r.gdd(&[
        "ask",
        "w3",
        "--of",
        "stop",
        "The builder touched AGENTS.md: put it back?",
    ]);
    r.gdd(&["answer", "w3", "yes"]);
    let n = next_of(&r, "w3");
    assert_eq!(step_of(&n), "build", "{}", n.out);
    assert!(n.has("puts back what judges the change"), "{}", n.out);
    assert_eq!(r.gdd(&["launch", "w3", "build"]).code, 0);
    r.commit_on(
        "work/w3",
        None,
        &[("AGENTS.md", "# A project\n")],
        "Put back what judges it",
    );
    r.report("w3", "build", &report_block("gdd-build", "done"), &[]);
    assert_eq!(r.gdd(&["gates", "w3"]).code, 0);
    assert_eq!(step_of(&next_of(&r, "w3")), "review");

    // Checks that stopped twice: the person decides, then a new version of the brief.
    let id = "w4";
    r.write(
        &format!("gdd/briefs/{id}.md"),
        "---\ncontract: gdd/1\nid: w4\nversion: 1\n---\n\n# Ok\n",
    );
    r.write(&format!("features/{id}.feature"), "Feature: Ok\n");
    let files = [
        format!("gdd/briefs/{id}.md"),
        format!("features/{id}.feature"),
    ];
    r.gdd(&["seal", id, &files[0], &files[1]]);
    r.gdd(&[
        "brief",
        id,
        &files[0],
        &files[1],
        "--paths",
        "src/core/app.txt",
    ]);
    r.gdd(&[
        "approve",
        id,
        "--of",
        "brief",
        "--who",
        "P",
        "--where",
        "conversation",
        &files[1],
    ]);
    for _ in 0..2 {
        assert_eq!(cannot(&r, id, "checks", "gdd-checks").code, 0);
    }
    let n = next_of(&r, id);
    assert_eq!(blocked_of(&n), "cannot", "{}", n.out);
    assert!(
        command_of(&n).starts_with(&format!("gdd ask {id} --of decision")),
        "{}",
        n.out
    );
    r.gdd(&[
        "ask",
        id,
        "--of",
        "decision",
        "The check can't be written: change the brief?",
    ]);
    r.gdd(&["answer", id, "yes, say what the screen shows"]);
    let n = next_of(&r, id);
    assert!(
        command_of(&n).starts_with(&format!("gdd brief {id}")),
        "{}",
        n.out
    );
    assert!(n.has("a new version of the brief"), "{}", n.out);
    r.write(
        &files[0],
        "---\ncontract: gdd/1\nid: w4\nversion: 2\n---\n\n# Ok, on the screen\n",
    );
    r.gdd(&["seal", id, &files[0]]);
    r.gdd(&[
        "brief",
        id,
        &files[0],
        &files[1],
        "--paths",
        "src/core/app.txt",
    ]);
    assert_eq!(step_of(&next_of(&r, id)), "checks");

    // Where git can't write, the way on is said, and once it can, it goes on.
    let git_dir = r.path(".git");
    let lock = |mode: &str| {
        assert!(
            Command::new("chmod")
                .args(["-R", mode])
                .arg(&git_dir)
                .status()
                .unwrap()
                .success()
        );
    };
    lock("a-w");
    let n = next_of(&r, "w1");
    lock("u+w");
    assert_eq!(blocked_of(&n), "cannot", "{}", n.out);
    assert_eq!(command_of(&n), "gdd next w1", "{}", n.out);
    assert!(n.has("where git can write"), "{}", n.out);
    assert_eq!(step_of(&next_of(&r, "w1")), "build");
}

// ----- 7. every check's command gets what the layer knows -----

/// A blueprint whose green changes run a scan that takes `<start>` twice, inside double quotes,
/// as the secret scan does, and no seal check, so that a start missing reaches the scan.
fn scan_blueprint() -> String {
    let scan = r#"  - id: scan
    type: machine
    command: "git rev-parse --verify --quiet \"<start>^{commit}\" > /dev/null && printf '%s\\n' --log-opts=\"<start>..HEAD\" | grep -Eq '^--log-opts=[0-9a-f]{40}[.][.]HEAD$'"
    runs: each round
  - id: slow
"#;
    BLUEPRINT.replace("  - id: slow\n", scan).replace(
        "checks: [seal, tests, slow, review]",
        "checks: [tests, scan, review]",
    )
}

#[test]
fn every_check_gets_its_start_inside_its_own_quotes() {
    let r = Repo::with(&scan_blueprint());
    assert_eq!(
        r.gdd(&["brief", "s7", "--paths", "src/ui/page.txt"]).code,
        0
    );
    let base = r.git(&["rev-parse", "main"]);
    r.commit_on(
        "work/s7",
        Some(&base),
        &[("src/ui/page.txt", "a page\nmore\n")],
        "Build",
    );
    r.report("s7", "build", &report_block("gdd-build", "done"), &[]);
    let g = r.gdd(&["gates", "s7"]);
    assert_eq!(g.code, 0, "{}", g.out);
    let scan = &r
        .log_lines("s7")
        .into_iter()
        .rev()
        .find(|e| e["step"] == "gates")
        .unwrap()["gates"][1];
    assert_eq!(scan["check"], "scan");
    let command = scan["command"].as_str().unwrap();
    assert!(
        command.contains(&format!("\"{base}^{{commit}}\""))
            && command.contains(&format!("--log-opts=\"{base}..HEAD\"")),
        "{command}"
    );
    // Where <start> isn't there, as in a clone that has only the head, the scan fails.
    let shallow = r.dir.path().join("shallow");
    let url = format!("file://{}", r.root.display());
    assert!(
        Command::new("git")
            .args(["clone", "-q", "--depth", "1", "--branch", "work/s7", &url])
            .arg(&shallow)
            .status()
            .unwrap()
            .success()
    );
    let g = r.gdd(&["gates", "s7", "--at", &shallow.to_string_lossy()]);
    assert_eq!(g.code, 1, "{}", g.out);
    let failed: Vec<&str> = g.facts["checks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["result"] == "fail")
        .map(|c| c["check"].as_str().unwrap())
        .collect();
    assert_eq!(failed, vec!["scan"], "{}", g.out);
}

#[test]
fn a_command_with_a_placeholder_the_layer_does_not_fill_is_refused() {
    for (command, held) in [
        (
            "\"<lint and type check command from the constitution>\"",
            "<lint and type check command from the constitution>",
        ),
        ("\"git diff --quiet <start> <head>\"", "<head>"),
        (
            "\"<secret scan command from the constitution, from <start> to the head: the file, the line and the kind>\"",
            "<secret scan command from the constitution",
        ),
    ] {
        let bp = BLUEPRINT.replace(
            "    command: sh tests/run.sh\n",
            &format!("    command: {command}\n"),
        );
        let r = Repo::with(&bp);
        r.gdd(&["brief", "u7", "--paths", "src/ui/page.txt"]);
        let base = r.git(&["rev-parse", "main"]);
        r.commit_on(
            "work/u7",
            Some(&base),
            &[("src/ui/page.txt", "p\n")],
            "Build",
        );
        r.report("u7", "build", &report_block("gdd-build", "done"), &[]);
        let g = r.gdd(&["gates", "u7"]);
        assert_eq!(g.facts["refused"], "unfilled", "{command}: {}", g.out);
        assert!(g.has("the check tests"), "{}", g.out);
        assert!(g.has(held), "{}", g.out);
        assert!(
            r.log_lines("u7").iter().all(|e| e["step"] != "gates"),
            "nothing ran, nothing was written"
        );
    }
}

#[test]
fn the_standing_checks_of_a_project_with_no_code_are_the_seal_and_the_secret_scan() {
    let r = Repo::new();
    let l = r.gdd(&["launch", "standing-checks", "checks"]);
    assert!(
        !l.facts["prompt"].as_str().unwrap().contains("no code yet"),
        "{}",
        l.out
    );
    // A project with nothing but its GDD files, its agents' files and its data.
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    std::fs::create_dir_all(root.join("gdd/log")).unwrap();
    std::fs::create_dir_all(root.join("data")).unwrap();
    std::fs::write(root.join("gdd/blueprint.md"), BLUEPRINT).unwrap();
    std::fs::write(root.join("gdd/backlog.md"), BACKLOG).unwrap();
    std::fs::write(root.join("AGENTS.md"), "# A shop\n").unwrap();
    std::fs::write(root.join("data/products.json"), "[]\n").unwrap();
    git(&root, &["init", "-q", "-b", "main", "."]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "The shop, before its code"]);
    let l = run_gdd(&root, &["launch", "standing-checks", "checks"], &[]);
    let prompt = l.facts["prompt"].as_str().unwrap();
    assert!(prompt.contains("no code yet"), "{prompt}");
    assert!(
        prompt.contains("only the seal and the secret scan"),
        "{prompt}"
    );
    assert!(prompt.contains("`when`"), "{prompt}");
}

#[test]
fn a_change_is_approved_with_the_files_of_its_branch() {
    let r = Repo::new();
    r.gdd(&["brief", "a7", "--paths", "src/pay/new.txt"]);
    let base = r.git(&["rev-parse", "main"]);
    r.commit_on(
        "work/a7",
        Some(&base),
        &[("src/pay/new.txt", "a new way to pay\n")],
        "Build",
    );
    r.report("a7", "build", &report_block("gdd-build", "done"), &[]);
    let a = r.gdd(&[
        "approve",
        "a7",
        "--of",
        "change",
        "--who",
        "Person",
        "--where",
        "pull request",
        "src/pay/new.txt",
    ]);
    assert_eq!(a.code, 0, "{}", a.out);
    let blob = r.git(&["rev-parse", "work/a7:src/pay/new.txt"]);
    assert_eq!(a.facts["files"]["src/pay/new.txt"], blob.as_str());
}

// ----- 8. a setup log's fix round never loops -----

fn sent_back(r: &Repo, id: &str) {
    let rv = r.report(
        id,
        "review",
        &text(
            "gdd-review",
            "fix",
            "fix\nCOLOR: yellow\nf1 tests/run.sh:1 it doesn't run when only the browser tests change",
        ),
        &[],
    );
    assert_eq!(rv.code, 0, "{}", rv.out);
    assert_eq!(step_of(&next_of(r, id)), "checks");
}

/// In one run, the role sealed its fix, its step was voided because the fix changed what only the
/// person approves, they approved it, it reached the main branch and the seal took main in. The
/// fix was on the seal, and the layer refused the report forever: "the seal didn't move".
#[test]
fn a_fix_already_on_the_seal_after_a_merge_gets_its_new_trial() {
    let r = Repo::new();
    let id = "standing-checks-catalog";
    set_up(&r, id, "tests/standing.sh", "true\n");
    sent_back(&r, id);
    let fix = "grep -q ok src/core/app.txt && echo browser too\n";
    set_up(&r, id, "tests/run.sh", fix);
    let line = r.log_lines(id).len().to_string();
    let v = r.gdd(&[
        "correct",
        id,
        "--line",
        &line,
        "--void",
        "the role changed what only the person approves",
    ]);
    assert_eq!(v.code, 0, "{}", v.out);
    // The person approves it: the same change lands on the main branch, and the seal takes it in.
    r.write("tests/run.sh", fix);
    r.commit("Run the tests when the browser tests change");
    let m = r.gdd(&["seal", id, "--merge-main"]);
    assert_eq!(m.code, 0, "{}", m.out);
    // The role has nothing left to seal: what the trial asked for is there, and it says so.
    let c = r.report(
        id,
        "checks",
        &text("gdd-checks", "sealed", "The fix is on the seal."),
        &[],
    );
    assert_eq!(c.code, 0, "{}", c.out);
    let lines = r.log_lines(id);
    let sealed = &lines.last().unwrap()["seal"];
    assert_eq!(
        sealed["commit"],
        r.git(&["rev-parse", &format!("gdd-seal/{id}")]).as_str()
    );
    assert!(sealed["checks"]["tests/run.sh"].is_string(), "{sealed}");
    assert_eq!(step_of(&next_of(&r, id)), "review");
}

/// The fix the trial asked for can come through the main branch alone, with the person's
/// approval: the seal holds it once main is merged in, and the role seals nothing of its own.
#[test]
fn a_fix_that_came_through_the_main_branch_gets_its_new_trial() {
    let r = Repo::new();
    let id = "standing-checks-catalog";
    set_up(&r, id, "tests/standing.sh", "true\n");
    sent_back(&r, id);
    r.write(
        "tests/run.sh",
        "grep -q ok src/core/app.txt && echo browser too\n",
    );
    r.commit("Run the tests when the browser tests change");
    assert_eq!(r.gdd(&["seal", id, "--merge-main"]).code, 0);
    let c = r.report(
        id,
        "checks",
        &text("gdd-checks", "sealed", "The fix came through main."),
        &[],
    );
    assert_eq!(c.code, 0, "{}", c.out);
    assert_eq!(step_of(&next_of(&r, id)), "review");
}

/// A seal the trial already judged, with nothing new on it, is still nothing sealed.
#[test]
fn a_seal_the_trial_already_judged_is_not_a_fix() {
    let r = Repo::new();
    let id = "standing-checks-catalog";
    set_up(&r, id, "tests/standing.sh", "true\n");
    sent_back(&r, id);
    let c = r.report(
        id,
        "checks",
        &text("gdd-checks", "sealed", "Nothing to change."),
        &[],
    );
    assert_eq!(c.facts["refused"], "not-sealed", "{}", c.out);
}

/// A checks role that stops in a setup log is launched once more, and then the person decides:
/// the layer never names the same launch again and again.
#[test]
fn a_setup_logs_checks_that_stop_twice_go_to_the_person() {
    let r = Repo::new();
    let id = "standing-checks-catalog";
    set_up(&r, id, "tests/standing.sh", "true\n");
    sent_back(&r, id);
    assert_eq!(cannot(&r, id, "checks", "gdd-checks").code, 0);
    let n = next_of(&r, id);
    assert_eq!(step_of(&n), "checks", "{}", n.out);
    assert_eq!(command_of(&n), format!("gdd launch {id} checks"));
    assert_eq!(cannot(&r, id, "checks", "gdd-checks").code, 0);
    let n = next_of(&r, id);
    assert_eq!(blocked_of(&n), "cannot", "{}", n.out);
    assert!(
        command_of(&n).starts_with(&format!("gdd ask {id} --of decision")),
        "{}",
        n.out
    );
    // The project's own view says it waits for the person, not that a launch is next.
    let all = r.gdd(&["next"]);
    assert_eq!(
        all.facts["under_way"][0]["blocked"], "cannot",
        "{}",
        all.out
    );
    // Told, and answered: their answer is the way on, and a third stop goes to them again.
    r.gdd(&[
        "ask",
        id,
        "--of",
        "decision",
        "The checks role can't fix it: try once more?",
    ]);
    r.gdd(&["answer", id, "yes, once more"]);
    let n = next_of(&r, id);
    assert_eq!(step_of(&n), "checks", "{}", n.out);
    assert!(n.has("the person answered"), "{}", n.out);
    assert_eq!(cannot(&r, id, "checks", "gdd-checks").code, 0);
    assert_eq!(blocked_of(&next_of(&r, id)), "cannot");
}

/// The same holds before the first trial: a setup that can't be made isn't launched forever.
#[test]
fn a_setup_that_stops_twice_before_its_first_seal_goes_to_the_person() {
    let r = Repo::new();
    let id = "standing-checks-catalog";
    assert_eq!(cannot(&r, id, "checks", "gdd-checks").code, 0);
    assert_eq!(cannot(&r, id, "checks", "gdd-checks").code, 0);
    let n = next_of(&r, id);
    assert_eq!(blocked_of(&n), "cannot", "{}", n.out);
    assert!(
        command_of(&n).starts_with(&format!("gdd ask {id} --of decision")),
        "{}",
        n.out
    );
}
