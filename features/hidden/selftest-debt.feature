Feature: The self-test of an adopted project counts known debt as G2 does (hidden examples)

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
      lint = "echo Found 3 errors.; false"
      unit = "test ! -f tests/failing.txt"

      [g3]
      acceptance_full = "true"

      [count]
      lint = ["Found {n} error"]

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

  Scenario: Known debt in G2 does not stop the self-test
    Given I run "gdd adopt"
    When I run "gdd selftest"
    Then it succeeds
    And the output contains "OK a failing test turns G2 red"
    And the output contains "harness 4/4"

  Scenario: A G2 command above its known debt still fails the self-test
    Given I run "gdd adopt"
    And the gate command "g2" "lint" is "echo Found 4 errors.; false"
    When I run "gdd selftest"
    Then the exit code is not 0
    And the output contains "KO a failing test turns G2 red (G2 is red without it: lint)"
