Feature: An agent can say it cannot do the task
  An agent the recipe starts (the Builder in test mode or in build mode, the Reviewer, the
  Orchestrator when it replans) can end its answer saying it cannot do the task, and why. That is
  an honest exit, not a failure to hide: the change stops at once and parks with the agent's reason
  in the human's messages, without spending the attempts or the replan it had left. Words never
  open a gate: saying it cannot never counts as green nor as an OK, and what the agent touched is
  judged by Path Guard as always, now or when the change runs again. Once the human answers, the
  change runs again as any parked change does.

  Background:
    Given a project that follows the method, with the sensitive area "src/storage/**"

  Scenario: A Builder that says it cannot stops the change at once, with its attempts unspent
    Given the project allows 3 attempts before a replan
    And a change that plans "src/app.txt" is ready to work
    And the Builder in build mode writes nothing
    And the "Builder in build mode" says it cannot do the task because "the package does not say which currency to use"
    When the change runs
    Then the change parks
    And the agents were called in this order:
      | role          | model  | arguments            |
      | builder-test  | sonnet | change=c1 mode=test  |
      | builder-build | sonnet | change=c1 mode=build |

  Scenario: Saying it cannot never counts as green, even when the work passes the gate
    Given a change that plans "src/app.txt" is ready to work
    And the "Builder in build mode" says it cannot do the task because "src/app.txt is only a stub"
    When the change runs
    Then the change parks
    And the log has no "G3" line for "c1"
    And the agents were called in this order:
      | role          | model  | arguments            |
      | builder-test  | sonnet | change=c1 mode=test  |
      | builder-build | sonnet | change=c1 mode=build |

  Scenario: A Reviewer that says it cannot never counts as an OK
    Given a change that plans "src/app.txt" is ready to work
    And the Reviewer answers "VERDICT: OK" every time
    And the "Reviewer" says it cannot do the task because "the change is too large to read in one session"
    When the change runs
    Then the change parks
    And the agents were called in this order:
      | role          | model  | arguments            |
      | builder-test  | sonnet | change=c1 mode=test  |
      | builder-build | sonnet | change=c1 mode=build |
      | reviewer      | gpt-6  | change=c1 round=1    |

  Scenario: The human reads the agent's reason in the change's messages and the log keeps it apart
    Given a change that plans "src/app.txt" is ready to work
    And the "Builder in test mode" says it cannot do the task because "the scenarios name a screen the project does not have"
    When the change runs
    Then it fails with a message that contains "parked: the Builder in test mode says it cannot do the task: the scenarios name a screen the project does not have"
    And the file ".tminus/changes/c1/inbox.md" contains "the Builder in test mode says it cannot do the task: the scenarios name a screen the project does not have"
    And the log has a "G2" line for "c1" with result "cannot"
    And the agents were called in this order:
      | role         | model  | arguments           |
      | builder-test | sonnet | change=c1 mode=test |

  Scenario: Saying it cannot does not spare test mode from Path Guard
    Given a change that plans "src/app.txt" is ready to work
    And the Builder in test mode also writes "src/app.txt" with:
      """
      written in test mode
      """
    And the "Builder in test mode" says it cannot do the task because "the steps need production code"
    When the change runs
    Then it fails with a message that contains "parked: Path Guard failed after test mode: src/app.txt"

  Scenario: What the agent left when it said it cannot is judged before the change runs again
    Given a change that plans "src/app.txt" is ready to work
    And the Builder in build mode also writes "tests/steps/sneaky.txt" with:
      """
      written in build mode
      """
    And the "Builder in build mode" says it cannot do the task because "the steps are wrong"
    When the change runs
    Then it fails with a message that contains "parked: the Builder in build mode says it cannot do the task: the steps are wrong"
    When the change runs
    Then it fails with a message that contains "parked: Work of the earlier run that its gates did not accept: tests/steps/sneaky.txt: protected path for builder-build"
    And the agents were called in this order:
      | role          | model  | arguments            |
      | builder-test  | sonnet | change=c1 mode=test  |
      | builder-build | sonnet | change=c1 mode=build |

  Scenario: Once the agent can do the task, the change runs again with all its attempts
    Given the project allows 3 attempts before a replan
    And a change that plans "src/app.txt" is ready to work
    And the Builder in build mode writes "src/app.txt" from call 4
    And the "Builder in build mode" says it cannot do the task because "the package does not say which currency to use"
    When the change runs
    Then the change parks
    Given the "Builder in build mode" can do the task now
    When the change runs
    Then the record of "c1" is in state "closed"
    And the agents were called in this order:
      | role          | model  | arguments            |
      | builder-test  | sonnet | change=c1 mode=test  |
      | builder-build | sonnet | change=c1 mode=build |
      | builder-test  | sonnet | change=c1 mode=test  |
      | builder-build | sonnet | change=c1 mode=build |
      | builder-build | sonnet | change=c1 mode=build |
      | builder-build | sonnet | change=c1 mode=build |
      | reviewer      | gpt-6  | change=c1 round=1    |
