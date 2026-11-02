Feature: What a change touched
  The core judges a change only from what it touched, reported as items: an item added, changed
  or removed counts as touched. How the work is kept and listed is the blueprint's (for software,
  git and the paths of the project); Path Guard, the scope check, the change's color and the
  gates judge the items it reports, against the rules and the zone map the human approved.

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

  Scenario: An item the change removed counts as touched for Path Guard
    When the change deletes "tminus/rules.toml"
    And I run "tminus guard c1 --role builder-build"
    Then Path Guard fails naming "tminus/rules.toml"

  Scenario: An item the change removed outside its plan counts for the scope
    When the change deletes "src/storage/db.rs"
    And I run "tminus scope c1"
    Then the scope fails naming "src/storage/db.rs" and "red"

  Scenario: The color comes from what the change removed too
    When the change deletes "src/storage/db.rs"
    And I run "tminus classify c1"
    Then it succeeds
    And the JSON field "color" is "red"

  Scenario: The gate commands get the color and the items the change touched
    Given the gate command "g2" "unit" is "test \"$TMINUS_COLOR\" = red && echo \"$TMINUS_DIFF_FILES\" | grep -qx src/storage/db.rs"
    When the change deletes "src/storage/db.rs"
    And I run "tminus g2 c1"
    Then the gate passes
