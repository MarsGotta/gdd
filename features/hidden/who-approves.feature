@hidden
Feature: Only a human approves the scenarios, the plan and the result (hidden examples)

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
    And I run "tminus new-change c1 --title Drafts --paths src/storage/db.rs features/drafts.feature"

  Scenario: Someone who is not the human cannot reject the plan
    When I run "tminus gate plan ko c1 --by deimos"
    Then it fails with a message that contains "plan is decided by a human (--by human)"
    And the log has no "plan" line for "c1"

  Scenario: A Reviewer cannot record the human's approval of the result
    When the role "reviewer" runs "tminus gate G4 ok c1 --by human"
    Then it fails with a message that contains "Agents that tminus started cannot run tminus gate"
    And the file ".tminus/changes/c1/change.json" does not contain "G4"
