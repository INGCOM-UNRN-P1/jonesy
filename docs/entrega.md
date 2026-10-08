# Entrega del examen y su verificación

## Qué arma jonesy

Al entregar (F12 o `[ Entregar Examen ]`), jonesy guarda todo, corre la verificación
(`[examen] verificar`, `make test` o el `verificar` del entorno de la cátedra), cierra la
bitácora con `SessionEnd` y arma `.jonesy/entrega/entrega-<usuario>-<ts>.tar`:

| Entrada | Contenido |
|---|---|
| `entrega/<ruta>` | cada archivo de la entrega (`*.c`, `*.h`, `*.md`, Makefile o `[examen] archivos`) |
| `telemetry.log` | la bitácora JSON Lines con SHA-256 encadenado (§9.1 de la especificación) |
| `entrega.json` | usuario, momento, versión de jonesy, resultado de la verificación, SHA-256 de cada archivo, último hash y cantidad de registros de la bitácora |

Junto al paquete queda `<paquete>.tar.sha256`, y el SHA-256 se le muestra al alumno.

## Cómo verificarla (dredd, docentes)

```bash
jonesy verificar entrega-ana-1700000000.tar          # legible
jonesy verificar entrega-ana-1700000000.tar --json   # para dredd
```

Comprueba, y sale con 1 si algo falla:

1. el SHA-256 del paquete contra su `.sha256` (si está al lado);
2. que cada archivo coincida con el SHA-256 del manifiesto y que no sobren ni falten;
3. la cadena completa de `telemetry.log` (`seq`, `prev_hash`, `payload_hash`);
4. que el último hash y la cantidad de registros sean los sellados en `entrega.json`
   (nadie agregó ni quitó registros después de entregar);
5. que la sesión termine con `SessionEnd`.

Con `--json` emite el sobre común del ecosistema:

```json
{"schema_version": "1.0.0", "herramienta": "jonesy", "comando": "verificar",
 "ok": true, "paquete": "…", "sha256": "…", "usuario": "ana",
 "pegados": 1, "caracteres_pegados": 120,
 "chequeos": [{"nombre": "cadena de la bitácora", "ok": true, "detalle": "…"}]}
```

`pegados` y `caracteres_pegados` resumen lo que llegó por pegado (bracketed paste);
`jonesy replay telemetry.log` permite revisar la sesión paso a paso.

## Para dredd

dredd no tiene que reimplementar la cadena: alcanza con correr
`jonesy verificar <paquete.tar> --json` y usar `ok` y `chequeos` para su sección de
devolución. Integrarlo es un cambio en el repositorio de dredd.
