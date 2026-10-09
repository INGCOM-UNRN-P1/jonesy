# Diagnóstico y Propuesta de Reorganización: Menús de jonesy

Evaluando esta interfaz desde la perspectiva de **estudiantes principiantes de C y entorno educativo/universitario**, el diseño actual tiene una base sólida pero sufre de sobrecarga cognitiva, colisiones conceptuales y dispersión de configuración.

**Nivel de madurez de la estructura:** En desarrollo (Developing).

---

## 1. Diagnóstico y Crítica

### A. Fricción en el modelo mental (Construir vs. Depurar)
* **Problema:** En el menú `Construir` conviven tres fases muy distintas del ciclo de desarrollo: **Build/Make** (`Compilar`, `make doc`), **Execution/Testing** (`make test`), **Debugging** (6 acciones de GDB/LLDB: paso, entrar, salir, breakpoint) y **Profiling/Memoria** (`valgrind`, `cat-scan`, `mem-trace`).
* **Impacto en el principiante:** En C, uno de los conceptos más difíciles de internalizar es la separación entre *compilación*, *ejecución* y *depuración*. Meter los controles de flujo de ejecución (F5, F10, F11) adentro de «Construir» confunde la acción de construir un binario con la de inspeccionar un proceso en marcha.
* **Acción requerida:** Separar `Construir` (Build) de `Depurar` (Debug), tal como hacen los entornos estándar (Code::Blocks, CLion, VS Code).

### B. Sobrecarga por el menú «Herramientas» y dispersión de configuración
* **Problema:** El menú `Herramientas` actual mezcla utilidades duplicadas (`Autoformato`, `make doc`, `Explicar regla`) con opciones avanzadas de extensibilidad del IDE (`Integraciones: qué herramientas corren...`, plugins en Python).
* **Impacto:** Los estudiantes principiantes no distinguen entre editar/compilar su programa y configurar el entorno de desarrollo. Tener un menú de primer nivel solo para plugins y configuración de linters desperdicia espacio horizontal valioso en una TUI y agrega ruido cognitivo.
* **Acción requerida:** **Eliminar `Herramientas` de la barra superior**. Mover la gestión de plugins, linters e integraciones a una pantalla unificada de **`Configuración del entorno...` (`Ctrl+,`) dentro de `Archivo`**, reubicando las acciones de código en sus menús canónicos (`Autoformato` en Editar, `make doc` en Construir).

### C. Saturación y ambigüedad en «Ver»
* **Problema:** El menú `Ver` tiene 15 entradas que mezclan tres conceptos distintos:
  1. *Herramientas de inspección de código:* `quick-doc` y su disposición.
  2. *Paneles laterales o inferiores:* `Terminal`, `Problemas`, `Git`, `Reglas`.
  3. *Modos de vista / Enfoque:* `Ver consigna (F7)`, `Ver código (F8)`, `Ver terminal (F9)`.
* **Impacto:** F7, F8 y F9 son cruciales para el flujo de trabajo en ejercicios universitarios, pero quedan enterrados al final de una lista larga de paneles utilitarios. Además, `Mostrar/ocultar quick-doc` y `Quick-doc: derecha ⟷ inferior` ocupan dos entradas valiosas en lugar de abstraerse.
* **Acción requerida:** Agrupar con separadores claros o mover las vistas de flujo pedagógico (Consigna / Código / Terminal) a un bloque prioritario superior.

### D. Confusión con los atajos en el menú Git
* **Problema:** En el menú `Git`, las entradas muestran teclas de una sola letra (`a`, `u`, `c`, `l`, `z`). La documentación aclara que funcionan sólo «con el foco en el panel de git».
* **Impacto:** Si un estudiante despliega el menú `Git` con el cursor en el editor de código y ve `Git: commit... c`, esperará presionar `c` y que ocurra un commit. Al ver que solo escribe una "c" en su código C, interpretará que el IDE no funciona. Los menús de barra superior tradicionalmente comunican atajos *globales*.
* **Acción requerida:** No listar atajos locales de panel como si fueran aceleradores globales del menú, o desambiguar visualmente que pertenecen al panel.

### E. Conflicto ergonómico con `Alt+letra` y mnemónicos
* **Problema:** Casi todos los menús (`Editar`, `Git`, `Examen`, `Ayuda`) quedan inhabilitados para `Alt+letra` porque chocan con paneles (`Alt+E` explorador, `Alt+G` git).
* **Impacto:** La promesa del sistema de menú («Alt+letra abre el menú») se rompe en más del 60% de los casos. Al retirar `Herramientas` de la barra, liberamos además la colisión de `Alt+H`. Para el resto, se deben usar **mnemónicos estándar subrayados** (ej. `Arc_h_ivo`, `_E_ditar`, `_V_er`, `_C_onstruir`, `_D_epurar`, `A_y_uda`).

---

## 2. Propuesta de Reorganización de la Barra de Menús

Al absorber `Herramientas` dentro de `Archivo → Configuración...`, ganamos el espacio horizontal exacto que requiere la creación del menú `Depurar`. La barra pasa a tener una estructura simétrica, limpia y orientada al ciclo de vida del estudiante:

### Barra propuesta:
```text
  Archivo   Editar   Ver   Construir   Depurar   Git   [Examen]   Ayuda
```

---

## 3. Detalle de Menús Reorganizados

### Archivo (`Alt+A`)
Centrado en archivos, proyectos, ciclo de vida de la sesión y acceso a la configuración global del entorno.

```text
┌──────────────────────────────────────────────┐
│ Nuevo archivo                         Ctrl+N │
│ Abrir archivo...                      Ctrl+O │
│ Abrir proyecto o directorio...               │
│ ──────────────────────────────────────────── │
│ Guardar                          Ctrl+S / F2 │
│ Guardar todo                    Ctrl+Shift+S │
│ ──────────────────────────────────────────── │
│ Cerrar pestaña                        Ctrl+W │
│ Pestaña siguiente                   Ctrl+Tab │
│ Pestaña anterior              Ctrl+Shift+Tab │
│ ──────────────────────────────────────────── │
│ Configuración del entorno...          Ctrl+, │
│ ──────────────────────────────────────────── │
│ Salir                                 Ctrl+Q │
└──────────────────────────────────────────────┘
```

#### Nueva Pantalla Modal: `Configuración del entorno` (`Ctrl+,`)
Al seleccionar `Configuración del entorno...` (o presionar `Ctrl+,`), se abre una pantalla dedicada dividida en solapas/secciones:
1. **Integraciones y Linters:** Qué herramientas corren y cuándo (`gaff fix`, `tetsuo`, `motoko`, `hal`). Reglas pedagógicas de `ripley`.
2. **Plugins (.jonesy/plugins):** Listado de plugins activos en Python, botón para crear nuevo plugin y recargar plugins en caliente.
3. **Compilador y Formato:** Configuración de banderas de GCC/Clang y estilo de `clang-format`.
4. **Apariencia:** Tema (oscuro, claro, alto contraste) y tamaños predeterminados de paneles.

---

### Editar (`Alt+E` o `_E_ditar`)
Operaciones exclusivas sobre el texto del buffer activo.

```text
┌──────────────────────────────────────────────┐
│ Deshacer                              Ctrl+Z │
│ Rehacer                               Ctrl+Y │
│ ──────────────────────────────────────────── │
│ Cortar                                Ctrl+X │
│ Copiar                                Ctrl+C │
│ Pegar                                 Ctrl+V │
│ Seleccionar todo                      Ctrl+A │
│ ──────────────────────────────────────────── │
│ Buscar...                        Ctrl+F / F3 │
│ Buscar siguiente                    Shift+F3 │
│ Ir a la línea...                      Ctrl+G │
│ ──────────────────────────────────────────── │
│ Autoformatear código (clang-format)   Ctrl+I │ <!-- o Alt+Shift+F -->
└──────────────────────────────────────────────┘
```

---

### Ver (`Alt+V`)
Jerarquizado en tres bloques: **Modos de trabajo del alumno**, **Paneles de desarrollo** y **Apariencia**.

```text
┌───────────────────────────────────────────────────────────┐
│ [Modo] Código                                          F8 │
│ [Modo] Consigna                                        F7 │
│ [Modo] Terminal integrada                              F9 │
│ ───────────────────────────────────────────────────────── │
│ Explorador de archivos                              Alt+E │
│ Problemas y diagnósticos                            Alt+P │
│ Salida de la compilación                            Alt+O │
│ Terminal (deck)                                     Alt+T │
│ Panel de Git                                        Alt+G │
│ Panel de reglas pedagógicas (ripley)                Alt+R │
│ ───────────────────────────────────────────────────────── │
│ Documentación contextual (quick-doc)           F4 / Alt+D │
│ Vista previa de Markdown                           Ctrl+P │
│ ───────────────────────────────────────────────────────── │
│ Restablecer disposición de paneles                        │
└───────────────────────────────────────────────────────────┘
```

---

### Construir (`Alt+C`)
Dedicado exclusivamente a la generación de binarios y ejecución directa/automatizada (Make/Build).

```text
┌───────────────────────────────────────────────────────────┐
│ Compilar archivo/proyecto                              F6 │
│ Compilar y ejecutar                               Ctrl+F5 │
│ Correr pruebas (make test)                        Ctrl+F6 │
│ ───────────────────────────────────────────────────────── │
│ Limpiar compilación (make clean)                          │
│ Generar documentación (make doc)                          │
│ ───────────────────────────────────────────────────────── │
│ Ver ejercicios y objetivos del proyecto             Alt+Y │
└───────────────────────────────────────────────────────────┘
```

---

### Depurar (`Alt+D` o `_D_epurar`) *(Nuevo Menú)*
El núcleo didáctico en C: seguimiento paso a paso y visibilidad de memoria (GDB + Valgrind).

```text
┌───────────────────────────────────────────────────────────┐
│ Iniciar depuración / Continuar                         F5 │
│ Detener depuración                               Shift+F5 │
│ Poner / Quitar punto de interrupción (breakpoint)  Ctrl+B │
│ ───────────────────────────────────────────────────────── │
│ Paso a paso (siguiente línea)                         F10 │
│ Entrar en la función (step into)                      F11 │
│ Salir de la función (step out)                  Shift+F11 │
│ ───────────────────────────────────────────────────────── │
│ Revisar memoria con Valgrind (fugas y accesos)            │
│ Inspección visual de memoria (cat-scan)             Alt+M │
│ Exportar traza de memoria (reporte HTML)                  │
└───────────────────────────────────────────────────────────┘
```

---

### Git (`Alt+G`)
Enfoque en acciones seguras y claridad en los atajos.

```text
┌───────────────────────────────────────────────────────────┐
│ Abrir panel interactivo de Git                      Alt+G │
│ ───────────────────────────────────────────────────────── │
│ Comparar cambios (Diff lado a lado)           Alt+Shift+D │
│ Historial de cambios (Log)                                │
│ ───────────────────────────────────────────────────────── │
│ Confirmar cambios (Commit)...                             │
│ Deshacer último commit (mantener en stage)                │
│ Clonar repositorio...                                     │
└───────────────────────────────────────────────────────────┘
```
*Nota:* Los atajos de una sola letra (`a`, `u`, `c`, `l`, `z`) se indican en el pie del propio panel de Git (`Alt+G`) cuando tiene foco, eliminando la ambigüedad en el editor.

---

### Examen (Sólo activo en `--exam`)
```text
┌──────────────────────────────────────┐
│ Entregar examen...               F12 │
│ Verificar estado de entrega          │
└──────────────────────────────────────┘
```

---

### Ayuda (`A_y_uda` o `F1`)
```text
┌───────────────────────────────────────────┐
│ Paleta de comandos                 Ctrl+K │
│ Explicar error o regla actual          F1 │
│ ───────────────────────────────────────── │
│ Atajos de teclado                         │
│ Guía rápida para estudiantes de C         │
│ Acerca de jonesy                          │
└───────────────────────────────────────────┘
```

---

## 4. Cuadro Comparativo de Impacto Pedagógico

| Aspecto | Estructura Original | Estructura Propuesta | Beneficio para el Principiante |
|---|---|---|---|
| **Menú Herramientas** | Menú separado con mezcla de plugins, linters y duplicados. | **Eliminado.** Configuración centralizada en `Archivo → Configuración...` (`Ctrl+,`). | Despeja la barra superior y evita que el alumno confunda configurar el entorno con programar. |
| **Ciclo de vida en C** | Mezcla compilar con depurar y trazas de memoria en `Construir`. | Menú `Construir` separado de `Depurar`. | Claridad conceptual: compilar produce un binario; depurar observa la memoria y ejecución. |
| **Atajos de Git** | Teclas sueltas (`a`, `c`) en menú global. | Teclas confinadas al panel; menú con acciones explícitas. | Evita que el alumno intente presionar `c` en el editor esperando un commit. |
| **Duplicación** | Formato, Make doc y Reglas repetidos en 2 o 3 menús. | Acciones con ubicación canónica única. | Menor ambigüedad; fomenta el uso de `Ctrl+K` para búsquedas directas. |
| **Navegación F7-F9** | Al fondo del menú `Ver` (15 ítems). | Primer bloque prioritario de `Ver`. | Acceso inmediato a la tríada fundamental: Consigna $\leftrightarrow$ Código $\leftrightarrow$ Terminal. |