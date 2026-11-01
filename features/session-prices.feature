Feature: Prices come from main
  The prices of the models ([prices.<model>] in tminus/models.toml) are read as the main branch
  has them, as the zone map is: tminus session, run on a change's branch made before a price was
  added, still prices the conversation with the human, and tminus launch, run from the branch of
  the change, still prices its agents. A price that only a branch has does not count.

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

  Scenario: A session imported on a branch made before the prices is priced from main
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the models also say:
      """
      [prices.claude-opus-5-5]
      input = 20.0
      cache = 2.0
      output = 50.0
      """
    And I am on the branch "change/c1"
    And the Claude Code session "s1" used "claude-opus-5-5" with 40 output tokens
    When I run "tminus session --phase 2-build --change c1"
    And I run "tminus report --by agent"
    Then the output contains "claude-interactive -> 1 · 10 · 5 · 40 · ~0.0022 · 0"
    And the output does not contain "unknown"

  Scenario: A price that only a change's branch has does not price the session
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And I am on the branch "change/c1"
    And the models also say:
      """
      [prices.claude-opus-5-5]
      input = 20.0
      cache = 2.0
      output = 50.0
      """
    And the Claude Code session "s1" used "claude-opus-5-5" with 40 output tokens
    When I run "tminus session --phase 2-build --change c1"
    And I run "tminus report --by agent"
    Then the output contains "claude-interactive -> 1 · 10 · 5 · 40 · unknown · 0"

  Scenario: A change launched from its own branch prices its agents from main
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the models also say:
      """
      [prices.gpt-6]
      input = 20.0
      cache = 2.0
      output = 40.0
      """
    And I am on the branch "change/c1"
    When I run "tminus launch c1"
    Then it succeeds
    When I run "tminus report --by role --change c1"
    Then the output contains "reviewer -> 1 · 50 · 50 · 25 · ~0.0021 · "
