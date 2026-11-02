Feature: A gate counts only once it was seen to close
  A gate that never closes is not a gate: before its green is trusted, it is seen to turn red.
  The blueprint declares, for each gate, the stage its planted failure must turn red; the
  project writes the failure to plant (the software blueprint reads works' failure from
  [selftest], as before, and quality's from [close_test.quality]). tminus selftest runs each
  close test where the gate's commands look: the stage's commands of that gate without the
  failure (green), then with it (red), and gives the place back as it was. It keeps the result
  with the gate's lock: its commands and its planted failure. A gate seen to close with its lock
  as it is now is valid, and its green in a round is a key. A gate never seen to close is
  unproven: it still runs and its red still blocks, but its green proves nothing, and the status
  and the approval request at G4 name it. A gate that stays green with its planted failure does
  not close, and a gate whose lock changed since it closed must close again: no change starts
  until tminus selftest sees them close. Security and release get their close tests later.

  Background:
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["tminus/**", "bin/**", ".github/**"]
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
      unit = "test ! -f tests/failing.txt"
      lint = "test ! -f src/badly_formatted.txt"

      [g3]
      acceptance_full = "true"
      secrets = "true"

      [selftest]
      file = "tests/failing.txt"
      content = "fails"

      [close_test.quality]
      file = "src/badly_formatted.txt"
      content = "fn  badly ( ) { }"

      [release]
      simulated = true
      build = "true"
      deploy_pre = "true"
      tests_pre = "true"
      demo = ""
      promote = ""
      observe = ""
      rollback = ""
      rollback_pre = ""
      """
    And the file ".github/workflows/tminus.yml" says "run: tminus ci"
    And the scenario "features/base.feature" with 1 scenarios
    And everything is committed on main
    And the Setup is done
    And the red zones are approved
    And the agents are fakes

  Scenario: A gate counts as valid only once a planted failure turned it red
    When I run "tminus selftest"
    Then it succeeds
    And the output contains "OK works closes: its planted failure turns G2 red"
    And the output contains "OK quality closes: its planted failure turns G2 red"
    And the output contains "harness 4/4"
    And the file "src/badly_formatted.txt" does not exist

  Scenario: Once a gate closed, its green in a round is a key
    Given I run "tminus selftest"
    And the change "c1" plans "src/app.txt" and its build has started on its branch
    When I run "tminus g2 c1"
    Then the gate passes
    And the key of the gate "works" counts as proof
    And the key of the gate "quality" counts as proof

  Scenario: A gate that stays green with its planted failure is not valid
    Given the gate command "g2" "lint" is "true"
    When I run "tminus selftest"
    Then the exit code is not 0
    And the output contains "KO quality does not close: G2 stayed green with its planted failure"
    And the output contains "harness 4/4"

  Scenario: No change starts while a gate does not close
    Given the gate command "g2" "lint" is "true"
    And I run "tminus selftest"
    And I run "tminus new-change c1 --title App --paths src/app.txt"
    And I am on the branch "change/c1"
    When I run "tminus prepare c1"
    Then it fails with a message that contains "The gate quality does not close"

  Scenario: A gate whose lock changed after it closed must close again before the next change
    Given I run "tminus selftest"
    And the gate command "g2" "lint" is "test ! -f src/badly_formatted.txt && test ! -f src/other.txt"
    And I run "tminus new-change c1 --title App --paths src/app.txt"
    And I am on the branch "change/c1"
    When I run "tminus prepare c1"
    Then it fails with a message that contains "The gate quality must close again"

  Scenario: A gate that closes again with its new lock counts again
    Given I run "tminus selftest"
    And the gate command "g2" "lint" is "test ! -f src/badly_formatted.txt && test ! -f src/other.txt"
    And I run "tminus selftest"
    And the change "c1" plans "src/app.txt" and its build has started on its branch
    When I run "tminus g2 c1"
    Then the gate passes
    And the key of the gate "quality" counts as proof

  Scenario: A gate never seen to close still blocks when it is red
    Given I run "tminus selftest"
    And the gate command "g3" "secrets" is "echo leaked; false"
    And the change "c1" plans "src/app.txt" and its build has started on its branch
    When I run "tminus g3 c1"
    Then the gate fails on "secrets"
    And the key of the gate "security" is no proof

  Scenario: The approval request names the gates never seen to close
    Given I run "tminus selftest"
    And I run "tminus new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "tminus gate G1 ok c1 --by human"
    When I run "tminus launch c1"
    Then it succeeds
    And the record of "c1" is in state "in-pre"
    And the file ".tminus/changes/c1/inbox.md" contains "security: not seen to close"
    And the file ".tminus/changes/c1/inbox.md" does not contain "quality: not seen to close"

  Scenario: The status names the gates never seen to close
    Given I run "tminus selftest"
    When I run "tminus status"
    Then the output contains "Harness self-test: harness 4/4"
    And the output contains "seen to close: works, quality"
    And the output contains "unproven: security"
