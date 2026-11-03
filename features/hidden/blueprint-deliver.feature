@hidden
Feature: Delivering a change to pre and to production (hidden examples)

  Background:
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["tminus/**", "bin/**"]
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

  Scenario: The demo runs from the main checkout, on the main line, before the change waits for G4
    Given the gate command "release" "demo" is "test $(git branch --show-current) = main"
    And I run "tminus new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "tminus gate G1 ok c1 --by human"
    When I run "tminus launch c1"
    Then it succeeds
    And the output contains "Paused at G4"
    And the record of "c1" is in state "in-pre"

  Scenario: The human's OK releases the next version after the last one once its promotion passed
    Given the tag "v0.3.0" is on main
    And I run "tminus new-change c1 --title App --paths src/app.txt"
    And I run "tminus changelog-line c1 Fixed Crash on empty notes"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "tminus gate G1 ok c1 --by human"
    And I run "tminus launch c1"
    When I run "tminus resume c1 --g4 ok"
    Then it succeeds
    And the output contains "Released v0.3.1"
    And main has the tag "v0.3.1"

  Scenario: When production gets worse after the human's OK, the rollback has already run
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "tminus gate G1 ok c1 --by human"
    And I run "tminus launch c1"
    And the gate command "release" "observe" is "echo error rate up; exit 1"
    And the gate command "release" "rollback" is "true"
    When I run "tminus resume c1 --g4 ok"
    Then it fails with a message that contains "Rollback: ok"
    And the record of "c1" is in state "rolled-back"
    And the file ".tminus/changes/c1/inbox.md" contains "A hotfix is needed"
