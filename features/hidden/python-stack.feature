Feature: A Python project runs its scenarios, and its G3 writes nothing under .gdd (hidden examples)

  Scenario: The Python skeleton keeps pytest below what breaks pytest-bdd
    Given an empty folder
    When I run "gdd init --name Shop --platform web --stack python --agents claude codex"
    Then it succeeds
    And the file "pyproject.toml" contains "pytest>=9.1.1,<9.2"

  Scenario: The Python dependencies check writes nothing under .gdd
    Given an empty folder
    When I run "gdd init --name Shop --platform web --stack python rust --agents claude codex"
    Then it succeeds
    And the file "gdd/commands.toml" does not contain "-o .gdd/"
    And the file "gdd/commands.toml" does not contain "mkdir -p .gdd"
