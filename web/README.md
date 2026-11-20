# gate-driven.dev

The website of Gate-Driven Development. A static site, dressed with [Kimen](https://kimen.dev/): its tokens (`@kimen/tokens`) under the site's own theme, Plano, and a few of its components (`@kimen/elements`). Every command below runs from this `web/` folder.

## Publish

```
npm run deploy
```

It builds the site and publishes `dist/` to `gate-driven.dev` (see `wrangler.jsonc`). The first time, Wrangler opens the browser so you can sign in to Cloudflare.

## Change the content

The texts are written here, in `content/`, since version 0.8 of the Manifesto. To change them, edit those files and rebuild:

```
npm install
npm run build
node scripts/comprobar.mjs
```

`scripts/comprobar.mjs` confirms that the document text is complete, that internal links lead somewhere and that nothing slipped in that shouldn't be there. If a block the home page uses changes its name, the build stops and says which one.

## The documentation

The Documentation section is a short book of 24 chapters in five parts (Understand, Apply, Guarantee, T-minus, Look up). The T-minus part (The engine, The kits) is the only place where `kit` and the engine's internal words may appear. Each chapter is one file, `content/doc/NN-key.html` and `content/en/doc/NN-key.html`, and the map is `00-documentacion.html`. Order, routes, titles and descriptions live in `scripts/estructura.mjs`: that's where you add or move a chapter. Links between documents are written `<a href="doc:key">` (or `doc:key#anchor`), with the key of the Spanish route. Each chapter ends with a sentence that leads to the next one; the chapter bar and the Previous and Next buttons come from the order in `estructura.mjs`.

Each chapter also says what kind of page it is, in the four kinds Diátaxis names (tutorial, how-to guide, reference, explanation): it's set in `estructura.mjs` and shown in the chapter's label, and the build fails if a chapter has none.

No page writes the version of the skills or the contract by hand. A document writes `{{skills}}` or `{{contrato}}`, and `scripts/versiones.mjs` fills them in from `.claude-plugin/plugin.json` and the blueprint template. `scripts/comprobar.mjs` fails if a page outside What's new cites another version of the skills, if What's new has no entry for the current one, if `skills/README.md` names another, or if a scenario shown as an example has no rule above it, no `@id` or more than one action.

`<figure data-pieza="puertas">` and `<figure data-pieza="viaje">` are replaced at build time by the interactive pieces of the Hands-on chapter (`scripts/tutorial.mjs`, `src/js/tutorial.js`, `src/css/tutorial.css`). Both read one JSON file, `content/tutorial/viaje.json` (the English version is in `content/en/tutorial/`): `puertas` draws the four gates by color, and `viaje` reveals the four changes one by one. The HTML of `viaje` carries all the text, so without the script it reads like a normal chapter.

Old routes (`/guia/`, `/empieza-hoy/`...) redirect in `dist/_redirects`; the list is also in `estructura.mjs`.

Every page of a document also lives as Markdown, at its address plus `index.html.md` (`scripts/markdown.mjs` turns the built page into it), and `dist/llms.txt` lists them for agents, in English, with the Spanish documentation under Optional. The pages link to their copy with `<link rel="alternate" type="text/markdown">`, and `scripts/comprobar.mjs` fails if a copy is missing or a link leads nowhere.

## Two languages

Spanish lives at the root and English under `/en/`. Each document has its translation in `content/en/`, with the same structure (`scripts/paridad.mjs` checks it, for example `node scripts/paridad.mjs doc/03-las-piezas`) and the vocabulary of `content/en/GLOSARIO.md`. Words retired from the method are watched by `scripts/comprobar.mjs`, with a short list of exceptions.

`content/en/origen.json` keeps the fingerprint of each Spanish source (regenerate it with `node scripts/huellas.mjs` once the translation is up to date). If you change a document, `scripts/comprobar.mjs` fails until you update its translation and that fingerprint, so the English version can't fall behind without anyone noticing.

If the English manifesto changes, regenerate its PDF (`content/en/manifesto-gdd-vX.Y.pdf`) and the sharing image: `npm run build && node scripts/ingles-pdf-og.cjs && npm run build`. It needs Playwright.

## Layout

- `content/`: the Spanish documents, the diagrams and the PDF. `content/en/`: the same in English.
- `scripts/build.mjs`: builds the pages, their Markdown copies and `llms.txt`. `scripts/markdown.mjs`: turns a page into Markdown. `scripts/partes.mjs`: the blueprint and the drawings.
- `src/css`, `src/js`, `src/img`: styles, the interactive pieces (the home blueprint and the Hands-on pieces) and the sharing image.
- `src/css/tema-gdd.css`: Plano, the site's theme for Kimen, opted into with `data-ki-theme="gdd"` on every page. Light is ink on drafting film, dark is its blueprint. It reassigns Kimen's theme and semantic tokens only; `src/css/site.css` names its colours, fonts and corners after those tokens. The fonts, Archivo and JetBrains Mono, are served from `/assets/`, never from a font service.
- `src/js/kimen.js`: the Kimen components the pages use, bundled by the build into `/assets/kimen.js`. Today the header (`ki-top-nav`, with `ki-icon-button` for light and dark) and the code blocks: every `pre` of a document becomes a `ki-code-block`, with a copy button where the document marks it `data-copiar`. Register a component there before a page uses its tag. Without scripts, each tag still reads as plain markup.
- `scripts/estructura.mjs`: the list of documents and chapters, with the kind of each one. `scripts/versiones.mjs`: the versions the pages show. `scripts/huellas.mjs`: regenerates `origen.json`.
- `dist/`: what gets published.
