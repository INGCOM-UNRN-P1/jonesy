# CU-35 · Revisar la salud del entorno

**Actor:** Alumno, Administrador · **Grabación:** [`CU-35-doctor.cast`](CU-35-doctor.cast) (0:12) · **En la lista de casos:** [CU-35](../casos_de_uso.md#cu-35--revisar-la-salud-del-entorno)

## De qué se trata

`jonesy doctor` (o Ayuda → Diagnóstico del entorno) revisa lo que jonesy necesita:
compilar (gcc, make), depurar (gdb, ptrace), memoria (valgrind, mem-trace), git, la
terminal, Python, el proyecto y las herramientas del ecosistema. Sin `--rapido` además
compila, depura y revisa un programa de prueba. Cada falta viene con para qué sirve y
cómo instalarla; termina con código 1 si falta algo requerido.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | jonesy doctor --rapido: qué hay y qué falta en el entorno |
| 0:05 | jonesy doctor: además compila, depura y revisa un programa de prueba |

## Cómo verla

```bash
asciinema play docs/casos/CU-35-doctor.cast        # de corrido
asciinema play -m docs/casos/CU-35-doctor.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
