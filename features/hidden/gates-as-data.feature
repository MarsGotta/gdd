@hidden
Feature: Gates are declared as data (hidden examples)

  Background:
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["tminus/**", "bin/**"]
      builder_build = ["features/**", "tests/steps/**"]
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
      unit = "true"
      lint = "true"
      dependencies = "true"
      e2e = "echo e2e-ran"

      [g3]
      acceptance_full = "true"
      mutation = "echo mutation-ran"
      mutation_fast = "echo fast-ran"
      lockfile = "true"
      """
    And the scenario "features/base.feature" with 1 scenarios

  Scenario: The commands of every round keep the order of their names across gates
    Given the change "c1" plans "src/lib.rs" and its build has started on its branch
    When I run "tminus g2 c1"
    Then the gate passes
    And the gate passed "dependencies", "e2e", "lint", "unit" and "path-guard"
    And in the file ".tminus/changes/c1/g2.json", "dependencies" comes before "e2e"
    And in the file ".tminus/changes/c1/g2.json", "e2e" comes before "lint"
    And in the file ".tminus/changes/c1/g2.json", "lint" comes before "unit"

  Scenario: A command no gate claims is checked before integrating too
    Given the change "c1" plans "src/lib.rs" and its build has started on its branch
    And the gate command "g3" "acceptance_full" is "echo full-broke-7; exit 5"
    When I run "tminus g3 c1"
    Then the gate fails on "acceptance_full"
    And the failure of "acceptance_full" shows "full-broke-7"

  Scenario: Optional commands of every full name stay out where the level does not ask for them
    Given the change "c1" plans "docs/b.md" and its build has started on its branch
    When I run "tminus g3 c1"
    Then the gate skipped "mutation"
    And the gate skipped "mutation_fast"
    And the gate passed "acceptance_full" and "lockfile"
