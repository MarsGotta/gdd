@hidden
Feature: Hidden examples for what the gates keep of a failed command is clean

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

  Scenario: A secret on a later line of a TypeScript file is named without the scanner's log
    Given the gate command "g3" "secrets" is the one of the "typescript" sheet
    And the change "c1" has the file "src/deep/conf.ts" with a fake AWS key on line 5
    When I run "tminus g3 c1"
    Then the gate fails on "secrets"
    And the failure of "secrets" shows "secrets: src/deep/conf.ts:5: aws-access-token (value not shown)"
    And the file ".tminus/changes/c1/g3.json" does not contain "scanned ~"
    And the output does not show the fake AWS key

  Scenario: A progress line drawn over itself in color keeps its last state and no color
    Given the gate command "g3" "acceptance_full" is "printf '\033[1m10%%\033[0m\r\033[1m50%%\033[0m\r\033[1m100%%\033[0m done\nError: 2 left\n'; exit 1"
    When I run "tminus g3 c1"
    Then the gate fails on "acceptance_full"
    And the failure of "acceptance_full" shows "100% done"
    And the file ".tminus/changes/c1/g3.json" does not contain "50%"

  Scenario: A Python mutant that survives fails even with the copies of an earlier run around
    Given the gate command "g3" "mutation" is the one of the "python" sheet
    And the file "src/app.py" says "def total(): return 1"
    And the file "mutants/src/app.py" says "def total(): return 2"
    And the program "uv" on the PATH prints "app.x_total__mutmut_3: survived" and exits with 0
    When I run "tminus g3 c1"
    Then the gate fails on "mutation"
    And the failure of "mutation" shows "1 mutant(s) not killed (survived, or no test runs that code)"
    And the file "mutants/src/app.py" does not exist
