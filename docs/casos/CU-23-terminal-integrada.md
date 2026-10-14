# CU-23 · Usar la terminal integrada

**Actor:** Alumno · **Grabación:** [`CU-23-terminal-integrada.cast`](CU-23-terminal-integrada.cast) (0:14) · **En la lista de casos:** [CU-23](../casos_de_uso.md#cu-23--usar-la-terminal-integrada)

## De qué se trata

**Alt+T** abre una terminal real dentro de jonesy, en la carpeta del proyecto (con las
variables y el `PATH` del entorno de la cátedra, si está instalado), para lo que no tiene
un botón. **Esc Esc** vuelve al editor y **Alt+Z** la maximiza.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | Alt+T abre la terminal en la carpeta del proyecto |
| 0:02 | Cualquier comando: compilar la biblioteca y listar lo que generó |
| 0:07 | Alt+Z la maximiza y la vuelve a su tamaño |
| 0:11 | Esc Esc vuelve al editor |

## Cómo verla

```bash
asciinema play docs/casos/CU-23-terminal-integrada.cast        # de corrido
asciinema play -m docs/casos/CU-23-terminal-integrada.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
