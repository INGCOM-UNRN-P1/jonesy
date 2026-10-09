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
| Primera vez | falta | una pantalla de bienvenida corta (F6, Ctrl+F5, Ctrl+F6, F5, F1) para quien nunca usó el IDE. |
| Explicaciones de errores de ripley/gaff | flojo | F1 depende de que cada herramienta tenga `explain`; unificar el formato con el de los errores de gcc. |
| Menús reorganizados | hecho | según `docs/critica_y_propuesta_de_reorganizacion.md`: Construir y Depurar separados, Herramientas pasa a Archivo → Configuración del entorno (integraciones, plugins, compilador y formato, apariencia), separadores, Ver con los modos F7/F8/F9 primero, sin atajos de panel en el menú Git, guía rápida, limpiar compilación, restablecer paneles y estado de la entrega. Letra subrayada en cada menú: F10 y la letra abre cualquiera; Alt+letra directo salvo Editar, Git y Ayuda (Alt+E, Alt+G, Alt+Y son de paneles). |
| Menú Plugins | hecho | Alt+L: los comandos de los plugins Python y las acciones de las herramientas para el archivo activo, ejecutables desde el menú; los comandos se piden al worker al abrirlo. |
| Ctrl+, para la configuración | flojo | sólo llega en terminales con el protocolo de teclado extendido; en el resto se usa Archivo → Configuración del entorno. |
| Pruebas: salida de otros frameworks | falta | sólo se interpreta `p1_test`; con otro formato se ve la salida cruda. |

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
