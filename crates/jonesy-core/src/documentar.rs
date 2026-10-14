//! Editar → Documentar la función: el comentario `/** @brief … @param … */` a partir de
//! la firma, con el mismo esqueleto que `corbel scaffold` (los textos entre corchetes
//! son los que corbel marca como pendientes).

use crate::simbolos::{self, Tipo};

#[derive(Debug, Clone, PartialEq)]
pub struct Firma {
    pub nombre: String,
    pub retorno: String,
    pub parametros: Vec<String>,
    /// Cada parámetro como está escrito (`const int v[]`), en el mismo orden.
    pub declaraciones: Vec<String>,
    /// Línea (0-based) donde empieza la declaración.
    pub linea: usize,
    pub sangria: String,
}

fn es_id(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// El nombre de un parámetro (`const int v[]` → `v`, `int (*f)(int)` → `f`).
fn nombre_parametro(p: &str) -> Option<String> {
    let p = p.trim();
    if p.is_empty() || p == "void" {
        return None;
    }
    if p == "..." {
        return Some("...".into());
    }
    if let Some(i) = p.find("(*") {
        let n: String = p[i + 2..].chars().take_while(|c| es_id(*c)).collect();
        return (!n.is_empty()).then_some(n);
    }
    // Sin los [] del final, el último identificador.
    let sin_corchetes = match p.find('[') {
        Some(i) => &p[..i],
        None => p,
    };
    let fin = sin_corchetes.trim_end();
    let n: String = fin
        .chars()
        .rev()
        .take_while(|c| es_id(*c))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    // Un parámetro sin nombre (sólo el tipo, en un prototipo): nada que documentar.
    let solo_tipo = fin.len() == n.len()
        || matches!(
            n.as_str(),
            "int" | "char" | "float" | "double" | "long" | "short" | "unsigned" | "size_t"
        );
    (!n.is_empty() && !solo_tipo).then_some(n)
}

/// La función (definición o prototipo) del cursor: la que empieza en esa línea o en las
/// dos siguientes, o la última que empieza antes.
pub fn firma_en(fuente: &str, linea_cursor: usize) -> Option<Firma> {
    let funciones: Vec<simbolos::Simbolo> = simbolos::de_texto(fuente)
        .into_iter()
        .filter(|s| matches!(s.tipo, Tipo::Funcion | Tipo::Prototipo))
        .collect();
    let s = funciones
        .iter()
        .find(|s| s.linea >= linea_cursor && s.linea <= linea_cursor + 2)
        .or_else(|| funciones.iter().rev().find(|s| s.linea <= linea_cursor))?;
    let chars: Vec<char> = fuente.chars().collect();
    let limpio = simbolos::limpio(&chars);
    // Posición del comienzo de la línea de la declaración.
    let mut inicio = 0;
    for _ in 0..s.linea {
        inicio += limpio[inicio..].iter().position(|c| *c == '\n')? + 1;
    }
    let linea: String = limpio[inicio..]
        .iter()
        .take_while(|c| **c != '\n')
        .collect();
    let sangria: String = linea.chars().take_while(|c| c.is_whitespace()).collect();
    let pos_nombre = inicio + s.columna;
    let retorno: String = limpio[inicio..pos_nombre]
        .iter()
        .collect::<String>()
        .trim()
        .to_string();
    let abre = pos_nombre + limpio[pos_nombre..].iter().position(|c| *c == '(')?;
    let mut nivel = 0;
    let mut cierra = None;
    for (i, c) in limpio[abre..].iter().enumerate() {
        match c {
            '(' => nivel += 1,
            ')' => {
                nivel -= 1;
                if nivel == 0 {
                    cierra = Some(abre + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let adentro: String = limpio[abre + 1..cierra?].iter().collect();
    // Separar por comas de primer nivel (no las de un puntero a función).
    let mut partes = vec![];
    let (mut actual, mut nivel) = (String::new(), 0);
    for c in adentro.chars() {
        match c {
            '(' => nivel += 1,
            ')' => nivel -= 1,
            ',' if nivel == 0 => {
                partes.push(std::mem::take(&mut actual));
                continue;
            }
            _ => {}
        }
        actual.push(c);
    }
    partes.push(actual);
    Some(Firma {
        nombre: s.nombre.clone(),
        retorno: retorno.split_whitespace().collect::<Vec<_>>().join(" "),
        parametros: partes.iter().filter_map(|p| nombre_parametro(p)).collect(),
        declaraciones: partes
            .iter()
            .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|p| !p.is_empty() && p != "void")
            .collect(),
        linea: s.linea,
        sangria,
    })
}

/// ¿Ya tiene un `/** … */` justo arriba?
pub fn documentada(fuente: &str, linea: usize) -> bool {
    let lineas: Vec<&str> = fuente.lines().collect();
    let Some(fin) = (0..linea.min(lineas.len()))
        .rev()
        .find(|i| !lineas[*i].trim().is_empty())
    else {
        return false;
    };
    if !lineas[fin].trim_end().ends_with("*/") {
        return false;
    }
    (0..=fin)
        .rev()
        .find(|j| lineas[*j].contains("/*"))
        .is_some_and(|j| lineas[j].trim_start().starts_with("/**"))
}

/// El comentario de documentación (con la sangría de la función y un salto final).
pub fn docblock(f: &Firma) -> String {
    let i = &f.sangria;
    let mut l = vec![
        format!("{i}/**"),
        format!(
            "{i} * @brief [Descripción breve de la función {}]",
            f.nombre
        ),
    ];
    if !f.parametros.is_empty() {
        l.push(format!("{i} *"));
        for p in &f.parametros {
            l.push(format!("{i} * @param {p} [Descripción del parámetro {p}]"));
        }
    }
    let sin_retorno =
        f.retorno.split_whitespace().last() == Some("void") && !f.retorno.contains('*');
    if !sin_retorno {
        if f.parametros.is_empty() {
            l.push(format!("{i} *"));
        }
        l.push(format!("{i} * @return [Descripción del valor de retorno]"));
    }
    l.push(format!("{i} * @pre [Precondiciones / requisitos previos]"));
    l.push(format!("{i} * @post [Postcondiciones / estado resultante]"));
    l.push(format!("{i} */"));
    l.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    const FUENTE: &str = "#include <stddef.h>\n\
\n\
/* suma de un vector */\n\
int vector_suma(const int v[], size_t n)\n\
{\n\
    return 0;\n\
}\n\
\n\
static void aplicar(int *datos,\n\
                    size_t n, int (*f)(int))\n\
{\n\
}\n\
\n\
/**\n\
 * @brief Ya está.\n\
 */\n\
double promedio(void);\n";

    #[test]
    fn firma_y_comentario() {
        let f = firma_en(FUENTE, 5).unwrap();
        assert_eq!(f.nombre, "vector_suma");
        assert_eq!(f.retorno, "int");
        assert_eq!(f.parametros, vec!["v", "n"]);
        assert_eq!(f.linea, 3);
        let d = docblock(&f);
        assert!(
            d.starts_with("/**\n * @brief [Descripción breve de la función vector_suma]\n *\n"),
            "{d}"
        );
        assert!(
            d.contains(" * @param v [Descripción del parámetro v]\n * @param n"),
            "{d}"
        );
        assert!(d.contains("@return") && d.ends_with(" */\n"), "{d}");
        assert!(!documentada(FUENTE, 3), "un /* */ común no cuenta");

        // Firma en dos líneas, void y un puntero a función.
        let f = firma_en(FUENTE, 7).unwrap();
        assert_eq!(f.nombre, "aplicar");
        assert_eq!(f.parametros, vec!["datos", "n", "f"]);
        assert!(!docblock(&f).contains("@return"));

        // Ya documentada; sin parámetros.
        let f = firma_en(FUENTE, 16).unwrap();
        assert_eq!(f.nombre, "promedio");
        assert!(f.parametros.is_empty());
        assert!(documentada(FUENTE, f.linea));
        assert!(docblock(&f).contains(" *\n * @return"));
    }

    #[test]
    fn parametros_sin_nombre_o_raros() {
        assert_eq!(nombre_parametro("int"), None);
        assert_eq!(nombre_parametro("const char *"), None);
        assert_eq!(nombre_parametro("char *s"), Some("s".into()));
        assert_eq!(nombre_parametro("int m[][3]"), Some("m".into()));
        assert_eq!(nombre_parametro("..."), Some("...".into()));
    }
}
