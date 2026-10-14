# CU-18 · Revisar la memoria con valgrind

**Actor:** Alumno · **Grabación:** [`CU-18-valgrind.cast`](CU-18-valgrind.cast) (0:20) · **En la lista de casos:** [CU-18](../casos_de_uso.md#cu-18--revisar-la-memoria-con-valgrind)

## De qué se trata

**Depurar → Revisar memoria con valgrind** corre `make memcheck` (o el programa) bajo
valgrind y deja cada problema en la línea del alumno, explicado: accesos inválidos, uso
después de `free`, `free` doble y fugas, con dónde se pidió o liberó el bloque.

En la grabación falta el `free` de la copia: valgrind informa la fuga y jonesy la ubica
en el `malloc` de `copiar`.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | Depurar → Revisar memoria con valgrind |
| 0:08 | La fuga queda explicada en la línea donde se pidió la memoria |
| 0:10 | Se agrega el free que faltaba y se revisa otra vez |

## Cómo verla

```bash
asciinema play docs/casos/CU-18-valgrind.cast        # de corrido
asciinema play -m docs/casos/CU-18-valgrind.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
