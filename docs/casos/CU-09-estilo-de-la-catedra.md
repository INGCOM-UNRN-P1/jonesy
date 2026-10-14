# CU-09 · Revisar el estilo con las reglas de la cátedra

**Actor:** Alumno · **Grabación:** [`CU-09-estilo-de-la-catedra.cast`](CU-09-estilo-de-la-catedra.cast) (0:28) · **En la lista de casos:** [CU-09](../casos_de_uso.md#cu-09--revisar-el-estilo-con-las-reglas-de-la-cátedra)

## De qué se trata

Al guardar, las herramientas de la cátedra (ripley, gaff, kaneda, spunkmeyer, si están
instaladas) revisan el archivo. Lo que encuentran aparece al final de la línea y en el
**panel de reglas** (Alt+R), agrupado por regla. **F1** sobre la línea explica la regla
con el mismo formato que los errores de gcc: qué es, qué hacer, «así no» y «así sí».
**Ayuda → Reglas de la cátedra** es el catálogo de todas las reglas con sus ejemplos.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | Se escribe una declaración doble, sin espacios, y se guarda |
| 0:04 | Al guardar, las herramientas revisan el estilo y marcan las líneas |
| 0:08 | Alt+R: el panel de reglas, agrupadas |
| 0:12 | F1 sobre la línea explica la regla con «así no / así sí» |
| 0:17 | Ayuda → Reglas de la cátedra: el catálogo navegable |

## Cómo verla

```bash
asciinema play docs/casos/CU-09-estilo-de-la-catedra.cast        # de corrido
asciinema play -m docs/casos/CU-09-estilo-de-la-catedra.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
