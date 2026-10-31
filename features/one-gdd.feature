Feature: One command, gdd
  People, agents and CI run the same command: gdd, the one on the PATH. A new project gets no
  bin/gdd entry point. The agents' instructions, their permissions, the CI workflow and gdd's own
  messages all say gdd. A project made earlier keeps working: its bin/gdd runs the gdd on the
  PATH, and the self-test still accepts a workflow that runs bin/gdd ci.

  Scenario: A new project has no bin/gdd and no file names it
    Given an empty folder
    When I run "gdd init --name Notes --platform cli --stack rust --agents claude codex opencode"
    Then it succeeds
    And the file "bin/gdd" does not exist
    And no file of the project contains "bin/gdd"

  Scenario: The agents' instructions call gdd
    Given an empty folder
    When I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"
    Then it succeeds
    And the file "AGENTS.md" contains "`gdd` runs the gates"
    And the file ".claude/skills/gdd-build/SKILL.md" contains "You may only run `gdd g2`, `gdd guard`, `gdd note`"

  Scenario: Bootstrap points to the agent
    Given an empty folder
    And I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"
    And the program "cargo" on the PATH writes the file "Cargo.lock"
    When I run "gdd bootstrap"
    Then it succeeds
    And the output contains "Next: in your coding agent, the gdd-setup command finishes the Setup"
    And the output does not contain "bin/gdd"

  Scenario: The self-test accepts a workflow that runs gdd ci
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["gdd/**", "bin/**", ".github/**"]
      """
    And the gate commands:
      """
      [g2]
      unit = "test ! -f tests/failing.txt"

      [selftest]
      file = "tests/failing.txt"
      content = "fails"

      [release]
      simulated = true
      """
    And the file ".github/workflows/gdd.yml" says "run: gdd ci"
    And the scenario "features/base.feature" with 1 scenarios
    And everything is committed on main
    When I run "gdd selftest"
    Then it succeeds
    And the output contains "harness 4/4"

  Scenario: The self-test still accepts a workflow that runs bin/gdd ci
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["gdd/**", "bin/**", ".github/**"]
      """
    And the gate commands:
      """
      [g2]
      unit = "test ! -f tests/failing.txt"

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
    Then it succeeds
    And the output contains "harness 4/4"

  Scenario: The self-test refuses a workflow that runs neither
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["gdd/**", "bin/**", ".github/**"]
      """
    And the gate commands:
      """
      [g2]
      unit = "test ! -f tests/failing.txt"

      [selftest]
      file = "tests/failing.txt"
      content = "fails"

      [release]
      simulated = true
      """
    And the file ".github/workflows/gdd.yml" says "run: echo gdd"
    And the scenario "features/base.feature" with 1 scenarios
    And everything is committed on main
    When I run "gdd selftest"
    Then the exit code is not 0
    And the output contains "KO editing a protected file is blocked (Path Guard and CI)"
