// Shared markup: icons, the mark, the floor plan and the small drawings.

export const sprite = `<svg class="sprite" aria-hidden="true" width="0" height="0" focusable="false"><defs>
<symbol id="i-cerradura" viewBox="0 0 24 24"><rect x="5" y="11" width="14" height="9.5" rx="1" fill="none" stroke="currentColor" stroke-width="2"/><path d="M8 11V7.5a4 4 0 0 1 8 0V11" fill="none" stroke="currentColor" stroke-width="2"/></symbol>
<symbol id="i-llave" viewBox="0 0 24 24"><circle cx="8" cy="16" r="4.5" fill="none" stroke="currentColor" stroke-width="2"/><path d="M11.2 12.8 20.5 3.5M16 8l3.2 3.2M13.2 10.8l2.2 2.2" fill="none" stroke="currentColor" stroke-width="2"/></symbol>
<symbol id="i-vigilante" viewBox="0 0 24 24"><path d="M2 12s3.600-6.500 10-6.500S22 12 22 12s-3.600 6.500-10 6.500S2 12 2 12Z" fill="none" stroke="currentColor" stroke-width="2"/><circle cx="12" cy="12" r="2.8" fill="currentColor"/></symbol>
<symbol id="i-persona" viewBox="0 0 24 24"><circle cx="12" cy="7.5" r="4" fill="none" stroke="currentColor" stroke-width="2"/><path d="M4 21c.6-4.600 3.800-7 8-7s7.400 2.400 8 7" fill="none" stroke="currentColor" stroke-width="2"/></symbol>
<symbol id="i-tema" viewBox="0 0 24 24"><circle cx="12" cy="12" r="8.5" fill="none" stroke="currentColor" stroke-width="2"/><path d="M12 3.5a8.5 8.5 0 0 1 0 17Z" fill="currentColor"/></symbol>
<symbol id="i-cruz" viewBox="0 0 24 24"><path d="m6 6 12 12M18 6 6 18" fill="none" stroke="currentColor" stroke-width="2.4"/></symbol>
<symbol id="i-abajo" viewBox="0 0 14 14"><path d="M7 1v11M2.500 7.500 7 12l4.500-4.500" stroke="currentColor" stroke-width="1.8" fill="none"/></symbol>
<symbol id="i-ok" viewBox="0 0 24 24"><path d="m4.500 12.500 5 5L19.500 7" fill="none" stroke="currentColor" stroke-width="2.4"/></symbol>
<symbol id="i-limite" viewBox="0 0 24 24"><rect x="4" y="4" width="16" height="16" rx="1" fill="none" stroke="currentColor" stroke-width="2" stroke-dasharray="3.5 2.5"/><circle cx="12" cy="12" r="2.8" fill="currentColor"/></symbol>
<symbol id="i-abierta" viewBox="0 0 24 24"><path d="M3 21h3M18 21h3" stroke="currentColor" stroke-width="2.4" fill="none"/><path d="M6 21V5" stroke="currentColor" stroke-width="2" fill="none"/><path d="M6 5a16 16 0 0 1 12 16" stroke="currentColor" stroke-width="1.2" fill="none"/></symbol>
<symbol id="i-vuelve" viewBox="0 0 24 24"><path d="M9.500 4.500 4.500 9.500l5 5" fill="none" stroke="currentColor" stroke-width="2.2"/><path d="M4.500 9.500h10.500a5 5 0 0 1 0 10H10" fill="none" stroke="currentColor" stroke-width="2.2"/></symbol>
<symbol id="i-para" viewBox="0 0 24 24"><path d="M2.500 12h11M10 8l4 4-4 4" fill="none" stroke="currentColor" stroke-width="2.2"/><path d="M19.500 3.500v17" fill="none" stroke="currentColor" stroke-width="3"/></symbol>
</defs></svg>`;

// The door symbol of a floor plan: a gap in a wall, its leaf and its swing.
export const marca = (clase = '') => `<svg class="marca ${clase}" viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path d="M1 21.500h5M18.500 21.500H23" stroke="currentColor" stroke-width="3" fill="none"/><path d="M6 21.500V4" stroke="currentColor" stroke-width="2" fill="none"/><path d="M6 4a15 17.500 0 0 1 12.500 17.500" stroke="currentColor" stroke-width="1.3" fill="none"/></svg>`;

export const icono = (id, cls = '') => `<svg class="icono ${cls}" aria-hidden="true" focusable="false"><use href="#i-${id}"/></svg>`;

// --- The door of the opening, seen from above: one agent, one lock, one key, in ink. ---
export const escena = (tr) => `<svg class="escena" viewBox="120 20 510 400" role="img" aria-label="${tr('Un plano visto desde arriba. Un agente llega a una puerta cerrada y dice hecho. La puerta comprueba el trabajo y no le deja pasar. El agente lo arregla, pasa la comprobación, la puerta se abre y pasa.', 'A floor plan seen from above. An agent reaches a closed gate and says done. The gate checks the work and does not let it through. The agent fixes it, passes the check, the gate opens and it goes through.')}" focusable="false">
  <path class="escena__ruta" d="M130 220H372"/><path class="escena__ruta" d="M388 220H600"/>
  <path class="escena__muro" d="M380 150V30H620V410H380V290"/>
  <path class="escena__arco" d="M380 290A140 140 0 0 0 520 150"/>
  <text x="604" y="62" text-anchor="end">${tr('producción', 'production')}</text>
  <text x="392" y="136">${tr('puerta', 'gate')}</text>
  <text x="368" y="354" text-anchor="end">${tr('comprobación', 'check')}</text>
  <circle class="escena__insignia" cx="350" cy="316" r="16"/>
  <use href="#i-cerradura" x="339" y="305" width="22" height="22"/>
  <g class="escena__mal"><circle class="escena__lleno" cx="350" cy="316" r="16"/><use class="escena__sobre" href="#i-cruz" x="339" y="305" width="22" height="22"/></g>
  <g class="escena__bien"><circle class="escena__lleno" cx="350" cy="316" r="16"/><use class="escena__sobre" href="#i-llave" x="339" y="305" width="22" height="22"/></g>
  <line class="escena__hoja" x1="380" y1="150" x2="380" y2="290"/>
  <g class="escena__llave"><use href="#i-llave" x="-14" y="-14" width="28" height="28"/><text x="0" y="-20" text-anchor="middle">${tr('pasa', 'passes')}</text></g>
  <g class="escena__agente">
    <circle class="escena__lleno" cx="0" cy="220" r="11"/>
    <text x="0" y="198" text-anchor="middle">${tr('agente', 'agent')}</text>
    <g class="escena__globo"><text class="escena__dicho" x="0" y="176" text-anchor="middle">${tr('"hecho"', '"done"')}</text></g>
  </g>
</svg>`;

// --- The floor plan: three rooms in a row, three doors and a small office for you. ---
const Y0 = 60, Y1 = 220, GAP0 = 110, GAP1 = 170, W = 8;
const X = { 1: 230, 2: 470, 3: 680 };

const muroV = (x) => `<rect x="${x - W / 2}" y="${Y0 - W / 2}" width="${W}" height="${GAP0 - Y0 + W / 2}"/><rect x="${x - W / 2}" y="${GAP1}" width="${W}" height="${Y1 - GAP1 + W / 2}"/>`;

const insignia = (clase, id, x, y) => `<g class="insignia ${clase}" style="transform-origin:${x}px ${y}px"><circle cx="${x}" cy="${y}" r="15"/><use href="#i-${id}" x="${x - 10}" y="${y - 10}" width="20" height="20"/></g>`;

const puerta = (n, x, extra = '') => {
  const r = GAP1 - GAP0;
  return `<g class="puerta" data-puerta="${n}" data-estado="cerrada">
    <path class="puerta__arco" pathLength="1" d="M ${x} ${GAP0} A ${r} ${r} 0 0 1 ${x + r} ${GAP1}"/>
    <line class="puerta__hoja" style="transform-origin:${x}px ${GAP1}px" x1="${x}" y1="${GAP1}" x2="${x}" y2="${GAP0}"/>
    ${insignia('insignia--cerradura', 'cerradura', x, 87)}
    ${insignia('insignia--llave', 'llave', x, 87)}
    ${extra}
  </g>`;
};

const reja = (x) => `<rect x="${x - 16}" y="${Y0 + W / 2}" width="32" height="${GAP0 - Y0 - W / 2}" fill="url(#rayado)"/><rect x="${x - 16}" y="${GAP1}" width="32" height="${Y1 - GAP1 - W / 2}" fill="url(#rayado)"/>`;

export const svgPlano = (tr) => `<svg class="plano__dibujo" viewBox="0 0 800 300" role="img" aria-label="${tr('Plano de una casa con tres salas en fila. El agente tiene que cruzar tres puertas para llegar a producción.', 'Floor plan of a house with three rooms in a row. The agent has to get through three gates to reach production.')}" focusable="false">
  <defs><pattern id="rayado" width="7" height="7" patternUnits="userSpaceOnUse" patternTransform="rotate(45)"><line x1="0" y1="0" x2="0" y2="7" stroke="currentColor" stroke-width="2.2"/></pattern></defs>
  <rect class="plano__suelo" x="40" y="60" width="640" height="160"/>
  <rect class="plano__suelo-anexo" x="40" y="220" width="126" height="64"/>
  <g class="plano__blindaje">${reja(X[2])}</g>
  <g class="plano__muros">
    <rect x="${40 - W / 2}" y="${Y0 - W / 2}" width="${680 - 40 + W}" height="${W}"/>
    <rect x="${40 - W / 2}" y="${Y1 - W / 2}" width="${680 - 40 + W}" height="${W}"/>
    ${muroV(40)}${muroV(X[1])}${muroV(X[2])}${muroV(X[3])}
    <rect x="${40 - W / 2}" y="${Y1}" width="${W}" height="${64 + W / 2}"/>
    <rect x="${166 - W / 2}" y="${Y1}" width="${W}" height="${64 + W / 2}"/>
    <rect x="${40 - W / 2}" y="${284 - W / 2}" width="${126 + W}" height="${W}"/>
  </g>
  <path class="plano__entrada" d="M 10 129 L 26 140 L 10 151 Z"/>
  <path class="plano__llamada" data-llamada="1" pathLength="1" d="M 170 238 H ${X[1] - 32} V 208"/>
  <path class="plano__llamada" data-llamada="3" pathLength="1" d="M 170 238 H ${X[3] - 32} V 208"/>
  <path class="plano__fuga" d="M 745 166 V 240 H ${X[2]} V 234"/>
  <path class="plano__fuga-punta" d="M ${X[2] - 7} 234 L ${X[2]} 225 L ${X[2] + 7} 234 Z"/>
  ${puerta(1, X[1], insignia('insignia--persona', 'persona', X[1] - 32, 193))}
  ${puerta(2, X[2], insignia('insignia--vigilante', 'vigilante', X[2] - 32, 193) + insignia('insignia--segunda', 'cerradura', X[2] + 34, 87))}
  ${puerta(3, X[3], insignia('insignia--persona', 'persona', X[3] - 32, 193))}
</svg>`;

const filaPuerta = (tr, n, nombre, items, nueva = '') => `<div class="plano__puerta" data-fila="${n}">
  <strong>${nombre}</strong>
  <ul>
    ${items.map(([id, texto, riesgos]) => `<li data-en="${riesgos}"${riesgos.includes('rojo') ? '' : ' hidden'}>${icono(id, id === 'persona' ? 'icono--persona' : '')}<span>${texto}</span></li>`).join('\n    ')}
    ${nueva}
    <li class="plano__llave" hidden>${icono('llave')}<span>${tr('Pasada', 'Passed')}</span></li>
  </ul>
</div>`;

export const plano = (tr, { pie, tocaVerde, tocaAmarillo, tocaRojo }) => `<figure class="plano" id="plano" data-riesgo="rojo">
  <div class="plano__eleccion" role="radiogroup" aria-labelledby="eleccion-titulo">
    <label class="opcion"><input type="radio" name="riesgo" value="verde"><span class="opcion__caja"><span class="lampara lampara--verde" aria-hidden="true"></span><span class="opcion__texto">${tr('Cambiar un texto', 'Change some text')}</span><small>${tr('Verde', 'Green')}</small></span></label>
    <label class="opcion"><input type="radio" name="riesgo" value="amarillo"><span class="opcion__caja"><span class="lampara lampara--amarillo" aria-hidden="true"></span><span class="opcion__texto">${tr('Un filtro nuevo en un listado', 'A new filter on a list')}</span><small>${tr('Amarillo', 'Yellow')}</small></span></label>
    <label class="opcion"><input type="radio" name="riesgo" value="rojo" checked><span class="opcion__caja"><span class="lampara lampara--rojo" aria-hidden="true"></span><span class="opcion__texto">${tr('Algo que toca el cobro', 'Something that touches payment')}</span><small>${tr('Rojo', 'Red')}</small></span></label>
  </div>

  <div class="plano__lienzo">
    <div class="plano__marco">
      ${svgPlano(tr)}
      <span class="plano__nota plano__nota--tu"><span class="plano__tu-punto" aria-hidden="true"></span>${tr('Tú', 'You')}</span>
      <span class="plano__nota plano__nota--prod">${tr('Producción', 'Production')}</span>
      <span class="plano__nota plano__nota--fuga" hidden>${tr('se cuela', 'slips through')}</span>
      <span class="plano__rechazo" hidden>${icono('cruz')}<span></span></span>
      <span class="plano__agente" style="--x:16.25%"><span class="plano__punto" aria-hidden="true"></span><span class="plano__globo" hidden>${tr('Hecho', 'Done')}</span><span class="plano__nombre">${tr('Agente', 'Agent')}</span></span>
    </div>

    <div class="plano__puertas">
      ${filaPuerta(tr, 1, tr('Escenarios', 'Scenarios'), [
        ['abierta', tr('Abierta', 'Open'), 'verde'],
        ['cerradura', tr('Comprobación', 'Check'), 'amarillo rojo'],
        ['persona', tr('Tú lees', 'You read'), 'amarillo'],
        ['persona', tr('Tu aprobación', 'Your approval'), 'rojo'],
      ])}
      ${filaPuerta(tr, 2, tr('Tests y linters', 'Tests and linters'), [
        ['cerradura', tr('Comprobación', 'Check'), 'verde amarillo rojo'],
        ['vigilante', tr('Vigilante', 'Reviewer'), 'amarillo rojo'],
      ], `<li class="plano__nueva" hidden>${icono('cerradura')}<span>${tr('Comprobación nueva', 'New check')}</span></li>`)}
      ${filaPuerta(tr, 3, tr('Salida', 'Release'), [
        ['abierta', tr('Abierta', 'Open'), 'verde'],
        ['cerradura', tr('Comprobación', 'Check'), 'amarillo rojo'],
        ['persona', tr('Tu aprobación', 'Your approval'), 'rojo'],
      ])}
    </div>

    <div class="plano__relato">
      <div>
        <p class="plano__paso" data-paso></p>
        <p class="plano__texto" data-relato>${tr('Te llama antes de empezar: lees los escenarios y los corriges.', 'It calls you before it starts: you read the scenarios and correct them.')}</p>
      </div>
      <div class="plano__mandos" data-mandos hidden>
        <button type="button" class="mando" data-accion="pausa">${tr('Pausar', 'Pause')}</button>
        <button type="button" class="mando" data-accion="atras">${tr('Anterior', 'Back')}</button>
        <button type="button" class="mando" data-accion="adelante">${tr('Siguiente', 'Next')}</button>
      </div>
      <div class="plano__avance" aria-hidden="true"><span data-avance></span></div>
    </div>
  </div>

  <dl class="plano__detalle">
    <div><dt>${tr('Qué te toca hacer', 'What you have to do')}</dt>
      <dd data-en="verde" hidden>${tocaVerde}</dd><dd data-en="amarillo" hidden>${tocaAmarillo}</dd><dd data-en="rojo">${tocaRojo}</dd></div>
    <div><dt>${tr('Veces que te han llamado', 'Times you were called')}</dt><dd class="plano__cuenta" data-cuenta>0</dd></div>
  </dl>
  <figcaption>${pie}</figcaption>
</figure>`;

// --- Three small plans for the problem: a shared corridor drawn three ways. ---
const pasillo = (interior) => `<svg class="dibujo" viewBox="0 0 300 150" aria-hidden="true" focusable="false"><g class="plano__muros"><rect x="8" y="28" width="284" height="6"/><rect x="8" y="116" width="284" height="6"/>${interior}</g>`;
const punto = (x, y = 75, clase = 'dibujo__agente') => `<circle class="${clase}" cx="${x}" cy="${y}" r="7"/>`;
const jambas = '<rect x="187" y="34" width="6" height="18"/><rect x="187" y="98" width="6" height="18"/>';

export const dibujoTodo = `${pasillo(jambas)}${[34, 62, 90, 118, 146].map((x) => punto(x)).join('')}${punto(190, 75, 'dibujo__persona')}</svg>`;

export const dibujoNada = `${pasillo('')}${[26, 72, 118, 164, 210].map((x) => punto(x)).join('')}<use class="dibujo__fallo" href="#i-cruz" x="244" y="63" width="24" height="24"/></svg>`;

export const dibujoSolo = `${pasillo(jambas)}<line class="dibujo__hoja" x1="190" y1="98" x2="236" y2="98"/><path class="dibujo__arco" d="M190 52 A46 46 0 0 1 236 98"/>${punto(128)}<use class="dibujo__objeto" href="#i-cerradura" x="86" y="62" width="26" height="26"/><use class="dibujo__objeto" href="#i-llave" x="144" y="62" width="26" height="26"/></svg>`;

// --- A single door, drawn large, to name its parts. ---
export const anatomia = (tr) => `<figure class="anatomia">
  <div class="anatomia__lienzo">
    <svg viewBox="0 0 760 320" role="img" aria-label="${tr('Una puerta con su comprobación, el agente que la pasa, el vigilante y la persona que suman las suyas, y el riesgo del cambio, que tiñe el suelo.', 'A gate with its check, the agent getting through it, the reviewer and the person adding theirs, and the risk of the change, which tints the floor.')}" focusable="false">
      <rect class="anatomia__suelo" x="60" y="44" width="640" height="232"/>
      <g class="plano__muros">
        <rect x="56" y="40" width="648" height="8"/><rect x="56" y="272" width="648" height="8"/>
        <rect x="56" y="40" width="8" height="240"/><rect x="696" y="40" width="8" height="240"/>
        <rect x="376" y="48" width="8" height="66"/><rect x="376" y="206" width="8" height="66"/>
      </g>
      <path class="dibujo__arco" d="M380 114 A92 92 0 0 1 472 206"/>
      <line class="dibujo__hoja" x1="380" y1="206" x2="472" y2="206"/>
      <circle class="dibujo__agente" cx="300" cy="160" r="10"/>
      <g class="insignia insignia--fija"><circle cx="380" cy="84" r="18"/><use href="#i-cerradura" x="368" y="72" width="24" height="24"/></g>
      <g class="insignia insignia--fija"><circle cx="330" cy="240" r="18"/><use href="#i-vigilante" x="318" y="228" width="24" height="24"/></g>
      <g class="insignia insignia--fija insignia--azul"><circle cx="430" cy="240" r="18"/><use href="#i-persona" x="418" y="228" width="24" height="24"/></g>
      <use class="dibujo__objeto" href="#i-llave" x="248" y="146" width="28" height="28"/>
      <g class="anatomia__guias">
        <path d="M428 160 H 520"/><path d="M398 84 H 520"/><path d="M262 174 V 212"/><path d="M380 240 H 348 M380 240 H 412 M380 240 V 296"/><path d="M640 276 V 296"/>
      </g>
    </svg>
    <span class="anatomia__rotulo" style="left:69%;top:46%">${tr('Puerta', 'Gate')}</span>
    <span class="anatomia__rotulo" style="left:69%;top:22.500%">${tr('Comprobación', 'Check')}</span>
    <span class="anatomia__rotulo" style="left:34.500%;top:67%">${tr('Pasa', 'Passes')}</span>
    <span class="anatomia__rotulo" style="left:50%;top:92.500%">${tr('Vigilante y persona', 'Reviewer and person')}</span>
    <span class="anatomia__rotulo" style="left:84.200%;top:92.500%">${tr('Riesgo', 'Risk')}</span>
  </div>
</figure>`;
