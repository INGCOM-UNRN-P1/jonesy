"""Muestra cómo se ve una grabación en cada paso (para revisarlas sin reproducirlas).

    uv run --with pyte python docs/casos/grabador/cuadros.py <caso.cast> [paso]

Sin `paso`, imprime la pantalla justo antes de cada marcador y al final; con un número,
sólo la de ese paso (1 = el primero).
"""

import json
import sys

import pyte


def main():
    lineas = open(sys.argv[1], encoding="utf-8").read().splitlines()
    cab = json.loads(lineas[0])
    pantalla = pyte.Screen(cab["width"], cab["height"])
    flujo = pyte.Stream(pantalla)
    elegido = int(sys.argv[2]) if len(sys.argv) > 2 else None
    n = 0

    def mostrar(titulo):
        if elegido is None or elegido == n:
            print(f"===== {titulo}")
            print("\n".join(pantalla.display))

    for l in lineas[1:]:
        if not l.strip():
            continue
        t, tipo, dato = json.loads(l)
        if tipo == "o":
            flujo.feed(dato)
        elif tipo == "m":
            if n:
                mostrar(f"fin del paso {n} ({t:.1f} s)")
            n += 1
    mostrar("final")


if __name__ == "__main__":
    main()
