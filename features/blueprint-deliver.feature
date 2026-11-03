Feature: Delivering a change to pre and to production
  Once joined to the main line, a change goes to pre: its tests and its demo run there, and a
  change that needs the human waits there for G4, again after a polish pass that brought nothing
  new. A change that needs no human approval is promoted by the machines and observed in
  production. The human's OK promotes the change and observes it; a change that production has
  already is only observed, never promoted twice. Its version is released once its promotion
  passed, and what the promotion waits for is said on the way. When production gets worse while
  the change is observed, its rollback has already run by itself: the change no longer waits.
  These scenarios hold today and must keep holding once the recipe asks the blueprint to deliver
  the change at each stage.

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

  Scenario: The change waits for G4 in pre, where its tests and its demo ran
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "tminus gate G1 ok c1 --by human"
    When I run "tminus launch c1"
    Then it succeeds
    And the output contains "Paused at G4"
    And the record of "c1" is in state "in-pre"
    And the file ".tminus/log.jsonl" contains "tests_pre" exactly 1 times
    And the log has no gate command "promote" of "c1"

  Scenario: A polish pass that brings nothing new is in pre again before it waits for G4
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "tminus gate G1 ok c1 --by human"
    And I run "tminus launch c1"
    And the Builder in build mode writes nothing
    When I run "tminus resume c1 --g4 polish --note Nothing to change"
    Then it succeeds
    And the output contains "Paused at G4"
    And the record of "c1" is in state "in-pre"
    And the file ".tminus/log.jsonl" contains "tests_pre" exactly 2 times

  Scenario: A change that needs no human approval is promoted by the machines, then observed
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    When I run "tminus launch c1"
    Then it succeeds
    And the record of "c1" has the gate "G4" with decision "ok" by "machines"
    And the record of "c1" is in state "closed"
    And the file ".tminus/log.jsonl" contains "tests_pre" exactly 1 times
    And the log has the gate command "promote" of "c1" in "-" with result "simulated"
    And the log has the gate command "observe" of "c1" in "-" with result "simulated"

  Scenario: A promotion the machines could not do leaves the change in pre
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the gate command "release" "promote" is "echo registry down; exit 1"
    When I run "tminus launch c1"
    Then it fails with a message that contains "registry down"
    And the record of "c1" is in state "in-pre"
    And the log has no gate command "observe" of "c1"

  Scenario: The change is delivered from the main checkout, on the main line
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the gate command "release" "tests_pre" is "test $(git branch --show-current) = main"
    And the gate command "release" "promote" is "test -f src/app.txt"
    When I run "tminus launch c1"
    Then it succeeds
    And the record of "c1" is in state "closed"

  Scenario: A change production already has is only observed on the human's OK, never promoted twice
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "tminus gate G1 ok c1 --by human"
    And I run "tminus launch c1"
    And the change "c1" is in state "in-production"
    When I run "tminus resume c1 --g4 ok"
    Then it succeeds
    And the record of "c1" is in state "closed"
    And the log has no gate command "promote" of "c1"
    And the log has the gate command "observe" of "c1" in "-" with result "simulated"

  Scenario: The human's OK releases the version of the change once its promotion passed
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And I run "tminus changelog-line c1 Added Export notes as Markdown"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "tminus gate G1 ok c1 --by human"
    And I run "tminus launch c1"
    When I run "tminus resume c1 --g4 ok"
    Then it succeeds
    And the output contains "Released v0.1.0"
    And main has the tag "v0.1.0"
    And the record of "c1" is in state "closed"

  Scenario: A promotion that waits for the merge of another change says so on the way
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "tminus gate G1 ok c1 --by human"
    And I run "tminus launch c1"
    And another tminus holds the merge lock for 2 seconds
    When I run "tminus resume c1 --g4 ok"
    Then it succeeds
    And the output contains "waiting for the merge of another change"
    And the record of "c1" is in state "closed"

  Scenario: When production gets worse while it is observed, the rollback has already run
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the gate command "release" "observe" is "exit 1"
    And the gate command "release" "rollback" is "true"
    When I run "tminus launch c1"
    Then it fails with a message that contains "Rollback: ok"
    And the record of "c1" is in state "rolled-back"
    And the file ".tminus/changes/c1/inbox.md" contains "A hotfix is needed"
