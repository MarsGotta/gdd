@hidden
Feature: A close test runs in a place of its own (hidden examples)

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
      unit = "test ! -f tests/broken.txt"
      lint = "test ! -f src/ugly.txt"

      [g3]
      acceptance_full = "true"
      lockfile = "true"
      secrets = "true"

      [selftest]
      file = "tests/broken.txt"
      content = "broken"

      [close_test.quality]
      file = "src/ugly.txt"
      content = "let   x=1"

      [close_test.security]
      file = "config/creds.txt"
      content = "github = {secret}"

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
    And the output contains "OK quality closes: its planted failure turns G2 red"
    And the file "config/creds.txt" does not exist
    And no file of the project contains "ghp_"
    And the project has no worktree besides the main checkout

  Scenario: A close test never touches the place where a task works
    Given the change "c2" plans "src/other.txt" and its build has started on its branch
    And the change "c2" has a worktree with the file "config/creds.txt"
    And the gate command "g3" "lockfile" is "touch locked.txt"
    When I run "tminus selftest"
    Then the output contains "OK security closes: its planted failure turns G3 red"
    And the file "locked.txt" does not exist
    And the file "config/creds.txt" does not exist
    And the worktree of "c2" has no "locked.txt"

  Scenario: After a close test the checkout is as it was, and a change starts
    Given I run "tminus new-change c2 --title Other --paths src/other.txt"
    And the change "c2" has the scenario "features/other.feature" on its branch
    And I am on the branch "change/c2"
    And I run "tminus gate G1 ok c2 --by human"
    And I am on the branch "main"
    And I run "tminus selftest"
    When I run "tminus launch c2"
    Then it succeeds
    And the record of "c2" is in state "in-pre"
    And the file ".tminus/changes/c2/inbox.md" does not contain "security: not seen to close"
    And the file ".tminus/changes/c2/inbox.md" does not contain "quality: not seen to close"

  Scenario: A security gate that misses the made secret does not close
    Given the gate command "g3" "secrets" is "echo nothing found"
    When I run "tminus selftest"
    Then the exit code is not 0
    And the output contains "KO security does not close: G3 stayed green with its planted failure"
    And the project has no worktree besides the main checkout

  Scenario: A close test sees the main line as it is, not the work left in the checkout
    Given the gate command "g2" "lint" is "test ! -f src/ugly.txt && test ! -f src/notes.txt"
    And the file "src/notes.txt" says "work in progress"
    When I run "tminus selftest"
    Then the exit code is not 0
    And the output contains "KO a failing test turns G2 red (G2 is red without it: lint)"
    And the output contains "OK quality closes: its planted failure turns G2 red"
    And the file "src/notes.txt" contains "work in progress"
