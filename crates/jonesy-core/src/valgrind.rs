//! Errores de memoria de valgrind (memcheck) explicados en español y ubicados en el código
//! del alumno: escrituras y lecturas fuera de un bloque, uso después de `free`, valores
//! sin inicializar, `free` doble y fugas.
//!
//! Cada error de valgrind es un bloque de líneas `==PID== …`: el título («Invalid read of
//! size 4»), la pila (`at`/`by … (archivo.c:línea)`) y, a veces, de qué bloque se trata
//! («Address … is 0 bytes after a block of size 12 alloc'd» con la pila del malloc).

use std::path::{Path, PathBuf};

use crate::diagnostico::{directorio_de_make, Diagnostico, Origen, Severidad};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Resultado {
    pub diagnosticos: Vec<Diagnostico>,
    /// Errores informados por valgrind (ERROR SUMMARY).
    pub errores: usize,
    /// Bytes perdidos sin remedio (definitely lost).
    pub bytes_perdidos: usize,
    /// Se encontró salida de valgrind.
    pub reconocida: bool,
}

/// Un marco de la pila: función y ubicación (`main (mem.c:14)`).
#[derive(Debug, Clone)]
pub(crate) struct Marco {
    pub(crate) funcion: String,
    pub(crate) archivo: String,
    pub(crate) linea: usize,
}

fn marco(texto: &str) -> Option<Marco> {
    // «at 0x4004DA: main (mem.c:14)» / «by 0x4004CD: main (mem.c:13)»
    let t = texto.trim();
    let t = t.strip_prefix("at ").or_else(|| t.strip_prefix("by "))?;
    let (_, resto) = t.split_once(": ")?;
    let (funcion, ubic) = resto.rsplit_once(" (")?;
    let ubic = ubic.trim_end_matches(')');
    let (archivo, linea) = ubic.rsplit_once(':')?;
    Some(Marco {
        funcion: funcion.trim().to_string(),
        archivo: archivo.to_string(),
        linea: linea.parse().ok()?,
    })
}

/// Bloque de un error: título, su pila y las secciones que siguen («Address …», «Block
/// was alloc'd at»), cada una con su pila.
#[derive(Debug, Default)]
struct Bloque {
    titulo: String,
    pila: Vec<Marco>,
    /// (texto de la sección, su pila)
    secciones: Vec<(String, Vec<Marco>)>,
}

fn contenido(linea: &str) -> Option<&str> {
    // «==1702081== Invalid read of size 4»
    let resto = linea.strip_prefix("==")?;
    let (pid, texto) = resto.split_once("== ")?;
    pid.chars().all(|c| c.is_ascii_digit()).then_some(texto)
}

/// Ubica `archivo` (valgrind sólo da el nombre) en el directorio donde corrió el
/// programa o en el proyecto.
pub(crate) fn ubicar(archivo: &str, base: &Path, raiz: &Path) -> Option<PathBuf> {
    let directo = base.join(archivo);
    if directo.is_file() {
        return Some(directo);
    }
    let directo = raiz.join(archivo);
    if directo.is_file() {
        return Some(directo);
    }
    // Búsqueda acotada por nombre (bibliotecas en libs/*).
    let nombre = Path::new(archivo).file_name()?;
    fn buscar(dir: &Path, nombre: &std::ffi::OsStr, prof: usize) -> Option<PathBuf> {
        if prof == 0 {
            return None;
        }
        let mut entradas: Vec<PathBuf> = std::fs::read_dir(dir)
            .ok()?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .collect();
        entradas.sort();
        for p in &entradas {
            if p.is_file() && p.file_name() == Some(nombre) {
                return Some(p.clone());
            }
        }
        entradas
            .iter()
            .filter(|p| {
                p.is_dir()
                    && !p
                        .file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with('.') || n == "target")
            })
            .find_map(|p| buscar(p, nombre, prof - 1))
    }
    buscar(raiz, nombre, 4)
}

/// Primer marco que cae en un archivo del proyecto.
pub(crate) fn propio(pila: &[Marco], base: &Path, raiz: &Path) -> Option<(PathBuf, Marco)> {
    pila.iter()
        .find_map(|m| ubicar(&m.archivo, base, raiz).map(|p| (p, m.clone())))
}

pub(crate) fn donde(pila: &[Marco], base: &Path, raiz: &Path) -> Option<String> {
    propio(pila, base, raiz).map(|(p, m)| {
        format!(
            "{}:{} ({}())",
            p.file_name().unwrap_or_default().to_string_lossy(),
            m.linea,
            m.funcion
        )
    })
}

fn tamano(titulo: &str) -> Option<usize> {
    titulo.rsplit("size ").next()?.trim().parse().ok()
}

/// Qué bloque y dónde se pidió o liberó, según las secciones del error.
fn contexto(b: &Bloque, base: &Path, raiz: &Path) -> String {
    let mut partes = vec![];
    for (texto, pila) in &b.secciones {
        let d = donde(pila, base, raiz);
        if let Some(i) = texto.find("bytes after a block of size") {
            let antes = texto[..i]
                .split_whitespace()
                .last()
                .and_then(|n| n.parse::<usize>().ok())
                .unwrap_or(0);
            let tam = texto[i..].split_whitespace().nth(6).unwrap_or("?");
            partes.push(format!(
                "{} del final de un bloque de {tam} bytes{}",
                if antes == 0 {
                    "justo después".to_string()
                } else {
                    format!("{antes} bytes después")
                },
                d.map(|d| format!(" pedido en {d}")).unwrap_or_default()
            ));
        } else if texto.contains("bytes before a block of size") {
            partes.push(format!(
                "antes del principio de un bloque{}",
                d.map(|d| format!(" pedido en {d}")).unwrap_or_default()
            ));
        } else if texto.contains("free'd") {
            partes.push(format!(
                "en un bloque ya liberado{}",
                d.map(|d| format!(" en {d}")).unwrap_or_default()
            ));
        } else if texto.starts_with("Block was alloc'd at") {
            if let Some(d) = d {
                partes.push(format!("pedido en {d}"));
            }
        } else if texto.contains("is not stack'd, malloc'd or") {
            partes.push("una dirección que no es de la pila ni de malloc (¿puntero sin inicializar o NULL?)".into());
        }
    }
    partes.join("; ")
}

fn explicar(b: &Bloque, base: &Path, raiz: &Path) -> Option<(Severidad, String)> {
    let t = b.titulo.as_str();
    let ctx = contexto(b, base, raiz);
    let con = |s: String| {
        if ctx.is_empty() {
            s
        } else {
            format!("{s}: {ctx}")
        }
    };
    let bytes = tamano(t)
        .map(|n| format!("{n} bytes"))
        .unwrap_or_else(|| "memoria".into());
    Some(if t.starts_with("Invalid write") {
        (
            Severidad::Error,
            con(format!(
                "Escritura fuera de la memoria pedida ({bytes}; ¿índice fuera del arreglo o uso después de free?)"
            )),
        )
    } else if t.starts_with("Invalid read") {
        (
            Severidad::Error,
            con(format!(
                "Lectura fuera de la memoria pedida ({bytes}; ¿índice fuera del arreglo o uso después de free?)"
            )),
        )
    } else if t.starts_with("Invalid free") {
        let doble = b.secciones.iter().any(|(s, _)| s.contains("free'd"));
        (
            Severidad::Error,
            con(if doble {
                "free doble: este bloque ya se había liberado".into()
            } else {
                "free de algo que no se pidió con malloc".into()
            }),
        )
    } else if t.starts_with("Mismatched free") {
        (
            Severidad::Error,
            "Se libera con una función que no corresponde a cómo se pidió".into(),
        )
    } else if t.starts_with("Conditional jump or move depends on uninitialised") {
        (
            Severidad::Error,
            "Una condición (if, while, for) usa una variable sin inicializar: contiene basura"
                .into(),
        )
    } else if t.starts_with("Use of uninitialised value") {
        (
            Severidad::Error,
            "Se usa un valor sin inicializar (contiene basura)".into(),
        )
    } else if t.starts_with("Syscall param") {
        (
            Severidad::Error,
            "Se pasa al sistema (printf, write…) memoria sin inicializar".into(),
        )
    } else if t.starts_with("Source and destination overlap") {
        (
            Severidad::Error,
            "Origen y destino se superponen: usá memmove en vez de memcpy/strcpy".into(),
        )
    } else if t.contains("has a fishy") {
        (
            Severidad::Error,
            "malloc recibió un tamaño negativo o enorme (¿una cuenta que dio negativa?)".into(),
        )
    } else if t.contains("are definitely lost") {
        (
            Severidad::Error,
            format!(
                "Fuga de memoria: {} pedidos acá nunca se liberaron (falta un free)",
                bytes_de_fuga(t)
            ),
        )
    } else if t.contains("are indirectly lost") {
        (
            Severidad::Advertencia,
            format!(
                "Fuga indirecta: {} apuntados desde otro bloque que se perdió (al liberar la estructura, liberá también sus nodos)",
                bytes_de_fuga(t)
            ),
        )
    } else if t.contains("are possibly lost") {
        (
            Severidad::Advertencia,
            format!("Posible fuga de memoria: {} pedidos acá", bytes_de_fuga(t)),
        )
    } else if t.contains("are still reachable") {
        (
            Severidad::Nota,
            format!(
                "{} pedidos acá siguen sin liberar al terminar (todavía apuntados)",
                bytes_de_fuga(t)
            ),
        )
    } else {
        return None;
    })
}

fn bytes_de_fuga(titulo: &str) -> String {
    // «10 bytes in 1 blocks are definitely lost…» o «24 (16 direct, 8 indirect) bytes in…»
    let n = titulo.split_whitespace().next().unwrap_or("?");
    let n = n.replace(',', "");
    let bloques = titulo
        .split(" bytes in ")
        .nth(1)
        .and_then(|r| r.split_whitespace().next())
        .unwrap_or("?");
    if bloques == "1" {
        format!("{n} bytes")
    } else {
        format!("{n} bytes en {bloques} bloques")
    }
}

/// Interpreta la salida de valgrind (mezclada con la de make) corrida desde `raiz`.
pub fn interpretar(salida: &str, raiz: &Path) -> Resultado {
    let limpia = crate::explicacion::sin_ansi(salida);
    let salida = limpia.as_str();
    let mut r = Resultado::default();
    let mut pila_dirs: Vec<PathBuf> = vec![raiz.to_path_buf()];
    let mut bloques: Vec<(PathBuf, Bloque)> = vec![];
    let mut actual: Option<Bloque> = None;
    let mut en_pila_principal = true;
    for linea in salida.lines() {
        if let Some(d) = directorio_de_make(linea, "Entering directory")
            .or_else(|| directorio_de_make(linea, "Se entra en el directorio"))
        {
            pila_dirs.push(d);
            continue;
        }
        if directorio_de_make(linea, "Leaving directory")
            .or_else(|| directorio_de_make(linea, "Se sale del directorio"))
            .is_some()
        {
            if pila_dirs.len() > 1 {
                pila_dirs.pop();
            }
            continue;
        }
        let Some(c) = contenido(linea) else {
            continue;
        };
        r.reconocida = true;
        let base = pila_dirs.last().unwrap().clone();
        if c.trim().is_empty() {
            if let Some(b) = actual.take() {
                bloques.push((base, b));
            }
            continue;
        }
        if let Some(resto) = c.strip_prefix("ERROR SUMMARY: ") {
            r.errores += resto
                .split_whitespace()
                .next()
                .and_then(|n| n.replace(',', "").parse::<usize>().ok())
                .unwrap_or(0);
            continue;
        }
        if let Some(resto) = c.trim().strip_prefix("definitely lost: ") {
            r.bytes_perdidos += resto
                .split_whitespace()
                .next()
                .and_then(|n| n.replace(',', "").parse::<usize>().ok())
                .unwrap_or(0);
            continue;
        }
        match &mut actual {
            None => {
                if !c.starts_with(' ') {
                    actual = Some(Bloque {
                        titulo: c.trim().to_string(),
                        ..Default::default()
                    });
                    en_pila_principal = true;
                }
            }
            Some(b) => {
                if let Some(m) = marco(c) {
                    if en_pila_principal {
                        b.pila.push(m);
                    } else if let Some((_, p)) = b.secciones.last_mut() {
                        p.push(m);
                    }
                } else {
                    en_pila_principal = false;
                    b.secciones.push((c.trim().to_string(), vec![]));
                }
            }
        }
    }
    if let Some(b) = actual.take() {
        bloques.push((pila_dirs.last().unwrap().clone(), b));
    }
    for (base, b) in bloques {
        let Some((severidad, resumen)) = explicar(&b, &base, raiz) else {
            continue;
        };
        // La ubicación: la pila del error (o la del malloc en una fuga), en el código propio.
        let (archivo, linea) = match propio(&b.pila, &base, raiz)
            .or_else(|| b.secciones.iter().find_map(|(_, p)| propio(p, &base, raiz)))
        {
            Some((p, m)) => (Some(p), m.linea),
            None => (None, 0),
        };
        r.diagnosticos.push(Diagnostico {
            archivo,
            linea,
            columna: 1,
            severidad,
            mensaje: b.titulo.clone(),
            resumen: Some(resumen),
            origen: Origen::Herramienta,
            herramienta: Some("valgrind".into()),
            categoria: Some("memoria".into()),
            ..Default::default()
        });
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    const SALIDA: &str = include_str!("../tests/datos/valgrind_salida.txt");

    #[test]
    fn errores_y_fugas_en_el_codigo_del_alumno() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("mem.c"), "/* … */\n").unwrap();
        let r = interpretar(SALIDA, d.path());
        assert!(r.reconocida);
        assert_eq!(r.errores, 6);
        assert_eq!(r.bytes_perdidos, 10);
        let lineas: Vec<(usize, &str)> = r
            .diagnosticos
            .iter()
            .map(|d| (d.linea, d.resumen.as_deref().unwrap()))
            .collect();
        assert_eq!(r.diagnosticos.len(), 6, "{lineas:#?}");
        let mem = d.path().join("mem.c");
        assert!(r
            .diagnosticos
            .iter()
            .all(|x| x.archivo.as_deref() == Some(mem.as_path())));

        assert_eq!(lineas[0].0, 14);
        assert_eq!(
            lineas[0].1,
            "Escritura fuera de la memoria pedida (4 bytes; ¿índice fuera del arreglo o uso después de free?): justo después del final de un bloque de 12 bytes pedido en mem.c:7 (crear())"
        );
        assert_eq!(lineas[1].0, 15);
        assert!(lineas[1].1.contains("8 bytes después del final"));
        assert_eq!(lineas[2].0, 17);
        assert!(
            lineas[2]
                .1
                .contains("en un bloque ya liberado en mem.c:16 (main())"),
            "{}",
            lineas[2].1
        );
        assert_eq!(lineas[3].0, 21);
        assert!(lineas[3].1.starts_with("Una condición"));
        assert_eq!(lineas[4].0, 22);
        assert!(lineas[4].1.starts_with("free doble"));
        // La fuga se ubica en el malloc que no se liberó.
        assert_eq!(lineas[5].0, 18);
        assert_eq!(
            lineas[5].1,
            "Fuga de memoria: 10 bytes pedidos acá nunca se liberaron (falta un free)"
        );
    }

    #[test]
    fn sin_valgrind() {
        let r = interpretar("make: algo\nok\n", Path::new("/tp"));
        assert!(!r.reconocida && r.diagnosticos.is_empty());
    }
}
