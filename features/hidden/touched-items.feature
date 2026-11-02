@hidden
Feature: What a change touched (hidden examples)

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
      """
    And the file "src/storage/db.rs" says:
      """
      fn save() {}
      """
    And the gate commands:
      """
      [g2]
      lint = "true"
      unit = "true"

      [g3]
      acceptance_full = "true"
      """
    And the change "c1" plans "src/app.rs" and "tests/steps/app.rs" and its build has started on its branch

  Scenario: Test mode that removes production code is stopped
    When the change deletes "src/storage/db.rs"
    And I run "tminus guard c1 --role builder-test"
    Then Path Guard fails naming "src/storage/db.rs"
