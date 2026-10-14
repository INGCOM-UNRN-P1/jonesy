# CU-10 · Formatear el código

**Actor:** Alumno · **Grabación:** [`CU-10-formatear.cast`](CU-10-formatear.cast) (0:12) · **En la lista de casos:** [CU-10](../casos_de_uso.md#cu-10--formatear-el-código)

## De qué se trata

**Alt+Shift+F** (Editar → Autoformatear) formatea el archivo con gaff o, si no está, con
`clang-format` y el estilo de la cátedra. El cambio se puede deshacer con Ctrl+Z. Las
demás acciones de las herramientas del ecosistema para el archivo (gaff fix, tetsuo,
motoko…) están en **Alt+H** y en el menú Plugins.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | Una función escrita toda en una línea |
| 0:02 | Alt+Shift+F la formatea con el estilo de la cátedra |
| 0:07 | Alt+H: las acciones de las herramientas para este archivo |

## Cómo verla

```bash
asciinema play docs/casos/CU-10-formatear.cast        # de corrido
asciinema play -m docs/casos/CU-10-formatear.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
