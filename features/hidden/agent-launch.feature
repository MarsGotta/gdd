@hidden
Feature: Hidden examples for gdd starting the agents itself

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


  Scenario: Other API keys and tokens stay out too
    Given the environment has "ANTHROPIC_API_KEY" set to "sk-ant-test"
    And the environment has "GH_TOKEN" set to "gh-test"
    And I run "gdd new-change c2 --title Menu --paths src/app.txt"
    When I run "gdd launch c2"
    Then it succeeds
    And agent call 1 did not get "ANTHROPIC_API_KEY"
    And agent call 1 did not get "GH_TOKEN"

  Scenario: The test mode Builder's tokens are logged too
    Given I run "gdd new-change c2 --title Menu --paths src/app.txt"
    When I run "gdd launch c2"
    Then the log line of agent call 1 has "tokens_out" 20

  Scenario: Rewriting the log is changing GDD state
    Given I run "gdd new-change c2 --title Menu --paths src/app.txt"
    And the Builder in build mode writes ".gdd/log.jsonl" from call 1
    When I run "gdd launch c2"
    Then it fails with a message that contains "changed GDD state"
