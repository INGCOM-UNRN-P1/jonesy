//! Autocompletar (Ctrl+Espacio): los nombres del archivo (variables, funciones, tipos,
//! macros) y las funciones de la biblioteca que conoce el quick-doc, filtrados por lo que
//! se está escribiendo.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Candidato {
    pub texto: String,
    /// La firma (de la biblioteca) o «en este archivo».
    pub detalle: String,
}

const RESERVADAS: &[&str] = &[
    "auto", "break", "case", "char", "const", "continue", "default", "do", "double", "else",
    "enum", "extern", "float", "for", "goto", "if", "inline", "int", "long", "register",
    "restrict", "return", "short", "signed", "sizeof", "static", "struct", "switch", "typedef",
    "union", "unsigned", "void", "volatile", "while", "bool", "true", "false", "include", "define",
    "ifndef", "ifdef", "endif", "pragma", "NULL",
];

/// ¿Palabra reservada de C (o del preprocesador)?
pub fn es_reservada(palabra: &str) -> bool {
    RESERVADAS.contains(&palabra)
}

fn es_id(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Dónde empieza la palabra que se está escribiendo (en caracteres) y la palabra.
pub fn prefijo(fuente: &[char], cursor: usize) -> (usize, String) {
    let mut i = cursor.min(fuente.len());
    while i > 0 && es_id(fuente[i - 1]) {
        i -= 1;
    }
    (i, fuente[i..cursor.min(fuente.len())].iter().collect())
}

/// Los candidatos para lo que hay antes del cursor. `biblioteca` son (nombre, firma).
pub fn candidatos(fuente: &str, cursor: usize, biblioteca: &[(String, String)]) -> Vec<Candidato> {
    let chars: Vec<char> = fuente.chars().collect();
    let (inicio, pre) = prefijo(&chars, cursor);
    // Los identificadores del archivo (sin comentarios ni cadenas), con cuántas veces
    // aparecen, salvo la palabra que se está escribiendo.
    let limpio = crate::simbolos::limpio(&chars);
    let mut cuenta: HashMap<String, usize> = HashMap::new();
    let mut i = 0;
    while i < limpio.len() {
        if es_id(limpio[i]) && (i == 0 || !es_id(limpio[i - 1])) {
            let ini = i;
            while i < limpio.len() && es_id(limpio[i]) {
                i += 1;
            }
            if ini == inicio {
                continue;
            }
            let palabra: String = limpio[ini..i].iter().collect();
            if palabra.chars().count() >= 2
                && !palabra.starts_with(|c: char| c.is_ascii_digit())
                && !RESERVADAS.contains(&palabra.as_str())
            {
                *cuenta.entry(palabra).or_default() += 1;
            }
        } else {
            i += 1;
        }
    }
    let minus = pre.to_lowercase();
    let coincide = |n: &str| -> Option<u8> {
        if n == pre {
            None
        } else if n.starts_with(&pre) {
            Some(0)
        } else if n.to_lowercase().starts_with(&minus) {
            Some(1)
        } else {
            None
        }
    };
    let mut del_archivo: Vec<(u8, usize, String)> = cuenta
        .into_iter()
        .filter_map(|(n, c)| coincide(&n).map(|g| (g, c, n)))
        .collect();
    del_archivo.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
    let mut de_biblioteca: Vec<(u8, &String, &String)> = biblioteca
        .iter()
        .filter(|(n, _)| !del_archivo.iter().any(|(_, _, x)| x == n))
        .filter_map(|(n, f)| coincide(n).map(|g| (g, n, f)))
        .collect();
    de_biblioteca.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(b.1)));
    let mut v: Vec<(u8, Candidato)> = del_archivo
        .into_iter()
        .map(|(g, _, n)| {
            (
                g,
                Candidato {
                    texto: n,
                    detalle: "en este archivo".into(),
                },
            )
        })
        .chain(de_biblioteca.into_iter().map(|(g, n, f)| {
            (
                g,
                Candidato {
                    texto: n.clone(),
                    detalle: f.clone(),
                },
            )
        }))
        .collect();
    // Primero los que empiezan exactamente igual; el orden de cada grupo se mantiene.
    v.sort_by_key(|(g, _)| *g);
    v.into_iter().map(|(_, c)| c).take(60).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nombres_del_archivo_y_de_la_biblioteca() {
        let fuente = "int contador = 0;\nint contar(int n) { return contador + n; }\n/* comentario_largo */\nint main(void) { co";
        let biblio = vec![
            (
                "strcpy".to_string(),
                "char *strcpy(char *d, const char *o)".to_string(),
            ),
            ("cos".to_string(), "double cos(double x)".to_string()),
        ];
        let c = candidatos(fuente, fuente.chars().count(), &biblio);
        let textos: Vec<&str> = c.iter().map(|c| c.texto.as_str()).collect();
        // Los del archivo primero (el más usado antes), después la biblioteca; sin los
        // del comentario ni palabras reservadas.
        assert_eq!(textos, vec!["contador", "contar", "cos"], "{c:?}");
        assert_eq!(c[2].detalle, "double cos(double x)");
        // Sin distinguir mayúsculas, después de los exactos.
        let c = candidatos("int Total;\nint t", 16, &[]);
        assert_eq!(c[0].texto, "Total");
        let chars: Vec<char> = "x = cont".chars().collect();
        assert_eq!(prefijo(&chars, 8), (4, "cont".into()));
    }
}
