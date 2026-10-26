@hidden
Feature: Hidden examples for running a gate

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
      color = "red"
      paths = ["src/storage/**"]
      """
    And the gate commands:
      """
      [g2]
      lint = "true"
      unit = "true"

      [g3]
      acceptance_full = "true"
      mutation = "true"
      """
    And the change "c1" plans "src/storage/db.rs" and its build has started on its branch

  Scenario: Commands know the color of a red change
    Given the gate command "g2" "unit" is "test \"$GDD_COLOR\" = red"
    When I run "gdd g2 c1"
    Then the gate passes

  Scenario: Two failing commands are both reported
    Given the gate command "g2" "lint" is "exit 1"
    And the gate command "g2" "unit" is "exit 2"
    When I run "gdd g2 c1"
    Then the gate fails on "lint"
    And the gate fails on "unit"

  Scenario: G3 failures are saved too
    Given the gate command "g3" "acceptance_full" is "echo nope; exit 1"
    When I run "gdd g3 c1"
    Then the failure of "acceptance_full" shows "nope"
