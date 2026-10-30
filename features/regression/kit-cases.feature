Feature: The cases of the old kit's end-to-end suite, end to end
  The Python kit had an end-to-end suite of 16 cases (t0-t15, gdd-kit/tests/e2e.sh in the pilot
  repository). gdd's scenarios already cover twelve of them end to end: t0 "Without approved red
  zones, only spikes start", t1 "A yellow change without new scenarios goes all the way", t2 "A
  change with new scenarios stops at G4" and "Merge keeps the last work and records the change in
  the merge commit", t3 "Two Reviewer KOs park the change", t4 "The harness is protected from every
  role", t5 "A planned red change", t6 "A relaunch clears the requests of the earlier run", t7
  "Outside the plan, higher color, fails", t8 "The replan can send the change back to test mode",
  t9 "A relaunch whose code already passes G2 skips the Builder", t10 every launch (test mode
  writes protected steps and build mode is judged from after it), t13 "G1 refuses hidden examples
  with sentences no visible scenario has". The four below had only unit tests; here they run
  through gdd launch.

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
      "gpt" = "openai"
      """
    And the gate commands:
      """
      [g2]
      unit = "test -f src/app.txt"

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
    And the scenario "features/base.feature" with 1 scenarios
    And everything is committed on main
    And the Setup is done
    And the red zones are approved
    And the agents are fakes

  Scenario: t11 · Deleting the rules does not switch Path Guard off
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Builder in build mode deletes "gdd/rules.toml"
    When I run "gdd launch c1"
    Then it fails with a message that contains "parked"
    And the file ".gdd/changes/c1/g2.json" contains "gdd/rules.toml: protected path for builder-build"

  Scenario: t12 · Deleting a protected step written in test mode is caught
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Builder in build mode deletes "tests/steps/app.txt"
    When I run "gdd launch c1"
    Then it fails with a message that contains "parked"
    And the file ".gdd/changes/c1/g2.json" contains "tests/steps/app.txt: protected path for builder-build"

  Scenario: t14 · Gate commands swapped for checks that restore the file never run
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Builder in build mode swaps "gdd/commands.toml" for checks that put it back and write "swapped.ran"
    When I run "gdd launch c1"
    Then it fails with a message that contains "parked"
    And the file ".gdd/changes/c1/g2.json" contains "The gate commands did not run: Path Guard must pass first."
    And the worktree of "c1" has no "swapped.ran"

  Scenario: t15 · A protected change committed by an earlier run never runs on relaunch and stays visible
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Builder in build mode commits to "gdd/commands.toml" a G2 command that writes "planted.ran"
    And I run "gdd launch c1"
    When I run "gdd launch c1"
    Then it fails with a message that contains "parked"
    And the worktree of "c1" has no "planted.ran"
    And the file ".gdd/changes/c1/g2.json" contains "gdd/commands.toml: protected path"
