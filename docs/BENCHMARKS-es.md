<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Benchmarks

La línea base de 1.37.0: qué tan rápido es el runtime bajo carga, y cuánto se
apoya en el asignador de memoria para llegar ahí.

```sh
cargo run --release -p cobolt-bench              # everything
cargo run --release -p cobolt-bench -- dispatch  # one workload, by substring
PRC_BENCH_SCALE=0.05 cargo run --release -p cobolt-bench   # a twentieth, for a quick check
```

`--release` no es opcional. Una compilación debug mide la *ausencia* de
optimización, y el harness lo dice en su cabecera en lugar de dejar que los
números se citen.

## Qué se mide

Toda carga de trabajo COBOL recorre el **mismo camino que recorre un binario
entregado** — tokenizar, parsear, analizar, `Interpreter::run` — porque eso es
lo que hace el `main.rs` generado por `rcrun build` con su AST incorporada.
Ejecutar dentro del propio proceso es lo que hace posibles los contadores del
asignador: los números describen el intérprete que va dentro de cada binario que
usted entrega.

La memoria se reporta como comportamiento de asignación, no como una curva de
resident set. Rust no tiene recolector de basura, así que no hay pausas que
medir; lo que importa bajo carga es la **rotación** — cuántas veces una carga
entra en el asignador, cuántos bytes pasan por él y cuánto queda vivo en el
pico. Un asignador global con contadores
([`counting_alloc.rs`](../crates/cobolt-bench/src/counting_alloc.rs)) entrega
los tres con exactitud, en las tres plataformas, sin ningún profiler externo.

Dos cosas que esto deliberadamente **no** mide: el arranque del proceso y el
tamaño del binario. Mida esas sobre el artefacto real que produce
`rcrun build`.

## La línea base de 1.37.0

Apple M3 Pro, 18 GB, macOS 15.5, rustc 1.95.0, perfil release, 2026-07-27. Los
números absolutos viajan mal entre máquinas; **asignaciones por operación**
viaja bien y es la columna a vigilar.

| Carga de trabajo | Ops | Tiempo | Ops/s | Asignaciones | Asig./op | MB rotados | Pico vivo (MB) |
|---|---:|---:|---:|---:|---:|---:|---:|
| dispatch (PERFORM VARYING) | 6 000 000 stmt | 1,049s | 5 721 961 | 24 000 334 | 4,00 | 72,5 | 0,0 |
| dispatch (PERFORM paragraph) | 500 000 call | 0,729s | 686 318 | 9 000 356 | 18,00 | 409,6 | 0,0 |
| decimal COMPUTE | 500 000 compute | 0,824s | 606 461 | 10 000 499 | 20,00 | 41,0 | 0,0 |
| record batch (1000 rows, write+read) | 400 000 record | 2,179s | 183 612 | 26 023 007 | 65,06 | 227,9 | 0,8 |
| object churn (create/read/destroy) | 20 000 object | 0,092s | 216 320 | 1 100 000 | 55,00 | 27,5 | 0,0 |
| indexed redb (bulk insert) | 100 000 record | 0,710s | 140 922 | 65 854 | 0,66 | 188,9 | 22,4 |
| indexed redb (random read) | 50 000 read | 0,034s | 1 489 965 | 9 | 0,00 | 0,0 | 22,4 |

## Qué dice la línea base

**El cuello de botella es el asignador, no el recorrido del árbol.** 5,7 M de
sentencias por segundo es una tasa de despacho respetable — pero llegar ahí
costó **24 millones de asignaciones para 6 millones de sentencias**. Un
`ADD 1 TO ACC` sobre dos campos `COMP`, que no debería tocar el heap en
absoluto, cuesta cuatro viajes al asignador. Eso reencuadra el trabajo de
optimización: las primeras ganancias están en el sistema de valores y en el
camino de los operandos, no en sustituir el intérprete que recorre el árbol por
una VM de bytecode. Una VM abarataría el despacho dejando intactas las cuatro
asignaciones por sentencia.

**Las llamadas a párrafo son caras fuera de toda proporción.** 18 asignaciones y
unos 820 bytes por `PERFORM <paragraph>`, frente a 4 por sentencia en línea.
Medio millón de llamadas rotan 410 MB. Sea lo que sea que el camino de llamada
esté construyendo en cada invocación, es el objetivo de mayor densidad de la
tabla.

**Los registros alfanuméricos asignan por campo, como era de esperar.** 65
asignaciones por registro para una fila de 4 campos leída y escrita es
`CobolValue::String` siendo dueño de un `Vec<u8>` por campo, más uno nuevo en
cada `MOVE`. Una representación en línea para cadenas cortas, o rebanar dentro
del propio búfer del registro, se notaría aquí de inmediato.

**Las lecturas de propiedades de objeto asignan sin motivo.** 55 asignaciones
por objeto a lo largo de 24 lecturas de propiedad. `CoboltObject::get_property`,
`get_str`, `get_bool` y `get_i64` llaman cada uno a
`name.to_ascii_uppercase()` — una `String` asignada y descartada **por
lectura**, únicamente para que la búsqueda no distinga mayúsculas. Un envoltorio
de clave insensible a mayúsculas elimina la columna entera.

**El motor INDEXED no es el problema.** redb inserta a 141 mil registros por
segundo con 0,66 asignaciones por registro y sirve 1,5 M de lecturas aleatorias
por segundo con asignación esencialmente nula. El almacenamiento va cómodamente
por delante del intérprete que lo alimenta.

Ordenado por retorno esperado, el orden de optimización que sugiere la línea
base es: las asignaciones por sentencia, después el camino de llamada a párrafo,
después `CobolValue` para alfanuméricos, después el paso a mayúsculas de las
propiedades de objeto. El almacenamiento no aparece hasta bastante por debajo de
todo eso.

## Cargas de trabajo

| Carga de trabajo | Qué aísla |
|---|---|
| `dispatch (PERFORM VARYING)` | Coste del recorrido del árbol: prueba del bucle, incremento, una sentencia, trabajo mínimo por debajo |
| `dispatch (PERFORM paragraph)` | Coste de la llamada a párrafo, frente al caso en línea anterior |
| `decimal COMPUTE` | La aritmética escalada en i128 de `CobolNumeric` — las cuentas de dinero de COBOL |
| `record batch` | Tabla de 1000 filas escrita y releída con campos alfanuméricos; el sistema de valores bajo carga por lotes |
| `object churn` | Crear/leer/destruir de `ObjectRegistry` — lo que cuesta un formulario con muchos controles |
| `indexed redb` | El motor de archivos INDEXED: inserción masiva y luego lecturas por clave aleatoria |

Las dos filas de `indexed redb` son una versión recuperada y generalizada del
micro-benchmark `open_table_cost`, que vivía marcado con `#[ignore]` dentro de
`cobolt-runtime::indexed_redb`. Solo se ejecutaba cuando alguien recordaba una
invocación exacta con `--ignored`, de modo que el motor no tenía línea base
permanente; ahora la tiene. Su conclusión original se conserva: el handle de la
tabla se abre una sola vez para toda la transacción de escritura, lo que se
midió como un 16 % más rápido que abrirlo dos veces por inserción.

## Añadir una carga de trabajo

Añada una función `bench_*` en
[`crates/cobolt-bench/src/main.rs`](../crates/cobolt-bench/src/main.rs) que
devuelva `measure(name, unit, || { ...; ops_performed })`, y regístrela en
`main` detrás de un filtro `wanted(...)`. Los contadores envuelven la closure
automáticamente. Devuelva el número de unidades de *trabajo*, no de iteraciones,
para que `ops/sec` y `allocs/op` sigan siendo comparables entre cargas.

Mantenga deterministas las cargas nuevas. La sonda de lectura aleatoria usa un
paso multiplicativo fijo en lugar de un generador de números aleatorios
justamente por esto: un benchmark que se rebaraja entre ejecuciones no puede
compararse con el número de ayer.
