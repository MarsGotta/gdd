@hidden
Feature: Hidden examples for adopting a project that already has code

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
      """
    And the scenario "features/base.feature" with 1 scenarios

  Scenario: Dependencies come before types, and both before lint
    Given the gate commands:
      """
      [g2]
      lint = "echo long line; false"
      unit = "true"
      acceptance = "true"
      types = "echo wrong type; false"

      [g3]
      acceptance_full = "true"
      security = "true"
      dependencies = "echo GPL; false"
      """
    When I run "gdd adopt"
    Then the output contains "1. debt-dependencies"
    And the output contains "2. debt-types"
    And the output contains "3. debt-lint"

  Scenario: G3 passes with a known security debt
    Given the gate commands:
      """
      [g2]
      lint = "true"
      unit = "true"
      acceptance = "true"

      [g3]
      acceptance_full = "true"
      security = "echo old openssl; false"
      """
    And I run "gdd adopt"
    And the change "c1" plans "src/app.rs" and its build has started on its branch
    When I run "gdd g3 c1"
    Then the gate passes
    And the output contains "known_debt"

  Scenario: The merge keeps the debt that is still unpaid
    Given the gate commands:
      """
      [g2]
      lint = "echo long line; false"
      unit = "echo flaky; false"
      acceptance = "true"

      [g3]
      acceptance_full = "true"
      security = "true"
      """
    And I run "gdd adopt"
    And the gate commands:
      """
      [g2]
      lint = "true"
      unit = "echo flaky; false"
      acceptance = "true"

      [g3]
      acceptance_full = "true"
      security = "true"
      """
    And the change "c1" plans "src/app.rs" and its build has started on its branch
    When the change writes "src/app.rs"
    And I run "gdd g2 c1"
    And I run "gdd g3 c1"
    And I run "gdd gate G3 ok c1 --by machines"
    And I run "gdd merge c1"
    Then it succeeds
    And the last commit on main says "Known debt paid: lint"
    And the file "gdd/debt.toml" contains "unit"
