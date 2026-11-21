// Builds the static site in dist/ from the text of the documents, kept in content/ as they are
// published, so the wording is never retyped here: if a block is renamed, the build stops and says which.
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import * as cheerio from 'cheerio';
import esbuild from 'esbuild';
import { sprite, marca, icono, plano, anatomia } from './partes.mjs';
import { portada } from './portada.mjs';
import { PARTES, TIPOS, DOCS, CAPITULOS, TOTAL, ORDEN, MENU, PIE, REDIRECCIONES } from './estructura.mjs';
import { SKILLS, CONTRATO, conVersiones } from './versiones.mjs';
import { aMarkdown } from './markdown.mjs';

const raiz = path.join(path.dirname(fileURLToPath(import.meta.url)), '..');
const dist = path.join(raiz, 'dist');
const leer = (p) => fs.readFileSync(path.join(raiz, p), 'utf8');

const DOMINIO = 'https://gate-driven.dev';
// The interactive pieces of the hands-on chapter (the gates and the journey) are written by its own files (see the README). If they are not there yet, the build says so and goes on.
const TUTORIAL_FICHEROS = ['src/css/tutorial.css', 'src/js/tutorial.js', 'scripts/tutorial.mjs'];
const TUTORIAL_FALTA = TUTORIAL_FICHEROS.filter((f) => !fs.existsSync(path.join(raiz, f)));
if (TUTORIAL_FALTA.length) console.warn(`AVISO: faltan ${TUTORIAL_FALTA.join(', ')}; las piezas data-pieza quedan vacías.`);
const TUTORIAL_CSS = fs.existsSync(path.join(raiz, 'src/css/tutorial.css'));
const TUTORIAL_JS = fs.existsSync(path.join(raiz, 'src/js/tutorial.js'));
let tutorial = () => '';
if (!TUTORIAL_FALTA.includes('scripts/tutorial.mjs')) ({ tutorial } = await import('./tutorial.mjs'));
const VERSION = createHash('sha256')
  .update(['src/css/tema-gdd.css', 'src/css/site.css', 'src/css/documentacion.css', 'src/css/portada.css', 'src/css/tutorial.css', 'src/js/plano.js', 'src/js/portada.js', 'src/js/tutorial.js', 'src/js/sitio.js', 'src/js/kimen.js', 'node_modules/@kimen/elements/package.json', 'node_modules/@kimen/tokens/package.json'].filter((f) => fs.existsSync(path.join(raiz, f))).map(leer).join(''))
  .digest('hex').slice(0, 10);

// --- Two languages -------------------------------------------------------------------------
// Spanish lives at the root and English under /en/. Each pass of the build sets the language,
// and every text the documents do not provide is written here in both, side by side.
const IDIOMAS = ['es', 'en'];
let idioma = 'es';
const tr = (es, en) => (idioma === 'en' ? en : es);
const carpeta = () => (idioma === 'en' ? 'content/en' : 'content');
const rutaInicio = () => tr('/', '/en/');

// --- The public documents: see estructura.mjs ----------------------------------------------

const norm = (s) => s.replace(/[  ]/g, ' ').replace(/\s+/g, ' ').trim();
const slug = (t) => t.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
const escapar = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
const MESES = ['enero', 'febrero', 'marzo', 'abril', 'mayo', 'junio', 'julio', 'agosto', 'septiembre', 'octubre', 'noviembre', 'diciembre'];
const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];
const fechaLarga = (iso) => { const [a, m, d] = iso.split('-').map(Number); return tr(`${d} de ${MESES[m - 1]} de ${a}`, `${MONTHS[m - 1]} ${d}, ${a}`); };

// --- Diagrams: the document's own widget modules, rendered to static SVG ----------------
const attrMap = {
  strokeWidth: 'stroke-width', fontSize: 'font-size', fontWeight: 'font-weight', textAnchor: 'text-anchor',
  fillOpacity: 'fill-opacity', markerEnd: 'marker-end', strokeDasharray: 'stroke-dasharray',
};
const __h = (tag, props, ...kids) => {
  const attrs = Object.entries(props || {})
    .filter(([k, v]) => v != null && !k.startsWith('data-'))
    .map(([k, v]) => ` ${attrMap[k] || k}="${String(v).replace(/"/g, '&quot;')}"`)
    .join('');
  const inner = kids.flat(Infinity).filter((x) => x != null && x !== false).join('');
  return `<${tag}${attrs}>${inner}</${tag}>`;
};
const renderSvg = (fichero) => {
  const { code } = esbuild.transformSync(leer(fichero), { loader: 'jsx', jsxFactory: '__h', format: 'cjs', target: 'node18' });
  const module = { exports: {} };
  new Function('__h', 'module', 'exports', code)(__h, module, module.exports);
  return module.exports.default();
};
const diagramas = (dir) => ({ sencillo: renderSvg(`${dir}/sencillo.jsx`), recorrido: renderSvg(`${dir}/recorrido.jsx`), momentos: renderSvg(`${dir}/momentos.jsx`) });
const DIAGRAMAS = { es: diagramas('content'), en: diagramas('content/en') };

// --- Loading a document: code from its files, diagrams, anchors, numbered steps ------------
const TODOS = { es: {}, en: {} };
let T = TODOS.es;
const cargar = (nombre) => {
  const $ = cheerio.load(conVersiones(leer(`${carpeta()}/${DOCS[nombre].fichero}`)), null, false);

  $('code[data-fuente]').each((_, c) => {
    const f = $(c).attr('data-fuente');
    $(c).removeAttr('data-fuente').text(conVersiones(leer(`${carpeta()}/codigo/${f}`)).replace(/\n$/, ''));
  });

  $('figure[data-pieza]').each((_, f) => {
    const fig = $(f);
    // The piece writes its own heading levels: its title is an h2 and its steps are h3.
    const html = tutorial({ clave: fig.attr('data-pieza'), idioma });
    fig.replaceWith(html);
  });

  $('figure[data-embed]').each((_, f) => {
    const fig = $(f);
    const svg = DIAGRAMAS[idioma][fig.attr('data-embed')];
    if (!svg) throw new Error(`Figura sin diagrama: ${fig.attr('data-embed')}`);
    const pie = fig.find('figcaption').html();
    fig.replaceWith(`<figure class="diagrama"><div class="diagrama__marco" tabindex="0">${svg}</div>${pie ? `<figcaption>${pie}</figcaption>` : ''}</figure>`);
  });

  // Anchors for the headings that name a section.
  const usados = new Set();
  $('h2, h3').each((_, h) => {
    let id = $(h).attr('id') || slug($(h).text());
    while (usados.has(id)) id += '-2';
    usados.add(id);
    $(h).attr('id', id);
  });

  // A paragraph that is only a bold line works as a small heading.
  $('p').each((_, n) => {
    const p = $(n);
    if (p.children('strong').length === 1 && norm(p.text()) === norm(p.children('strong').text()) && !p.is('.meta')) p.addClass('subtitulo');
  });

  // The numbered practices keep an anchor each and their scope as a quiet note.
  if (nombre === 'buenas-practicas') {
    $('p').each((_, n) => {
      const p = $(n);
      const m = norm(p.children('strong').first().text()).match(/^(\d+)\. /);
      if (m && p.children('em').length) p.attr('id', `practica-${m[1]}`).addClass('practica').removeClass('subtitulo');
    });
  }

  // "1. Something." paragraphs, with their code and notes, become a real list of steps.
  const hijos = $.root().children().toArray();
  let lista = null;
  let actual = null;
  for (const n of hijos) {
    const nodo = $(n);
    const esPaso = nodo.is('p') && /^<strong>\d+\.\s/.test(nodo.html() || '') && !nodo.children('em').length;
    if (esPaso) {
      if (!lista) { lista = $('<ol class="pasos"></ol>'); nodo.before(lista); }
      const fuerte = nodo.children('strong').first();
      const m = norm(fuerte.text()).match(/^(\d+)\.\s+(.*)$/);
      fuerte.remove();
      actual = $(`<li id="paso-${m[1]}"></li>`);
      actual.append(`<h3>${m[2]}</h3>\n`);
      nodo.removeClass('subtitulo').html(nodo.html().trim());
      actual.append(nodo);
      lista.append(actual);
      lista.append('\n');
    } else if (lista && actual) {
      if (nodo.is('p') && /^<strong>(Y ya está|And that's it)\./.test(nodo.html() || '')) { nodo.addClass('remate'); actual = null; lista = null; } else { actual.append('\n'); actual.append(nodo); }
    }
  }
  T[nombre] = $;
};
for (const i of IDIOMAS) {
  idioma = i;
  T = TODOS[i];
  for (const nombre of Object.keys(DOCS)) cargar(nombre);
}

// --- Reading blocks from the documents ----------------------------------------------------
const buscar = (nombre, prefijo) => {
  const el = T[nombre]('p, li, td, th, h1, h2, h3').filter((_, n) => norm(T[nombre](n).text()).startsWith(prefijo)).first();
  if (!el.length) throw new Error(`No encuentro en ${nombre}: "${prefijo}"`);
  return el;
};
const interior = (nombre, prefijo) => buscar(nombre, prefijo).html();
const texto = (nombre, prefijo) => norm(buscar(nombre, prefijo).text());
const parrafo = (nombre, prefijo, clase = '') => `<p${clase ? ` class="${clase}"` : ''}>${interior(nombre, prefijo)}</p>`;
const frase = (nombre, inicio) => {
  const el = T[nombre]('p, li, td').filter((_, n) => norm(T[nombre](n).text()).includes(inicio)).first();
  if (!el.length) throw new Error(`No encuentro la frase en ${nombre}: "${inicio}"`);
  const t = norm(el.text());
  const i = t.indexOf(inicio);
  return t.slice(i, t.indexOf('.', i) + 1);
};
// What follows the colon in a sentence of the document, as a sentence of its own.
const resto = (nombre, inicio) => { const f = frase(nombre, inicio); const r = f.slice(f.indexOf(':') + 2); return r.charAt(0).toUpperCase() + r.slice(1); };
const fila = (nombre, prefijo) => T[nombre].html(buscar(nombre, prefijo).closest('tr'));
const tabla = (nombre, primeraCabeza) => {
  const t = T[nombre]('table').filter((_, x) => norm(T[nombre](x).find('th').first().text()) === primeraCabeza).first();
  if (!t.length) throw new Error(`No hay tabla "${primeraCabeza}" en ${nombre}`);
  return T[nombre].html(t);
};
const hasta = (nombre, desde, parar) => {
  const salida = [];
  let n = buscar(nombre, desde);
  while (n.length) {
    if (parar && norm(n.text()).startsWith(parar)) break;
    salida.push(T[nombre].html(n));
    n = n.next();
  }
  return salida.join('\n');
};

// --- Dressing the document's HTML for the page --------------------------------------------
const vestir = (html, { copiar = false } = {}) => {
  const $ = cheerio.load(html, null, false);

  $('a[href^="doc:"]').each((_, a) => {
    const [clave, ancla] = $(a).attr('href').slice(4).split('#');
    const destino = DOCS[clave]?.[idioma].ruta;
    if (!destino) throw new Error(`Enlace a un documento que no existe: ${$(a).attr('href')}`);
    $(a).attr('href', ancla ? `${destino}#${ancla}` : destino);
  });

  $('table').each((_, t) => {
    const tab = $(t);
    tab.find('thead th').attr('scope', 'col');
    const cabezas = tab.find('thead th').map((_, th) => norm($(th).text())).get();
    tab.find('tbody tr').each((_, tr) => {
      $(tr).children('td').each((i, td) => { $(td).attr('data-label', cabezas[i]); });
    });
    tab.wrap('<div class="tabla tabla--apilada"></div>');
  });

  for (const [palabra, clase] of [[tr('Verde', 'Green'), 'verde'], [tr('Amarillo', 'Yellow'), 'amarillo'], [tr('Rojo', 'Red'), 'rojo']]) {
    $('td > strong').filter((_, s) => $(s).text() === palabra).each((_, s) => {
      const td = $(s).parent();
      td.html(`<span class="celda-lampara"><span class="lampara lampara--${clase}" aria-hidden="true"></span><span>${td.html()}</span></span>`);
    });
  }

  $('ol:not(.pasos):not(.mapa-lista)').addClass('lista-pasos');
  $('dl').each((_, dl) => {
    let grupo = null;
    $(dl).children('dt, dd').each((_, n) => {
      if (!grupo || (n.name === 'dt' && grupo.children().last().is('dd'))) grupo = $('<div></div>').insertBefore(n);
      grupo.append(n);
    });
  });
  if (!copiar) $('pre').removeAttr('data-copiar');
  // A code block of a document is a Kimen ki-code-block, which copies its code where the document
  // asks for it (data-copiar). The code stays as text inside it, so the Markdown copy, the checks
  // and a page without scripts still read it. The pieces of the hands-on chapter keep their pre.
  $('pre').filter((_, pre) => !$(pre).closest('.tutorial').length).each((_, pre) => {
    const p = $(pre);
    const bloque = $('<ki-code-block></ki-code-block>').text(p.text().replace(/\n$/, ''));
    bloque.attr({ label: tr('Código', 'Code'), 'copy-label': tr('Copiar el código', 'Copy the code'), 'copied-label': tr('Copiado', 'Copied'), 'copy-error-label': tr('No se ha podido copiar: selecciónalo a mano', "Couldn't copy: select it by hand") });
    if (!p.is('[data-copiar]')) bloque.attr('copyable', 'false');
    p.replaceWith(bloque);
  });

  let salida = $.html();
  // On the Spanish pages, the sentences quoted in English say so.
  if (idioma === 'es') salida = salida
    .replaceAll('"Humans steer. Agents execute."', '"<span lang="en">Humans steer. Agents execute.</span>"')
    .replaceAll('"corrections are cheap, and waiting is expensive"', '"<span lang="en">corrections are cheap, and waiting is expensive</span>"')
    .replaceAll('"the harness is most needed where it is hardest to build"', '"<span lang="en">the harness is most needed where it is hardest to build</span>"');
  // Number and unit stay together. Code stays as written.
  salida = salida.split(/(<pre[\s\S]*?<\/pre>|<ki-code-block[\s\S]*?<\/ki-code-block>|<svg[\s\S]*?<\/svg>)/).map((parte) => (/^<(pre|ki-code-block|svg)/.test(parte) ? parte : parte.replace(/(\d) (%|€)/g, '$1&nbsp;$2'))).join('');
  return salida;
};

// --- Page frame ---------------------------------------------------------------------------
let VERSION_TXT;
let ESTADO_TXT;
let FECHA;
let FIRMA;
let PDF;
const usar = (i) => {
  idioma = i;
  T = TODOS[i];
  const meta = T.manifiesto('p.meta');
  [, VERSION_TXT, ESTADO_TXT] = norm(meta.eq(1).text()).match(/^(v[\d.]+) · (.+)$/);
  FECHA = meta.eq(0).attr('data-fecha');
  FIRMA = `Marcela Gotta, ${fechaLarga(FECHA)}`;
  PDF = tr(`manifiesto-gdd-${VERSION_TXT}.pdf`, `manifesto-gdd-${VERSION_TXT}.pdf`);
};
const TMINUS = 'https://github.com/MarsGotta/t-minus';
const IMAGEN = () => tr('og.png', 'og-en.png');

// The same page in the other language.
const otraRuta = (ruta) => {
  const otro = tr('en', 'es');
  const k = Object.keys(DOCS).find((x) => DOCS[x][idioma].ruta === ruta);
  return k ? DOCS[k][otro].ruta : tr('/en/', '/');
};

// The header is Kimen's ki-top-nav, which draws its own banner and navigation landmarks: the
// brand, the sections and, as actions, the other language and the light or dark switch. Below
// 48rem, where the row no longer fits, it folds the sections into a menu (Kimen 0.1.1 needs the
// attribute written: without it the bar never folds).
const cabecera = (ruta, enPlano = false) => `<div class="cabecera${enPlano ? ' cabecera--plano' : ''}"${enPlano ? ' data-gdd-sheet' : ''}><ki-top-nav class="contenedor" label="${tr('Principal', 'Main')}" navigation-label="${tr('Abrir el menú', 'Open the menu')}" collapse-at="48rem">
  <a slot="brand" class="logo" href="${rutaInicio()}"${ruta === rutaInicio() ? ' aria-current="page"' : ''}>${marca()}<span>Gate-Driven Development</span></a>
  ${MENU.map((k) => {
    const r = DOCS[k][idioma].ruta;
    const dentro = k === 'documentacion' && ruta !== r && ruta.startsWith(r);
    return `<a href="${r}"${ruta === r ? ' aria-current="page"' : dentro ? ' aria-current="true"' : ''}>${DOCS[k][idioma].menu}</a>`;
  }).join('\n  ')}
  <a slot="actions" class="idioma" href="${otraRuta(ruta)}" hreflang="${tr('en', 'es')}" lang="${tr('en', 'es')}" aria-label="${tr('Read this page in English', 'Lee esta página en español')}">${tr('English', 'Español')}</a>
  <ki-icon-button slot="actions" class="tema" variant="quaternary" data-tema-boton label="${tr('Cambiar entre claro y oscuro', 'Switch between light and dark')}">${icono('tema')}</ki-icon-button>
</ki-top-nav></div>`;

// On the home page the footer is part of its last room, on the cyanotype sheet.
const pie = (enPlano = false) => `<footer class="pie${enPlano ? ' pie--plano' : ''}"${enPlano ? ' data-gdd-sheet' : ''}><div class="contenedor pie__dentro">
  <div>
    <a class="logo" href="${rutaInicio()}">${marca()}<span>Gate-Driven Development</span></a>
    <p>${tr('Manifiesto', 'Manifesto')} ${VERSION_TXT}, ${ESTADO_TXT}. ${tr('Skills y capa', 'Skills and layer')} ${SKILLS}, ${tr('contrato', 'contract')} <code>${CONTRATO}</code>. ${FIRMA}.</p>
    <p>${frase('manifiesto', tr('Este manifiesto es la referencia.', 'This manifesto is the reference.'))}</p>
  </div>
  <div>
    <p>${texto('manifiesto', tr('Esta versión todavía no trae datos propios.', "This version doesn't bring its own data yet."))}</p>
    <p>${tr('T-minus, el motor que hace cumplir GDD, está en desarrollo y todavía no cubre todas las garantías del núcleo.', 'T-minus, the engine that enforces GDD, is in development and does not cover all the guarantees of the core yet.')}</p>
  </div>
  <div>
    <p><a href="/${PDF}">${tr('Descargar el manifiesto en PDF', 'Download the manifesto as a PDF')}</a> (${VERSION_TXT}, ${tr('solo el manifiesto', 'manifesto only')}).</p>
    <p><a href="${TMINUS}">${tr('T-minus, en GitHub', 'T-minus on GitHub')}</a></p>
    <p>${tr('Los textos tienen licencia', 'The texts are under')} <a href="https://creativecommons.org/licenses/by/4.0/">CC BY 4.0</a> ${tr('y el código, MIT: puedes usarlos y adaptarlos si citas la fuente. Cómo citar: Marcela Gotta, <em>Gate-Driven Development</em>, gate-driven.dev.', 'and the code under MIT: you may use and adapt them if you credit the source. How to cite: Marcela Gotta, <em>Gate-Driven Development</em>, gate-driven.dev.')}</p>
  </div>
  <nav class="pie__paginas" aria-label="${tr('Más páginas', 'More pages')}">
    <ul>${PIE.map((k) => `<li><a href="${DOCS[k][idioma].ruta}">${DOCS[k][idioma].menu}</a></li>`).join('')}</ul>
  </nav>
</div></footer>`;

// What search engines read about the page, beyond its words.
const datos = ({ ruta, titulo, descripcion, inicial }) => {
  const url = `${DOMINIO}${ruta}`;
  const autora = { '@type': 'Person', name: 'Marcela Gotta' };
  const sitio = { '@type': 'WebSite', name: 'Gate-Driven Development', alternateName: 'GDD', url: `${DOMINIO}${rutaInicio()}`, inLanguage: idioma };
  const cuerpo = inicial
    ? { ...sitio, description: descripcion, author: autora }
    : { '@type': 'Article', headline: titulo, description: descripcion, url, mainEntityOfPage: url, inLanguage: idioma, datePublished: FECHA, dateModified: FECHA, author: autora, image: `${DOMINIO}/${IMAGEN()}`, isPartOf: sitio };
  return `<script type="application/ld+json">${JSON.stringify({ '@context': 'https://schema.org', ...cuerpo }).replace(/</g, '\\u003c')}</script>`;
};

// Every page of a document also lives as Markdown, at its address plus index.html.md.
const RUTAS_MD = new Set(Object.values(DOCS).flatMap((d) => [d.es.ruta, d.en.ruta]));
const aMd = (ruta) => `${DOMINIO}${ruta}index.html.md`;

const documento = ({ ruta, titulo, descripcion, cuerpo, scripts = [], estilos = [], cabeceraPlano = false, indexable = true, enlacesCabeza = '' }) => {
  const enEs = idioma === 'es' ? ruta : otraRuta(ruta);
  const enEn = idioma === 'en' ? ruta : otraRuta(ruta);
  const buscadores = indexable
    ? `<link rel="canonical" href="${DOMINIO}${ruta}">
<link rel="alternate" hreflang="es" href="${DOMINIO}${enEs}">
<link rel="alternate" hreflang="en" href="${DOMINIO}${enEn}">
<link rel="alternate" hreflang="x-default" href="${DOMINIO}${enEn}">${RUTAS_MD.has(ruta) ? `\n<link rel="alternate" type="text/markdown" href="${aMd(ruta)}">` : ''}
${datos({ ruta, titulo, descripcion, inicial: ruta === rutaInicio() })}`
    : '<meta name="robots" content="noindex">';
  return `<!doctype html>
<html lang="${idioma}" data-ki-theme="gdd">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<script>document.documentElement.classList.add('js');try{var t=localStorage.getItem('tema');if(t==='claro'||t==='oscuro')document.documentElement.dataset.kiColorScheme=t==='oscuro'?'dark':'light'}catch(e){}</script>
<title>${titulo}</title>
<meta name="description" content="${descripcion}">
<meta name="author" content="Marcela Gotta">
<meta name="theme-color" content="${cabeceraPlano ? '#063879' : '#f7f9fb'}" media="(prefers-color-scheme: light)">
<meta name="theme-color" content="${cabeceraPlano ? '#063879' : '#0f1622'}" media="(prefers-color-scheme: dark)">
${buscadores}
${enlacesCabeza}
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<link rel="preload" href="/assets/archivo.woff2" as="font" type="font/woff2" crossorigin>
<link rel="stylesheet" href="/assets/kimen.css?v=${VERSION}">
<link rel="stylesheet" href="/assets/site.css?v=${VERSION}">
${estilos.map((e) => `<link rel="stylesheet" href="/assets/${e}?v=${VERSION}">`).join('\n')}
<noscript><style>.plano__eleccion, .plano__avance { display: none; }</style></noscript>
<meta property="og:type" content="${ruta === rutaInicio() ? 'website' : 'article'}">
<meta property="og:locale" content="${tr('es_ES', 'en_US')}">
<meta property="og:locale:alternate" content="${tr('en_US', 'es_ES')}">
<meta property="og:site_name" content="Gate-Driven Development">
<meta property="og:title" content="${titulo}">
<meta property="og:description" content="${descripcion}">
<meta property="og:url" content="${DOMINIO}${ruta}">
<meta property="og:image" content="${DOMINIO}/${IMAGEN()}">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta property="og:image:alt" content="${tr('Plano de una casa con tres puertas por las que pasa un agente de IA.', 'Floor plan of a house with three gates an AI agent has to get through.')}">
<meta name="twitter:card" content="summary_large_image">
</head>
<body>
<a class="saltar" href="#contenido">${tr('Saltar al contenido', 'Skip to content')}</a>
${sprite}
${cabecera(ruta, cabeceraPlano)}
<main id="contenido">
${cuerpo}
</main>
${pie(cabeceraPlano)}
<script src="/assets/kimen.js?v=${VERSION}" type="module"></script>
<script src="/assets/sitio.js?v=${VERSION}" defer></script>
${scripts.map((s) => `<script src="/assets/${s}?v=${VERSION}" defer></script>`).join('\n')}
</body>
</html>
`;
};


const indice = (entradas) => `<nav class="indice" aria-label="${tr('En esta página', 'On this page')}">
  <details>
    <summary>${tr('En esta página', 'On this page')}</summary>
    <ol>${entradas.map(([id, t]) => `<li><a href="#${id}">${t}</a></li>`).join('')}</ol>
  </details>
</nav>`;

// --- The home page ------------------------------------------------------------------------
// The landing of the design canvas (scripts/portada.mjs), under a dark header that joins its
// blueprint hero, with the site's own walkthrough of the risk (plano.js).
const inicio = () => documento({
  ruta: rutaInicio(),
  titulo: tr('Gate-Driven Development: puertas para agentes de IA', 'Gate-Driven Development: quality gates for AI agents'),
  descripcion: frase('manifiesto', tr('Gate-Driven Development (GDD) es una forma', 'Gate-Driven Development (GDD) is a way')),
  cuerpo: portada({
    tr,
    ruta: (k) => DOCS[k][idioma].ruta,
    menu: (k) => DOCS[k][idioma].menu,
    texto,
    frase,
    interior,
    manifiesto: T.manifiesto,
    plano,
    version: VERSION_TXT,
    estado: ESTADO_TXT,
  }),
  scripts: ['plano.js', 'portada.js'],
  estilos: ['portada.css'],
  cabeceraPlano: true,
});

// --- The pages of the documents -----------------------------------------------------------
// A question with its answers (aside[data-cruzar] in a document): Kimen's ki-radio-group holds the
// answers, and sitio.js shows why on each one. The one a chapter ends with, "Antes de cruzar", is
// drawn as the door the canvas closes each chapter with, which opens on the right answer; the
// workshop's cases (data-caso) are the question alone. Without scripts, the answers and both
// explanations stay in view.
const pregunta = (aparte, $, final) => {
  const enunciado = aparte.children('p').not('[data-si]');
  const respuestas = aparte.find('li');
  const por = (si) => aparte.children(`p[data-si="${si}"]`);
  const falta = [
    enunciado.length !== 1 && 'one question',
    (respuestas.length < 2 || respuestas.filter('[data-bien]').length !== 1) && 'two answers or more, one of them data-bien',
    (por('bien').length !== 1 || por('mal').length !== 1) && 'one p[data-si="bien"] and one p[data-si="mal"]',
  ].filter(Boolean);
  if (falta.length) throw new Error(`aside[data-cruzar] needs ${falta.join(', ')}: ${norm(aparte.text()).slice(0, 80)}`);
  const opciones = respuestas.map((i, li) => `<ki-radio value="${$(li).is('[data-bien]') ? 'bien' : `mal-${i}`}">${$(li).html()}</ki-radio>`).get().join('');
  const puerta = final ? `<div class="cruzar__muro" aria-hidden="true"><i></i><svg viewBox="0 0 84 84"><path class="cruzar__arco" d="M84 5A84 84 0 0 1 0 89"></path><path class="cruzar__hoja" d="M0 5H84"></path></svg><i></i></div>
<h2 class="cruzar__titulo" id="cruzar-titulo">${tr('Antes de cruzar', 'Before you cross')}</h2>
` : '';
  return `<section class="cruzar${final ? '' : ' cruzar--caso'}"${final ? ' aria-labelledby="cruzar-titulo"' : ''}>
${puerta}<p class="cruzar__pregunta">${enunciado.html()}</p>
<ki-radio-group class="cruzar__opciones" label="${tr('Tu respuesta', 'Your answer')}">${opciones}</ki-radio-group>
<div aria-live="polite"><p class="cruzar__si" data-si="bien">${por('bien').html()}</p>
<p class="cruzar__si" data-si="mal">${por('mal').html()}</p></div>
</section>`;
};

// The workshop's lab (section[data-laboratorio]): each of the four questions is a Kimen ki-switch,
// on to begin with, and turning one off shows what slips through and opens the door of the plan.
const laboratorio = (seccion, $) => {
  const lista = seccion.children('ul').first();
  const mandos = lista.children('li').map((i, li) => {
    const resto = $(li).clone();
    const titulo = resto.children('strong').first();
    if (!titulo.length) throw new Error(`Each question of section[data-laboratorio] starts in <strong>: ${norm($(li).text()).slice(0, 80)}`);
    const q = titulo.html();
    titulo.remove();
    return `<div class="laboratorio__mando"><ki-switch checked data-n="${i}">${q}</ki-switch>\n<p class="laboratorio__porque" data-n="${i}">${resto.html().trim()}</p></div>`;
  }).get().join('');
  lista.replaceWith(`<div class="laboratorio__mandos">${mandos}</div>
<div class="laboratorio__plano" aria-hidden="true"><span class="laboratorio__lugar laboratorio__lugar--agente">${tr('Agente', 'Agent')}</span><span class="laboratorio__lugar laboratorio__lugar--produccion">${tr('Producción', 'Production')}</span><span class="laboratorio__muro"></span><svg class="laboratorio__puerta" viewBox="0 0 84 84"><path class="cruzar__arco" d="M84 5A84 84 0 0 1 0 89"></path><path class="cruzar__hoja" d="M0 5H84"></path></svg><span class="laboratorio__agente"></span></div>`);
  const resultados = seccion.children('p[data-resultado]');
  if (resultados.filter('[data-resultado="cierra"]').length !== 1 || resultados.filter('[data-resultado="abre"]').length !== 1) throw new Error('section[data-laboratorio] needs one p[data-resultado="cierra"] and one "abre"');
  resultados.wrapAll('<div aria-live="polite"></div>');
  seccion.addClass('laboratorio').attr('data-estado', 'cierra');
};

// Shared by every page of text: the body of the document, dressed, and its index of sections.
const cuerpoDe = (nombre) => {
  const $ = T[nombre];
  const copia = cheerio.load($.html(), null, false);
  const finales = copia('aside[data-cruzar]:not([data-caso])');
  if (finales.length > 1) throw new Error(`${nombre}: ${finales.length} questions without data-caso; only the one that closes the chapter goes without it`);
  const final = finales.length ? pregunta(finales, copia, true) : '';
  finales.remove();
  copia('aside[data-cruzar][data-caso]').each((_, a) => { copia(a).replaceWith(pregunta(copia(a), copia, false)); });
  copia('section[data-laboratorio]').each((_, sec) => laboratorio(copia(sec), copia));
  const h1 = copia('h1').first().html();
  copia('h1').first().remove();
  copia('p.meta').remove();
  if (nombre === 'las-piezas') {
    const hueco = copia('[data-anatomia]');
    if (hueco.length) hueco.replaceWith(anatomia(tr));
    else copia('th').filter((_, th) => copia(th).text() === tr('Pieza', 'Piece')).first().closest('table').after(anatomia(tr));
  }
  // A page without sections keeps its headings one level below the title, with no jump.
  if (!copia('h2').length) copia('h3').each((_, h) => { h.tagName = 'h2'; h.name = 'h2'; });
  const conH2 = copia('h2').length > 0;
  const entradas = copia(conH2 ? 'h2[id]' : 'h3[id]').filter((_, h) => !copia(h).closest('.tutorial').length).map((_, h) => [[copia(h).attr('id'), copia(h).text()]]).get();
  if (copia('[data-boton]').length) copia('[data-boton] a').addClass('boton').parent().addClass('salidas').removeAttr('data-boton');
  const cuerpo = vestir(copia.html() + final, { copiar: true });
  return { h1, cuerpo, entradas, usaTutorial: copia('.tutorial').length > 0 };
};

const paginaDe = (nombre) => {
  const doc = DOCS[nombre][idioma];
  const { h1, cuerpo, entradas } = cuerpoDe(nombre);
  const bajada = [];
  if (nombre === 'manifiesto') {
    bajada.push(`${FIRMA}. ${VERSION_TXT}, ${ESTADO_TXT}.`);
    bajada.push(`${frase('manifiesto', tr('Este manifiesto es la referencia.', 'This manifesto is the reference.'))} <a href="/${PDF}">${tr('Descárgalo en PDF', 'Download it as a PDF')}</a>.`);
  } else if (nombre === 'articulo') {
    bajada.push(FIRMA);
  }
  const cabeza = `<div class="pagina-cabeza contenedor"><h1>${nombre === 'manifiesto' ? `${tr('Manifiesto', 'Manifesto')}: <span class="sin-corte">Gate-Driven Development</span>` : h1}</h1>${bajada.map((b) => `<p>${b}</p>`).join('')}</div>`;
  return documento({
    ruta: doc.ruta,
    titulo: doc.titulo,
    descripcion: doc.descripcion,
    cuerpo: `${cabeza}
    <div class="con-indice contenedor">${indice(entradas)}<article class="lectura">${cuerpo}</article></div>`,
  });
};

// The map of the documentation: a short text and the chapters in their parts. Each list of the
// map starts at the number its first chapter has in the book, so a chapter added or moved never
// leaves the map counting behind.
// Three ways into the documentation, as the design canvas proposes: each one numbers its stops on
// the map. The number is drawn from data-orden-<way>, so the map's text stays its document's.
const RUTAS = {
  a: { es: 'Usarlo ya', en: 'Use it now', paradas: ['empieza', 'las-skills', 'cuando-se-para', 'los-colores'] },
  b: { es: 'Entenderlo', en: 'Understand it', paradas: ['la-idea', 'manos-a-la-obra', 'las-piezas', 'los-principios', 'los-momentos'] },
  c: { es: 'A mano', en: 'By hand', paradas: ['empieza-hoy', 'un-caso-delicado', 'como-se-engana-una-puerta', 'el-nucleo'] },
};
const cuantas = (n) => tr(['', '', 'Dos', 'Tres', 'Cuatro', 'Cinco'][n], ['', '', 'Two', 'Three', 'Four', 'Five'][n]);

const paginaMapa = () => {
  const doc = DOCS.documentacion[idioma];
  const { h1, cuerpo: tal } = cuerpoDe('documentacion');
  const $m = cheerio.load(tal, null, false);
  let n = 1;
  $m('ol.mapa-lista').each((_, ol) => {
    $m(ol).attr('start', String(n));
    n += $m(ol).children('li').length;
  });
  for (const [r, { paradas }] of Object.entries(RUTAS)) {
    paradas.forEach((k, i) => { $m(`ol.mapa-lista > li:has(> a[href="${DOCS[k][idioma].ruta}"])`).attr(`data-orden-${r}`, String(i + 1)); });
  }
  const cuerpo = $m.html();
  const rutas = `<div class="rutas contenedor">
  <ki-segmented-control class="rutas__control" label="${tr('Cómo quieres empezar', 'How you want to start')}" value="a" full-width>${Object.entries(RUTAS).map(([r, x]) => `<ki-segment value="${r}">${x[idioma]}</ki-segment>`).join('')}</ki-segmented-control>
  <div class="rutas__salida">${Object.entries(RUTAS).map(([r, { paradas }]) => `<p class="rutas__ruta" data-ruta="${r}"><a class="boton" href="${DOCS[paradas[0]][idioma].ruta}">${tr(`Ir a «${DOCS[paradas[0]].es.menu}»`, `Go to "${DOCS[paradas[0]].en.menu}"`)}</a> <span>${cuantas(paradas.length)} ${tr('paradas, numeradas abajo.', 'stops, numbered below.')}</span></p>`).join('')}<p class="rutas__taller"><a href="${DOCS.taller[idioma].ruta}">${DOCS.taller[idioma].titulo}</a></p></div>
</div>`;
  return documento({
    ruta: doc.ruta,
    titulo: doc.titulo,
    descripcion: doc.descripcion,
    cuerpo: `<div class="pagina-cabeza contenedor"><h1>${h1}</h1></div>
    ${rutas}
    <div class="mapa contenedor"><article class="lectura">${cuerpo}</article></div>`,
  });
};

// A chapter: its place in the book, the list of chapters beside it, and Previous and Next.
const capitulosNav = (actual, entradas) => {
  const items = (parte) => CAPITULOS.filter((k) => DOCS[k].capitulo.parte === parte).map((k) => {
    const d = DOCS[k];
    const es = k === actual;
    const apartados = es && entradas.length ? `<ol class="capitulos__apartados">${entradas.map(([id, t]) => `<li><a href="#${id}">${t}</a></li>`).join('')}</ol>` : '';
    return `<li><a href="${d[idioma].ruta}"${es ? ' aria-current="page"' : ''}><span class="capitulos__n">${d.capitulo.n}</span><span>${d[idioma].menu}</span></a>${apartados}</li>`;
  }).join('');
  const mapa = DOCS.documentacion[idioma];
  return `<nav class="capitulos" aria-label="${tr('Capítulos de la documentación', 'Documentation chapters')}">
  <details>
    <summary>${tr('Capítulos', 'Chapters')}</summary>
    <p class="capitulos__mapa"><a href="${mapa.ruta}">${mapa.menu}</a></p>
    <ol class="capitulos__partes">${Object.keys(PARTES).map((p) => `<li><p class="capitulos__parte">${PARTES[p][idioma]}</p><ol>${items(p)}</ol></li>`).join('')}</ol>
  </details>
</nav>`;
};

// How long a chapter takes to read, at about 200 words a minute, never less than one.
const lectura = (html) => {
  const $ = cheerio.load(html, null, false);
  $('figure, .tutorial').remove(); // the pieces and diagrams are looked at, not read word by word
  const minutos = Math.max(1, Math.round(norm($.root().text()).split(' ').length / 200));
  return minutos === 1 ? tr('Un minuto', 'One minute') : tr(`Unos ${minutos} minutos`, `About ${minutos} minutes`);
};

const vecino = (clave, sentido, n) => {
  const d = DOCS[clave];
  const titulo = d.capitulo ? `${d.capitulo.n}. ${d[idioma].menu}` : d[idioma].menu;
  return `<a class="vecino vecino--${sentido}" rel="${sentido === 'anterior' ? 'prev' : 'next'}" href="${d[idioma].ruta}"><span>${sentido === 'anterior' ? tr('Anterior', 'Previous') : tr('Siguiente', 'Next')}</span><strong>${titulo}</strong></a>`;
};

const paginaCapitulo = (nombre) => {
  const doc = DOCS[nombre][idioma];
  const { n, parte, tipo } = DOCS[nombre].capitulo;
  const { h1, cuerpo, entradas, usaTutorial } = cuerpoDe(nombre);
  const antes = n === 1 ? 'documentacion' : CAPITULOS[n - 2];
  const despues = n === TOTAL ? null : CAPITULOS[n];
  const enlaces = `<link rel="prev" href="${DOMINIO}${DOCS[antes][idioma].ruta}">${despues ? `\n<link rel="next" href="${DOMINIO}${DOCS[despues][idioma].ruta}">` : ''}`;
  return documento({
    ruta: doc.ruta,
    titulo: doc.titulo,
    descripcion: doc.descripcion,
    enlacesCabeza: enlaces,
    cuerpo: `<div class="capitulo contenedor">
  ${capitulosNav(nombre, entradas)}
  <header class="capitulo__cabeza"><p class="capitulo__etiqueta">${tr(`Capítulo ${n} de ${TOTAL}`, `Chapter ${n} of ${TOTAL}`)} · ${PARTES[parte][idioma]} · ${TIPOS[tipo][idioma]} · ${lectura(cuerpo)}</p><h1>${h1}</h1><p class="capitulo__bajada">${doc.descripcion}</p></header>
  <article class="lectura">${cuerpo}</article>
  <nav class="vecinos" aria-label="${tr('Anterior y siguiente', 'Previous and next')}">${vecino(antes, 'anterior')}${despues ? vecino(despues, 'siguiente') : ''}</nav>
</div>`,
    scripts: usaTutorial && TUTORIAL_JS ? ['tutorial.js'] : [],
  });
};

const noEncontrada = () => documento({
  ruta: tr('/404', '/en/404'),
  titulo: tr('Esta página no existe', 'This page does not exist'),
  descripcion: tr('Esta dirección no lleva a ninguna página de Gate-Driven Development.', 'This address does not lead to any page of Gate-Driven Development.'),
  indexable: false,
  cuerpo: `<div class="perdido contenedor"><h1>${tr('Esta puerta no lleva a ningún sitio', 'This gate does not lead anywhere')}</h1><p>${tr('La página que buscas no existe o ha cambiado de sitio.', 'The page you are looking for does not exist or has moved.')} <a href="${rutaInicio()}">${tr('Vuelve al inicio', 'Go back to the home page')}</a> ${tr('o', 'or')} <a href="${DOCS.manifiesto[idioma].ruta}">${tr('lee el manifiesto', 'read the manifesto')}</a>.</p></div>`,
});

// --- Writing dist/ ------------------------------------------------------------------------
fs.rmSync(dist, { recursive: true, force: true });
fs.mkdirSync(path.join(dist, 'assets'), { recursive: true });

const PAGINAS = {};
for (const i of IDIOMAS) {
  usar(i);
  const prefijo = i === 'en' ? 'en/' : '';
  PAGINAS[`${prefijo}index.html`] = inicio();
  PAGINAS[`${prefijo}404.html`] = noEncontrada();
  for (const nombre of Object.keys(DOCS)) {
    PAGINAS[`${DOCS[nombre][i].ruta.slice(1)}index.html`] = nombre === 'documentacion' ? paginaMapa() : DOCS[nombre].capitulo ? paginaCapitulo(nombre) : paginaDe(nombre);
  }
}
for (const [ruta, html] of Object.entries(PAGINAS)) {
  const destino = path.join(dist, ruta);
  fs.mkdirSync(path.dirname(destino), { recursive: true });
  fs.writeFileSync(destino, html);
}

// --- The same pages as Markdown, and llms.txt, their index for agents ----------------------
for (const nombre of Object.keys(DOCS)) {
  for (const i of IDIOMAS) {
    const { ruta, descripcion } = DOCS[nombre][i];
    const $ = cheerio.load(PAGINAS[`${ruta.slice(1)}index.html`]);
    // Links lead to the Markdown copy of a page when there is one, and always to a full address.
    const enlace = (href) => {
      if (href.startsWith('#')) return `${aMd(ruta)}${href}`;
      if (!href.startsWith('/')) return href;
      const [camino, ancla] = href.split('#');
      return RUTAS_MD.has(camino) ? `${aMd(camino)}${ancla ? `#${ancla}` : ''}` : `${DOMINIO}${href}`;
    };
    const md = `# ${norm($('main h1').first().text())}\n\n> ${descripcion}\n\n${aMarkdown($.html($('main .pagina-cabeza > p')) + $('article.lectura').html(), enlace)}\n`;
    fs.writeFileSync(path.join(dist, `${ruta.slice(1)}index.html.md`), md);
  }
}
usar('en');
const resumen = texto('manifiesto', 'Gate-Driven Development (GDD) is a way').split(/(?<=\.) /).slice(0, 2).join(' ');
const linea = (nombre) => `- [${DOCS[nombre].en.menu}](${aMd(DOCS[nombre].en.ruta)}): ${DOCS[nombre].en.descripcion}`;
const partes = Object.keys(PARTES).map((p) => `## ${PARTES[p].en}\n\n${CAPITULOS.filter((k) => DOCS[k].capitulo.parte === p).map(linea).join('\n')}`);
fs.writeFileSync(path.join(dist, 'llms.txt'), `# Gate-Driven Development (GDD)

> ${resumen}

GDD comes with eight skills for Claude Code, Codex and Antigravity, which bring a small command of their own, the layer; it can also be applied by hand. Skills and layer ${SKILLS}, contract ${CONTRATO}, Manifesto ${VERSION_TXT}, ${ESTADO_TXT}. When the documentation and the Manifesto disagree, the Manifesto is the reference.

## The reference

${linea('manifiesto')}
${linea('documentacion')}

${partes.join('\n\n')}

## Practice

${linea('taller')}

## Optional

${linea('articulo')}
- [Gate-Driven Development en español](${aMd(DOCS.documentacion.es.ruta)}): the same documentation in Spanish, the original; each page links to the rest.
`);

// Kimen's tokens (its default theme, both schemes), then the site's theme, Plano, and the site.
fs.copyFileSync(path.join(raiz, 'node_modules/@kimen/tokens/dist/css/tokens.css'), path.join(dist, 'assets/kimen.css'));
fs.writeFileSync(path.join(dist, 'assets/site.css'), leer('src/css/tema-gdd.css') + leer('src/css/site.css') + leer('src/css/documentacion.css') + (TUTORIAL_CSS ? `\n${leer('src/css/tutorial.css')}` : ''));
// The Kimen components the pages use, in one module (src/js/kimen.js).
esbuild.buildSync({ entryPoints: [path.join(raiz, 'src/js/kimen.js')], bundle: true, minify: true, format: 'esm', target: 'es2022', outfile: path.join(dist, 'assets/kimen.js'), legalComments: 'eof', logLevel: 'warning',
  // Stencil's lazy loader looks for entry chunks this build never makes: each component is defined directly.
  logOverride: { 'empty-glob': 'silent' } });
fs.copyFileSync(path.join(raiz, 'src/css/portada.css'), path.join(dist, 'assets/portada.css'));
for (const js of ['plano.js', 'portada.js', 'sitio.js', ...(TUTORIAL_JS ? ['tutorial.js'] : [])]) {
  fs.copyFileSync(path.join(raiz, 'src/js', js), path.join(dist, 'assets', js));
}
const fuente = (paquete, fichero, salida) => fs.copyFileSync(path.join(raiz, 'node_modules', paquete, 'files', fichero), path.join(dist, 'assets', salida));
fuente('@fontsource-variable/archivo', 'archivo-latin-standard-normal.woff2', 'archivo.woff2');
fuente('@fontsource-variable/archivo', 'archivo-latin-standard-italic.woff2', 'archivo-italic.woff2');
fuente('@fontsource-variable/jetbrains-mono', 'jetbrains-mono-latin-wght-normal.woff2', 'jetbrains-mono.woff2');

// The PDF is the manifesto itself, at the version it carries, in each language.
for (const i of IDIOMAS) {
  usar(i);
  const origen = path.join(raiz, `${carpeta()}/${PDF}`);
  if (fs.existsSync(origen)) fs.copyFileSync(origen, path.join(dist, PDF));
}

for (const imagen of ['og.png', 'og-en.png']) {
  const og = path.join(raiz, 'src/img', imagen);
  if (fs.existsSync(og)) fs.copyFileSync(og, path.join(dist, imagen));
}

fs.writeFileSync(path.join(dist, 'favicon.svg'), `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><style>svg{color:#10192a}@media (prefers-color-scheme:dark){svg{color:#f3f7fc}}</style><path d="M1 21.500h5M18.500 21.500H23" stroke="currentColor" stroke-width="3" fill="none"/><path d="M6 21.500V4" stroke="currentColor" stroke-width="2.200" fill="none"/><path d="M6 4a15 17.500 0 0 1 12.500 17.500" stroke="currentColor" stroke-width="1.400" fill="none"/></svg>\n`);
fs.writeFileSync(path.join(dist, 'robots.txt'), `User-agent: *\nAllow: /\n\nSitemap: ${DOMINIO}/sitemap.xml\n`);
const parejas = [['/', '/en/'], ...ORDEN.map((k) => [DOCS[k].es.ruta, DOCS[k].en.ruta])];
const entrada = (loc, es, en) => `  <url><loc>${DOMINIO}${loc}</loc><lastmod>${FECHA}</lastmod><xhtml:link rel="alternate" hreflang="es" href="${DOMINIO}${es}"/><xhtml:link rel="alternate" hreflang="en" href="${DOMINIO}${en}"/><xhtml:link rel="alternate" hreflang="x-default" href="${DOMINIO}${en}"/></url>`;
fs.writeFileSync(path.join(dist, 'sitemap.xml'), `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">\n${parejas.flatMap(([es, en]) => [entrada(es, es, en), entrada(en, es, en)]).join('\n')}\n</urlset>\n`);
fs.writeFileSync(path.join(dist, '_headers'), `/assets/*\n  Cache-Control: public, max-age=31536000, immutable\n\n/*.md\n  X-Robots-Tag: noindex\n\n/*\n  X-Content-Type-Options: nosniff\n  Referrer-Policy: strict-origin-when-cross-origin\n`);
// Old addresses lead to what replaced them, with and without the final slash.
fs.writeFileSync(path.join(dist, '_redirects'), `${REDIRECCIONES.flatMap(([de, a]) => [`${de}/ ${a} 301`, `${de} ${a} 301`]).join('\n')}\n/manifiesto-gdd-v0.3.pdf /manifiesto-gdd-v0.11.pdf 301\n/manifiesto-gdd-v0.4.pdf /manifiesto-gdd-v0.11.pdf 301\n/manifiesto-gdd-v0.5.pdf /manifiesto-gdd-v0.11.pdf 301\n/manifiesto-gdd-v0.6.pdf /manifiesto-gdd-v0.11.pdf 301\n/manifiesto-gdd-v0.7.pdf /manifiesto-gdd-v0.11.pdf 301\n/manifiesto-gdd-v0.8.pdf /manifiesto-gdd-v0.11.pdf 301\n/manifesto-gdd-v0.4.pdf /manifesto-gdd-v0.11.pdf 301\n/manifesto-gdd-v0.5.pdf /manifesto-gdd-v0.11.pdf 301\n/manifesto-gdd-v0.6.pdf /manifesto-gdd-v0.11.pdf 301\n/manifesto-gdd-v0.7.pdf /manifesto-gdd-v0.11.pdf 301\n/manifesto-gdd-v0.8.pdf /manifesto-gdd-v0.11.pdf 301\n/manifiesto-gdd-v0.9.pdf /manifiesto-gdd-v0.11.pdf 301\n/manifesto-gdd-v0.9.pdf /manifesto-gdd-v0.11.pdf 301\n/manifiesto-gdd-v0.10.pdf /manifiesto-gdd-v0.11.pdf 301\n/manifesto-gdd-v0.10.pdf /manifesto-gdd-v0.11.pdf 301\n`);

console.log('Listo:', Object.keys(PAGINAS).length, 'páginas en dist/');
