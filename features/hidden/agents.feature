@hidden
Feature: Hidden examples for agent sheets and tminus agents install

  Background:
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["tminus/**"]
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
    When I run "tminus agents install"
    Then the file ".agents/skills/tminus-status/SKILL.md" contains "`$tminus-setup`"

  Scenario: Check names an edited Codex command
    Given I run "tminus agents install"
    When the change modifies ".agents/skills/tminus-build/SKILL.md"
    And I run "tminus agents check"
    Then it fails with a message that contains ".agents/skills/tminus-build/SKILL.md"

  Scenario: Naming one agent writes only its commands
    When I run "tminus agents install --agent claude"
    Then it succeeds
    And the file ".claude/skills/tminus-replan/SKILL.md" contains "name: tminus-replan"
    And the file ".agents/skills/tminus-build/SKILL.md" does not exist
