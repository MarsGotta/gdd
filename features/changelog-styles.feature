Feature: Changelogs in the common heading styles
  Not every changelog writes its headings as Keep a Changelog does. gdd also reads
  `## Unreleased`, `## 0.1.0 - <day>`, `## v0.1.0` and `## [0.1.0](<link>) (<day>)`, any case for
  Unreleased. It adds lines to the file's own Unreleased, writes new versions in the style of
  the file's newest version heading, and writes plain lines, without `###` sections, in a file
  that has none. A release by hand turns Unreleased into the version in the same style.

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

  Scenario: An Unreleased heading without brackets is the file's Unreleased
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Added Export notes as Markdown"
    And I run "gdd launch c1"
    And the file "CHANGELOG.md" says:
      """
      # Changelog

      ## Unreleased

      ## 0.1.0 - 2026-10-23

      First release.

      - Something older
      """
    When I run "gdd changelog --write"
    Then it succeeds
    And the file "CHANGELOG.md" contains "## Unreleased" exactly 1 times
    And the file "CHANGELOG.md" does not contain "## [Unreleased]"
    And in the file "CHANGELOG.md", "## Unreleased" comes before "- Export notes as Markdown"
    And in the file "CHANGELOG.md", "- Export notes as Markdown" comes before "## 0.1.0"

  Scenario: A changelog without sections gets plain lines
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Added Export notes as Markdown"
    And I run "gdd launch c1"
    And the file "CHANGELOG.md" says:
      """
      # Changelog

      ## Unreleased

      ## 0.1.0 - 2026-10-23

      First release.

      - Something older
      """
    When I run "gdd changelog --write"
    Then it succeeds
    And the file "CHANGELOG.md" contains "- Export notes as Markdown <!-- gdd: "
    And the file "CHANGELOG.md" does not contain "### Added"

  Scenario: A changelog with sections keeps getting them
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Added Export notes as Markdown"
    And I run "gdd launch c1"
    And the file "CHANGELOG.md" says:
      """
      # Changelog

      ## Unreleased

      ### Fixed

      - Crash
      """
    When I run "gdd changelog --write"
    Then it succeeds
    And the file "CHANGELOG.md" contains "### Added"
    And in the file "CHANGELOG.md", "### Added" comes before "### Fixed"

  Scenario: A missing version is written in the style of the file
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Added Export notes as Markdown"
    And I run "gdd launch c1"
    And the tag "v0.2.0" is on main
    And the file "CHANGELOG.md" says:
      """
      # Changelog

      ## Unreleased

      ## 0.1.0 - 2026-10-23

      - Something older
      """
    When I run "gdd changelog --write"
    Then it succeeds
    And the file "CHANGELOG.md" contains "## 0.2.0 - "
    And the file "CHANGELOG.md" does not contain "## [0.2.0]"
    And in the file "CHANGELOG.md", "## 0.2.0" comes before "## 0.1.0"

  Scenario: Versions written with a v keep it
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Added Export notes as Markdown"
    And I run "gdd launch c1"
    And the tag "v0.2.0" is on main
    And the file "CHANGELOG.md" says:
      """
      # Changelog

      ## v0.1.0 - 2026-10-23

      - Something older
      """
    When I run "gdd changelog --write"
    Then it succeeds
    And the file "CHANGELOG.md" contains "## v0.2.0 - "

  Scenario: A version heading with a link counts as that version
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Added Export notes as Markdown"
    And I run "gdd launch c1"
    And the tag "v0.2.0" is on main
    And the file "CHANGELOG.md" says:
      """
      # Changelog

      ## [0.2.0](https://example.com/compare/v0.1.0...v0.2.0) (2026-10-25)

      - Export notes as Markdown
      """
    When I run "gdd changelog --check"
    Then it succeeds
    And the output contains "CHANGELOG.md covers every change on main"

  Scenario: A release by hand turns Unreleased in the style of the file
    Given main has the file "CHANGELOG.md" with:
      """
      # Changelog

      ## Unreleased

      ## 0.3.0 - 2026-10-23

      - Something older
      """
    And the tag "v0.3.0" is on main
    And I run "gdd new-change c1 --title Crash --type Hotfix --paths src/app.txt"
    And I run "gdd launch c1"
    When I run "gdd release --yes"
    Then it succeeds
    And the file "CHANGELOG.md" contains "## 0.3.1 - "
    And the file "CHANGELOG.md" does not contain "## [0.3.1]"
    And the file "CHANGELOG.md" contains "## Unreleased" exactly 1 times
    And in the file "CHANGELOG.md", "## Unreleased" comes before "## 0.3.1"
    And in the file "CHANGELOG.md", "## 0.3.1" comes before "- Crash"
