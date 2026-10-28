Feature: A person's writes to shared state wait for the agents at work
  Several changes run at once, and gdd checks every agent call: GDD state the agent changed is
  put back and its change parks. Some of that state is shared by every change: the approval of
  the red zones, G0 and the self-test. A person who writes it while an agent of another change
  is at work must not park that change. So approving the red zones as they are already approved
  writes nothing, and a person's write to shared state waits until no agent call is under way:
  it lands between two calls, never during one. A change's own records are not shared and do
  not wait.

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

  Scenario: Approving the red zones as they are already approved writes nothing
    When I run "gdd gate zones ok c2 --by human --note second sitting"
    Then it succeeds
    And the output contains "already approved"
    And the file ".gdd/zones.json" does not contain "second sitting"
    And the log has a "zones" line for "c2" with result "ok"

  Scenario: A new red zone map is approved and written
    Given the red zone paths become "src/storage/**" and "src/sync/**"
    When I run "gdd gate zones ok c2 --by human --note second sitting"
    Then it succeeds
    And the output does not contain "already approved"
    And the file ".gdd/zones.json" contains "src/sync/**"
    And the file ".gdd/zones.json" contains "second sitting"

  Scenario: Approving a new red zone map waits for the agents at work
    Given the red zone paths become "src/storage/**" and "src/sync/**"
    And an agent of another change is at work for 2 seconds
    When I run "gdd gate zones ok c2 --by human"
    Then it succeeds
    And the output contains "waiting for the agents at work to end their call"
    And the file ".gdd/zones.json" contains "src/sync/**"

  Scenario: Approving the red zones as they are does not wait
    Given an agent of another change is at work for 30 seconds
    When I run "gdd gate zones ok c2 --by human"
    Then it succeeds
    And the output contains "already approved"
    And the output does not contain "waiting for the agents at work"

  Scenario: Opening G0 waits for the agents at work
    Given an agent of another change is at work for 2 seconds
    When I run "gdd gate G0 ok --by machines"
    Then it succeeds
    And the output contains "waiting for the agents at work to end their call"

  Scenario: Nothing to wait for, nothing said
    Given the red zone paths become "src/storage/**" and "src/sync/**"
    When I run "gdd gate zones ok c2 --by human"
    Then it succeeds
    And the output does not contain "waiting for the agents at work"

  Scenario: A change's own approval does not wait
    Given I run "gdd new-change c2 --title Other --paths src/other.txt"
    And an agent of another change is at work for 30 seconds
    When I run "gdd gate plan ok c2 --by human"
    Then it succeeds
    And the output does not contain "waiting for the agents at work"

  Scenario: Opening G0 during the Builder's call does not park the change
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the machines open G0 again during the Builder call
    When I run "gdd launch c1"
    Then it succeeds
    And the record of "c1" is in state "in-pre"
    And the G0 opened during the Builder call succeeded
