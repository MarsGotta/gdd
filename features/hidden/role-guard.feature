@hidden
Feature: Agents run only the commands their work needs (hidden examples)

  Background:
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["tminus/**", "bin/**"]
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
    When the role "builder-test" runs "tminus launch c9"
    Then it fails with a message that contains "Agents that tminus started cannot run tminus launch"

  Scenario: A Reviewer cannot start a change
    When the role "reviewer" runs "tminus new-change c3 --title Import"
    Then it fails with a message that contains "Agents that tminus started cannot run tminus new-change"
    And the branch "change/c3" does not exist
