Feature: gdd upgrade
  A newer gdd may write some of the files `gdd init` wrote once (`gdd/*.toml`, the Brief, CI)
  differently. Those files are the project's, so `gdd upgrade` never writes them on main: it
  proposes the new version on the branch `gdd-upgrade`, merged three ways (what the older gdd
  wrote, what the project made of it, what this gdd writes), as Copier updates a project from
  its template. Files nobody touched take gdd's new text, files the project edited are merged
  with its changes, and lines both changed are marked as a conflict. The branch has two
  commits: what this gdd writes, pure (the base of the next upgrade), then the merge. The human
  reviews the branch and merges it.

  Background:
    Given an empty folder
    And I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"

  Scenario: gdd init records what it was asked, for the upgrade
    Then the TOML file "gdd/generated.toml" has "answers.name" = "Notes"
    And the TOML file "gdd/generated.toml" has "answers.stacks" = "rust"
    And the TOML file "gdd/generated.toml" has "answers.agents" = "claude, codex"
    And the TOML file "gdd/generated.toml" has "answers.platform" = "cli"

  Scenario: A file nobody touched takes the new text, on the branch
    And an older gdd "0.0.0" wrote ".github/workflows/gdd.yml" with "actions/checkout@v3" where this one writes "actions/checkout@v4"
    When I run "gdd upgrade"
    Then it succeeds
    And the output contains ".github/workflows/gdd.yml: updated (you had not changed it)"
    And the file ".github/workflows/gdd.yml" on the branch "gdd-upgrade" contains "actions/checkout@v4"
    And the file ".github/workflows/gdd.yml" contains "actions/checkout@v3"
    And the current branch is "main"

  Scenario: A file the project edited keeps its changes and takes gdd's
    And an older gdd "0.0.0" wrote ".github/workflows/gdd.yml" with "actions/checkout@v3" where this one writes "actions/checkout@v4"
    And in the file ".github/workflows/gdd.yml", "runs-on: ubuntu-latest" is replaced by "runs-on: ubuntu-24.04"
    And everything is committed on main
    When I run "gdd upgrade"
    Then it succeeds
    And the output contains ".github/workflows/gdd.yml: merged with your changes"
    And the file ".github/workflows/gdd.yml" on the branch "gdd-upgrade" contains "actions/checkout@v4"
    And the file ".github/workflows/gdd.yml" on the branch "gdd-upgrade" contains "runs-on: ubuntu-24.04"

  Scenario: Lines both changed are marked as a conflict
    And an older gdd "0.0.0" wrote ".github/workflows/gdd.yml" with "actions/checkout@v3" where this one writes "actions/checkout@v4"
    And in the file ".github/workflows/gdd.yml", "actions/checkout@v3" is replaced by "actions/checkout@v3.6"
    And everything is committed on main
    When I run "gdd upgrade"
    Then it succeeds
    And the output contains ".github/workflows/gdd.yml: conflict, settle the marks"
    And the file ".github/workflows/gdd.yml" on the branch "gdd-upgrade" contains "<<<<<<< yours"
    And the file ".github/workflows/gdd.yml" on the branch "gdd-upgrade" contains "actions/checkout@v3.6"
    And the file ".github/workflows/gdd.yml" on the branch "gdd-upgrade" contains ">>>>>>> gdd"
    And the output contains "git switch gdd-upgrade"

  Scenario: A file gdd writes as before stays the project's
    And an older gdd "0.0.0" wrote ".github/workflows/gdd.yml" with "actions/checkout@v3" where this one writes "actions/checkout@v4"
    And in the file "gdd/zones.toml", "Documentation" is replaced by "Documentation and notes"
    And everything is committed on main
    When I run "gdd upgrade"
    Then it succeeds
    And the output contains "gdd/zones.toml: yours, gdd writes it as before"
    And the file "gdd/zones.toml" on the branch "gdd-upgrade" contains "Documentation and notes"

  Scenario: A file the older gdd did not write is added
    Given an older gdd "0.0.0" did not write "gdd/models.toml"
    When I run "gdd upgrade"
    Then it succeeds
    And the output contains "gdd/models.toml: added"
    And the branch "gdd-upgrade" holds "gdd/models.toml"

  Scenario: The branch has the pure new text first, then the merge
    And an older gdd "0.0.0" wrote ".github/workflows/gdd.yml" with "actions/checkout@v3" where this one writes "actions/checkout@v4"
    When I run "gdd upgrade"
    Then it succeeds
    And the last commit on gdd-upgrade says "gdd upgrade 0.0.0 to"
    And the last commit on gdd-upgrade says "merged with the project's changes"
    And the file "gdd/generated.toml" on the branch "gdd-upgrade" does not contain "0.0.0"
    And the output contains "git diff main gdd-upgrade"
    And the output contains "git merge gdd-upgrade"

  Scenario: Nothing to propose when this gdd writes what the older one did
    When I run "gdd upgrade"
    Then it succeeds
    And the output contains "Already up to date"
    And the branch "gdd-upgrade" does not exist

  Scenario: Uncommitted changes stop it
    And an older gdd "0.0.0" wrote ".github/workflows/gdd.yml" with "actions/checkout@v3" where this one writes "actions/checkout@v4"
    And in the file "gdd/zones.toml", "Documentation" is replaced by "Docs"
    When I run "gdd upgrade"
    Then it fails with a message that contains "commit or stash"
    And the branch "gdd-upgrade" does not exist

  Scenario: An upgrade branch already there stops it
    And an older gdd "0.0.0" wrote ".github/workflows/gdd.yml" with "actions/checkout@v3" where this one writes "actions/checkout@v4"
    And I run "gdd upgrade"
    When I run "gdd upgrade"
    Then it fails with a message that contains "the branch gdd-upgrade is already there"

  Scenario: A project without the record of what gdd wrote says so
    Given an older gdd "0.0.0" did not write "gdd/generated.toml"
    When I run "gdd upgrade"
    Then it fails with a message that contains "gdd/generated.toml"

  Scenario: A Builder cannot upgrade
    And an older gdd "0.0.0" wrote ".github/workflows/gdd.yml" with "actions/checkout@v3" where this one writes "actions/checkout@v4"
    When the role "builder-build" runs "gdd upgrade"
    Then it fails with a message that contains "Builders and Reviewers cannot"
