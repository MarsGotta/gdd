Feature: The fix rounds after a red G3 or a Reviewer KO are the number the team set
  When G3 is red, or the Reviewer says KO, the Builder gets fix rounds before the change parks: as
  many as the rules set in [limits] reviewer_fixes. Each round is one more Builder call, G2 and G3
  and, after a KO, the next review. How many is the team's choice, and 0 parks at the first no. A
  project whose rules do not say keeps one round, as before, so no project gets more rounds than
  it asked for. tminus init writes 2 for a new project. More rounds loosen the gate, so tminus
  upgrade never raises a limit the project has: it keeps the project's line and says so.

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

  Scenario: The Builder gets the fix rounds the team set after Reviewer KOs
    Given the rules have "limits.reviewer_fixes" = "2"
    And I run "tminus new-change c1 --title App --paths src/app.txt"
    And the Reviewer says KO 2 times and then OK
    When I run "tminus launch c1"
    Then it succeeds
    And the agents were called in this order:
      | role          | model  | arguments                       |
      | builder-test  | sonnet | change=c1 mode=test             |
      | builder-build | sonnet | change=c1 mode=build            |
      | reviewer      | gpt-6  | change=c1 round=1               |
      | builder-build | sonnet | change=c1 mode=build fix=review |
      | reviewer      | gpt-6  | change=c1 round=2               |
      | builder-build | sonnet | change=c1 mode=build fix=review |
      | reviewer      | gpt-6  | change=c1 round=3               |
    And the record of "c1" is in state "closed"

  Scenario: A change the Reviewer still finds KO after its last fix round parks, saying how many
    Given the rules have "limits.reviewer_fixes" = "2"
    And I run "tminus new-change c1 --title App --paths src/app.txt"
    And the Reviewer answers "VERDICT: KO" every time
    When I run "tminus launch c1"
    Then it fails with a message that contains "parked: Reviewer KO or G3 red after 2 fixes"
    And the record of "c1" is in state "parked"

  Scenario: A red G3 gets the fix rounds the team set, then parks
    Given the rules have "limits.reviewer_fixes" = "2"
    And the gate command "g3" "acceptance_full" is "test -f src/extra.txt"
    And I run "tminus new-change c1 --title App --paths src/app.txt"
    When I run "tminus launch c1"
    Then it fails with a message that contains "parked: G3 still red after 2 fixes"
    And the agents were called in this order:
      | role          | model  | arguments                   |
      | builder-test  | sonnet | change=c1 mode=test         |
      | builder-build | sonnet | change=c1 mode=build        |
      | builder-build | sonnet | change=c1 mode=build fix=g3 |
      | builder-build | sonnet | change=c1 mode=build fix=g3 |
    And the record of "c1" is in state "parked"

  Scenario: A project whose rules do not set the fix rounds keeps one
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the Reviewer says KO 2 times and then OK
    When I run "tminus launch c1"
    Then it fails with a message that contains "parked: Reviewer KO or G3 red after one fix"
    And the agents were called in this order:
      | role          | model  | arguments                       |
      | builder-test  | sonnet | change=c1 mode=test             |
      | builder-build | sonnet | change=c1 mode=build            |
      | reviewer      | gpt-6  | change=c1 round=1               |
      | builder-build | sonnet | change=c1 mode=build fix=review |
      | reviewer      | gpt-6  | change=c1 round=2               |

  Scenario: A new project gets two fix rounds, and an upgrade keeps the number a project has
    Given an empty folder
    And I run "tminus init --name Notes --platform cli --stack rust --agents claude codex"
    And an older tminus "0.0.0" wrote "tminus/rules.toml" with "reviewer_fixes = 1" where this one writes "reviewer_fixes = 2"
    When I run "tminus upgrade"
    Then it succeeds
    And the output contains "tminus/rules.toml: kept your limits.reviewer_fixes = 1"
    And the file "tminus/rules.toml" on the branch "tminus-upgrade" contains "reviewer_fixes = 1"
    And the file "tminus/rules.toml" on the branch "tminus-upgrade" does not contain "reviewer_fixes = 2"
