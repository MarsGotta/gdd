# Writing gdd's documentation

How the docs are organised and written, so every page reads as part of one book.

## Two layers: the method and the tool

GDD is a method (phases, gates, colors, roles) and `gdd` is the tool that runs it. The docs
keep both visible: [`GDD.md`](../GDD.md) is the full rulebook of the method, and the pages
under `docs/` teach it, show how to use the tool and explain why it works the way it does.
A page about the tool links the method page that owns the idea instead of restating it.

## Four kinds of page

The docs follow [Diátaxis](https://diataxis.fr): each page does one job, and the folder says
which.

| Folder | Kind | The reader... | The page... |
|---|---|---|---|
| `getting-started/` | Tutorial | is learning | takes one guaranteed path, start to finish, with no choices to make |
| `how-to/` | How-to guide | has a goal | starts from the goal, gives numbered steps, mentions alternatives |
| `reference/` | Reference | is looking something up | is complete, terse and consistent; generated from code where possible |
| `explanation/` | Explanation | wants to understand | explains why, the trade-offs and what was rejected |

If a page starts doing a second job, move that part to the page that owns it and link to it.
A tutorial that stops to list every option, or a reference page that tells a story, is the
usual sign.

The project's own GDD files also live in `docs/` and are not part of this book: `brief.md`,
`standards.md`, `adr/` and `discover/` belong to gdd the project, built with GDD. Do not move
or rewrite them; link to them as real examples.

## Shape of a page

1. `# Title`: a noun phrase for reference and explanation ("Configuration", "How risk picks
   the gates"), the task for how-to guides ("Answer the Release Gate").
2. One or two sentences under the title: what the page gives the reader.
3. `## Before you start`, when something must already be in place: link to it.
4. The body.
5. `## Next steps` (tutorials, how-to) or `## See also` (reference, explanation): two to four
   links, each with a few words on why to go there.

## Voice

- English, second person, present tense, active voice: "gdd runs", "you approve".
- American spelling, as the code and its messages use it: "color", not "colour".
- Short sentences. One idea per paragraph. No filler ("simply", "just", "easy", "powerful").
- Write out a term the first time a page uses it, then use the short form: "the Release
  Gate (G4)", then "G4".
- Name things the same way everywhere (see [the glossary](glossary.md)): a *change*, never a
  task or ticket; the *Builder*, the *Reviewer*, the *Orchestrator*, the *human*; *red zone*,
  *sitting*, *Decision Inbox*, *park*, *polish*.
- No em dashes: use a colon, a comma or a new sentence. Few semicolons.
- Wrap prose at about 95 characters, as the rest of the repository does.

## Commands and examples

- Every command is copyable: a fenced block with a language (`bash`, `toml`, `gherkin`,
  `json`, `text`), no `$` prompt, one command per line.
- Show what the reader should see in a separate `text` block introduced by "You should
  see:". Trim long output with `...`. Paste real output only, from one real run; never
  invent or merge output from two runs. When a block could not be run, say so in a comment.
- Placeholders are uppercase: `CHANGE`, `YOUR-REPO`. Example projects are `shop` (a small
  web shop) and `notes` (a command-line notes tool); example changes are `cart-total`,
  `export-csv`, `login-rate-limit`. Paths are relative to the project root; a home folder is
  `/home/me`.
- Never a real host, address, token, private path, person or company. Secrets only by the
  name of their variable.
- Every command, flag, option, default and message on a page has been checked against the
  code (`gdd <command> --help`, the source in `crates/`, the scenarios in `features/`), and
  run in a throwaway project under a temporary folder when possible.
- The scenarios in `features/` are the executable specification of gdd: when a page states a
  behaviour, a scenario should back it. Quote scenario sentences when they say it best.

## Diagrams

Mermaid, rendered by GitHub and by the website. One idea per diagram, at most about twelve
nodes, short labels. A diagram with more than one loop (the phases and gates of `explanation/method.md`) is a
drawn SVG instead, in `docs/img/` with its Spanish twin in `docs/es/img/`: Mermaid tangles
crossing arrows. `scripts/make_phases_svg.py` draws it in both languages. `flowchart TD` (top-down), not `LR`, so long diagrams stay readable on
a phone. Use `sequenceDiagram` for who calls whom and `stateDiagram-v2` for the states of a
change. The colors of risk are always these classes:

```text
classDef green fill:#d9f2e3,stroke:#2e8b57,color:#123
classDef yellow fill:#fff4cc,stroke:#c9a400,color:#321
classDef red fill:#fbdcdc,stroke:#c0392b,color:#311
classDef human fill:#e6e0f8,stroke:#6a4fc0,color:#213
```

Every diagram has a sentence before it that says what to look at, and is never the only
place a fact appears.

## Callouts

GitHub alerts, sparingly, at most two per screen:

```markdown
> [!NOTE]
> Context that is useful but not essential.

> [!WARNING]
> Something that loses work, leaks a secret or opens a gate that should stay shut.
```

## Links

- Relative links between pages (`../reference/cli.md#gdd-launch`).
- Link the first mention of a concept on a page to its explanation or reference.
- `reference/cli.md` is generated by `scripts/make_reference.py` from `gdd <command>
  --help`: change the help text in the code, then run the script. `--check` fails when the
  page is out of date.
- Anchors follow GitHub's rule: lowercase, spaces to `-`, punctuation dropped
  (`## gdd new-change` is `#gdd-new-change`).

## Spanish translation

`docs/es/` is the Spanish translation. It mirrors `docs/` page by page, at the same paths, and
the site serves it under `/es/`. English is the source: change the English page first, then its
Spanish twin in the same pull request. If you cannot write the Spanish, say so in the pull
request and the change is translated before the next release.

- What is not translated: the project's records (`brief.md`, `standards.md`, `adr/`,
  `discover/`), this page, `CONTRIBUTING.md`, and everything gdd itself prints. Code blocks,
  commands, configuration and real output stay byte for byte as in English; Mermaid diagrams
  translate their labels only.
- `docs/es/reference/cli.md` is generated with the English one by `scripts/make_reference.py`.
- Spanish from Spain, neutral, *tuteo* ("apruebas", "lanzas"), the same voice and rules as
  above: short sentences, no em dashes.
- Anchors follow the Spanish headings, accents kept as GitHub keeps them: `## Las vías` is
  `#las-vías`. Headings that are names (files, TOML tables, commands) stay in English.
- `scripts/check_translation.py` checks that every page has its twin, with the same headings
  and the same code; `scripts/check_md_links.py` checks the links of both languages.

Use these words, so every page says the same thing the same way:

| English | Spanish |
|---|---|
| change (a unit of work), Change | Change (masculine: "el Change", "los Changes"); never "cambio" for the unit |
| Feature, Hotfix, Spike (work types) | Feature, Hotfix, Spike (feminine "la Feature", masculine "el Hotfix", "el Spike") |
| Builder, Reviewer, Orchestrator | Builder, Reviewer, Orchestrator (roles, untranslated) |
| test mode / build mode (Builder) | modo test / modo build |
| role | rol |
| gate | puerta |
| Setup Gate (G0) | puerta de preparación (G0, *Setup Gate*) |
| Scenario Gate (G1) | puerta de escenarios (G1, *Scenario Gate*) |
| Loop Gate (G2) | puerta del bucle (G2, *Loop Gate*) |
| Merge Gate (G3) | puerta de integración (G3, *Merge Gate*) |
| Release Gate (G4) | puerta de salida (G4, *Release Gate*) |
| phase | fase (Setup, Discover, Build, Integrate, Release, Learn keep their English names) |
| traffic light | semáforo |
| green / yellow / red | verde / amarillo / rojo (a red Change: "un Change rojo") |
| color | color |
| zone, red zone | zona, zona roja |
| lane, Fast Lane, Full Lane | vía, vía rápida (*Fast Lane*), vía completa (*Full Lane*) |
| sitting | sentada |
| human stop | parada humana |
| Decision Inbox | Decision Inbox (la bandeja de decisiones de cada Change) |
| Board | Board (el tablero) |
| harness | arnés |
| self-test | autotest (`gdd selftest`) |
| scenario, hidden example, step | escenario, ejemplo oculto, paso |
| seal (G1 seals the scenarios) | sellar |
| package (package.md) | paquete |
| plan (plan.md) | plan |
| park, parked, relaunch | aparcar, aparcado, relanzar |
| replan | replanificar |
| known debt, ratchet | deuda conocida, trinquete |
| merge, merge queue, merge commit | fusionar / fusión, cola de fusión, commit de fusión |
| worktree, branch, main | worktree, rama, main |
| pre, production, promote, roll back | pre, producción, promover, revertir (*rollback*) |
| polish (G4 answer) | polish (pulir) |
| OK / KO | OK / KO |
| mutation testing | mutation testing (pruebas de mutación) |
| lint, linter | lint, linter |
| tokens, cache | tokens, caché |
| agent call | llamada de agente |
| stack, stack sheet | stack, ficha de stack |
| Path Guard | Path Guard |
| Brief | Brief |
| scope | alcance (*scope*) |
| spend, budget | gasto, presupuesto |
| tutorial, how-to guide, reference, explanation | tutorial, guía práctica, referencia, explicación |
| "You should see:" | "Deberías ver:" |
| Learn / Do / Look up / Understand / Contribute | Aprender / Hacer / Consultar / Entender / Contribuir |
