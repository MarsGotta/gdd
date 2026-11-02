Feature: Where a change works, what it marks and what it keeps
  Each change works in a place of its own, which the blueprint makes or reuses for it (for
  software, a worktree on the change's branch), and lets go once the change ends: closed or
  rejected. A spike with variants has one place per variant, and each variant is built and judged
  in its own. The core reads where the work stands and what it touched there, and asks the
  blueprint to keep work only once it is accepted: after a park, what a person changed by hand is
  kept as theirs, the agents' work only when the gates of the earlier run accept it, and nothing
  at all while those gates refuse it. These scenarios hold today and must keep holding once the
  recipe no longer knows git.

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

  Scenario: A change rejected at its approval lets its place go
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "tminus gate G1 ok c1 --by human"
    And I run "tminus launch c1"
    When I run "tminus resume c1 --g4 ko --note Not what I asked"
    Then it succeeds
    And the record of "c1" is in state "rejected"
    And the change "c1" has no worktree
    And the current branch is "main"

  Scenario: A spike rejected at its approval lets the place of every variant go
    Given I run "tminus new-change c1 --type Spike --title Storage --paths src/app.txt --option SQLite --option Postgres"
    And the Builder in build mode also writes "docs/adr/0002-storage.md" with:
      """
      # 0002 · Where the notes live
      ## Decision
      The option of this variant.
      """
    And I run "tminus launch c1"
    When I run "tminus resume c1 --g4 ko --note Neither"
    Then it succeeds
    And the record of "c1" is in state "rejected"
    And the project has no worktree besides the main checkout

  Scenario: Each variant of a spike is built and judged in a place of its own
    Given I run "tminus new-change c1 --type Spike --title Storage --paths src/app.txt --option SQLite --option Postgres"
    And the Builder in build mode also writes "docs/adr/0002-storage.md" with:
      """
      # 0002 · Where the notes live
      ## Decision
      The option of this variant.
      """
    And the Builder in build mode writes nothing for variant 1
    When I run "tminus launch c1"
    Then it succeeds
    And the file ".tminus/changes/c1/inbox.md" contains "Variant 2 · Postgres · ready"
    And the branch "change/c1-v2" holds "src/app.txt"
    And the branch "change/c1-v1" does not hold "src/app.txt"
    And the branch "change/c1" does not hold "src/app.txt"

  Scenario: A polish pass is judged from where its work stands, so what a person kept there is not a Builder's
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "tminus gate G1 ok c1 --by human"
    And I run "tminus launch c1"
    And the file "tests/steps/helper.txt" in the worktree of "c1" is committed by hand saying "a helper step"
    When I run "tminus resume c1 --g4 polish --note Shorter error messages"
    Then it succeeds
    And the output contains "Paused at G4"
    And the agents were called in this order:
      | role          | model  | arguments                       |
      | builder-test  | sonnet | change=c1 mode=test             |
      | builder-build | sonnet | change=c1 mode=build            |
      | reviewer      | gpt-6  | change=c1 round=1               |
      | builder-build | sonnet | change=c1 mode=build fix=polish |
      | reviewer      | gpt-6  | change=c1 round=1               |
    And the branch "main" holds "tests/steps/helper.txt"

  Scenario: Without a gate that accepted the agents' earlier work, a relaunch keeps only what a person changed by hand
    Given the Builder in test mode also writes "src/sneaky.txt" with:
      """
      code test mode may not write
      """
    And I run "tminus new-change c1 --title App --paths src/app.txt"
    And I run "tminus launch c1"
    And the file "docs/notes.md" in the worktree of "c1" is written by hand saying "a note of mine"
    When I run "tminus launch c1"
    Then it fails with a message that contains "parked: Path Guard failed after test mode: src/sneaky.txt: protected path for builder-test"
    And the agents were called in this order:
      | role         | model  | arguments           |
      | builder-test | sonnet | change=c1 mode=test |
      | builder-test | sonnet | change=c1 mode=test |
    And the last commit on change/c1 says "changed by hand after the change parked"
    And the branch "change/c1" holds "docs/notes.md"
    And the branch "change/c1" does not hold "src/sneaky.txt"

  Scenario: Earlier work its gates refuse keeps nothing, not even what a person changed by hand
    Given the Builder in build mode also writes "tests/steps/sneaky.txt" with:
      """
      a step build mode wrote
      """
    And I run "tminus new-change c1 --title App --paths src/app.txt"
    And I run "tminus launch c1"
    And the file "docs/notes.md" in the worktree of "c1" is written by hand saying "a note of mine"
    When I run "tminus launch c1"
    Then it fails with a message that contains "parked: Work of the earlier run that its gates did not accept: tests/steps/sneaky.txt: protected path for builder-build"
    And the record of "c1" is in state "parked"
    And the branch "change/c1" does not hold "docs/notes.md"
    And the branch "change/c1" does not hold "tests/steps/sneaky.txt"
