Feature: Minimal by default, a core that stays on
  GDD starts small: a new project runs mutation testing on red changes only, and anyone who wants
  it on yellow too turns it on in gdd/rules.toml. What guards against the agents is the core, and
  no rules file can switch it off: a red change always gets mutation testing, the Reviewer and the
  human G4. A rules file that tries is refused, loudly, by every command that reads it. `gdd status`
  says when the project has no mutation testing command, since red changes would go without it.

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

  Scenario: A rules file cannot switch mutation testing off for red changes
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
      mutation = false
      reviewer = true
      g4_if_g1 = true
      """
    When I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs"
    Then it fails with a message that contains "red changes always get mutation testing, the Reviewer and the human G4"
    And it fails with a message that contains "color.red.full.mutation"

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
    When I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs"
    Then it fails with a message that contains "color.red.full.reviewer"

  Scenario: A rules file cannot take the human G4 from red changes
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
      reviewer = true
      g4_if_g1 = false
      """
    When I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs"
    Then it fails with a message that contains "color.red.full.g4_if_g1"

  Scenario: Yellow changes may go without mutation testing
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
      mutation = false
      reviewer = true
      g4_if_g1 = true

      [color.red.full]
      tests_first = true
      mutation = true
      reviewer = true
      g4_if_g1 = true
      """
    And I run "gdd new-change c1 --title App --paths src/app.rs"
    When I run "gdd classify c1"
    Then it succeeds
    And the JSON field "mutation" is false

  Scenario: The status says when red changes would go without mutation testing
    Given the gate commands:
      """
      [g2]
      unit = "true"

      [g3]
      acceptance_full = "true"
      """
    When I run "gdd status"
    Then it succeeds
    And the output contains "no mutation testing command in [g3] of gdd/commands.toml: red changes go without it"

  Scenario: With a mutation testing command the status says nothing of it
    When I run "gdd status"
    Then it succeeds
    And the output does not contain "no mutation testing command"
