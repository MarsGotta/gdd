// Checks dist/ after the build, the site made from the design canvas. It stops CI when a page is
// missing, a hole of the canvas or a link to one of its pages reached a page, a link leads
// nowhere, the Spanish and English pages differ in structure, a retired word is back, or the
// Markdown copy for agents leaves out text of its page.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import * as cheerio from 'cheerio';
import { DOCS, ORDEN } from './estructura.mjs';

const raiz = path.join(path.dirname(fileURLToPath(import.meta.url)), '..');
const dist = path.join(raiz, 'dist');
const norm = (s) => s.replace(/[  ]/g, ' ').replace(/\s+/g, ' ').trim();
const fallos = [];

// --- Every page, in both languages ---------------------------------------------------------
const RUTAS = { es: ['/', ...ORDEN.map((k) => DOCS[k].es.ruta)], en: ['/en/', ...ORDEN.map((k) => DOCS[k].en.ruta)] };
const archivo = (ruta) => path.join(dist, ruta.endsWith('/') ? `${ruta}index.html` : ruta);
const PAGINAS = {};
for (const [idioma, rutas] of Object.entries(RUTAS)) {
  for (const r of rutas) {
    if (!fs.existsSync(archivo(r))) { fallos.push(`Falta la página ${r}`); continue; }
    PAGINAS[r] = { idioma, $: cheerio.load(fs.readFileSync(archivo(r), 'utf8')) };
  }
}
console.log(`Páginas: ${Object.keys(PAGINAS).length} de ${RUTAS.es.length + RUTAS.en.length}.`);

// --- Nothing of the canvas's own format reaches a page ------------------------------------
for (const [r, { $ }] of Object.entries(PAGINAS)) {
  const html = $.html().replace(/ data-dc-[\w-]+="[^"]*"/g, '');
  if (html.includes('{{')) fallos.push(`${r}: queda un hueco {{…}} del lienzo`);
  if (html.includes('.dc.html')) fallos.push(`${r}: queda un enlace a una página del lienzo`);
}

// --- Links ----------------------------------------------------------------------------------
let internos = 0;
const externos = new Set();
for (const [r, { $ }] of Object.entries(PAGINAS)) {
  $('a[href]').each((_, a) => {
    const href = $(a).attr('href');
    if (/^https?:/.test(href)) { externos.add(href); return; }
    if (href.startsWith('mailto:')) return;
    internos += 1;
    const [camino, ancla] = href.split('#');
    const destino = camino || r;
    if (!fs.existsSync(archivo(destino))) { fallos.push(`${r} enlaza a ${href}, que no existe`); return; }
    if (ancla) {
      const otra = PAGINAS[destino]?.$ ?? cheerio.load(fs.readFileSync(archivo(destino), 'utf8'));
      if (!otra(`[id="${ancla}"]`).length) fallos.push(`${r} enlaza a ${href}, y esa página no tiene #${ancla}`);
    }
  });
}
console.log(`Enlaces internos: ${internos}. Enlaces externos distintos: ${externos.size}.`);

// --- Spanish and English: the same page, word for word apart ---------------------------------
// The same tags in the same order, with the same attributes, but for those that carry words and
// the addresses of the site's own pages, which are different in each language.
const PALABRAS = new Set(['aria-label', 'title', 'alt', 'lang', 'hreflang', 'placeholder', 'content']);
const esqueleto = ($) => $('body *').toArray().map((n) => {
  const attrs = Object.entries(n.attribs).filter(([k]) => !PALABRAS.has(k)).map(([k, v]) => (k === 'href' && v.startsWith('/') ? [k, ''] : [k, v]));
  return `${n.name} ${JSON.stringify(attrs.sort())}`;
});
let iguales = 0;
for (const k of ['inicio', ...ORDEN]) {
  const es = PAGINAS[k === 'inicio' ? '/' : DOCS[k].es.ruta];
  const en = PAGINAS[k === 'inicio' ? '/en/' : DOCS[k].en.ruta];
  if (!es || !en) continue;
  const a = esqueleto(es.$);
  const b = esqueleto(en.$);
  const i = a.findIndex((x, j) => x !== b[j]);
  if (i >= 0 || a.length !== b.length) {
    const j = i >= 0 ? i : Math.min(a.length, b.length);
    fallos.push(`${k}: el español y el inglés cambian en el elemento ${j}\n    es: ${a[j] ?? '(nada)'}\n    en: ${b[j] ?? '(nada)'}`);
  } else iguales += 1;
}
console.log(`Traducción: ${iguales} páginas con la misma estructura en los dos idiomas.`);

// --- Retired words ----------------------------------------------------------------------------
// The escape room of La idea keeps its locks and keys: a block that names them also says what they
// stand for, a gate or the escape room.
const RETIRADAS = {
  es: [/\bcerraduras?\b/i, /\bllaves?\b/i, /\bnormas\b/i, /reglas del juego/i, /\bfugas?\b/i, /\bconstructor(es)?\b/i, /\brefuerzos?\b/i],
  en: [/\blocks?\b/i, /\bkeys?\b/i, /\bleaks?\b/i, /\breinforcements?\b/i, /rules of the game/i],
};
const ESCAPE = /escape room|\bpuerta|\bgate|\bdoor/i;
const BLOQUES = 'h1, h2, h3, p, li, dt, dd, th, td, figcaption, pre, button, span.g-cap';
for (const [r, { idioma, $ }] of Object.entries(PAGINAS)) {
  const idea = r === DOCS['la-idea'][idioma].ruta;
  $(BLOQUES).each((_, n) => {
    if ($(n).find(BLOQUES).length) return;
    const t = norm($(n).text());
    for (const re of RETIRADAS[idioma]) {
      if (re.test(t) && !(idea && ESCAPE.test(t) && /cerradura|llave|lock|key/i.test(t))) fallos.push(`${r}: palabra retirada (${re.source}) en «${t.slice(0, 80)}»`);
    }
  });
}

// --- The Markdown copies, for agents ----------------------------------------------------------
const textos = (n) => (n.type === 'text' ? [n.data] : /\bwhen-(on|off)\b/.test(n.attribs?.class ?? '') ? [] : (n.children ?? []).flatMap(textos));
const llano = (s) => s.normalize('NFC').toLowerCase().replace(/\]\([^)]*\)/g, ']').replace(/[^\p{L}\p{N}]+/gu, ' ').trim();
let leidos = 0;
for (const [r, { $ }] of Object.entries(PAGINAS)) {
  if (r === '/' || r === '/en/') continue;
  const copia = path.join(dist, r, 'index.html.md');
  if (!fs.existsSync(copia)) { fallos.push(`Falta la copia en Markdown de ${r}`); continue; }
  const md = llano(fs.readFileSync(copia, 'utf8'));
  $('main').find('p, li, h2, h3, dt, dd, th, td, pre, button').filter((_, n) => !$(n).closest('[aria-hidden="true"]').length && !$(n).find('p, li, h2, h3, dt, dd, th, td, pre, button').length).each((_, n) => {
    // Its text, piece by piece: the canvas sets some pieces side by side with no space between them.
    const t = llano(textos(n).join(' '));
    if (!t) return;
    leidos += 1;
    if (!md.includes(t)) fallos.push(`${r}index.html.md no lleva «${norm($(n).text()).slice(0, 70)}»`);
  });
}
const llms = fs.existsSync(path.join(dist, 'llms.txt')) ? fs.readFileSync(path.join(dist, 'llms.txt'), 'utf8') : '';
if (!llms) fallos.push('Falta dist/llms.txt');
for (const k of ORDEN) if (!llms.includes(`${DOCS[k].en.ruta}index.html.md`)) fallos.push(`llms.txt no lleva ${DOCS[k].en.ruta}`);
console.log(`Para agentes: llms.txt y las copias en Markdown, con los ${leidos} bloques de texto de sus páginas.`);

if (fallos.length) {
  console.log(`\n${fallos.length} problemas:\n${fallos.map((f) => `  ${f}`).join('\n')}`);
  process.exit(1);
}
console.log('Todo en orden.');
