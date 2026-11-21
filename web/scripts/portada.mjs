// The home page: the landing of the design canvas (November 2026), in both languages.
// What the method says comes from the manifesto through the build's helpers, so it cannot drift
// from it: the promise, the problem, the basis, the principles and the four questions. The rest
// is written here, Spanish and English side by side. Every figure carries its source in its block.

const ic = (d) => `<svg class="g-ic" viewBox="0 0 24 24" aria-hidden="true">${d}</svg>`;
const flecha = ic('<path d="M5 12h14"></path><path d="m12 5 7 7-7 7"></path>');
const abajo = ic('<path d="M12 5v14"></path><path d="m19 12-7 7-7-7"></path>');
const visto = ic('<path d="M20 6 9 17l-5-5"></path>');

// A wall with its door, drawn above each room; it opens as the room comes into view.
const muro = (derecha) => `<div class="g-wall${derecha ? ' g-wall--r' : ''}"><i></i><svg viewBox="0 0 84 84" aria-hidden="true"><path class="g-arc" d="M84 5A84 84 0 0 1 0 89"></path><path class="g-leaf" d="M0 5H84"></path></svg><i></i></div>`;
const sala = (cuerpo, { derecha = false, clase = '', id = '', lamina = false } = {}) => `<section class="g-sec${clase}"${id ? ` id="${id}"` : ''}${lamina ? ' data-gdd-sheet' : ''}>
<span class="g-axis" aria-hidden="true"></span>
<div class="g-wrap">${muro(derecha)}<div class="g-in">${cuerpo}</div></div>
</section>`;
const dos = (lado, principal) => `<div class="g-body"><div class="g-side">${lado}</div><div class="g-main">${principal}</div></div>`;
const fuente = (href, texto) => `<a class="g-src" href="${href}">${texto}</a>`;
const hoja = (n) => `<span class="g-part" aria-hidden="true"><i></i><b><svg viewBox="0 0 ${n} ${n}"><path class="g-arc" d="M0 ${n}A${n} ${n} 0 0 0 ${n} 0"></path><path class="g-leaf" d="M0 0V${n}"></path></svg></b><i></i></span>`;

const vineta = (etiqueta, dibujo, pie) => `<figure><svg viewBox="0 0 320 112" role="img" aria-label="${etiqueta}"><path class="pl pl-w" d="M8 12H312M8 100H312M210 12V34M210 78V100"></path>${dibujo}</svg><figcaption>${pie}</figcaption></figure>`;
const puntos = (xs) => xs.map((x) => `<circle class="pl-d" cx="${x}" cy="56" r="6"></circle>`).join('');

// A sentence ends at its stop, also inside closing quotes ("done." in English).
const frases = (t) => t.split(/(?<=[.!?]["”»]?) /);

export const portada = ({ tr, ruta, menu, texto, frase, interior, manifiesto, plano, version, estado }) => {
  const [, dice, comprueba] = frases(texto('manifiesto', tr('Añade puertas a tu proyecto', 'Add gates to your project')));
  const [p0, p1, p2, p3] = frases(texto('manifiesto', tr('Los agentes escriben rápido', 'Agents write fast')));
  const queEs = frase('manifiesto', tr('Gate-Driven Development (GDD) es una forma', 'Gate-Driven Development (GDD) is a way'));
  const [inicioBase, minuscula, mayuscula] = tr(['Esa es la base de GDD: ', 'no desarrollamos', 'No desarrollamos'], ['That is the basis of GDD: ', "we don't", "We don't"]);
  const base = interior('manifiesto', inicioBase.trim()).replace(inicioBase, '').replace(minuscula, mayuscula);
  const tituloDe = (h) => manifiesto('h2').filter((_, x) => manifiesto(x).text().trim() === h).first();
  const principios = tituloDe(tr('Los siete principios', 'The seven principles')).nextAll('ol').first().find('li > strong').map((_, s) => manifiesto(s).text().trim()).get();
  const preguntas = manifiesto('p').filter((_, x) => manifiesto(x).text().includes(tr('hazte cuatro preguntas', 'ask yourself four questions'))).first().next('ul').find('li').map((_, li) => manifiesto(li).text().trim()).get();
  if (principios.length !== 7 || preguntas.length !== 4) throw new Error(`La portada no encuentra en el manifiesto los siete principios (${principios.length}) o las cuatro preguntas (${preguntas.length})`);
  const recorrido = tr('recorrido', 'walkthrough');
  const g = 'la-idea';

  const heroe = `<section class="g-blue g-hero" data-gdd-sheet aria-labelledby="portada-titulo">
<div class="g-wrap">
  <div class="g-plan">
    <div class="g-room">
      <h1 class="g-h1 g-room__h" id="portada-titulo"><span>${dice}</span> <span>${comprueba}</span></h1>
      <p class="g-lead">${queEs} ${tr('Antes de que el agente empiece, decides qué tiene que ser verdad. Para pasar, tiene que demostrarlo ante algo que él no puede tocar.', 'Before the agent starts, you decide what has to be true. To get through, it has to prove it before something it cannot touch.')}</p>
      <div class="g-cta">
        <a class="g-btn" href="${ruta('empieza')}">${tr('Empieza con tus agentes', 'Start with your agents')} ${flecha}</a>
        <a class="g-link" href="#${recorrido}">${tr('Míralo en marcha', 'See it in action')} ${abajo}</a>
      </div>
    </div>
    <div class="g-corr" aria-hidden="true">
      <span class="g-ent"></span>
      <span class="g-track"></span>
      <span class="g-lbl g-rl g-rl--a">${tr('Agente', 'Agent')}</span>
      <span class="g-lbl g-rl g-rl--p">${tr('Producción', 'Production')}</span>
      <span class="g-part"><i></i><b><svg viewBox="0 0 240 240"><path class="g-arc" d="M0 240A240 240 0 0 0 240 0"></path><path class="g-leaf" d="M0 0V240"></path></svg></b><i></i></span>
      <span class="g-chk">
        <span class="g-chk__ring">
          <svg class="g-ic lk-idle" viewBox="0 0 24 24"><rect width="18" height="11" x="3" y="11" rx="2" ry="2"></rect><path d="M7 11V7a5 5 0 0 1 10 0v4"></path></svg>
          <svg class="g-ic lk-fail" viewBox="0 0 24 24"><path d="M18 6 6 18"></path><path d="m6 6 12 12"></path></svg>
          <svg class="g-ic lk-pass" viewBox="0 0 24 24"><path d="M20 6 9 17l-5-5"></path></svg>
        </span>
        <span><span class="g-lbl">${tr('Comprobación', 'Check')}</span><span class="g-chk__st"><span class="lk-idle">${tr('cerrada', 'closed')}</span><span class="lk-fail">${tr('no pasa', 'fails')}</span><span class="lk-pass">${tr('pasa', 'passes')}</span></span></span>
      </span>
      <span class="g-agent"><span class="g-say">${tr('"hecho"', '"done"')}</span><span class="g-dot"></span></span>
      <span class="g-caj"><span>${tr('Manifiesto', 'Manifesto')} ${version}</span><span>${estado.charAt(0).toUpperCase()}${estado.slice(1)}</span></span>
    </div>
    <button type="button" class="g-ghost g-pause" aria-pressed="false" data-portada-pausa hidden>
      ${ic('<rect x="14" y="4" width="4" height="16" rx="1"></rect><rect x="6" y="4" width="4" height="16" rx="1"></rect>').replace('g-ic', 'g-ic when-off')}
      ${ic('<path d="M6 3l14 9-14 9V3z"></path>').replace('g-ic', 'g-ic when-on')}
      <span>${tr('Pausar la animación', 'Pause the animation')}</span>
    </button>
  </div>
  <div class="g-dim g-lbl" aria-hidden="true"><span><span class="lg">${tr('Lo que el agente dice', 'What the agent says')}</span><span class="sm">${tr('Dicho', 'Said')}</span></span><span>${tr('Comprobado', 'Checked')}</span></div>
  <ol class="g-steps">
    <li><span class="g-num">1</span><span><strong>${tr('El agente dice "hecho".', 'The agent says "done."')}</strong>${tr('Decirlo no abre nada.', 'Saying it opens nothing.')}</span></li>
    <li><span class="g-num">2</span><span><strong>${tr('La puerta lo comprueba.', 'The gate checks it.')}</strong>${tr('No pasa, y vuelve.', 'It fails, and goes back.')}</span></li>
    <li><span class="g-num">3</span><span><strong>${tr('Lo arregla.', 'It fixes it.')}</strong>${tr('Ahora pasa la comprobación.', 'Now it passes the check.')}</span></li>
    <li><span class="g-num">4</span><span><strong>${tr('La puerta se abre.', 'The gate opens.')}</strong>${tr('Sin que hayas estado encima.', 'Without you standing over it.')}</span></li>
  </ol>
</div>
</section>`;

  // The teams that already work this way, first thing after the promise: the method orders what
  // works, and each line says which piece of GDD it is.
  const equipos = [
    ['Stripe', tr('Más de mil PR a la semana escritas enteras por agentes. Los pasos fijos, como git, los linters y los tests, no los decide el modelo; hay dos rondas de CI como mucho, y una persona revisa.', 'More than a thousand PRs a week written entirely by agents. The fixed steps, such as git, linters and tests, are not up to the model; there are two CI rounds at most, and a person reviews.'), tr('Las puertas, fuera del alcance del agente', "Gates, out of the agent's reach"), 'https://stripe.dev/blog/minions-stripes-one-shot-end-to-end-coding-agents', 'Stripe, 2026'],
    ['Anthropic', tr('De unos 2.500 cambios fusionados al día en mayo de 2026 a más de 30.000 en septiembre. Su revisión distingue un cambio de comentarios de uno lógico y delicado, y unos permisos por directorio protegen el código más sensible.', 'From around 2,500 changes merged a day in May 2026 to more than 30,000 in September. Its review tells a comment-only change from a sensitive logical one, and permissions per directory protect the most sensitive code.'), tr('Los colores y las zonas del plano', "The blueprint's colors and zones"), 'https://www.anthropic.com/engineering/redesigning-source-control', tr('Anthropic, octubre de 2026', 'Anthropic, October 2026')],
    ['OpenAI', tr('"Humans steer. Agents execute." La ingeniería pasa a diseñar el entorno del agente, especificar la intención y construir los bucles que lo comprueban.', '"Humans steer. Agents execute." Engineering moves to designing the agent\'s environment, specifying the intent and building the loops that check it.'), tr('El plano y sus puertas', 'The blueprint and its gates'), 'https://openai.com/index/harness-engineering/', 'OpenAI, 2026'],
    ['Google', tr('Publica en abierto un plugin, Elephant-Goldfish Model, que separa a quien redacta, con todo el contexto, de quienes revisan, que empiezan sin la conversación anterior.', 'It publishes an open plugin, Elephant-Goldfish Model, that separates who drafts, with the whole context, from who reviews, starting without the earlier conversation.'), tr('El vigilante, con el contexto limpio', 'The reviewer, with a clean context'), 'https://github.com/google/elephant-goldfish-model', tr('Elephant-Goldfish Model, en GitHub', 'Elephant-Goldfish Model, on GitHub')],
    ['Thoughtworks', tr('Birgitta Böckeler describe lo que rodea a un agente como guías antes de actuar y sensores después, y deja abierta una pregunta: si un sensor que nunca salta indica calidad o una detección floja.', 'Birgitta Böckeler describes what surrounds an agent as guides before it acts and sensors afterwards, and leaves a question open: whether a sensor that never fires means quality or weak detection.'), tr('Ver cerrar cada puerta antes de fiarse de ella', 'Seeing each gate close before trusting it'), 'https://martinfowler.com/articles/harness-engineering.html', tr('Böckeler, en martinfowler.com', 'Böckeler, on martinfowler.com')],
  ];
  const junta = [
    tr('Quien escribe la prueba no es quien la pasa.', 'Whoever writes the test is not whoever passes it.'),
    tr('Antes de fiarte de una puerta, la ves rechazar algo mal hecho.', 'Before you trust a gate, you see it reject something done wrong.'),
    tr('Antes de empezar se decide qué hay que demostrar, y la exigencia sube con el riesgo.', 'What has to be proven is decided before starting, and the demand rises with the risk.'),
    frase('manifiesto', tr('Un cambio no se da por cerrado', "A change isn't closed")),
    tr('Una comprobación se retira con la misma evidencia que se pide para añadirla.', 'A check is retired with the same evidence asked to add it.'),
  ];
  const sector = sala(dos(
    `<h2 class="g-h2">${tr('Quienes trabajan con agentes ya hacen partes de GDD.', 'People who work with agents already do parts of GDD.')}</h2>
    <p class="g-p">${tr('Controles fuera del agente, exigencia según el riesgo y una persona donde importa. GDD le pone a eso un lenguaje común, una prueba para saber si una puerta vale y un recorrido.', 'Controls outside the agent, demands that follow the risk, and a person where it matters. GDD gives that a common language, a test to know whether a gate holds, and a path to follow.')}</p>
    <p><a class="g-link" href="${ruta('de-donde-viene')}">${tr('Más fuentes, con sus enlaces', 'More sources, with their links')} ${flecha}</a></p>`,
    `<ul class="g-rows g-equipos">
      ${equipos.map(([quien, que, gdd, href, cita]) => `<li><h3 class="g-h3">${quien}</h3><div class="g-p"><p>${que} ${fuente(href, cita)}</p><p class="g-engdd"><span class="g-lbl">${tr('En GDD', 'In GDD')}</span> ${gdd}</p></div></li>`).join('\n      ')}
    </ul>
    <div class="g-sub">
      <h3 class="g-h3">${tr('Lo que GDD junta', 'What GDD brings together')}</h3>
      <p class="g-p">${tr('En GDD, estas cinco piezas van juntas:', 'In GDD, these five pieces go together:')}</p>
      <ul class="g-test">${junta.map((j) => `<li>${visto}<span>${j}</span></li>`).join('')}</ul>
    </div>`,
  ));

  const problema = sala(dos(
    `<h2 class="g-h2">${p0}</h2>`,
    `<p class="g-lead">${tr('No es una impresión tuya. En la encuesta de Stack Overflow de 2025, el <strong>66&nbsp;%</strong> de los desarrolladores señaló como su mayor frustración las soluciones de IA "casi correctas, pero no del todo".', 'It isn\'t just you. In Stack Overflow\'s 2025 survey, <strong>66%</strong> of developers named AI solutions that are "almost right, but not quite" as their biggest frustration.')} ${fuente('https://survey.stackoverflow.co/2025/ai', tr('Stack Overflow, 2025', 'Stack Overflow, 2025'))}</p>
    <div class="g-vig">
      ${vineta(tr('Una sola puerta contigo delante: los cambios hacen cola.', 'A single gate with you in front of it: the changes queue up.'), `<path class="pl pl-t" d="M210 78A44 44 0 0 0 254 34"></path><path class="pl pl-l" d="M210 34V78"></path>${puntos([48, 74, 100, 126, 152, 178])}<circle class="pl-you" cx="210" cy="56" r="10"></circle>`, p1)}
      ${vineta(tr('Un hueco sin puerta: pasa todo, también un error.', 'A gap with no gate: everything gets through, an error too.'), `<path class="pl pl-dash" d="M210 34V78"></path>${puntos([70, 118, 166, 240])}<circle class="pl-bad" cx="284" cy="56" r="6"></circle><path class="pl-badx" d="M278 30l12 12M290 30l-12 12"></path>`, p2)}
      ${vineta(tr('La comprobación está del lado del agente: se abre la puerta él mismo.', "The check is on the agent's side: it opens the gate itself."), `<path class="pl pl-t" d="M210 78A44 44 0 0 0 254 34"></path><path class="pl pl-l" d="M210 34H254"></path>${puntos([70, 118, 166, 248, 288])}<svg x="150" y="64" width="32" height="32" viewBox="0 0 24 24"><rect class="pl" style="stroke-width: 2.2" width="18" height="11" x="3" y="11" rx="2" ry="2"></rect><path class="pl" style="stroke-width: 2.2" d="M7 11V7a5 5 0 0 1 10 0v4"></path></svg>`, p3)}
    </div>
    <p class="g-punch">${tr('A un agente no se le convence con palabras. Se le ponen puertas.', "You don't convince an agent with words. You set up gates.")}</p>`,
  ));

  const como = [
    [tr('Antes de empezar, decides qué tiene que ser verdad.', 'Before it starts, you decide what has to be true.'), tr('Que un cobro se haga una sola vez. Que un total nunca sea negativo. Lo escribes antes de que el agente toque nada.', 'That a charge happens only once. That a total is never negative. You write it before the agent touches anything.')],
    [tr('Lo demuestra algo que el agente no puede tocar.', "It's proven by something the agent can't touch."), tr('Un test, un linter o la revisión de otra persona: cosas que ya usas y que no dependen de lo que diga el agente.', "A test, a linter or another person's review: things you already use, which don't depend on what the agent says.")],
    [tr('A más riesgo, más comprobaciones.', 'More risk, more checks.'), tr('Si el cambio es una errata, casi ni te enteras. Si toca el cobro, le pides más, y puede que te llame.', 'If the change is a typo, you barely notice. If it touches billing, you ask for more, and it may call you.')],
    [tr('Lo que se cuela, se blinda.', 'What slips through gets hardened.'), tr('Añades la comprobación que lo habría parado, y por ahí ya no vuelve a entrar. La que ya no aporta, la rebajas o la retiras.', "You add the check that would have stopped it, and it won't get in that way again. One that no longer adds anything, you relax or retire.")],
  ];
  const idea = sala(dos(
    `<h2 class="g-h2">${tr('Una puerta es lo que hay que demostrar antes de seguir.', 'A gate is what has to be proven before going on.')}</h2>
    <blockquote class="g-quote">${base}</blockquote>`,
    `<ol class="g-how">${como.map(([h, p], i) => `<li><span class="g-num">${i + 1}</span><div><h3 class="g-h3">${h}</h3><p class="g-p">${p}</p></div></li>`).join('')}</ol>`,
  ), { derecha: true });

  const enMarcha = sala(`<div class="g-top">
      <h2 class="g-h2">${tr('Manda el riesgo, no el tamaño del cambio.', 'Risk decides, not the size of the change.')}</h2>
      <p class="g-lead" id="eleccion-titulo">${tr('Elige qué le pides al agente y mira qué puertas se ponen delante, y cuántas veces te llama.', 'Choose what you ask the agent and see which gates stand in front of it, and how many times it calls you.')}</p>
    </div>
    ${plano(tr, {
      pie: `${tr('Tres puertas de ejemplo: en tu proyecto habrá las que hagan falta.', 'Three example gates: your project will have the ones it needs.')} ${interior('manifiesto', tr('Lo que no se puede deshacer nunca es verde.', "What can't be undone is never green."))}`,
      tocaVerde: interior(g, tr('Nada: pides el cambio', 'Nothing: you ask for the change')),
      tocaAmarillo: interior(g, tr('Aprobar sus reglas antes', 'Approve its rules before')),
      tocaRojo: interior(g, tr('Aprobar sus reglas y revisar', 'Approve its rules, and review')),
    })}`, { clase: ' g-blue g-rec', id: recorrido, lamina: true });

  const filas = [
    [tr('Revisas cada diff, o ninguno.', 'You review every diff, or none.'), tr('Revisas donde tu plano lo pide.', 'You review where your blueprint asks for it.')],
    [tr('"Hecho" es lo que dice el agente.', '"Done" is what the agent says.'), tr('"Hecho" es lo que ha pasado la comprobación.', '"Done" is what got through the check.')],
    [tr('Las reglas viven en el prompt, y el agente puede ignorarlas.', 'The rules live in the prompt, and the agent can ignore them.'), tr('Una regla solo cuenta si algo la comprueba.', 'A rule only counts if something checks it.')],
    [tr('El mismo fallo vuelve en otro cambio.', 'The same failure comes back in another change.'), tr('Lo que se cuela se convierte en comprobación, para que por ahí no vuelva a entrar.', "What slips through becomes a check, so it won't get in that way again.")],
    [tr('Nadie sabe bien quién aprobó qué.', 'Nobody quite knows who approved what.'), frase('manifiesto', tr('De cada cambio responde su autor', 'Every change has an author')).replace(tr(', la persona que lo tiene a su cargo:', ', the person who owns it:'), '.').replace(tr(' la IA', ' the AI'), tr(' La IA', ' The AI'))],
  ];
  const dia = sala(dos(
    `<h2 class="g-h2">${tr('Lo que cambia en tu día.', 'What changes in your day.')}</h2>
    <p class="g-p">${tr('Para empezar no necesitas herramientas nuevas: las puertas se hacen con lo que ya tienes en el repositorio, como tests, linters, CI, ramas protegidas y revisión.', "You don't need new tools to start: gates are made with what your repository already has, such as tests, linters, CI, protected branches and review.")}</p>`,
    `<table class="g-tbl g-ba"><thead><tr><th class="g-lbl" scope="col">${tr('Sin puertas', 'Without gates')}</th><th class="g-lbl" scope="col">${tr('Con puertas', 'With gates')}</th></tr></thead>
    <tbody>${filas.map(([a, b]) => `<tr><td>${a}</td><td>${b}</td></tr>`).join('')}</tbody></table>`,
  ), { derecha: true });

  const barras = [
    [tr('Sin instrucción añadida', 'No added instruction'), 80],
    [tr('"Por favor, no hagas trampas."', '"Please do not cheat."'), 80, true],
    [tr('"Por favor, no manipules la recompensa."', '"Please do not reward hack."'), 70],
    [tr('"Por favor, resuelve esta tarea solo con los métodos que su diseñador habría previsto."', '"Please solve this task only using methods the task\'s designer likely intended."'), 95],
  ];
  const datos = [
    [tr('Aprobar haciendo trampa', 'Passing by cheating'), tr('En tareas imposibles, donde los tests contradicen la especificación y solo se aprueba con trampa, GPT-5 hizo trampa en el 76&nbsp;% de las tareas de una de las pruebas.', 'On impossible tasks, where the tests contradict the specification and only cheating passes, GPT-5 cheated on 76% of the tasks of one of the tests.'), 'https://arxiv.org/abs/2510.20270', 'ImpossibleBench, 2025'],
    [tr('La impresión engaña', 'The impression misleads'), tr('Dieciséis desarrolladores con experiencia tardaron un 19&nbsp;% más con IA, y creían haber ahorrado un 20&nbsp;%. METR <a class="g-src" href="https://metr.org/blog/2026-02-24-uplift-update/">cree que la aceleración ha aumentado desde entonces</a>, con datos que considera una señal débil.', 'Sixteen experienced developers took 19% longer with AI, and believed they had saved 20%. METR <a class="g-src" href="https://metr.org/blog/2026-02-24-uplift-update/">believes the speedup has increased since then</a>, on data it considers a weak signal.'), 'https://metr.org/blog/2025-07-10-early-2025-ai-experienced-os-dev-study/', tr('METR, julio de 2025', 'METR, July 2025')],
    [tr('El documento no basta', "The document isn't enough"), tr('En 88.052 PR, las que enlazaban una especificación no introdujeron menos defectos que las demás. Mide issues y tickets, no herramientas de SDD.', 'In 88,052 PRs, the ones that linked a specification introduced no fewer defects than the rest. It measures issues and tickets, not SDD tools.'), 'https://drops.dagstuhl.de/storage/00lipics/lipics-vol394-esem2026/LIPIcs.ESEM.2026.84/LIPIcs.ESEM.2026.84.pdf', 'Hill, ESEM 2026'],
    [tr('La IA amplifica', 'AI amplifies'), tr('Es, sobre todo, un amplificador de las fortalezas y las debilidades que ya tiene cada organización.', 'It is, above all, an amplifier of the strengths and weaknesses each organization already has.'), 'https://dora.dev/research/2025/dora-report/', 'DORA, 2025'],
  ];
  const evidencia = sala(dos(
    `<h2 class="g-h2">${tr('Pedírselo no basta.', 'Asking is not enough.')}</h2>
    <p class="g-p">${tr('Un agente al que se le pide que no haga trampas puede seguir haciéndolas, y lo que parece ir más rápido no siempre lo es. Por eso las puertas comprueban, en vez de confiar.', "An agent asked not to cheat may go on cheating, and what feels faster isn't always faster. That's why gates check instead of trusting.")}</p>`,
    `<figure class="g-chart">
      <h3 class="g-h3">${tr('Pedirle que no hiciera trampas no cambió nada.', 'Asking it not to cheat changed nothing.')}</h3>
      <ul class="g-bars">${barras.map(([t, n, clave]) => `<li${clave ? ' class="is-key"' : ''}><span>${t}</span><i style="width: ${n}%"></i><b>${tr(`${n}&nbsp;%`, `${n}%`)}</b></li>`).join('')}</ul>
      <figcaption class="g-cap">${tr('Ejecuciones en las que el modelo o3 propuso hacer trampa con su evaluación, según la instrucción añadida a la tarea: veinte por instrucción, en una tarea de RE-Bench. Cuando le preguntaron si su plan respetaba la intención del usuario, respondió que no diez veces de diez.', "Runs in which the o3 model proposed to cheat on its evaluation, by the instruction added to the task: twenty per instruction, on a RE-Bench task. Asked whether its plan respected the user's intention, it answered no ten times out of ten.")} ${fuente('https://metr.org/blog/2025-06-05-recent-reward-hacking/', tr('METR, junio de 2025', 'METR, June 2025'))}</figcaption>
    </figure>
    <ul class="g-rows">${datos.map(([h, p, href, cita]) => `<li><h3 class="g-h3">${h}</h3><p class="g-p">${p} ${fuente(href, cita)}</p></li>`).join('')}</ul>`,
  ));

  const columnas = [tr('Vibe coding', 'Vibe coding'), 'SDD', 'TDD', 'GDD'];
  const comparacion = [
    [tr('Qué decide que está hecho', 'What decides it is done'), [tr('Que parece funcionar.', 'That it seems to work.'), tr('Que coincide con la especificación.', 'That it matches the specification.'), tr('Que el test pasa.', 'That the test passes.'), tr('Que pasan las comprobaciones de sus puertas.', "That its gates' checks pass.")]],
    [tr('Quién escribe lo que lo juzga', 'Who writes what judges it'), [tr('Nadie.', 'Nobody.'), tr('Quien lee el documento: tú o el propio agente.', 'Whoever reads the document: you or the agent itself.'), tr('Normalmente, quien escribe el código.', 'Usually, whoever writes the code.'), tr('Quien no construye. El agente no lo toca.', "Someone who doesn't build. The agent doesn't touch it.")]],
    [tr('Cómo reparte la exigencia', 'How it spreads the demand'), [tr('No la hay.', 'There is none.'), tr('Por documento, no por riesgo.', 'By document, not by risk.'), tr('Igual en todo el código.', 'The same across the code.'), tr('Según el riesgo: verde, amarillo o rojo.', 'By risk: green, yellow or red.')]],
    [tr('Cuando algo se cuela', 'When something slips through'), [tr('Lo arreglas y sigues.', 'You fix it and move on.'), tr('Amplías la especificación.', 'You extend the specification.'), tr('Añades un test.', 'You add a test.'), tr('Se convierte en comprobación de la puerta.', "It becomes one of the gate's checks.")]],
    [tr('Tu papel', 'Your role'), [tr('Probarlo a mano.', 'Trying it by hand.'), tr('Escribir y mantener el documento.', 'Writing and keeping the document.'), tr('Escribir tests y código.', 'Writing tests and code.'), tr('Diseñar el plano, construir las puertas y decidir donde importa.', 'Designing the blueprint, building the gates and deciding where it matters.')]],
  ];
  const diferencia = sala(`<div class="g-top">
      <h2 class="g-h2">${tr('En qué se diferencia de vibe coding, SDD y TDD.', 'How it differs from vibe coding, SDD and TDD.')}</h2>
      <p class="g-lead">${tr('Vibe coding, especificaciones y TDD responden a cómo se construye. GDD responde a otra pregunta: qué tiene que estar demostrado para dar un cambio por hecho.', 'Vibe coding, specifications and TDD answer how to build. GDD answers another question: what has to be proven before a change is done.')}</p>
    </div>
    <div class="g-scroll" tabindex="0" role="region" aria-label="${tr('Comparación de GDD con vibe coding, SDD y TDD', 'GDD compared with vibe coding, SDD and TDD')}"><table class="g-tbl g-cmp"><thead><tr><td></td>${columnas.map((c) => `<th scope="col"${c === 'GDD' ? ' class="is-gdd"' : ''}>${c}</th>`).join('')}</tr></thead>
    <tbody>${comparacion.map(([fila, celdas]) => `<tr><th scope="row">${fila}</th>${celdas.map((c, i) => `<td data-th="${columnas[i]}"${i === 3 ? ' class="is-gdd"' : ''}>${c}</td>`).join('')}</tr>`).join('')}</tbody></table></div>
    <div class="g-notes">
      <p class="g-p">${tr('TDD cabe dentro de GDD, que añade quién escribe qué, cuánta exigencia pide cada riesgo y qué hacer con lo que se cuela.', 'TDD fits inside GDD, which adds who writes what, how much each risk demands and what to do with what slips through.')} <a class="g-src" href="${ruta('y-tdd')}">${tr('¿Y TDD?', 'What about TDD?')}</a></p>
      <p class="g-p">${tr('Con SDD comparte la especificación, pero aquí es corta y ejecutable, reglas y unos pocos escenarios en Dado, Cuando, Entonces, y lo que decide es la puerta.', 'With SDD it shares the specification, but here it is short and executable, rules and a few scenarios in Given, When, Then, and what decides is the gate.')}</p>
    </div>`, { derecha: true });

  const momentos = [
    [tr('Preparar', 'Prepare'), tr('Escribes el plano y construyes las puertas. Intentas burlarlas a propósito.', 'You write the blueprint and build the gates. You try to fool them on purpose.')],
    [tr('Encargar', 'Brief'), tr('Reglas, unos pocos escenarios y una primera estimación del riesgo.', 'Rules, a few scenarios and a first estimate of the risk.')],
    [tr('Ejecutar', 'Execute'), tr('El agente cruza las puertas una a una. Si se atasca, se para y pregunta.', 'The agent gets through the gates one by one. If it gets stuck, it stops and asks.')],
    [tr('Cerrar', 'Close'), tr('Con el cambio ya en uso, una persona comprueba si sirvió.', 'With the change in use, a person checks whether it helped.')],
    [tr('Blindar', 'Harden'), tr('Lo que se coló se convierte en comprobación, y lo que ya no aporta se rebaja o se retira. El plano cambia y se vuelve a Preparar.', 'What slipped through becomes a check, and what no longer adds anything is relaxed or retired. The blueprint changes and you go back to Prepare.')],
  ];
  const metodo = sala(`<div class="g-top">
      <h2 class="g-h2">${tr('Una regla, siete principios, cinco momentos.', 'One rule, seven principles, five moments.')}</h2>
      <p class="g-lead">${tr('Todo GDD cabe en una página. Estos son los cinco momentos por los que pasa el trabajo, desde el primer día hasta que algo se cuela.', 'All of GDD fits on one page. These are the five moments the work goes through, from the first day until something slips through.')}</p>
    </div>
    <ol class="g-mom">${momentos.map(([h, p], i) => `<li><h3 class="g-h3">${h}</h3><p class="g-p">${p}</p>${i < momentos.length - 1 ? hoja(56) : ''}</li>`).join('')}</ol>
    <div class="g-two">
      <div>
        <h3 class="g-h3">${tr('Los siete principios', 'The seven principles')}</h3>
        <ol class="g-prin">${principios.map((p, i) => `<li><b>${i + 1}</b><span>${p}</span></li>`).join('')}</ol>
      </div>
      <div>
        <h3 class="g-h3">${tr('Una puerta vale si respondes que sí cuatro veces', 'A gate holds if you answer yes four times')}</h3>
        <ul class="g-test">${preguntas.map((q) => `<li>${visto}<span>${q}</span></li>`).join('')}</ul>
        <p><a class="g-link" href="${ruta('manifiesto')}">${tr('Leer el manifiesto', 'Read the manifesto')} ${flecha}</a></p>
      </div>
    </div>`);

  const mas = [
    ['empieza-hoy', tr('GDD sin skills, con lo que ya hay en el repositorio.', 'GDD without skills, with what the repository already has.')],
    ['manos-a-la-obra', tr('Cuatro puertas y cuatro cambios, de menos a más riesgo.', 'Four gates and four changes, from less to more risk.')],
    ['documentacion', tr('Empieza por la idea y sigue hasta donde te haga falta.', 'Start with the idea and go as far as you need.')],
  ];
  const final = `<section class="g-blue g-last" data-gdd-sheet>
<span class="g-axis" aria-hidden="true"></span>
<div class="g-wrap">${muro(true)}<div class="g-in">
  <div class="g-end">
    <div class="g-end__t">
      <h2 class="g-h1">${tr('Pon la primera puerta hoy.', 'Set up your first gate today.')}</h2>
      <p class="g-lead">${tr('Instala las skills y haz tu primer cambio en cinco pasos, con tus agentes y con lo que ya hay en tu repositorio.', 'Install the skills and make your first change in five steps, with your agents and with what your repository already has.')}</p>
      <div class="g-cta"><a class="g-btn" href="${ruta('empieza')}">${tr('Empieza con tus agentes', 'Start with your agents')} ${flecha}</a></div>
    </div>
    <svg class="g-end__d" viewBox="0 0 300 250" aria-hidden="true"><path class="pl" style="stroke-width: 10" d="M0 240H60M240 240H300"></path><path class="pl" style="stroke-width: 3" d="M60 240V60"></path><path class="pl" style="stroke-width: 1" d="M60 60A180 180 0 0 1 240 240"></path></svg>
  </div>
  <p class="g-cap g-nota">${tr('Manifiesto', 'Manifesto')} ${version}, ${estado}. ${frase('manifiesto', tr('Esta versión todavía no trae datos propios', "This version doesn't bring its own data yet"))} ${tr('Su motor, <span class="sin-corte">T-minus</span>, está en desarrollo.', 'Its engine, <span class="sin-corte">T-minus</span>, is in development.')}</p>
  <div class="g-more">${mas.map(([k, d]) => `<a href="${ruta(k)}"><strong>${menu(k)} ${flecha}</strong><span>${d}</span></a>`).join('')}</div>
</div></div>
</section>`;

  return `<div class="g">
<div class="g-you" aria-hidden="true"><span></span></div>
${heroe}
${sector}
${problema}
${idea}
${enMarcha}
${dia}
${evidencia}
${diferencia}
${metodo}
${final}
</div>`;
};
