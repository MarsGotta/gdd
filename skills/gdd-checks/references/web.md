# Gates for the web

Open this when the project builds for the web: a site, a shop, an app that runs in a browser. It has four parts: what usually goes wrong, what to start with, what to add when something slips through, and the traps of setting it up. No tool is named here: look them up for the stack, from each tool's own page, as `research.md` says.

## What usually goes wrong

Candidates for the rules of the product in the constitution, for the person to correct in their own words. Take only those the product has.

- **Money.** A price that changes between the cart and the payment, a total with a fraction of a cent, one order paid twice, a refund larger than the payment, a sale of what isn't in stock. In a shop: nothing sold without stock, stock never below zero, money in whole cents, a paid order never changes.
- **Accounts and personal data.** One customer seeing another's orders, a password stored so it can be read, a session still open after logging out, personal data in a log or an error. What one account can see, another can't.
- **Data loaded from another system.** A record loaded twice or lost halfway, a load that half fails and reports success, a format that changes without warning. A load lands whole, or not at all.
- **The interface.** A page that shows what isn't true (in stock, paid, booked), a form that loses what was typed on an error, what works only with a mouse or at one width. What a page shows is what the data says.

## What to start with

The minimum of `gates.md`, and:

1. unit tests of those rules, written from the scenarios of each change, each round;
2. one test that drives a browser through what the change touches, once before the review and only when the change touches what a person sees: the checks before the review take under ten minutes in all;
3. nothing else until something slips through.

## What to add when something slips through

- A check of accessibility on the pages a change touches, when a page shipped that someone couldn't use.
- Mutation testing on the unit tests of the files a change touches, when a test passed without its behavior.
- The browser tests on more engines, when something worked in one and not in another.
- A scan of the dependencies, when one with a known hole got in.

## The traps of setting it up

Each was met in a real run, with what works:

- **Each runner, its own files.** Unit tests and browser tests in separate folders, and each runner given a pattern that takes its folder and leaves out the other's: in one run, one runner picked up the other's files.
- **Mutation runs the unit tests.** A mutation tool drives a unit runner and doesn't find browser tests: in one run, rounds went into making it. When hardening adds it, it runs the unit tests of the files the change touches.
- **A port and a build folder for each copy.** Builders and reviewers work in copies side by side, so each takes its port and its build folder from an environment variable, never a fixed one: in one run, builds in parallel collided.
- **Nothing reloads during a test.** A development server reloads when a file changes, and a test that runs while one is being written fails at random. Browser tests run against the built product, or a server with reloading off.
- **The test makes what it needs.** A temporary folder, a database, a server: its setup creates them, outside the repository, and removes them. In one run, a folder that had to exist beforehand wasn't there.
- **Options in the runner's own configuration,** not in the environment of the language, each checked against the version the constitution names before it is written down: in one run, an option was refused and every check failed with it.
- **A test waits for what it watches,** a text on the screen or a response, never for a fixed time, and keeps its tool's default timeout: a test that needs more has a cause to find.
