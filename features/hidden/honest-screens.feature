@hidden
Feature: The screens say what is true (hidden examples)

  Background:
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["gdd/**", "bin/**"]
      builder_build = ["features/**", "tests/steps/**"]
      builder_test = ["src/**"]
      """
    And the zone map:
      """
      default = "yellow"

      [[zone]]
      color = "green"
      paths = ["docs/**"]

      [[zone]]
      color = "red"
      paths = ["src/storage/**"]
      reason = "Losing the user's drafts"
      """
    And everything is committed on main

  Scenario: A hotfix shows the line of its type at G4
    Given I run "gdd new-change c2 --title Crash --type Hotfix --paths src/app.rs"
    And the change "c2" is in state "in-pre"
    When I open the sitting of "c2"
    Then the sitting shows "Changelog: Fixed · Crash"
    And the sitting does not show "none set"

  Scenario: A finding linked to the project shows its path, and a link elsewhere its words
    Given I run "gdd new-change c2 --title Export --paths src/export.rs"
    And the change "c2" is in state "in-pre"
    And the review of "c2" says, with its folders filled in:
      """
      - **F1 · Fixed** · [lib.rs:9](<project>/src/lib.rs:9): the export is closed.
      - **F2 · Style** · [notes](/elsewhere/notes.md): kept short.

      VERDICT: OK
      """
    When I open the sitting of "c2"
    Then the sitting shows "· src/lib.rs:9: the export is closed."
    And the sitting shows "· notes: kept short."
    And the sitting does not show "elsewhere"

  Scenario: A rejected change asks nothing more either
    Given I run "gdd new-change c2 --title Export --paths src/export.rs"
    And I run "gdd note c2 needs Which folder gets the files"
    And the change "c2" is in state "rejected"
    When I run "gdd inbox c2"
    Then it succeeds
    And the output does not contain "- Which folder gets the files"
    And the output contains "1 request(s) were still open when the change ended (kept in change.json)"

  Scenario: A change without scenarios waits for no G1
    Given I run "gdd new-change c2 --title Export --paths src/export.rs"
    When I run "gdd inbox c2"
    Then it succeeds
    And the output contains "## Needs your decision"
    And the output does not contain "G1 · Scenario Gate"
