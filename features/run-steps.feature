Feature: The single steps of the recipe
  The recipe that carries a change is made of single steps. Prepare checks that the change
  may enter Build and says which model plays each role. Park stops a change with a message
  for the human. Merge closes G3 into the main branch, with the change's record in the merge
  commit. The release steps take the merged change to pre, then to production, and roll it
  back if production gets worse.

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

  Scenario: Without the Setup, a change cannot start
    Given I run "gdd new-change c1 --title App --paths src/app.rs"
    And I am on the branch "change/c1"
    When I run "gdd prepare c1"
    Then it fails with a message that contains "G0 pending"

  Scenario: Without approved red zones, only spikes start
    Given the Setup is done
    And I run "gdd new-change c1 --title App --paths src/app.rs"
    And I am on the branch "change/c1"
    When I run "gdd prepare c1"
    Then it fails with a message that contains "Red zones not approved"

  Scenario: A red change needs its approved plan
    Given the Setup is done
    And the red zones are approved
    And I run "gdd new-change c1 --title Save --paths src/storage/db.rs"
    And I am on the branch "change/c1"
    When I run "gdd prepare c1"
    Then it fails with a message that contains "approve the plan"

  Scenario: The Reviewer must be of another model family than the Builder
    Given the Setup is done
    And the red zones are approved
    And the reviewer is "claude" "haiku"
    And I run "gdd new-change c1 --title App --paths src/app.rs"
    And I am on the branch "change/c1"
    When I run "gdd prepare c1"
    Then it fails with a message that contains "same model family"

  Scenario: Prepare runs on the change's own branch
    Given the Setup is done
    And the red zones are approved
    And I run "gdd new-change c1 --title App --paths src/app.rs"
    When I run "gdd prepare c1"
    Then it fails with a message that contains "Not on branch change/c1"

  Scenario: Prepare starts the build and names the model of each role
    Given the Setup is done
    And the red zones are approved
    And I run "gdd new-change c1 --title App --paths src/app.rs"
    And I am on the branch "change/c1"
    When I run "gdd prepare c1"
    Then the JSON field "color" is "yellow"
    And the JSON field "builder" is {"integration": "claude", "model": "sonnet"}
    And the JSON field "reviewer_model" is {"integration": "codex", "model": "gpt-6"}
    And the JSON field "retries" is 3
    And the JSON field "resume_green" is false
    And the record of "c1" is in state "building"

  Scenario: A relaunch clears the requests of the earlier run
    Given the Setup is done
    And the red zones are approved
    And I run "gdd new-change c1 --title App --paths src/app.rs"
    And I run "gdd note c1 needs Which format"
    And I am on the branch "change/c1"
    And I run "gdd prepare c1"
    When I run "gdd inbox c1"
    Then the output does not contain "Which format"

  Scenario: A relaunch whose code already passes G2 skips the Builder
    Given the Setup is done
    And the red zones are approved
    And I run "gdd new-change c1 --title App --paths src/app.rs"
    And the log has a Builder call in build mode for "c1"
    And I am on the branch "change/c1"
    When I run "gdd prepare c1"
    Then the JSON field "resume_green" is true

  Scenario: Parking stops the change with a message for the human
    Given I run "gdd new-change c1 --title App --paths src/app.rs"
    When I run "gdd park c1 G2 still red after 3 retries"
    Then it fails with a message that contains "parked: G2 still red after 3 retries"
    And the record of "c1" is in state "parked"
    And the file ".gdd/changes/c1/inbox.md" contains "G2 still red after 3 retries"

  Scenario: Merge needs G3
    Given I run "gdd new-change c1 --title App --paths src/app.rs"
    And I am on the branch "change/c1"
    When I run "gdd merge c1"
    Then it fails with a message that contains "G3 is not open"

  Scenario: Merge keeps the last work and records the change in the merge commit
    Given I run "gdd new-change c1 --title App --paths src/app.rs"
    And I am on the branch "change/c1"
    And I run "gdd gate G3 ok c1 --by machines"
    When the change writes "src/app.rs"
    And I run "gdd merge c1"
    Then it succeeds
    And the branch "main" holds "src/app.rs"
    And the last commit on main says "gdd(c1): G3 Merge Gate · App"
    And the last commit on main says "Color: yellow"
    And the record of "c1" is in state "merged"

  Scenario: Pre runs the tests and the demo
    Given I run "gdd new-change c1 --title App --paths src/app.rs"
    When I run "gdd release pre c1"
    Then it succeeds
    And the record of "c1" is in state "in-pre"

  Scenario: A change that needs G4 is not promoted without it
    Given I run "gdd new-change c1 --title Save --paths src/storage/db.rs"
    When I run "gdd release promote c1"
    Then it fails with a message that contains "needs a human G4"

  Scenario: A change without G4 is promoted by the machines
    Given I run "gdd new-change c1 --title App --paths src/app.rs"
    When I run "gdd release promote c1"
    Then it succeeds
    And the record of "c1" has the gate "G4" with decision "ok" by "machines"
    And the record of "c1" is in state "in-production"

  Scenario: Production getting worse rolls the change back
    Given I run "gdd new-change c1 --title App --paths src/app.rs"
    And the gate command "release" "observe" is "exit 1"
    When I run "gdd release observe c1"
    Then it fails with a message that contains "rolled back"
    And the record of "c1" is in state "rolled-back"
    And the file ".gdd/changes/c1/inbox.md" contains "A hotfix is needed"
