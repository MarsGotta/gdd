@hidden
Feature: Hidden examples for recording approvals

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
    And I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs features/drafts.feature"

  Scenario: A Reviewer cannot record an approval
    When the role "reviewer" runs "gdd gate G1 ok c1 --by human"
    Then it fails with a message that contains "cannot open or close gates"

  Scenario: G1 accepts hidden examples that reuse a visible sentence
    Given the change "c1" has the scenario "features/drafts.feature" on its branch
    And the change "c1" has the hidden example "features/hidden/drafts.feature" with the step "Given a name"
    When I run "gdd gate G1 ok c1 --by human"
    Then it succeeds

  Scenario: Status shows approved red zones
    Given I run "gdd gate zones ok --by human"
    When I run "gdd status"
    Then the output contains "Red zones: approved"
