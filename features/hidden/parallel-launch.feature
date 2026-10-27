@hidden
Feature: Hidden examples for each change running in its own worktree

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

  Scenario: Resuming with KO removes the worktree too
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    When I run "gdd resume c1 --g4 ko --note Not what I asked"
    Then it succeeds
    And the record of "c1" is in state "rejected"
    And the change "c1" has no worktree

  Scenario: A parked change keeps its worktree in the chosen folder
    Given the project keeps its worktrees in "../elsewhere"
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Builder in build mode writes nothing
    When I run "gdd launch c1"
    Then the record of "c1" is in state "parked"
    And the change "c1" has a worktree in "../elsewhere/c1"

  Scenario: An agent that changes the shared state parks the change
    Given I run "gdd new-change c2 --title Other --paths src/other.txt"
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Builder in build mode changes the record file "change.json" of the change "c1"
    When I run "gdd launch c1"
    Then it fails with a message that contains "changed GDD state"
    And the record of "c2" is in state "open"
