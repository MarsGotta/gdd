//! The reading of a brief, split in three parts where the blueprint asks its color for three
//! readings: what a reader took the change to do, what it can break and what it could spare.

mod common;
use common::*;

/// The fixture's blueprint, with a red brief read in three parts.
fn split_in_three() -> String {
    let red = "  red:\n    checks: [brief-approval, seal, tests, review, person-review]\n";
    assert!(BLUEPRINT.contains(red));
    BLUEPRINT.replace(red, &format!("{red}    readings: 3\n"))
}

/// What a reader of the brief says at the end, naming its part when the reading is split.
fn reading(part: Option<&str>, body: &str) -> String {
    let text = review_text("pass", "red", body);
    match part {
        Some(part) => text.replace("caught: none", &format!("caught: none\npart: {part}")),
        None => text,
    }
}

fn next_step(r: &Repo, id: &str) -> (String, Run) {
    let n = r.gdd(&["next", id]);
    let step = n.facts["state"]["next"]["step"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    (step, n)
}

#[test]
fn a_brief_whose_color_asks_for_three_readings_is_read_in_three_parts() {
    let r = Repo::with(&split_in_three());
    yellow_brief(&r, "s1", "src/pay/pay.txt");
    for part in ["understood", "breaks", "spare"] {
        let (step, n) = next_step(&r, "s1");
        assert_eq!(step, "reading", "{}", n.out);
        assert!(n.has(&format!("`{part}` is next")), "{}", n.out);
        let launch = r.gdd(&["launch", "s1", "reading"]);
        let prompt = launch.facts["prompt"].as_str().unwrap_or_default();
        assert!(prompt.contains(&format!("part: {part}")), "{prompt}");
        let body = if part == "understood" {
            "UNDERSTOOD: The app says ok when you ask for its status, and nothing else changes.\n"
        } else {
            ""
        };
        let read = r.report("s1", "reading", &reading(Some(part), body), &[]);
        assert_eq!(read.code, 0, "{}", read.out);
    }
    let parts: Vec<String> = r
        .log_lines("s1")
        .iter()
        .filter(|v| v["step"] == "reading")
        .map(|v| v["part"].as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(parts, ["understood", "breaks", "spare"]);

    // With the three in, the person's yes is next, and what a reader understood goes first.
    let (step, n) = next_step(&r, "s1");
    assert_ne!(step, "reading", "{}", n.out);
    let understood =
        "understood: The app says ok when you ask for its status, and nothing else changes.";
    let rule = "rule: The app reports its status";
    let (first, second) = (n.out.find(understood), n.out.find(rule));
    assert!(first.is_some() && first < second, "{}", n.out);
    assert!(n.has("Give 'understood' first"), "{}", n.out);
}

#[test]
fn a_split_reading_says_which_part_it_is() {
    let r = Repo::with(&split_in_three());
    yellow_brief(&r, "s2", "src/pay/pay.txt");
    for part in [None, Some("style")] {
        let refused = r.report("s2", "reading", &reading(part, ""), &[]);
        assert_eq!(refused.facts["refused"], "no-part", "{}", refused.out);
    }
    // Any part can go first, for readers that run side by side.
    let launch = r.gdd(&["launch", "s2", "reading", "--part", "spare"]);
    let prompt = launch.facts["prompt"].as_str().unwrap_or_default();
    assert!(prompt.contains("part: spare"), "{prompt}");
    let read = r.report("s2", "reading", &reading(Some("spare"), ""), &[]);
    assert_eq!(read.code, 0, "{}", read.out);
    let twice = r.report("s2", "reading", &reading(Some("spare"), ""), &[]);
    assert_eq!(twice.facts["refused"], "out-of-order", "{}", twice.out);
    let (step, n) = next_step(&r, "s2");
    assert_eq!(step, "reading");
    assert!(n.has("`understood` is next"), "{}", n.out);
    assert_eq!(
        r.gdd(&["launch", "s2", "reading", "--part", "style"]).code,
        2
    );
}

#[test]
fn one_reading_reads_the_brief_whole_where_the_blueprint_asks_for_no_more() {
    let r = Repo::new();
    yellow_brief(&r, "s3", "src/pay/pay.txt");
    let launch = r.gdd(&["launch", "s3", "reading"]);
    let prompt = launch.facts["prompt"].as_str().unwrap_or_default();
    assert!(!prompt.is_empty() && !prompt.contains("part:"), "{prompt}");
    assert_eq!(
        r.gdd(&["launch", "s3", "reading", "--part", "spare"]).code,
        2
    );
    let read = r.report("s3", "reading", &reading(None, ""), &[]);
    assert_eq!(read.code, 0, "{}", read.out);
    let (step, n) = next_step(&r, "s3");
    assert_ne!(step, "reading", "{}", n.out);
}

/// Read the three parts of a brief, the first one saying what it understood.
fn read_in_three(r: &Repo, id: &str, understood: &str) {
    for part in ["understood", "breaks", "spare"] {
        let body = if part == "understood" {
            format!("UNDERSTOOD: {understood}\n")
        } else {
            String::new()
        };
        let read = r.report(id, "reading", &reading(Some(part), &body), &[]);
        assert_eq!(read.code, 0, "{}", read.out);
    }
}

#[test]
fn what_a_reader_understood_only_stands_for_the_version_it_read() {
    let r = Repo::with(&split_in_three());
    yellow_brief(&r, "s4", "src/pay/pay.txt");
    read_in_three(&r, "s4", "The app says ok, and nothing else changes.");
    assert!(r.gdd(&["next", "s4"]).has("understood: The app says ok"));

    // A new version, with a rule nobody read: the old paraphrase no longer speaks for it.
    let (brief, feature) = ("gdd/briefs/s4.md", "features/s4.feature");
    r.write(brief, &r.read(brief).replace("version: 1", "version: 2"));
    r.write(
        feature,
        &format!(
            "{}  Rule: Every order is refunded\n    @brief:s4 @id:s4-2\n    Scenario: A refund\n      Given an order\n      When I refund it\n      Then it is refunded\n",
            ok_feature("s4")
        ),
    );
    assert_eq!(r.gdd(&["seal", "s4", brief, feature]).code, 0);
    let b = r.gdd(&["brief", "s4", brief, feature, "--paths", "src/pay/pay.txt"]);
    assert_eq!(b.code, 0, "{}", b.out);
    let n = r.gdd(&["next", "s4"]);
    assert!(n.has("rule: Every order is refunded"), "{}", n.out);
    assert!(!n.has("understood:"), "{}", n.out);
}

#[test]
fn once_the_three_parts_are_in_another_reading_is_one_more() {
    let r = Repo::with(&split_in_three());
    yellow_brief(&r, "s5", "src/pay/pay.txt");
    read_in_three(&r, "s5", "The app says ok.");
    let launch = r.gdd(&["launch", "s5", "reading"]);
    let prompt = launch.facts["prompt"].as_str().unwrap_or_default();
    assert!(!prompt.is_empty() && !prompt.contains("part:"), "{prompt}");
    let whole = r.report("s5", "reading", &reading(None, ""), &[]);
    assert_eq!(whole.code, 0, "{}", whole.out);
    let again = r.report("s5", "reading", &reading(Some("spare"), ""), &[]);
    assert_eq!(again.code, 0, "{}", again.out);
}

#[test]
fn the_layer_knows_which_part_it_launched() {
    let r = Repo::with(&split_in_three());
    yellow_brief(&r, "s6", "src/pay/pay.txt");
    // Its reader says it read another part: the layer launched `understood`, and that is logged.
    let script = r.dir.path().join("reader.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\nprintf 'pass\\nCOLOR: red\\n```gdd-report\\nskill: gdd-review 0.22.0\\nresult: pass\\ncaught: none\\npart: spare\\n```\\n'\n",
    )
    .unwrap();
    let command = format!("sh {} <model> <prompt file>", script.display());
    let run = r.gdd(&[
        "launch",
        "s6",
        "reading",
        "--run",
        "--command",
        &command,
        "--start-wait",
        "5",
    ]);
    assert_eq!(run.code, 0, "{}", run.out);
    let log = r.log_lines("s6");
    let read = log.iter().find(|v| v["step"] == "reading").unwrap();
    assert_eq!(read["part"], "understood");
    let (step, n) = next_step(&r, "s6");
    assert_eq!(step, "reading");
    assert!(n.has("`breaks` is next"), "{}", n.out);
}

#[test]
fn a_number_of_readings_that_is_neither_one_nor_three_is_said() {
    let r = Repo::with(&split_in_three().replace("readings: 3", "readings: 2"));
    yellow_brief(&r, "s7", "src/pay/pay.txt");
    // The project's next says what is wrong with its blueprint.
    let n = r.gdd(&["next"]);
    assert!(n.has("readings 2, which is neither 1 nor 3"), "{}", n.out);
    // It reads as one reading, whole.
    let launch = r.gdd(&["launch", "s7", "reading"]);
    let prompt = launch.facts["prompt"].as_str().unwrap_or_default();
    assert!(!prompt.is_empty() && !prompt.contains("part:"), "{prompt}");
}

/// A reading that found a scenario the checks role couldn't turn into a failing test.
fn reading_with_a_gap(gap: &str) -> String {
    review_text("fix", "red", &format!("MISSING:\n- {gap}\n"))
}

/// The same brief in a new version, its scenario with one more step.
fn a_new_version(r: &Repo, id: &str, path: &str) {
    let (brief, feature) = (format!("gdd/briefs/{id}.md"), format!("features/{id}.feature"));
    r.write(&brief, &r.read(&brief).replace("version: 1", "version: 2"));
    r.write(
        &feature,
        &r.read(&feature).replace("      Given the app\n", "      Given the app is running\n"),
    );
    assert_eq!(r.gdd(&["seal", id, &brief, &feature]).code, 0);
    let b = r.gdd(&["brief", id, &brief, &feature, "--paths", path]);
    assert_eq!(b.code, 0, "{}", b.out);
}

#[test]
fn what_a_reading_left_as_missing_is_answered_before_the_person_is_asked() {
    let r = Repo::new();
    yellow_brief(&r, "m1", "src/pay/pay.txt");
    let gap = "m1-1 names no starting state";
    let read = r.report("m1", "reading", &reading_with_a_gap(gap), &[]);
    assert_eq!(read.code, 0, "{}", read.out);

    // The next step is a new version of the brief, not the person's yes.
    let (step, n) = next_step(&r, "m1");
    assert_eq!(step, "brief", "{}", n.out);
    assert!(n.has(gap) && n.has("before asking the person"), "{}", n.out);
    let lines = r.log_lines("m1").len();
    let early = r.gdd(&["ask", "m1", "--of", "brief", "Shall I go ahead?"]);
    assert_eq!(early.facts["refused"], "gaps-unanswered", "{}", early.out);
    assert!(early.has(gap), "{}", early.out);
    assert_eq!(r.log_lines("m1").len(), lines, "nothing was written");

    // The new version answers it: now the person is asked.
    a_new_version(&r, "m1", "src/pay/pay.txt");
    let (step, n) = next_step(&r, "m1");
    assert_ne!(step, "brief", "{}", n.out);
    assert!(!n.has(gap), "{}", n.out);
    let asked = r.gdd(&["ask", "m1", "--of", "brief", "Shall I go ahead?"]);
    assert_eq!(asked.code, 0, "{}", asked.out);
}

#[test]
fn a_yes_that_comes_with_a_gap_open_is_kept_and_the_gap_still_comes_first() {
    let r = Repo::new();
    yellow_brief(&r, "m2", "src/core/app.txt");
    let gap = "m2-1 names no result to observe";
    let read = review_text("fix", "yellow", &format!("MISSING:\n- {gap}\n"));
    assert_eq!(r.report("m2", "reading", &read, &[]).code, 0);
    // The yes is the person's to give, unasked too: it is written, with a word on what is left.
    let yes = r.gdd(&[
        "approve",
        "m2",
        "--of",
        "brief",
        "--who",
        "Person",
        "--where",
        "conversation",
        "gdd/briefs/m2.md",
        "features/m2.feature",
    ]);
    assert_eq!(yes.code, 0, "{}", yes.out);
    assert!(yes.has("note: the reading left") && yes.has(gap), "{}", yes.out);
    let (step, n) = next_step(&r, "m2");
    assert_eq!(step, "brief", "{}", n.out);
    assert!(n.has("before its checks"), "the yes is in: {}", n.out);
    // No step is refused for it: a log written before this rule reads as it did.
    r.commit_on(
        "gdd-seal/m2",
        None,
        &[("tests/m2.sh", "true # gdd:m2-1\n")],
        "Seal",
    );
    let sealed = r.report("m2", "checks", &sealed_report(&["tests"]), &[]);
    assert_eq!(sealed.code, 0, "{}", sealed.out);
    // Once the checks role got past it, the gap holds nothing, and a reading that comes late
    // holds nothing either.
    let (step, n) = next_step(&r, "m2");
    assert_eq!(step, "build", "{}", n.out);
    assert_eq!(r.report("m2", "reading", &read, &[]).code, 0);
    let (step, n) = next_step(&r, "m2");
    assert_eq!(step, "build", "{}", n.out);
    assert_eq!(r.gdd(&["verify", "m2"]).facts["errors"], 0);
}

#[test]
fn a_brief_logged_again_as_it_was_answers_nothing() {
    let r = Repo::new();
    yellow_brief(&r, "m4", "src/pay/pay.txt");
    let gap = "m4-1 names no starting state";
    assert_eq!(r.report("m4", "reading", &reading_with_a_gap(gap), &[]).code, 0);
    // The same gap from a second reader is said once.
    assert_eq!(r.report("m4", "reading", &reading_with_a_gap(gap), &[]).code, 0);
    let (_, n) = next_step(&r, "m4");
    assert_eq!(n.out.matches(gap).count(), 1, "{}", n.out);
    // The same files, or no file at all, are no new version.
    let (brief, feature) = ("gdd/briefs/m4.md", "features/m4.feature");
    for files in [vec![brief, feature], vec![]] {
        let mut args = vec!["brief", "m4"];
        args.extend(files);
        args.extend(["--paths", "src/pay/pay.txt"]);
        assert_eq!(r.gdd(&args).code, 0);
        let (step, n) = next_step(&r, "m4");
        assert_eq!(step, "brief", "{}", n.out);
    }
    // Asking twice all the same sends nobody to the person: the way on is still the new version.
    for _ in 0..2 {
        let early = r.gdd(&["ask", "m4", "--of", "brief", "Shall I go ahead?"]);
        assert_eq!(early.facts["refused"], "gaps-unanswered", "{}", early.out);
    }
    let (step, n) = next_step(&r, "m4");
    assert_eq!(step, "brief", "{}", n.out);
    a_new_version(&r, "m4", "src/pay/pay.txt");
    let (step, n) = next_step(&r, "m4");
    assert_ne!(step, "brief", "{}", n.out);
}

#[test]
fn a_reading_that_found_nothing_missing_holds_nothing() {
    for none in ["MISSING: none\n", "MISSING:\n- none\n", "MISSING:\n- None.\n"] {
        let r = Repo::new();
        yellow_brief(&r, "m5", "src/pay/pay.txt");
        let read = r.report("m5", "reading", &reading(None, none), &[]);
        assert_eq!(read.code, 0, "{}", read.out);
        let (step, n) = next_step(&r, "m5");
        assert_ne!(step, "brief", "{none}: {}", n.out);
        assert_eq!(r.gdd(&["ask", "m5", "--of", "brief", "Shall I go ahead?"]).code, 0);
    }
    // A change with no brief file has no version to write: a gap holds nothing there.
    let r = Repo::new();
    assert_eq!(r.gdd(&["brief", "g5", "--paths", "src/ui/page.txt"]).code, 0);
    let read = review_text("fix", "green", "MISSING:\n- nothing says what the page shows\n");
    assert_eq!(r.report("g5", "reading", &read, &[]).code, 0);
    let (step, n) = next_step(&r, "g5");
    assert_eq!(step, "build", "{}", n.out);
}

#[test]
fn where_no_yes_is_asked_a_gap_comes_before_the_checks() {
    let asks = "  yellow:\n    checks: [brief-approval, seal,";
    assert!(BLUEPRINT.contains(asks));
    let r = Repo::with(&BLUEPRINT.replace(asks, "  yellow:\n    checks: [seal,"));
    yellow_brief(&r, "m3", "src/core/app.txt");
    let (step, n) = next_step(&r, "m3");
    assert_eq!(step, "checks", "{}", n.out);
    let read = review_text("fix", "yellow", "MISSING:\n- m3-1 names no action\n");
    assert_eq!(r.report("m3", "reading", &read, &[]).code, 0);
    let (step, n) = next_step(&r, "m3");
    assert_eq!(step, "brief", "{}", n.out);
    assert!(n.has("before its checks"), "{}", n.out);
    a_new_version(&r, "m3", "src/core/app.txt");
    let (step, n) = next_step(&r, "m3");
    assert_eq!(step, "checks", "{}", n.out);
}
