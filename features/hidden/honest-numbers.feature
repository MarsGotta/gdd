@hidden
Feature: Hidden examples for honest numbers

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

  Scenario: Another price for the Reviewer's model gives another estimate, by model
    Given the models also say:
      """
      [prices.gpt-6]
      input = 10.0
      cache = 1.0
      output = 30.0
      """
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd launch c1"
    Then it succeeds
    And the last commit on main says "· 3 agent call(s)"
    When I run "gdd report --by model --change c1"
    Then the output contains "gpt-6 -> 1 · 50 · 50 · 25 · ~0.0013 · "

  Scenario: A Fixed line of a polished change is listed once
    Given the versions are released by hand
    And I run "gdd new-change c1 --title Crash --type Hotfix --paths src/app.txt"
    And I run "gdd changelog-line c1 Fixed Crash on empty notes"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    And I run "gdd resume c1 --g4 polish --note Shorter error messages"
    And I run "gdd resume c1 --g4 ok"
    When I run "gdd release"
    Then it succeeds
    And the output contains "Next version: v0.1.0"
    And the output contains "- Crash on empty notes" exactly 1 times

  Scenario: Another model of the conversation without a price is unknown
    Given the Claude Code session "s2" used "claude-sonnet-5" with 7 output tokens
    When I run "gdd session --phase 2-build"
    And I run "gdd report --by model"
    Then the output contains "claude-sonnet-5 -> 1 · 10 · 5 · 7 · unknown · 0"
