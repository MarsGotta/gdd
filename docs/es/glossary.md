# Glosario

Las palabras que usan estas páginas, por orden alfabético, cada una con una definición breve y
un enlace a la página que la explica. Cuando gdd muestra un término en inglés distinto, va entre
paréntesis. Si dos páginas parecen no coincidir en una palabra, mandan esta página y la página a
la que enlaza.

## ADR

Un Architecture Decision Record: un fichero corto en `docs/adr/` con el contexto, la decisión, las
alternativas y las consecuencias de una elección que cuesta cambiar. Todo Spike termina en uno.
Consulta el [ADR 0001](../adr/0001-technology.md) para ver un ejemplo real.

## Alcance

El alcance (*scope*) son las rutas que un Change planea tocar. Una ruta fuera del plan con el
mismo color o uno más bajo se acepta y se anota; una de un color más alto hace fallar G2.
Consulta [Cómo elige el riesgo las puertas](explanation/risk.md#la-regla-del-alcance).

## Aparcar

Detener un Change (*park*) dejando un mensaje para la persona en su Decision Inbox, cuando se
alcanza un límite o falla una comprobación. Un nuevo `gdd launch` lo retoma. Consulta
[Volver a poner en marcha un Change aparcado](how-to/parked-change.md).

## Arnés

Los ficheros que ejecutan y protegen el método (*harness*): `gdd/`, `bin/`, el workflow de CI y
las carpetas de los agentes. Es una zona roja y está protegido frente a todos los roles. Consulta
[Mantener honestos a los agentes](explanation/safety.md#capa-1-path-guard).

## Autotest

`gdd selftest` (*self-test*): cuatro comprobaciones de que las puertas fallan cuando deben. Las
máquinas abren G0 solo cuando pasan las cuatro. Consulta
[Mantener honestos a los agentes](explanation/safety.md#el-autotest).

## Board

La pantalla de terminal, de solo lectura, que muestra todos los Changes por fase, con su color,
su estado y sus puertas. Se abre con `gdd` sin argumentos en una terminal, o con `gdd board`.
Consulta la [referencia de comandos](reference/cli.md#gdd-board).

## Brief del proyecto

`docs/brief.md` (*Project Brief*): qué es el proyecto, para quién, en qué plataforma y con qué
tecnología, y qué sería grave si fallara. Se escribe en el Setup. Consulta el
[Brief](../brief.md) del propio gdd.

## Builder

El rol de agente que escribe los pasos, los tests y el código, en dos modos: modo test y modo
build. Su modelo depende del color: barato o local en verde, más capaz en rojo. Consulta
[Roles](reference/roles.md).

## Change

Una pieza pequeña de valor que pasa las puertas como una unidad, en su propia rama
`change/<id>`. Puede cubrir un escenario, varios o ninguno. Siempre un *Change*, nunca una tarea
ni un ticket. Consulta [GDD en una página](explanation/method.md).

## Cola de fusión

Cómo llegan los Changes a main de uno en uno (*merge queue*): con un bloqueo de fusión, un Change
cuyo main ha avanzado toma el main más reciente y vuelve a pasar G3 antes de fusionarse.
Consulta [Ejecutar Changes en paralelo](how-to/parallel-changes.md).

## Color

El nivel de riesgo de una ruta o de un Change: verde, amarillo o rojo. Un Change toma el color
más alto de entre las rutas que toca, y el color decide sus puertas. También se llama su
[Gate Level](#gate-level). Consulta [Cómo elige el riesgo las puertas](explanation/risk.md).

## Decision Inbox

El único mensaje que un Change deja para la persona, en `.gdd/changes/<id>/inbox.md`: qué necesita
su decisión, qué se decidió sin él y qué quedó aparcado. Consulta
[Pocas paradas para la persona](explanation/human-stops.md#la-decision-inbox).

## Deuda conocida

Un comando de puerta que ya fallaba cuando un proyecto con código adoptó GDD (*known debt*).
Avisa en lugar de bloquear, en las puertas y en el autotest, hasta que un Refactor lo pone en
verde, y el [trinquete](#trinquete) impide que crezca. Se guarda en `gdd/debt.toml`. Consulta
[Llevar gdd a un proyecto existente](how-to/adopt-existing-project.md).

## Discover

La fase 1: investigación, opciones, Spikes, el MVP y su regla de abandono, y los siguientes
Changes, todo trabajado con la persona. Termina mapeando las zonas rojas. Consulta
[GDD en una página](explanation/method.md#fases-y-puertas).

## Ejemplo oculto

Un ejemplo extra en `features/hidden/` (*hidden example*) que reutiliza las frases de los
escenarios visibles con datos nuevos. El paquete del Builder lo deja fuera; G3 lo ejecuta.
Consulta
[Mantener honestos a los agentes](explanation/safety.md#capa-2-jueces-que-el-builder-no-puede-tocar).

## Escenario

Una frase en Gherkin (`Given... When... Then...`) dentro de un fichero `.feature` que dice qué
debe pasar (*scenario*). Los escenarios son el contrato que la persona aprueba en G1. Consulta
[Escribir y aprobar escenarios](how-to/write-scenarios.md).

## Escenario de regresión

Un escenario dentro de `features/regression/`, normalmente para fijar un bug. Cambiar solo estos
escenarios no necesita G1. Consulta
[Cómo elige el riesgo las puertas](explanation/risk.md#las-vías).

## Estándares

`docs/standards.md` (*standards*): el listón de calidad del proyecto, por escrito. El Builder y
el Reviewer lo leen, y Learn lo amplía. Consulta los [estándares](../standards.md) del propio
gdd.

## Familia de modelos

La familia de proveedor de un modelo (*model family*), según `[families]` en
`gdd/models.toml`. El Reviewer debe ser de otra familia que el Builder, para que no compartan
puntos ciegos. Consulta [Elegir agentes y modelos](how-to/agents-and-models.md).

## Feature

El tipo de trabajo por defecto: comportamiento nuevo o modificado, normalmente con escenarios
nuevos. Consulta [Tipo de trabajo](#tipo-de-trabajo).

## Ficha de agente

Los datos (*agent sheet*) que gdd guarda de cada agente de programación que puede arrancar
(Claude Code, Codex, OpenCode): su programa, dónde van sus comandos, cómo lanzarlo sin interfaz, los
argumentos de cada rol y cómo leer sus tokens. Consulta
[Mantener honestos a los agentes](explanation/safety.md#capa-3-lo-que-un-agente-puede-llegar-a-hacer).

## Ficha de stack

Los datos (*stack sheet*) que gdd trae para una tecnología (Rust, TypeScript, Python): los
comandos de las puertas, el ejecutor de escenarios, las rutas de pasos protegidas y las
instrucciones para los agentes. Un proyecto puede combinar varias. Consulta
[Stacks](reference/stacks.md).

## G0, puerta de preparación

La puerta que cierra la fase 0 (*G0 Setup Gate*). La abren las máquinas, una vez que existe el
Brief del proyecto y el autotest del arnés pasa las cuatro comprobaciones. Ningún Change empieza
antes. Consulta [Puertas](reference/gates.md).

## G1, puerta de escenarios

La puerta en la que la persona aprueba los escenarios nuevos o modificados de un Change
(*G1 Scenario Gate*), en la misma sentada que las zonas rojas y el plan rojo. Aprobar sella los
ficheros de escenarios con sus hashes. Consulta
[Escribir y aprobar escenarios](how-to/write-scenarios.md).

## G2, puertas del bucle

Las comprobaciones que se ejecutan después de cada vuelta del Builder (*G2 Loop Gates*): linter,
tipos, tests, los escenarios del Change, las reglas de arquitectura, Path Guard y la comprobación
del alcance. Consulta
[La vida de un Change](explanation/life-of-a-change.md#4-modo-build-y-g2).

## G3, puerta de integración

La cadena automática completa antes de la fusión (*G3 Merge Gate*): todos los escenarios,
incluidos los ejemplos ocultos, seguridad, dependencias, secretos y, según el color, mutation
testing. Después, el Reviewer. Consulta
[La vida de un Change](explanation/life-of-a-change.md#5-g3-la-puerta-de-integración).

## G4, puerta de salida

La puerta entre pre y producción (*G4 Release Gate*). La persona responde OK, KO o polish cuando
el Change es rojo o tuvo G1; si no, las máquinas lo promueven en cuanto pre está en verde.
Consulta [Responder a la puerta de salida](how-to/release-gate.md).

## Gate Level

Otro nombre para el [color](#color) de un Change.

## Hotfix

El tipo de trabajo para algo roto en producción: primero se revierte y después se arregla por su
vía, con un escenario de regresión. Su color sigue decidiendo sus puertas. Consulta
[Tipo de trabajo](#tipo-de-trabajo).

## Humano

La persona que usa GDD (*human*): la parte interesada que aprueba escenarios, zonas rojas, planes
rojos y salidas a producción, y no lee código para hacerlo. Consulta
[Pocas paradas para la persona](explanation/human-stops.md).

## Instantánea

Los hashes de lo que un Change ha tocado en un momento dado (*snapshot*). La instantánea de
build, que se toma después del modo test, hace que el modo build responda solo de su propio
trabajo. Consulta [Mantener honestos a los agentes](explanation/safety.md#capa-1-path-guard).

## Learn

La fase 5, sin puerta: los problemas repetidos se convierten en reglas, lints, hooks, tests y
líneas de `docs/standards.md`. `gdd learn` lista las repeticiones. Consulta
[Pocas paradas para la persona](explanation/human-stops.md#learn-convierte-las-notas-repetidas-en-reglas).

## Mapa de zonas

`gdd/zones.toml` (*zone map*): los colores de las rutas del proyecto, con un color por defecto
para el resto (amarillo). Consulta
[Cómo elige el riesgo las puertas](explanation/risk.md#el-mapa-de-zonas).

## Migration

El tipo de trabajo para un Change que se hace en dos pasos, primero ampliar y después contraer:
el camino antiguo se elimina tras un número fijo de días sin uso. Consulta
[Tipo de trabajo](#tipo-de-trabajo).

## Modo build

El segundo modo del Builder (*build mode*): escribe el dominio, la conexión entre piezas y los
tests unitarios hasta que G2 está en verde, y no puede tocar los pasos escritos en modo test.
Consulta [La vida de un Change](explanation/life-of-a-change.md#4-modo-build-y-g2).

## Modo test

El primer modo del Builder (*test mode*): escribe las definiciones de los pasos de los
escenarios, con aserciones reales, y tests de caracterización en amarillo y en rojo. No puede
escribir código de producción. Consulta
[La vida de un Change](explanation/life-of-a-change.md#3-modo-test).

## No-code

El tipo de trabajo para un Change que no añade código. Como un Spike o un Refactor invisible, no
recibe línea de changelog salvo que se le asigne una. Consulta [Tipo de trabajo](#tipo-de-trabajo).

## Núcleo

La parte de GDD que protege frente a los agentes y sigue activa diga lo que diga la
configuración (*core*): las puertas G1 a G4, el semáforo, Path Guard, los ejemplos ocultos, un
Reviewer de otra familia de modelos y mutation testing en los Changes rojos. Consulta
[Decisiones de diseño](explanation/design-decisions.md#mínimo-por-defecto-incremental-si-lo-eliges).

## Orchestrator

El rol de agente que tiene el mapa (arquitectura, zonas, escenarios, estado), divide el trabajo
en Changes, escribe sus paquetes, replanifica una vez un Change atascado y agrupa lo que
necesita a la persona. Nunca escribe código. Consulta [Roles](reference/roles.md).

## Paquete

El contexto mínimo que el Orchestrator escribe para un Change, en `package.md` (*package*): el
objetivo, el color, los escenarios, las rutas que puede tocar, qué reutilizar y el comando de G2.
Consulta
[Decisiones de diseño](explanation/design-decisions.md#roles-con-el-contexto-mínimo).

## Path Guard

La comprobación de que un rol no ha tocado ninguna de sus rutas protegidas ni ningún escenario
sellado en G1 desde que empezó el build del Change. Se ejecuta antes y después de los comandos
de cada puerta. Consulta
[Mantener honestos a los agentes](explanation/safety.md#capa-1-path-guard).

## Plan rojo

El plan de un Change rojo, a nivel de plan, nunca de código (*red plan*): qué, cómo, riesgos y
cómo revertirlo. La persona lo aprueba en la misma sentada que los escenarios. Consulta
[Pocas paradas para la persona](explanation/human-stops.md#una-sentada-por-change).

## Polish

La tercera respuesta en G4 (pulir): una vuelta más del Builder con las notas de la persona, y
después otra vez las puertas y la demo. Sustituye a un KO; no es una parada extra. Consulta
[Pocas paradas para la persona](explanation/human-stops.md#hecho-no-es-bueno).

## Pre

El entorno en el que se despliega un Change fusionado antes de producción, donde se ejecutan sus
tests en pre y su demo. Consulta
[La vida de un Change](explanation/life-of-a-change.md#7-fusionar-y-después-pre).

## Producción

Donde los usuarios reciben el Change (*production*). La promoción toma el mismo artefacto de
pre, sin volver a construirlo, y lo revierte si suben los errores. Consulta
[La vida de un Change](explanation/life-of-a-change.md#9-promover-observar-cerrar).

## Puerta

Un punto de control que un Change debe pasar para seguir adelante (*gate*). Responde sí o no, y
dice por qué. Consulta [GDD en una página](explanation/method.md#la-idea).

## Refactor

El tipo de trabajo para cambiar cómo está construido el código sin cambiar lo que hace. Consulta
[Tipo de trabajo](#tipo-de-trabajo).

## Registro

Los hechos de un Change (*record*), en `.gdd/changes/<id>/change.json`, que solo escribe gdd: su
decisión, aprobaciones, revisiones, notas, estado y lo que ha gastado. Su commit de fusión lleva
un resumen a git. Consulta
[Mantener honestos a los agentes](explanation/safety.md#capa-4-un-registro-que-solo-escribe-gdd).

## Regla de abandono

Lo que tendría que pasar para abandonar un MVP (*kill rule*), escrito en Discover antes de que
empiece el trabajo y con números que los informes puedan mostrar. Consulta
[Medir lo que importa](explanation/measuring.md#la-regla-de-abandono).

## Replanificar

El único intento del Orchestrator de desatascar un Change cuya G2 sigue en rojo después de las
vueltas del Builder (*replan*). Termina con `NEXT: build`, o con `NEXT: test` para devolver el
Change una vez al modo test. Consulta
[La vida de un Change](explanation/life-of-a-change.md#4-modo-build-y-g2).

## Reviewer

El rol de agente, de otra familia de modelos que el Builder, que lee el Change entero y los
resultados de las puertas y responde OK o KO con sus motivos. Solo lee, y tiene una corrección.
Consulta [Roles](reference/roles.md).

## Sentada

El único momento antes de Build en el que la persona aprueba todo lo que un Change necesita de él
(*sitting*): los escenarios, las zonas rojas y el plan rojo. G4 tiene su propia sentada.
Consulta [Pocas paradas para la persona](explanation/human-stops.md#una-sentada-por-change).

## Setup

La fase 0, una vez por proyecto: unas pocas preguntas básicas, el Brief del proyecto, el ADR de
tecnología, el esqueleto y el arnés. Consulta
[GDD en una página](explanation/method.md#fases-y-puertas).

## Spike

Un Change que responde a una pregunta para que la persona pueda decidir: se construye rápido,
dentro de un presupuesto fijo, y termina en un ADR. Solo el ADR llega a main; su código, nunca.
Consulta [Explorar con un Spike](how-to/spikes.md).

## Tipo de trabajo

La clase de trabajo que es un Change (*work type*): Feature, Hotfix, Refactor, Migration, Spike o
No-code. Nunca cambia el color; un Spike conserva su color pero se salta las puertas que
necesitaría su código, porque su código nunca se fusiona. Consulta
[Cómo elige el riesgo las puertas](explanation/risk.md#el-color-manda-sobre-el-tipo-de-trabajo-y-la-vía).

## Trinquete

Un techo para la deuda conocida (*ratchet*): `gdd adopt` cuenta lo que informa cada comando que
falla, un Change que sube la cuenta hace fallar la puerta y uno que la baja baja el techo para
siempre. Consulta [Llevar gdd a un proyecto existente](how-to/adopt-existing-project.md).

## Variante

Una de las dos a cuatro versiones de un Spike (*variant*), cada una construida por su propio
Builder en su propia rama, que se comparan en una sola página en G4. Consulta
[Explorar con un Spike](how-to/spikes.md).

## Vía

El camino que recorre un Change (*lane*): la [vía rápida](#vía-rápida) o la
[vía completa](#vía-completa). La vía nunca baja el color. Consulta
[Cómo elige el riesgo las puertas](explanation/risk.md#las-vías).

## Vía completa

La vía que toma todo Change salvo que cumpla los requisitos de la vía rápida (*Full Lane*), con
las puertas que la tabla de reglas asigna a su color. Consulta
[Cómo elige el riesgo las puertas](explanation/risk.md#las-vías).

## Vía rápida

La vía con G2 y G3 y sin parada humana (*Fast Lane*), abierta solo a un Change que no es rojo,
no crea ni modifica ningún escenario y es retomable. Consulta
[Cómo elige el riesgo las puertas](explanation/risk.md#las-vías).

## Worktree

La carpeta de trabajo de git aparte donde se construye un Change, `<repo>.worktrees/<id>` por
defecto, para que el checkout principal se quede en main. Consulta
[La vida de un Change](explanation/life-of-a-change.md#1-lanzar).

## Zona roja

Una ruta del mapa de zonas en la que un error sería grave (*red zone*). La persona aprueba las
zonas rojas, y vuelve a aprobarlas cuando cambian. Consulta
[Mapear y aprobar zonas rojas](how-to/red-zones.md).

## Ver también

- [GDD en una página](explanation/method.md): los términos en contexto.
- [Las reglas completas del método](../../GDD.md): el origen de cada definición (en inglés).
