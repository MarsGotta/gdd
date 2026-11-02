@hidden
Feature: The setup, status and learn skills start from what the project has (hidden examples)

  Background:
    Given an empty folder
    And I run "tminus init --name Notes --platform cli --stack rust --agents claude codex"

  Scenario: Codex gets the same setup skill
    Then the file ".agents/skills/tminus-setup/SKILL.md" contains "run `tminus adopt` before `tminus selftest`"
    And the file ".agents/skills/tminus-status/SKILL.md" does not contain "GDD.md"
    And the file ".agents/skills/tminus-learn/SKILL.md" contains "Run `tminus learn`"
