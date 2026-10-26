# Discover · 2026-10-26 · Done is not good

Participants: the human (stakeholder) and the Orchestrator. Source: the video «El código generado por IA funciona. Ese es el problema» (BettaTech, 2026-10-25) and the talk it builds on, Katie Dill (Stripe), «How to scale intent, quality, and artistry with AI».

## The idea

AI reaches something that *looks* finished very fast: green tests, good shape. The last 10% (what makes it correct and specific) is left undone because it already «works». GDD already covers the machine half of the problem: hidden examples and mutation testing catch tests that lie and silent off-by-one bugs; the Reviewer of another model family is the adversarial critic; the human writes the intent in the Brief and the scenarios. What was missing is the editor half: an explicit place for the last 10% and a written quality bar.

Evidence checked: Stack Overflow Developer Survey 2025, 66% of developers name «AI solutions that are almost right, but not quite» their biggest frustration (https://survey.stackoverflow.co/2025/ai). DORA's five delivery metrics split into throughput and instability (https://dora.dev/guides/dora-metrics/). DORA 2025: AI is an amplifier of an organization's strengths and weaknesses (https://dora.dev/research/2025/dora-report/).

## Decisions (approved by the human)

1. **Principle: done is not good.** A change that passes every gate can still be unfinished. The human is the editor at G4.
2. **Polish at G4.** G4 has three answers: OK, KO and **polish** (one more Builder round with the human's notes, then the usual gates and the demo again). Polish is not a failure and replaces a KO, it does not add a stop. In Learn, polish notes that repeat become standards or automatic checks. Delivered in `recipe-run`.
3. **Standards file.** `docs/standards.md`, per project: what good looks like here, in words (tone, naming, error messages, what we do not want). The Builder and the Reviewer read it; Learn feeds it. The Brief says what and for whom; the standards say how good.
4. **Tools, only community-backed** (data checked 2026-10-26 with `gh api`):
   - Secrets in G3, every stack: **gitleaks** (30.3k★) on the change's diff; alternative TruffleHog (28.7k★).
   - Complexity in G2: the rule each linter already has, no new tool: ESLint `complexity`, Ruff `C901`, clippy `cognitive_complexity`.
   - Duplication: **jscpd** (6.5k★, many languages), as a warning first.
   - Unused code and dependencies, in Learn: knip (TypeScript, 12.5k★), vulture (Python, 4.9k★), cargo-machete (Rust, 1.4k★).
   - Not chosen: Sonar, Aikido, Snyk (commercial or heavy; GDD runs in the repository with community tools). Semgrep stays a candidate after the MVP.
5. **Metrics.** DORA's five, computed from GDD's own log (`gdd report --dora`): change lead time, deployment frequency, failed deployment recovery time, change fail rate, deployment rework rate (DORA's definition: unplanned deployments caused by a production incident). Plus GDD signals, never targets: polish rate at G4, parks per change, Reviewer findings, surviving mutants before the fix, human stops, tokens per change. Tokens measure cost, not productivity; lines of code are at most an informative sign of bloat (Goodhart's law, as DORA warns).
6. **Spikes with variants.** `gdd spike --variants N`: N Builders (or models) build the same spike on separate branches, and the human picks one side by side. Spike code never enters main as is.

## Where each one lands

- Now, in the harness: gitleaks and the complexity rule in this repository's gates; the standards file; Builder and Reviewer read it; Learn turns polish notes and unused code into rules.
- `recipe-run`: the polish answer at G4.
- After `agent-launch`: `delivery-report` (DORA and GDD signals) and `spike-variants`.
- Stack sheets (block «what the human sees»): gitleaks, the complexity rule, jscpd and the unused-code tool of each stack.
