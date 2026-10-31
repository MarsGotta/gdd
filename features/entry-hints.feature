Feature: Next steps point to the agent
  A person works through their coding agent, so what gdd prints as the next step names the agent
  command, not a command to type: gdd bootstrap points to the gdd-setup command, the sitting
  after an approval says to tell the agent (which launches the change), and gdd adopt says to ask
  the agent for each Refactor. The help of gdd init says that the form installs the stacks.

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
      """
    And the scenario "features/base.feature" with 1 scenarios
  Scenario: Adoption says to ask the agent for each Refactor
    Given the gate commands:
      """
      [g2]
      lint = "echo Found 5 errors.; false"
      unit = "true"
      acceptance = "true"

      [g3]
      acceptance_full = "true"
      security = "true"
      """
    When I run "gdd adopt"
    Then it succeeds
    And the output contains "Ask your agent for each one: /gdd-change pay the debt of"
    And the output does not contain "gdd new-change debt-"

  Scenario: The help of gdd init says the form installs the stacks
    When I run "gdd init --help"
    Then it succeeds
    And the output contains "The form also installs each stack"
    And the output does not contain "installs nothing (gdd bootstrap does)"
