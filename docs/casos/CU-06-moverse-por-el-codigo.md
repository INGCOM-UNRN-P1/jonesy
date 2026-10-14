# CU-06 · Moverse por el código y el proyecto

**Actor:** Alumno · **Grabación:** [`CU-06-moverse-por-el-codigo.cast`](CU-06-moverse-por-el-codigo.cast) (0:26) · **En la lista de casos:** [CU-06](../casos_de_uso.md#cu-06--moverse-por-el-código-y-el-proyecto)

## De qué se trata

**F12** (o Ctrl+clic) va a la definición de la función bajo el cursor, aunque esté en
otro archivo, y **Alt+←** vuelve. **Alt+S** muestra el esquema del archivo (funciones,
tipos, macros) para saltar a cualquiera. **Ctrl+F3** (o Ctrl+Shift+F) busca en todo el
proyecto y deja los resultados en un panel. **Ctrl+F2** pone marcadores y **Shift+F2**
salta entre ellos.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | F12 sobre promedio(…) va a su definición, en otro archivo |
| 0:05 | Alt+← vuelve a donde estaba |
| 0:08 | Alt+S: el esquema del archivo para saltar a una función o macro |
| 0:11 | Buscar en el proyecto (Ctrl+Shift+F): los resultados quedan en un panel |
| 0:20 | Ctrl+F2 pone un marcador; Shift+F2 vuelve a él |

## Cómo verla

```bash
asciinema play docs/casos/CU-06-moverse-por-el-codigo.cast        # de corrido
asciinema play -m docs/casos/CU-06-moverse-por-el-codigo.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
