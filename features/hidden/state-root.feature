@hidden
Feature: Hidden examples for one state for every worktree

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
    And the gate commands:
      """
      [g2]
      unit = "test -f only-in-the-worktree.txt"

      [g3]
      acceptance_full = "true"
      """
    And the scenario "features/base.feature" with 1 scenarios
    And the change "c1" plans "src/app.rs" and "tests/steps/app.rs" and its build has started on its branch
    And the change "c1" has a worktree with the file "only-in-the-worktree.txt"

  Scenario: An approval given in a worktree is the change's approval
    Given I run "gdd gate G1 ok c1 --by human" in the worktree of "c1"
    Then the record of "c1" has the gate "G1" with decision "ok" by "human"

  Scenario: Resuming is not run from a worktree yet
    When I run "gdd resume c1 --g4 ok" in the worktree of "c1"
    Then it fails with a message that contains "run gdd resume from the main checkout"

  Scenario: The inbox in a worktree is the same inbox
    Given I run "gdd note c1 needs A decision from the worktree" in the worktree of "c1"
    When I run "gdd inbox c1" in the worktree of "c1"
    Then the output contains "A decision from the worktree"
