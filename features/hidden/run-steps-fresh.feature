@hidden
Feature: Fresh hidden examples for the single steps of the recipe (written after the Builder read the first ones)

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
      color = "red"
      paths = ["src/storage/**"]
      reason = "Losing the user's drafts"
      """
    And the models:
      """
      [orchestrator]
      integration = "claude"
      model = "opus"

      [builder.yellow]
      integration = "claude"
      model = "sonnet"

      [builder.red]
      integration = "claude"
      model = "opus"

      [builder_test]
      integration = "claude"
      model = "sonnet"

      [reviewer]
      integration = "codex"
      model = "gpt-6"

      [families]
      "opus" = "anthropic"
      "sonnet" = "anthropic"
      "haiku" = "anthropic"
      "gpt" = "openai"
      """
    And the gate commands:
      """
      [g2]
      lint = "true"
      unit = "true"

      [g3]
      acceptance_full = "true"

      [release]
      simulated = true
      build = "true"
      deploy_pre = "true"
      tests_pre = "true"
      demo = ""
      promote = ""
      observe = ""
      rollback = ""
      """
    And everything is committed on main

  Scenario: A red change with a G4 recorded as ko is not promoted
    Given I run "gdd new-change c3 --title Archive --paths src/storage/archive.rs"
    And I run "gdd gate G4 ko c3 --by human"
    When I run "gdd release promote c3"
    Then it fails with a message that contains "needs a human G4"

  Scenario: Merge does not run from main
    Given I run "gdd new-change c3 --title Menu --paths src/menu.rs"
    And I run "gdd gate G3 ok c3 --by machines"
    When I run "gdd merge c3"
    Then it fails with a message that contains "Not on branch change/c3"

  Scenario: Parking keeps the exact reason
    Given I run "gdd new-change c3 --title Menu --paths src/menu.rs"
    When I run "gdd park c3 Reviewer KO twice on the same point"
    Then it fails with a message that contains "parked: Reviewer KO twice on the same point"
    And the record of "c3" is in state "parked"
    And the file ".gdd/changes/c3/inbox.md" contains "Reviewer KO twice on the same point"

  Scenario: A red change cannot use a Reviewer of the Builder's family
    Given the Setup is done
    And the red zones are approved
    And the reviewer is "claude" "sonnet"
    And I run "gdd new-change c3 --title Archive --paths src/storage/archive.rs"
    And I run "gdd gate plan ok c3 --by human"
    And I am on the branch "change/c3"
    When I run "gdd prepare c3"
    Then it fails with a message that contains "same model family"

  Scenario: Any failing observe rolls back and asks for a hotfix
    Given I run "gdd new-change c3 --title Menu --paths src/menu.rs"
    And the gate command "release" "observe" is "exit 3"
    When I run "gdd release observe c3"
    Then it fails with a message that contains "rolled back"
    And the record of "c3" is in state "rolled-back"
    And the file ".gdd/changes/c3/inbox.md" contains "A hotfix is needed"
