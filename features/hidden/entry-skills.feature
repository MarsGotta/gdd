@hidden
Feature: The setup, status and learn skills start from what the project has (hidden examples)

  Background:
    Given an empty folder
    And I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"

  Scenario: Codex gets the same setup skill
    Then the file ".agents/skills/gdd-setup/SKILL.md" contains "run `gdd adopt` before `gdd selftest`"
    And the file ".agents/skills/gdd-status/SKILL.md" does not contain "GDD.md"
    And the file ".agents/skills/gdd-learn/SKILL.md" contains "Run `gdd learn`"
