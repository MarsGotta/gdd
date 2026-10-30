@hidden
Feature: Agents run only the commands their work needs (hidden examples)

  Background:
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["gdd/**", "bin/**"]
      builder_build = ["features/**", "tests/steps/**"]
      builder_test = ["src/**"]
      """
    And the gate commands:
      """
      [g2]
      unit = "true"
      """
    And everything is committed on main

  Scenario: A test-mode Builder cannot launch another change
    When the role "builder-test" runs "gdd launch c9"
    Then it fails with a message that contains "Builders and Reviewers cannot run gdd launch"

  Scenario: A Reviewer cannot start a change
    When the role "reviewer" runs "gdd new-change c3 --title Import"
    Then it fails with a message that contains "Builders and Reviewers cannot run gdd new-change"
    And the branch "change/c3" does not exist
