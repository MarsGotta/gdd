@hidden
Feature: Versions from the changes (fresh hidden examples, written after the Builder saw three hidden lines)

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
    And the gate commands:
      """
      [g2]
      unit = "test -f src/app.txt"

      [g3]
      acceptance_full = "true"

      [release]
      simulated = true
      build = "true"
      deploy_pre = "true"
      tests_pre = "true"
      demo = ""
      promote = ""
      observe = ""
      rollback = ""
      """
    And the scenario "features/base.feature" with 1 scenarios
    And everything is committed on main
    And the Setup is done
    And the red zones are approved
    And the agents are fakes

  Scenario Outline: The next version follows the line of the change
    Given the tag "<last>" is on main
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd changelog-line c1 <line>"
    When I run "gdd launch c1"
    Then it succeeds
    And main has the tag "<next>"

    Examples:
      | last   | line                          | next   |
      | v4.1.9 | Deprecated Plain text export  | v4.2.0 |
      | v0.0.3 | Fixed --breaking Dates in UTC | v0.1.0 |
      | v1.0.0 | Security Tokens expire sooner | v1.0.1 |

  Scenario: A version chosen by hand for the first stable release
    Given the versions are released by hand
    And the tag "v1.5.0" is on main
    And I run "gdd new-change c1 --title Crash --type Hotfix --paths src/app.txt"
    And I run "gdd launch c1"
    When I run "gdd release --yes --version v2.0.0"
    Then it succeeds
    And the output contains "Released v2.0.0"
    And main has the tag "v2.0.0"
