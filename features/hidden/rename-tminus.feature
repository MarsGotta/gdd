@hidden
Feature: The tool is called tminus, and gdd is another name for it (hidden examples)

  Scenario: A project set up before the rename keeps working when called as gdd
    Given an empty folder
    And the file "gdd/project.toml" says:
      """
      name = "Shop"
      language = "es"
      """
    And the file "gdd/zones.toml" says:
      """
      default = "red"
      """
    And the folder is a git repository with the file "docs/brief.md"
    When I run "gdd new-change c7 --title Menu --paths docs/menu.md"
    Then it succeeds
    And the file ".gdd/changes/c7/change.json" exists
    And the file ".tminus" does not exist
    And the file "tminus" does not exist

  Scenario: A TMINUS_ variable comes first, whichever change each one names
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["tminus/**", ".github/**"]
      """
    And the zone map:
      """
      default = "red"

      [[zone]]
      color = "green"
      paths = ["notes/**"]
      """
    And everything is committed on main
    And I run "tminus new-change c1 --title Docs --paths notes/a.md"
    And I run "tminus new-change c2 --title Menu --paths src/menu.txt"
    And the environment has "GDD_CHANGE" set to "c2"
    And the environment has "TMINUS_CHANGE" set to "c1"
    When I run "tminus classify"
    Then it succeeds
    And the JSON field "color" is "green"
