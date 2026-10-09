# Integraciones con el ecosistema

jonesy usa las herramientas de la cátedra (ripley, gaff, kaneda, spunkmeyer, corbel,
daedalus, tetsuo, motoko, callahan, hal, vasquez, holden) como **plugins declarativos**: cada una se describe
con un manifiesto TOML que dice cuándo correrla, cómo leer su salida y qué acciones suma
al menú **Herramientas**. No hace falta tocar jonesy para integrar una herramienta nueva.

## Dónde están

Se leen en este orden; una integración con el mismo `nombre` reemplaza a la anterior:

1. **De fábrica**, embebidas en el binario
   ([`crates/jonesy-core/data/integraciones.toml`](../crates/jonesy-core/data/integraciones.toml)).
2. **Del usuario**: `~/.config/jonesy/integraciones/*.toml`
   (`%APPDATA%\jonesy\integraciones` en Windows).
3. **Del proyecto**: `.jonesy/integraciones/*.toml`.

Cada una se activa sola si su `binario` está en el `PATH`. En `.jonesy.toml`:

```toml
[integraciones]
desactivar = ["spunkmeyer"]   # no correrlas en este proyecto
activar = ["mi-linter"]       # encender las que un manifiesto trae con activa = false
```

Dentro del IDE, *Archivo → Configuración del entorno* (solapa Integraciones) muestra el estado de cada una y permite
encenderlas o apagarlas (queda en la sesión). `jonesy doctor` lista cuáles están
instaladas y cómo instalar las que faltan.

## Formato del manifiesto

```toml
[[integracion]]
nombre = "kaneda"                          # identificador (y el de --json de las herramientas)
descripcion = "Seguridad: desbordes y funciones peligrosas"
binario = "kaneda"                         # la habilita si está en el PATH (vacío: siempre)
cuando = "guardar"                         # "guardar" o "manual" (sólo sus acciones)
archivos = ["*.c", "*.h"]                  # globs del nombre del archivo
comando = "kaneda audit {archivo} --json"  # lo que corre al guardar
formato = "hallazgos"                      # cómo leer la salida (ver abajo)
explicar = "kaneda explain {codigo}"       # F1 sobre uno de sus hallazgos
catalogo = "kaneda rules --json"           # catálogo de reglas (Ayuda y F1 unificado)
omitir_si = ""                             # campo booleano que descarta un hallazgo
activa = true                              # encendida por defecto
examen = true                              # permitida en modo examen
lento = false                              # tarda segundos (se indica en la barra)

  [[integracion.accion]]                   # acciones de la herramienta (Alt+H y la paleta)
  titulo = "Corregir el estilo automáticamente (gaff fix)"
  comando = "gaff fix {archivo}"
  modo = "recargar"
  archivos = ["*.c"]                       # opcional; por defecto, los de la integración
```

### Marcadores

| Marcador | Valor (entre comillas de shell) |
|---|---|
| `{archivo}` | archivo activo, relativo a la raíz del proyecto |
| `{dir}` | su directorio |
| `{modulo}` | directorio del módulo con Makefile (biblioteca o ejercicio) |
| `{codigo}` | código del hallazgo, en `explicar` |

Los comandos corren desde la raíz del proyecto con `sh -c` (o `cmd /C` en Windows sin sh),
sin colores (`NO_COLOR=1`). Antes de una acción se guardan todos los archivos.

### Formatos de salida

| `formato` | Qué espera |
|---|---|
| `hallazgos` | JSON con una lista `"hallazgos"` en la forma común de `yutani.hallazgos`: `codigo`, `categoria`, `severidad`, `mensaje`, `archivo`, `linea`, `columna`, `enlace`, `sugerencia`. Es lo que emiten con `--json` gaff, kaneda, spunkmeyer, daedalus, tetsuo y hal. |
| `ripley` | `ripley check --format json` (`ast_findings`). |
| `corbel` | `corbel check --json` (`missing`). |
| `gcc` | líneas `archivo:línea:col: error: mensaje` (también sigue los `make -C`). |
| `sarif` | SARIF 2.1.0, el formato estándar de analizadores (gcc con `-fdiagnostics-format=sarif-stderr`, clang, `cppcheck --output-format=sarif`, CodeQL…): nivel, mensaje, regla con su descripción y enlace (`helpUri`), y la primera ubicación. |

Puede haber texto antes del JSON: se lee desde la primera llave. Las rutas relativas se
resuelven contra la raíz del proyecto; un hallazgo sin archivo se asigna al analizado.

### Modos de las acciones

| `modo` | Qué hace |
|---|---|
| `panel` | salida en vivo en el panel inferior (errores de gcc en Problemas) |
| `terminal` | corre en la terminal interna (programas interactivos, `tetsuo run`) |
| `visor` | ventana con la salida, con sus colores |
| `recargar` | la herramienta modifica el archivo en disco; el buffer toma el contenido nuevo como una edición que Ctrl+Z deshace |
| `diagnosticos` | interpreta la salida con el `formato` de la integración y la muestra en Problemas |

## Catálogo de reglas

Con `catalogo`, jonesy lee una vez (en segundo plano, la primera vez que hace falta) las
reglas de la herramienta: una lista `reglas`, `patrones` o `rules` con `codigo`,
`titulo` o `nombre`, `descripcion` o `explicacion`, `sugerencia`, `ejemplo_incorrecto`,
`ejemplo_correcto`, `categoria` y, si hay, `alias`, `cwe` y `regla_catedra`. Con eso:

- *Ayuda → Reglas de la cátedra* las lista y muestra cada una con sus ejemplos;
- F1 sobre un hallazgo muestra la regla con el mismo formato que los errores de gcc
  (qué es, qué hacer, «así no», «así sí» y el apunte). Si la regla no está en el
  catálogo, se usa el comando `explicar`.

## Modo examen

Una integración con `examen = false` no corre en examen (callahan). Dentro de las que sí
corren, una **acción** se permite salvo que modifique el código (`modo = "recargar"`:
`gaff fix`, `corbel scaffold`), porque haría el trabajo por el alumno; el manifiesto puede
fijarlo con `examen = true` o `false` en la acción.

## Duplicados

Varias herramientas informan lo mismo (ripley incluye reglas de gaff y de kaneda). jonesy
muestra una sola vez cada regla en una misma línea, prefiriendo la versión que trae
categoría y enlace al apunte. `omitir_si` descarta además los hallazgos que la propia
herramienta marca como repetidos (spunkmeyer usa `tambien_en_gaff`).

## Ejemplo: cppcheck en un proyecto

`.jonesy/integraciones/cppcheck.toml`:

```toml
[[integracion]]
nombre = "cppcheck"
descripcion = "Análisis estático de cppcheck"
binario = "cppcheck"
cuando = "guardar"
archivos = ["*.c"]
comando = "cppcheck --enable=warning,style --template=gcc --quiet {archivo} 2>&1"
formato = "gcc"
```

## Ejemplo: cppcheck con SARIF

```toml
[[integracion]]
nombre = "cppcheck"
descripcion = "Análisis estático de cppcheck (SARIF)"
binario = "cppcheck"
cuando = "guardar"
archivos = ["*.c"]
comando = "cppcheck --enable=warning --output-format=sarif --quiet {archivo} 2>&1"
formato = "sarif"
```

## ¿Integración o plugin Python?

jonesy tiene dos formas de extenderse; conviven y sirven para cosas distintas:

| | Integración (este documento) | Plugin Python (`.jonesy/plugins/*.py`) |
|---|---|---|
| Qué es | un comando externo descrito en TOML | código Python que corre en un proceso aparte (worker JSON-RPC) |
| Para qué | usar una herramienta que ya existe (de la cátedra o no) | lógica propia chica: un chequeo puntual, un comando de la paleta |
| Recibe | la ruta del archivo (lo lee del disco) | el texto del buffer (aunque no esté guardado) |
| Devuelve | hallazgos en JSON (yutani), salida de gcc, o lo que muestre en un visor | diagnósticos y texto para la paleta |
| Si falla | la salida no se entiende: aviso en la barra | el error (sintaxis, excepción en `al_guardar`) aparece en Problemas sobre el script, en su línea; guardar el plugin lo recarga |
| En examen | todas salvo `examen = false` y las acciones que corrigen | sólo los plugins con SHA-256 en `[examen] plugins_permitidos` |

Si la herramienta existe como CLI, conviene una integración. Si es un chequeo de pocas
líneas propio de una materia o un TP, un plugin (ver `docs/plugin-ejemplo.py`).

## Para quien mantiene una herramienta del ecosistema

Para que jonesy la integre sin código nuevo alcanza con:

1. un comando que analice **un archivo** y emita `--json` con `"hallazgos"` en la forma
   de `yutani.hallazgos` (con `categoria` y `enlace` al apunte);
2. un comando `explain <código>` para F1;
3. si corrige archivos, que lo haga en el lugar (modo `recargar`);
4. `doctor --json` y `--version` (jonesy los usa en `jonesy doctor`).

Después se agrega su manifiesto a `data/integraciones.toml` (de fábrica) o se distribuye
en `~/.config/jonesy/integraciones/`.
