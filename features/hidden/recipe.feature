@hidden
Feature: Hidden examples for gdd launch and gdd resume

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

  Scenario: A Reviewer without a readable verdict counts as KO
    Given I run "gdd new-change c2 --title Menu --paths src/app.txt"
    And the Reviewer answers "Looks fine to me" every time
    When I run "gdd launch c2"
    Then it fails with a message that contains "parked: Reviewer KO or G3 red after one fix"
    And the record of "c2" is in state "parked"

  Scenario: Polishing needs the human's notes
    Given I run "gdd new-change c2 --title Menu --paths src/app.txt"
    And the change "c2" has the scenario "features/menu.feature" on its branch
    And I am on the branch "change/c2"
    And I run "gdd gate G1 ok c2 --by human"
    And I run "gdd launch c2"
    When I run "gdd resume c2 --g4 polish"
    Then it fails with a message that contains "polish needs your notes"
    And the record of "c2" is in state "in-pre"

  Scenario: A red change gets the strong Builder and stops at G4
    Given I run "gdd new-change c2 --title Store --paths src/storage/files.txt"
    And I run "gdd gate plan ok c2 --by human"
    When I run "gdd launch c2"
    Then it succeeds
    And the output contains "Paused at G4"
    And the agents were called in this order:
      | role          | model  | arguments            |
      | builder-test  | sonnet | change=c2 mode=test  |
      | builder-build | opus   | change=c2 mode=build |
      | reviewer      | gpt-6  | change=c2 round=1    |
