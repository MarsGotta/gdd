@hidden
Feature: A first change without stumbles (hidden examples)

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

  Scenario: r keeps the third change selected when another one starts
    Given I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs"
    And I run "gdd new-change c2 --title Export --paths src/export.rs"
    And I run "gdd new-change c3 --title Sync --paths src/sync.rs"
    When I open the board
    And I press Down on the board
    And I press Down on the board
    And I run "gdd new-change c4 --title Guide --paths docs/guide.md"
    And I press r on the board
    Then the board shows "Discover (4)"
    And the board shows "› c3 · yellow · Sync"

  Scenario: Three seconds after it opened, the Board shows a change parked meanwhile
    Given I run "gdd new-change c2 --title Export --paths src/export.rs"
    When I open the board
    And I run "gdd park c2 waiting"
    And 3 seconds pass on the board
    Then the board shows "c2 · yellow · Export · parked"

  Scenario: A parked change that never launched has no color in its Decision Inbox either
    Given I run "gdd new-change c2 --title Export --paths src/export.rs"
    And I run "gdd park c2 stuck"
    When I run "gdd inbox c2"
    Then it succeeds
    And the output contains "Color and lane not decided yet (gdd decides them at launch) · state parked"

  Scenario: The sitting of a yellow change without committed scenarios
    Given I run "gdd new-change c2 --title Export --paths src/export.rs features/export.feature"
    When I open the sitting of "c2"
    Then the sitting shows "No scenarios on change/c2 yet: they show here once your agent commits them"

  Scenario: The Reviewer reads the same refusal
    When the role "reviewer" runs "gdd merge c1"
    Then it fails with a message that contains "Agents that gdd started cannot run gdd merge: they may only run gdd g2"
