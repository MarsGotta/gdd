Feature: One Builder session, by choice
  By default test mode and build mode run in separate sessions: build mode starts fresh and reads
  what test mode wrote. A project can choose one Builder session instead, with `[session] builder =
  "shared"` in gdd/models.toml: build mode continues the conversation of test mode, and each later
  round continues it too, so nothing is lost at the handoff. Path Guard still checks each mode, and
  mutation testing and the Reviewer stay as they are. It needs the same agent for both modes and an
  agent that can resume a session (Claude Code); otherwise the sessions stay separate and
  `gdd status` says why. In an experiment on this repository (40 runs) the shared session used 21%
  fewer new tokens with quality within the noise.

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

  Scenario: By default build mode starts a session of its own
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd launch c1"
    Then it succeeds
    And agent call 2 starts with "-p /gdd-build"
    And agent call 2 does not include "--resume"

  Scenario: A shared Builder session continues the conversation of test mode
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Builder session is "shared"
    When I run "gdd launch c1"
    Then it succeeds
    And agent call 2 starts with "-p /gdd-build"
    And agent call 2 includes "--resume fake-session"

  Scenario: Every later round of build mode continues it too
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Builder session is "shared"
    And the Builder in build mode writes "src/app.txt" from call 3
    When I run "gdd launch c1"
    Then it succeeds
    And agent call 3 includes "--resume fake-session"
    And agent call 4 includes "--resume fake-session"

  Scenario: The Reviewer never joins the Builder's session
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Builder session is "shared"
    When I run "gdd launch c1"
    Then it succeeds
    And agent call 3 starts with "exec"
    And agent call 3 does not include "fake-session"

  Scenario: With another agent in build mode the sessions stay separate, and the status says why
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Builder session is "shared"
    And the yellow Builder is "opencode" with the model "minimax/MiniMax-M3"
    When I run "gdd status"
    Then it succeeds
    And the output contains "Builder session: shared is set, but test mode (claude) and build mode (opencode) use different agents: the sessions stay separate"

  Scenario: A Builder session that is neither separate nor shared is refused
    Given the Builder session is "joint"
    When I run "gdd new-change c1 --title App --paths src/app.txt"
    Then it fails with a message that contains "session.builder must be separate or shared"

  Scenario: The classification says which Builder session the change gets
    Given I run "gdd new-change c1 --title App --paths src/app.txt"
    And the Builder session is "shared"
    When I run "gdd classify c1"
    Then it succeeds
    And the JSON field "builder_session" is "shared"
