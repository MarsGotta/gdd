Feature: Only a human approves the scenarios, the plan and the result
  Three gates are the human's: G1 (the scenarios), the plan of a red change and G4 (the result).
  tminus gate records them only with --by human; with any other --by, the machines' or an agent's,
  it refuses, says that a human decides that gate, and records and logs nothing. The gates of
  the machines stay theirs: G0 is opened only by the machines, G2 and G3 are recorded as today,
  and the red zones stay the human's. No agent tminus started records anything, whatever it claims
  to be: the core refuses it as well as the command line.

  Background:
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["tminus/**", "bin/**"]
      builder_build = ["features/**", "tests/steps/**"]
      builder_test = ["src/**"]
      """
    And the zone map:
      """
      default = "yellow"

      [[zone]]
      color = "green"
      paths = ["docs/**"]

      [[zone]]
      color = "red"
      paths = ["src/storage/**"]
      reason = "Losing the user's drafts"
      """
    And everything is committed on main
    And I run "tminus new-change c1 --title Drafts --paths src/storage/db.rs features/drafts.feature"

  # Invariant: an agent's approval never replaces the human's.
  Scenario: An agent's approval never replaces the human's: the machines cannot approve the scenarios
    When I run "tminus gate G1 ok c1 --by machines"
    Then it fails with a message that contains "G1 is decided by a human (--by human)"
    And the file ".tminus/changes/c1/change.json" does not contain "G1"
    And the log has no "G1" line for "c1"

  Scenario Outline: Only a human records <gate> <decision>
    When I run "tminus gate <gate> <decision> c1 --by <by>"
    Then it fails with a message that contains "<gate> is decided by a human (--by human)"
    And the log has no "<gate>" line for "c1"

    Examples:
      | gate | decision | by           |
      | plan | ok       | machines     |
      | G4   | ok       | machines     |
      | G4   | ko       | machines     |
      | G1   | ok       | orchestrator |

  Scenario: A human still records the scenarios, the plan and the result
    When I run "tminus gate G1 ok c1 --by human"
    Then it succeeds
    When I run "tminus gate plan ok c1 --by human"
    Then it succeeds
    When I run "tminus gate G4 ko c1 --by human"
    Then it succeeds
    And the record of "c1" has the gate "G1" with decision "ok" by "human"
    And the record of "c1" has the gate "plan" with decision "ok" by "human"
    And the record of "c1" has the gate "G4" with decision "ko" by "human"

  Scenario: The machines still record the gates they run
    When I run "tminus gate G2 ok c1 --by machines"
    Then it succeeds
    When I run "tminus gate G3 ok c1 --by machines"
    Then it succeeds
    And the record of "c1" has the gate "G2" with decision "ok" by "machines"
    And the record of "c1" has the gate "G3" with decision "ok" by "machines"

  Scenario: The replanning agent cannot record the human's approval either
    When the role "orchestrator" runs "tminus gate G1 ok c1 --by human"
    Then it fails with a message that contains "Agents that tminus started cannot run tminus gate"
    And the file ".tminus/changes/c1/change.json" does not contain "G1"
