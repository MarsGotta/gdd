Feature: A Reviewer finding written as a link names its file
  Reviewers often write the file of a finding as a Markdown link, `[path:line](/full/path)`. gdd
  reads the link's text as the file, as the merge commit already does, so a finding on a file only
  test mode may write still gets the test-mode fix round first.

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

  Scenario: A finding whose file is a link gets a test-mode fix first
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Reviewer finds "- **F1 · weak step · [tests/steps/app.txt:3](/elsewhere/c1/tests/steps/app.txt:3)** · one step checks three sentences", says KO, and then OK
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

  Scenario: A finding whose link names the code keeps the one build-mode fix
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Reviewer finds "F1 · off by one · [src/app.txt:1](/elsewhere/c1/src/app.txt:1) · the total drops the last line", says KO, and then OK
    When I run "gdd launch c1"
    Then it succeeds
    And the agents were called in this order:
      | role          | model  | arguments                       |
      | builder-test  | sonnet | change=c1 mode=test             |
      | builder-build | sonnet | change=c1 mode=build            |
      | reviewer      | gpt-6  | change=c1 round=1               |
      | builder-build | sonnet | change=c1 mode=build fix=review |
      | reviewer      | gpt-6  | change=c1 round=2               |
