@hidden
Feature: Hidden examples for tminus init

  Background:
    Given an empty folder

  Scenario: A Python project gets its own commands and supply chain
    When I run "tminus init --name Garden --platform web --stack python --agents claude codex"
    Then the TOML file "tminus/commands.toml" has "g2.unit" = "uv run pytest -q tests/unit"
    And the file "tminus/zones.toml" contains "uv.lock"

  Scenario: A CI workflow that is already there stops the setup
    Given the folder is a git repository with the file ".github/workflows/tminus.yml"
    When I run "tminus init --name Garden --platform web --stack python --agents claude codex"
    Then it fails with a message that contains ".github/workflows/tminus.yml already exists"
    And the file "tminus/project.toml" does not exist

  Scenario: OpenCode can review what Claude Code builds
    When I run "tminus init --name Garden --platform web --stack python --agents claude opencode"
    Then the TOML file "tminus/models.toml" has "reviewer.integration" = "opencode"
    And the TOML file "tminus/models.toml" has "builder.red.model" = "opus"
