# CU-13 · Comparar contra las salidas esperadas

**Actor:** Alumno · **Grabación:** [`CU-13-comparar-salidas.cast`](CU-13-comparar-salidas.cast) (0:23) · **En la lista de casos:** [CU-13](../casos_de_uso.md#cu-13--comparar-contra-las-salidas-esperadas)

## De qué se trata

Si el ejercicio tiene casos (`entrada1.txt` con su `salida1.txt`, …), **Construir →
Comparar con las salidas esperadas** corre el programa con cada entrada y compara lo que
imprime con lo esperado: dice qué casos pasan y, en los que no, dónde difiere.

En la grabación el promedio divide enteros: el primer caso pasa por casualidad (5.00) y el
segundo no (1.00 en lugar de 1.50). Se corrige y se vuelve a comparar.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | Comparar con las salidas esperadas: un caso pasa y el otro no |
| 0:06 | El error está en promedio.c: F12 lleva a la definición |
| 0:10 | Se convierte a double antes de dividir y se guarda |
| 0:15 | Comparar otra vez: pasan los dos casos |

## Cómo verla

```bash
asciinema play docs/casos/CU-13-comparar-salidas.cast        # de corrido
asciinema play -m docs/casos/CU-13-comparar-salidas.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
