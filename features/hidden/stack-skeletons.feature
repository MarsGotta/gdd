@hidden
Feature: Hidden examples for stack skeletons

  Scenario: A Python project name with dashes becomes a package
    Given an empty folder
    When I run "gdd init --name my-garden --platform web --stack python --agents claude codex"
    Then git tracks "src/my_garden/__init__.py"

  Scenario: Bootstrap runs the stacks in the order of the project
    Given an empty folder
    And I run "gdd init --name Notes --platform desktop --stack rust typescript --agents claude codex"
    And the program "cargo" on the PATH writes the file "Cargo.lock"
    And the program "npm" on the PATH writes the file "package-lock.json"
    When I run "gdd bootstrap"
    Then the output contains "rust: cargo generate-lockfile"
    And the output contains "typescript: npm install --no-fund --no-audit"
    And git tracks "package-lock.json"

  Scenario: A TypeScript folder with its own package.json keeps it
    Given the folder is a git repository with the file "package.json"
    When I run "gdd init --name Notes --platform web --stack typescript --agents claude codex"
    Then it succeeds
    And the file "eslint.config.js" does not exist
