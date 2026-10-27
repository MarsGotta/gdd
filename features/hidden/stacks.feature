@hidden
Feature: Hidden examples for stack sheets

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
      integration = "opencode"
      model = "kimi-k3"
      """

  Scenario: TypeScript shows its mutation tool
    When I run "gdd stacks show typescript"
    Then the output contains "Mutation (G3): StrykerJS (3.3k stars)"

  Scenario: Python and TypeScript run both unit suites and share the source folder
    When I run "gdd stacks compose typescript python"
    Then the output contains "unit-python = "
    And the output contains "src/**" exactly 1 times

  Scenario: Codex gets the TypeScript instructions as a skill
    Given the project uses the stack "typescript"
    When I run "gdd agents install --agent codex"
    Then the file ".agents/skills/gdd-stack-typescript/SKILL.md" contains "# GDD · TypeScript"
