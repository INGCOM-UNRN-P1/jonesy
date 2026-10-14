# CU-05 · Consultar una función de la biblioteca estándar

**Actor:** Alumno · **Grabación:** [`CU-05-quick-doc.cast`](CU-05-quick-doc.cast) (0:19) · **En la lista de casos:** [CU-05](../casos_de_uso.md#cu-05--consultar-una-función-de-la-biblioteca-estándar)

## De qué se trata

El **quick-doc** muestra, mientras se escribe, la documentación de la función bajo el
cursor: firma, qué devuelve, cada parámetro (resaltando el que se está escribiendo), las
trampas típicas y las reglas de cátedra relacionadas. Son unas 80 funciones de libc y
POSIX embebidas en jonesy, sin red. **F4** lo muestra u oculta y **Shift+F4** lo pasa de
la derecha a abajo.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | Con el cursor sobre scanf, el quick-doc muestra su documentación |
| 0:05 | Al escribir una llamada, resalta el parámetro que se está escribiendo |
| 0:09 | Shift+F4 lo pasa abajo, donde entra más texto |
| 0:13 | F4 lo oculta y lo vuelve a mostrar |

## Cómo verla

```bash
asciinema play docs/casos/CU-05-quick-doc.cast        # de corrido
asciinema play -m docs/casos/CU-05-quick-doc.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
