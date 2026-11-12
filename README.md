# Gate-Driven Development

Add gates to your project and automate with AI agents without losing quality.

Your agent says "done." The gate checks if it's true.

You ask your agent for something, it works for a while and tells you it's done. You try it, and it isn't. GDD is the method I use so that the agent's word doesn't open anything: before it starts, you decide what it has to prove, and checks it can't touch decide whether it did.

```mermaid
flowchart LR
    accTitle: A change, with a gate in the way
    accDescr: You ask for a change. What it has to prove is decided before anyone builds. Your agent builds it, and the gate runs checks the agent can't touch. If they pass, the change reaches your main branch. If one fails, it goes back to the agent.
    Y(["You ask<br/>for a change"]) --> B["What it has to prove<br/>is decided first"]
    B --> A["Your agent<br/>builds it"]
    A --> G{"The gate"}
    G -- "its checks pass" --> M(["Your main branch"])
    G -- "a check fails" --> A
    %% An empty node keeps the right side free: GitHub draws its zoom buttons there
    M ~~~ Z["zoom buttons"]
    classDef you fill:#0969da,stroke:#0969da,color:#ffffff
    classDef room fill:none,stroke:none,color:transparent
    class Y you
    class Z room
```

## Try it in your project

1. **Install the skills** in your agent, with [a command or two](skills/README.md#1-install-the-skills) on Claude Code, Codex or Antigravity.
2. **Ask it to set GDD up.** You decide together what is delicate in your project and which gates it needs.
3. **Ask for a change,** in your own words, and it goes through its gates before it reaches your main branch.

The [guide to the skills](skills/README.md) takes you through each step, with what you say, what happens and what you end up with.

## What's in this repository

- [`manifesto/`](manifesto/): the Manifesto, version 0.10, in English and Spanish. If anything else contradicts it, the Manifesto wins.
- [`web/`](web/): the source of [gate-driven.dev](https://gate-driven.dev), where the whole method is explained in both languages.
- [`skills/`](skills/): skills for your coding agent, so you can apply GDD in your own project without an engine. This is version 0.8. They install as a plugin on Claude Code, Codex and Antigravity.
- [`evals/`](evals/): scenarios that check, with a real model, how an agent behaves with those skills.

The engine that enforces the method is [T-minus](https://github.com/MarsGotta/t-minus). It has its own repository and it's still in development.

## License

GDD is free to use, by anyone and for anything. The Manifesto and the website's texts are under [CC BY 4.0](LICENSE-docs.md). The skills, the scripts and the website's code are under the [MIT license](LICENSE).

## How to cite

If you write about GDD or build on it, credit it like this:

> Marcela Gotta. *Gate-Driven Development*. 2026. https://gate-driven.dev

GitHub's "Cite this repository" button gives the same in other formats, from [`CITATION.cff`](CITATION.cff).
