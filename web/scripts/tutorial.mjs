// The two pieces of the hands-on chapter, both written from content/tutorial/viaje.json:
//   <figure data-pieza="puertas">  how the four gates change with the colour of a change
//   <figure data-pieza="viaje">    the four changes, one at a time
// The HTML of "viaje" carries every text in order, so it reads like a normal chapter without a script.
// src/js/tutorial.js reveals that same content step by step. "puertas" is drawn by the script alone (the table next to it says the same).
// What each piece says lives in content/tutorial/viaje.json (and content/en/tutorial/), so a new example needs no new code.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { DOCS } from './estructura.mjs';

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

// The words of the pieces themselves. The texts of the example come from its JSON.
export const UI = {
  es: {
    leerTodo: 'Leerlo todo de una vez',
    verEntero: 'Ver el cambio entero',
    reiniciar: 'Empezar de nuevo',
    nDeM: '{n} de {m}',
    toca: 'Toca:',
    pregunta: '¿Qué color le pondrías?',
    tuPondrias: 'Tú le pondrías {color}.',
    siguiente: 'Siguiente',
    siguienteCambio: 'Siguiente cambio',
    pasillo: 'El agente recorre las puertas',
    lleva: 'Además de lo de siempre, lleva:',
    comprueban: 'La comprueban:',
    abierta: 'Abierta',
    vuelto: 'Ha vuelto al agente: {n}',
    llamado: 'Te ha llamado: {n}',
    agente: 'agente',
    sale: 'sale',
    colado: 'se ha colado',
    hecho: 'hecho',
    estadoAbierta: 'abierta',
    estadoPasa: 'pasa',
    estadoAprobada: 'aprobaste',
    estadoVuelve: 'vuelve',
    estadoTu: 'te toca',
    estadoPara: 'la puerta lo para',
    funciono: '¿Ha funcionado el plano?',
    si: 'Sí',
    no: 'No',
    teToco: 'Te ha tocado:',
    colorCambio: 'Color del cambio',
    leyenda: 'Marcas de las comprobaciones',
    cambioN: 'Cambio {n}',
    sinColor: 'sin color todavía',
    progreso: 'Cambios y su color',
    tipos: { maquina: 'de máquina', vigilante: 'de vigilante', persona: 'de persona' },
  },
  en: {
    leerTodo: 'Read it all at once',
    verEntero: 'See the whole change',
    reiniciar: 'Start over',
    nDeM: '{n} of {m}',
    toca: 'It touches:',
    pregunta: 'What color would you give it?',
    tuPondrias: "You'd make it {color}.",
    siguiente: 'Next',
    siguienteCambio: 'Next change',
    pasillo: 'The agent walks through the gates',
    lleva: 'On top of what it always carries:',
    comprueban: 'Checked by:',
    abierta: 'Open',
    vuelto: 'Sent back to the agent: {n}',
    llamado: 'Called you: {n}',
    agente: 'agent',
    sale: 'ships',
    colado: 'slipped through',
    hecho: 'done',
    estadoAbierta: 'open',
    estadoPasa: 'passes',
    estadoAprobada: 'approved',
    estadoVuelve: 'sends it back',
    estadoTu: 'your turn',
    estadoPara: 'the gate stops it',
    funciono: 'Did the blueprint work?',
    si: 'Yes',
    no: 'No',
    teToco: 'What fell to you:',
    colorCambio: 'Color of the change',
    leyenda: 'Marks of the checks',
    cambioN: 'Change {n}',
    sinColor: 'no color yet',
    progreso: 'Changes and their color',
    tipos: { maquina: 'machine', vigilante: 'reviewer', persona: 'person' },
  },
};

const esc = (s) => String(s ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
// Plain text with `code` spans.
const fmt = (s) => esc(s).replace(/`([^`]+)`/g, '<code>$1</code>');
const pre = (v) => `<pre tabindex="0"><code>${esc(v)}</code></pre>`;
const lleno = (s, v) => String(s).replace(/\{(\w+)\}/g, (_, k) => v[k]);

// "doc:key#anchor" becomes the route of that chapter in this language.
const ruta = (enlace, idioma) => {
  const [clave, ancla] = String(enlace).slice(4).split('#');
  const doc = DOCS[clave];
  if (!doc) throw new Error(`tutorial: enlace a un documento que no existe: ${enlace}`);
  return `${doc[idioma].ruta}${ancla ? `#${ancla}` : ''}`;
};
const resolver = (v, idioma) => {
  if (typeof v === 'string') return v.replace(/\]\((doc:[^)\s]+)\)/g, (_, e) => `](${ruta(e, idioma)})`);
  if (Array.isArray(v)) return v.map((x) => resolver(x, idioma));
  if (v && typeof v === 'object') return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, resolver(x, idioma)]));
  return v;
};

const leer = (clave, idioma, dir = path.join(raiz, 'content')) => {
  const fichero = path.join(dir, idioma === 'en' ? 'en/tutorial' : 'tutorial', `${clave}.json`);
  if (!fs.existsSync(fichero)) throw new Error(`tutorial: no existe ${path.relative(raiz, fichero)}`);
  return JSON.parse(fs.readFileSync(fichero, 'utf8'));
};

const json = (v) => JSON.stringify(v).replace(/</g, '\\u003c');
const lampara = (c) => `<span class="lampara lampara--${esc(c)}" aria-hidden="true"></span>`;

// `dir` replaces the content folder (for tests only).
export function tutorial({ clave, idioma = 'es', dir }) {
  const lang = idioma === 'en' ? 'en' : 'es';
  const ui = UI[lang];
  const d = resolver(leer('viaje', lang, dir), lang);

  if (clave === 'puertas') {
    // Drawn by the script. Without one the figure stays hidden and the table below says it all.
    const datos = json({ ui, puertas: d.puertas, colores: d.colores });
    return `<figure class="tutorial tutorial--puertas" data-tutorial="puertas" hidden>
  <script type="application/json" class="tutorial__datos">${datos}</script>
</figure>`;
  }
  if (clave !== 'viaje') throw new Error(`tutorial: pieza desconocida "${clave}"`);

  const nombreColor = (c) => d.colores[c].nombre;
  const total = d.cambios.length;
  const puerta = Object.fromEntries(d.puertas.map((p) => [p.id, p]));

  const cambio = (c, i) => {
    const pasos = c.pasos.map((p) => `<li data-tipo="${esc(p.tipo)}"${p.puerta ? ` data-puerta="${esc(p.puerta)}"` : ''}><p>${p.puerta ? `<strong>${esc(puerta[p.puerta].nombre)}</strong> ` : ''}${fmt(p.texto)}</p>${p.codigo ? pre(p.codigo) : ''}</li>`).join('\n      ');
    const b = c.blindar;
    const blindar = b ? `
    <div class="cambio__blindar">
      <p class="cambio__intro">${fmt(b.intro)}</p>
      <ul>${b.items.map((x) => `<li>${fmt(x)}</li>`).join('')}</ul>
      ${pre(b.codigo)}
      <p class="cambio__cierre">${fmt(b.cierre)}</p>
    </div>` : '';
    return `<article class="cambio" data-cambio="${esc(c.id)}" data-n="${i + 1}">
    <h3 class="cambio__titulo">${i + 1}. ${esc(c.nombre)}</h3>
    <p class="cambio__peticion">"${fmt(c.peticion)}"</p>
    <p class="cambio__toca"><strong>${esc(ui.toca)}</strong> ${c.toca.map((t) => `<code>${esc(t)}</code>`).join(', ')}</p>
    <p class="cambio__plano">${fmt(d.plano)}</p>
    <p class="cambio__razon"><span class="celda-lampara">${lampara(c.color)}<strong>${esc(nombreColor(c.color))}.</strong></span> ${fmt(c.razon)}</p>${c.siRojo ? `
    <p class="cambio__sirojo">${fmt(c.siRojo)}</p>` : ''}
    <ol class="cambio__pasos">
      ${pasos}
    </ol>
    <div class="cambio__juicio">
      <p class="cambio__pregunta"><strong>${esc(ui.funciono)}</strong></p>
      <p class="cambio__respuesta"><strong>${esc(c.resultado.funciono ? ui.si : ui.no)}.</strong> ${fmt(c.resultado.texto)}</p>
    </div>
    <p class="cambio__toco"><strong>${esc(ui.teToco)}</strong> ${fmt(c.teToco)}</p>${blindar}
  </article>`;
  };

  // Only what the script needs and the HTML does not say: the gates, the colours and the shape of each step.
  const datos = json({
    ui,
    puertas: d.puertas,
    colores: d.colores,
    cambios: d.cambios.map((c) => ({
      id: c.id,
      color: c.color,
      auto: !!c.auto,
      funciono: c.resultado.funciono,
      pasos: c.pasos.map((p) => ({ puerta: p.puerta, tipo: p.tipo, hecho: !!p.hecho })),
      blindar: c.blindar ? { color: c.blindar.color, paraEn: c.blindar.paraEn, boton: c.blindar.boton } : null,
    })),
  });

  return `<div class="tutorial tutorial--viaje" data-tutorial="viaje" data-total="${total}">
  <div class="viaje">
  ${d.cambios.map(cambio).join('\n  ')}
  </div>
  <script type="application/json" class="tutorial__datos">${datos}</script>
</div>`;
}
