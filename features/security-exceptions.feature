Feature: Security exceptions
  A vulnerability in a dependency blocks G3, dev tools included. The developer may make an
  exception, under their own responsibility, in the scanner's own file: [[IgnoredVulns]] (or a
  [[PackageOverrides]] that ignores vulnerabilities) in osv-scanner.toml, or [advisories] ignore in
  deny.toml. Those files are a red zone, so adding one is a red change approved at a sitting. gdd
  shows every active exception in the status, refuses one without a reason in G3, and G3 names
  the exceptions a change adds, so the human sees them at G4.

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

  Scenario: A project without exceptions says so
    When I run "gdd status"
    Then the output contains "Security exceptions: none"

  Scenario: The status lists an OSV exception with its end date and reason
    Given the file "osv-scanner.toml" says:
      """
      [[IgnoredVulns]]
      id = "GHSA-4mjr-xmp4-gh2g"
      ignoreUntil = 2099-01-31
      reason = "qs inside StrykerJS, dev only"
      """
    When I run "gdd status"
    Then the output contains "Security exceptions: 1"
    And the output contains "GHSA-4mjr-xmp4-gh2g · osv-scanner.toml · until 2099-01-31 · qs inside StrykerJS, dev only"

  Scenario: A cargo-deny exception has no end date
    Given the file "deny.toml" says:
      """
      [advisories]
      ignore = [{ id = "RUSTSEC-2026-0001", reason = "not reachable from our code" }]
      """
    When I run "gdd status"
    Then the output contains "RUSTSEC-2026-0001 · deny.toml · no end date · not reachable from our code"

  Scenario: An exception past its date shows as expired
    Given the file "osv-scanner.toml" says:
      """
      [[IgnoredVulns]]
      id = "GHSA-q8mj-m7cp-5q26"
      ignoreUntil = 2020-01-31
      reason = "old"
      """
    When I run "gdd status"
    Then the output contains "GHSA-q8mj-m7cp-5q26 · osv-scanner.toml · expired 2020-01-31 · old"

  Scenario: A package override that ignores vulnerabilities is an exception
    Given the file "osv-scanner.toml" says:
      """
      [[PackageOverrides]]
      name = "qs"
      ecosystem = "npm"
      vulnerability.ignore = true
      reason = "pinned by StrykerJS"
      """
    When I run "gdd status"
    Then the output contains "package qs (npm) · osv-scanner.toml · no end date · pinned by StrykerJS"

  Scenario: An override that only touches licenses is not a security exception
    Given the file "osv-scanner.toml" says:
      """
      [[PackageOverrides]]
      group = "dev"
      license.ignore = true
      reason = "dev tools are not shipped"
      """
    When I run "gdd status"
    Then the output contains "Security exceptions: none"

  Scenario: G3 passes when every exception has a reason
    Given the file "osv-scanner.toml" says:
      """
      [[IgnoredVulns]]
      id = "GHSA-4mjr-xmp4-gh2g"
      reason = "dev only"
      """
    When I run "gdd g3 c1"
    Then the gate passes
    And the gate passed "exceptions"

  Scenario: G3 refuses an exception without a reason
    Given the file "osv-scanner.toml" says:
      """
      [[IgnoredVulns]]
      id = "GHSA-4mjr-xmp4-gh2g"
      """
    When I run "gdd g3 c1"
    Then the gate fails on "exceptions"
    And the failure of "exceptions" shows "osv-scanner.toml: GHSA-4mjr-xmp4-gh2g has no reason"

  Scenario: An advisory ignored by id alone has no reason
    Given the file "deny.toml" says:
      """
      [advisories]
      ignore = ["RUSTSEC-2026-0002"]
      """
    When I run "gdd g3 c1"
    Then the failure of "exceptions" shows "deny.toml: RUSTSEC-2026-0002 has no reason"

  Scenario: An exceptions file that cannot be read stops G3
    Given the file "osv-scanner.toml" says "[[IgnoredVulns]"
    When I run "gdd g3 c1"
    Then the failure of "exceptions" shows "osv-scanner.toml: invalid TOML"

  Scenario: G3 names the exceptions the change adds
    When the change writes "osv-scanner.toml" saying:
      """
      [[IgnoredVulns]]
      id = "GHSA-x5fp-wj9c-mxmx"
      ignoreUntil = 2099-06-30
      reason = "dev only, reviewed"
      """
    And I run "gdd g3 c1"
    Then the output contains "exception added: GHSA-x5fp-wj9c-mxmx · osv-scanner.toml · until 2099-06-30 · dev only, reviewed"

  Scenario: Exceptions already on main are not named as added
    Given the file "osv-scanner.toml" says:
      """
      [[IgnoredVulns]]
      id = "GHSA-4mjr-xmp4-gh2g"
      reason = "dev only"
      """
    And everything is committed on main
    When I run "gdd g3 c1"
    Then the output does not contain "exception added"
