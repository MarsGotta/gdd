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
        "#!/bin/sh\nprintf 'pass\\nCOLOR: red\\n```gdd-report\\nskill: gdd-review 0.20.0\\nresult: pass\\ncaught: none\\npart: spare\\n```\\n'\n",
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
