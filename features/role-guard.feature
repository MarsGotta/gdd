Feature: Agents run only the commands their work needs
  gdd starts each agent with its role in GDD_ROLE: builder-test, builder-build, reviewer or
  orchestrator (the replan). Such an agent may run only what its work needs: gdd g2, guard,
  note, status, classify, inbox, report and scope, plus help and the version. Every other
  command refuses it before doing anything, so no agent can start, merge, park or release a
  change, adopt or set up the project, or write GDD state, even when its own permissions would
  let it. A person, with no GDD_ROLE, runs every command as before.

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

  Scenario: A Builder cannot launch a change
    When the role "builder-build" runs "gdd launch c1"
    Then it fails with a message that contains "Builders and Reviewers cannot run gdd launch"

  Scenario: A Reviewer cannot adopt the project
    When the role "reviewer" runs "gdd adopt"
    Then it fails with a message that contains "Builders and Reviewers cannot run gdd adopt"
    And the file "gdd/debt.toml" does not exist

  Scenario: The replanning Orchestrator cannot start a change
    When the role "orchestrator" runs "gdd new-change c2 --title Export"
    Then it fails with a message that contains "Builders and Reviewers cannot run gdd new-change"
    And the branch "change/c2" does not exist

  Scenario Outline: An agent cannot run <command>
    When the role "builder-test" runs "<line>"
    Then it fails with a message that contains "Builders and Reviewers cannot run gdd <command>"

    Examples:
      | command   | line                   |
      | prepare   | gdd prepare c1         |
      | merge     | gdd merge c1           |
      | park      | gdd park c1 stuck      |
      | bootstrap | gdd bootstrap          |
      | selftest  | gdd selftest           |
      | snapshot  | gdd snapshot c1        |
      | session   | gdd session --phase 2-build |
      | agents    | gdd agents install     |

  Scenario: The refusal names what an agent may run
    When the role "builder-build" runs "gdd launch c1"
    Then it fails with a message that contains "gdd g2, gdd guard, gdd note, gdd status, gdd classify, gdd inbox, gdd report and gdd scope"

  Scenario: An agent still reads where things stand
    When the role "builder-build" runs "gdd status"
    Then it succeeds

  Scenario: A person still runs every command
    When I run "gdd new-change c2 --title Export"
    Then it succeeds
