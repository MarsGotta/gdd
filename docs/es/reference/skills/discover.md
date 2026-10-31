# gdd-discover

La skill de Discover es la fase 1 de GDD, una conversación contigo. Partiendo del Brief del
proyecto, el Orchestrator explora un tema, te trae opciones para decidir, lanza un Spike cuando
una pregunta bloquea una decisión y propone los próximos Changes en orden. No escribe código del
producto.

Ejecútala después de Setup cuando el producto necesita primero una arquitectura o un mapa de sus
áreas sensibles, y siempre que el siguiente paso sea más grande que un Change pequeño.

## Antes de empezar

- G0 está abierta: [`gdd-setup`](setup.md) ha terminado y [`gdd status`](../cli/status.md) dice
  `G0: ok`.

## Llámala

Con un tema, en Claude Code y OpenCode:

```text
/gdd-discover how the notes are stored, and whether they sync between machines
```

En Codex:

```text
$gdd-discover how the notes are stored, and whether they sync between machines
```

Sin tema, `/gdd-discover` toma la siguiente pieza de valor del producto.

## Qué te pregunta

No repite las preguntas de Setup: parte de `docs/brief.md`. Para cada pregunta importante del
tema (diseño, datos, privacidad, riesgos, mercado) trae dos o tres opciones, cada una con sus
pros y sus contras, y su recomendación. Busca en la web cuando la pregunta lo necesita. Cuando el
Brief aún no tiene producto mínimo viable (MVP) ni regla de abandono, propone el MVP más pequeño que aporte valor y qué
tendría que pasar para abandonarlo.

Pocas preguntas, agrupadas, con la recomendación delante. Cada mensaje termina con un siguiente
paso.

## Qué hace

1. **Lee el Brief** y trabaja a partir de tus respuestas anteriores.
2. **Explora** el tema contigo, opción por opción.
3. **Lanza un Spike solo cuando una pregunta bloquea una decisión.** Un Spike es un Change de
   tipo Spike, con un presupuesto fijo, que responde a una pregunta y deja un registro de
   decisión de arquitectura (ADR). Lo crea con
   [`gdd new-change --type Spike`](../cli/new-change.md). Cuando la pregunta tiene de 2 a 4
   opciones, pide una variante por opción (`--option SQLite --option Postgres`); si no hay
   opciones que distinguir, varias variantes del mismo encargo (`--variants 2`). Comparas las
   variantes en la puerta de salida (G4, *Release Gate*) y eliges una. Consulta
   [Explorar con un Spike](../../how-to/spikes.md).
4. **Registra las decisiones costosas** como ADR en `docs/adr/NNNN-title.md`: el contexto, la
   decisión, las alternativas y las consecuencias.
5. **Propone los próximos Changes**, en orden, cada uno pequeño y con valor.
6. **Mapea las zonas rojas, al final.** Solo ahora hay carpetas reales que mapear: convierte las
   áreas sensibles del Brief en rutas de `gdd/zones.toml`, rojas, y verdes donde es obvio. Entre dos
   colores elige el más alto. Todavía no te pide la aprobación: apruebas las zonas rojas en la
   sentada del primer Change.
7. **Guarda la conversación** en `docs/discover/YYYY-MM-DD-<topic>.md`, y actualiza
   `docs/brief.md` cuando algo ha cambiado.
8. **Cuenta la conversación** con [`gdd session --phase 1-discover`](../cli/session.md).

## Qué decides tú

- La opción de cada pregunta, o que antes necesita un Spike.
- El MVP y la regla de abandono, cuando el Brief no los tiene.
- El orden de los próximos Changes.
- En la G4 de un Spike con variantes, qué variante conserva su ADR.

Las zonas rojas que mapea no se deciden aquí: esperan tu aprobación en la siguiente sentada.

## Qué deja

| Qué | Dónde |
|---|---|
| El registro de la conversación | `docs/discover/YYYY-MM-DD-<topic>.md` |
| Las decisiones que cuesta cambiar | `docs/adr/NNNN-title.md` |
| Un Brief actualizado, cuando algo ha cambiado | `docs/brief.md` |
| Las zonas rojas, todavía sin aprobar | `gdd/zones.toml` |
| Un Spike, cuando ha hecho falta | `.gdd/changes/<id>/` y su rama `change/<id>` |

[`gdd status`](../cli/status.md) muestra las zonas rojas como pendientes hasta que las apruebas.

## Qué viene después

La skill termina proponiendo [`gdd-change`](change.md) para el primer Change del orden que ha
propuesto. Su sentada muestra las zonas rojas junto con los escenarios, para un solo OK.

## Ver también

- [Mapear y aprobar zonas rojas](../../how-to/red-zones.md): cómo recibe su color una ruta.
- [Explorar con un Spike](../../how-to/spikes.md): presupuestos, variantes y el ADR.
- [GDD en una página](../../explanation/method.md#fases-y-puertas): dónde está Discover entre las
  fases.
