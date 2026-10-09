# Menús de jonesy

La barra superior agrupa todas las acciones del IDE. Todo lo que está en un menú también
se encuentra en la **paleta de comandos** (Ctrl+K), que busca por nombre.

## Cómo se usan

| Qué | Cómo |
|---|---|
| Abrir un menú | clic en su nombre, **F10** (abre el primero) o **Alt+letra** (ver la tabla) |
| Moverse | ← → cambia de menú, ↑ ↓ elige la entrada |
| Ejecutar | Enter o clic en la entrada |
| Cerrar sin hacer nada | Esc, F10 o clic fuera del menú |

- **F10 durante la depuración** es «Paso» (siguiente línea): para abrir el menú en ese
  momento, usá el clic o Alt+letra.
- **Alt+letra** abre el primer menú cuyo nombre empieza con esa letra, salvo que la letra
  sea un atajo global con Alt (Alt+E explorador, Alt+G panel de git, Alt+H herramientas,
  Alt+T terminal, Alt+P problemas, Alt+O salida, Alt+R reglas, Alt+M cat-scan, Alt+D
  quick-doc, Alt+Y proyecto). Por eso Editar, Git, Examen, Herramientas y Ayuda se abren
  con F10 y flechas o con el clic (Ayuda comparte la A con Archivo).
- El **atajo** de cada entrada aparece a la derecha. Las letras sueltas (`a`, `u`, `c`,
  `l`, `z`) se usan con el foco en el panel de git.

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

## Contenido que depende del proyecto

- **Herramientas → Herramientas del ecosistema para este archivo** abre una lista con
  las acciones de las integraciones instaladas que aplican al archivo activo (gaff fix,
  tetsuo, motoko, hal…). Ver [integraciones.md](integraciones.md).
- **Construir → Proyecto** (y Ver → Proyecto) muestra las bibliotecas, ejercicios y
  objetivos de make y `tp.sh`, listos para correr.
- Los **plugins Python** suman sus comandos a la paleta (Ctrl+K), no a los menús.

## Modo examen

Con `--exam` aparece el menú **Examen** (Entregar, F12). Algunas entradas se niegan con un
aviso: abrir otro proyecto, clonar, renombrar o borrar archivos, crear plugins, y las
acciones de herramientas que corrigen el código (por ejemplo `gaff fix`).

## Los menús

Lo que sigue se genera desde el código (`crates/jonesy-tui/src/accion.rs`); una prueba
comprueba que esté al día.

<!-- menus: inicio (generado por accion::documento_menus) -->

```text
  Archivo  Editar  Ver  Construir  Git  Examen  Herramientas  Ayuda 
```

| Menú | Abre con | Entradas |
|---|---|---|
| Archivo | Alt+A | 9 |
| Editar | F10 y flechas, o clic | 10 |
| Ver | Alt+V | 15 |
| Construir | Alt+C | 14 |
| Git | F10 y flechas, o clic | 8 |
| Examen | F10 y flechas, o clic | 1 |
| Herramientas | F10 y flechas, o clic | 7 |
| Ayuda | F10 y flechas, o clic | 4 |

## Archivo

```text
┌──────────────────────────────────────────────┐
│ Nuevo archivo…                        Ctrl+N │
│ Abrir archivo…                        Ctrl+O │
│ Abrir proyecto o directorio…                 │
│ Guardar                          Ctrl+S / F2 │
│ Guardar todo                    Ctrl+Shift+S │
│ Cerrar pestaña                        Ctrl+W │
│ Pestaña siguiente                   Ctrl+Tab │
│ Pestaña anterior              Ctrl+Shift+Tab │
│ Salir                                 Ctrl+Q │
└──────────────────────────────────────────────┘
```

## Editar

```text
┌──────────────────────────────────────┐
│ Deshacer                      Ctrl+Z │
│ Rehacer                       Ctrl+Y │
│ Cortar                        Ctrl+X │
│ Copiar                        Ctrl+C │
│ Pegar                         Ctrl+V │
│ Seleccionar todo              Ctrl+A │
│ Buscar…                  F3 / Ctrl+F │
│ Buscar siguiente            Shift+F3 │
│ Ir a la línea…                Ctrl+G │
│ Autoformato del archivo  Alt+Shift+F │
└──────────────────────────────────────┘
```

## Ver

```text
┌───────────────────────────────────────────────────────────┐
│ Explorador de archivos                              Alt+E │
│ Proyecto: bibliotecas, ejercicios y objetivos       Alt+Y │
│ Mostrar/ocultar quick-doc                      F4 / Alt+D │
│ Quick-doc: derecha ⟷ inferior                    Shift+F4 │
│ Terminal (deck)                                     Alt+T │
│ Problemas                                           Alt+P │
│ Salida de la compilación                            Alt+O │
│ Panel de Git                                        Alt+G │
│ Panel de reglas de ripley                           Alt+R │
│ Vista previa de Markdown                           Ctrl+P │
│ Ver consigna                                           F7 │
│ Ver código                                             F8 │
│ Ver terminal                                           F9 │
│ Tamaños de paneles…                                       │
│ Cambiar tema (oscuro, claro, contraste)                   │
└───────────────────────────────────────────────────────────┘
```

## Construir

```text
┌─────────────────────────────────────────────────────────────────────┐
│ Proyecto: bibliotecas, ejercicios y objetivos                 Alt+Y │
│ Generar la documentación del proyecto (make doc)                    │
│ Compilar                                                         F6 │
│ Compilar y ejecutar                                         Ctrl+F5 │
│ Correr las pruebas (make test)                              Ctrl+F6 │
│ Revisar la memoria: fugas y accesos inválidos (valgrind)            │
│ Depurar / continuar                                              F5 │
│ Paso (siguiente línea)                                          F10 │
│ Entrar en la función                                            F11 │
│ Salir de la función                                       Shift+F11 │
│ Detener la depuración                                      Shift+F5 │
│ Poner/quitar breakpoint                                      Ctrl+B │
│ Memoria (cat-scan)                                            Alt+M │
│ Exportar la traza de memoria (reporte HTML de mem-trace)            │
└─────────────────────────────────────────────────────────────────────┘
```

## Git

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ Comparar con la versión de git (diff lado a lado)                Alt+Shift+D │
│ Git: clonar un repositorio…                                                  │
│ Panel de Git                                                           Alt+G │
│ Git: agregar todo al stage                                                 a │
│ Git: quitar todo del stage                                                 u │
│ Git: commit…                                                               c │
│ Git: historial                                                             l │
│ Git: deshacer el último commit (sus cambios quedan en el stage)            z │
└──────────────────────────────────────────────────────────────────────────────┘
```

## Examen

Sólo en modo examen (`--exam`).

```text
┌──────────────────────┐
│ Entregar examen  F12 │
└──────────────────────┘
```

## Herramientas

```text
┌───────────────────────────────────────────────────────────────┐
│ Herramientas del ecosistema para este archivo…          Alt+H │
│ Autoformato del archivo                           Alt+Shift+F │
│ Generar la documentación del proyecto (make doc)              │
│ Explicar la regla del diagnóstico                          F1 │
│ Integraciones: qué herramientas corren y cuándo…              │
│ Nuevo plugin Python (.jonesy/plugins)…                        │
│ Recargar los plugins Python                                   │
└───────────────────────────────────────────────────────────────┘
```

## Ayuda

```text
┌───────────────────────────────────────────┐
│ Paleta de comandos                 Ctrl+K │
│ Explicar la regla del diagnóstico      F1 │
│ Atajos de teclado                         │
│ Acerca de jonesy                          │
└───────────────────────────────────────────┘
```

<!-- menus: fin -->
