# Medir el trabajo

Averigua cuánto cuestan tus Changes en tokens, en qué se les va el tiempo, cómo va la entrega y
qué conviene reforzar después, a partir del registro que lleva gdd. Usa esta guía cuando se
hayan ejecutado unos cuantos Changes, y otra vez en cada Learn, la fase en la que conviertes los
errores repetidos en comprobaciones.

## Antes de empezar

- Un proyecto GDD con algunos Changes ya ejecutados: cada llamada de agente, cada puerta y cada
  decisión humana añade una línea a `.gdd/log.jsonl`, y cada Change guarda su expediente en
  `.gdd/changes/<id>/`.
- Lee [Medir lo que importa](../explanation/measuring.md) para saber por qué se miden tokens y
  no líneas de código, y por qué las señales de GDD son indicios que leer, nunca objetivos.

Tu agente puede sacar estos informes por ti: `/gdd-report` (`$gdd-report` en Codex) resume los
tokens, y `/gdd-learn` ejecuta Learn y propone qué reforzar. Los informes solo leen el registro,
así que también puedes ejecutarlos tú en un terminal. Los pasos de abajo muestran lo que imprime
cada uno y cómo leerlo.

Los ejemplos de esta página son salida real del repositorio de gdd, que se construye con GDD:
50 Changes en una semana, más o menos.

## 1. Cuenta los tokens

```bash
gdd report
```

Deberías ver una línea por rol y luego el total. Los tokens son las unidades de texto que un
modelo lee y escribe:

```text
role -> calls · tokens in · cache · out · cost USD · seconds
builder-build -> 178 · 23,521,360 · 937,297,814 · 8,591,698 · 526.6018 · 85112
builder-test -> 63 · 2,708,102 · 28,256,111 · 328,557 · 16.9410 · 6604
orchestrator -> 10 · 535,311 · 7,194,620 · 123,555 · 8.1913 · 1336
reviewer -> 124 · 8,677,291 · 51,913,984 · 345,867 · 0.0000 · 12263
TOTAL -> in 35,442,064 · cache 1,024,662,529 · out 9,389,677 · 551.7341 USD
Human stops logged: 201
```

Lee las columnas en este orden:

1. **tokens in** y **out**: los tokens de entrada nuevos y los tokens de salida. Son los que
   sirven para comparar Changes, modelos y roles.
2. **cache**: los tokens de entrada leídos de la caché de prompts del agente, texto que el
   modelo ya vio. Son muchos (aquí, unos 29 por cada token de entrada nuevo) y mucho más baratos, así que no los sumes a los nuevos.
3. **cost USD**: una estimación a precios de API, nunca una factura. Claude Code da su propia
   estimación, calculada en el cliente, y OpenCode da un coste. Codex no da ninguno, por eso el
   Reviewer de arriba muestra 0 hasta que rellenes `[prices]` (consulta [Elegir agentes y
   modelos](agents-and-models.md#6-rellena-los-precios-para-las-estimaciones)). Con una
   suscripción, el coste es teórico: compara tokens.

Agrupa y filtra:

```bash
gdd report --by agent,model
gdd report --by role --change CHANGE
gdd report --by model --json
```

`--by` admite cualquiera de los campos del registro, separados por comas (`change`, `phase`,
`model`, `role`, `agent`...). Por defecto es `role`. Con `--by agent,model`:

```text
agent · model -> calls · tokens in · cache · out · cost USD · seconds
claude · opus -> 140 · 21,382,030 · 896,621,430 · 8,056,377 · 512.7214 · 78893
claude · sonnet -> 111 · 5,382,743 · 76,127,115 · 987,433 · 39.0127 · 14160
codex · gpt-6-astra -> 124 · 8,677,291 · 51,913,984 · 345,867 · 0.0000 · 12263
TOTAL -> in 35,442,064 · cache 1,024,662,529 · out 9,389,677 · 551.7341 USD
Human stops logged: 201
```

`--json` imprime los mismos grupos como una lista, con `tokens_in`, `tokens_cache`,
`tokens_out`, `cost_usd` y `duration_s`.

Las conversaciones con tu agente fuera de la receta (el Setup, Discover) no están en el
registro hasta que las añades. Esto funciona con Claude Code, y `/gdd-report` lo hace antes de
su informe. A mano:

```bash
gdd session --phase 1-discover
```

## 2. Mira en qué se va el tiempo

```bash
gdd report --time --change ratchet
```

```text
ratchet · A ratchet for known debt: a change cannot add to it, and what it pays stays paid · closed
  Flow time 1h 18m · active 1h 07m (86%) · waiting 11m · between steps 0s
  Agents: builder-build · opus 35m, reviewer · gpt-6-astra 5m, builder-test · sonnet 1m
  Gates: mutation 15m, acceptance_full 8m, unit 13s, security 4s, acceptance 3s, dependencies 2s, secrets 1s, lint 1s, lockfile 0s, architecture 0s, path-guard 0s
  Release: demo 2m, build 9s
  Waiting: at the sitting 9m, for the human at G4 2m, to start 8s
```

En la segunda línea, lee las cuatro partes del tiempo del Change:

- **Flow time** (tiempo de flujo) va desde la primera línea del Change en el registro hasta la
  última.
- **Active** (tiempo activo) son las llamadas de agente, los comandos de las puertas y los
  pasos de release. El porcentaje es la eficiencia del flujo: el tiempo activo dividido por el
  tiempo de flujo.
- **Waiting** (espera) es el tiempo que se espera a una persona: en la sentada, en G4, aparcado
  hasta que se relanza, en la cola de fusión, antes de empezar.
- **Between steps** (entre pasos) es el resto: el tiempo que gdd emplea entre un paso y otro, o
  un hueco en el registro.

Las líneas de debajo dicen qué se llevó el tiempo activo: aquí, el comando de puerta
`mutation`, con 15 minutos.

Sin `--change`, el informe lista todos los Changes y termina con los totales del proyecto y sus
mayores esperas:

```text
...
Project: 50 change(s) · flow time 106h 27m · active 47h 45m (45%) · waiting 59h 02m · between steps 22m
Biggest waits: for the human at G4 32h 49m, at the sitting 18h 12m, to start 3h 57m
```

Aquí el paso más lento fueron las personas, no los agentes: un tercio del tiempo de flujo
fueron Changes esperando en G4. Agrupar las sentadas y responder antes en G4 ahorraría más que
un modelo más rápido. `--since YYYY-MM-DD` deja solo los Changes creados ese día o después (en
UTC), y `--json` da un objeto por Change.

## 3. Comprueba la entrega con las métricas de DORA

DORA (*DevOps Research and Assessment*) es un programa de investigación que define cinco métricas
de entrega de software; [Las cinco de DORA](../explanation/measuring.md#las-cinco-de-dora)
explica cada una.

```bash
gdd report --dora
```

```text
Delivery · 48 deployment(s) from 2026-10-25 to 2026-10-30 · 48 simulated
Throughput
  Change lead time: median 1h 17m, from the build's start to the promotion
  Deployment frequency: 8.0 per day · median 1h 29m between deployments
  Failed deployment recovery time: none failed
Instability
  Change fail rate: 0% (0 of 48)
  Deployment rework rate: 2% (1 of 48 are Hotfixes)
GDD signals (signs to read, never targets)
  Polish at G4: 14% (8 of 56 decisions)
  Parks: 0.7 per change (37 in 50)
  Reviewer KO: 48% (59 of 124 reviews)
  Reviews to pass: median 2 per change · first review OK in 12 of 48 changes
  Mutation testing red in G3: 11 of 70 runs
  Human stops: 2.1 per change (105 in 50)
  Tokens out: median 190,562 per change
```

El primer bloque son las cinco de DORA, donde un despliegue es una promoción a producción. El
tiempo de entrega de un cambio (*change lead time*) es lo que tarda un Change desde que empieza
su build hasta producción. La tasa de fallos de cambios (*change fail rate*) es la proporción de
despliegues cuya observación falló o que se revirtieron. La primera línea dice que aquí los
despliegues son simulados, porque este repositorio no tiene una producción real.

El segundo bloque son las señales propias de GDD:

| Señal | Qué sugiere un valor alto |
|---|---|
| Polish at G4 | Hecho no es bueno: el trabajo pasa las puertas, pero está sin terminar. Llévalo a `docs/standards.md`. |
| Parks | Changes demasiado grandes, o comandos de puerta que fallan por la máquina y no por el código |
| Reviewer KO, reviews to pass | El Builder repite una clase de bug: conviértela en una regla (paso 5) |
| Mutation testing red | Tests que ejecutan el código sin comprobarlo |
| Human stops | Demasiadas sentadas: agrúpalas |
| Tokens out | El coste por Change, para comparar entre meses o entre modelos |

Aquí `Human stops` cuenta una vez cada sentada y una vez cada respuesta de G4. La línea
`Human stops logged` de [`gdd report`](../reference/cli/report.md) cuenta cada decisión humana
del registro, así que tres aprobaciones en una misma sentada cuentan allí tres veces.

Compara periodos con `--since`:

```bash
gdd report --dora --since 2026-10-29
```

`--json` da un objeto con campos como `lead_time_s`, `change_fail_rate`, `polish_rate` y
`reviewer_ko_rate`, para que montes tu propio panel.

## 4. Aplica tu regla de abandono

Discover escribe una [regla de abandono](../glossary.md#regla-de-abandono) (*kill rule*) en
`docs/brief.md`: el resultado que te haría parar y revisar antes de seguir. La de gdd era esta:
si gdd gasta más tokens o pide más paradas humanas que la herramienta anterior en el mismo tipo
de Change, para y revisa antes de añadir más.

1. Escribe la regla con números que gdd da: tokens de salida por Change, paradas humanas por
   Change, tiempo de flujo.
2. En cada Learn, ejecuta `gdd report --dora --since DAY` para el periodo y compara.
3. Cuando la regla salte, deja de añadir trabajo y abre un Discover.

## 5. Ejecuta Learn

Learn no tiene puerta: refuerza el proyecto. [`gdd learn`](../reference/cli/learn.md) ejecuta las herramientas de Learn de
`[learn]` en `gdd/commands.toml` (solo dan avisos). También lista las notas de polish que dieron
las personas en G4 y las clases de hallazgos del Reviewer que se repiten de un Change a otro.
`/gdd-learn` lo ejecuta por ti y propone un arreglo para cada cosa que se repite.

```bash
gdd learn
```

En un proyecto Python pequeño, deberías ver algo así:

```text
duplication: nothing found
  ...
  All it said: .gdd/learn/duplication.txt
unused: found something (exit code 3)
  src/shop/cart.py:4: unused function 'total' (60% confidence)
  src/shop/cart.py:9: unused function 'unused_helper' (60% confidence)
  ...
  All it said: .gdd/learn/unused.txt

Polish notes at G4 (0)

Reviewer findings that repeat: none
Reviews whose findings name no class: none
```

`gdd learn` siempre termina con 0, encuentren lo que encuentren las herramientas. Convierte lo
que se repite en algo que se ejecute cada vez:

- una nota de polish que vuelve a aparecer se convierte en una línea de `docs/standards.md`,
  que leen el Builder y el Reviewer;
- una clase de hallazgo del Reviewer presente en las revisiones KO de dos Changes o más se
  convierte en una regla de lint, una comprobación o una línea de `docs/standards.md`;
- un aviso de una herramienta que importa se convierte en un comando de puerta en `[g2]` o
  `[g3]`.

Lee con criterio lo que encuentran las herramientas: vulture, por ejemplo, señala funciones
públicas a las que no llama ningún código del proyecto. Sin una sección `[learn]`, `gdd learn`
explica cómo añadir una a partir de las fichas de stack. `--since YYYY-MM-DD` deja solo las
notas y las revisiones de los Changes recientes.

## Siguientes pasos

- [Ajustar las puertas](gates-and-commands.md): convierte lo que encontró Learn en comandos de
  puerta.
- [Elegir agentes y modelos](agents-and-models.md): actúa según lo que diga el informe de
  tokens.
- [Medir lo que importa](../explanation/measuring.md): DORA, las señales de GDD y la regla de
  abandono.
