Feature: The person answers G4 through their agent
  At G4 the Decision Inbox asks the person to answer their agent, or the sitting screen: OK,
  KO or polish with their notes. The exact gdd resume command stays in the request, as what
  the agent runs; the person never types it.

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

  Scenario: At G4 the Decision Inbox asks the person to answer their agent
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    When I run "gdd launch c1"
    Then it succeeds
    And the output contains "Paused at G4"
    And the file ".gdd/changes/c1/inbox.md" contains "then answer your agent (or the sitting screen)"
    And the file ".gdd/changes/c1/inbox.md" contains "Your agent runs gdd resume c1 --g4 ok, --g4 ko or --g4 polish --note"
    And the file ".gdd/changes/c1/inbox.md" does not contain "then answer with gdd resume"

  Scenario: Once answered, the request of G4 leaves the Decision Inbox
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    When I run "gdd resume c1 --g4 ok"
    Then it succeeds
    And the file ".gdd/changes/c1/inbox.md" does not contain "answer your agent"
    And the file ".gdd/changes/c1/inbox.md" contains "1 earlier request(s) were resolved"
