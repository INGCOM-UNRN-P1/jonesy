# CU-34 · Extender jonesy con plugins y herramientas

**Actor:** Docente · **Grabación:** [`CU-34-plugins.cast`](CU-34-plugins.cast) (0:16) · **En la lista de casos:** [CU-34](../casos_de_uso.md#cu-34--extender-jonesy-con-plugins-y-herramientas)

## De qué se trata

Los plugins son scripts de Python en `.jonesy/plugins/`: cada función `comando_*` aparece
en el menú **Plugins** y en los comandos, recibe el archivo y la selección y devuelve un
texto o diagnósticos; sus errores aparecen en Problemas en su línea. **Nuevo plugin
Python** parte de una plantilla y al guardarlo se recarga. Las herramientas externas se
suman con manifiestos de integraciones (ver `docs/integraciones.md`).

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | Nuevo plugin Python: parte de una plantilla |
| 0:08 | Al guardarlo se recarga; Alt+L muestra sus comandos en el menú Plugins |
| 0:13 | Enter corre el comando sobre el archivo abierto |

## Cómo verla

```bash
asciinema play docs/casos/CU-34-plugins.cast        # de corrido
asciinema play -m docs/casos/CU-34-plugins.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
