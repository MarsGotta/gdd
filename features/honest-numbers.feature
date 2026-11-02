Feature: Honest numbers
  The numbers tminus reports must be ones it can stand behind. The merge commit counts the agent
  calls of the change, the same calls `tminus report --change` counts, and never the gate commands.
  A call whose agent reports no cost (Codex, and the conversation with the human that
  `tminus session` imports) is priced from `[prices.<model>]` in `tminus/models.toml` and marked as an
  estimate with `~`; without a price for its model its cost is unknown, never 0. A change that
  merged twice (a polish at G4) carries its changelog line in both merge commits, and a release
  lists it once.

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

  Scenario: The merge commit counts the agent calls, not the gate commands
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    When I run "tminus launch c1"
    Then it succeeds
    And the last commit on main says "Tokens: in 250 · cache 150 · out 65"
    And the last commit on main says "· 3 agent call(s)"

  Scenario: With a price for its model, the Reviewer's cost is estimated and marked
    Given the models also say:
      """
      [prices.gpt-6]
      input = 20.0
      cache = 2.0
      output = 40.0
      """
    And I run "tminus new-change c1 --title App --paths src/app.txt"
    When I run "tminus launch c1"
    Then it succeeds
    When I run "tminus report --by role --change c1"
    Then the output contains "reviewer -> 1 · 50 · 50 · 25 · ~0.0021 · "
    And the output contains "builder-build -> 1 · 100 · 50 · 20 · 0.0100 · "
    And the output contains "TOTAL -> in 250 · cache 150 · out 65 · ~0.0221 USD"
    And the output contains "~ estimated from the prices in tminus/models.toml"

  Scenario: Without a price for its model, the Reviewer's cost is unknown, never 0
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    When I run "tminus launch c1"
    Then it succeeds
    And the last commit on main says "0.02 USD (estimate) + unknown (1 call(s) with no price)"
    When I run "tminus report --by role --change c1"
    Then the output contains "reviewer -> 1 · 50 · 50 · 25 · unknown · "
    And the output contains "TOTAL -> in 250 · cache 150 · out 65 · 0.0200 USD + unknown (1 call(s) with no price)"
    And the output contains "unknown: no price in tminus/models.toml for gpt-6"
    And the output does not contain "0.0000"

  Scenario: The machine-readable report says which costs are estimated or unknown
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    When I run "tminus launch c1"
    And I run "tminus report --by role --change c1 --json"
    Then the entry with "role" "reviewer" has "calls_without_price" 1 and "calls" 1
    And the entry with "role" "builder-build" has "calls_without_price" 0 and "calls" 1

  Scenario: The conversation with the human is priced from the table and marked
    Given the models also say:
      """
      [prices.claude-opus-5-5]
      input = 20.0
      cache = 2.0
      output = 50.0
      """
    And the Claude Code session "s1" used "claude-opus-5-5" with 40 output tokens
    When I run "tminus session --phase 1-discover"
    And I run "tminus report --by agent"
    Then the output contains "claude-interactive -> 1 · 10 · 5 · 40 · ~0.0022 · 0"
    And the output contains "TOTAL -> in 10 · cache 5 · out 40 · ~0.0022 USD"

  Scenario: Without a price, the conversation with the human costs unknown, never 0
    Given the Claude Code session "s1" used "claude-opus-5-5" with 40 output tokens
    When I run "tminus session --phase 1-discover"
    And I run "tminus report --by agent"
    Then the output contains "claude-interactive -> 1 · 10 · 5 · 40 · unknown · 0"
    And the output contains "TOTAL -> in 10 · cache 5 · out 40 · unknown (1 call(s) with no price)"

  Scenario: A polished change is listed once in the next version
    Given the versions are released by hand
    And I run "tminus new-change c1 --title App --paths src/app.txt"
    And I run "tminus changelog-line c1 Added Export notes as Markdown"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "tminus gate G1 ok c1 --by human"
    And I run "tminus launch c1"
    And I run "tminus resume c1 --g4 polish --note Shorter error messages"
    And I run "tminus resume c1 --g4 ok"
    When I run "tminus release"
    Then it succeeds
    And the output contains "Next version: v0.1.0"
    And the output contains "- Export notes as Markdown" exactly 1 times

  Scenario: The release writes a polished change's line once in the changelog
    Given main has the file "CHANGELOG.md" with:
      """
      # Changelog

      ## [Unreleased]
      """
    And I run "tminus new-change c1 --title App --paths src/app.txt"
    And I run "tminus changelog-line c1 Added Export notes as Markdown"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "tminus gate G1 ok c1 --by human"
    And I run "tminus launch c1"
    And I run "tminus resume c1 --g4 polish --note Shorter error messages"
    When I run "tminus resume c1 --g4 ok"
    Then it succeeds
    And the output contains "Released v0.1.0"
    And the file "CHANGELOG.md" contains "- Export notes as Markdown" exactly 1 times

  Scenario: tminus changelog shows a polished change's line once
    Given the versions are released by hand
    And I run "tminus new-change c1 --title App --paths src/app.txt"
    And I run "tminus changelog-line c1 Added Export notes as Markdown"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "tminus gate G1 ok c1 --by human"
    And I run "tminus launch c1"
    And I run "tminus resume c1 --g4 polish --note Shorter error messages"
    And I run "tminus resume c1 --g4 ok"
    When I run "tminus changelog"
    Then it succeeds
    And the output contains "- Export notes as Markdown" exactly 1 times
