@hidden
Feature: gdd upgrade (hidden examples)

  Background:
    Given an empty folder
    And I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"

  Scenario: A file the project edited keeps its changes and takes gdd's
    And an older gdd "0.0.0" wrote ".github/workflows/gdd.yml" with "fetch-depth: 1" where this one writes "fetch-depth: 0"
    And in the file ".github/workflows/gdd.yml", "components: rustfmt, clippy" is replaced by "components: rustfmt, clippy, llvm-tools"
    And everything is committed on main
    When I run "gdd upgrade"
    Then it succeeds
    And the output contains ".github/workflows/gdd.yml: merged with your changes"
    And the file ".github/workflows/gdd.yml" on the branch "gdd-upgrade" contains "fetch-depth: 0"
    And the file ".github/workflows/gdd.yml" on the branch "gdd-upgrade" contains "components: rustfmt, clippy, llvm-tools"

  Scenario: Lines both changed are marked as a conflict
    And an older gdd "0.0.0" wrote ".github/workflows/gdd.yml" with "fetch-depth: 1" where this one writes "fetch-depth: 0"
    And in the file ".github/workflows/gdd.yml", "fetch-depth: 1" is replaced by "fetch-depth: 50"
    And everything is committed on main
    When I run "gdd upgrade"
    Then it succeeds
    And the output contains ".github/workflows/gdd.yml: conflict, settle the marks"
    And the file ".github/workflows/gdd.yml" on the branch "gdd-upgrade" contains "fetch-depth: 50"
    And the file ".github/workflows/gdd.yml" contains "fetch-depth: 50"

  Scenario: A file the older gdd did not write is added
    Given an older gdd "0.0.0" did not write "docs/brief.md"
    When I run "gdd upgrade"
    Then it succeeds
    And the output contains "docs/brief.md: added"
    And the branch "gdd-upgrade" holds "docs/brief.md"
