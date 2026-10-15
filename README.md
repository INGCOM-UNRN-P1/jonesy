# jonesy

IDE de terminal para la cátedra de Programación I: un único binario en Rust
(ratatui + crossterm) para escribir, compilar, depurar y entregar C en el
laboratorio, en un servidor por SSH o en el examen, desde terminales de 80×24.

- Editor sobre cuerda (`ropey`) con atajos CUA y teclas de función, mouse,
  resaltado de C, Markdown, YAML y TOML, búsqueda, deshacer agrupado y paleta de
  comandos.
- **F6** compila con el Makefile o gcc y salta a los errores, **explicados en
  español** (unos 60 errores y advertencias comunes de gcc y del enlazador, con
  ejemplo en F1); con **Alt+B** (o el botón «compilar al guardar» de la barra de
  estado) cada Ctrl+S de un `.c` o `.h` también compila, sin mover el foco;
  **Ctrl+F5** corre el programa en la terminal interna (`deck`,
  PTY real con vt100) y, si se corta (violación de segmento, abort…), explica
  por qué y **F5** lo depura hasta la línea donde falló; **Ctrl+F6** corre las
  pruebas (`make test` con `p1_test`, u otro framework que informe en **SARIF**
  con `[pruebas] comando` y `sarif`) y deja cada fallo en su línea con lo
  esperado y lo obtenido; **Depurar → Revisar memoria con valgrind** corre
  valgrind (`make memcheck` o el programa, con su entrada por teclado si la
  lee) y explica cada acceso inválido, uso después de `free`, `free` doble y
  fuga en la línea del alumno, con dónde se pidió o liberó el bloque; sin
  valgrind (Windows), lo mismo con AddressSanitizer si el compilador lo trae.
- **Edición y navegación**: pares que se cierran solos, comentar (Ctrl+/), mover
  y duplicar líneas, buscar y reemplazar (Ctrl+H), ir a la definición (F12 o
  Ctrl+clic) y volver (Alt+←), esquema del archivo (Alt+S), buscar en el
  proyecto (Ctrl+Shift+F), marcadores (Ctrl+F2) y problema siguiente (Alt+N).
  Errores en cascada marcados («empezá por acá») y la columna exacta subrayada.
- **Ejecución**: casos de entrada y salida esperada (entrada1.txt ↔ salida1.txt)
  para correr con la entrada desde un archivo o comparar todos; argumentos por
  ejercicio; tiempo y memoria máxima al terminar; aviso de ciclo infinito
  (Shift+F5 lo corta); correr sólo la prueba del cursor (Shift+F6) y correr
  hasta el cursor (Ctrl+F10).
- **Simulacro de examen** (`--simulacro`), push y pull desde el panel de git con
  ayuda ante conflictos, y revisión del mensaje de commit.
- **Herramientas del ecosistema** como plugins: ripley, gaff, kaneda y
  spunkmeyer al guardar (texto virtual, panel por tema, **F1** con la
  explicación y el enlace al apunte) y sus acciones (gaff fix, tetsuo, motoko…)
  con Alt+H.
- **Depurador** visual sobre GDB/MI con breakpoints en el margen, paso a paso,
  pila y variables, y **cat-scan**: stack y heap dibujados con flechas a partir
  de la traza de `mem-trace`, con punteros colgantes en rojo.
- **quick-doc**: firma, retorno, parámetros, trampas y reglas de cátedra de la
  función bajo el cursor (82 funciones de libc/POSIX embebidas, sin red),
  resaltando el parámetro que se está escribiendo.
- **Proyecto**: al abrir un TP lee sus Makefiles (raíz, `libs/*`,
  `ejercicios/*`), `library.json` y `tp.sh`, y muestra bibliotecas, ejercicios y
  qué hace cada objetivo de make, listos para correr. F6, Ctrl+F5 y F5 actúan
  sobre el módulo del archivo activo (compilando antes las bibliotecas que usa).
- **Ejercicios de deckard**: una carpeta del banco (`ejercicio.yaml`,
  `enunciado.md`, `tests/`) o un starter kit (`metadata.json`, `README.md`) se ve
  como la consigna, con el tema, el nivel de Bloom, el tiempo estimado, las
  funciones a implementar y las pistas de a una (`p` en la consigna); los casos de
  `tests/` entran en Comparar con las salidas esperadas y nunca se abre
  `solucion.c`.
- **Explorador de archivos**, pestañas con cierre y desborde, paneles que se
  redimensionan con el mouse o desde las opciones, archivo nuevo, abrir otro
  proyecto (con recientes) y autoformato con `gaff` o `clang-format`.
- **Visor hexadecimal** para ejecutables, `.o` y `.a`: bytes y ASCII, ir a un
  offset, buscar, las secciones del ELF explicadas, los objetos de una
  biblioteca y sus símbolos con `nm`.
- **Git** desde el panel lateral: stage, commit, log y clonar repositorios, con
  diff lado a lado (o unificado) de archivos, commits y del buffer sin guardar.
- **Modo examen**: consigna de solo lectura, cronómetro, autoguardado,
  telemetría forense encadenada con SHA-256 y entrega asistida con paquete y
  hash; `jonesy replay` para auditar la sesión.
- **Extensiones** nativas en Rust y plugins Python en un proceso aparte, con
  tiempo límite y reinicio.

La especificación completa está en [`jonesy_spec.md`](jonesy_spec.md); qué se hace
con jonesy, caso por caso, en [`docs/casos_de_uso.md`](docs/casos_de_uso.md).

## Instalación

Sin compilar: cada [release](https://github.com/INGCOM-UNRN-P1/jonesy/releases)
trae el binario para Linux (`linux-x86_64`, y `linux-x86_64-musl`, estático, para
distribuciones viejas o servidores) y para Windows (`windows-x86_64.zip`), con
`SHA256SUMS` para verificarlo:

```bash
tar -xzf jonesy-0.1.0-linux-x86_64.tar.gz
install -m 755 jonesy-0.1.0-linux-x86_64/jonesy ~/.local/bin/
```

Los releases los arma `.github/workflows/release.yml` al subir un tag `vX.Y.Z` que
coincida con la versión de `Cargo.toml`.

Con Cargo, directo desde el repositorio (queda en `~/.cargo/bin`, que rustup
agrega al `PATH`):

```bash
cargo install --locked --git https://github.com/INGCOM-UNRN-P1/jonesy jonesy-bin
```

Desde una copia local del repositorio:

```bash
cargo install --locked --path crates/jonesy-bin
```

El paquete se llama `jonesy-bin` y el ejecutable, `jonesy`. Para actualizar se
repite el mismo comando (con `--force` si la versión no cambió);
`cargo uninstall jonesy-bin` lo quita. Sin `cargo install`, compilando a mano:

```bash
cargo build --release
install -m 755 target/release/jonesy ~/.local/bin/
```

Requiere Rust ≥ 1.85. En tiempo de ejecución usa lo que haya en el `PATH`:
`gcc`/`make` para compilar, `gdb` para depurar, `git`, `ripley` y `mem-trace`
(si faltan, esas funciones se desactivan solas) y `python3` sólo si el proyecto
tiene plugins.

Después de instalar, `jonesy doctor` dice si falta algo en el entorno. La
primera vez que se abre, jonesy muestra una pantalla de bienvenida con las
teclas del ciclo de trabajo (después, en _Ayuda → Bienvenida_).

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

jonesy doctor                                  # ¿falta algo en el entorno?
jonesy reporte --salida reporte.md             # informe para reportar un problema
jonesy config exportar prefs.toml              # tema, paneles, integraciones…
jonesy config importar prefs.toml              # en otra máquina
```

### Atajos principales

| Acción                                   | Teclas                                                                                        |
| ---------------------------------------- | --------------------------------------------------------------------------------------------- |
| Guardar · deshacer · rehacer             | Ctrl+S / F2 · Ctrl+Z · Ctrl+Y                                                                 |
| Copiar · cortar · pegar                  | Ctrl+C · Ctrl+X · Ctrl+V (OSC 52 al copiar)                                                   |
| Buscar · ir a línea · abrir              | F3 / Ctrl+F · Ctrl+G · Ctrl+O (Tab marca varios)                                              |
| Nuevo archivo · autoformato              | Ctrl+N · Alt+Shift+F                                                                          |
| Autocompletar · arreglo rápido           | Ctrl+Espacio · Alt+Enter (falta «;» o un #include, «=» por «==»)                              |
| Del .c al .h · referencias               | Alt+I · Shift+F12 (dónde se usa en el proyecto)                                               |
| Pestañas                                 | Ctrl+Tab / Ctrl+Shift+Tab, Alt+1…9, Ctrl+W; clic del medio o × cierra                         |
| Explorador · proyecto (make y tp.sh)     | Alt+E · Alt+Y (Tab alterna dentro de la barra)                                                |
| Diff del archivo contra git, lado a lado | Alt+Shift+D (s alterna con unificado, n/p salta entre cambios)                                |
| Paleta de comandos                       | Ctrl+K o F1 (F1 sobre un diagnóstico explica la regla)                                        |
| Menú                                     | Alt+letra subrayada, F10 y la letra, o clic ([docs/menus.md](docs/menus.md))                  |
| Configuración del entorno                | Archivo → Configuración (integraciones, plugins, compilador, apariencia)                      |
| Compilar · ejecutar · probar             | F6 · Ctrl+F5 · Ctrl+F6                                                                        |
| Objetivo de make (ejercicio o general)   | Shift+F9 o Construir → Correr un objetivo de make; también en F1                              |
| Depurar                                  | F5 continuar · F10 paso · F11 entrar · Shift+F11 salir · Shift+F5 detener · Ctrl+B breakpoint |
| Memoria (cat-scan)                       | Alt+M                                                                                         |
| quick-doc                                | F4; Shift+F4 alterna derecha ⟷ inferior                                                       |
| Paneles                                  | Alt+T terminal · Alt+P problemas · Alt+O salida · Alt+G git · Alt+R reglas                    |
| Vista previa de Markdown                 | Ctrl+P                                                                                        |
| Modo compacto (≤ 89 col.)                | F7 consigna · F8 código · F9 terminal · F4 doc                                                |
| Entregar examen                          | F12 o el botón `[ Entregar Examen ]`                                                          |

En la terminal interna, `Alt+T` o `Esc Esc` devuelven el foco al editor y
`Shift+RePág/AvPág` recorre el historial. La tabla completa está en _Ayuda →
Atajos de teclado_, y `[atajos]` en `.jonesy.toml` suma los propios (por ejemplo
`"ctrl+e" = "Compilar"` en terminales que no pasan las F-teclas).

### Layout adaptativo

| Tamaño                 | Modo      | Distribución                                                   |
| ---------------------- | --------- | -------------------------------------------------------------- |
| ≥ 120 × 28             | espacioso | consigna 25 % · editor 50 % · quick-doc 25 % (o dock inferior) |
| 90–119 col.            | estándar  | consigna 35 % · editor 65 %; quick-doc en dock o flotante      |
| ≤ 89 col. o ≤ 24 filas | compacto  | una vista a pantalla completa (F7/F8/F9/F4)                    |

La barra izquierda (archivos y proyecto) se abre con Alt+E / Alt+Y. Todos los
bordes entre paneles se arrastran con el mouse, y _Configuración del entorno →
Apariencia → Tamaños de paneles_ permite ajustarlos con el teclado; los valores
iniciales salen de `[interfaz.paneles]` en `.jonesy.toml`. La sesión (archivos,
cursores, paneles y sus tamaños, carpetas desplegadas, breakpoints) se guarda en
`.jonesy/session.json`.

### Documentación del proyecto

`make doc` (con `corbel proyecto`) genera `doc/` con la documentación de las
funciones de bibliotecas y ejercicios; no se versiona. jonesy busca
`doc/api.json` al abrir el proyecto y la usa en el quick-doc (firma,
descripción, parámetros, retorno, pre y poscondiciones). Si falta o quedó vieja
respecto del código (lo sabe por los SHA-256 de las fuentes que registra
corbel), la barra de estado muestra `[sin doc: make doc]` o
`[doc vieja: make doc]`: un clic la regenera. Nunca se regenera sola.

### Proyectos de la cátedra

jonesy no necesita configuración para un TP armado con plantilla-TP: lee el
Makefile de la raíz y el de cada subdirectorio (hasta dos niveles), clasifica
bibliotecas (`libs/`, `library.json`) y ejercicios (`ejercicios/`, `main.c`) y
toma la descripción de cada objetivo del comentario que lo precede en el
Makefile (`# Ejecutar las pruebas` sobre `test:`) o de un `## descripción` en la
misma línea. De `tp.sh` lee sus comandos desde la ayuda.

En la pestaña **Proyecto** (Alt+Y), Enter corre el objetivo elegido: `run` en la
terminal interna (es interactivo) y el resto con la salida en el panel y los
errores de gcc y del enlazador en Problemas. Los comandos de `tp.sh` que piden
argumentos (`add-lib`, `add-ex`) los preguntan antes. La paleta (Ctrl+K) también
lista todos los objetivos. `[[modulo]]` en `.jonesy.toml` agrega descripciones o
módulos que no se detectan solos.

## Configuración

Opcional, en `.jonesy.toml` en la raíz del proyecto. Ver
[`docs/ejemplo.jonesy.toml`](docs/ejemplo.jonesy.toml). Sin configuración: F6
corre `make -C <módulo>` para el ejercicio o biblioteca del archivo activo
(antes `make -C` de cada biblioteca de su `LIB_NAME`), `make` en la raíz o
`gcc -std=c11 -Wall -Wextra -g -O0` sobre el `.c` activo; Ctrl+F5 corre el
objetivo `run` del módulo (o compila y ejecuta); F5 depura su `PROG_TARGET`; la
entrega verifica con `make test`.

## Modo examen

- La consigna (`consigna.md`, `enunciado.md` o la configurada) se muestra
  formateada y es de solo lectura, igual que `.jonesy.toml` y `.jonesy/`.
- Cronómetro en la barra superior que sobrevive a reinicios
  (`.jonesy/examen.json`), con avisos a los 30, 15 y 5 minutos.
- Autoguardado atómico cada 30 s, al perder el foco de la terminal y ante
  SIGHUP/SIGINT/SIGTERM.
- Sólo se cargan los plugins cuyo SHA-256 figura en `examen.plugins_permitidos`.
- **Entrega**: guarda todo, corre la verificación, cierra la bitácora con
  `SessionEnd` y arma `.jonesy/entrega/entrega-<usuario>-<ts>.tar` con los
  fuentes, `telemetry.log` y `entrega.json` (resultado de la verificación,
  último hash de la cadena, SHA-256 de cada archivo). El SHA-256 del paquete se
  muestra y se deja en `<paquete>.tar.sha256`. Después los archivos quedan de
  solo lectura.

### Telemetría

`.jonesy/telemetry.log` es JSON Lines append-only: cada registro tiene `seq`,
`ts_micros`, `prev_hash` (SHA-256 de la línea anterior; la primera usa 64
ceros), `event` y `payload_hash` (SHA-256 del JSON canónico del evento).
Registra `SessionStart`, `TextDelta`, `BracketedPaste` (longitud y hash de lo
pegado), `CursorMove` (cada ≥ 200 ms), `KeyPress` (teclas de control; el texto
ya está en `TextDelta`), `BufferSave`, `CommandExec`, `FocusChange` y
`SessionEnd`. Se escribe en lotes de 32 eventos o 500 ms desde un hilo propio;
guardados, cambios de foco e inicio/fin de sesión fuerzan `flush` + `sync_data`.
`jonesy verificar` recorre la cadena y `jonesy replay` reconstruye el texto
evento por evento marcando lo pegado.

## Herramientas del ecosistema

Las herramientas de la cátedra se integran como **plugins declarativos**
(manifiestos TOML, ver [`docs/integraciones.md`](docs/integraciones.md)):

- **Al guardar** corren ripley, gaff, kaneda y spunkmeyer si están instaladas;
  sus hallazgos (en la forma común de `yutani.hallazgos`) aparecen como texto
  virtual, en Problemas con el nombre de la herramienta, y en el panel Alt+R
  agrupados por tema (memoria, punteros, seguridad, estilo…). Lo repetido entre
  herramientas se muestra una vez.
- **F1** explica el hallazgo con la herramienta que lo informó (`gaff explain`,
  `kaneda explain`…) y muestra el enlace al apunte.
- **Acciones de las herramientas (Alt+H)**: `gaff fix`, `gaff explain-syntax`,
  `corbel check` y `corbel scaffold`, hal (segfaults), vasquez (fallos
  inyectados), holden (mocks), daedalus, `tetsuo run` (sanitizers), motoko
  (opacidad de TDAs) y callahan (contratos ACSL), sobre el archivo activo.
- **Archivo → Configuración del entorno** (solapa Integraciones) muestra cuáles
  están activas y permite apagarlas; un proyecto suma las suyas en
  `.jonesy/integraciones/` y un usuario en `~/.config/jonesy/integraciones/`.
- `jonesy doctor [--rapido] [--json] [dir]` revisa la **salud del entorno** y
  dice qué falta y cómo resolverlo, por categoría: compilación (gcc compila y
  corre un programa de prueba), depuración (gdb lo depura, ptrace), memoria
  (valgrind lo revisa), git (nombre y correo para los commits), terminal (UTF-8,
  TERM, color verdadero), Python, proyecto (`.jonesy.toml` válido, permisos),
  entorno de la cátedra y herramientas del ecosistema. ✗ es necesario (sale con
  código 1), ! recomendado, · opcional. `--rapido` saltea las pruebas de humo;
  `--json` emite el sobre común del ecosistema. También desde _Ayuda →
  Diagnóstico del entorno_ (con `JONESY_DOCTOR_RAPIDO=1`, sin las pruebas de
  humo).

## Extensiones

- **Rust** (compiladas en el binario): traits `EditorPlugin`,
  `DiagnosticProvider`, `WidgetView` y `TelemetryHook` de `jonesy-plugins`,
  registrados en `Registro::de_fabrica()`.
- **Python**: archivos `.jonesy/plugins/*.py` con `al_guardar(archivo, texto)`
  (devuelve diagnósticos) y funciones `comando_<nombre>(ctx)` que aparecen en la
  paleta. Ver [`docs/plugin-ejemplo.py`](docs/plugin-ejemplo.py). El worker
  (`tools/jonesy-worker-py`, embebido en el binario) sólo se lanza si hay
  plugins, habla JSON-RPC por stdin/stdout y se reinicia si una llamada supera
  los 2 s o el proceso muere. `JONESY_PYTHON` elige el intérprete.

## Arquitectura

| Crate              | Contenido                                                                                                                                               |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `jonesy-core`      | buffers, proyecto y `.jonesy.toml`, sesión, procesos, diagnósticos de gcc y ripley, controlador GDB/MI, modelo y dibujo de cat-scan, paquete de entrega |
| `jonesy-tui`       | aplicación, layout §9.2, dibujo, resaltado, Markdown, paleta, reproductor                                                                               |
| `jonesy-doc`       | base offline de quick-doc (`data/libc.toml`) y lexer de contexto de llamada                                                                             |
| `jonesy-terminal`  | deck: PTY (`portable-pty`) + pantalla virtual `vt100`; PTY del inferior de GDB                                                                          |
| `jonesy-telemetry` | bitácora encadenada, verificación y replay                                                                                                              |
| `jonesy-git`       | estado, stage, commit, log y diff vía el binario `git`                                                                                                  |
| `jonesy-plugins`   | traits nativos y supervisor del worker Python                                                                                                           |
| `jonesy-bin`       | CLI y benchmarks                                                                                                                                        |

## Rendimiento (§10)

`bin/bench-tui [proyecto]` corre todos los presupuestos y sale con código 1 si
alguno se excede. Medido en release (Fedora 44, x86-64):

| Métrica                                              | Presupuesto | Medido                  |
| ---------------------------------------------------- | ----------- | ----------------------- |
| Arranque (proyecto + primer cuadro)                  | ≤ 50 ms     | 18 ms                   |
| RSS: consigna + C + git + quick-doc + terminal       | ≤ 30 MiB    | 5,6 MiB                 |
| RSS con worker Python                                | ≤ 50 MiB    | 15,6 MiB                |
| quick-doc (detección + documentación)                | ≤ 70 ms     | 0,02 ms                 |
| Telemetría por evento (p99)                          | ≤ 1 ms      | 0,002 ms                |
| `git status` con 500 archivos                        | ≤ 30 ms     | 3,3 ms                  |
| Primer render de Markdown (la especificación entera) | ≤ 15 ms     | 2,9 ms                  |
| Tamaño del binario                                   | ≤ 25 MiB    | 3,2 MiB                 |
| 100 sesiones en reposo (§12.5): memoria total        | ≤ 5,5 GiB   | 0,55 GiB                |
| 100 sesiones en reposo (§12.5): CPU global           | < 1,5 %     | ~0 % (16 núcleos, 30 s) |

`bin/estres-sesiones [--sesiones 100] [--segundos 30]` mide lo de §12.5: levanta
jonesy reales, cada uno en su terminal virtual y con su copia de un proyecto
(repo git, consigna, telemetría), y suma RSS y CPU de todos sus procesos.

En reposo el bucle duerme en el canal de avisos: sin eventos sólo despierta cada
2 s para mirar las fechas de `.git/HEAD` e índice (y cada 1 s en examen, por el
cronómetro). `/usr/bin/time` suma los procesos hijos: con un `.bashrc` pesado la
shell del deck domina la cifra; `SHELL=/bin/sh` aísla la del IDE.

## Desvíos respecto de la especificación

- **Menús reorganizados** según el ciclo de trabajo del alumno
  ([docs/menus.md](docs/menus.md)): Construir y Depurar separados, el menú
  Herramientas pasa a _Archivo → Configuración del entorno_, y **Alt+D** abre el
  menú Depurar (el quick-doc se muestra con F4, no con Alt+D como dice §RF-9).
- **Hilos y `std::sync::mpsc` en lugar de tokio**: el bucle, la telemetría, los
  procesos, el deck, GDB y los plugins usan hilos dedicados y un canal único de
  avisos. Cumple lo mismo (no bloquear el teclado) sin runtime asíncrono ni
  dependencias extra.
- **Verificación de la entrega**: la especificación (§RF-6) corre
  `dredd eval --local`, pero dredd es una herramienta del perfil docente. Del lado
  del alumno el equivalente es `ripley run`, que compila, corre los casos
  públicos y aplica los checks de la práctica. La entrega corre
  `[examen] verificar` (por ejemplo
  `ripley run *.c --practica examen.ripkg`) o, si no está, `make test` o el
  `verificar` del entorno de la cátedra.
- **Reglas de ripley**: se muestran los códigos reales de cátedra (`0x3001h`, …)
  en lugar de los `P1-MEM-01` de ejemplo de la especificación, y las categorías
  del panel salen del primer dígito del código.
- **cat-scan** no lee un flujo en vivo de mem-trace: al iniciar la depuración
  corre `mem-trace run --outdir .jonesy/memtrace` en segundo plano y alinea cada
  parada de GDB con el paso de la traza por función, línea y profundidad. Sólo
  para programas de un archivo sin Makefile.
- **Detección de cambios de git** por sondeo de las fechas de `.git/HEAD` e
  índice cada 2 s (sin inotify), además de refrescar tras cada guardado o acción
  del panel.
- **Ctrl+Enter** no llega como tal en muchas terminales: el commit se confirma
  también con Alt+Enter o Ctrl+S.
- **Reconexión de sesión ≤ 20 ms** (§10.2) no aplica: jonesy no tiene modo
  servidor; la sesión se restaura de `.jonesy/session.json` al arrancar.
- La telemetría está activa siempre en examen y, fuera de él, sólo con
  `--telemetria`.

## Plataformas

Linux es la plataforma principal. macOS y Windows compilan y pasan las pruebas
en la CI; en Windows los comandos usan `sh` si está en el `PATH` (UCRT64/MSYS2
del entorno de la cátedra, Git Bash) o `cmd`, el deck abre bash o `%COMSPEC%`, y
el programa depurado comparte la consola con GDB (no hay PTY separado). Ver
[PENDIENTES.md](PENDIENTES.md) para lo que falta probar a mano.

## Licencia

GPL-3.0-or-later. Ver [LICENSE](LICENSE).
