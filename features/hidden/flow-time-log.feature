@hidden
Feature: Each gate command in the log (hidden examples)

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

  Scenario: Path Guard leaves its line too
    When I run "gdd g2 c1"
    Then the log has the gate command "path-guard" of "c1" in "G2" with result "ok"

  Scenario: A failing G3 command leaves a ko line
    Given the gate command "g3" "mutation" is "echo survived; exit 2"
    When I run "gdd g3 c1"
    Then the log has the gate command "mutation" of "c1" in "G3" with result "ko"
