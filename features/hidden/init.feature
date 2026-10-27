@hidden
Feature: Hidden examples for gdd init

  Background:
    Given an empty folder

  Scenario: A Python project gets its own commands and supply chain
    When I run "gdd init --name Garden --platform web --stack python --agents claude codex"
    Then the TOML file "gdd/commands.toml" has "g2.unit" = "uv run pytest -q tests/unit"
    And the file "gdd/zones.toml" contains "uv.lock"

  Scenario: A CI workflow that is already there stops the setup
    Given the folder is a git repository with the file ".github/workflows/gdd.yml"
    When I run "gdd init --name Garden --platform web --stack python --agents claude codex"
    Then it fails with a message that contains ".github/workflows/gdd.yml already exists"
    And the file "gdd/project.toml" does not exist

  Scenario: OpenCode can review what Claude Code builds
    When I run "gdd init --name Garden --platform web --stack python --agents claude opencode"
    Then the TOML file "gdd/models.toml" has "reviewer.integration" = "opencode"
    And the TOML file "gdd/models.toml" has "builder.red.model" = "opus"
