# Decisiones de diseño

¿Por qué GDD está construido como está? Esta página reúne las decisiones que dan forma a GDD y a
gdd, las alternativas que se valoraron y por qué perdió cada una. Léela cuando algo de GDD te
parezca más estricto o más simple de lo que esperabas y quieras saber si es a propósito.

## Antes de empezar

[GDD en una página](method.md) presenta el método. Los registros del propio proyecto guardan
el razonamiento completo: la decisión de tecnología en el [ADR 0001](../../adr/0001-technology.md)
(un ADR, registro de decisión de arquitectura, es un fichero corto que fija una decisión y sus
motivos), las [notas de Discover](../../discover/) y la [hoja de ruta](../../../ROADMAP.md). Esta
página reúne las decisiones por las que más pregunta quien llega nuevo.

## Un único binario en Rust

**Elegido.** gdd es un único binario en Rust, construido como un workspace de Cargo: una
biblioteca con el método (`gdd-core`, sin dependencias de terminal ni de línea de comandos),
el binario con la interfaz de línea de comandos (CLI) y las pantallas de terminal, y los
escenarios de aceptación, que hablan con la biblioteca, no con la pantalla.

**Por qué.** gdd lanza agentes, ejecuta los comandos de las puertas y guarda estado en disco, y
se publica como código abierto, así que instalarlo tiene que llevar un solo paso. Un único
binario estático se instala en cualquier sitio sin un runtime de Python ni de Node.

**Descartado.** Python con Textual era lo más rápido de escribir, porque la primera versión de
la lógica ya existía en Python, pero necesita un runtime de Python para instalarse. Go con
Bubble Tea también da un único binario, pero los otros proyectos del autor ya usaban Rust y
ninguno usaba Go. El coste de Rust se aceptó a sabiendas: los tiempos de compilación hacen más
lenta G2, así que los escenarios de aceptación se quedan al nivel de la biblioteca para que
siga siendo rápida.

## Gherkin es obligatorio; el runner lo eliges tú

**Elegido.** GDD impone el método, no la herramienta. Cada proyecto escribe sus escenarios en
[Gherkin](https://cucumber.io/docs/gherkin/reference/), el lenguaje de `Given`, `When` y
`Then`, en ficheros `.feature` dentro de `features/`. Cada proyecto elige el runner que encaja
con su tecnología, entre runners con amplio respaldo de la comunidad: la implementación oficial
de Cucumber o el estándar de facto de su ecosistema. Sea cual sea el runner, tiene que:

- **fallar con pasos sin definir o pendientes** (modo estricto), para que una frase que nadie
  ha ligado a código no pueda pasar nunca;
- **ejecutar solo algunos ficheros `.feature`**, para que G2 ejecute los escenarios del Change
  y G3 los ejecute todos;
- **ejecutar los ejemplos ocultos** de `features/hidden/`.

**Por qué Gherkin.** Los escenarios son el contrato que apruebas sin leer código, así que tienen
que ser frases que una persona pueda juzgar y, a la vez, el test que se ejecuta. Esa es la
promesa del desarrollo guiado por comportamiento (BDD, *behavior-driven development*), en
palabras de Cucumber: «documentación del sistema que se comprueba automáticamente contra el
comportamiento del sistema» ([Cucumber, BDD](https://cucumber.io/docs/bdd/)).

**Por qué cualquier runner.** Un método que exigiera un runner concreto dejaría fuera todos los
stacks en los que ese runner encaja mal. Los tres requisitos son lo que necesitan las puertas de
GDD; el resto lo elige el proyecto.

**Descartado.** Una versión temprana del método exigía el propio Cucumber, lo que lo ataba a una
sola familia de herramientas. También se descartó un «catálogo Gherkin»: tests normales con el
nombre del id de cada escenario y un script que comprueba que cada id tiene el suyo. Eso
demuestra que un test *existe*, no que compruebe lo que dice la frase aprobada, y un agente
podría entregar un test más débil con el nombre correcto. Ejecutar el Gherkin aprobado forma
parte del núcleo.

**La crítica habitual, respondida.** En BDD, la capa de definiciones de pasos tiende a crecer y a
volverse difícil de mantener. GDD lo limita con instrucciones, no con herramientas: el modo test
reutiliza un paso que ya dice lo mismo y solo añade uno cuando ninguno encaja, y conecta cada
paso con la capa más baja que muestra el comportamiento (una función del dominio, luego la línea
de comandos, luego la pantalla). El Reviewer señala un paso duplicado como hallazgo. Solo unos
pocos escenarios, etiquetados `@ui`, pasan por la pantalla, y la demo en pre comprueba la
interfaz real.

## Mínimo por defecto, incremental si lo eliges

**Elegido.** Tres niveles, y solo el primero es fijo:

1. **El núcleo** que te protege de los agentes está siempre activo, y ningún Builder puede
   desactivarlo: las puertas G1 a G4, el semáforo, Path Guard, los ejemplos ocultos, un
   Reviewer de otra familia de modelos y mutation testing en los Changes rojos. gdd rechaza un
   fichero de reglas que desactive el núcleo para los Changes rojos, y una clave que el fichero
   no incluya cuenta como activa.
2. **Las comprobaciones del stack** están activas por defecto y se pueden desactivar: el
   linter, los tipos, los tests unitarios, los escáneres de la ficha de stack del proyecto.
3. **Los extras** son opcionales, y cada uno se activa añadiendo su sección a un
   `gdd/*.toml`: mutation testing en los Changes amarillos, una revisión profunda para el
   amarillo, las herramientas de Learn en `[learn]`, una única sesión compartida del Builder.
   Ninguno añade una pregunta a `gdd init`.

**Por qué.** Un método que pide a quien empieza decidir veinte cosas antes del primer Change es
un método que nadie empieza. Un método cuya seguridad se puede quitar con configuración no da
ninguna seguridad. Los niveles separan las dos cosas: lo que te protege no se puede quitar, y lo
que cuesta tiempo o tokens lo añades tú cuando compensa.

**Descartado.** Mutation testing en todos los Changes amarillos por defecto. Sigue siendo la
regla en [`GDD.md`](../../../GDD.md#gate-level-traffic-light) y en el propio repositorio de gdd,
pero es una puerta lenta (gdd necesitó un Change propio para acelerarla en Rust). El principio de
mínimo por defecto la movió: al núcleo para los Changes rojos, y a una línea
(`mutation = true`) para los amarillos.

## Roles con el contexto mínimo

**Elegido.** Cada agente recibe solo lo que su tarea necesita. El Orchestrator tiene el mapa
completo y escribe, para cada Change, un pequeño **paquete**: el objetivo en tres líneas, el
color, los escenarios, las rutas que puede tocar, los contratos y funciones que hay que
reutilizar, las reglas de su zona y el comando de G2. El Builder lee el paquete, no el mapa.

**Por qué.** Un contexto más pequeño es más barato y más centrado, y además es una medida de
seguridad: el paquete del Builder deja fuera los ejemplos ocultos.

Separar los roles también pone un segundo modelo entre el trabajo y main. El Builder y el
Reviewer forman lo que Anthropic llama un flujo *evaluator-optimizer* (evaluador-optimizador):
«una llamada al LLM genera una respuesta mientras otra la evalúa y da retroalimentación en un
bucle» (un LLM es un modelo de lenguaje grande). Anthropic lo recomienda «cuando tenemos
criterios de evaluación claros»
([Building effective agents](https://www.anthropic.com/engineering/building-effective-agents)).
En GDD los criterios son los escenarios, el paquete y `docs/standards.md`, y el bucle tiene un
límite: una corrección.

**Las alternativas.** Un solo agente que planifica, construye y revisa es más simple, pero se
pone nota a sí mismo, y su contexto crece con cada Change. Un segundo agente del mismo modelo es
mejor, pero suele compartir los puntos ciegos del primero, y por eso GDD pide un Reviewer de otra
familia de modelos.

## Herramientas deterministas antes que agentes

**Elegido.** Las máquinas deciden lo que se puede comprobar, y esas máquinas son, ante todo,
herramientas deterministas: el linter, el comprobador de tipos, los tests, los escenarios,
mutation testing, los escáneres de seguridad y de licencias, la detección de secretos. Un agente
juzga solo lo que ninguna herramienta puede juzgar, y una persona solo lo que ningún agente
debería.

**Por qué.** Una herramienta da la misma respuesta cada vez y dice por qué; un agente puede que
no. La guía de Anthropic sobre agentes dice lo mismo desde el otro lado: es «crucial que los
agentes obtengan la "verdad de base" del entorno en cada paso (como los resultados de las
llamadas a herramientas o la ejecución de código)»
([Building effective agents](https://www.anthropic.com/engineering/building-effective-agents)).
Las puertas son esa verdad de base. Learn empuja en la misma dirección: cada error repetido se
convierte en una regla, un lint, un hook o un test, para que la próxima vez lo atrape una
herramienta.

**Descartado.** Un agente como revisor de seguridad de cada Change. Hoy solo los escáneres miran
la seguridad por su cuenta. Está prevista una revisión de seguridad aparte, hecha por un agente,
para las zonas rojas de seguridad, y se sumará a los escáneres, no los sustituirá.

## Modos test y build separados

**Elegido.** El Builder trabaja en dos modos, en sesiones separadas por defecto. El modo test
escribe las definiciones de pasos de los escenarios aprobados, con aserciones reales, y tiene
que verlas fallar porque falta el comportamiento. El modo build escribe el código hasta que G2
está en verde, y no puede tocar los pasos.

**Por qué.** Si el agente que escribe el código también escribe los tests que lo juzgan, puede
hacer que ambos cuadren de la forma equivocada. Separar los modos, y dejar que Path Guard
compruebe cada uno contra su propia instantánea, hace que los tests existan antes que el código
y que después no se puedan torcer para ajustarlos a él. Cuando el Reviewer encuentra un fallo en
un paso, el modo test lo arregla en su propia ronda, así que el modo build nunca necesita tocar
los tests.

**La sesión compartida opcional.** Dos sesiones pierden contexto en el relevo. Un experimento
con los Changes del propio gdd lo midió: una única sesión compartida gastó alrededor de un 21 %
menos de tokens nuevos y alrededor de un 7 % menos de tiempo, con una calidad dentro de la
variación normal entre ejecuciones. No bastó para cambiar el valor por defecto, así que pasó a
ser una opción: `[session] builder = "shared"` en `gdd/models.toml`, disponible para Claude Code.
Path Guard sigue demostrando que los pasos no se editan después del modo test.
[Medir lo que importa](measuring.md#un-ejemplo-una-sesión-de-builder-o-dos) tiene las cifras.

**Descartado.** Un único modo que escriba los tests y el código juntos, de una vez y sin ninguna
comprobación entre medias.

## Spikes cuyo código nunca se fusiona

**Elegido.** Un **Spike** responde a una pregunta para que puedas decidir. Se construye rápido,
sin tests primero, sin mutation testing y sin Reviewer, dentro de un presupuesto fijo
(`[spike] max_usd`, 5 dólares estadounidenses por defecto). Su puerta es Path Guard más un ADR
con una sección `## Decision`. Un OK en G4 lleva a main solo el ADR, y el código se queda en su
rama. Un Spike puede construir de dos a cuatro variantes, una por opción, para que las compares
en una sola página.

**Por qué.** El código de exploración se escribe para aprender, no para durar. Si pudiera
fusionarse, acabaría fusionándose, y el proyecto se quedaría con código que nadie ha probado de
verdad. Quedarse solo con la decisión hace que el Spike sea barato de empezar y seguro de tirar.

**Descartado.** Fusionar el código de un Spike tal cual. Los [estándares](../../standards.md)
del propio gdd lo dicen claramente: «El código de un Spike no entra en main tal cual». Lo que el
Spike aprendió va a un Change nuevo, con sus propios escenarios y puertas.

## Ideas de Spec Kit, sin depender de él

**Elegido.** gdd no depende de [GitHub Spec Kit](https://github.com/github/spec-kit), el kit de
herramientas para el desarrollo guiado por especificaciones («define qué construir y por qué, y
después convierte la especificación en un plan, tareas y código que funciona»). Toma prestadas
tres de sus ideas: una ficha de datos por agente, comandos escritos una sola vez como plantillas
y generados para cada agente, y ejecuciones reanudables con su estado en disco.

**Por qué.** El primer arnés de GDD, un kit de arranque en Python, se apoyaba en Spec Kit, y la
integración trajo tres problemas: comandos registrados solo para el agente activo, enlaces rotos
en modo desarrollo y una captura de solo texto que ocultaba el uso de tokens. Tener sus propias
fichas de agente y su propia receta eliminó los tres, y permitió que gdd arranque cada agente
por sí mismo, con los permisos de su rol y sin secretos.

**Descartado.** Mantener Spec Kit por debajo. El núcleo del método, las puertas y Path Guard,
habría dependido de una capa que gdd no controla.

## Colores, no tipos de trabajo

**Elegido.** El riesgo decide las puertas. El color de un Change sale de las rutas que toca, y
**el color manda sobre el tipo de trabajo y sobre la vía**: un Hotfix en una zona roja es rojo.

**Por qué.** El tipo dice por qué cambias algo; el color dice qué se rompe si el Change está
mal. Solo lo segundo debería decidir cuánto cuidado hay que poner.
[Cómo elige el riesgo las puertas](risk.md) tiene el argumento completo.

**Descartado.** Puertas más ligeras para los Hotfixes o para los Changes pequeños. GDD resuelve
la urgencia revirtiendo primero, no comprobando menos.

## Una herramienta en inglés, agentes en tu idioma

**Elegido.** La herramienta, sus ficheros, sus comandos y sus mensajes están en inglés. Los
agentes te hablan en el idioma configurado en `gdd/project.toml` (`language`): sus preguntas,
notas, resúmenes y entradas de la Decision Inbox.

**Por qué.** gdd es código abierto, y el inglés es el idioma común de su código, su
configuración y sus issues. Pero la persona que aprueba los escenarios y responde a G4 debería
hacerlo en el idioma en el que piensa. Separar las dos cosas mantiene la herramienta coherente y
te deja decidir en tu propio idioma.

**Descartado.** Una herramienta traducida. Cada mensaje existiría en muchas versiones, y los
escenarios que los comprueban se multiplicarían.

## Decisiones menores

- **Los Changes que se solapan avisan en vez de rechazarse.** Al principio, gdd rechazaba un
  lanzamiento cuyas rutas previstas se solapaban con las de un Change en curso. En la historia
  del propio gdd todos los Changes tocaban el mismo fichero de pasos, así que nunca podían
  ejecutarse dos a la vez. Ahora los solapamientos solo avisan y git los resuelve en la fusión,
  como hace una cola de fusión; un conflicto aparca el Change y explica cómo seguir.
- **Worktrees en una carpeta hermana.** Cada Change se construye en `<repo>.worktrees/<id>`,
  junto al repositorio y no dentro, donde los runners de tests y los linters encontrarían las
  copias. Cambiarlo es una línea de configuración.
- **Una sola familia de modelos, aceptada abiertamente.** GDD quiere el Reviewer de otra
  familia. Si solo hay una instalada, `gdd init` se niega salvo que lo aceptes, registra la
  fecha, y cada G4 y cada commit de fusión lo indican. Exigir dos familias dejaría fuera a
  cualquiera que tenga una sola suscripción.
- **El changelog es del proyecto.** gdd propone una línea por Change pensada para los usuarios,
  que viaja en el commit de fusión, y añade a `CHANGELOG.md` solo lo que le falta, en formato
  [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); nunca reescribe el fichero. Las
  versiones siguen [Semantic Versioning](https://semver.org/) y solo se etiquetan después de
  que una promoción salga bien.
- **El registro viaja en git, los ficheros de trabajo se quedan en local.** El commit de fusión
  lleva lo que pasó; `.gdd/` guarda los borradores, las salidas y los logs, y no se sube en
  ningún commit.
- **Esta documentación sigue [Diátaxis](https://diataxis.fr/)**: tutoriales, guías prácticas,
  referencia y explicación, y cada página hace un solo trabajo. Esta página es una explicación.

## Ver también

- [ADR 0001 · Technology](../../adr/0001-technology.md): la decisión sobre Rust al completo.
- [Mantener honestos a los agentes](safety.md): el núcleo que protegen estas decisiones.
- [Cómo elige el riesgo las puertas](risk.md): por qué decide el color.
- [Medir lo que importa](measuring.md): cómo se comprueban las decisiones con cifras.
