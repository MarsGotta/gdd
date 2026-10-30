Feature: The Setup form of gdd init
  In a terminal, gdd init without flags opens a form for the deterministic part of the Setup,
  one step at a time: the project's name, a description, the platform, its stacks from the
  catalog (or not sure), the coding agents (those found on the machine come preselected), the
  language the agents use with the human, and what would be serious if it failed. A summary
  shows every answer before anything is written; Enter sets the project up exactly as the flags
  would. Esc goes back one step and keeps what was typed. Without a terminal the flags are
  needed. gdd status tells a project that accepted one model family when a second one shows up.

  Background:
    Given an empty folder
    And the coding agents on this machine are "claude codex"

  Scenario: Without a terminal gdd init asks for the flags
    When I run "gdd init"
    Then it fails with a message that contains "in a terminal"
    And the output contains "--name"

  Scenario: The form starts with the project's name
    When I open the Setup form
    Then the form shows "Project name"
    And the form shows "Step 1 of 7"

  Scenario: A name, then a description, then the platform
    When I open the Setup form
    And I type "Notes"
    And I press Enter
    And I type "Personal notes"
    And I press Enter
    Then the form shows "Platform"
    And the form shows "mobile"
    And the form shows "cli"

  Scenario: Esc goes back one step and keeps what was typed
    When I open the Setup form
    And I type "Notes"
    And I press Enter
    And I press Esc
    Then the form shows "Project name"
    And the form shows "Notes"

  Scenario: The agents found on the machine come preselected
    Given the form is filled up to the agents with:
      | field    | answer |
      | name     | Notes  |
      | platform | cli    |
      | stacks   | rust   |
    Then the form shows "[x] claude"
    And the form shows "[x] codex"
    And the form shows "[ ] opencode"

  Scenario: A stack must be picked
    Given the form is filled up to the stacks with:
      | field    | answer |
      | name     | Notes  |
      | platform | cli    |
    When I press Enter
    Then the form shows "Pick at least one stack, or Not sure"

  Scenario: Several stacks can be picked
    Given the form is filled up to the stacks with:
      | field    | answer |
      | name     | Notes  |
      | platform | web    |
    When I press Space
    And I press Down
    And I press Space
    And I press Enter
    Then the form shows "Coding agents"

  Scenario: The summary shows every answer before writing anything
    Given the form is filled with:
      | field     | answer                 |
      | name      | Notes                  |
      | platform  | web                    |
      | stacks    | rust, typescript       |
      | language  | es                     |
      | sensitive | Payments; User data    |
    Then the form shows "Stacks: rust, typescript"
    And the form shows "Sensitive: Payments; User data"
    And the form shows "Enter to set up"
    And the file "gdd/project.toml" does not exist

  Scenario: Enter on the summary sets the project up
    Given the form is filled with:
      | field     | answer   |
      | name      | Notes    |
      | platform  | cli      |
      | stacks    | rust     |
      | language  | es       |
      | sensitive | Payments |
    And the program "cargo" on the PATH writes the file "Cargo.lock"
    When I press Enter
    Then the file "gdd/project.toml" contains "Notes"
    And the file "gdd/project.toml" contains "es"
    And the last commit on main says "gdd bootstrap"

  Scenario: Not sure about the technology ends with the command to run later
    Given the form is filled with:
      | field    | answer   |
      | name     | Notes    |
      | platform | cli      |
      | stacks   | not sure |
    Then the form shows "gdd init --name Notes --platform cli"
    And the form does not show "/gdd-setup"
    And the form does not show "Enter to set up"

  Scenario: One model family needs an explicit yes
    Given the coding agents on this machine are "claude"
    And the form is filled with:
      | field    | answer |
      | name     | Notes  |
      | platform | cli    |
      | stacks   | rust   |
    When I press Enter
    Then the form shows "same model family"
    And the file "gdd/project.toml" does not exist
    When I press y
    Then the file "gdd/models.toml" contains "same_family_accepted"

  Scenario: The status tells a one-family project when a second family shows up
    Given the coding agents on this machine are "claude"
    And I run "gdd init --name Notes --platform cli --stack rust --agents claude --accept-same-family"
    And the coding agents on this machine are "claude codex"
    When I run "gdd status"
    Then the output contains "second model family"
    And the output contains "codex"
