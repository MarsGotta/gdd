# gate-driven.dev

The website of Gate-Driven Development. A static site with no dependencies in the browser. Every command below runs from this `web/` folder.

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

The Documentation section is a short book of 17 chapters in five parts (Understand, Apply, Guarantee, T-minus, Look up). The T-minus part (The engine, The kits) is the only place where `kit` and the engine's internal words may appear. Each chapter is one file, `content/doc/NN-key.html` and `content/en/doc/NN-key.html`, and the map is `00-documentacion.html`. Order, routes, titles and descriptions live in `scripts/estructura.mjs`: that's where you add or move a chapter. Links between documents are written `<a href="doc:key">` (or `doc:key#anchor`), with the key of the Spanish route. Each chapter ends with a sentence that leads to the next one; the chapter bar and the Previous and Next buttons come from the order in `estructura.mjs`.

`<figure data-pieza="puertas">` and `<figure data-pieza="viaje">` are replaced at build time by the interactive pieces of the Hands-on chapter (`scripts/tutorial.mjs`, `src/js/tutorial.js`, `src/css/tutorial.css`). Both read one JSON file, `content/tutorial/viaje.json` (the English version is in `content/en/tutorial/`): `puertas` draws the four gates by color, and `viaje` reveals the four changes one by one. The HTML of `viaje` carries all the text, so without the script it reads like a normal chapter.

Old routes (`/guia/`, `/empieza-hoy/`...) redirect in `dist/_redirects`; the list is also in `estructura.mjs`.

## Two languages

Spanish lives at the root and English under `/en/`. Each document has its translation in `content/en/`, with the same structure (`scripts/paridad.mjs` checks it, for example `node scripts/paridad.mjs doc/03-las-piezas`) and the vocabulary of `content/en/GLOSARIO.md`. Words retired from the method are watched by `scripts/comprobar.mjs`, with a short list of exceptions.

`content/en/origen.json` keeps the fingerprint of each Spanish source (regenerate it with `node scripts/huellas.mjs` once the translation is up to date). If you change a document, `scripts/comprobar.mjs` fails until you update its translation and that fingerprint, so the English version can't fall behind without anyone noticing.

If the English manifesto changes, regenerate its PDF (`content/en/manifesto-gdd-vX.Y.pdf`) and the sharing image: `npm run build && node scripts/ingles-pdf-og.cjs && npm run build`. It needs Playwright.

## Layout

- `content/`: the Spanish documents, the diagrams and the PDF. `content/en/`: the same in English.
- `scripts/build.mjs`: builds the pages. `scripts/partes.mjs`: the blueprint and the drawings.
- `src/css`, `src/js`, `src/img`: styles, the interactive pieces (the home blueprint and the Hands-on pieces) and the sharing image.
- `scripts/estructura.mjs`: the list of documents and chapters. `scripts/huellas.mjs`: regenerates `origen.json`.
- `dist/`: what gets published.
