@hidden
Feature: Hidden examples for agent sheets and gdd agents install

  Background:
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["gdd/**"]
      """
    And the models:
      """
      [orchestrator]
      integration = "claude"
      model = "opus"

      [builder.yellow]
      integration = "claude"
      model = "sonnet"

      [builder_test]
      integration = "claude"
      model = "sonnet"

      [reviewer]
      integration = "codex"
      model = "gpt-6"
      """

  Scenario: Codex is told to call the setup its own way
    When I run "gdd agents install"
    Then the file ".agents/skills/gdd-status/SKILL.md" contains "`$gdd-setup`"

  Scenario: Check names an edited Codex command
    Given I run "gdd agents install"
    When the change modifies ".agents/skills/gdd-build/SKILL.md"
    And I run "gdd agents check"
    Then it fails with a message that contains ".agents/skills/gdd-build/SKILL.md"

  Scenario: Naming one agent writes only its commands
    When I run "gdd agents install --agent claude"
    Then it succeeds
    And the file ".claude/skills/gdd-replan/SKILL.md" contains "name: gdd-replan"
    And the file ".agents/skills/gdd-build/SKILL.md" does not exist
