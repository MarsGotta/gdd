@hidden
Feature: Changelogs in the common heading styles (hidden examples)

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

  Scenario: A lowercase unreleased heading is the file's Unreleased
    Given I run "gdd new-change c4 --title App --paths src/app.txt"
    And I run "gdd changelog-line c4 Fixed Dates in the right time zone"
    And I run "gdd launch c4"
    And the file "CHANGELOG.md" says:
      """
      # Changes

      ## unreleased

      - A line by hand
      """
    When I run "gdd changelog --write"
    Then it succeeds
    And the file "CHANGELOG.md" does not contain "## [Unreleased]"
    And in the file "CHANGELOG.md", "- A line by hand" comes before "- Dates in the right time zone"

  Scenario: Keep a Changelog headings keep their brackets
    Given I run "gdd new-change c4 --title App --paths src/app.txt"
    And I run "gdd changelog-line c4 Added Folders inside folders"
    And I run "gdd launch c4"
    And the tag "v1.4.0" is on main
    And the file "CHANGELOG.md" says:
      """
      # Changelog

      ## [Unreleased]

      ## [1.3.2] - 2026-10-01

      ### Fixed

      - Something older
      """
    When I run "gdd changelog --write"
    Then it succeeds
    And the file "CHANGELOG.md" contains "## [1.4.0] - "
    And in the file "CHANGELOG.md", "## [1.4.0]" comes before "## [1.3.2]"

  Scenario: A release by hand keeps the v of the file
    Given main has the file "CHANGELOG.md" with:
      """
      # Changelog

      ## Unreleased

      ## v2.0.0 - 2026-10-01

      - Something older
      """
    And the tag "v2.0.0" is on main
    And I run "gdd new-change c4 --title Freeze --type Hotfix --paths src/app.txt"
    And I run "gdd launch c4"
    When I run "gdd release --yes"
    Then it succeeds
    And the file "CHANGELOG.md" contains "## v2.0.1 - "
