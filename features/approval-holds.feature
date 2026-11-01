Feature: An approval holds only for the scenarios the human saw
  Approving G1 seals the scenario files of a change, visible and hidden, with their SHA-256. The
  launch checks that seal before anything else: when the branch has a scenario file that G1 did
  not seal, one whose text is no longer the one G1 sealed, or has lost one G1 sealed, the change
  does not start and no agent is called. The refusal names each file and what happened to it
  (added, edited or removed) and asks a human to approve the scenarios again. Regression
  scenarios need no approval, so adding one asks for nothing. A new G1 seals the scenarios as
  they are, and the change starts as before.

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

  # Invariant: the human's approval is valid only for what was seen.
  Scenario: The human's approval holds only for what was seen: a scenario edited after G1 waits for a new one
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I run "tminus gate G1 ok c1 --by human"
    And I am on the branch "change/c1"
    When the change commits "features/app.feature"
    And I run "tminus launch c1"
    Then it fails with a message that contains "features/app.feature (edited)"
    And it fails with a message that contains "approve the scenarios again"
    And the agents were called in this order:
      | role | model | arguments |
    And the change "c1" has no worktree

  Scenario: A scenario added after G1 stops the launch before any agent call
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I run "tminus gate G1 ok c1 --by human"
    And the change "c1" has the scenario "features/extra.feature" on its branch
    When I run "tminus launch c1"
    Then it fails with a message that contains "features/extra.feature (added)"
    And it fails with a message that contains "approve the scenarios again"
    And the agents were called in this order:
      | role | model | arguments |
    And the change "c1" has no worktree

  Scenario: A hidden example edited after G1 stops the launch before any agent call
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And the change "c1" has the hidden example "features/hidden/app.feature" with the step "Given a draft"
    And I run "tminus gate G1 ok c1 --by human"
    And the change "c1" has the hidden example "features/hidden/app.feature" with the step "And a name"
    When I run "tminus launch c1"
    Then it fails with a message that contains "features/hidden/app.feature (edited)"
    And it fails with a message that contains "approve the scenarios again"
    And the agents were called in this order:
      | role | model | arguments |

  Scenario: A scenario removed after G1 stops the launch before any agent call
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And the change "c1" has the scenario "features/extra.feature" on its branch
    And I run "tminus gate G1 ok c1 --by human"
    And the change "c1" no longer has the scenario "features/extra.feature" on its branch
    When I run "tminus launch c1"
    Then it fails with a message that contains "features/extra.feature (removed)"
    And it fails with a message that contains "approve the scenarios again"
    And the agents were called in this order:
      | role | model | arguments |

  Scenario: Scenarios approved again after a change launch as before
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I run "tminus gate G1 ok c1 --by human"
    And the change "c1" has the scenario "features/extra.feature" on its branch
    And I run "tminus gate G1 ok c1 --by human"
    When I run "tminus launch c1"
    Then it succeeds
    And the record of "c1" is in state "in-pre"

  Scenario: A regression scenario added after G1 asks for no new approval
    Given I run "tminus new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    And I run "tminus gate G1 ok c1 --by human"
    And the change "c1" has the scenario "features/regression/app.feature" on its branch
    When I run "tminus launch c1"
    Then it succeeds
    And the record of "c1" is in state "in-pre"
