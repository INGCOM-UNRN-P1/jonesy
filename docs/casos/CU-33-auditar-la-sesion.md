# CU-33 · Auditar la sesión de un examen

**Actor:** Docente · **Grabación:** [`CU-33-auditar-la-sesion.cast`](CU-33-auditar-la-sesion.cast) (0:20) · **En la lista de casos:** [CU-33](../casos_de_uso.md#cu-33--auditar-la-sesión-de-un-examen)

## De qué se trata

`jonesy replay <telemetry.log>` reproduce la sesión del examen tecla a tecla (Espacio
pausa, → avanza, + acelera, q sale). Con `--resumen` (o `--json`) da la auditoría:
integridad de la cadena, duración, cuántas veces y cuánto se pegó, y la velocidad de
tipeo por minuto.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | El resumen de auditoría de la bitácora del examen |
| 0:06 | La reproducción de la sesión, tecla a tecla |

## Cómo verla

```bash
asciinema play docs/casos/CU-33-auditar-la-sesion.cast        # de corrido
asciinema play -m docs/casos/CU-33-auditar-la-sesion.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
