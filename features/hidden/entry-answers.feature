@hidden
Feature: The person answers G4 through their agent (hidden examples)

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

  Scenario: Another change waiting at G4 asks the same
    Given I run "gdd new-change c2 --title Menu --paths src/app.txt"
    And the change "c2" has the scenario "features/menu.feature" on its branch
    And I am on the branch "change/c2"
    And I run "gdd gate G1 ok c2 --by human"
    When I run "gdd launch c2"
    Then it succeeds
    And the file ".gdd/changes/c2/inbox.md" contains "then answer your agent (or the sitting screen)"
    And the file ".gdd/changes/c2/inbox.md" contains "Your agent runs gdd resume c2 --g4 ok, --g4 ko or --g4 polish --note"
