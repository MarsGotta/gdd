# Designing the GDD skills

These notes record what has been decided about the skills, and why. The Manifesto is the reference: if anything here contradicts it, the Manifesto wins.

## What the skills are for

They let an agent apply GDD in any project with what the project already has (tests, linters, CI, git, branch protection), before any engine exists. The biggest benefit comes from engines, which enforce the method and automate it. The skills bring the method itself, so agents can work with the idea while engines get built.

## Decisions

1. **No scripts.** GDD is an open method, so the skills are instructions only. They use the tools a project already has and install nothing.
2. **The core of any engine.** The skills say what has to happen and who decides. An engine, T-minus or any other, loads them as its core and only adds how it does things: computing the color, sealing checks, launching agents. Never the other way round: if an engine needs the method to change, the change happens here.
3. **One skill per role, plus an entry skill.** Engines launch agents by role, and each agent loads only the skill of its role. An entry skill guides the four moments: Prepare, Brief, Execute and Harden.
4. **No models or providers.** The skills recommend by what a task needs, never by name:

   | Level | What it means | For |
   |---|---|---|
   | light | little reasoning, cheap and fast | mechanical work and green changes |
   | standard | normal reasoning | yellow changes and orchestrating |
   | deep | the most reasoning | red changes and checks that are hard to write |

   Each project says in its blueprint which model is each level. One recommendation does come from the method: the reviewer should be from a different model family than the agent that did the work, whenever possible.
5. **Context: almost all per gate, a minimal core across the flow.** The cross-cutting context goes into every call, so it has to stay small, and the agent that passes a gate never sees its hidden examples or the conversation where its checks were made.

   | Layer | What it carries | Where it lives | Who loads it |
   |---|---|---|---|
   | Cross-cutting | A few lines: the project uses GDD, each role edits only what it owns, a sealed file is never touched, gates are passed with facts, when something doesn't fit you stop and say so, and a map of the files | The GDD block in AGENTS.md, which imports the constitution, written when preparing | Every agent, always |
   | Per gate | The skill of each role, and the context the blueprint gives that gate | Given to each agent when it is launched for its gate | Only the agent of that gate |
   | What the previous gate left | The brief, the sealed checks and the log | Files in the repository | The next gate, as part of its context |

   The constitution holds only what applies everywhere. A rule that belongs to one zone goes into that zone's context in the blueprint, so it reaches only the gates that need it. An engine does this split on its own. Without one, the orchestrator does it when it launches each agent, and its skill has to teach it how.
6. **Everything in English.**
7. **Seven skills.** One entry skill and six more, each for one role in one moment:

   | Skill | Who loads it | Moment | Reads | Leaves | Level |
   |---|---|---|---|---|---|
   | `gdd` (entry) | The orchestrator | Guides all four | The request, the blueprint and where each brief stands | What comes next, who is launched and with which context | standard |
   | `gdd-prepare` | The orchestrator, with the person, at the start or when the project changes | Prepare | The repository and what it already has (tests, CI) | The blueprint with its gates in Gherkin, the constitution, the GDD block in AGENTS.md and the protections (branches, CODEOWNERS) | deep |
   | `gdd-brief` | The orchestrator, for each change | Brief | The request and the blueprint | The brief: the scenarios that say what has to pass, the expected color and the gates with their context | standard |
   | `gdd-checks` | Another agent, never the one that builds | Brief and Execute | The brief, or the gates of a zone | Checks that fail for the right reason, proof that they can pass, and their seal | by color |
   | `gdd-build` | The agent that passes the gate | Execute | The brief and the sealed checks | The change and the facts that prove it | by color |
   | `gdd-review` | The reviewer, with a clean context | Execute | The change, the blueprint and the checks, never the conversation | The verdict and the recomputed color, with the evidence of the checks it ran itself | by color, from another model family |
   | `gdd-harden` | The orchestrator, with the person, when something slips through | Harden | What slipped through and its brief | The crack, the scenario that would have caught it and the blueprint change, for the person to approve | deep |

   Building is split in two. `gdd-prepare` decides with the person which gates each zone needs and writes what each one checks as scenarios (decision 15), but builds nothing. `gdd-checks` builds both kinds of checks: when preparing (and after hardening), the standing gates of each zone from their scenarios, each with the bad case it must reject; when briefing, the checks of each change. `gdd-review` then tries to cheat the new gates on purpose, and no change starts in a zone until its gates hold.

   Proportion lives in `gdd-brief`: the color comes from what the change touches (decision 14), and it decides which checks are needed and at which level. `gdd-checks` applies the same proportion: green keeps the existing checks, yellow adds a test for each scenario, each one broken on purpose before it's sealed, and red adds mutation testing where the stack has a tool for it. Hidden examples written by another hand join red only from `platform` up (decision 23).
8. **A shared log for metrics, no extra skill.** Each brief has its own log, `gdd/log/<id>.jsonl`, where `gdd` appends one event when each step closes. Every role ends its work by reporting the same fields: the skill and its version, level, rounds, result, what it caught, and time and tokens when its environment provides them. `gdd-harden` reads the logs to see what slips through, which piece catches what and what each role costs, so a project can compare itself with itself. An engine fills the same logs on its own, with exact figures. One file per brief keeps branches from colliding when they merge. Its format is part of the contract (decision 13).

9. **Preparing, from an idea.** `gdd-prepare` works as a guided conversation, from "I want an online shop for cat products" or from an existing repository.
   - It proposes first and asks afterwards: "for a shop, these are the usual zones; correct what doesn't fit". It asks only what changes a zone or a gate (payments, accounts, regulated products, the stack), and when the person doesn't know, it proposes what is usual for the sector.
   - In an existing project it starts with a complete analysis: stack, structure, tests and what they cover, CI, linters, dependencies and their health, security findings and documentation, with existing code mapped to risk zones. Complete means it covers everything, not that it reads every file: deterministic tools do the counting (linter totals, coverage, dependency audits) and the agent reads their summaries. If there is nothing yet, it is the greenfield path.
   - It asks, in any project, for external references that enrich the context: designs (Figma), MCP servers, documentation, templates, API specifications, brand guides. Each one is kept as a copy with its source and date, and the blueprint says which gate needs it, so it doesn't become cross-cutting by accident. MCP servers get the least access their gate needs.
   - It may research similar products and the sector's rules, only with proven sources (official documentation, the regulation itself, the vendor's own pages before third-party posts), always cited and kept as copies. That helps the person decide; the person decides, and it is not legal advice.
   - It leaves three documents in the repository:
     - **the constitution**: the stack, the tools and the commands to build, test and lint (whatever the agent can't deduce from the code, and in a new project, all of it), plus the rules that hold everywhere (prices in cents). A rule of a single zone, such as never storing card data, goes to that zone instead. Every agent reads the constitution before working (decision 13);
     - **the blueprint**: risk zones as git pathspecs, the gates of each zone and color, their checks, their context and the model levels, in Markdown with a Mermaid diagram, so the person sees it drawn and agents read it as text. The gates of each zone are written in Gherkin beside it (decision 15). An interactive page can be generated from it on request, never the other way round. No PDF;
     - **the backlog**: the functions to build, each with its expected color. The roadmap, when wanted, is the same backlog ordered in phases. Functions change often; risk zones rarely do, so the blueprint stays stable because it holds no functions.
   - The person approves the first blueprint, and the approval stays written: a commit or a pull request of theirs, with the blueprint protected by CODEOWNERS.
   - In a new project, the first item of the backlog is building the gates. Then it advises where to start: low-risk functions first, and money only once its gates have held.
   - A change that touches something no zone covers counts as red until the person classifies it, and `gdd-brief` stops to propose updating the blueprint.
10. **Known debt never blocks, and never grows.** In an existing project, what already fails a check (lint errors, low coverage, an outdated dependency) is recorded as known debt with its baseline in `gdd/debt.md`, and becomes an item of the backlog with the color of its zone, where `gdd-prepare` may recommend a refactor. Gates don't block that debt, but they block any new debt: a baseline may only go down, and lowering it needs no one's approval. Recording something as debt loosens a check, so the person approves it, as part of approving the blueprint; it never happens on its own. A test that fails today is debt too, visible in the backlog and the log, never silently skipped.
11. **Deterministic first.** Whatever something deterministic can check is checked that way: it costs no tokens and always gives the same answer. When proposing and building gates, climb this ladder and stop at the first step that works:
    1. what the stack already brings: types, a linter rule, the test runner;
    2. a mature open-source tool with a solid community or company behind it;
    3. a small script of the project's own, only when nothing exists: simple, nothing it doesn't need yet, protected like any other check, with its bad case and the practices of decision 16;
    4. a new or less proven tool, only with a notice (how old it is, who maintains it, its license, what it does with your data), and the person decides;
    5. when nothing deterministic works, the reviewer or a person.

    The skills ship no scripts: they recommend that projects use their own.
12. **Reporting progress, briefly.** By default `gdd` tells the person one line per gate (passed or failed, and in how many rounds), the detail only when something fails or the person has to decide, and a summary at the end. Everything else goes to the log.
13. **The contract.** What the method leaves in a repository is the contract between the skills and any engine, T-minus included: an engine reads and writes the same files. Its version (`gdd/1`) goes in the front matter of the blueprint and of each brief, and in every log event.

    | File | What it holds | Who writes it | How it changes |
    |---|---|---|---|
    | `AGENTS.md`, GDD block | The cross-cutting core, in under 100 words, and the map. It imports the constitution with `@gdd/constitution.md` (Claude Code imports it; for other agents it stays a path to open) | `gdd-prepare` | With the person's approval |
    | `gdd/constitution.md` | The stack, the tools, the commands to build, test and lint, and the rules that hold everywhere | `gdd-prepare` | With the person's approval |
    | `gdd/blueprint.md` | Zones as git pathspecs with their color, the gates of each zone and color, the context of each gate and the model levels, in YAML front matter, plus a Mermaid diagram | `gdd-prepare`, `gdd-harden` | Protected by CODEOWNERS; the person approves |
    | `gdd/gates/<zone>.feature` | The standing gates of each zone, in Gherkin | `gdd-prepare`, with the person | Like the blueprint |
    | `gdd/backlog.md` | The functions to build, with their expected color, in phases if wanted | `gdd-prepare`, `gdd-brief`, `gdd-harden` | Freely |
    | `gdd/debt.md` | Known debt, with the baseline of each check | `gdd-prepare` | Down freely; up only with the person's approval |
    | `gdd/briefs/<id>.md` | What has to pass for one change: the scenarios it adds or changes, what it touches, its gates and its expected color. The person approves it when its color asks for `brief-approval`. No status and no seals | `gdd-brief` | Never once its hashes are in the log: a change makes a new version, which goes back to the person only if its color asks for `brief-approval` and what has to pass changes |
    | `features/<capability>.feature` | The scenarios of changes, grouped by capability, each tagged with its brief and with an id that its test cites | `gdd-brief`, with the person | Only through a brief |
    | `gdd/log/<id>.jsonl` | Everything that happens to a brief, one event per line: the approval with the hash of what was approved, each seal, the color computed and why, each gate's result with its evidence (command, exit code and last lines), the verdict and any waiver | `gdd`, from what each role reports | Only appended |
    | `gdd/context/` | Copies of external references and research, each with its source and date | `gdd-prepare` | Refreshed with a new date |

    The seal of a change is a git ref the builder never writes to, and hidden examples exist only where the builder's credentials can't read them (decision 23). A green change has no brief file: it passes the existing checks, and its events go to a log like any other. If the project has a CLAUDE.md, `gdd-prepare` adds `@AGENTS.md` to it, because Claude Code reads AGENTS.md only when there is no CLAUDE.md.
14. **The color: git sets the floor, and judgment can only raise it.** Both parts, always together:
    - **Deterministic.** For each zone, an exact command on the change, such as `git diff --name-only <base> -- ':(glob)src/payments/**'`, and one more that excludes every zone to find what none covers, which counts as red until the person classifies it. It gives the same answer every time, costs no tokens and needs no script.
    - **Judgment.** Then the agent looks for what the paths miss: code that a delicate zone imports, configuration and environment, dependencies and lockfiles, migrations and data, permissions and feature flags, build and CI files. It may raise the color, never lower it, and it writes its reason in the log.

    `gdd-brief` does both when briefing, on what the change will touch; at the end, `gdd-review` does them again, with a clean context, on what it actually touched. If the color rises, the change stops and goes back to Brief: `gdd-checks` adds what the new color asks for, and the person approves the brief again if that color asks for `brief-approval`. Only a blueprint change approved by the person can lower a color, because that is loosening. When `gdd-harden` sees the same reason come back, or a judgment that caught something, it proposes turning it into a pathspec or a zone: what judgment finds today, git checks tomorrow.
15. **Gates are written in Gherkin.** When preparing, `gdd-prepare` writes the standing gates of each zone with the person in `gdd/gates/<zone>.feature`: one `Rule:` for each thing a gate demonstrates, and under it at least the scenario its check must reject (the bad case) and one it must accept, each tagged with the check that covers it.

    ```gherkin
    @zone:payments
    Feature: Payments gates

      Rule: Card data is never stored

        @check:no-card-data
        Scenario: A change that saves a card number is rejected
          Given a change that writes a card number to the orders table
          When the gate runs
          Then the no-card-data check fails and names the file and the line

        @check:no-card-data
        Scenario: A change that saves only the payment token passes
          Given a change that writes the provider's payment token
          When the gate runs
          Then the no-card-data check passes
    ```

    The person approves them with the blueprint, and changing them is changing the blueprint. They are the reference for everyone who touches a check: `gdd-checks` builds each check from its scenarios, at whatever step of the ladder, and proves it fails for the right reason by running it on both cases; `gdd-review` uses them to try to sneak the bad case through and to judge a check that fails; and the agent that passes the gate reads them to understand why a check failed, never the hidden examples. The same language describes what must pass, from the gates of a zone to the scenarios of a change.
16. **Good practices for the project's own scripts.** When the ladder reaches step 3, the script is a check like any other. These rules live in `gdd-checks`, and `gdd-review` judges scripts by them:
    - one job, written from the scenarios of its gate, with the bad case as its first test;
    - deterministic: the same input gives the same output, with no network, clock or randomness;
    - never interactive: everything comes in through arguments, environment variables or stdin, and `--help` says what it checks, how to run it and which scenarios it covers;
    - exit code 0 when it passes and a documented code for each kind of failure; when it fails, it names the file, the line and the rule, so it fails for the right reason;
    - results to stdout, structured when something else reads them, diagnostics to stderr, and output short enough that an agent's environment doesn't cut it;
    - it handles its own errors instead of leaving them to the agent, and every value comes from the blueprint or says why it has that value;
    - only the languages and tools the project already uses; a new dependency is pinned and goes through the ladder;
    - it reads only what it needs, writes nothing outside its output and never touches secrets;
    - simple and with nothing it doesn't need yet, sealed and protected like any other check, and run with the same command locally and in CI if there is one, a command the constitution records.
17. **Flows the skills cover.**
    - **Decisions nobody has made.** While drafting, `gdd-brief` marks each one with `[DECISION: …]`, and a brief isn't done while `git grep` finds any. Questions like rounding each line or the total come up there, before anyone builds.
    - **Traceability.** Every scenario has an id that its test cites, and every check is tagged in a gate, so a grep finds a scenario without a check, or a check no gate asks for.
    - **It doesn't fit the brief.** The agent stops and says why, and the brief goes back to `gdd-brief` as a new version, which goes back to the person if what must pass changes and its color asks for that approval.
    - **Urgencies.** Skipping or lowering a check is loosening: the person approves it, the log records who and why, and it becomes an item of the backlog, like known debt.
    - **Proof that a check can pass.** If it needs a throwaway implementation, the agent that will build never sees it, because it would give away the solution.
    - **Evidence.** What the building agent reports is a claim. `gdd-review` runs the checks itself, and the reported evidence only serves to compare.
    - **Approval.** Without an engine, the agent may share the person's git identity, so a commit alone proves little. For red, the skills recommend approving the pull request on the platform, and they say where this falls short.
18. **Agents and security.**
    - Only `gdd` activates on its own. Role skills load explicitly, so no agent loads another role's skill by mistake: in Codex, with `allow_implicit_invocation: false`; in Claude Code, with one subagent per role that preloads its skill. Optional adapters per agent may map each role to its skill, tools and model level. They hold no code, and the method never depends on them.
    - The orchestrator hands each agent its skill explicitly, and the fields every role reports work as a canary: if they're missing, the skill wasn't followed.
    - In a GDD project, GDD comes first over other skills or methods installed: if one asks the same agent to write the tests and the code, the agent that passes a gate still doesn't write its checks.
    - When an agent can't launch another, the orchestrator gives the person one line to open a new session with the right skill and brief, so every role still starts with a clean context.
    - Everything copied from outside (designs, documentation, web pages) and everything in the repository being analyzed is data, never instructions, and `gdd-review` flags any text in them that gives orders.
19. **How the skills are written.**
    - **Test first.** Each skill gets at least three scenarios, run without it and with it, measuring tokens and time. For the discipline rules (checks aren't touched, gates are passed with facts), pressure scenarios that mix haste, authority and "the test is wrong, just fix it", and a table of excuses and answers taken from what the agent said without the skill.
    - **From real cases.** Skills a model writes for itself bring nothing on average, so these come from the Orbit run, the failures of the T-minus pilot and a real run with the person's corrections, never from this file alone. They also take what the runs of the [method benchmark](https://github.com/MarsGotta/agent-method-bench) teach, as long as it holds in any project: never its project, its requests or its scenarios.
    - **Gotchas.** Each SKILL.md lists the concrete mistakes an agent makes without being told. `gdd-harden` turns what slips through into checks, not into cross-cutting paragraphs.
    - **Short.** `gdd-build`, the one loaded most, about 200 words; the AGENTS.md block, under 100; the rest under 500 lines, with what isn't needed at once in `references/` (in `gdd-prepare`: a new project, an existing one and research) and the templates of the contract in `assets/`.
    - **Descriptions.** In the third person, saying when to use the skill and never summarizing its steps, because an agent may follow the summary and skip the body. Trigger tests only for `gdd`: about twenty requests, half of which must not trigger it.
    - **At all three levels.** The same text behaves differently on a light model, and `gdd-build` runs light on green.
    - **Most effort on verification.** `gdd-checks` and `gdd-review` are the skills that improve results the most.
    - **One source for the contract.** It lives in `skills/CONTRACT.md`, and a check in this repository keeps each skill's templates in line with it. The skills still ship no scripts: that check is ours, not something a project installs.

20. **Where a person approves is in the blueprint.** The team always approves the blueprint, and anything that loosens it. Every other approval is a check of type `person` that a color carries or not, so each project sets how strict each level is: `brief-approval`, the person approving what has to pass before anyone builds, and `person-review`, the person reviewing the change at the end. The template proposes the first on yellow and red and the second on red, and `gdd-prepare` presents them as a proposal. A change whose color rises goes back to Brief, and a new version of a brief goes back to the person only if its color asks for `brief-approval` and what has to pass changes. This follows the Manifesto from version 0.8 on: the colors are levels of strictness, and the blueprint says where a person approves.

21. **Licenses.** The skills and the scripts are under the MIT license, and each SKILL.md says so in its front matter. The Manifesto and the website's texts are under CC BY 4.0. Both are at the root of the repository.
22. **What the log needs so the method can be measured.** The website asks for three notes from the first day: how often a change stops a person, how long it takes to go out and what slips through. The log only had the last one. So an `ask` event marks the moment something starts waiting for a person, a `ship` step closes the log of a change, each finding of a review gets an `id` and is closed later with how it ended, `limits` gains a cap on tokens for the environments that report them, and the body of the blueprint holds the baseline and the figure that would make the team simplify. Nothing else was added: what slips through by color comes from crossing `harden` with the color already in the log.
23. **The seal is a ref, and the blueprint says how far out of reach it is.** The website sealed a change in a branch only the person writes, and the skills sealed it in a commit on the builder's own branch. Now both are the same thing: a git ref, `gdd-seal/<id>`, that the builder starts from and never writes to. The blueprint's `guarantee` says who can write to it: on `rule` nothing stops the builder, on `platform` a ruleset and credentials of its own do, and an engine holds it itself. `gdd-prepare` sets it after a probe that tries, from the builder's environment, to push to the main branch, write to a seal, read the hidden examples and find a credential it shouldn't have. What was pure git in the review became a machine check, `seal`, that every color carries. Hidden examples left the default red: on `rule` the builder could read them, so they exist only from `platform` up. The builder no longer gets the log, which names the commit of what it must not see.

## Where the method lives

Each principle of the Manifesto and each point of its core, with the part of the contract that carries it and the skill that makes it happen. Without an engine these are rules, not guarantees, so the last column says what only the platform or an engine adds, and what is still missing.

| From the Manifesto | In the contract | In the skills | What the platform or an engine adds |
|---|---|---|---|
| 1. Gates come first | `gdd/gates/<zone>.feature`, the brief and its scenarios, with their hashes in the log before anyone builds | `gdd-prepare`, `gdd-brief`, `gdd-checks` | That the order can't be skipped: here the orchestrator keeps it |
| 2. A gate that never closes is not a gate | The `proofs` of each seal: every check once rejecting and once passing | `gdd-checks` (watch it reject, break it on purpose), `gdd-review` (tries to fool a zone's new gates) | Nothing more: the proof is evidence, and the log holds it |
| 3. To each gate, its context | The blueprint's `context`, `gdd/context/`, the GDD block of `AGENTS.md` and the constitution | `gdd` (what each role gets and never gets), `gdd-prepare` | That a role can't read what it wasn't given |
| 4. Gates are passed with facts | `claims` and `gates` in the log, the `gdd-report` block, `limits` | `gdd-build` (command, exit code, last lines), `gdd` (runs the gates itself, and stops) | Limits counted by code, and a cost that many environments don't report |
| 5. Whoever goes through the gate doesn't decide how it's validated | The seal, a git ref (`gdd-seal/<id>`) with its commit and blob hashes in the log, and the `seal` check that every color carries | `gdd-checks` never builds, `gdd-build` never touches a sealed check, `gdd-review` compares with the seal | That the builder can't write or read them: protected branches and its own credentials on the platform, the whole guarantee from an engine |
| 6. More risk, more checks | The blueprint's `zones` and `colors`, and the color in the log, planned and touched | `gdd-brief` (git sets the floor, judgment only raises it), `gdd-review` (recomputes it), `gdd-prepare` (proposes what each color asks for) | That nobody edits the blueprint without the team: CODEOWNERS and branch protection |
| 7. Gates with cracks get hardened | A new `Rule:` in `gdd/gates/`, the `harden` event | `gdd-harden`, then `gdd-checks` and `gdd-review` | The reviewer's exam as something that runs: the contract has no place for its cases or its score yet |
| Core: the agent doesn't touch the blueprint or the checks | CODEOWNERS on the blueprint and the gates, the seal | `gdd-prepare` (the protections), `gdd-review` (nothing that judges was touched) | The guarantee itself: here it's a rule that git helps with |
| Core: the possible damage is bounded | Nothing in the contract: it belongs to the agent's environment | `gdd-prepare` (asks the person to confine the agent), `gdd-review` (tries the way back on red), `gdd` (a person confirms what can't be undone) | The sandbox and the credentials. The person applies them, and the probe of `gdd-prepare` sees whether they hold |
| Core: there are decisions only a person takes | The `approval` and `waiver` events, the checks of type `person` in `colors` | `gdd` (rules that don't bend), `gdd-brief`, `gdd-harden` (loosening) | An approval path no agent can reach: a pull request approved on the platform, or an engine |
| Core: nothing loops, and nothing stays in the dark | `limits`, the `stop` of a step, one log per brief | `gdd` (stopping), the report every role ends with | Limits enforced by code, and what it cost: tokens are reported only when the environment gives them |

The website tells a change in six steps, the ones of its delicate case. They are the steps of `gdd`, told to a person:

| Step, as the website tells it | In `gdd` | Skill |
|---|---|---|
| 1. You decide what has to pass | Brief | `gdd-brief` |
| 2. An agent builds the checks | Checks | `gdd-checks` |
| 3. Another agent gets through the gate | Build | `gdd-build` |
| 4. The gate closes | Gates, then Review | `gdd`, which runs the checks out of the builder's reach, and `gdd-review` |
| 5. You look only where it matters | Ship, when the color's checks include `person-review` | `gdd` |
| 6. If something slips through, you harden | Harden | `gdd-harden` |

## Still open

- Which agents beyond Claude Code and Codex, which already read the Agent Skills format.

## Sources

- Anthropic: [Equipping agents for the real world with Agent Skills](https://www.anthropic.com/engineering/equipping-agents-for-the-real-world-with-agent-skills), [Skill authoring best practices](https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices) and Thariq Shihipar's [lessons on how the Claude Code team uses skills](https://claude.dev/blog/lessons-from-building-claude-code-how-we-use-skills/).
- The Agent Skills standard: [specification](https://agentskills.io/specification), [best practices](https://agentskills.io/skill-creation/best-practices), [optimizing descriptions](https://agentskills.io/skill-creation/optimizing-descriptions), [evaluating skills](https://agentskills.io/skill-creation/evaluating-skills) and [using scripts](https://agentskills.io/skill-creation/using-scripts).
- OpenAI, [Build skills](https://learn.chatgpt.com/docs/build-skills). Jesse Vincent, [writing-skills](https://github.com/obra/superpowers/blob/main/skills/writing-skills/SKILL.md).
- [SkillsBench](https://arxiv.org/abs/2602.12670): curated skills help, self-written ones don't, and focused ones beat exhaustive ones. [Agent Skills in the Wild](https://arxiv.org/abs/2601.10338): 26.1% of 31,132 public skills have a vulnerability, and those with scripts are 2.12 times more likely to.
- [Spec Kit](https://github.com/github/spec-kit) and [BMAD-METHOD](https://github.com/bmad-code-org/BMAD-METHOD), for how they structure their documents: the decision markers, the traceability, the waivers and the owner of each part come from there.
- Claude Code documentation on [CLAUDE.md and AGENTS.md](https://code.claude.com/docs/en/memory) and [subagents](https://code.claude.com/docs/en/sub-agents).
