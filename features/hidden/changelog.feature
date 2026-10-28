@hidden
Feature: The changelog from the changes (hidden examples)

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

  Scenario: A migration goes under Changed
    Given I run "gdd new-change c4 --title Storage --type Migration --paths src/app.txt"
    When I run "gdd launch c4"
    Then it succeeds
    And the last commit on main says "Changelog: Changed · Storage"
    And the last commit on main says "Breaking: no"

  Scenario: A security line
    Given I run "gdd new-change c4 --title App --paths src/app.txt"
    And I run "gdd changelog-line c4 Security Notes shared by link expire after a day"
    And I run "gdd launch c4"
    When I run "gdd changelog"
    Then it succeeds
    And the file "CHANGELOG.md" contains "### Security"
    And the file "CHANGELOG.md" contains "- Notes shared by link expire after a day"

  Scenario: A refactor with a visible effect gets its line
    Given I run "gdd new-change c4 --title Tidy --type Refactor --paths src/app.txt"
    And I run "gdd changelog-line c4 Changed Notes open twice as fast"
    When I run "gdd launch c4"
    Then it succeeds
    And the last commit on main says "Changelog: Changed · Notes open twice as fast"
