@hidden
Feature: An approval holds only for the scenarios the human saw (hidden examples)

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

  Scenario: A hidden example added after G1 stops the launch before any agent call
    Given I run "gdd new-change c2 --title Notes --paths src/notes.txt"
    And the change "c2" has the scenario "features/notes.feature" on its branch
    And I run "gdd gate G1 ok c2 --by human"
    And the change "c2" has the hidden example "features/hidden/notes.feature" with the step "Then it is saved"
    When I run "gdd launch c2"
    Then it fails with a message that contains "features/hidden/notes.feature (added)"
    And it fails with a message that contains "approve the scenarios again"
    And the agents were called in this order:
      | role | model | arguments |
    And the change "c2" has no worktree

  Scenario: Two scenarios changed after G1 are both named
    Given I run "gdd new-change c2 --title Notes --paths src/notes.txt"
    And the change "c2" has the scenario "features/notes.feature" on its branch
    And the change "c2" has the scenario "features/tags.feature" on its branch
    And I run "gdd gate G1 ok c2 --by human"
    And the change "c2" no longer has the scenario "features/tags.feature" on its branch
    And the change "c2" has the scenario "features/search.feature" on its branch
    When I run "gdd launch c2"
    Then it fails with a message that contains "features/tags.feature (removed)"
    And it fails with a message that contains "features/search.feature (added)"
    And the agents were called in this order:
      | role | model | arguments |
