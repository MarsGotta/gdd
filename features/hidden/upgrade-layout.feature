@hidden
Feature: tminus upgrade moves a project set up before the rename to the tminus names (hidden examples)

  Background:
    Given an empty folder
    And I run "tminus init --name Notes --platform cli --stack rust --agents claude codex"
    And the project keeps the names gdd gave it before the rename

  Scenario: A change waiting at G4 stops the move
    Given I run "tminus new-change c2 --title Menu --paths src/menu.txt"
    And the change "c2" is in state "in-pre"
    When I run "tminus upgrade"
    Then it fails with a message that contains "c2 (in-pre)"
    And the branch "tminus-upgrade" does not exist

  Scenario: A line the project changed in the zone map keeps its words, and the harness zone moves
    And in the file "gdd/zones.toml", "Documentation" is replaced by "Docs and notes"
    And everything is committed on main
    When I run "tminus upgrade"
    Then it succeeds
    And the output contains "tminus/zones.toml: merged with your changes"
    And the file "tminus/zones.toml" on the branch "tminus-upgrade" contains "Docs and notes"
    And the file "tminus/zones.toml" on the branch "tminus-upgrade" contains ".tminus/bin/**"
