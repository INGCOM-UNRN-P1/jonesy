# Casos de uso grabados

Una grabación de terminal ([asciinema](https://asciinema.org)) por cada caso de
[la lista de casos de uso](../casos_de_uso.md), con un documento que explica el caso
y los pasos de la grabación con sus tiempos.

```bash
asciinema play -m docs/casos/CU-08-compilar-y-entender-los-errores.cast
```

Con `-m` la reproducción se detiene en cada paso (Espacio sigue, `]` salta al
siguiente). Las grabaciones usan el proyecto de demostración de
[`grabador/demo`](grabador/demo): un TP con Makefile general, una biblioteca
(`libs/vector`) y tres ejercicios.

| Caso | Actor | Duración | Grabación |
|---|---|---|---|
| [CU-01 · Empezar a usar jonesy por primera vez](CU-01-primer-uso.md) | Alumno | 0:22 | [`CU-01-primer-uso.cast`](CU-01-primer-uso.cast) |
| [CU-02 · Abrir un TP o un ejercicio](CU-02-abrir-un-tp.md) | Alumno | 0:26 | [`CU-02-abrir-un-tp.cast`](CU-02-abrir-un-tp.cast) |
| [CU-03 · Leer la consigna mientras se programa](CU-03-leer-la-consigna.md) | Alumno | 0:15 | [`CU-03-leer-la-consigna.cast`](CU-03-leer-la-consigna.cast) |
| [CU-04 · Escribir y editar código](CU-04-escribir-y-editar.md) | Alumno | 0:26 | [`CU-04-escribir-y-editar.cast`](CU-04-escribir-y-editar.cast) |
| [CU-05 · Consultar una función de la biblioteca estándar](CU-05-quick-doc.md) | Alumno | 0:19 | [`CU-05-quick-doc.cast`](CU-05-quick-doc.cast) |
| [CU-06 · Moverse por el código y el proyecto](CU-06-moverse-por-el-codigo.md) | Alumno | 0:26 | [`CU-06-moverse-por-el-codigo.cast`](CU-06-moverse-por-el-codigo.cast) |
| [CU-07 · Renombrar una variable o función](CU-07-renombrar.md) | Alumno | 0:16 | [`CU-07-renombrar.cast`](CU-07-renombrar.cast) |
| [CU-08 · Compilar y entender los errores](CU-08-compilar-y-entender-los-errores.md) | Alumno | 0:18 | [`CU-08-compilar-y-entender-los-errores.cast`](CU-08-compilar-y-entender-los-errores.cast) |
| [CU-09 · Revisar el estilo con las reglas de la cátedra](CU-09-estilo-de-la-catedra.md) | Alumno | 0:28 | [`CU-09-estilo-de-la-catedra.cast`](CU-09-estilo-de-la-catedra.cast) |
| [CU-10 · Formatear el código](CU-10-formatear.md) | Alumno | 0:12 | [`CU-10-formatear.cast`](CU-10-formatear.cast) |
| [CU-11 · Ejecutar el programa](CU-11-ejecutar.md) | Alumno | 0:08 | [`CU-11-ejecutar.cast`](CU-11-ejecutar.cast) |
| [CU-12 · Ejecutar con un archivo de entrada o con argumentos](CU-12-entrada-y-argumentos.md) | Alumno | 0:20 | [`CU-12-entrada-y-argumentos.cast`](CU-12-entrada-y-argumentos.cast) |
| [CU-13 · Comparar contra las salidas esperadas](CU-13-comparar-salidas.md) | Alumno | 0:23 | [`CU-13-comparar-salidas.cast`](CU-13-comparar-salidas.cast) |
| [CU-14 · Correr las pruebas](CU-14-correr-las-pruebas.md) | Alumno | 0:18 | [`CU-14-correr-las-pruebas.cast`](CU-14-correr-las-pruebas.cast) |
| [CU-15 · Correr un objetivo de make](CU-15-objetivo-de-make.md) | Alumno | 0:13 | [`CU-15-objetivo-de-make.cast`](CU-15-objetivo-de-make.cast) |
| [CU-16 · Entender por qué el programa se cortó](CU-16-el-programa-se-corto.md) | Alumno | 0:08 | [`CU-16-el-programa-se-corto.cast`](CU-16-el-programa-se-corto.cast) |
| [CU-17 · Depurar paso a paso](CU-17-depurar-paso-a-paso.md) | Alumno | 0:21 | [`CU-17-depurar-paso-a-paso.cast`](CU-17-depurar-paso-a-paso.cast) |
| [CU-18 · Revisar la memoria con valgrind](CU-18-valgrind.md) | Alumno | 0:20 | [`CU-18-valgrind.cast`](CU-18-valgrind.cast) |
| [CU-19 · Ver la memoria dibujada (cat-scan)](CU-19-cat-scan.md) | Alumno | 0:16 | [`CU-19-cat-scan.cast`](CU-19-cat-scan.cast) |
| [CU-20 · Versionar con git](CU-20-git.md) | Alumno | 0:25 | [`CU-20-git.cast`](CU-20-git.cast) |
| [CU-21 · Aprender C desde el IDE](CU-21-aprender-c.md) | Alumno | 0:25 | [`CU-21-aprender-c.cast`](CU-21-aprender-c.cast) |
| [CU-22 · Escribir el informe en Markdown](CU-22-informe-markdown.md) | Alumno | 0:13 | [`CU-22-informe-markdown.cast`](CU-22-informe-markdown.cast) |
| [CU-23 · Usar la terminal integrada](CU-23-terminal-integrada.md) | Alumno | 0:14 | [`CU-23-terminal-integrada.cast`](CU-23-terminal-integrada.cast) |
| [CU-24 · Trabajar sin mouse](CU-24-sin-mouse.md) | Alumno | 0:21 | [`CU-24-sin-mouse.cast`](CU-24-sin-mouse.cast) |
| [CU-25 · Ajustar la apariencia y llevarla a otra máquina](CU-25-apariencia-y-preferencias.md) | Alumno | 0:33 | [`CU-25-apariencia-y-preferencias.cast`](CU-25-apariencia-y-preferencias.cast) |
| [CU-26 · Recuperar el trabajo tras un corte](CU-26-recuperar-tras-un-corte.md) | Alumno | 0:11 | [`CU-26-recuperar-tras-un-corte.cast`](CU-26-recuperar-tras-un-corte.cast) |
| [CU-27 · Practicar con un simulacro de examen](CU-27-simulacro.md) | Alumno | 0:20 | [`CU-27-simulacro.cast`](CU-27-simulacro.cast) |
| [CU-28 · Rendir un examen](CU-28-rendir-un-examen.md) | Alumno en examen | 0:23 | [`CU-28-rendir-un-examen.cast`](CU-28-rendir-un-examen.cast) |
| [CU-29 · Revisar y confirmar la entrega](CU-29-entregar.md) | Alumno en examen | 0:11 | [`CU-29-entregar.cast`](CU-29-entregar.cast) |
| [CU-30 · Pausar el cronómetro del examen](CU-30-pausa-del-examen.md) | Docente (en la máquina del alumno) | 0:14 | [`CU-30-pausa-del-examen.cast`](CU-30-pausa-del-examen.cast) |
| [CU-31 · Configurar un proyecto o un examen](CU-31-configurar-el-proyecto.md) | Docente | 0:24 | [`CU-31-configurar-el-proyecto.cast`](CU-31-configurar-el-proyecto.cast) |
| [CU-32 · Verificar una entrega](CU-32-verificar-una-entrega.md) | Docente | 0:21 | [`CU-32-verificar-una-entrega.cast`](CU-32-verificar-una-entrega.cast) |
| [CU-33 · Auditar la sesión de un examen](CU-33-auditar-la-sesion.md) | Docente | 0:20 | [`CU-33-auditar-la-sesion.cast`](CU-33-auditar-la-sesion.cast) |
| [CU-34 · Extender jonesy con plugins y herramientas](CU-34-plugins.md) | Docente | 0:16 | [`CU-34-plugins.cast`](CU-34-plugins.cast) |
| [CU-35 · Revisar la salud del entorno](CU-35-doctor.md) | Alumno, Administrador | 0:12 | [`CU-35-doctor.cast`](CU-35-doctor.cast) |
| [CU-36 · Reportar un problema de jonesy](CU-36-reportar-un-problema.md) | Alumno, Administrador | 0:15 | [`CU-36-reportar-un-problema.cast`](CU-36-reportar-un-problema.cast) |
| [CU-37 · Alojar muchas sesiones en un servidor](CU-37-muchas-sesiones.md) | Administrador | 0:17 | [`CU-37-muchas-sesiones.cast`](CU-37-muchas-sesiones.cast) |
| [CU-38 · Mirar por dentro un ejecutable, un .o o un .a](CU-38-visor-hexadecimal.md) | Alumno | 0:31 | [`CU-38-visor-hexadecimal.cast`](CU-38-visor-hexadecimal.cast) |

## Volver a grabarlas

```bash
bin/grabar-casos                 # todas
bin/grabar-casos CU-08 CU-14     # algunas
```

Cada guion está en [`grabador/guiones.py`](grabador/guiones.py): las teclas de cada
paso y lo que tiene que aparecer en la pantalla. Si algo no aparece, la grabación
falla y muestra la pantalla de ese momento, así que sirven también como prueba de
punta a punta de la interfaz. Necesita asciinema 3, `uv` (para `pyte`), gcc, make,
gdb y valgrind.
