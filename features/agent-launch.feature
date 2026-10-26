Feature: gdd starts the agents itself
  gdd starts each agent's own program (found on the PATH of the environment gdd runs in), with the
  arguments its sheet gives each role, an environment without API keys, and reads the agent's own
  JSON output for the tokens, the final text and the log line. Its own checks hold for every agent:
  no agent changes GDD state, and the Reviewer only reads. The Python wrapper is no longer needed.

  The fakes are programs named claude, codex and opencode on that PATH. Each records its full
  arguments and the names of its environment variables, and prints its agent's JSON output with 100
  input, 50 cached and 20 output tokens (Codex and OpenCode also report 5 reasoning tokens) and its
  final text. The Reviewer's final text is its answer ("VERDICT: OK" by default).

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


  Scenario: A Claude Code Builder may edit but never approve, merge or write GDD state
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd launch c1"
    Then it succeeds
    And agent call 2 includes "--permission-mode acceptEdits"
    And agent call 2 includes "Bash(bin/gdd gate:*)"
    And agent call 2 includes "Write(.gdd/**)"

  Scenario: Codex gets the reasoning effort of its role
    Given the reviewer is "codex" "gpt-6" with effort "high"
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd launch c1"
    Then it succeeds
    And agent call 3 was "exec --sandbox read-only -c mcp_servers={} -c model_reasoning_effort=high $gdd-review change=c1 round=1 --model gpt-6 --json"

  Scenario: API keys never reach the agents
    Given the environment has "OPENAI_API_KEY" set to "sk-test"
    And the environment has "GITHUB_TOKEN" set to "ghp-test"
    And the environment has "EDITOR" set to "vim"
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd launch c1"
    Then it succeeds
    And agent call 1 did not get "OPENAI_API_KEY"
    And agent call 1 did not get "GITHUB_TOKEN"
    And agent call 1 got "EDITOR"

  Scenario: A project can pass one key to its agents on purpose
    Given the models pass "MINIMAX_API_KEY" to the agents
    And the environment has "MINIMAX_API_KEY" set to "mm-test"
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd launch c1"
    Then it succeeds
    And agent call 1 got "MINIMAX_API_KEY"

  Scenario: The tokens of Claude Code go to the log
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd launch c1"
    Then the log line of agent call 2 has "tokens_out" 20
    And the log line of agent call 2 has "tokens_cache" 50

  Scenario: The tokens of Codex count its reasoning and leave out the cached input
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd launch c1"
    Then the log line of agent call 3 has "tokens_out" 25
    And the log line of agent call 3 has "tokens_in" 50

  Scenario: The tokens of OpenCode count its reasoning
    Given the reviewer is "opencode" "kimi-k3"
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd launch c1"
    Then the log line of agent call 3 has "tokens_out" 25

  Scenario: The Reviewer's answer is kept with the change
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd launch c1"
    Then the file ".gdd/changes/c1/outputs/reviewer-1.md" contains "VERDICT: OK"
    And the file ".gdd/changes/c1/review.md" contains "VERDICT: OK"

  Scenario: An agent that changes GDD state parks the change and the state is put back
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Builder in build mode writes ".gdd/changes/c1/change.json" from call 1
    When I run "gdd launch c1"
    Then it fails with a message that contains "changed GDD state"
    And the record of "c1" is in state "parked"

  Scenario: A Reviewer that changes files parks the change
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Reviewer writes "src/app.txt"
    When I run "gdd launch c1"
    Then it fails with a message that contains "Reviewers only read"
    And the record of "c1" is in state "parked"

  Scenario: Messages name the GDD commands, not Spec Kit
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the change "c1" has the scenario "features/app.feature" on its branch
    When I run "gdd launch c1"
    Then it fails with a message that contains "gdd-change"
