Feature: The screens point to the agent
  A person starts work by talking to their coding agent, so the screens say so. gdd alone, in a
  terminal, opens the Board; without one it prints the help, as before. The empty Board says to
  ask the agent with the gdd-change command, not to type gdd new-change. A sitting whose
  scenarios are already approved, with nothing else waiting, says that nothing waits for the
  person and offers no approval.

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

  Scenario: The empty Board says to ask the agent
    When I open the board
    Then the board shows "No changes yet"
    And the board shows "ask your agent: /gdd-change"
    And the board does not show "gdd new-change"

  Scenario: A sitting already approved offers nothing to approve
    Given I run "gdd gate zones ok --by human"
    And I run "gdd new-change c1 --title Notes --paths docs/notes.md features/notes.feature"
    And the change "c1" has the scenario "features/notes.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    When I open the sitting of "c1"
    Then the sitting shows "Nothing waits for you"
    And the sitting does not show "a approve"

  Scenario: Without a terminal, gdd alone still prints the help
    When I run "gdd"
    Then it succeeds
    And the output contains "Usage: gdd [COMMAND]"
