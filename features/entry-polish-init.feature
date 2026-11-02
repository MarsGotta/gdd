Feature: A new project starts its changelog
  tminus init writes CHANGELOG.md in the format of Keep a Changelog, with an empty Unreleased
  section, and commits it with the rest of its first commit, so the first release has a place
  for its lines. A CHANGELOG.md already there is the project's: tminus init leaves it as it is.

  Background:
    Given an empty folder

  Scenario: A new project starts its changelog
    When I run "tminus init --name Notes --platform cli --stack rust --agents claude codex"
    Then it succeeds
    And the output contains "CHANGELOG.md"
    And the file "CHANGELOG.md" contains "## [Unreleased]"
    And the file "CHANGELOG.md" contains "keepachangelog.com"
    And git tracks "CHANGELOG.md"
    And the last commit on main says "tminus init"

  Scenario: A changelog already there is kept as it is
    Given the file "CHANGELOG.md" says "# Release notes"
    When I run "tminus init --name Notes --platform cli --stack rust --agents claude codex"
    Then it succeeds
    And the file "CHANGELOG.md" contains "# Release notes"
    And the file "CHANGELOG.md" does not contain "## [Unreleased]"
    And git does not track "CHANGELOG.md"
