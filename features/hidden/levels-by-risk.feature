@hidden
Feature: Levels by risk (hidden examples)

  Background:
    Given a project that follows the method, with the sensitive area "src/storage/**"

  Scenario: The project's locks are in the most sensitive area even where the zone map says nothing of them
    When a change that plans "tminus/commands.toml" is opened
    Then the change counts as sensitive
    And the change waits for the guard
    And the change waits for the human's approval
