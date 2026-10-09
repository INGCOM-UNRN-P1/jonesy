//! Por qué se cortó un programa: las señales que más ven los alumnos de Programación I,
//! explicadas en español (RNF-C.1).
//!
//! Se usan al correr el programa (Ctrl+F5: el shell informa el código 128 + señal) y en el
//! depurador (GDB informa el nombre de la señal).

/// Un corte del programa por una señal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fallo {
    pub senal: &'static str,
    pub numero: i32,
    /// Una línea: qué pasó.
    pub titulo: &'static str,
    /// Causas típicas y cómo buscarlas (Markdown).
    pub explicacion: &'static str,
    /// Es un error del programa (y vale la pena ir a la línea con el depurador), no una
    /// interrupción pedida por el usuario.
    pub error: bool,
}

const FALLOS: &[Fallo] = &[
    Fallo {
        senal: "SIGSEGV",
        numero: 11,
        titulo: "Violación de segmento: el programa accedió a memoria que no le corresponde",
        explicacion: "Causas típicas:\n\n\
- un puntero **NULL**: ¿verificaste lo que devolvió `malloc`, `fopen` o una búsqueda?\n\
- un puntero **sin inicializar**;\n\
- un **índice fuera del arreglo** (de `0` a `n - 1`);\n\
- usar memoria **después de `free`**;\n\
- `scanf` **sin `&`**;\n\
- una **recursión sin caso base**, que agota la pila.\n",
        error: true,
    },
    Fallo {
        senal: "SIGABRT",
        numero: 6,
        titulo: "El programa abortó: casi siempre es un free doble o inválido, o memoria corrupta",
        explicacion: "Causas típicas:\n\n\
- **`free` dos veces** del mismo puntero (o de uno que no vino de `malloc`);\n\
- **escribir fuera** de un bloque de `malloc` (pisa los datos internos del heap y \
`free` lo detecta después);\n\
- un **`assert`** que no se cumplió.\n\n\
cat-scan (en el depurador) muestra los bloques del heap y cuáles ya se liberaron.\n",
        error: true,
    },
    Fallo {
        senal: "SIGFPE",
        numero: 8,
        titulo: "Error aritmético: una división (o módulo) entera por cero",
        explicacion: "Una división entera por `0` (o `%` por `0`) no tiene resultado y \
corta el programa. Verificá el divisor antes de dividir.\n",
        error: true,
    },
    Fallo {
        senal: "SIGBUS",
        numero: 7,
        titulo: "Error de bus: acceso a una dirección de memoria inválida",
        explicacion: "Parecido a una violación de segmento: un puntero con una dirección \
que no es válida. Revisá punteros sin inicializar o mal convertidos.\n",
        error: true,
    },
    Fallo {
        senal: "SIGILL",
        numero: 4,
        titulo: "Instrucción inválida: el programa saltó a memoria que no es código",
        explicacion: "Suele ser memoria corrupta: escribir fuera de un arreglo local \
puede pisar la dirección de retorno de la función.\n",
        error: true,
    },
    Fallo {
        senal: "SIGKILL",
        numero: 9,
        titulo: "El sistema terminó el programa (¿se quedó sin memoria?)",
        explicacion: "Un programa que pide memoria sin parar (por ejemplo, `malloc` \
dentro de un ciclo que no termina) puede ser terminado por el sistema.\n",
        error: true,
    },
    Fallo {
        senal: "SIGPIPE",
        numero: 13,
        titulo: "Se escribió en una tubería que ya estaba cerrada",
        explicacion: "El programa escribía hacia otro proceso (`|`) que ya había \
terminado.\n",
        error: false,
    },
    Fallo {
        senal: "SIGINT",
        numero: 2,
        titulo: "Interrumpido con Ctrl+C",
        explicacion: "",
        error: false,
    },
    Fallo {
        senal: "SIGTERM",
        numero: 15,
        titulo: "Terminado desde afuera (SIGTERM)",
        explicacion: "",
        error: false,
    },
];

/// Por nombre de señal (`SIGSEGV`), como lo informa GDB.
pub fn de_senal(nombre: &str) -> Option<&'static Fallo> {
    FALLOS.iter().find(|f| f.senal == nombre)
}

/// Por código de salida de un shell POSIX: 128 + número de señal.
pub fn de_codigo_salida(codigo: i64) -> Option<&'static Fallo> {
    let n = codigo.checked_sub(128).filter(|n| *n > 0)?;
    FALLOS.iter().find(|f| i64::from(f.numero) == n)
}

fn comillas(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Líneas de `sh` que, después de correr un programa y guardar su código en `$c`, explican
/// en la terminal por qué se cortó.
pub fn explicacion_sh() -> String {
    let mut s = String::from("case \"$c\" in\n");
    for f in FALLOS {
        let pista = if f.error {
            "Apretá F5: el depurador se detiene en la línea donde falló."
        } else {
            ""
        };
        s.push_str(&format!(
            "  {}) printf '\\033[1;31m%s (%s)\\033[0m\\n%s\\n' {} {} {} ;;\n",
            128 + f.numero,
            comillas(f.titulo),
            f.senal,
            comillas(pista),
        ));
    }
    s.push_str("esac\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn por_nombre_y_por_codigo() {
        assert_eq!(de_senal("SIGSEGV").unwrap().numero, 11);
        assert_eq!(de_codigo_salida(139).unwrap().senal, "SIGSEGV");
        assert_eq!(de_codigo_salida(134).unwrap().senal, "SIGABRT");
        assert_eq!(de_codigo_salida(136).unwrap().senal, "SIGFPE");
        assert!(de_codigo_salida(0).is_none());
        assert!(de_codigo_salida(1).is_none());
        assert!(de_codigo_salida(128).is_none());
        assert!(
            !de_codigo_salida(130).unwrap().error,
            "Ctrl+C no es un error"
        );
    }

    #[test]
    #[cfg(unix)]
    fn el_shell_explica_el_corte() {
        let guion = format!("sh -c 'kill -SEGV $$'\nc=$?\n{}", explicacion_sh());
        let salida = std::process::Command::new("sh")
            .arg("-c")
            .arg(guion)
            .output()
            .unwrap();
        let texto = String::from_utf8_lossy(&salida.stdout);
        assert!(texto.contains("Violación de segmento"), "{texto}");
        assert!(
            texto.contains("(SIGSEGV)") && texto.contains("F5"),
            "{texto}"
        );
    }
}
