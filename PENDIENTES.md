# Pendientes de jonesy

Rastro de lo que falta o está flojo respecto de [`jonesy_spec.md`](jonesy_spec.md) y de
los pedidos posteriores. Cada ítem dice su estado y, si aplica, el requisito.

Estados: **falta** (no está), **flojo** (está pero incompleto o sin probar),
**en curso**, **hecho** (se deja unos días para referencia y después se borra),
**externo** (depende de otro repositorio o de una máquina que no está acá).

## Perfil estudiante (prioridad)

jonesy es un IDE para alumnos de Programación I: los esfuerzos van primero a lo que el
alumno usa todos los días (entender errores, compilar, correr, probar, depurar,
memoria). Lo de docentes e infraestructura queda en segundo plano.

| Ítem | Estado | Notas |
|---|---|---|
| Errores de gcc explicados en español | hecho | ~60 errores y advertencias (sintaxis, declaraciones, printf/scanf, punteros, `-Wall -Wextra`, enlazador) con resumen en Problemas y al final de la línea, y F1 con ejemplo «así no / así sí». Catálogo ampliable en `data/errores_gcc.toml`. |
| El programa revienta | hecho | la terminal y la barra explican la señal (violación de segmento, abort, división por cero…); «[F5 Ver dónde falló]» corre el depurador hasta la línea, saltando fuera de libc al código del alumno. |
| Correr las pruebas | hecho | Ctrl+F6: `make test` del módulo; cada fallo de `p1_test` en su línea con esperado/obtenido, los cortes en la definición `TEST(…)`. |
| Revisar la memoria | hecho | valgrind (`make memcheck` o el programa): accesos inválidos, uso tras `free`, `free` doble, sin inicializar y fugas, explicados y en la línea del alumno. |
| Markdown pegaba palabras en el borde | hecho | afectaba consignas y ayudas. |
| Memoria en Windows | falta | no hay valgrind; evaluar AddressSanitizer donde exista o apoyarse en cat-scan. |
| Programas con entrada bajo valgrind | falta | Revisar la memoria corre sin teclado (en el panel): un programa que lee de stdin recibe EOF. Correrlo en el deck y leer la salida al terminar. |
| Salud del entorno | hecho | `jonesy doctor` por categorías con pruebas de humo (gcc, gdb, valgrind), ptrace, identidad de git, terminal, `.jonesy.toml` y permisos; también en Ayuda. |
| Primera vez | hecho | pantalla de bienvenida al primer inicio (fuera del examen) con el ciclo de un ejercicio y qué falta del entorno; Ayuda → Bienvenida; G guía rápida, D diagnóstico; casilla para verla siempre. |
| Explicaciones de errores de ripley/gaff | flojo | F1 depende de que cada herramienta tenga `explain`; unificar el formato con el de los errores de gcc. |
| Menús reorganizados | hecho | según `docs/critica_y_propuesta_de_reorganizacion.md`: Construir y Depurar separados, Herramientas pasa a Archivo → Configuración del entorno (integraciones, plugins, compilador y formato, apariencia), separadores, Ver con los modos F7/F8/F9 primero, sin atajos de panel en el menú Git, guía rápida, limpiar compilación, restablecer paneles y estado de la entrega. Letra subrayada en cada menú: F10 y la letra abre cualquiera; Alt+letra directo salvo Editar, Git y Ayuda (Alt+E, Alt+G, Alt+Y son de paneles). |
| Menú Plugins | hecho | Alt+L: los comandos de los plugins Python y las acciones de las herramientas para el archivo activo, ejecutables desde el menú; los comandos se piden al worker al abrirlo. |
| Ctrl+, para la configuración | flojo | sólo llega en terminales con el protocolo de teclado extendido; en el resto se usa Archivo → Configuración del entorno. |
| Pruebas: salida de otros frameworks | hecho | SARIF 2.1.0 en la salida o en el archivo de `[pruebas] sarif`; `[pruebas] comando` cambia el comando. Las integraciones aceptan `formato = "sarif"`. |

## Mejoras de calidad de vida (QoL)

Cincuenta mejoras chicas pensadas para el alumno, tipificadas por área. **Impacto**: cuánto
cambia el día a día de quien cursa (alto, medio, bajo). **Esfuerzo**: S (horas), M (un día
o dos), L (más). Hechas 28 de 50 (ver el historial de git para cada una). Por tipo: Edición 10 · Navegación 5 · Compilación 5 · Ejecución 5 · Pruebas 3 · Depuración 5 · Memoria 3 · Git 4 · Aprendizaje 4 · Interfaz 3 · Examen 2 · Mantenimiento 1.

| # | Mejora | Tipo | Impacto | Esfuerzo | Estado |
|---|---|---|---|---|---|
| 1 | Cerrar solos paréntesis, corchetes, llaves y comillas, y saltar el cierre al escribirlo | Edición | alto | S | hecho |
| 2 | Desindentar sola la línea al escribir `}` (Enter ya copia la sangría y suma un nivel tras `{`) | Edición | alto | S | hecho |
| 3 | Resaltar la llave, paréntesis o corchete que hace pareja con el del cursor | Edición | alto | S | hecho |
| 4 | Comentar o descomentar las líneas elegidas con Ctrl+/ | Edición | medio | S | hecho |
| 5 | Mover la línea arriba/abajo (Alt+↑/↓) y duplicarla (Ctrl+D) | Edición | medio | S | hecho |
| 6 | Autocompletar identificadores del archivo y funciones de la base del quick-doc (Ctrl+Espacio) | Edición | alto | M | falta |
| 7 | Fragmentos de C con Tab: `for`, `while`, `if/else`, `switch`, `main`, `malloc` con su verificación de NULL | Edición | medio | M | falta |
| 8 | Buscar y reemplazar (Ctrl+H) con mayúsculas, palabra completa y confirmación una por una | Edición | alto | M | hecho |
| 9 | Mostrar espacios al final y tabs mezclados con espacios, y limpiarlos al guardar (opcional) | Edición | bajo | S | hecho |
| 10 | Plegar funciones y bloques | Edición | bajo | M | falta |
| 11 | Ir a la definición de una función del proyecto (F12 o Ctrl+clic) y volver (Alt+←) | Navegación | alto | M | hecho |
| 12 | Lista de funciones del archivo (esquema) en la barra izquierda, con salto al hacer clic | Navegación | medio | M | hecho |
| 13 | Buscar en todos los archivos del proyecto (Ctrl+Shift+F) con los resultados en un panel | Navegación | alto | M | hecho |
| 14 | Marcadores de línea para volver rápido a un lugar del código (poner, listar, saltar) | Navegación | medio | S | hecho |
| 15 | Teclas para ir al problema siguiente y al anterior sin abrir el panel de Problemas | Navegación | medio | S | hecho |
| 16 | Marcar el primer error de una cascada como «empezá por acá» y atenuar los que dependen de él | Compilación | alto | S | hecho |
| 17 | Arreglo rápido de errores simples (falta `;`, falta `#include`, `=` por `==`) con una tecla | Compilación | alto | M | falta |
| 18 | Subrayar la columna exacta del error en el editor, no sólo marcar la línea | Compilación | medio | S | hecho |
| 19 | Compilar al guardar (opcional) para ver los errores sin apretar F6 | Compilación | medio | S | falta |
| 20 | Reconocer los mensajes de gcc en español sin forzar `LC_MESSAGES=C` | Compilación | bajo | M | hecho |
| 21 | Entrada del programa desde un archivo (`< entrada.txt`) elegible por ejercicio, con varios casos | Ejecución | alto | M | hecho |
| 22 | Comparar la salida del programa con la salida esperada de la consigna (diff línea a línea) | Ejecución | alto | M | hecho |
| 23 | Recordar los argumentos de línea de comandos de cada ejercicio | Ejecución | medio | S | hecho |
| 24 | Avisar si el programa parece colgado (sin salida ni entrada por N segundos) y ofrecer cortarlo | Ejecución | medio | S | hecho |
| 25 | Mostrar el tiempo de ejecución y la memoria máxima al terminar el programa | Ejecución | bajo | S | hecho |
| 26 | Correr sólo la prueba en la que está el cursor | Pruebas | medio | M | hecho |
| 27 | Marcas ✓/✗ en el margen de cada `TEST()` según la última corrida | Pruebas | medio | M | falta |
| 28 | Crear el esqueleto de una prueba (`TEST` + `RUN_TEST`) para la función bajo el cursor | Pruebas | medio | S | falta |
| 29 | Expresiones vigiladas en el depurador que se recalculan en cada paso | Depuración | alto | M | falta |
| 30 | Ver arreglos como tabla y cadenas como texto en el panel de variables | Depuración | alto | M | falta |
| 31 | Valor de una variable al pasar el mouse o el cursor sobre ella mientras se depura | Depuración | medio | M | falta |
| 32 | Correr hasta el cursor, sin poner un breakpoint | Depuración | medio | S | hecho |
| 33 | Breakpoints condicionales (`i == 5`) y por cantidad de pasadas | Depuración | medio | M | falta |
| 34 | Revisar la memoria de programas que leen de la entrada: valgrind en la terminal y el informe al terminar | Memoria | alto | M | falta |
| 35 | Memoria en Windows: AddressSanitizer donde esté disponible, o cat-scan como alternativa guiada | Memoria | medio | L | falta |
| 36 | Para cada fuga, señalar dónde se pierde la última referencia al bloque | Memoria | bajo | L | falta |
| 37 | Ver el diff de lo que se va a confirmar dentro del diálogo de commit | Git | medio | S | falta |
| 38 | Revisar el mensaje de commit (no vacío, primera línea corta) con una plantilla | Git | bajo | S | hecho |
| 39 | Push y pull desde el panel de git, con el estado adelante/atrás y qué hacer ante un conflicto | Git | medio | M | hecho |
| 40 | Recordar hacer commit tras un rato con cambios sin confirmar | Git | bajo | S | hecho |
| 41 | Unificar F1 de ripley, gaff y las demás herramientas con el formato «así no / así sí» de gcc | Aprendizaje | medio | M | falta |
| 42 | Ejemplos en el quick-doc que se copian al editor con una tecla | Aprendizaje | medio | S | falta |
| 43 | Glosario de C (puntero, heap, stack, segfault, fuga…) enlazado desde las explicaciones | Aprendizaje | bajo | M | falta |
| 44 | Recorrido guiado la primera vez (marcar en pantalla dónde está cada panel) | Aprendizaje | bajo | M | falta |
| 45 | Maximizar temporalmente el panel actual y volver (alternar con una tecla) | Interfaz | medio | S | hecho |
| 46 | Tema apto para daltonismo (sin depender de rojo/verde para errores y aciertos) | Interfaz | medio | S | falta |
| 47 | Atajos configurables en `.jonesy.toml` (por ejemplo, para terminales que no pasan F-keys) | Interfaz | bajo | M | falta |
| 48 | Mostrar en la barra cuándo fue el último autoguardado del examen | Examen | medio | S | hecho |
| 49 | Simulacro de examen: modo práctica con cronómetro y entrega sin sellar | Examen | medio | M | hecho |
| 50 | Avisar cuando hay una versión nueva de jonesy y cómo actualizar (`cargo install …`) | Mantenimiento | bajo | S | falta |

## Lo que queda (fuera del perfil estudiante)

Todo lo que se puede resolver dentro de este repositorio y en esta máquina está hecho.
Queda:

| Ítem | Estado | Qué hace falta |
|---|---|---|
| `corbel proyecto` y objetivo `doc` en plantilla-TP | externo | implementar `corbel/especificacion_documentacion_proyecto.md` (fases 1 a 5) en corbel y sumar `make doc` a plantilla-TP. jonesy ya lee `doc/api.json` y avisa qué correr. |
| dredd verifica la bitácora | externo | que dredd llame a `jonesy verificar <paquete.tar> --json` y use `ok`/`chequeos` (contrato en [`docs/entrega.md`](docs/entrega.md)). Es un cambio en el repositorio de dredd. |
| Prueba manual en Windows | externo | nadie lo corrió en una máquina con Windows. La CI compila, pasa clippy y las pruebas en `windows-latest`. |
| Verificación local para Windows | externo | no hay target de Windows en esta máquina. Para tenerlo: `sudo dnf install rust-std-static-x86_64-pc-windows-gnu` y `cargo check --target x86_64-pc-windows-gnu`. |
| E/S del programa depurado en Windows | flojo | sin PTY separado: la salida del programa se mezcla con la consola de GDB. Requiere ConPTY para el inferior; conviene encararlo con la prueba manual. |
| Shell y comandos en Windows | flojo | se usa `sh` si está (UCRT64/MSYS2, Git Bash) y si no `cmd /C`, con comillas de cmd. Los comandos de fábrica (`make`, `./programa`) suponen el entorno UCRT64 de `entorno`. |
| MSRV | externo | `rust-version` 1.85 se comprueba sólo en la CI (acá está el Rust de dnf). |
| ripley tarda ~10 s por archivo | externo | jonesy lo corre en segundo plano, con indicador y 1,5 s de espera tras el último guardado; la latencia es de ripley. |

## Decisiones tomadas

| Tema | Decisión |
|---|---|
| Carga al guardar | gaff, kaneda y spunkmeyer (rápidas, ~0,3 s) corren en cada guardado; ripley (`lento = true`) espera 1,5 s sin guardados nuevos y se cancela si llega otro. Lo repetido se muestra una vez por regla y línea, prefiriendo la versión con categoría y enlace. |
| Herramientas en examen | corren todas salvo las de `examen = false` (callahan). Se ocultan las acciones que corrigen el código (`modo = "recargar"`: `gaff fix`, `corbel scaffold`). Cada acción puede fijarlo con `examen = true/false` en su manifiesto. |
| dredd | no es una acción del IDE: el contrato es `jonesy verificar` (hash del paquete, archivos contra el manifiesto, cadena y sello de la bitácora, sesión cerrada). |
| Integraciones o plugins Python | conviven: integración para una herramienta CLI existente, plugin para lógica propia chica (tabla en `docs/integraciones.md`). |
| Documentación del proyecto | `doc/` no se versiona; se genera con `make doc` y el IDE lo indica. Sólo se avisa en proyectos que la usan (un Makefile con objetivo `doc`, `corbel.toml` o `doc/` presente); `[documentacion] aviso` lo fuerza o lo apaga. |

## Hecho en la última tanda

| Ítem | Requisito | Cómo quedó |
|---|---|---|
| Tráfico en reposo 0 B/s | §10 / RNF-A.3 | sólo se redibuja si cambió algo visible: 0 bytes en 10 s de reposo (antes 160). |
| Enlaces de Markdown con teclado | RF-8.3 | Tab/Shift+Tab los recorre en la consigna y la vista previa, Enter los abre; las anclas internas saltan dentro del documento. |
| cat-scan: eventos y fugas | RF-4.3 | muestra los eventos del paso (doble free, uso tras free) y el resumen final de fugas de mem-trace. |
| cat-scan en ejercicios con Makefile | RF-4.3 | mem-trace compila el módulo con sus bibliotecas e includes. |
| Recuperación tras un corte | RF-6.3 | al reabrir se ofrece restaurar las copias de `.jonesy/recuperacion` (s restaura, n descarta, Esc las conserva). |
| Shell de cátedra | RF-5.3 | detecta `entorno` (`P1_ENTORNO`, `~/p1/entorno`, `PORTABLE_ROOT`), aplica sus variables y `bin` al PATH y usa su gdbinit. `jonesy doctor` lo informa. |
| Verificar una entrega | RF-6.5 / RNF-D.3 | `jonesy verificar <paquete.tar> [--json]`. |
| Explorador: renombrar y borrar | — | F2 y Supr (no en examen). |
| Dependencias entre bibliotecas | — | F6 compila las dependencias transitivas (`DEPENDENCIAS`/`LIB_DEPS` del Makefile y `dependencies` de library.json). |
| Diff: resaltado, líneas largas, binarios, renombres | — | sintaxis por lenguaje, ←/→ desplaza, binarios y renombres con su propio bloque. |
| Diff entre dos commits | — | `m` marca un commit en el log y Enter sobre otro los compara. |
| Aviso de documentación | — | sólo en proyectos que la usan; `[documentacion] aviso` lo fuerza o lo apaga. |
| hal, vasquez, holden | — | manifiestos de fábrica (hal y vasquez en la terminal, holden en el panel). |
| Integraciones en Windows | — | comillas según el intérprete (`cmd` o `sh`). |
| Descripciones de objetivos en la barra | — | barra izquierda de 32 columnas por defecto. |
| Gráfico de velocidad de tipeo | Could | en `jonesy replay` (tipeado y pegado por minuto) y en `--resumen`. |
| Exportar la traza de memoria | Could | genera `memoria-<programa>.html` y lo abre (o copia la ruta por OSC 52). |
| Editor de snippets para plugins | Could | Configuración del entorno → Plugins → Nuevo plugin parte de una plantilla; guardarlo lo recarga y sus errores (sintaxis, excepciones) aparecen en Problemas en su línea. |
| Temas | Could | oscuro, claro, contraste (16 colores ANSI), turbo (Turbo C++ de Borland), solarizado y fósforo verde; `[interfaz] tema` o Archivo → Configuración del entorno → Apariencia, recordado en la sesión. |
| Deshacer el último commit | Could | `z` en el panel de git (`reset --soft HEAD~1`); se niega si el commit ya se subió. |
| Prueba del corte por SIGHUP | §12.6 | `crates/jonesy-bin/tests/corte.rs`: el binario real en una PTY, con y sin examen. |
| Estrés de 100 sesiones | §12.5 | `bin/estres-sesiones`: 0,55 GiB y ~0 % de CPU con 100 sesiones en 16 núcleos; la CI lo corre con 20. |
| Pruebas del menú con mouse y del compacto con barra | — | en `crates/jonesy-tui/tests/app.rs`. |

## Hecho antes

Paneles redimensionables, Archivo → Nuevo/Abrir proyecto, clonar, explorador,
autoformato, pestañas, estructura del proyecto (Makefiles, library.json, tp.sh),
resaltado de Makefiles, integraciones con el ecosistema, diff lado a lado,
documentación del proyecto en el quick-doc, botón Abrir en Terminal (RF-11.5), CI
en Linux/Windows/macOS. RF-3.1 (ripley vía worker Python) y §10.2 (reconexión) no
aplican: ripley corre como integración y jonesy no tiene modo servidor.

## Proceso

| Ítem | Estado | Detalle |
|---|---|---|
| Commits con una prueba fallando | hecho | `95f8d41` (corregido en `0ce85fb`), `aa44c4b` (en `38e606c`) y `52d623c` (bienvenida, en `7f36109`): el chequeo previo encadenado con `\|` o `;` perdía el código de error. Los commits se hacen sólo si fmt, clippy y las pruebas devuelven 0, sin pasar la salida por otro comando. `7f36109` además aísla el estado de usuario (`JONESY_ESTADO`) en las pruebas. |
