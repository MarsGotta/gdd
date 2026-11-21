# gate-driven.dev

The website of Gate-Driven Development: a static site built from the design canvas. Every command below runs from this `web/` folder.

## Publish

```
npm run deploy
```

It builds the site and publishes `dist/` to `gate-driven.dev` (see `wrangler.jsonc`). The first time, Wrangler opens the browser so you can sign in to Cloudflare.

## The pages are the canvas

Each page is a page of the design canvas, kept in the canvas's own format:

- `lienzo/<Name>.dc.html` in Spanish;
- `lienzo/en/<Name>.dc.html` in English.

Each file has its markup inside `<x-dc>`, its fonts and body colour in `<helmet>`, `{{…}}` holes in some attributes, and a `<script type="text/x-dc">` with the page's `Component`. Its `renderVals()` gives what goes in each hole.

To bring a page from the canvas, copy its file here as it is. Its English copy goes in `lienzo/en/`, with the same markup and the text translated. The vocabulary is in `content/en/GLOSARIO.md`.

```
npm install
npm run build
node scripts/comprobar.mjs
```

`scripts/build.mjs`:

- runs each page's script once, to write the page's first state, so it reads without scripts;
- keeps next to each changing attribute its template (`data-dc-*`);
- turns the links between canvas pages (`Cap-las-skills.dc.html`) into the site's addresses, in the same language;
- points the header's language link to the same page in the other language.

In the browser, `src/lienzo.js` runs the same `Component` and fills the templates again on every change of state. It also remembers the reader's choice of light or dark.

The styles are the canvas's own, `lienzo/gdd-theme.css` (the Plano theme on Kimen's tokens: Delineado for light, Nocturno for dark, and the Cianotipo sheet) and `lienzo/gdd-site.css`. `src/sitio.css` adds only the fonts, served from `/assets/` instead of a font service, and the reader's scheme before the page is painted.

Which page of the site each canvas page is, and its address, title and description, come from `scripts/estructura.mjs` (`CLAVE` in `build.mjs` maps the files). Old addresses redirect from the list in `estructura.mjs`.

## Checks

`scripts/comprobar.mjs` runs in CI after the build. It fails if:

- a page is missing in either language;
- a hole or a link to a canvas page reached a page;
- an internal link or anchor leads nowhere;
- the Spanish and English pages differ in structure;
- a word retired from the method is back (the escape room of La idea keeps its locks and keys);
- the Markdown copy of a page leaves out its text.

Every page but the home page also lives as Markdown, at its address plus `index.html.md` (`scripts/markdown.mjs`), and `dist/llms.txt` lists them for agents.

If the English manifesto changes, regenerate its PDF (`content/en/manifesto-gdd-vX.Y.pdf`) and the sharing image: `npm run build && node scripts/ingles-pdf-og.cjs && npm run build`. It needs Playwright.

## Layout

- `lienzo/`: the pages of the canvas and its two stylesheets. `lienzo/en/`: the same pages in English.
- `content/`: the manifesto's PDF in each language, the English glossary, and the full texts of the manifesto and the article.
- `scripts/build.mjs`, `scripts/markdown.mjs`, `scripts/comprobar.mjs`, `scripts/estructura.mjs`, `scripts/versiones.mjs`.
- `src/lienzo.js`, `src/sitio.css`, `src/img/`: the canvas's page logic in the browser, the site's few styles of its own, and the sharing images.
- `dist/`: what gets published.
