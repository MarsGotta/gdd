//! What the person sees when asked to approve a sealed brief.
mod common;
use common::*;
use serde_json::json;

const FILE: &str = "features/lines.feature";
const LOG: &str = "gdd/log/lines.jsonl";
const PATH: &str = "src/core/app.txt";
const LINES: &str = "rule: Keep the title\nyours: Wrap long lines\nlater: Who can edit?\nread: features/lines.feature";
const CLOSES: &str = "closes: nothing to close, it promises nothing to whoever uses it";
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
    ok(r, &["seal", "lines", &brief_md(r, "lines"), FILE]);
    ok(
        r,
        &[
            "brief",
            "lines",
            "gdd/briefs/lines.md",
            FILE,
            "--paths",
            PATH,
        ],
    );
}

fn approval_lines(result: &Run) -> String {
    result
        .out
        .lines()
        .filter(|line| {
            ["rule: ", "yours: ", "later: ", "unchecked: ", "no rule: ", "read: "]
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
    // How the change closes comes last of what the yes covers.
    let after = after.trim_start().strip_prefix(CLOSES).unwrap_or_default();
    assert!(after.trim_start().starts_with("next: "), "{after}");
    for guidance in [
        "yes covers",
        "words and language",
        "whole sentences",
        "grouping the technical ones by what they protect while naming each",
        "what closes it",
        "as yours",
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
    ok(&r, &["seal", "lines", &brief_md(&r, "lines"), FILE]);
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
}

#[test]
fn approval_needs_a_seal_and_prints_no_rule_without_a_feature() {
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
            // A brief with no feature still says how it closes, and the person approves that.
            assert_eq!(result.has("yes covers"), id == "green", "{}", result.out);
            assert_eq!(
                result.has("closes: nothing to close, it promises nothing to whoever uses it"),
                id == "green"
            );
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
fn the_command_that_leaves_the_approval_next_shows_the_lines() {
    let r = Repo::new();
    r.write(FILE, &format!("Feature: Lines\n{RULES}"));
    let seal = ok(&r, &["seal", "lines", &brief_md(&r, "lines"), FILE]);
    assert_eq!(approval_lines(&seal), "");
    let brief = ok(
        &r,
        &[
            "brief",
            "lines",
            "gdd/briefs/lines.md",
            FILE,
            "--paths",
            PATH,
        ],
    );
    assert_eq!(approval_lines(&brief), LINES);
    // The lines are the last thing said before the next step, and no count follows them.
    let said = brief.out.split("\nnext: ").next().unwrap();
    assert!(
        said.trim_end().ends_with(&format!("{LINES}\n{CLOSES}")),
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
fn a_file_with_no_rule_the_layer_can_read_shows_only_how_it_closes() {
    let r = Repo::new();
    r.write(
        FILE,
        "# language: es\nCaracterística: Líneas\nRegla: Mantén el título\n",
    );
    ok(&r, &["seal", "lines", &brief_md(&r, "lines"), FILE]);
    let brief = ok(
        &r,
        &[
            "brief",
            "lines",
            "gdd/briefs/lines.md",
            FILE,
            "--paths",
            PATH,
        ],
    );
    assert_eq!(
        approval_lines(&brief),
        "read: features/lines.feature",
        "{}",
        brief.out
    );
    assert!(brief.has(CLOSES), "{}", brief.out);
}

#[test]
fn a_new_seal_shows_no_lines_until_its_brief_is_recorded() {
    let r = Repo::new();
    record(&r, RULES);
    let whole = RULES.replace("Keep the title", "Keep the whole title");
    r.write(FILE, &format!("Feature: Lines\n{whole}"));
    let seal = ok(&r, &["seal", "lines", &brief_md(&r, "lines"), FILE]);
    assert_eq!(approval_lines(&seal), "");
    let brief = ok(
        &r,
        &[
            "brief",
            "lines",
            "gdd/briefs/lines.md",
            FILE,
            "--paths",
            PATH,
        ],
    );
    assert_eq!(
        approval_lines(&brief),
        LINES.replace("Keep the title", "Keep the whole title")
    );
}

#[test]
fn what_the_change_takes_as_true_unseen_is_shown_as_unchecked() {
    let r = Repo::new();
    record(
        &r,
        &format!("{RULES}@unverified\nRule: The carrier answers within a day\n"),
    );
    let next = ok(&r, &["next", "lines"]);
    assert_eq!(
        approval_lines(&next),
        LINES.replace(
            "read: ",
            "unchecked: The carrier answers within a day\nread: "
        )
    );
    assert!(
        next.has("Mark each 'unchecked' as something you took as true of the outside without seeing it."),
        "{}",
        next.out
    );
    // A brief that takes nothing on trust hears nothing about it.
    let plain = Repo::new();
    record(&plain, RULES);
    assert!(!ok(&plain, &["next", "lines"]).has("unchecked"));
}
