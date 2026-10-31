# gdd-report

La skill de Report te dice cuánto han costado las llamadas de agente, en tokens y en dinero,
agrupado como pidas, y lo resume en unas pocas líneas. Ejecútala cuando quieras saber en qué se
va el gasto, por ejemplo antes de pasar un rol a un modelo más barato.

## Llámala

En Claude Code y OpenCode:

```text
/gdd-report
```

En Codex:

```text
$gdd-report
```

Para agrupar de otra forma, di cómo, con cualquiera de `change`, `phase`, `model`, `role` y
`agent`:

```text
/gdd-report by phase and model
```

## Qué te pregunta

Nada.

## Qué hace

1. Cuenta esta conversación con [`gdd session --phase <phase>`](../cli/session.md), para la fase
   en la que está el proyecto. Añade al log las peticiones de tus sesiones interactivas de Claude
   Code.
2. Ejecuta [`gdd report --by change,role,model`](../cli/report.md), o la agrupación que hayas
   pedido.
3. Lo resume en unas pocas líneas: el total, el rol y el modelo que más gastan, los reintentos y
   las paradas humanas.

Un agente que no informa de un precio muestra un coste de 0. La skill lo dice, y te recuerda que
con una suscripción el coste es teórico: lo que sirve para comparar son los tokens. Rellena
`[prices]` en `gdd/models.toml` para tener una estimación; consulta
[Elegir agentes y modelos](../../how-to/agents-and-models.md#6-rellena-los-precios-para-las-estimaciones).

## Qué decides tú

Nada. Actúa sobre los números con [`gdd-learn`](learn.md) o cambiando modelos en
`gdd/models.toml`.

## Qué deja

Las líneas que `gdd session` añade a `.gdd/log.jsonl`, una por modelo de tu conversación.

## Ver también

- [Mide el trabajo](../../how-to/measure.md): los mismos números, además del tiempo y las
  métricas de entrega de DORA (*DevOps Research and Assessment*).
- [`gdd report`](../cli/report.md): cada agrupación y cada flag.
