# Publicar versiones y mantener un changelog

Dale a cada Change una línea en el changelog, el fichero que cuenta a las personas que usan tu
proyecto qué cambió en cada versión. Después publica versiones automáticamente o por lotes, y
configura los pasos que llevan un Change a producción. Usa esta guía antes de tu primera
publicación, o cuando quieras cambiar cómo se crean las versiones.

## Antes de empezar

- Un proyecto GDD con al menos un Change fusionado: consulta [Describir un Change a tu
  agente](run-a-change.md).
- Conoces la puerta de salida (G4, *Release Gate*): consulta
  [Revisar un Change en G4](release-gate.md).

## 1. Dale a cada Change su línea del changelog

Cada Change lleva una línea en su commit de fusión, como trailers de git:

```text
Changelog: Added · The cart shows its total
Breaking: no
```

Si no tiene línea, el título del Change va a la sección de su tipo: Feature a `Added`, Hotfix a
`Fixed`, Migration a `Changed`. Los Changes de tipo Refactor, Spike y No-code se quedan fuera.
Si el título no es la línea que quieres, pide a tu agente que fije una mejor antes de la fusión,
o fíjala tú:

```bash
gdd changelog-line cart-total Added The cart shows its total
```

```text
Changelog line of cart-total: Added · The cart shows its total
```

La sección es una de las de [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), un
formato habitual de changelog: `Added`, `Changed`, `Deprecated`, `Removed`,
`Fixed` o `Security`. Cualquier otra se rechaza:

```text
error: the section must be one of Added, Changed, Deprecated, Removed, Fixed, Security
```

Marca un Change que rompe algo que antes funcionaba, o deja uno fuera a propósito:

```bash
gdd changelog-line export-csv Changed Orders export as CSV instead of JSON --breaking
gdd changelog-line tidy-cart --none
```

El commit de fusión de `export-csv` termina entonces con:

```text
Changelog: Changed · Orders export as CSV instead of JSON
Breaking: yes
```

Los Builders y los Reviewers no pueden fijar la línea. En gdd 0.1.0, si las palabras de una
línea empiezan por `--`, pon un `--` delante de ellas:
[`gdd changelog-line CHANGE Changed -- --time is faster`](../reference/cli/changelog-line.md).

## 2. Pon al día CHANGELOG.md

`CHANGELOG.md` es tuyo: gdd nunca borra ni reescribe ni una palabra. En gdd 0.1.0,
[`gdd init`](../reference/cli/init.md) no lo crea. Mientras el fichero no exista, cada línea vive
solo en los trailers de su commit de fusión, y una versión automática etiqueta ese commit de
fusión. Para empezar el fichero, pide a tu agente que empiece `CHANGELOG.md`, o créalo con
`gdd changelog --write` como se muestra abajo y haz commit.

[`gdd changelog`](../reference/cli/changelog.md) compara el fichero con las líneas que llevan los
commits de fusión de main, versión a versión, y muestra lo que falta. Lo que todavía no ha
publicado ninguna etiqueta `v<semver>` va en Unreleased. Sin fichero, lista todas las líneas
fusionadas hasta ahora.

```bash
gdd changelog
```

Deberías ver:

```text
Missing from CHANGELOG.md:

## [Unreleased]

### Added

- The cart shows its total <!-- gdd: 2dabea449211 -->

Nothing written: gdd changelog --write adds them.
```

Para añadir lo que falta, o crear el fichero con la cabecera de Keep a Changelog, escríbelo y haz
tú el commit:

```bash
gdd changelog --write
git add CHANGELOG.md
git commit -m "Update CHANGELOG.md"
```

```text
CHANGELOG.md: 1 line(s) added, nothing else changed.
```

Cada línea añadida lleva un comentario que nombra su commit, así que una línea que reescribas
más tarde sigue contando como presente. Una línea con las mismas palabras también cuenta. En una
terminal, `gdd changelog` sin `--write` pregunta antes de escribir.

Si tu fichero ya tiene versiones, gdd mantiene el estilo de sus encabezados:

| Tu fichero tiene | gdd lo lee como |
|---|---|
| `## [Unreleased]`, `## Unreleased` (en mayúsculas o minúsculas) | la sección Unreleased |
| `## [0.1.0] - 2026-10-30` | Keep a Changelog |
| `## 0.1.0 - 2026-10-30` | sin corchetes |
| `## v0.1.0` | con una `v` |
| `## [0.1.0](<compare link>) (2026-10-30)` | un encabezado con enlace |

Las versiones nuevas siguen el estilo de tu encabezado de versión más reciente, y un fichero sin
secciones `###` recibe líneas sueltas.

## 3. Elige cómo se crean las versiones

Una versión es una etiqueta git `v<major>.<minor>.<patch>` que sigue el versionado semántico
(*semver*), una numeración en la que una patch corrige, una minor añade y una major rompe lo que
funcionaba. La siguiente versión sale de las líneas de los Changes desde la etiqueta más
reciente:

| Línea | Siguiente versión |
|---|---|
| `Fixed`, `Security` | patch |
| `Added`, `Changed`, `Deprecated`, `Removed` | minor |
| cualquier línea con `--breaking` | major, o minor mientras la major sea 0 |

La primera versión de un proyecto es `0.1.0`. Elige uno de estos dos modos en `gdd/rules.toml`:

- **auto** (el predeterminado, sin sección `[versions]`): cada Change con línea de changelog
  recibe su versión cuando se promueve a producción. El paso de promoción la recibe en
  `GDD_VERSION`, y gdd crea la etiqueta solo cuando la promoción sale bien. Si main tiene un
  `CHANGELOG.md` en git, su sección Unreleased se convierte en esa versión en un commit
  `Release v<x.y.z>`, que es al que apunta la etiqueta. Sin el fichero, la etiqueta apunta al
  commit de fusión.
- **manual**: las promociones no crean etiquetas; publicas por lotes con
  [`gdd release`](../reference/cli/release.md).

  ```toml
  [versions]
  mode = "manual"   # release in batches with gdd release
  ```

En modo auto, un Change promovido en un proyecto que tiene su changelog en git se ve así en main
(`Start CHANGELOG.md` es el commit que creó el fichero):

```text
d78a276 Release v0.1.0
06118e8 Start CHANGELOG.md
2dabea4 gdd(cart-total): G3 Merge Gate · Cart total
```

## 4. Publica a mano

`gdd release` sin paso muestra la siguiente versión de lo que main ha fusionado desde su
etiqueta más reciente, y no publica nada:

```bash
gdd release
```

```text
Next version: v0.2.0 (from v0.1.0)

### Changed

- **Breaking:** Orders export as CSV instead of JSON

### Security

- Repeated failed logins are slowed down

Nothing released: gdd release --yes releases it.
```

Publícala:

```bash
gdd release --yes
```

```text
Released v0.2.0
```

Funciona en los dos modos. Si main tiene `CHANGELOG.md` en git, gdd añade las líneas que faltan
y convierte Unreleased en `## [0.2.0] - <day>` en un commit propio (`Release v0.2.0`). Después
crea una etiqueta anotada `v0.2.0`. No se hace push de nada: sube tú el commit y la etiqueta.

Para elegir la versión, añade `--version`. Sin `--yes`, muestra lo que publicaría:

```bash
gdd release --version 1.0.0
```

```text
Next version: v1.0.0 (from v0.2.0)

### Added

- Receipt

Nothing released: gdd release --yes releases it.
```

La versión tiene que ir después de la etiqueta más reciente. `gdd release --version 0.1.0`
en un proyecto que ya está en `v0.1.0` responde:

```text
error: the version must come after v0.1.0
```

Si no se ha fusionado nada desde la etiqueta más reciente, `gdd release` dice
`Nothing to release since v0.2.0`. Los Builders y los Reviewers no pueden publicar.

## 5. Configura los pasos de publicación

Después de la fusión, la receta lleva un Change por estos pasos, cada uno un comando de la
sección `[release]` de `gdd/commands.toml`:

```mermaid
flowchart TD
    M["Fusión: build, deploy_pre"] --> P[pre: tests_pre, demo]
    P --> G{"¿Hace falta G4?"}
    G -->|"sí"| H[La persona responde a G4]:::human
    G -->|no| R[promote]
    H -->|OK| R
    R --> O[observe]
    O -->|suben los errores| B[rollback]:::red
    O -->|todo bien| C[cerrado]:::green
    classDef green fill:#d9f2e3,stroke:#2e8b57,color:#123
    classDef red fill:#fbdcdc,stroke:#c0392b,color:#311
    classDef human fill:#e6e0f8,stroke:#6a4fc0,color:#213
```

[`gdd init`](../reference/cli/init.md) los escribe vacíos, con `simulated = true`: los pasos
vacíos pasan y se registran como simulados hasta que tengas un pre y una producción reales.
Rellénalos y pon `simulated = false`:

```toml
[release]
simulated = false
build = "uv build -q"                                       # build the artifact once
deploy_pre = "mkdir -p .gdd/pre && cp dist/*.whl .gdd/pre/"
tests_pre = "ls .gdd/pre/*.whl"
demo = "mkdir -p .gdd/changes/$GDD_CHANGE/demo && GDD_FEATURES= uv run pytest -q tests/acceptance > .gdd/changes/$GDD_CHANGE/demo/scenarios.txt"
promote = "./deploy.sh production $GDD_VERSION"             # the same artifact, to production
observe = "./check-error-rate.sh"                           # fails if errors rise
rollback = "./deploy.sh production previous"
rollback_pre = "./deploy.sh pre previous"                   # used by the self-test
```

Arriba, `promote`, `observe` y `rollback` llaman a scripts propios, y el resto es un ejemplo que
funciona para un paquete Python. Promueve el mismo artefacto que construiste una vez: nunca lo
vuelvas a construir para producción. Cada paso puede durar lo que marque `command_seconds` en
`[limits]`. Para probar un paso, ejecútalo por separado:

```bash
gdd release pre CHANGE
gdd release promote CHANGE
gdd release observe CHANGE
```

Un Change que necesita la G4 humana nunca se promueve sin ella
(`This change needs a human G4 before promotion.`). Una promoción que falla deja producción
intacta y el Change esperando en G4; el mismo OK la reintenta.

Si `observe` falla, se ejecuta el `rollback` y el Change pide un Hotfix. Con un `observe`
que imprimió `error rate up` y salió con 1:

```text
error: release.observe failed (exit code 1):
error rate up
--- end of output ---
error rate up
Rollback: ok
Production failed while observing: rolled back to the previous version. A hotfix is needed. Message in .gdd/changes/receipt/inbox.md
```

El Change queda en `rolled-back`, y su Decision Inbox dice lo que tienes que decidir:

```text
## Needs your decision
- Production failed while observing: rolled back to the previous version. A hotfix is needed.
```

Todas las claves están en [Configuración](../reference/configuration.md#commandstoml).

## 6. Comprueba el changelog en CI (opcional)

`gdd changelog --check` no escribe nada y falla cuando al fichero le falta alguna línea:

```text
error: CHANGELOG.md misses 1 line(s): gdd changelog shows them, gdd changelog --write adds them
```

Cuando no falta nada, imprime `CHANGELOG.md covers every change on main.`. Si mantienes el
fichero a mano, añádelo como paso de tu workflow en main: consulta
[Ejecutar las puertas en CI](ci.md).

## Siguientes pasos

- [Revisar un Change en G4](release-gate.md): OK, KO o polish en G4, antes de la promoción.
- [Ejecutar las puertas en CI](ci.md): qué bloquea un Change antes de que se fusione, en
  integración continua (CI).
- [Medir el trabajo](measure.md): frecuencia de despliegue y tasa de fallos de cambios
  (*change fail rate*, la proporción de despliegues que fallan) con `--dora`.
