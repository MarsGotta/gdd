@hidden
Feature: Hidden examples for Path Guard and scope

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
      """
    And the change "c1" plans "src/app.rs" and "tests/steps/app.rs" and its build has started on its branch

  Scenario: Local GDD state never counts
    When the change writes ".gdd/changes/c1/notes.md"
    And I run "gdd guard c1 --role builder-build"
    Then Path Guard passes

  Scenario: A deleted approved scenario fails
    Given the scenario "features/app.feature" was approved in G1
    When the change deletes "features/app.feature"
    And I run "gdd guard c1 --role builder-test"
    Then Path Guard fails naming "features/app.feature"

  Scenario: A change whose build never started has no starting point
    Given the change "c2" plans "src/b.rs" and its build has not started
    When I run "gdd guard c2 --role builder-build"
    Then Path Guard fails naming "not been prepared"
