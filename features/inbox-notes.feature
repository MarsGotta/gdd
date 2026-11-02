Feature: The Decision Inbox shows what still stands
  A launch settles the reasons a change parked, as it settles the open requests: they stay in
  the record and leave the inbox, and its last line counts them. The inbox of a change that was
  relaunched and closed no longer tells you to settle a conflict in a worktree that is gone. A
  human's KO at a gate of the change shows under Your decisions, with its note, until that gate
  is decided again; tminus gate saves the inbox right away.

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

  Scenario: A launch settles the reasons the change parked before it
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the change "c1" has an open parked note "conflicts in src/app.txt: merge main into change/c1 in its worktree (/tmp/gone)"
    When I run "tminus launch c1"
    And I run "tminus inbox c1"
    Then the output does not contain "in its worktree (/tmp/gone)"
    And the output contains "1 earlier reason(s) to park were settled by a later launch"
    And the record of "c1" is in state "closed"

  Scenario: The reasons of a parked change stay in its inbox until it is launched again
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the change "c1" has an open parked note "Reviewer KO or G3 red after one fix"
    When I run "tminus inbox c1"
    Then the output contains "## Parked"
    And the output contains "- Reviewer KO or G3 red after one fix"
    And the output does not contain "to park were settled"

  Scenario: The note of a G1 KO reaches the inbox
    Given I run "tminus new-change c1 --title App --paths src/app.txt features/app.feature"
    And the change "c1" has the scenario "features/app.feature" on its branch
    When I run "tminus gate G1 ko c1 --by human --note too-vague"
    Then it succeeds
    And the file ".tminus/changes/c1/inbox.md" contains "## Your decisions"
    And the file ".tminus/changes/c1/inbox.md" contains "- G1 ko: too-vague"
    When I run "tminus inbox c1"
    Then the output contains "- G1 ko: too-vague"

  Scenario: A G1 KO without a note still shows
    Given I run "tminus new-change c1 --title App --paths src/app.txt features/app.feature"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I run "tminus gate G1 ko c1 --by human"
    When I run "tminus inbox c1"
    Then the output contains "- G1 ko"

  Scenario: Approving G1 after a KO takes the KO out of the inbox
    Given I run "tminus new-change c1 --title App --paths src/app.txt features/app.feature"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I run "tminus gate G1 ko c1 --by human --note too-vague"
    When I run "tminus gate G1 ok c1 --by human --note fine"
    And I run "tminus inbox c1"
    Then the output does not contain "too-vague"
    And the output does not contain "## Your decisions"
