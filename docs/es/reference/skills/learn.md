# gdd-learn

La skill de Learn es la fase 5 de GDD. El Orchestrator lee el log y los Changes cerrados y
aparcados, encuentra los errores que se repiten y propone una regla o una comprobación para cada
uno. Learn no tiene puerta: solo endurece las que ya tienes.

## Llámala

En Claude Code y OpenCode:

```text
/gdd-learn
```

En Codex:

```text
$gdd-learn
```

La entrada es opcional. Úsala para señalar algo que hayas notado:

```text
/gdd-learn changes keep parking on lint at G2
```

## Qué te pregunta

Nada hasta el final, cuando te trae sus propuestas en unas pocas líneas, con su recomendación.

## Qué hace

1. **Ejecuta primero [`gdd learn`](../cli/learn.md)**: ejecuta las herramientas de Learn de
   `gdd/commands.toml` y enumera las notas de polish que diste en G4 y las clases de hallazgo del
   Reviewer que se repiten.
2. **Lee los números y la historia**: [`gdd report`](../cli/report.md),
   `gdd report --by phase,role`, y los Changes cerrados y aparcados de `.gdd/changes/`.
3. **Busca errores que se repiten**: el mismo fallo de G2 en varios Changes, el mismo motivo de
   un KO del Reviewer, escenarios mal escritos, muchos reintentos con un modelo. Tus notas de
   polish en G4 y las mejoras del Reviewer también cuentan, cuando se repiten.
4. **Propone un arreglo para cada uno**: una regla, un lint, un hook o un test que lo atrape la
   próxima vez. Una nota de polish repetida se convierte en una línea de `docs/standards.md` o,
   cuando una máquina puede comprobarla, en una puerta. Nunca afloja una puerta.
5. **Busca código y dependencias sin usar** con la herramienta del stack (knip, vulture o
   cargo-machete) y propone quitarlos.
6. **Propone un refactor** solo cuando el log lo justifica.
7. **Escribe la propuesta** en `docs/learn/YYYY-MM-DD.md` y te la trae. Tocar `gdd/` es
   trabajo rojo, así que nunca lo hace por su cuenta.
8. **Cuenta la conversación** con [`gdd session --phase 5-learn`](../cli/session.md).

## Qué decides tú

Qué propuestas aceptar. Nada en `gdd/` cambia sin tu OK.

## Qué deja

| Qué | Dónde |
|---|---|
| Las propuestas | `docs/learn/YYYY-MM-DD.md` |
| Lo que dijo cada herramienta de Learn | `.gdd/learn/`, escrito por `gdd learn` |
| La conversación, contada | `.gdd/log.jsonl` |

## Cuándo ejecutarla

De vez en cuando: después de varios Changes, después de un Change que se ha aparcado, o cuando
los números de [`gdd-report`](report.md) no cuadran.

## Ver también

- [Pocas paradas para la persona](../../explanation/human-stops.md#learn-convierte-las-notas-repetidas-en-reglas):
  por qué las notas repetidas se convierten en reglas.
- [De los números a Learn](../../explanation/measuring.md#de-los-números-a-learn): qué números
  apuntan a qué.
- [Puertas](../gates.md#learn): qué puede y qué no puede cambiar Learn.
