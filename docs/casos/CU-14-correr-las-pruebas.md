# CU-14 · Correr las pruebas

**Actor:** Alumno · **Grabación:** [`CU-14-correr-las-pruebas.cast`](CU-14-correr-las-pruebas.cast) (0:18) · **En la lista de casos:** [CU-14](../casos_de_uso.md#cu-14--correr-las-pruebas)

## De qué se trata

**Ctrl+F6** corre las pruebas del ejercicio (`make test`). Cada prueba que falla queda en
el panel Problemas y en la línea de la aserción, con lo esperado y lo obtenido; una que se
corta (violación de segmento) queda en la definición de la prueba. **Shift+F6** corre sólo
la prueba donde está el cursor.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | Ctrl+F6 corre las pruebas del ejercicio |
| 0:02 | La que falla queda en su línea, con lo esperado y lo obtenido |
| 0:05 | F12 sobre promedio lleva al código; se corrige la división |
| 0:11 | Ctrl+F6 otra vez: pasan todas |
| 0:14 | Shift+F6 corre sólo la prueba donde está el cursor |

## Cómo verla

```bash
asciinema play docs/casos/CU-14-correr-las-pruebas.cast        # de corrido
asciinema play -m docs/casos/CU-14-correr-las-pruebas.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
