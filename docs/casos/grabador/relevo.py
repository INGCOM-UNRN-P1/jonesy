"""Relevo de una grabación: corre jonesy (o una shell) en una terminal virtual propia,
le manda las teclas del guion y copia todo lo que dibuja a la salida estándar, que es
lo que graba `asciinema rec`.

    python relevo.py <caso> <directorio de trabajo> <marcas.json>

Cada paso del guion espera a que en la pantalla aparezca lo que tiene que aparecer
(con una pantalla virtual de `pyte`); si no aparece, la grabación falla y en la salida
de error queda cómo estaba la pantalla. Las marcas (tiempo y descripción de cada paso)
se guardan para insertarlas como marcadores en el .cast.
"""

import fcntl
import json
import os
import pty
import select
import signal
import struct
import sys
import termios
import time

import pyte

import guiones

# Teclas como las manda una terminal xterm.
ESC = "\x1b"
ENTER = "\r"
TAB = "\t"
SHIFT_TAB = "\x1b[Z"
BORRAR = "\x7f"
ARRIBA, ABAJO, DERECHA, IZQUIERDA = "\x1b[A", "\x1b[B", "\x1b[C", "\x1b[D"
INICIO, FIN = "\x1b[H", "\x1b[F"
REPAG, AVPAG = "\x1b[5~", "\x1b[6~"
_F = {
    1: ("\x1bOP", "\x1b[1;{m}P"),
    2: ("\x1bOQ", "\x1b[1;{m}Q"),
    3: ("\x1bOR", "\x1b[1;{m}R"),
    4: ("\x1bOS", "\x1b[1;{m}S"),
    5: ("\x1b[15~", "\x1b[15;{m}~"),
    6: ("\x1b[17~", "\x1b[17;{m}~"),
    7: ("\x1b[18~", "\x1b[18;{m}~"),
    8: ("\x1b[19~", "\x1b[19;{m}~"),
    9: ("\x1b[20~", "\x1b[20;{m}~"),
    10: ("\x1b[21~", "\x1b[21;{m}~"),
    11: ("\x1b[23~", "\x1b[23;{m}~"),
    12: ("\x1b[24~", "\x1b[24;{m}~"),
}


def F(n, shift=False, ctrl=False, alt=False):
    """La tecla de función `n`, con modificadores."""
    m = 1 + (1 if shift else 0) + (2 if alt else 0) + (4 if ctrl else 0)
    sola, con = _F[n]
    return sola if m == 1 else con.format(m=m)


def CTRL(c):
    return chr(ord(c.lower()) & 0x1F)


def ALT(c):
    return ESC + c


def ALT_FLECHA(f):
    return {"arriba": "\x1b[1;3A", "abajo": "\x1b[1;3B", "izquierda": "\x1b[1;3D"}[f]


CTRL_AVPAG, CTRL_REPAG = "\x1b[6;5~", "\x1b[5;5~"

SINCRONIZACION = b"\x1b]2;casos-jonesy\x07"


class Pantalla(pyte.Screen):
    """Pantalla virtual que contesta las consultas de la aplicación (posición del
    cursor, atributos del dispositivo)."""

    def __init__(self, cols, filas, responder):
        super().__init__(cols, filas)
        self._responder = responder

    def write_process_input(self, data):
        self._responder(data)


class Relevo:
    def __init__(self, caso, trabajo, marcas):
        self.caso = caso
        self.trabajo = trabajo
        self.tp = os.path.join(trabajo, "tp-demo")
        self.marcas_ruta = marcas
        self.cols, self.filas = caso.tamano
        # Punto de sincronización: grabar.py alinea los marcadores con esta secuencia
        # (un título invisible) y después la quita.
        sys.stdout.buffer.write(SINCRONIZACION)
        sys.stdout.buffer.flush()
        self.inicio = time.monotonic()
        self.marcas = []
        self.fd = None
        self.pid = None
        self.en_shell = False
        self._nueva_pantalla()

    # ------------------------------------------------------------ procesos

    def _nueva_pantalla(self):
        self.pantalla = Pantalla(self.cols, self.filas, self._responder)
        self.flujo = pyte.ByteStream(self.pantalla)

    def _responder(self, data):
        if self.fd is not None:
            os.write(self.fd, data.encode() if isinstance(data, str) else data)

    def lanzar(self, argv, cwd=None):
        """Corre `argv` en una terminal virtual nueva."""
        self._nueva_pantalla()
        pid, fd = pty.fork()
        if pid == 0:
            os.chdir(cwd or self.tp)
            os.execvpe(argv[0], argv, os.environ)
        fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", self.filas, self.cols, 0, 0))
        self.pid, self.fd = pid, fd
        # Limpia la pantalla de la grabación entre programas.
        sys.stdout.buffer.write(b"\x1b[2J\x1b[H")
        sys.stdout.buffer.flush()
        self.leer(0.3)

    def jonesy(self, *args, cwd=None):
        self.en_shell = False
        self.lanzar(["jonesy", *args], cwd)

    def shell(self):
        """Una bash limpia con un prompt corto."""
        rc = os.path.join(os.environ["HOME"], ".bashrc")
        self.lanzar(["bash", "--noprofile", "--rcfile", rc, "-i"])
        self.en_shell = True
        self.leer(0.3)

    def leer(self, segundos):
        """Copia lo que llega durante `segundos` a la grabación y a la pantalla virtual."""
        fin = time.monotonic() + segundos
        while True:
            restante = fin - time.monotonic()
            if restante <= 0 or self.fd is None:
                return
            r, _, _ = select.select([self.fd], [], [], min(restante, 0.05))
            if r:
                try:
                    datos = os.read(self.fd, 65536)
                except OSError:
                    datos = b""
                if not datos:
                    self.fd = None
                    return
                sys.stdout.buffer.write(datos)
                sys.stdout.buffer.flush()
                self.flujo.feed(datos)

    def texto(self):
        return "\n".join(self.pantalla.display)

    def cortar(self, senal=signal.SIGHUP):
        """Corta el programa (como cuando se cierra la terminal) y espera que termine."""
        if self.pid:
            try:
                os.kill(self.pid, senal)
            except ProcessLookupError:
                pass
            fin = time.monotonic() + 5
            while time.monotonic() < fin:
                self.leer(0.1)
                if os.waitpid(self.pid, os.WNOHANG) != (0, 0):
                    break
            self.pid, self.fd = None, None

    # ------------------------------------------------------------ guion

    def paso(self, descripcion):
        """Marca el comienzo de un paso (queda como marcador en el .cast)."""
        if os.environ.get("RELEVO_CUADROS"):
            with open(os.environ["RELEVO_CUADROS"], "a", encoding="utf-8") as f:
                f.write(f"===== antes de: {descripcion}\n{self.texto()}\n")
        self.marcas.append([round(time.monotonic() - self.inicio, 3), descripcion])
        self.leer(0.4)

    def teclas(self, *secuencia, pausa=0.35):
        for t in secuencia:
            os.write(self.fd, t.encode())
            self.leer(pausa)

    def escribir(self, texto, cps=14):
        """Tipea `texto` de a un carácter, como una persona."""
        for c in texto:
            os.write(self.fd, ("\r" if c == "\n" else c).encode())
            self.leer(1 / cps)

    def comando(self, linea, esperar=None, tiempo=30):
        """En la shell: tipea un comando, Enter, y espera su salida."""
        self.escribir(linea)
        self.teclas(ENTER, pausa=0.2)
        if esperar:
            self.esperar(esperar, tiempo=tiempo)
        self.pausa(1.5)

    def esperar(self, *textos, tiempo=20):
        """Espera a que aparezca alguno de `textos` en la pantalla."""
        fin = time.monotonic() + tiempo
        while time.monotonic() < fin:
            pantalla = self.texto()
            if any(t in pantalla for t in textos):
                return
            self.leer(0.1)
        raise AssertionError(
            f"{self.caso.id}: no apareció {textos!r} en la pantalla:\n{self.texto()}"
        )

    def esperar_que_no(self, texto, tiempo=20):
        """Espera a que `texto` deje de verse (algo que se está cargando)."""
        fin = time.monotonic() + tiempo
        while time.monotonic() < fin:
            if texto not in self.texto():
                return
            self.leer(0.1)
        raise AssertionError(
            f"{self.caso.id}: sigue en pantalla {texto!r}:\n{self.texto()}"
        )

    def no_esta(self, texto):
        if texto in self.texto():
            raise AssertionError(f"{self.caso.id}: no debería estar {texto!r}:\n{self.texto()}")

    def pausa(self, segundos=1.2):
        self.leer(segundos)

    def salir_jonesy(self):
        """Ctrl+Q y que termine."""
        if self.fd is None:
            return
        self.pausa(1.0)
        os.write(self.fd, CTRL("q").encode())
        if self.en_shell:
            # jonesy corría dentro de la shell: se vuelve al prompt.
            self.leer(1.0)
            self.esperar("ana@lab:", tiempo=10)
            return
        fin = time.monotonic() + 5
        while self.fd is not None and time.monotonic() < fin:
            self.leer(0.1)
        if self.fd is not None:
            self.cortar()

    def correr(self):
        try:
            self.caso.guion(self)
            self.pausa(1.5)
        finally:
            if self.fd is not None:
                self.cortar(signal.SIGTERM)
            with open(self.marcas_ruta, "w", encoding="utf-8") as f:
                json.dump(self.marcas, f, ensure_ascii=False)


def main():
    caso = guiones.CASOS[sys.argv[1]]
    r = Relevo(caso, sys.argv[2], sys.argv[3])
    try:
        r.correr()
    except AssertionError as e:
        with open(sys.argv[3] + ".error", "w", encoding="utf-8") as f:
            f.write(str(e))
        sys.exit(1)


if __name__ == "__main__":
    main()
