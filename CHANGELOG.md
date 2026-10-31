# Changelog

All notable changes to gdd are documented in this file. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and gdd follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- The cases of the old kit's end-to-end suite that only had unit tests, end to end <!-- gdd: fc8065e55c7a -->
- One Builder session, by choice: build mode can continue the conversation of test mode <!-- gdd: 340b162f9d33 -->
- Spikes with variants: one per option, compared on one page, the human picks <!-- gdd: 15d3d1e3a822 -->
- The G4 page says how the change works, in a few plain lines from the Reviewer <!-- gdd: 75baeb91e41f -->
- gdd learn lists the classes of Reviewer findings that repeat across changes <!-- gdd: 688cbdc73213 -->
- Review depth by risk: a quick review for green and yellow changes, a deep one for red <!-- gdd: 9338882f84cf -->
- Known debt becomes a ratchet: a change cannot add to it, and what it pays stays paid <!-- gdd: dcba7c46722f -->
- gdd upgrade: a newer gdd proposes its files on a branch, merged with yours <!-- gdd: fb0924d2649a -->
- gdd learn: the Learn tools and the polish notes <!-- gdd: 4ba13e5bbbdf -->
- gdd report --dora: DORA's five and GDD's signals <!-- gdd: 89aca6b20ee2 -->
- gdd sitting at G4: the demo, the review and OK, KO or polish <!-- gdd: 7f0575e3d156 -->
- gdd sitting: approve a change's scenarios, red zones and plan in the terminal <!-- gdd: d7476af77f47 -->
- The Board shows where the time goes <!-- gdd: b96dd56563cc -->
- gdd report --time: where each change's time goes <!-- gdd: c7cb08925555 -->
- Versions as git tags, made on promotion or by hand with gdd release <!-- gdd: 659155f6b62b -->

### Changed

- Faster mutation testing for Rust, and a mutant that only timed out gets a second chance before the gate fails <!-- gdd: 9f9cd53ed066 -->
- A spike never merges its code: only its ADR reaches main, at the human's OK, within a fixed budget <!-- gdd: 23b29199a172 -->
- The Builder reuses the steps there are and drives each at the lowest layer, and the Reviewer names a repeated step <!-- gdd: da930ab9bf65 -->
- New projects run mutation testing on red changes only, and no rules file can switch off the core for red changes <!-- gdd: 330edcf95ef9 -->
- The Reviewer gives every finding at once, by class, then looks only at what changed <!-- gdd: 84107843bd33 -->
- CHANGELOG.md is the project's: gdd changelog shows what is missing and adds only that <!-- gdd: 3ef51bac9e34 -->
- One command, gdd, for people, agents and CI: new projects get no bin/gdd, and every instruction, permission and message says gdd <!-- gdd: bf0b35c640d6 -->
- The setup skill starts from what gdd init wrote, installs missing lock files and adopts a repository that already has code; the status and learn skills read and run what every project has <!-- gdd: 3187d97a4f82 -->
- The Setup form also installs the stacks, the empty Board and the form point to your agent, a sitting with nothing to approve says so, and gdd alone opens the Board in a terminal <!-- gdd: 722042b659cc -->

### Fixed

- What a change spent lives in its record: an agent cannot lift a spike's budget through the log <!-- gdd: 36cd64d089d1 -->
- Changelogs in the common heading styles <!-- gdd: 855039356ee9 -->
- A new Python project runs its scenarios, and its G3 writes nothing under .gdd <!-- gdd: 768c33f2b099 -->
- An adopted project with known debt passes its self-test, so G0 can open <!-- gdd: 7e2cc0087fba -->

### Security

- Secrets reach no agent under their other common names, nor inside a URL with a password <!-- gdd: 117485d97883 -->
- Agents gdd starts can run only the commands their work needs: gdd refuses every other command, whatever the agent's own permissions allow <!-- gdd: 57fe4b7163bf -->
