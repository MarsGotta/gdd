@hidden
Feature: The sitting at G4 (hidden examples)

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

  Scenario: Polish with the human's words runs one more round
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    When I open the sitting of "c1"
    And I press p at the sitting
    And I type "Name the folder in the message" at the sitting
    And I press Enter at the sitting
    Then the sitting shows "Name the folder in the message"
    When I press y at the sitting
    And the sitting runs what was chosen
    Then it succeeds
    And the output contains "Paused at G4"

  Scenario: The demo shows its numbers
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    And the file ".gdd/changes/c1/demo/scenarios.txt" says "7 scenarios (6 passed, 1 failed)"
    When I open the sitting of "c1"
    Then the sitting shows "Demo: 7 scenarios (6 passed, 1 failed) · .gdd/changes/c1/demo/scenarios.txt"

  Scenario: KO with the human's words
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    When I open the sitting of "c1"
    And I press k at the sitting
    And I type "Keep the old names" at the sitting
    And I press Enter at the sitting
    And I press y at the sitting
    And the sitting runs what was chosen
    Then the record of "c1" is in state "rejected"
