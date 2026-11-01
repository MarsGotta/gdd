@hidden
Feature: Hidden examples for prices come from main

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

  Scenario: Another model priced on main after the branch was made prices its session too
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the models also say:
      """
      [prices.claude-sonnet-5-5]
      input = 3.0
      cache = 0.3
      output = 15.0
      """
    And I am on the branch "change/c1"
    And the Claude Code session "s2" used "claude-sonnet-5-5" with 100 output tokens
    When I run "gdd session --phase 2-build --change c1"
    And I run "gdd report --by agent"
    Then the output contains "claude-interactive -> 1 · 10 · 5 · 100 · ~0.0015 · 0"
    And the output does not contain "unknown"

  Scenario: A model that only the branch prices stays unknown next to the prices of main
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the models also say:
      """
      [prices.claude-opus-5-5]
      input = 20.0
      cache = 2.0
      output = 50.0
      """
    And I am on the branch "change/c1"
    And the models also say:
      """
      [prices.claude-sonnet-5-5]
      input = 3.0
      cache = 0.3
      output = 15.0
      """
    And the Claude Code session "s2" used "claude-sonnet-5-5" with 40 output tokens
    When I run "gdd session --phase 2-build --change c1"
    And I run "gdd report --by agent"
    Then the output contains "claude-interactive -> 1 · 10 · 5 · 40 · unknown · 0"

  Scenario: A change launched from its own branch prices its Reviewer at the price of main
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the models also say:
      """
      [prices.gpt-6]
      input = 30.0
      cache = 2.0
      output = 40.0
      """
    And I am on the branch "change/c1"
    When I run "gdd launch c1"
    Then it succeeds
    When I run "gdd report --by role --change c1"
    Then the output contains "reviewer -> 1 · 50 · 50 · 25 · ~0.0026 · "
