# CU-01 · Empezar a usar jonesy por primera vez

**Actor:** Alumno · **Grabación:** [`CU-01-primer-uso.cast`](CU-01-primer-uso.cast) (0:22) · **En la lista de casos:** [CU-01](../casos_de_uso.md#cu-01--empezar-a-usar-jonesy-por-primera-vez)

## De qué se trata

La primera vez que se abre jonesy aparece la pantalla de bienvenida: las teclas del ciclo
de trabajo de un ejercicio (consigna, guardar, compilar, ejecutar, probar, depurar) y, si
falta algo en el entorno, qué es y cómo instalarlo. Desde ahí, `R` empieza el
**recorrido guiado**, que resalta en la pantalla cada zona (menús, editor, consigna,
quick-doc, barra de estado, panel de abajo y barra izquierda), dice para qué sirve y, si
está cerrada, cómo abrirla. La bienvenida, el recorrido y la guía rápida quedan después en
el menú Ayuda.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | Al abrir jonesy por primera vez aparece la bienvenida |
| 0:02 | R empieza el recorrido guiado por la pantalla |
| 0:04 | Enter pasa a la zona siguiente: el editor |
| 0:07 | Enter pasa a la zona siguiente: la consigna |
| 0:09 | Enter pasa a la zona siguiente: la documentación |
| 0:12 | Enter pasa a la zona siguiente: la barra de estado |
| 0:14 | Enter pasa a la zona siguiente: el panel de abajo |
| 0:16 | Enter pasa a la zona siguiente: la barra izquierda |
| 0:19 | El último paso dice dónde volver a encontrar la ayuda |

## Cómo verla

```bash
asciinema play docs/casos/CU-01-primer-uso.cast        # de corrido
asciinema play -m docs/casos/CU-01-primer-uso.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
