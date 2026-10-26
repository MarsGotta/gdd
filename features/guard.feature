Feature: Path Guard
  Path Guard looks at what a change touched since its build started and stops a role
  that touched what it must not: the harness for everyone, the approved scenarios and the
  steps written in test mode for build mode, production code for test mode. It also stops
  anyone who changed a scenario approved in G1.

  Background:
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["gdd/**", "bin/**"]
      builder_build = ["features/**", "tests/steps/**"]
      builder_test = ["src/**"]
      """
    And the zone map:
      """
      default = "yellow"

      [[zone]]
      color = "green"
      paths = ["docs/**"]

      [[zone]]
      color = "red"
      paths = ["src/storage/**"]
      """
    And the change "c1" plans "src/app.rs" and "tests/steps/app.rs" and its build has started on its branch

  Scenario: Build mode inside its paths passes
    When the change writes "src/app.rs"
    And I run "gdd guard c1 --role builder-build"
    Then Path Guard passes

  Scenario: The harness is protected from every role
    When the change writes "gdd/rules.toml"
    And I run "gdd guard c1 --role builder-build"
    Then Path Guard fails naming "gdd/rules.toml"

  Scenario: Build mode cannot touch the steps written in test mode
    When the change writes "tests/steps/app.rs"
    And I run "gdd guard c1 --role builder-build"
    Then Path Guard fails naming "tests/steps/app.rs"

  Scenario: Test mode cannot touch production code
    When the change writes "src/app.rs"
    And I run "gdd guard c1 --role builder-test"
    Then Path Guard fails naming "src/app.rs"

  Scenario: Committed files count, not only the working tree
    When the change commits "bin/tool"
    And I run "gdd guard c1 --role builder-build"
    Then Path Guard fails naming "bin/tool"

  Scenario: A scenario approved in G1 cannot change afterwards
    Given the scenario "features/app.feature" was approved in G1
    When the change modifies "features/app.feature"
    And I run "gdd guard c1 --role builder-test"
    Then Path Guard fails naming "features/app.feature" and "approved in G1"

  Scenario: Only strict mode turns a failure into an exit code
    When the change writes "gdd/rules.toml"
    And I run "gdd guard c1 --role builder-build"
    Then the exit code is 0
    When I run "gdd guard c1 --role builder-build --strict"
    Then the exit code is not 0

  Scenario: After a snapshot, only the new round counts
    Given the change wrote "src/app.rs"
    And I run "gdd snapshot c1"
    When the change writes "tests/steps/app.rs"
    And I run "gdd guard c1 --role builder-test --since-snapshot"
    Then Path Guard passes
    When I run "gdd guard c1 --role builder-test"
    Then Path Guard fails naming "src/app.rs"
