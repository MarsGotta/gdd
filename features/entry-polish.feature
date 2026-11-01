Feature: A first change without stumbles
  What a person meets on a first change says what is true and what to do next. The Board
  reloads the project by itself every 2 seconds and with r, keeping the selected change. The
  Decision Inbox of a change not launched yet says that its color and lane come at launch,
  instead of question marks. A sitting with no scenarios committed on the change's branch says
  when they show. The refusal of a command to an agent names every agent tminus started, the
  Orchestrator too, not only Builders and Reviewers.

  Background:
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["tminus/**", "bin/**"]
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

  Scenario: r reloads the Board and keeps the selected change
    Given I run "tminus new-change c1 --title Drafts --paths src/storage/db.rs"
    And I run "tminus new-change c2 --title Export --paths src/export.rs"
    When I open the board
    And I press Down on the board
    And I run "tminus new-change c3 --title Sync --paths src/sync.rs"
    Then the board shows "Discover (2)"
    And the board shows "d done · t time · r reload · q quit"
    When I press r on the board
    Then the board shows "Discover (3)"
    And the board shows "› c2 · yellow · Export"
    And the board shows "c3 · yellow · Sync · open"

  Scenario: The Board reloads by itself every 2 seconds
    Given I run "tminus new-change c1 --title Drafts --paths src/storage/db.rs"
    When I open the board
    And I run "tminus park c1 stuck"
    And 1 second passes on the board
    Then the board shows "c1 · red · Drafts · open"
    When 2 seconds pass on the board
    Then the board shows "c1 · red · Drafts · parked"

  Scenario: The Decision Inbox of a change not launched yet says when its color comes
    Given I run "tminus new-change c1 --title Drafts --paths src/storage/db.rs"
    When I run "tminus inbox c1"
    Then it succeeds
    And the output contains "Color and lane not decided yet (tminus decides them at launch) · state open"
    And the output does not contain "Color ?"

  Scenario: A sitting with no scenarios committed says when they show
    Given I run "tminus new-change c1 --title Drafts --paths src/storage/db.rs features/drafts.feature"
    When I open the sitting of "c1"
    Then the sitting shows "No scenarios on change/c1 yet: they show here once your agent commits them"
    And the sitting does not show "No scenario files on"

  Scenario: The refusal to an agent names every agent tminus started
    When the role "orchestrator" runs "tminus launch c1"
    Then it fails with a message that contains "Agents that tminus started cannot run tminus launch: they may only run tminus g2"
