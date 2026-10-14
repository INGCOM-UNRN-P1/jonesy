# CU-12 · Ejecutar con un archivo de entrada o con argumentos

**Actor:** Alumno · **Grabación:** [`CU-12-entrada-y-argumentos.cast`](CU-12-entrada-y-argumentos.cast) (0:20) · **En la lista de casos:** [CU-12](../casos_de_uso.md#cu-12--ejecutar-con-un-archivo-de-entrada-o-con-argumentos)

## De qué se trata

**Construir → Ejecutar con entrada…** elige un archivo del ejercicio como entrada estándar
del programa (o el teclado), y **Argumentos del programa…** los argumentos de la línea de
comandos. Las dos cosas se recuerdan por ejercicio: Ctrl+F5 las usa hasta que se cambien,
así no hay que tipear la misma entrada en cada prueba.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | Construir → Ejecutar con entrada: se elige el archivo |
| 0:09 | Ctrl+F5 corre el programa leyendo de ese archivo |
| 0:12 | Argumentos del programa: se escriben una vez y quedan para el ejercicio |

## Cómo verla

```bash
asciinema play docs/casos/CU-12-entrada-y-argumentos.cast        # de corrido
asciinema play -m docs/casos/CU-12-entrada-y-argumentos.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
