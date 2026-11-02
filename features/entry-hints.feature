Feature: Next steps point to the agent
  A person works through their coding agent, so what tminus prints as the next step names the agent
  command, not a command to type: tminus bootstrap points to the tminus-setup command, the sitting
  after an approval says to tell the agent (which launches the change), and tminus adopt says to ask
  the agent for each Refactor. The help of tminus init says that the form installs the stacks.

  Background:
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["tminus/**", "bin/**"]
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
    When I run "tminus adopt"
    Then it succeeds
    And the output contains "Ask your agent for each one: /tminus-change pay the debt of"
    And the output does not contain "tminus new-change debt-"

  Scenario: The help of tminus init says the form installs the stacks
    When I run "tminus init --help"
    Then it succeeds
    And the output contains "The form also installs each stack"
    And the output does not contain "installs nothing (tminus bootstrap does)"
