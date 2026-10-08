# Pendientes de jonesy

Rastro de lo que falta o está flojo respecto de [`jonesy_spec.md`](jonesy_spec.md) y de
los pedidos posteriores. Cada ítem dice su estado y, si aplica, el requisito.

Estados: **falta** (no está), **flojo** (está pero incompleto o sin probar),
**en curso**, **hecho** (se deja unos días para referencia y después se borra).

## Pedidos recientes

| Ítem | Estado | Notas |
|---|---|---|
| Paneles redimensionables con mouse y desde opciones | hecho | todos los separadores; `[interfaz.paneles]` y sesión |
| Archivo → Nuevo archivo, Abrir proyecto/directorio | hecho | con proyectos recientes |
| Git → Clonar repositorio | hecho | corre en el deck para la autenticación interactiva |
| Explorador de directorios | hecho | barra izquierda, pestaña Archivos |
| Editar → Autoformato | hecho | `gaff format` o `clang-format`, configurable |
| Pestañas para varios archivos | hecho | ×, clic del medio, desborde, Ctrl+Tab, Alt+1…9, abrir varios |
| Estructura del proyecto (Makefiles, library.json, tp.sh) | hecho | pestaña Proyecto; F6/Ctrl+F5/F5 por módulo |
| Soporte de Windows | flojo | ver sección Windows |
| Más verificaciones en la CI | hecho | matriz Linux/Windows/macOS, MSRV, docs, shellcheck, worker, audit, binarios |
| Explorador: renombrar y borrar archivos | falta | hoy sólo crea archivos y carpetas |
| Estructura: dependencias entre bibliotecas | falta | F6 compila las bibliotecas de `LIB_NAME` de un ejercicio, pero no las dependencias de una biblioteca con otra |
| Resaltado de Makefiles | hecho | directivas, variables, objetivos, recetas; TAB visible y receta con espacios marcada en rojo |
| Diff lado a lado | hecho | panel de git (archivos y commits) y buffer actual contra HEAD (Alt+Shift+D); `s` alterna con unificado |
| quick-doc con la documentación del proyecto | hecho | busca `doc/api.json` (corbel), lo muestra en el quick-doc y avisa con un botón `make doc` si falta o quedó viejo (RF-9 de la especificación de corbel). Falta que corbel implemente `corbel proyecto` y que plantilla-TP tenga el objetivo `doc` |
| Diff: resaltado de sintaxis y líneas largas | flojo | el diff no colorea C y las líneas que no entran se cortan (sin desplazamiento horizontal) |
| Diff: archivos binarios y renombres | flojo | un binario aparece como diferencias de texto vacías; un renombre se ve como borrado + nuevo |
| Diff: entre dos commits cualesquiera | falta | hoy: archivo contra índice/HEAD, buffer contra HEAD y cada commit del log contra su padre |
| Aviso de documentación con corbel instalado | flojo | si corbel está en el PATH, `[sin doc: corbel]` aparece en cualquier proyecto sin `doc/api.json`, aunque nadie use corbel ahí; se podría silenciar por proyecto |
| `corbel proyecto` y objetivo `doc` en plantilla-TP | falta | dependen de implementar `corbel/especificacion_documentacion_proyecto.md` (fases 1 a 5) |
| Descripciones de objetivos en la barra | flojo | con 28 columnas se cortan; el pie muestra la completa. Se puede ensanchar la barra |

## Requisitos de la especificación

| Requisito | Estado | Detalle |
|---|---|---|
| §10 / RNF-A.3 tráfico en reposo 0 B/s | falta | en reposo redibuja cada 2 s (sondeo de git) y ratatui reescribe el cursor: 32 B cada 2 s. Redibujar sólo si cambió algo. |
| RF-8.3 enlaces con Enter | flojo | sólo se abren con clic; falta navegarlos con teclado y abrirlos en el panel adyacente en vez de reemplazar el editor. |
| RF-11.5 botón `[ Abrir en Terminal ]` en git | hecho | botones [Commit] [Log] [Clonar] [Abrir en Terminal]. |
| RF-4.3 accesos inválidos en cat-scan | flojo | marca punteros colgantes; no muestra `events` ni `meta.warnings` de mem-trace (accesos tras `free`, fugas al final). |
| RF-4.3 cat-scan con Makefile | flojo | mem-trace sólo se lanza para programas de un archivo sin Makefile. |
| RF-6.3 recuperación | flojo | las copias de `.jonesy/recuperacion/` se escriben ante SIGHUP fuera del examen, pero al reabrir no se ofrece restaurarlas. |
| RF-5.3 shell de cátedra | falta | el deck abre `$SHELL`; no toma las variables de `entorno` (`PORTABLE_ROOT`, rutas de bibliotecas) ni `entorno/config/gdb`. |
| RF-6.5 / RNF-D.3 dredd | falta | la entrega verifica con `make test` (no existe `dredd eval --local`) y dredd no verifica la cadena de `telemetry.log`. Requiere cambios en dredd o integrar con `entorno entregar`/`verificar`. Decidir cuál. |
| RF-3.1 ripley vía worker Python | falta | opcional en la spec; hoy siempre por CLI. |
| §12.5 estrés de 100 sesiones | falta | no hay script ni medición (≤ 5,5 GiB y < 1,5 % de CPU). |
| §12.6 corte por SIGHUP | flojo | implementado (guarda, sella y restaura la sesión) pero sin prueba automática. |
| §10.2 reconexión de sesión ≤ 20 ms | no aplica | jonesy no tiene modo servidor. |

## Could (MoSCoW)

| Ítem | Estado |
|---|---|
| Gráfico de velocidad de tipeo para el docente | falta |
| Exportar la traza de memoria a archivo | falta (el HTML de mem-trace queda en `.jonesy/memtrace/`, pero no hay acción para abrirlo o copiarlo) |
| Editor interactivo de snippets para plugins Python | falta |
| Temas visuales adicionales | falta |
| Deshacer el último commit (`git reset --soft HEAD~1`) | falta |

## Windows

| Ítem | Estado | Detalle |
|---|---|---|
| Compilación | flojo | señales, PTY del inferior de GDB y `libc` quedan detrás de `cfg(unix)`; la CI compila y prueba en Windows, pero no se corrió todavía. |
| Verificación local | falta | no hay target de Windows en esta máquina; se verifica sólo en la CI. Para chequear localmente: `sudo dnf install rust-std-static-x86_64-pc-windows-gnu` y `cargo check --target x86_64-pc-windows-gnu`. |
| E/S del programa depurado | flojo | sin PTY separado en Windows: la salida del programa se mezcla con la consola de GDB. |
| Shell y comandos | flojo | se usa `sh` si está en el `PATH` (UCRT64/MSYS2, Git Bash) y si no `cmd /C`; los comandos por defecto (`make`, `./programa`) suponen el entorno UCRT64 de `entorno`. |
| Prueba manual en Windows | falta | nadie lo corrió todavía en una máquina con Windows. |

## Calidad

| Ítem | Estado | Detalle |
|---|---|---|
| Pruebas de la TUI | flojo | cubren flujos principales con TestBackend (incluidos arrastre de separadores, explorador, proyecto, clonar, diff y documentación); faltan pruebas del menú con mouse y del modo compacto con la barra izquierda. |
| ripley tarda ~10 s por archivo | flojo | jonesy lo corre en segundo plano con indicador y cola; la latencia es de ripley. |
| MSRV | flojo | `rust-version` 1.85 declarada sin compilar con esa versión; la CI la comprueba. |
| Commit con una prueba fallando | hecho | `95f8d41` se commiteó con la prueba del layout fallando; la corrigió `0ce85fb`. Los commits se hacen ahora sólo si fmt, clippy y las pruebas pasan. |
