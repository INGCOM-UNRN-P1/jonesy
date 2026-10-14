# CU-37 · Alojar muchas sesiones en un servidor

**Actor:** Administrador · **Grabación:** [`CU-37-muchas-sesiones.cast`](CU-37-muchas-sesiones.cast) (0:17) · **En la lista de casos:** [CU-37](../casos_de_uso.md#cu-37--alojar-muchas-sesiones-en-un-servidor)

## De qué se trata

jonesy es un único binario: arranca en milisegundos, consume poco en reposo (no redibuja
si nada cambió) y guarda el estado de cada sesión en el directorio del proyecto, así que
decenas de alumnos pueden usarlo a la vez en el mismo servidor. `bin/estres-sesiones`
levanta N sesiones reales y mide la memoria y la CPU en reposo contra el presupuesto.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | El arranque, medido: proyecto leído y primer cuadro dibujado |
| 0:05 | Diez sesiones a la vez en reposo: memoria y CPU contra el presupuesto |

## Cómo verla

```bash
asciinema play docs/casos/CU-37-muchas-sesiones.cast        # de corrido
asciinema play -m docs/casos/CU-37-muchas-sesiones.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
