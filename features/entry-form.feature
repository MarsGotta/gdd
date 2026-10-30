Feature: The Setup form also installs the stacks
  After the form writes the harness, it runs the first install of each stack, as gdd bootstrap
  does, and commits the lock files: in a terminal, gdd init is the only command a person types to
  set a project up. When an install fails, the harness stays committed and the form says what
  failed and that gdd bootstrap runs it again. Its last line sends the person to their coding
  agent and the gdd-setup command. When the person is not sure of the technology, the form gives
  the gdd init command to run later and no agent command, since no skill is installed yet.

  Background:
    Given an empty folder
    And the coding agents on this machine are "claude codex"

  Scenario: Enter on the summary sets the project up and installs its stack
    Given the form is filled with:
      | field    | answer |
      | name     | Notes  |
      | platform | cli    |
      | stacks   | rust   |
    And the program "cargo" on the PATH writes the file "Cargo.lock"
    When I press Enter
    Then it succeeds
    And the output contains "rust: cargo generate-lockfile"
    And git tracks "Cargo.lock"
    And the last commit on main says "gdd bootstrap"

  Scenario: The form ends by sending you to your agent
    Given the form is filled with:
      | field    | answer |
      | name     | Notes  |
      | platform | cli    |
      | stacks   | rust   |
    And the program "cargo" on the PATH writes the file "Cargo.lock"
    When I press Enter
    Then it succeeds
    And the output contains "/gdd-setup in Claude Code and OpenCode, $gdd-setup in Codex"
    And the output does not contain "run gdd bootstrap"

  Scenario: A failed install keeps the setup and says how to retry
    Given the form is filled with:
      | field    | answer     |
      | name     | Notes      |
      | platform | web        |
      | stacks   | typescript |
    And the program "npm" on the PATH fails
    When I press Enter
    Then it fails with a message that contains "typescript: npm install --no-fund --no-audit failed"
    And it fails with a message that contains "gdd bootstrap"
    And the last commit on main says "gdd init"
