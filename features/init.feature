Feature: gdd init
  A project gets GDD from flags, without a screen (the form comes later, on top of this). gdd init
  writes the harness into the folder it runs in: the project basics, the gate commands of the
  chosen stack sheets, the protected paths, a first zone map, which agent and model plays each role,
  the Brief skeleton, bin/gdd, the CI workflow and the agents' commands; then it commits what it
  wrote, and only that. It never overwrites a file: when one is already there it names it and
  writes nothing. The Reviewer is of another model family than the Builder; with one family only,
  the human must say they accept it, and the choice is recorded with its date.

  Background:
    Given an empty folder

  Scenario: A new folder gets the harness and its first commit
    When I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"
    Then it succeeds
    And the last commit on main says "gdd init"
    And the file "bin/gdd" is executable
    And the file ".gitignore" contains ".gdd/"
    And the file "gdd/generated.toml" contains "gdd/commands.toml"

  Scenario: The project basics are written
    When I run "gdd init --name Notes --platform cli --stack rust --agents claude codex --language es"
    Then the TOML file "gdd/project.toml" has "name" = "Notes"
    And the TOML file "gdd/project.toml" has "platform" = "cli"
    And the TOML file "gdd/project.toml" has "stack" = "rust"
    And the TOML file "gdd/project.toml" has "language" = "es"

  Scenario: The gate commands come from the stack sheet
    When I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"
    Then the TOML file "gdd/commands.toml" has "g2.unit" = "cargo test --workspace"
    And the TOML file "gdd/commands.toml" has "release.simulated" = "true"

  Scenario: Two stacks run the commands of both and protect the paths of both
    When I run "gdd init --name Notes --platform desktop --stack rust typescript --agents claude codex"
    Then the TOML file "gdd/commands.toml" has "g2.lint-typescript" = "npx eslint . --max-warnings 0"
    And the TOML file "gdd/rules.toml" has "protected.builder_test" = "src/**, crates/*/src/**"
    And the TOML file "gdd/project.toml" has "stack" = "rust, typescript"

  Scenario: The protected paths cover the harness and the approved steps
    When I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"
    Then the TOML file "gdd/rules.toml" has "protected.builder_build" = "features/**, tests/acceptance/**, tests/acceptance.rs"
    And the TOML file "gdd/rules.toml" has "main_branch" = "main"
    And the file "gdd/rules.toml" contains ".github/**"

  Scenario: The supply chain of the stack is a red zone from the start
    Given I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"
    When I run "gdd new-change c1 --title Deps --paths Cargo.lock"
    Then its color is "red"

  Scenario: Claude Code builds and Codex reviews
    When I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"
    Then the TOML file "gdd/models.toml" has "builder.red.model" = "opus"
    And the TOML file "gdd/models.toml" has "builder.yellow.model" = "sonnet"
    And the TOML file "gdd/models.toml" has "reviewer.integration" = "codex"
    And the TOML file "gdd/models.toml" has "reviewer.effort" = "high"

  Scenario: One model family only is refused unless the human accepts it
    When I run "gdd init --name Notes --platform cli --stack rust --agents claude"
    Then it fails with a message that contains "one model family only"
    And the file "gdd/project.toml" does not exist

  Scenario: Accepting one model family is recorded with its date
    When I run "gdd init --name Notes --platform cli --stack rust --agents claude --accept-same-family"
    Then it succeeds
    And the TOML file "gdd/models.toml" has "reviewer.integration" = "claude"
    And the file "gdd/models.toml" contains "same_family_accepted"

  Scenario: The agents get the GDD commands and the stack instructions
    When I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"
    Then the file ".claude/skills/gdd-setup/SKILL.md" contains "name: gdd-setup"
    And the file ".agents/skills/gdd-stack-rust/SKILL.md" contains "# GDD · Rust"

  Scenario: The Brief keeps the description and the sensitive areas in words
    When I run "gdd init --name Notes --platform cli --stack rust --agents claude codex --description A notes app for the terminal --sensitive Losing notes --sensitive Leaking private notes"
    Then the file "docs/brief.md" contains "A notes app for the terminal"
    And the file "docs/brief.md" contains "1. Losing notes"
    And the file "docs/brief.md" contains "2. Leaking private notes"

  Scenario: A file that is already there is never overwritten
    Given the folder is a git repository with the file "docs/brief.md"
    When I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"
    Then it fails with a message that contains "docs/brief.md already exists"
    And the file "gdd/project.toml" does not exist

  Scenario: A GDD project cannot be set up twice
    Given I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"
    When I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"
    Then it fails with a message that contains "already a GDD project"

  Scenario: The code already in the repository is left as it is
    Given the folder is a git repository with the file "src/main.rs"
    And the file "notes.txt" says "mine"
    When I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"
    Then it succeeds
    And git does not track "notes.txt"
    And the last commit on main says "gdd init"

  Scenario: Instructions the project already has are left alone
    Given the file "AGENTS.md" says "Our rules"
    When I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"
    Then it succeeds
    And the file "AGENTS.md" contains "Our rules"
    And the output contains "add to AGENTS.md"

  Scenario: An unknown stack writes nothing
    When I run "gdd init --name Notes --platform cli --stack cobol --agents claude codex"
    Then it fails with a message that contains "unknown stack cobol"
    And the file "gdd/project.toml" does not exist

  Scenario: Without agents named, gdd looks for them on the PATH
    Given no coding agent is on the PATH
    When I run "gdd init --name Notes --platform cli --stack rust"
    Then it fails with a message that contains "no coding agent found on the PATH"
