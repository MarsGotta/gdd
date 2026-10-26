Feature: gdd launch and gdd resume run the recipe
  The fixed recipe of a change, in Rust: Build (test mode, then build mode) -> G2 -> G3 -> Reviewer
  -> merge -> pre -> G4 pause -> promote -> observe. Spec Kit no longer runs it.

  The agents are fakes: `.gdd/bin/gdd-claude`, `.gdd/bin/gdd-codex` and `.gdd/bin/gdd-opencode`, the
  executables the recipe starts. Each call is recorded (role, model, arguments). By default the test
  mode Builder writes "tests/steps/app.txt", the build mode Builder writes "src/app.txt" with the
  number of its call (so every round changes it), the Reviewer writes "VERDICT: OK" in the change's
  review.md and the replan writes "NEXT: build" in its replan.md.

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

  Scenario: A yellow change without new scenarios goes all the way and the machines promote it
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd launch c1"
    Then it succeeds
    And the agents were called in this order:
      | role          | model  | arguments           |
      | builder-test  | sonnet | change=c1 mode=test |
      | builder-build | sonnet | change=c1 mode=build |
      | reviewer      | gpt-6  | change=c1 round=1   |
    And the last commit on main says "gdd(c1): G3 Merge Gate · App"
    And the record of "c1" has the gate "G4" with decision "ok" by "machines"
    And the record of "c1" is in state "closed"

  Scenario: A change with new scenarios stops at G4 for the human
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    When I run "gdd launch c1"
    Then it succeeds
    And the output contains "Paused at G4"
    And the record of "c1" is in state "in-pre"
    And the file ".gdd/changes/c1/inbox.md" contains "G4"

  Scenario: Resuming with OK promotes the change
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    When I run "gdd resume c1 --g4 ok"
    Then it succeeds
    And the record of "c1" has the gate "G4" with decision "ok" by "human"
    And the record of "c1" is in state "closed"

  Scenario: Resuming with KO keeps the change out of production
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    When I run "gdd resume c1 --g4 ko --note Not what I asked"
    Then it succeeds
    And the record of "c1" has the gate "G4" with decision "ko" by "human"
    And the record of "c1" is in state "rejected"

  Scenario: Polishing at G4 runs one more Builder round with the human's notes
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    When I run "gdd resume c1 --g4 polish --note Shorter error messages"
    Then it succeeds
    And the output contains "Paused at G4"
    And the agents were called in this order:
      | role          | model  | arguments                       |
      | builder-test  | sonnet | change=c1 mode=test             |
      | builder-build | sonnet | change=c1 mode=build            |
      | reviewer      | gpt-6  | change=c1 round=1               |
      | builder-build | sonnet | change=c1 mode=build fix=polish |
      | reviewer      | gpt-6  | change=c1 round=1               |
    And the file ".gdd/changes/c1/inbox.md" contains "Shorter error messages"
    And the record of "c1" is in state "in-pre"

  Scenario: G2 still red after the retries and the replan parks the change
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Builder in build mode writes nothing
    When I run "gdd launch c1"
    Then it fails with a message that contains "parked: G2 still red after 3 retries and 1 replan"
    And the agents were called in this order:
      | role          | model  | arguments                         |
      | builder-test  | sonnet | change=c1 mode=test               |
      | builder-build | sonnet | change=c1 mode=build              |
      | builder-build | sonnet | change=c1 mode=build              |
      | builder-build | sonnet | change=c1 mode=build              |
      | orchestrator  | opus   | change=c1 reason=g2               |
      | builder-build | sonnet | change=c1 mode=build after=replan |
    And the record of "c1" is in state "parked"

  Scenario: The replan can send the change back to test mode
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Builder in build mode writes "src/app.txt" from call 4
    And the replan answers "NEXT: test"
    When I run "gdd launch c1"
    Then it succeeds
    And the agents were called in this order:
      | role          | model  | arguments                         |
      | builder-test  | sonnet | change=c1 mode=test               |
      | builder-build | sonnet | change=c1 mode=build              |
      | builder-build | sonnet | change=c1 mode=build              |
      | builder-build | sonnet | change=c1 mode=build              |
      | orchestrator  | opus   | change=c1 reason=g2               |
      | builder-test  | sonnet | change=c1 mode=test after=replan  |
      | builder-build | sonnet | change=c1 mode=build after=replan |
      | reviewer      | gpt-6  | change=c1 round=1                 |
    And the record of "c1" is in state "closed"

  Scenario: G3 red gets one fix and then parks the change
    Given the gate command "g3" "acceptance_full" is "test -f src/extra.txt"
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd launch c1"
    Then it fails with a message that contains "parked: G3 still red after one fix"
    And the agents were called in this order:
      | role          | model  | arguments                   |
      | builder-test  | sonnet | change=c1 mode=test         |
      | builder-build | sonnet | change=c1 mode=build        |
      | builder-build | sonnet | change=c1 mode=build fix=g3 |
    And the record of "c1" is in state "parked"

  Scenario: A Reviewer KO gets one fix and a second review
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Reviewer answers "VERDICT: KO" and then "VERDICT: OK"
    When I run "gdd launch c1"
    Then it succeeds
    And the agents were called in this order:
      | role          | model  | arguments                       |
      | builder-test  | sonnet | change=c1 mode=test             |
      | builder-build | sonnet | change=c1 mode=build            |
      | reviewer      | gpt-6  | change=c1 round=1               |
      | builder-build | sonnet | change=c1 mode=build fix=review |
      | reviewer      | gpt-6  | change=c1 round=2               |
    And the record of "c1" is in state "closed"

  Scenario: Two Reviewer KOs park the change
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Reviewer answers "VERDICT: KO" every time
    When I run "gdd launch c1"
    Then it fails with a message that contains "parked: Reviewer KO or G3 red after one fix"
    And the record of "c1" is in state "parked"

  Scenario: A relaunch whose code already passes G2 skips the Builder
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the log has a Builder call in build mode for "c1"
    And I am on the branch "change/c1"
    When the change commits "src/app.txt"
    And I run "gdd launch c1"
    Then it succeeds
    And the agents were called in this order:
      | role     | model | arguments         |
      | reviewer | gpt-6 | change=c1 round=1 |

  Scenario: An agent that does not answer in time is stopped and the change parks
    Given the agent time limit is 2 seconds
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Builder in build mode never answers
    When I run "gdd launch c1"
    Then it fails with a message that contains "did not answer within 2 seconds"
    And the record of "c1" is in state "parked"
    And no process the agent started is still running

  Scenario: Launch refuses uncommitted changes outside the change's branch
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    When the change modifies "features/base.feature"
    And I run "gdd launch c1"
    Then it fails with a message that contains "uncommitted changes"

  Scenario: Resume needs a change paused at G4
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd resume c1 --g4 ok"
    Then it fails with a message that contains "not paused at G4"

  Scenario: A Builder cannot answer G4
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    When the role "builder-build" runs "gdd resume c1 --g4 ok"
    Then it fails with a message that contains "cannot open or close gates"
    And the record of "c1" is in state "in-pre"
