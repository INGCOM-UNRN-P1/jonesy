# CU-02 · Abrir un TP o un ejercicio

**Actor:** Alumno · **Grabación:** [`CU-02-abrir-un-tp.cast`](CU-02-abrir-un-tp.cast) (0:26) · **En la lista de casos:** [CU-02](../casos_de_uso.md#cu-02--abrir-un-tp-o-un-ejercicio)

## De qué se trata

Se abre jonesy en el directorio del TP (o se le pasa un directorio o archivos). jonesy
busca la raíz del proyecto, lee el Makefile general, los de cada biblioteca y ejercicio y
`tp.sh`, y muestra la consigna al lado del código. **Alt+Y** abre la vista del proyecto:
bibliotecas, ejercicios y sus objetivos de make, listos para correr. **Ctrl+O** abre
cualquier archivo buscándolo por nombre. Al volver a abrir el proyecto, jonesy restaura
la sesión: los mismos archivos, cursores y paneles.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | En la carpeta del TP: un Makefile general, bibliotecas y ejercicios |
| 0:04 | jonesy abre el proyecto con la consigna al lado del código |
| 0:07 | Alt+Y: la vista del proyecto con bibliotecas, ejercicios y objetivos |
| 0:15 | Ctrl+O busca y abre cualquier archivo del proyecto |
| 0:19 | Ctrl+Q sale; al volver a abrir, la sesión está como se dejó |

## Cómo verla

```bash
asciinema play docs/casos/CU-02-abrir-un-tp.cast        # de corrido
asciinema play -m docs/casos/CU-02-abrir-un-tp.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
