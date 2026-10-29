Feature: Steps that do not rot
  The usual critique of BDD is that the layer of step definitions grows and rots: each new
  scenario brings its own steps, near copies of the ones there are. gdd keeps running the
  scenarios the human approved, and tells the agents how to keep that layer small: the Builder in
  test mode reuses a step that already says the same and adds one only when none fits, and each
  step drives the behavior at the lowest layer that shows it, so a scenario is not an end-to-end
  test by default. The Reviewer names a new step that repeats one there was.

  Background:
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["gdd/**", "bin/**"]
      builder_build = ["features/**", "tests/steps/**"]
      builder_test = ["src/**"]
      """
    And the zone map:
      """
      default = "yellow"

      [[zone]]
      color = "red"
      paths = ["src/storage/**"]
      reason = "Losing the user's drafts"
      """
    And the models:
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

  Scenario: The Builder in test mode is told to reuse the steps there are
    When I run "gdd agents install"
    Then it succeeds
    And the file ".claude/skills/gdd-build-test/SKILL.md" contains "reuse a step that already says the same"
    And the file ".claude/skills/gdd-build-test/SKILL.md" contains "add a new one only when none fits"

  Scenario: The Builder in test mode is told to drive each step at the lowest layer
    When I run "gdd agents install"
    Then it succeeds
    And the file ".claude/skills/gdd-build-test/SKILL.md" contains "the lowest layer that shows the behavior"
    And the file ".claude/skills/gdd-build-test/SKILL.md" contains "a domain function, then the command line, then the screen"

  Scenario: The Reviewer is told to name a step that repeats another
    When I run "gdd agents install"
    Then it succeeds
    And the file ".claude/skills/gdd-review/SKILL.md" contains "a new step that says what an existing step already says"
