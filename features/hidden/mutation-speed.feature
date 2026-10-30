@hidden
Feature: Faster mutation testing (hidden examples)

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
      """
    And the gate commands:
      """
      [g2]
      lint = "true"
      unit = "echo unit-ran"
      types = ""

      [g3]
      acceptance_full = "true"
      mutation = "echo mutation-ran"
      """
    And the change "c1" plans "src/app.rs" and "tests/steps/app.rs" and its build has started on its branch

  Scenario: A baseline that fails gets no second chance
    Given the gate command "g3" "mutation" is the one of the "rust" sheet
    And the program "cargo" on the PATH exits with 4, then 0
    When I run "gdd g3 c1"
    Then the gate fails on "mutation"
    And the program "cargo" was called 1 times

  Scenario: Every mutant caught at once needs one run
    Given the gate command "g3" "mutation" is the one of the "rust" sheet
    And the program "cargo" on the PATH exits with 0, then 3
    When I run "gdd g3 c1"
    Then the gate passes
    And the program "cargo" was called 1 times

  Scenario: The time limit of the rules holds in every gate
    Given the rules have "limits.command_seconds" = "1"
    And the gate command "g3" "acceptance_full" is "sleep 5"
    When I run "gdd g3 c1"
    Then the gate fails on "acceptance_full"
    And the failure of "acceptance_full" shows "timed out (1s)"
