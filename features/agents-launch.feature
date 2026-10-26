Feature: The recipe starts each agent as its sheet says
  The agents are the fakes of the recipe scenarios: each call is recorded with its full arguments.
  Since agent-launch, each role also gets the arguments its sheet gives that role.

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


  Scenario: Claude Code and Codex are started as their sheets say
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd launch c1"
    Then it succeeds
    And agent call 2 starts with "-p /gdd-build change=c1 mode=build --model sonnet --output-format json --strict-mcp-config"
    And agent call 3 was "exec --sandbox read-only -c mcp_servers={} $gdd-review change=c1 round=1 --model gpt-6 --json"

  Scenario: OpenCode gets the command apart from its arguments
    Given the reviewer is "opencode" "kimi-k3"
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd launch c1"
    Then it succeeds
    And agent call 3 was "run --command gdd-review -m kimi-k3 --format json change=c1 round=1"

  Scenario: A model whose agent has no sheet stops the launch
    Given the reviewer is "gemini" "g-3"
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd launch c1"
    Then it fails with a message that contains "has no agent sheet"
