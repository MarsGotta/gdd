Feature: Mutation testing and the secrets check say what they checked
  Mutation testing never passes on code that no test runs. mutmut marks such a mutant "no
  tests", and the Python sheet used to fail only on survivors, so a module that no test calls
  passed. Each sheet now fails a mutant that is neither caught nor timed out, says why when no
  unit test ran at all, and passes a change with nothing to mutate (constants, types, an empty
  module) with a last line that says so, which the gate report keeps as its summary. The secrets
  check reads only the lines the change adds and names the file, the line and the rule of each
  secret it finds; it never prints the value, nor a piece of it.

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

  Scenario Outline: A Python mutant that no test catches fails the gate
    Given the gate command "g3" "mutation" is the one of the "python" sheet
    And the program "uv" on the PATH prints "<mutant>" and exits with 0
    When I run "tminus g3 c1"
    Then the gate fails on "mutation"
    And the failure of "mutation" shows "1 mutant(s) not killed (survived, or no test runs that code)"

    Examples:
      | mutant                             |
      | app.x_total__mutmut_1: no tests    |
      | app.x_total__mutmut_2: survived    |
      | app.x_total__mutmut_3: suspicious  |

  Scenario: A Python mutant that timed out counts as caught, as in the other sheets
    Given the gate command "g3" "mutation" is the one of the "python" sheet
    And the program "uv" on the PATH prints "app.x_total__mutmut_4: timeout" and exits with 0
    When I run "tminus g3 c1"
    Then the gate passes
    And the output contains "mutation: every mutant of the changed Python code was caught"

  Scenario: Python code with nothing to mutate passes and says so
    Given the gate command "g3" "mutation" is the one of the "python" sheet
    And the program "uv" on the PATH prints "AssertionError: Filtered for specific mutants, but nothing matches" and exits with 1
    When I run "tminus g3 c1"
    Then the gate passes
    And the output contains "mutation: nothing to mutate in the changed Python files (no function or method)"

  Scenario: A Python project where no unit test runs fails and says why
    Given the gate command "g3" "mutation" is the one of the "python" sheet
    And the program "uv" on the PATH prints "failed to collect stats. runner returned 5" and exits with 1
    When I run "tminus g3 c1"
    Then the gate fails on "mutation"
    And the failure of "mutation" shows "mutation: no unit test ran (tests/unit/ collected none), so no mutant was checked"

  Scenario: TypeScript code with nothing to mutate passes and says so
    Given the gate command "g3" "mutation" is the one of the "typescript" sheet
    And the program "npx" on the PATH prints "INFO Instrumenter Instrumented 1 source file(s) with 0 mutant(s)" and exits with 1
    When I run "tminus g3 c1"
    Then the gate passes
    And the output contains "mutation: nothing to mutate in the changed TypeScript files"

  Scenario: TypeScript changes that no unit test runs fail and say why
    Given the gate command "g3" "mutation" is the one of the "typescript" sheet
    And the program "npx" on the PATH prints "ERROR Stryker No tests were executed." and exits with 1
    When I run "tminus g3 c1"
    Then the gate fails on "mutation"
    And the failure of "mutation" shows "mutation: no unit test runs the changed files, so no mutant was checked"

  Scenario Outline: The secrets check names the file, the line and the rule, never the value
    Given the gate command "g3" "secrets" is the one of the "<sheet>" sheet
    And the change "c1" has the file "src/settings.py" with a fake AWS key on line 3
    When I run "tminus g3 c1"
    Then the gate fails on "secrets"
    And the failure of "secrets" shows "secrets: src/settings.py:3: aws-access-token (value not shown)"
    And the output does not show the fake AWS key

    Examples:
      | sheet      |
      | python     |
      | rust       |
      | typescript |

  Scenario: A change that adds no secret passes the secrets check
    Given the gate command "g3" "secrets" is the one of the "python" sheet
    When I run "tminus g3 c1"
    Then the gate passes
