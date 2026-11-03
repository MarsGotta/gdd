@hidden
Feature: Bringing a change up to date and joining it to the main line (hidden examples)

  Background:
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["tminus/**", "bin/**"]
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

  Scenario: When the main line moved with a roadmap, the change takes it in a merge of its own and passes G3 again
    Given I run "tminus new-change c1 --title Notes --paths src/app.txt"
    And the main branch gets the file "docs/roadmap.md" during the Reviewer call
    When I run "tminus launch c1"
    Then it succeeds
    And the last commit on change/c1 says "tminus(c1): take the latest main"
    And the file "docs/roadmap.md" on the branch "change/c1" contains "main moved on"
    And the record of "c1" has the gate "G3" with decision "ok" by "machines"
    And the branch "main" holds "src/app.txt"

  Scenario: A conflict on a step file parks the change with its work kept on its line and G3 closed
    Given I run "tminus new-change c1 --title Notes --paths src/app.txt"
    And the main branch gets the file "tests/steps/app.txt" during the Reviewer call
    When I run "tminus launch c1"
    Then it fails with a message that contains "conflicts in tests/steps/app.txt"
    And the record of "c1" has the gate "G3" with decision "ko" by "machines"
    And the last commit on change/c1 says "tminus(c1): change work"
    And the file "tests/steps/app.txt" on the branch "change/c1" does not contain "main moved on"

  Scenario: Launched from its own line, the change keeps a to-do left in the main checkout as the earlier run's work
    Given I run "tminus new-change c1 --title Notes --paths src/app.txt"
    And I am on the branch "change/c1"
    And the file "notes/todo.md" says:
      """
      check the drafts before joining
      """
    When I run "tminus launch c1"
    Then it succeeds
    And the output contains "Change c1 closed"
    And the current branch is "main"
    And the branch "change/c1" holds "notes/todo.md"
    And the branch "main" holds "notes/todo.md"
