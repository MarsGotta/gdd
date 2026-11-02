Feature: Agents run only the commands their work needs
  tminus starts each agent with its role in TMINUS_ROLE: builder-test, builder-build, reviewer or
  orchestrator (the replan). Such an agent may run only what its work needs: tminus g2, guard,
  note, status, classify, inbox, report and scope, plus help and the version. Every other
  command refuses it before doing anything, so no agent can start, merge, park or release a
  change, adopt or set up the project, or write GDD state, even when its own permissions would
  let it. A person, with no TMINUS_ROLE, runs every command as before.

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

  Scenario: A Builder cannot launch a change
    When the role "builder-build" runs "tminus launch c1"
    Then it fails with a message that contains "Agents that tminus started cannot run tminus launch"

  Scenario: A Reviewer cannot adopt the project
    When the role "reviewer" runs "tminus adopt"
    Then it fails with a message that contains "Agents that tminus started cannot run tminus adopt"
    And the file "tminus/debt.toml" does not exist

  Scenario: The replanning Orchestrator cannot start a change
    When the role "orchestrator" runs "tminus new-change c2 --title Export"
    Then it fails with a message that contains "Agents that tminus started cannot run tminus new-change"
    And the branch "change/c2" does not exist

  Scenario Outline: An agent cannot run <command>
    When the role "builder-test" runs "<line>"
    Then it fails with a message that contains "Agents that tminus started cannot run tminus <command>"

    Examples:
      | command   | line                   |
      | prepare   | tminus prepare c1         |
      | merge     | tminus merge c1           |
      | park      | tminus park c1 stuck      |
      | bootstrap | tminus bootstrap          |
      | selftest  | tminus selftest           |
      | snapshot  | tminus snapshot c1        |
      | session   | tminus session --phase 2-build |
      | agents    | tminus agents install     |

  Scenario: The refusal names what an agent may run
    When the role "builder-build" runs "tminus launch c1"
    Then it fails with a message that contains "tminus g2, tminus guard, tminus note, tminus status, tminus classify, tminus inbox, tminus report and tminus scope"

  Scenario: An agent still reads where things stand
    When the role "builder-build" runs "tminus status"
    Then it succeeds

  Scenario: A person still runs every command
    When I run "tminus new-change c2 --title Export"
    Then it succeeds
