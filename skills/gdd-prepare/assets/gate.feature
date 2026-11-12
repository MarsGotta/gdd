@gate:<gate>
Feature: <Gate>
  <What a change has to demonstrate to get through it, in one sentence.>

  Rule: When it asks for what

    Scenario: <a kind of change>
      Given a <color> change
      And it touches "<pathspec>"
      Then it has to pass "<check id>"

  Rule: It closes

    @check:<check id>
    Scenario: <the bad case, which the check must reject>
      Given <a change that does the wrong thing>
      When the gate runs
      Then the <check id> check fails and names what is wrong
