//! Check the form of a brief through the public CLI and its change log.
mod common;
use common::*;

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
