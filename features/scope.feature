Feature: Scope of a change
  A change plans the paths it will touch. A path outside the plan with the same or lower
  color is accepted and listed. A path of a higher color fails: the change would need gates
  it was not given, and only the Orchestrator can widen the plan. Classifying a change counts
  the paths it really touched, not only the planned ones.

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

  Scenario: Inside the plan
    When the change writes "src/app.rs"
    And I run "gdd scope c1"
    Then the scope passes with nothing accepted

  Scenario: Outside the plan, lower color, is accepted and listed
    When the change writes "docs/notes.md"
    And I run "gdd scope c1"
    Then the scope passes accepting "docs/notes.md"

  Scenario: Outside the plan, higher color, fails
    When the change writes "src/storage/db.rs"
    And I run "gdd scope c1"
    Then the scope fails naming "src/storage/db.rs" and "red"

  Scenario: Classify counts what the change really touched
    When the change writes "src/storage/db.rs"
    And I run "gdd classify c1"
    Then the JSON field "color" is "red"
