@hidden
Feature: Production code written in test mode never reaches main through a relaunch (hidden examples)

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

  Scenario: Production code that test mode wrote in the Reviewer fix round and then never answered parks the relaunch
    Given the agent time limit is 4 seconds
    And I run "tminus new-change c1 --title App --paths src/app.txt"
    And the Reviewer finds "F2 · missing case · tests/steps/app.txt:7 · no step for an empty list", says KO, and then OK
    And the Builder in test mode, on its call 2, writes only "src/app.txt" and then never answers
    And I run "tminus launch c1"
    When I run "tminus launch c1"
    Then it fails with a message that contains "parked: Work of the earlier run that its gates did not accept: src/app.txt: protected path for builder-test"
    And the agents were called in this order:
      | role          | model  | arguments                      |
      | builder-test  | sonnet | change=c1 mode=test            |
      | builder-build | sonnet | change=c1 mode=build           |
      | reviewer      | gpt-6  | change=c1 round=1              |
      | builder-test  | sonnet | change=c1 mode=test fix=review |
    And the branch "main" does not hold "src/app.txt"
