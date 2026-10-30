Feature: What a change spent lives in its record
  The spike budget used to add up the costs of the change's lines in the log, and an agent can add
  lines to the log (one with a negative cost would lift the budget). gdd now keeps what each change
  spent in its record, which only gdd writes: after each agent call it adds the cost the agent
  reported. The budget reads it from there. The log stays a report: gdd learn and gdd report leave
  out a line that is not one of gdd's (not JSON, or a cost below zero) and say how many they left out.

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

  Scenario: A line an agent adds to the log does not lift the spike budget
    Given I run "gdd new-change c1 --type Spike --title Storage --paths src/app.txt"
    And the spike budget is "0.02" USD
    And the Builder in build mode writes nothing
    And the Builder in build mode appends to ".gdd/log.jsonl" the line:
      """
      {"ts":"2026-10-30T10:00:00+00:00","change":"c1","phase":"2-build","gate":"G2","role":"builder-build","agent":"claude","model":"sonnet","tokens_in":0,"tokens_cache":0,"tokens_out":0,"cost_usd":-100.0,"duration_s":1.0,"result":"ok","retry":0,"note":"forged"}
      """
    When I run "gdd launch c1"
    Then it fails with a message that contains "parked: the spike spent 0.02 USD of its budget of 0.02 USD"
    And the record of "c1" is in state "parked"

  Scenario: The record keeps what the change spent
    Given I run "gdd new-change c1 --type Spike --title Storage --paths src/app.txt"
    And the spike budget is "0.02" USD
    And the Builder in build mode writes nothing
    When I run "gdd launch c1"
    Then the file ".gdd/changes/c1/change.json" contains "spent_usd"

  Scenario: gdd learn leaves out a line of the log that is not one of gdd's, and says so
    Given the log has the line:
      """
      not a line of gdd
      """
    When I run "gdd learn"
    Then it succeeds
    And the output contains "1 line of the log is not one of gdd's: left out"
