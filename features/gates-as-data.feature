Feature: Gates are declared as data
  The core of tminus defines no gate: it only knows how to run well the gates a blueprint
  declares. The software blueprint declares its gates as data: what each one checks, the kinds
  of command it claims (a command's kind is its name up to the first -), its optional commands
  and its named checks. A command no gate claims goes to the gate that checks the work, so no
  command is ever left out. The commands file and the records keep their old names: [g2] is the
  stage of every round and [g3] the stage before integrating, g2.json and g3.json keep the key
  of each stage, and the log and the known debt say G2 and G3. Every command of a stage runs
  once, in the order of its name, whatever gate claims it; an optional command runs only where
  the level of the change asks for it. Nothing a project wrote before reads differently.

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
      """
    And the gate commands:
      """
      [g2]
      unit = "echo unit-ran"
      lint = "true"
      security = "true"
      smoke = "echo smoke-ran"
      types = ""

      [g3]
      acceptance_full = "true"
      mutation = "echo mutation-ran"
      mutation-slow = "echo slow-ran"
      secrets = "true"
      """
    And the scenario "features/base.feature" with 1 scenarios

  Scenario: Every command of a stage runs once, in the order of its name, whatever gate claims it
    Given the change "c1" plans "src/app.rs" and its build has started on its branch
    When I run "tminus g2 c1"
    Then the gate passes
    And the gate passed "lint", "security", "smoke", "unit" and "path-guard"
    And in the file ".tminus/changes/c1/g2.json", "lint" comes before "security"
    And in the file ".tminus/changes/c1/g2.json", "security" comes before "smoke"
    And in the file ".tminus/changes/c1/g2.json", "smoke" comes before "unit"

  Scenario: A command no gate claims goes to the gate that checks the work
    Given the change "c1" plans "src/app.rs" and its build has started on its branch
    And the gate command "g2" "smoke" is "echo smoke-broke; exit 4"
    When I run "tminus g2 c1"
    Then the gate fails on "smoke"
    And the failure of "smoke" shows "smoke-broke"

  Scenario: An optional command does not run where the level of the change does not ask for it
    Given the change "c1" plans "docs/a.md" and its build has started on its branch
    When I run "tminus g3 c1"
    Then the gate skipped "mutation"
    And the gate skipped "mutation-slow"
    And the gate passed "acceptance_full" and "secrets"

  Scenario: An optional command runs where the level of the change asks for it, whatever its full name
    Given the change "c1" plans "src/app.rs" and its build has started on its branch
    When I run "tminus g3 c1"
    Then the gate passes
    And the gate passed "mutation" and "mutation-slow"

  Scenario: Old records and old rules files are read as before
    Given the gate commands:
      """
      [g2]
      lint = "echo unused import; false"
      unit = "true"

      [g3]
      acceptance_full = "true"
      security = "true"
      """
    And I run "tminus adopt"
    And the gate commands:
      """
      [g2]
      lint = "true"
      unit = "true"

      [g3]
      acceptance_full = "true"
      security = "true"
      """
    And the change "c1" plans "src/app.rs" and its build has started on its branch
    And the file ".tminus/changes/c1/g2.json" says:
      """
      {"ok": true, "gate": "G2", "color": "yellow", "passed": ["lint", "unit", "path-guard"], "skipped": [], "failures": [], "out_of_scope": [], "paid_debt": ["lint"]}
      """
    When the change writes "src/app.rs"
    And I run "tminus g3 c1"
    And I run "tminus gate G3 ok c1 --by machines"
    And I run "tminus merge c1"
    Then it succeeds
    And the last commit on main says "Known debt paid: lint"
    And the file "tminus/debt.toml" does not exist
    And the log has the gate command "acceptance_full" of "c1" in "G3" with result "ok"
