Feature: Secrets reach no agent under their other common names, nor inside a URL
  gdd gives each agent the environment it runs in, without the variables that hold a secret.
  A name says so when one of its words, split at "_", "-" or ".", in any case, is KEY, KEYS,
  TOKEN, TOKENS, SECRET, SECRETS, PASSWORD, PASSWORDS, PASSWD, PASS, CREDENTIAL, CREDENTIALS,
  APIKEY or DSN: AWS_SECRET_ACCESS_KEY, DB_PASS and SENTRY_DSN are secrets, PWD and KEYBOARD_LAYOUT
  are not. A value says so when it is a URL with a password in it (scheme://user:password@host,
  or scheme://:password@host): DATABASE_URL usually is. A project still passes a variable to its
  agents on purpose, by its exact name, in [env] pass of gdd/models.toml.

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
    Given the environment has "AWS_SECRET_ACCESS_KEY" set to "aws-test"
    And the environment has "DB_PASS" set to "pass-test"
    And the environment has "GOOGLE_APPLICATION_CREDENTIALS" set to "/home/me/key.json"
    And the environment has "EDITOR" set to "vim"
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd launch c1"
    Then it succeeds
    And agent call 1 did not get "AWS_SECRET_ACCESS_KEY"
    And agent call 1 did not get "DB_PASS"
    And agent call 1 did not get "GOOGLE_APPLICATION_CREDENTIALS"
    And agent call 1 got "EDITOR"

  Scenario: A URL with a password in it never reaches the agents
    Given the environment has "DATABASE_URL" set to "postgres://app:s3cret@db.example/app"
    And the environment has "DOCS_URL" set to "https://example.com/docs"
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd launch c1"
    Then it succeeds
    And agent call 1 did not get "DATABASE_URL"
    And agent call 1 got "DOCS_URL"

  Scenario: A name that only looks like a secret reaches the agents
    Given the environment has "KEYBOARD_LAYOUT" set to "us"
    And the environment has "PASSAGE_DIR" set to "/home/me/passage"
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd launch c1"
    Then it succeeds
    And agent call 1 got "KEYBOARD_LAYOUT"
    And agent call 1 got "PASSAGE_DIR"

  Scenario: A project can still pass a URL with a password on purpose
    Given the models pass "DATABASE_URL" to the agents
    And the environment has "DATABASE_URL" set to "postgres://app:s3cret@db.example/app"
    And I run "gdd new-change c1 --title App --paths src/app.txt"
    When I run "gdd launch c1"
    Then it succeeds
    And agent call 1 got "DATABASE_URL"
