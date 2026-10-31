Feature: The sheets of a first change
  The Setup asks the person to approve the red zones before it ends: the harness zone ships red,
  and a change cannot be launched before a person approved the zone map. It ends by sending the
  person to describe a first change, with Discover first only when the product needs an
  architecture. The test-mode sheet says what to do with a Reviewer finding (`fix=review`).

  Background:
    Given an empty folder
    And I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"

  Scenario: The setup skill asks for the red zones before it ends
    Then the file ".claude/skills/gdd-setup/SKILL.md" contains "ask the person to approve the red zones"
    And the file ".claude/skills/gdd-setup/SKILL.md" contains "`gdd gate zones ok --by human`"

  Scenario: The setup skill ends with the first change
    Then the file ".claude/skills/gdd-setup/SKILL.md" contains "Next step: the first change, with `/gdd-change`"

  Scenario: The test-mode sheet fixes the Reviewer findings on its files
    Then the file ".claude/skills/gdd-build-test/SKILL.md" contains "With `fix=review`"

  Scenario: New projects do not protect a bin folder they no longer have
    Then the file "gdd/rules.toml" does not contain '"bin/**"'
    And the file "gdd/zones.toml" does not contain '"bin/**"'
    And the file "gdd/rules.toml" contains '".gdd/bin/**"'
