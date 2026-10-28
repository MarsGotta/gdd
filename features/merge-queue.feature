Feature: Several changes at once, merged one at a time
  Each change runs in its own worktree, so several can run at once. A change that plans a path
  overlapping one of a change in flight (building, parked or waiting at G4) still starts, with a
  warning that names the paths: a folder pattern overlaps every path inside it, and git settles
  the overlap when the change merges, as a merge queue does. No more changes build at once than
  [parallel] max in gdd/rules.toml, 2 by default, because the agents' subscriptions are shared.
  Merges go one at a time. When main has
  moved since a change passed G3, the change takes the latest main and passes G3 again on top of
  it before merging; red or a conflict parks it, and the message says how to settle the conflict.

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

  Scenario: A change that overlaps a parked change starts with a warning
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Builder in build mode writes nothing
    And I run "gdd launch c1"
    And I run "gdd new-change c2 --title Other --paths src/app.txt"
    When I run "gdd launch c2"
    Then it succeeds
    And the output contains "overlaps c1 (parked)"
    And the output contains "src/app.txt"
    And the output contains "git settles it when c2 merges"
    And the record of "c2" is in state "closed"

  Scenario: A change that overlaps a change waiting at G4 starts with a warning
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/c1.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    And I run "gdd new-change c2 --title Other --paths src/app.txt"
    When I run "gdd launch c2"
    Then it succeeds
    And the output contains "overlaps c1 (in-pre)"

  Scenario: A folder pattern overlaps the files inside it
    Given I run "gdd new-change c1 --title App --paths src/lib/**"
    And the change "c1" has the scenario "features/c1.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    And I run "gdd new-change c2 --title Other --paths src/lib/parse.txt"
    When I run "gdd launch c2"
    Then it succeeds
    And the output contains "overlaps c1"
    And the output contains "src/lib/parse.txt"

  Scenario: Changes that do not overlap run while another waits at G4
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/c1.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    And I run "gdd new-change c2 --title Other --paths src/other.txt"
    When I run "gdd launch c2"
    Then it succeeds
    And the output does not contain "overlaps"
    And the record of "c2" is in state "closed"

  Scenario: A closed change does not count
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And I run "gdd launch c1"
    And I run "gdd new-change c2 --title Other --paths src/app.txt"
    When I run "gdd launch c2"
    Then it succeeds
    And the output does not contain "overlaps"

  Scenario: No more changes build at once than the cap
    Given the cap of changes building at once is 1
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" is building
    And I run "gdd new-change c2 --title Other --paths src/other.txt"
    When I run "gdd launch c2"
    Then it fails with a message that contains "the cap is 1"
    And the output contains "c1"

  Scenario: The cap is 2 by default
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" is building
    And I run "gdd new-change c2 --title Other --paths src/other.txt"
    And the change "c2" is building
    And I run "gdd new-change c3 --title Third --paths src/third.txt"
    When I run "gdd launch c3"
    Then it fails with a message that contains "the cap is 2"

  Scenario: Changes waiting at G4 do not count toward the cap
    Given the cap of changes building at once is 1
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/c1.feature" on its branch
    And I am on the branch "change/c1"
    And I run "gdd gate G1 ok c1 --by human"
    And I run "gdd launch c1"
    And I run "gdd new-change c2 --title Other --paths src/other.txt"
    When I run "gdd launch c2"
    Then it succeeds

  Scenario: When main moved, the change passes G3 again on top of it and merges
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the main branch gets the file "docs/news.md" during the Reviewer call
    When I run "gdd launch c1"
    Then it succeeds
    And the output contains "main moved under c1"
    And the branch "main" holds "docs/news.md"
    And the branch "main" holds "src/app.txt"
    And the last commit on main says "gdd(c1): G3 Merge Gate · App"

  Scenario: G3 red on top of the latest main parks the change
    Given the gate command "g3" "acceptance_full" is "test ! -f docs/breaking.md"
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    And the main branch gets the file "docs/breaking.md" during the Reviewer call
    When I run "gdd launch c1"
    Then it fails with a message that contains "G3 red on top of the latest main"
    And the record of "c1" is in state "parked"

  Scenario: A conflict with the latest main parks the change
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the main branch gets the file "src/app.txt" during the Reviewer call
    When I run "gdd launch c1"
    Then it fails with a message that contains "conflicts in src/app.txt"
    And it fails with a message that contains "merge main into change/c1 in its worktree"
    And the record of "c1" is in state "parked"

  Scenario: Merges go one at a time
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And another gdd holds the merge lock for 30 seconds
    When I run "gdd launch c1"
    Then it succeeds
    And the output contains "waiting for the merge of another change"
