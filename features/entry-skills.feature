Feature: The setup, status and learn skills start from what the project has
  The Setup form (or the flags of gdd init) already wrote the project basics and a Brief
  skeleton, so the gdd-setup skill reads them first and asks the person only what they leave
  out. It installs a stack whose lock file is missing with gdd bootstrap, and in a repository
  that already has code it runs gdd adopt before the self-test, so old lint or test debt does
  not keep G0 closed. The gdd-status skill reads only files every project has: AGENTS.md, not
  GDD.md, which gdd init does not write. The gdd-learn skill runs gdd learn, which lists the
  polish notes and the Reviewer findings that repeat.

  Background:
    Given an empty folder
    And I run "gdd init --name Notes --platform cli --stack rust --agents claude codex"

  Scenario: The setup skill starts from what gdd init wrote
    Then the file ".claude/skills/gdd-setup/SKILL.md" contains "Read `gdd/project.toml` and `docs/brief.md` first"
    And the file ".claude/skills/gdd-setup/SKILL.md" contains "ask only what they leave out"

  Scenario: The setup skill installs a stack without a lock file
    Then the file ".claude/skills/gdd-setup/SKILL.md" contains "run `gdd bootstrap`"

  Scenario: The setup skill adopts a repository that already has code
    Then the file ".claude/skills/gdd-setup/SKILL.md" contains "run `gdd adopt` before `gdd selftest`"

  Scenario: The status skill reads only what every project has
    Then the file ".claude/skills/gdd-status/SKILL.md" contains "read `AGENTS.md`"
    And the file ".claude/skills/gdd-status/SKILL.md" does not contain "GDD.md"

  Scenario: The learn skill runs gdd learn
    Then the file ".claude/skills/gdd-learn/SKILL.md" contains "Run `gdd learn`"
