Feature: A close test runs in a place of its own
  A close test plants a failure to see its gate turn red; planted where a change works, it would
  leave that work changed, and a scanner that marks new files for its diff would leave the
  checkout's index changed too, so no change could start. tminus selftest now runs each close
  test (but the one of works, which reads the first check's runs) in a place the blueprint makes
  for it alone from the main line as it is: the gate's commands of its stage run there without
  the failure, the blueprint plants the failure there, they run again, and the place goes. The
  checkout, its index included, and the places where changes work stay as they were. The
  security gate closes with a secret the blueprint makes at run time: the project writes in
  [close_test.security] where the secret goes and {secret} where it stands, so no secret is ever
  written in the project's files.

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
      dependencies = "true"
      secrets = "true"

      [selftest]
      file = "tests/failing.txt"
      content = "fails"

      [close_test.quality]
      file = "src/badly_formatted.txt"
      content = "fn  badly ( ) { }"

      [close_test.security]
      file = "src/leak.txt"
      content = "token = {secret}"

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
    And the gate command "g3" "secrets" is the one of the "rust" sheet

  Scenario: The security gate is seen to close with a secret made at run time
    When I run "tminus selftest"
    Then it succeeds
    And the output contains "OK security closes: its planted failure turns G3 red"
    And the file "src/leak.txt" does not exist
    And no file of the project contains "ghp_"
    And the project has no worktree besides the main checkout

  Scenario: A close test never touches the place where a task works
    Given the change "c1" plans "src/app.txt" and its build has started on its branch
    And the change "c1" has a worktree with the file "src/app.txt"
    And the gate command "g3" "dependencies" is "touch checked.txt"
    When I run "tminus selftest"
    Then the output contains "OK security closes: its planted failure turns G3 red"
    And the file "checked.txt" does not exist
    And the worktree of "c1" has no "checked.txt"
    And the worktree of "c1" has no "src/leak.txt"

  Scenario: After a close test the checkout is as it was, and a change starts
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "tminus gate G1 ok c1 --by human"
    And I am on the branch "main"
    And I run "tminus selftest"
    When I run "tminus launch c1"
    Then it succeeds
    And the record of "c1" is in state "in-pre"
    And the file ".tminus/changes/c1/inbox.md" does not contain "security: not seen to close"

  Scenario: Once the security gate closed, its green in a round is a key
    Given I run "tminus selftest"
    And the change "c1" plans "src/app.txt" and its build has started on its branch
    When I run "tminus g3 c1"
    Then the gate passes
    And the key of the gate "security" counts as proof

  Scenario: A security gate that misses the made secret does not close
    Given the gate command "g3" "secrets" is "true"
    When I run "tminus selftest"
    Then the exit code is not 0
    And the output contains "KO security does not close: G3 stayed green with its planted failure"
    And the project has no worktree besides the main checkout

  Scenario: A close test sees the main line as it is, not the work left in the checkout
    Given the gate command "g2" "lint" is "test ! -f src/badly_formatted.txt && test ! -f src/draft.txt"
    And the file "src/draft.txt" says "not kept yet"
    When I run "tminus selftest"
    Then the exit code is not 0
    And the output contains "KO a failing test turns G2 red (G2 is red without it: lint)"
    And the output contains "OK quality closes: its planted failure turns G2 red"
    And the file "src/draft.txt" contains "not kept yet"
