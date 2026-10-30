Feature: A Python project runs its scenarios, and its G3 writes nothing under .gdd
  The Python skeleton runs its scenarios with pytest-bdd, which breaks on pytest 9.2 (pytest-bdd
  issue 850, fixed upstream but not released yet): the skeleton keeps pytest below 9.2 until a
  pytest-bdd release supports it. The G3 dependencies command of the Python sheet exports the
  runtime requirements to a temporary file it removes, never under .gdd: .gdd is gdd's state, and
  a gate that writes there in a change's worktree parks the change, blaming the next agent.

  Scenario: The Python skeleton keeps pytest below what breaks pytest-bdd
    Given an empty folder
    When I run "gdd init --name Notes --platform cli --stack python --agents claude codex"
    Then it succeeds
    And the file "pyproject.toml" contains "pytest>=9.1.1,<9.2"
    And the file "pyproject.toml" contains "pytest-bdd>=9.0.0"

  Scenario: The Python dependencies check writes nothing under .gdd
    Given an empty folder
    When I run "gdd init --name Notes --platform cli --stack python --agents claude codex"
    Then it succeeds
    And the file "gdd/commands.toml" does not contain "-o .gdd/"
    And the file "gdd/commands.toml" does not contain "mkdir -p .gdd"
    And the file "gdd/commands.toml" contains "osv-scanner"
