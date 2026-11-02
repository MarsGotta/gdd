Feature: What the gates keep of a failed command is clean
  A failed gate command keeps what a person or an agent needs to read: tminus keeps the output as
  a terminal shows it, so a progress line drawn over itself keeps its last state and color codes
  go. The secrets check keeps only its own lines, never the scanner's log, and the Python mutation
  check keeps the mutants that were not killed rather than the whole run of mutmut. Each Python
  mutation run starts without the copies (mutants/) an earlier run left, so a test the change
  deleted cannot still catch mutants, nor hide that no unit test ran.

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

  Scenario Outline: The secrets check keeps its own lines and none of the scanner's log
    Given the gate command "g3" "secrets" is the one of the "<sheet>" sheet
    And the change "c1" has the file "src/settings.py" with a fake AWS key on line 3
    When I run "tminus g3 c1"
    Then the gate fails on "secrets"
    And the failure of "secrets" shows "secrets: src/settings.py:3: aws-access-token (value not shown)"
    And the file ".tminus/changes/c1/g3.json" does not contain "leaks found"
    And the output does not show the fake AWS key

    Examples:
      | sheet      |
      | python     |
      | rust       |
      | typescript |

  Scenario: A failing command keeps only the last frame of a progress line
    Given the gate command "g3" "acceptance_full" is "printf 'Running 1/3\rRunning 2/3\rRunning 3/3\n1 scenario failed\n'; exit 1"
    When I run "tminus g3 c1"
    Then the gate fails on "acceptance_full"
    And the failure of "acceptance_full" shows "Running 3/3"
    And the file ".tminus/changes/c1/g3.json" does not contain "Running 1/3"

  Scenario: A failing command keeps its words without the color codes
    Given the gate command "g3" "acceptance_full" is "printf '\033[31mFAILED\033[0m step 7\n'; exit 1"
    When I run "tminus g3 c1"
    Then the gate fails on "acceptance_full"
    And the failure of "acceptance_full" shows "FAILED step 7"
    And the file ".tminus/changes/c1/g3.json" does not contain "[31m"

  Scenario: A Python mutation failure keeps the mutants that were not killed, not the whole run
    Given the gate command "g3" "mutation" is the one of the "python" sheet
    And the program "uv" on the PATH prints "app.x_total__mutmut_2: survived" and exits with 0
    When I run "tminus g3 c1"
    Then the gate fails on "mutation"
    And the failure of "mutation" shows "1 mutant(s) not killed (survived, or no test runs that code)"
    And the file ".tminus/changes/c1/g3.json" contains "app.x_total__mutmut_2: survived" exactly 1 times

  Scenario: The Python mutation run starts without the copies an earlier run left
    Given the gate command "g3" "mutation" is the one of the "python" sheet
    And the file "src/app.py" says "def total(): return 1"
    And the file "mutants/tests/unit/test_gone.py" says "def test_gone(): pass"
    And the program "uv" on the PATH prints "failed to collect stats. runner returned 5" and exits with 1
    When I run "tminus g3 c1"
    Then the gate fails on "mutation"
    And the failure of "mutation" shows "mutation: no unit test ran (tests/unit/ collected none), so no mutant was checked"
    And the file "mutants/tests/unit/test_gone.py" does not exist
