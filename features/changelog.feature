Feature: The changelog from the changes
  Every change carries one line for the people who use the project, in the merge commit as git
  trailers: `Changelog: <section> · <line>` and `Breaking: yes|no`. The Orchestrator proposes the
  line at the sitting with `gdd changelog-line`; without one, the title goes under the section of
  the change's type (Feature: Added, Hotfix: Fixed, Migration: Changed). Refactors, Spikes and
  No-code changes are left out unless they get a line. `gdd changelog` writes CHANGELOG.md in the
  Keep a Changelog format from the merge commits on main, with what is not released yet under
  Unreleased.

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

  Scenario: The merge commit carries the line of the change
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Added Export notes as Markdown"
    When I run "gdd launch c1"
    Then it succeeds
    And the last commit on main says "Changelog: Added · Export notes as Markdown"
    And the last commit on main says "Breaking: no"

  Scenario: Without a line, the title goes under the section of its type
    Given I run "gdd new-change c1 --title Faster search --type Hotfix --paths src/app.txt"
    When I run "gdd launch c1"
    Then it succeeds
    And the last commit on main says "Changelog: Fixed · Faster search"

  Scenario: A refactor is left out
    Given I run "gdd new-change c1 --title Tidy --type Refactor --paths src/app.txt"
    When I run "gdd launch c1"
    Then it succeeds
    And the last commit on main says "gdd(c1): G3 Merge Gate · Tidy"
    And the last commit on main does not say "Changelog:"

  Scenario: A breaking change says so
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Changed --breaking Notes are stored in one file per folder"
    When I run "gdd launch c1"
    Then it succeeds
    And the last commit on main says "Changelog: Changed · Notes are stored in one file per folder"
    And the last commit on main says "Breaking: yes"

  Scenario: A line can leave a change out on purpose
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 --none"
    When I run "gdd launch c1"
    Then it succeeds
    And the last commit on main does not say "Changelog:"

  Scenario: Only the sections of Keep a Changelog
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd changelog-line c1 Improved Faster"
    Then it fails with a message that contains "Added, Changed, Deprecated, Removed, Fixed, Security"

  Scenario: The changelog of what is merged and not released
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Added Export notes as Markdown"
    And I run "gdd launch c1"
    And I run "gdd new-change c2 --title Crash on empty notes --type Hotfix --paths src/app.txt"
    And I run "gdd launch c2"
    When I run "gdd changelog"
    Then it succeeds
    And the file "CHANGELOG.md" contains "## [Unreleased]"
    And the file "CHANGELOG.md" contains "### Added"
    And the file "CHANGELOG.md" contains "- Export notes as Markdown"
    And the file "CHANGELOG.md" contains "### Fixed"
    And the file "CHANGELOG.md" contains "- Crash on empty notes"
    And the file "CHANGELOG.md" contains "keepachangelog.com"

  Scenario: A breaking line stands out in the changelog
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Changed --breaking Notes are stored in one file per folder"
    And I run "gdd launch c1"
    When I run "gdd changelog"
    Then it succeeds
    And the file "CHANGELOG.md" contains "- **Breaking:** Notes are stored in one file per folder"

  Scenario: A changelog written by hand is left alone
    Given the file "CHANGELOG.md" says "# My own notes"
    When I run "gdd changelog"
    Then it fails with a message that contains "CHANGELOG.md was not written by gdd"
    And the file "CHANGELOG.md" contains "# My own notes"

  Scenario: A Builder cannot write the line of its change
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    When the role "builder-build" runs "gdd changelog-line c1 Added Something"
    Then it fails with a message that contains "A Builder or Reviewer cannot"
