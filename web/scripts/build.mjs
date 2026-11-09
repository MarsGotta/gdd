// Builds the static site in dist/ from the text of the documents, kept in content/ as they are
// published, so the wording is never retyped here: if a block is renamed, the build stops and says which.
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import * as cheerio from 'cheerio';
import esbuild from 'esbuild';
import { sprite, marca, icono, plano, escena, dibujoTodo, dibujoNada, dibujoSolo, anatomia } from './partes.mjs';
import { PARTES, DOCS, CAPITULOS, TOTAL, ORDEN, MENU, PIE, REDIRECCIONES } from './estructura.mjs';

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
  .update(['src/css/site.css', 'src/css/tutorial.css', 'src/js/plano.js', 'src/js/tutorial.js', 'src/js/sitio.js'].filter((f) => fs.existsSync(path.join(raiz, f))).map(leer).join(''))
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
  const $ = cheerio.load(leer(`${carpeta()}/${DOCS[nombre].fichero}`), null, false);

  $('code[data-fuente]').each((_, c) => {
    const f = $(c).attr('data-fuente');
    $(c).removeAttr('data-fuente').text(leer(`${carpeta()}/codigo/${f}`).replace(/\n$/, ''));
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
  $('pre').attr('tabindex', '0');
  if (!copiar) $('pre').removeAttr('data-copiar');

  let salida = $.html();
  // On the Spanish pages, the sentences quoted in English say so.
  if (idioma === 'es') salida = salida
    .replaceAll('"Humans steer. Agents execute."', '"<span lang="en">Humans steer. Agents execute.</span>"')
    .replaceAll('"corrections are cheap, and waiting is expensive"', '"<span lang="en">corrections are cheap, and waiting is expensive</span>"')
    .replaceAll('"the harness is most needed where it is hardest to build"', '"<span lang="en">the harness is most needed where it is hardest to build</span>"');
  // Number and unit stay together. Code stays as written.
  salida = salida.split(/(<pre[\s\S]*?<\/pre>|<svg[\s\S]*?<\/svg>)/).map((parte) => (/^<(pre|svg)/.test(parte) ? parte : parte.replace(/(\d) (%|€)/g, '$1&nbsp;$2'))).join('');
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
const TMINUS = 'https://github.com/MarsGotta/tminus';
const IMAGEN = () => tr('og.png', 'og-en.png');

// The same page in the other language.
const otraRuta = (ruta) => {
  const otro = tr('en', 'es');
  const k = Object.keys(DOCS).find((x) => DOCS[x][idioma].ruta === ruta);
  return k ? DOCS[k][otro].ruta : tr('/en/', '/');
};

const cabecera = (ruta) => `<header class="cabecera"><div class="contenedor cabecera__dentro">
  <a class="logo" href="${rutaInicio()}"${ruta === rutaInicio() ? ' aria-current="page"' : ''}>${marca()}<span>Gate-Driven Development</span></a>
  <nav class="navegacion" aria-label="${tr('Principal', 'Main')}"><ul>
    ${MENU.map((k) => {
    const r = DOCS[k][idioma].ruta;
    const dentro = k === 'documentacion' && ruta !== r && ruta.startsWith(r);
    return `<li><a href="${r}"${ruta === r ? ' aria-current="page"' : dentro ? ' aria-current="true"' : ''}>${DOCS[k][idioma].menu}</a></li>`;
  }).join('\n    ')}
  </ul></nav>
  <a class="idioma" href="${otraRuta(ruta)}" hreflang="${tr('en', 'es')}" lang="${tr('en', 'es')}" aria-label="${tr('Read this page in English', 'Lee esta página en español')}">${tr('English', 'Español')}</a>
  <button type="button" class="tema" data-tema-boton title="${tr('Cambiar entre claro y oscuro', 'Switch between light and dark')}">${icono('tema')}<span class="tema__a-oscuro">${tr('Modo oscuro', 'Dark mode')}</span><span class="tema__a-claro">${tr('Modo claro', 'Light mode')}</span></button>
</div></header>`;

const pie = () => `<footer class="pie"><div class="contenedor pie__dentro">
  <div>
    <a class="logo" href="${rutaInicio()}">${marca()}<span>Gate-Driven Development</span></a>
    <p>${VERSION_TXT}, ${ESTADO_TXT}. ${FIRMA}.</p>
    <p>${frase('manifiesto', tr('Este manifiesto es la referencia.', 'This manifesto is the reference.'))}</p>
  </div>
  <div>
    <p>${texto('manifiesto', tr('Esta versión todavía no trae datos propios.', "This version doesn't bring its own data yet."))}</p>
    <p>${tr('T-minus, el motor que hace cumplir GDD, está en desarrollo y todavía no cubre todas las garantías del núcleo.', 'T-minus, the engine that enforces GDD, is in development and does not cover all the guarantees of the core yet.')}</p>
  </div>
  <div>
    <p><a href="/${PDF}">${tr('Descargar el manifiesto en PDF', 'Download the manifesto as a PDF')}</a> (${VERSION_TXT}, ${tr('solo el manifiesto', 'manifesto only')}).</p>
    <p><a href="${TMINUS}">${tr('T-minus, en GitHub', 'T-minus on GitHub')}</a></p>
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

const documento = ({ ruta, titulo, descripcion, cuerpo, scripts = [], indexable = true, enlacesCabeza = '' }) => {
  const enEs = idioma === 'es' ? ruta : otraRuta(ruta);
  const enEn = idioma === 'en' ? ruta : otraRuta(ruta);
  const buscadores = indexable
    ? `<link rel="canonical" href="${DOMINIO}${ruta}">
<link rel="alternate" hreflang="es" href="${DOMINIO}${enEs}">
<link rel="alternate" hreflang="en" href="${DOMINIO}${enEn}">
<link rel="alternate" hreflang="x-default" href="${DOMINIO}${enEn}">
${datos({ ruta, titulo, descripcion, inicial: ruta === rutaInicio() })}`
    : '<meta name="robots" content="noindex">';
  return `<!doctype html>
<html lang="${idioma}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<script>document.documentElement.classList.add('js');try{var t=localStorage.getItem('tema');if(t==='claro'||t==='oscuro')document.documentElement.dataset.tema=t}catch(e){}</script>
<title>${titulo}</title>
<meta name="description" content="${descripcion}">
<meta name="author" content="Marcela Gotta">
<meta name="theme-color" content="#ffffff" media="(prefers-color-scheme: light)">
<meta name="theme-color" content="#15181b" media="(prefers-color-scheme: dark)">
${buscadores}
${enlacesCabeza}
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<link rel="preload" href="/assets/plex-sans.woff2" as="font" type="font/woff2" crossorigin>
<link rel="stylesheet" href="/assets/site.css?v=${VERSION}">
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
${cabecera(ruta)}
<main id="contenido">
${cuerpo}
</main>
${pie()}
<script src="/assets/sitio.js?v=${VERSION}" defer></script>
${scripts.map((s) => `<script src="/assets/${s}?v=${VERSION}" defer></script>`).join('\n')}
</body>
</html>
`;
};

const muro = '<div class="muro" aria-hidden="true"></div>';
const seccion = (id, titulo, cuerpo) => `<section class="seccion contenedor" aria-labelledby="${id}">
  <div class="seccion__cab"><h2 id="${id}">${titulo}</h2></div>
  <div class="seccion__cuerpo">${cuerpo}</div>
</section>`;

const indice = (entradas) => `<nav class="indice" aria-label="${tr('En esta página', 'On this page')}">
  <details>
    <summary>${tr('En esta página', 'On this page')}</summary>
    <ol>${entradas.map(([id, t]) => `<li><a href="#${id}">${t}</a></li>`).join('')}</ol>
  </details>
</nav>`;

// --- The home page ------------------------------------------------------------------------
const puente = (id, t) => `<a class="puente" href="#${id}">${t}${icono('abajo', 'flecha')}</a>`;

const inicio = () => {
  const [promesa, ...subtitulo] = texto('manifiesto', tr('Añade puertas a tu proyecto', 'Add gates to your project')).split(/(?<=\.) /);
  const [p0, p1, p2, p3] = texto('manifiesto', tr('Los agentes escriben rápido', 'Agents write fast')).split(/(?<=\.) /);
  const g = 'la-idea';

  const portada = `<section class="heroe contenedor">
    <div>
      <h1>${promesa}</h1>
      <p class="heroe__sub">${subtitulo.join(' ')}</p>
      <div class="salidas">
        <a class="boton" href="#${tr('el-problema', 'the-problem')}">${tr('Cómo funciona', 'How it works')}${icono('abajo', 'flecha')}</a>
        <a href="${DOCS['empieza-hoy'][idioma].ruta}">${tr('Monta tu primera puerta', 'Build your first gate')}</a>
      </div>
    </div>
    <div class="heroe__dibujo">
      ${escena(tr)}
      <div class="heroe__pie">
        <ol class="relato">
          <li class="relato__1">${tr('El agente dice: "hecho". Decirlo no abre nada.', 'The agent says "done." Saying it opens nothing.')}</li>
          <li class="relato__2">${tr('La puerta lo comprueba. No pasa, y vuelve.', 'The gate checks it. It fails, and goes back.')}</li>
          <li class="relato__3">${tr('Lo arregla. Ahora pasa la comprobación.', 'It fixes it. Now it passes the check.')}</li>
          <li class="relato__4">${tr('La puerta se abre, sin que hayas estado encima.', 'The gate opens, without you standing over it.')}</li>
        </ol>
        <button type="button" class="pausa" data-heroe-pausa>${tr('Pausar', 'Pause')}</button>
      </div>
    </div>
  </section>`;

  const problema = seccion(tr('el-problema', 'the-problem'), tr('El problema', 'The problem'), `
    <p class="grande">${p0}</p>
    <div class="tres">
      <figure>${dibujoTodo}<figcaption>${p1}</figcaption></figure>
      <figure>${dibujoNada}<figcaption>${p2}</figcaption></figure>
      <figure>${dibujoSolo}<figcaption>${p3}</figcaption></figure>
    </div>
    <p class="declaracion">${interior('manifiesto', tr('Al final es una bestia indomable', 'In the end it is an untamable beast'))}</p>
    ${puente(tr('la-idea', 'the-idea'), tr('Cómo se ponen esas puertas', 'How to set up those gates'))}
  `);

  const cuatro = [
    ['persona', tr('Antes de que el agente empiece, decides qué tiene que ser verdad cuando termine.', 'Before the agent starts, you decide what has to be true when it finishes.')],
    ['cerradura', tr('Lo demuestra algo que ya usas y que él no puede tocar: un test, un linter, una revisión.', "It's proven by something you already use and it can't touch: a test, a linter, a review.")],
    [null, tr('Si el cambio es una errata, casi ni te enteras. Si toca el cobro, le pides más, y puede que te llame.', 'If the change is a typo, you barely notice. If it touches billing, you ask for more, and it may call you.')],
    ['llave', tr('Y cuando algo se cuela, añades lo que lo habría parado, y por ahí ya no vuelve a entrar.', "And when something slips through, you add what would have stopped it, and it won't get in that way again.")],
  ];
  const base = idioma === 'es'
    ? interior('manifiesto', 'Esa es la base de GDD:').replace(/^Esa es la base de GDD: /, '').replace('no desarrollamos', 'No desarrollamos')
    : interior('manifiesto', 'That is the basis of GDD:').replace(/^That is the basis of GDD: /, '').replace("we don't", "We don't");
  const idea = seccion(tr('la-idea', 'the-idea'), tr('La idea', 'The idea'), `
    <p class="grande">${tr('Funciona así.', 'It works like this.')}</p>
    <ul class="cuatro">
      ${cuatro.map(([ic, t]) => `<li>${ic ? icono(ic, ic === 'persona' ? 'icono--persona' : '') : '<span class="cuatro__lamparas" aria-hidden="true"><span class="lampara lampara--rojo"></span><span class="lampara lampara--amarillo"></span><span class="lampara lampara--verde"></span></span>'}<span>${t}</span></li>`).join('\n')}
    </ul>
    <p class="declaracion">${base}</p>
    ${puente(tr('el-recorrido', 'the-walkthrough'), tr('Míralo en marcha', 'See it in action'))}
  `);

  const recorrido = `<section class="recorrido contenedor" aria-labelledby="${tr('el-recorrido', 'the-walkthrough')}">
    <div class="recorrido__cabeza">
      <h2 id="${tr('el-recorrido', 'the-walkthrough')}">${tr('El recorrido', 'The walkthrough')}</h2>
      <p class="grande" id="eleccion-titulo">${tr('Elige qué le pides al agente y mira qué cambia.', 'Choose what you ask the agent and see what changes.')}</p>
    </div>
    ${plano(tr, {
      pie: `${tr('Tres puertas de ejemplo: en tu proyecto habrá las que hagan falta.', 'Three example gates: your project will have the ones it needs.')} ${interior('manifiesto', tr('Lo que no se puede deshacer nunca es verde.', "What can't be undone is never green."))}`,
      tocaVerde: interior(g, tr('Nada: pides el cambio', 'Nothing: you ask for the change')),
      tocaAmarillo: interior(g, tr('Leer unos pocos escenarios', 'Read a few scenarios')),
      tocaRojo: interior(g, tr('Leer los escenarios y dar tu aprobación', 'Read the scenarios and give your approval')),
    })}
    ${puente(tr('donde-seguir', 'where-to-go-next'), tr('Pruébalo en tu proyecto', 'Try it on your project'))}
  </section>`;

  const caminos = [
    ['manos-a-la-obra', DOCS['manos-a-la-obra'][idioma].menu, tr('Preparamos cuatro puertas y revisamos cuatro cambios, de menos a más riesgo', 'We prepare four gates and review four changes, from less to more risk')],
    ['documentacion', DOCS.documentacion[idioma].menu, tr('Empieza por la idea y sigue hasta donde te haga falta', 'Start with the idea and go as far as you need')],
    ['manifiesto', tr('Manifiesto', 'Manifesto'), tr('Una regla, siete principios y cuatro momentos, en una página', 'One rule, seven principles and four moments, on one page')],
  ];
  const seguir = seccion(tr('donde-seguir', 'where-to-go-next'), tr('Por dónde seguir', 'Where to go next'), `
    <p class="grande">${tr('Puedes empezar hoy con lo que ya tienes.', 'You can start today with what you already have.')}</p>
    <p class="salidas"><a class="boton" href="${DOCS['manos-a-la-obra'][idioma].ruta}">${tr('Manos a la obra', 'Get hands on')}</a><a href="${DOCS['empieza-hoy'][idioma].ruta}">${tr('Monta tu primera puerta', 'Build your first gate')}</a></p>
    <div class="caminos">
      ${caminos.map(([k, t, d]) => `<a class="camino" href="${DOCS[k][idioma].ruta}"><strong>${t}</strong><span>${d}</span></a>`).join('\n')}
    </div>
    <p class="nota-version">GDD ${VERSION_TXT}, ${ESTADO_TXT}. ${frase('manifiesto', tr('Esta versión todavía no trae datos propios', "This version doesn't bring its own data yet"))} ${tr('Su motor, <span class="sin-corte">T-minus</span>, está en desarrollo.', 'Its engine, <span class="sin-corte">T-minus</span>, is in development.')}</p>
  `);

  return documento({
    ruta: rutaInicio(),
    titulo: tr('Gate-Driven Development: puertas para agentes de IA', 'Gate-Driven Development: quality gates for AI agents'),
    descripcion: frase('manifiesto', tr('Gate-Driven Development (GDD) es una forma', 'Gate-Driven Development (GDD) is a way')),
    cuerpo: [portada, muro, problema, muro, idea, muro, recorrido, muro, seguir].join('\n'),
    scripts: ['plano.js'],
  });
};

// --- The pages of the documents -----------------------------------------------------------
// Shared by every page of text: the body of the document, dressed, and its index of sections.
const cuerpoDe = (nombre) => {
  const $ = T[nombre];
  const copia = cheerio.load($.html(), null, false);
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
  const cuerpo = vestir(copia.html(), { copiar: true });
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

// The map of the documentation: a short text and the chapters in four parts.
const paginaMapa = () => {
  const doc = DOCS.documentacion[idioma];
  const { h1, cuerpo } = cuerpoDe('documentacion');
  return documento({
    ruta: doc.ruta,
    titulo: doc.titulo,
    descripcion: doc.descripcion,
    cuerpo: `<div class="pagina-cabeza contenedor"><h1>${h1}</h1></div>
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

const vecino = (clave, sentido, n) => {
  const d = DOCS[clave];
  const titulo = d.capitulo ? `${d.capitulo.n}. ${d[idioma].menu}` : d[idioma].menu;
  return `<a class="vecino vecino--${sentido}" rel="${sentido === 'anterior' ? 'prev' : 'next'}" href="${d[idioma].ruta}"><span>${sentido === 'anterior' ? tr('Anterior', 'Previous') : tr('Siguiente', 'Next')}</span><strong>${titulo}</strong></a>`;
};

const paginaCapitulo = (nombre) => {
  const doc = DOCS[nombre][idioma];
  const { n, parte } = DOCS[nombre].capitulo;
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
  <header class="capitulo__cabeza"><p class="capitulo__etiqueta">${tr(`Capítulo ${n} de ${TOTAL}`, `Chapter ${n} of ${TOTAL}`)} · ${PARTES[parte][idioma]}</p><h1>${h1}</h1></header>
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

fs.writeFileSync(path.join(dist, 'assets/site.css'), leer('src/css/site.css') + (TUTORIAL_CSS ? `\n${leer('src/css/tutorial.css')}` : ''));
for (const js of ['plano.js', 'sitio.js', ...(TUTORIAL_JS ? ['tutorial.js'] : [])]) {
  fs.copyFileSync(path.join(raiz, 'src/js', js), path.join(dist, 'assets', js));
}
const fuente = (paquete, fichero, salida) => fs.copyFileSync(path.join(raiz, 'node_modules', paquete, 'files', fichero), path.join(dist, 'assets', salida));
fuente('@fontsource-variable/ibm-plex-sans', 'ibm-plex-sans-latin-wght-normal.woff2', 'plex-sans.woff2');
fuente('@fontsource-variable/ibm-plex-sans', 'ibm-plex-sans-latin-wght-italic.woff2', 'plex-sans-italic.woff2');
fuente('@fontsource/ibm-plex-mono', 'ibm-plex-mono-latin-400-normal.woff2', 'plex-mono.woff2');

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

fs.writeFileSync(path.join(dist, 'favicon.svg'), `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><style>svg{color:#15181b}@media (prefers-color-scheme:dark){svg{color:#eef0f1}}</style><path d="M1 21.500h5M18.500 21.500H23" stroke="currentColor" stroke-width="3" fill="none"/><path d="M6 21.500V4" stroke="currentColor" stroke-width="2.200" fill="none"/><path d="M6 4a15 17.500 0 0 1 12.500 17.500" stroke="currentColor" stroke-width="1.400" fill="none"/></svg>\n`);
fs.writeFileSync(path.join(dist, 'robots.txt'), `User-agent: *\nAllow: /\n\nSitemap: ${DOMINIO}/sitemap.xml\n`);
const parejas = [['/', '/en/'], ...ORDEN.map((k) => [DOCS[k].es.ruta, DOCS[k].en.ruta])];
const entrada = (loc, es, en) => `  <url><loc>${DOMINIO}${loc}</loc><lastmod>${FECHA}</lastmod><xhtml:link rel="alternate" hreflang="es" href="${DOMINIO}${es}"/><xhtml:link rel="alternate" hreflang="en" href="${DOMINIO}${en}"/><xhtml:link rel="alternate" hreflang="x-default" href="${DOMINIO}${en}"/></url>`;
fs.writeFileSync(path.join(dist, 'sitemap.xml'), `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">\n${parejas.flatMap(([es, en]) => [entrada(es, es, en), entrada(en, es, en)]).join('\n')}\n</urlset>\n`);
fs.writeFileSync(path.join(dist, '_headers'), `/assets/*\n  Cache-Control: public, max-age=31536000, immutable\n\n/*\n  X-Content-Type-Options: nosniff\n  Referrer-Policy: strict-origin-when-cross-origin\n`);
// Old addresses lead to what replaced them, with and without the final slash.
fs.writeFileSync(path.join(dist, '_redirects'), `${REDIRECCIONES.flatMap(([de, a]) => [`${de}/ ${a} 301`, `${de} ${a} 301`]).join('\n')}\n/manifiesto-gdd-v0.3.pdf /manifiesto-gdd-v0.7.pdf 301\n/manifiesto-gdd-v0.4.pdf /manifiesto-gdd-v0.7.pdf 301\n/manifiesto-gdd-v0.5.pdf /manifiesto-gdd-v0.7.pdf 301\n/manifiesto-gdd-v0.6.pdf /manifiesto-gdd-v0.7.pdf 301\n/manifesto-gdd-v0.4.pdf /manifesto-gdd-v0.7.pdf 301\n/manifesto-gdd-v0.5.pdf /manifesto-gdd-v0.7.pdf 301\n/manifesto-gdd-v0.6.pdf /manifesto-gdd-v0.7.pdf 301\n`);

console.log('Listo:', Object.keys(PAGINAS).length, 'páginas en dist/');
