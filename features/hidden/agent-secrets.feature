Feature: Secrets reach no agent under their other common names, nor inside a URL (hidden examples)

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


  Scenario: Secrets under other common names never reach the agents
    Given the environment has "STRIPE_SECRET_KEY" set to "sk-live-test"
    And the environment has "SMTP_PASSWD" set to "smtp-test"
    And the environment has "SENTRY_DSN" set to "https://k@o1.example/1"
    And the environment has "EDITOR" set to "nano"
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd launch c1"
    Then it succeeds
    And agent call 1 did not get "STRIPE_SECRET_KEY"
    And agent call 1 did not get "SMTP_PASSWD"
    And agent call 1 did not get "SENTRY_DSN"
    And agent call 1 got "EDITOR"

  Scenario: A URL with a password in it never reaches the agents
    Given the environment has "REDIS_URL" set to "redis://:hunter2@cache.example:6379"
    And the environment has "API_BASE_URL" set to "https://api.example.com"
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd launch c1"
    Then it succeeds
    And agent call 1 did not get "REDIS_URL"
    And agent call 1 got "API_BASE_URL"
