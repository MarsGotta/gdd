@hidden
Feature: Minimal by default, a core that stays on (hidden examples)

  Background:
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["gdd/**", "bin/**"]
      """
    And the zone map:
      """
      default = "yellow"

      [[zone]]
      color = "red"
      paths = ["src/storage/**"]
      reason = "Losing the user's drafts"
      """
    And the gate commands:
      """
      [g2]
      unit = "true"

      [g3]
      acceptance_full = "true"
      mutation = "echo mutation-ran"
      """
    And everything is committed on main

  Scenario: A rules file cannot take the Reviewer from red changes
    Given the file "gdd/rules.toml" says:
      """
      [protected]
      all = ["gdd/**", "bin/**"]

      [color.green.full]
      tests_first = true
      mutation = false
      reviewer = true
      g4_if_g1 = true

      [color.yellow.full]
      tests_first = true
      mutation = true
      reviewer = true
      g4_if_g1 = true

      [color.red.full]
      tests_first = true
      mutation = true
      reviewer = false
      g4_if_g1 = true
      """
    When I run "gdd new-change c1 --title Sync --paths src/storage/sync.rs"
    Then it fails with a message that contains "red changes always get mutation testing, the Reviewer and the human G4"

  Scenario: The status says when red changes would go without mutation testing
    Given the gate commands:
      """
      [g2]
      unit = "true"

      [g3]
      acceptance_full = "true"
      mutation = ""
      """
    When I run "gdd status"
    Then it succeeds
    And the output contains "no mutation testing command in [g3] of gdd/commands.toml: red changes go without it"
