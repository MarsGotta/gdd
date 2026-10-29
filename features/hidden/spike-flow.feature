@hidden
Feature: A spike answers a question (hidden examples)

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

  Scenario: A spike may try a red zone: it never merges
    Given I run "gdd new-change c1 --type Spike --title Sync --paths src/storage/db.rs"
    And the Builder in build mode writes "src/storage/db.rs" from call 1
    And the Builder in build mode also writes "docs/adr/0002-storage.md" with:
      """
      # 0002 · Where the notes live
      ## Decision
      SQLite, one file per project.
      """
    When I run "gdd launch c1"
    Then it succeeds
    And the agents were called in this order:
      | role          | model | arguments            |
      | builder-build | opus  | change=c1 mode=spike |
    And the record of "c1" is in state "in-pre"
    And the last commit on main does not say "gdd(c1)"

  Scenario: A polished spike is built again and waits again, main untouched
    Given I run "gdd new-change c1 --type Spike --title Storage --paths src/app.txt"
    And the Builder in build mode also writes "docs/adr/0002-storage.md" with:
      """
      # 0002 · Where the notes live
      ## Decision
      SQLite, one file per project.
      """
    And I run "gdd launch c1"
    When I run "gdd resume c1 --g4 polish --note Measure the cold start too"
    Then it succeeds
    And agent call 2 includes "mode=spike fix=polish"
    And the record of "c1" is in state "in-pre"
    And the last commit on main does not say "gdd(c1)"

  Scenario: Path Guard still holds in a spike
    Given I run "gdd new-change c1 --type Spike --title Storage --paths src/app.txt"
    And the Builder in build mode writes "bin/tool" from call 1
    And the Builder in build mode also writes "docs/adr/0002-storage.md" with:
      """
      # 0002 · Where the notes live
      ## Decision
      SQLite, one file per project.
      """
    When I run "gdd launch c1"
    Then it fails with a message that contains "parked"
    And the branch "main" does not hold "bin/tool"
