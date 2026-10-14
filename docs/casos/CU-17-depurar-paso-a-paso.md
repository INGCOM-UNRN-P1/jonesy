# CU-17 · Depurar paso a paso

**Actor:** Alumno · **Grabación:** [`CU-17-depurar-paso-a-paso.cast`](CU-17-depurar-paso-a-paso.cast) (0:21) · **En la lista de casos:** [CU-17](../casos_de_uso.md#cu-17--depurar-paso-a-paso)

## De qué se trata

**F5** depura sobre GDB: se detiene en los breakpoints (clic en el margen o **Ctrl+B**) y
muestra la pila y las variables. **F10** avanza una línea, **F11** entra en la función,
**Shift+F11** sale, **Ctrl+F10** corre hasta el cursor y **F5** continúa. La consola de
gdb queda a mano para lo avanzado.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | Ctrl+B pone un breakpoint en la llamada a copiar |
| 0:03 | F5 compila y corre hasta el breakpoint |
| 0:06 | F11 entra en la función copiar |
| 0:08 | F10 avanza de a una línea; las variables se actualizan |
| 0:14 | Shift+F11 sale de la función y vuelve a main |
| 0:18 | F5 continúa hasta el final |

## Cómo verla

```bash
asciinema play docs/casos/CU-17-depurar-paso-a-paso.cast        # de corrido
asciinema play -m docs/casos/CU-17-depurar-paso-a-paso.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
