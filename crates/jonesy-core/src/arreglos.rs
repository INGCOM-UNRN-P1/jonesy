//! Arreglo rápido (Alt+Enter) de los errores más simples de gcc: falta un `;`, falta un
//! `#include` para una función de la biblioteca, o `=` donde se quería comparar con `==`.

use crate::errores_gcc;

/// Un reemplazo en el texto: [inicio, fin) en caracteres por `texto`.
#[derive(Debug, Clone, PartialEq)]
pub struct Arreglo {
    /// Qué hace, en español («Agregar «;» al final de la línea 12»).
    pub descripcion: String,
    pub inicio: usize,
    pub fin: usize,
    pub texto: String,
}

/// Posición (en caracteres) de la línea y columna 1-based de gcc.
fn posicion(lineas: &[&str], linea: usize, columna: usize) -> Option<usize> {
    let l = linea.checked_sub(1)?;
    if l >= lineas.len() {
        return None;
    }
    let antes: usize = lineas[..l].iter().map(|x| x.chars().count() + 1).sum();
    Some(antes + columna.saturating_sub(1).min(lineas[l].chars().count()))
}

/// El primer nombre entre comillas del mensaje (`'printf'`, `‘malloc’`).
fn nombre_entre_comillas(mensaje: &str) -> Option<String> {
    let m: String = mensaje
        .chars()
        .map(|c| if matches!(c, '‘' | '’') { '\'' } else { c })
        .collect();
    m.split('\'')
        .skip(1)
        .step_by(2)
        .map(str::trim)
        .find(|s| !s.is_empty())
        .map(str::to_string)
}

/// El arreglo de un diagnóstico de gcc en `fuente`, si es uno de los simples.
pub fn proponer(fuente: &str, mensaje: &str, linea: usize, columna: usize) -> Option<Arreglo> {
    let id = errores_gcc::explicar(mensaje)?.id;
    let lineas: Vec<&str> = fuente.lines().collect();
    let chars: Vec<char> = fuente.chars().collect();
    match id.as_str() {
        "falta-punto-y-coma" => {
            // gcc marca lo que sigue: el `;` va al final de lo anterior.
            let pos = posicion(&lineas, linea, columna)?;
            let mut i = pos;
            while i > 0 && chars[i - 1].is_whitespace() {
                i -= 1;
            }
            if i == 0 || chars[i - 1] == ';' {
                return None;
            }
            let n = chars[..i].iter().filter(|c| **c == '\n').count() + 1;
            Some(Arreglo {
                descripcion: format!("Agregar «;» al final de la línea {n}"),
                inicio: i,
                fin: i,
                texto: ";".into(),
            })
        }
        "declaracion-implicita" | "builtin-incompatible" | "nota-include" => {
            let cabecera = if id == "nota-include" {
                mensaje
                    .split(['<', '>'])
                    .nth(1)
                    .map(str::to_string)
                    .filter(|h| h.ends_with(".h"))?
            } else {
                errores_gcc::cabecera_de(&nombre_entre_comillas(mensaje)?)?
            };
            let include = format!("#include <{cabecera}>");
            if lineas
                .iter()
                .any(|l| l.split_whitespace().collect::<String>() == include.replace(' ', ""))
            {
                return None;
            }
            // Después del último #include del principio (o arriba de todo).
            let ultimo = lineas
                .iter()
                .take(60)
                .rposition(|l| l.trim_start().starts_with("#include"));
            let pos = match ultimo {
                Some(i) => lineas[..=i].iter().map(|x| x.chars().count() + 1).sum(),
                None => 0,
            };
            Some(Arreglo {
                descripcion: format!("Agregar «{include}»"),
                inicio: pos,
                fin: pos,
                texto: format!("{include}\n"),
            })
        }
        "asignacion-en-condicion" => {
            let l = linea.checked_sub(1)?;
            let texto: Vec<char> = lineas.get(l)?.chars().collect();
            let base = posicion(&lineas, linea, 1)?;
            let desde = columna.saturating_sub(1).min(texto.len());
            // El primer `=` suelto desde la columna (no ==, <=, >=, !=, +=…).
            let k = (desde..texto.len()).find(|&k| {
                texto[k] == '='
                    && texto.get(k + 1) != Some(&'=')
                    && (k == 0 || !"=!<>+-*/%&|^".contains(texto[k - 1]))
            })?;
            Some(Arreglo {
                descripcion: format!("Cambiar «=» por «==» en la línea {linea}"),
                inicio: base + k,
                fin: base + k + 1,
                texto: "==".into(),
            })
        }
        _ => None,
    }
}

/// Aplica un arreglo al texto (para las pruebas y para mostrar cómo queda).
pub fn aplicar(fuente: &str, a: &Arreglo) -> String {
    let c: Vec<char> = fuente.chars().collect();
    let mut s: String = c[..a.inicio].iter().collect();
    s.push_str(&a.texto);
    s.extend(&c[a.fin..]);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falta_punto_y_coma() {
        let f = "int main(void)\n{\n    int x = 0\n\n    return x;\n}\n";
        // gcc: «expected ',' or ';' before 'return'» en 5:5.
        let a = proponer(f, "expected ',' or ';' before 'return'", 5, 5).unwrap();
        assert_eq!(a.descripcion, "Agregar «;» al final de la línea 3");
        assert!(aplicar(f, &a).contains("    int x = 0;\n"));
    }

    #[test]
    fn falta_include() {
        let f = "#include <stdlib.h>\n\nint main(void)\n{\n    printf(\"hola\");\n}\n";
        let a = proponer(
            f,
            "implicit declaration of function ‘printf’ [-Wimplicit-function-declaration]",
            5,
            5,
        )
        .unwrap();
        assert_eq!(a.descripcion, "Agregar «#include <stdio.h>»");
        assert!(aplicar(f, &a).starts_with("#include <stdlib.h>\n#include <stdio.h>\n\n"));
        // Si ya está, nada que hacer.
        let g = "#include <stdio.h>\nint main(void) { printf(\"x\"); }\n";
        assert!(proponer(g, "implicit declaration of function 'printf'", 2, 18).is_none());
        // La nota de gcc trae el header.
        let a = proponer(
            "int main(void) { return 0; }\n",
            "include '<stdlib.h>' or provide a declaration of 'malloc'",
            1,
            1,
        )
        .unwrap();
        assert_eq!(a.texto, "#include <stdlib.h>\n");
        assert_eq!(a.inicio, 0);
    }

    #[test]
    fn asignacion_en_condicion() {
        let f =
            "int f(int x)\n{\n    if (x = 5)\n    {\n        return 1;\n    }\n    return 0;\n}\n";
        let a = proponer(
            f,
            "suggest parentheses around assignment used as truth value [-Wparentheses]",
            3,
            9,
        )
        .unwrap();
        assert!(aplicar(f, &a).contains("if (x == 5)"));
        // Un error que no es simple: sin arreglo.
        assert!(proponer(f, "conflicting types for 'f'", 1, 5).is_none());
    }
}
