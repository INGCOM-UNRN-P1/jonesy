"""Guiones de las grabaciones de los casos de uso (docs/casos_de_uso.md).

Cada caso es una función que recibe el relevo (`s`) y usa:

- `s.jonesy(*args)` / `s.shell()`: arranca jonesy (en el TP de demostración) o una bash;
- `s.paso("…")`: marca el comienzo de un paso (queda como marcador en el .cast y en la
  tabla del documento);
- `s.teclas(…)`, `s.escribir("…")`, `s.comando("…", esperar=…)`: lo que tipea;
- `s.esperar("…")`: lo que tiene que aparecer en pantalla (si no, la grabación falla);
- `s.pausa(seg)`: para que se alcance a leer.

El docstring es la descripción del documento del caso.
"""

import os
import subprocess
from dataclasses import dataclass, field
from typing import Callable, Optional

from relevo import (
    ABAJO,
    ALT,
    ALT_FLECHA,
    ARRIBA,
    AVPAG,
    BORRAR,
    CTRL,
    CTRL_AVPAG,
    DERECHA,
    ENTER,
    ESC,
    F,
    FIN,
    INICIO,
    IZQUIERDA,
    REPAG,
    SHIFT_TAB,
    TAB,
)

CASOS = {}


@dataclass
class Caso:
    id: str
    slug: str
    titulo: str
    actor: str
    guion: Callable
    descripcion: str
    preparar: Optional[Callable] = None
    tamano: tuple = (132, 36)
    bienvenida_vista: bool = True
    notas: str = ""


def caso(id_, slug, titulo, actor="Alumno", preparar=None, tamano=(132, 36),
         bienvenida_vista=True, notas=""):
    def registrar(f):
        CASOS[id_] = Caso(id_, slug, titulo, actor, f, f.__doc__ or "", preparar, tamano,
                          bienvenida_vista, notas)
        return f

    return registrar


@dataclass
class Preparacion:
    """Lo que puede hacer la preparación de un caso antes de grabarlo."""

    trabajo: str
    tp: str
    env: dict = field(default_factory=dict)

    def ruta(self, rel):
        return os.path.join(self.tp, rel)

    def reemplazar(self, rel, viejo, nuevo):
        p = self.ruta(rel)
        texto = open(p, encoding="utf-8").read()
        assert viejo in texto, f"{rel}: no está {viejo!r}"
        open(p, "w", encoding="utf-8").write(texto.replace(viejo, nuevo, 1))

    def escribir(self, rel, texto):
        p = self.ruta(rel)
        os.makedirs(os.path.dirname(p), exist_ok=True)
        open(p, "w", encoding="utf-8").write(texto)

    def correr(self, *argv, cwd=None):
        subprocess.run(argv, cwd=cwd or self.tp, env=self.env, check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    def git_inicial(self):
        self.correr("git", "init", "-q")
        self.escribir(".gitignore", "*.o\n*.a\ntest_bin\npromedio\nintercambio\ncopia\n.jonesy/\n.frama-c/\n")
        self.correr("git", "add", "-A")
        self.correr("git", "commit", "-q", "-m", "Primera versión del TP")


EJ1 = "ejercicios/ej1-promedio/main.c"


# ======================================================================= alumno


@caso("CU-01", "primer-uso", "Empezar a usar jonesy por primera vez", bienvenida_vista=False)
def cu01(s):
    """
La primera vez que se abre jonesy aparece la pantalla de bienvenida: las teclas del ciclo
de trabajo de un ejercicio (consigna, guardar, compilar, ejecutar, probar, depurar) y, si
falta algo en el entorno, qué es y cómo instalarlo. Desde ahí, `R` empieza el
**recorrido guiado**, que resalta en la pantalla cada zona (menús, editor, consigna,
quick-doc, barra de estado, panel de abajo y barra izquierda), dice para qué sirve y, si
está cerrada, cómo abrirla. La bienvenida, el recorrido y la guía rápida quedan después en
el menú Ayuda.
"""
    s.jonesy()
    s.paso("Al abrir jonesy por primera vez aparece la bienvenida")
    s.esperar("Te damos la bienvenida")
    s.pausa(3)
    s.paso("R empieza el recorrido guiado por la pantalla")
    s.teclas("r")
    s.esperar("Los menús · 1/8")
    s.pausa(2.5)
    for titulo in ("El editor", "La consigna", "La documentación", "La barra de estado",
                   "El panel de abajo", "La barra izquierda"):
        s.paso(f"Enter pasa a la zona siguiente: {titulo.lower()}")
        s.teclas(ENTER)
        s.esperar(titulo)
        s.pausa(2.5)
    s.paso("El último paso dice dónde volver a encontrar la ayuda")
    s.teclas(ENTER)
    s.esperar("Listo")
    s.pausa(2.5)
    s.teclas(ENTER)
    s.salir_jonesy()


@caso("CU-08", "compilar-y-entender-los-errores", "Compilar y entender los errores",
      preparar=lambda p: p.reemplazar(EJ1, "    int leidos = 0;\n", "    int leidos = 0\n"))
def cu08(s):
    """
**F6** compila el ejercicio del archivo activo con su Makefile, compilando antes las
bibliotecas que usa. Cada error y advertencia de gcc aparece en el panel Problemas y al
final de su línea, **explicado en español**, con la columna exacta subrayada. **F1** sobre
la línea abre la explicación completa, con un ejemplo de «así no» y «así sí», y la tecla
`g` lleva a los términos del glosario que menciona.

En la grabación falta un `;` al final de una declaración: gcc informa el error en la línea
siguiente, jonesy lo traduce y ayuda a encontrar la causa real.
"""
    s.jonesy(EJ1)
    s.esperar("main.c")
    s.pausa(1.5)
    s.paso("F6 compila el ejercicio del archivo abierto")
    s.teclas(F(6))
    s.esperar("Problemas (1)", tiempo=60)
    s.pausa(3)
    s.paso("Problemas muestra el error explicado; Alt+N lleva el cursor al error")
    s.teclas(ALT("n"))
    s.pausa(2.5)
    s.paso("F1 abre la explicación completa con «así no / así sí»")
    s.teclas(F(1))
    s.esperar("Así")
    s.pausa(4)
    s.teclas(AVPAG)
    s.pausa(2)
    s.teclas(ESC)
    s.paso("El ; falta al final de la declaración de la línea 13: Ctrl+G 13 y se agrega")
    s.teclas(CTRL("g"))
    s.escribir("13")
    s.teclas(ENTER, FIN)
    s.escribir(";")
    s.teclas(CTRL("s"))
    s.pausa(1)
    s.paso("F6 otra vez: compila sin errores")
    s.teclas(F(6))
    s.esperar("compilación correcta", tiempo=60)
    s.pausa(3)
    s.salir_jonesy()


# ----------------------------------------------------------------- ayudas


def paleta(s, texto, pausa=1.2):
    """Ctrl+K, busca una acción por su nombre y la corre."""
    s.teclas(CTRL("k"))
    s.escribir(texto)
    s.pausa(pausa)
    s.teclas(ENTER)


def buscar(s, texto):
    """Ctrl+F con el campo vacío (conserva la búsqueda anterior), Enter y Esc: queda
    seleccionada la coincidencia."""
    s.teclas(CTRL("f"))
    s.teclas(*([BORRAR] * 30), pausa=0.01)
    s.escribir(texto)
    s.teclas(ENTER, ESC)


def ir_a_linea(s, n):
    s.teclas(CTRL("g"))
    s.escribir(str(n))
    s.teclas(ENTER)


EJ2 = "ejercicios/ej2-intercambio/main.c"
EJ3 = "ejercicios/ej3-memoria/main.c"


def bug_promedio(p):
    p.reemplazar("ejercicios/ej1-promedio/promedio.c",
                 "return (double)vector_suma(v, n) / n;", "return vector_suma(v, n) / n;")


def desde_cu29(p):
    """Parte de la entrega que hizo CU-29 (el paquete y la bitácora)."""
    import shutil
    origen = os.path.join(os.path.dirname(p.trabajo), "CU-29", "tp-demo")
    assert os.path.isdir(os.path.join(origen, ".jonesy", "entrega")), \
        "grabá antes CU-29 (deja el paquete que usa este caso)"
    shutil.rmtree(p.tp)
    shutil.copytree(origen, p.tp, symlinks=True)


# ----------------------------------------------------------------- casos


@caso("CU-02", "abrir-un-tp", "Abrir un TP o un ejercicio")
def cu02(s):
    """
Se abre jonesy en el directorio del TP (o se le pasa un directorio o archivos). jonesy
busca la raíz del proyecto, lee el Makefile general, los de cada biblioteca y ejercicio y
`tp.sh`, y muestra la consigna al lado del código. **Alt+Y** abre la vista del proyecto:
bibliotecas, ejercicios y sus objetivos de make, listos para correr. **Ctrl+O** abre
cualquier archivo buscándolo por nombre. Al volver a abrir el proyecto, jonesy restaura
la sesión: los mismos archivos, cursores y paneles.
"""
    s.shell()
    s.paso("En la carpeta del TP: un Makefile general, bibliotecas y ejercicios")
    s.comando("ls -F libs ejercicios", esperar="ej1-promedio")
    s.paso("jonesy abre el proyecto con la consigna al lado del código")
    s.comando("jonesy", esperar="jonesy · tp-demo")
    s.pausa(2)
    s.paso("Alt+Y: la vista del proyecto con bibliotecas, ejercicios y objetivos")
    s.teclas(ALT("y"))
    s.esperar("EJERCICIOS")
    s.pausa(1.5)
    s.teclas(ABAJO, ABAJO, ABAJO, ABAJO, ABAJO, ABAJO, ABAJO, ABAJO, ABAJO, pausa=0.4)
    s.teclas(DERECHA, pausa=0.6)
    s.pausa(2.5)
    s.teclas(ALT("y"))
    s.teclas(ESC)
    s.paso("Ctrl+O busca y abre cualquier archivo del proyecto")
    s.teclas(CTRL("o"))
    s.escribir("copia.c")
    s.pausa(1)
    s.teclas(ENTER)
    s.esperar("copia.c Ln")
    s.pausa(1.5)
    s.paso("Ctrl+Q sale; al volver a abrir, la sesión está como se dejó")
    s.salir_jonesy()
    s.esperar("ana@lab")
    s.comando("jonesy", esperar="copia.c Ln")
    s.pausa(2.5)
    s.salir_jonesy()


@caso("CU-03", "leer-la-consigna", "Leer la consigna mientras se programa")
def cu03(s):
    """
El enunciado (`consigna.md`, `enunciado.md` o el README) se muestra de solo lectura al
lado del código, con el Markdown dibujado. **F7** pone el foco en la consigna: las
flechas y RePág/AvPág la desplazan y **Tab** recorre sus enlaces (Enter los sigue). Otra
vez **F7** la oculta para ganar espacio y vuelve al código. En pantallas chicas F7 la
muestra a pantalla completa y F8 vuelve al código.
"""
    s.jonesy(EJ1)
    s.esperar("TP demo")
    s.paso("La consigna está al lado del código, con el Markdown dibujado")
    s.pausa(2.5)
    s.paso("F7 pone el foco en la consigna; las flechas la desplazan")
    s.teclas(F(7))
    s.teclas(ABAJO, ABAJO, ABAJO, ABAJO, ABAJO, ABAJO, pausa=0.4)
    s.pausa(1.5)
    s.paso("Tab recorre los enlaces de la consigna")
    s.teclas(TAB)
    s.pausa(2)
    s.paso("F7 otra vez la oculta y vuelve al código; F7 la muestra de nuevo")
    s.teclas(F(7))
    s.pausa(2)
    s.teclas(F(7))
    s.esperar("Consigna")
    s.pausa(1.5)
    s.teclas(ESC)
    s.salir_jonesy()


@caso("CU-04", "escribir-y-editar", "Escribir y editar código")
def cu04(s):
    """
El editor usa los atajos de siempre (Ctrl+S, Ctrl+Z, Ctrl+C/V) y suma lo que ayuda con C:
los paréntesis y comillas se cierran solos, **Ctrl+D** duplica la línea, **Alt+↑/↓** la
mueve, **Ctrl+/** comenta o descomenta, **Ctrl+F** busca y **Ctrl+H** reemplaza. Al
guardar, las herramientas de la cátedra revisan el archivo (ver CU-09).
"""
    s.jonesy(EJ3)
    s.esperar("main.c")
    s.paso("Ctrl+G va a una línea; al escribir, paréntesis y comillas se cierran solos")
    ir_a_linea(s, 16)
    s.teclas(FIN, ENTER)
    s.escribir('printf("Copia lista')
    s.teclas(FIN)
    s.escribir(";")
    s.pausa(1.5)
    s.paso("Ctrl+D duplica la línea y Alt+↑ la sube")
    s.teclas(CTRL("d"))
    s.pausa(1)
    s.teclas(ALT_FLECHA("arriba"))
    s.pausa(1.5)
    s.paso("Ctrl+/ comenta la línea; Ctrl+Z deshace los cambios")
    s.teclas(CTRL("/"))
    s.pausa(1.5)
    s.teclas(CTRL("z"), CTRL("z"), CTRL("z"), CTRL("z"), pausa=0.6)
    s.pausa(1)
    s.paso("Ctrl+F busca; Enter va a la siguiente coincidencia")
    s.teclas(CTRL("f"))
    s.escribir("copia")
    s.teclas(ENTER, ENTER, ENTER, pausa=0.7)
    s.teclas(ESC)
    s.paso("Ctrl+S guarda")
    s.teclas(CTRL("s"))
    s.pausa(2)
    s.salir_jonesy()


@caso("CU-05", "quick-doc", "Consultar una función de la biblioteca estándar")
def cu05(s):
    """
El **quick-doc** muestra, mientras se escribe, la documentación de la función bajo el
cursor: firma, qué devuelve, cada parámetro (resaltando el que se está escribiendo), las
trampas típicas y las reglas de cátedra relacionadas. Son unas 80 funciones de libc y
POSIX embebidas en jonesy, sin red. **F4** lo muestra u oculta y **Shift+F4** lo pasa de
la derecha a abajo.
"""
    s.jonesy(EJ1)
    s.esperar("main.c")
    s.paso("Con el cursor sobre scanf, el quick-doc muestra su documentación")
    ir_a_linea(s, 18)
    s.teclas(INICIO)
    buscar(s, "scanf")
    s.esperar("scanf")
    s.pausa(3.5)
    s.paso("Al escribir una llamada, resalta el parámetro que se está escribiendo")
    ir_a_linea(s, 37)
    s.teclas(FIN, ENTER)
    s.escribir("fgets(linea, ")
    s.pausa(3.5)
    s.paso("Shift+F4 lo pasa abajo, donde entra más texto")
    s.teclas(F(4, shift=True))
    s.pausa(3)
    s.teclas(F(4, shift=True))
    s.teclas(CTRL("z"), CTRL("z"), CTRL("z"), pausa=0.4)
    s.paso("F4 lo oculta y lo vuelve a mostrar")
    s.teclas(F(4))
    s.pausa(1.5)
    s.teclas(F(4))
    s.pausa(1.5)
    s.teclas(CTRL("s"))
    s.salir_jonesy()


@caso("CU-06", "moverse-por-el-codigo", "Moverse por el código y el proyecto")
def cu06(s):
    """
**F12** (o Ctrl+clic) va a la definición de la función bajo el cursor, aunque esté en
otro archivo, y **Alt+←** vuelve. **Alt+S** muestra el esquema del archivo (funciones,
tipos, macros) para saltar a cualquiera. **Alt+F** (o Ctrl+Shift+F) busca en todo el
proyecto y deja los resultados en un panel. **Ctrl+F2** pone marcadores y **Shift+F2**
salta entre ellos.
"""
    s.jonesy(EJ1)
    s.esperar("main.c")
    s.paso("F12 sobre promedio(…) va a su definición, en otro archivo")
    ir_a_linea(s, 37)
    buscar(s, "promedio(")
    s.teclas(IZQUIERDA)
    s.teclas(F(12))
    s.esperar("promedio.c")
    s.pausa(2)
    s.paso("Alt+← vuelve a donde estaba")
    s.teclas(ALT_FLECHA("izquierda"))
    s.pausa(2)
    s.paso("Alt+S: el esquema del archivo para saltar a una función o macro")
    s.teclas(ALT("s"))
    s.pausa(2.5)
    s.teclas(ESC)
    s.paso("Buscar en el proyecto (Ctrl+Shift+F): los resultados quedan en un panel")
    paleta(s, "Buscar en el proyecto")
    s.escribir("vector_suma")
    s.teclas(ENTER)
    s.esperar("Búsqueda")
    s.pausa(2.5)
    s.teclas(ABAJO, ABAJO, ENTER)
    s.pausa(2)
    s.paso("Ctrl+F2 pone un marcador; Shift+F2 vuelve a él")
    s.teclas(F(2, ctrl=True))
    s.pausa(1)
    s.teclas(CTRL_AVPAG)
    s.pausa(1)
    s.teclas(F(2, shift=True))
    s.pausa(2)
    s.salir_jonesy()


@caso("CU-07", "renombrar", "Renombrar una variable o función")
def cu07(s):
    """
**Ctrl+R** (Editar → Renombrar en su ámbito) cambia el nombre de la variable o función
del cursor **sólo donde vale esa declaración**: una variable local que tapa a otra, un
parámetro o la variable de un `for` no arrastran a otra con el mismo nombre, y no se tocan
campos, cadenas ni comentarios. La ventana dice cuántas apariciones cambia y en qué
ámbito, rechaza nombres inválidos o ya usados, y todo se deshace con un solo Ctrl+Z.
"""
    s.jonesy(EJ1)
    s.esperar("main.c")
    s.paso("Con el cursor sobre la variable leidos, Ctrl+R")
    buscar(s, "leidos")
    s.teclas(CTRL("r"))
    s.esperar("Renombrar «leidos»")
    s.pausa(2.5)
    s.paso("Un nombre reservado se rechaza al escribirlo")
    s.teclas(*([BORRAR] * 6), pausa=0.1)
    s.escribir("while")
    s.esperar("palabra reservada")
    s.pausa(2)
    s.paso("Con un nombre válido, Enter cambia todas las apariciones de su ámbito")
    s.teclas(*([BORRAR] * 5), pausa=0.1)
    s.escribir("lecturas")
    s.teclas(ENTER)
    s.esperar("lecturas")
    s.pausa(3)
    s.paso("Un solo Ctrl+Z lo deshace")
    s.teclas(CTRL("z"))
    s.pausa(2)
    s.salir_jonesy()


@caso("CU-09", "estilo-de-la-catedra", "Revisar el estilo con las reglas de la cátedra")
def cu09(s):
    """
Al guardar, las herramientas de la cátedra (ripley, gaff, kaneda, spunkmeyer, si están
instaladas) revisan el archivo. Lo que encuentran aparece al final de la línea y en el
**panel de reglas** (Alt+R), agrupado por regla. **F1** sobre la línea explica la regla
con el mismo formato que los errores de gcc: qué es, qué hacer, «así no» y «así sí».
**Ayuda → Reglas de la cátedra** es el catálogo de todas las reglas con sus ejemplos.
"""
    s.jonesy(EJ2)
    s.esperar("main.c")
    s.paso("Se escribe una declaración doble, sin espacios, y se guarda")
    ir_a_linea(s, 8)
    s.teclas(FIN, ENTER)
    s.escribir("int extra=0,otro=1;")
    s.pausa(1)
    s.teclas(CTRL("s"))
    s.paso("Al guardar, las herramientas revisan el estilo y marcan las líneas")
    s.esperar("▲", "●", tiempo=60)
    s.pausa(3)
    s.paso("Alt+R: el panel de reglas, agrupadas")
    s.teclas(ALT("r"))
    s.pausa(3)
    s.teclas(ESC)
    s.paso("F1 sobre la línea explica la regla con «así no / así sí»")
    ir_a_linea(s, 9)
    s.teclas(F(1))
    s.esperar("Así", tiempo=20)
    s.pausa(4)
    s.teclas(ESC)
    s.paso("Ayuda → Reglas de la cátedra: el catálogo navegable")
    paleta(s, "Reglas de la cátedra")
    s.pausa(3)
    s.escribir("0x0001")
    s.pausa(1.5)
    s.teclas(ENTER)
    s.pausa(3)
    s.teclas(ESC)
    s.salir_jonesy()


@caso("CU-10", "formatear", "Formatear el código",
      preparar=lambda p: p.escribir(
          "ejercicios/ej2-intercambio/intercambio.c",
          '#include "intercambio.h"\n\nvoid intercambiar(int *a,int *b){int auxiliar=*a;'
          "*a=*b;*b=auxiliar;}\n"))
def cu10(s):
    """
**Alt+Shift+F** (Editar → Autoformatear) formatea el archivo con gaff o, si no está, con
`clang-format` y el estilo de la cátedra. El cambio se puede deshacer con Ctrl+Z. Las
demás acciones de las herramientas del ecosistema para el archivo (gaff fix, tetsuo,
motoko…) están en **Alt+H** y en el menú Plugins.
"""
    s.jonesy("ejercicios/ej2-intercambio/intercambio.c")
    s.esperar("intercambio.c")
    s.paso("Una función escrita toda en una línea")
    s.pausa(2.5)
    s.paso("Alt+Shift+F la formatea con el estilo de la cátedra")
    paleta(s, "Autoformatear")
    s.esperar("auxiliar = *a", tiempo=30)
    s.pausa(3)
    s.paso("Alt+H: las acciones de las herramientas para este archivo")
    s.teclas(ALT("h"))
    s.pausa(3)
    s.teclas(ESC)
    s.teclas(CTRL("s"))
    s.salir_jonesy()


@caso("CU-11", "ejecutar", "Ejecutar el programa")
def cu11(s):
    """
**Ctrl+F5** compila y corre el programa en la terminal de abajo, una terminal real donde
el programa lee del teclado. Al terminar, jonesy informa cómo terminó, cuánto tardó y
cuánta memoria usó; si el programa parece estar en un ciclo infinito avisa y Shift+F5 lo
corta.
"""
    s.jonesy(EJ1)
    s.esperar("main.c")
    s.paso("Ctrl+F5 compila y corre el programa en la terminal de abajo")
    s.teclas(F(5, ctrl=True))
    s.esperar("Cantidad de números", tiempo=60)
    s.pausa(1.5)
    s.paso("El programa lee lo que se tipea")
    s.escribir("3\n")
    s.escribir("4 5 6\n")
    s.esperar("Promedio: 5.00")
    s.pausa(1)
    s.paso("Al terminar: cómo terminó, el tiempo y la memoria")
    s.pausa(3)
    s.teclas(ESC, ESC)
    s.salir_jonesy()


@caso("CU-12", "entrada-y-argumentos", "Ejecutar con un archivo de entrada o con argumentos")
def cu12(s):
    """
**Construir → Ejecutar con entrada…** elige un archivo del ejercicio como entrada estándar
del programa (o el teclado), y **Argumentos del programa…** los argumentos de la línea de
comandos. Las dos cosas se recuerdan por ejercicio: Ctrl+F5 las usa hasta que se cambien,
así no hay que tipear la misma entrada en cada prueba.
"""
    s.jonesy(EJ1)
    s.esperar("main.c")
    s.paso("Construir → Ejecutar con entrada: se elige el archivo")
    paleta(s, "Ejecutar con entrada")
    s.pausa(2)
    s.escribir("entrada2")
    s.pausa(1)
    s.teclas(ENTER)
    s.pausa(1.5)
    s.paso("Ctrl+F5 corre el programa leyendo de ese archivo")
    s.teclas(F(5, ctrl=True))
    s.esperar("Promedio: 1.50", tiempo=60)
    s.pausa(3)
    s.teclas(ESC, ESC)
    s.paso("Argumentos del programa: se escriben una vez y quedan para el ejercicio")
    paleta(s, "Argumentos del programa")
    s.pausa(1)
    s.escribir("--detalle")
    s.pausa(1.5)
    s.teclas(ESC)
    s.salir_jonesy()


@caso("CU-13", "comparar-salidas", "Comparar contra las salidas esperadas",
      preparar=bug_promedio)
def cu13(s):
    """
Si el ejercicio tiene casos (`entrada1.txt` con su `salida1.txt`, …), **Construir →
Comparar con las salidas esperadas** corre el programa con cada entrada y compara lo que
imprime con lo esperado: dice qué casos pasan y, en los que no, dónde difiere.

En la grabación el promedio divide enteros: el primer caso pasa por casualidad (5.00) y el
segundo no (1.00 en lugar de 1.50). Se corrige y se vuelve a comparar.
"""
    s.jonesy(EJ1)
    s.esperar("main.c")
    s.paso("Comparar con las salidas esperadas: un caso pasa y el otro no")
    paleta(s, "Comparar con las salidas")
    s.esperar("distintas", tiempo=60)
    s.pausa(4)
    s.teclas(ESC)
    s.paso("El error está en promedio.c: F12 lleva a la definición")
    ir_a_linea(s, 37)
    s.teclas(CTRL("f"))
    s.escribir("promedio(")
    s.teclas(ENTER, ESC, IZQUIERDA, F(12))
    s.esperar("promedio.c")
    s.paso("Se convierte a double antes de dividir y se guarda")
    buscar(s, "vector_suma(v")
    s.teclas(IZQUIERDA)
    s.escribir("(double)")
    s.teclas(CTRL("s"))
    s.pausa(1.5)
    s.paso("Comparar otra vez: pasan los dos casos")
    paleta(s, "Comparar con las salidas")
    s.esperar("✓ entrada2", tiempo=60)
    s.pausa(4)
    s.teclas(ESC)
    s.salir_jonesy()


@caso("CU-14", "correr-las-pruebas", "Correr las pruebas", preparar=bug_promedio)
def cu14(s):
    """
**Ctrl+F6** corre las pruebas del ejercicio (`make test`). Cada prueba que falla queda en
el panel Problemas y en la línea de la aserción, con lo esperado y lo obtenido; una que se
corta (violación de segmento) queda en la definición de la prueba. **Shift+F6** corre sólo
la prueba donde está el cursor.
"""
    s.jonesy("ejercicios/ej1-promedio/prueba.c")
    s.esperar("prueba.c")
    s.paso("Ctrl+F6 corre las pruebas del ejercicio")
    s.teclas(F(6, ctrl=True))
    s.esperar("Problemas (1)", tiempo=60)
    s.pausa(3)
    s.paso("La que falla queda en su línea, con lo esperado y lo obtenido")
    s.teclas(ALT("n"))
    s.pausa(3)
    s.paso("F12 sobre promedio lleva al código; se corrige la división")
    s.teclas(CTRL("f"))
    s.escribir("promedio(")
    s.teclas(ENTER, ESC, IZQUIERDA, F(12))
    s.esperar("promedio.c")
    buscar(s, "vector_suma(v")
    s.teclas(IZQUIERDA)
    s.escribir("(double)")
    s.teclas(CTRL("s"))
    s.paso("Ctrl+F6 otra vez: pasan todas")
    s.teclas(F(6, ctrl=True))
    s.esperar("pasan las 2", "Pruebas: pasan", tiempo=60)
    s.pausa(3)
    s.paso("Shift+F6 corre sólo la prueba donde está el cursor")
    s.teclas(CTRL_AVPAG)
    s.esperar("prueba.c")
    ir_a_linea(s, 12)
    s.teclas(F(6, shift=True))
    s.esperar("promedio_con_decimales", tiempo=60)
    s.pausa(3)
    s.salir_jonesy()


@caso("CU-15", "objetivo-de-make", "Correr un objetivo de make")
def cu15(s):
    """
**Shift+F9** (Construir → Correr un objetivo de make…) lista los objetivos de todos los
Makefile del proyecto con su descripción: primero los del ejercicio del archivo abierto,
después los del Makefile general, las bibliotecas y los demás ejercicios. Se filtra
escribiendo y Enter lo corre; la salida va al panel de abajo. También están en los
comandos (F1 o Ctrl+K, con el prefijo «make:») y en la vista del proyecto (Alt+Y).
"""
    s.jonesy(EJ1)
    s.esperar("main.c")
    s.paso("Shift+F9: los objetivos de make, los del ejercicio actual primero")
    s.teclas(F(9, shift=True))
    s.esperar("Objetivos de make")
    s.pausa(3.5)
    s.paso("Se filtra escribiendo: el test del Makefile general")
    s.escribir("general test")
    s.pausa(1.5)
    s.teclas(ENTER)
    s.esperar("Tests aprobados", tiempo=90)
    s.pausa(3)
    s.paso("También desde los comandos (Ctrl+K), con el prefijo make:")
    s.teclas(CTRL("k"))
    s.escribir("make clean")
    s.pausa(2.5)
    s.teclas(ENTER)
    s.pausa(2.5)
    s.salir_jonesy()


@caso("CU-16", "el-programa-se-corto", "Entender por qué el programa se cortó",
      preparar=lambda p: p.reemplazar(EJ2, "int *destino = &segundo;", "int *destino = NULL;"))
def cu16(s):
    """
Si el programa termina por una señal (violación de segmento, abort, división por cero…),
la terminal y la barra de estado explican en español qué pasó y aparece el botón
**[F5 Ver dónde falló]**: corre el depurador hasta la línea del programa donde se produjo,
saliendo de las bibliotecas del sistema, para ver el estado en ese momento.
"""
    s.jonesy(EJ2)
    s.esperar("main.c")
    s.paso("Ctrl+F5: el programa se corta por un puntero NULL")
    s.teclas(F(5, ctrl=True))
    s.esperar("Ver dónde falló", tiempo=60)
    s.pausa(4)
    s.paso("F5 (Ver dónde falló) lleva a la línea donde se cortó, con el depurador")
    s.teclas(F(5))
    s.esperar("intercambio.c", tiempo=40)
    s.pausa(4)
    s.paso("Shift+F5 termina la depuración")
    s.teclas(F(5, shift=True))
    s.pausa(2)
    s.salir_jonesy()


@caso("CU-17", "depurar-paso-a-paso", "Depurar paso a paso")
def cu17(s):
    """
**F5** depura sobre GDB: se detiene en los breakpoints (clic en el margen o **Ctrl+B**) y
muestra la pila y las variables. **F10** avanza una línea, **F11** entra en la función,
**Shift+F11** sale, **Ctrl+F10** corre hasta el cursor y **F5** continúa. La consola de
gdb queda a mano para lo avanzado.
"""
    s.jonesy(EJ3)
    s.esperar("main.c")
    s.paso("Ctrl+B pone un breakpoint en la llamada a copiar")
    ir_a_linea(s, 9)
    s.teclas(CTRL("b"))
    s.pausa(1.5)
    s.paso("F5 compila y corre hasta el breakpoint")
    s.teclas(F(5))
    s.esperar("original", tiempo=60)
    s.pausa(3)
    s.paso("F11 entra en la función copiar")
    s.teclas(F(11))
    s.esperar("copia.c", tiempo=20)
    s.pausa(2)
    s.paso("F10 avanza de a una línea; las variables se actualizan")
    for _ in range(4):
        s.teclas(F(10), pausa=1.2)
    s.pausa(1.5)
    s.paso("Shift+F11 sale de la función y vuelve a main")
    s.teclas(F(11, shift=True))
    s.pausa(2.5)
    s.teclas(F(10), pausa=1.5)
    s.paso("F5 continúa hasta el final")
    s.teclas(F(5))
    s.pausa(3)
    s.salir_jonesy()


@caso("CU-18", "valgrind", "Revisar la memoria con valgrind",
      preparar=lambda p: p.reemplazar(EJ3, "    free(copia);\n", ""))
def cu18(s):
    """
**Depurar → Revisar memoria con valgrind** corre `make memcheck` (o el programa) bajo
valgrind y deja cada problema en la línea del alumno, explicado: accesos inválidos, uso
después de `free`, `free` doble y fugas, con dónde se pidió o liberó el bloque.

En la grabación falta el `free` de la copia: valgrind informa la fuga y jonesy la ubica
en el `malloc` de `copiar`.
"""
    s.jonesy(EJ3)
    s.esperar("main.c")
    s.paso("Depurar → Revisar memoria con valgrind")
    paleta(s, "Revisar memoria con valgrind")
    s.esperar("Problemas (", tiempo=90)
    s.pausa(4)
    s.paso("La fuga queda explicada en la línea donde se pidió la memoria")
    s.teclas(ALT("n"))
    s.pausa(4)
    s.paso("Se agrega el free que faltaba y se revisa otra vez")
    s.teclas(CTRL_AVPAG)
    s.esperar("main.c")
    ir_a_linea(s, 16)
    s.teclas(FIN, ENTER)
    s.escribir("free(copia);")
    s.teclas(CTRL("s"))
    paleta(s, "Revisar memoria con valgrind")
    s.esperar("Sin problemas", "sin errores", "Sin errores", tiempo=90)
    s.pausa(3)
    s.salir_jonesy()


MAIN_CON_MALLOC = """#include <stdio.h>
#include <stdlib.h>

int main(void)
{
    int original[] = {10, 20, 30};
    int *copia = malloc(3 * sizeof(int));
    if (copia == NULL)
    {
        return 1;
    }
    for (int i = 0; i < 3; i++)
    {
        copia[i] = original[i];
    }
    copia[0] = 99;
    printf("Original: %d · copia: %d\\n", original[0], copia[0]);
    free(copia);
    copia = NULL;
    return 0;
}
"""


@caso("CU-19", "cat-scan", "Ver la memoria dibujada (cat-scan)",
      preparar=lambda p: p.escribir(EJ3, MAIN_CON_MALLOC),
      notas="""
mem-trace instrumenta sólo el archivo con `main`: la memoria que se pide en otros `.c`
del ejercicio (por ejemplo dentro de `copiar()` en `copia.c`) no aparece en el dibujo y
el puntero se marca como inválido. Por eso la grabación usa una variante del ejercicio 3
que pide la memoria en `main.c`.
""")
def cu19(s):
    """
Durante la depuración, **Alt+M** abre **cat-scan**: con la traza de `mem-trace`, dibuja el
stack y el heap con flechas desde cada puntero a lo que apunta, marca en rojo los punteros
colgantes y muestra los eventos del paso (pedidos, liberaciones, accesos inválidos). Se
puede exportar a una página HTML.
"""
    s.jonesy(EJ3)
    s.esperar("main.c")
    s.paso("Breakpoint después de copiar el vector y F5")
    ir_a_linea(s, 16)
    s.teclas(CTRL("b"))
    s.teclas(F(5))
    s.esperar("original", tiempo=60)
    s.pausa(2)
    s.paso("Alt+M: el stack y el heap dibujados, con flechas de los punteros")
    s.teclas(ALT("m"))
    s.esperar("Memoria (cat-scan)")
    s.esperar_que_no("Esperando la traza", tiempo=90)
    s.pausa(4)
    s.paso("F10 avanza: tras el free, el puntero queda colgante (en rojo)")
    s.teclas(F(10), pausa=2)
    s.teclas(F(10), pausa=2)
    s.teclas(F(10), pausa=2)
    s.esperar("liberado", tiempo=20)
    s.pausa(3)
    s.paso("Al asignarle NULL deja de apuntar al bloque liberado")
    s.teclas(F(10), pausa=3)
    s.teclas(F(5, shift=True))
    s.salir_jonesy()


def con_remoto(p):
    p.git_inicial()
    remoto = os.path.join(p.trabajo, "remoto.git")
    p.correr("git", "init", "-q", "--bare", remoto)
    p.correr("git", "remote", "add", "origin", remoto)
    p.correr("git", "push", "-q", "-u", "origin", "main")


@caso("CU-20", "git", "Versionar con git", preparar=con_remoto)
def cu20(s):
    """
**Alt+G** abre el panel de git: los archivos cambiados, **Espacio** los agrega o saca del
próximo commit, **Enter** muestra el diff lado a lado, **c** hace el commit (con una
revisión del mensaje), **l** muestra el historial, **s** sube y **b** trae (con ayuda ante
conflictos) y **z** deshace el último commit. Usa la identidad y las credenciales de git
del alumno.
"""
    s.jonesy(EJ1)
    s.esperar("main.c")
    s.paso("Un cambio en el código, guardado")
    ir_a_linea(s, 7)
    s.teclas(FIN)
    s.escribir("  /* como máximo */")
    s.teclas(CTRL("s"))
    s.pausa(1)
    s.paso("Alt+G: el panel de git con el archivo cambiado")
    s.teclas(ALT("g"))
    s.esperar("main.c")
    s.pausa(2)
    s.paso("Enter muestra el diff lado a lado")
    s.teclas(ENTER)
    s.pausa(3.5)
    s.teclas(ESC)
    s.paso("Espacio lo agrega al commit; c escribe el mensaje y Ctrl+S confirma")
    s.teclas(" ")
    s.pausa(1)
    s.teclas("c")
    s.escribir("Aclara el máximo de números")
    s.pausa(1.5)
    s.teclas(CTRL("s"))
    s.pausa(2)
    s.paso("l muestra el historial con el commit nuevo")
    s.teclas("l")
    s.esperar("Aclara el máximo")
    s.pausa(2.5)
    s.teclas("l")
    s.paso("s sube el commit al repositorio remoto")
    s.teclas("s")
    s.pausa(4)
    s.teclas(ESC)
    s.salir_jonesy()


@caso("CU-21", "aprender-c", "Aprender C desde el IDE")
def cu21(s):
    """
El menú Ayuda reúne material para aprender: el **glosario** de C (puntero, heap,
segfault, fuga…), las **tablas de consulta** (tipos con su tamaño en esa máquina,
especificadores de printf y scanf, secuencias de escape, precedencia de operadores y
ASCII) y el catálogo de reglas de la cátedra. **Editar → Ver la precedencia de la
expresión** muestra la línea con sus paréntesis implícitos y avisa trampas comunes.
"""
    s.jonesy(EJ1)
    s.esperar("main.c")
    s.paso("Ayuda → Glosario de C: se busca un término")
    paleta(s, "Glosario")
    s.pausa(1.5)
    s.escribir("puntero")
    s.pausa(1)
    s.teclas(ENTER)
    s.pausa(4)
    s.teclas(ESC)
    s.paso("Ayuda → Tablas de consulta: tipos, printf/scanf, precedencia y ASCII")
    paleta(s, "Tablas de consulta")
    s.esperar("Tablas de consulta")
    s.pausa(3)
    s.teclas(AVPAG, pausa=2)
    s.teclas(AVPAG, pausa=2)
    s.teclas(ESC)
    s.paso("Ver la precedencia de la condición: sus paréntesis implícitos")
    ir_a_linea(s, 19)
    paleta(s, "Ver la precedencia")
    s.esperar("Precedencia")
    s.pausa(4.5)
    s.teclas(ESC)
    s.salir_jonesy()


@caso("CU-22", "informe-markdown", "Escribir el informe en Markdown")
def cu22(s):
    """
Los `.md` se editan con resaltado y **Ctrl+P** abre la vista previa dibujada al lado:
títulos, tablas, listas, código y enlaces (que se recorren con Tab). La vista se
actualiza mientras se escribe.
"""
    s.jonesy("informe.md")
    s.esperar("informe.md")
    s.paso("El informe en Markdown, con resaltado")
    s.pausa(2)
    s.paso("Ctrl+P: la vista previa dibujada al lado")
    s.teclas(CTRL("p"))
    s.pausa(3)
    s.paso("La vista se actualiza mientras se escribe")
    s.teclas("\x1b[1;5F")  # Ctrl+Fin
    s.escribir("\n## Ejercicio 2: intercambio\n\n- Se pasan **punteros** a `intercambiar`.\n")
    s.pausa(3)
    s.teclas(CTRL("s"))
    s.salir_jonesy()


@caso("CU-23", "terminal-integrada", "Usar la terminal integrada")
def cu23(s):
    """
**Alt+T** abre una terminal real dentro de jonesy, en la carpeta del proyecto (con las
variables y el `PATH` del entorno de la cátedra, si está instalado), para lo que no tiene
un botón. **Esc Esc** vuelve al editor y **Alt+Z** la maximiza.
"""
    s.jonesy(EJ1)
    s.esperar("main.c")
    s.paso("Alt+T abre la terminal en la carpeta del proyecto")
    s.teclas(ALT("t"))
    s.esperar("ana@lab", tiempo=15)
    s.pausa(1)
    s.paso("Cualquier comando: compilar la biblioteca y listar lo que generó")
    s.escribir("make -C libs/vector && ls libs/vector\n")
    s.esperar("libvector.a", tiempo=40)
    s.pausa(2)
    s.paso("Alt+Z la maximiza y la vuelve a su tamaño")
    s.teclas(ALT("z"))
    s.pausa(2)
    s.teclas(ALT("z"))
    s.pausa(1)
    s.paso("Esc Esc vuelve al editor")
    s.teclas(ESC, ESC, pausa=0.2)
    s.pausa(1.5)
    s.salir_jonesy()


@caso("CU-24", "sin-mouse", "Trabajar sin mouse")
def cu24(s):
    """
Todo tiene una tecla. **F10** y la letra (o **Alt** y la letra subrayada) abren los menús
y las flechas los recorren; **Ctrl+K** (o F1 donde no hay un error que explicar) busca
cualquier acción, objetivo de make o herramienta por su nombre. En cada panel, **?**
muestra sus atajos, y **Alt+Z** maximiza el panel con el foco.
"""
    s.jonesy(EJ1)
    s.esperar("main.c")
    s.paso("F10 abre los menús; las flechas los recorren")
    s.teclas(F(10))
    s.pausa(1.5)
    s.teclas(DERECHA, DERECHA, DERECHA, pausa=1.2)
    s.teclas(ABAJO, ABAJO, pausa=0.8)
    s.pausa(1)
    s.teclas(ESC)
    s.paso("Ctrl+K busca cualquier acción por su nombre")
    s.teclas(CTRL("k"))
    s.escribir("depurar")
    s.pausa(2.5)
    s.teclas(ESC)
    s.paso("Alt+E pone el foco en los archivos; ? muestra los atajos de ese panel")
    s.teclas(ALT("e"))
    s.pausa(1)
    s.teclas("?")
    s.esperar("Atajos: archivos")
    s.pausa(3)
    s.teclas(ESC)
    s.paso("Alt+Z maximiza el panel con el foco")
    s.teclas(ESC)
    s.teclas(ALT("z"))
    s.pausa(2)
    s.teclas(ALT("z"))
    s.pausa(1)
    s.salir_jonesy()


@caso("CU-25", "apariencia-y-preferencias", "Ajustar la apariencia y llevarla a otra máquina")
def cu25(s):
    """
**Archivo → Configuración del entorno** (Alt+,) reúne lo que configura el IDE. En
**Apariencia** se elige el tema (oscuro, claro, contraste, turbo al estilo Turbo C++,
solarizado, fósforo), la posición del quick-doc y los tamaños de los paneles. **Exportar
mis preferencias** las guarda en un `.toml` (con los manifiestos de integraciones
propios) para llevarlas a otra máquina, donde **Importar** (o `jonesy config importar`)
las aplica y las deja para los proyectos nuevos.
"""
    s.jonesy(EJ1)
    s.esperar("main.c")
    s.paso("Configuración del entorno, solapa Apariencia")
    paleta(s, "Configuración del entorno")
    s.pausa(1)
    s.teclas("4")
    s.esperar("Tema")
    s.pausa(2)
    s.paso("Enter sobre Tema cambia el tema: claro, contraste, turbo…")
    for _ in range(3):
        s.teclas(ENTER, pausa=1.8)
    s.pausa(1)
    s.teclas(ESC)
    s.pausa(2)
    s.paso("Exportar mis preferencias a un archivo")
    paleta(s, "Exportar mis preferencias")
    s.pausa(1.5)
    s.teclas(ENTER)
    s.esperar("Preferencias exportadas")
    s.pausa(2.5)
    s.salir_jonesy()
    s.shell()
    s.paso("El archivo exportado, para importarlo en otra máquina")
    s.comando("cat ~/jonesy-preferencias.toml", esperar="tema")
    s.pausa(2)
    s.comando("jonesy config importar ~/jonesy-preferencias.toml", esperar="Importado")
    s.pausa(2)


@caso("CU-26", "recuperar-tras-un-corte", "Recuperar el trabajo tras un corte")
def cu26(s):
    """
Si se cierra la terminal o se corta la conexión con cambios sin guardar, jonesy deja
copias de recuperación antes de salir. Al volver a abrir el proyecto ofrece restaurarlas
(**s**), descartarlas (**n**) o dejarlas para después (Esc).
"""
    s.jonesy(EJ3)
    s.esperar("main.c")
    s.paso("Cambios sin guardar…")
    ir_a_linea(s, 16)
    s.teclas(FIN, ENTER)
    s.escribir('printf("Listo.\\n");')
    s.pausa(1.5)
    s.paso("…y se corta la terminal (como al cerrar la ventana o perder la conexión)")
    s.cortar()
    s.pausa(1)
    s.jonesy(EJ3)
    s.paso("Al volver a abrir el proyecto, jonesy ofrece recuperarlos")
    s.esperar("Recuperar cambios")
    s.pausa(3)
    s.paso("s los restaura en el editor")
    s.teclas("s")
    s.esperar("Listo.")
    s.pausa(3)
    s.teclas(CTRL("s"))
    s.salir_jonesy()


@caso("CU-27", "simulacro", "Practicar con un simulacro de examen")
def cu27(s):
    """
**Archivo → Simulacro de examen…** (o `jonesy --simulacro [minutos]`) practica con el
cronómetro y los avisos del examen, pero sin restricciones ni entrega sellada. **F12** lo
termina y muestra cómo fue: compilaciones, cuántas con errores, ejecuciones y pruebas.
"""
    s.shell()
    s.paso("jonesy --simulacro 30: cronómetro y avisos como en el examen")
    s.comando("jonesy --simulacro 30 ejercicios/ej1-promedio/main.c", esperar="main.c")
    s.pausa(2.5)
    s.paso("Se trabaja como siempre: compilar y probar")
    s.teclas(F(6))
    s.pausa(4)
    s.teclas(F(6, ctrl=True))
    s.esperar("Tests aprobados", "pasan", tiempo=60)
    s.pausa(2)
    s.paso("F12 termina el simulacro y muestra cómo fue")
    s.teclas(F(12))
    s.pausa(1)
    s.teclas("s")
    s.esperar("Simulacro", tiempo=15)
    s.pausa(4)
    s.teclas(ESC)
    s.salir_jonesy()


@caso("CU-28", "rendir-un-examen", "Rendir un examen", actor="Alumno en examen")
def cu28(s):
    """
En modo examen (`jonesy --exam`) la consigna es de solo lectura, el cronómetro avisa los
minutos que quedan y todo se guarda solo cada 30 s. Una bitácora registra la sesión con
SHA-256 encadenado (lo pegado queda marcado). Se trabaja con las herramientas de siempre,
pero se niegan las acciones que no corresponden: abrir otro proyecto, clonar, renombrar
o borrar archivos, editar `.jonesy.toml` o aplicar correcciones automáticas.
"""
    s.shell()
    s.paso("jonesy --exam: consigna, cronómetro y bitácora")
    s.comando("jonesy --exam ejercicios/ej1-promedio/main.c", esperar="EXAMEN")
    s.pausa(3)
    s.paso("Se compila y se prueba como siempre")
    s.teclas(F(6))
    s.pausa(4)
    s.paso("Lo que no corresponde en un examen se niega con un aviso")
    paleta(s, "Abrir proyecto")
    s.pausa(3)
    s.paso("Examen → Verificar estado de entrega: tiempo, autoguardado y bitácora")
    paleta(s, "Verificar estado de entrega")
    s.esperar("Estado del examen")
    s.pausa(4)
    s.teclas(ESC)
    s.salir_jonesy()


@caso("CU-29", "entregar", "Revisar y confirmar la entrega", actor="Alumno en examen")
def cu29(s):
    """
**F12** (o el botón `[ Entregar Examen ]`) abre la entrega. Antes de confirmar, la ventana
lista los archivos que van en el paquete (y avisa si un `.c` abierto queda afuera) y cómo
terminaron la última compilación y las últimas pruebas. Al confirmar, jonesy guarda todo,
corre la verificación (`[examen] verificar`, por ejemplo `make test` o `ripley run`),
sella la bitácora y arma el paquete `.tar` con su SHA-256, que identifica la entrega.
"""
    s.jonesy("--exam", EJ1)
    s.esperar("EXAMEN")
    s.paso("Antes de entregar, se corren las pruebas")
    s.teclas(F(6, ctrl=True))
    s.esperar("Tests aprobados", "pasan", tiempo=60)
    s.pausa(2)
    s.paso("F12: qué archivos van en el paquete y cómo están la compilación y las pruebas")
    s.teclas(F(12))
    s.esperar("Van en el paquete")
    s.pausa(5)
    s.paso("s confirma: verificación, bitácora sellada y paquete con su SHA-256")
    s.teclas("s")
    s.esperar("Entrega registrada", tiempo=90)
    s.pausa(5)
    s.teclas(ESC)
    s.salir_jonesy()


@caso("CU-30", "pausa-del-examen", "Pausar el cronómetro del examen",
      actor="Docente (en la máquina del alumno)")
def cu30(s):
    """
Ante un imprevisto (un corte de luz, una consulta larga), el docente elige **Examen →
Pausar** y escribe su código en la máquina del alumno; se ve con asteriscos y se compara
con el SHA-256 de `[examen] pausa_sha256`. En pausa el cronómetro no corre, la barra dice
EN PAUSA y no se puede editar; la pausa sobrevive a un reinicio. Reanudar no pide código.
La bitácora registra cada pausa, cada reanudación y cada código rechazado.
"""
    s.jonesy("--exam", EJ1)
    s.esperar("EXAMEN")
    s.paso("Examen → Pausar: el docente escribe su código (con asteriscos)")
    paleta(s, "Pausar")
    s.esperar("Código del docente")
    s.escribir("pausa-demo-larga")
    s.pausa(1.5)
    s.teclas(ENTER)
    s.paso("En pausa: el cronómetro se detiene y no se puede editar")
    s.esperar("EN PAUSA")
    s.pausa(2)
    s.escribir("x")
    s.pausa(3)
    s.paso("Examen → Reanudar: el cronómetro sigue (no pide código)")
    paleta(s, "Pausar")
    s.pausa(3)
    s.salir_jonesy()


# ======================================================================= docente


@caso("CU-31", "configurar-el-proyecto", "Configurar un proyecto o un examen", actor="Docente")
def cu31(s):
    """
Cada proyecto puede tener un `.jonesy.toml` (ver `docs/ejemplo.jonesy.toml`): el comando
de compilación, el programa y sus argumentos para depurar, las descripciones de los
objetivos de make, las integraciones activas, las pruebas en SARIF y, para el examen, la
consigna, la duración, la verificación, los archivos de la entrega, los plugins
autorizados y el SHA-256 del código de pausa (largo y al azar: el alumno puede leer el
archivo). **Archivo → Configuración del entorno** muestra y abre todo eso.
"""
    s.shell()
    s.paso("El .jonesy.toml del TP: duración, verificación, archivos y código de pausa")
    s.comando("cat .jonesy.toml", esperar="pausa_sha256")
    s.pausa(2)
    s.paso("El hash del código de pausa se calcula así (el código no va en el archivo)")
    s.comando("printf '%s' 'pausa-demo-larga' | sha256sum", esperar="0ccf61")
    s.pausa(1.5)
    s.paso("En jonesy, Configuración del entorno muestra lo que está configurado")
    s.comando("jonesy", esperar="jonesy · tp-demo")
    paleta(s, "Configuración del entorno")
    s.esperar("Integraciones")
    s.pausa(3)
    s.teclas("3", pausa=3)
    s.teclas("2", pausa=2.5)
    s.teclas(ESC)
    s.salir_jonesy()


@caso("CU-32", "verificar-una-entrega", "Verificar una entrega", actor="Docente",
      preparar=desde_cu29)
def cu32(s):
    """
`jonesy verificar <paquete.tar>` comprueba una entrega: el SHA-256 del paquete, cada
archivo contra el manifiesto, la cadena de la bitácora, su sello y que la sesión esté
cerrada. Termina con código 0 si es íntegra y 1 si no; con `--json` da lo mismo para las
herramientas del perfil docente.
"""
    s.shell()
    s.paso("El paquete que dejó la entrega (CU-29)")
    s.comando("ls .jonesy/entrega/", esperar=".tar")
    s.paso("jonesy verificar comprueba el paquete, los archivos y la bitácora")
    s.comando("jonesy verificar .jonesy/entrega/entrega-*.tar", esperar="íntegra")
    s.pausa(2.5)
    s.paso("Si alguien cambia el paquete, la verificación lo detecta")
    s.comando("cp .jonesy/entrega/entrega-*.tar copia.tar && echo x >> copia.tar")
    s.comando("jonesy verificar copia.tar; echo \"código: $?\"", esperar="código:")
    s.pausa(3)


@caso("CU-33", "auditar-la-sesion", "Auditar la sesión de un examen", actor="Docente",
      preparar=desde_cu29)
def cu33(s):
    """
`jonesy replay <telemetry.log>` reproduce la sesión del examen tecla a tecla (Espacio
pausa, → avanza, + acelera, q sale). Con `--resumen` (o `--json`) da la auditoría:
integridad de la cadena, duración, cuántas veces y cuánto se pegó, y la velocidad de
tipeo por minuto.
"""
    s.shell()
    s.paso("El resumen de auditoría de la bitácora del examen")
    s.comando("jonesy replay .jonesy/telemetry.log --resumen", esperar="egistros")
    s.pausa(3)
    s.paso("La reproducción de la sesión, tecla a tecla")
    s.escribir("jonesy replay .jonesy/telemetry.log\n")
    s.pausa(3)
    s.teclas("+", "+", "+", pausa=0.5)
    s.pausa(5)
    s.teclas("q")
    s.pausa(1.5)


@caso("CU-34", "plugins", "Extender jonesy con plugins y herramientas", actor="Docente")
def cu34(s):
    """
Los plugins son scripts de Python en `.jonesy/plugins/`: cada función `comando_*` aparece
en el menú **Plugins** y en los comandos, recibe el archivo y la selección y devuelve un
texto o diagnósticos; sus errores aparecen en Problemas en su línea. **Nuevo plugin
Python** parte de una plantilla y al guardarlo se recarga. Las herramientas externas se
suman con manifiestos de integraciones (ver `docs/integraciones.md`).
"""
    s.jonesy(EJ1)
    s.esperar("main.c")
    s.paso("Nuevo plugin Python: parte de una plantilla")
    paleta(s, "Nuevo plugin Python")
    s.pausa(1)
    s.teclas(*([BORRAR] * len("mi_plugin.py")), pausa=0.05)
    s.escribir("contar.py")
    s.teclas(ENTER)
    s.esperar(".py", tiempo=15)
    s.pausa(3)
    s.paso("Al guardarlo se recarga; Alt+L muestra sus comandos en el menú Plugins")
    s.teclas(CTRL("s"))
    s.pausa(2)
    s.teclas(CTRL_AVPAG)
    s.teclas(ALT("l"))
    s.esperar_que_no("(ninguno en .jonesy/plugins)", tiempo=15)
    s.pausa(3)
    s.paso("Enter corre el comando sobre el archivo abierto")
    s.teclas(ENTER)
    s.pausa(4)
    s.teclas(ESC)
    s.salir_jonesy()


# ======================================================================= entorno


@caso("CU-35", "doctor", "Revisar la salud del entorno", actor="Alumno, Administrador")
def cu35(s):
    """
`jonesy doctor` (o Ayuda → Diagnóstico del entorno) revisa lo que jonesy necesita:
compilar (gcc, make), depurar (gdb, ptrace), memoria (valgrind, mem-trace), git, la
terminal, Python, el proyecto y las herramientas del ecosistema. Sin `--rapido` además
compila, depura y revisa un programa de prueba. Cada falta viene con para qué sirve y
cómo instalarla; termina con código 1 si falta algo requerido.
"""
    s.shell()
    s.paso("jonesy doctor --rapido: qué hay y qué falta en el entorno")
    s.comando("jonesy doctor --rapido", esperar="ana@lab:tp-demo$ ", tiempo=60)
    s.pausa(4)
    s.paso("jonesy doctor: además compila, depura y revisa un programa de prueba")
    s.comando("clear; jonesy doctor | tail -20", esperar="ana@lab", tiempo=120)
    s.pausa(4)


@caso("CU-36", "reportar-un-problema", "Reportar un problema de jonesy",
      actor="Alumno, Administrador")
def cu36(s):
    """
`jonesy reporte` (o Ayuda → Reportar un problema) arma un informe para pegar en el aviso
del problema: versión, sistema, terminal, un diagnóstico rápido, el `.jonesy.toml`, el
último error de la sesión y el último pánico. No incluye código, el directorio personal
aparece como `~` y se ocultan la identidad de git y el hash de la pausa del examen.
"""
    s.shell()
    s.paso("jonesy reporte --salida guarda el informe en un archivo")
    s.comando("jonesy reporte --salida reporte.md", esperar="Reporte guardado", tiempo=60)
    s.paso("Versión, sistema, terminal, entorno y configuración (sin el hash de la pausa)")
    s.comando("head -32 reporte.md", esperar="Reporte de jonesy")
    s.pausa(4)
    s.comando("grep -n -A3 'pausa_sha256' reporte.md", esperar="oculto")
    s.pausa(3)


@caso("CU-37", "muchas-sesiones", "Alojar muchas sesiones en un servidor",
      actor="Administrador")
def cu37(s):
    """
jonesy es un único binario: arranca en milisegundos, consume poco en reposo (no redibuja
si nada cambió) y guarda el estado de cada sesión en el directorio del proyecto, así que
decenas de alumnos pueden usarlo a la vez en el mismo servidor. `bin/estres-sesiones`
levanta N sesiones reales y mide la memoria y la CPU en reposo contra el presupuesto.
"""
    raiz = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(
        os.path.abspath(__file__)))))
    s.shell()
    s.paso("El arranque, medido: proyecto leído y primer cuadro dibujado")
    s.comando("jonesy --headless --benchmark-startup .", esperar="arranque", tiempo=30)
    s.pausa(2)
    s.paso("Diez sesiones a la vez en reposo: memoria y CPU contra el presupuesto")
    s.comando(f"{raiz}/bin/estres-sesiones --sesiones 10 --calentamiento 3 --segundos 5",
              esperar="ana@lab", tiempo=120)
    s.pausa(4)


@caso("CU-38", "visor-hexadecimal", "Mirar por dentro un ejecutable, un .o o un .a")
def cu38(s):
    """
Los binarios (ejecutables, objetos `.o`, bibliotecas `.a`) se abren en una pestaña con el
**visor hexadecimal**: offset, bytes y ASCII, con el byte del cursor en hexadecimal,
decimal y carácter, y los 4 bytes desde ahí leídos como `int`. `g` (o Ctrl+G) va a un
offset y `/` (o Ctrl+F) busca texto o bytes. `i` describe el archivo (tipo, arquitectura,
las secciones del ELF explicadas o los objetos de un `.a`) y `s` muestra los símbolos con
`nm`: las funciones que define (T) y las que usa de otro lado (U), lo que explica un
«undefined reference». El archivo nunca se modifica.
"""
    s.jonesy(EJ1)
    s.esperar("main.c")
    s.paso("F6 compila: quedan el ejecutable, los .o y la biblioteca .a")
    s.teclas(F(6))
    s.esperar("Sin problemas", "compilación correcta", tiempo=60)
    s.pausa(1.5)
    s.paso("Ver → Visor hexadecimal lista los binarios del proyecto")
    paleta(s, "Visor hexadecimal")
    s.pausa(2.5)
    s.escribir("promedio —")
    s.teclas(ENTER)
    s.esperar("7f 45 4c 46")
    s.pausa(3)
    s.paso("/ busca un texto dentro del ejecutable")
    s.teclas("/")
    s.escribir("Promedio")
    s.teclas(ENTER)
    s.pausa(3)
    s.paso("i describe el archivo: tipo y secciones del ELF explicadas")
    s.teclas("i")
    s.esperar(".text")
    s.pausa(4)
    s.teclas(ESC)
    s.paso("s muestra los símbolos: lo que define (T) y lo que usa de otro lado (U)")
    s.teclas("s")
    s.esperar("Símbolos")
    s.pausa(4)
    s.teclas(ESC)
    s.paso("La biblioteca libvector.a: un paquete de objetos .o")
    paleta(s, "Visor hexadecimal")
    s.escribir("libvector")
    s.teclas(ENTER)
    s.teclas("i")
    s.esperar("vector.o")
    s.pausa(4)
    s.teclas(ESC)
    s.paso("Ctrl+W cierra la pestaña")
    s.teclas(CTRL("w"), CTRL("w"))
    s.pausa(1.5)
    s.salir_jonesy()
