@hidden
Feature: A parked change gets going again (hidden examples)

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

  Scenario: A new step file written by hand lets the relaunch skip the Builder
    Given the gate command "g3" "acceptance_full" is "grep -q split tests/steps/more.txt"
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd launch c1"
    And the file "tests/steps/more.txt" in the worktree of "c1" is written by hand saying "split in two steps"
    When I run "gdd launch c1"
    Then it succeeds
    And the agents were called in this order:
      | role          | model  | arguments                   |
      | builder-test  | sonnet | change=c1 mode=test         |
      | builder-build | sonnet | change=c1 mode=build        |
      | builder-build | sonnet | change=c1 mode=build fix=g3 |
      | reviewer      | gpt-6  | change=c1 round=1           |
    And the record of "c1" is in state "closed"
    And the file ".gdd/changes/c1/inbox.md" contains "not a Builder's work: tests/steps/more.txt"

  Scenario: Earlier work on another protected step still parks the relaunch at once
    Given the Builder in build mode also writes "tests/steps/notes.txt" with:
      """
      notes build mode wrote
      """
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd launch c1"
    When I run "gdd launch c1"
    Then it fails with a message that contains "parked: Work of the earlier run that its gates did not accept: tests/steps/notes.txt: protected path for builder-build"
    And the record of "c1" is in state "parked"

  Scenario: A red change refused for its plan leaves no worktree
    Given I run "gdd new-change c1 --title App --paths src/storage/drafts.txt"
    And I am on the branch "change/c1"
    When I run "gdd launch c1"
    Then it fails with a message that contains "a human must approve the plan"
    And the change "c1" has no worktree
    And the current branch is "main"
