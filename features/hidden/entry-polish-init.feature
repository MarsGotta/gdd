@hidden
Feature: A new project starts its changelog (hidden examples)

  Background:
    Given an empty folder

  Scenario: A Python project starts its changelog too
    When I run "tminus init --name Shop --platform web --stack python --agents claude codex"
    Then it succeeds
    And the file "CHANGELOG.md" contains "## [Unreleased]"
    And git tracks "CHANGELOG.md"

  Scenario: The team's own changelog stays theirs
    Given the file "CHANGELOG.md" says "# Notes of the team"
    When I run "tminus init --name Shop --platform web --stack python --agents claude codex"
    Then it succeeds
    And the file "CHANGELOG.md" contains "# Notes of the team"
    And git does not track "CHANGELOG.md"
