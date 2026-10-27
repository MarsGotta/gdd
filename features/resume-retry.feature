Feature: A failed promotion after G4 can be retried
  The human's OK at G4 promotes the change and observes it in production. When the promotion
  fails, production is untouched: the change stays paused at G4, the Decision Inbox says why, and
  the same answer retries it once the cause is fixed (or KO rejects it). When the observation
  fails, the rollback has already run: the change no longer waits at G4, a hotfix is needed.

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


  Scenario: A failed promotion leaves the change paused at G4
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    And the gate command "release" "promote" is "exit 1"
    When I run "gdd resume c1 --g4 ok"
    Then it fails with a message that contains "the change is still paused at G4"
    And the record of "c1" is in state "in-pre"

  Scenario: The message says how to retry
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    And the gate command "release" "promote" is "exit 1"
    When I run "gdd resume c1 --g4 ok"
    Then it fails with a message that contains "To retry: gdd resume c1 --g4 ok"

  Scenario: The Decision Inbox says the promotion failed
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    And the gate command "release" "promote" is "echo registry down; exit 1"
    And I run "gdd resume c1 --g4 ok"
    When I run "gdd inbox c1"
    Then the output contains "Promotion failed"
    And the output contains "registry down"

  Scenario: Retrying once the cause is fixed promotes the change
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    And the gate command "release" "promote" is "exit 1"
    And I run "gdd resume c1 --g4 ok"
    And the gate command "release" "promote" is "true"
    When I run "gdd resume c1 --g4 ok"
    Then it succeeds
    And the record of "c1" is in state "closed"

  Scenario: After a failed promotion the human can still reject the change
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    And the gate command "release" "promote" is "exit 1"
    And I run "gdd resume c1 --g4 ok"
    When I run "gdd resume c1 --g4 ko --note Not now"
    Then it succeeds
    And the record of "c1" is in state "rejected"

  Scenario: A failed observation rolls back and the change no longer waits at G4
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    And the gate command "release" "observe" is "exit 1"
    And I run "gdd resume c1 --g4 ok"
    When I run "gdd resume c1 --g4 ok"
    Then it fails with a message that contains "is not paused at G4"
    And the record of "c1" is in state "rolled-back"
