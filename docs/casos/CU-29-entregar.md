# CU-29 · Revisar y confirmar la entrega

**Actor:** Alumno en examen · **Grabación:** [`CU-29-entregar.cast`](CU-29-entregar.cast) (0:11) · **En la lista de casos:** [CU-29](../casos_de_uso.md#cu-29--revisar-y-confirmar-la-entrega)

## De qué se trata

**F12** (o el botón `[ Entregar Examen ]`) abre la entrega. Antes de confirmar, la ventana
lista los archivos que van en el paquete (y avisa si un `.c` abierto queda afuera) y cómo
terminaron la última compilación y las últimas pruebas. Al confirmar, jonesy guarda todo,
corre la verificación (`[examen] verificar`, por ejemplo `make test` o `ripley run`),
sella la bitácora y arma el paquete `.tar` con su SHA-256, que identifica la entrega.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | Antes de entregar, se corren las pruebas |
| 0:02 | F12: qué archivos van en el paquete y cómo están la compilación y las pruebas |
| 0:07 | s confirma: verificación, bitácora sellada y paquete con su SHA-256 |

## Cómo verla

```bash
asciinema play docs/casos/CU-29-entregar.cast        # de corrido
asciinema play -m docs/casos/CU-29-entregar.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
