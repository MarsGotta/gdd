@hidden
Feature: The sitting (hidden examples)

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

  Scenario: y records G1 from the change's branch and leaves no worktree
    Given I run "gdd new-change c3 --title Sync --paths src/storage/sync.rs features/sync.feature"
    And the change "c3" has the scenario "features/sync.feature" on its branch
    When I open the sitting of "c3"
    And I press a at the sitting
    And I press y at the sitting
    Then the record of "c3" seals "features/sync.feature" with its current SHA-256
    And the current branch is "main"
    And the project has no worktree besides the main checkout

  Scenario: Asking for changes on a yellow change
    Given I run "gdd new-change c4 --title Search --paths src/search.rs features/search.feature"
    And the change "c4" has the scenario "features/search.feature" on its branch
    When I open the sitting of "c4"
    And I press c at the sitting
    And I type "Search by title too" at the sitting
    And I press Enter at the sitting
    Then the sitting shows "G1 ko on c4: Search by title too"
    When I press y at the sitting
    Then the record of "c4" has the gate "G1" with decision "ko" by "human"

  Scenario: A change waiting for its G4 shows its G4 page
    Given I run "gdd new-change c5 --title Guide --paths docs/guide.md features/guide.feature"
    And the change "c5" is in state "in-pre"
    When I open the sitting of "c5"
    Then the sitting shows "c5 · Guide · waiting for your G4"
