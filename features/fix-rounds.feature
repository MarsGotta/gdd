Feature: A first change reaches G4
  A Reviewer finding on a file that test mode writes (a scenario step, a test written first) cannot
  be fixed in build mode, since Path Guard keeps those files from it. So when the Reviewer says KO
  and one of its findings (`F<n> · <class> · <file>:<line> · ...`) names a file protected for
  build mode, the fix round starts with the Builder in test mode (`mode=test fix=review`), Path
  Guard checks it as after any test mode, and then build mode fixes the rest as before. And the red
  zones are approved on main for the whole project: gdd judges a change against the zone map of
  main, so zones approved after the change was created count for it.

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
  Scenario: A finding on a step file gets a test-mode fix first
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Reviewer finds "F1 · weak step · tests/steps/app.txt:3 · one step checks three sentences", says KO, and then OK
    When I run "gdd launch c1"
    Then it succeeds
    And the agents were called in this order:
      | role          | model  | arguments                       |
      | builder-test  | sonnet | change=c1 mode=test             |
      | builder-build | sonnet | change=c1 mode=build            |
      | reviewer      | gpt-6  | change=c1 round=1               |
      | builder-test  | sonnet | change=c1 mode=test fix=review  |
      | builder-build | sonnet | change=c1 mode=build fix=review |
      | reviewer      | gpt-6  | change=c1 round=2               |
    And the record of "c1" is in state "closed"

  Scenario: A finding only on the code keeps the one build-mode fix
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Reviewer finds "F1 · off by one · src/app.txt:1 · the total drops the last line", says KO, and then OK
    When I run "gdd launch c1"
    Then it succeeds
    And the agents were called in this order:
      | role          | model  | arguments                       |
      | builder-test  | sonnet | change=c1 mode=test             |
      | builder-build | sonnet | change=c1 mode=build            |
      | reviewer      | gpt-6  | change=c1 round=1               |
      | builder-build | sonnet | change=c1 mode=build fix=review |
      | reviewer      | gpt-6  | change=c1 round=2               |

  Scenario: Red zones approved on main after the change was created count for it
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And main has the file "gdd/zones.toml" with:
      """
      default = "yellow"

      [[zone]]
      color = "red"
      paths = ["src/storage/**", "src/sync/**"]
      reason = "Losing the user's drafts"
      """
    And I run "gdd gate zones ok --by human"
    When I run "gdd launch c1"
    Then it succeeds
    And the output does not contain "Red zones changed after their approval"
    And the record of "c1" is in state "closed"

  Scenario: Red zones changed on main and not approved again still stop the change
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And main has the file "gdd/zones.toml" with:
      """
      default = "yellow"

      [[zone]]
      color = "red"
      paths = ["src/storage/**", "src/sync/**"]
      reason = "Losing the user's drafts"
      """
    When I run "gdd launch c1"
    Then it fails with a message that contains "Red zones changed after their approval"
