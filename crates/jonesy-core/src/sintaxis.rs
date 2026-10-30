//! Errores de estructura del C sin compilar: llaves, paréntesis y corchetes que no cierran o
//! sobran, cadenas y caracteres sin cerrar y comentarios `/*` abiertos.
//!
//! Es lo que más desorienta al alumno: gcc informa el error lejos de donde está (una llave que
//! falta en la línea 12 aparece como «expected declaration» en la 40, o como cascada de errores
//! en la función siguiente). Acá se marca dónde empezó el problema, mientras se escribe y sin
//! esperar a compilar.
//!
//! Es un recorrido léxico (no un parser): salta cadenas, caracteres, comentarios y directivas del
//! preprocesador, que pueden tener llaves sueltas en macros. Una apertura sin cerrar se informa
//! como error cuando hay una señal fuerte de que falta el cierre (empieza otra función en la
//! columna 0 con el bloque todavía abierto, o termina el archivo); un cierre que no corresponde,
//! siempre.

use std::path::Path;

use crate::diagnostico::{Diagnostico, Origen, Severidad};

/// Nombre con el que estos diagnósticos se guardan y se muestran.
pub const HERRAMIENTA: &str = "jonesy";

#[derive(Debug, Clone, Copy)]
struct Apertura {
    c: char,
    linea: usize,
    columna: usize,
}

fn cierre_de(c: char) -> char {
    match c {
        '(' => ')',
        '[' => ']',
        _ => '}',
    }
}

fn nombre(c: char) -> &'static str {
    match c {
        '(' | ')' => "paréntesis",
        '[' | ']' => "corchete",
        _ => "llave",
    }
}

fn diag(
    linea: usize,
    columna: usize,
    severidad: Severidad,
    resumen: String,
    explicacion: &str,
) -> Diagnostico {
    Diagnostico {
        archivo: None,
        linea,
        columna,
        severidad,
        mensaje: resumen.clone(),
        codigo: None,
        sugerencia: None,
        origen: Origen::Herramienta,
        herramienta: Some(HERRAMIENTA.into()),
        categoria: Some("sintaxis".into()),
        enlace: None,
        resumen: Some(resumen),
        explicacion: Some(explicacion.into()),
    }
}

const POR_QUE_SIN_CERRAR: &str = "Cada `{`, `(` o `[` necesita su cierre. Cuando falta, gcc no se da cuenta \
ahí: sigue leyendo como si todo lo que viene estuviera adentro y protesta mucho más abajo, con un \
mensaje que no tiene que ver (por ejemplo, *expected declaration or statement at end of input*).\n\n\
**Así no**\n\n```c\nint doble(int x) {\n    return 2 * x;\n\nint main(void) {\n```\n\n**Así sí**\n\n\
```c\nint doble(int x) {\n    return 2 * x;\n}\n\nint main(void) {\n```\n\nMirá la sangría: el cierre va \
alineado con la línea que abrió el bloque.";

const POR_QUE_SOBRA: &str = "Este cierre no tiene una apertura que le corresponda: sobra, o falta abrir algo \
antes, o se cerró otra cosa en el medio. Con la sangría al día se ve qué bloque cierra cada llave.\n\n\
**Así no**\n\n```c\nif (x > 0)) {\n```\n\n**Así sí**\n\n```c\nif (x > 0) {\n```";

const POR_QUE_CRUZADO: &str = "Los bloques se cierran en el orden inverso al que se abren: primero lo último \
que se abrió. Acá se cierra uno de afuera con otro de adentro todavía abierto.\n\n**Así no**\n\n```c\n\
printf(\"%d\\n\", v[i);\n```\n\n**Así sí**\n\n```c\nprintf(\"%d\\n\", v[i]);\n```";

const POR_QUE_CADENA: &str = "Una cadena empieza y termina en la misma línea con `\"`. Si falta la comilla \
de cierre, todo lo que sigue (hasta otra comilla) queda adentro de la cadena. Para un salto de línea \
dentro del texto se escribe `\\n`.\n\n**Así no**\n\n```c\nprintf(\"Resultado: %d\\n, total);\n```\n\n\
**Así sí**\n\n```c\nprintf(\"Resultado: %d\\n\", total);\n```";

const POR_QUE_CARACTER: &str = "Un carácter va entre comillas simples y es uno solo: `'a'`, `'\\n'`, \
`'\\0'`. Para texto de más de una letra se usan comillas dobles.\n\n**Así no**\n\n```c\nchar c = 'a;\n\
char *s = 'hola';\n```\n\n**Así sí**\n\n```c\nchar c = 'a';\nchar *s = \"hola\";\n```";

const POR_QUE_COMENTARIO: &str =
    "Un comentario `/*` sigue hasta el primer `*/`. Si falta el cierre, el \
resto del archivo es comentario y gcc no ve el código.\n\n**Así no**\n\n```c\n/* suma los pares\n\
int suma(void) {\n```\n\n**Así sí**\n\n```c\n/* suma los pares */\nint suma(void) {\n```";

/// ¿Empieza en esta línea la definición de una función (en la columna 0)?
fn empieza_funcion(linea: &str) -> bool {
    let t = linea.trim_end();
    if t.is_empty()
        || t.starts_with(char::is_whitespace)
        || t.starts_with('#')
        || t.starts_with('}')
    {
        return false;
    }
    let primera = t
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .next()
        .unwrap_or("");
    const NO: [&str; 9] = [
        "return", "if", "else", "for", "while", "do", "switch", "case", "typedef",
    ];
    if primera.is_empty()
        || NO.contains(&primera)
        || !primera
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
    {
        return false;
    }
    // tipo nombre(...) con algo antes del paréntesis y sin `;` ni `=` antes de él.
    match t.find('(') {
        Some(p) => {
            let antes = &t[..p];
            antes.split_whitespace().count() >= 2 && !antes.contains('=') && !antes.contains(';')
        }
        None => false,
    }
}

/// Los problemas de estructura de un fuente C. Con `linea_cursor` (1-based), no se informa lo
/// que empieza en la línea que se está escribiendo.
pub fn revisar(texto: &str, linea_cursor: Option<usize>) -> Vec<Diagnostico> {
    let mut out = Vec::new();
    let mut pila: Vec<Apertura> = Vec::new();
    let mut comentario: Option<(usize, usize)> = None;
    let mut continua_directiva = false;
    // Cadena o carácter que sigue en la línea siguiente (terminó en `\`): su comilla.
    let mut cadena_continua: Option<char> = None;
    let lineas: Vec<&str> = texto.split('\n').collect();

    for (i, linea) in lineas.iter().enumerate() {
        let n = i + 1;
        let chars: Vec<char> = linea.trim_end_matches('\r').chars().collect();

        // Directivas del preprocesador (y sus continuaciones): se saltan enteras.
        let es_directiva = comentario.is_none()
            && (continua_directiva || chars.iter().find(|c| !c.is_whitespace()) == Some(&'#'));
        if es_directiva {
            continua_directiva = chars.last() == Some(&'\\');
            continue;
        }
        continua_directiva = false;

        // Empieza otra función con un bloque todavía abierto: falta un cierre.
        if comentario.is_none() && empieza_funcion(linea) {
            if let Some(a) = pila.iter().find(|a| a.c == '{').copied() {
                out.push(diag(
                    a.linea,
                    a.columna,
                    Severidad::Error,
                    format!("Esta llave no se cierra: en la línea {n} empieza otra función con el bloque todavía abierto."),
                    POR_QUE_SIN_CERRAR,
                ));
                pila.clear();
            }
        }

        let mut j = 0;
        if let Some(q) = cadena_continua.take() {
            // Seguimos adentro de la cadena de la línea anterior: hasta su comilla de cierre.
            let mut k = 0;
            while k < chars.len() && chars[k] != q {
                k += if chars[k] == '\\' { 2 } else { 1 };
            }
            if k >= chars.len() {
                if chars.last() == Some(&'\\') {
                    cadena_continua = Some(q);
                }
                continue;
            }
            j = k + 1;
        }
        while j < chars.len() {
            let c = chars[j];
            let col = j + 1;
            if comentario.is_some() {
                if c == '*' && chars.get(j + 1) == Some(&'/') {
                    comentario = None;
                    j += 2;
                } else {
                    j += 1;
                }
                continue;
            }
            match c {
                '/' if chars.get(j + 1) == Some(&'/') => break,
                '/' if chars.get(j + 1) == Some(&'*') => {
                    comentario = Some((n, col));
                    j += 2;
                    continue;
                }
                '"' | '\'' => {
                    let mut k = j + 1;
                    let mut cerrada = false;
                    while k < chars.len() {
                        if chars[k] == '\\' {
                            k += 2;
                            continue;
                        }
                        if chars[k] == c {
                            cerrada = true;
                            break;
                        }
                        k += 1;
                    }
                    let continua = chars.last() == Some(&'\\');
                    if !cerrada && !continua && linea_cursor != Some(n) {
                        let (resumen, por_que) = if c == '"' {
                            ("Esta cadena no se cierra: falta la comilla `\"` antes del fin de la línea.", POR_QUE_CADENA)
                        } else {
                            (
                                "Este carácter no se cierra: falta la comilla simple `'`.",
                                POR_QUE_CARACTER,
                            )
                        };
                        out.push(diag(n, col, Severidad::Error, resumen.into(), por_que));
                    }
                    if !cerrada {
                        if continua {
                            cadena_continua = Some(c);
                        }
                        break;
                    }
                    j = k + 1;
                    continue;
                }
                '(' | '[' | '{' => pila.push(Apertura {
                    c,
                    linea: n,
                    columna: col,
                }),
                ')' | ']' | '}' => match pila.last() {
                    Some(a) if cierre_de(a.c) == c => {
                        pila.pop();
                    }
                    Some(a) if pila.iter().any(|x| cierre_de(x.c) == c) => {
                        // Cierra algo de afuera con otra cosa abierta adentro.
                        let a = *a;
                        if linea_cursor != Some(n) {
                            out.push(diag(
                                n,
                                col,
                                Severidad::Error,
                                format!(
                                    "Este `{c}` cierra antes de tiempo: queda abierto el {} `{}` de la línea {}, columna {}.",
                                    nombre(a.c),
                                    a.c,
                                    a.linea,
                                    a.columna
                                ),
                                POR_QUE_CRUZADO,
                            ));
                        }
                        while let Some(x) = pila.pop() {
                            if cierre_de(x.c) == c {
                                break;
                            }
                        }
                    }
                    _ => {
                        if linea_cursor != Some(n) {
                            out.push(diag(
                                n,
                                col,
                                Severidad::Error,
                                format!(
                                    "Sobra este `{c}`: no hay un {} abierto que cierre.",
                                    nombre(c)
                                ),
                                POR_QUE_SOBRA,
                            ));
                        }
                    }
                },
                _ => {}
            }
            j += 1;
        }
    }

    if let Some((l, c)) = comentario {
        out.push(diag(
            l,
            c,
            Severidad::Error,
            "Este comentario `/*` no se cierra: falta `*/` y el resto del archivo queda comentado."
                .into(),
            POR_QUE_COMENTARIO,
        ));
    }
    // Lo que queda abierto al final del archivo (salvo lo de la línea que se está escribiendo).
    for a in pila.iter().filter(|a| linea_cursor != Some(a.linea)) {
        out.push(diag(
            a.linea,
            a.columna,
            Severidad::Error,
            format!(
                "Este {} `{}` no se cierra antes del fin del archivo.",
                nombre(a.c),
                a.c
            ),
            POR_QUE_SIN_CERRAR,
        ));
    }
    out.sort_by_key(|d| (d.linea, d.columna));
    out
}

/// Como [`revisar`], con el archivo puesto en cada diagnóstico.
pub fn revisar_archivo(
    texto: &str,
    archivo: &Path,
    linea_cursor: Option<usize>,
) -> Vec<Diagnostico> {
    revisar(texto, linea_cursor)
        .into_iter()
        .map(|mut d| {
            d.archivo = Some(archivo.to_path_buf());
            d
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resumenes(texto: &str) -> Vec<(usize, String)> {
        revisar(texto, None)
            .into_iter()
            .map(|d| (d.linea, d.mensaje))
            .collect()
    }

    #[test]
    fn codigo_correcto_sin_hallazgos() {
        let c =
            "#include <stdio.h>\n#define MAX(a, b) ((a) > (b) ? (a) : (b))\n#define BLOQUE {\n\n\
                 /* llaves en un comentario: { ( [ */\nint main(void) {\n    char c = '{';\n    \
                 printf(\"}%c)\\n\", c); // )\n    int v[3] = {1, 2, 3};\n    return v[0];\n}\n";
        assert!(resumenes(c).is_empty(), "{:?}", resumenes(c));
    }

    #[test]
    fn llave_que_falta_se_marca_donde_abre() {
        let c = "int doble(int x) {\n    if (x > 0) {\n        return 2 * x;\n    return 0;\n}\n\nint main(void) {\n    return doble(2);\n}\n";
        let r = resumenes(c);
        assert_eq!(r.len(), 1, "{r:?}");
        assert_eq!(r[0].0, 1);
        assert!(r[0].1.contains("línea 7"), "{r:?}");
    }

    #[test]
    fn cierre_que_sobra_y_cruzado() {
        let r = resumenes("int main(void) {\n    if (x > 0)) {\n    }\n    f(v[i);\n}\n");
        assert!(
            r.iter()
                .any(|(l, m)| *l == 2 && m.starts_with("Sobra este `)`")),
            "{r:?}"
        );
        assert!(
            r.iter()
                .any(|(l, m)| *l == 4 && m.contains("queda abierto el corchete")),
            "{r:?}"
        );
    }

    #[test]
    fn cadena_caracter_y_comentario_sin_cerrar() {
        let r = resumenes("int main(void) {\n    printf(\"hola\\n);\n    char c = 'a;\n    return 0;\n}\n/* sin cierre\n");
        assert!(
            r.iter().any(|(l, m)| *l == 2 && m.contains("cadena")),
            "{r:?}"
        );
        assert!(
            r.iter().any(|(l, m)| *l == 3 && m.contains("carácter")),
            "{r:?}"
        );
        assert!(
            r.iter().any(|(l, m)| *l == 6 && m.contains("comentario")),
            "{r:?}"
        );
    }

    #[test]
    fn cadena_que_sigue_en_la_linea_siguiente() {
        let c = "const char *s = \"abc\\\ndef\";\nint main(void) {\n    return 0;\n}\n";
        assert!(resumenes(c).is_empty(), "{:?}", resumenes(c));
    }

    #[test]
    fn la_linea_que_se_escribe_no_se_marca() {
        let c = "int main(void) {\n    printf(\"hola\n";
        assert!(revisar(c, Some(2)).iter().all(|d| d.linea != 2));
        // la llave de la línea 1 sigue sin cerrar al final del archivo
        assert!(revisar(c, Some(2)).iter().any(|d| d.linea == 1));
    }

    #[test]
    fn deteccion_de_funcion_en_columna_cero() {
        assert!(empieza_funcion("int main(void) {"));
        assert!(empieza_funcion("static size_t contar(const char *s)"));
        assert!(!empieza_funcion("    int x = f(2);"));
        assert!(!empieza_funcion("return f(x);"));
        assert!(!empieza_funcion("x = f(2);"));
        assert!(!empieza_funcion("#define F(x) (x)"));
    }
}
