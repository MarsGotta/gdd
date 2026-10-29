Feature: A ratchet for known debt
  A project with code adopts gdd with the debt it has. Known debt warns and does not block, but
  a change must not add to it unseen. When gdd/commands.toml says how to count what a command
  reports ([count]: texts where {n} is a number), gdd adopt records the count of each command with
  known debt, and that count is a ceiling, like a ratchet that turns one way only: a change that
  reports more fails the gate with the difference, one that reports as much warns as before, and
  one that reports less lowers the ceiling for good when it merges. A failing command whose count
  gdd cannot read fails the gate: an unread count would let anything through. Without [count],
  known debt warns as before, and gdd status says that a change can add to it unseen.

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

  Scenario: Adoption records the count of each command with known debt
    Given the gate commands:
      """
      [g2]
      lint = "echo Found 5 errors.; false"
      unit = "true"
      acceptance = "true"

      [g3]
      acceptance_full = "true"
      security = "true"

      [count]
      lint = ["Found {n} error"]
      """
    When I run "gdd adopt"
    Then it succeeds
    And the output contains "Known debt: lint (5)"
    And the file "gdd/debt.toml" contains "count = 5"

  Scenario: A change that adds to the known debt fails the gate
    Given the gate commands:
      """
      [g2]
      lint = "echo Found 5 errors.; false"
      unit = "true"
      acceptance = "true"

      [g3]
      acceptance_full = "true"
      security = "true"

      [count]
      lint = ["Found {n} error"]
      """
    And I run "gdd adopt"
    And the gate commands:
      """
      [g2]
      lint = "echo Found 7 errors.; false"
      unit = "true"
      acceptance = "true"

      [g3]
      acceptance_full = "true"
      security = "true"

      [count]
      lint = ["Found {n} error"]
      """
    And the change "c1" plans "src/app.rs" and its build has started on its branch
    When I run "gdd g2 c1"
    Then the gate fails on "lint"
    And the output contains "lint reports 7, the known debt is 5: 2 more"

  Scenario: A change that keeps the known debt as it was passes with a warning
    Given the gate commands:
      """
      [g2]
      lint = "echo Found 5 errors.; false"
      unit = "true"
      acceptance = "true"

      [g3]
      acceptance_full = "true"
      security = "true"

      [count]
      lint = ["Found {n} error"]
      """
    And I run "gdd adopt"
    And the change "c1" plans "src/app.rs" and its build has started on its branch
    When I run "gdd g2 c1"
    Then the gate passes
    And the output contains "known_debt"

  Scenario: A change that lowers the known debt lowers the ceiling when it merges
    Given the gate commands:
      """
      [g2]
      lint = "echo Found 5 errors.; false"
      unit = "true"
      acceptance = "true"

      [g3]
      acceptance_full = "true"
      security = "true"

      [count]
      lint = ["Found {n} error"]
      """
    And I run "gdd adopt"
    And the gate commands:
      """
      [g2]
      lint = "echo Found 3 errors.; false"
      unit = "true"
      acceptance = "true"

      [g3]
      acceptance_full = "true"
      security = "true"

      [count]
      lint = ["Found {n} error"]
      """
    And the change "c1" plans "src/app.rs" and its build has started on its branch
    When the change writes "src/app.rs"
    And I run "gdd g2 c1"
    And I run "gdd g3 c1"
    And I run "gdd gate G3 ok c1 --by machines"
    And I run "gdd merge c1"
    Then it succeeds
    And the last commit on main says "Known debt lowered: lint from 5 to 3"
    And the file "gdd/debt.toml" contains "count = 3"

  Scenario: A failing command whose count cannot be read fails the gate
    Given the gate commands:
      """
      [g2]
      lint = "echo Found 5 errors.; false"
      unit = "true"
      acceptance = "true"

      [g3]
      acceptance_full = "true"
      security = "true"

      [count]
      lint = ["Found {n} error"]
      """
    And I run "gdd adopt"
    And the gate commands:
      """
      [g2]
      lint = "echo panicked; false"
      unit = "true"
      acceptance = "true"

      [g3]
      acceptance_full = "true"
      security = "true"

      [count]
      lint = ["Found {n} error"]
      """
    And the change "c1" plans "src/app.rs" and its build has started on its branch
    When I run "gdd g2 c1"
    Then the gate fails on "lint"
    And the output contains "lint failed and gdd cannot count what it reported"

  Scenario: Known debt without a count warns as before, and the status says so
    Given the gate commands:
      """
      [g2]
      lint = "echo Found 5 errors.; false"
      unit = "true"
      acceptance = "true"

      [g3]
      acceptance_full = "true"
      security = "true"
      """
    And I run "gdd adopt"
    When I run "gdd status"
    Then the output contains "Known debt: lint (no count: a change can add to it unseen)"

  Scenario: A new project gets the counts of its stack
    Given an empty folder
    When I run "gdd init --name Notes --platform cli --stack python --agents claude codex"
    Then it succeeds
    And the file "gdd/commands.toml" contains "[count]"
    And the file "gdd/commands.toml" contains "Found {n} error"
