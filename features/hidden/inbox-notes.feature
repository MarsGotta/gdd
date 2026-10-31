@hidden
Feature: The Decision Inbox shows what still stands (hidden examples)

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

  Scenario: A launch settles a conflict that named a worktree
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has an open parked note "main moved under it and the merge has conflicts in src/app.txt: merge main into change/c1 in its worktree (/var/tmp/gone/c1)"
    When I run "gdd launch c1"
    And I run "gdd inbox c1"
    Then the output does not contain "(/var/tmp/gone/c1)"
    And the output contains "1 earlier reason(s) to park were settled by a later launch"

  Scenario: Another G1 KO note reaches the inbox
    Given I run "gdd new-change c1 --title App --paths src/app.txt features/app.feature"
    And the change "c1" has the scenario "features/app.feature" on its branch
    When I run "gdd gate G1 ko c1 --by human --note add-taxes"
    Then it succeeds
    And the file ".gdd/changes/c1/inbox.md" contains "- G1 ko: add-taxes"
