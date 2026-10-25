@hidden
Feature: Hidden examples for classifying a change

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

      [[zone]]
      color = "green"
      paths = ["src/storage/cache/**"]
      """

  Scenario: Overlapping zones take the higher color
    When a change touches "src/storage/cache/tmp.rs"
    Then its color is "red"

  Scenario: Order of paths does not matter
    When a change touches "src/storage/save.rs" and "docs/intro.md"
    Then its color is "red"

  Scenario: Two stars at the start also match the top folder
    Given the zone map:
      """
      default = "yellow"

      [[zone]]
      color = "red"
      paths = ["**/secrets.toml"]
      """
    When a change touches "secrets.toml"
    Then its color is "red"
