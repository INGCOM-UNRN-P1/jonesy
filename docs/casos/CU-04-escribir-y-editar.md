# CU-04 · Escribir y editar código

**Actor:** Alumno · **Grabación:** [`CU-04-escribir-y-editar.cast`](CU-04-escribir-y-editar.cast) (0:26) · **En la lista de casos:** [CU-04](../casos_de_uso.md#cu-04--escribir-y-editar-código)

## De qué se trata

El editor usa los atajos de siempre (Ctrl+S, Ctrl+Z, Ctrl+C/V) y suma lo que ayuda con C:
los paréntesis y comillas se cierran solos, **Ctrl+D** duplica la línea, **Alt+↑/↓** la
mueve, **Ctrl+/** comenta o descomenta, **Ctrl+F** busca y **Ctrl+H** reemplaza. Al
guardar, las herramientas de la cátedra revisan el archivo (ver CU-09).

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | Ctrl+G va a una línea; al escribir, paréntesis y comillas se cierran solos |
| 0:05 | Ctrl+D duplica la línea y Alt+↑ la sube |
| 0:09 | Ctrl+/ comenta la línea; Ctrl+Z deshace los cambios |
| 0:14 | Ctrl+F busca; Enter va a la siguiente coincidencia |
| 0:18 | Ctrl+S guarda |

## Cómo verla

```bash
asciinema play docs/casos/CU-04-escribir-y-editar.cast        # de corrido
asciinema play -m docs/casos/CU-04-escribir-y-editar.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
