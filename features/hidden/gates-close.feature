@hidden
Feature: A gate counts only once it was seen to close (hidden examples)

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
      secrets = "true"

      [selftest]
      file = "tests/broken.txt"
      content = "broken"

      [close_test.quality]
      file = "src/ugly.txt"
      content = "let   x=1"

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

  Scenario: A gate that stays green with its planted failure is not valid
    Given the gate command "g2" "unit" is "true"
    When I run "tminus selftest"
    Then the exit code is not 0
    And the output contains "KO works does not close: G2 stayed green with its planted failure"

  Scenario: A gate whose lock changed after it closed must close again before the next change
    Given I run "tminus selftest"
    And the gate command "g2" "unit" is "test ! -f tests/broken.txt && test ! -f tests/other.txt"
    And I run "tminus new-change c1 --title App --paths src/app.txt"
    And I am on the branch "change/c1"
    When I run "tminus prepare c1"
    Then it fails with a message that contains "The gate works must close again"

  Scenario: A gate whose lock changed after it closed is no proof until it closes again
    Given I run "tminus selftest"
    And the gate command "g2" "lint" is "test ! -f src/ugly.txt && test ! -f src/other.txt"
    And the change "c1" plans "src/app.txt" and its build has started on its branch
    When I run "tminus g2 c1"
    Then the gate passes
    And the key of the gate "quality" is no proof
    And the key of the gate "works" counts as proof

  Scenario: The status names a gate that does not close
    Given the gate command "g2" "lint" is "true"
    And I run "tminus selftest"
    When I run "tminus status"
    Then the output contains "seen to close: works"
    And the output contains "unproven: security"
