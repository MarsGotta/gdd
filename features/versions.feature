Feature: Versions from the changes
  A version is a git tag `v<major>.<minor>.<patch>` (Semantic Versioning). By default every change
  with a changelog line gets its version when it is promoted: the next one after the newest tag,
  from its line (Fixed and Security: patch; Added, Changed, Deprecated and Removed: minor; a
  breaking line: major, or minor while the major is 0). The first version of a project is 0.1.0.
  The promotion gets the version in `GDD_VERSION`, and the tag is made only once it succeeds.
  When the project keeps a CHANGELOG.md, its Unreleased section becomes the version in a release
  commit, which the tag names. Projects that release in batches set `[versions] mode = "manual"`
  in `gdd/rules.toml` and release with `gdd release`.

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

  Scenario: The first version of a project is 0.1.0
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Added Export notes as Markdown"
    When I run "gdd launch c1"
    Then it succeeds
    And the output contains "Released v0.1.0"
    And main has the tag "v0.1.0"

  Scenario Outline: The next version follows the line of the change
    Given the tag "<last>" is on main
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 <line>"
    When I run "gdd launch c1"
    Then it succeeds
    And main has the tag "<next>"

    Examples:
      | last   | line                                  | next   |
      | v0.3.0 | Added Search inside notes             | v0.4.0 |
      | v0.3.0 | Fixed Crash on empty notes            | v0.3.1 |
      | v0.3.0 | Changed --breaking One file per folder | v0.4.0 |
      | v1.2.3 | Changed --breaking One file per folder | v2.0.0 |
      | v1.2.3 | Security Shared links expire          | v1.2.4 |

  Scenario: A change left out of the changelog releases nothing
    Given I run "gdd new-change c1 --title Tidy --type Refactor --paths src/app.txt"
    When I run "gdd launch c1"
    Then it succeeds
    And main has no version tag

  Scenario: The promotion gets its version
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Added Export notes as Markdown"
    And the gate command "release" "promote" is "test x$GDD_VERSION = x0.1.0"
    When I run "gdd launch c1"
    Then it succeeds
    And the record of "c1" is in state "closed"
    And main has the tag "v0.1.0"

  Scenario: Without a changelog file the tag names the merge commit
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Added Export notes as Markdown"
    When I run "gdd launch c1"
    Then it succeeds
    And the last commit on main says "gdd(c1): G3 Merge Gate"
    And the tag "v0.1.0" names the last commit on main

  Scenario: The Unreleased section of the changelog becomes the version
    Given the file "CHANGELOG.md" says:
      """
      # Changelog

      Our own words.

      ## [Unreleased]
      """
    And everything is committed on main
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Added Export notes as Markdown"
    When I run "gdd launch c1"
    Then it succeeds
    And the last commit on main says "Release v0.1.0"
    And the tag "v0.1.0" names the last commit on main
    And the file "CHANGELOG.md" contains "Our own words."
    And the file "CHANGELOG.md" contains "## [0.1.0] - "
    And the file "CHANGELOG.md" contains "- Export notes as Markdown"
    And in the file "CHANGELOG.md", "## [Unreleased]" comes before "## [0.1.0]"
    And in the file "CHANGELOG.md", "## [0.1.0]" comes before "- Export notes as Markdown"

  Scenario: A failed promotion releases nothing, and the retry releases once
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Added Export notes as Markdown"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    And the gate command "release" "promote" is "exit 1"
    And I run "gdd resume c1 --g4 ok"
    And the gate command "release" "promote" is "true"
    When I run "gdd resume c1 --g4 ok"
    Then it succeeds
    And main has the tag "v0.1.0"
    And main does not have the tag "v0.2.0"

  Scenario: A change rejected after a failed promotion releases nothing
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Added Export notes as Markdown"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    And the gate command "release" "promote" is "exit 1"
    And I run "gdd resume c1 --g4 ok"
    When I run "gdd resume c1 --g4 ko --note Not now"
    Then it succeeds
    And main has no version tag

  Scenario: Releasing by hand, promotions make no tag
    Given the versions are released by hand
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Added Export notes as Markdown"
    When I run "gdd launch c1"
    Then it succeeds
    And the record of "c1" is in state "closed"
    And main has no version tag

  Scenario: gdd release shows the next version and releases nothing without a yes
    Given the versions are released by hand
    And the tag "v0.1.0" is on main
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 Added Export notes as Markdown"
    And I run "gdd launch c1"
    And I run "gdd new-change c2 --title Crash --type Hotfix --paths src/app.txt"
    And I run "gdd launch c2"
    When I run "gdd release"
    Then it succeeds
    And the output contains "Next version: v0.2.0"
    And the output contains "- Export notes as Markdown"
    And the output contains "- Crash"
    And the output contains "gdd release --yes"
    And main does not have the tag "v0.2.0"

  Scenario: gdd release --yes releases what is merged
    Given the versions are released by hand
    And the tag "v0.1.0" is on main
    And I run "gdd new-change c1 --title Crash --type Hotfix --paths src/app.txt"
    And I run "gdd launch c1"
    When I run "gdd release --yes"
    Then it succeeds
    And the output contains "Released v0.1.1"
    And main has the tag "v0.1.1"

  Scenario: A version chosen by hand must come after the last one
    Given the versions are released by hand
    And the tag "v0.4.0" is on main
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd launch c1"
    When I run "gdd release --yes --version 0.3.9"
    Then it fails with a message that contains "the version must come after v0.4.0"
    And main does not have the tag "v0.3.9"

  Scenario: Nothing to release
    Given the versions are released by hand
    And the tag "v0.1.0" is on main
    When I run "gdd release --yes"
    Then it succeeds
    And the output contains "Nothing to release since v0.1.0"
    And main does not have the tag "v0.1.1"

  Scenario: A Builder cannot release
    Given the versions are released by hand
    When the role "builder-build" runs "gdd release --yes"
    Then it fails with a message that contains "A Builder or Reviewer cannot"
