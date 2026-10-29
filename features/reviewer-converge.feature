Feature: A Reviewer that converges
  Each review is kept in the change's record: its verdict, the file it wrote and, from the second
  review on, the files that changed since the review before it (relaunches included). The first
  review lists every finding in one pass, each with its class of bug, so the Builder fixes every
  place of that class at once. A later review checks the earlier findings and looks for new ones
  only where the code changed since, so reviews converge instead of opening one new topic each
  round (Anthropic's Claude Code review). `gdd report --dora` says how many reviews a change needs.

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

  Scenario: Every review is kept in the record
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Reviewer answers "VERDICT: KO" and then "VERDICT: OK"
    When I run "gdd launch c1"
    Then it succeeds
    And the record of "c1" lists the reviews "KO, OK"

  Scenario: A later review knows what changed since the one before
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Reviewer answers "VERDICT: KO" and then "VERDICT: OK"
    When I run "gdd launch c1"
    Then it succeeds
    And review 2 of "c1" lists the changed file "src/app.txt"
    And review 1 of "c1" lists no changed files

  Scenario: The reviews of an earlier launch stay in the record
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Reviewer answers "VERDICT: KO" every time
    And I run "gdd launch c1"
    When I run "gdd launch c1"
    Then the record of "c1" lists the reviews "KO, KO, KO, KO"

  Scenario: How many reviews a change needs
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Reviewer answers "VERDICT: KO" and then "VERDICT: OK"
    And I run "gdd launch c1"
    When I run "gdd report --dora"
    Then it succeeds
    And the output contains "Reviews to pass: median 2 per change · first review OK in 0 of 1 changes"

  Scenario: The Reviewer is told to give every finding at once, by class
    When I run "gdd agents install"
    Then it succeeds
    And the file ".claude/skills/gdd-review/SKILL.md" contains "every finding in one pass"
    And the file ".claude/skills/gdd-review/SKILL.md" contains "class of bug"

  Scenario: A later review is told to look only where the code changed
    When I run "gdd agents install"
    Then it succeeds
    And the file ".claude/skills/gdd-review/SKILL.md" contains "only in the files that changed since the last review"
    And the file ".claude/skills/gdd-review/SKILL.md" contains "never repeat a finding"

  Scenario: The Builder is told to fix every place of a class
    When I run "gdd agents install"
    Then it succeeds
    And the file ".claude/skills/gdd-build/SKILL.md" contains "every place of that class"
