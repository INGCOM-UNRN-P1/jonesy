# CU-28 · Rendir un examen

**Actor:** Alumno en examen · **Grabación:** [`CU-28-rendir-un-examen.cast`](CU-28-rendir-un-examen.cast) (0:23) · **En la lista de casos:** [CU-28](../casos_de_uso.md#cu-28--rendir-un-examen)

## De qué se trata

En modo examen (`jonesy --exam`) la consigna es de solo lectura, el cronómetro avisa los
minutos que quedan y todo se guarda solo cada 30 s. Una bitácora registra la sesión con
SHA-256 encadenado (lo pegado queda marcado). Se trabaja con las herramientas de siempre,
pero se niegan las acciones que no corresponden: abrir otro proyecto, clonar, renombrar
o borrar archivos, editar `.jonesy.toml` o aplicar correcciones automáticas.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | jonesy --exam: consigna, cronómetro y bitácora |
| 0:06 | Se compila y se prueba como siempre |
| 0:08 | Lo que no corresponde en un examen se niega con un aviso |
| 0:13 | Examen → Verificar estado de entrega: tiempo, autoguardado y bitácora |

## Cómo verla

```bash
asciinema play docs/casos/CU-28-rendir-un-examen.cast        # de corrido
asciinema play -m docs/casos/CU-28-rendir-un-examen.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
