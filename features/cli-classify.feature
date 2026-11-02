Feature: Classify a change from the command line
  `tminus classify <change>` reads the planned change from .tminus/changes/<change>/change.json,
  the zone map and the rules table of the project, and prints the decision as JSON.

  Background:
    Given a GDD project with the zone map:
      """
      default = "yellow"

      [[zone]]
      color = "green"
      paths = ["docs/**"]

      [[zone]]
      color = "red"
      paths = ["src/storage/**"]
      """
    And the project's rules table is the default one

  Scenario: A planned red change
    Given the planned change "save" touches "src/storage/save.rs" and "docs/intro.md" and is resumable
    When I run "tminus classify save"
    Then it succeeds
    And the JSON field "color" is "red"
    And the JSON field "lane" is "full"
    And the JSON field "g4" is true
    And the JSON field "human_plan" is true

  Scenario: A declared minimum color
    Given the planned change "intro" touches "docs/intro.md" with minimum color "yellow" and is resumable
    When I run "tminus classify intro"
    Then the JSON field "color" is "yellow"
    And the JSON field "lane" is "fast"

  Scenario: An unknown change
    When I run "tminus classify nope"
    Then it fails with a message that contains "nope"

  Scenario: From a subfolder of the project
    Given the planned change "save" touches "src/storage/save.rs" and is resumable
    When I run "tminus classify save" from the folder "src/storage"
    Then the JSON field "color" is "red"

  Scenario: Outside a GDD project
    When I run "tminus classify save" outside any GDD project
    Then it fails with a message that contains "not a GDD project"

  Scenario: A change that cannot be resumed takes the full lane
    Given the planned change "notes" touches "docs/notes.md" and is not resumable
    When I run "tminus classify notes"
    Then the JSON field "color" is "green"
    And the JSON field "lane" is "full"
