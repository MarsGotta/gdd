Feature: Bringing a change up to date and joining it to the main line
  A change is joined to the main line one at a time, once its gates passed. When the main line
  moved since the change passed G3, the change first takes the latest main line in a merge of its
  own, never rewriting its work: what the main line brought is not the change's doing, so its build
  starts again there, and it passes G3 again on top of it before it is joined. A conflict parks
  it with its work kept on its line and G3 closed. A polish pass that brings nothing new joins
  nothing again. A launch from the change's own line keeps what was left in the main checkout as
  the earlier run's work before the change gets its place. The merge says the change's color, as
  the core classified it. tminus merge, run where the change's work is, does all of this too.
  These scenarios hold today and must keep holding once the recipe asks the blueprint to bring a
  change up to date and to join it.

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

  Scenario: When the main line moved, the change takes it in a merge of its own and passes G3 again
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the main branch gets the file "docs/news.md" during the Reviewer call
    When I run "tminus launch c1"
    Then it succeeds
    And the last commit on change/c1 says "tminus(c1): take the latest main"
    And the file "docs/news.md" on the branch "change/c1" contains "main moved on"
    And the record of "c1" has the gate "G3" with decision "ok" by "machines"
    And the branch "main" holds "src/app.txt"

  Scenario: What the main line brought is not the change's doing, so its build starts again where it took it
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the main branch gets the file "tests/steps/news.txt" during the Reviewer call
    When I run "tminus launch c1"
    Then it succeeds
    And the output contains "main moved under c1"
    And the branch "main" holds "tests/steps/news.txt"
    And the record of "c1" is in state "closed"

  Scenario: A conflict with the main line parks the change with its work kept on its line and G3 closed
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the main branch gets the file "src/app.txt" during the Reviewer call
    When I run "tminus launch c1"
    Then it fails with a message that contains "conflicts in src/app.txt"
    And the record of "c1" has the gate "G3" with decision "ko" by "machines"
    And the last commit on change/c1 says "tminus(c1): change work"
    And the file "src/app.txt" on the branch "change/c1" does not contain "main moved on"

  Scenario: A polish pass that brings nothing new to the main line joins nothing again
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "tminus gate G1 ok c1 --by human"
    And I run "tminus launch c1"
    And the Builder in build mode writes nothing
    When I run "tminus resume c1 --g4 polish --note Nothing to change"
    Then it succeeds
    And the output contains "Paused at G4"
    And the file ".tminus/log.jsonl" contains "deploy_pre" exactly 1 times

  Scenario: Launched from its own line, the change keeps what was left in the main checkout as the earlier run's work
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And I am on the branch "change/c1"
    And the file "docs/notes.md" says:
      """
      a note left on the change's line
      """
    When I run "tminus launch c1"
    Then it succeeds
    And the output contains "Change c1 closed"
    And the current branch is "main"
    And the branch "change/c1" holds "docs/notes.md"
    And the branch "main" holds "docs/notes.md"

  Scenario: The merge says the change's color, as the core classified it
    Given I run "tminus new-change c1 --title Storage --paths src/storage/notes.txt"
    And I run "tminus gate plan ok c1 --by human"
    When I run "tminus launch c1"
    Then it succeeds
    And the output contains "Paused at G4"
    And the last commit on main says "Color: red"

  Scenario: tminus merge, run where the change's work is, takes the moved main line before it joins it
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the change "c1" has a worktree with the file "src/app.txt"
    And I run "tminus gate G3 ok c1 --by machines"
    And main has the file "docs/news.md" with:
      """
      news on the main line
      """
    When I run "tminus merge c1" in the worktree of "c1"
    Then it succeeds
    And the last commit on change/c1 says "tminus(c1): take the latest main"
    And the branch "main" holds "docs/news.md"
    And the branch "main" holds "src/app.txt"
    And the last commit on main says "tminus(c1): G3 Merge Gate · App"
