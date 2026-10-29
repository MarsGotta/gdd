Feature: Review depth by risk
  A miss costs more in a red change than in a yellow one, so the review does not have to cost the
  same. Each change gets a review depth from its color: quick for green and yellow, deep for red.
  The classification says it (`review`), the record keeps it, and the Reviewer reads it and is
  told what each depth means. A project can ask for a deep review on yellow changes too with one
  line in gdd/rules.toml; a red change always gets the deep one, as the rest of the core.

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


  Scenario: A yellow change gets a quick review
    Given I run "gdd new-change c1 --title App --paths src/app.rs"
    When I run "gdd classify c1"
    Then it succeeds
    And the JSON field "review" is "quick"

  Scenario: A red change gets a deep review
    Given I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs"
    When I run "gdd classify c1"
    Then it succeeds
    And the JSON field "review" is "deep"

  Scenario: A project asks for a deep review on yellow changes too
    Given the file "gdd/rules.toml" says:
      """
      [protected]
      all = ["gdd/**", "bin/**"]

      [color.yellow.full]
      tests_first = true
      mutation = false
      reviewer = true
      g4_if_g1 = true
      review = "deep"

      [color.red.full]
      tests_first = true
      mutation = true
      reviewer = true
      g4_if_g1 = true
      """
    And I run "gdd new-change c1 --title App --paths src/app.rs"
    When I run "gdd classify c1"
    Then it succeeds
    And the JSON field "review" is "deep"

  Scenario: A rules file cannot make the review of red changes quick
    Given the file "gdd/rules.toml" says:
      """
      [protected]
      all = ["gdd/**", "bin/**"]

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
      review = "quick"
      """
    When I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs"
    Then it fails with a message that contains "red changes always get mutation testing, the Reviewer and the human G4"
    And it fails with a message that contains "color.red.full.review"

  Scenario: A review depth that is neither quick nor deep is refused
    Given the file "gdd/rules.toml" says:
      """
      [protected]
      all = ["gdd/**", "bin/**"]

      [color.yellow.full]
      tests_first = true
      mutation = false
      reviewer = true
      g4_if_g1 = true
      review = "medium"

      [color.red.full]
      tests_first = true
      mutation = true
      reviewer = true
      g4_if_g1 = true
      """
    When I run "gdd new-change c1 --title App --paths src/app.rs"
    Then it fails with a message that contains "review must be quick or deep"

  Scenario: The Reviewer is told what each depth means
    Given the models:
      """
      [orchestrator]
      integration = "claude"
      model = "opus"

      [builder.yellow]
      integration = "claude"
      model = "sonnet"

      [builder.red]
      integration = "claude"
      model = "opus"

      [builder_test]
      integration = "claude"
      model = "sonnet"

      [reviewer]
      integration = "codex"
      model = "gpt-6"

      [families]
      "opus" = "anthropic"
      "sonnet" = "anthropic"
      "gpt" = "openai"
      """
    When I run "gdd agents install"
    Then it succeeds
    And the file ".claude/skills/gdd-review/SKILL.md" contains "Quick review"
    And the file ".claude/skills/gdd-review/SKILL.md" contains "Deep review"
    And the file ".claude/skills/gdd-review/SKILL.md" contains "only the files the change touched"
    And the file ".claude/skills/gdd-review/SKILL.md" contains "the code that calls what changed"

  Scenario: A new project writes the depth of each color
    Given an empty folder
    When I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"
    Then it succeeds
    And the TOML file "gdd/rules.toml" has "color.yellow.full.review" = "quick"
    And the TOML file "gdd/rules.toml" has "color.red.full.review" = "deep"
