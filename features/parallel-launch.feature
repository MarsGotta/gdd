Feature: Each change runs in its own worktree
  gdd launch runs a change in a git worktree of its own, a second working folder of the same
  repository on the change's branch, next to the project by default (<project>.worktrees/<id>).
  The main checkout stays on the main branch, so the human and the Orchestrator keep working
  there while agents work on the change. The state stays in the main checkout's .gdd/ for every
  worktree. The worktree is kept while the change is parked or waits at G4, and removed when the
  change closes or is rejected. While an agent works, gdd checks that it did not touch the state
  of its own change or the shared state; another change's records and new lines in the log are
  not its doing, so they do not park it. Running several changes at once (overlap and a cap) and
  merging them one at a time come with the merge queue.

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
      color = "red"
      paths = ["src/storage/**"]
      reason = "Losing the user's drafts"
      """
    And the models:
      """
      [orchestrator]
      integration = "claude"
      model = "opus"

      [builder.yellow]
      integration = "claude"
      model = "sonnet"

      [builder.red]
      integration = "claude"
      model = "opus"

      [builder_test]
      integration = "claude"
      model = "sonnet"

      [reviewer]
      integration = "codex"
      model = "gpt-6"

      [families]
      "opus" = "anthropic"
      "sonnet" = "anthropic"
      "gpt" = "openai"
      """
    And the gate commands:
      """
      [g2]
      unit = "test -f src/app.txt"

      [g3]
      acceptance_full = "true"

      [release]
      simulated = true
      build = "true"
      deploy_pre = "true"
      tests_pre = "true"
      demo = ""
      promote = ""
      observe = ""
      rollback = ""
      """
    And the scenario "features/base.feature" with 1 scenarios
    And everything is committed on main
    And the Setup is done
    And the red zones are approved
    And the agents are fakes

  Scenario: A launched change runs in its own worktree while the main checkout stays on main
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd launch c1"
    Then it succeeds
    And the current branch is "main"
    And the agents worked in the worktree of "c1"
    And the last commit on main says "gdd(c1): G3 Merge Gate · App"
    And the branch "main" holds "src/app.txt"

  Scenario: The worktree goes when the change closes
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd launch c1"
    Then the record of "c1" is in state "closed"
    And the change "c1" has no worktree

  Scenario: A change waiting at G4 keeps its worktree
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    When I run "gdd launch c1"
    Then the output contains "Paused at G4"
    And the change "c1" has a worktree next to the project
    And the current branch is "main"

  Scenario: Resuming from the main checkout closes the change and removes its worktree
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    When I run "gdd resume c1 --g4 ok"
    Then it succeeds
    And the record of "c1" is in state "closed"
    And the change "c1" has no worktree
    And the current branch is "main"

  Scenario: A parked change keeps its worktree
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Builder in build mode writes nothing
    When I run "gdd launch c1"
    Then the record of "c1" is in state "parked"
    And the change "c1" has a worktree next to the project
    And the current branch is "main"

  Scenario: The worktrees can live in another folder
    Given the project keeps its worktrees in "../trees"
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    When I run "gdd launch c1"
    Then the change "c1" has a worktree in "../trees/c1"

  Scenario: Launching from the change's own branch keeps the earlier work and goes back to main
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the log has a Builder call in build mode for "c1"
    And I am on the branch "change/c1"
    When the change commits "src/app.txt"
    And I run "gdd launch c1"
    Then it succeeds
    And the current branch is "main"
    And the branch "main" holds "src/app.txt"

  Scenario: An agent that writes a state folder in its worktree parks the change
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Builder in build mode writes ".gdd/changes/c1/change.json" from call 1
    When I run "gdd launch c1"
    Then it fails with a message that contains "changed GDD state"
    And the record of "c1" is in state "parked"
    And the worktree of "c1" has no ".gdd"

  Scenario: An agent that changes its own change's record parks the change
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Builder in build mode changes the record file "change.json" of the change "c1"
    When I run "gdd launch c1"
    Then it fails with a message that contains "changed GDD state"
    And the record of "c1" is in state "parked"

  Scenario: Another change's records may change while an agent works
    Given I run "gdd new-change c2 --title Other --paths src/other.txt"
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Builder in build mode changes the record file "inbox.md" of the change "c2"
    When I run "gdd launch c1"
    Then it succeeds
    And the record of "c1" is in state "closed"

  Scenario: New lines in the log while an agent works do not park the change
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the log gets a line for the change "c2" during Builder call 1
    When I run "gdd launch c1"
    Then it succeeds
    And the record of "c1" is in state "closed"

  Scenario: A Claude Code Builder in a worktree is denied the state by its full path
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd launch c1"
    Then agent call 2 includes "Edit(//"
