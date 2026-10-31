# gdd-setup

La skill de Setup es la fase 0 de GDD. El Orchestrator te hace unas pocas preguntas básicas,
escribe el Brief del proyecto y deja listo el arnés, para que las máquinas abran la puerta de
preparación (G0, *Setup Gate*). La ejecutas una vez por proyecto, siempre la primera, justo
después de `gdd init`. El arnés es el conjunto de ficheros que usa gdd para comprobar tu
proyecto.

## Antes de empezar

- Un proyecto hecho con [`gdd init`](../cli/init.md). El formulario también ejecuta la primera
  instalación de cada stack, y termina diciéndote que ejecutes esta skill. Después de un
  `gdd init` con flags, esa instalación la ejecuta la propia skill.
- El proyecto puede ser nuevo o tener ya código. En un repositorio con código, la skill registra
  lo que falla hoy como deuda conocida (paso 8).

## Llámala

En Claude Code y OpenCode:

```text
/gdd-setup
```

En Codex:

```text
$gdd-setup
```

La entrada es opcional. Úsala para decir lo que ya sabes, y así habrá menos preguntas:

```text
/gdd-setup a command-line notes tool for one person; losing a note would be serious
```

## Qué te pregunta

Primero lee lo que ya hay: `gdd/project.toml` y `docs/brief.md`, que `gdd init` escribió con tus
respuestas al formulario, después cualquier documento de semilla o de idea del repositorio (un
`SEED.md`, por ejemplo) y el código, cuando el repositorio no está vacío. Después pregunta, en un
solo mensaje corto, solo lo que esos ficheros no dicen. Un nombre, una plataforma y un stack en
`gdd/project.toml` no se vuelven a preguntar; una sección vacía del Brief, sí. Las preguntas que
puede hacerte:

- ¿Qué es el proyecto, en cinco líneas? Cuando existe un documento de semilla, lo resume y te
  pide que lo confirmes.
- ¿Dónde se ejecuta: móvil, escritorio, web, línea de comandos u otro?
- ¿Está decidida la tecnología? Si no, recomienda una opción que encaje con la plataforma, con el
  motivo en dos líneas, y como mucho una alternativa.
- ¿Qué sería grave si fallara: perder datos, la privacidad, el dinero, producción, las claves?

Antes de terminar te pide una cosa más: tu OK a las zonas rojas que escribió `gdd init` (paso 6).

Cada pregunta llega con su recomendación delante. Cuando algo es reversible y duda entre
preguntar y decidir, decide y te lo dice. Su último mensaje termina con un siguiente paso claro.

## Qué hace

1. **Escribe el Brief del proyecto** en `docs/brief.md`: la idea, para quién es, la plataforma,
   la tecnología y por qué, las áreas sensibles, las restricciones, el producto mínimo viable
   (MVP) y la regla de abandono cuando existen, y las preguntas abiertas. Rellena `name`,
   `platform` y `stack` en `gdd/project.toml` y registra la elección de tecnología como registro
   de decisión de arquitectura (ADR) en `docs/adr/0001-technology.md`.
2. **Elige el ejecutor de escenarios.** GDD exige Gherkin, el formato en lenguaje llano de los
   escenarios, no una herramienta para ejecutarlo. El Orchestrator solo recomienda ejecutores
   con amplio soporte, como la implementación oficial de Cucumber para el lenguaje o el estándar
   de su ecosistema. Por ejemplo, cucumber-js para TypeScript, pytest-bdd o behave para Python y
   cucumber-rs para Rust. Comprueba que el ejecutor sigue mantenido, falla con pasos sin definir
   o pendientes, ejecuta un subconjunto de ficheros `.feature` y ejecuta `features/hidden/`. La
   elección va al Brief y a `scenario_runner` de `gdd/project.toml`.
3. **Completa el esqueleto** en esa tecnología: las carpetas, un test unitario, un primer
   escenario de humo en `features/` con su paso, y las carpetas `features/regression/` y
   `features/hidden/`. Los pasos hablan con el núcleo de la aplicación, no con la pantalla.
4. **Escribe los comandos de las puertas** en `gdd/commands.toml`: lo que ejecutan la puerta del
   bucle (G2, *Loop Gate*), la puerta de integración (G3, *Merge Gate*), los pasos de release y
   el autotest. Prueba cada comando a mano antes de escribirlo. Si todavía no hay pre ni
   producción reales, pone `simulated = true` y te lo dice.
5. **Completa el workflow de integración continua (CI)** `.github/workflows/gdd.yml` con la
   preparación de la tecnología. La CI ejecuta [`gdd ci`](../cli/ci.md).
6. **Te pide que apruebes las zonas rojas que escribió `gdd init`.** Tus áreas sensibles se
   quedan como palabras en el Brief, porque todavía no hay carpetas reales que mapear, y el
   código sin mapear es amarillo. El mapa de zonas solo tiene las zonas que escribió `gdd init`,
   y la zona del arnés viene en rojo. Ningún Change puede lanzarse antes de que una persona
   apruebe el mapa de zonas, así que la skill te lo pide en una línea y te dice qué protege la
   zona del arnés: los ficheros del propio gdd, la CI y las instrucciones de los agentes. Con tu
   OK ejecuta [`gdd gate zones ok --by human`](../cli/gate.md). La skill `gdd-discover` mapea
   después tus propias áreas sensibles.
7. **Protege las rutas correctas** en `gdd/rules.toml`: el código de producción frente al Builder
   en modo test, y la carpeta de los pasos de los escenarios frente al Builder en modo build.
8. **Instala y adopta cuando hace falta.** Cuando falta el fichero de lock de un stack (el
   fichero que fija las versiones exactas de sus dependencias) después de un `gdd init` con
   flags, ejecuta [`gdd bootstrap`](../cli/bootstrap.md). Cuando el
   repositorio ya tenía código, ejecuta [`gdd adopt`](../cli/adopt.md) antes del autotest: lo que
   falla hoy pasa a ser deuda conocida en `gdd/debt.toml`, para que G0 pueda abrirse, y te dice
   en dos líneas qué se ha convertido en deuda y que cada deuda recibe un Refactor.
9. **Ejecuta el autotest**, [`gdd selftest`](../cli/selftest.md), hasta que dice
   `harness 4/4`.
10. **Abre G0 como las máquinas**, con [`gdd gate G0 ok --by machines`](../cli/gate.md). G0 solo
    se abre con el Brief escrito y el autotest en 4/4. Te dice en dos líneas que el arnés está
    listo.
11. **Hace commit en `main`** con el mensaje `gdd: setup (G0)`.
12. **Cuenta la conversación** con [`gdd session --phase 0-setup`](../cli/session.md), que añade
    al log las peticiones de tus sesiones interactivas de Claude Code.

## Qué decides tú

- Tus respuestas a las preguntas, y si aceptas cada recomendación: sobre todo la tecnología y el
  ejecutor de escenarios.
- Las zonas rojas que escribió `gdd init`, con un solo OK.
- Nada sobre G0 en sí. Las máquinas la abren cuando existe el Brief y pasa el autotest; no es una
  parada humana. Consulta [G0 Setup Gate](../gates.md#g0-setup-gate).

## Qué deja

| Qué | Dónde |
|---|---|
| El Brief del proyecto | `docs/brief.md` |
| La decisión de tecnología | `docs/adr/0001-technology.md` |
| Nombre, plataforma, stacks y ejecutor de escenarios | `gdd/project.toml` |
| Los comandos de las puertas y del release | `gdd/commands.toml` |
| Las rutas protegidas | `gdd/rules.toml` |
| El esqueleto y el primer escenario de humo | las carpetas del stack, `features/` |
| El workflow de CI | `.github/workflows/gdd.yml` |
| La deuda conocida, en un proyecto que ya tenía código | `gdd/debt.toml`, en su propio commit (`gdd adopt: known debt in ...`) |
| Tu aprobación de las zonas rojas | `.gdd/zones.json` |
| El resultado del autotest y G0 | `.gdd/selftest.json`, `.gdd/g0.json`, `.gdd/log.jsonl` |
| El commit de Setup | `gdd: setup (G0)` en `main` |

[Ficheros](../files.md) describe cada fichero.

## Qué viene después

La skill termina enviándote al primer Change: ejecuta [`gdd-change`](change.md) con lo que
quieres, en una frase. Cuando el producto necesita primero una arquitectura o un mapa de sus
áreas sensibles, propone antes [`gdd-discover`](discover.md).
[`gdd status`](../cli/status.md) ya dice `G0: ok`.

## Ver también

- [Llevar gdd a un proyecto existente](../../how-to/adopt-existing-project.md): Setup en un
  repositorio que ya tiene código.
- [Puertas](../gates.md#g0-setup-gate): qué comprueba G0 y cuándo dice que no.
- [Stacks](../stacks.md): lo que aporta cada ficha de stack al esqueleto y a los comandos de las
  puertas.
