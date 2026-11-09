// The hands-on chapter. Two pieces share this file:
//   .tutorial--puertas  how the four gates change with the colour of the change (drawn here from its JSON).
//   .tutorial--viaje    the four changes, revealed one at a time. The server already wrote every text in order;
//                       here they are shown little by little, and nothing is ever required to go on.
// Nothing the reader does is stored.
(() => {
  const reducido = () => window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  const lleno = (s, v) => String(s).replace(/\{(\w+)\}/g, (_, k) => v[k]);
  const h = (tag, attrs, ...hijos) => {
    const e = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs || {})) {
      if (v === false || v == null) continue;
      if (k === 'class') e.className = v;
      else if (k.slice(0, 2) === 'on') e.addEventListener(k.slice(2), v);
      else e.setAttribute(k, v === true ? '' : v);
    }
    for (const x of hijos.flat()) if (x != null && x !== false) e.append(x.nodeType ? x : document.createTextNode(x));
    return e;
  };
  const dePlantilla = (html) => { const t = document.createElement('template'); t.innerHTML = html.trim(); return t.content.firstChild; };
  const ico = (id) => dePlantilla(`<svg class="icono" aria-hidden="true" focusable="false"><use href="#i-${id}"/></svg>`);
  const lampara = (c) => h('span', { class: `lampara lampara--${c}`, 'aria-hidden': 'true' });
  const sr = (t) => h('span', { class: 'tutorial__sr' }, t);
  let contador = 0;
  const unico = (p) => `${p}-${++contador}`;

  // The mark of each kind of check: a square for a machine, a diamond for a reviewer, a filled circle for a person.
  const MARCAS = {
    maquina: '<rect x="2" y="2" width="10" height="10" fill="none" stroke="currentColor" stroke-width="2"/>',
    vigilante: '<path d="M7 1.200 12.800 7 7 12.800 1.200 7Z" fill="none" stroke="currentColor" stroke-width="2"/>',
    persona: '<circle cx="7" cy="7" r="6" fill="currentColor"/>',
  };
  const marca = (tipo) => dePlantilla(`<svg class="marca marca--${tipo}" viewBox="0 0 14 14" aria-hidden="true" focusable="false">${MARCAS[tipo]}</svg>`);
  // A check: its mark, its words and, for a screen reader, its kind.
  const comprobacion = (c, ui) => h('li', { class: `comprobacion comprobacion--${c.tipo}` }, marca(c.tipo), h('span', null, c.texto, ' ', sr(`(${ui.tipos[c.tipo]})`)));

  // A door seen from above: a gap in a wall, its leaf and its swing. Closed, the leaf fills the gap.
  const glifo = () => dePlantilla('<svg class="glifo" viewBox="0 0 48 34" width="48" height="34" aria-hidden="true" focusable="false"><path class="glifo__muro" d="M1.500 29H12M36 29h10.500"/><path class="glifo__jamba" d="M12 24v10M36 24v10"/><path class="glifo__arco" pathLength="1" d="M36 29A24 24 0 0 0 12 5"/><line class="glifo__hoja" x1="12" y1="29" x2="36" y2="29"/></svg>');

  // ------------------------------------------------------------------------------------------
  // The gates and the colour
  // ------------------------------------------------------------------------------------------
  const montarPuertas = (fig) => {
    const D = JSON.parse(fig.querySelector('.tutorial__datos').textContent);
    const { ui, puertas, colores } = D;
    const nombre = unico('color');
    const ids = ['verde', 'amarillo', 'rojo'].filter((c) => colores[c]);
    const tabla = fig.nextElementSibling;

    const resumen = h('p', { class: 'pc__resumen', 'aria-live': 'polite' });
    const radios = ids.map((c, i) => {
      const input = h('input', { type: 'radio', name: nombre, value: c, checked: i === 0 });
      input.addEventListener('change', () => { if (input.checked) pintar(c, true); });
      return h('label', { class: 'pc__op' }, input, h('span', { class: 'pc__caja' }, lampara(c), colores[c].nombre));
    });
    const celdas = puertas.map((p) => {
      const g = glifo();
      const lista = h('ul', { class: 'pc__checks' });
      const cuerpo = h('div', { class: 'pc__cuerpo' });
      return { p, g, lista, cuerpo, el: h('div', { class: 'pc__puerta' }, g, h('p', { class: 'pc__nombre' }, p.nombre), cuerpo) };
    });
    const leyenda = h('ul', { class: 'pc__leyenda', 'aria-label': ui.leyenda }, ['maquina', 'vigilante', 'persona'].map((t) => h('li', { class: `pc__ley pc__ley--${t}` }, marca(t), ui.tipos[t])));

    function pintar(c, animar) {
      const col = colores[c];
      fig.dataset.color = c;
      celdas.forEach((x) => {
        const cs = (col[x.p.id] && col[x.p.id].comprobaciones) || [];
        x.g.classList.toggle('esta-abierta', cs.length === 0);
        x.cuerpo.replaceChildren(cs.length ? h('ul', { class: 'pc__checks' }, cs.map((k) => comprobacion(k, ui))) : h('p', { class: 'pc__abierta' }, ui.abierta));
        if (animar) { x.cuerpo.classList.remove('pc__cambia'); void x.cuerpo.offsetWidth; x.cuerpo.classList.add('pc__cambia'); }
      });
      resumen.textContent = col.resumen;
      // The table below lights the column of that colour.
      if (tabla && tabla.querySelector('table')) tabla.dataset.color = c;
    }

    fig.append(
      h('fieldset', { class: 'pc__color' }, h('legend', null, ui.colorCambio), h('div', { class: 'pc__ops' }, radios)),
      h('div', { class: 'pc__puertas' }, celdas.map((x) => x.el)),
      leyenda,
      resumen,
    );
    pintar(ids[0], false);
    fig.hidden = false;
    fig.dataset.listo = '';
  };

  // ------------------------------------------------------------------------------------------
  // The corridor: the four gates in one row, and the agent walking them
  // ------------------------------------------------------------------------------------------
  // Position of the agent: 0 is the start, 1 to 4 the centre of each gate, 5 the end. Between gates: a fraction.
  const izq = (pos) => (pos <= 0 ? '7px' : pos >= 5 ? 'calc(100% - 7px)' : `calc(14px + (100% - 28px) * ${((pos - 0.5) / 4).toFixed(4)})`);

  const crearPasillo = ({ ui, puertas, colores, color, contadores }) => {
    const col = colores[color];
    const estados = Object.fromEntries(puertas.map((p) => [p.id, 'cerrada']));
    let activa = null;
    let pos = 0;
    let fin = null;
    let vueltas = 0;
    let llamadas = 0;
    let pendientes = [];

    const agente = h('span', { class: 'pasillo__agente', 'aria-hidden': 'true' }, h('span', { class: 'pasillo__bocadillo', hidden: true }, ui.hecho));
    const bocadillo = agente.firstChild;
    const finMarca = h('span', { class: 'pasillo__fin', 'aria-hidden': 'true' });
    const etqFin = h('span', { class: 'pasillo__etq pasillo__etq--fin' }, ui.sale);
    const filas = puertas.map((p) => {
      const g = glifo();
      const estado = h('span', { class: 'puerta__estado' });
      return { p, g, estado, el: h('li', { class: 'puerta', 'data-puerta': p.id, 'data-estado': 'cerrada' }, g, h('span', { class: 'puerta__nombre' }, p.nombre), estado) };
    });
    const panel = h('div', { class: 'pasillo__panel', hidden: true });
    const cuentaA = h('span', null);
    const cuentaB = h('span', null);
    const el = h('div', { class: 'pasillo', role: 'group', 'aria-label': ui.pasillo },
      // A change already shows its colour with its reason. The repeat in Harden has no reason above it.
      contadores ? null : h('p', { class: 'pasillo__color' }, lampara(color), col.nombre),
      h('div', { class: 'pasillo__marcas' }, h('span', { class: 'pasillo__etq' }, ui.agente), etqFin),
      h('div', { class: 'pasillo__via' },
        h('span', { class: 'pasillo__linea' }),
        h('span', { class: 'pasillo__ini', 'aria-hidden': 'true' }),
        h('ul', { class: 'pasillo__puertas' }, filas.map((f) => f.el)),
        finMarca,
        agente),
      panel,
      contadores ? h('p', { class: 'pasillo__cuentas' }, cuentaA, cuentaB) : null);

    const textoEstado = {
      abierta: ['abierta', ui.estadoAbierta], pasa: ['ok', ui.estadoPasa], aprobada: ['persona', ui.estadoAprobada], vuelve: ['vuelve', ui.estadoVuelve], tu: ['persona', ui.estadoTu], para: ['para', ui.estadoPara],
    };
    const pintarPuertas = () => {
      filas.forEach((f) => {
        const e = estados[f.p.id];
        f.el.dataset.estado = e;
        f.el.classList.toggle('es-activa', activa === f.p.id);
        if (activa === f.p.id) f.el.setAttribute('aria-current', 'step'); else f.el.removeAttribute('aria-current');
        f.g.classList.toggle('esta-abierta', e === 'abierta' || e === 'pasa' || e === 'aprobada');
        const t = textoEstado[e];
        if (!t) { f.estado.replaceChildren(); return; }
        f.estado.replaceChildren(t[0] === 'persona' ? marca('persona') : ico(t[0] === 'abierta' ? 'abierta' : t[0]), t[1]);
      });
      agente.style.left = izq(pos);
      finMarca.dataset.fin = fin || '';
      finMarca.replaceChildren(fin === 'sale' ? ico('ok') : fin === 'colado' ? ico('cruz') : '');
      etqFin.textContent = fin === 'colado' ? ui.colado : ui.sale;
      etqFin.classList.toggle('es-aviso', fin === 'colado');
      if (contadores) {
        cuentaA.textContent = lleno(ui.vuelto, { n: vueltas });
        cuentaB.textContent = lleno(ui.llamado, { n: llamadas });
      }
      // The panel says what the active gate carries and what checks it.
      panel.hidden = !activa;
      if (activa) {
        const p = puertas.find((x) => x.id === activa);
        const cs = (col[activa] && col[activa].comprobaciones) || [];
        panel.replaceChildren(
          h('p', { class: 'panel__nombre' }, h('strong', null, p.nombre)),
          h('p', { class: 'panel__fila' }, h('strong', null, ui.lleva), ' ', p.contexto),
          h('div', { class: 'panel__fila' }, h('strong', null, ui.comprueban), ' ', cs.length ? h('ul', { class: 'pc__checks' }, cs.map((k) => comprobacion(k, ui))) : h('span', null, ui.abierta)),
        );
      }
    };
    const despues = (fn, ms) => { const t = { fn, id: setTimeout(() => { pendientes = pendientes.filter((x) => x !== t); fn(); pintarPuertas(); }, ms) }; pendientes.push(t); };
    // Whatever was waiting to happen happens now.
    const flush = () => { const p = pendientes; pendientes = []; p.forEach((t) => { clearTimeout(t.id); t.fn(); }); bocadillo.hidden = true; };

    const aplicar = (p, { animar, ultimo }) => {
      flush();
      // Once the agent moves on, what was waiting for you is what you approved.
      puertas.forEach((x) => { if (estados[x.id] === 'tu' && x.id !== p.puerta) estados[x.id] = 'aprobada'; });
      const k = p.puerta ? puertas.findIndex((x) => x.id === p.puerta) + 1 : 0;
      if (p.puerta) activa = p.puerta; else activa = null;
      if (p.tipo === 'abierta') { estados[p.puerta] = 'abierta'; pos = k + 0.35; }
      else if (p.tipo === 'info') { pos = k; }
      else if (p.tipo === 'pasa') { estados[p.puerta] = estados[p.puerta] === 'tu' ? 'aprobada' : 'pasa'; pos = k + 0.35; }
      else if (p.tipo === 'tu') { estados[p.puerta] = 'tu'; pos = k - 0.3; llamadas += 1; }
      else if (p.tipo === 'para') { estados[p.puerta] = 'para'; pos = k - 0.3; }
      else if (p.tipo === 'vuelve') {
        vueltas += 1;
        if (animar) {
          pos = k;
          bocadillo.hidden = !p.hecho;
          despues(() => { estados[p.puerta] = 'vuelve'; pos = k - 0.5; bocadillo.hidden = true; }, 650);
        } else { estados[p.puerta] = 'vuelve'; pos = k - 0.5; }
      } else if (p.tipo === 'mal') { pos = 5; fin = 'colado'; }
      if (ultimo && (p.tipo === 'pasa' || p.tipo === 'abierta') && !Object.values(estados).some((e) => e === 'vuelve' || e === 'para')) { pos = 5; fin = 'sale'; }
      pintarPuertas();
    };
    const reiniciar = () => {
      flush();
      puertas.forEach((p) => { estados[p.id] = 'cerrada'; });
      activa = null; pos = 0; fin = null; vueltas = 0; llamadas = 0;
      pintarPuertas();
    };
    // The state after the first n steps, with no movement.
    const fijar = (pasos, n) => {
      reiniciar();
      pasos.slice(0, n).forEach((p, i) => aplicar(p, { animar: false, ultimo: i === pasos.length - 1 }));
    };
    pintarPuertas();
    return { el, aplicar, fijar, reiniciar, flush };
  };

  // ------------------------------------------------------------------------------------------
  // The journey
  // ------------------------------------------------------------------------------------------
  const montarViaje = (raiz) => {
    const D = JSON.parse(raiz.querySelector('.tutorial__datos').textContent);
    const { ui, puertas, colores } = D;
    const total = D.cambios.length;
    const cont = raiz.querySelector('.viaje');
    const arts = [...cont.querySelectorAll('.cambio')];
    const globales = []; // timers of the whole piece
    let visibles = 1;
    const smooth = () => (reducido() ? 'auto' : 'smooth');

    // --- Top: the lamps of progress and "read it all"; bottom: start over ---
    const lamparas = D.cambios.map((c, i) => {
      const li = h('li', { class: 'avance__item' });
      return { li, c, i };
    });
    const avance = h('ol', { class: 'avance', 'aria-label': ui.progreso }, lamparas.map((l) => l.li));
    const btnTodo = h('button', { type: 'button', class: 'tbtn', onclick: () => leerTodo() }, ui.leerTodo);
    const btnReiniciar = h('button', { type: 'button', class: 'tbtn tbtn--suave', onclick: () => reiniciar() }, ui.reiniciar);
    const barra = h('div', { class: 'viaje__barra' }, avance, btnTodo);
    const pie = h('div', { class: 'viaje__pie' }, btnReiniciar);
    cont.before(barra);
    cont.after(pie);

    const pintarAvance = () => {
      lamparas.forEach((l) => {
        const rev = ctrl[l.i] && ctrl[l.i].S.color;
        l.li.replaceChildren(...[
          h('span', { class: 'avance__n' }, String(l.i + 1)),
          rev ? lampara(l.c.color) : h('span', { class: 'lampara lampara--vacia', 'aria-hidden': 'true' }),
          rev ? h('span', { class: 'avance__txt' }, colores[l.c.color].nombre) : null,
          sr(lleno(ui.cambioN, { n: l.i + 1 }) + ': ' + (rev ? colores[l.c.color].nombre : ui.sinColor)),
        ].filter(Boolean));
      });
      // Starting over only makes sense once the last change has been answered.
      const ultimo = ctrl[total - 1];
      pie.hidden = !(ultimo && ultimo.S.resp);
    };

    // --- One change ---
    const crear = (art, i) => {
      const dato = D.cambios[i];
      const N = dato.pasos.length;
      const q = (s) => art.querySelector(s);
      const nodos = {
        titulo: q('.cambio__titulo'), razon: q('.cambio__razon'), sirojo: q('.cambio__sirojo'), pasos: q('.cambio__pasos'),
        juicio: q('.cambio__juicio'), respuesta: q('.cambio__respuesta'), toco: q('.cambio__toco'), blindar: q('.cambio__blindar'), cierre: q('.cambio__cierre'),
      };
      const lis = [...nodos.pasos.children];
      const S = { color: false, elegido: null, paso: 0, juicio: false, resp: null, blindarEjecutado: false, siguiente: false };
      let timers = [];
      let pasillo = null;
      let pasillo2 = null;
      const dinamicos = [];
      const anadir = (e) => { dinamicos.push(e); return e; };

      nodos.titulo.setAttribute('tabindex', '-1');
      nodos.pasos.setAttribute('aria-live', 'polite');
      nodos.pasos.setAttribute('aria-relevant', 'additions text');

      // Header: "1 of 4" and the way out.
      const btnEntero = anadir(h('button', { type: 'button', class: 'tbtn tbtn--suave', onclick: () => { entero(); enfocar(btnSiguienteCambio.hidden ? nodos.titulo : btnSiguienteCambio); } }, ui.verEntero));
      nodos.titulo.before(anadir(h('div', { class: 'cambio__cabeza' }, h('span', { class: 'cambio__n' }, lleno(ui.nDeM, { n: i + 1, m: total })), btnEntero)));

      // The question: which colour?
      const idP = unico('pregunta');
      const botonesColor = ['verde', 'amarillo', 'rojo'].map((c) => h('button', { type: 'button', class: 'eleccion', 'data-color': c, 'aria-pressed': 'false', onclick: () => elegir(c) }, lampara(c), colores[c].nombre));
      const eleccion = anadir(h('div', { class: 'cambio__eleccion', role: 'group', 'aria-labelledby': idP },
        h('p', { class: 'cambio__pide', id: idP }, h('strong', null, ui.pregunta)),
        h('div', { class: 'cambio__colores' }, botonesColor)));
      q('.cambio__plano').after(eleccion);
      const tuPondrias = anadir(h('p', { class: 'cambio__tupondrias', hidden: true }));
      nodos.razon.after(tuPondrias);

      // The corridor goes between the colour and the list of steps.
      pasillo = crearPasillo({ ui, puertas, colores, color: dato.color, contadores: true });
      const pasilloEl = anadir(pasillo.el);
      nodos.pasos.before(pasilloEl);

      // Next step.
      const btnSiguiente = anadir(h('button', { type: 'button', class: 'tbtn tbtn--principal', onclick: () => siguientePaso(true) }, ui.siguiente));
      const acciones = anadir(h('div', { class: 'cambio__acciones' }, btnSiguiente));
      nodos.pasos.after(acciones);

      // The verdict.
      const botonesJuicio = ['si', 'no'].map((r) => h('button', { type: 'button', class: 'tbtn', 'aria-pressed': 'false', 'data-resp': r, onclick: () => responder(r) }, ui[r]));
      const filaJuicio = anadir(h('div', { class: 'cambio__resp' }, botonesJuicio));
      nodos.juicio.firstElementChild.after(filaJuicio);

      // Harden: repeat the corridor in red.
      let btnBlindar = null;
      if (dato.blindar && nodos.blindar) {
        btnBlindar = anadir(h('button', { type: 'button', class: 'tbtn', onclick: () => blindar(true) }, dato.blindar.boton));
        const pre = nodos.blindar.querySelector('pre');
        const zona = anadir(h('div', { class: 'cambio__repite' }, btnBlindar));
        pre.after(zona);
        nodos.zonaBlindar = zona;
      }

      // Next change.
      const btnSiguienteCambio = anadir(h('button', { type: 'button', class: 'tbtn tbtn--principal', hidden: true, onclick: () => abrirSiguiente() }, ui.siguienteCambio));
      art.append(anadir(h('div', { class: 'cambio__fin' }, btnSiguienteCambio)));

      const hechoTodo = () => !!S.resp && (!dato.blindar || S.blindarEjecutado);

      const render = () => {
        const sinElegir = S.color && S.elegido == null;
        eleccion.hidden = sinElegir;
        botonesColor.forEach((b) => { b.disabled = S.color; b.setAttribute('aria-pressed', String(S.elegido === b.dataset.color)); });
        nodos.razon.hidden = !S.color;
        const otro = S.elegido && S.elegido !== dato.color;
        const rojo = S.elegido === 'rojo' && nodos.sirojo;
        tuPondrias.hidden = !(otro && !rojo);
        if (otro && !rojo) tuPondrias.textContent = lleno(ui.tuPondrias, { color: colores[S.elegido].nombre.toLowerCase() });
        if (nodos.sirojo) nodos.sirojo.hidden = !(S.color && rojo);
        pasilloEl.hidden = !S.color;
        lis.forEach((li, n) => { li.hidden = n >= S.paso; });
        nodos.pasos.hidden = S.paso === 0;
        acciones.hidden = !(S.color && !dato.auto && S.paso < N);
        nodos.juicio.hidden = !S.juicio;
        nodos.respuesta.hidden = !S.resp;
        filaJuicio.hidden = S.resp === 'salto';
        botonesJuicio.forEach((b) => { b.disabled = !!S.resp; b.setAttribute('aria-pressed', String(S.resp === b.dataset.resp)); });
        nodos.toco.hidden = !S.resp;
        if (nodos.blindar) {
          nodos.blindar.hidden = !S.resp;
          nodos.cierre.hidden = !S.blindarEjecutado;
          btnBlindar.disabled = S.blindarEjecutado;
        }
        btnEntero.hidden = hechoTodo() || S.resp === 'salto';
        btnSiguienteCambio.hidden = !(hechoTodo() && i < total - 1 && !S.siguiente);
        art.dataset.fase = hechoTodo() ? 'hecho' : S.resp ? 'blindar' : S.juicio ? 'juicio' : S.color ? 'pasos' : 'inicio';
        pintarAvance();
      };

      const parar = () => { timers.forEach(clearTimeout); timers = []; };
      const elegir = (c) => {
        S.elegido = c;
        revelarColor();
        enfocar(dato.auto ? btnEntero : btnSiguiente);
      };
      const revelarColor = () => {
        S.color = true;
        render();
        if (dato.auto) {
          if (reducido()) { todosLosPasos(); return; }
          const tic = () => {
            if (S.paso >= N) return;
            siguientePaso(true);
            if (S.paso < N) timers.push(setTimeout(tic, 900));
          };
          timers.push(setTimeout(tic, 900));
        }
      };
      const siguientePaso = (animar) => {
        if (S.paso >= N) return;
        const p = dato.pasos[S.paso];
        pasillo.aplicar(p, { animar: animar && !reducido(), ultimo: S.paso === N - 1 });
        S.paso += 1;
        if (S.paso >= N) {
          const tenia = art.contains(document.activeElement);
          S.juicio = true;
          render();
          if (tenia) enfocar(botonesJuicio[0]);
          return;
        }
        render();
      };
      const todosLosPasos = () => {
        parar();
        pasillo.fijar(dato.pasos, N);
        S.paso = N; S.juicio = true;
        render();
      };
      const responder = (r) => {
        S.resp = r;
        render();
        enfocar(btnBlindar || (btnSiguienteCambio.hidden ? null : btnSiguienteCambio));
      };
      const blindar = (animar) => {
        if (S.blindarEjecutado) return;
        const b = dato.blindar;
        pasillo2 = crearPasillo({ ui, puertas, colores, color: b.color, contadores: false });
        nodos.zonaBlindar.append(anadir(pasillo2.el));
        const pasos = [{ puerta: puertas[0].id, tipo: 'pasa' }, { puerta: b.paraEn, tipo: 'para' }];
        S.blindarEjecutado = true;
        if (animar && !reducido()) {
          pasillo2.aplicar(pasos[0], { animar: true, ultimo: false });
          timers.push(setTimeout(() => { pasillo2.aplicar(pasos[1], { animar: true, ultimo: true }); }, 900));
        } else {
          pasillo2.fijar(pasos, 2);
        }
        render();
        if (animar) enfocar(btnSiguienteCambio.hidden ? null : btnSiguienteCambio);
      };
      const entero = () => {
        parar();
        if (!S.color) { S.color = true; render(); }
        todosLosPasos();
        if (!S.resp) S.resp = 'salto';
        if (dato.blindar && !S.blindarEjecutado) blindar(false);
        render();
      };
      const abrirSiguiente = () => {
        S.siguiente = true;
        render();
        mostrar(i + 1);
      };
      const quitar = () => {
        parar();
        pasillo.flush();
        dinamicos.splice(0).forEach((e) => e.remove());
        lis.forEach((li) => { li.hidden = false; });
        [nodos.razon, nodos.sirojo, nodos.pasos, nodos.juicio, nodos.respuesta, nodos.toco, nodos.blindar, nodos.cierre].forEach((e) => { if (e) e.hidden = false; });
        nodos.pasos.removeAttribute('aria-live');
        nodos.pasos.removeAttribute('aria-relevant');
      };
      render();
      return { S, art, render, entero, quitar, parar, titulo: nodos.titulo, primero: botonesColor[0] };
    };

    const enfocar = (e) => { if (e && !e.hidden && !e.disabled) e.focus({ preventScroll: true }); };
    const mostrar = (n) => {
      visibles = Math.max(visibles, n + 1);
      arts.forEach((a, k) => { a.hidden = k >= visibles; });
      const c = ctrl[n];
      c.titulo.focus({ preventScroll: true });
      c.art.scrollIntoView({ behavior: smooth(), block: 'start' });
    };

    let ctrl = [];
    const montar = () => {
      ctrl = arts.map((a, i) => crear(a, i));
      visibles = 1;
      arts.forEach((a, k) => { a.hidden = k >= visibles; });
      pintarAvance();
    };
    const leerTodo = () => {
      visibles = total;
      arts.forEach((a) => { a.hidden = false; });
      ctrl.forEach((c) => { c.S.siguiente = true; c.entero(); });
      ctrl[0].titulo.focus({ preventScroll: true });
      ctrl[0].art.scrollIntoView({ behavior: smooth(), block: 'start' });
    };
    const reiniciar = () => {
      globales.splice(0).forEach(clearTimeout);
      ctrl.forEach((c) => c.quitar());
      montar();
      barra.scrollIntoView({ behavior: smooth(), block: 'start' });
      ctrl[0].titulo.focus({ preventScroll: true });
    };

    montar();
    raiz.dataset.listo = '';
    raiz.classList.add('tutorial--js');
  };

  document.querySelectorAll('.tutorial--puertas').forEach(montarPuertas);
  document.querySelectorAll('.tutorial--viaje').forEach(montarViaje);
})();
