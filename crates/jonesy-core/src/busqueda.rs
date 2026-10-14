//! Buscar en todos los archivos del proyecto (Editar → Buscar en el proyecto).
//!
//! Recorre los archivos de texto (salteando ocultos, `target`, binarios y los de más de
//! 1 MiB) y devuelve cada línea con una coincidencia. Usa las mismas opciones que el
//! buscar y reemplazar del editor: distinguir mayúsculas y palabra completa.

use std::path::{Path, PathBuf};

/// Una línea con coincidencias.
#[derive(Debug, Clone, PartialEq)]
pub struct Hallazgo {
    pub archivo: PathBuf,
    /// 0-based.
    pub linea: usize,
    /// Columna (en caracteres) de la primera coincidencia de la línea.
    pub columna: usize,
    /// Coincidencias en la línea: (inicio, fin) en caracteres.
    pub rangos: Vec<(usize, usize)>,
    pub texto: String,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Resultado {
    pub hallazgos: Vec<Hallazgo>,
    pub archivos: usize,
    /// Se cortó al llegar al máximo de resultados.
    pub truncado: bool,
}

pub const MAXIMO: usize = 2000;
const TAM_MAXIMO: u64 = 1 << 20;

fn es_texto(datos: &[u8]) -> bool {
    !datos.iter().take(8000).any(|b| *b == 0)
}

/// Coincidencias de `patron` en `linea` como rangos de caracteres.
pub fn en_linea(linea: &str, patron: &str, mayusculas: bool, palabra: bool) -> Vec<(usize, usize)> {
    if patron.is_empty() {
        return vec![];
    }
    let norm = |c: char| {
        if mayusculas {
            c
        } else {
            c.to_lowercase().next().unwrap_or(c)
        }
    };
    let t: Vec<char> = linea.chars().map(norm).collect();
    let p: Vec<char> = patron.chars().map(norm).collect();
    let de_palabra = |c: char| c.is_alphanumeric() || c == '_';
    let mut v = vec![];
    let mut i = 0;
    while i + p.len() <= t.len() {
        if t[i..i + p.len()] == p[..] {
            let ini = i == 0 || !de_palabra(t[i - 1]);
            let fin = i + p.len() == t.len() || !de_palabra(t[i + p.len()]);
            if !palabra || (ini && fin) {
                v.push((i, i + p.len()));
                i += p.len();
                continue;
            }
        }
        i += 1;
    }
    v
}

/// Busca `patron` en los archivos de texto de `raiz`.
pub fn buscar(raiz: &Path, patron: &str, mayusculas: bool, palabra: bool) -> Resultado {
    let mut r = Resultado::default();
    if patron.is_empty() {
        return r;
    }
    let mut archivos = vec![];
    crate::simbolos::recorrer(raiz, &mut archivos, 0, &|p| {
        std::fs::metadata(p).is_ok_and(|m| m.len() <= TAM_MAXIMO)
            && !p.extension().is_some_and(|e| {
                matches!(e.to_str(), Some("o" | "a" | "so" | "exe" | "out" | "tar"))
            })
    });
    for a in archivos {
        let Ok(datos) = std::fs::read(&a) else {
            continue;
        };
        if !es_texto(&datos) {
            continue;
        }
        let texto = String::from_utf8_lossy(&datos);
        let mut alguno = false;
        for (n, linea) in texto.lines().enumerate() {
            let rangos = en_linea(linea, patron, mayusculas, palabra);
            if rangos.is_empty() {
                continue;
            }
            alguno = true;
            if r.hallazgos.len() >= MAXIMO {
                r.truncado = true;
                return r;
            }
            r.hallazgos.push(Hallazgo {
                archivo: a.clone(),
                linea: n,
                columna: rangos[0].0,
                rangos,
                texto: linea.to_string(),
            });
        }
        if alguno {
            r.archivos += 1;
        }
    }
    r
}

/// Dónde se usa `nombre` (una función, variable, tipo o macro) en los `.c` y `.h` del
/// proyecto: palabra completa, distinguiendo mayúsculas y sin contar comentarios ni
/// cadenas (Editar → Buscar referencias, Shift+F12).
pub fn referencias(raiz: &Path, nombre: &str) -> Resultado {
    let mut r = Resultado::default();
    if nombre.is_empty() {
        return r;
    }
    let mut archivos = vec![];
    crate::simbolos::recorrer(raiz, &mut archivos, 0, &|p| {
        std::fs::metadata(p).is_ok_and(|m| m.len() <= TAM_MAXIMO)
            && p.extension()
                .is_some_and(|e| matches!(e.to_str(), Some("c" | "h")))
    });
    for a in archivos {
        let Ok(texto) = std::fs::read_to_string(&a) else {
            continue;
        };
        let chars: Vec<char> = texto.chars().collect();
        let limpio: String = crate::simbolos::limpio(&chars).into_iter().collect();
        let mut alguno = false;
        for (n, (linea, sin_comentarios)) in texto.lines().zip(limpio.lines()).enumerate() {
            let rangos = en_linea(sin_comentarios, nombre, true, true);
            if rangos.is_empty() {
                continue;
            }
            alguno = true;
            if r.hallazgos.len() >= MAXIMO {
                r.truncado = true;
                return r;
            }
            r.hallazgos.push(Hallazgo {
                archivo: a.clone(),
                linea: n,
                columna: rangos[0].0,
                rangos,
                texto: linea.to_string(),
            });
        }
        if alguno {
            r.archivos += 1;
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn referencias_sin_comentarios_ni_cadenas() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            d.path().join("main.c"),
            "#include \"suma.h\"\n/* suma de dos */\nint main(void)\n{\n    printf(\"suma\");\n    return suma(1, sumas);\n}\n",
        )
        .unwrap();
        std::fs::write(d.path().join("suma.h"), "int suma(int a, int b);\n").unwrap();
        std::fs::write(d.path().join("notas.md"), "suma\n").unwrap();
        let r = referencias(d.path(), "suma");
        let lugares: Vec<(String, usize)> = r
            .hallazgos
            .iter()
            .map(|h| {
                (
                    h.archivo.file_name().unwrap().to_string_lossy().into(),
                    h.linea,
                )
            })
            .collect();
        assert_eq!(
            lugares,
            vec![("main.c".into(), 5), ("suma.h".into(), 0)],
            "sin el comentario, la cadena, el include ni el .md"
        );
        assert_eq!(r.hallazgos[0].rangos, vec![(11, 15)]);
    }

    #[test]
    fn busca_en_los_archivos_de_texto() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("libs/a")).unwrap();
        std::fs::write(d.path().join("main.c"), "int total = 0;\ntotal += Total;\n").unwrap();
        std::fs::write(d.path().join("libs/a/a.c"), "int subtotal;\n").unwrap();
        std::fs::write(d.path().join("programa"), b"total\0\x01binario").unwrap();
        std::fs::create_dir_all(d.path().join(".git")).unwrap();
        std::fs::write(d.path().join(".git/config"), "total").unwrap();
        let r = buscar(d.path(), "total", false, false);
        assert_eq!(r.hallazgos.len(), 3, "{:#?}", r.hallazgos);
        assert_eq!(r.archivos, 2);
        let m = r.hallazgos.iter().find(|h| h.linea == 1).unwrap();
        assert_eq!(m.rangos, vec![(0, 5), (9, 14)]);
        let r = buscar(d.path(), "total", true, true);
        assert_eq!(r.hallazgos.len(), 2, "sin Total ni subtotal");
        assert!(r.hallazgos.iter().all(|h| h.archivo.ends_with("main.c")));
    }
}
