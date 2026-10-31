# gdd-change

La skill de Change lleva un Change desde tu frase hasta la puerta de salida (G4, *Release Gate*).
El Orchestrator crea el Change, escribe sus escenarios y su plan cuando lo necesita, te pide el OK
una sola vez, entrega el Change a la receta de gdd y vuelve cuando tienes que responder otra vez.
Ejecútala cada vez que quieras construir, arreglar o reorganizar algo.

## Antes de empezar

- G0 está abierta: [`gdd-setup`](setup.md) ha terminado.
- Tu agente trabaja en la carpeta del proyecto, en `main`.

## Llámala

Di qué quieres y por qué, con tus propias palabras. En Claude Code y OpenCode:

```text
/gdd-change export the notes as CSV, so I can open them in a spreadsheet
```

En Codex:

```text
$gdd-change export the notes as CSV, so I can open them in a spreadsheet
```

Si ya tienes un id en mente, empieza por él: `/gdd-change export-csv export the notes as CSV`.
Los ids son letras minúsculas, dígitos y guiones. Di qué y por qué, no cómo: el código es trabajo
de los agentes.

## Qué te pregunta

Todo lo que necesita de ti antes del build llega en **un solo mensaje**, la sentada:

- **Los escenarios**, como texto plano, cuando el Change crea o cambia un comportamiento visible.
  No ves los ejemplos ocultos, los casos extra que no recibe el Builder: están ahí para atrapar
  el código que solo encaja con los visibles.
- **Las zonas rojas**, cuando [`gdd status`](../cli/status.md) dice que están pendientes o han
  cambiado, o cuando este Change crea un área sensible nueva: una lista corta, cada zona con su
  motivo.
- **El plan**, cuando el Change es rojo: qué, cómo, los riesgos y cómo revertirlo, en palabras.

Respondes OK, o dices qué cambiar. Más tarde vuelve una vez más, cuando el Change espera en G4, y
puede dejarte preguntas en la Decision Inbox del Change, cada una con su recomendación.

## Qué hace

1. **Comprueba el proyecto** con [`gdd status`](../cli/status.md): G0 tiene que estar abierta.
2. **Crea el Change** con [`gdd new-change`](../cli/new-change.md): el id, un título, el tipo
   (Feature, Hotfix, Refactor, Migration, Spike o No-code) y las rutas que espera tocar. gdd
   responde con la clasificación: el color, la vía y las puertas. Para un Spike con 2 a 4
   opciones pide una variante por opción, y para un Spike sin nada que distinguir, varias
   variantes del mismo encargo.
3. **Escribe el plan** de un Change rojo en `.gdd/changes/<id>/plan.md`, nunca código.
4. **Escribe los escenarios** en la rama del Change, `change/<id>`: ficheros `.feature` en
   `features/`, con las palabras del dominio, pocos escenarios con ejemplos concretos, más uno o
   dos ejemplos ocultos en `features/hidden/`. Después vuelve a `main`.
5. **Celebra la sentada** contigo, como se ha dicho arriba. Con tu OK registra cada parte:
   - los escenarios: un commit en la rama y después
     [`gdd gate G1 ok <id> --by human`](../cli/gate.md), que los sella;
   - las zonas rojas: un commit de `gdd/zones.toml` en `main` y después `gdd gate zones ok`;
   - el plan: `gdd gate plan ok <id>`.
6. **Escribe el paquete de contexto** en `.gdd/changes/<id>/package.md`, el único contexto que
   recibe el Builder: el objetivo en tres líneas, el color, los escenarios que cubre, las rutas
   que puede tocar, qué reutilizar, las reglas de su zona y el comando de G2.
7. **Deja notas** en la Decision Inbox con [`gdd note`](../cli/note.md): `decided` para lo que ha
   elegido sin ti, `needs` para lo que necesita tu decisión.
8. **Hace commit de lo pendiente en `main` y lanza** el Change con
   [`gdd launch <id>`](../cli/launch.md). La receta lo construye (G2), lo integra (G3), hace que
   el Reviewer lo lea, lo fusiona y lo despliega en pre. Eso tarda minutos.
9. **Te trae G4**, cuando el Change se para ahí: te enseña la Decision Inbox
   (`.gdd/changes/<id>/inbox.md`) y la demo. Tu respuesta se convierte en
   [`gdd resume <id> --g4 ok`](../cli/resume.md), `--g4 ko`, o `--g4 polish --note` con tus
   palabras, que ejecuta una ronda más del Builder y vuelve a G4.
10. **Propone el siguiente paso en una línea** cuando el Change se ha aparcado, después de leer su
    bandeja.
11. **Cuenta el trabajo** con [`gdd session --phase 1-discover --change <id>`](../cli/session.md),
    porque esta conversación (clasificación, escenarios, sentada) es la fase Discover del Change,
    y muestra lo que ha costado el Change con [`gdd report --change <id>`](../cli/report.md).

## Qué decides tú

| Cuándo | Decides | Se convierte en |
|---|---|---|
| La sentada | los escenarios: OK, o qué cambiar | G1, la puerta de escenarios |
| La sentada | las zonas rojas, cuando esperan | la aprobación de las zonas rojas |
| La sentada | el plan de un Change rojo | la aprobación del plan |
| Por el camino | las preguntas que deja bajo *Needs your decision* | su siguiente paso |
| G4 | OK, KO o polish | promoción, rechazo o una ronda más |

¿Prefieres una pantalla para la sentada o para G4? Ejecuta
[`gdd sitting <id>`](../cli/sitting.md) en una terminal: registra las mismas decisiones.
[Pantallas](../screens.md#la-sentada) la muestra.

## Qué deja

| Qué | Dónde |
|---|---|
| El registro del Change: tipo, rutas, color, puertas, bandeja | `.gdd/changes/<id>/change.json` |
| La rama, con los escenarios y su commit | `change/<id>` |
| El plan de un Change rojo | `.gdd/changes/<id>/plan.md` |
| El paquete de contexto del Builder | `.gdd/changes/<id>/package.md` |
| La Decision Inbox | `.gdd/changes/<id>/inbox.md` |
| Cada puerta y cada llamada de agente | `.gdd/log.jsonl` |
| Las zonas rojas aprobadas, cuando esperaban | `gdd/zones.toml` en `main`, `.gdd/zones.json` |

Después del lanzamiento, el Change tiene su propio worktree junto al proyecto
(`../PROJECT.worktrees/<id>`) mientras se ejecuta. Una vez fusionado, `main` tiene un commit de
fusión que lleva el registro del Change. Cuando se cierra tras tu OK, el worktree desaparece y la
rama se queda; con `[versions] mode = "auto"` también puede recibir un tag de versión.

## Qué viene después

- ¿El Change espera en G4 y el chat ha terminado? Ejecuta [`gdd-status`](status.md): encuentra el
  Change que te espera.
- ¿El Change se ha aparcado? Consulta
  [Volver a poner en marcha un Change aparcado](../../how-to/parked-change.md).
- El siguiente Change: ejecuta `gdd-change` otra vez. Cada pocos Changes, [`gdd-learn`](learn.md).

## Ver también

- [Describe un Change a tu agente](../../how-to/run-a-change.md): esta skill en uso, con lo que
  ves en cada paso.
- [Aprobar los escenarios](../../how-to/write-scenarios.md): qué mirar en la sentada.
- [Responder a la puerta de salida](../../how-to/release-gate.md): qué comprobar antes de decir
  OK.
- [La vida de un Change](../../explanation/life-of-a-change.md): qué hace la receta después del
  lanzamiento.
