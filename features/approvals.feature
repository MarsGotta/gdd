Feature: Record the approvals
  Gates that need a person are recorded with gdd gate. The machines open G0; a human approves
  the red zones, the scenarios (G1), the red plan and the release (G4). Approving G1 seals the
  scenario files, so nobody can change them afterwards without a new approval. Builders and
  Reviewers cannot record any approval.

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
      color = "green"
      paths = ["docs/**"]

      [[zone]]
      color = "red"
      paths = ["src/storage/**"]
      reason = "Losing the user's drafts"
      """
    And everything is committed on main
    And I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs features/drafts.feature"

  Scenario: The red zones are approved by a human
    When I run "gdd gate zones ok --by human"
    Then it succeeds
    And the output contains "1 red zone(s)"
    And the log has a "zones" line for "-" with result "ok"

  Scenario: The red zones are not approved by the machines
    When I run "gdd gate zones ok --by machines"
    Then it fails with a message that contains "by a human"

  Scenario: Changing a red zone after its approval asks for it again
    Given I run "gdd gate zones ok --by human"
    And the red zone paths become "src/storage/**" and "src/sync/**"
    When I run "gdd status"
    Then the output contains "Red zones changed after their approval"

  Scenario: G0 opens only by the machines, with the Brief and a green self-test
    Given the self-test passed
    When I run "gdd gate G0 ok --by machines"
    Then it fails with a message that contains "docs/brief.md"
    When the change writes "docs/brief.md"
    And I run "gdd gate G0 ok --by machines"
    Then it succeeds
    When I run "gdd gate G0 ok --by human"
    Then it fails with a message that contains "--by machines"

  Scenario: Approving G1 seals the scenario files from the change's branch
    Given the change "c1" has the scenario "features/drafts.feature" on its branch
    When I run "gdd gate G1 ok c1 --by human"
    Then it succeeds
    And the record of "c1" seals "features/drafts.feature" with its current SHA-256
    And the log has a "G1" line for "c1" with result "ok"

  Scenario: G1 refuses hidden examples with sentences no visible scenario has
    Given the change "c1" has the scenario "features/drafts.feature" on its branch
    And the change "c1" has the hidden example "features/hidden/drafts.feature" with the step "Given a brand new sentence"
    When I run "gdd gate G1 ok c1 --by human"
    Then it fails with a message that contains "a brand new sentence"

  Scenario: The plan and G4 are recorded with their note
    When I run "gdd gate plan ok c1 --by human --note Looks-fine"
    Then it succeeds
    And the record of "c1" has the gate "plan" with decision "ok" by "human"

  Scenario: Approve and reject mean ok and ko
    When I run "gdd gate G4 reject c1 --by human"
    Then the record of "c1" has the gate "G4" with decision "ko" by "human"

  Scenario: Only ok or ko
    When I run "gdd gate G4 maybe c1 --by human"
    Then it fails with a message that contains "ok or ko"

  Scenario: A Builder cannot record an approval
    When the role "builder-build" runs "gdd gate G4 ok c1 --by human"
    Then it fails with a message that contains "cannot open or close gates"
