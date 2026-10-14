# CU-11 · Ejecutar el programa

**Actor:** Alumno · **Grabación:** [`CU-11-ejecutar.cast`](CU-11-ejecutar.cast) (0:08) · **En la lista de casos:** [CU-11](../casos_de_uso.md#cu-11--ejecutar-el-programa)

## De qué se trata

**Ctrl+F5** compila y corre el programa en la terminal de abajo, una terminal real donde
el programa lee del teclado. Al terminar, jonesy informa cómo terminó, cuánto tardó y
cuánta memoria usó; si el programa parece estar en un ciclo infinito avisa y Shift+F5 lo
corta.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | Ctrl+F5 compila y corre el programa en la terminal de abajo |
| 0:02 | El programa lee lo que se tipea |
| 0:04 | Al terminar: cómo terminó, el tiempo y la memoria |

## Cómo verla

```bash
asciinema play docs/casos/CU-11-ejecutar.cast        # de corrido
asciinema play -m docs/casos/CU-11-ejecutar.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
