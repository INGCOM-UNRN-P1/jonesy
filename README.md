# jonesy

IDE de terminal para la cátedra de Programación I: un único binario en Rust
(ratatui + crossterm) para escribir, compilar, depurar y entregar C en el
laboratorio, en un servidor por SSH o en el examen, desde terminales de 80×24.

- Editor sobre cuerda (`ropey`) con atajos CUA y teclas de función, mouse,
  resaltado de C, Markdown, YAML y TOML, búsqueda, deshacer agrupado y paleta
  de comandos.
- **F6** compila con el Makefile o gcc y salta a los errores; **Ctrl+F5** corre
  el programa en la terminal interna (`deck`, PTY real con vt100).
- **ripley** al guardar: observaciones como texto virtual, panel de reglas por
  categoría y **F1** para la explicación didáctica de la regla.
- **Depurador** visual sobre GDB/MI con breakpoints en el margen, paso a paso,
  pila y variables, y **cat-scan**: stack y heap dibujados con flechas a
  partir de la traza de `mem-trace`, con punteros colgantes en rojo.
- **quick-doc**: firma, retorno, parámetros, trampas y reglas de cátedra de la
  función bajo el cursor (82 funciones de libc/POSIX embebidas, sin red),
  resaltando el parámetro que se está escribiendo.
- **Git** desde el panel lateral: stage, diff, commit y log.
- **Modo examen**: consigna de solo lectura, cronómetro, autoguardado,
  telemetría forense encadenada con SHA-256 y entrega asistida con paquete y
  hash; `jonesy replay` para auditar la sesión.
- **Extensiones** nativas en Rust y plugins Python en un proceso aparte, con
  tiempo límite y reinicio.

La especificación completa está en [`jonesy_spec.md`](jonesy_spec.md).

## Instalación

```bash
cargo build --release
install -m 755 target/release/jonesy ~/.local/bin/
```

Requiere Rust ≥ 1.80. En tiempo de ejecución usa lo que haya en el `PATH`:
`gcc`/`make` para compilar, `gdb` para depurar, `git`, `ripley` y `mem-trace`
(si faltan, esas funciones se desactivan solas) y `python3` sólo si el
proyecto tiene plugins.

## Uso

```bash
jonesy                    # proyecto en el directorio actual
jonesy ~/tp3              # otro directorio
jonesy lista.c lista.h    # archivos; la raíz es el ancestro con .jonesy.toml o .git
jonesy --exam             # modo examen (también JONESY_EXAM_MODE=1)
jonesy --ascii            # sin caracteres de caja (automático si el locale no es UTF-8)
jonesy --telemetria       # bitácora también fuera del examen

jonesy replay .jonesy/telemetry.log            # reproductor interactivo
jonesy replay .jonesy/telemetry.log --resumen  # resumen de auditoría
jonesy replay .jonesy/telemetry.log --json
jonesy verificar .jonesy/telemetry.log         # código 1 si la cadena está rota
```

### Atajos principales

| Acción | Teclas |
|---|---|
| Guardar · deshacer · rehacer | Ctrl+S / F2 · Ctrl+Z · Ctrl+Y |
| Copiar · cortar · pegar | Ctrl+C · Ctrl+X · Ctrl+V (OSC 52 al copiar) |
| Buscar · ir a línea · abrir | F3 / Ctrl+F · Ctrl+G · Ctrl+O |
| Paleta de comandos | Ctrl+K o F1 (F1 sobre un diagnóstico explica la regla) |
| Menú | F10 (fuera de la depuración), Alt+letra o clic |
| Compilar · ejecutar | F6 · Ctrl+F5 |
| Depurar | F5 continuar · F10 paso · F11 entrar · Shift+F11 salir · Shift+F5 detener · Ctrl+B breakpoint |
| Memoria (cat-scan) | Alt+M |
| quick-doc | F4 / Alt+D; Shift+F4 alterna derecha ⟷ inferior |
| Paneles | Alt+T terminal · Alt+P problemas · Alt+O salida · Alt+G git · Alt+R reglas |
| Vista previa de Markdown | Ctrl+P |
| Modo compacto (≤ 89 col.) | F7 consigna · F8 código · F9 terminal · F4 doc |
| Entregar examen | F12 o el botón `[ Entregar Examen ]` |

En la terminal interna, `Alt+T` o `Esc Esc` devuelven el foco al editor y
`Shift+RePág/AvPág` recorre el historial. La tabla completa está en
*Ayuda → Atajos de teclado*.

### Layout adaptativo

| Tamaño | Modo | Distribución |
|---|---|---|
| ≥ 120 × 28 | espacioso | consigna 25 % · editor 50 % · quick-doc 25 % (o dock inferior) |
| 90–119 col. | estándar | consigna 35 % · editor 65 %; quick-doc en dock o flotante |
| ≤ 89 col. o ≤ 24 filas | compacto | una vista a pantalla completa (F7/F8/F9/F4) |

Los separadores se arrastran con el mouse. La sesión (archivos, cursores,
paneles, breakpoints) se guarda en `.jonesy/session.json`.

## Configuración

Opcional, en `.jonesy.toml` en la raíz del proyecto. Ver
[`docs/ejemplo.jonesy.toml`](docs/ejemplo.jonesy.toml). Sin configuración:
F6 corre `make` (o `gcc -std=c11 -Wall -Wextra -g -O0` sobre el `.c` activo),
Ctrl+F5 corre `make run` (o compila y ejecuta) y la entrega verifica con
`make test`.

## Modo examen

- La consigna (`consigna.md`, `enunciado.md` o la configurada) se muestra
  formateada y es de solo lectura, igual que `.jonesy.toml` y `.jonesy/`.
- Cronómetro en la barra superior que sobrevive a reinicios
  (`.jonesy/examen.json`), con avisos a los 30, 15 y 5 minutos.
- Autoguardado atómico cada 30 s, al perder el foco de la terminal y ante
  SIGHUP/SIGINT/SIGTERM.
- Sólo se cargan los plugins cuyo SHA-256 figura en
  `examen.plugins_permitidos`.
- **Entrega**: guarda todo, corre la verificación, cierra la bitácora con
  `SessionEnd` y arma `.jonesy/entrega/entrega-<usuario>-<ts>.tar` con los
  fuentes, `telemetry.log` y `entrega.json` (resultado de la verificación,
  último hash de la cadena, SHA-256 de cada archivo). El SHA-256 del paquete
  se muestra y se deja en `<paquete>.tar.sha256`. Después los archivos quedan
  de solo lectura.

### Telemetría

`.jonesy/telemetry.log` es JSON Lines append-only: cada registro tiene `seq`,
`ts_micros`, `prev_hash` (SHA-256 de la línea anterior; la primera usa 64
ceros), `event` y `payload_hash` (SHA-256 del JSON canónico del evento).
Registra `SessionStart`, `TextDelta`, `BracketedPaste` (longitud y hash de lo
pegado), `CursorMove` (cada ≥ 200 ms), `KeyPress` (teclas de control; el texto
ya está en `TextDelta`), `BufferSave`, `CommandExec`, `FocusChange` y
`SessionEnd`. Se escribe en lotes de 32 eventos o 500 ms desde un hilo propio;
guardados, cambios de foco e inicio/fin de sesión fuerzan `flush` +
`sync_data`. `jonesy verificar` recorre la cadena y `jonesy replay` reconstruye
el texto evento por evento marcando lo pegado.

## Extensiones

- **Rust** (compiladas en el binario): traits `EditorPlugin`,
  `DiagnosticProvider`, `WidgetView` y `TelemetryHook` de `jonesy-plugins`,
  registrados en `Registro::de_fabrica()`.
- **Python**: archivos `.jonesy/plugins/*.py` con `al_guardar(archivo, texto)`
  (devuelve diagnósticos) y funciones `comando_<nombre>(ctx)` que aparecen en
  la paleta. Ver [`docs/plugin-ejemplo.py`](docs/plugin-ejemplo.py). El
  worker (`tools/jonesy-worker-py`, embebido en el binario) sólo se lanza si
  hay plugins, habla JSON-RPC por stdin/stdout y se reinicia si una llamada
  supera los 2 s o el proceso muere. `JONESY_PYTHON` elige el intérprete.

## Arquitectura

| Crate | Contenido |
|---|---|
| `jonesy-core` | buffers, proyecto y `.jonesy.toml`, sesión, procesos, diagnósticos de gcc y ripley, controlador GDB/MI, modelo y dibujo de cat-scan, paquete de entrega |
| `jonesy-tui` | aplicación, layout §9.2, dibujo, resaltado, Markdown, paleta, reproductor |
| `jonesy-doc` | base offline de quick-doc (`data/libc.toml`) y lexer de contexto de llamada |
| `jonesy-terminal` | deck: PTY (`portable-pty`) + pantalla virtual `vt100`; PTY del inferior de GDB |
| `jonesy-telemetry` | bitácora encadenada, verificación y replay |
| `jonesy-git` | estado, stage, commit, log y diff vía el binario `git` |
| `jonesy-plugins` | traits nativos y supervisor del worker Python |
| `jonesy-bin` | CLI y benchmarks |

## Rendimiento (§10)

`bin/bench-tui [proyecto]` corre todos los presupuestos y sale con código 1
si alguno se excede. Medido en release (Fedora 44, x86-64):

| Métrica | Presupuesto | Medido |
|---|---|---|
| Arranque (proyecto + primer cuadro) | ≤ 50 ms | 18 ms |
| RSS: consigna + C + git + quick-doc + terminal | ≤ 30 MiB | 5,6 MiB |
| RSS con worker Python | ≤ 50 MiB | 15,6 MiB |
| quick-doc (detección + documentación) | ≤ 70 ms | 0,02 ms |
| Telemetría por evento (p99) | ≤ 1 ms | 0,002 ms |
| `git status` con 500 archivos | ≤ 30 ms | 3,3 ms |
| Primer render de Markdown (la especificación entera) | ≤ 15 ms | 2,9 ms |
| Tamaño del binario | ≤ 25 MiB | 3,2 MiB |

En reposo el bucle duerme en el canal de avisos: sin eventos sólo despierta
cada 2 s para mirar las fechas de `.git/HEAD` e índice (y cada 1 s en examen,
por el cronómetro). `/usr/bin/time` suma los procesos hijos: con un `.bashrc`
pesado la shell del deck domina la cifra; `SHELL=/bin/sh` aísla la del IDE.

## Desvíos respecto de la especificación

- **Hilos y `std::sync::mpsc` en lugar de tokio**: el bucle, la telemetría,
  los procesos, el deck, GDB y los plugins usan hilos dedicados y un canal
  único de avisos. Cumple lo mismo (no bloquear el teclado) sin runtime
  asíncrono ni dependencias extra.
- **Verificación de la entrega**: `dredd eval --local` no existe; la entrega
  corre `examen.verificar` o, por defecto, `make test`.
- **Reglas de ripley**: se muestran los códigos reales de cátedra
  (`0x3001h`, …) en lugar de los `P1-MEM-01` de ejemplo de la especificación,
  y las categorías del panel salen del primer dígito del código.
- **cat-scan** no lee un flujo en vivo de mem-trace: al iniciar la depuración
  corre `mem-trace run --outdir .jonesy/memtrace` en segundo plano y alinea
  cada parada de GDB con el paso de la traza por función, línea y
  profundidad. Sólo para programas de un archivo sin Makefile.
- **Detección de cambios de git** por sondeo de las fechas de `.git/HEAD` e
  índice cada 2 s (sin inotify), además de refrescar tras cada guardado o
  acción del panel.
- **Ctrl+Enter** no llega como tal en muchas terminales: el commit se confirma
  también con Alt+Enter o Ctrl+S.
- **Reconexión de sesión ≤ 20 ms** (§10.2) no aplica: jonesy no tiene modo
  servidor; la sesión se restaura de `.jonesy/session.json` al arrancar.
- La telemetría está activa siempre en examen y, fuera de él, sólo con
  `--telemetria`.

## Licencia

GPL-3.0-or-later. Ver [LICENSE](LICENSE).
