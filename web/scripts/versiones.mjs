// The versions the site shows, read from where they live, so that no page writes one by hand.
// A document writes {{skills}} or {{contrato}} where it shows them, and the build fills them in.
// A missing value or an unknown placeholder stops the build instead of reaching the page.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const leer = (...p) => fs.readFileSync(path.join(repo, ...p), 'utf8');
const exigir = (v, que) => {
  if (!v) throw new Error(`versiones.mjs: falta ${que}`);
  return v;
};

export const SKILLS = exigir(JSON.parse(leer('.claude-plugin', 'plugin.json')).version, 'la versión de .claude-plugin/plugin.json');
export const CONTRATO = exigir(leer('skills', 'gdd-prepare', 'assets', 'blueprint.md').match(/^contract: (\S+)/m)?.[1], '"contract:" en la plantilla del plano');
const VALORES = { skills: SKILLS, contrato: CONTRATO };
export const conVersiones = (texto) => texto.replace(/{{(\w+)}}/g, (marca, clave) => exigir(VALORES[clave], marca));
