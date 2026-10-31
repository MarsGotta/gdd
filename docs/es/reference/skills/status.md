# gdd-status

La skill de Status te dice en qué punto de GDD está el proyecto y te da un siguiente paso, con la
recomendación del Orchestrator. No cambia nada. Ejecútala cuando vuelves a un proyecto, o
siempre que no sepas qué viene después.

## Llámala

En Claude Code y OpenCode:

```text
/gdd-status
```

En Codex:

```text
$gdd-status
```

Puedes añadir una pregunta, y la responde a partir del mismo estado:

```text
/gdd-status is anything waiting for me?
```

## Qué te pregunta

Nada. Responde en unas pocas líneas.

## Qué hace

1. Lee `AGENTS.md`, las instrucciones del proyecto para los agentes, si no lo ha leído en esta
   sesión: todo proyecto que ha preparado `gdd init` tiene uno. Después ejecuta
   [`gdd status`](../cli/status.md): G0, las zonas rojas, el autotest, los escenarios, los
   Changes y el log.
2. Elige un único siguiente paso, en este orden:

   | Cuándo | Propone |
   |---|---|
   | G0 está pendiente | [`gdd-setup`](setup.md): el proyecto está en la fase 0 |
   | G0 está abierta y ningún registro de Discover cubre el trabajo siguiente | [`gdd-discover`](discover.md) |
   | Las zonas rojas están pendientes o han cambiado | aprobarlas junto con los escenarios del siguiente Change, en [`gdd-change`](change.md) |
   | Un Change espera en G4 | tu respuesta: OK, KO o polish |
   | Un Change se ha aparcado | lo que dice su Decision Inbox, `.gdd/changes/<id>/inbox.md` |
   | No hay ningún Change en marcha | el siguiente Change, con [`gdd-change`](change.md) |

3. Responde: dónde estás y un siguiente paso.

## Qué decides tú

Si das el paso que propone.

## Qué deja

Nada: `gdd status` solo lee.

## Qué viene después

La skill que ha propuesto. También es la forma de volver después de una pausa: arranca otra vez
tu agente, ejecuta `gdd-status` y encuentra un Change que espera tu G4 o uno aparcado.

## Ver también

- [`gdd status`](../cli/status.md): el comando, y cada línea que imprime.
- [El Board](../screens.md#el-board): los mismos Changes por fase, en una pantalla.
