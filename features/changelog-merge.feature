Feature: The changelog is the project's
  CHANGELOG.md belongs to the people of the project: gdd never removes or rewrites a word of it.
  `gdd changelog` compares it with the lines the merge commits on main carry, version by
  version, and shows what is missing: a whole version, or single lines of a version that is
  there. `gdd changelog --write` adds only that, each line with a comment naming the commit that
  carries it, so a line reworded later still counts. A line with the same words counts too.
  `gdd changelog --check` fails when something is missing, for CI.

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

  Scenario: Without a changelog, gdd shows the whole of it and writes nothing
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Added Export notes as Markdown"
    And I run "gdd launch c1"
    When I run "gdd changelog"
    Then it succeeds
    And the output contains "Missing from CHANGELOG.md"
    And the output contains "- Export notes as Markdown"
    And the output contains "gdd changelog --write adds them"
    And the file "CHANGELOG.md" does not exist

  Scenario: Writing creates it, each line with the commit that carries it
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Added Export notes as Markdown"
    And I run "gdd launch c1"
    When I run "gdd changelog --write"
    Then it succeeds
    And the output contains "CHANGELOG.md: 1 line(s) added"
    And the file "CHANGELOG.md" contains "# Changelog"
    And the file "CHANGELOG.md" contains "## [Unreleased]"
    And the file "CHANGELOG.md" contains "- Export notes as Markdown <!-- gdd: "

  Scenario: The changelog of the project keeps every word and gets only what is missing
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Added Export notes as Markdown"
    And I run "gdd launch c1"
    And the file "CHANGELOG.md" says:
      """
      # Release notes

      Written by the team.

      ## [Unreleased]

      ### Added

      - Dark theme
      """
    When I run "gdd changelog --write"
    Then it succeeds
    And the file "CHANGELOG.md" contains "# Release notes"
    And the file "CHANGELOG.md" contains "Written by the team."
    And the file "CHANGELOG.md" contains "- Export notes as Markdown <!-- gdd: "
    And in the file "CHANGELOG.md", "- Dark theme" comes before "- Export notes as Markdown"

  Scenario: A line reworded by hand still counts, by its commit
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Added Export notes as Markdown"
    And I run "gdd launch c1"
    And I run "gdd changelog --write"
    And in the file "CHANGELOG.md", "Export notes as Markdown" is replaced by "Export your notes as Markdown files"
    When I run "gdd changelog"
    Then it succeeds
    And the output contains "CHANGELOG.md covers every change on main"
    And the file "CHANGELOG.md" does not contain "- Export notes as Markdown"

  Scenario: The same words written by hand count too
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Added Export notes as Markdown"
    And I run "gdd launch c1"
    And the file "CHANGELOG.md" says:
      """
      # Changelog

      ## [Unreleased]

      ### Added

      - Export notes as Markdown
      """
    When I run "gdd changelog"
    Then it succeeds
    And the output contains "CHANGELOG.md covers every change on main"

  Scenario: A version missing from the file gets its own section, in order
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Added Export notes as Markdown"
    And I run "gdd launch c1"
    And the tag "v0.1.0" is on main
    And I run "gdd new-change c2 --title Crash --type Hotfix --paths src/app.txt"
    And I run "gdd launch c2"
    And the file "CHANGELOG.md" says:
      """
      # Changelog

      ## [Unreleased]

      ### Fixed

      - Crash
      """
    When I run "gdd changelog --write"
    Then it succeeds
    And the output contains "CHANGELOG.md: 1 line(s) added"
    And the file "CHANGELOG.md" contains "## [0.1.0] - "
    And the file "CHANGELOG.md" contains "- Export notes as Markdown <!-- gdd: "
    And in the file "CHANGELOG.md", "## [Unreleased]" comes before "## [0.1.0]"

  Scenario: A version that is there gets only its missing line, under its section
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Added Export notes as Markdown"
    And I run "gdd launch c1"
    And I run "gdd new-change c2 --title Crash --type Hotfix --paths src/app.txt"
    And I run "gdd launch c2"
    And the file "CHANGELOG.md" says:
      """
      # Changelog

      ## [Unreleased]

      ### Fixed

      - Crash
      """
    When I run "gdd changelog --write"
    Then it succeeds
    And the output contains "CHANGELOG.md: 1 line(s) added"
    And the file "CHANGELOG.md" contains "- Export notes as Markdown <!-- gdd: "
    And in the file "CHANGELOG.md", "### Added" comes before "### Fixed"

  Scenario: A breaking line keeps its mark when it is added
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Changed --breaking Notes are stored in one file per folder"
    And I run "gdd launch c1"
    And the file "CHANGELOG.md" says:
      """
      # Changelog

      ## [Unreleased]

      ### Added

      - Dark theme
      """
    When I run "gdd changelog --write"
    Then it succeeds
    And the file "CHANGELOG.md" contains "- **Breaking:** Notes are stored in one file per folder <!-- gdd: "
    And in the file "CHANGELOG.md", "### Added" comes before "### Changed"

  Scenario: The check for CI fails when a line is missing
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Added Export notes as Markdown"
    And I run "gdd launch c1"
    When I run "gdd changelog --check"
    Then it fails with a message that contains "CHANGELOG.md misses 1 line(s)"
    And it fails with a message that contains "gdd changelog --write"
    And the file "CHANGELOG.md" does not exist

  Scenario: The check passes when every change is there
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Added Export notes as Markdown"
    And I run "gdd launch c1"
    And I run "gdd changelog --write"
    When I run "gdd changelog --check"
    Then it succeeds
    And the output contains "CHANGELOG.md covers every change on main"
