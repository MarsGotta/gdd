@hidden
Feature: Hidden examples for the last commands of the bootstrap kit

  Background:
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["gdd/**", "bin/**", ".github/**"]
      builder_build = ["features/**"]
      builder_test = ["src/**"]
      """
    And the zone map:
      """
      default = "yellow"

      [[zone]]
      color = "green"
      paths = ["docs/**"]
      """
    And the gate commands:
      """
      [g2]
      unit = "test ! -f tests/failing.txt"

      [g3]
      acceptance_full = "true"

      [selftest]
      file = "tests/failing.txt"
      content = "fails"

      [release]
      simulated = true
      rollback_pre = ""
      """
    And the file ".github/workflows/gdd.yml" says "run: bin/gdd ci"
    And the scenario "features/base.feature" with 1 scenarios
    And everything is committed on main

  Scenario: The status shows a failed self-test
    Given the gate command "g2" "unit" is "true"
    And I run "gdd selftest"
    When I run "gdd status"
    Then the output contains "Harness self-test: failed"

  Scenario: Touching the CI workflow inside a change fails CI
    Given I run "gdd new-change c2 --title Menu --paths src/menu.txt"
    And I am on the branch "change/c2"
    When the change commits ".github/workflows/other.yml"
    And I run "gdd ci"
    Then the exit code is not 0
    And the output contains ".github/workflows/other.yml: harness path changed inside a change"

  Scenario: Two models in the conversation are two log lines
    Given the Claude Code session "s7" used "claude-opus-5-5" with 10 output tokens
    And the Claude Code session "s8" used "claude-sonnet-5" with 5 output tokens
    When I run "gdd session --phase 5-learn"
    Then the output contains "Session imported: 2 new requests across 2 model(s), phase 5-learn."

  Scenario: The inbox finds its change in GDD_CHANGE
    Given I run "gdd new-change c2 --title Menu --paths src/menu.txt"
    And the environment has "GDD_CHANGE" set to "c2"
    When I run "gdd inbox"
    Then it succeeds
