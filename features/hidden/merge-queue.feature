@hidden
Feature: Hidden examples for several changes at once

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

  Scenario: A file inside a parked change's folder pattern starts with a warning
    Given I run "gdd new-change c1 --title App --paths src/core/**"
    And the Builder in build mode writes nothing
    And I run "gdd launch c1"
    And I run "gdd new-change c2 --title Other --paths src/core/rules.txt"
    When I run "gdd launch c2"
    Then it succeeds
    And the output contains "overlaps c1 (parked)"
    And the output contains "src/core/rules.txt"
    And the output contains "git settles it when c2 merges"

  Scenario: A cap of 3 lets a third change start
    Given the cap of changes building at once is 3
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" is building
    And I run "gdd new-change c2 --title Other --paths src/other.txt"
    And the change "c2" is building
    And I run "gdd new-change c3 --title Third --paths src/third.txt"
    When I run "gdd launch c3"
    Then it succeeds

  Scenario: Folders with a common start but different names do not overlap
    Given I run "gdd new-change c1 --title App --paths src/lib/**"
    And the change "c1" has the scenario "features/c1.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    And I run "gdd new-change c2 --title Other --paths src/library.txt"
    When I run "gdd launch c2"
    Then it succeeds
    And the output does not contain "overlaps"
