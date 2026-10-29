@hidden
Feature: A Reviewer that converges (hidden examples)

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

  Scenario: Every review is kept in the record
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Reviewer answers "VERDICT: OK" every time
    When I run "gdd launch c1"
    Then it succeeds
    And the record of "c1" lists the reviews "OK"

  Scenario: How many reviews a change needs
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Reviewer answers "VERDICT: OK" every time
    And I run "gdd launch c1"
    When I run "gdd report --dora"
    Then it succeeds
    And the output contains "Reviews to pass: median 1 per change · first review OK in 1 of 1 changes"
