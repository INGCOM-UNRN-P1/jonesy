# CU-25 · Ajustar la apariencia y llevarla a otra máquina

**Actor:** Alumno · **Grabación:** [`CU-25-apariencia-y-preferencias.cast`](CU-25-apariencia-y-preferencias.cast) (0:33) · **En la lista de casos:** [CU-25](../casos_de_uso.md#cu-25--ajustar-la-apariencia-y-llevarla-a-otra-máquina)

## De qué se trata

**Archivo → Configuración del entorno** (Ctrl+,) reúne lo que configura el IDE. En
**Apariencia** se elige el tema (oscuro, claro, contraste, turbo al estilo Turbo C++,
solarizado, fósforo), la posición del quick-doc y los tamaños de los paneles. **Exportar
mis preferencias** las guarda en un `.toml` (con los manifiestos de integraciones
propios) para llevarlas a otra máquina, donde **Importar** (o `jonesy config importar`)
las aplica y las deja para los proyectos nuevos.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | Configuración del entorno, solapa Apariencia |
| 0:07 | Enter sobre Tema cambia el tema: claro, contraste, turbo… |
| 0:15 | Exportar mis preferencias a un archivo |
| 0:23 | El archivo exportado, para importarlo en otra máquina |

## Cómo verla

```bash
asciinema play docs/casos/CU-25-apariencia-y-preferencias.cast        # de corrido
asciinema play -m docs/casos/CU-25-apariencia-y-preferencias.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
