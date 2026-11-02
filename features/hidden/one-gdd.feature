@hidden
Feature: One command, tminus (hidden examples)

  Scenario: A Python and TypeScript project with Claude Code and OpenCode has no bin/tminus
    Given an empty folder
    When I run "tminus init --name Shop --platform web --stack python typescript --agents claude opencode"
    Then it succeeds
    And the file "bin/tminus" does not exist
    And no file of the project contains "bin/tminus"

  Scenario: A Python project's bootstrap points to the agent
    Given an empty folder
    And I run "tminus init --name Shop --platform web --stack python --agents claude codex"
    And the program "uv" on the PATH writes the file "uv.lock"
    When I run "tminus bootstrap"
    Then it succeeds
    And the output contains "Next: in your coding agent, the tminus-setup command finishes the Setup"
    And the output does not contain "bin/tminus"
