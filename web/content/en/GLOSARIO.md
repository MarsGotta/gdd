# Gate-Driven Development: English glossary and style

Translate from Spanish to American English. The reader is a developer or product person who has never heard of GDD.

## Voice

- Plain, warm and direct, like the Spanish: a senior developer talking to a colleague, active voice, "you" for the reader, "I" for the author. Never the editorial "we" (the one exception is the manifesto's central sentence: "We design the gates our agents will go through").
- Mix long and short sentences and let them flow into each other. Read it aloud: if nobody would say it that way, rewrite it. Contractions are welcome.
- Avoid what sounds like a machine: the text talking about itself ("this chapter", "here is"), "it's not X, it's Y" unless it corrects a real misconception, rule-of-three lists as a default rhythm, colons for effect, closing lines that sum up, the same key word three times in a paragraph.
- Where the Spanish speaks in first person (Marcela Gotta, the author), keep the first person.
- Natural English, never word for word. Do not add ideas, drop ideas, soften claims or add marketing.
- Never use em dashes or en dashes. Use commas, parentheses or colons. Keep straight quotes (").
- Do not add any mention of an AI assistant or of who translated this.

## Terms (always these)

| Spanish | English |
|---|---|
| puerta | gate (in the physical escape-room story or a floor plan drawing, "door" is fine where "gate" would sound wrong) |
| comprobación (how a gate is validated; a gate has one or several, and it is passed only if all pass) | check |
| contexto (what you give the agent so it can get through a gate; it helps, and it proves nothing) | context |
| contexto transversal (what the agent always carries, at every gate) | cross-cutting context |
| contexto limpio (only the cross-cutting context, its gate's and what the previous gate generated; no other agent's conversation) | clean context |
| constitución (the short document with the project's rules, the main piece of the cross-cutting context) | constitution |
| de máquina, de vigilante, de persona (the three types of check) | machine, reviewer, person ("machine checks", "a reviewer's check", "a person's check") |
| pasar la puerta (what the agent does; there is no "key" in the method) | get through the gate |
| reforzar (verb only; the noun "refuerzo" is retired, say comprobación) | reinforce (verb only; never the noun "reinforcement", say check) |
| vigilante | reviewer (the "robot vigilante" of the escape room: robot reviewer) |
| riesgo | risk |
| semáforo | traffic light |
| Verde, Amarillo, Rojo | Green, Yellow, Red |
| (bajo), (medio), (alto) | (low), (medium), (high) |
| el agente (the one who does the work) | the agent |
| otro agente (the one who makes the checks) | another agent |
| recepcionista (only in the escape-room story) | receptionist |
| autor | author |
| equipo | team |
| lo que se cuela (what used to be "fuga") | what slips through |
| se cuela | slips through |
| grieta | crack |
| blindar, blindaje | harden, hardening |
| escenario | scenario |
| recorrido | path (the whole run through the gates) |
| motor | engine |
| caja | box |
| plano (the project's rules; replaces "normas", "reglas del juego") | blueprint |
| zona (a set of paths in the blueprint, with its level of risk) | zone (never "area") |
| área (the field that ready-made rules are for) | area |
| lo que debe pasar, qué debe pasar, qué tiene que pasar | what has to pass (never "what should happen" or "what has to happen") |
| entorno del agente (replaces "harness", except in titles and quotes of sources) | the agent's environment |
| reglas ya hechas (outside the T-minus part) | ready-made rules |
| registro | record |
| aprobación, aprobar (replaces "firma", "tu OK") | approval, approve |
| núcleo | core |
| momento (the four moments of the method) | moment |
| Preparar | Prepare |
| Encargar | Brief |
| Ejecutar | Execute |
| Blindar | Harden |
| Gherkin, CI, PR, linter, escape room | unchanged |
| kit | kit, only in the T-minus part (chapters The engine and The kits) and in the name "Spec Kit"; elsewhere "ready-made rules" |
| sale, salir (un cambio) | ships, ship |
| Salida (name of a gate) | Release |
| dar tu aprobación, tu aprobación | give your approval, your approval |
| KO | "says no" or "rejects"; avoid the word KO |
| "hecho" (what the agent says) | "done" |
| criterio (de una persona) | judgment |
| Manifiesto | Manifesto |
| documentación | documentation (the section of the site made of chapters) |
| capítulo | chapter |
| Entender, Aplicar, Garantizar, T-minus, Consultar (the five parts) | Understand, Apply, Guarantee, T-minus, Look up |
| La idea | The idea |
| Manos a la obra | Hands on |
| Las piezas y los roles | The pieces and the roles |
| Los siete principios | The seven principles |
| Los cuatro momentos | The four moments |
| Empieza hoy | Start today |
| Un caso delicado | A delicate case |
| El recorrido entero | The whole path |
| Cómo se engaña una puerta | How a gate gets fooled |
| El núcleo | The core |
| El motor | The engine |
| Cómo se mide | How it is measured |
| Los kits | The kits |
| T-minus (the fifth part of the documentation) | T-minus |
| Buenas prácticas | Good practices |
| ¿Y TDD? | What about TDD? |
| De dónde viene | Where it comes from |
| Anterior, Siguiente (between chapters) | Previous, Next |
| norma, garantía | rule, guarantee |
| sellar, sellada (T-minus part) | seal, sealed |

## The running example

Online store (tienda online), discount codes (códigos de descuento), `VERANO10` is `SUMMER10`, `conDescuento` is `withDiscount`, `pagos/` is `payments/`, `descuentos.test.js` is `discounts.test.js`. Prices go first in English: 50 € is €50, 80 € is €80. Percent: 10 % is 10%. Thousands: 2.500 is 2,500; decimals: 1,5 is 1.5.

## Sentences with a fixed translation

The site looks these up, so when they appear use exactly this English.

| Spanish | English |
|---|---|
| Añade puertas a tu proyecto y automatiza con agentes de IA sin perder calidad. | Add gates to your project and automate with AI agents without losing quality. |
| Tu agente dice "hecho". La puerta comprueba si es verdad. | Your agent says "done." The gate checks if it's true. |
| Gate-Driven Development (GDD) es una forma de crear productos de calidad con agentes de IA sin supervisarlos paso a paso. | Gate-Driven Development (GDD) is a way to build quality products with AI agents without supervising them step by step. |
| Los agentes escriben rápido y casi bien. Si lo revisas todo, te conviertes en el paso más lento. Si no revisas nada, los errores llegan a producción. Y si el agente puede tocar lo que lo juzga, aprende a aprobarse solo. | Agents write fast and almost right. If you review everything, you become the slowest step. If you review nothing, the errors reach production. And if the agent can touch what judges it, it learns to approve itself. |
| Al final es una bestia indomable, y a una bestia así no se la convence con palabras: se le ponen puertas. | In the end it is an untamable beast, and you don't persuade a beast like that with words: you put gates on it. |
| Esa es la base de GDD: no desarrollamos las especificaciones, ni el código, ni siquiera los tests. Diseñamos las puertas por las que pasarán nuestros agentes. | That is the basis of GDD: we don't develop the specifications, or the code, or even the tests. We design the gates our agents will go through. |
| Lo que no se puede deshacer nunca es verde. | What can't be undone is never green. |
| Esta versión todavía no trae datos propios. | This version doesn't bring its own data yet. |
| Este manifiesto es la referencia. | This manifesto is the reference. |
| v0.8 · borrador para revisión | v0.8 · draft for review |
| Antes de que el agente empiece, decides qué tiene que demostrar. | Before the agent starts, you decide what it has to prove. |
| Lo comprueban cosas que ya conoces, como tests, linters o una revisión, y el agente no puede tocarlas. | It is checked by things you already know, like tests, linters or a review, and the agent can't touch them. |
| Cuanto más delicado es el cambio, más comprobaciones tiene que pasar, y solo te llama cuando hace falta tu criterio. | The more delicate the change, the more checks it has to pass, and it only calls you when your judgment is needed. |
| Nada: pides el cambio y, si pasa las puertas, sale. | Nothing: you ask for the change and, if it passes the gates, it ships. |
| Leer unos pocos escenarios. | Read a few scenarios. |
| Leer los escenarios y dar tu aprobación donde haga falta. | Read the scenarios and give your approval where needed. |
| Así se pasa: no por lo que dijo el agente, sino por lo que comprobó la puerta. | That is how you get through: not by what the agent said, but by what the gate checked. |
| Quien cruza la puerta no decide cómo se valida. | Whoever goes through the gate doesn't decide how it's validated. |
| Decir "hecho" no abre nada. | Saying "done" opens nothing. |
| Y ya está. (opening a closing paragraph) | And that's it. |
| Pieza (table head) | Piece |
| Riesgo (table head) | Risk |
| A la IA no se la convence con palabras: se le ponen puertas (article title) | You don't persuade AI with words: you put gates on it |

## The running example, second thread

The first change is a greeting: `saludo` is `greet`, "Hola, Ana" is "Hello, Ana". The names Ana, Luis and Marta stay as they are.

## HTML rules

- The English file has exactly the same elements, in the same order, with the same attributes. Translate only text.
- Keep every `href` as it is, including `doc:la-idea` style links (and their `#anchor`, which is the same in both languages: the heading gets an explicit `id`). Keep `data-*` attributes and classes.
- `<code data-fuente="...">` stays empty, as in the Spanish file.
- Inline `<code>` follows the running example above. Otherwise leave code as it is.

## Quotations

- English quotations already in English in the Spanish text stay word for word.
- If the Spanish puts in quotation marks a sentence that comes from an English source, open the link with WebFetch and use the exact English. If you cannot confirm the exact wording, drop the quotation marks, paraphrase, and say so in your report.
- Never invent a quotation. Names of companies, studies and figures stay exactly as they are, only the number format changes.
