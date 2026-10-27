Feature: Stack sheets
  gdd comes with one data sheet per stack (Rust, TypeScript, Python): the gate commands, the
  scenario runner, the self-test file, the paths each Builder mode must not touch, and the tools
  chosen for each need. A project uses one or more of them (`stack` in gdd/project.toml). `gdd
  stacks` lists the sheets and shows one; `gdd stacks compose` shows what several give together,
  without writing anything: every gate runs the commands of every sheet, named `<command>-<stack>`
  so a gate still knows its mutation commands, a command that is the same in every sheet runs
  once, and the protected paths add up. `gdd agents install` also writes the instructions of each
  stack the project uses, as the command `gdd-stack-<name>`.

  Background:
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["gdd/**"]
      """
    And the models:
      """
      [orchestrator]
      integration = "claude"
      model = "opus"

      [builder.yellow]
      integration = "claude"
      model = "sonnet"

      [builder_test]
      integration = "claude"
      model = "sonnet"

      [reviewer]
      integration = "opencode"
      model = "kimi-k3"
      """

  Scenario: gdd lists the sheets it comes with
    When I run "gdd stacks"
    Then it succeeds
    And the output contains "rust · Rust · cucumber-rs"
    And the output contains "typescript · TypeScript · cucumber-js"
    And the output contains "python · Python · pytest-bdd"

  Scenario: A sheet shows the tool chosen for each need and its alternative
    When I run "gdd stacks show python"
    Then it succeeds
    And the output contains "Gherkin runner (G2/G3): pytest-bdd (1.5k stars), shares pytest · alternative: behave (3.5k stars)"

  Scenario: A sheet shows its gate commands
    When I run "gdd stacks show python"
    Then the output contains "g2.types: mypy src"

  Scenario: An unknown sheet is named in the error
    When I run "gdd stacks show cobol"
    Then it fails with a message that contains "unknown stack cobol (known: python, rust, typescript)"

  Scenario: One sheet keeps the names of its commands
    When I run "gdd stacks compose rust"
    Then it succeeds
    And the output contains "acceptance_full = "
    And the output does not contain "acceptance_full-rust"

  Scenario: Two sheets run the commands of both
    When I run "gdd stacks compose rust typescript"
    Then it succeeds
    And the output contains "lint-rust = "
    And the output contains "lint-typescript = "

  Scenario: Mutation commands keep the word mutation first
    When I run "gdd stacks compose rust typescript"
    Then the output contains "mutation-rust = "
    And the output contains "mutation-typescript = "

  Scenario: A command that is the same in every sheet runs once
    When I run "gdd stacks compose rust python"
    Then the output contains "secrets = "
    And the output does not contain "secrets-rust"

  Scenario: Protected paths add up and are listed once
    When I run "gdd stacks compose python typescript"
    Then the output contains "tests/acceptance/**"
    And the output contains "src/**" exactly 1 times

  Scenario: The installs run one after the other
    When I run "gdd stacks compose rust python"
    Then the output contains "cargo fetch --locked && python -m pip install"

  Scenario: The self-test file comes from the first sheet
    When I run "gdd stacks compose python rust"
    Then the output contains "tests/unit/test_gdd_selftest.py"
    And the output does not contain "gdd_selftest_failing.rs"

  Scenario: The same sheet twice counts once
    When I run "gdd stacks compose rust rust"
    Then it succeeds
    And the output does not contain "lint-rust"

  Scenario: Composing needs at least one sheet
    When I run "gdd stacks compose"
    Then it fails with a message that contains "name at least one stack"

  Scenario: The agents get the instructions of each stack of the project
    Given the project uses the stacks "rust python"
    When I run "gdd agents install"
    Then it succeeds
    And the file ".claude/skills/gdd-stack-rust/SKILL.md" contains "# GDD · Rust"
    And the file ".opencode/commands/gdd-stack-python.md" contains "# GDD · Python"
    And the file ".claude/skills/gdd-stack-typescript/SKILL.md" does not exist

  Scenario: A project with one stack written as a word gets its instructions
    Given the project uses the stack "typescript"
    When I run "gdd agents install"
    Then the file ".claude/skills/gdd-stack-typescript/SKILL.md" contains "name: gdd-stack-typescript"

  Scenario: Check names the instructions of a stack added after the install
    Given the project uses the stack "rust"
    And I run "gdd agents install"
    And the project uses the stacks "rust python"
    When I run "gdd agents check"
    Then it fails with a message that contains "gdd-stack-python"

  Scenario: A project that names an unknown stack cannot install
    Given the project uses the stack "cobol"
    When I run "gdd agents install"
    Then it fails with a message that contains "unknown stack cobol (known: python, rust, typescript)"

  Scenario: The Builder is told to read the stack instructions
    Given the project uses the stack "rust"
    When I run "gdd agents install"
    Then the file ".claude/skills/gdd-build/SKILL.md" contains "gdd-stack-<name>"
