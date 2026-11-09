// Writes content/en/origen.json: the fingerprint of every Spanish source, once its translation is up to date.
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { DOCS } from './estructura.mjs';

const raiz = path.join(path.dirname(fileURLToPath(import.meta.url)), '..');
const huella = (rel) => createHash('sha256').update(fs.readFileSync(path.join(raiz, rel))).digest('hex').slice(0, 16);
const fuentes = [
  ...Object.values(DOCS).map((d) => `content/${d.fichero}`),
  ...fs.readdirSync(path.join(raiz, 'content')).filter((f) => f.endsWith('.jsx')).map((f) => `content/${f}`),
  ...fs.readdirSync(path.join(raiz, 'content/codigo')).map((f) => `content/codigo/${f}`),
].sort();
fs.writeFileSync(path.join(raiz, 'content/en/origen.json'), `${JSON.stringify(Object.fromEntries(fuentes.map((f) => [f, huella(f)])), null, 2)}\n`);
console.log(`origen.json: ${fuentes.length} fuentes.`);
