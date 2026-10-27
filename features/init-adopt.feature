Feature: Adopting a project that already has code
  A project that already has code may fail some gate commands on day one: lint, types or tests
  scan the whole project, and old debt is no change's fault. gdd adopt runs every command that
  judges the whole project once and records the ones that fail as known debt in gdd/debt.toml,
  with a Refactor proposed for each, riskiest first. A command with known debt runs and warns
  in every gate, but does not block. When a change makes it pass, the merge takes it off the
  list and from then on it blocks for good. Commands that judge only the change (mutation,
  secrets) and the scenarios never become debt: they block from day one.

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

  Scenario: Adoption records the commands that fail as known debt
    Given the gate commands:
      """
      [g2]
      lint = "echo unused import; false"
      unit = "true"
      acceptance = "true"

      [g3]
      acceptance_full = "true"
      security = "echo vulnerable qs; false"
      """
    When I run "gdd adopt"
    Then it succeeds
    And the output contains "Known debt: security, lint"
    And the file "gdd/debt.toml" contains "lint"
    And the last commit on main says "gdd adopt"

  Scenario: Adoption proposes one Refactor per command, riskiest first
    Given the gate commands:
      """
      [g2]
      lint = "echo unused import; false"
      unit = "true"
      acceptance = "true"

      [g3]
      acceptance_full = "true"
      security = "echo vulnerable qs; false"
      """
    When I run "gdd adopt"
    Then the output contains "1. debt-security"
    And the output contains "2. debt-lint"

  Scenario: What a failing command said is kept for its Refactor
    Given the gate commands:
      """
      [g2]
      lint = "echo unused import; false"
      unit = "true"
      acceptance = "true"

      [g3]
      acceptance_full = "true"
      security = "echo vulnerable qs; false"
      """
    When I run "gdd adopt"
    Then the file ".gdd/adoption/lint.txt" contains "unused import"

  Scenario: Mutation, secrets and the scenarios never become debt
    Given the gate commands:
      """
      [g2]
      lint = "true"
      acceptance = "false"

      [g3]
      acceptance_full = "false"
      secrets = "false"
      mutation = "false"
      """
    When I run "gdd adopt"
    Then it succeeds
    And the output contains "Known debt: none"
    And the file "gdd/debt.toml" does not exist

  Scenario: A project is adopted only once
    Given the gate commands:
      """
      [g2]
      lint = "echo unused import; false"
      unit = "true"
      acceptance = "true"

      [g3]
      acceptance_full = "true"
      security = "echo vulnerable qs; false"
      """
    And I run "gdd adopt"
    When I run "gdd adopt"
    Then it fails with a message that contains "already adopted"

  Scenario: A command with known debt warns but does not block
    Given the gate commands:
      """
      [g2]
      lint = "echo unused import; false"
      unit = "true"
      acceptance = "true"

      [g3]
      acceptance_full = "true"
      security = "true"
      """
    And I run "gdd adopt"
    And the change "c1" plans "src/app.rs" and its build has started on its branch
    When I run "gdd g2 c1"
    Then the gate passes
    And the output contains "known_debt"

  Scenario: A command without known debt still blocks
    Given the gate commands:
      """
      [g2]
      lint = "echo unused import; false"
      unit = "true"
      acceptance = "true"

      [g3]
      acceptance_full = "true"
      security = "true"
      """
    And I run "gdd adopt"
    And the gate commands:
      """
      [g2]
      lint = "echo unused import; false"
      unit = "false"
      acceptance = "true"

      [g3]
      acceptance_full = "true"
      security = "true"
      """
    And the change "c1" plans "src/app.rs" and its build has started on its branch
    When I run "gdd g2 c1"
    Then the gate fails on "unit"
    And the output contains "known_debt"

  Scenario: A known debt that passes now is named as paid
    Given the gate commands:
      """
      [g2]
      lint = "echo unused import; false"
      unit = "true"
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
      unit = "true"
      acceptance = "true"

      [g3]
      acceptance_full = "true"
      security = "true"
      """
    And the change "c1" plans "src/app.rs" and its build has started on its branch
    When I run "gdd g2 c1"
    Then the gate passes
    And the output contains "paid_debt"

  Scenario: The merge takes the paid debt off the list
    Given the gate commands:
      """
      [g2]
      lint = "echo unused import; false"
      unit = "true"
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
      unit = "true"
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
    And the file "gdd/debt.toml" does not exist

  Scenario: A debt list that cannot be read stops the gate
    Given the gate commands:
      """
      [g2]
      lint = "true"
      unit = "true"
      acceptance = "true"

      [g3]
      acceptance_full = "true"
      security = "true"
      """
    And the file "gdd/debt.toml" says "[[debt]"
    And the change "c1" plans "src/app.rs" and its build has started on its branch
    When I run "gdd g2 c1"
    Then the gate fails on "debt"

  Scenario: The status lists the known debt and its Refactor
    Given the gate commands:
      """
      [g2]
      lint = "echo unused import; false"
      unit = "true"
      acceptance = "true"

      [g3]
      acceptance_full = "true"
      security = "true"
      """
    And I run "gdd adopt"
    When I run "gdd status"
    Then the output contains "Known debt: lint"
    And the output contains "debt-lint"

  Scenario: The status says when there is no known debt
    When I run "gdd status"
    Then the output contains "Known debt: none"
