// The door of the opening plays by itself, so it can be paused.
(() => {
  const heroe = document.querySelector('.heroe');
  const boton = heroe?.querySelector('[data-heroe-pausa]');
  if (!boton) return;
  boton.addEventListener('click', () => {
    const pausada = heroe.dataset.pausa !== 'si';
    heroe.dataset.pausa = pausada ? 'si' : 'no';
    const ingles = document.documentElement.lang === 'en';
    boton.textContent = pausada ? (ingles ? 'Resume' : 'Seguir') : (ingles ? 'Pause' : 'Pausar');
  });
})();

// The floor plan on the home page: one run through the doors for each level of risk.
// Every step only declares what changes; the state of a step is the sum of those before it.
(() => {
  const figura = document.getElementById('plano');
  if (!figura) return;

  const PARADAS = {
    inicio: 16.25, ante1: 25.5, tras1: 41.25, ante2: 55.5, atras2: 45, tras2: 71.875, ante3: 81.75, fuera: 95.5,
  };

  const t = (es, en) => (document.documentElement.lang === 'en' ? en : es);

  // From less to more: each colour only adds what the one before did not have.
  const GUIONES = {
    verde: {
      base: { p1: 'abierta', p2: 'cerrada', p3: 'abierta', agente: 'inicio', llamadas: 0 },
      pasos: [
        { texto: t('Poco riesgo: algunas puertas quedan abiertas.', 'Low risk: some gates stay open.'), ms: 3200 },
        { texto: t('El agente dice "hecho". Decirlo no abre nada.', 'The agent says "done." Saying it opens nothing.'), agente: 'ante2', globo: true, ms: 3400 },
        { texto: t('Así se pasa: no por lo que dijo el agente, sino por lo que comprobó la puerta.', 'That is how you get through: not by what the agent said, but by what the gate checked.'), p2: 'ganada', globo: false, ms: 4400 },
        { texto: t('Sale, y a ti no te ha llamado nadie.', 'It ships, and nobody called you.'), agente: 'fuera', ms: 3400 },
      ],
    },
    amarillo: {
      base: { p1: 'cerrada', p2: 'cerrada', p3: 'cerrada', agente: 'inicio', llamadas: 0 },
      pasos: [
        { texto: t('Antes de empezar, te llama: lees unos pocos escenarios.', 'Before it starts, it calls you: you read a few scenarios.'), llamada: 1, llamadas: 1, ms: 3800 },
        { texto: t('Pasa, y dice "hecho". Aquí la puerta suma otra comprobación: un vigilante.', 'It gets through and says "done." Here the gate adds another check: a reviewer.'), llamada: 0, p1: 'ganada', agente: 'ante2', globo: true, ms: 4400 },
        { texto: t('El vigilante ejecuta el cambio y no funciona. Nadie te llama: el agente lo arregla.', 'The reviewer runs the change and it does not work. Nobody calls you: the agent fixes it.'), p2: 'rechaza', globo: false, rechazo: t('KO del vigilante', 'Reviewer says no'), agente: 'atras2', ms: 5000 },
        { texto: t('Ahora sí funciona, y el vigilante da su aprobación.', 'Now it works, and the reviewer gives its approval.'), p2: 'ganada', rechazo: '', agente: 'ante2', ms: 3400 },
        { texto: t('Sale. Solo te ha llamado cuando hacía falta tu criterio.', 'It ships. It only called you when your judgment was needed.'), p3: 'ganada', agente: 'fuera', ms: 3800 },
      ],
    },
    rojo: {
      base: { p1: 'cerrada', p2: 'cerrada', p3: 'cerrada', agente: 'inicio', llamadas: 0 },
      pasos: [
        { texto: t('Te llama antes de empezar: lees los escenarios y los corriges.', 'It calls you before it starts: you read the scenarios and correct them.'), llamada: 1, llamadas: 1, ms: 3800 },
        { texto: t('Pasa la primera puerta y dice "hecho".', 'It gets through the first gate and says "done."'), llamada: 0, p1: 'ganada', agente: 'ante2', globo: true, ms: 3000 },
        { texto: t('La puerta dice que no: falla un test.', 'The gate says no: a test fails.'), p2: 'rechaza', globo: false, rechazo: t('Falla un test', 'A test fails'), ms: 3200 },
        { texto: t('Nadie te llama: el error vuelve al agente y lo arregla.', 'Nobody calls you: the error goes back to the agent, and it fixes it.'), rechazo: '', p2: 'cerrada', agente: 'atras2', ms: 3600 },
        { texto: t('Así se pasa: no por lo que dijo el agente, sino por lo que comprobó la puerta.', 'That is how you get through: not by what the agent said, but by what the gate checked.'), p2: 'ganada', agente: 'ante2', ms: 4400 },
        { texto: t('Como toca el cobro, te llama otra vez: sin tu aprobación no sale.', 'Because it touches payment, it calls you again: without your approval it does not ship.'), agente: 'ante3', llamada: 3, llamadas: 2, ms: 3800 },
        { texto: t('Compruebas que se puede deshacer, das tu aprobación y sale.', 'You check it can be undone, give your approval, and it ships.'), llamada: 0, p3: 'ganada', agente: 'fuera', ms: 3800 },
        { texto: t('Una semana después aparece un error que se coló.', 'A week later, an error that slipped through turns up.'), fuga: true, ms: 3800 },
        { texto: t('Añades lo que lo habría parado, y por ahí ya no vuelve a entrar.', "You add what would have stopped it, and it won't get in that way again."), blindaje: true, fuga: false, ms: 4600 },
      ],
    },
  };
  const NOMBRES = { verde: t('Verde', 'Green'), amarillo: t('Amarillo', 'Yellow'), rojo: t('Rojo', 'Red') };
  const PROXIMO = { verde: 'amarillo', amarillo: 'rojo', rojo: null };

  const q = (selector) => figura.querySelector(selector);
  const qq = (selector) => [...figura.querySelectorAll(selector)];
  const relato = q('[data-relato]');
  const paso = q('[data-paso]');
  const radios = qq('input[name="riesgo"]');
  const agente = q('.plano__agente');
  const globo = q('.plano__globo');
  const nombre = q('.plano__nombre');
  const rechazo = q('.plano__rechazo');
  const notaFuga = q('.plano__nota--fuga');
  const cuenta = q('[data-cuenta]');
  const avance = q('[data-avance]');
  const mandos = q('[data-mandos]');
  const pausa = q('[data-accion="pausa"]');
  const atras = q('[data-accion="atras"]');
  const adelante = q('[data-accion="adelante"]');

  const sinMovimiento = window.matchMedia('(prefers-reduced-motion: reduce)');
  let riesgo = 'verde';
  let indice = 0;
  let enMarcha = !sinMovimiento.matches;
  let visible = false;
  let reloj = null;

  const estadoEn = (guion, i) => {
    const estado = { globo: false, rechazo: '', llamada: 0, fuga: false, blindaje: false, ...guion.base };
    guion.pasos.slice(0, i + 1).forEach((paso) => Object.assign(estado, paso));
    return estado;
  };

  const pintar = () => {
    const guion = GUIONES[riesgo];
    const estado = estadoEn(guion, indice);
    figura.dataset.riesgo = riesgo;
    qq('.puerta').forEach((puerta) => { puerta.dataset.estado = estado[`p${puerta.dataset.puerta}`]; });
    figura.dataset.llamada = String(estado.llamada);
    figura.dataset.fuga = String(estado.fuga);
    figura.dataset.blindaje = String(estado.blindaje);
    agente.style.setProperty('--x', `${PARADAS[estado.agente]}%`);
    nombre.hidden = estado.agente !== 'inicio';
    globo.hidden = !estado.globo;
    rechazo.hidden = !estado.rechazo;
    rechazo.querySelector('span').textContent = estado.rechazo;
    notaFuga.hidden = !estado.fuga;
    paso.textContent = t(`${NOMBRES[riesgo]}, paso ${indice + 1} de ${guion.pasos.length}`, `${NOMBRES[riesgo]}, step ${indice + 1} of ${guion.pasos.length}`);
    relato.textContent = estado.texto;
    cuenta.textContent = String(estado.llamadas);
    qq('[data-fila]').forEach((fila) => {
      const n = fila.dataset.fila;
      fila.querySelector('.plano__llave').hidden = estado[`p${n}`] !== 'ganada';
      fila.classList.toggle('es-activa', estado[`p${n}`] === 'rechaza' || estado.llamada === Number(n));
    });
    q('.plano__nueva').hidden = !estado.blindaje;
    qq('[data-en]').forEach((el) => { el.hidden = !el.dataset.en.split(' ').includes(riesgo); });
    avance.style.width = `${((indice + 1) / guion.pasos.length) * 100}%`;
    atras.disabled = indice === 0;
    const alFinal = indice === guion.pasos.length - 1;
    const proximo = alFinal ? PROXIMO[riesgo] : null;
    adelante.disabled = alFinal && !proximo;
    adelante.textContent = proximo ? t(`Ahora, ${NOMBRES[proximo].toLowerCase()}`, `Now, ${NOMBRES[proximo].toLowerCase()}`) : t('Siguiente', 'Next');
    adelante.classList.toggle('mando--fuerte', Boolean(proximo));
    pausa.textContent = alFinal ? t('Volver a verlo', 'Watch again') : (enMarcha ? t('Pausar', 'Pause') : t('Seguir', 'Resume'));
    // Announce a step only when the reader moved there; never while it plays by itself.
    relato.setAttribute('aria-live', enMarcha ? 'off' : 'polite');
  };

  // A button that switches off while it has the focus would leave the keyboard nowhere to go.
  const conservarFoco = () => {
    if (document.activeElement === adelante && adelante.disabled) pausa.focus();
    if (document.activeElement === atras && atras.disabled) adelante.focus();
  };

  const parar = () => { clearTimeout(reloj); reloj = null; };
  const programar = () => {
    parar();
    if (!enMarcha || !visible) return;
    const guion = GUIONES[riesgo];
    if (indice >= guion.pasos.length - 1) { enMarcha = false; pintar(); return; }
    reloj = setTimeout(() => { indice += 1; pintar(); programar(); }, guion.pasos[indice].ms);
  };

  const desdeElPrincipio = (reproducir) => {
    indice = 0;
    enMarcha = reproducir;
    pintar();
    programar();
  };

  radios.forEach((radio) => radio.addEventListener('change', () => {
    riesgo = radio.value;
    desdeElPrincipio(!sinMovimiento.matches);
  }));

  pausa.addEventListener('click', () => {
    if (indice === GUIONES[riesgo].pasos.length - 1) { desdeElPrincipio(!sinMovimiento.matches); return; }
    enMarcha = !enMarcha;
    pintar();
    programar();
  });
  atras.addEventListener('click', () => { enMarcha = false; indice = Math.max(0, indice - 1); pintar(); conservarFoco(); programar(); });
  adelante.addEventListener('click', () => {
    // At the end of a colour the button leads to the next one.
    if (indice === GUIONES[riesgo].pasos.length - 1 && PROXIMO[riesgo]) {
      riesgo = PROXIMO[riesgo];
      radios.find((radio) => radio.value === riesgo).checked = true;
      desdeElPrincipio(!sinMovimiento.matches);
      return;
    }
    enMarcha = false;
    indice = Math.min(GUIONES[riesgo].pasos.length - 1, indice + 1);
    pintar();
    conservarFoco();
    programar();
  });

  new IntersectionObserver(([entrada]) => {
    visible = entrada.isIntersecting;
    if (visible) programar(); else parar();
  }, { threshold: 0.25 }).observe(q('.plano__lienzo'));

  radios.find((radio) => radio.value === riesgo).checked = true;
  mandos.hidden = false;
  figura.classList.add('con-script');
  pintar();
  programar();

  // For the checks: the words on screen must come from the document.
  window.__guiones = GUIONES;
})();
