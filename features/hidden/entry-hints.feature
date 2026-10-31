@hidden
Feature: Next steps point to the agent (hidden examples)

  Scenario: A TypeScript project's bootstrap points to the agent
    Given an empty folder
    And I run "gdd init --name Shop --platform web --stack typescript --agents claude codex"
    And the program "npm" on the PATH writes the file "package-lock.json"
    When I run "gdd bootstrap"
    Then it succeeds
    And the output contains "Next: in your coding agent, the gdd-setup command finishes the Setup"
    And the output does not contain "Next: gdd selftest"
