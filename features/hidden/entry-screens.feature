@hidden
Feature: The screens point to the agent (hidden examples)

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
      color = "red"
      paths = ["src/storage/**"]
      reason = "Losing the user's drafts"
      """
    And everything is committed on main

  Scenario: A sitting approved for another change offers nothing to approve
    Given I run "tminus gate zones ok --by human"
    And I run "tminus new-change c2 --title Tags --paths docs/tags.md features/tags.feature"
    And the change "c2" has the scenario "features/tags.feature" on its branch
    And I am on the branch "change/c2"
    And I run "tminus gate G1 ok c2 --by human"
    When I open the sitting of "c2"
    Then the sitting shows "Nothing waits for you"
    And the sitting does not show "a approve"
