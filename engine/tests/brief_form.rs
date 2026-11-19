//! Check the form of a brief through the public CLI and its change log.
mod common;
use common::*;
use serde_json::json;

const FILE: &str = "features/shape.feature";
const LOG: &str = "gdd/log/form.jsonl";
const RULE: &str = "Rule: The output fits on one line";
const LEGACY: &str = "Feature: Output\n  Scenario: Original\n    Then I see one line\n";
const VALID: &str =
    "@brief:form\n@id:one-line\nScenario: One line\nWhen I read the output\nThen I see one line\n";

fn seal(r: &Repo, file: &str, text: &str) {
    r.write(file, text);
    let result = r.gdd(&["seal", "form", file]);
    assert_eq!(result.code, 0, "{}", result.out);
}

fn brief(r: &Repo) -> Run {
    r.gdd(&["brief", "form", FILE, "--paths", "src/ui/page.txt"])
}

fn refused(r: &Repo, args: &[&str]) -> Run {
    let before = r.read(LOG);
    let result = r.gdd(args);
    assert_eq!(result.code, 1, "{}", result.out);
    assert_eq!(result.facts["refused"], "scenario-form", "{}", result.out);
    assert_eq!(r.read(LOG), before);
    result
}

fn reject(r: &Repo) -> Run {
    refused(r, &["brief", "form", FILE, "--paths", "src/ui/page.txt"])
}

fn written(r: &Repo, result: &Run) {
    assert_eq!(result.code, 0, "{}", result.out);
    let last = r.log_lines("form").pop().unwrap();
    assert_eq!(last["step"], "brief");
    assert_eq!(last["files"][FILE], r.git(&["hash-object", FILE]));
}

fn diagnostic(result: &Run, location: &str, reason: &str) {
    assert!(
        result
            .out
            .lines()
            .any(|line| line.starts_with(location) && line.contains(reason)),
        "{}",
        result.out
    );
}

#[test]
fn a_new_file_needs_a_rule_and_records_the_corrected_scenario() {
    let r = Repo::new();
    seal(&r, FILE, &format!("Feature: Output\n{VALID}"));
    diagnostic(
        &reject(&r),
        "features/shape.feature:4: Scenario: One line",
        "Rule:",
    );
    seal(&r, FILE, &format!("Feature: Output\n{RULE}\n{VALID}"));
    written(&r, &brief(&r));
}

#[test]
fn a_new_scenario_without_a_brief_tag_still_needs_a_rule() {
    let r = Repo::new();
    seal(
        &r,
        FILE,
        "# language: en\nFeature: Output\n@id:first\nScenario: Untagged change\nWhen I read it\n",
    );
    diagnostic(
        &reject(&r),
        "features/shape.feature:4: Scenario: Untagged change",
        "Rule:",
    );
}

#[test]
fn a_scenario_needs_its_own_valid_id() {
    let r = Repo::new();
    for tags in ["@brief:form", "@brief:form @id:Bad"] {
        seal(
            &r,
            FILE,
            &format!("@id:rule\n{RULE}\n{tags}\nScenario: Missing id\nWhen I read @id:step\n"),
        );
        diagnostic(
            &reject(&r),
            "features/shape.feature:4: Scenario: Missing id",
            "valid @id",
        );
    }
}

#[test]
fn a_scenario_with_two_actions_names_the_count() {
    let r = Repo::new();
    seal(&r, FILE, &format!("{RULE}\n{VALID}When I read it again\n"));
    diagnostic(
        &reject(&r),
        "features/shape.feature:4: Scenario: One line",
        "one When step (found 2)",
    );
}

#[test]
fn a_scenario_with_no_action_names_the_count() {
    let r = Repo::new();
    seal(
        &r,
        FILE,
        &format!(
            "{RULE}\n@id:x\nScenario: No action\nGiven a line\nWhenever I read it\nThen I see it\n"
        ),
    );
    diagnostic(
        &reject(&r),
        "features/shape.feature:3: Scenario: No action",
        "one When step (found 0)",
    );
}

#[test]
fn one_refusal_lists_every_scenario_across_files_and_gives_the_retry() {
    let r = Repo::new();
    let second = "features/two words.feature";
    seal(&r, FILE, "@id:x\nScenario: No rule\nWhen I read it\n");
    seal(
        &r,
        second,
        &format!(
            "{RULE}\nScenario Outline: No id\nWhen I read <n>\n@id:end\nExample: No action\nThen I see it\n"
        ),
    );
    let args = [
        "brief",
        "form",
        FILE,
        second,
        "--paths",
        "src/ui/page.txt",
        "--color",
        "yellow",
        "--why",
        "two files",
    ];
    let result = refused(&r, &args);
    diagnostic(
        &result,
        "features/shape.feature:2: Scenario: No rule",
        "Rule:",
    );
    diagnostic(
        &result,
        "features/two words.feature:2: Scenario Outline: No id",
        "valid @id",
    );
    diagnostic(
        &result,
        "features/two words.feature:5: Example: No action",
        "one When step (found 0)",
    );
    let files = "features/shape.feature 'features/two words.feature'";
    assert!(result.has(&format!("gdd seal form {files}\ngdd brief form {files} --paths src/ui/page.txt --color yellow --why 'two files'")), "{}", result.out);
}

#[test]
fn an_unchanged_legacy_scenario_can_stay_above_a_new_rule() {
    let r = Repo::new();
    r.write(FILE, LEGACY);
    r.commit("Existing scenario");
    let mixed =
        format!("Feature: Output\nScenario: Original\n\nThen I see one line\n{RULE}\n{VALID}");
    seal(&r, FILE, &mixed);
    written(&r, &brief(&r));
    seal(
        &r,
        FILE,
        &mixed.replacen("Then I see one line", "Then I see two lines", 1),
    );
    let result = reject(&r);
    for reason in ["Rule:", "valid @id", "one When step (found 0)"] {
        diagnostic(
            &result,
            "features/shape.feature:2: Scenario: Original",
            reason,
        );
    }
    let retagged = mixed.replace("Scenario: Original", "@id:original\nScenario: Original");
    seal(&r, FILE, &retagged);
    diagnostic(
        &reject(&r),
        "features/shape.feature:3: Scenario: Original",
        "Rule:",
    );
}

#[test]
fn a_second_copy_of_a_legacy_scenario_is_new() {
    let r = Repo::new();
    r.write(FILE, LEGACY);
    r.commit("Existing scenario");
    seal(
        &r,
        FILE,
        &format!("{LEGACY}  Scenario: Original\n    Then I see one line\n"),
    );
    diagnostic(
        &reject(&r),
        "features/shape.feature:4: Scenario: Original",
        "Rule:",
    );
}

#[test]
fn a_file_without_changed_scenarios_gets_one_notice() {
    let r = Repo::new();
    r.write(FILE, LEGACY);
    r.commit("Existing scenario");
    seal(
        &r,
        FILE,
        &LEGACY.replace("Feature: Output", "Feature: Text"),
    );
    let result = brief(&r);
    written(&r, &result);
    assert_eq!(
        result
            .out
            .lines()
            .filter(|line| line.contains(FILE) && line.contains("no new or changed scenarios"))
            .count(),
        1,
        "{}",
        result.out
    );
}

#[test]
fn a_language_other_than_english_gets_one_notice_and_the_brief_is_written() {
    let r = Repo::new();
    seal(
        &r,
        FILE,
        "# language: es\nCaracterística: Texto\n  Escenario: Una línea\n",
    );
    let result = brief(&r);
    written(&r, &result);
    assert_eq!(
        result
            .out
            .lines()
            .filter(|line| line.contains(FILE)
                && line.contains("cannot check")
                && line.contains("language is es"))
            .count(),
        1,
        "{}",
        result.out
    );
    assert!(!result.has("no new or changed scenarios"));
}

#[test]
fn the_second_brief_compares_the_scenario_with_the_original_base() {
    let r = Repo::new();
    seal(&r, FILE, &format!("Feature: Output\n{RULE}\n{VALID}"));
    written(&r, &brief(&r));
    // The scenario block is identical on both seals. Only its rule has gone.
    seal(&r, FILE, &format!("Feature: Output\n{VALID}"));
    diagnostic(
        &reject(&r),
        "features/shape.feature:4: Scenario: One line",
        "Rule:",
    );
}

#[test]
fn a_doc_string_is_text_and_a_comment_leaves_the_tags_with_their_scenario() {
    let r = Repo::new();
    let text = "When it is ready\nExample: one line\n@all\nFeature: text";
    seal(
        &r,
        FILE,
        &format!(
            "Feature: Output\n{RULE}\n@id:one-line\n# the smallest case\nScenario: One line\nWhen I read the output\nThen I see\n\"\"\"\n{text}\n\"\"\"\n@id:two\nScenario: Another\nWhen I read it\n"
        ),
    );
    written(&r, &brief(&r));
    seal(
        &r,
        FILE,
        "Feature: Output\nBackground:\nGiven the note\n```\nRule: only text\n```\n@id:one-line\nScenario: One line\nWhen I read it\n",
    );
    diagnostic(
        &reject(&r),
        "features/shape.feature:8: Scenario: One line",
        "Rule:",
    );
}

#[test]
fn an_old_scenario_changes_when_its_examples_or_a_later_step_change() {
    let r = Repo::new();
    let old = "Feature: Output\nScenario Outline: Width\nWhen I set <n> columns\nAnd I wait\n@wide\nExamples:\n| n | lines |\n| 80 | 1 |\n";
    r.write(FILE, old);
    r.commit("Existing scenario");
    for (before, after) in [("| 80 | 1 |", "| 80 | 3 |"), ("And I wait", "And I leave")] {
        seal(&r, FILE, &old.replace(before, after));
        diagnostic(
            &reject(&r),
            "features/shape.feature:2: Scenario Outline: Width",
            "Rule:",
        );
    }
}

#[test]
fn a_file_that_is_not_utf8_is_still_read() {
    let r = Repo::new();
    std::fs::create_dir_all(r.path("features")).unwrap();
    let text = b"Feature: Caf\xe9\n@id:x\nScenario: No rule\nWhen I read it\n";
    std::fs::write(r.path(FILE), text).unwrap();
    assert_eq!(r.gdd(&["seal", "form", FILE]).code, 0);
    diagnostic(
        &reject(&r),
        "features/shape.feature:3: Scenario: No rule",
        "Rule:",
    );
}

#[test]
fn a_file_the_log_already_holds_is_checked_again_only_when_it_changes() {
    // A brief from before the layer checked its form gets one more path and keeps its seal.
    let r = Repo::new();
    let base = r.git(&["rev-parse", "HEAD"]);
    let text = "Feature: Output\n@brief:form @id:old-1\nScenario: Briefed before\nWhen I read it\n";
    let sealed = r.commit_on(
        "gdd-seal/form",
        Some(&base),
        &[(FILE, text)],
        "An older brief",
    );
    let blob = r.git(&["rev-parse", &format!("{sealed}:{FILE}")]);
    let log = json!({"contract":"gdd/1","ts":"2026-01-01T00:00:00Z","brief":"form","event":"step",
        "step":"brief","version":1,"report":{"skill":"gdd-brief","result":"green"},
        "color":{"color":"green","base":base},"seal":sealed,"files":{FILE:blob}});
    r.write(LOG, &format!("{log}\n"));
    r.write(FILE, text);
    let again = [
        "brief",
        "form",
        FILE,
        "--paths",
        "src/ui/page.txt",
        "src/ui/b.txt",
    ];
    written(&r, &r.gdd(&again));
    seal(
        &r,
        FILE,
        &text.replace("When I read it", "When I read it twice"),
    );
    diagnostic(
        &reject(&r),
        "features/shape.feature:3: Scenario: Briefed before",
        "Rule:",
    );
}

#[test]
fn only_a_feature_file_and_only_its_own_headings_are_read() {
    let r = Repo::new();
    // A brief that quotes a scenario is not a feature file.
    let md = "gdd/briefs/form.md";
    r.write(md, "# Shape\n\nScenario: quoted in the brief\n");
    // The steps under a later rule belong to no scenario, and a comment is no language header.
    seal(
        &r,
        FILE,
        &format!(
            "Feature: Output\n# language: es\n{RULE}\n{VALID}Rule: It starts once\nBackground:\nWhen the app starts\n"
        ),
    );
    let s = r.gdd(&["seal", "form", md, FILE]);
    assert_eq!(s.code, 0, "{}", s.out);
    let result = r.gdd(&["brief", "form", md, FILE, "--paths", "src/ui/page.txt"]);
    written(&r, &result);
    assert!(
        !result.has("cannot check") && !result.has(md),
        "{}",
        result.out
    );
}

#[test]
fn a_rule_with_no_text_is_no_rule() {
    let r = Repo::new();
    seal(&r, FILE, &format!("Rule:  \n{VALID}"));
    diagnostic(
        &reject(&r),
        "features/shape.feature:4: Scenario: One line",
        "Rule:",
    );
}
