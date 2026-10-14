# CU-30 · Pausar el cronómetro del examen

**Actor:** Docente (en la máquina del alumno) · **Grabación:** [`CU-30-pausa-del-examen.cast`](CU-30-pausa-del-examen.cast) (0:14) · **En la lista de casos:** [CU-30](../casos_de_uso.md#cu-30--pausar-el-cronómetro-del-examen)

## De qué se trata

Ante un imprevisto (un corte de luz, una consulta larga), el docente elige **Examen →
Pausar** y escribe su código en la máquina del alumno; se ve con asteriscos y se compara
con el SHA-256 de `[examen] pausa_sha256`. En pausa el cronómetro no corre, la barra dice
EN PAUSA y no se puede editar; la pausa sobrevive a un reinicio. Reanudar no pide código.
La bitácora registra cada pausa, cada reanudación y cada código rechazado.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | Examen → Pausar: el docente escribe su código (con asteriscos) |
| 0:06 | En pausa: el cronómetro se detiene y no se puede editar |
| 0:10 | Examen → Reanudar: el cronómetro sigue (no pide código) |

## Cómo verla

```bash
asciinema play docs/casos/CU-30-pausa-del-examen.cast        # de corrido
asciinema play -m docs/casos/CU-30-pausa-del-examen.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
