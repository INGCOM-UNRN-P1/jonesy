# CU-15 · Correr un objetivo de make

**Actor:** Alumno · **Grabación:** [`CU-15-objetivo-de-make.cast`](CU-15-objetivo-de-make.cast) (0:13) · **En la lista de casos:** [CU-15](../casos_de_uso.md#cu-15--correr-un-objetivo-de-make)

## De qué se trata

**Shift+F9** (Construir → Correr un objetivo de make…) lista los objetivos de todos los
Makefile del proyecto con su descripción: primero los del ejercicio del archivo abierto,
después los del Makefile general, las bibliotecas y los demás ejercicios. Se filtra
escribiendo y Enter lo corre; la salida va al panel de abajo. También están en los
comandos (F1 o Ctrl+K, con el prefijo «make:») y en la vista del proyecto (Alt+Y).

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | Shift+F9: los objetivos de make, los del ejercicio actual primero |
| 0:02 | Se filtra escribiendo: el test del Makefile general |
| 0:07 | También desde los comandos (Ctrl+K), con el prefijo make: |

## Cómo verla

```bash
asciinema play docs/casos/CU-15-objetivo-de-make.cast        # de corrido
asciinema play -m docs/casos/CU-15-objetivo-de-make.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
