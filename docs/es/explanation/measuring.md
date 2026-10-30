# Medir lo que importa

Qué mide GDD, por qué cuenta los tokens antes que los dólares, cómo toma prestadas las métricas
de entrega de DORA y cómo un número se convierte en una decisión. Léela antes de juzgar si GDD
compensa en tu proyecto.

## Antes de empezar

[GDD en una página](method.md) da los principios; el último es el tema de esta página. Los
informes que se describen aquí son `gdd report`, `gdd report --time` y `gdd report --dora`, y los
comprueban [`features/report.feature`](../../../features/report.feature),
[`features/flow-time.feature`](../../../features/flow-time.feature) y
[`features/delivery-report.feature`](../../../features/delivery-report.feature).

## Resultados, no volumen

El octavo principio de GDD dice: *mide resultados, no volumen* (*outcomes, not output*). El
volumen es lo que fabrican los agentes: líneas de código, ficheros, llamadas. Los resultados
son lo que obtienes tú: Changes que llegan a producción, se quedan allí y te han pedido poco
tiempo.

El volumen es barato de contar y fácil de inflar. Un agente al que se le pide más código
escribe más código. Cuando una medida se convierte en objetivo, deja de ser una buena medida (la
ley de Goodhart); DORA pone "setting metrics as a goal" (fijar las métricas como objetivo) en
primer lugar entre los [errores habituales con sus propias
métricas](https://dora.dev/guides/dora-metrics/#common-pitfalls). Por eso GDD nunca pone un
objetivo sobre las líneas de código, y como mucho las trata como un síntoma de hinchazón.

## Primero los tokens, después los dólares

El coste de un Change son los tokens que usaron sus agentes. Cada llamada de agente añade una
línea a `.gdd/log.jsonl`, leída de la propia salida JSON del agente, y `gdd report` las suma:

```text
role -> calls · tokens in · cache · out · cost USD · seconds
```

Las columnas separan tres clases de tokens:

- **tokens in**: la entrada que el modelo leyó nueva, no de su caché;
- **cache**: la entrada que leyó de la caché, mucho más barata y que crece con cada turno de una
  sesión larga;
- **out**: lo que escribió.

Sumar las tres en un solo número esconde lo que ha cambiado. Una sesión larga puede mostrar
decenas de millones de tokens de caché y una cantidad modesta de tokens nuevos; lo que comparas
entre dos formas de trabajar es la entrada nueva y la salida.

Los dólares van después porque no en todas partes son una medición. Claude Code informa de un
coste que es una estimación hecha en el cliente. Codex no informa de ninguno; una tabla de precios
en `gdd/models.toml` permite que el log lo estime. Con una suscripción, el coste por llamada es
nominal. Los tokens son lo que informan todos los agentes, así que los tokens son lo que permite
comparar.

`gdd report --by change,model,role` agrupa las llamadas como necesites: por Change, fase, modelo,
rol o agente. Lo que gastó un Change también vive en su registro, que solo escribe gdd, y en su
commit de fusión.

## En qué se va el tiempo

Un Change puede ser barato y aun así lento. `gdd report --time` sigue cada Change desde su primera
línea en el log hasta la última, su **tiempo de flujo** (*flow time*), y lo reparte:

- **activo**: llamadas de agente por rol y modelo, ejecuciones de puertas por comando, pasos de
  salida;
- **en espera**: en la sentada, antes de que empiece el Change, aparcado hasta que se relanza,
  esperándote en G4;
- **entre pasos**: lo que queda.

La **eficiencia de flujo** (*flow efficiency*) es el tiempo activo dividido entre el tiempo de
flujo, una medida del Flow Framework. Un Change que tardó seis horas con cuarenta minutos de
trabajo no necesitaba agentes más rápidos: estuvo esperando. El informe nombra las esperas más
largas, para que sepas qué paso pulir primero. El Board muestra lo mismo en el detalle de un
Change y en una vista de informe.

## Las cinco de DORA

[La investigación de DORA](https://dora.dev/guides/dora-metrics/) define cinco métricas de entrega
de software, en dos grupos:

| Grupo | Métrica | Definición de DORA, en resumen |
|---|---|---|
| Rendimiento (*throughput*) | Tiempo de entrega de un cambio (*change lead time*) | Desde el commit hasta el despliegue en producción |
| Rendimiento (*throughput*) | Frecuencia de despliegue (*deployment frequency*) | Despliegues por periodo, o el tiempo entre uno y otro |
| Rendimiento (*throughput*) | Tiempo de recuperación de un despliegue fallido (*failed deployment recovery time*) | Tiempo para recuperarse de un despliegue que necesita intervención inmediata |
| Inestabilidad (*instability*) | Tasa de fallos de cambios (*change fail rate*) | Proporción de despliegues que necesitan intervención inmediata |
| Inestabilidad (*instability*) | Tasa de retrabajo de despliegues (*deployment rework rate*) | Proporción de despliegues no planificados, causados por un incidente en producción |

`gdd report --dora` las calcula a partir del log y de los registros, con estas decisiones:

- Un **despliegue** es una promoción: el paso `promote` de la salida, pasado o simulado.
- Un despliegue **falló** cuando falló su observación o se revirtió.
- El **tiempo de entrega de un cambio** va desde el inicio de la construcción hasta la promoción.

Las métricas de DORA son el motivo por el que GDD mide la entrega, no la actividad. Su
[informe de 2025](https://dora.dev/research/2025/dora-report/) añade un aviso que encaja bien con
los agentes: la IA es "un amplificador, que magnifica las fortalezas y debilidades que ya tiene
una organización". Agentes más rápidos en un proceso débil entregan antes software débil.

## Las señales propias de GDD

DORA no ve el interior de un Change. GDD añade las señales que producen sus propias puertas,
impresas después de las cinco de DORA bajo un título que dice lo que son: *GDD signals (signs to
read, never targets)*, es decir, señales que leer, nunca objetivos.

| Señal | Qué sugiere cuando sube |
|---|---|
| Polish en G4 | Los Changes parecen terminados antes de estarlo: a los estándares o al paquete les falta algo |
| Aparcamientos por Change | Los Changes son demasiado grandes, o a sus paquetes les falta contexto |
| KO del Reviewer | El Builder no ve lo que ve el Reviewer: una lección para Learn |
| Revisiones hasta pasar | Los hallazgos llegan con cuentagotas en lugar de en una sola pasada |
| Mutation testing en rojo en G3 | Tests que no comprueban lo que parece |
| Paradas humanas por Change | Se te pregunta más de lo que promete el método |
| Tokens out por Change | El coste por Change, por su mediana |

Ninguna es una meta. Un equipo que bajara su tasa de polish a cero lo conseguiría dejando de
mirar. Cada una es una pregunta que hacerse en Learn.

## La regla de abandono

Cada MVP de GDD recibe en Discover una **regla de abandono** (*kill rule*): lo que tendría que
pasar para dejarlo. Se escribe antes de que empiece el trabajo, cuando todavía nadie se ha
encariñado con él, y se escribe en números que los informes puedan mostrar.

gdd tiene una para sí mismo, en su [roadmap](../../../ROADMAP.md): si gdd gasta más tokens o pide
más paradas humanas que la herramienta a la que sustituye, en el mismo tipo de Change, hay que
parar y revisar antes de añadir nada más. Los dos números de esa frase son una columna de
`gdd report` y una línea de `gdd report --dora`.

## De los números a Learn

Learn, la fase 5, es donde los números se convierten en cambios. Una nota de polish repetida se
convierte en una línea de `docs/standards.md`; una clase de hallazgo del Reviewer que vuelve a
salir se convierte en un lint o un test; un paso que siempre espera se convierte en lo próximo que
arreglar. `gdd learn` enumera las repeticiones; los informes dicen si el arreglo funcionó.

## Un ejemplo: una sesión de Builder o dos

Por defecto, el Builder ejecuta el modo test y el modo build en sesiones separadas: el modo build
empieza de cero y lee lo que escribió el modo test. Una objeción razonable es que el traspaso
pierde contexto y gasta tokens en volver a cargarlo. gdd zanjó la cuestión con un experimento
sobre sus propios Changes.

El montaje: diez Changes cerrados, reproducidos desde su commit base, cada uno ejecutado dos veces
con cada configuración y con el modelo fijo, y calificados con sus propios escenarios aprobados y
ocultos. La configuración A era la de por defecto, dos sesiones. La configuración B era una sola
sesión del Builder que primero escribía los pasos y después seguía en modo build, con Path Guard
demostrando que los pasos no se editaron después de su primera ejecución.

El resultado, en 40 ejecuciones:

| | A: dos sesiones | B: una sesión |
|---|---|---|
| Ejecuciones que pasaron todas las puertas | 14 de 20 | 13 de 20 |
| Tokens nuevos | 6,0M | 4,7M, alrededor de un 21 % menos |

B también tardó alrededor de un 7 % menos. La calidad quedó dentro del ruido: cuatro ejecuciones
de cada configuración fallaron por el sandbox (el mutation testing agotó el tiempo con dos Changes
a la vez), y la mayoría de los demás fallos, dos en A y tres o cuatro en B, fueron mutantes que
ningún test pilló. El ahorro era real, pero modesto.

Así que lo de por defecto siguió siendo dos sesiones, y una sola sesión pasó a ser una opción:
`[session] builder = "shared"` en `gdd/models.toml`, hoy para Claude Code, donde la ficha de agente
dice cómo retomar una sesión. Path Guard, el mutation testing y el Reviewer siguen como estaban.

Lo importante es el método, no el resultado. Una pregunta que suena a cuestión de gustos (¿una
sesión o dos?) se convirtió en una medición con una calificación fija y una regla decidida antes
de las ejecuciones: quedarse con una sola sesión solo si la calidad se mantiene.

## Ver también

- [Medir el trabajo](../how-to/measure.md): los comandos de informes, paso a paso.
- [Referencia de comandos](../reference/cli.md#gdd-report): todas las opciones de `gdd report`.
- [Pocas paradas para la persona](human-stops.md): de dónde salen el polish y las paradas humanas.
- [Decisiones de diseño](design-decisions.md#modos-test-y-build-separados): por qué hay dos modos.
