# CU-08 · Compilar y entender los errores

**Actor:** Alumno · **Grabación:** [`CU-08-compilar-y-entender-los-errores.cast`](CU-08-compilar-y-entender-los-errores.cast) (0:18) · **En la lista de casos:** [CU-08](../casos_de_uso.md#cu-08--compilar-y-entender-los-errores)

## De qué se trata

**F6** compila el ejercicio del archivo activo con su Makefile, compilando antes las
bibliotecas que usa. Cada error y advertencia de gcc aparece en el panel Problemas y al
final de su línea, **explicado en español**, con la columna exacta subrayada. **F1** sobre
la línea abre la explicación completa, con un ejemplo de «así no» y «así sí», y la tecla
`g` lleva a los términos del glosario que menciona.

En la grabación falta un `;` al final de una declaración: gcc informa el error en la línea
siguiente, jonesy lo traduce y ayuda a encontrar la causa real.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:01 | F6 compila el ejercicio del archivo abierto |
| 0:04 | Problemas muestra el error explicado; Alt+N lleva el cursor al error |
| 0:06 | F1 abre la explicación completa con «así no / así sí» |
| 0:13 | El ; falta al final de la declaración de la línea 13: Ctrl+G 13 y se agrega |
| 0:16 | F6 otra vez: compila sin errores |

## Cómo verla

```bash
asciinema play docs/casos/CU-08-compilar-y-entender-los-errores.cast        # de corrido
asciinema play -m docs/casos/CU-08-compilar-y-entender-los-errores.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
