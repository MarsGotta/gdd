@hidden
Feature: A ratchet for known debt (hidden examples)

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

  Scenario: Every text of a count adds up, and a text without {n} counts its lines
    Given the gate commands:
      """
      [g2]
      lint = "printf 'a.ts(1,7): error TS2322\\nb.ts(2,7): error TS2322\\n2 files would be reformatted\\n'; false"
      unit = "true"
      acceptance = "true"

      [g3]
      acceptance_full = "true"
      security = "true"

      [count]
      lint = ["error TS", "{n} files would be reformatted"]
      """
    When I run "gdd adopt"
    Then it succeeds
    And the file "gdd/debt.toml" contains "count = 4"

  Scenario: A change that adds to the known debt of G3 fails it
    Given the gate commands:
      """
      [g2]
      lint = "true"
      unit = "true"
      acceptance = "true"

      [g3]
      acceptance_full = "true"
      security = "echo Found 2 vulnerabilities; false"

      [count]
      security = ["Found {n} vulnerabilit"]
      """
    And I run "gdd adopt"
    And the gate commands:
      """
      [g2]
      lint = "true"
      unit = "true"
      acceptance = "true"

      [g3]
      acceptance_full = "true"
      security = "echo Found 3 vulnerabilities; false"

      [count]
      security = ["Found {n} vulnerabilit"]
      """
    And the change "c1" plans "src/app.rs" and its build has started on its branch
    When the change writes "src/app.rs"
    And I run "gdd g2 c1"
    And I run "gdd g3 c1"
    Then the gate fails on "security"
    And the output contains "security reports 3, the known debt is 2: 1 more"
