@hidden
Feature: The Board (hidden examples)

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

  Scenario: Two in Discover and one rolled back
    Given I run "gdd new-change c7 --title Notes --paths src/notes.rs"
    And I run "gdd new-change c8 --title Backup --paths src/storage/backup.rs"
    And I run "gdd new-change c9 --title Theme --paths docs/theme.md"
    And the change "c9" is in state "rolled-back"
    When I open the board
    Then the board shows "Discover (2)"
    And the board shows "Done (1) · d to show"
    And the board shows "c8 · red · Backup · open · no gates yet"

  Scenario: Selecting the second change and opening it
    Given I run "gdd new-change c7 --title Notes --paths src/notes.rs"
    And I run "gdd new-change c8 --title Backup --paths src/storage/backup.rs"
    When I open the board
    And I press Down on the board
    And I press Enter on the board
    Then the board shows "Paths: src/storage/backup.rs"
    And the board shows "Inbox: .gdd/changes/c8/inbox.md"

  Scenario: A polishing change is building
    Given I run "gdd new-change c7 --title Notes --paths src/notes.rs"
    And the change "c7" is in state "polishing"
    When I open the board
    Then the board shows "Build (1)"
    And the board shows "c7 · yellow · Notes · polishing"
