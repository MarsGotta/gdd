Feature: The tool identifies itself

  Scenario: Asking for the version
    Given the tminus tool
    When I ask for its version
    Then the answer starts with "tminus "
