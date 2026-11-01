Feature: tminus upgrade
  A newer tminus may write some of the files `tminus init` wrote once (`tminus/*.toml`, the Brief, CI)
  differently. Those files are the project's, so `tminus upgrade` never writes them on main: it
  proposes the new version on the branch `tminus-upgrade`, merged three ways (what the older tminus
  wrote, what the project made of it, what this tminus writes), as Copier updates a project from
  its template. Files nobody touched take tminus's new text, files the project edited are merged
  with its changes, and lines both changed are marked as a conflict. The branch has two
  commits: what this tminus writes, pure (the base of the next upgrade), then the merge. The human
  reviews the branch and merges it.

  Background:
    Given an empty folder
    And I run "tminus init --name Notes --platform cli --stack rust --agents claude codex"

  Scenario: tminus init records what it was asked, for the upgrade
    Then the TOML file "tminus/generated.toml" has "answers.name" = "Notes"
    And the TOML file "tminus/generated.toml" has "answers.stacks" = "rust"
    And the TOML file "tminus/generated.toml" has "answers.agents" = "claude, codex"
    And the TOML file "tminus/generated.toml" has "answers.platform" = "cli"

  Scenario: A file nobody touched takes the new text, on the branch
    And an older tminus "0.0.0" wrote ".github/workflows/tminus.yml" with "actions/checkout@v3" where this one writes "actions/checkout@v4"
    When I run "tminus upgrade"
    Then it succeeds
    And the output contains ".github/workflows/tminus.yml: updated (you had not changed it)"
    And the file ".github/workflows/tminus.yml" on the branch "tminus-upgrade" contains "actions/checkout@v4"
    And the file ".github/workflows/tminus.yml" contains "actions/checkout@v3"
    And the current branch is "main"

  Scenario: A file the project edited keeps its changes and takes tminus's
    And an older tminus "0.0.0" wrote ".github/workflows/tminus.yml" with "actions/checkout@v3" where this one writes "actions/checkout@v4"
    And in the file ".github/workflows/tminus.yml", "runs-on: ubuntu-latest" is replaced by "runs-on: ubuntu-24.04"
    And everything is committed on main
    When I run "tminus upgrade"
    Then it succeeds
    And the output contains ".github/workflows/tminus.yml: merged with your changes"
    And the file ".github/workflows/tminus.yml" on the branch "tminus-upgrade" contains "actions/checkout@v4"
    And the file ".github/workflows/tminus.yml" on the branch "tminus-upgrade" contains "runs-on: ubuntu-24.04"

  Scenario: Lines both changed are marked as a conflict
    And an older tminus "0.0.0" wrote ".github/workflows/tminus.yml" with "actions/checkout@v3" where this one writes "actions/checkout@v4"
    And in the file ".github/workflows/tminus.yml", "actions/checkout@v3" is replaced by "actions/checkout@v3.6"
    And everything is committed on main
    When I run "tminus upgrade"
    Then it succeeds
    And the output contains ".github/workflows/tminus.yml: conflict, settle the marks"
    And the file ".github/workflows/tminus.yml" on the branch "tminus-upgrade" contains "<<<<<<< yours"
    And the file ".github/workflows/tminus.yml" on the branch "tminus-upgrade" contains "actions/checkout@v3.6"
    And the file ".github/workflows/tminus.yml" on the branch "tminus-upgrade" contains ">>>>>>> tminus"
    And the output contains "git switch tminus-upgrade"

  Scenario: A file tminus writes as before stays the project's
    And an older tminus "0.0.0" wrote ".github/workflows/tminus.yml" with "actions/checkout@v3" where this one writes "actions/checkout@v4"
    And in the file "tminus/zones.toml", "Documentation" is replaced by "Documentation and notes"
    And everything is committed on main
    When I run "tminus upgrade"
    Then it succeeds
    And the output contains "tminus/zones.toml: yours, tminus writes it as before"
    And the file "tminus/zones.toml" on the branch "tminus-upgrade" contains "Documentation and notes"

  Scenario: A file the older tminus did not write is added
    Given an older tminus "0.0.0" did not write "tminus/models.toml"
    When I run "tminus upgrade"
    Then it succeeds
    And the output contains "tminus/models.toml: added"
    And the branch "tminus-upgrade" holds "tminus/models.toml"

  Scenario: The branch has the pure new text first, then the merge
    And an older tminus "0.0.0" wrote ".github/workflows/tminus.yml" with "actions/checkout@v3" where this one writes "actions/checkout@v4"
    When I run "tminus upgrade"
    Then it succeeds
    And the last commit on tminus-upgrade says "tminus upgrade 0.0.0 to"
    And the last commit on tminus-upgrade says "merged with the project's changes"
    And the file "tminus/generated.toml" on the branch "tminus-upgrade" does not contain "0.0.0"
    And the output contains "git diff main tminus-upgrade"
    And the output contains "git merge tminus-upgrade"

  Scenario: Nothing to propose when this tminus writes what the older one did
    When I run "tminus upgrade"
    Then it succeeds
    And the output contains "Already up to date"
    And the branch "tminus-upgrade" does not exist

  Scenario: Uncommitted changes stop it
    And an older tminus "0.0.0" wrote ".github/workflows/tminus.yml" with "actions/checkout@v3" where this one writes "actions/checkout@v4"
    And in the file "tminus/zones.toml", "Documentation" is replaced by "Docs"
    When I run "tminus upgrade"
    Then it fails with a message that contains "commit or stash"
    And the branch "tminus-upgrade" does not exist

  Scenario: An upgrade branch already there stops it
    And an older tminus "0.0.0" wrote ".github/workflows/tminus.yml" with "actions/checkout@v3" where this one writes "actions/checkout@v4"
    And I run "tminus upgrade"
    When I run "tminus upgrade"
    Then it fails with a message that contains "the branch tminus-upgrade is already there"

  Scenario: A project without the record of what tminus wrote says so
    Given an older tminus "0.0.0" did not write "tminus/generated.toml"
    When I run "tminus upgrade"
    Then it fails with a message that contains "tminus/generated.toml"

  Scenario: A Builder cannot upgrade
    And an older tminus "0.0.0" wrote ".github/workflows/tminus.yml" with "actions/checkout@v3" where this one writes "actions/checkout@v4"
    When the role "builder-build" runs "tminus upgrade"
    Then it fails with a message that contains "Agents that tminus started cannot run tminus upgrade"
