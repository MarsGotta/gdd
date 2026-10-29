Feature: The G4 page says how the change works
  At G4 the human approves code she has not read line by line. Approving what nobody understands
  piles up comprehension debt. So the Reviewer, which has just read the whole change, ends an OK
  review with a short "How it works" in plain words: where the change sits in the code, how it
  fits with what was there, the pattern it sets or follows, and one thing worth knowing later.
  The G4 page and the Decision Inbox show it first, apart from the findings, and say so when the
  Reviewer did not write it.

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

  Scenario: The G4 page shows how the change works
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    And the file ".gdd/changes/c1/review.md" says:
      """
      No blocking findings.

      ## How it works
      The export reads each note once and writes one Markdown file per note.
      It follows the pattern of the import: a pure function, then the writing at the edge.

      VERDICT: OK
      """
    When I open the sitting of "c1"
    Then the sitting shows "How it works"
    And the sitting shows "The export reads each note once and writes one Markdown file per note."
    And the sitting does not show "## How it works"

  Scenario: The G4 page says when the Reviewer did not explain the change
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    When I open the sitting of "c1"
    Then the sitting shows "How it works: the Reviewer did not explain the change"

  Scenario: The Decision Inbox says how the change works
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    And the file ".gdd/changes/c1/review.md" says:
      """
      No blocking findings.

      ## How it works
      The export reads each note once and writes one Markdown file per note.
      It follows the pattern of the import: a pure function, then the writing at the edge.

      VERDICT: OK
      """
    When I run "gdd inbox c1"
    Then it succeeds
    And the output contains "## How it works"
    And the output contains "It follows the pattern of the import"

  Scenario: The Reviewer is told to explain the change it approves
    When I run "gdd agents install"
    Then it succeeds
    And the file ".claude/skills/gdd-review/SKILL.md" contains "## How it works"
    And the file ".claude/skills/gdd-review/SKILL.md" contains "for the human who approves without reading every line"
