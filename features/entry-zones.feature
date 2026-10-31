Feature: Before the first change, gdd points to the Setup
  Before the Setup, gdd status says that the Setup asks the person to approve the red zones, and
  a change refused for red zones not approved yet says the same. The change skill commits the
  scenarios on the change's branch before it shows them, so the sitting shows them before the
  person's OK; G1 then seals what the branch has.

  Background:
    Given an empty folder
    And I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"

  Scenario: Before the Setup, status says the Setup asks for the red zones
    When I run "gdd status"
    Then it succeeds
    And the output contains "Red zones: pending · Red zones not approved yet: the Setup asks you to approve them (the gdd-setup command)."
    And the output does not contain "end of Discover"

  Scenario: A change refused for its red zones points to the Setup
    Given the Setup is done
    And I run "gdd new-change c1 --title App --paths src/app.rs"
    And I am on the branch "change/c1"
    When I run "gdd prepare c1"
    Then it fails with a message that contains "Red zones not approved yet: the Setup asks you to approve them"

  Scenario: The change skill commits the scenarios before the sitting
    Then the file ".claude/skills/gdd-change/SKILL.md" contains "Commit them on the branch before you show them"
