@hidden
Feature: Agents can tell when to use each tminus skill and how to check the project (hidden examples)

  Background:
    Given an empty folder

  Scenario: The AGENTS.md of a TypeScript project names its install and check commands
    When I run "tminus init --name Notes --platform cli --stack typescript --agents claude codex"
    Then it succeeds
    And the file "AGENTS.md" contains "- Install: `npm ci`"
    And the file "AGENTS.md" contains "- `types`: `npx tsc --noEmit`"
    And the file "AGENTS.md" contains "- `architecture`: `npx depcruise src --config .dependency-cruiser.cjs`"
    And the file "AGENTS.md" does not contain "cargo test"

  Scenario: The Codex Reviewer skill and the TypeScript instructions say when they apply
    When I run "tminus init --name Notes --platform cli --stack python typescript --agents claude codex opencode"
    Then it succeeds
    And the file ".agents/skills/tminus-review/SKILL.md" contains "description: 'Started by tminus; do not run it yourself."
    And the file ".opencode/commands/tminus-stack-typescript.md" contains "description: 'Use when writing or reviewing TypeScript code"
    And the file "AGENTS.md" contains "- `lint-typescript`: `npx eslint . --max-warnings 0`"
