# CU-31 · Configurar un proyecto o un examen

**Actor:** Docente · **Grabación:** [`CU-31-configurar-el-proyecto.cast`](CU-31-configurar-el-proyecto.cast) (0:24) · **En la lista de casos:** [CU-31](../casos_de_uso.md#cu-31--configurar-un-proyecto-o-un-examen)

## De qué se trata

Cada proyecto puede tener un `.jonesy.toml` (ver `docs/ejemplo.jonesy.toml`): el comando
de compilación, el programa y sus argumentos para depurar, las descripciones de los
objetivos de make, las integraciones activas, las pruebas en SARIF y, para el examen, la
consigna, la duración, la verificación, los archivos de la entrega, los plugins
autorizados y el SHA-256 del código de pausa (largo y al azar: el alumno puede leer el
archivo). **Archivo → Configuración del entorno** muestra y abre todo eso.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | El .jonesy.toml del TP: duración, verificación, archivos y código de pausa |
| 0:04 | El hash del código de pausa se calcula así (el código no va en el archivo) |
| 0:09 | En jonesy, Configuración del entorno muestra lo que está configurado |

## Cómo verla

```bash
asciinema play docs/casos/CU-31-configurar-el-proyecto.cast        # de corrido
asciinema play -m docs/casos/CU-31-configurar-el-proyecto.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
