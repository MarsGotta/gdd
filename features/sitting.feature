Feature: The sitting
  `gdd sitting <change>` is the screen of a change's sitting: the scenarios as its branch has
  them, its hidden examples, the red zones waiting for approval with their reasons, and the plan
  of a red change. `a` approves and `c` asks for changes, always through a confirmation that
  shows exactly what will be recorded; nothing is written before `y`. Approving records G1 from
  the change's branch (sealing its scenario files), the red zones when they wait, and the plan
  of a red change. Asking for changes records G1 KO with the human's words.

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

  Scenario: The sitting shows the scenarios of the change's branch
    Given I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs features/drafts.feature"
    And the change "c1" has the scenario "features/drafts.feature" on its branch
    When I open the sitting of "c1"
    Then the sitting shows "c1 · Drafts · red"
    And the sitting shows "features/drafts.feature"
    And the sitting shows "Scenario: One"
    And the sitting shows "Then it is saved"

  Scenario: Hidden examples show too, marked as hidden
    Given I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs features/drafts.feature"
    And the change "c1" has the scenario "features/drafts.feature" on its branch
    And the change "c1" has the hidden example "features/hidden/drafts.feature" with the step "Given a draft"
    When I open the sitting of "c1"
    Then the sitting shows "features/hidden/drafts.feature · hidden: the Builders never see it"
    And the sitting shows "Scenario: Other"

  Scenario: Red zones waiting for approval show with their reason
    Given I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs features/drafts.feature"
    And the change "c1" has the scenario "features/drafts.feature" on its branch
    When I open the sitting of "c1"
    Then the sitting shows "Red zones to approve"
    And the sitting shows "src/storage/** · Losing the user's drafts"

  Scenario: Red zones already approved say so
    Given I run "gdd gate zones ok --by human"
    And I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs features/drafts.feature"
    And the change "c1" has the scenario "features/drafts.feature" on its branch
    When I open the sitting of "c1"
    Then the sitting shows "Red zones: approved"
    And the sitting does not show "Red zones to approve"

  Scenario: A red change shows its plan
    Given I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs features/drafts.feature"
    And the change "c1" has the scenario "features/drafts.feature" on its branch
    And the file ".gdd/changes/c1/plan.md" says:
      """
      # Plan

      Keep every draft in one file.
      """
    When I open the sitting of "c1"
    Then the sitting shows "Keep every draft in one file."

  Scenario: A yellow change needs no plan
    Given I run "gdd new-change c2 --title Export --paths src/export.rs features/export.feature"
    And the change "c2" has the scenario "features/export.feature" on its branch
    When I open the sitting of "c2"
    Then the sitting shows "c2 · Export · yellow"
    And the sitting shows "Plan: not needed for a yellow change"

  Scenario: Approving asks first and shows exactly what will be recorded
    Given I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs features/drafts.feature"
    And the change "c1" has the scenario "features/drafts.feature" on its branch
    When I open the sitting of "c1"
    And I press a at the sitting
    Then the sitting shows "This records, as you:"
    And the sitting shows "G1 ok on c1, sealing features/drafts.feature"
    And the sitting shows "red zones ok: 1 red zone(s)"
    And the sitting shows "plan ok on c1"
    And the sitting shows "y to record · n to go back"
    And the log has no "G1" line for "c1"

  Scenario: y records it all, G1 from the change's branch, and leaves no worktree
    Given I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs features/drafts.feature"
    And the change "c1" has the scenario "features/drafts.feature" on its branch
    When I open the sitting of "c1"
    And I press a at the sitting
    And I press y at the sitting
    Then the sitting shows "Recorded"
    And the sitting shows "Next: gdd launch c1"
    And the record of "c1" seals "features/drafts.feature" with its current SHA-256
    And the log has a "G1" line for "c1" with result "ok"
    And the log has a "zones" line for "-" with result "ok"
    And the record of "c1" has the gate "plan" with decision "ok" by "human"
    And the current branch is "main"
    And the project has no worktree besides the main checkout

  Scenario: n goes back and records nothing
    Given I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs features/drafts.feature"
    And the change "c1" has the scenario "features/drafts.feature" on its branch
    When I open the sitting of "c1"
    And I press a at the sitting
    And I press n at the sitting
    Then the sitting shows "Scenario: One"
    And the log has no "G1" line for "c1"

  Scenario: Asking for changes records G1 KO with the human's words
    Given I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs features/drafts.feature"
    And the change "c1" has the scenario "features/drafts.feature" on its branch
    When I open the sitting of "c1"
    And I press c at the sitting
    And I type "Two drafts at once" at the sitting
    And I press Enter at the sitting
    Then the sitting shows "G1 ko on c1: Two drafts at once"
    When I press y at the sitting
    Then the record of "c1" has the gate "G1" with decision "ko" by "human"
    And the log has no "zones" line for "-"

  Scenario: A yellow change with the red zones approved records only G1
    Given I run "gdd gate zones ok --by human"
    And I run "gdd new-change c2 --title Export --paths src/export.rs features/export.feature"
    And the change "c2" has the scenario "features/export.feature" on its branch
    When I open the sitting of "c2"
    And I press a at the sitting
    Then the sitting shows "G1 ok on c2"
    And the sitting does not show "red zones ok"
    And the sitting does not show "plan ok"

  Scenario: A G1 the gate refuses records nothing and says why
    Given I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs features/drafts.feature"
    And the change "c1" has the scenario "features/drafts.feature" on its branch
    And the change "c1" has the hidden example "features/hidden/drafts.feature" with the step "Given a brand new sentence"
    When I open the sitting of "c1"
    And I press a at the sitting
    And I press y at the sitting
    Then the sitting shows "a brand new sentence"
    And the sitting shows "Nothing recorded"
    And the log has no "zones" line for "-"
    And the project has no worktree besides the main checkout

  Scenario: A change past its sitting has nothing to approve
    Given I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs features/drafts.feature"
    And the change "c1" has the scenario "features/drafts.feature" on its branch
    And the change "c1" is in state "building"
    When I open the sitting of "c1"
    Then the sitting shows "c1 is building: its sitting is over"
    When I press a at the sitting
    Then the sitting does not show "This records"

  Scenario: q closes the sitting and nothing was written
    Given I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs features/drafts.feature"
    And the change "c1" has the scenario "features/drafts.feature" on its branch
    When I open the sitting of "c1"
    And I press q at the sitting
    Then the sitting is closed
    And the log has no "G1" line for "c1"

  Scenario: Without a terminal the sitting says what to use instead
    Given I run "gdd new-change c1 --title Drafts --paths src/storage/db.rs features/drafts.feature"
    And the change "c1" has the scenario "features/drafts.feature" on its branch
    When I run "gdd sitting c1"
    Then it fails with a message that contains "gdd sitting needs a terminal"
    And it fails with a message that contains "gdd gate G1 ok c1"
