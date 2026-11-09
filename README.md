# Gate-Driven Development

Add gates to your project and automate with AI agents without losing quality.

Your agent says "done." The gate checks if it's true.

You ask your agent for something, it works for a while and tells you it's done. You try it, and it isn't. GDD is the method I use so that the agent's word doesn't open anything: before it starts, you decide what it has to prove, and checks it can't touch decide whether it did.

This repository holds the method:

- [`manifesto/`](manifesto/): the Manifesto, version 0.7, in English and Spanish. If anything else contradicts it, the Manifesto wins.
- [`web/`](web/): the source of [gate-driven.dev](https://gate-driven.dev), where the whole method is explained in both languages.
- [`skills/`](skills/): skills for your coding agent, so you can apply GDD in your own project without an engine. I'm designing them now.

The engine that enforces the method is [T-minus](https://github.com/MarsGotta/tminus). It has its own repository and it's still in development.
