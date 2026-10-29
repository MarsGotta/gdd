Feature: A new project starts minimal
  `gdd init` writes a rules file that runs mutation testing on red changes only: the slowest gate
  goes where the risk is, and the file says how to turn it on for yellow changes too.

  Background:
    Given an empty folder

  Scenario: Mutation testing on red changes only
    When I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"
    Then it succeeds
    And the TOML file "gdd/rules.toml" has "color.red.full.mutation" = "true"
    And the TOML file "gdd/rules.toml" has "color.yellow.full.mutation" = "false"
    And the TOML file "gdd/rules.toml" has "color.yellow.fast.mutation" = "false"
    And the file "gdd/rules.toml" contains "mutation = true here runs it on yellow changes too"
