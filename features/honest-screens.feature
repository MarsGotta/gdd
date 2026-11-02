Feature: The screens say what is true
  What the person reads to decide must match what tminus did and will do. The sitting at G4 shows
  the changelog line the merge commit carries, also when it comes from the title of the change,
  and a Reviewer's finding linked to a file of the change shows the file's path in the
  repository, never a folder of this machine. The Decision Inbox lists the G1 a change waits
  for before its launch, and a change that ended asks nothing more. Builders and Reviewers hear
  the same refusal as every agent tminus started.

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
      color = "green"
      paths = ["docs/**"]

      [[zone]]
      color = "red"
      paths = ["src/storage/**"]
      reason = "Losing the user's drafts"
      """
    And everything is committed on main

  Scenario: The sitting at G4 shows the changelog line the merge carries
    Given I run "tminus new-change c1 --title App --paths src/app.rs"
    And the change "c1" is in state "in-pre"
    When I open the sitting of "c1"
    Then the sitting shows "Changelog: Added · App"
    And the sitting does not show "none set"

  Scenario: A finding linked to a file of the worktree shows its path in the repository
    Given I run "tminus new-change c1 --title App --paths src/app.rs"
    And the change "c1" is in state "in-pre"
    And the review of "c1" says, with its folders filled in:
      """
      - **F1 · Fixed** · [app.rs:3](<worktree>/src/app.rs:3): the name is checked now.

      VERDICT: OK
      """
    When I open the sitting of "c1"
    Then the sitting shows "· src/app.rs:3: the name is checked now."
    And the sitting does not show "]("

  Scenario: The Decision Inbox lists the G1 a change waits for
    Given I run "tminus new-change c1 --title App --paths src/app.rs"
    And the change "c1" has the scenario "features/app.feature" on its branch
    When I run "tminus inbox c1"
    Then it succeeds
    And the output contains "- G1 · Scenario Gate: read the scenarios of change/c1, then answer your agent"

  Scenario: Scenarios changed after G1 wait for G1 again
    Given I run "tminus new-change c1 --title App --paths src/app.rs"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "tminus gate G1 ok c1 --by human"
    And the change "c1" has the hidden example "features/hidden/app.feature" with the step "Given a draft"
    When I run "tminus inbox c1"
    Then it succeeds
    And the output contains "- G1 · Scenario Gate: read the scenarios of change/c1"

  Scenario: An approved G1 leaves the inbox
    Given I run "tminus new-change c1 --title App --paths src/app.rs"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I am on the branch "change/c1"
    And I run "tminus gate G1 ok c1 --by human"
    When I run "tminus inbox c1"
    Then it succeeds
    And the output contains "## Needs your decision"
    And the output does not contain "G1 · Scenario Gate"

  Scenario: A change that ended asks nothing more
    Given I run "tminus new-change c1 --title App --paths src/app.rs"
    And I run "tminus note c1 needs Pick a name for the export"
    And the change "c1" is in state "closed"
    When I run "tminus inbox c1"
    Then it succeeds
    And the output does not contain "- Pick a name for the export"
    And the output contains "1 request(s) were still open when the change ended (kept in change.json)"

  Scenario Outline: A Builder or a Reviewer hears the refusal every agent hears
    When the role "<role>" runs "<line>"
    Then it fails with a message that contains "Agents that tminus started cannot run tminus <command>: they may only run tminus g2"

    Examples:
      | role          | command        | line                                  |
      | builder-build | gate           | tminus gate G1 ok c1 --by human          |
      | reviewer      | changelog-line | tminus changelog-line c1 Added Something |
      | builder-test  | learn          | tminus learn                             |
      | reviewer      | resume         | tminus resume c1 --g4 ok                 |
      | builder-build | release        | tminus release --yes                     |
      | builder-build | upgrade        | tminus upgrade                           |
