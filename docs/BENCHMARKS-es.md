<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# Pruebas de rendimiento

La línea base de 1.37.0: qué velocidad alcanza el runtime bajo carga, y con
cuánta fuerza se apoya en el asignador de memoria para conseguirla.

```sh
cargo run --release -p cobolt-bench              # todo
cargo run --release -p cobolt-bench -- dispatch  # una carga, por subcadena
PRC_BENCH_SCALE=0.05 cargo run --release -p cobolt-bench   # una veinteava parte, para una comprobación rápida
```

`--release` no es opcional. Una compilación de depuración mide la ausencia de
optimización, y el arnés lo indica en su cabecera en lugar de dejar que se citen
esas cifras.

## Qué se mide

Cada carga COBOL recorre **el mismo camino que toma un binario entregado** —
tokenizar, analizar sintácticamente, analizar semánticamente, `Interpreter::run`
— porque eso es lo que hace el `main.rs` generado por `rcrun build` con su AST
embebido. Ejecutar en proceso es lo que hace posibles los contadores del
asignador: las cifras describen el intérprete que va dentro de cada binario que
usted entrega.

La memoria se reporta como comportamiento de asignación, no como una curva de
conjunto residente. Rust no tiene recolector de basura, así que no hay pausas
que medir; lo que importa bajo carga es la **rotación** — cuántas veces una carga
entra en el asignador, cuántos bytes lo atraviesan y cuánto está vivo en el pico.
Un asignador global contador
([`counting_alloc.rs`](../crates/cobolt-bench/src/counting_alloc.rs)) proporciona
las tres cifras con exactitud, en las tres plataformas, sin ningún perfilador
externo.

Dos cosas que esto deliberadamente **no** mide: el arranque del proceso y el
tamaño del binario. Mida eso sobre el artefacto real de `rcrun build`.

## La línea base de 1.37.0

Apple M3 Pro, 18 GB, macOS 15.5, rustc 1.95.0, perfil release, 2026-07-27.
Las cifras absolutas viajan mal entre máquinas; **asignaciones por operación**
viaja bien y es la columna que hay que vigilar.

| Carga | Ops | Tiempo | Ops/s | Asign. | Asign./op | MB rotados | Pico vivo MB |
|---|---:|---:|---:|---:|---:|---:|---:|
| dispatch (PERFORM VARYING) | 6 000 000 sent. | 1,049 s | 5 721 961 | 24 000 334 | 4,00 | 72,5 | 0,0 |
| dispatch (PERFORM paragraph) | 500 000 llam. | 0,729 s | 686 318 | 9 000 356 | 18,00 | 409,6 | 0,0 |
| decimal COMPUTE | 500 000 cálculos | 0,824 s | 606 461 | 10 000 499 | 20,00 | 41,0 | 0,0 |
| record batch (1000 filas, escritura+lectura) | 400 000 registros | 2,179 s | 183 612 | 26 023 007 | 65,06 | 227,9 | 0,8 |
| object churn (crear/leer/destruir) | 20 000 objetos | 0,092 s | 216 320 | 1 100 000 | 55,00 | 27,5 | 0,0 |
| indexed redb (inserción masiva) | 100 000 registros | 0,710 s | 140 922 | 65 854 | 0,66 | 188,9 | 22,4 |
| indexed redb (lectura aleatoria) | 50 000 lecturas | 0,034 s | 1 489 965 | 9 | 0,00 | 0,0 | 22,4 |

## Qué dice la línea base

**El cuello de botella es el asignador, no el recorrido del árbol.** 5,7 M de
sentencias por segundo es una tasa de despacho respetable — pero alcanzarla
costó **24 millones de asignaciones para 6 millones de sentencias**.
`ADD 1 TO ACC` sobre dos campos `COMP`, que no debería tocar el montículo en
absoluto, cuesta cuatro viajes por el asignador. Eso replantea el trabajo de
optimización: las primeras victorias están en el sistema de valores y en el
camino de los operandos, no en sustituir el intérprete por una máquina virtual de
bytecode. Una VM abarataría el despacho dejando intactas las cuatro asignaciones
por sentencia.

**Las llamadas a párrafo son caras de forma desproporcionada.** 18 asignaciones y
unos 820 bytes por `PERFORM <paragraph>`, frente a 4 por sentencia en línea.
Medio millón de llamadas rotan 410 MB. Lo que sea que el camino de llamada
construya en cada invocación es el objetivo de mayor densidad de la tabla.

**Los registros alfanuméricos asignan por campo, como cabe esperar.** 65
asignaciones por registro para una fila de 4 campos leída y escrita es
`CobolValue::String` poseyendo un `Vec<u8>` por campo, más uno nuevo por cada
`MOVE`. Una representación de cadena corta en línea, o rebanar dentro del propio
búfer del registro, se notaría aquí de inmediato.

**Las lecturas de propiedades de objeto asignan sin motivo.** 55 asignaciones por
objeto a lo largo de 24 lecturas de propiedad. `CoboltObject::get_property`,
`get_str`, `get_bool` y `get_i64` llaman cada uno a `name.to_ascii_uppercase()`
— una `String` asignada y descartada **por lectura**, únicamente para que la
búsqueda no distinga mayúsculas. Un envoltorio de clave insensible a mayúsculas
elimina la columna entera.

**El motor INDEXED no es el problema.** redb inserta a 141 k registros/s con 0,66
asignaciones por registro y sirve 1,5 M de lecturas aleatorias por segundo
prácticamente sin asignar. El almacenamiento va cómodamente por delante del
intérprete que lo alimenta.

Ordenado por retorno esperado, el orden de optimización que sugiere la línea base
es: las asignaciones por sentencia, después el camino de llamada a párrafo,
después `CobolValue` para alfanuméricos, y después el paso a mayúsculas de las
propiedades de objeto. El almacenamiento no aparece hasta bastante por debajo de
todo eso.

## Cargas de trabajo

| Carga | Qué aísla |
|---|---|
| `dispatch (PERFORM VARYING)` | Sobrecoste del recorrido del árbol: prueba del bucle, incremento, una sentencia, trabajo mínimo por debajo |
| `dispatch (PERFORM paragraph)` | Sobrecoste de la llamada a párrafo, frente al caso en línea anterior |
| `decimal COMPUTE` | La aritmética escalada a i128 de `CobolNumeric` — matemática monetaria COBOL |
| `record batch` | Tabla de 1000 filas escrita y releída con campos alfanuméricos; el sistema de valores bajo carga por lotes |
| `object churn` | `ObjectRegistry` crear/leer/destruir — lo que cuesta un formulario con muchos controles |
| `indexed redb` | El motor de ficheros INDEXED: inserción masiva y después lecturas por clave aleatoria |

Las dos filas de `indexed redb` son una versión recuperada y generalizada del
micro-benchmark `open_table_cost`, que sigue estando marcado `#[ignore]` dentro
de `cobolt-runtime::indexed_redb` (`indexed_redb.rs:1264`). Solo se ejecuta
cuando alguien recuerda una invocación `--ignored` exacta, de modo que el motor
no tenía una línea base permanente; ahora la tiene aquí, y el original se deja
donde está. Se conserva su conclusión original: el manejador de la tabla se abre
una sola vez para toda la transacción de escritura, lo que se midió ~16 % más
rápido que abrirlo dos veces por inserción.

## Añadir una carga de trabajo

Añada una función `bench_*` a
[`crates/cobolt-bench/src/main.rs`](../crates/cobolt-bench/src/main.rs) que
devuelva `measure(name, unit, || { ...; ops_performed })`, y regístrela en `main`
detrás de un filtro `wanted(...)`. Los contadores envuelven el cierre
automáticamente. Devuelva el número de unidades de *trabajo*, no de iteraciones,
para que `ops/sec` y `allocs/op` sigan siendo comparables entre cargas.

Mantenga deterministas las cargas nuevas. La sonda de lectura aleatoria usa un
paso multiplicativo fijo en lugar de un generador de números aleatorios
precisamente por esto: un benchmark que se rebaraja entre ejecuciones no se puede
comparar con la cifra de ayer.

.<<
