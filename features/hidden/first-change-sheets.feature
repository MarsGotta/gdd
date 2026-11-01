@hidden
Feature: The sheets of a first change (hidden examples)

  Background:
    Given an empty folder
    And I run "tminus init --name Notes --platform cli --stack rust --agents claude codex"

  Scenario: Codex gets the same setup and test-mode sheets
    Then the file ".agents/skills/tminus-setup/SKILL.md" contains "ask the person to approve the red zones"
    And the file ".agents/skills/tminus-setup/SKILL.md" contains "Next step: the first change, with `$tminus-change`"
    And the file ".agents/skills/tminus-build-test/SKILL.md" contains "With `fix=review`"
