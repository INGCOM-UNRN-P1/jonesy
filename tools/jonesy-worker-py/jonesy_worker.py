#!/usr/bin/env python3
"""jonesy-worker-py: worker de extensiones de jonesy.

Corre fuera del proceso del editor y habla JSON-RPC 2.0 por stdin/stdout, un mensaje por
línea. Si un plugin se cuelga o revienta, sólo cae este proceso: el supervisor de jonesy
lo reinicia sin afectar la sesión.

Un plugin es un archivo `.jonesy/plugins/*.py` que puede definir:

    def al_guardar(archivo: str, texto: str) -> list[dict]:
        # cada dict: {"linea": int, "columna": int, "mensaje": str,
        #             "severidad": "error"|"advertencia"|"nota"|"estilo", "codigo": str}
        ...

    def comando_contar(ctx: dict) -> str:   # cualquier función comando_<nombre>
        # ctx: {"archivo": str, "texto": str, "linea": int, "columna": int}
        ...

Métodos: ping, cargar {rutas}, al_guardar {archivo, texto}, comandos, ejecutar
{plugin, comando, ctx}. Sólo usa la biblioteca estándar.
"""

import importlib.util
import json
import os
import sys
import traceback

PLUGINS = {}  # nombre -> módulo
PREFIJO_COMANDO = "comando_"


def _error(nombre, ruta, e):
    """Error de un plugin con la línea de su archivo donde ocurrió (si se sabe)."""
    linea = None
    if isinstance(e, SyntaxError) and e.filename == ruta:
        linea = e.lineno
    else:
        for marco in traceback.extract_tb(e.__traceback__):
            if marco.filename == ruta:
                linea = marco.lineno
    return {"plugin": nombre, "ruta": ruta, "linea": linea,
            "error": f"{type(e).__name__}: {e}"}


def cargar(rutas):
    PLUGINS.clear()
    cargados, errores = [], []
    for ruta in rutas:
        nombre = os.path.splitext(os.path.basename(ruta))[0]
        try:
            spec = importlib.util.spec_from_file_location(f"jonesy_plugin_{nombre}", ruta)
            modulo = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(modulo)
        except Exception as e:  # noqa: BLE001 - un plugin roto no tumba al resto
            errores.append(_error(nombre, ruta, e))
            continue
        PLUGINS[nombre] = modulo
        cargados.append({"nombre": nombre, "comandos": _comandos_de(modulo),
                         "al_guardar": callable(getattr(modulo, "al_guardar", None))})
    return {"plugins": cargados, "errores": errores}


def _comandos_de(modulo):
    return sorted(n[len(PREFIJO_COMANDO):] for n in dir(modulo)
                  if n.startswith(PREFIJO_COMANDO) and callable(getattr(modulo, n)))


def al_guardar(archivo, texto):
    diagnosticos, errores = [], []
    for nombre, modulo in PLUGINS.items():
        funcion = getattr(modulo, "al_guardar", None)
        if not callable(funcion):
            continue
        try:
            for d in funcion(archivo, texto) or []:
                d = dict(d)
                d.setdefault("plugin", nombre)
                diagnosticos.append(d)
        except Exception as e:  # noqa: BLE001
            errores.append(_error(nombre, getattr(modulo, "__file__", ""), e))
    return {"diagnosticos": diagnosticos, "errores": errores}


def comandos():
    return [{"plugin": n, "comando": c} for n, m in PLUGINS.items() for c in _comandos_de(m)]


def ejecutar(plugin, comando, ctx):
    modulo = PLUGINS.get(plugin)
    funcion = getattr(modulo, PREFIJO_COMANDO + comando, None) if modulo else None
    if not callable(funcion):
        raise LookupError(f"comando desconocido: {plugin}.{comando}")
    resultado = funcion(ctx)
    return {"texto": "" if resultado is None else str(resultado)}


METODOS = {
    "ping": lambda: "pong",
    "cargar": cargar,
    "al_guardar": al_guardar,
    "comandos": comandos,
    "ejecutar": ejecutar,
}


def atender(linea):
    try:
        pedido = json.loads(linea)
    except json.JSONDecodeError as e:
        return {"jsonrpc": "2.0", "id": None, "error": {"code": -32700, "message": str(e)}}
    ident = pedido.get("id")
    metodo = METODOS.get(pedido.get("method"))
    if metodo is None:
        return {"jsonrpc": "2.0", "id": ident,
                "error": {"code": -32601, "message": f"método desconocido: {pedido.get('method')}"}}
    try:
        resultado = metodo(**(pedido.get("params") or {}))
    except Exception as e:  # noqa: BLE001
        return {"jsonrpc": "2.0", "id": ident,
                "error": {"code": -32000, "message": f"{type(e).__name__}: {e}",
                          "data": traceback.format_exc(limit=5)}}
    return {"jsonrpc": "2.0", "id": ident, "result": resultado}


def main():
    # La salida de los plugins (print) no debe corromper el canal: va a stderr.
    canal = sys.stdout
    sys.stdout = sys.stderr
    for linea in sys.stdin:
        if not linea.strip():
            continue
        canal.write(json.dumps(atender(linea), ensure_ascii=False) + "\n")
        canal.flush()


if __name__ == "__main__":
    main()
