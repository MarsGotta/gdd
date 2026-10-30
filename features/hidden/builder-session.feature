@hidden
Feature: One Builder session, by choice (hidden examples)

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

  Scenario: The replan of the Orchestrator does not join the Builder's session
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Builder session is "shared"
    And the Builder in build mode writes nothing
    When I run "gdd launch c1"
    Then it fails with a message that contains "parked: G2 still red"
    And agent call 5 starts with "-p /gdd-replan"
    And agent call 5 does not include "--resume"
    And agent call 6 includes "--resume fake-session"

  Scenario: Without a separate session set, the classification says separate
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd classify c1"
    Then it succeeds
    And the JSON field "builder_session" is "separate"
