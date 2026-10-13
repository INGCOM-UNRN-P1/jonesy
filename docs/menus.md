# Menús de jonesy

La barra sigue el ciclo de trabajo de un ejercicio: **archivos → edición → vistas →
construir (compilar, correr, probar) → depurar (paso a paso y memoria) → git**, más los
**plugins** del proyecto. Todo lo
que está en un menú también se encuentra en la **paleta de comandos** (Ctrl+K), que busca
por nombre. La reorganización sigue [la crítica y propuesta](critica_y_propuesta_de_reorganizacion.md).

## Cómo se usan

| Qué | Cómo |
|---|---|
| Abrir un menú | clic en su nombre, **Alt+letra subrayada**, o **F10** y la letra |
| Moverse | ← → cambia de menú, ↑ ↓ elige la entrada (los separadores se saltean) |
| Ejecutar | Enter o clic en la entrada |
| Cerrar sin hacer nada | Esc, F10 o clic fuera del menú |

- Cada menú tiene una **letra subrayada**. **F10 y la letra** abre cualquier menú; con
  **Alt+letra** se abre directo, salvo Editar (Alt+E es el explorador), Git (Alt+G es el
  panel de git) y Ayuda (Alt+Y es la vista del proyecto), que se abren con F10 y la letra.
- **F10 durante la depuración** es «Paso» (siguiente línea): para abrir un menú en ese
  momento, usá Alt+letra o el clic.
- Alt+S abre el esquema del archivo en la barra izquierda (no es un menú).
- Alt+N y Alt+Shift+N van al problema siguiente y anterior (no son menús).
- Alt+Z maximiza el panel que tiene el foco y lo vuelve a su tamaño (no es un menú).
- Con el foco en la terminal, Alt+letra va a la shell (Alt+D, Alt+C… son atajos de bash).
- A la derecha de cada entrada está su **atajo global**. Las teclas de una sola letra del
  panel de git (`a`, `u`, `c`, `l`, `z`, `m`) no aparecen en los menús: se muestran al pie
  del panel cuando tiene el foco.

## La barra de estado

Abajo a la derecha hay botones para lo de todos los días (se usan con clic o con su
tecla):

```text
 [F6 Compilar] [^F5 Ejecutar] [^F6 Probar] [F5 Depurar] [Terminal]
```

- `[^F6 Probar]` aparece sólo si el ejercicio (o la raíz) tiene objetivo `test`.
- `[F5 Depurar]` pasa a `[F5 Continuar]` durante la depuración y a
  `[F5 Ver dónde falló]` después de que el programa se cortó por un error.
- Si el proyecto usa documentación generada y falta o quedó vieja, aparece primero un
  botón para regenerarla (`[sin doc: make doc]`).

## Configuración del entorno

**Archivo → Configuración del entorno…** (Ctrl+, en las terminales que transmiten esa
combinación) reúne lo que configura el IDE, separado de lo que hace el programa. Cuatro
solapas (← → o 1-4):

| Solapa | Qué hay |
|---|---|
| **Integraciones y linters** | cada herramienta del ecosistema (ripley, gaff, kaneda…) con su estado y cuándo corre; Enter o Espacio la activa o apaga. Las acciones de las herramientas para el archivo activo (Alt+H) y releer los manifiestos. |
| **Plugins** | los `.jonesy/plugins/*.py` con sus comandos o errores (Enter abre el script), crear uno nuevo y recargarlos. |
| **Compilador y formato** | el comando con el que se compila y el formateador que se usa, y abrir `.jonesy.toml` o `.clang-format` para cambiarlos (se crean si no existen). |
| **Apariencia** | tema (oscuro, claro, contraste, turbo, solarizado, fósforo), posición del quick-doc, tamaños de paneles y restablecerlos. |

## Contenido que depende del proyecto

- **Construir → Ver ejercicios y objetivos del proyecto** (Alt+Y) muestra las
  bibliotecas, ejercicios y objetivos de make y `tp.sh`, listos para correr.
- El menú **Plugins** (Alt+L) se arma con lo configurado en el proyecto y cada entrada
  se ejecuta con Enter o clic:
  - los comandos de los **plugins Python** de `.jonesy/plugins` (las funciones
    `comando_*`; ver `docs/plugin-ejemplo.py`), con el resultado en una ventana;
  - las **acciones de las herramientas** del ecosistema que aplican al archivo activo
    (gaff fix, tetsuo, motoko, hal…; también con Alt+H). Ver
    [integraciones.md](integraciones.md);
  - y al final, configurar, crear y recargar los plugins.

  Las dos listas también están en la paleta (Ctrl+K).

## Modo examen

**Archivo → Simulacro de examen…** (o `jonesy --simulacro [min]`) practica con el
cronómetro y los avisos del examen pero sin restricciones ni entrega; F12 lo termina y
muestra cómo fue.

Con `--exam` aparece el menú **Examen**: entregar (F12) y ver el estado de la entrega
(tiempo, autoguardado, bitácora y paquete). Algunas entradas se niegan con un aviso: abrir
otro proyecto, clonar, renombrar o borrar archivos, crear plugins, editar `.jonesy.toml` y
las acciones de herramientas que corrigen el código (por ejemplo `gaff fix`).

## Los menús

Lo que sigue se genera desde el código (`crates/jonesy-tui/src/accion.rs`); una prueba
comprueba que esté al día.

<!-- menus: inicio (generado por accion::documento_menus) -->

```text
  Archivo  Editar  Ver  Construir  Depurar  Git  Plugins  Examen  Ayuda 
```

| Menú | Letra | Se abre con | Entradas |
|---|---|---|---|
| Archivo | [A]rchivo | Alt+A (o F10 y A) | 11 |
| Editar | [E]ditar | F10 y E | 24 |
| Ver | [V]er | Alt+V (o F10 y V) | 16 |
| Construir | [C]onstruir | Alt+C (o F10 y C) | 10 |
| Depurar | [D]epurar | Alt+D (o F10 y D) | 10 |
| Git | [G]it | F10 y G | 8 |
| Plugins | P[l]ugins | Alt+L (o F10 y L) | 4 |
| Examen | E[x]amen | Alt+X (o F10 y X) | 3 |
| Ayuda | A[y]uda | F10 y Y | 12 |

## Archivo

```text
┌──────────────────────────────────────────────┐
│ Nuevo archivo…                        Ctrl+N │
│ Abrir archivo…                        Ctrl+O │
│ Abrir proyecto o directorio…                 │
├──────────────────────────────────────────────┤
│ Guardar                          Ctrl+S / F2 │
│ Guardar todo                    Ctrl+Shift+S │
├──────────────────────────────────────────────┤
│ Cerrar pestaña                        Ctrl+W │
│ Pestaña siguiente                   Ctrl+Tab │
│ Pestaña anterior              Ctrl+Shift+Tab │
├──────────────────────────────────────────────┤
│ Simulacro de examen…                         │
│ Configuración del entorno…            Ctrl+, │
├──────────────────────────────────────────────┤
│ Salir                                 Ctrl+Q │
└──────────────────────────────────────────────┘
```

## Editar

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│ Deshacer                                                             Ctrl+Z │
│ Rehacer                                                              Ctrl+Y │
├─────────────────────────────────────────────────────────────────────────────┤
│ Cortar                                                               Ctrl+X │
│ Copiar                                                               Ctrl+C │
│ Pegar                                                                Ctrl+V │
│ Seleccionar todo                                                     Ctrl+A │
├─────────────────────────────────────────────────────────────────────────────┤
│ Buscar…                                                         F3 / Ctrl+F │
│ Buscar siguiente                                                   Shift+F3 │
│ Buscar y reemplazar…                                                 Ctrl+H │
│ Buscar en el proyecto…                                         Ctrl+Shift+F │
│ Ir a la línea…                                                       Ctrl+G │
│ Ir a la definición                                          F12 / Ctrl+clic │
│ Volver a donde estaba                                                 Alt+← │
├─────────────────────────────────────────────────────────────────────────────┤
│ Poner / quitar marcador                                             Ctrl+F2 │
│ Ir al marcador siguiente                                           Shift+F2 │
│ Marcadores…                                                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│ Comentar / descomentar líneas                                        Ctrl+/ │
│ Duplicar línea                                                       Ctrl+D │
│ Mover línea arriba                                                    Alt+↑ │
│ Mover línea abajo                                                     Alt+↓ │
├─────────────────────────────────────────────────────────────────────────────┤
│ Autoformatear código (gaff o clang-format)                      Alt+Shift+F │
│ Quitar espacios al final de las líneas                                      │
│ Renombrar en su ámbito…                                              Ctrl+R │
│ Ver la precedencia de la expresión (paréntesis implícitos)                  │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Ver

```text
┌──────────────────────────────────────────────────────┐
│ Modo código                                       F8 │
│ Modo consigna                                     F7 │
│ Modo terminal integrada                           F9 │
├──────────────────────────────────────────────────────┤
│ Explorador de archivos                         Alt+E │
│ Esquema del archivo (funciones y tipos)        Alt+S │
│ Problemas y diagnósticos                       Alt+P │
│ Ir al problema siguiente                       Alt+N │
│ Ir al problema anterior                  Alt+Shift+N │
│ Salida de la compilación                       Alt+O │
│ Terminal (deck)                                Alt+T │
│ Abrir panel interactivo de Git                 Alt+G │
│ Panel de reglas pedagógicas (ripley)           Alt+R │
├──────────────────────────────────────────────────────┤
│ Documentación contextual (quick-doc)              F4 │
│ Vista previa de Markdown                      Ctrl+P │
├──────────────────────────────────────────────────────┤
│ Maximizar el panel actual (y volver)           Alt+Z │
│ Restablecer disposición de paneles                   │
└──────────────────────────────────────────────────────┘
```

## Construir

```text
┌───────────────────────────────────────────────────────────────────┐
│ Compilar archivo o proyecto                                    F6 │
│ Compilar y ejecutar                                       Ctrl+F5 │
│ Ejecutar con entrada… (archivo de entrada para Ctrl+F5)           │
│ Argumentos del programa…                                          │
│ Comparar con las salidas esperadas                                │
│ Correr pruebas (make test)                                Ctrl+F6 │
│ Correr la prueba del cursor                              Shift+F6 │
├───────────────────────────────────────────────────────────────────┤
│ Limpiar compilación (make clean)                                  │
│ Generar documentación (make doc)                                  │
├───────────────────────────────────────────────────────────────────┤
│ Ver ejercicios y objetivos del proyecto                     Alt+Y │
└───────────────────────────────────────────────────────────────────┘
```

## Depurar

```text
┌───────────────────────────────────────────────────────────┐
│ Iniciar depuración / continuar                         F5 │
│ Detener depuración                               Shift+F5 │
│ Poner / quitar breakpoint                          Ctrl+B │
├───────────────────────────────────────────────────────────┤
│ Paso a paso (siguiente línea)                         F10 │
│ Entrar en la función (step into)                      F11 │
│ Salir de la función (step out)                  Shift+F11 │
│ Correr hasta el cursor                           Ctrl+F10 │
├───────────────────────────────────────────────────────────┤
│ Revisar memoria con valgrind (fugas y accesos)            │
│ Inspección visual de memoria (cat-scan)             Alt+M │
│ Exportar traza de memoria (reporte HTML)                  │
└───────────────────────────────────────────────────────────┘
```

## Git

```text
┌─────────────────────────────────────────────────────────┐
│ Abrir panel interactivo de Git                    Alt+G │
├─────────────────────────────────────────────────────────┤
│ Comparar cambios (diff lado a lado)         Alt+Shift+D │
│ Historial de cambios (log)                              │
├─────────────────────────────────────────────────────────┤
│ Confirmar cambios (commit)…                             │
│ Deshacer último commit (mantener en stage)              │
├─────────────────────────────────────────────────────────┤
│ Traer cambios (pull)                                    │
│ Subir cambios (push)                                    │
├─────────────────────────────────────────────────────────┤
│ Clonar repositorio…                                     │
└─────────────────────────────────────────────────────────┘
```

## Plugins

```text
┌───────────────────────────────────────────────────────┐
│ Plugins Python                                        │
│   ‹plugin›: ‹comando›                                 │
├───────────────────────────────────────────────────────┤
│ Herramientas para este archivo                  Alt+H │
│   ‹herramienta›: ‹acción›                             │
├───────────────────────────────────────────────────────┤
│ Configurar plugins…                                   │
│ Configurar integraciones (qué corre y cuándo)…        │
│ Nuevo plugin Python (.jonesy/plugins)…                │
│ Recargar los plugins Python                           │
└───────────────────────────────────────────────────────┘
```

## Examen

Sólo en modo examen (`--exam`).

```text
┌────────────────────────────────────────────────────────────┐
│ Entregar examen…                                       F12 │
│ Verificar estado de entrega                                │
├────────────────────────────────────────────────────────────┤
│ Pausar / reanudar el cronómetro (código del docente)…      │
└────────────────────────────────────────────────────────────┘
```

## Ayuda

```text
┌───────────────────────────────────────────────────────────────────────┐
│ Paleta de comandos                                             Ctrl+K │
│ Explicar error o regla actual                                      F1 │
├───────────────────────────────────────────────────────────────────────┤
│ Bienvenida                                                            │
│ Recorrido guiado por la pantalla                                      │
│ Guía rápida para estudiantes de C                                     │
│ Glosario de C…                                                        │
│ Tablas de consulta (tipos, printf, ASCII, precedencia)                │
│ Reglas de la cátedra (catálogo con ejemplos)…                         │
│ Atajos de teclado                                                     │
│ Atajos de este panel                                    ? en el panel │
│ Diagnóstico del entorno (jonesy doctor)                               │
│ Acerca de jonesy                                                      │
└───────────────────────────────────────────────────────────────────────┘
```

<!-- menus: fin -->
