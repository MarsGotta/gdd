Feature: Stack skeletons
  A new project starts with the minimum code of each of its stacks: the scenario runner wired to
  GDD_FEATURES and GDD_HIDDEN, one function with its unit tests, and the configuration of every
  gate tool, checked for real so that every gate is green on day one. gdd init writes the skeleton
  of each stack the folder has no project of yet, never over a file, and the lines its tools need
  in .gitignore; the CI workflow sets up the tools of each stack. gdd init stays offline: gdd
  bootstrap runs the first install of each stack, which writes its lock file, and commits it. The
  self-test only counts a failing test as caught when G2 was green without it.

  Scenario: A new Rust project gets its skeleton
    Given an empty folder
    When I run "gdd init --name Notes App --platform cli --stack rust --agents claude codex"
    Then it succeeds
    And the file "Cargo.toml" contains "name = \"notes_app\""
    And the file "tests/acceptance.rs" contains "GDD_FEATURES"
    And the file "deny.toml" contains "[licenses.private]"
    And git tracks "src/lib.rs"

  Scenario: A new TypeScript project gets its skeleton
    Given an empty folder
    When I run "gdd init --name Notes App --platform web --stack typescript --agents claude codex"
    Then the file "package.json" contains "\"name\": \"notes_app\""
    And the file "cucumber.js" contains "features/steps/**/*.ts"
    And the file "osv-scanner.toml" contains "IgnoredVulns"

  Scenario: A new Python project gets its skeleton
    Given an empty folder
    When I run "gdd init --name Notes App --platform web --stack python --agents claude codex"
    Then the file "src/notes_app/greeting.py" contains "def greeting"
    And the file "pyproject.toml" contains "root_package = \"notes_app\""
    And the file "tests/unit/test_greeting.py" contains "from notes_app.greeting import greeting"

  Scenario: The project name becomes an identifier
    Given an empty folder
    When I run "gdd init --name 2nd Brain! --platform cli --stack rust --agents claude codex"
    Then the file "Cargo.toml" contains "name = \"app_2nd_brain\""

  Scenario: The tools of every stack are kept out of git
    Given an empty folder
    When I run "gdd init --name Notes --platform desktop --stack rust typescript --agents claude codex"
    Then the file ".gitignore" contains "node_modules/"
    And the file ".gitignore" contains "/target"
    And the file ".gitignore" contains ".gdd/"

  Scenario: Two stacks share a file they both have
    Given an empty folder
    When I run "gdd init --name Notes --platform web --stack rust python --agents claude codex"
    Then it succeeds
    And git tracks "features/.gitkeep"

  Scenario: A folder with a project of the stack keeps its own code
    Given the folder is a git repository with the file "Cargo.toml"
    When I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"
    Then it succeeds
    And the file "src/lib.rs" does not exist
    And the file "deny.toml" does not exist

  Scenario: A skeleton file already there stops everything
    Given the folder is a git repository with the file "src/lib.rs"
    When I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"
    Then it fails with a message that contains "src/lib.rs already exists"
    And the file "gdd/project.toml" does not exist

  Scenario: The gate commands are the ones checked for real
    Given an empty folder
    When I run "gdd init --name Notes --platform web --stack python --agents claude codex"
    Then the TOML file "gdd/commands.toml" has "g2.unit" = "uv run pytest -q tests/unit"
    And the TOML file "gdd/commands.toml" has "g3.lockfile" = "uv lock --check"

  Scenario: CI sets up the tools of each stack
    Given an empty folder
    When I run "gdd init --name Notes --platform web --stack rust python --agents claude codex"
    Then the file ".github/workflows/gdd.yml" contains "dtolnay/rust-toolchain"
    And the file ".github/workflows/gdd.yml" contains "astral-sh/setup-uv"
    And the file ".github/workflows/gdd.yml" contains "run: bin/gdd ci"

  Scenario: gdd init says what comes next
    Given an empty folder
    When I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"
    Then the output contains "bin/gdd bootstrap"

  Scenario: Bootstrap installs each stack and commits its lock file
    Given an empty folder
    And I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"
    And the program "cargo" on the PATH writes the file "Cargo.lock"
    When I run "gdd bootstrap"
    Then it succeeds
    And the output contains "rust: cargo generate-lockfile"
    And git tracks "Cargo.lock"
    And the last commit on main says "gdd bootstrap"

  Scenario: A failed install stops the bootstrap and commits nothing
    Given an empty folder
    And I run "gdd init --name Notes --platform web --stack typescript --agents claude codex"
    And the program "npm" on the PATH fails
    When I run "gdd bootstrap"
    Then it fails with a message that contains "typescript: npm install --no-fund --no-audit failed"
    And the last commit on main says "gdd init"

  Scenario: Bootstrap with nothing new to commit still succeeds
    Given an empty folder
    And I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"
    And the program "cargo" on the PATH does nothing
    When I run "gdd bootstrap"
    Then it succeeds
    And the last commit on main says "gdd init"

  Scenario: The self-test does not count a G2 that was already red
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["gdd/**", "bin/**", ".github/**"]
      """
    And the gate commands:
      """
      [g2]
      unit = "test ! -f tests/failing.txt"
      lint = "false"

      [selftest]
      file = "tests/failing.txt"
      content = "fails"

      [release]
      simulated = true
      """
    And the file ".github/workflows/gdd.yml" says "run: bin/gdd ci"
    And the scenario "features/base.feature" with 1 scenarios
    And everything is committed on main
    When I run "gdd selftest"
    Then the exit code is not 0
    And the output contains "KO a failing test turns G2 red (G2 is red without it: lint)"
