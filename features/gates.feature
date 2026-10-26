Feature: Run a gate
  A gate runs the commands written for it in gdd/commands.toml, from the project root, and
  passes only if all of them pass. Each command gets the change's color, its scenario files
  and the files it touched. The result is saved next to the change and logged, so the
  Builder, the Reviewer and the report can read it.

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

  Scenario: Every command passes
    When I run "gdd g2 c1"
    Then the gate passes
    And the gate passed "lint", "unit" and "path-guard"

  Scenario: Empty commands are not run
    When I run "gdd g2 c1"
    Then the gate did not run "types"

  Scenario: A failing command fails the gate and keeps its output
    Given the gate command "g2" "unit" is "echo boom-42; exit 3"
    When I run "gdd g2 c1"
    Then the gate fails on "unit"
    And the failure of "unit" shows "boom-42"

  Scenario: The failure lines survive a long output
    Given the gate command "g2" "unit" is "echo 'FAILED step-7'; seq 1 5000; exit 1"
    When I run "gdd g2 c1"
    Then the failure of "unit" shows "FAILED step-7"

  Scenario: Commands know the change
    Given the gate command "g2" "unit" is "test \"$GDD_COLOR\" = yellow && test \"$GDD_CHANGE\" = c1"
    When I run "gdd g2 c1"
    Then the gate passes

  Scenario: A gate with no commands is not configured
    Given the gate command "g2" "lint" is ""
    And the gate command "g2" "unit" is ""
    When I run "gdd g2 c1"
    Then the gate fails on "G2 not configured"

  Scenario: Mutation testing is skipped when the change does not need it
    Given the change "c1" plans "docs/a.md" and its build has started on its branch
    When I run "gdd g3 c1"
    Then the gate skipped "mutation"

  Scenario: Mutation testing runs when the change needs it
    When I run "gdd g3 c1"
    Then the gate passed "mutation"

  Scenario: G3 with no scenarios fails
    When I run "gdd g3 c1"
    Then the gate fails on "scenarios"

  Scenario: G3 counts the scenarios
    Given the scenario "features/app.feature" with 2 scenarios
    When I run "gdd g3 c1"
    Then the gate passed "scenarios (2)"

  Scenario: Path Guard runs inside the gate
    When the change writes "gdd/rules.toml"
    And I run "gdd g2 c1"
    Then the gate fails on "path-guard"

  Scenario: Build mode also checks the scope
    When the change writes "src/storage/db.rs"
    And I run "gdd g2 c1 --role builder-build"
    Then the gate fails on "scope"

  Scenario: The result is saved and logged
    When I run "gdd g2 c1"
    Then the file ".gdd/changes/c1/g2.json" says the gate passed
    And the log has a "G2" line for "c1" with result "ok"

  Scenario: Only strict mode turns a red gate into an exit code
    Given the gate command "g2" "unit" is "exit 1"
    When I run "gdd g2 c1"
    Then the exit code is 0
    When I run "gdd g2 c1 --strict"
    Then the exit code is not 0
