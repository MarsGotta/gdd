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
