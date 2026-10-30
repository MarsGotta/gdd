Feature: Faster mutation testing, and no gate fails because the machine is slow
  Mutation testing rebuilds the code and runs the tests once per mutant, so it is the slowest
  check of G3 (about 3 s per mutant on this repository, 70% of it building). The Rust sheet builds
  the mutants without debug info (a mutant only has to fail, nobody debugs it) and links them with
  mold when mold is installed (cargo-mutants' performance guide: about 20% faster). A mutant can
  time out because it hangs (a loop that never ends) or because the machine is busy: when only
  timeouts are left, the sheet runs those mutants again, one at a time with more time. A mutant
  that hangs again counts as caught, as Stryker counts it; one that now survives is a missed
  mutant. The time limit of each gate command is set in gdd/rules.toml.

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
    And the gate commands:
      """
      [g2]
      lint = "true"
      unit = "echo unit-ran"
      types = ""

      [g3]
      acceptance_full = "true"
      mutation = "echo mutation-ran"
      """
    And the change "c1" plans "src/app.rs" and "tests/steps/app.rs" and its build has started on its branch

  Scenario: The Rust sheet builds the mutants without debug info
    When I run "gdd stacks show rust"
    Then the output contains "CARGO_PROFILE_TEST_DEBUG=0"

  Scenario: The Rust sheet links the mutants with mold when it is installed
    When I run "gdd stacks show rust"
    Then the output contains "command -v mold"
    And the output contains "-fuse-ld=mold"

  Scenario: Mutants that only timed out get a second chance, one at a time with more time
    Given the gate command "g3" "mutation" is the one of the "rust" sheet
    And the program "cargo" on the PATH exits with 3, then 0
    When I run "gdd g3 c1"
    Then the gate passes
    And the program "cargo" was called 2 times
    And call 2 of the program "cargo" includes "--iterate"
    And call 2 of the program "cargo" includes "-j 1"
    And call 2 of the program "cargo" includes "--minimum-test-timeout 120"

  Scenario: A mutant that hangs again counts as caught
    Given the gate command "g3" "mutation" is the one of the "rust" sheet
    And the program "cargo" on the PATH exits with 3, then 3
    When I run "gdd g3 c1"
    Then the gate passes
    And the output contains "hung again: counted as caught"

  Scenario: A mutant that survives its second chance is a missed mutant
    Given the gate command "g3" "mutation" is the one of the "rust" sheet
    And the program "cargo" on the PATH exits with 3, then 2
    When I run "gdd g3 c1"
    Then the gate fails on "mutation"

  Scenario: Missed mutants get no second chance
    Given the gate command "g3" "mutation" is the one of the "rust" sheet
    And the program "cargo" on the PATH exits with 2, then 0
    When I run "gdd g3 c1"
    Then the gate fails on "mutation"
    And the program "cargo" was called 1 times

  Scenario: The time limit of a gate command comes from the rules
    Given the rules have "limits.command_seconds" = "1"
    And the gate command "g2" "unit" is "sleep 5"
    When I run "gdd g2 c1"
    Then the gate fails on "unit"
    And the failure of "unit" shows "timed out (1s): raise limits.command_seconds in gdd/rules.toml"

  Scenario: A time limit that is not a whole number of seconds is refused
    Given the rules have "limits.command_seconds" = "0"
    When I run "gdd g2 c1"
    Then it fails with a message that contains "limits.command_seconds must be a whole number of seconds above 0"
