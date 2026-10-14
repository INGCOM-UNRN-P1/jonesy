# CU-07 · Renombrar una variable o función

**Actor:** Alumno · **Grabación:** [`CU-07-renombrar.cast`](CU-07-renombrar.cast) (0:16) · **En la lista de casos:** [CU-07](../casos_de_uso.md#cu-07--renombrar-una-variable-o-función)

## De qué se trata

**Ctrl+R** (Editar → Renombrar en su ámbito) cambia el nombre de la variable o función
del cursor **sólo donde vale esa declaración**: una variable local que tapa a otra, un
parámetro o la variable de un `for` no arrastran a otra con el mismo nombre, y no se tocan
campos, cadenas ni comentarios. La ventana dice cuántas apariciones cambia y en qué
ámbito, rechaza nombres inválidos o ya usados, y todo se deshace con un solo Ctrl+Z.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | Con el cursor sobre la variable leidos, Ctrl+R |
| 0:04 | Un nombre reservado se rechaza al escribirlo |
| 0:07 | Con un nombre válido, Enter cambia todas las apariciones de su ámbito |
| 0:11 | Un solo Ctrl+Z lo deshace |

## Cómo verla

```bash
asciinema play docs/casos/CU-07-renombrar.cast        # de corrido
asciinema play -m docs/casos/CU-07-renombrar.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
