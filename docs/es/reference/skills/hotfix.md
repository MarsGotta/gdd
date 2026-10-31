# gdd-hotfix

La skill de Hotfix es para algo roto en producción. El Orchestrator revierte producción primero y
después arregla la causa como un Change normal de tipo Hotfix, con un escenario que reproduce el
fallo. Ejecútala cuando producción está rota.

## Llámala

Di qué está roto. En Claude Code y OpenCode:

```text
/gdd-hotfix the CSV export writes an empty file since the last release
```

En Codex:

```text
$gdd-hotfix the CSV export writes an empty file since the last release
```

## Qué te pregunta

- Nada antes de la reversión: revierte y te lo dice en una línea.
- Tu OK para cualquier atajo, cuando revertir es imposible.
- El OK de siempre a los escenarios del arreglo, en su sentada.

## Qué hace

1. **Primero mitiga.** gdd revierte un Change por sí mismo cuando producción empeora mientras la
   observa; entonces el Change termina en `rolled-back` y su Decision Inbox pide un Hotfix. Cuando
   esa reversión automática no ha ocurrido, el Orchestrator ejecuta el comando `rollback` de
   `[release]` en `gdd/commands.toml` y te lo dice en una línea.
2. **Lo arregla por su vía.** Con producción estable no hay prisa: crea el arreglo como un Change
   de tipo Hotfix, con [`gdd-change`](change.md), con un escenario de regresión en
   `features/regression/` que reproduce el fallo. El arreglo pasa las puertas de su color, como
   cualquier Change.
3. **Pregunta antes de un atajo.** Cuando revertir es imposible, explica en tres líneas qué pasa y
   qué propone. Un atajo sin reversión necesita tu OK.
4. **Deja una lección**: una nota corta en `docs/learn/` con la causa y la regla nueva que lo
   habría evitado.

## Qué decides tú

- Si tomar un atajo, cuando no hay reversión.
- Los escenarios del arreglo, en su sentada, y su G4, como en cualquier Change.

## Qué deja

| Qué | Dónde |
|---|---|
| Producción en la versión anterior | después de la reversión |
| El arreglo, como un Change de tipo Hotfix | `.gdd/changes/<id>/`, rama `change/<id>` |
| El escenario que reproduce el fallo | `features/regression/` |
| La causa y la regla nueva | `docs/learn/` |

## Ver también

- [Responder a la puerta de salida](../../how-to/release-gate.md#ok-promuévelo): qué hace una
  observación fallida.
- [`gdd release`](../cli/release.md): los pasos de release, observe incluido.
- [`gdd-learn`](learn.md): adónde va después la lección.
