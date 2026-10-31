# gdd guard

<!--
Generada por scripts/make_reference.py a partir de gdd 0.1.0. Edita el texto de ayuda en el
código o scripts/cli_notes/guard.md, nunca esta página.
-->

Path Guard: what a change touched since its build started that a role must not touch, as JSON.

## Uso

```text
Usage: gdd guard [OPTIONS] [CHANGE]
```

## Argumentos

| Argumento | Descripción |
|---|---|
| `[CHANGE]` | Id of the change (its record is .gdd/changes/<id>/change.json); else GDD_CHANGE. |

## Opciones

| Opción | Descripción |
|---|---|
| `--role <ROLE>` | Role whose protected paths count (builder-build, builder-test, reviewer...). Por defecto: `builder`. |
| `--strict` | Exit with code 1 when Path Guard fails. |
| `--since-snapshot` | Count only what changed since the last `gdd snapshot`. |

## Quién lo ejecuta

Your agent, as the Builder, in the [`gdd-build-test`](../skills/agent-skills.md) and
[`gdd-build`](../skills/agent-skills.md) skills, before G2. G2 and G3 also run Path Guard.

## Ejemplos

The Builder in test mode may not touch production code:

```bash
gdd guard cart-total --role builder-test
```

<!-- REAL OUTPUT: gdd 0.1.0, sandbox project shop (python; claude and codex). -->

```text
{"ok":false,"failures":["src/shop/cart.py: protected path for builder-test"]}
```

With `--since-snapshot`, it judges only what changed since the last
[`gdd snapshot`](snapshot.md):

```bash
gdd guard cart-total --role builder-test --since-snapshot
```

```text
{"ok":true,"failures":[]}
```

## Ver también

- [Referencia de la CLI](index.md): todos los comandos, por área, y lo que comparten.
- [Roles](../roles.md): what each role may touch.
- [Safety](../../explanation/safety.md): why Path Guard exists.
