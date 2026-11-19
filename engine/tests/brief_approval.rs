//! What the person sees when asked to approve a sealed brief.
mod common;
use common::*;
use serde_json::json;

const FILE: &str = "features/lines.feature";
const LOG: &str = "gdd/log/lines.jsonl";
const PATH: &str = "src/core/app.txt";
const LINES: &str = "rule: Keep the title\nyours: Wrap long lines\nlater: Who can edit?\nread: features/lines.feature";
const RULES: &str = "Rule: Keep the title
  @id:one
  Scenario: Read one title
    When I open it
  @id:two
  Scenario: Read another title
    When I open another
@source @assumed
Rule: Wrap long lines
@question
Rule: Who can edit?
";

fn ok(r: &Repo, args: &[&str]) -> Run {
    let result = r.gdd(args);
    assert_eq!(result.code, 0, "{}", result.out);
    result
}

fn record(r: &Repo, rules: &str) {
    r.write(FILE, &format!("Feature: Lines\n{rules}"));
    ok(r, &["seal", "lines", FILE]);
    ok(r, &["brief", "lines", FILE, "--paths", PATH]);
}

fn approval_lines(result: &Run) -> String {
    result
        .out
        .lines()
        .filter(|line| {
            ["rule: ", "yours: ", "later: ", "no rule: ", "read: "]
                .iter()
                .any(|prefix| line.starts_with(prefix))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn next_shows_only_the_changed_rules_once_in_file_order() {
    let r = Repo::new();
    let old = "Rule: Keep the old spelling\n@id:old\nScenario: Old spelling\nWhen I read it\n";
    r.write(
        FILE,
        &format!("Feature: Lines\n{old}Rule: Keep the title\n"),
    );
    r.commit("Existing rules");
    record(&r, &format!("{old}{RULES}"));
    let next = ok(&r, &["next", "lines"]);
    assert_eq!(approval_lines(&next), LINES);
    let (before, after) = next.out.split_once(LINES).unwrap();
    assert!(after.starts_with("\nThese lines use 9 words."));
    for guidance in [
        "yes covers",
        "own line",
        "words and language",
        "your choice",
        "each file",
        "one question",
    ] {
        assert!(before.contains(guidance), "{before}");
    }
}

#[test]
fn next_repeats_the_lines_while_the_brief_question_is_open() {
    let r = Repo::new();
    record(&r, RULES);
    ok(&r, &["ask", "lines", "--of", "brief", "Use these rules?"]);
    let next = ok(&r, &["next", "lines"]);
    assert!(next.has("gdd-brief"));
    assert_eq!(approval_lines(&next), LINES);
}

#[test]
fn ask_shows_the_lines_only_for_a_brief_question() {
    let r = Repo::new();
    record(&r, RULES);
    let ask = |kind| ok(&r, &["ask", "lines", "--of", kind, "Use these rules?"]);
    assert_eq!(approval_lines(&ask("decision")), "");
    assert_eq!(approval_lines(&ok(&r, &["next", "lines"])), "");
    ok(&r, &["answer", "lines", "Yes"]);
    let brief = ask("brief");
    assert_eq!(approval_lines(&brief), LINES);
    assert!(brief.out.find("wrote the ask").unwrap() < brief.out.find(LINES).unwrap());
}

#[test]
fn approval_reads_the_recorded_version_after_the_seal_and_working_copy_move() {
    let r = Repo::new();
    record(&r, &RULES.replace("Keep the title", "First version"));
    record(&r, RULES);
    r.write(FILE, "Rule: A newer seal\n");
    ok(&r, &["seal", "lines", FILE]);
    r.write(FILE, "Rule: A draft\n");
    assert_eq!(approval_lines(&ok(&r, &["next", "lines"])), LINES);
}

#[test]
fn a_legacy_brief_shows_only_changed_orphans_and_names_each_file() {
    let r = Repo::new();
    let old = "Feature: Lines\nScenario: Keep this\nWhen I read it\n";
    r.write(FILE, old);
    let base = r.commit("Existing scenario");
    let text = format!("{old}Scenario Outline: Read: <n> lines\nWhen I open it\n{RULES}");
    let other = "features/more lines.feature";
    let seal = r.commit_on(
        "gdd-seal/lines",
        Some(&base),
        &[
            (FILE, &text),
            (other, "Feature: More lines\nExample: Another line\n"),
        ],
        "Legacy brief",
    );
    let blob = |path| r.git(&["rev-parse", &format!("{seal}:{path}")]);
    let log = json!({"contract":"gdd/1","ts":"2026-01-01T00:00:00Z","brief":"lines","event":"step",
        "step":"brief","version":1,"report":{"skill":"gdd-brief","result":"yellow"},"color":{"color":"yellow","base":base},
        "seal":seal,"files":{FILE:blob(FILE), other:blob(other)}});
    r.write(LOG, &format!("{log}\n"));
    let next = ok(&r, &["next", "lines"]);
    assert_eq!(
        approval_lines(&next),
        format!("no rule: Read: <n> lines\n{LINES}\nno rule: Another line\nread: {other}")
    );
    assert!(next.has("These lines use 14 words."));
}

fn word_count(words: usize) -> Run {
    let r = Repo::new();
    record(&r, &format!("Rule: {}\n", vec!["word"; words].join(" ")));
    ok(&r, &["next", "lines"])
}

#[test]
fn at_149_words_the_whole_message_must_stay_under_the_limit() {
    assert!(
        word_count(149)
            .has("149 words. Give each its own line and keep the whole message under 150 words.")
    );
}

#[test]
fn at_150_words_each_line_stays_and_none_is_merged() {
    assert!(word_count(150).has(
        "150 words, 150 or more on their own. Give each its own line: shorten them, and never drop or merge one."
    ));
}

#[test]
fn approval_needs_both_a_seal_and_a_feature_to_print_any_lines() {
    let r = Repo::new();
    record(&r, RULES);
    let mut brief = r.log_lines("lines").pop().unwrap();
    assert_eq!(approval_lines(&ok(&r, &["next", "lines"])), LINES);
    brief["seal"] = json!(null);
    r.write(LOG, &format!("{brief}\n"));

    let green = BLUEPRINT.replace(
        "checks: [seal, tests, slow, review]",
        "checks: [brief-approval, seal, tests, slow, review]",
    );
    r.write("gdd/blueprint.md", &green);
    let file = "gdd/briefs/green.md";
    r.write(file, "Keep the spelling.\n");
    ok(&r, &["seal", "green", file]);
    ok(&r, &["brief", "green", file, "--paths", "src/ui/page.txt"]);

    for id in ["lines", "green"] {
        let next = ok(&r, &["next", id]);
        assert!(next.has(&format!("run: gdd ask {id} --of brief")));
        for result in [
            next,
            ok(&r, &["ask", id, "--of", "brief", "Use these rules?"]),
            ok(&r, &["next", id]),
        ] {
            assert_eq!(approval_lines(&result), "");
            assert!(!result.has("yes covers") && !result.has("These lines use"));
        }
    }
}

#[test]
fn a_seal_written_by_hand_as_a_git_option_writes_no_file() {
    let r = Repo::new();
    record(&r, RULES);
    let mut brief = r.log_lines("lines").pop().unwrap();
    let planted = r.path("planted");
    brief["seal"] = json!(format!("--output={}", planted.display()));
    brief["files"] = json!({"x.feature": brief["files"][FILE]});
    r.write(LOG, &format!("{brief}\n"));

    let next = r.gdd(&["next", "lines"]);
    assert_eq!(approval_lines(&next), "", "{}", next.out);
    let written = std::fs::read_dir(&r.root)
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| e.file_name().to_string_lossy().starts_with("planted"));
    assert!(!written, "git took the seal as an option and wrote a file");
}

#[test]
fn a_comment_keeps_an_assumption_yours_and_a_doc_string_adds_no_rule() {
    let r = Repo::new();
    record(
        &r,
        "@assumed\n# nobody said how long\nRule: Wrap long lines\n@id:one\nScenario: Wrap\nWhen I read\n\"\"\"\nRule: only text\n\"\"\"\n",
    );
    assert_eq!(
        approval_lines(&ok(&r, &["next", "lines"])),
        format!("yours: Wrap long lines\nread: {FILE}")
    );
}

#[test]
fn an_old_rule_is_shown_when_only_the_examples_of_its_scenario_change() {
    let r = Repo::new();
    let old = "Rule: Keep the width\n@id:one\nScenario Outline: Width\nWhen I set <n>\nExamples:\n| n |\n| 80 |\n";
    r.write(FILE, &format!("Feature: Lines\n{old}"));
    r.commit("Existing rule");
    record(&r, &old.replace("| 80 |", "| 72 |"));
    assert_eq!(
        approval_lines(&ok(&r, &["next", "lines"])),
        format!("rule: Keep the width\nread: {FILE}")
    );
}

#[test]
fn the_command_that_leaves_the_approval_next_shows_the_lines_and_carries_them_in_its_facts() {
    let r = Repo::new();
    r.write(FILE, &format!("Feature: Lines\n{RULES}"));
    let seal = ok(&r, &["seal", "lines", FILE]);
    assert_eq!(approval_lines(&seal), "");
    let brief = ok(&r, &["brief", "lines", FILE, "--paths", PATH]);
    assert_eq!(approval_lines(&brief), LINES);
    let facts: Vec<&str> = brief.facts["yes_covers"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|line| line.as_str())
        .collect();
    assert_eq!(facts.join("\n"), LINES);
    // An agent that keeps only the end of the output still reads the rule about the lines.
    let said = brief.out.split("\nnext: ").next().unwrap();
    assert!(
        said.trim_end()
            .ends_with("Give each its own line and keep the whole message under 150 words."),
        "{said}"
    );
}

#[test]
fn ask_shows_the_lines_once() {
    let r = Repo::new();
    record(&r, RULES);
    let ask = ok(&r, &["ask", "lines", "--of", "brief", "Use these rules?"]);
    assert_eq!(
        ask.out.matches("Their yes covers").count(),
        1,
        "{}",
        ask.out
    );
}

#[test]
fn a_file_with_no_rule_the_layer_can_read_shows_no_lines() {
    let r = Repo::new();
    r.write(
        FILE,
        "# language: es\nCaracterística: Líneas\nRegla: Mantén el título\n",
    );
    ok(&r, &["seal", "lines", FILE]);
    let brief = ok(&r, &["brief", "lines", FILE, "--paths", PATH]);
    assert!(
        !brief.has("yes covers") && !brief.has("These lines use"),
        "{}",
        brief.out
    );
    assert!(brief.facts.get("yes_covers").is_none());
}

#[test]
fn a_new_seal_shows_no_lines_until_its_brief_is_recorded() {
    let r = Repo::new();
    record(&r, RULES);
    let whole = RULES.replace("Keep the title", "Keep the whole title");
    r.write(FILE, &format!("Feature: Lines\n{whole}"));
    let seal = ok(&r, &["seal", "lines", FILE]);
    assert_eq!(approval_lines(&seal), "");
    assert!(seal.facts.get("yes_covers").is_none());
    let brief = ok(&r, &["brief", "lines", FILE, "--paths", PATH]);
    assert_eq!(
        approval_lines(&brief),
        LINES.replace("Keep the title", "Keep the whole title")
    );
}
