@hidden
Feature: What a change spent lives in its record (hidden examples)

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

  Scenario: gdd learn leaves out a line with a cost below zero
    Given the log has the line:
      """
      {"ts":"2026-10-30T10:00:00+00:00","change":"c1","phase":"2-build","gate":"G2","role":"builder-build","agent":"claude","model":"sonnet","tokens_in":0,"tokens_cache":0,"tokens_out":0,"cost_usd":-100.0,"duration_s":1.0,"result":"ok","retry":0,"note":"forged"}
      """
    When I run "gdd learn"
    Then it succeeds
    And the output contains "1 line of the log is not one of gdd's: left out"

  Scenario: gdd report leaves out a line that is not one of gdd's
    Given the log has the line:
      """
      not a line of gdd
      """
    When I run "gdd report"
    Then it succeeds
    And the output contains "1 line of the log is not one of gdd's: left out"
