# CU-27 · Practicar con un simulacro de examen

**Actor:** Alumno · **Grabación:** [`CU-27-simulacro.cast`](CU-27-simulacro.cast) (0:20) · **En la lista de casos:** [CU-27](../casos_de_uso.md#cu-27--practicar-con-un-simulacro-de-examen)

## De qué se trata

**Archivo → Simulacro de examen…** (o `jonesy --simulacro [minutos]`) practica con el
cronómetro y los avisos del examen, pero sin restricciones ni entrega sellada. **F12** lo
termina y muestra cómo fue: compilaciones, cuántas con errores, ejecuciones y pruebas.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | jonesy --simulacro 30: cronómetro y avisos como en el examen |
| 0:06 | Se trabaja como siempre: compilar y probar |
| 0:11 | F12 termina el simulacro y muestra cómo fue |

## Cómo verla

```bash
asciinema play docs/casos/CU-27-simulacro.cast        # de corrido
asciinema play -m docs/casos/CU-27-simulacro.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
