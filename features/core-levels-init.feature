Feature: A new project starts minimal
  `tminus init` writes a rules file that runs mutation testing on red changes only: the slowest gate
  goes where the risk is, and the file says how to turn it on for yellow changes too.

  Background:
    Given an empty folder

  Scenario: Mutation testing on red changes only
    When I run "tminus init --name Notes --platform cli --stack rust --agents claude codex"
    Then it succeeds
    And the TOML file "tminus/rules.toml" has "color.red.full.mutation" = "true"
    And the TOML file "tminus/rules.toml" has "color.yellow.full.mutation" = "false"
    And the TOML file "tminus/rules.toml" has "color.yellow.fast.mutation" = "false"
    And the file "tminus/rules.toml" contains "mutation = true here runs it on yellow changes too"
