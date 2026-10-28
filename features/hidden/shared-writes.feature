@hidden
Feature: A person's writes to shared state wait for the agents at work (hidden examples)

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

  Scenario: A third-sitting with the red zones as they are
    When I run "gdd gate zones ok c7 --by human --note third-sitting"
    Then it succeeds
    And the output contains "already approved"
    And the file ".gdd/zones.json" does not contain "third-sitting"
    And the log has a "zones" line for "c7" with result "ok"

  Scenario: A new red zone for exports waits for a short call
    Given the red zone paths become "src/storage/**" and "src/export/**"
    And an agent of another change is at work for 1 seconds
    When I run "gdd gate zones ok c7 --by human --note exports"
    Then it succeeds
    And the output contains "waiting for the agents at work to end their call"
    And the file ".gdd/zones.json" contains "src/export/**"
    And the file ".gdd/zones.json" contains "exports"

  Scenario: Opening G0 during the Builder's call of another change
    Given I run "gdd new-change c3 --title Notes --paths src/app.txt"
    And the machines open G0 again during the Builder call
    When I run "gdd launch c3"
    Then it succeeds
    And the record of "c3" is in state "closed"
    And the G0 opened during the Builder call succeeded
