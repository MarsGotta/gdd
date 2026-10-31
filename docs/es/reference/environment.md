# Variables de entorno

Las variables de entorno que lee gdd, las que añade para los comandos y los agentes que lanza,
y las que quita del entorno de los agentes porque guardan secretos.

## Qué lee gdd

| Variable | La lee | Significado |
|---|---|---|
| `GDD_CHANGE` | `gdd classify`, `snapshot`, `inbox`, `prepare`, `merge`, `release`, `g2`, `g3`, `guard` | El id del Change cuando la línea de comandos no da ninguno. Si está vacía, cuenta como no definida. |
| `GDD_ROLE` | todos los comandos | El rol de un agente que gdd ha lanzado. Con cualquier valor, gdd solo ejecuta `gdd g2`, `guard`, `note`, `status`, `classify`, `inbox`, `report` y `scope` (además de la ayuda y la versión); cualquier otro comando falla antes de hacer nada (consulta [Roles](roles.md#qué-puede-ejecutar-un-agente)), y no se abre ninguna pantalla. `gdd g2` no registra su ejecución. Sin definir: una persona, que ejecuta todos los comandos. |
| `GITHUB_HEAD_REF` | `gdd ci` | La rama que comprobar, tal como la fija GitHub Actions en una pull request; sin ella, la rama que hay en el checkout. |
| `HOME` | `gdd session` | Dónde están las transcripciones de Claude Code: `$HOME/.claude/projects/<project root>`, con cada carácter de la raíz que no esté en `A-Za-z0-9` convertido en `-`. |
| `PATH` | `gdd init`, `gdd launch`, `gdd resume` | Dónde están los programas de los agentes (`claude`, `codex`, `opencode`): la primera carpeta absoluta que contiene el programa, ejecutable. `gdd init` sin `--agents` configura los agentes que encuentra ahí. |

gdd no lee `GDD_ROOT` ni `GDD_STATE`: encuentra la raíz del proyecto buscando
`gdd/project.toml` desde la carpeta en la que se ejecuta hacia arriba, y la raíz del estado
preguntando a git por el checkout principal.

Las demás variables pasan tal cual: los comandos de las puertas, los pasos de release,
`gdd bootstrap` y git se ejecutan en el mismo entorno que gdd, así que las variables propias de
git (`GIT_AUTHOR_NAME` y las demás) dicen quién hace los commits.

```bash
GDD_CHANGE=cart-total gdd g2 --role builder-build
GITHUB_HEAD_REF=change/cart-total gdd ci
```

## Qué reciben los comandos de las puertas

Cada comando de `[g2]` y `[g3]` se ejecuta con el entorno de gdd más cuatro variables. Nombran
el Change que comprueba el comando. Las listas tienen una ruta por línea, relativa a la raíz
del proyecto.

| Variable | Valor |
|---|---|
| `GDD_CHANGE` | El id del Change. |
| `GDD_COLOR` | Su color: `green`, `yellow` o `red`. |
| `GDD_DIFF_FILES` | Lo que el Change ha tocado respecto a la rama principal; sus rutas planificadas mientras no ha tocado nada. Los comandos de mutación prueban solo estas. |
| `GDD_FEATURES` | Los ficheros de escenarios del Change: los ficheros `.feature` que necesitan G1 y que planifica o ha tocado, y los que su registro selló en G1, ejemplos ocultos incluidos. Vacía cuando no tiene ninguno. |

`gdd ci` da las mismas cuatro a sus comandos de puerta. Otras ejecuciones las fijan de otra
forma:

| Ejecución | Variables |
|---|---|
| `gdd adopt`, `gdd learn` | `GDD_CHANGE`, `GDD_COLOR`, `GDD_DIFF_FILES` y `GDD_FEATURES`, todas vacías. |
| `gdd selftest` | `GDD_CHANGE=selftest`. |
| `gdd bootstrap` | No añade ninguna. |

## Qué reciben los pasos de release

Cada paso de `[release]` se ejecuta con el entorno de gdd más:

| Variable | Valor |
|---|---|
| `GDD_CHANGE` | El id del Change. |
| `GDD_VERSION` | Una vez que el Change tiene versión (`[versions] mode = "auto"`, una línea de changelog, en `promote`): la versión sin la `v`, por ejemplo `0.1.0`. Sin definir hasta entonces. |

```toml
promote = "./scripts/promote.sh \"$GDD_VERSION\""
```

## Variables que leen los ejecutores de escenarios

Los ejecutores de escenarios de los [esqueletos de
stack](stacks.md#escenarios-y-ejemplos-ocultos) leen dos variables; gdd fija la primera, y los
comandos de G3 fijan las dos.

| Variable | Significado |
|---|---|
| `GDD_FEATURES` | Los ficheros `.feature` que ejecutar, uno por línea; todos si está vacía. |
| `GDD_HIDDEN` | `1`: ejecuta también `features/hidden/`. Solo en Python y Rust; TypeScript, en cambio, deja fuera en G2 los escenarios con la etiqueta `@hidden`. |

```toml
acceptance_full = "GDD_FEATURES= GDD_HIDDEN=1 uv run pytest -q tests/acceptance || [ $? -eq 5 ]"
```

## Qué reciben los agentes

gdd lanza el propio programa de cada agente, nunca a través de un shell,
desde el worktree del Change, sin nada en su entrada estándar y descartando su salida de error.
Su entorno es el de gdd, primero vaciado y luego reconstruido:

1. Todas las variables del entorno de gdd, salvo las que [guardan un
   secreto](#qué-variables-guardan-un-secreto), por su nombre o por su valor.
2. Las variables quitadas que `[env] pass` de `gdd/models.toml` nombra exactamente.
3. Cuatro variables propias:

| Variable | Valor |
|---|---|
| `GDD_CHANGE` | El id del Change. |
| `GDD_ROOT` | La raíz del proyecto: el worktree del Change, donde se ejecuta el agente. |
| `GDD_STATE` | La raíz del estado: el checkout principal, cuyo `.gdd/` guarda los registros del Change. Los comandos de los agentes les indican que lean el estado allí. |
| `GDD_ROLE` | `builder-test`, `builder-build`, `reviewer` u `orchestrator`: limita los comandos de gdd que puede ejecutar el agente (consulta [Roles](roles.md#qué-puede-ejecutar-un-agente)). |

### Qué variables guardan un secreto

Una variable guarda un secreto, y se quita, en dos casos.

El primer caso es el nombre. gdd divide el nombre en palabras por `_`, `-` y `.`, y las lee sin
distinguir mayúsculas de minúsculas. Cuando una de las palabras es `KEY`, `KEYS`, `TOKEN`,
`TOKENS`, `SECRET`, `SECRETS`, `PASSWORD`, `PASSWORDS`, `PASSWD`, `PASS`, `CREDENTIAL`,
`CREDENTIALS`, `APIKEY` o `DSN`, la variable se quita. Solo cuentan las palabras completas:

| Nombre | Sus palabras | Llega al agente |
|---|---|---|
| `OPENAI_API_KEY`, `XAPI_KEY`, `API_KEY_FILE` | `KEY` | no |
| `TOKEN`, `github_token`, `MY_TOKENS`, `npm-tokens` | `TOKEN`, `TOKENS` | no |
| `AWS_SECRET_ACCESS_KEY`, `client.secret` | `SECRET`, `KEY` | no |
| `Db_Password`, `MYSQL_PASSWD`, `DB_PASS`, `PASS_FILE` | `PASSWORD`, `PASSWD`, `PASS` | no |
| `GOOGLE_APPLICATION_CREDENTIALS`, `MAPS_APIKEY`, `SENTRY_DSN` | `CREDENTIALS`, `APIKEY`, `DSN` | no |
| `KEYBOARD_LAYOUT`, `TOKENIZER_PATH`, `PASSAGE_DIR`, `MONKEY`, `APITOKEN` | ninguna de esas | sí |
| `PATH`, `HOME`, `PWD`, `EDITOR` | ninguna de esas | sí |

El segundo caso es el valor. Un valor que es una URL con una contraseña en la parte del usuario,
`scheme://user:password@host` o `scheme://:password@host`, guarda un secreto, se llame como se
llame la variable:

| Valor | Llega al agente |
|---|---|
| `postgres://app:***@db.example/app`, `redis://:PASSWORD@cache:6379` | no |
| `https://example.com/docs`, `http://localhost:8080`, `https://me@host` (sin contraseña) | sí |

Así, `DATABASE_URL=postgres://app:***@db.example/app` nunca llega a un agente, mientras que
`DOCS_URL=https://example.com/docs` sí.

### Pasa una variable a propósito

`[env] pass` en `gdd/models.toml` lista, por su nombre exacto, las variables que reciben los
agentes aunque guarden un secreto:

```toml
[env]
pass = ["MM_API_KEY", "DATABASE_URL"]
```

> [!WARNING]
> Un secreto que no delatan ni su nombre ni su valor (una clave en bruto en `STRIPE_LIVE`, o
> en `APITOKEN`, que es una sola palabra) llega a los agentes. Mantén las credenciales de
> producción fuera del entorno desde el que lanzas los Changes.

Los agentes usan sus propios inicios de sesión (suscripciones), no claves que gdd les pase. Un
comando `gdd` que ejecuta un agente hereda el entorno del agente, así que sus comandos de
puerta tampoco ven las variables quitadas.

## Ver también

- [Roles](roles.md#qué-puede-ejecutar-un-agente): qué puede ejecutar un agente con `GDD_ROLE`.
- [models.toml](configuration.md#modelstoml): `[env] pass` y el resto de ajustes de los
  agentes.
- [Mantener honestos a los agentes](../explanation/safety.md): por qué los secretos se
  mantienen lejos de los agentes.
- [Ejecuta las puertas en CI](../how-to/ci.md): `gdd ci` y `GITHUB_HEAD_REF`.
