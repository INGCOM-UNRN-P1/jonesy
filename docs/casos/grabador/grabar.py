"""Graba los casos de uso de jonesy con asciinema y escribe su documento.

    bin/grabar-casos [CU-08 CU-11 ...] [--jonesy target/release/jonesy]

Para cada caso: copia el proyecto de demostración (`demo/`) a un directorio de trabajo,
le aplica la preparación del caso, graba con `asciinema rec --headless` al relevo
(`relevo.py`), que maneja jonesy con las teclas del guion y verifica cada paso, inserta
los pasos como marcadores en el .cast y escribe `docs/casos/CU-XX-…md` con la
descripción y los pasos con sus tiempos. Al final regenera el índice
(`docs/casos/README.md`).

Necesita asciinema 3, Python con `pyte` (el script `bin/grabar-casos` lo resuelve con
`uv run --with pyte`), gcc, make, gdb y valgrind.
"""

import argparse
import json
import os
import re
import shutil
import subprocess
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
CASOS_DIR = os.path.dirname(AQUI)
RAIZ = os.path.dirname(os.path.dirname(CASOS_DIR))
sys.path.insert(0, AQUI)

import guiones  # noqa: E402

TRABAJO = "/tmp/casos-jonesy"
LIMITE_INACTIVO = 2.0
# El directorio personal real no queda en las grabaciones (mismo largo, para no
# correr el dibujo).
HOME_REAL = os.path.expanduser("~")


def anonimo(home):
    nombre = os.path.basename(home)
    reemplazo = ("anita" * 4)[: len(nombre)]
    return os.path.join(os.path.dirname(home), reemplazo)


def preparar(caso, jonesy_bin):
    trabajo = os.path.join(TRABAJO, caso.id)
    shutil.rmtree(trabajo, ignore_errors=True)
    os.makedirs(trabajo)
    tp = os.path.join(trabajo, "tp-demo")
    shutil.copytree(os.path.join(AQUI, "demo"), tp)
    estado = os.path.join(trabajo, "estado")
    config = os.path.join(trabajo, "config")
    home = os.path.join(trabajo, "home")
    for d in (estado, config, home):
        os.makedirs(d)
    if caso.bienvenida_vista:
        open(os.path.join(estado, "bienvenida-vista"), "w").close()
    with open(os.path.join(home, ".bashrc"), "w") as f:
        f.write("PS1='\\[\\e[1;32m\\]ana@lab\\[\\e[0m\\]:\\[\\e[1;34m\\]\\W\\[\\e[0m\\]$ '\n")
    gitconfig = os.path.join(trabajo, "gitconfig")
    with open(gitconfig, "w") as f:
        f.write(
            "[user]\n\tname = Ana Alumna\n\temail = ana@ejemplo.edu.ar\n"
            "[init]\n\tdefaultBranch = main\n[advice]\n\tdetachedHead = false\n"
        )
    bindir = os.path.join(trabajo, "bin")
    os.makedirs(bindir)
    os.symlink(jonesy_bin, os.path.join(bindir, "jonesy"))
    env = dict(os.environ)
    env.update(
        {
            "PATH": bindir + os.pathsep + env.get("GRABAR_PATH", env.get("PATH", "")),
            "HOME": home,
            "SHELL": "/bin/bash",
            "TERM": "xterm-256color",
            "COLORTERM": "truecolor",
            "LANG": "C.UTF-8",
            "LC_ALL": "C.UTF-8",
            "JONESY_ESTADO": estado,
            "JONESY_CONFIG": config,
            "GIT_CONFIG_GLOBAL": gitconfig,
            "GIT_CONFIG_NOSYSTEM": "1",
            "PS1": "ana@lab:\\W$ ",
        }
    )
    for k in ("TERM_PROGRAM", "TMUX", "NO_COLOR", "JONESY_EXAM_MODE", "VIRTUAL_ENV",
              "GRABAR_PATH", "UV", "UV_RUN_RECURSION_DEPTH"):
        env.pop(k, None)
    if caso.preparar:
        caso.preparar(guiones.Preparacion(trabajo, tp, env))
    return trabajo, env


def con_marcadores(cast, marcas, home):
    """Inserta los marcadores (asciicast v2) y saca el directorio personal real."""
    lineas = open(cast, encoding="utf-8").read().splitlines()
    cabecera = json.loads(lineas[0])
    eventos = [json.loads(l) for l in lineas[1:] if l.strip()]
    falso = anonimo(home)
    sincronizacion = "\x1b]2;casos-jonesy\x07"
    origen = next((e[0] for e in eventos if e[1] == "o" and sincronizacion in e[2]), 0.0)
    for e in eventos:
        if e[1] == "o":
            e[2] = e[2].replace(sincronizacion, "").replace(home, falso)
    eventos = [e for e in eventos if e[1] != "o" or e[2]]
    eventos += [[round(t + origen, 6), "m", texto] for t, texto in marcas]
    eventos.sort(key=lambda e: (e[0], e[1] != "m"))
    cabecera["idle_time_limit"] = LIMITE_INACTIVO
    cabecera.pop("env", None)
    cabecera.pop("command", None)
    with open(cast, "w", encoding="utf-8") as f:
        f.write(json.dumps(cabecera, ensure_ascii=False) + "\n")
        for e in eventos:
            f.write(json.dumps(e, ensure_ascii=False) + "\n")
    # Tiempos como se ven al reproducir (las pausas largas se acortan).
    reproduccion, anterior, t = [], 0.0, 0.0
    for e in eventos:
        t += min(e[0] - anterior, LIMITE_INACTIVO)
        anterior = e[0]
        if e[1] == "m":
            reproduccion.append((t, e[2]))
    return reproduccion, t


def minutos(t):
    return f"{int(t) // 60}:{int(t) % 60:02d}"


def ancla(texto):
    t = texto.lower()
    t = re.sub(r"[^\w\- ]", "", t)
    return t.replace(" ", "-")


def documento(caso, pasos, duracion):
    nombre = f"{caso.id}-{caso.slug}"
    titulo_lista = f"{caso.id} · {caso.titulo}"
    s = [
        f"# {caso.id} · {caso.titulo}",
        "",
        f"**Actor:** {caso.actor} · **Grabación:** [`{nombre}.cast`]({nombre}.cast) "
        f"({minutos(duracion)}) · **En la lista de casos:** "
        f"[{caso.id}](../casos_de_uso.md#{ancla(titulo_lista)})",
        "",
        "## De qué se trata",
        "",
        caso.descripcion.strip(),
        "",
        "## Qué muestra la grabación",
        "",
        "| Tiempo | Paso |",
        "|---|---|",
    ]
    for t, p in pasos:
        s.append(f"| {minutos(t)} | {p} |")
    s += [
        "",
        "## Cómo verla",
        "",
        "```bash",
        f"asciinema play docs/casos/{nombre}.cast        # de corrido",
        f"asciinema play -m docs/casos/{nombre}.cast     # se detiene en cada paso (Espacio sigue)",
        "```",
        "",
        "Las pausas largas (compilar, esperar al programa) se acortan a "
        f"{LIMITE_INACTIVO:.0f} s al reproducir. La grabación se hizo con el proyecto de "
        "demostración de [`grabador/demo`](grabador/demo) en una terminal de "
        f"{caso.tamano[0]}×{caso.tamano[1]}.",
    ]
    if caso.notas:
        s += ["", "## Notas", "", caso.notas.strip()]
    with open(os.path.join(CASOS_DIR, nombre + ".md"), "w", encoding="utf-8") as f:
        f.write("\n".join(s) + "\n")


def indice():
    s = [
        "# Casos de uso grabados",
        "",
        "Una grabación de terminal ([asciinema](https://asciinema.org)) por cada caso de",
        "[la lista de casos de uso](../casos_de_uso.md), con un documento que explica el caso",
        "y los pasos de la grabación con sus tiempos.",
        "",
        "```bash",
        "asciinema play -m docs/casos/CU-08-compilar-y-entender-los-errores.cast",
        "```",
        "",
        "Con `-m` la reproducción se detiene en cada paso (Espacio sigue, `]` salta al",
        "siguiente). Las grabaciones usan el proyecto de demostración de",
        "[`grabador/demo`](grabador/demo): un TP con Makefile general, una biblioteca",
        "(`libs/vector`) y tres ejercicios.",
        "",
        "| Caso | Actor | Duración | Grabación |",
        "|---|---|---|---|",
    ]
    for c in sorted(guiones.CASOS.values(), key=lambda c: c.id):
        nombre = f"{c.id}-{c.slug}"
        cast = os.path.join(CASOS_DIR, nombre + ".cast")
        dur = "—"
        if os.path.isfile(cast):
            dur = minutos(duracion_cast(cast))
        s.append(
            f"| [{c.id} · {c.titulo}]({nombre}.md) | {c.actor} | {dur} | "
            f"[`{nombre}.cast`]({nombre}.cast) |"
        )
    s += [
        "",
        "## Volver a grabarlas",
        "",
        "```bash",
        "bin/grabar-casos                 # todas",
        "bin/grabar-casos CU-08 CU-14     # algunas",
        "```",
        "",
        "Cada guion está en [`grabador/guiones.py`](grabador/guiones.py): las teclas de cada",
        "paso y lo que tiene que aparecer en la pantalla. Si algo no aparece, la grabación",
        "falla y muestra la pantalla de ese momento, así que sirven también como prueba de",
        "punta a punta de la interfaz. Necesita asciinema 3, `uv` (para `pyte`), gcc, make,",
        "gdb y valgrind.",
    ]
    with open(os.path.join(CASOS_DIR, "README.md"), "w", encoding="utf-8") as f:
        f.write("\n".join(s) + "\n")


def duracion_cast(cast):
    lineas = open(cast, encoding="utf-8").read().splitlines()
    limite = json.loads(lineas[0]).get("idle_time_limit") or 1e9
    t, anterior = 0.0, 0.0
    for l in lineas[1:]:
        if l.strip():
            e = json.loads(l)
            t += min(e[0] - anterior, limite)
            anterior = e[0]
    return t


def grabar(caso, jonesy_bin):
    trabajo, env = preparar(caso, jonesy_bin)
    nombre = f"{caso.id}-{caso.slug}"
    cast = os.path.join(CASOS_DIR, nombre + ".cast")
    marcas = os.path.join(trabajo, "marcas.json")
    relevo = f"{sys.executable} {os.path.join(AQUI, 'relevo.py')} {caso.id} {trabajo} {marcas}"
    r = subprocess.run(
        [
            "asciinema",
            "rec",
            "--headless",
            "--quiet",
            "--overwrite",
            "--return",
            "--output-format",
            "asciicast-v2",
            "--window-size",
            f"{caso.tamano[0]}x{caso.tamano[1]}",
            "--title",
            f"jonesy · {caso.id} · {caso.titulo}",
            "--command",
            relevo,
            cast,
        ],
        env=env,
        cwd=os.path.join(trabajo, "tp-demo"),
    )
    if r.returncode != 0:
        error = marcas + ".error"
        detalle = open(error).read() if os.path.isfile(error) else f"código {r.returncode}"
        return False, detalle
    pasos, duracion = con_marcadores(cast, json.load(open(marcas)), HOME_REAL)
    documento(caso, pasos, duracion)
    return True, f"{minutos(duracion)}, {len(pasos)} pasos"


def main():
    p = argparse.ArgumentParser()
    p.add_argument("casos", nargs="*")
    p.add_argument("--jonesy", default=os.path.join(RAIZ, "target", "release", "jonesy"))
    a = p.parse_args()
    if not os.path.isfile(a.jonesy):
        subprocess.run(["cargo", "build", "--release", "-p", "jonesy-bin"], cwd=RAIZ, check=True)
    elegidos = a.casos or sorted(guiones.CASOS)
    fallas = []
    for id_ in elegidos:
        caso = guiones.CASOS[id_]
        print(f"{caso.id} {caso.titulo}… ", end="", flush=True)
        ok, detalle = grabar(caso, os.path.abspath(a.jonesy))
        print("ok (" + detalle + ")" if ok else "FALLÓ")
        if not ok:
            fallas.append((caso.id, detalle))
    indice()
    for id_, detalle in fallas:
        print(f"\n--- {id_} ---\n{detalle}", file=sys.stderr)
    sys.exit(1 if fallas else 0)


if __name__ == "__main__":
    main()
