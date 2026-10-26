# Discover · 2026-10-26 · Changelog and versions from the changes

Participants: the human (stakeholder) and the Orchestrator.

## The need

The human wants gdd to keep a changelog through the publication of changes and versions, using the changes themselves, alone or together with existing tools.

## What the sector does (checked 2026-10-26 with `gh api`)

- Keep a Changelog 1.1.0 (6.7k★): sections Added, Changed, Deprecated, Removed, Fixed, Security, and an Unreleased section on top that becomes a version at release time.
- Changesets (12.5k★): every pull request adds a file with the version bump and a summary for users; release joins them. It is the closest model to a GDD change.
- Generators from git history: git-cliff (12.3k★, Rust, regex commit parsers), release-please (7.7k★), semantic-release (24.1k★, a version on every merge). towncrier (926★) left out.

## Decision (approved by the human)

1. A change is the changeset. Its user-facing line is proposed by the Orchestrator at the sitting, next to the scenarios: no extra stop. Spike, No-code and refactors with no visible effect are left out.
2. The line travels in git, not in `.gdd/` (local): standard trailers of the merge commit, `Changelog: <section> · <line>` and `Breaking: yes|no`. Anyone, CI included, can rebuild the changelog from the repository, and git-cliff can read the trailers.
3. Type to section: Feature → Added (Changed when it modifies existing behaviour), Hotfix → Fixed, Migration → Changed, a security fix → Security, a removal → Removed.
4. `gdd changelog` writes `CHANGELOG.md` in Keep a Changelog format, with Unreleased for what is merged and not yet released.
5. Versions are automatic on promotion by default (Hotfix → patch, Feature → minor, breaking → major; git tag; Unreleased moves to the version), so they are not one more decision. A `manual` mode serves projects that release in batches, such as desktop apps.
6. The version tag is the deployment event of the DORA report.

Where it lands: the block «what the human sees», change `changelog`.
