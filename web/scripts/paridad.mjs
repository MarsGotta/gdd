// The English documents must keep the structure of the Spanish ones: same elements, same order,
// same attributes and links. Only the text changes. Used by comprobar.mjs, and by hand:
//   node scripts/paridad.mjs manifiesto
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import * as cheerio from 'cheerio';

const raiz = path.join(path.dirname(fileURLToPath(import.meta.url)), '..');
const firma = ($) => $('*').map((_, n) => {
  const atributos = Object.entries(n.attribs || {}).map(([k, v]) => `${k}=${k === 'id' ? '' : v}`).sort().join(' ');
  return `${n.name} ${atributos}`.trim();
}).get();

// Words that only a Spanish sentence would carry. In English text a few of them can appear inside a
// name or a quotation, so this counts them instead of forbidding them.
const ESPANOL = /\b(que|para|como|una|los|las|del|pero|porque|cuando|también|solo|puerta|cerradura|llave)\b/gi;

// `nombre` is the file without extension, relative to content/ (for example "manifiesto" or "doc/03-las-piezas").
export const compara = (nombre) => {
  const fallos = [];
  const es = cheerio.load(fs.readFileSync(path.join(raiz, `content/${nombre}.html`), 'utf8'), null, false);
  const ficheroEn = path.join(raiz, `content/en/${nombre}.html`);
  if (!fs.existsSync(ficheroEn)) return [`${nombre}: falta content/en/${nombre}.html`];
  const crudo = fs.readFileSync(ficheroEn, 'utf8');
  const en = cheerio.load(crudo, null, false);
  const a = firma(es);
  const b = firma(en);
  if (a.length !== b.length) fallos.push(`${nombre}: ${a.length} elementos en español y ${b.length} en inglés`);
  for (let i = 0; i < Math.min(a.length, b.length); i += 1) {
    if (a[i] !== b[i]) {
      fallos.push(`${nombre}: el elemento ${i + 1} cambia\n    es: ${a[i].slice(0, 110)}\n    en: ${b[i].slice(0, 110)}`);
      break;
    }
  }
  if (/[—–]/.test(crudo)) fallos.push(`${nombre}: tiene rayas`);
  en('code, pre').remove();
  const palabras = en.root().text().split(/\s+/).length;
  const espanolas = (en.root().text().match(ESPANOL) || []).length;
  if (espanolas > Math.max(3, palabras * 0.004)) fallos.push(`${nombre}: ${espanolas} palabras que parecen en español`);
  return fallos;
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const nombres = process.argv.slice(2);
  const fallos = nombres.flatMap(compara);
  fallos.forEach((f) => console.log(f));
  console.log(fallos.length ? `${fallos.length} problemas` : `Estructura igual: ${nombres.join(', ')}`);
  process.exit(fallos.length ? 1 : 0);
}
