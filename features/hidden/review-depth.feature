@hidden
Feature: Review depth by risk (hidden examples)

  Background:
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["tminus/**", "bin/**"]
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
    Given the file "tminus/rules.toml" says:
      """
      [protected]
      all = ["tminus/**", "bin/**"]

      [color.yellow.full]
      tests_first = true
      mutation = false
      reviewer = true
      g4_if_g1 = true

      [color.red]
      review = "quick"
      """
    When I run "tminus new-change c1 --title Sync --paths src/storage/sync.rs"
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
    And I run "tminus new-change c1 --title Docs --paths docs/guide.md"
    When I run "tminus classify c1"
    Then it succeeds
    And the JSON field "review" is "quick"
