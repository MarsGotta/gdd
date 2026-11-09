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
5. **Context: almost all per gate, a minimal core across the flow.** The cross-cutting context goes into every call, so it has to stay small, and an agent must not see how the checks of its own gate are made.

   | Layer | What it carries | Where it lives | Who loads it |
   |---|---|---|---|
   | Cross-cutting | A few lines: the project uses GDD, checks are not touched, gates are passed with facts, when something doesn't fit you stop and say so, where the blueprint and the briefs are | The constitution (AGENTS.md or its equivalent), written when preparing | Every agent, always |
   | Per gate | The skill of each role | Given to each agent when it is launched for its gate | Only the agent of that gate |
   | What the previous gate left | The brief, the sealed checks, the evidence | Files in the repository | The next gate, as part of its context |

   An engine does this split on its own. Without one, the orchestrator does it when it launches each agent, and its skill has to teach it how.
6. **Everything in English.**
7. **Seven skills.** One entry skill and six more, each for one role in one moment:

   | Skill | Who loads it | Moment | Reads | Leaves | Level |
   |---|---|---|---|---|---|
   | `gdd` (entry) | The orchestrator | Guides all four | The request, the blueprint and where each brief stands | What comes next, who is launched and with which context | standard |
   | `gdd-prepare` | The orchestrator, with the person, at the start or when the project changes | Prepare | The repository and what it already has (tests, CI) | The blueprint, the cross-cutting core in the constitution and the protections (branches, CODEOWNERS) | deep |
   | `gdd-brief` | The orchestrator, for each change | Brief | The request and the blueprint | The brief: the scenarios the person approves, the computed color and the gates with their context | standard |
   | `gdd-checks` | Another agent, never the one that builds | Brief and Execute | The approved brief | Checks that fail for the right reason, proof that they can pass, and their seal | by color |
   | `gdd-build` | The agent that passes the gate | Execute | The brief and the sealed checks | The change and the facts that prove it | by color |
   | `gdd-review` | The reviewer, with a clean context | Execute | The change, the blueprint and the checks, never the conversation | The verdict, with its evidence | by color, from another model family |
   | `gdd-harden` | The orchestrator, with the person, when something slips through | Harden | What slipped through and its brief | The crack, the new check and the blueprint change, for the person to approve | deep |

   Building is split in two. `gdd-prepare` decides with the person which gates each zone needs and what each one checks, but builds nothing. `gdd-checks` builds both kinds of checks: when preparing (and after hardening), the standing gates of each zone from the blueprint, each with the bad case it must reject; when briefing, the checks of each change. `gdd-review` then tries to cheat the new gates on purpose, and no change starts in a zone until its gates hold.

   Proportion lives in `gdd-brief`: the color comes from what the change touches, and it decides which checks are needed and at which level. `gdd-checks` applies the same proportion: green keeps the existing checks, yellow adds mutation testing, red adds hidden examples written by another hand.
8. **A shared log for metrics, no extra skill.** `gdd` writes one line per gate when each step closes. Every role ends its work by reporting the same fields: level, rounds, result, what it caught, and time and tokens when its environment provides them. `gdd-harden` reads the log to see what slips through, which piece catches what and what each role costs, so a project can compare itself with itself. An engine fills the same log on its own, with exact figures. Its format is part of the contract below.

9. **Preparing, from an idea.** `gdd-prepare` works as a guided conversation, from "I want an online shop for cat products" or from an existing repository.
   - It proposes first and asks afterwards: "for a shop, these are the usual zones; correct what doesn't fit". It asks only what changes a zone or a gate (payments, accounts, regulated products, the stack), and when the person doesn't know, it proposes what is usual for the sector.
   - It may research similar products and the sector's rules, always keeping each source and a copy of what it used. That helps the person decide; the person decides, and it is not legal advice.
   - It leaves three documents in the repository:
     - **the constitution**: the stack, the tools and the commands to build, test and lint (whatever the agent can't deduce from the code, and in a new project, all of it), plus the rules that don't change from one change to the next (prices in cents, card data is never stored). Only its short GDD core and these essentials are cross-cutting;
     - **the blueprint**: risk zones, the gates of each zone and color, their checks, their context and the model levels, in Markdown with a Mermaid diagram, so the person sees it drawn and agents read it as text. An interactive page can be generated from it on request, never the other way round. No PDF;
     - **the backlog**: the functions to build, each with its expected color. The roadmap, when wanted, is the same backlog ordered in phases. Functions change often; risk zones rarely do, so the blueprint stays stable because it holds no functions.
   - The person approves the first blueprint, and the approval stays written: a commit or a pull request of theirs, with the blueprint protected by CODEOWNERS.
   - In a new project, the first item of the backlog is building the gates. Then it advises where to start: low-risk functions first, and money only once its gates have held.
   - A change that touches something no zone covers counts as red until the person classifies it, and `gdd-brief` stops to propose updating the blueprint.
10. **Deterministic first.** Whatever something deterministic can check is checked that way: it costs no tokens and always gives the same answer. When proposing and building gates, climb this ladder and stop at the first step that works:
    1. what the stack already brings: types, a linter rule, the test runner;
    2. a mature open-source tool with a solid community or company behind it;
    3. a small script of the project's own, only when nothing exists: simple, nothing it doesn't need yet, protected like any other check and with its bad case;
    4. a new or less proven tool, only with a notice (how old it is, who maintains it, its license, what it does with your data), and the person decides;
    5. when nothing deterministic works, the reviewer or a person.

    The skills ship no scripts: they recommend that projects use their own.
11. **Reporting progress, briefly.** By default `gdd` tells the person one line per gate (passed or failed, and in how many rounds), the detail only when something fails or the person has to decide, and a summary at the end. Everything else goes to the log.

## Still open

- The format of what the method leaves in a repository (the blueprint, each brief, the evidence and the log). It is the contract between the skills and any engine.
- Which agents beyond Claude Code and Codex, which already read the Agent Skills format.
