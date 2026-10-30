# Pocas paradas para la persona

Por qué GDD te pregunta tan poco, qué es lo que sí te pregunta y cómo agrupa las preguntas para
que un Change te cueste como mucho dos sentadas cortas. Esta es la página que tienes que leer si
te preocupa que los agentes te inunden a preguntas o que publiquen sin contar contigo.

## Antes de empezar

[GDD en una página](method.md) presenta las puertas y los colores. Las sentadas, la Decision
Inbox y las respuestas de G4 que se describen aquí las comprueban
[`features/sitting.feature`](../../../features/sitting.feature),
[`features/sitting-g4.feature`](../../../features/sitting-g4.feature) y
[`features/recipe.feature`](../../../features/recipe.feature).

## Tú eres la parte interesada, no quien lee el código

Un agente escribe código más rápido de lo que tú puedes leerlo, y un agente que te espera es un
agente que no está trabajando. Si cada Change necesitara tu revisión, serías la parte más lenta
del sistema, y la revisión empeoraría a medida que te cansaras.

Así que GDD te da el papel que tiene la parte interesada (*stakeholder*) en un buen equipo. Tú
decides **qué** tiene que hacer el software, **dónde** sería grave un error y **si el resultado
es lo bastante bueno para salir**. No decides si un bucle es correcto: eso lo deciden los tests,
los escenarios, el mutation testing (pruebas de mutación) y el Reviewer, que no se cansan.

Tus tres decisiones se corresponden con tres cosas que puedes juzgar sin leer código:

| Decides | Mirando | En |
|---|---|---|
| Qué tiene que hacer el Change | Escenarios en frases sencillas | G1 |
| Dónde sería grave un error | Zonas rojas, cada una con su motivo en palabras | La misma sentada |
| Cómo se va a construir un Change rojo | Un plan de alto nivel: qué, cómo, riesgos, cómo revertirlo | La misma sentada |
| Si el resultado es bueno | La demo en pre y el "How it works" del Reviewer | G4 |

## Cuántas paradas

Una *parada* es un momento en que un Change te espera. Según [`GDD.md`](../../../GDD.md#human-stops):

| Tipo de Change | Tus paradas |
|---|---|
| El Setup (G0) | 0: la abren las máquinas |
| Vía rápida (*Fast Lane*) | 0 |
| Vía completa (*Full Lane*) | De 0 a 2 |
| Rojo | 2: una sentada (plan, escenarios y zonas) y después G4 |
| Hotfix | 0 o 1 |

Y como mucho un mensaje por Change. La mayoría de los Changes de un proyecto ya mapeado son
amarillos y sin escenarios nuevos: van del lanzamiento a producción sin pararse ni una vez.

La regla para recordarlo: *las máquinas abren la 0; la persona abre la 1 y a veces la 4; las
máquinas vigilan la 2 y la 3.*

## Una sentada por Change

Una **sentada** es el único momento, antes de Build, en que apruebas todo lo que un Change
necesita de ti. El Orchestrator lo reúne en un solo mensaje, o `gdd sitting CHANGE` te lo enseña
en una sola pantalla:

- los escenarios de la rama del Change, con los ejemplos ocultos marcados como ocultos;
- las zonas rojas que esperan tu aprobación, cada una con su motivo, o una nota que dice que ya
  están aprobadas;
- en un Change rojo, su plan.

En el mismo mensaje, el Orchestrator propone también la línea que el Change añadirá al changelog,
así que ni eso te cuesta una parada más.

Apruebas una vez. La pantalla te enseña primero exactamente lo que se va a registrar, y no se
escribe nada hasta que confirmas: entonces registra G1, las zonas rojas y el plan a la vez. Si
algo está mal, pides cambios con tus propias palabras, y eso se registra como un KO de G1 con
esas palabras.

¿Por qué una sentada y no una aprobación por puerta? Porque cada aprobación es una interrupción,
y una interrupción cuesta más que la decisión. Tres preguntas en una sentada llevan unos minutos;
tres preguntas en tres tardes distintas suponen tres cambios de contexto.

¿Por qué antes de Build? Porque cambiar un escenario cuando la construcción ya ha empezado
significa aprobarlo otra vez y volver a construir. Decidir primero sale más barato que decidir dos
veces.

## La sentada de G4

G4 es la otra parada, y solo para los Changes que la necesitan: los rojos y los que tuvieron
escenarios nuevos. El Change ya está fusionado y funcionando en pre. `gdd sitting CHANGE` te
enseña con qué juzgarlo:

- la demo: vídeos o capturas de los escenarios o, en una biblioteca o una herramienta sin
  pantalla, la ejecución legible de sus escenarios, con sus números;
- **How it works** (cómo funciona): de tres a seis líneas sencillas que escribe el Reviewer cuando
  dice OK, sobre dónde encaja el Change en el código y qué patrón establece, para que entiendas lo
  que apruebas sin leer cada línea;
- las decisiones tomadas sin ti;
- la línea del changelog;
- en qué se fue el tiempo del Change.

Después respondes: `o` para OK, `k` para KO, `p` para polish. KO y polish necesitan tus palabras.
Antes de ejecutar nada, la pantalla te enseña el comando `gdd resume` exacto y lo que viene a
continuación.

## Hecho no es bueno

Un Change que pasa todas las puertas todavía puede estar sin terminar. La IA llega muy rápido a
algo que *parece* terminado, y la última parte, la que lo hace correcto y concreto, se queda sin
hacer porque ya "funciona". GDD llama a este principio **hecho no es bueno** (*done is not
good*), y sale de una sesión de Discover sobre el problema
([`docs/discover/2026-10-26-done-is-not-good.md`](../../discover/2026-10-26-done-is-not-good.md)).

Las máquinas cubren la mitad. Los ejemplos ocultos y el mutation testing pillan los tests que
mienten; el Reviewer de otra familia es el crítico. La otra mitad es tuya: en G4 eres el
**editor**.

Por eso G4 tiene tres respuestas, no dos:

- **OK**: promoverlo.
- **KO**: rechazarlo; producción se queda como está.
- **Polish** (pulir): una ronda más del Builder con tus notas, después las puertas y la demo otra
  vez, y vuelta a G4.

Polish no es un fracaso, ni tampoco una parada más: **sustituye a un KO**. "El botón funciona,
pero su mensaje dice *Error 3*" es una nota de polish, no un motivo para tirar el Change.

El listón de calidad está escrito, para que no tengas que repetirlo. Cada proyecto tiene
`docs/standards.md`: cómo es lo bueno aquí, en palabras, por ejemplo el tono, los nombres, los
mensajes de error y lo que no quieres. El Builder lo lee antes de construir y el Reviewer juzga
con él. El Brief dice qué y para quién; los estándares dicen cómo de bueno. Los
[estándares](../../standards.md) del propio gdd son un ejemplo.

## La Decision Inbox

Cada Change te deja un mensaje, su **Decision Inbox** (su bandeja de decisiones), en
`.gdd/changes/<id>/inbox.md` y en `gdd inbox CHANGE`. Tiene una forma fija, así que siempre sabes
dónde mirar (gdd escribe los títulos en inglés):

- **How it works** (cómo funciona), del Reviewer, cuando el Change espera en G4;
- **Needs your decision** (necesita tu decisión): lo que solo tú puedes decidir, cada cosa con una
  recomendación y como mucho una alternativa;
- **Decided without you** (decidido sin ti): las decisiones reversibles que tomaron los agentes
  por su cuenta, y las rutas fuera del plan que aceptó la regla del alcance;
- **Parked** (aparcado): por qué se paró el Change y qué hacer;
- y, cuando corresponde, tus notas de polish, las excepciones de seguridad que añade el Change y
  su deuda conocida.

"Decided without you" es lo que permite a los agentes seguir adelante. La regla para el
Orchestrator y el Builder es: si una decisión es reversible y dudarías entre preguntar y decidir,
decide y apúntalo. Tú lees la lista después, en un solo sitio, en lugar de contestar cada pregunta
según llega.

## Learn convierte las notas repetidas en reglas

Si escribes la misma nota de polish dos veces, el método ya te ha fallado una. La fase 5, Learn,
existe para evitarlo. `gdd learn` pone tus notas de polish de G4 una al lado de otra, para que el
Orchestrator detecte las que se repiten, y enumera las clases de hallazgos del Reviewer que han
vuelto a salir en dos Changes o más.

Una nota repetida se convierte en algo que corta el problema antes de que vuelva a llegarte: una
línea de `docs/standards.md`, una regla de lint, un test o un hook. Learn no tiene puerta. Solo
endurece.

## Lo que se descartó

- **Revisar código como puerta humana.** No escala, y pone a la persona donde las máquinas lo
  hacen mejor.
- **Una aprobación por puerta.** Más interrupciones, las mismas decisiones.
- **Polish como parada extra después de un KO.** Haría que lo "casi bien" te costara dos rondas en
  lugar de una.
- **Preguntar cada decisión reversible.** Los agentes se pasarían el día esperando; en su lugar,
  la Decision Inbox te enseña lo que decidieron.

## Ver también

- [Escribir y aprobar escenarios](../how-to/write-scenarios.md): cómo es un buen escenario en G1.
- [Responder a la puerta de salida](../how-to/release-gate.md): OK, KO o polish, paso a paso.
- [Poner en marcha otra vez un Change aparcado](../how-to/parked-change.md): cómo leer la
  Decision Inbox.
- [Medir lo que importa](measuring.md): la tasa de polish y las paradas humanas como señales.
