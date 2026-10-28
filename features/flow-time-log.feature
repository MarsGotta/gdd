Feature: Each gate command in the log
  Besides the gate's own line (`checks`), every command a gate runs leaves a line in the log with
  its name, its result and how long it took, so the time report can tell mutation testing from
  the scenarios. Empty commands are not run and leave no line.

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

  Scenario: Each command of a passing gate leaves its line
    When I run "gdd g2 c1"
    Then the gate passes
    And the log has the gate command "lint" of "c1" in "G2" with result "ok"
    And the log has the gate command "unit" of "c1" in "G2" with result "ok"
    And the log has a "G2" line for "c1" with result "ok"

  Scenario: A failing command leaves a ko line
    Given the gate command "g2" "unit" is "echo boom-42; exit 3"
    When I run "gdd g2 c1"
    Then the gate fails on "unit"
    And the log has the gate command "unit" of "c1" in "G2" with result "ko"
    And the log has the gate command "lint" of "c1" in "G2" with result "ok"

  Scenario: An empty command leaves no line
    When I run "gdd g2 c1"
    Then the log has the gate command "unit" of "c1" in "G2" with result "ok"
    And the log has no gate command "types" of "c1"

  Scenario: The commands of G3 too
    When I run "gdd g3 c1"
    Then the log has the gate command "mutation" of "c1" in "G3" with result "ok"
    And the log has the gate command "acceptance_full" of "c1" in "G3" with result "ok"

  Scenario: The time report splits the gate by its commands
    Given I run "gdd g2 c1"
    When I run "gdd report --time --change c1"
    Then it succeeds
    And the output contains "Gates: "
    And the output contains "unit"
