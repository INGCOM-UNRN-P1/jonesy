//! Diferencias entre versiones de un archivo, en un modelo de filas que sirve tanto para
//! la vista lado a lado como para la unificada.
//!
//! - [`comparar`] calcula el diff entre dos textos (con `similar`): lo usan el panel de
//!   git (árbol de trabajo contra índice o HEAD) y la comparación del buffer actual.
//! - [`desde_unificado`] interpreta la salida de `git diff` / `git show` (varios archivos),
//!   para los commits del log.
//!
//! En ambos casos, un bloque de líneas borradas seguido de uno de agregadas se empareja
//! línea a línea como «cambiadas», con el tramo de caracteres que difiere resaltado.

use similar::{ChangeTag, DiffTag, TextDiff};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    Igual,
    Borrada,
    Agregada,
    /// Mitad de un par borrada/agregada (la línea se modificó).
    Cambiada,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Celda {
    /// Número de línea (1-based) en su versión.
    pub num: usize,
    pub texto: String,
    pub tipo: Tipo,
    /// Tramos de caracteres [ini, fin) que difieren dentro de la línea (sólo en Cambiada).
    pub resaltes: Vec<(usize, usize)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FilaDiff {
    /// Comienzo de un archivo (diffs de varios archivos).
    Archivo(String),
    /// Encabezado de un bloque (`@@ -10,4 +10,6 @@ función`).
    Bloque(String),
    /// Una fila: izquierda (versión vieja) y derecha (nueva); una de las dos puede faltar.
    Par {
        izq: Option<Celda>,
        der: Option<Celda>,
    },
}

impl FilaDiff {
    pub fn es_cambio(&self) -> bool {
        matches!(self, FilaDiff::Par { izq, der } if izq.as_ref().is_none_or(|c| c.tipo != Tipo::Igual) || der.is_none())
    }
}

/// Líneas agregadas y borradas.
pub fn estadistica(filas: &[FilaDiff]) -> (usize, usize) {
    let mut agregadas = 0;
    let mut borradas = 0;
    for f in filas {
        if let FilaDiff::Par { izq, der } = f {
            if izq.as_ref().is_some_and(|c| c.tipo != Tipo::Igual) {
                borradas += 1;
            }
            if der.as_ref().is_some_and(|c| c.tipo != Tipo::Igual) {
                agregadas += 1;
            }
        }
    }
    (agregadas, borradas)
}

/// Índices de las filas donde empieza cada grupo de cambios (para saltar con n/p).
pub fn inicios_de_cambios(filas: &[FilaDiff]) -> Vec<usize> {
    let mut v = vec![];
    let mut anterior = false;
    for (i, f) in filas.iter().enumerate() {
        let c = f.es_cambio();
        if c && !anterior {
            v.push(i);
        }
        anterior = c;
    }
    v
}

/// Tramos que difieren entre dos líneas, por caracteres.
/// Tramos [ini, fin) de caracteres.
type Tramos = Vec<(usize, usize)>;

fn resaltes(a: &str, b: &str) -> (Tramos, Tramos) {
    // Líneas muy distintas: resaltar todo no aporta; se deja sin resaltes.
    let d = TextDiff::from_chars(a, b);
    if d.ratio() < 0.3 {
        return (vec![], vec![]);
    }
    let (mut ra, mut rb) = (vec![], vec![]);
    for op in d.ops() {
        let (tag, va, vb) = op.as_tag_tuple();
        match tag {
            DiffTag::Equal => {}
            DiffTag::Delete => ra.push((va.start, va.end)),
            DiffTag::Insert => rb.push((vb.start, vb.end)),
            DiffTag::Replace => {
                ra.push((va.start, va.end));
                rb.push((vb.start, vb.end));
            }
        }
    }
    (ra, rb)
}

/// Agrega a `filas` un bloque de borradas y agregadas consecutivas, emparejadas.
fn volcar(
    filas: &mut Vec<FilaDiff>,
    borradas: &mut Vec<(usize, String)>,
    agregadas: &mut Vec<(usize, String)>,
) {
    let n = borradas.len().max(agregadas.len());
    for i in 0..n {
        let b = borradas.get(i).cloned();
        let a = agregadas.get(i).cloned();
        let par = b.is_some() && a.is_some();
        let (rb, ra) = match (&b, &a) {
            (Some((_, tb)), Some((_, ta))) => resaltes(tb, ta),
            _ => (vec![], vec![]),
        };
        filas.push(FilaDiff::Par {
            izq: b.map(|(num, texto)| Celda {
                num,
                texto,
                tipo: if par { Tipo::Cambiada } else { Tipo::Borrada },
                resaltes: rb,
            }),
            der: a.map(|(num, texto)| Celda {
                num,
                texto,
                tipo: if par { Tipo::Cambiada } else { Tipo::Agregada },
                resaltes: ra,
            }),
        });
    }
    borradas.clear();
    agregadas.clear();
}

fn sin_salto(s: &str) -> String {
    s.trim_end_matches(['\n', '\r']).replace('\t', "    ")
}

/// Diff entre dos versiones, con `contexto` líneas iguales alrededor de cada cambio.
pub fn comparar(viejo: &str, nuevo: &str, contexto: usize) -> Vec<FilaDiff> {
    let d = TextDiff::from_lines(viejo, nuevo);
    let mut filas = vec![];
    for grupo in d.grouped_ops(contexto) {
        let (Some(primero), Some(ultimo)) = (grupo.first(), grupo.last()) else {
            continue;
        };
        let (vi, ni) = (primero.old_range().start, primero.new_range().start);
        let (vf, nf) = (ultimo.old_range().end, ultimo.new_range().end);
        filas.push(FilaDiff::Bloque(format!(
            "@@ -{},{} +{},{} @@",
            vi + 1,
            vf - vi,
            ni + 1,
            nf - ni
        )));
        let (mut borradas, mut agregadas) = (vec![], vec![]);
        for op in &grupo {
            for c in d.iter_changes(op) {
                match c.tag() {
                    ChangeTag::Delete => {
                        borradas.push((c.old_index().unwrap_or(0) + 1, sin_salto(c.value())))
                    }
                    ChangeTag::Insert => {
                        agregadas.push((c.new_index().unwrap_or(0) + 1, sin_salto(c.value())))
                    }
                    ChangeTag::Equal => {
                        volcar(&mut filas, &mut borradas, &mut agregadas);
                        let texto = sin_salto(c.value());
                        let celda = |num: usize| Celda {
                            num,
                            texto: texto.clone(),
                            tipo: Tipo::Igual,
                            resaltes: vec![],
                        };
                        filas.push(FilaDiff::Par {
                            izq: Some(celda(c.old_index().unwrap_or(0) + 1)),
                            der: Some(celda(c.new_index().unwrap_or(0) + 1)),
                        });
                    }
                }
            }
        }
        volcar(&mut filas, &mut borradas, &mut agregadas);
    }
    filas
}

/// true si el contenido parece binario (tiene bytes nulos en el primer bloque).
pub fn es_binario(datos: &[u8]) -> bool {
    datos.iter().take(8000).any(|b| *b == 0)
}

/// Interpreta un diff unificado de git (uno o varios archivos). Las líneas que no son
/// del diff (encabezado de un commit) se ignoran.
pub fn desde_unificado(texto: &str) -> Vec<FilaDiff> {
    let mut filas = vec![];
    let (mut borradas, mut agregadas) = (vec![], vec![]);
    let (mut nv, mut nn) = (0usize, 0usize);
    let mut en_bloque = false;
    for linea in texto.lines() {
        if let Some(resto) = linea.strip_prefix("diff --git ") {
            volcar(&mut filas, &mut borradas, &mut agregadas);
            en_bloque = false;
            // «a/viejo b/nuevo»: con nombres distintos es un renombre.
            let nuevo = resto.split(" b/").last().unwrap_or(resto).to_string();
            let viejo = resto
                .strip_prefix("a/")
                .and_then(|r| r.split(" b/").next())
                .unwrap_or(&nuevo)
                .to_string();
            filas.push(FilaDiff::Archivo(if viejo != nuevo {
                format!("{viejo} → {nuevo}")
            } else {
                nuevo
            }));
            continue;
        }
        if linea.starts_with("Binary files ") || linea.starts_with("Los archivos binarios ") {
            filas.push(FilaDiff::Bloque(
                "archivo binario: no se compara como texto".into(),
            ));
            continue;
        }
        if linea.starts_with("@@") {
            volcar(&mut filas, &mut borradas, &mut agregadas);
            // @@ -a,b +c,d @@ contexto
            let mut partes = linea.split_whitespace().skip(1);
            let leer = |p: Option<&str>| {
                p.and_then(|x| x[1..].split(',').next())
                    .and_then(|n| n.parse::<usize>().ok())
                    .unwrap_or(1)
            };
            nv = leer(partes.next());
            nn = leer(partes.next());
            en_bloque = true;
            filas.push(FilaDiff::Bloque(linea.to_string()));
            continue;
        }
        if !en_bloque {
            continue;
        }
        match linea.chars().next() {
            Some('-') => {
                borradas.push((nv, sin_salto(&linea[1..])));
                nv += 1;
            }
            Some('+') => {
                agregadas.push((nn, sin_salto(&linea[1..])));
                nn += 1;
            }
            Some(' ') | None => {
                volcar(&mut filas, &mut borradas, &mut agregadas);
                let texto = sin_salto(linea.get(1..).unwrap_or(""));
                filas.push(FilaDiff::Par {
                    izq: Some(Celda {
                        num: nv,
                        texto: texto.clone(),
                        tipo: Tipo::Igual,
                        resaltes: vec![],
                    }),
                    der: Some(Celda {
                        num: nn,
                        texto,
                        tipo: Tipo::Igual,
                        resaltes: vec![],
                    }),
                });
                nv += 1;
                nn += 1;
            }
            // «\ No newline at end of file» y otras marcas.
            _ => {}
        }
    }
    volcar(&mut filas, &mut borradas, &mut agregadas);
    filas
}

#[cfg(test)]
mod tests {
    use super::*;

    type Lado = Option<(usize, Tipo)>;

    fn pares(f: &[FilaDiff]) -> Vec<(Lado, Lado)> {
        f.iter()
            .filter_map(|x| match x {
                FilaDiff::Par { izq, der } => Some((
                    izq.as_ref().map(|c| (c.num, c.tipo)),
                    der.as_ref().map(|c| (c.num, c.tipo)),
                )),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn compara_y_empareja_cambios() {
        let viejo = "int main(void)\n{\n    int x = 1;\n    return x;\n}\n";
        let nuevo = "int main(void)\n{\n    int x = 2;\n    int y = 3;\n    return x;\n}\n";
        let f = comparar(viejo, nuevo, 1);
        assert!(matches!(&f[0], FilaDiff::Bloque(b) if b.starts_with("@@ -2,")));
        assert_eq!(
            pares(&f),
            vec![
                (Some((2, Tipo::Igual)), Some((2, Tipo::Igual))),
                (Some((3, Tipo::Cambiada)), Some((3, Tipo::Cambiada))),
                (None, Some((4, Tipo::Agregada))),
                (Some((4, Tipo::Igual)), Some((5, Tipo::Igual))),
            ]
        );
        // El resalte marca sólo el 1 → 2.
        let FilaDiff::Par {
            izq: Some(i),
            der: Some(d),
        } = &f[2]
        else {
            panic!()
        };
        assert_eq!(i.resaltes, vec![(12, 13)]);
        assert_eq!(d.resaltes, vec![(12, 13)]);
        assert_eq!(estadistica(&f), (2, 1));
        assert_eq!(inicios_de_cambios(&f), vec![2]);
    }

    #[test]
    fn sin_cambios_no_hay_filas() {
        assert!(comparar("a\nb\n", "a\nb\n", 3).is_empty());
    }

    #[test]
    fn renombres_y_binarios() {
        let t = "diff --git a/lista.c b/listas.c\nsimilarity index 90%\nrename from lista.c\nrename to listas.c\n@@ -1 +1 @@\n-int a;\n+int b;\ndiff --git a/foto.png b/foto.png\nBinary files a/foto.png and b/foto.png differ\n";
        let f = desde_unificado(t);
        assert_eq!(f[0], FilaDiff::Archivo("lista.c → listas.c".into()));
        assert!(f.contains(&FilaDiff::Bloque(
            "archivo binario: no se compara como texto".into()
        )));
        assert!(es_binario(b"PNG\0\x01"));
        assert!(!es_binario(b"int main(void);\n"));
    }

    #[test]
    fn interpreta_git_show() {
        let show = "commit abc123\nAuthor: x\n\n    mensaje\n\ndiff --git a/main.c b/main.c\nindex 1..2 100644\n--- a/main.c\n+++ b/main.c\n@@ -1,3 +1,3 @@\n int main(void)\n {\n-    return 1;\n+    return 0;\n }\ndiff --git a/nuevo.h b/nuevo.h\n--- /dev/null\n+++ b/nuevo.h\n@@ -0,0 +1,1 @@\n+int f(void);\n";
        let f = desde_unificado(show);
        assert_eq!(f[0], FilaDiff::Archivo("main.c".into()));
        assert!(f.iter().any(|x| *x == FilaDiff::Archivo("nuevo.h".into())));
        let p = pares(&f);
        assert_eq!(p[2], (Some((3, Tipo::Cambiada)), Some((3, Tipo::Cambiada))));
        assert_eq!(p.last().unwrap(), &(None, Some((1, Tipo::Agregada))));
        assert_eq!(estadistica(&f), (2, 1));
    }
}
