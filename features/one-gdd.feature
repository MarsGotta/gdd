Feature: One command, tminus
  People, agents and CI run the same command: tminus, the one on the PATH. A new project gets no
  bin/tminus entry point. The agents' instructions, their permissions, the CI workflow and tminus's own
  messages all say tminus. A project made earlier keeps working: its bin/tminus runs the tminus on the
  PATH, and the self-test still accepts a workflow that runs bin/tminus ci.

  Scenario: A new project has no bin/tminus and no file names it
    Given an empty folder
    When I run "tminus init --name Notes --platform cli --stack rust --agents claude codex opencode"
    Then it succeeds
    And the file "bin/tminus" does not exist
    And no file of the project contains "bin/tminus"

  Scenario: The agents' instructions call tminus
    Given an empty folder
    When I run "tminus init --name Notes --platform cli --stack rust --agents claude codex"
    Then it succeeds
    And the file "AGENTS.md" contains "`tminus` runs the gates"
    And the file ".claude/skills/tminus-build/SKILL.md" contains "You may only run `tminus g2`, `tminus guard`, `tminus note`"

  Scenario: Bootstrap points to the agent
    Given an empty folder
    And I run "tminus init --name Notes --platform cli --stack rust --agents claude codex"
    And the program "cargo" on the PATH writes the file "Cargo.lock"
    When I run "tminus bootstrap"
    Then it succeeds
    And the output contains "Next: in your coding agent, the tminus-setup command finishes the Setup"
    And the output does not contain "bin/tminus"

  Scenario: The self-test accepts a workflow that runs tminus ci
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["tminus/**", "bin/**", ".github/**"]
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
    And the file ".github/workflows/tminus.yml" says "run: tminus ci"
    And the scenario "features/base.feature" with 1 scenarios
    And everything is committed on main
    When I run "tminus selftest"
    Then it succeeds
    And the output contains "harness 4/4"

  Scenario: The self-test still accepts a workflow that runs bin/tminus ci
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["tminus/**", "bin/**", ".github/**"]
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
    And the file ".github/workflows/tminus.yml" says "run: bin/tminus ci"
    And the scenario "features/base.feature" with 1 scenarios
    And everything is committed on main
    When I run "tminus selftest"
    Then it succeeds
    And the output contains "harness 4/4"

  Scenario: The self-test refuses a workflow that runs neither
    Given a GDD project in a git repository with the protected paths:
      """
      [protected]
      all = ["tminus/**", "bin/**", ".github/**"]
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
    And the file ".github/workflows/tminus.yml" says "run: echo tminus"
    And the scenario "features/base.feature" with 1 scenarios
    And everything is committed on main
    When I run "tminus selftest"
    Then the exit code is not 0
    And the output contains "KO editing a protected file is blocked (Path Guard and CI)"
