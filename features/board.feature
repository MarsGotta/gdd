Feature: The Board
  `gdd board` shows the changes of the project by phase, in a terminal, read-only: Discover for
  the changes not launched yet, Build for the ones building or parked, Release for the ones in
  pre or waiting for the human's G4, and Done, folded, for the closed, rejected and rolled-back.
  Each change shows its color, its title, its state and its gates. Enter shows the details of
  the selected change; nothing on the Board writes anything.

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

  Scenario: The changes by phase
    Given I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs"
    And I run "gdd new-change c2 --title Export --paths src/export.rs"
    And the change "c2" is in state "building"
    And I run "gdd new-change c3 --title Sync --paths src/sync.rs"
    And the change "c3" is in state "parked"
    And I run "gdd new-change c4 --title Search --paths src/search.rs"
    And the change "c4" is in state "in-pre"
    And I run "gdd new-change c5 --title Guide --paths docs/guide.md"
    And the change "c5" is in state "closed"
    When I open the board
    Then the board shows "Discover (1)"
    And the board shows "Build (2)"
    And the board shows "Release (1)"
    And the board shows "Done (1)"

  Scenario: Each change shows its color, title, state and gates
    Given I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs"
    And I run "gdd gate plan ok c1 --by human"
    And I run "gdd new-change c2 --title Guide --paths docs/guide.md"
    When I open the board
    Then the board shows "c1 · red · Drafts · open · plan:ok"
    And the board shows "c2 · green · Guide · open · no gates yet"

  Scenario: A parked change and one waiting for the human stand out
    Given I run "gdd new-change c3 --title Sync --paths src/sync.rs"
    And the change "c3" is in state "parked"
    And I run "gdd new-change c4 --title Search --paths src/search.rs"
    And the change "c4" is in state "in-pre"
    When I open the board
    Then the board shows "c3 · yellow · Sync · parked"
    And the board shows "c4 · yellow · Search · waiting for your G4"

  Scenario: Done is folded until asked for
    Given I run "gdd new-change c5 --title Guide --paths docs/guide.md"
    And the change "c5" is in state "closed"
    When I open the board
    Then the board shows "Done (1) · d to show"
    And the board does not show "c5 · green · Guide"
    When I press d on the board
    Then the board shows "c5 · green · Guide · closed"

  Scenario: The first change is selected and the arrows move the selection
    Given I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs"
    And I run "gdd new-change c2 --title Export --paths src/export.rs"
    When I open the board
    Then the board shows "› c1 · red · Drafts"
    When I press Down on the board
    Then the board shows "› c2 · yellow · Export"
    And the board does not show "› c1"

  Scenario: Enter shows the details of the selected change and Esc goes back
    Given I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs src/storage/index.rs"
    When I open the board
    And I press Enter on the board
    Then the board shows "Type: Feature"
    And the board shows "Paths: src/storage/db.rs, src/storage/index.rs"
    And the board shows "Inbox: .gdd/changes/c1/inbox.md"
    When I press Esc on the board
    Then the board shows "Discover (1)"

  Scenario: q closes the Board and nothing was written
    Given I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs"
    When I open the board
    And I press q on the board
    Then the board is closed
    And the record of "c1" is in state "open"

  Scenario: A project without changes says how to start one
    When I open the board
    Then the board shows "No changes yet"
    And the board shows "gdd new-change"

  Scenario: A record that cannot be read is shown, not skipped
    Given I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs"
    And the file ".gdd/changes/c9/change.json" says "{not json"
    When I open the board
    Then the board shows "c9 · change.json is not valid"
    And the board shows "c1 · red · Drafts"

  Scenario: Without a terminal the Board says what to use instead
    When I run "gdd board"
    Then it fails with a message that contains "gdd board needs a terminal"
    And it fails with a message that contains "gdd status"
