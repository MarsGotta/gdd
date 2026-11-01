@hidden
Feature: Before the first change, tminus points to the Setup (hidden examples)

  Background:
    Given an empty folder
    And I run "tminus init --name Notes --platform cli --stack rust --agents claude codex"

  Scenario: Another change is refused the same way
    Given the Setup is done
    And I run "tminus new-change c2 --title Export --paths src/export.rs"
    And I am on the branch "change/c2"
    When I run "tminus prepare c2"
    Then it fails with a message that contains "Red zones not approved yet: the Setup asks you to approve them"

  Scenario: Codex gets the same change skill
    Then the file ".agents/skills/tminus-change/SKILL.md" contains "Commit them on the branch before you show them"
