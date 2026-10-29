Feature: A spike answers a question
  A Spike is an experiment that answers a question before the real work: its code is written fast
  and thrown away (Extreme Programming's spike solution). What stays is the decision, in an ADR
  under docs/adr/. So a Spike is built without tests first, mutation testing or the Reviewer; its
  gate is Path Guard and its ADR; it never merges; it waits for the human's G4, and OK brings only
  its ADR to main, while its code stays on its branch. It is built within a fixed budget.

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

  Scenario: A spike is built and waits at G4 without touching main
    Given I run "gdd new-change c1 --type Spike --title Storage --paths src/app.txt"
    And the Builder in build mode also writes "docs/adr/0002-storage.md" with:
      """
      # 0002 · Where the notes live
      ## Decision
      SQLite, one file per project.
      """
    When I run "gdd launch c1"
    Then it succeeds
    And the output contains "docs/adr/0002-storage.md"
    And the agents were called in this order:
      | role          | model  | arguments            |
      | builder-build | sonnet | change=c1 mode=spike |
    And the record of "c1" is in state "in-pre"
    And the last commit on main does not say "gdd(c1)"

  Scenario: OK at G4 brings only the ADR to main
    Given I run "gdd new-change c1 --type Spike --title Storage --paths src/app.txt"
    And the Builder in build mode also writes "docs/adr/0002-storage.md" with:
      """
      # 0002 · Where the notes live
      ## Decision
      SQLite, one file per project.
      """
    And I run "gdd launch c1"
    When I run "gdd resume c1 --g4 ok"
    Then it succeeds
    And the last commit on main says "gdd(c1): Spike ADR · Storage"
    And the branch "main" holds "docs/adr/0002-storage.md"
    And the branch "main" does not hold "src/app.txt"
    And the branch "change/c1" holds "src/app.txt"
    And the record of "c1" is in state "closed"

  Scenario: KO at G4 leaves main as it was
    Given I run "gdd new-change c1 --type Spike --title Storage --paths src/app.txt"
    And the Builder in build mode also writes "docs/adr/0002-storage.md" with:
      """
      # 0002 · Where the notes live
      ## Decision
      SQLite, one file per project.
      """
    And I run "gdd launch c1"
    When I run "gdd resume c1 --g4 ko --note Not this"
    Then it succeeds
    And the branch "main" does not hold "docs/adr/0002-storage.md"
    And the record of "c1" is in state "rejected"

  Scenario: A spike that leaves no ADR does not reach G4
    Given I run "gdd new-change c1 --type Spike --title Storage --paths src/app.txt"
    When I run "gdd launch c1"
    Then it fails with a message that contains "parked: G2 still red"
    And the file ".gdd/changes/c1/inbox.md" contains "a Spike leaves its decision in a new file under docs/adr/ with a ## Decision section"
    And the record of "c1" is in state "parked"

  Scenario: A spike stops at its budget
    Given I run "gdd new-change c1 --type Spike --title Storage --paths src/app.txt"
    And the spike budget is "0.02" USD
    And the Builder in build mode writes nothing
    When I run "gdd launch c1"
    Then it fails with a message that contains "parked: the spike spent 0.02 USD of its budget of 0.02 USD"
    And the agents were called in this order:
      | role          | model  | arguments            |
      | builder-build | sonnet | change=c1 mode=spike |
      | builder-build | sonnet | change=c1 mode=spike |
    And the record of "c1" is in state "parked"

  Scenario: The Builder is told what a spike is for
    When I run "gdd agents install"
    Then it succeeds
    And the file ".claude/skills/gdd-build/SKILL.md" contains "mode=spike"
    And the file ".claude/skills/gdd-build/SKILL.md" contains "Write the decision in a new file under docs/adr/"

  Scenario: A new project gives spikes a budget
    Given an empty folder
    When I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"
    Then it succeeds
    And the TOML file "gdd/rules.toml" has "spike.max_usd" = "5"
