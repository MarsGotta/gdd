# Documentación de gdd

gdd ejecuta GDD, Gate-Driven Development: una forma de construir software con agentes de
programación de IA en la que cada Change avanza pasando puertas. El riesgo decide cuántas
puertas encuentra un Change y lo estrictas que son. Las máquinas comprueban lo que se puede
comprobar; tú decides solo lo que necesita a una persona, en el menor número de sentadas
posible.

¿Acabas de llegar? Empieza por el tutorial: en una media hora preparas un proyecto pequeño,
apruebas sus primeros escenarios y ves cómo los agentes construyen, revisan y publican un
Change.

**[Tu primer Change →](getting-started/tutorial.md)**

*This documentation is also [in English](../README.md). gdd, its commands and its messages
are in English; this translation explains them in Spanish.*

## Aprender

Paso a paso, de cero a un Change publicado.

- [Tu primer Change](getting-started/tutorial.md): prepara un proyecto, escribe y aprueba un
  escenario, lanza el Change y responde a su puerta de salida.
- [GDD en una página](explanation/method.md): las fases, las puertas y el semáforo, con un
  diagrama de cada cosa.

## Hacer

Guías para un objetivo que ya tienes.

**El día a día**

- [Instalar gdd](how-to/install.md): el binario, los agentes y las herramientas que usan las
  puertas.
- [Ejecutar un Change](how-to/run-a-change.md): créalo, lánzalo y síguelo en el Board.
- [Escribir y aprobar escenarios](how-to/write-scenarios.md): Gherkin, ejemplos ocultos y la
  puerta de escenarios (G1).
- [Responder a la puerta de salida](how-to/release-gate.md): OK, KO o polish en G4.
- [Volver a poner en marcha un Change aparcado](how-to/parked-change.md): la Decision Inbox,
  los relanzamientos y los conflictos de fusión.

**Dar forma al trabajo**

- [Mapear y aprobar zonas rojas](how-to/red-zones.md): dile a gdd qué rutas son arriesgadas.
- [Explorar con un Spike](how-to/spikes.md): un presupuesto fijo, variantes lado a lado y un
  ADR.
- [Ejecutar Changes en paralelo](how-to/parallel-changes.md): el límite, los solapes y la cola
  de fusión.

**Preparar y ajustar**

- [Llevar gdd a un proyecto existente](how-to/adopt-existing-project.md): init, bootstrap y
  adopt.
- [Elegir agentes y modelos](how-to/agents-and-models.md): quién construye y quién revisa, por
  color.
- [Ajustar las puertas](how-to/gates-and-commands.md): los comandos que ejecuta cada puerta,
  los límites y mutation testing.
- [Publicar versiones y llevar un changelog](how-to/releases-and-changelog.md): tags,
  `gdd release`, CHANGELOG.md.
- [Ejecutar las puertas en CI](how-to/ci.md): qué comprueba el workflow en cada pull request.
- [Medir el trabajo](how-to/measure.md): tokens, tiempo de flujo, DORA y Learn.
- [Actualizar gdd en un proyecto](how-to/upgrade.md): versiones nuevas de los ficheros que
  escribió gdd.
- [Solución de problemas](how-to/troubleshooting.md): síntomas, causas y soluciones.

## Consultar

Exacto y completo. La referencia de comandos se genera a partir del código.

- [Referencia de comandos](reference/cli.md): todos los comandos de `gdd` y sus opciones.
- [Configuración](reference/configuration.md): cada fichero de `gdd/` y cada clave.
- [Puertas](reference/gates.md): qué comprueba cada puerta, quién decide y sus límites, por
  color.
- [Roles](reference/roles.md): la persona, el Orchestrator, el Builder y el Reviewer.
- [Stacks](reference/stacks.md): las fichas de Python, Rust y TypeScript.
- [Ficheros que escribe gdd](reference/files.md): registros, el log, la Decision Inbox y las
  salidas.
- [Variables de entorno](reference/environment.md): qué lee gdd y qué les da a los agentes.
- [Glosario](glossary.md): las palabras que usan estas páginas.

## Entender

Por qué GDD funciona como funciona.

- [GDD en una página](explanation/method.md): fases, puertas, colores y roles.
- [La vida de un Change](explanation/life-of-a-change.md): cada paso de un lanzamiento, en
  orden.
- [Cómo elige el riesgo las puertas](explanation/risk.md): el semáforo y por qué gana el color.
- [Mantener honestos a los agentes](explanation/safety.md): Path Guard, los ejemplos ocultos y
  lo que los agentes no pueden tocar.
- [Pocas paradas para la persona](explanation/human-stops.md): sentadas, polish y por qué
  hecho no es bueno.
- [Medir lo que importa](explanation/measuring.md): tokens, DORA y la regla de abandono.
- [Decisiones de diseño](explanation/design-decisions.md): qué se eligió y qué no.

## Este proyecto, construido con GDD

gdd se construye con GDD, así que sus propios ficheros de GDD son ejemplos reales de cada
artefacto (en inglés):

- [Las reglas completas del método](../../GDD.md) y la [hoja de ruta](../../ROADMAP.md).
- [Brief del proyecto](../brief.md), [estándares](../standards.md), [decisiones](../adr/) y
  [notas de Discover](../discover/).
- [Escenarios](../../features/): la especificación ejecutable de gdd.

## Contribuir

- [Cómo contribuir](../../CONTRIBUTING.md): prepara el entorno, prueba y envía tu contribución.
- [Escribir la documentación](../STYLE.md): cómo se organizan y se escriben estas páginas.
- [Changelog](../../CHANGELOG.md).
