Feature: Spikes with variants
  A spike can try several options of one question at once (Toyota's set-based design: explore a
  set of alternatives and narrow it down). `gdd new-change --type Spike --option A --option B`
  builds one variant per option, each by its own Builder on its own branch, with the gate and the
  budget of a spike; `--variants N` builds N variants of the same brief. The G4 page puts the
  variants on one page; the human picks one with `--variant`, and only its ADR reaches main, with
  the variants that were not chosen listed in it. No variant's code ever reaches main.

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

  Scenario: A spike with options builds one variant per option, each on its own branch
    Given I run "gdd new-change c1 --type Spike --title Storage --paths src/app.txt --option SQLite --option Postgres"
    And the Builder in build mode also writes "docs/adr/0002-storage.md" with:
      """
      # 0002 · Where the notes live
      ## Decision
      The option of this variant.
      """
    When I run "gdd launch c1"
    Then it succeeds
    And the agents were called in this order:
      | role          | model  | arguments                      |
      | builder-build | sonnet | change=c1 mode=spike variant=1 |
      | builder-build | sonnet | change=c1 mode=spike variant=2 |
    And the branch "change/c1-v1" holds "src/app.txt"
    And the branch "change/c1-v2" holds "src/app.txt"
    And the record of "c1" is in state "in-pre"
    And the last commit on main does not say "gdd(c1)"

  Scenario: The G4 page puts the variants on one page
    Given I run "gdd new-change c1 --type Spike --title Storage --paths src/app.txt --option SQLite --option Postgres"
    And the Builder in build mode also writes "docs/adr/0002-storage.md" with:
      """
      # 0002 · Where the notes live
      ## Decision
      The option of this variant.
      """
    And I run "gdd launch c1"
    When I open the sitting of "c1"
    Then the sitting shows "Variant 1 · SQLite"
    And the sitting shows "Variant 2 · Postgres"
    And the sitting shows "docs/adr/0002-storage.md"

  Scenario: OK with a variant brings its ADR to main, with the variants not chosen
    Given I run "gdd new-change c1 --type Spike --title Storage --paths src/app.txt --option SQLite --option Postgres"
    And the Builder in build mode also writes "docs/adr/0002-storage.md" with:
      """
      # 0002 · Where the notes live
      ## Decision
      The option of this variant.
      """
    And I run "gdd launch c1"
    When I run "gdd resume c1 --g4 ok --variant 2 --note Postgres scales"
    Then it succeeds
    And the last commit on main says "gdd(c1): Spike ADR · Storage"
    And the file "docs/adr/0002-storage.md" on the branch "main" contains "Chosen: variant 2 (Postgres)"
    And the file "docs/adr/0002-storage.md" on the branch "main" contains "Variant 1 (SQLite): not chosen, branch change/c1-v1"
    And the branch "main" does not hold "src/app.txt"
    And the record of "c1" is in state "closed"

  Scenario: OK on a spike with variants needs the variant
    Given I run "gdd new-change c1 --type Spike --title Storage --paths src/app.txt --option SQLite --option Postgres"
    And the Builder in build mode also writes "docs/adr/0002-storage.md" with:
      """
      # 0002 · Where the notes live
      ## Decision
      The option of this variant.
      """
    And I run "gdd launch c1"
    When I run "gdd resume c1 --g4 ok"
    Then it fails with a message that contains "pick a variant: gdd resume c1 --g4 ok --variant 1|2"
    And the record of "c1" is in state "in-pre"

  Scenario: A variant that fails its gate does not stop the others
    Given I run "gdd new-change c1 --type Spike --title Storage --paths src/app.txt --option SQLite --option Postgres"
    And the Builder in build mode also writes "docs/adr/0002-storage.md" with:
      """
      # 0002 · Where the notes live
      ## Decision
      The option of this variant.
      """
    And the Builder in build mode writes nothing for variant 1
    When I run "gdd launch c1"
    Then it succeeds
    And the file ".gdd/changes/c1/inbox.md" contains "Variant 1 · SQLite · parked"
    And the record of "c1" is in state "in-pre"
    When I run "gdd resume c1 --g4 ok --variant 1"
    Then it fails with a message that contains "variant 1 did not pass its gate"

  Scenario: Variants of the same brief
    Given I run "gdd new-change c1 --type Spike --title Storage --paths src/app.txt --variants 3"
    And the Builder in build mode also writes "docs/adr/0002-storage.md" with:
      """
      # 0002 · Where the notes live
      ## Decision
      The option of this variant.
      """
    When I run "gdd launch c1"
    Then it succeeds
    And agent call 3 includes "variant=3"
    And the branch "change/c1-v3" holds "src/app.txt"

  Scenario: A spike has 2 to 4 variants, and only a spike has them
    When I run "gdd new-change c1 --type Spike --title Storage --paths src/app.txt --variants 5"
    Then it fails with a message that contains "a spike has 2 to 4 variants"
    When I run "gdd new-change c2 --title App --paths src/app.txt --option A --option B"
    Then it fails with a message that contains "only a Spike has variants"

  Scenario: The Builder is told how to build a variant
    When I run "gdd agents install"
    Then it succeeds
    And the file ".claude/skills/gdd-build/SKILL.md" contains "variant=<n>"
    And the file ".claude/skills/gdd-build/SKILL.md" contains "Build only your option"
