# CU-26 · Recuperar el trabajo tras un corte

**Actor:** Alumno · **Grabación:** [`CU-26-recuperar-tras-un-corte.cast`](CU-26-recuperar-tras-un-corte.cast) (0:11) · **En la lista de casos:** [CU-26](../casos_de_uso.md#cu-26--recuperar-el-trabajo-tras-un-corte)

## De qué se trata

Si se cierra la terminal o se corta la conexión con cambios sin guardar, jonesy deja
copias de recuperación antes de salir. Al volver a abrir el proyecto ofrece restaurarlas
(**s**), descartarlas (**n**) o dejarlas para después (Esc).

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | Cambios sin guardar… |
| 0:05 | …y se corta la terminal (como al cerrar la ventana o perder la conexión) |
| 0:05 | Al volver a abrir el proyecto, jonesy ofrece recuperarlos |
| 0:07 | s los restaura en el editor |

## Cómo verla

```bash
asciinema play docs/casos/CU-26-recuperar-tras-un-corte.cast        # de corrido
asciinema play -m docs/casos/CU-26-recuperar-tras-un-corte.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
