Feature: The core of the method, other data
  Background:
    Given a project that follows the method, with the sensitive area "src/payments/**"

  Scenario: Risk comes from what the change touched, not from what it declared, in another area
    Given a change that plans "src/ui/button.txt" is at work
    When the change also touches "src/payments/card.txt"
    Then the change counts as sensitive

  Scenario: Another agent cannot give the human's approval of the result
    Given a change that plans "src/ui/button.txt" is opened
    When the "Builder in test mode" tries to record the human's approval of "the result"
    Then the approval is refused and the record of the change has none

  Scenario: With three attempts the change still parks when they run out
    Given the project allows 3 attempts before a replan
    And a change that plans "src/ui/button.txt" is ready to work
    And the Builder in build mode writes nothing
    When the change runs
    Then the change parks after 3 attempts and 1 replan
    And the change's messages tell the human why it parked
