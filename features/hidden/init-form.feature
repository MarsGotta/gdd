@hidden
Feature: Hidden examples for the Setup form of gdd init

  Background:
    Given an empty folder
    And the coding agents on this machine are "claude codex"

  Scenario: Esc on the platform goes back to the description
    When I open the Setup form
    And I type "Notes"
    And I press Enter
    And I type "Shared lists"
    And I press Enter
    And I press Esc
    Then the form shows "Description"
    And the form shows "Shared lists"

  Scenario: The summary of a mobile project in Python
    Given the form is filled with:
      | field     | answer      |
      | name      | Grocer      |
      | platform  | mobile      |
      | stacks    | python      |
      | language  | en          |
      | sensitive | Stock counts |
    Then the form shows "Platform: mobile"
    And the form shows "Stacks: python"
    And the form shows "Sensitive: Stock counts"

  Scenario: A project with two families says nothing in the status
    Given I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"
    When I run "gdd status"
    Then the output does not contain "second model family"
