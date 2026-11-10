@zone:<zone>
Feature: <Zone> gates

  Rule: <one thing this zone's gates demonstrate, in one sentence>

    @check:<check id>
    Scenario: <the bad case, which the check must reject>
      Given <a change that does the wrong thing>
      When the gate runs
      Then the <check id> check fails and names the file and the line

    @check:<check id>
    Scenario: <a good case, which the check must accept>
      Given <a change that does the right thing>
      When the gate runs
      Then the <check id> check passes
