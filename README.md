# Gate-Driven Development

Add gates to your project and automate with AI agents without losing quality.

Your agent says "done." The gate checks if it's true.

You ask your agent for something, it works for a while and tells you it's done. You try it, and it isn't. GDD is the method I use so that the agent's word doesn't open anything: before it starts, you decide what it has to prove, and checks it can't touch decide whether it did.

This repository holds the method:

- [`manifesto/`](manifesto/): the Manifesto, version 0.8, in English and Spanish. If anything else contradicts it, the Manifesto wins.
- [`web/`](web/): the source of [gate-driven.dev](https://gate-driven.dev), where the whole method is explained in both languages.
- [`skills/`](skills/): skills for your coding agent, so you can apply GDD in your own project without an engine. This is version 0.2. On Claude Code they also install as a plugin: `/plugin install gdd --marketplace MarsGotta/gdd`.
- [`evals/`](evals/): scenarios that check, with a real model, how an agent behaves with those skills.

The engine that enforces the method is [T-minus](https://github.com/MarsGotta/t-minus). It has its own repository and it's still in development.

## License

The Manifesto and the website's texts are under [CC BY 4.0](LICENSE-docs.md). The skills, the scripts and the website's code are under the [MIT license](LICENSE).
