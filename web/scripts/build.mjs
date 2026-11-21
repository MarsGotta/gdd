// Builds gate-driven.dev from the design canvas (November 2026).
//
// Each page is a page of the canvas, kept in the canvas's own format: lienzo/<Name>.dc.html in
// Spanish and lienzo/en/<Name>.dc.html in English. Its markup has {{…}} holes in some attributes,
// and a small script, the page's Component, whose renderVals() gives what goes in them. The build
// runs that script once to write the page's first state, so the page reads without scripts, and
// leaves next to each changing attribute its template (data-dc-*), which assets/lienzo.js fills
// again in the browser each time the page's state changes.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import * as cheerio from 'cheerio';
import { DOCS, CAPITULOS, ORDEN, REDIRECCIONES } from './estructura.mjs';
import { aMarkdown } from './markdown.mjs';
import { SKILLS, CONTRATO } from './versiones.mjs';

const raiz = path.join(path.dirname(fileURLToPath(import.meta.url)), '..');
const dist = path.join(raiz, 'dist');
const leer = (p) => fs.readFileSync(path.join(raiz, p), 'utf8');
const DOMINIO = 'https://gate-driven.dev';
const IDIOMAS = ['es', 'en'];
const otro = (idioma) => (idioma === 'es' ? 'en' : 'es');
const norm = (s) => s.replace(/[  ]/g, ' ').replace(/\s+/g, ' ').trim();
const MANIFIESTO = 'v0.11';
const PDF = { es: `manifiesto-gdd-${MANIFIESTO}.pdf`, en: `manifesto-gdd-${MANIFIESTO}.pdf` };

// --- Which page of the site each page of the canvas is ------------------------------------
const CLAVE = { 'Main.dc.html': 'inicio', 'Docs.dc.html': 'documentacion', 'Capitulo.dc.html': 'la-idea', 'Taller.dc.html': 'taller', 'Manifiesto.dc.html': 'manifiesto', 'Articulo.dc.html': 'articulo' };
for (const k of CAPITULOS) if (k !== 'la-idea') CLAVE[`Cap-${k}.dc.html`] = k;
const INICIO = {
  es: { ruta: '/', titulo: 'Gate-Driven Development: puertas para agentes de IA', descripcion: 'Gate-Driven Development (GDD) es una forma de crear productos de calidad con agentes de IA sin supervisarlos paso a paso.' },
  en: { ruta: '/en/', titulo: 'Gate-Driven Development: quality gates for AI agents', descripcion: 'Gate-Driven Development (GDD) is a way to build quality products with AI agents without supervising them step by step.' },
};
const datos = (clave, idioma) => (clave === 'inicio' ? INICIO[idioma] : DOCS[clave][idioma]);
const ruta = (clave, idioma) => datos(clave, idioma).ruta;
for (const f of fs.readdirSync(path.join(raiz, 'lienzo')).filter((f) => f.endsWith('.dc.html'))) {
  if (!CLAVE[f]) throw new Error(`lienzo/${f} no es ninguna página de la web: añádela a CLAVE o a estructura.mjs`);
}

// --- Reading a page of the canvas ---------------------------------------------------------
const leerPagina = (fichero, idioma) => {
  const s = leer(path.join('lienzo', idioma === 'en' ? 'en' : '', fichero));
  const cabeza = s.split('<body>')[0];
  const casco = s.match(/<helmet>([\s\S]*?)<\/helmet>/)?.[1] ?? '';
  const guion = s.match(/<script type="text\/x-dc"[^>]*data-props='([^']*)'[^>]*>([\s\S]*?)<\/script>/);
  if (!guion) throw new Error(`lienzo/${idioma}/${fichero}: falta su <script type="text/x-dc">`);
  return {
    // Its own stylesheets (gdd-theme.css, gdd-site.css), served from /assets; webfonts come from the site.
    hojas: [...cabeza.matchAll(/<link rel="stylesheet" href="([\w.-]+\.css)">/g)].map((m) => m[1]),
    estilo: [...casco.matchAll(/<style>([\s\S]*?)<\/style>/g)].map((m) => m[1].trim()).join('\n'),
    marcado: s.match(/<x-dc>([\s\S]*)<\/x-dc>/)[1].replace(/<helmet>[\s\S]*?<\/helmet>/, '').trim(),
    codigo: guion[2].trim(),
    props: Object.fromEntries(Object.entries(JSON.parse(guion[1])).filter(([k]) => !k.startsWith('$')).map(([k, v]) => [k, v.default])),
  };
};

// The page's first state: its own script, run once with its default properties.
const valores = (codigo, props) => {
  const caja = vm.createContext({});
  vm.runInContext(`class DCLogic { constructor(props) { this.props = props; this.state = null; } setState() {} }\n${codigo}\nthis.Component = Component;`, caja);
  return new caja.Component(props).renderVals();
};
const rellenar = (plantilla, v) => plantilla.replace(/\{\{(\w+)\}\}/g, (_, k) => (v[k] === undefined || typeof v[k] === 'function' ? '' : String(v[k])));

// Each attribute with a hole gets its first value and keeps its template beside it. A handler
// (onClick) keeps only the name of its function. The colour scheme is left to the reader: until
// the script runs, the system's, or the one the reader chose (read in <head>).
const marcar = (marcado, v) => marcado.replace(/ ([\w:-]+)="([^"]*\{\{[^"]*)"/g, (_, attr, plantilla) => {
  const nombre = attr.toLowerCase();
  if (nombre.startsWith('on')) return ` data-dc-${nombre}="${plantilla.replace(/[{}\s]/g, '')}"`;
  if (nombre === 'data-ki-color-scheme') return ` data-dc-${nombre}="${plantilla}"`;
  return ` ${attr}="${rellenar(plantilla, v)}" data-dc-${nombre}="${plantilla}"`;
});

// Links between pages of the canvas lead to their address on the site, in the same language; the
// language link of the header, to this same page in the other one.
const enlazar = (marcado, clave, idioma) => marcado
  .replace(/href="([\w-]+\.dc\.html)(#[^"]*)?"/g, (_, f, ancla = '') => {
    if (!CLAVE[f]) throw new Error(`Enlace a ${f}, que no es ninguna página de la web`);
    return `href="${ruta(CLAVE[f], idioma)}${ancla}"`;
  })
  .replace(/href="https:\/\/gate-driven\.dev\/[^"]*"( lang="(?:es|en)" hreflang="(?:es|en)")/g, (_, resto) => `href="${ruta(clave, otro(idioma))}"${resto}`);

// --- A page of the site -------------------------------------------------------------------
const VERSION = createHash('sha256').update(['lienzo/gdd-theme.css', 'lienzo/gdd-site.css', 'src/sitio.css', 'src/lienzo.js'].map(leer).join('')).digest('hex').slice(0, 10);

const pagina = (clave, idioma, fichero) => {
  const p = leerPagina(fichero, idioma);
  const cuerpo = enlazar(marcar(p.marcado, valores(p.codigo, p.props)), clave, idioma);
  if (cuerpo.replace(/ data-dc-[\w-]+="[^"]*"/g, '').includes('{{')) throw new Error(`lienzo/${idioma}/${fichero}: queda un hueco {{…}} fuera de un atributo`);
  if (cuerpo.includes('.dc.html')) throw new Error(`lienzo/${idioma}/${fichero}: queda un enlace a una página del lienzo`);
  const { titulo, descripcion } = datos(clave, idioma);
  const aqui = ruta(clave, idioma);
  const de = { [idioma]: aqui, [otro(idioma)]: ruta(clave, otro(idioma)) };
  const md = clave !== 'inicio' ? `\n<link rel="alternate" type="text/markdown" href="${DOMINIO}${aqui}index.html.md">` : '';
  return `<!doctype html>
<html lang="${idioma}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<script>try{var t=localStorage.getItem('tema');if(t==='oscuro'||t==='claro')document.documentElement.dataset.tema=t}catch(e){}</script>
<title>${titulo}</title>
<meta name="description" content="${descripcion}">
<meta name="author" content="Marcela Gotta">
<link rel="canonical" href="${DOMINIO}${aqui}">
<link rel="alternate" hreflang="es" href="${DOMINIO}${de.es}">
<link rel="alternate" hreflang="en" href="${DOMINIO}${de.en}">
<link rel="alternate" hreflang="x-default" href="${DOMINIO}${de.en}">${md}
<meta property="og:type" content="${clave === 'inicio' ? 'website' : 'article'}">
<meta property="og:locale" content="${idioma === 'es' ? 'es_ES' : 'en_US'}">
<meta property="og:title" content="${titulo}">
<meta property="og:description" content="${descripcion}">
<meta property="og:url" content="${DOMINIO}${aqui}">
<meta property="og:image" content="${DOMINIO}/${idioma === 'es' ? 'og.png' : 'og-en.png'}">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta name="twitter:card" content="summary_large_image">
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<link rel="preload" href="/assets/archivo.woff2" as="font" type="font/woff2" crossorigin>
<link rel="stylesheet" href="/assets/sitio.css?v=${VERSION}">
${p.hojas.map((h) => `<link rel="stylesheet" href="/assets/${h}?v=${VERSION}">`).join('\n')}
${p.estilo ? `<style>\n${p.estilo}\n</style>` : ''}
</head>
<body>
${cuerpo}
<script>window.DC=function(DCLogic){${p.codigo}
return Component};window.DC_PROPS=${JSON.stringify(p.props)};</script>
<script src="/assets/lienzo.js?v=${VERSION}" defer></script>
</body>
</html>
`;
};

// A missing page: the documentation's header from the canvas, and a way back.
const perdida = (idioma) => {
  const p = leerPagina('Docs.dc.html', idioma);
  const cabecera = enlazar(p.marcado.match(/<header class="d-head">[\s\S]*?<\/header>/)[0], 'documentacion', idioma).replace(' aria-current="page"', '').replace(/ onClick="\{\{\w+\}\}"/, '');
  const t = (es, en) => (idioma === 'es' ? es : en);
  return `<!doctype html>
<html lang="${idioma}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>${t('Esta página no existe', 'This page does not exist')}</title>
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<link rel="stylesheet" href="/assets/sitio.css?v=${VERSION}">
${p.hojas.map((h) => `<link rel="stylesheet" href="/assets/${h}?v=${VERSION}">`).join('\n')}
<style>\n${p.estilo}\n</style>
</head>
<body>
<div class="g d" data-ki-theme="gdd">
${cabecera}
<main class="g-wrap" style="padding-block: clamp(36px, 4.4vw, 64px) clamp(56px, 6vw, 88px)">
  <div class="d-title" style="max-width: 46em">
    <h1 class="g-h1">${t('Esta puerta no lleva a ningún sitio', 'This gate does not lead anywhere')}</h1>
    <p class="g-lead">${t('La página que buscas no existe o ha cambiado de sitio.', 'The page you are looking for does not exist or has moved.')} <a class="g-src" href="${ruta('inicio', idioma)}">${t('Vuelve al inicio', 'Go back to the home page')}</a>.</p>
  </div>
</main>
</div>
</body>
</html>
`;
};

// --- Writing dist/ ------------------------------------------------------------------------
fs.rmSync(dist, { recursive: true, force: true });
fs.mkdirSync(path.join(dist, 'assets'), { recursive: true });
const PAGINAS = {};
for (const idioma of IDIOMAS) {
  for (const [fichero, clave] of Object.entries(CLAVE)) {
    if (!fs.existsSync(path.join(raiz, 'lienzo', idioma === 'en' ? 'en' : '', fichero))) {
      if (idioma === 'en') { console.warn(`AVISO: falta lienzo/en/${fichero}; ${ruta(clave, 'en')} no se publica`); continue; }
      throw new Error(`Falta lienzo/${fichero}`);
    }
    PAGINAS[ruta(clave, idioma)] = { clave, idioma, html: pagina(clave, idioma, fichero) };
  }
}
for (const [r, { html }] of Object.entries(PAGINAS)) {
  fs.mkdirSync(path.join(dist, r), { recursive: true });
  fs.writeFileSync(path.join(dist, r, 'index.html'), html);
}
fs.writeFileSync(path.join(dist, '404.html'), perdida('es'));
fs.mkdirSync(path.join(dist, 'en'), { recursive: true });
if (fs.existsSync(path.join(raiz, 'lienzo/en/Docs.dc.html'))) fs.writeFileSync(path.join(dist, 'en/404.html'), perdida('en'));

// --- The same pages as Markdown, and llms.txt, their index for agents ----------------------
const aMd = (r) => `${DOMINIO}${r}index.html.md`;
const CON_MD = new Set(Object.keys(PAGINAS).filter((r) => PAGINAS[r].clave !== 'inicio'));
for (const [r, { clave, idioma, html }] of Object.entries(PAGINAS)) {
  if (clave === 'inicio') continue;
  const $ = cheerio.load(html);
  const principal = $('main').first().clone();
  const titulo = norm(principal.find('h1').first().text());
  principal.find('h1').first().remove();
  principal.find('[aria-hidden="true"], svg').remove();
  // Links lead to the Markdown copy of a page when there is one, and always to a full address.
  const enlace = (href) => {
    if (href.startsWith('#')) return `${aMd(r)}${href}`;
    if (!href.startsWith('/')) return href;
    const [camino, ancla] = href.split('#');
    return CON_MD.has(camino) ? `${aMd(camino)}${ancla ? `#${ancla}` : ''}` : `${DOMINIO}${href}`;
  };
  const md = `# ${titulo}\n\n> ${datos(clave, idioma).descripcion}\n\n${aMarkdown(principal.html(), enlace)}\n`;
  fs.writeFileSync(path.join(dist, r, 'index.html.md'), md);
}
const linea = (k) => `- [${DOCS[k].en.menu}](${aMd(DOCS[k].en.ruta)}): ${DOCS[k].en.descripcion}`;
fs.writeFileSync(path.join(dist, 'llms.txt'), `# Gate-Driven Development (GDD)

> ${INICIO.en.descripcion}

GDD comes with eight skills for Claude Code, Codex and Antigravity, which bring a small command of their own, the layer; it can also be applied by hand. Skills and layer ${SKILLS}, contract ${CONTRATO}, Manifesto ${MANIFIESTO}, draft for review. When the documentation and the Manifesto disagree, the Manifesto is the reference.

## The reference

${linea('manifiesto')}
${linea('documentacion')}

## The documentation

${CAPITULOS.map(linea).join('\n')}

## Practice

${linea('taller')}

## Optional

${linea('articulo')}
- [Gate-Driven Development en español](${aMd(DOCS.documentacion.es.ruta)}): the same documentation in Spanish, the original; each page links to the rest.
`);

// --- Assets -------------------------------------------------------------------------------
fs.copyFileSync(path.join(raiz, 'lienzo/gdd-theme.css'), path.join(dist, 'assets/gdd-theme.css'));
fs.copyFileSync(path.join(raiz, 'lienzo/gdd-site.css'), path.join(dist, 'assets/gdd-site.css'));
fs.copyFileSync(path.join(raiz, 'src/sitio.css'), path.join(dist, 'assets/sitio.css'));
fs.copyFileSync(path.join(raiz, 'src/lienzo.js'), path.join(dist, 'assets/lienzo.js'));
const fuente = (paquete, fichero, salida) => fs.copyFileSync(path.join(raiz, 'node_modules', paquete, 'files', fichero), path.join(dist, 'assets', salida));
fuente('@fontsource-variable/archivo', 'archivo-latin-standard-normal.woff2', 'archivo.woff2');
fuente('@fontsource-variable/archivo', 'archivo-latin-standard-italic.woff2', 'archivo-italic.woff2');
fuente('@fontsource-variable/jetbrains-mono', 'jetbrains-mono-latin-wght-normal.woff2', 'jetbrains-mono.woff2');
// The PDF is the manifesto itself, at the version it carries, in each language.
for (const idioma of IDIOMAS) {
  const origen = path.join(raiz, idioma === 'en' ? 'content/en' : 'content', PDF[idioma]);
  if (fs.existsSync(origen)) fs.copyFileSync(origen, path.join(dist, PDF[idioma]));
}
for (const imagen of ['og.png', 'og-en.png']) {
  const og = path.join(raiz, 'src/img', imagen);
  if (fs.existsSync(og)) fs.copyFileSync(og, path.join(dist, imagen));
}
fs.writeFileSync(path.join(dist, 'favicon.svg'), `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><style>svg{color:#10192a}@media (prefers-color-scheme:dark){svg{color:#f3f7fc}}</style><path d="M1 21.500h5M18.500 21.500H23" stroke="currentColor" stroke-width="3" fill="none"/><path d="M6 21.500V4" stroke="currentColor" stroke-width="2.200" fill="none"/><path d="M6 4a15 17.500 0 0 1 12.500 17.500" stroke="currentColor" stroke-width="1.400" fill="none"/></svg>\n`);
fs.writeFileSync(path.join(dist, 'robots.txt'), `User-agent: *\nAllow: /\n\nSitemap: ${DOMINIO}/sitemap.xml\n`);
const FECHA = new Date().toISOString().slice(0, 10);
const parejas = [['/', '/en/'], ...ORDEN.map((k) => [DOCS[k].es.ruta, DOCS[k].en.ruta])];
const entrada = (loc, es, en) => `  <url><loc>${DOMINIO}${loc}</loc><lastmod>${FECHA}</lastmod><xhtml:link rel="alternate" hreflang="es" href="${DOMINIO}${es}"/><xhtml:link rel="alternate" hreflang="en" href="${DOMINIO}${en}"/><xhtml:link rel="alternate" hreflang="x-default" href="${DOMINIO}${en}"/></url>`;
fs.writeFileSync(path.join(dist, 'sitemap.xml'), `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">\n${parejas.flatMap(([es, en]) => [entrada(es, es, en), entrada(en, es, en)]).join('\n')}\n</urlset>\n`);
fs.writeFileSync(path.join(dist, '_headers'), `/assets/*\n  Cache-Control: public, max-age=31536000, immutable\n\n/*.md\n  X-Robots-Tag: noindex\n\n/*\n  X-Content-Type-Options: nosniff\n  Referrer-Policy: strict-origin-when-cross-origin\n`);
// Old addresses lead to what replaced them, with and without the final slash.
const pdfs = ['0.3', '0.4', '0.5', '0.6', '0.7', '0.8', '0.9', '0.10'].flatMap((v) => [`/manifiesto-gdd-v${v}.pdf /${PDF.es} 301`, `/manifesto-gdd-v${v}.pdf /${PDF.en} 301`]);
fs.writeFileSync(path.join(dist, '_redirects'), `${[...REDIRECCIONES.flatMap(([de, a]) => [`${de}/ ${a} 301`, `${de} ${a} 301`]), ...pdfs].join('\n')}\n`);

console.log('Listo:', Object.keys(PAGINAS).length, 'páginas en dist/');
