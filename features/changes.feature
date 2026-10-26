Feature: Create a change and follow it
  A change is born with a record and its own branch. Agents and the Orchestrator leave notes
  on it; the Decision Inbox shows the human only what is still open. Status gives the
  picture of the whole project.

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
      color = "green"
      paths = ["docs/**"]

      [[zone]]
      color = "red"
      paths = ["src/storage/**"]
      reason = "Losing the user's drafts"
      """
    And everything is committed on main

  Scenario: Creating a change makes its record and its branch
    When I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs"
    Then it succeeds
    And the JSON field "color" is "red"
    And the branch "change/c1" exists
    And the log has a "-" line for "c1" with result "change-created"

  Scenario: A change id cannot leave the changes folder
    When I run "gdd new-change ../c1 --title Drafts"
    Then it fails with a message that contains "change id"
    And the branch "change/../c1" does not exist

  Scenario: An existing change is never overwritten
    Given I run "gdd new-change c1 --title Drafts --paths src/app.rs"
    When I run "gdd new-change c1 --title Other --paths docs/a.md"
    Then it fails with a message that contains "already exists"

  Scenario: A minimum color raises the change
    When I run "gdd new-change c1 --title Docs --paths docs/a.md --min-color red"
    Then the JSON field "color" is "red"

  Scenario: Without a color table, classify and the gates use the built-in one
    Given I run "gdd new-change c1 --title App --paths src/app.rs"
    When I run "gdd classify c1"
    Then the JSON field "color" is "yellow"
    And the JSON field "mutation" is true

  Scenario: Notes reach the Decision Inbox grouped by kind
    Given I run "gdd new-change c1 --title App --paths src/app.rs"
    And I run "gdd note c1 needs Pick a name for the export"
    And I run "gdd note c1 decided Kept the old format"
    When I run "gdd inbox c1"
    Then the output contains "## Needs your decision"
    And the output contains "- Pick a name for the export"
    And the output contains "- Kept the old format"
    And the file ".gdd/changes/c1/inbox.md" holds the same text as the output

  Scenario: Requests resolved before the last launch leave the inbox
    Given I run "gdd new-change c1 --title App --paths src/app.rs"
    And the change "c1" has a resolved request "Old question"
    When I run "gdd inbox c1"
    Then the output does not contain "Old question"
    And the output contains "1 earlier request(s) were resolved"

  Scenario: Status lists the changes with their gates
    Given I run "gdd new-change c1 --title App --paths src/app.rs"
    When I run "gdd status"
    Then the output contains "Red zones: pending"
    And the output contains "c1 · App · open · no gates yet"
