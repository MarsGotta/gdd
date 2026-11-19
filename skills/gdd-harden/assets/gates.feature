@zone:<zone>
Feature: <Zone> gates

  # Repeat for each rule, always with both scenarios. For a proposed red zone rule, tag both
  # with the reviewer check id, and add a machine check's tag to each one a test or a scan
  # could observe, even before that test is written.
  Rule: <what must never happen here, in the person's words>

    @check:<check id>
    Scenario: <the bad case, which the check must reject>
      Given <a change that does the wrong thing>
      When the gate runs
      Then <where the breach is visible, with the file and line when a machine check fails>

    @check:<check id>
    Scenario: <a good case, which the check must accept>
      Given <a change that does the right thing>
      When the gate runs
      Then <what shows the rule held and where>
