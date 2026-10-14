# CU-32 · Verificar una entrega

**Actor:** Docente · **Grabación:** [`CU-32-verificar-una-entrega.cast`](CU-32-verificar-una-entrega.cast) (0:21) · **En la lista de casos:** [CU-32](../casos_de_uso.md#cu-32--verificar-una-entrega)

## De qué se trata

`jonesy verificar <paquete.tar>` comprueba una entrega: el SHA-256 del paquete, cada
archivo contra el manifiesto, la cadena de la bitácora, su sello y que la sesión esté
cerrada. Termina con código 0 si es íntegra y 1 si no; con `--json` da lo mismo para las
herramientas del perfil docente.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | El paquete que dejó la entrega (CU-29) |
| 0:04 | jonesy verificar comprueba el paquete, los archivos y la bitácora |
| 0:09 | Si alguien cambia el paquete, la verificación lo detecta |

## Cómo verla

```bash
asciinema play docs/casos/CU-32-verificar-una-entrega.cast        # de corrido
asciinema play -m docs/casos/CU-32-verificar-una-entrega.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
