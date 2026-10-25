Feature: Classify a change
  The color of a change comes from the paths it touches and the zone map.
  The color, whether scenarios change and whether the change is resumable
  decide its lane and which gates it must pass.

  Background:
    Given the zone map:
      """
      default = "yellow"

      [[zone]]
      color = "green"
      paths = ["docs/**", "*.md"]

      [[zone]]
      color = "red"
      paths = ["src/storage/**"]
      """
    And the default rules table

  Scenario: The highest color wins
    When a change touches "docs/intro.md" and "src/storage/save.rs"
    Then its color is "red"

  Scenario: A path outside every zone takes the default color
    When a change touches "src/ui/button.rs"
    Then its color is "yellow"

  Scenario: A change with no paths takes the default color
    When a change touches no paths
    Then its color is "yellow"

  Scenario: One star stays inside a folder, two stars cross folders
    When a change touches "docs/guide/deep/page.txt"
    Then its color is "green"
    When a change touches "notes/readme.md"
    Then its color is "yellow"

  Scenario: A declared minimum color raises the color, never lowers it
    When a change touches "docs/intro.md" with minimum color "red"
    Then its color is "red"
    When a change touches "src/storage/save.rs" with minimum color "green"
    Then its color is "red"

  Scenario: An unknown color in the zone map is an error, not a color
    Given the zone map:
      """
      default = "yellow"

      [[zone]]
      color = "purple"
      paths = ["src/**"]
      """
    When a change touches "src/main.rs"
    Then classifying fails with a message that names "purple"

  Scenario Outline: The lane
    When a change touches "<path>"<scenarios> and is <resumable>
    Then its lane is "<lane>"

    Examples:
      | path                | scenarios                                   | resumable     | lane |
      | docs/intro.md       |                                             | resumable     | fast |
      | src/ui/button.rs    |                                             | resumable     | fast |
      | src/ui/button.rs    |                                             | not resumable | full |
      | src/storage/save.rs |                                             | resumable     | full |
      | docs/intro.md       | and the scenario file "features/a.feature"  | resumable     | full |

  Scenario: Changing only regression scenarios does not need scenario approval
    When a change touches "src/ui/button.rs" and the scenario file "features/regression/bug-12.feature" and is resumable
    Then it does not need scenario approval (G1)
    And its lane is "fast"

  Scenario: A red change always needs the human plan, the release gate and the Reviewer
    When a change touches "src/storage/save.rs" and is resumable
    Then it needs the human plan
    And it needs the human release gate (G4)
    And it needs the Reviewer
    And it needs mutation testing

  Scenario: New scenarios outside red need the release gate
    When a change touches "src/ui/button.rs" and the scenario file "features/a.feature" and is resumable
    Then it needs scenario approval (G1)
    And it needs the human release gate (G4)
    And it does not need the human plan

  Scenario: A green change in the fast lane skips the Reviewer, mutation testing and tests first
    When a change touches "docs/intro.md" and is resumable
    Then it does not need the Reviewer
    And it does not need mutation testing
    And it does not need tests first
    And it does not need the human release gate (G4)
