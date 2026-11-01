@hidden
Feature: Hidden examples for the smoke scenario

  Scenario: The version line has no trailing space
    Given the tminus tool
    When I ask for its version
    Then the answer starts with "tminus 0"
