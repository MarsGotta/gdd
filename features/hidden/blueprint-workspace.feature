@hidden
Feature: Where a change works, what it marks and what it keeps (hidden examples)

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

  Scenario: A change rejected at its approval lets its place go, wherever the places live
    Given the project keeps its worktrees in "../trees"
    And I run "tminus new-change c1 --title Export --paths src/export.txt"
    And the change "c1" has the scenario "features/export.feature" on its branch
    And I am on the branch "change/c1"
    And I run "tminus gate G1 ok c1 --by human"
    And I run "tminus launch c1"
    When I run "tminus resume c1 --g4 ko --note Wrong format"
    Then it succeeds
    And the record of "c1" is in state "rejected"
    And the change "c1" has no worktree
    And the current branch is "main"

  Scenario: A spike of three variants rejected at its approval lets every place go
    Given I run "tminus new-change c1 --type Spike --title Storage --paths src/app.txt --variants 3"
    And the Builder in build mode also writes "docs/adr/0002-storage.md" with:
      """
      # 0002 · Where the notes live
      ## Decision
      The option of this variant.
      """
    And the Builder in build mode writes nothing for variant 2
    And I run "tminus launch c1"
    When I run "tminus resume c1 --g4 ko --note None of them"
    Then it succeeds
    And the record of "c1" is in state "rejected"
    And the project has no worktree besides the main checkout

  Scenario: A relaunch keeps only the note a person wrote, without the code test mode wrote
    Given the Builder in test mode also writes "src/cache.txt" with:
      """
      a cache test mode may not write
      """
    And I run "tminus new-change c1 --title App --paths src/app.txt"
    And I run "tminus launch c1"
    And the file "notes/todo.md" in the worktree of "c1" is written by hand saying "check the drafts"
    When I run "tminus launch c1"
    Then it fails with a message that contains "parked: Path Guard failed after test mode: src/cache.txt: protected path for builder-test"
    And the last commit on change/c1 says "changed by hand after the change parked"
    And the branch "change/c1" holds "notes/todo.md"
    And the branch "change/c1" does not hold "src/cache.txt"
