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

## Still open

- The skill set (see the proposal in the conversation of 09-11-2026).
- The format of what the method leaves in a repository (the blueprint, each brief, the evidence). It is the contract between the skills and any engine.
- Which agents beyond Claude Code and Codex, which already read the Agent Skills format.
