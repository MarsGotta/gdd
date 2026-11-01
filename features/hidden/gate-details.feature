@hidden
Feature: Mutation testing and the secrets check say what they checked (hidden examples)

  Background:
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["tminus/**", "bin/**"]
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

      [g3]
      acceptance_full = "true"
      mutation = "true"
      secrets = "true"
      """
    And the change "c1" plans "src/app.py" and "src/app.ts" and its build has started on its branch

  Scenario: A Python method that no test runs fails the gate
    Given the gate command "g3" "mutation" is the one of the "python" sheet
    And the program "uv" on the PATH prints "app.xǁCartǁcount__mutmut_1: no tests" and exits with 0
    When I run "tminus g3 c1"
    Then the gate fails on "mutation"
    And the failure of "mutation" shows "1 mutant(s) not killed (survived, or no test runs that code)"

  Scenario: A mutant of a module the change did not touch does not fail it
    Given the gate command "g3" "mutation" is the one of the "python" sheet
    And the program "uv" on the PATH prints "other.x_total__mutmut_1: no tests" and exits with 0
    When I run "tminus g3 c1"
    Then the gate passes

  Scenario: TypeScript with ten mutants is not nothing to mutate
    Given the gate command "g3" "mutation" is the one of the "typescript" sheet
    And the program "npx" on the PATH prints "INFO Instrumenter Instrumented 1 source file(s) with 10 mutant(s)" and exits with 1
    When I run "tminus g3 c1"
    Then the gate fails on "mutation"

  Scenario: A secret deeper in a new file is named at its line
    Given the gate command "g3" "secrets" is the one of the "typescript" sheet
    And the change "c1" has the file "src/deep/conf.ts" with a fake AWS key on line 7
    When I run "tminus g3 c1"
    Then the gate fails on "secrets"
    And the failure of "secrets" shows "secrets: src/deep/conf.ts:7: aws-access-token (value not shown)"
    And the output does not show the fake AWS key
