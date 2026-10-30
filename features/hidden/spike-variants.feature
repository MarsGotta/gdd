@hidden
Feature: Spikes with variants (hidden examples)

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

  Scenario: Polish goes to one variant
    Given I run "gdd new-change c1 --type Spike --title Storage --paths src/app.txt --option SQLite --option Postgres"
    And the Builder in build mode also writes "docs/adr/0002-storage.md" with:
      """
      # 0002 · Where the notes live
      ## Decision
      The option of this variant.
      """
    And I run "gdd launch c1"
    When I run "gdd resume c1 --g4 polish --variant 2 --note Measure the cold start"
    Then it succeeds
    And agent call 3 includes "variant=2"
    And agent call 3 includes "fix=polish"
    And the record of "c1" is in state "in-pre"

  Scenario: KO leaves main as it was and keeps every branch
    Given I run "gdd new-change c1 --type Spike --title Storage --paths src/app.txt --option SQLite --option Postgres"
    And the Builder in build mode also writes "docs/adr/0002-storage.md" with:
      """
      # 0002 · Where the notes live
      ## Decision
      The option of this variant.
      """
    And I run "gdd launch c1"
    When I run "gdd resume c1 --g4 ko --note None convinces"
    Then it succeeds
    And the branch "main" does not hold "docs/adr/0002-storage.md"
    And the branch "change/c1-v1" holds "src/app.txt"
    And the branch "change/c1-v2" holds "src/app.txt"

  Scenario: Each variant has a budget of its own
    Given I run "gdd new-change c1 --type Spike --title Storage --paths src/app.txt --option SQLite --option Postgres"
    And the spike budget is "0.01" USD
    And the Builder in build mode writes nothing
    When I run "gdd launch c1"
    Then the agents were called in this order:
      | role          | model  | arguments                      |
      | builder-build | sonnet | change=c1 mode=spike variant=1 |
      | builder-build | sonnet | change=c1 mode=spike variant=2 |
