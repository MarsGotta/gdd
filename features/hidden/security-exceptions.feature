@hidden
Feature: Hidden examples for security exceptions

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
      color = "red"
      paths = ["osv-scanner.toml", "deny.toml"]
      reason = "Dependencies: supply chain"
      """
    And the gate commands:
      """
      [g2]
      unit = "true"

      [g3]
      acceptance_full = "true"
      mutation = "true"
      """
    And the scenario "features/app.feature" with 1 scenarios
    And the change "c1" plans "src/app.rs" and "osv-scanner.toml" and its build has started on its branch

  Scenario: The status counts exceptions from both files
    Given the file "osv-scanner.toml" says:
      """
      [[IgnoredVulns]]
      id = "GHSA-1111-2222-3333"
      reason = "a"
      """
    And the file "deny.toml" says:
      """
      [advisories]
      ignore = [{ id = "RUSTSEC-2026-0003", reason = "b" }]
      """
    When I run "gdd status"
    Then the output contains "Security exceptions: 2"

  Scenario: An override that ignores a package entirely is an exception
    Given the file "osv-scanner.toml" says:
      """
      [[PackageOverrides]]
      name = "lodash"
      ignore = true
      """
    When I run "gdd g3 c1"
    Then the failure of "exceptions" shows "osv-scanner.toml: package lodash has no reason"

  Scenario: A reason made of spaces is no reason
    Given the file "deny.toml" says:
      """
      [advisories]
      ignore = [{ id = "RUSTSEC-2026-0004", reason = "   " }]
      """
    When I run "gdd g3 c1"
    Then the failure of "exceptions" shows "deny.toml: RUSTSEC-2026-0004 has no reason"
