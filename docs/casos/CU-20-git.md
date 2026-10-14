# CU-20 · Versionar con git

**Actor:** Alumno · **Grabación:** [`CU-20-git.cast`](CU-20-git.cast) (0:25) · **En la lista de casos:** [CU-20](../casos_de_uso.md#cu-20--versionar-con-git)

## De qué se trata

**Alt+G** abre el panel de git: los archivos cambiados, **Espacio** los agrega o saca del
próximo commit, **Enter** muestra el diff lado a lado, **c** hace el commit (con una
revisión del mensaje), **l** muestra el historial, **s** sube y **b** trae (con ayuda ante
conflictos) y **z** deshace el último commit. Usa la identidad y las credenciales de git
del alumno.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | Un cambio en el código, guardado |
| 0:04 | Alt+G: el panel de git con el archivo cambiado |
| 0:07 | Enter muestra el diff lado a lado |
| 0:11 | Espacio lo agrega al commit; c escribe el mensaje y Ctrl+S confirma |
| 0:19 | l muestra el historial con el commit nuevo |
| 0:21 | s sube el commit al repositorio remoto |

## Cómo verla

```bash
asciinema play docs/casos/CU-20-git.cast        # de corrido
asciinema play -m docs/casos/CU-20-git.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
