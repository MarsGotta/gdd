Feature: An agent can say it cannot do the task (hidden examples)

  Background:
    Given a project that follows the method, with the sensitive area "src/storage/**"

  Scenario: The replanning Orchestrator that says it cannot parks the change before one more build
    Given the project allows 2 attempts before a replan
    And a change that plans "src/app.txt" is ready to work
    And the Builder in build mode writes nothing
    And the "Orchestrator" says it cannot do the task because "only the human can choose where drafts are kept"
    When the change runs
    Then it fails with a message that contains "parked: the Orchestrator says it cannot do the task: only the human can choose where drafts are kept"
    And the agents were called in this order:
      | role          | model  | arguments            |
      | builder-test  | sonnet | change=c1 mode=test  |
      | builder-build | sonnet | change=c1 mode=build |
      | builder-build | sonnet | change=c1 mode=build |
      | orchestrator  | opus   | change=c1 reason=g2  |

  Scenario: A Reviewer that says it cannot gets no fix round, even with a KO
    Given a change that plans "src/app.txt" is ready to work
    And the Reviewer answers "VERDICT: KO" every time
    And the "Reviewer" says it cannot do the task because "the project's standards file is missing"
    When the change runs
    Then it fails with a message that contains "parked: the Reviewer says it cannot do the task: the project's standards file is missing"
    And the agents were called in this order:
      | role          | model  | arguments            |
      | builder-test  | sonnet | change=c1 mode=test  |
      | builder-build | sonnet | change=c1 mode=build |
      | reviewer      | gpt-6  | change=c1 round=1    |
