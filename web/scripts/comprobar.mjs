// Checks on the built site, run before every publication:
// each page carries the full text of its document and nothing else, the words on screen come from
// the documents, internal links lead somewhere, nothing old or forbidden slipped in, and nothing
// promises what does not exist yet.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import zlib from 'node:zlib';
import * as cheerio from 'cheerio';
import { compara } from './paridad.mjs';
import { DOCS as ESTRUCTURA, CAPITULOS, TOTAL, REDIRECCIONES } from './estructura.mjs';

const raiz = path.join(path.dirname(fileURLToPath(import.meta.url)), '..');
const dist = path.join(raiz, 'dist');
const norm = (s) => s.replace(/[  ]/g, ' ').replace(/\s+/g, ' ').trim();
const fallos = [];

const paginas = {};
const recorrer = (dir) => {
  for (const f of fs.readdirSync(dir)) {
    const p = path.join(dir, f);
    if (fs.statSync(p).isDirectory()) recorrer(p);
    else if (f.endsWith('.html')) paginas[`/${path.relative(dist, p)}`] = fs.readFileSync(p, 'utf8');
  }
};
recorrer(dist);

// The documents and the page each one lives on, in each language (the list is in estructura.mjs).
const DOCS = {
  es: Object.fromEntries(Object.entries(ESTRUCTURA).map(([k, d]) => [k, `${d.es.ruta}index.html`])),
  en: Object.fromEntries(Object.entries(ESTRUCTURA).map(([k, d]) => [k, `${d.en.ruta}index.html`])),
};
const IDIOMAS = Object.keys(DOCS);
const carpeta = (i) => (i === 'en' ? 'content/en' : 'content');
const rutaDe = (i, nombre) => DOCS[i][nombre].replace('index.html', '');
const BLOQUES = 'h1, h2, h3, p, li, th, td, figcaption, pre';

// Source of a document, with the code files its blocks point at.
const fuenteDe = (nombre, i = 'es') => {
  const $ = cheerio.load(fs.readFileSync(path.join(raiz, `${carpeta(i)}/${ESTRUCTURA[nombre].fichero}`), 'utf8'), null, false);
  $('code[data-fuente]').each((_, c) => {
    $(c).text(fs.readFileSync(path.join(raiz, `${carpeta(i)}/codigo`, $(c).attr('data-fuente')), 'utf8').replace(/\n$/, ''));
  });
  return $;
};
const bloquesDe = ($) => $(BLOQUES).map((_, n) => norm($(n).text())).get().filter(Boolean);

// 1. Every block of each document is on its page, word for word, and the page adds no block of its own.
const textoDeTodos = [];
let comprobados = 0;
for (const [i, nombre, ruta] of IDIOMAS.flatMap((x) => Object.entries(DOCS[x]).map(([n, r]) => [x, n, r]))) {
  const fuente = fuenteDe(nombre, i);
  const page = cheerio.load(paginas[ruta]);
  const principal = norm(page('main').text());
  const fuenteTexto = norm(fuente.root().text());
  textoDeTodos.push(fuenteTexto);

  for (const b of bloquesDe(fuente)) {
    if (b === 'Marcela Gotta' || /^v\d+\.\d+ · /.test(b)) continue; // the byline and the version go in the page head
    comprobados += 1;
    // The number of a step is drawn by the page, not written in it.
    if (!principal.includes(b) && !principal.includes(b.replace(/^\d+\. /, ''))) fallos.push(`Falta en ${ruta}: ${b.slice(0, 100)}`);
  }

  const articulo = page('article.lectura');
  // The one piece a page adds to its document are the pieces of the hands-on chapter (.tutorial), made from its own files.
  for (const b of articulo.find(BLOQUES).filter((_, n) => !page(n).closest('.tutorial').length).map((_, n) => norm(page(n).text())).get().filter(Boolean)) {
    if (!fuenteTexto.includes(b)) fallos.push(`Sobra en ${ruta}, no está en el documento: ${b.slice(0, 100)}`);
  }

  // Mentions between documents lead to the page of the document they name.
  const mencionesDoc = fuente('a[href^="doc:"]').map((_, a) => {
    const [clave, ancla] = fuente(a).attr('href').slice(4).split('#');
    return `${rutaDe(i, clave)}${ancla ? `#${ancla}` : ''}|${norm(fuente(a).text())}`;
  }).get();
  const enlacesPagina = articulo.find('a[href^="/"]').map((_, a) => `${page(a).attr('href')}|${norm(page(a).text())}`).get();
  for (const m of mencionesDoc) {
    const i = enlacesPagina.indexOf(m);
    if (i < 0) fallos.push(`${ruta}: la mención ${m} no lleva a su página`);
    else enlacesPagina.splice(i, 1);
  }
  // Every external link of the document is kept.
  const externosFuente = fuente('a[href^="http"]').map((_, a) => fuente(a).attr('href')).get().sort().join('\n');
  const externosPagina = articulo.find('a[href^="http"]').map((_, a) => page(a).attr('href')).get().sort().join('\n');
  if (externosFuente !== externosPagina) fallos.push(`${ruta}: los enlaces externos no coinciden con los del documento`);
}
console.log(`Documentos: ${comprobados} bloques comprobados en ${IDIOMAS.flatMap((x) => Object.keys(DOCS[x])).length} páginas, y ninguno de más.`);
const todo = norm(textoDeTodos.join(' '));

const TOPE_HEROE = 40;
const TOPE_INICIO = 300;

// 2. The words of the floor plan and the demo come from the documents, apart from a short list.
const propias = new Set([
  'Poco riesgo: algunas puertas quedan abiertas.',
  'Low risk: some gates stay open.',
  'El agente dice "hecho". Decirlo no abre nada.',
  'The agent says "done." Saying it opens nothing.',
  'Sale, y a ti no te ha llamado nadie.',
  'It ships, and nobody called you.',
  'Antes de empezar, te llama: lees unos pocos escenarios.',
  'Before it starts, it calls you: you read a few scenarios.',
  'Pasa, y dice "hecho". Aquí la puerta suma otra comprobación: un vigilante.',
  'It gets through and says "done." Here the gate adds another check: a reviewer.',
  'El vigilante ejecuta el cambio y no funciona. Nadie te llama: el agente lo arregla.',
  'The reviewer runs the change and it does not work. Nobody calls you: the agent fixes it.',
  'Ahora sí funciona, y el vigilante da su aprobación.',
  'Now it works, and the reviewer gives its approval.',
  'Sale. Solo te ha llamado cuando hacía falta tu criterio.',
  'It ships. It only called you when your judgment was needed.',
  'Te llama antes de empezar: lees los escenarios y los corriges.',
  'It calls you before it starts: you read the scenarios and correct them.',
  'Pasa la primera puerta y dice "hecho".',
  'It gets through the first gate and says "done."',
  'La puerta dice que no: falla un test.',
  'The gate says no: a test fails.',
  'Nobody calls you: the error goes back to the agent, and it fixes it.',
  'Como toca el cobro, te llama otra vez: sin tu aprobación no sale.',
  'Because it touches payment, it calls you again: without your approval it does not ship.',
  'Compruebas que se puede deshacer, das tu aprobación y sale.',
  'You check it can be undone, give your approval, and it ships.',
  'Una semana después aparece un error que se coló.',
  'A week later, an error that slipped through turns up.',
  'Añades lo que lo habría parado, y por ahí ya no vuelve a entrar.',
  "You add what would have stopped it, and it won't get in that way again.",
]);
const extraer = (fichero, patron) => {
  const js = fs.readFileSync(path.join(raiz, 'src/js', fichero), 'utf8');
  return [...js.matchAll(patron)].flatMap((m) => [m[1], m[2]]);
};
const lineasPlano = extraer('plano.js', /texto: t\('([^']+)', '([^']+)'\)/g);
for (const linea of lineasPlano) {
  if (!todo.includes(norm(linea).replace(/\.$/, '')) && !propias.has(linea)) fallos.push(`No sale de los documentos: ${linea}`);
}
console.log(`Plano: ${lineasPlano.length} frases comprobadas (${propias.size} redactadas para la web).`);

// 3. Links inside the site lead to a page and an anchor that exist.
const ids = {};
for (const [ruta, html] of Object.entries(paginas)) {
  const $ = cheerio.load(html);
  ids[ruta] = new Set($('[id]').map((_, n) => $(n).attr('id')).get());
}
let enlaces = 0;
const externos = new Set();
for (const [ruta, html] of Object.entries(paginas)) {
  const $ = cheerio.load(html);
  $('a[href]').each((_, a) => {
    const href = $(a).attr('href');
    if (/^https?:/.test(href)) { externos.add(href); return; }
    if (/^doc:/.test(href)) { fallos.push(`${ruta}: mención sin resolver ${href}`); return; }
    enlaces += 1;
    const [camino, ancla] = href.split('#');
    const destino = camino === '' ? ruta : (camino.endsWith('/') ? `${camino}index.html` : camino);
    if (destino.endsWith('.html')) {
      if (!ids[destino]) { fallos.push(`${ruta}: enlace roto ${href}`); return; }
      if (ancla && !ids[destino].has(decodeURIComponent(ancla))) fallos.push(`${ruta}: ancla inexistente ${href}`);
    } else if (!fs.existsSync(path.join(dist, destino))) {
      fallos.push(`${ruta}: fichero inexistente ${href}`);
    }
  });
  $('link[href^="/"], script[src^="/"]').each((_, n) => {
    const u = ($(n).attr('href') || $(n).attr('src')).split('?')[0];
    if (!fs.existsSync(path.join(dist, u))) fallos.push(`${ruta}: recurso inexistente ${u}`);
  });
}
console.log(`Enlaces internos: ${enlaces}. Enlaces externos distintos: ${externos.size}.`);

// 4. Forbidden marks in what a reader can see, in the head and in the code served.
for (const [ruta, html] of Object.entries(paginas)) {
  const $ = cheerio.load(html);
  const cabeza = `${$('title').text()} ${$('meta[name="description"]').attr('content')}`;
  $('script, style').remove();
  $('.tutorial code').remove(); // the piece shows code literally, and a function can return undefined
  const visible = `${$('body').text()} ${cabeza}`;
  // "Claude Code" names an agent the skills install in. "Claude" on its own stays out.
  const sinAgente = visible.replaceAll('Claude Code', '');
  for (const marca of ['—', '–', 'Claude', 'undefined', 'NaN', 'lorem']) {
    if (sinAgente.includes(marca)) fallos.push(`${ruta}: contiene "${marca}"`);
  }
  if (/data-claude/.test(html)) fallos.push(`${ruta}: quedan atributos internos`);
  const attrs = html.match(/(alt|title|aria-label|content|placeholder)="[^"]*"/g) || [];
  for (const a of attrs) if (/—|–|Claude/.test(a)) fallos.push(`${ruta}: atributo con marca prohibida: ${a}`);
}
for (const f of ['assets/site.css', 'assets/plano.js', 'assets/sitio.js', ...(fs.existsSync(path.join(dist, 'assets/tutorial.js')) ? ['assets/tutorial.js'] : [])]) {
  const c = fs.readFileSync(path.join(dist, f), 'utf8');
  if (/Claude|—|–/.test(c)) fallos.push(`${f}: contiene una marca prohibida`);
}

// 5. Nothing from the old single-document version is left, and nothing promises what does not exist.
const viejo = [
  /nueve principios/i, /seis principios/i, /planos? de terceros/i, /\bv0\.3\b/, /cada tarea/i, /en cada tarea/i,
  /todavía no existe/i, /Puerta de integración/, /Aprendizaje/, /feature flag: doy el último OK/,
];
const promesas = [/próximamente/i, /muy pronto/i, /disponible (en|desde)/i, /lanzamiento/i, /\b2027\b/, /\bQ[1-4]\b/, /en las próximas semanas/i, /coming soon/i, /very soon/i, /in the coming weeks/i, /\blaunching\b/i];
for (const [ruta, html] of Object.entries(paginas)) {
  const $ = cheerio.load(html);
  $('script, style').remove();
  const visible = norm(`${$('body').text()} ${$('title').text()} ${$('meta[name="description"]').attr('content')}`);
  for (const re of viejo) if (re.test(visible)) fallos.push(`${ruta}: queda texto de la versión anterior (${re})`);
  for (const re of promesas) if (re.test(visible)) fallos.push(`${ruta}: promete algo con fecha (${re})`);
}
for (const viejo of ['manifiesto-gdd-v0.3.pdf', 'manifiesto-gdd-v0.4.pdf', 'manifesto-gdd-v0.4.pdf', 'manifiesto-gdd-v0.5.pdf', 'manifesto-gdd-v0.5.pdf', 'manifiesto-gdd-v0.6.pdf', 'manifesto-gdd-v0.6.pdf', 'manifiesto-gdd-v0.7.pdf', 'manifesto-gdd-v0.7.pdf', 'manifiesto-gdd-v0.8.pdf', 'manifesto-gdd-v0.8.pdf', 'manifiesto-gdd-v0.9.pdf', 'manifesto-gdd-v0.9.pdf']) {
  if (fs.existsSync(path.join(dist, viejo))) fallos.push(`${viejo} sigue en dist/`);
}

// 5b. Retired words. Each idea has one name across the web. These words came out of the method's
// vocabulary and must not come back in what a reader sees, in either language. The exceptions are
// few and each one says why. A block is a paragraph, list item, cell, heading, caption or code block.
const RETIRADAS = [
  /\bcerradura/i, /\bllaves?\b/i, /(?<!package-)\blocks?\b/i, /\bkeys?\b/i, /\bnormas\b/i, /reglas del juego/i, /rules of the game/i,
  /\bfugas?\b/i, /\bleaks?\b/i, /\btu OK\b/i, /\byour OK\b/i, /\bconstructor/i, /\bcerrajer/i, /\blocksmith/i,
  /\brefuerzos?\b/i, /\breinforcements?\b/i,
];
// Plural "normas" is retired; the singular stays ("norma y garantía", the PCI norm). "package-lock" is a file name.
// "kit" and "firma" belong to the T-minus part only (chapters The engine and The kits).
const SOLO_TMINUS = [/\bkits?\b/i, /\bfirmas?\b/i, /\bsignatures?\b/i];
const PAGINAS_TMINUS = new Set(['el-motor', 'los-kits'].flatMap((k) => IDIOMAS.map((i) => DOCS[i][k])));
// Each exception excuses one word, on some pages, in blocks that match.
const EXCEPCIONES = [
  { ruta: /^\/(en\/docs\/the-idea|documentacion\/la-idea)\//, palabra: /cerradura|llave|lock|key/i, bloque: /escape|puerta|door|sala|room/i, motivo: 'la historia del escape room usa cerraduras y llaves como imagen, no como término del método' },
  { ruta: /./, palabra: /kit/i, bloque: /Spec Kit/, motivo: 'nombre propio de un producto de otra empresa (GitHub Spec Kit)' },
  { ruta: /./, palabra: /kit/i, bloque: /Los kits|The kits/, motivo: 'título del capítulo de T-minus y los enlaces a él' },
];
for (const [ruta, html] of Object.entries(paginas)) {
  const $ = cheerio.load(html);
  $('script, style, nav, header, footer').remove();
  const bloques = [];
  $(BLOQUES).each((_, n) => { if (!$(n).find(BLOQUES).length) bloques.push(norm($(n).text())); });
  bloques.push(norm($('title').text()), norm($('meta[name="description"]').attr('content') || ''));
  $('[aria-label], [alt]').each((_, n) => bloques.push(norm($(n).attr('aria-label') || $(n).attr('alt'))));
  const reglas = [...RETIRADAS, ...(PAGINAS_TMINUS.has(ruta) ? [] : SOLO_TMINUS)];
  for (const b of bloques) {
    for (const re of reglas) {
      const m = b.match(re);
      if (!m) continue;
      if (EXCEPCIONES.some((e) => e.ruta.test(ruta) && e.palabra.test(m[0]) && e.bloque.test(b))) continue;
      fallos.push(`${ruta}: palabra retirada "${m[0]}" en: ${b.slice(0, 90)}`);
    }
  }
}

// Where the links of a PDF lead: its /URI entries, which usually sit inside compressed streams.
// A PDF may write a string between parentheses, with escapes, or in hexadecimal: both are read.
const enlacesDelPdf = (fichero) => {
  const crudo = fs.readFileSync(fichero).toString('latin1');
  const trozos = [crudo];
  for (const m of crudo.matchAll(/stream\r?\n([\s\S]*?)endstream/g)) {
    try { trozos.push(zlib.inflateSync(Buffer.from(m[1], 'latin1')).toString('latin1')); } catch { /* not a deflate stream */ }
  }
  const literal = (s) => s.replace(/\\([0-7]{1,3}|[\s\S])/g, (_, c) => (/^[0-7]/.test(c) ? String.fromCharCode(parseInt(c, 8)) : ({ n: '\n', r: '\r', t: '\t', b: '\b', f: '\f' }[c] ?? c)));
  const hex = (s) => Buffer.from(s.replace(/\s+/g, ''), 'hex').toString('latin1');
  return trozos.flatMap((t) => [
    ...[...t.matchAll(/\/URI\s*\(((?:\\[\s\S]|[^\\)])*)\)/g)].map((m) => literal(m[1])),
    ...[...t.matchAll(/\/URI\s*<([0-9A-Fa-f\s]+)>/g)].map((m) => hex(m[1])),
  ]);
};

// The version and the draft notice are the manifesto's own, and they appear where a reader lands.
for (const [i, inicio, pdf, avisos] of [
  ['es', '/index.html', 'manifiesto-gdd', [['en desarrollo', 'T-minus está en desarrollo'], ['Esta versión todavía no trae datos propios', 'GDD aún no tiene datos propios'], ['Diseñamos las puertas por las que pasarán nuestros agentes', 'la idea central']]],
  ['en', '/en/index.html', 'manifesto-gdd', [['in development', 'T-minus is in development'], ["This version doesn't bring its own data yet", 'GDD has no data of its own yet'], ['We design the gates our agents will go through', 'the central idea']]],
]) {
  const [, version, estado] = norm(fuenteDe('manifiesto', i)('p.meta').eq(1).text()).match(/^(v[\d.]+) · (.+)$/);
  for (const ruta of [inicio, DOCS[i].manifiesto]) {
    if (!norm(cheerio.load(paginas[ruta])('body').text()).includes(`${version}, ${estado}`)) fallos.push(`${ruta}: falta "${version}, ${estado}"`);
  }
  const ficheroPdf = `${pdf}-${version}.pdf`;
  if (!fs.existsSync(path.join(dist, ficheroPdf))) fallos.push(`Falta el PDF de la ${version} en ${i}`);
  else {
    // The PDF is the manifesto page printed from a local server, so every link in it has to be a link of that
    // page as it is published. A link to localhost, or one the page no longer has, means the PDF is stale.
    const $m = cheerio.load(paginas[DOCS[i].manifiesto]);
    const base = `https://gate-driven.dev${rutaDe(i, 'manifiesto')}`;
    const enPagina = new Set($m('a[href]').map((_, a) => new URL($m(a).attr('href'), base).href).get());
    const enPdf = new Set(enlacesDelPdf(path.join(dist, ficheroPdf)));
    if (!enPdf.size) fallos.push(`${ficheroPdf}: no encuentro ningún enlace dentro, así que no puedo comprobarlos`);
    for (const uri of enPdf) {
      if (!enPagina.has(uri)) fallos.push(`${ficheroPdf}: enlaza a ${uri}, que no es un enlace de ${DOCS[i].manifiesto} publicada`);
    }
  }
  const inicioTexto = norm(cheerio.load(paginas[inicio])('main').text());
  for (const [txt, porque] of avisos) {
    if (!inicioTexto.includes(txt)) fallos.push(`${inicio} no dice que ${porque}`);
  }
  console.log(`Versión ${version}, ${estado}; PDF de la ${version} presente.`);
}

// The number of good practices is written out in words in the map and in the chapter's description,
// and it has to be the number the chapter really has.
const EN_LETRAS = {
  es: { 25: 'veinticinco', 26: 'veintiséis', 27: 'veintisiete', 28: 'veintiocho', 29: 'veintinueve', 30: 'treinta', 31: 'treinta y una', 32: 'treinta y dos', 33: 'treinta y tres', 34: 'treinta y cuatro', 35: 'treinta y cinco' },
  en: { 25: 'twenty-five', 26: 'twenty-six', 27: 'twenty-seven', 28: 'twenty-eight', 29: 'twenty-nine', 30: 'thirty', 31: 'thirty-one', 32: 'thirty-two', 33: 'thirty-three', 34: 'thirty-four', 35: 'thirty-five' },
};
for (const i of IDIOMAS) {
  const capitulo = DOCS[i]['buenas-practicas'];
  const $p = cheerio.load(paginas[capitulo]);
  const cuantas = $p('.practica').length;
  const letras = EN_LETRAS[i][cuantas];
  if (!letras) {
    fallos.push(`${capitulo}: tiene ${cuantas} prácticas y EN_LETRAS no sabe escribir ese número`);
    continue;
  }
  const frase = `${letras} ${i === 'es' ? 'prácticas' : 'practices'}`;
  for (const [ruta, texto] of [
    [DOCS[i].documentacion, norm(cheerio.load(paginas[DOCS[i].documentacion])('main').text())],
    [capitulo, $p('meta[name="description"]').attr('content') ?? ''],
  ]) {
    if (!texto.toLowerCase().includes(frase)) fallos.push(`${ruta}: el capítulo tiene ${cuantas} prácticas y aquí no dice "${frase}"`);
  }
  console.log(`Buenas prácticas (${i}): ${cuantas}, como dicen el mapa y la descripción.`);
}

// 6. Every figure on a page carries its source: a link in the same block. Figures of the examples are not data.
const sinFuente = [];
for (const [ruta, html] of Object.entries(paginas)) {
  const $ = cheerio.load(html);
  $('script, style, nav, header, footer').remove();
  $('p, li, td').each((_, n) => {
    const b = $(n);
    if (b.find('p, li, td').length) return; // only the innermost block
    const t = norm(b.text());
    if (/[€]|VERANO|SUMMER|£/.test(t) || b.closest('pre, figure, .colarte, .plano').length) return;
    if (/\d[\d.,]*\s?%|\d{1,3}[.,]\d{3}\b|\b\d+[.,]\d+ (veces|times)|(más de (mil|tres millones|cincuenta))|(more than (a thousand|three million|fifty))/.test(t) && !b.find('a[href^="http"]').length && !b.parent().closest('li').find('a[href^="http"]').length) {
      sinFuente.push(`${ruta}: ${t.slice(0, 110)}`);
    }
  });
}
for (const s of sinFuente) fallos.push(`Cifra sin fuente en el mismo bloque. ${s}`);

// 7. Each page has its own title and one main heading.
const titulos = new Set();
for (const [ruta, html] of Object.entries(paginas)) {
  const $ = cheerio.load(html);
  const t = $('title').text();
  if (titulos.has(t)) fallos.push(`${ruta}: título repetido`);
  titulos.add(t);
  if ($('h1').length !== 1) fallos.push(`${ruta}: tiene ${$('h1').length} h1`);
  const lengua = ruta.startsWith('/en/') ? 'en' : 'es';
  if ($('html').attr('lang') !== lengua) fallos.push(`${ruta}: sin lang="${lengua}"`);
  const rotos = $('img:not([alt])').length;
  if (rotos) fallos.push(`${ruta}: imágenes sin alt`);
}

// 8. The home stays light and guided: a door, three drawings, four sentences and the floor plan.
for (const [ruta, secciones, tienda] of [
  ['/index.html', 'El problema > La idea > El recorrido > Por dónde seguir', /tienda online|VERANO|80 €|pagos\//],
  ['/en/index.html', 'The problem > The idea > The walkthrough > Where to go next', /online store|online shop|SUMMER|€80|payments\//],
]) {
  const $ = cheerio.load(paginas[ruta]);
  const main = $('main');
  if (tienda.test(norm(main.text()))) fallos.push(`${ruta} vuelve a hablar de la tienda`);
  const orden = main.find('h2').map((_, h) => norm($(h).text())).get().join(' > ');
  if (orden !== secciones) fallos.push(`${ruta} cambia de orden: ${orden}`);
  if (!main.find('.heroe .escena').length) fallos.push(`${ruta} ha perdido la escena de la puerta`);
  if (main.find('.tres figure').length !== 3) fallos.push(`${ruta}: el problema ha perdido sus tres dibujos`);
  if (!main.find('#plano').length) fallos.push(`${ruta} ha perdido el recorrido`);
  if (main.find('.puente').length !== 3) fallos.push(`${ruta}: alguna sección ya no lleva a la siguiente`);
  const heroe = norm(main.find('.heroe > div').first().text()).split(' ').length;
  if (heroe > TOPE_HEROE) fallos.push(`${ruta}: el héroe tiene ${heroe} palabras y el tope es ${TOPE_HEROE}`);
  main.find('.plano, .relato').remove();
  const palabras = norm(main.text()).split(' ').length;
  if (palabras > TOPE_INICIO) fallos.push(`${ruta} tiene ${palabras} palabras y el tope es ${TOPE_INICIO}`);
  console.log(`${ruta}: ${heroe} palabras en el héroe y ${palabras} en toda la página, sin contar el recorrido.`);
}

// 9. The English documents keep the structure of the Spanish ones, and nothing Spanish has changed
// since they were translated. If it has: update the translation and then content/en/origen.json.
const origen = JSON.parse(fs.readFileSync(path.join(raiz, 'content/en/origen.json'), 'utf8'));
const huella = (rel) => createHash('sha256').update(fs.readFileSync(path.join(raiz, rel))).digest('hex').slice(0, 16);
for (const nombre of Object.keys(DOCS.es)) fallos.push(...compara(ESTRUCTURA[nombre].fichero.replace(/\.html$/, '')));
for (const [rel, valor] of Object.entries(origen)) {
  if (huella(rel) !== valor) fallos.push(`${rel} ha cambiado desde que se tradujo: actualiza su versión en content/en/ y luego content/en/origen.json`);
}
console.log(`Traducción: ${Object.keys(DOCS.es).length} documentos con la misma estructura y ${Object.keys(origen).length} fuentes al día.`);

// 9b. The documentation is a book: every chapter exists in both languages, in order, with the right Previous and Next,
// and every old address leads to something that exists.
const nombresPagina = (i) => Object.fromEntries(Object.entries(DOCS[i]).map(([k, r]) => [r, k]));
for (const i of IDIOMAS) {
  const mapa = DOCS[i].documentacion;
  CAPITULOS.forEach((k, n) => {
    const ruta = DOCS[i][k];
    if (!paginas[ruta]) { fallos.push(`Falta el capítulo ${k} en ${i}`); return; }
    const $ = cheerio.load(paginas[ruta]);
    const etiqueta = norm($('.capitulo__etiqueta').text());
    if (!new RegExp(`(Capítulo|Chapter) ${n + 1} (de|of) ${TOTAL} · `).test(etiqueta)) fallos.push(`${ruta}: etiqueta de capítulo incorrecta (${etiqueta})`);
    const esperado = (k2) => rutaDe(i, k2);
    const antes = n === 0 ? 'documentacion' : CAPITULOS[n - 1];
    const despues = n === TOTAL - 1 ? null : CAPITULOS[n + 1];
    if ($('.vecino--anterior').attr('href') !== esperado(antes) || $('link[rel="prev"]').attr('href')?.replace('https://gate-driven.dev', '') !== esperado(antes)) fallos.push(`${ruta}: Anterior debería ser ${antes}`);
    if (despues) {
      if ($('.vecino--siguiente').attr('href') !== esperado(despues) || $('link[rel="next"]').attr('href')?.replace('https://gate-driven.dev', '') !== esperado(despues)) fallos.push(`${ruta}: Siguiente debería ser ${despues}`);
    } else if ($('.vecino--siguiente').length || $('link[rel="next"]').length) fallos.push(`${ruta}: el último capítulo no debe tener Siguiente`);
    const actuales = $('.capitulos a[aria-current="page"]');
    if (actuales.length !== 1 || actuales.attr('href') !== esperado(k)) fallos.push(`${ruta}: la barra de capítulos no marca este capítulo`);
    if ($('.capitulos ol.capitulos__partes a[href^="/"]').length !== TOTAL) fallos.push(`${ruta}: la barra de capítulos no lista los ${TOTAL}`);
  });
  const $m = cheerio.load(paginas[mapa]);
  for (const k of CAPITULOS) if (!$m(`a[href="${rutaDe(i, k)}"]`).length) fallos.push(`${mapa}: el mapa no lista ${k}`);
  const lista = nombresPagina(i);
  for (const [de, a] of REDIRECCIONES) {
    if ((i === 'en') !== de.startsWith('/en/')) continue;
    if (!paginas[`${a}index.html`]) fallos.push(`La ruta antigua ${de} redirige a ${a}, que no existe`);
  }
}
const redirecciones = fs.readFileSync(path.join(dist, '_redirects'), 'utf8');
for (const [de, a] of REDIRECCIONES) for (const origen of [de, `${de}/`]) if (!redirecciones.includes(`${origen} ${a} 301`)) fallos.push(`_redirects no lleva ${origen} a ${a}`);
console.log(`Documentación: ${TOTAL} capítulos en cada idioma, con su Anterior y su Siguiente, y ${REDIRECCIONES.length} rutas antiguas que redirigen.`);

// 10. What search engines read, and the order of the headings.
const noEsPagina = (ruta) => /\/404\.html$/.test(ruta);
const dirDe = (url) => `${url.replace('https://gate-driven.dev', '')}index.html`;
for (const [ruta, html] of Object.entries(paginas)) {
  const $ = cheerio.load(html);
  const niveles = $('main h1, main h2, main h3, main h4').map((_, h) => Number($(h).attr('aria-level') || h.tagName[1])).get();
  niveles.forEach((n, k) => { if (k && n - niveles[k - 1] > 1) fallos.push(`${ruta}: salta de h${niveles[k - 1]} a h${n}`); });
  if (noEsPagina(ruta)) {
    if ($('meta[name="robots"]').attr('content') !== 'noindex') fallos.push(`${ruta}: la página de error no lleva noindex`);
    continue;
  }
  const titulo = $('title').text();
  const descripcion = $('meta[name="description"]').attr('content') || '';
  if (titulo.length > 60) fallos.push(`${ruta}: título de ${titulo.length} caracteres`);
  if (descripcion.length < 70 || descripcion.length > 160) fallos.push(`${ruta}: descripción de ${descripcion.length} caracteres`);
  const canonica = $('link[rel="canonical"]').attr('href') || '';
  if (dirDe(canonica) !== ruta) fallos.push(`${ruta}: canonical ${canonica}`);
  const alternas = Object.fromEntries($('link[rel="alternate"][hreflang]').map((_, n) => [[$(n).attr('hreflang'), $(n).attr('href')]]).get());
  for (const h of ['es', 'en', 'x-default']) {
    if (!alternas[h] || !paginas[dirDe(alternas[h])]) fallos.push(`${ruta}: falta hreflang ${h}`);
  }
  for (const h of ['es', 'en']) {
    if (alternas[h] && paginas[dirDe(alternas[h])]) {
      const vuelta = cheerio.load(paginas[dirDe(alternas[h])])(`link[rel="alternate"][hreflang="${ruta.startsWith('/en/') ? 'en' : 'es'}"]`).attr('href');
      if (dirDe(vuelta || '') !== ruta) fallos.push(`${ruta}: hreflang ${h} no vuelve a esta página`);
    }
  }
  const destinoIdioma = $('a.idioma').attr('href') || '';
  if (!paginas[`${destinoIdioma}index.html`]) fallos.push(`${ruta}: el enlace de idioma no lleva a una página`);
  try {
    JSON.parse($('script[type="application/ld+json"]').html());
  } catch {
    fallos.push(`${ruta}: datos estructurados inválidos`);
  }
  if (!fs.existsSync(path.join(dist, new URL($('meta[property="og:image"]').attr('content')).pathname))) fallos.push(`${ruta}: falta la imagen para compartir`);
  if (ruta.startsWith('/en/')) {
    const visible = (() => { const c = cheerio.load(html); c('script, style, [lang="es"]').remove(); return norm(c('body').text()); })();
    const resto = /Saltar al contenido|En esta página|Modo oscuro|Modo claro|Descargar el manifiesto|Monta tu primera puerta|Manos a la obra|Capítulo \d|Anterior y siguiente|Por dónde seguir/.exec(visible);
    if (resto) fallos.push(`${ruta}: queda español en la interfaz: ${resto[0]}`);
  }
}

const tam = (dir) => fs.readdirSync(dir).reduce((s, f) => { const p = path.join(dir, f); return s + (fs.statSync(p).isDirectory() ? tam(p) : fs.statSync(p).size); }, 0);
console.log(`Tamaño total: ${(tam(dist) / 1024).toFixed(0)} KB.`);

if (fallos.length) {
  console.error(`\n${fallos.length} problemas:`);
  fallos.forEach((f) => console.error(`  ${f}`));
  process.exit(1);
}
console.log('Todo en orden.');
