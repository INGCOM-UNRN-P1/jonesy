# Pendientes de jonesy

Rastro de lo que falta o está flojo respecto de [`jonesy_spec.md`](jonesy_spec.md) y de
los pedidos posteriores. Cada ítem dice su estado y, si aplica, el requisito.

Estados: **falta** (no está), **flojo** (está pero incompleto o sin probar),
**en curso**, **hecho** (se deja unos días para referencia y después se borra).

## Pedidos en curso

| Ítem | Estado | Notas |
|---|---|---|
| Paneles redimensionables con mouse y desde opciones | en curso | todos los separadores; tamaños en `.jonesy.toml` y en la sesión |
| Archivo → Nuevo archivo, Abrir proyecto/directorio | en curso | |
| Git → Clonar repositorio | en curso | corre en el deck para que funcione la autenticación interactiva |
| Explorador de directorios | en curso | |
| Editar → Autoformato | en curso | `gaff format` o `clang-format`, configurable |
| Pestañas para varios archivos | en curso | cierre con ×, desborde, Ctrl+Tab, abrir varios a la vez |
| Soporte de Windows | en curso | ver sección Windows |
| Más verificaciones en la CI | en curso | matriz de SO, MSRV, docs, shellcheck, artefactos |

## Requisitos de la especificación

| Requisito | Estado | Detalle |
|---|---|---|
| §10 / RNF-A.3 tráfico en reposo 0 B/s | falta | en reposo redibuja cada 2 s (sondeo de git) y ratatui reescribe el cursor: 32 B cada 2 s. Redibujar sólo si cambió algo. |
| RF-8.3 enlaces con Enter | flojo | sólo se abren con clic; falta navegarlos con teclado y abrirlos en el panel adyacente en vez de reemplazar el editor. |
| RF-11.5 botón `[ Abrir en Terminal ]` en git | falta | Alt+T funciona; el botón en el panel no está. |
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
| Compilación | en curso | señales, PTY del inferior de GDB y `libc` quedan detrás de `cfg(unix)`. |
| Verificación local | falta | no hay target de Windows en esta máquina; se verifica sólo en la CI. Para chequear localmente: `sudo dnf install rust-std-static-x86_64-pc-windows-gnu` y `cargo check --target x86_64-pc-windows-gnu`. |
| E/S del programa depurado | flojo | sin PTY separado en Windows: la salida del programa se mezcla con la consola de GDB. |
| Shell y comandos | flojo | se usa `sh` si está en el `PATH` (UCRT64/MSYS2, Git Bash) y si no `cmd /C`; los comandos por defecto (`make`, `./programa`) suponen el entorno UCRT64 de `entorno`. |
| Prueba manual en Windows | falta | nadie lo corrió todavía en una máquina con Windows. |

## Calidad

| Ítem | Estado | Detalle |
|---|---|---|
| Pruebas de la TUI | flojo | cubren flujos principales con TestBackend; no hay pruebas de mouse sobre separadores ni del explorador. |
| ripley tarda ~10 s por archivo | flojo | jonesy lo corre en segundo plano con indicador y cola; la latencia es de ripley. |
| MSRV | flojo | `rust-version` declarada sin verificar; la CI la comprueba. |
