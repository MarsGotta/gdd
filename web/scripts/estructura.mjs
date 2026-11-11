// The map of the site: which documents exist, where each lives and what the chapters of the
// documentation are. build.mjs, comprobar.mjs and paridad.mjs all read it from here.

const doc = (fichero, es, en) => ({ fichero, es, en });

export const PARTES = {
  entender: { es: 'Entender', en: 'Understand' },
  aplicar: { es: 'Aplicar', en: 'Apply' },
  garantizar: { es: 'Garantizar', en: 'Guarantee' },
  tminus: { es: 'T-minus', en: 'T-minus' },
  consultar: { es: 'Consultar', en: 'Look up' },
};

// [number, part, key (the Spanish route), file, es{menu,titulo,descripcion}, en{...}]
const CAP = [
  [1, 'entender', 'la-idea', 'the-idea', '01-la-idea',
    ['La idea', 'La idea de GDD: puertas para agentes de IA', 'El problema que resuelve Gate-Driven Development y su idea central, contada como un escape room: puertas, comprobaciones y riesgo.'],
    ['The idea', 'The idea behind GDD: gates for AI agents', 'The problem Gate-Driven Development solves and its central idea, told as an escape room: gates, checks and risk.']],
  [2, 'entender', 'manos-a-la-obra', 'hands-on', '02-manos-a-la-obra',
    ['Manos a la obra', 'Manos a la obra: GDD paso a paso con una tienda online', 'Prepara las puertas de una tienda online y revisa cuatro cambios de menos a más riesgo, hasta el que se cuela. Sin instalar nada.'],
    ['Hands on', 'Hands on: GDD step by step with an online shop', 'Prepare the gates of an online shop and review four changes from less to more risk, up to the one that slips through. Nothing to install.']],
  [3, 'entender', 'las-piezas', 'the-pieces', '03-las-piezas',
    ['Las piezas y los roles', 'Las piezas y los roles de GDD', 'Puerta, comprobaciones y riesgo, y quién hace qué en Gate-Driven Development: tú, el orquestador, el agente y el vigilante.'],
    ['The pieces and the roles', 'The pieces and the roles of GDD', 'Gate, checks and risk, and who does what in Gate-Driven Development: you, the orchestrator, the agent and the reviewer.']],
  [4, 'entender', 'los-principios', 'the-principles', '04-los-principios',
    ['Los siete principios', 'Los siete principios de GDD', 'Los siete principios que hacen funcionar Gate-Driven Development, cada uno con su porqué y la evidencia de otros equipos.'],
    ['The seven principles', 'The seven principles of GDD', 'The seven principles that make Gate-Driven Development work, each with its reason and the evidence from other teams.']],
  [5, 'entender', 'los-momentos', 'the-moments', '05-los-momentos',
    ['Los cuatro momentos', 'Los cuatro momentos de GDD', 'Preparar, Encargar, Ejecutar y Blindar: cuándo pasa cada cosa en Gate-Driven Development, y de dónde sale el plano.'],
    ['The four moments', 'The four moments of GDD', 'Prepare, Brief, Execute and Harden: when each thing happens in Gate-Driven Development, and where the blueprint comes from.']],
  [6, 'aplicar', 'empieza-hoy', 'start-today', '06-empieza-hoy',
    ['Empieza hoy', 'Empieza hoy con GDD, con lo que ya tienes', 'La traducción de GDD a lo que ya tienes (CI, tests, CODEOWNERS) y una primera puerta lista para copiar en tu repositorio.'],
    ['Start today', 'Start today with GDD, with what you have', 'GDD translated into what you already have (CI, tests, CODEOWNERS) and a first gate ready to copy into your repository.']],
  [7, 'aplicar', 'las-skills', 'the-skills', '17-las-skills',
    ['Las skills', 'Las skills de GDD: el método entero con tus agentes', 'Siete skills para que tus agentes apliquen Gate-Driven Development con lo que tu proyecto ya tiene, sin motor: qué hace cada una y qué no garantizan.'],
    ['The skills', 'The GDD skills: the whole method with your agents', 'Seven skills for applying Gate-Driven Development with your agents, without an engine: what each one does and what they cannot guarantee.']],
  [8, 'aplicar', 'un-caso-delicado', 'a-delicate-case', '07-un-caso-delicado',
    ['Un caso delicado', 'Un caso delicado con GDD: el cobro', 'Qué cambia cuando el cambio toca el cobro: la zona delicada, tu aprobación y algo que se cuela y se blinda, de principio a fin en seis pasos.'],
    ['A delicate case', 'A delicate case with GDD: payment', 'What changes when the change touches payment: the delicate zone, your approval and something that slips through and gets hardened, start to finish in six steps.']],
  [9, 'aplicar', 'el-recorrido-entero', 'the-whole-path', '08-el-recorrido-entero',
    ['El recorrido entero', 'El recorrido entero de GDD, con cinco puertas', 'Gate-Driven Development de punta a punta, con cinco puertas, los mismos códigos de descuento y lo que pasa cuando algo sale mal.'],
    ['The whole path', 'The whole GDD path, with five gates', 'Gate-Driven Development from end to end, with five gates, the same discount codes and what happens when something goes wrong.']],
  [10, 'garantizar', 'como-se-engana-una-puerta', 'how-a-gate-gets-fooled', '09-como-se-engana-una-puerta',
    ['Cómo se engaña una puerta', 'Cómo se engaña una puerta de GDD', 'Cuatro trampas para engañar a una puerta sin tocar el test, y qué las para. La diferencia entre una norma y una garantía.'],
    ['How a gate gets fooled', 'How a GDD gate gets fooled', 'Four tricks for fooling a gate without touching the test, and what stops them. The difference between a rule and a guarantee.']],
  [11, 'garantizar', 'el-nucleo', 'the-core', '10-el-nucleo',
    ['El núcleo', 'El núcleo de GDD: lo que no se negocia', 'Las garantías que ningún motor ni configuración puede quitar y que cualquier motor que diga aplicar GDD tiene que dar.'],
    ['The core', 'The core of GDD: what is not negotiable', 'The guarantees that no engine or configuration can remove and that any engine claiming to apply GDD has to give.']],
  [12, 'garantizar', 'como-se-mide', 'how-it-is-measured', '11-como-se-mide',
    ['Cómo se mide', 'Cómo se mide si GDD compensa', 'Cuatro cosas que GDD tiene que demostrar con datos, cómo compararlas con tu forma de trabajar antes y qué dice la evidencia disponible.'],
    ['How it is measured', 'How to measure whether GDD pays off', 'Four things GDD has to show with data, how to compare them with how you worked before and what the available evidence says.']],
  [13, 'tminus', 'el-motor', 'the-engine', '12-el-motor',
    ['El motor', 'El motor de GDD y cuándo hace falta', 'Qué falta sin motor para cumplir el núcleo entero de GDD, cuándo empieza a hacer falta uno y cómo encajan documentación y buenas prácticas.'],
    ['The engine', 'The GDD engine and when you need one', 'What is missing without an engine to meet the whole core of GDD, when you start to need one and how documentation and good practices fit.']],
  [14, 'tminus', 'los-kits', 'the-kits', '13-los-kits',
    ['Los kits', 'Los kits de T-minus: reglas ya hechas para un área', 'Qué es un kit de T-minus: reglas ya hechas para un área, con sus comprobaciones de serie. Es parte del motor, pero no es el motor.'],
    ['The kits', 'The T-minus kits: ready-made rules for an area', 'What a T-minus kit is: ready-made rules for an area, with their standing checks. Part of the engine, but not the engine.']],
  [15, 'consultar', 'buenas-practicas', 'good-practices', '14-buenas-practicas',
    ['Buenas prácticas', 'Buenas prácticas para trabajar con agentes de IA en GDD', 'Treinta prácticas para mantener ciertos los principios de Gate-Driven Development al trabajar con agentes de IA, ordenadas por momento.'],
    ['Good practices', 'Good practices for working with AI agents in GDD', 'Thirty practices that keep the principles of Gate-Driven Development true when you work with AI agents, ordered by moment.']],
  [16, 'consultar', 'y-tdd', 'and-tdd', '15-y-tdd',
    ['¿Y TDD?', '¿Y TDD? Qué cambia con GDD', 'En qué se parecen y en qué se distinguen TDD y Gate-Driven Development, y por qué no compiten: TDD puede formar parte de GDD.'],
    ['What about TDD?', 'What about TDD? How GDD differs', 'How TDD and Gate-Driven Development are alike and how they differ, and why they do not compete: TDD can be part of GDD.']],
  [17, 'consultar', 'de-donde-viene', 'where-it-comes-from', '16-de-donde-viene',
    ['De dónde viene', 'De dónde viene Gate-Driven Development: ideas y fuentes', 'Las ideas, investigaciones y equipos en los que se apoya Gate-Driven Development para trabajar con agentes de IA, con todas las fuentes.'],
    ['Where it comes from', 'Where Gate-Driven Development comes from: ideas and sources', 'The ideas, research and teams that Gate-Driven Development builds on for working with AI agents, with all the sources.']],
];

const cuerpo = (a) => ({ menu: a[0], titulo: a[1], descripcion: a[2] });

export const DOCS = {
  manifiesto: doc('manifiesto.html',
    { ruta: '/manifiesto/', menu: 'Manifiesto', titulo: 'Manifiesto de Gate-Driven Development (GDD)', descripcion: 'El texto completo del manifiesto de GDD: siete principios, cuatro momentos, cuatro puntos que no se negocian y la prueba para saber si una puerta es válida.' },
    { ruta: '/en/manifesto/', menu: 'Manifesto', titulo: 'The Gate-Driven Development (GDD) Manifesto', descripcion: 'The full text of the GDD manifesto: seven principles, four moments, four points that are not negotiable and the test for knowing whether a gate is valid.' }),
  documentacion: doc('doc/00-documentacion.html',
    { ruta: '/documentacion/', menu: 'Documentación', titulo: 'Documentación de Gate-Driven Development (GDD)', descripcion: 'Qué problema resuelve GDD, una tienda online paso a paso, las garantías y lo que necesitas para empezar hoy.' },
    { ruta: '/en/docs/', menu: 'Documentation', titulo: 'Gate-Driven Development (GDD) documentation', descripcion: 'The problem GDD solves, an online shop step by step, the guarantees and what you need to start today.' }),
  articulo: doc('articulo.html',
    { ruta: '/articulo/', menu: 'Artículo', titulo: 'A la IA no se la convence con palabras: se le ponen puertas', descripcion: 'Por qué nació GDD: la historia de una frustración con los agentes de IA y lo que encontré al investigar.' },
    { ruta: '/en/article/', menu: 'Article', titulo: "You don't persuade AI with words: you put gates on it", descripcion: 'Why GDD was born: the story of a frustration with AI agents, and what I found when I looked into it.' }),
};
for (const [n, parte, clave, claveEn, fichero, es, en] of CAP) {
  DOCS[clave] = {
    ...doc(`doc/${fichero}.html`, { ...cuerpo(es), ruta: `/documentacion/${clave}/` }, { ...cuerpo(en), ruta: `/en/docs/${claveEn}/` }),
    capitulo: { n, parte },
  };
}
export const CAPITULOS = CAP.map((c) => c[2]);
export const TOTAL = CAPITULOS.length;
// Order of the pages in the sitemap and the footer.
export const ORDEN = ['documentacion', ...CAPITULOS, 'manifiesto', 'articulo'];
export const MENU = ['documentacion', 'manifiesto'];
export const PIE = ['documentacion', 'manifiesto', 'articulo'];
// Old addresses and where they lead now.
export const REDIRECCIONES = [
  ['/documentacion/otra-area', '/documentacion/'], ['/en/docs/another-area', '/en/docs/'],
  ['/guia', '/documentacion/'], ['/como-funciona', '/documentacion/'],
  ['/empieza-hoy', '/documentacion/empieza-hoy/'], ['/ejemplo-completo', '/documentacion/el-recorrido-entero/'],
  ['/buenas-practicas', '/documentacion/buenas-practicas/'], ['/de-donde-viene', '/documentacion/de-donde-viene/'],
  ['/en/guide', '/en/docs/'], ['/en/start-today', '/en/docs/start-today/'], ['/en/full-example', '/en/docs/the-whole-path/'],
  ['/en/good-practices', '/en/docs/good-practices/'], ['/en/where-it-comes-from', '/en/docs/where-it-comes-from/'],
  ['/documentacion/tu-primer-cambio', '/documentacion/manos-a-la-obra/'], ['/en/docs/your-first-change', '/en/docs/hands-on/'],
];
