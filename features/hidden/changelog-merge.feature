@hidden
Feature: The changelog is the project's (hidden examples)

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
    And the versions are released by hand

  Scenario: A security line goes after Added in a version that is there
    Given I run "gdd new-change c4 --title App --paths src/app.txt"
    And I run "gdd changelog-line c4 Security Shared links expire after a day"
    And I run "gdd launch c4"
    And the file "CHANGELOG.md" says:
      """
      # Changelog

      ## [Unreleased]

      ### Added

      - Dark theme
      """
    When I run "gdd changelog --write"
    Then it succeeds
    And the file "CHANGELOG.md" contains "- Shared links expire after a day <!-- gdd: "
    And in the file "CHANGELOG.md", "### Added" comes before "### Security"

  Scenario: A reworded breaking line still counts by its commit
    Given I run "gdd new-change c4 --title App --paths src/app.txt"
    And I run "gdd changelog-line c4 Changed --breaking Notes are stored in one file per folder"
    And I run "gdd launch c4"
    And I run "gdd changelog --write"
    And in the file "CHANGELOG.md", "Notes are stored in one file per folder" is replaced by "Every folder keeps its notes in one file"
    When I run "gdd changelog --check"
    Then it succeeds
    And the output contains "CHANGELOG.md covers every change on main"

  Scenario: A hotfix written by hand with the same words
    Given I run "gdd new-change c4 --title Freeze --type Hotfix --paths src/app.txt"
    And I run "gdd launch c4"
    And the file "CHANGELOG.md" says:
      """
      # Changelog

      ## [Unreleased]

      ### Fixed

      - Freeze
      """
    When I run "gdd changelog --check"
    Then it succeeds
    And the output contains "CHANGELOG.md covers every change on main"
