# Casos de uso de jonesy

Qué se hace con jonesy, quién lo hace y cómo se resuelve en el IDE. Los casos siguen el
ciclo de trabajo de un ejercicio (abrir, escribir, compilar, correr, probar, depurar,
versionar) y después el examen, el trabajo docente y el mantenimiento del entorno.
Cada caso tiene una grabación de terminal (asciinema) con su explicación paso a paso en
[casos/](casos/README.md). Las teclas y menús están al día con [menus.md](menus.md);
los detalles de la entrega, en [entrega.md](entrega.md), y los de las herramientas del
ecosistema, en [integraciones.md](integraciones.md).

## Actores

| Actor | Quién es | Dónde trabaja |
|---|---|---|
| **Alumno** | cursa Programación I y resuelve los TP en C | su computadora, el laboratorio o un servidor por SSH, desde 80×24 |
| **Alumno en examen** | el mismo alumno durante una evaluación | una sesión de terminal provista por la cátedra, con `jonesy --exam` |
| **Docente** | diseña ejercicios, prepara exámenes y revisa entregas | su máquina; usa las herramientas del perfil docente (dredd, …) |
| **Administrador** | mantiene las máquinas o el servidor del examen | el host donde corren muchas sesiones de jonesy |

Las herramientas del ecosistema que aparecen en los casos (ripley, gaff, kaneda,
spunkmeyer, mem-trace…) son del lado del alumno; dredd es del perfil docente y jonesy no
la invoca.

## Resumen

| # | Caso de uso | Actor |
|---|---|---|
| CU-01 | Empezar a usar jonesy por primera vez | Alumno |
| CU-02 | Abrir un TP o un ejercicio | Alumno |
| CU-03 | Leer la consigna mientras se programa | Alumno |
| CU-04 | Escribir y editar código | Alumno |
| CU-05 | Consultar una función de la biblioteca estándar | Alumno |
| CU-06 | Moverse por el código y el proyecto | Alumno |
| CU-07 | Renombrar una variable o función | Alumno |
| CU-08 | Compilar y entender los errores | Alumno |
| CU-09 | Revisar el estilo con las reglas de la cátedra | Alumno |
| CU-10 | Formatear el código | Alumno |
| CU-11 | Ejecutar el programa | Alumno |
| CU-12 | Ejecutar con un archivo de entrada o con argumentos | Alumno |
| CU-13 | Comparar contra las salidas esperadas | Alumno |
| CU-14 | Correr las pruebas | Alumno |
| CU-15 | Correr un objetivo de make | Alumno |
| CU-16 | Entender por qué el programa se cortó | Alumno |
| CU-17 | Depurar paso a paso | Alumno |
| CU-18 | Revisar la memoria con valgrind | Alumno |
| CU-19 | Ver la memoria dibujada (cat-scan) | Alumno |
| CU-20 | Versionar con git | Alumno |
| CU-21 | Aprender C desde el IDE | Alumno |
| CU-22 | Escribir el informe en Markdown | Alumno |
| CU-23 | Usar la terminal integrada | Alumno |
| CU-24 | Trabajar sin mouse | Alumno |
| CU-25 | Ajustar la apariencia y llevarla a otra máquina | Alumno |
| CU-26 | Recuperar el trabajo tras un corte | Alumno |
| CU-27 | Practicar con un simulacro de examen | Alumno |
| CU-38 | Mirar por dentro un ejecutable, un `.o` o un `.a` | Alumno |
| CU-28 | Rendir un examen | Alumno en examen |
| CU-29 | Revisar y confirmar la entrega | Alumno en examen |
| CU-30 | Pausar el cronómetro del examen | Docente (en la máquina del alumno) |
| CU-31 | Configurar un proyecto o un examen | Docente |
| CU-32 | Verificar una entrega | Docente |
| CU-33 | Auditar la sesión de un examen | Docente |
| CU-34 | Extender jonesy con plugins y herramientas | Docente |
| CU-35 | Revisar la salud del entorno | Alumno, Administrador |
| CU-36 | Reportar un problema de jonesy | Alumno, Administrador |
| CU-37 | Alojar muchas sesiones en un servidor | Administrador |

---

## Alumno: el día a día

### CU-01 · Empezar a usar jonesy por primera vez

> Grabación: [CU-01-primer-uso.cast](casos/CU-01-primer-uso.cast) · [qué muestra](casos/CU-01-primer-uso.md)

- **Disparador:** el alumno instala jonesy y lo abre por primera vez.
- **Flujo:** se muestra la pantalla de bienvenida con las teclas del ciclo de trabajo y
  lo que falta en el entorno (gcc, gdb, valgrind…) con cómo instalarlo. Desde ahí, `R`
  inicia el **recorrido guiado**, que marca en la pantalla cada zona (menús, editor,
  consigna, quick-doc, barra de estado, panel de abajo, barra izquierda) y dice para qué
  sirve y cómo abrirla si está cerrada.
- **Resultado:** el alumno sabe dónde está cada cosa. La bienvenida, el recorrido y la
  guía rápida siguen en el menú Ayuda.

### CU-02 · Abrir un TP o un ejercicio

> Grabación: [CU-02-abrir-un-tp.cast](casos/CU-02-abrir-un-tp.cast) · [qué muestra](casos/CU-02-abrir-un-tp.md)

- **Disparador:** `jonesy` en el directorio del TP, `jonesy ~/tp3`, `jonesy lista.c`, o
  Archivo → Abrir proyecto (con los recientes).
- **Flujo:** jonesy busca la raíz (el ancestro con `.jonesy.toml` o `.git`), lee el
  Makefile general, los de `libs/*` y `ejercicios/*`, `library.json` y `tp.sh`, y
  restaura la sesión anterior (archivos abiertos, cursores, paneles, marcadores). Si no
  hay sesión, parte de las preferencias del usuario.
- **Resultado:** el proyecto abierto como se dejó; la vista de proyecto (Alt+Y) muestra
  bibliotecas, ejercicios y sus objetivos.

### CU-03 · Leer la consigna mientras se programa

> Grabación: [CU-03-leer-la-consigna.cast](casos/CU-03-leer-la-consigna.cast) · [qué muestra](casos/CU-03-leer-la-consigna.md)

- **Disparador:** el ejercicio tiene `consigna.md`, `enunciado.md` o `README.md`.
- **Flujo:** la consigna se muestra de solo lectura al lado del código, con el
  Markdown dibujado. Tab y Shift+Tab recorren sus enlaces y Enter los sigue. En
  pantallas chicas F7 la muestra y F8 vuelve al código.
- **Resultado:** no hace falta salir del IDE ni abrir un navegador para leer el
  enunciado.

### CU-04 · Escribir y editar código

> Grabación: [CU-04-escribir-y-editar.cast](casos/CU-04-escribir-y-editar.cast) · [qué muestra](casos/CU-04-escribir-y-editar.md)

- **Flujo:** editor con atajos conocidos (Ctrl+S, Ctrl+Z, Ctrl+C/V), mouse, resaltado de
  C, pares que se cierran solos, comentar (Ctrl+/), duplicar (Ctrl+D) y mover líneas
  (Alt+↑/↓), buscar (Ctrl+F) y reemplazar (Ctrl+H), pestañas y archivos nuevos (Ctrl+N).
- **Resultado:** el código queda guardado; al guardar corren las herramientas de la
  cátedra (CU-09).

### CU-05 · Consultar una función de la biblioteca estándar

> Grabación: [CU-05-quick-doc.cast](casos/CU-05-quick-doc.cast) · [qué muestra](casos/CU-05-quick-doc.md)

- **Disparador:** el alumno escribe una llamada (`scanf(`, `strcpy(`…) o deja el cursor
  sobre ella.
- **Flujo:** el **quick-doc** (F4) muestra la firma, qué devuelve, cada parámetro (el que
  se está escribiendo resaltado), las trampas típicas y las reglas de cátedra
  relacionadas. Son unas 80 funciones de libc/POSIX embebidas, sin red.
- **Resultado:** la duda se resuelve sin manpages ni navegador, también en el examen.

### CU-06 · Moverse por el código y el proyecto

> Grabación: [CU-06-moverse-por-el-codigo.cast](casos/CU-06-moverse-por-el-codigo.cast) · [qué muestra](casos/CU-06-moverse-por-el-codigo.md)

- **Flujo:** ir a la definición (F12 o Ctrl+clic) y volver (Alt+←), esquema del archivo
  (Alt+S), ir a una línea (Ctrl+G), buscar en el proyecto (Ctrl+Shift+F o Ctrl+F3, con
  los resultados en un panel), marcadores (Ctrl+F2, Shift+F2) y el problema siguiente o
  anterior (Alt+N, Alt+Shift+N). El explorador (Alt+E) crea, renombra y borra archivos.
- **Resultado:** el alumno llega a lo que busca sin recorrer los archivos a mano.

### CU-07 · Renombrar una variable o función

> Grabación: [CU-07-renombrar.cast](casos/CU-07-renombrar.cast) · [qué muestra](casos/CU-07-renombrar.md)

- **Disparador:** Ctrl+R (Editar → Renombrar en su ámbito) con el cursor sobre el nombre.
- **Flujo:** jonesy encuentra la declaración que vale en ese punto y propone el cambio
  sólo donde vale: una local que tapa a otra, un parámetro o la variable de un `for` no
  arrastran a otra con el mismo nombre, y no se tocan campos, cadenas ni comentarios. La
  ventana dice cuántas apariciones y en qué ámbito, y rechaza nombres inválidos, palabras
  reservadas o ya usados.
- **Resultado:** el cambio se aplica de una vez y se deshace con un solo Ctrl+Z.
- **Límite:** trabaja sobre el archivo abierto, no entre archivos.

### CU-08 · Compilar y entender los errores

> Grabación: [CU-08-compilar-y-entender-los-errores.cast](casos/CU-08-compilar-y-entender-los-errores.cast) · [qué muestra](casos/CU-08-compilar-y-entender-los-errores.md)

- **Disparador:** F6 o `[F6 Compilar]`.
- **Flujo:** compila el ejercicio del archivo activo con su Makefile (compilando antes
  las bibliotecas que usa) o el archivo con gcc. Cada error y advertencia aparece en
  Problemas, en su línea y con la columna subrayada, **explicado en español** (unos 60
  casos de gcc y del enlazador). En una cascada se marca el primero («empezá por acá»).
  F1 sobre la línea abre la explicación con «así no / así sí»; desde la explicación, `g`
  lleva a los términos del glosario que menciona.
- **Resultado:** el alumno sabe qué corregir y dónde.

### CU-09 · Revisar el estilo con las reglas de la cátedra

> Grabación: [CU-09-estilo-de-la-catedra.cast](casos/CU-09-estilo-de-la-catedra.cast) · [qué muestra](casos/CU-09-estilo-de-la-catedra.md)

- **Disparador:** guardar el archivo.
- **Flujo:** ripley, gaff, kaneda y spunkmeyer revisan el código y sus hallazgos
  aparecen al final de la línea y en el panel de reglas (Alt+R), agrupados por regla. F1
  explica cualquier hallazgo con el mismo formato que los errores de gcc. **Ayuda →
  Reglas de la cátedra** es el catálogo navegable con ejemplos de cada regla.
- **Resultado:** el código cumple el estilo antes de entregarlo.

### CU-10 · Formatear el código

> Grabación: [CU-10-formatear.cast](casos/CU-10-formatear.cast) · [qué muestra](casos/CU-10-formatear.md)

- **Disparador:** Alt+Shift+F (Editar → Autoformatear).
- **Flujo:** formatea con gaff o con `clang-format` (con el `.clang-format` de la
  cátedra si no hay otro). Las acciones de las herramientas (gaff fix, tetsuo, motoko…)
  están en Alt+H y en el menú Plugins.
- **Resultado:** el archivo queda con el formato de la cátedra.

### CU-11 · Ejecutar el programa

> Grabación: [CU-11-ejecutar.cast](casos/CU-11-ejecutar.cast) · [qué muestra](casos/CU-11-ejecutar.md)

- **Disparador:** Ctrl+F5 o `[^F5 Ejecutar]`.
- **Flujo:** compila y corre el programa en la terminal interna (una PTY real), donde el
  alumno interactúa con él. Al terminar se informan el código de salida, el tiempo y la
  memoria máxima; si parece un ciclo infinito, se avisa y Shift+F5 lo corta.
- **Resultado:** el programa corrió dentro del IDE.

### CU-12 · Ejecutar con un archivo de entrada o con argumentos

> Grabación: [CU-12-entrada-y-argumentos.cast](casos/CU-12-entrada-y-argumentos.cast) · [qué muestra](casos/CU-12-entrada-y-argumentos.md)

- **Disparador:** Construir → Ejecutar con entrada… o Argumentos del programa….
- **Flujo:** se elige un archivo para la entrada estándar (o el teclado) y los
  argumentos; se recuerdan por ejercicio y Ctrl+F5 los usa.
- **Resultado:** no hay que tipear la misma entrada en cada prueba.

### CU-13 · Comparar contra las salidas esperadas

> Grabación: [CU-13-comparar-salidas.cast](casos/CU-13-comparar-salidas.cast) · [qué muestra](casos/CU-13-comparar-salidas.md)

- **Disparador:** Construir → Comparar con las salidas esperadas.
- **Flujo:** corre el programa con cada `entradaN.txt` y compara con `salidaN.txt`; cada
  caso dice si pasa y, si no, dónde difiere.
- **Resultado:** el alumno sabe qué casos fallan antes de entregar.

### CU-14 · Correr las pruebas

> Grabación: [CU-14-correr-las-pruebas.cast](casos/CU-14-correr-las-pruebas.cast) · [qué muestra](casos/CU-14-correr-las-pruebas.md)

- **Disparador:** Ctrl+F6 (`[^F6 Probar]`), o Shift+F6 para la prueba del cursor.
- **Flujo:** corre `make test` del ejercicio (u otro framework configurado que informe
  en SARIF). Cada fallo de `p1_test` queda en su línea con lo esperado y lo obtenido; los
  cortes, en la definición de la prueba.
- **Resultado:** las pruebas que fallan, listas para revisar desde Problemas.

### CU-15 · Correr un objetivo de make

> Grabación: [CU-15-objetivo-de-make.cast](casos/CU-15-objetivo-de-make.cast) · [qué muestra](casos/CU-15-objetivo-de-make.md)

- **Disparador:** Shift+F9 o Construir → Correr un objetivo de make…; también desde los
  comandos (F1 o Ctrl+K, con el prefijo «make:») y desde la vista de proyecto (Alt+Y).
- **Flujo:** la lista muestra los objetivos de todos los Makefile, con su descripción:
  primero los del ejercicio o la biblioteca del archivo activo, después el general, las
  bibliotecas y los demás ejercicios. Se filtra escribiendo y Enter lo corre. Los
  comandos de `tp.sh` están en los comandos y en la vista de proyecto.
- **Resultado:** cualquier objetivo (`all`, `test`, `memcheck`, `clean`, `doc`…) se corre
  sin mouse ni terminal; la salida va al panel de abajo.

### CU-16 · Entender por qué el programa se cortó

> Grabación: [CU-16-el-programa-se-corto.cast](casos/CU-16-el-programa-se-corto.cast) · [qué muestra](casos/CU-16-el-programa-se-corto.md)

- **Disparador:** el programa termina por una señal (violación de segmento, abort,
  división por cero…).
- **Flujo:** la terminal y la barra explican la señal en español y aparece
  `[F5 Ver dónde falló]`, que corre el depurador hasta la línea del alumno donde se
  produjo (saliendo de libc).
- **Resultado:** el alumno ve la línea y el estado en el momento del error.

### CU-17 · Depurar paso a paso

> Grabación: [CU-17-depurar-paso-a-paso.cast](casos/CU-17-depurar-paso-a-paso.cast) · [qué muestra](casos/CU-17-depurar-paso-a-paso.md)

- **Disparador:** F5, un breakpoint en el margen (clic o Ctrl+B) o Ctrl+F10 (correr
  hasta el cursor).
- **Flujo:** depurador visual sobre GDB: F10 paso, F11 entrar, Shift+F11 salir, F5
  continuar; la pila y las variables en el panel; la consola de gdb para lo avanzado.
- **Resultado:** el alumno sigue la ejecución y encuentra dónde cambia lo que no
  esperaba.

### CU-18 · Revisar la memoria con valgrind

> Grabación: [CU-18-valgrind.cast](casos/CU-18-valgrind.cast) · [qué muestra](casos/CU-18-valgrind.md)

- **Disparador:** Depurar → Revisar memoria con valgrind.
- **Flujo:** corre `make memcheck` o el programa bajo valgrind y deja cada acceso
  inválido, uso después de `free`, `free` doble y fuga en la línea del alumno, con dónde
  se pidió o liberó el bloque.
- **Resultado:** las fugas y los accesos inválidos explicados en su lugar.

### CU-19 · Ver la memoria dibujada (cat-scan)

> Grabación: [CU-19-cat-scan.cast](casos/CU-19-cat-scan.cast) · [qué muestra](casos/CU-19-cat-scan.md)

- **Disparador:** Alt+M durante la depuración.
- **Flujo:** con la traza de mem-trace, dibuja el stack y el heap con flechas de los
  punteros a lo que apuntan, los colgantes en rojo, y los eventos del paso. Se puede
  exportar a HTML.
- **Resultado:** el alumno ve qué pasa con sus punteros en cada línea.

### CU-20 · Versionar con git

> Grabación: [CU-20-git.cast](casos/CU-20-git.cast) · [qué muestra](casos/CU-20-git.md)

- **Disparador:** Alt+G (panel de git) o el menú Git.
- **Flujo:** agregar y sacar archivos del commit (Espacio, `a`, `u`), commit con revisión
  del mensaje (`c`), historial (`l`), diff lado a lado del archivo, del commit o entre
  dos commits, deshacer el último commit (`z`), subir y bajar (`s`, `b`) con ayuda ante
  conflictos, y clonar un repositorio. Usa la identidad y credenciales del alumno.
- **Resultado:** el TP queda versionado con commits claros, sin salir del IDE.

### CU-21 · Aprender C desde el IDE

> Grabación: [CU-21-aprender-c.cast](casos/CU-21-aprender-c.cast) · [qué muestra](casos/CU-21-aprender-c.md)

- **Flujo:** el menú Ayuda reúne el **glosario** de C (puntero, heap, segfault, fuga…),
  las **tablas de consulta** (tipos con sus tamaños en esa máquina, printf/scanf,
  secuencias de escape, precedencia, ASCII), el **catálogo de reglas** de la cátedra y la
  guía rápida. Editar → **Ver la precedencia de la expresión** muestra la línea con sus
  paréntesis implícitos y avisa trampas como `x & 1 == 0` o `*p++`.
- **Resultado:** las dudas de concepto se resuelven en el mismo lugar donde se programa.

### CU-22 · Escribir el informe en Markdown

> Grabación: [CU-22-informe-markdown.cast](casos/CU-22-informe-markdown.cast) · [qué muestra](casos/CU-22-informe-markdown.md)

- **Flujo:** los `.md` se editan con resaltado y Ctrl+P muestra la vista previa dibujada
  (tablas, listas, código, enlaces recorribles con Tab).
- **Resultado:** el informe del TP se escribe y revisa dentro del IDE.

### CU-23 · Usar la terminal integrada

> Grabación: [CU-23-terminal-integrada.cast](casos/CU-23-terminal-integrada.cast) · [qué muestra](casos/CU-23-terminal-integrada.md)

- **Disparador:** Alt+T o F9 en el modo compacto.
- **Flujo:** una shell real dentro del IDE (con las variables y el `PATH` del entorno de
  la cátedra si está instalado) para lo que no tiene botón.
- **Resultado:** no hace falta otra ventana para la línea de comandos.

### CU-24 · Trabajar sin mouse

> Grabación: [CU-24-sin-mouse.cast](casos/CU-24-sin-mouse.cast) · [qué muestra](casos/CU-24-sin-mouse.md)

- **Flujo:** todo tiene tecla. Los menús se abren con Alt+letra o F10 y la letra; los
  comandos (Ctrl+K, o F1 donde no hay un error que explicar) buscan cualquier acción,
  objetivo de make, comando de `tp.sh`, plugin o herramienta por nombre. En cada panel,
  `?` muestra sus atajos; Ayuda → Atajos de teclado, todos. Alt+Z maximiza el panel con
  el foco.
- **Resultado:** el IDE se usa entero desde el teclado, también por SSH.

### CU-25 · Ajustar la apariencia y llevarla a otra máquina

> Grabación: [CU-25-apariencia-y-preferencias.cast](casos/CU-25-apariencia-y-preferencias.cast) · [qué muestra](casos/CU-25-apariencia-y-preferencias.md)

- **Disparador:** Archivo → Configuración del entorno (Ctrl+,).
- **Flujo:** elegir tema (oscuro, claro, contraste, turbo, solarizado, fósforo), posición
  del quick-doc, tamaños de paneles e integraciones encendidas. **Exportar mis
  preferencias** las guarda en un `.toml` junto con los manifiestos de integraciones
  propios; **Importar** las aplica en otra máquina (también `jonesy config
  exportar|importar`) y quedan como punto de partida de los proyectos nuevos.
- **Resultado:** el mismo entorno en la casa, el laboratorio y el servidor.

### CU-26 · Recuperar el trabajo tras un corte

> Grabación: [CU-26-recuperar-tras-un-corte.cast](casos/CU-26-recuperar-tras-un-corte.cast) · [qué muestra](casos/CU-26-recuperar-tras-un-corte.md)

- **Disparador:** se cerró la terminal o se cortó la conexión con cambios sin guardar.
- **Flujo:** jonesy deja copias de recuperación al recibir la señal; al volver a abrir el
  proyecto ofrece restaurarlas (`s`), descartarlas (`n`) o conservarlas para después
  (Esc).
- **Resultado:** no se pierde lo escrito.

### CU-27 · Practicar con un simulacro de examen

> Grabación: [CU-27-simulacro.cast](casos/CU-27-simulacro.cast) · [qué muestra](casos/CU-27-simulacro.md)

- **Disparador:** Archivo → Simulacro de examen… o `jonesy --simulacro [min]`.
- **Flujo:** cronómetro y avisos de tiempo como en el examen, sin restricciones ni
  sellado. F12 lo termina y muestra cómo fue (compilaciones, cuántas con errores,
  ejecuciones, pruebas).
- **Resultado:** el alumno llega al examen sabiendo cómo funciona.

### CU-38 · Mirar por dentro un ejecutable, un `.o` o un `.a`

> Grabación: [CU-38-visor-hexadecimal.cast](casos/CU-38-visor-hexadecimal.cast) · [qué muestra](casos/CU-38-visor-hexadecimal.md)

- **Disparador:** abrir un binario desde el explorador o la línea de comandos, o Ver →
  Visor hexadecimal, que lista los binarios del proyecto.
- **Flujo:** el binario se abre en una pestaña más del editor (los paneles, el
  explorador y la barra de estado siguen como estaban; Ctrl+W la cierra) que muestra
  offset, bytes y ASCII en lugar de cargarlo como texto. Las flechas mueven el cursor; abajo se ve el byte en hexadecimal, decimal y como
  carácter, y los 4 bytes desde ahí leídos como `int`. `g` (o Ctrl+G) va a un offset, `/` (o Ctrl+F) busca
  texto o bytes (`7f 45 4c 46`) y `n` la siguiente coincidencia. `i` describe el
  archivo: tipo (ejecutable, objeto reubicable, biblioteca), arquitectura y las
  secciones del ELF explicadas (`.text`, `.data`, `.bss`, `.rodata`…) con su offset, o
  los objetos que contiene un `.a`. `s` muestra los símbolos con `nm`: las funciones
  que define (T) y las que usa de otro lado (U), lo que explica un «undefined
  reference». `?` muestra todos los atajos.
- **Resultado:** el alumno ve qué produce el compilador y qué busca el enlazador; el
  binario no se modifica.

---

## Alumno en examen

### CU-28 · Rendir un examen

> Grabación: [CU-28-rendir-un-examen.cast](casos/CU-28-rendir-un-examen.cast) · [qué muestra](casos/CU-28-rendir-un-examen.md)

- **Disparador:** la cátedra abre `jonesy --exam` (o `JONESY_EXAM_MODE=1`).
- **Flujo:** la consigna se muestra de solo lectura; el cronómetro avisa los minutos que
  quedan y cuándo se termina el tiempo; todo se guarda solo cada 30 s (la
  barra dice cuándo). La bitácora registra la sesión con SHA-256 encadenado (lo pegado
  queda marcado). Se niegan las acciones que no corresponden: abrir otro proyecto,
  clonar, renombrar o borrar archivos, editar `.jonesy.toml`, crear plugins o aplicar
  correcciones automáticas.
- **Resultado:** el alumno trabaja con las mismas herramientas de siempre y la sesión
  queda registrada.

### CU-29 · Revisar y confirmar la entrega

> Grabación: [CU-29-entregar.cast](casos/CU-29-entregar.cast) · [qué muestra](casos/CU-29-entregar.md)

- **Disparador:** F12 o `[ Entregar Examen ]`.
- **Flujo:** antes de confirmar, la ventana lista los archivos que van en el paquete
  (avisa si un `.c` abierto queda afuera) y cómo terminaron la última compilación y las
  últimas pruebas: si fallaron, si no se corrieron o si el código cambió después. Al
  confirmar se guarda todo, se corre la verificación (`[examen] verificar`, por ejemplo
  `ripley run *.c --practica examen.ripkg`, o `make test`), se sella la bitácora y se
  arma el paquete `.tar` con su SHA-256.
- **Resultado:** la entrega queda registrada, los archivos pasan a solo lectura y el
  alumno ve el SHA-256 que la identifica.

### CU-30 · Pausar el cronómetro del examen

> Grabación: [CU-30-pausa-del-examen.cast](casos/CU-30-pausa-del-examen.cast) · [qué muestra](casos/CU-30-pausa-del-examen.md)

- **Actor:** el docente, en la máquina del alumno.
- **Disparador:** un corte de luz, una consulta larga u otro imprevisto.
- **Flujo:** Examen → Pausar; el docente escribe su código (se ve con asteriscos), que
  se compara con `[examen] pausa_sha256`. En pausa el cronómetro no corre, la barra dice
  EN PAUSA y no se puede editar; la pausa sobrevive a un reinicio. Reanudar no pide
  código.
- **Resultado:** el alumno no pierde tiempo por algo ajeno; la bitácora registra cada
  pausa, cada reanudación y cada código rechazado.

---

## Docente

### CU-31 · Configurar un proyecto o un examen

> Grabación: [CU-31-configurar-el-proyecto.cast](casos/CU-31-configurar-el-proyecto.cast) · [qué muestra](casos/CU-31-configurar-el-proyecto.md)

- **Flujo:** en `.jonesy.toml` (ver [ejemplo.jonesy.toml](ejemplo.jonesy.toml)): comando
  de compilación, programa y argumentos para depurar, descripciones de objetivos,
  integraciones activas, pruebas en SARIF y, para el examen, consigna, duración,
  verificación, archivos de la entrega, plugins autorizados (por SHA-256) y el hash del
  código de pausa (largo y al azar: el alumno puede leer el archivo).
- **Resultado:** el TP o el examen se comportan como la cátedra necesita sin cambiar
  jonesy.

### CU-32 · Verificar una entrega

> Grabación: [CU-32-verificar-una-entrega.cast](casos/CU-32-verificar-una-entrega.cast) · [qué muestra](casos/CU-32-verificar-una-entrega.md)

- **Disparador:** llega un paquete `entrega-<alumno>-<ts>.tar`.
- **Flujo:** `jonesy verificar <paquete.tar> [--json]` comprueba el SHA-256 del paquete,
  cada archivo contra el manifiesto, la cadena de la bitácora, su sello y que la sesión
  esté cerrada. Las herramientas del perfil docente (dredd) pueden usar la salida JSON.
- **Resultado:** el docente sabe si la entrega es íntegra (código 0) o no (código 1).

### CU-33 · Auditar la sesión de un examen

> Grabación: [CU-33-auditar-la-sesion.cast](casos/CU-33-auditar-la-sesion.cast) · [qué muestra](casos/CU-33-auditar-la-sesion.md)

- **Flujo:** `jonesy replay <telemetry.log>` reproduce la sesión tecla a tecla;
  `--resumen` y `--json` dan el resumen: integridad de la cadena, tiempo, cantidad de
  pegados y caracteres pegados, velocidad de tipeo por minuto.
- **Resultado:** el docente puede distinguir el trabajo propio de los pegados masivos.

### CU-34 · Extender jonesy con plugins y herramientas

> Grabación: [CU-34-plugins.cast](casos/CU-34-plugins.cast) · [qué muestra](casos/CU-34-plugins.md)

- **Flujo:** plugins en Python en `.jonesy/plugins/*.py` (funciones `comando_*` que
  aparecen en el menú Plugins y en los comandos; errores en Problemas en su línea; ver
  [plugin-ejemplo.py](plugin-ejemplo.py)), y manifiestos de integraciones para sumar
  herramientas que corren al guardar o a mano, con su catálogo de reglas (ver
  [integraciones.md](integraciones.md)).
- **Resultado:** validadores y herramientas propias de la cátedra dentro del IDE.

---

## Mantenimiento del entorno

### CU-35 · Revisar la salud del entorno

> Grabación: [CU-35-doctor.cast](casos/CU-35-doctor.cast) · [qué muestra](casos/CU-35-doctor.md)

- **Actor:** el alumno al instalar, o el administrador al preparar las máquinas.
- **Disparador:** `jonesy doctor [--rapido] [--json]` o Ayuda → Diagnóstico del entorno.
- **Flujo:** revisa compilar (gcc, make), depurar (gdb, ptrace), memoria (valgrind,
  mem-trace), git y su identidad, la terminal (UTF-8, colores), Python, el proyecto y las
  herramientas del ecosistema; sin `--rapido` compila, depura y revisa un programa de
  prueba. Cada falta viene con para qué sirve y cómo instalarla.
- **Resultado:** se sabe qué falta antes de que falle en medio de un TP o un examen
  (código 1 si falta algo requerido).

### CU-36 · Reportar un problema de jonesy

> Grabación: [CU-36-reportar-un-problema.cast](casos/CU-36-reportar-un-problema.cast) · [qué muestra](casos/CU-36-reportar-un-problema.md)

- **Disparador:** algo no anda como debería.
- **Flujo:** `jonesy reporte [--salida archivo.md]` o Ayuda → Reportar un problema arma
  un informe con la versión, el sistema, la terminal, un doctor rápido, el
  `.jonesy.toml`, el último error de la sesión y el último pánico. No incluye código, el
  directorio personal aparece como `~` y se ocultan la identidad de git y el hash de la
  pausa. Desde el IDE se guarda en `.jonesy/` y se copia.
- **Resultado:** un reporte completo para pegar en el aviso del problema.

### CU-37 · Alojar muchas sesiones en un servidor

> Grabación: [CU-37-muchas-sesiones.cast](casos/CU-37-muchas-sesiones.cast) · [qué muestra](casos/CU-37-muchas-sesiones.md)

- **Actor:** el administrador del servidor de examen.
- **Flujo:** un binario único, sin dependencias de ejecución más allá de las
  herramientas del `PATH`; arranque inmediato, poco consumo en reposo (no redibuja si
  nada cambió) y el estado de cada sesión en el directorio del proyecto.
  `bin/estres-sesiones` mide el consumo con muchas sesiones simultáneas.
- **Resultado:** decenas de alumnos en el mismo host sin degradar el servicio.
