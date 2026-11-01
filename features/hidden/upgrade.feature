@hidden
Feature: tminus upgrade (hidden examples)

  Background:
    Given an empty folder
    And I run "tminus init --name Notes --platform cli --stack rust --agents claude codex"

  Scenario: A file the project edited keeps its changes and takes tminus's
    And an older tminus "0.0.0" wrote ".github/workflows/tminus.yml" with "fetch-depth: 1" where this one writes "fetch-depth: 0"
    And in the file ".github/workflows/tminus.yml", "components: rustfmt, clippy" is replaced by "components: rustfmt, clippy, llvm-tools"
    And everything is committed on main
    When I run "tminus upgrade"
    Then it succeeds
    And the output contains ".github/workflows/tminus.yml: merged with your changes"
    And the file ".github/workflows/tminus.yml" on the branch "tminus-upgrade" contains "fetch-depth: 0"
    And the file ".github/workflows/tminus.yml" on the branch "tminus-upgrade" contains "components: rustfmt, clippy, llvm-tools"

  Scenario: Lines both changed are marked as a conflict
    And an older tminus "0.0.0" wrote ".github/workflows/tminus.yml" with "fetch-depth: 1" where this one writes "fetch-depth: 0"
    And in the file ".github/workflows/tminus.yml", "fetch-depth: 1" is replaced by "fetch-depth: 50"
    And everything is committed on main
    When I run "tminus upgrade"
    Then it succeeds
    And the output contains ".github/workflows/tminus.yml: conflict, settle the marks"
    And the file ".github/workflows/tminus.yml" on the branch "tminus-upgrade" contains "fetch-depth: 50"
    And the file ".github/workflows/tminus.yml" contains "fetch-depth: 50"

  Scenario: A file the older tminus did not write is added
    Given an older tminus "0.0.0" did not write "docs/brief.md"
    When I run "tminus upgrade"
    Then it succeeds
    And the output contains "docs/brief.md: added"
    And the branch "tminus-upgrade" holds "docs/brief.md"
