# CU-16 · Entender por qué el programa se cortó

**Actor:** Alumno · **Grabación:** [`CU-16-el-programa-se-corto.cast`](CU-16-el-programa-se-corto.cast) (0:08) · **En la lista de casos:** [CU-16](../casos_de_uso.md#cu-16--entender-por-qué-el-programa-se-cortó)

## De qué se trata

Si el programa termina por una señal (violación de segmento, abort, división por cero…),
la terminal y la barra de estado explican en español qué pasó y aparece el botón
**[F5 Ver dónde falló]**: corre el depurador hasta la línea del programa donde se produjo,
saliendo de las bibliotecas del sistema, para ver el estado en ese momento.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | Ctrl+F5: el programa se corta por un puntero NULL |
| 0:03 | F5 (Ver dónde falló) lleva a la línea donde se cortó, con el depurador |
| 0:05 | Shift+F5 termina la depuración |

## Cómo verla

```bash
asciinema play docs/casos/CU-16-el-programa-se-corto.cast        # de corrido
asciinema play -m docs/casos/CU-16-el-programa-se-corto.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
