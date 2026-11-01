Feature: A parked change gets going again
  A relaunch of a parked change finds work in its worktree: what the agents of the earlier run
  wrote, and what a person changed by hand after it parked. gdd keeps the hashes of that work when
  the change parks, so the relaunch knows which is which. The agents' work is committed once the
  gates of the earlier run accept it, as before; what changed by hand after the park is committed
  as the person's, said in the Decision Inbox, and never judged as a Builder's. A Builder is judged
  only on what it writes in the relaunch: test mode never answers for the earlier code, and build
  mode never for the steps. Work of the earlier run that its gates did not accept parks the relaunch
  before any agent call, saying what to do. A launch refused before the build leaves no worktree
  behind, so the change's branch can be checked out again.

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

  Scenario: A relaunch with the earlier code and G2 red judges test mode only on what it writes
    Given the gate command "g2" "unit" is "test -f src/app.txt && ! grep -q TODO tests/steps/app.txt"
    And the gate command "g3" "acceptance_full" is "test -f src/extra.txt"
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd launch c1"
    And the file "src/extra.txt" in the worktree of "c1" is written by hand saying "the missing part"
    And the file "tests/steps/app.txt" in the worktree of "c1" is written by hand saying "TODO split this step"
    When I run "gdd launch c1"
    Then it succeeds
    And the agents were called in this order:
      | role          | model  | arguments                   |
      | builder-test  | sonnet | change=c1 mode=test         |
      | builder-build | sonnet | change=c1 mode=build        |
      | builder-build | sonnet | change=c1 mode=build fix=g3 |
      | builder-test  | sonnet | change=c1 mode=test         |
      | builder-build | sonnet | change=c1 mode=build        |
      | reviewer      | gpt-6  | change=c1 round=1           |
    And the record of "c1" is in state "closed"
    And the file ".gdd/changes/c1/inbox.md" contains "not a Builder's work: src/extra.txt, tests/steps/app.txt"

  Scenario: A relaunch whose code passes G2 after a step fixed by hand skips the Builder and passes G3
    Given the gate command "g3" "acceptance_full" is "grep -q split tests/steps/app.txt"
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd launch c1"
    And the file "tests/steps/app.txt" in the worktree of "c1" is written by hand saying "split in three steps"
    When I run "gdd launch c1"
    Then it succeeds
    And the agents were called in this order:
      | role          | model  | arguments                   |
      | builder-test  | sonnet | change=c1 mode=test         |
      | builder-build | sonnet | change=c1 mode=build        |
      | builder-build | sonnet | change=c1 mode=build fix=g3 |
      | reviewer      | gpt-6  | change=c1 round=1           |
    And the record of "c1" is in state "closed"
    And the file "tests/steps/app.txt" on the branch "main" contains "split in three steps"

  Scenario: A hand commit on the branch over the agents' uncommitted work is kept as the person's
    Given the gate command "g3" "acceptance_full" is "test -f tests/steps/helper.txt"
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd launch c1"
    And the file "tests/steps/helper.txt" in the worktree of "c1" is committed by hand saying "a helper step"
    When I run "gdd launch c1"
    Then it succeeds
    And the agents were called in this order:
      | role          | model  | arguments                   |
      | builder-test  | sonnet | change=c1 mode=test         |
      | builder-build | sonnet | change=c1 mode=build        |
      | builder-build | sonnet | change=c1 mode=build fix=g3 |
      | reviewer      | gpt-6  | change=c1 round=1           |
    And the record of "c1" is in state "closed"
    And the branch "main" holds "tests/steps/app.txt"
    And the file ".gdd/changes/c1/inbox.md" contains "not a Builder's work: tests/steps/helper.txt"

  Scenario: Earlier work its gates did not accept parks the relaunch before any agent call
    Given the Builder in build mode also writes "tests/steps/sneaky.txt" with:
      """
      a step build mode wrote
      """
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd launch c1"
    When I run "gdd launch c1"
    Then it fails with a message that contains "parked: Work of the earlier run that its gates did not accept: tests/steps/sneaky.txt: protected path for builder-build"
    And the output does not contain "tests/steps/app.txt: protected"
    And the agents were called in this order:
      | role          | model  | arguments                         |
      | builder-test  | sonnet | change=c1 mode=test               |
      | builder-build | sonnet | change=c1 mode=build              |
      | builder-build | sonnet | change=c1 mode=build              |
      | builder-build | sonnet | change=c1 mode=build              |
      | orchestrator  | opus   | change=c1 reason=g2               |
      | builder-build | sonnet | change=c1 mode=build after=replan |
    And the record of "c1" is in state "parked"
    And the file ".gdd/changes/c1/inbox.md" does not contain "G2 still red after"

  Scenario: A file build mode wrote where it may not stays its own after the replan sends the change to test mode
    Given the Builder in build mode also writes "tests/steps/sneaky.txt" with:
      """
      a step build mode wrote
      """
    And the replan answers "NEXT: test"
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd launch c1"
    Then it fails with a message that contains "parked: G2 still red after 3 retries and 1 replan"
    And the agents were called in this order:
      | role          | model  | arguments                         |
      | builder-test  | sonnet | change=c1 mode=test               |
      | builder-build | sonnet | change=c1 mode=build              |
      | builder-build | sonnet | change=c1 mode=build              |
      | builder-build | sonnet | change=c1 mode=build              |
      | orchestrator  | opus   | change=c1 reason=g2               |
      | builder-test  | sonnet | change=c1 mode=test after=replan  |
      | builder-build | sonnet | change=c1 mode=build after=replan |
    And the branch "main" does not hold "tests/steps/sneaky.txt"

  Scenario: A launch refused before the build leaves no worktree behind
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    When I run "gdd launch c1"
    Then it fails with a message that contains "New or changed scenarios without G1"
    And the change "c1" has no worktree
    And the current branch is "main"

  Scenario: A refused relaunch keeps the worktree of the parked change
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Builder in build mode writes nothing
    And I run "gdd launch c1"
    And main has the file "gdd/zones.toml" with:
      """
      default = "yellow"

      [[zone]]
      color = "red"
      paths = ["src/storage/**", "src/sync/**"]
      reason = "Losing the user's drafts"
      """
    When I run "gdd launch c1"
    Then it fails with a message that contains "Red zones changed after their approval"
    And the change "c1" has a worktree next to the project
