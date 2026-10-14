# CU-19 · Ver la memoria dibujada (cat-scan)

**Actor:** Alumno · **Grabación:** [`CU-19-cat-scan.cast`](CU-19-cat-scan.cast) (0:16) · **En la lista de casos:** [CU-19](../casos_de_uso.md#cu-19--ver-la-memoria-dibujada-cat-scan)

## De qué se trata

Durante la depuración, **Alt+M** abre **cat-scan**: con la traza de `mem-trace`, dibuja el
stack y el heap con flechas desde cada puntero a lo que apunta, marca en rojo los punteros
colgantes y muestra los eventos del paso (pedidos, liberaciones, accesos inválidos). Se
puede exportar a una página HTML.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | Breakpoint después de copiar el vector y F5 |
| 0:04 | Alt+M: el stack y el heap dibujados, con flechas de los punteros |
| 0:06 | F10 avanza: tras el free, el puntero queda colgante (en rojo) |
| 0:13 | Al asignarle NULL deja de apuntar al bloque liberado |

## Cómo verla

```bash
asciinema play docs/casos/CU-19-cat-scan.cast        # de corrido
asciinema play -m docs/casos/CU-19-cat-scan.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.

## Notas

mem-trace instrumenta sólo el archivo con `main`: la memoria que se pide en otros `.c`
del ejercicio (por ejemplo dentro de `copiar()` en `copia.c`) no aparece en el dibujo y
el puntero se marca como inválido. Por eso la grabación usa una variante del ejercicio 3
que pide la memoria en `main.c`.
