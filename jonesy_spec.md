# Documento de Requisitos — IDE TUI para el Ecosistema Docente (`jonesy`)

> **Versión**: 1.8-standalone-rust (especificación técnica consolidada con panel
> contextual dual)
>
> **Fecha**: 2026-10-07
>
> **Alcance**: especificación de requisitos y arquitectura de bajo nivel para un
> entorno de desarrollo integrado **ejecutado en la terminal (TUI)** como un
> **binario único autocontenido desde cero (Rust)**. Integra de forma nativa las
> herramientas del ecosistema docente (`ripley`, `dredd`, `entorno`,
> `mem-trace`, `alucarD`, plantillas) y está optimizado tanto para el trabajo
> diario de los estudiantes en máquinas modestas como para la **toma de exámenes
> en terminales efímeras o remotas**.
>
> **Restricciones rectoras**:
>
> 1. **Binario único compilado nativo en Rust**: arquitectura modular sobre
>    Cargo Workspace (`jonesy-bin` como ejecutable ensamblador); manipulación de
>    celdas y dibujo mediante Ratatui y Crossterm;
> 2. **Extensibilidad dual desacoplada (Rust nativo y worker Python
>    out-of-process)**: núcleo de alto rendimiento con traits internos en Rust y
>    motor de extensiones en Python ejecutado en proceso hijo aislado
>    (`jonesy-worker-py`) mediante canal IPC, garantizando total inmunidad
>    frente a segfaults o bloqueos del GIL;
> 3. **Consumo de recursos ultrarrestringido**: huella base en RAM $\le$ 25–35
>    MiB ($\le$ 50 MiB con worker Python activo), apto para cientos de sesiones
>    concurrentes en un servidor de evaluación;
> 4. **Depurador visual y terminal interna con emulación virtual completa**:
>    inspección esquemática de memoria (`cat-scan`) y PTY interactivo (`deck`)
>    con emulador de pantalla VT100 en memoria integrado a la cuadrícula de
>    celdas;
> 5. **Determinista y resiliente por diseño**: sin dependencias de IA
>    probabilística, sin telemetría externa a terceros, 100 % offline, layout
>    adaptativo con soporte para viewports de 80×24. En modo examen, registra
>    una bitácora local append-only criptográficamente sellada con SHA-256
>    encadenado;
> 6. **Git ligero invocando el binario del entorno**: orquesta asíncronamente el
>    binario `git` ya presente en el host para operaciones de status, stage,
>    commit y log, heredando configuración (`~/.gitconfig`), claves SSH y hooks,
>    delegando tareas complejas o interactivas a la terminal interna (`deck`).

---

## Índice

1.  [Motivación y contexto estudiantil](#1-motivación-y-contexto-estudiantil)
2.  [Estrategia técnica: binario único y extensibilidad Rust/Python](#2-estrategia-técnica-binario-único-y-extensibilidad-rustpython)
3.  [Contexto: ecosistema a integrar](#3-contexto-ecosistema-a-integrar)
4.  [Usuarios y perfiles (énfasis en el alumno)](#4-usuarios-y-perfiles-énfasis-en-el-alumno)
5.  [Principios rectores de diseño pedagógico](#5-principios-rectores-de-diseño-pedagógico)
6.  [Requisitos funcionales](#6-requisitos-funcionales)
7.  [Requisitos no funcionales](#7-requisitos-no-funcionales)
8.  [Arquitectura de referencia y modularización Cargo](#8-arquitectura-de-referencia-y-modularización-cargo)
9.  [Protocolo de telemetría y layout adaptativo](#9-protocolo-de-telemetría-y-layout-adaptativo)
10. [Presupuestos de recursos verificables](#10-presupuestos-de-recursos-verificables)
11. [Priorización MoSCoW](#11-priorización-moscow)
12. [Criterios de aceptación globales](#12-criterios-de-aceptación-globales)
13. [Fuera de alcance](#13-fuera-de-alcance)
14. [Autocrítica y análisis de riesgos resueltos](#14-autocrítica-y-análisis-de-riesgos-resueltos)
15. [Apéndice A — Identidad y nombres de componentes](#15-apéndice-a--identidad-y-nombres-de-componentes)

---

## 1. Motivación y contexto estudiantil

1.1. **La realidad del parque informático estudiantil y de laboratorio**: Muchos
estudiantes cursan con computadoras portátiles antiguas (2 a 4 GiB de RAM,
discos mecánicos o almacenamiento eMMC lento). Ejecutar simultáneamente un
entorno de consulta de consignas, un IDE gráfico convencional (VS Code con
extensiones C/C++ consume $\ge$ 700–900 MiB) y el compilador genera colapso por
saturación de swap y congelamientos frecuentes.

1.2. **El escenario de exámenes en terminales efímeras o de bajos recursos**:
Para instancias de evaluación presencial en laboratorios con máquinas
heterogéneas, terminales sin recursos dedicados o entornos efímeros
(contenedores de examen, sesiones SSH de cátedra), la infraestructura docente no
puede exigir una interfaz gráfica pesada ni protocolos de streaming de
escritorio (VNC/RDP). Un binario TUI de alto rendimiento permite:

- Servir decenas o cientos de sesiones de desarrollo completas en un único
  servidor de cátedra sin sobrecarga gráfica ni dependencias de ventanas.
- Ejecutar en cualquier estación que disponga de una terminal estándar
  (VT100/xterm), minimizando la carga de hardware local.
- Mantener al alumno dentro de un entorno controlado, enfocado y pedagógicamente
  asistido, con garantías de auditoría y preservación de autoría.

  1.3. **La barrera cognitiva de la terminal clásica**: Históricamente, forzar a
  estudiantes de primeros años a usar herramientas de terminal implicaba
  obligarlos a memorizar la modalidad de Vim/Emacs o a fragmentar su atención
  alternando manualmente entre `nano`, `gcc` en la shell y `gdb` por línea de
  comandos. **`jonesy` brinda la ergonomía visual de un IDE moderno** (menús
  descubribles, soporte de mouse completo, atajos familiares CUA y por teclas de
  función `F-keys`, split de paneles, visualización esquemática de memoria,
  panel de Git rápido, ayuda contextual dinámica de funciones bajo cursor o en
  pleno tipeo) utilizando únicamente la cuadrícula de celdas de la terminal.

  1.4. **Huella de recursos radicalmente contenida**: Al descartar capas
  intermedias (Electron, Qt y configuraciones complejas de Neovim) y optar por
  un binario autocontenido en Rust, la huella de memoria se reduce a $\le$
  **25–35 MiB en reposo**, con arranques en frío de $\le$ **50 ms**, eliminando
  cualquier fricción de inicio o lentitud ante compilaciones concurrentes.

---

## 2. Estrategia técnica: binario único y extensibilidad Rust/Python

### 2.1. Veredicto arquitectónico adoptado

| Opción evaluada                               | Depuración interactiva + Memoria               | Terminal interna en celda                             | Huella RSS en reposo                 | Extensibilidad                                           | Veredicto                                                                     |
| --------------------------------------------- | ---------------------------------------------- | ----------------------------------------------------- | ------------------------------------ | -------------------------------------------------------- | ----------------------------------------------------------------------------- |
| **Binario único desde cero (Rust + Ratatui)** | ✓ Nativo (controlador GDB/MI + widgets custom) | ✓ Embebido (`portable-pty` + parser VT100 en memoria) | **Mínima (**$\approx$ **25–35 MiB)** | **Sobresaliente (Rust nativo + Worker Python IPC)**      | **Estrategia adoptada**                                                       |
| Distribución sobre Neovim embebido (Lua/C)    | ✓ Plugin dap + GDB                             | ✓ Embebido (`libvterm`)                               | Media-baja ($\approx$ 45–65 MiB)     | Limitada a Lua/Vimscript (desalineada con el ecosistema) | Descartada: añade capa de abstracción y dependencias externas                 |
| Editor TUI simple (Micro / Helix) + scripts   | ✗ Sin debugger visual unificado                | ✗ Requiere multiplexor externo (`tmux`)               | Mínima ($\approx$ 20 MiB)            | Inexistente o rígida                                     | Descartada: no cumple con la experiencia integrada                            |
| IDE gráfico recortado (Qt Creator)            | ✓ Nativo                                       | ✓ Nativo                                              | Media ($\approx$ 220–420 MiB)        | C++ / Plugins pesados                                    | Descartada: inviable para sesiones masivas concurrentes en servidor de examen |

### 2.2. Enfoque técnico: binario único `jonesy`

El binario se compila como un ejecutable estático/dinámico único sin
dependencias de runtime externo en su modo base:

1. **Núcleo TUI y Manipulación de Celdas**: Desarrollado con el ecosistema Rust
   (`ratatui`, `crossterm`, `tokio`). Maneja un búfer diferencial de pantalla de
   alto rendimiento (diffing celda a celda) que minimiza la emisión de
   secuencias de escape ANSI hacia la terminal host.

2. **Motor de Texto y Buffers**: Implementado sobre estructuras de datos en
   cuerda (`ropey`), garantizando apertura instantánea y edición eficiente de
   archivos de código y Markdown con operaciones de guardado atómico.

3. **PTY Embebido y Parser VT100 Virtual (`jonesy-terminal`)**:
   - `portable-pty` crea y administra el master/slave PTY en Linux/macOS y
     ConPTY en Windows.
   - Un parser virtual de secuencias de escape ANSI (`vt100` crate) mantiene en
     memoria un búfer bidimensional de caracteres, estilos y cursor.
   - Sincronización desacoplada con el bucle TUI mediante canal
     `tokio::sync::mpsc` con coalescencia reactiva a 30–60 FPS para prevenir
     saturación ante comandos ruidosos.

4. **Motor de Telemetría Forense Local (`jonesy-telemetry`)**: Colector
   asíncrono en Rust que captura deltas de escritura, eventos de portapapeles y
   transiciones de cursor en un archivo append-only con encadenamiento
   criptográfico SHA-256 por bloques.

5. **Subcontrolador Git por CLI del Entorno (`jonesy-git`)**: Conector asíncrono
   ligero que invoca directamente el ejecutable `git` instalado en el host (vía
   `tokio::process::Command`), evitando incrustar `libgit2` y heredando de forma
   transparente la configuración del usuario (`.gitconfig`), claves SSH y hooks
   locales.

6. **Motor de Asistencia Contextual `quick-doc`**: Analizador léxico liviano
   basado en autómata finito determinista (sin requerir LSP de C pesado) que
   resuelve en microsegundos el símbolo bajo cursor o rastrea el contexto de
   llamada de función activa mientras el usuario escribe, proveyendo ayuda en
   panel lateral o inferior.

### 2.3. Modelo de extensibilidad dual (Rust y Python)

El ecosistema docente cuenta con herramientas programadas en Python (`ripley`,
`alucarD`, `mem-trace`). Para posibilitar extensiones rápidas por parte de
docentes sin recompilar el binario, y a la vez garantizar estabilidad absoluta:

- **Nivel 1 — Extensibilidad nativa en Rust (Core & Native Traits)**:
  - Traits públicos internos para componentes de máxima velocidad: proveedores
    de sintaxis, renderizadores semigráficos de memoria, adaptadores de
    depuración, hooks de telemetría y parsers PTY.
  - Cero sobrecarga de indirección, seguridad estricta de tipos e inclusión
    directa en el ejecutable.

- **Nivel 2 — Extensibilidad en Python Out-of-Process (`jonesy-plugins`)**:
  - **Aislamiento por proceso worker**: El runtime de Python **no se ejecuta
    in-process** dentro del hilo de Rust, eliminando el riesgo de que un
    _segfault_ o corrupción en una biblioteca C de Python derrumbe el IDE.
  - **Protocolo de comunicación**: Proceso hijo liviano (`jonesy-worker-py`)
    orquestado vía pipes anónimos / Unix Domain Sockets mediante un protocolo
    estructurado JSON-RPC / MessagePack.
  - **Lazy Initialization**: Si la sesión no contiene plugins en
    `.jonesy/plugins/*.py`, el proceso worker de Python jamás se inicia,
    conservando la huella en memoria $\le$ 30 MiB.
  - **Watchdog y Timeouts**: Cada llamada a plugin está protegida por un timeout
    configurable (ej. 2.0 s). Si un plugin entra en un bucle infinito, el worker
    es reiniciado de forma transparente sin bloquear el bucle de eventos TUI.

---

## 3. Contexto: ecosistema a integrar

| Componente                  | Rol en el ecosistema                                      | Contrato de integración con la TUI                                                                                                                                                                    |
| --------------------------- | --------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ripley`                    | Verificación pedagógica en C (reglas P1, TDD, estilo)     | Diagnósticos en línea (virtual text) y panel de problemas (`ripley check --format=json` o vía worker Python).                                                                                         |
| `dredd`                     | Orquestador docente (autograding, entregas, exámenes)     | Panel de entregas/consignas, despacho de autoevaluación (`dredd eval --json`) y empaquetado de telemetría de examen.                                                                                  |
| `entorno`                   | Entorno portable de cátedra (GCC, GDB, Git, scripts base) | Provisión del toolchain, variables de entorno, `git` en PATH y codificación UTF-8 homogénea.                                                                                                          |
| `mem-trace`                 | Trazador de memoria C (inspección de Stack y Heap)        | Backend de extracción JSON por stdio; `cat-scan` renderiza el diagrama esquemático con caracteres de caja.                                                                                            |
| `alucarD`                   | Motor de enunciados y exámenes (YAML/GIFT)                | Visor de consignas de examen formateadas en split protegido.                                                                                                                                          |
| `plantillas`                | Esqueletos de TP y ejercicios de examen                   | Autodetección de tareas (`Makefile`, `.jonesy.toml`) y perfiles de ejecución inmediatos.                                                                                                              |
| `git` (binario del entorno) | Control de versiones local y remoto                       | Invocado directamente por CLI por el IDE (`git status --porcelain=v2`, `git add`, `git commit`, `git log`) heredando configuración y credenciales; terminal interactiva `deck` para flujos avanzados. |

---

## 4. Usuarios y perfiles (énfasis en el alumno)

| Perfil                                                | Escenario habitual                                                                | Necesidad clave respecto a la TUI                                                                                                                                                                                                                                                                                                                                                                             |
| ----------------------------------------------------- | --------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Estudiante (Aprendizaje cotidiano)**                | Trabaja TPs en C en su computadora personal modesta o entorno portable.           | Interfaz intuitiva: soporte de mouse transparente, menú visible en pantalla, mensajes en español claro, atajos familiares (Ctrl+S, Ctrl+Z, F5 para debug), control de versiones básico en pantalla (commits claros y atómicos heredando su usuario configurado), ayuda contextual en vivo (panel derecho o inferior) para la función activa mientras programa o ubica el cursor, editor de informes Markdown. |
| **Estudiante (Rindiendo examen en terminal efímera)** | Conectado a una sesión terminal remota o contenedor provisto por la cátedra.      | Lectura clara de la consigna sin salir del IDE, atajos basados en teclas de función (`F-keys`) para evitar colisiones con atajos del sistema host, panel de documentación inmediata de funciones de biblioteca (`quick-doc`) sin recurrir a navegadores o manpages externas, guardado automático continuo ante caídas de sesión, botón evidente de "Entregar examen" con sellado auditable.                   |
| **Docente / Evaluador de Examen**                     | Diseña ejercicios, corrige o programa scripts de autoevaluación; audita entregas. | Posibilidad de escribir plugins y validadores en **Python**; acceso a la bitácora de flujo de texto (`jonesy replay`) para comprobar autoría genuina frente a pegados masivos; visor de diffs y resultados de `dredd`.                                                                                                                                                                                        |
| **Administrador de Servidor de Examen**               | Aloja decenas de instancias simultáneas de `jonesy` en un host multi-usuario.     | Consumo de RAM mínimo por sesión ($\le$ 35 MiB base), binario único sin dependencias complejas, arranque inmediato ($\le$ 50 ms).                                                                                                                                                                                                                                                                             |

---

## 5. Principios rectores de diseño pedagógico

1. **La celda de texto es un lienzo de alta densidad**: En lugar de desperdiciar
   espacio con márgenes decorativos, cada celda ANSI/Unicode transmite
   información útil (código, diagramas de cajas para estructuras en memoria,
   atributos tipográficos para Markdown, ayuda contextual de funciones y
   advertencias didácticas).
2. **Cero barrera de entrada para el principiante**: No se asume conocimiento de
   comandos arcanos de consola o modos de Vim. El alumno puede usar el mouse
   para posicionar el cursor, hacer clic en los menús superiores o en los
   botones de acción de la barra de estado.
3. **Mapeo de atajos robusto y accesible**: Cada acción fundamental cuenta con
   atajos CUA clásicos (`Ctrl+S`, `Ctrl+Z`) y una alternativa directa basada en
   teclas de función estándar (`F2`, `F4`, `F5`, `F6`, `F10`), garantizando
   operabilidad en cualquier emulador de terminal.
4. **Determinismo y transparencia formativa**: Los diagnósticos indican
   exactamente la regla de cátedra violada (ej.
   `[P1-MEM-02] Liberación duplicada`) con acceso inmediato a su justificación
   conceptual (`F1`). La ayuda contextual de funciones explica firmas, tipos de
   parámetros y trampas comunes de libc sin sugerencias opacas ni IA
   probabilística.
5. **Auditoría transparente del flujo de texto**: En exámenes, el proceso de
   resolución es tan formativo y probatorio como el código final. La telemetría
   registra el ritmo de escritura y ráfagas de edición de forma local y
   auditable, garantizando la autoría del estudiante.
6. **Local-first y tolerante a desconexiones de terminal**: Todo cambio se
   conserva en disco de inmediato en modo atómico / append-only. Si la sesión
   terminal se corta, el reingreso devuelve al estudiante exactamente a su
   estado anterior sin pérdida de código, cursor ni telemetría.
7. **Extensibilidad segura y desacoplada**: Los plugins en Python enriquecen la
   pedagogía sin poner en riesgo la estabilidad del núcleo en Rust: un fallo en
   un script corre en un proceso aislado y jamás interrumpe la sesión del
   estudiante.
8. **Reutilización pragmática de herramientas del sistema (Git CLI)**: En lugar
   de reimplementar motores de control de versiones o enlazar librerías de C
   propensas a incompatibilidades de configuración o fallos con llaves SSH, el
   IDE se apoya directamente en el binario `git` provisto por el entorno.

---

## 6. Requisitos funcionales

Convención: `[M]` = Must (imprescindible), `[S]` = Should (deseable), `[C]` =
Could (opcional).

### RF-1 — Núcleo de edición y navegación accesible `[M]`

- **RF-1.1. Manejo ágil de buffers sobre cuerda (`ropey`)**: Apertura en $\le$
  20 ms para archivos típicos; soporte fluido para múltiples buffers abiertos
  simultáneamente.
- **RF-1.2. Mapeo de atajos dual (CUA y Teclas de Función)**:
  - Esquema CUA estándar: `Ctrl+S` (guardar), `Ctrl+Z` (deshacer), `Ctrl+C` /
    `Ctrl+V` (copiar/pegar).
  - Esquema por teclas de función (`F-keys`): `F2` (guardar), `F3` (buscar),
    `F4` (alternar panel de documentación/ayuda), `F5` (depurar), `F6`
    (compilar) y menú superior accesible por `F10` o clic de mouse.
- **RF-1.3. Soporte universal de mouse en terminal**:
  - Clic para situar el cursor y enfocar paneles o splits.
  - Doble clic para seleccionar palabras/identificadores.
  - Scroll con rueda de mouse en código, Markdown y terminal interna.
  - Arrastre para redimensionar los separadores de paneles en pantalla.
- **RF-1.4. Resaltado de sintaxis ligero**: Sintaxis para C (C11/C17), Markdown,
  YAML y TOML procesada mediante gramáticas estáticas sin depender de procesos
  LSP pesados en memoria.
- **RF-1.5. Paleta de comandos flotante (`Ctrl+K` / `F1`)**: Buscador difuso
  (fuzzy) accesible por teclado o mouse para disparar cualquier acción o comando
  de extensiones.

### RF-2 — Superficie de construcción y automatización `[M]`

- **RF-2.1. Compilación con una sola pulsación**: `F6` (o botón "Compilar" en
  pantalla) ejecuta la tarea declarada en `.jonesy.toml` o el `Makefile` del
  proyecto.
- **RF-2.2. Panel inferior de diagnósticos del compilador**:
  - Parseo inmediato de salidas de GCC/Clang (`archivo:línea:col: mensaje`).
  - Clic con mouse o pulsar `Enter` salta instantáneamente a la ubicación del
    error en el editor.
- **RF-2.3. Ejecución directa**: `Ctrl+F5` compila y corre el binario en la
  terminal interna embebida (RF-5) manteniendo la salida visible tras finalizar.

### RF-3 — Integración con `ripley` (Tutor pedagógico) `[M]`

- **RF-3.1. Diagnósticos pedagógicos automáticos**: Al guardar el archivo (`F2`
  / `Ctrl+S`), se ejecuta en segundo plano `ripley check --format=json` (o vía
  worker Python) sin bloquear la interfaz.
- **RF-3.2. Anotaciones visuales en línea (Virtual Text)**: Las violaciones a
  las reglas se renderizan al final de la línea correspondiente con color
  diferenciado según severidad.
- **RF-3.3. Ayuda de regla interactiva**: Presionar `F1` sobre un diagnóstico
  abre una ventana flotante con la explicación didáctica completa de la regla
  P1.
- **RF-3.4. Panel unificado de reglas**: Split lateral con el árbol de problemas
  detectados agrupados por categorías (Memoria, Estilo, TDD, Seguridad).

### RF-4 — Depurador visual interactivo y vista de memoria (`cat-scan`) `[M]`

- **RF-4.1. Controles visuales de depuración**:
  - Botones interactivos por mouse y teclas: `[▶ Continuar]`, `[↷ Paso]` (F10),
    `[↓ Entrar]` (F11), `[↑ Salir]`, `[■ Detener]`.
  - Canal de breakpoints en el margen izquierdo: clic con mouse alterna un
    breakpoint (`●` rojo).
- **RF-4.2. Vista de Variables y Marcos (Call Stack)**: Panel lateral que
  desglosa marcos activos, variables locales y parámetros.
- **RF-4.3. Panel de Memoria Semigráfico en Celdas (`cat-scan`)**:
  - Representación esquemática a partir del flujo JSON de `mem-trace`.
  - **Stack**: Marcos delimitados por caracteres de caja (`┌─┐│└─┘`); muestra
    nombres, tipos, valores y direcciones hexadecimales.
  - **Heap**: Bloques dinámicos representados con su tamaño y estado; punteros
    en el stack trazan flechas Unicode (`──►`, `▲`, `│`) hacia sus bloques
    destino en el heap.
  - **Detección visual de anomalías**: Punteros colgantes o accesos
    post-`free()` resaltados en rojo brillante con texto de advertencia.
- **RF-4.4. Consola interactiva de GDB subordinada**: Panel desplegable para
  ingresar comandos manuales directos a la sesión GDB activa.

### RF-5 — Terminal interna integrada (`deck`) con emulación VT100 `[M]`

- **RF-5.1. Emulación PTY desacoplada (`portable-pty` + `vt100`)**:
  - `portable-pty` interactúa con el sistema operativo gestionando los
    descriptores master/slave.
  - La capa intermedia utiliza el parser virtual `vt100` en memoria para
    interpretar secuencias ANSI/xterm y sostener una matriz $W \times H$ con
    celdas, estilos, colores y posición del cursor.
  - El renderizador de Ratatui proyecta la matriz de la pantalla virtual a la
    cuadrícula TUI mediante coalescencia de actualizaciones a 30–60 FPS,
    evitando la degradación de la interfaz ante secuencias de escape masivas o
    bucles infinitos de `printf`.
- **RF-5.2. Soporte completo de I/O interactivo**: Admite entrada y salida para
  programas en C (`scanf`, `fgets`) en tiempo real sin desfases de búfer.
- **RF-5.3. Shell preconfigurada de cátedra**: Se inicializa con el entorno y
  variables de `entorno` (Bash sobre UCRT64 en Windows, Bash/Zsh en Linux).
- **RF-5.4. Alternancia rápida de foco**: Conmutación inmediata mediante `Alt+T`
  o clic directo de mouse.

### RF-6 — Modo Examen, Telemetría y Evaluación (`dredd` integration) `[M]`

- **RF-6.1. Activación por bandera de entorno / comando**: Inicio mediante
  `jonesy --exam` o variable `JONESY_EXAM_MODE=1`.
- **RF-6.2. Enunciado protegido en split lateral**:
  - Renderizado del enunciado (`consigna.md`) en panel de solo lectura con
    scroll independiente y atributos visuales formateados (RF-8).
  - Bloqueo estricto de edición del archivo de consigna.
- **RF-6.3. Guardado atómico continuo y recuperación**: Escritura atómica a
  disco cada 30 segundos y ante pérdida de foco o interrupción de la sesión de
  terminal.
- **RF-6.4. Telemetría forense local del flujo de texto (`telemetry-engine`)
  `[M]`**:
  - **Registro continuo append-only**: Bitácora estructurada
    (`.jonesy/telemetry.log`) en formato JSON Lines canónico detallado en §9.1.
  - **Métricas de flujo y cadencia**: Monitoreo de velocidad de tipeo, pausas
    reflexivas y frecuencias de borrado.
  - **Detección de ráfagas e inserciones masivas**: Registro unívoco de bloques
    insertados mediante _Bracketed Paste_, documentando longitud, hash del
    contenido pegado y timestamp.
  - **Encadenamiento y sellado criptográfico**: Cada bloque o evento incluye el
    hash SHA-256 del registro anterior, previniendo alteraciones locales.
  - **Capacidad de reproducción (Replay)**: Comando docente
    (`jonesy replay <sesion.log>`) para reconstruir la sesión paso a paso en
    terminal.
- **RF-6.5. Botón y comando de Entrega Asistida**: Botón visible
  `[ Entregar Examen ]`. Ejecuta `dredd eval --local`, muestra el resultado de
  los tests, sella la bitácora de telemetría y consolida el paquete final con su
  hash SHA-256.
- **RF-6.6. Cronómetro visible de examen**: Indicador con tiempo restante en la
  barra superior, con avisos discretos a los 30, 15 y 5 minutos finales.

### RF-7 — Portapapeles y pegado seguro en terminal (OSC 52 & Bracketed Paste) `[M]`

- **RF-7.1. Secuencias de portapapeles OSC 52**: Al copiar o cortar texto,
  `jonesy` puede emitir secuencias de escape estándar OSC 52 hacia el emulador
  de terminal host para interactuar con el portapapeles del sistema operativo de
  manera nativa.
- **RF-7.2. Modo de pegado protegido (Bracketed Paste Mode)**: Soporte de
  `\e[?2004h` para evitar autoindentaciones erráticas al pegar código C y
  permitir la detección confiable de bloques pegados por parte del subsistema de
  telemetría (RF-6.4).

### RF-8 — Edición y renderizado con formato de Markdown `[M]`

- **RF-8.1. Motor TUI de texto estructurado y formateado**:
  - Renderizado sobre matriz de celdas con soporte de estilos tipográficos ANSI:
    - **Encabezados**: Niveles H1 a H3 con atributos de color, negrita (`\e[1m`)
      y línea divisoria semigráfica.
    - **Énfasis**: Texto en **negrita**, _cursiva_ (`\e[3m`) y tachado
      renderizado con secuencias SGR.
    - **Citas y Listas**: Bloques de cita con barra lateral (`│ cita`), listas
      ordenadas y task lists (`[ ]`, `[x]`).
    - **Tablas GFM**: Tablas alineadas con bordes de caracteres semigráficos
      (`┌┬┐├┼┤└┴┘`).
    - **Bloques de código cercados**: Resaltado de sintaxis integrado para C,
      Bash, YAML y TOML.
- **RF-8.2. Capacidad dual: Visualización y Edición**: Apertura directa de
  archivos Markdown en el motor `ropey`. Modo split interactivo (`Ctrl+P`) con
  preview formateada en tiempo real (debounce $\ge$ 100 ms tras pausa de tipeo).
- **RF-8.3. Navegación por hipervínculos internos**: Clic o `Enter` sobre
  enlaces relativos locales abre el archivo de destino en el panel adyacente.

### RF-9 — Panel de Ayuda Contextual de Funciones (`quick-doc`) `[M]`

- **RF-9.1. Detección automática dual (bajo cursor y durante el tipeo
  interactivo)**:
  - **Modo Hover/Cursor estático**: Al posicionar el cursor sobre cualquier
    identificador o invocación de función, el panel recupera en $\le 70$ ms su
    documentación y firma.
  - **Modo Signature Help interactivo en tipeo**: Al escribir la apertura de
    paréntesis `(` o comas separadoras `,`, el analizador reconoce la función
    receptora y **resalta visualmente en negrita y color el parámetro posicional
    activo** que el alumno está completando en ese instante.
- **RF-9.2. Base de conocimiento offline y estructurada**:
  - Diccionario local estático embebido directamente en el binario sin requerir
    internet ni indexadores LSP externos.
  - Cobertura de la biblioteca estándar de C (C99/C11: `stdio.h`, `stdlib.h`,
    `string.h`, `stdbool.h`, etc.), llamadas POSIX recurrentes (`fork`, `pipe`,
    `read`, `write`, `waitpid`) y tipos comunes de cátedra (`lista_t`,
    `nodo_t`).
- **RF-9.3. Información didáctica presentada en el panel**:
  - **Firma normalizada**: Ejemplo `void *malloc(size_t size)`.
  - **Contrato y Retorno**: Explicación concisa del valor devuelto en caso de
    éxito y en fallo (`NULL`, `-1`, `errno`).
  - **Parámetros desglosados**: Tipo, nombre y propósito didáctico de cada
    argumento.
  - **Trampas frecuentes (_Pitfalls_)**: Recordatorios de errores habituales de
    alumnos (ej. "No olvidar validar si devolvió NULL", "Recordar el byte nulo
    terminador `\0` al reservar memoria para strings").
  - **Vínculo con Reglas Ripley**: Advertencias inmediatas de cátedra asociadas
    (ej. `[P1-MEM-01]`, `[P1-MEM-04]`).
- **RF-9.4. Disposición flexible de panel (Lateral derecho o Inferior)**:
  - **Ubicación configurable**: El panel puede proyectarse como **panel lateral
    derecho** (modo vertical, ideal en pantallas anchas) o como **panel inferior
    dockeable** (modo horizontal, maximizando la longitud de líneas de código).
  - **Conmutación rápida**:
    - `F4` o `Alt+D`: Muestra / oculta el panel contextual.
    - `Shift+F4` o comando en paleta: Alterna la orientación del panel
      (`Derecha ⟷ Inferior`).
    - Almacena la preferencia del usuario en el estado de sesión local
      (`.jonesy/session.json`).
  - En viewports reducidos ($\le 89$ columnas), se adapta automáticamente a la
    máquina de estados espacial de §9.2.

### RF-10 — Sistema de extensiones (Rust & Worker Python IPC) `[M]`

- **RF-10.1. API de extensiones nativas en Rust**: Traits públicos internos
  (`EditorPlugin`, `DiagnosticProvider`, `WidgetView`, `TelemetryHook`)
  ensamblados en tiempo de compilación dentro del crate principal.
- **RF-10.2. Worker de extensiones en Python (`jonesy-worker-py`)**: Subproceso
  worker aislado comunicado mediante JSON-RPC sobre pipes anónimos, con canal
  asíncrono en Rust (`tokio::sync::mpsc`). Un fallo o crash en Python no
  compromete la memoria de `jonesy`.
- **RF-10.3. Carga diferida (Lazy Initialization)**: Si el proyecto no define
  scripts en `.jonesy/plugins/*.py`, el proceso worker de Python nunca es
  lanzado.
- **RF-10.4. Restricción en Modo Examen**: En modo `jonesy --exam`, se
  deshabilita la carga de scripts Python arbitrarios de usuario, autorizando
  únicamente extensiones firmadas por la cátedra.

### RF-11 — Control de versiones Git básico integrado (vía CLI del host) `[M]`

- **RF-11.1. Estado de repositorio visible y en tiempo real (Status)**:
  - Invocación periódica o reactiva de `git status --porcelain=v2 --branch -z`.
  - Indicador permanente en la barra de estado: rama activa (ej. ` main`),
    estado (_clean/dirty_), y cuenta de archivos modificados/staged.
  - Panel desplegable o lateral de Git (`Alt+G`): lista organizada de archivos
    modificados, untracked, borrados y en stage.
- **RF-11.2. Preparación interactiva de archivos (Stage / Unstage)**: `Space`
  ejecuta `git add -- <ruta>` o `git restore --staged -- <ruta>`. Atajos
  globales `a` (stage all) y `u` (unstage all). Diff rápido en split adyacente.
- **RF-11.3. Creación asistida de commits**: Diálogo modal (`c`) con aviso si la
  primera línea supera 72 caracteres. Confirmación con `Ctrl+Enter` ejecuta
  directamente `git commit -m "<mensaje>"`, respetando identidad (`user.name`,
  `user.email`), hooks locales y firma del host.
- **RF-11.4. Visor de historial navegable (Log)**: Vista compacta (`l`) que
  lista commits recientes. Al pulsar `Enter`, muestra el diff invocando
  `git show <hash>`.
- **RF-11.5. Delegación de operaciones complejas a la terminal interna
  (`deck`)**: Operaciones avanzadas (`push`, `pull`, `fetch`, `rebase`,
  resolución interactiva de conflictos) se delegan al PTY interno mediante el
  botón o atajo `[ Abrir en Terminal ]` (`Alt+T`).

---

## 7. Requisitos no funcionales

### RNF-A — Consumo de recursos y rendimiento en servidores multi-usuario

- **RNF-A.1. Memoria de trabajo (RSS)**:
  - En reposo con buffer de código, consigna Markdown, status Git y panel
    `quick-doc` activo (núcleo Rust puro): $\le$ **30 MiB**.
  - Con worker de extensiones Python inicializado: $\le$ **50 MiB**.
  - En sesión de depuración activa con panel `cat-scan`, `quick-doc` y terminal
    `deck` activa: $\le$ **75 MiB**.
  - _Objetivo de escala_: Un servidor de cátedra con 16 GiB de RAM aloja hasta
    **150 sesiones concurrentes de examen** sin swap sostenido.
- **RNF-A.2. Tiempo de arranque**: Arranque en frío $\le$ **50 ms**; reconexión
  de sesión $\le$ **20 ms**.
- **RNF-A.3. Uso de CPU nulo en espera**: 0.0 % de consumo de CPU sostenido
  cuando el usuario no escribe.
- **RNF-A.4. Tamaño del binario compilado**: $\le$ **25 MiB** para el ejecutable
  autocontenido de `jonesy` (compilado con LTO y stripped, con base estática de
  libc/man3 embebida).

### RNF-B — Resiliencia de terminal y transporte

- **RNF-B.1. Renderizado diferencial estricto**: Matriz de diff celda a celda;
  solo se emiten secuencias de escape ANSI para celdas alteradas entre
  fotogramas.
- **RNF-B.2. Adaptabilidad a viewport y redibujado (`SIGWINCH`)**: Reajuste
  instantáneo ante cambios de tamaño de ventana. Soporte funcional desde
  terminales compactas de **80 columnas** $\times$ **24 filas** mediante la
  máquina de layout de §9.2.
- **RNF-B.3. Limpieza de terminal en salida y desconexión**: Ejecución en
  alternate buffer (`\e[?1049h`) y restauración limpia de la pantalla ante
  `SIGHUP`, `SIGINT` o caídas del proceso.

### RNF-C — Usabilidad pedagógica y lenguaje

- **RNF-C.1. Idioma unificado en español rioplatense**: Mensajes de interfaz,
  botones, advertencias y ayudas didácticas redactados en español claro ("Guardá
  antes de compilar", "¿Confirmás la entrega?", "Hay cambios sin commitear",
  "malloc() devolvió NULL").
- **RNF-C.2. Accesibilidad visual y degradación de caracteres**: Degradación
  automática de caracteres de dibujo de cajas a ASCII estándar (`+`, `-`, `|`,
  `>`) si la terminal no negocia UTF-8 completo.

### RNF-D — Determinismo, aislamiento, telemetría y seguridad

- **RNF-D.1. Aislamiento total de IA**: Cero generación de código asistida por
  IA, autocompletado probabilístico o llamadas a modelos remotos.
- **RNF-D.2. Modo offline estricto**: Cero llamadas de red salientes o consultas
  DNS desde el binario.
- **RNF-D.3. Integridad de la telemetría**: Archivo `.jonesy/telemetry.log`
  append-only con encadenamiento SHA-256 verificado en `dredd eval`.
- **RNF-D.4. Inmunidad a fallos de Python**: Errores, excepciones o caídas del
  proceso worker de Python son capturados por el supervisor de `jonesy` y jamás
  provocan el pánico (_panic_) del binario Rust principal.

---

## 8. Arquitectura de referencia y modularización Cargo

### 8.1. Árbol de Crates del Cargo Workspace

Para sostener el principio de diseño "Binario único pero arquitectura
desacoplada", el código fuente se estructura en un Cargo Workspace con
separación estricta de responsabilidades:

```text
jonesy/
├── Cargo.toml                   # Workspace manifest
├── crates/
│   ├── jonesy-core/             # Buffers (ropey), estado global, ciclo de vida, eventos
│   ├── jonesy-tui/              # Widgets Ratatui, layouts responsivos, quick-doc (dock/split), cat-scan
│   ├── jonesy-doc/              # Base de conocimiento estática de funciones C, firmas y lexer liviano
│   ├── jonesy-terminal/         # portable-pty + parser virtual vt100 + pipeline de dibujo
│   ├── jonesy-telemetry/        # Registro append-only, SHA-256 encadenado y motor de replay
│   ├── jonesy-git/              # Subproceso asíncrono para Git CLI del host y parseo porcelain
│   ├── jonesy-plugins/          # Supervisor IPC para worker Python (JSON-RPC)
│   └── jonesy-bin/              # Punto de entrada principal (ensambla los crates en 1 binario)
└── tools/
    └── jonesy-worker-py/        # Script worker mínimo de Python para ejecución out-of-process
```

### 8.2. Diagrama de Flujo y Concurrencia

```text
┌─────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                JONESY-BIN (Binario único en Rust)                              │
│                                                                                                 │
│  ┌─────────────────────── jonesy-tui: Responsive Ratatui Layout Manager ────────────────────┐  │
│  │ [Editor C / Alumno]          │ [quick-doc: Split Derecho]   │ [Consigna / Git Status]      │  │
│  │ Ropey Buffer + Text Styling  │ Firmas, Params y Ripley      │ Markdown / Staging & Commits │  │
│  ├──────────────────────────────┴──────────────────────────────┴──────────────────────────────┤  │
│  │ [quick-doc: Modo Alternativo Dock Inferior]                 │ [cat-scan: Vista Memoria]    │  │
│  │ Firma activa mientras se tipea + advertencias de parámetros │ Stack & Heap semigráfico     │  │
│  ├─────────────────────────────────────────────────────────────┴──────────────────────────────┤  │
│  │ [deck: Terminal Virtual jonesy-terminal] (portable-pty + vt100 screen, 30-60 FPS coalesce)  │  │
│  └────────────────────────────────────────────────────────────────────────────────────────────┘  │
│         ▲                                                            ▲                          │
│         │ Terminal I/O Events (ANSI, SGR Mouse, OSC 52, Paste)       │ State Updates / Diff Redraw│
│  ┌──────┴────────────────────────────────────────────────────────────┴───────────────────────┐  │
│  │                 Async Event Loop & Process Dispatcher (Tokio / jonesy-core)               │  │
│  └──────┬───────────────────────┬────────────────────────────┬────────────────────────┬──────┘  │
│         │                       │                            │                        │         │
│  ┌──────┴─────────┐     ┌───────┴──────────┐         ┌───────┴──────────┐     ┌───────┴──────┐  │
│  │ Rust Native    │     │ jonesy-plugins   │         │ DAP / GDB-MI     │     │ PTY Subproc  │  │
│  │ Traits API     │     │ IPC Supervisor   │         │ Client           │     │ Master       │  │
│  │ (Zero-cost)    │     │ (Pipes anónimos) │         │ (mem-trace sync) │     │ (Shell C)    │  │
│  └──────┬─────────┘     └───────┬──────────┘         └───────┬──────────┘     └───────┬──────┘  │
│         │                       │                            │                        │         │
│  ┌──────┴───────────────────────┴────────────────────────────┴────────────────────────┴──────┐  │
│  │   Telemetry Engine, Git CLI Wrapper & Doc DB (Append-only Ledger, Tokio Child Process)    │  │
│  └───────────────────────────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────┬────────────────────────────┬────────────────────────┬─────────┘
                                  │                            │                        │
                      (IPC Pipes / JSON-RPC)            (Sesión GDB/MI)            (Bash / GCC /
                                  │                            │                 git CLI host)
                                  ▼                            ▲                        ▲
                         ┌─────────────────┐           ┌───────┴──────────┐     ┌───────┴──────┐
                         │ jonesy-worker-py│           │ Terminal Host    │     │ Host Git CLI │
                         │ (Aislado / lazy)│           │ (xterm/VT100)    │     │ (Auth / Push)│
                         └─────────────────┘           └──────────────────┘     └──────────────┘
```

---

## 9. Protocolo de telemetría y layout adaptativo

### 9.1. Especificación del Ledger de Telemetría (`jonesy-telemetry`)

El archivo `.jonesy/telemetry.log` utiliza un formato **JSON Lines canónico**
append-only. Cada línea representa un registro atómico con sellado criptográfico
encadenado.

#### Contrato de Estructuras (Rust):

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TelemetryRecord {
    pub seq: u64,                   // Número de secuencia monotónico incremental
    pub ts_micros: u64,             // Timestamp UNIX en microsegundos
    pub prev_hash: String,          // Hex SHA-256 del registro de la línea previa
    pub event: TelemetryEvent,      // Carga útil del evento
    pub payload_hash: String,       // Hex SHA-256 del contenido JSON canónico de 'event'
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "data")]
pub enum TelemetryEvent {
    SessionStart { user: String, exam_mode: bool },
    KeyPress { key: String, modifiers: u8 },
    TextDelta { buffer_id: String, offset: usize, inserted: String, deleted_len: usize },
    BracketedPaste { buffer_id: String, length: usize, content_hash: String },
    CursorMove { buffer_id: String, line: usize, col: usize },
    BufferSave { buffer_id: String, total_bytes: usize },
    CommandExec { command: String, exit_code: Option<i32> },
    FocusChange { panel: String, focused: bool },
    SessionEnd { reason: String },
}
```

#### Política de I/O y Sincronización en Disco:

1. **Canal Asíncrono no bloqueante**: Los eventos se despachan desde el bucle
   TUI mediante un canal `tokio::sync::mpsc::channel(1024)`. La escritura jamás
   añade latencia perceptible al teclado ($\le 1$ ms).
2. **Vaciado por Lotes (Batch Flushing)**: Un hilo dedicado de I/O escribe al
   archivo con un búfer de memoria (`BufWriter`) y ejecuta `flush()`:
   - Al acumular **32 eventos**, o
   - Tras una ventana de inactividad de **500 ms**.
3. **Flushing Forzado de Resiliencia**: Se ejecuta `flush()` y `sync_data()`
   inmediato ante:
   - Eventos `BufferSave` (`F2` / `Ctrl+S`).
   - Pérdida de foco del terminal o cambio de split.
   - Recepción de señales de terminación del sistema (`SIGINT`, `SIGHUP`,
     `SIGTERM`).

### 9.2. Máquina de Estados del Layout Adaptativo (Responsive Grid)

Para garantizar legibilidad óptima tanto en pantallas amplias como en terminales
reducidas de **80 columnas** $\times$ **24 filas**, el gestor de ventanas
(`jonesy-tui`) implementa una máquina de tres modos espaciales según el evento
`SIGWINCH`:

```text
       Terminal Resize (SIGWINCH)
                 │
                 ▼
     ┌───────────────────────┐
     │ Ancho >= 120 cols &&  │ ──► [ MODO ESPACIOSO ]
     │ Alto  >= 28 filas     │     Consigna (25%) | Editor (50%) | Quick-Doc derecho (25%)
     └───────────────────────┘     Opcional: Quick-Doc en dock inferior (3 filas bajo el código)
                 │
                 ▼
     ┌───────────────────────┐
     │ Ancho: 90 - 119 cols  │ ──► [ MODO ESTÁNDAR ]
     │ Alto  >= 24 filas     │     Consigna (35%) | Editor (65%)
     └───────────────────────┘     Quick-Doc: Dock inferior de 3 filas o modal flotante (F4)
                 │
                 ▼
     ┌───────────────────────┐
     │ Ancho <= 89 cols ||   │ ──► [ MODO COMPACTO (Tabs 80x24) ]
     │ Alto  <= 24 filas     │     Pantalla completa exclusiva por vista:
     └───────────────────────┘     F7: Consigna · F8: Código · F9: Terminal · F4: Doc
```

- **Modo Espacioso ($\ge 120 \times 28$)**: Muestra en simultáneo la consigna
  del ejercicio, el búfer de código C y el panel lateral `quick-doc` en el
  margen derecho (o en el dock inferior de 3 filas según preferencia de
  usuario). La terminal `deck` o la vista de memoria `cat-scan` se abren en un
  split horizontal inferior bajo demanda.
- **Modo Estándar ($90 \dots 119 \text{ cols}$)**: Prioriza la superficie del
  código C (65 %) y la consigna (35 %). Por defecto, ubica `quick-doc` como un
  dock compacto inferior de 3 filas bajo el editor para no comprimir
  horizontalmente el código, o como ventana modal flotante temporal al pulsar
  `F4`.
- **Modo Compacto ($\le 89 \text{ cols o } \le 24 \text{ filas}$)**: Las
  columnas se unifican en una vista completa de 80 celdas de ancho. El
  estudiante conmuta entre pantallas de forma inmediata usando teclas de
  función: `F7` (Consigna), `F8` (Editor de código), `F9` (Terminal interna) y
  `F4` (Ayuda de funciones en pantalla completa).

---

## 10. Presupuestos de recursos verificables

Criterios de validación automatizados ejecutables mediante scripts de
benchmarking (`bin/bench-tui`).

### 10.1. Comandos de Verificación Automatizada

```bash
# RSS tras abrir proyecto típico con consigna Markdown formateada, buffer C, panel Git, quick-doc activo y terminal tras 10 s (Rust puro)
/usr/bin/time -v jonesy --benchmark-session ./tp-alumno 2>&1 | grep 'Maximum resident'

# Verificación de CPU nula en reposo
pidstat -p $(pgrep -f 'jonesy') 2 5

# Verificación de memoria con extensiones Python y telemetría activa
jonesy --benchmark-python-plugins ./tp-alumno

# Tiempo de arranque en frío con cachés limpias (debe ser ≤ 50 ms)
hyperfine --prepare 'sync; echo 3 > /proc/sys/vm/drop_caches' 'jonesy --headless --benchmark-startup'

# Latencia de detección de símbolo y renderizado en quick-doc (debe ser ≤ 70 ms)
jonesy --benchmark-doc-lookup "malloc"

# Latencia de inyección de evento en el registro de telemetría (debe ser ≤ 2 ms)
jonesy --benchmark-telemetry-write

# Tiempo de obtención de git status mediante invocación CLI en árbol de 500 archivos (debe ser ≤ 30 ms)
jonesy --benchmark-git-status ./tp-alumno
```

### 10.2. Tabla Resumen de Presupuestos Técnicos

| Métrica                                                                   | Presupuesto máximo            | Método de verificación                          |
| ------------------------------------------------------------------------- | ----------------------------- | ----------------------------------------------- |
| **RSS base** (buffer C + Markdown + Git status + `quick-doc`, sin Python) | $\le$ 30 MiB                  | `/usr/bin/time -v`                              |
| **RSS con worker Python inicializado**                                    | $\le$ 50 MiB                  | `pidstat` durante ejecución con plugins         |
| **RSS en debug interactivo + PTY** (`cat-scan` + `deck`)                  | $\le$ 75 MiB                  | `pidstat` durante sesión activa                 |
| **CPU en reposo**                                                         | 0.0 % (picos < 0.5 %)         | `pidstat 2 5`                                   |
| **Tiempo de arranque en frío**                                            | $\le$ 50 ms                   | `hyperfine`                                     |
| **Tiempo de reconexión de sesión**                                        | $\le$ 20 ms                   | Benchmark de socket PTY                         |
| **Sobrecarga de I/O de telemetría por tipeo**                             | $\le$ 1 ms (buffer asíncrono) | Benchmark interno de eventos                    |
| **Tiempo de refresco de estado Git (CLI)**                                | $\le$ 30 ms                   | Benchmark de consulta status CLI                |
| **Tamaño del binario empaquetado**                                        | $\le$ 25 MiB                  | `du -sb` (LTO + strip + docs libc, sin libgit2) |
| **Procesos persistentes por alumno**                                      | 1 (`jonesy`) + 1 (shell PTY)  | `pstree`                                        |
| **Tráfico en reposo (Red/Terminal)**                                      | 0 bytes/segundo               | `tcpdump` / monitor de socket                   |
| **Primer render de Markdown con estilos**                                 | $\le$ 15 ms                   | `jonesy --benchmark-markdown`                   |
| **Respuesta de panel `quick-doc` al cursor y tipeo**                      | $\le$ 70 ms                   | Benchmark de lookup de símbolos                 |

---

## 11. Priorización MoSCoW

| Categoría                            | Requisitos incluidos                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Must** (MVP Examen y Laboratorio)  | **RF-1.1–1.3** (Edición sobre cuerda, atajos F-keys, mouse) · **RF-2.1–2.2** (Compilación F6 y salto a errores) · **RF-3.1–3.3** (Ripley virtual text y explicación F1) · **RF-4.1–4.3** (Depurador visual y vista gráfica `cat-scan`) · **RF-5.1–5.2** (Terminal PTY `deck` interna con parser virtual VT100) · **RF-6.1–6.5** (Modo examen, consigna protegida, telemetría forense estructurada en §9.1, entrega asistida) · **RF-7.1–7.2** (OSC 52 y bracketed paste) · **RF-8.1–8.2** (Edición y visor Markdown con estilos ANSI) · **RF-9.1–9.4** (Panel `quick-doc` contextual al tipear o posar cursor, opción derecho/inferior) · **RF-10.1–10.4** (Extensibilidad Rust + Worker Python IPC desacoplado) · **RF-11.1–11.4** (Git status, stage, commit y log vía CLI) · **RNF-A** · **RNF-B** · **Layout adaptativo (§9.2)** |
| **Should** (Experiencia ampliada)    | **RF-1.4–1.5** (Resaltado ligero, paleta fuzzy) · **RF-3.4** (Panel completo de reglas P1) · **RF-4.4** (Consola GDB subordinada) · **RF-6.6** (Cronómetro en vivo de examen) · **RF-8.3** (Navegación de hipervínculos internos) · **RF-11.5** (Salto rápido a terminal para operaciones Git avanzadas) · **RNF-C**                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| **Could** (Mejoras de comodidad)     | Visualizador de gráfico de velocidad de tipeo para el docente · Exportación de traza de memoria a archivo descargable · Editor interactivo de snippets para plugins Python · Selector de temas visuales adicionales · Deshacer último commit local (`git reset --soft HEAD~1`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| **Won't** (Excluido deliberadamente) | Modelos de lenguaje o asistentes de IA integrados · Servidores gráficos embebidos (X11/Wayland/WebEngine) · Servidores LSP pesados residentes en memoria (`clangd`) · Autocompletado que oculte la sintaxis del lenguaje al estudiante · Telemetría remota hacia servidores externos de analítica · Asistente visual complejo de rebase interactivo o merge de tres vías en la TUI (resuelto vía CLI en `deck`) · Reimplementación interna de Git con librerías nativas embebidas (`git2-rs`)                                                                                                                                                                                                                                                                                                                                        |

---

## 12. Criterios de aceptación globales

1. **Flujo cotidiano de desarrollo con Git integrado llamando al binario del
   host**: El estudiante clona o abre un TP desde plantilla. Modifica `lista.c`
   y en la barra de estado se actualiza ` main *1`. Abre el panel Git
   (`Alt+G`), presiona `Espacio` para mover el archivo a _Staged_ (ejecuta
   `git add lista.c` de fondo), pulsa `c` y redacta el mensaje de commit.
   Confirma con `Ctrl+Enter`. El commit se crea localmente invocando el binario
   `git` con su identidad, firma y hooks ya configurados en el host. Para hacer
   `git push`, presiona `Alt+T`, pasa a la terminal interna `deck` donde la
   sesión SSH/HTTPS ya está activa, ejecuta `git push` y regresa al código con
   `Alt+T`.

2. **Flujo de examen integral del estudiante con ayuda contextual en vivo
   (`quick-doc`)**: El alumno inicia su sesión asignada donde `jonesy` arranca
   en **Modo Examen**. En terminal amplia, el split izquierdo exhibe la consigna
   formateada; el split central contiene el archivo `.c`; y a la derecha (o en
   el dock inferior, conmutable con `Shift+F4`) se ubica el panel `quick-doc`.
   Cuando el estudiante escribe `malloc(`, inmediatamente `quick-doc` resalta el
   parámetro `size_t size`, avisa la regla Ripley asociada y recuerda que debe
   verificar si devuelve `NULL`. Compila con `F6`, presiona `F5` para depurar y
   avanza paso a paso con `F10` mientras el widget `cat-scan` muestra la pila y
   los bloques en el montón. Durante la sesión, la telemetría registra deltas y
   pegados con SHA-256 encadenado. Al finalizar, pulsa `[ Entregar ]`, `jonesy`
   corre los tests con `dredd`, sella la telemetría y confirma la entrega.

3. **Auditoría forense de autoría y detección de pegados**: Un docente toma el
   archivo `.jonesy/telemetry.log` generado y ejecuta
   `jonesy replay entrega.log`. La herramienta reproduce visualmente el proceso
   de tipeo del estudiante, señalando en rojo una inserción masiva producida
   mediante pegado (_bracketed paste_), con timestamp y longitud precisa.

4. **Edición fluida y formateada de Markdown**: El estudiante abre `informe.md`.
   `jonesy` permite escribir en el búfer mientras un panel adyacente muestra
   encabezados resaltados, bloques de código C coloreados y tablas con
   caracteres semigráficos (`┌─┬─┐`), renderizados directamente en la matriz de
   terminal con refresco inmediato tras una pausa de tipeo.

5. **Prueba de estrés de concurrencia para laboratorios**: Un servidor de
   cátedra de gama media (8 cores, 16 GiB RAM) levanta 100 sesiones concurrentes
   de `jonesy` en terminal con telemetría activa, git status periódico y panel
   `quick-doc` disponible. La memoria total ocupada en el servidor no supera los
   5.5 GiB de RAM y el consumo de CPU global en reposo se mantiene por debajo
   del 1.5 %.

6. **Prueba de corte de conexión y resiliencia de sesión**: Durante la edición
   de un ejercicio de examen, la terminal sufre una desconexión abrupta
   (`SIGHUP`). Al reingresar a la sesión, `jonesy` restablece el estado de los
   buffers, el cursor y el panel contextual exactamente en el último punto,
   habiendo persistido todos los eventos de telemetría pendientes en disco sin
   corrupción.

---

## 13. Fuera de alcance

- **Capa de transporte o emulación de terminal externa**: La provisión de
  emuladores externos, túneles SSH, multiplexores o infraestructura de
  aprovisionamiento de sesiones terminales es ajena a `jonesy`. El IDE asume
  únicamente que corre sobre un estándar VT100/xterm provisto por el host.
- **Reimplementación interna o bindings embebidos de Git**: Se descarta
  expresamente enlazar `libgit2` o `git2-rs`; toda interacción con Git se
  realiza invocando el binario del entorno.
- **Operaciones Git complejas en la TUI**: Rebase interactivo, editores de merge
  de tres vías en pantalla dividida gráfica o cuadros de diálogo para
  contraseñas de remotos (se resuelven exclusivamente mediante el binario `git`
  estándar dentro del emulador PTY `deck`).
- **Asistentes de IA**: No se incluye ningún tipo de autocompletado
  probabilístico ni ventanas de chat generativo dentro del IDE.
- **Telemetría remota o espionaje de red**: La telemetría es estrictamente local
  al host donde corre el binario; no realiza envíos automáticos de red hacia
  servidores de analítica ni almacena datos fuera del directorio de trabajo del
  examen.
- **Renderizado de gráficos multimedia**: Cero soporte para previsualización de
  imágenes mapa de bits o PDFs dentro de las celdas de terminal (se resuelven
  exclusivamente mediante cajas y caracteres semigráficos Unicode).

---

## 14. Autocrítica y análisis de riesgos resueltos

| \#      | Riesgo detectado                                                                         | Impacto                                                                                  | Resolución técnica incorporada en la especificación                                                                                                                                                                       |
| ------- | ---------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------- |
| **R-1** | Emuladores de terminal que capturan atajos comunes (`Ctrl+S`, `Ctrl+Q`).                 | Interrupción accidental del flujo o bloqueo de scroll por control de flujo XON/XOFF.     | Mapeo dual universal con teclas de función (`F2`, `F4`, `F5`, `F6`, `F10`) y desactivación explícita de `IXON` en Crossterm/termios al inicializar el modo raw.                                                           |
| **R-2** | Terminales del laboratorio sin fuentes monoespaciadas completas con glifos Unicode.      | Diagramas de memoria de `cat-scan`, íconos Git o tablas Markdown desalineadas.           | Degradar limpiamente a caracteres ASCII estándar (`+`, `-`, `                                                                                                                                                             | `, `>`) sin romper alineación geométrica. |
| **R-3** | Incompatibilidad con el portapapeles en conexiones remotas.                              | El alumno no puede intercambiar texto entre la sesión de examen y su portapapeles local. | Implementación estricta de secuencias estándar OSC 52 (RF-7.1) y modo de pegado protegido (_bracketed paste_, RF-7.2).                                                                                                    |
| **R-4** | Sobrecarga de I/O por escritura continua de eventos de telemetría de texto.              | Posible micro-fricción o lag al escribir a alta velocidad en discos lentos (eMMC/SD).    | Canal asíncrono no bloqueante `tokio::sync::mpsc` con vaciado por lotes en segundo plano cada 32 eventos o 500 ms (§9.1).                                                                                                 |
| **R-5** | Pantallas reducidas (80x24) colapsadas por tener abiertos la consigna, editor y ayuda.   | Superficie de edición de código excesivamente estrecha para el estudiante.               | Máquina de estados de layout adaptativo (§9.2) que permite ubicar `quick-doc` en el dock inferior (3 filas) o conmutar a pestañas de pantalla completa (`F7`–`F9`, `F4`) en viewports $\le 89$ columnas o $\le 24$ filas. |
| **R-6** | Contienda del GIL de Python o segfaults en bibliotecas C de extensiones.                 | Micro-fricción en la interfaz o colapso completo del IDE en pleno examen.                | Arquitectura out-of-process (§2.3 y §8): worker hijo aislado `jonesy-worker-py` sobre pipes con JSON-RPC. Si Python falla, `jonesy` no se cae.                                                                            |
| **R-7** | Sincronización entre comandos Git del IDE y operaciones manuales concurrentes en `deck`. | Inconsistencias temporales de índice (`index.lock`) o visualización desfasada del árbol. | Ambos invocan el mismo binario `git` del host. El IDE vigila `.git/HEAD` e índice vía `notify` con debounce de 50 ms y gestiona reintentos limpios ante bloqueos transitorios.                                            |
| **R-8** | Desincronización y saturación visual en la terminal embebida (`deck`).                   | Secuencias ANSI masivas (`cat /dev/urandom` o bucles `printf`) congelan Ratatui.         | Integración del parser virtual `vt100` en memoria desacoplado del PTY, proyectando fotogramas coalescentes a 30–60 FPS a Ratatui (§5.1).                                                                                  |

---

## 15. Apéndice A — Identidad y nombres de componentes

Manteniendo la línea estilística inspirada en hitos de la ciencia ficción del
ecosistema (`ripley`, `dredd`, `alucarD`):

| Nombre de componente                 | Módulo asociado                                 | Referencia y rol didáctico                                                                                                                                                                     |
| ------------------------------------ | ----------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **`jonesy`**                         | El entorno TUI completo (binario Rust)          | El gato de _Alien_: ágil, silencioso, inmune a caídas, no consume nada y sobrevive en cualquier nave.                                                                                          |
| **`cat-scan`**                       | Widget de visualización de memoria (Stack/Heap) | Juego de palabras entre el escaneo tomográfico (CT Scan) y el gato Jonesy: inspecciona las entrañas de la memoria del programa sin alterarlo.                                                  |
| **`deck`**                           | Panel de terminal interactiva PTY               | Referencia al cyberdeck de _Neuromante_: la consola de acceso directo al sistema operativo y al Git completo del host.                                                                         |
| **`quick-doc`**                      | Panel de documentación y trampas de funciones   | Asistente de referencia rápida que explica firmas, valores de retorno y errores comunes de libc, ubicable como split derecho o dock inferior, activo tanto bajo el cursor como en pleno tipeo. |
| **`telemetry-engine`**               | Módulo local de auditoría y replay de texto     | Registro forense encadenado para garantizar autoría y analizar el proceso de resolución en exámenes.                                                                                           |
| **`jonesy-py` / `jonesy-worker-py`** | Módulo de enlace y worker de extensiones Python | El puente de automatización para que los docentes y herramientas del ecosistema extiendan las capacidades del IDE sin recompilar.                                                              |
