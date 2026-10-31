@hidden
Feature: The sheets of a first change (hidden examples)

  Background:
    Given an empty folder
    And I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"

  Scenario: Codex gets the same setup and test-mode sheets
    Then the file ".agents/skills/gdd-setup/SKILL.md" contains "ask the person to approve the red zones"
    And the file ".agents/skills/gdd-setup/SKILL.md" contains "Next step: the first change, with `$gdd-change`"
    And the file ".agents/skills/gdd-build-test/SKILL.md" contains "With `fix=review`"
