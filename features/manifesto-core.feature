Feature: The core of the method
  Gate-Driven Development has a core that no project, rules file or agent can switch off. Each
  scenario here states one invariant of that core in the method's own words: gate (what is
  checked), lock (how it is checked: the approved scenarios of a task, or the project's stock
  commands), guard (another agent who reviews), approval (the human's), level by risk, attempts,
  log. The words stay while the tool is restructured and renamed; only the steps under them change.
  Invariant 6 (gates only get stricter, and loosening one needs the human's approval) comes with a
  later Change.

  Background:
    Given a project that follows the method, with the sensitive area "src/storage/**"

  # Invariant 1: the agent cannot change its locks nor the rules that decide risk.
  Scenario: An agent cannot change the rules that decide risk
    Given a change that plans "src/app.txt" is at work
    When the "Builder in build mode" changes the rules that decide risk
    Then the agent's work is refused for touching the rules that decide risk

  # Invariant 1: the agent cannot change its locks nor the rules that decide risk.
  Scenario: An agent cannot change the project's locks
    Given a change that plans "src/app.txt" is at work
    When the "Builder in test mode" changes the project's locks
    Then the agent's work is refused for touching the project's locks

  # Invariant 2: risk is computed from what was touched, not from what was declared.
  Scenario: Risk comes from what the change touched, not from what it declared
    Given a change that plans "src/app.txt" is at work
    When the change also touches "src/storage/db.txt"
    Then the change counts as sensitive

  # Invariant 3: in the most sensitive areas the guard and the human's approval cannot be switched off.
  Scenario: Rules cannot take the guard from a sensitive area
    Given the rules that decide risk switch off "the guard" for sensitive changes
    When a change that plans "src/storage/db.txt" is opened
    Then the rules are refused for switching off the core

  # Invariant 3: in the most sensitive areas the guard and the human's approval cannot be switched off.
  Scenario: Rules cannot take the human's approval from a sensitive area
    Given the rules that decide risk switch off "the human's approval" for sensitive changes
    When a change that plans "src/storage/db.txt" is opened
    Then the rules are refused for switching off the core

  # Invariant 3: in the most sensitive areas the guard and the human's approval cannot be switched off.
  Scenario: A sensitive change always waits for the guard and the human's approval
    When a change that plans "src/storage/db.txt" is opened
    Then the change waits for the guard
    And the change waits for the human's approval

  # Invariant 4: an agent's approval never replaces the human's.
  Scenario: An agent cannot give the human's approval
    Given a change that plans "src/app.txt" is opened
    When the "Reviewer" tries to record the human's approval of "the scenarios"
    Then the approval is refused and the record of the change has none

  # Invariant 4: an agent's approval never replaces the human's.
  Scenario: An agent that writes the change's record itself is caught and the change parks
    Given a change that plans "src/app.txt" is ready to work
    And the Builder in build mode writes into the record of the change
    When the change runs
    Then the change parks
    And the record of the change is put back as it was before the agent's call

  # Invariant 4: the human's approval holds only for what was seen.
  Scenario: An approved scenario cannot change under the approval
    Given a change that plans "src/app.txt" is at work
    And the human approved its scenario "features/app.feature"
    When the "Builder in test mode" changes the approved scenario "features/app.feature"
    Then the agent's work is refused because "features/app.feature" changed after its approval

  # Invariant 4: a change after the approval needs the approval again.
  Scenario: Scenarios that change after their approval need it again
    Given the human approved the scenarios of a change that plans "src/app.txt"
    When a scenario of the change changes after the approval
    Then the change asks the human to approve its scenarios again

  # Invariant 5: attempts are limited; when they run out, the flow stops and asks.
  Scenario: When the attempts run out, the change stops and asks the human
    Given the project allows 2 attempts before a replan
    And a change that plans "src/app.txt" is ready to work
    And the Builder in build mode writes nothing
    When the change runs
    Then the change parks after 2 attempts and 1 replan
    And the change's messages tell the human why it parked

  # Invariant 7: everything is recorded: who did what, which gate judged it, what it cost.
  Scenario: The log keeps who did what, which gate judged it and what it cost
    Given a change that plans "src/app.txt" is ready to work
    When the change runs
    Then the log names the role and the cost of every agent's call
    And the log names each gate that judged the change and who judged it
    And the record of the change keeps what it spent
