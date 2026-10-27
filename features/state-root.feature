Feature: One state for every worktree
  A change can run in its own git worktree, a second working folder of the same repository on
  the change's branch. gdd keeps one state for all of them: every path under .gdd/ (records,
  approvals, gate reports, the log, the binary) resolves to the main checkout, the folder that
  holds the repository itself, found through git. Project files (gdd/*.toml, features/) and the
  gate commands stay with the folder gdd runs in. Nothing changes in a single checkout. Running
  the recipe from a worktree comes with parallel launches; until then it is refused.

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

  Scenario: The status in a worktree shows the project's changes
    When I run "gdd status" in the worktree of "c1"
    Then it succeeds
    And the output contains "c1"

  Scenario: A note written from a worktree reaches the shared record
    Given I run "gdd note c1 decided Kept in the worktree" in the worktree of "c1"
    When I run "gdd inbox c1"
    Then the output contains "Kept in the worktree"

  Scenario: A gate run in a worktree runs the commands there
    When I run "gdd g2 c1" in the worktree of "c1"
    Then the gate passes

  Scenario: The gate report of a worktree run is saved in the main checkout
    Given I run "gdd g2 c1" in the worktree of "c1"
    Then the file ".gdd/changes/c1/g2.json" says the gate passed
    And the log has a "G2" line for "c1" with result "ok"

  Scenario: A worktree gets no state folder of its own
    Given I run "gdd g2 c1" in the worktree of "c1"
    Then the worktree of "c1" has no ".gdd"

  Scenario: The main checkout still runs its own gates
    When I run "gdd g2 c1"
    Then the gate fails on "unit"

  Scenario: The recipe is not run from a worktree yet
    When I run "gdd launch c1" in the worktree of "c1"
    Then it fails with a message that contains "run gdd launch from the main checkout"
