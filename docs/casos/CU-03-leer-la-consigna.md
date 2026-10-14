# CU-03 · Leer la consigna mientras se programa

**Actor:** Alumno · **Grabación:** [`CU-03-leer-la-consigna.cast`](CU-03-leer-la-consigna.cast) (0:15) · **En la lista de casos:** [CU-03](../casos_de_uso.md#cu-03--leer-la-consigna-mientras-se-programa)

## De qué se trata

El enunciado (`consigna.md`, `enunciado.md` o el README) se muestra de solo lectura al
lado del código, con el Markdown dibujado. **F7** pone el foco en la consigna: las
flechas y RePág/AvPág la desplazan y **Tab** recorre sus enlaces (Enter los sigue). Otra
vez **F7** la oculta para ganar espacio y vuelve al código. En pantallas chicas F7 la
muestra a pantalla completa y F8 vuelve al código.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | La consigna está al lado del código, con el Markdown dibujado |
| 0:02 | F7 pone el foco en la consigna; las flechas la desplazan |
| 0:07 | Tab recorre los enlaces de la consigna |
| 0:09 | F7 otra vez la oculta y vuelve al código; F7 la muestra de nuevo |

## Cómo verla

```bash
asciinema play docs/casos/CU-03-leer-la-consigna.cast        # de corrido
asciinema play -m docs/casos/CU-03-leer-la-consigna.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
