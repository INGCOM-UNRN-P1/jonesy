# CU-24 · Trabajar sin mouse

**Actor:** Alumno · **Grabación:** [`CU-24-sin-mouse.cast`](CU-24-sin-mouse.cast) (0:21) · **En la lista de casos:** [CU-24](../casos_de_uso.md#cu-24--trabajar-sin-mouse)

## De qué se trata

Todo tiene una tecla. **F10** y la letra (o **Alt** y la letra subrayada) abren los menús
y las flechas los recorren; **Ctrl+K** (o F1 donde no hay un error que explicar) busca
cualquier acción, objetivo de make o herramienta por su nombre. En cada panel, **?**
muestra sus atajos, y **Alt+Z** maximiza el panel con el foco.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | F10 abre los menús; las flechas los recorren |
| 0:09 | Ctrl+K busca cualquier acción por su nombre |
| 0:12 | Alt+E pone el foco en los archivos; ? muestra los atajos de ese panel |
| 0:16 | Alt+Z maximiza el panel con el foco |

## Cómo verla

```bash
asciinema play docs/casos/CU-24-sin-mouse.cast        # de corrido
asciinema play -m docs/casos/CU-24-sin-mouse.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
