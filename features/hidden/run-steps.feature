@hidden
Feature: Hidden examples for the single steps of the recipe

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
      "haiku" = "anthropic"
      "gpt" = "openai"
      """
    And the gate commands:
      """
      [g2]
      lint = "true"
      unit = "true"

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
    And everything is committed on main

  Scenario: A red change with its plan approved gets the strong Builder
    Given the Setup is done
    And the red zones are approved
    And I run "gdd new-change c2 --title Store --paths src/storage/files.rs"
    And I run "gdd gate plan ok c2 --by human"
    And I am on the branch "change/c2"
    When I run "gdd prepare c2"
    Then the JSON field "builder" is {"integration": "claude", "model": "opus"}
    And the record of "c2" is in state "building"

  Scenario: A red change is promoted after the human G4
    Given I run "gdd new-change c2 --title Store --paths src/storage/files.rs"
    And I run "gdd gate G4 ok c2 --by human"
    When I run "gdd release promote c2"
    Then it succeeds
    And the record of "c2" is in state "in-production"

  Scenario: Merge refuses a G3 recorded as ko
    Given I run "gdd new-change c2 --title Store --paths src/app.rs"
    And I am on the branch "change/c2"
    And I run "gdd gate G3 ko c2 --by machines"
    When I run "gdd merge c2"
    Then it fails with a message that contains "G3 is not open"
