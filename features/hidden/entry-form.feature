@hidden
Feature: The Setup form also installs the stacks (hidden examples)

  Background:
    Given an empty folder
    And the coding agents on this machine are "claude codex"

  Scenario: A Python project is installed by the form
    Given the form is filled with:
      | field    | answer |
      | name     | Grocer |
      | platform | web    |
      | stacks   | python |
    And the program "uv" on the PATH writes the file "uv.lock"
    When I press Enter
    Then it succeeds
    And the output contains "python: uv lock && uv sync --locked"
    And git tracks "uv.lock"
    And the last commit on main says "gdd bootstrap"
