@hidden
Feature: A Reviewer finding written as a link names its file (hidden examples)

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

  Scenario: A link in code marks among two findings still gets both fix rounds
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the Reviewer finds "F1 · off by one · src/app.txt:2 · drops a line\nF2 · weak step · [`tests/steps/app.txt`:7](/elsewhere/c1/tests/steps/app.txt) · asserts nothing", says KO, and then OK
    When I run "tminus launch c1"
    Then it succeeds
    And the agents were called in this order:
      | role          | model  | arguments                       |
      | builder-test  | sonnet | change=c1 mode=test             |
      | builder-build | sonnet | change=c1 mode=build            |
      | reviewer      | gpt-6  | change=c1 round=1               |
      | builder-test  | sonnet | change=c1 mode=test fix=review  |
      | builder-build | sonnet | change=c1 mode=build fix=review |
      | reviewer      | gpt-6  | change=c1 round=2               |
    And the record of "c1" is in state "closed"
