# CU-36 · Reportar un problema de jonesy

**Actor:** Alumno, Administrador · **Grabación:** [`CU-36-reportar-un-problema.cast`](CU-36-reportar-un-problema.cast) (0:15) · **En la lista de casos:** [CU-36](../casos_de_uso.md#cu-36--reportar-un-problema-de-jonesy)

## De qué se trata

`jonesy reporte` (o Ayuda → Reportar un problema) arma un informe para pegar en el aviso
del problema: versión, sistema, terminal, un diagnóstico rápido, el `.jonesy.toml`, el
último error de la sesión y el último pánico. No incluye código, el directorio personal
aparece como `~` y se ocultan la identidad de git y el hash de la pausa del examen.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | jonesy reporte --salida guarda el informe en un archivo |
| 0:07 | Versión, sistema, terminal, entorno y configuración (sin el hash de la pausa) |

## Cómo verla

```bash
asciinema play docs/casos/CU-36-reportar-un-problema.cast        # de corrido
asciinema play -m docs/casos/CU-36-reportar-un-problema.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
