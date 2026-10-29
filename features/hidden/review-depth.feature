@hidden
Feature: Review depth by risk (hidden examples)

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


  Scenario: A red table without a lane cannot make the review quick
    Given the file "gdd/rules.toml" says:
      """
      [protected]
      all = ["gdd/**", "bin/**"]

      [color.yellow.full]
      tests_first = true
      mutation = false
      reviewer = true
      g4_if_g1 = true

      [color.red]
      review = "quick"
      """
    When I run "gdd new-change c1 --title Sync --paths src/storage/sync.rs"
    Then it fails with a message that contains "color.red.review"

  Scenario: A green change gets a quick review
    Given the zone map:
      """
      default = "yellow"

      [[zone]]
      color = "green"
      paths = ["docs/**"]
      reason = "Documentation"
      """
    And I run "gdd new-change c1 --title Docs --paths docs/guide.md"
    When I run "gdd classify c1"
    Then it succeeds
    And the JSON field "review" is "quick"
