# Documentación de gdd

gdd ejecuta GDD, Gate-Driven Development, una forma de construir software con agentes de
programación de IA. Cada Change avanza pasando *puertas*, comprobaciones que debe superar antes
de su siguiente paso. El riesgo decide cuántas puertas encuentra un Change y lo estrictas que
son. Las máquinas comprueban lo que se puede comprobar, y tú decides solo lo que necesita a una
persona: los escenarios antes de construir y el resultado al final.

*This documentation is also [in English](../README.md).* gdd, sus comandos y sus mensajes están
en inglés; esta traducción los explica en español.

## ¿Por dónde empiezas?

- **Proyecto nuevo:** el [Inicio rápido](getting-started/quickstart.md) va de instalar gdd a un
  primer Change publicado, a través de tu agente.
- **Código existente:** [Llevar gdd a un proyecto existente](how-to/adopt-existing-project.md)
  convierte lo que hoy falla en deuda conocida y da a tus carpetas sus colores de riesgo.
- **Un Change entero, paso a paso:** el tutorial [Tu primer Change](getting-started/tutorial.md)
  enseña un Change real y todas las pantallas del camino.
- **Primero el método:** [GDD en una página](explanation/method.md) explica las fases, las
  puertas y el semáforo.
- **Un comando:** la [referencia de la línea de comandos (CLI)](reference/cli/index.md) recoge
  todos los comandos de `gdd`, sus opciones y lo que imprimen.
- **Una skill:** la [referencia de skills](reference/skills/index.md) dice qué hacen
  `/gdd-setup`, `/gdd-change` y las demás skills, y cuándo ejecutar cada una.

## Cómo usas gdd

- **Terminal,** una vez por proyecto: instalas gdd y ejecutas `gdd init`. Su formulario prepara
  el proyecto, instala sus stacks y después te dice que arranques tu agente.
- **Chat del agente,** para todo lo demás: `/gdd-setup` una vez y después `/gdd-change` para
  cada Change. En Codex las skills empiezan por `$`: `$gdd-setup`, `$gdd-change`.
- **Aprobaciones,** en el chat o en la pantalla de la sentada (`gdd sitting CHANGE`): los
  escenarios antes de construir, y OK, KO o polish sobre el resultado en la puerta de salida
  (G4, *Release Gate*).
- **Board,** para mirar si quieres: `gdd` a secas en una terminal, o `gdd board`. No se
  actualiza solo; para ver qué ha avanzado, vuelve a abrirlo.

## Cómo orientarte

- **Empezar:** [Inicio rápido](getting-started/quickstart.md),
  [Tu primer Change](getting-started/tutorial.md) y [GDD en una página](explanation/method.md).
- **Guías,** con el nombre de lo que quieres hacer:
  [Describir un Change a tu agente](how-to/run-a-change.md),
  [Aprobar los escenarios](how-to/write-scenarios.md),
  [Revisar un Change en G4](how-to/release-gate.md),
  [Volver a poner en marcha un Change aparcado](how-to/parked-change.md),
  [Instalar gdd](how-to/install.md),
  [Llevar gdd a un proyecto existente](how-to/adopt-existing-project.md),
  [Mapear y aprobar zonas rojas](how-to/red-zones.md),
  [Explorar con un Spike](how-to/spikes.md),
  [Ejecutar Changes en paralelo](how-to/parallel-changes.md),
  [Elegir agentes y modelos](how-to/agents-and-models.md),
  [Ajustar las puertas](how-to/gates-and-commands.md),
  [Ejecutar las puertas en CI](how-to/ci.md),
  [Actualizar gdd en un proyecto](how-to/upgrade.md),
  [Publicar versiones y mantener un changelog](how-to/releases-and-changelog.md),
  [Medir el trabajo](how-to/measure.md) y
  [Solución de problemas](how-to/troubleshooting.md).
- **Referencia,** exacta y completa:
  - [Skills](reference/skills/index.md): [`gdd-setup`](reference/skills/setup.md),
    [`gdd-discover`](reference/skills/discover.md), [`gdd-change`](reference/skills/change.md),
    [`gdd-status`](reference/skills/status.md), [`gdd-report`](reference/skills/report.md),
    [`gdd-learn`](reference/skills/learn.md), [`gdd-hotfix`](reference/skills/hotfix.md) y
    [Skills de los agentes de gdd](reference/skills/agent-skills.md).
  - [Referencia de la CLI](reference/cli/index.md), una página por comando:
    [`init`](reference/cli/init.md), [`bootstrap`](reference/cli/bootstrap.md),
    [`adopt`](reference/cli/adopt.md), [`agents`](reference/cli/agents.md),
    [`agents install`](reference/cli/agents-install.md),
    [`agents check`](reference/cli/agents-check.md), [`stacks`](reference/cli/stacks.md),
    [`stacks show`](reference/cli/stacks-show.md),
    [`stacks compose`](reference/cli/stacks-compose.md), [`upgrade`](reference/cli/upgrade.md),
    [`new-change`](reference/cli/new-change.md), [`classify`](reference/cli/classify.md),
    [`sitting`](reference/cli/sitting.md), [`gate`](reference/cli/gate.md),
    [`note`](reference/cli/note.md), [`changelog-line`](reference/cli/changelog-line.md),
    [`launch`](reference/cli/launch.md), [`resume`](reference/cli/resume.md),
    [`park`](reference/cli/park.md), [`prepare`](reference/cli/prepare.md),
    [`merge`](reference/cli/merge.md), [`release`](reference/cli/release.md),
    [`changelog`](reference/cli/changelog.md), [`g2`](reference/cli/g2.md),
    [`g3`](reference/cli/g3.md), [`guard`](reference/cli/guard.md),
    [`snapshot`](reference/cli/snapshot.md), [`scope`](reference/cli/scope.md),
    [`selftest`](reference/cli/selftest.md), [`ci`](reference/cli/ci.md),
    [`status`](reference/cli/status.md), [`board`](reference/cli/board.md),
    [`inbox`](reference/cli/inbox.md), [`report`](reference/cli/report.md),
    [`learn`](reference/cli/learn.md) y [`session`](reference/cli/session.md).
  - [Pantallas](reference/screens.md), [Configuración](reference/configuration.md),
    [Puertas](reference/gates.md), [Roles](reference/roles.md), [Stacks](reference/stacks.md),
    [Ficheros que escribe gdd](reference/files.md),
    [Variables de entorno](reference/environment.md) y el [Glosario](glossary.md).
- **Conceptos,** por qué GDD funciona como funciona:
  [La vida de un Change](explanation/life-of-a-change.md),
  [Cómo elige el riesgo las puertas](explanation/risk.md),
  [Mantener honestos a los agentes](explanation/safety.md),
  [Pocas paradas para la persona](explanation/human-stops.md),
  [Medir lo que importa](explanation/measuring.md) y
  [Decisiones de diseño](explanation/design-decisions.md).

## Este proyecto, construido con GDD

gdd se construye con GDD, así que sus propios ficheros de GDD son ejemplos reales de cada
artefacto (en inglés):

- [Las reglas completas del método](../../GDD.md) y la [hoja de ruta](../../ROADMAP.md).
- [Brief del proyecto](../brief.md), [estándares](../standards.md), [decisiones](../adr/) y
  [notas de Discover](../discover/).
- [Escenarios](../../features/): la especificación ejecutable de gdd.

## Contribuir

- [Cómo contribuir](../../CONTRIBUTING.md): prepara el entorno, prueba y envía tu contribución.
- [Escribir la documentación de gdd](../STYLE.md): cómo se organizan y se escriben estas
  páginas.
- [Changelog](../../CHANGELOG.md).
