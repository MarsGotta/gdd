Feature: tminus upgrade moves a project set up before the rename to the tminus names
  A project set up before the tool was called tminus keeps its files in gdd/, its state in .gdd/
  and its agents' commands as gdd-<command>. tminus upgrade proposes the move on the branch
  tminus-upgrade, as it proposes any upgrade: first what this tminus writes, pure, then the merge
  with what the project made of it, now under tminus/, .github/workflows/tminus.yml and
  tminus-<command>. The folders gdd/ and .gdd/ take their new names in the files tminus init wrote,
  the project's own lines included; every other word of the project stays as it was, and so do
  the names of the variables its own code reads. The human reviews the branch and merges it. The
  state cannot ride a branch: once the branch is merged, every command names tminus upgrade, which
  then moves .gdd/ to .tminus/ as it is, approvals, records and log included. Nothing moves while a
  change is in flight.

  Background:
    Given an empty folder
    And I run "tminus init --name Notes --platform cli --stack rust --agents claude codex"
    And the project keeps the names gdd gave it before the rename

  Scenario: The upgrade proposes the project in the tminus names, on a branch
    When I run "tminus upgrade"
    Then it succeeds
    And the output contains "on the branch tminus-upgrade"
    And the branch "tminus-upgrade" holds "tminus/project.toml"
    And the branch "tminus-upgrade" does not hold "gdd/project.toml"
    And the file ".github/workflows/tminus.yml" on the branch "tminus-upgrade" contains "run: tminus ci"
    And the branch "tminus-upgrade" does not hold ".github/workflows/gdd.yml"
    And the file ".claude/skills/tminus-build/SKILL.md" on the branch "tminus-upgrade" contains "name: tminus-build"
    And the branch "tminus-upgrade" does not hold ".claude/skills/gdd-build/SKILL.md"
    And the file ".gitignore" on the branch "tminus-upgrade" contains ".tminus/"
    And the file "gdd/project.toml" exists
    And the current branch is "main"

  Scenario: The folders the harness protects take their new names
    When I run "tminus upgrade"
    Then it succeeds
    And the file "tminus/rules.toml" on the branch "tminus-upgrade" contains ".tminus/bin/**"
    And the file "tminus/rules.toml" on the branch "tminus-upgrade" does not contain "gdd/**"
    And the file "tminus/zones.toml" on the branch "tminus-upgrade" contains ".tminus/bin/**"
    And the file "tminus/zones.toml" on the branch "tminus-upgrade" does not contain "gdd/**"

  Scenario: A line the project changed keeps its words and takes the new folder names
    And in the file "gdd/commands.toml", "to hold its known debt" is replaced by "to hold our known debt"
    And everything is committed on main
    When I run "tminus upgrade"
    Then it succeeds
    And the output contains "tminus/commands.toml: merged with your changes"
    And the file "tminus/commands.toml" on the branch "tminus-upgrade" contains "to hold our known debt (tminus/debt.toml)"

  Scenario: The variables the project's own code reads keep their names
    When I run "tminus upgrade"
    Then it succeeds
    And the file "tminus/commands.toml" on the branch "tminus-upgrade" contains "GDD_FEATURES= GDD_HIDDEN=1 cargo test"
    And the file "tests/acceptance.rs" on the branch "tminus-upgrade" contains "GDD_HIDDEN"

  Scenario: A file of the project that tminus does not write moves as it is
    And main has the file "gdd/notes.md" with:
      """
      Our notes about gdd/ and .gdd/
      """
    When I run "tminus upgrade"
    Then it succeeds
    And the output contains "tminus/notes.md: moved as it was"
    And the file "tminus/notes.md" on the branch "tminus-upgrade" contains "Our notes about gdd/ and .gdd/"
    And the branch "tminus-upgrade" does not hold "gdd/notes.md"

  Scenario: Once the branch is merged, every command names the one way out
    Given I run "tminus upgrade"
    And the branch "tminus-upgrade" is merged into main and deleted
    When I run "tminus status"
    Then it fails with a message that contains "run tminus upgrade to move it to .tminus/"

  Scenario: Run again, the upgrade moves the state with its approvals, records and log
    Given I run "tminus new-change c1 --title Export --paths src/app.txt"
    And the scenario "features/export.feature" was approved in G1
    And I run "tminus upgrade"
    And the branch "tminus-upgrade" is merged into main and deleted
    When I run "tminus upgrade"
    Then it succeeds
    And the output contains "Moved .gdd/ to .tminus/"
    And the output contains "tminus gate zones ok --by human"
    And the file ".gdd" does not exist
    And the file ".tminus/changes/c1/change.json" contains "eabe0a165567706a70cc68fe0199283d68677bd39463770666831bcab3e86d5d"
    And the file ".tminus/log.jsonl" contains "change-created"

  Scenario: Once moved, the red zones ask for a new approval and the change keeps its G1
    Given I run "tminus new-change c1 --title Export --paths src/app.txt"
    And the scenario "features/export.feature" was approved in G1
    And the red zones are approved
    And I run "tminus upgrade"
    And the branch "tminus-upgrade" is merged into main and deleted
    And I run "tminus upgrade"
    When I run "tminus status"
    Then it succeeds
    And the output contains "Red zones changed after their approval"
    And the output contains "c1 · Export · open · G1:ok"
    And the output contains "in .tminus/log.jsonl"

  Scenario: Once moved, the next upgrade starts from the pure text and has nothing to propose
    Given I run "tminus upgrade"
    And the branch "tminus-upgrade" is merged into main and deleted
    And I run "tminus upgrade"
    When I run "tminus upgrade"
    Then it succeeds
    And the output contains "Already up to date"
    And the file ".tminus" exists

  Scenario: A change in flight stops the move
    Given I run "tminus new-change c1 --title Export --paths src/app.txt"
    And the change "c1" is in state "building"
    When I run "tminus upgrade"
    Then it fails with a message that contains "c1 (building)"
    And the branch "tminus-upgrade" does not exist
