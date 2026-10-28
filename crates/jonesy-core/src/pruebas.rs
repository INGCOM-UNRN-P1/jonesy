//! Resultado de `make test` con la biblioteca de pruebas de la cátedra (`p1_test`).
//!
//! `p1_test` informa cada aserción que falla como
//!
//! ```text
//!   [FALLO] prueba.c:13: ASSERT_INT_EQ(5, suma(2, 3))
//!     en test: suma_con_dos
//!     esperado: igual a 5
//!     obtenido: 6
//! ```
//!
//! y un corte como `[CRASH: SIGSEGV (…)]` seguido de `en test: nombre`. Cada uno se
//! convierte en un diagnóstico en la línea de la prueba, con lo esperado y lo obtenido.

use std::path::{Path, PathBuf};

use crate::diagnostico::{directorio_de_make, Diagnostico, Origen, Severidad};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Resultado {
    pub pasan: usize,
    pub fallan: usize,
    /// Fallos y cortes, con su ubicación (más los errores de compilación de la salida).
    pub diagnosticos: Vec<Diagnostico>,
    /// Se encontró la salida de `p1_test` (si no, quizás las pruebas usan otra cosa).
    pub reconocida: bool,
    /// Cada prueba que corrió y si pasó (para marcar ✓/✗ junto a su `TEST`).
    pub por_prueba: Vec<(String, bool)>,
}

/// Saca las secuencias de color ANSI (por si la biblioteca no respeta NO_COLOR).
fn sin_color(s: &str) -> String {
    let mut r = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\u{1b}' && it.peek() == Some(&'[') {
            for d in it.by_ref() {
                if d.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        r.push(c);
    }
    r
}

/// Línea donde se define la prueba `TEST(nombre)` en los `.c` de `dir`.
fn linea_de_prueba(dir: &Path, nombre: &str) -> Option<(PathBuf, usize)> {
    let buscado = format!("TEST({nombre})");
    let mut archivos: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "c"))
        .collect();
    archivos.sort();
    archivos.into_iter().find_map(|p| {
        let texto = std::fs::read_to_string(&p).ok()?;
        let l = texto
            .lines()
            .position(|l| l.replace(' ', "").contains(&buscado))?;
        Some((p, l + 1))
    })
}

fn numero_tras(linea: &str, rotulo: &str) -> Option<usize> {
    let resto = linea.trim().strip_prefix(rotulo)?;
    resto
        .trim_start_matches([' ', ':'])
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

/// Nombre de la prueba `TEST(nombre)` (o `RUN_TEST(nombre)`) donde está la línea `linea`
/// (0-based) de `texto`: la definición más cercana hacia arriba.
pub fn prueba_en(texto: &str, linea: usize) -> Option<String> {
    let lineas: Vec<&str> = texto.lines().collect();
    let extraer = |l: &str, pre: &str| -> Option<String> {
        let i = l.find(pre)?;
        let resto = &l[i + pre.len()..];
        let n: String = resto
            .trim_start()
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        (!n.is_empty()).then_some(n)
    };
    if let Some(n) = lineas.get(linea).and_then(|l| extraer(l, "RUN_TEST(")) {
        return Some(n);
    }
    (0..=linea.min(lineas.len().saturating_sub(1)))
        .rev()
        .find_map(|i| {
            let l = lineas[i].trim_start();
            l.starts_with("TEST(")
                .then(|| extraer(l, "TEST("))
                .flatten()
        })
}

/// Nombre del ejecutable de pruebas de un Makefile (`TEST_TARGET = …`; si no, `test_bin`).
pub fn binario_de_pruebas(makefile: &str) -> String {
    makefile
        .lines()
        .find_map(|l| {
            let (k, v) = l.split_once('=')?;
            (k.trim_end_matches([' ', '?', ':', '\t']).trim() == "TEST_TARGET")
                .then(|| v.trim().to_string())
        })
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "test_bin".into())
}

/// Resultado de pruebas que informan en SARIF (otros frameworks): el documento puede estar
/// en la salida o en `archivo` (relativo a `dir`), que sólo cuenta si se escribió después
/// de `desde` (para no leer el de una corrida anterior).
pub fn desde_sarif(
    salida: &str,
    archivo: Option<&Path>,
    desde: Option<std::time::SystemTime>,
    dir: &Path,
) -> Option<Resultado> {
    let doc = archivo
        .map(|a| dir.join(a))
        .filter(|a| {
            let nuevo = std::fs::metadata(a).and_then(|m| m.modified()).ok();
            match (desde, nuevo) {
                (Some(d), Some(n)) => n >= d,
                (None, Some(_)) => true,
                _ => false,
            }
        })
        .and_then(|a| std::fs::read_to_string(a).ok())
        .and_then(|t| crate::sarif::buscar(&t))
        .or_else(|| crate::sarif::buscar(salida))?;
    let s = crate::sarif::interpretar(&doc, dir);
    let mut diagnosticos = crate::diagnostico::parsear_gcc(salida, dir);
    diagnosticos.extend(s.diagnosticos);
    Some(Resultado {
        pasan: s.pasan,
        fallan: s.fallan,
        diagnosticos,
        reconocida: true,
        por_prueba: vec![],
    })
}

/// Interpreta la salida de `make test` corrida desde `dir` (la raíz del proyecto).
pub fn interpretar(salida: &str, dir: &Path) -> Resultado {
    let salida = sin_color(salida);
    let mut r = Resultado {
        diagnosticos: crate::diagnostico::parsear_gcc(&salida, dir),
        ..Default::default()
    };
    let (mut pasan_lineas, mut fallan_lineas) = (0, 0);
    let (mut pasan_resumen, mut fallan_resumen, mut con_resumen) = (0, 0, false);
    let mut pila: Vec<PathBuf> = vec![dir.to_path_buf()];
    // Fallo en curso: se completa con las líneas indentadas que lo siguen.
    let mut actual: Option<Diagnostico> = None;
    // Prueba que empezó y todavía no dijo cómo terminó.
    let mut corriendo: Option<String> = None;
    let mut detalles: Vec<String> = vec![];
    let cerrar =
        |actual: &mut Option<Diagnostico>, detalles: &mut Vec<String>, r: &mut Resultado| {
            if let Some(mut d) = actual.take() {
                if !detalles.is_empty() {
                    d.resumen = Some(format!(
                        "{} — {}",
                        d.resumen.take().unwrap_or_default(),
                        detalles.join(", ")
                    ));
                }
                r.diagnosticos.push(d);
            }
            detalles.clear();
        };
    for linea in salida.lines() {
        if let Some(d) = directorio_de_make(linea, "Entering directory")
            .or_else(|| directorio_de_make(linea, "Se entra en el directorio"))
        {
            pila.push(d);
            continue;
        }
        if directorio_de_make(linea, "Leaving directory")
            .or_else(|| directorio_de_make(linea, "Se sale del directorio"))
            .is_some()
        {
            if pila.len() > 1 {
                pila.pop();
            }
            continue;
        }
        let base = pila.last().unwrap().clone();
        let t = linea.trim();
        if let Some(resto) = t.strip_prefix("[FALLO]") {
            cerrar(&mut actual, &mut detalles, &mut r);
            r.reconocida = true;
            // «prueba.c:13: ASSERT_INT_EQ(5, suma(2, 3))»
            let mut partes = resto.trim().splitn(3, ':');
            let archivo = partes.next().unwrap_or("").trim();
            let num = partes.next().and_then(|n| n.trim().parse().ok());
            let expr = partes.next().unwrap_or("").trim().to_string();
            actual = Some(Diagnostico {
                archivo: (!archivo.is_empty()).then(|| {
                    crate::valgrind::ubicar(archivo, &base, dir)
                        .unwrap_or_else(|| base.join(archivo))
                }),
                linea: num.unwrap_or(1),
                columna: 1,
                severidad: Severidad::Error,
                mensaje: expr,
                resumen: Some("La prueba falló".into()),
                origen: Origen::Herramienta,
                herramienta: Some("pruebas".into()),
                ..Default::default()
            });
        } else if let Some(resto) = t.strip_prefix("[CRASH:") {
            cerrar(&mut actual, &mut detalles, &mut r);
            r.reconocida = true;
            let senal = resto.trim_end_matches(']').trim().to_string();
            let nombre = senal.split_whitespace().next().unwrap_or("").to_string();
            let titulo = crate::fallos::de_senal(&nombre)
                .map(|f| f.titulo.to_string())
                .unwrap_or_else(|| senal.clone());
            actual = Some(Diagnostico {
                archivo: None,
                linea: 1,
                columna: 1,
                severidad: Severidad::Error,
                mensaje: format!("[CRASH: {senal}]"),
                resumen: Some(format!("La prueba se cortó: {titulo}")),
                origen: Origen::Herramienta,
                herramienta: Some("pruebas".into()),
                ..Default::default()
            });
        } else if let (Some(d), Some(nombre)) = (&mut actual, t.strip_prefix("en test:")) {
            let nombre = nombre.trim();
            d.resumen = Some(format!(
                "{} «{nombre}»",
                d.resumen.take().unwrap_or_default()
            ));
            // Un corte no trae línea: se ubica en la definición de la prueba.
            if d.archivo.is_none() {
                if let Some((a, l)) = linea_de_prueba(&base, nombre) {
                    d.archivo = Some(a);
                    d.linea = l;
                }
            }
        } else if actual.is_some()
            && [
                "en subcaso:",
                "mensaje:",
                "esperado:",
                "obtenido:",
                "discrepancia",
            ]
            .iter()
            .any(|p| t.starts_with(p))
        {
            detalles.push(t.trim_end_matches(':').to_string());
        } else if let Some(resto) = t.strip_prefix("[CORRIENDO]") {
            cerrar(&mut actual, &mut detalles, &mut r);
            r.reconocida = true;
            let nombre = resto.split_whitespace().next().unwrap_or("").to_string();
            if t.contains("PASÓ") {
                pasan_lineas += 1;
                r.por_prueba.push((nombre, true));
            } else if t.contains("FALLÓ") {
                r.por_prueba.push((nombre, false));
            } else {
                corriendo = Some(nombre);
            }
        } else if t.starts_with("[RESULTADO]") {
            cerrar(&mut actual, &mut detalles, &mut r);
            if t.contains("FALLÓ") {
                fallan_lineas += 1;
            }
            if let Some(n) = corriendo.take() {
                r.por_prueba.push((n, !t.contains("FALLÓ")));
            }
        } else if let Some(n) = numero_tras(t, "Tests aprobados") {
            cerrar(&mut actual, &mut detalles, &mut r);
            pasan_resumen += n;
            con_resumen = true;
        } else if let Some(n) = numero_tras(t, "Tests desaprobados") {
            fallan_resumen += n;
            con_resumen = true;
        } else if !t.is_empty()
            && !linea.starts_with(' ')
            && !linea.starts_with('\t')
            && !t.starts_with("make")
        {
            cerrar(&mut actual, &mut detalles, &mut r);
        }
    }
    cerrar(&mut actual, &mut detalles, &mut r);
    // El resumen de cada suite es lo más confiable (stdout y stderr pueden mezclarse).
    (r.pasan, r.fallan) = if con_resumen {
        (pasan_resumen, fallan_resumen)
    } else {
        (pasan_lineas, fallan_lineas)
    };
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    const SALIDA: &str = include_str!("../tests/datos/p1_test_salida.txt");

    #[test]
    fn fallos_cortes_y_cuentas() {
        let d = tempfile::tempdir().unwrap();
        let modulo = d.path().join("ejercicios/ej1");
        std::fs::create_dir_all(&modulo).unwrap();
        std::fs::write(
            modulo.join("prueba.c"),
            "#include \"p1_test.h\"\n\nTEST(cadena_nula)\n{\n}\n",
        )
        .unwrap();
        let salida = format!(
            "make: Entering directory '{}'\n{SALIDA}make: Leaving directory '{}'\n",
            modulo.display(),
            modulo.display()
        );
        let r = interpretar(&salida, d.path());
        assert!(r.reconocida);
        assert_eq!((r.pasan, r.fallan), (1, 3));
        assert_eq!(r.diagnosticos.len(), 3, "{:#?}", r.diagnosticos);
        assert_eq!(
            r.por_prueba,
            vec![
                ("suma_basica".to_string(), true),
                ("suma_con_dos".to_string(), false),
                ("cadena_nula".to_string(), false),
                ("cadenas".to_string(), false),
            ]
        );
        // El resultado en la misma línea (otras bibliotecas que imitan a p1_test).
        let r2 = interpretar(
            "[CORRIENDO] a ... PASÓ\n[CORRIENDO] b ... FALLÓ\n",
            d.path(),
        );
        assert_eq!(
            r2.por_prueba,
            vec![("a".to_string(), true), ("b".to_string(), false)]
        );

        let f = &r.diagnosticos[0];
        assert_eq!(
            f.archivo.as_deref(),
            Some(modulo.join("prueba.c").as_path())
        );
        assert_eq!(f.linea, 13);
        assert_eq!(f.mensaje, "ASSERT_INT_EQ(5, suma(2, 3))");
        assert_eq!(
            f.resumen.as_deref(),
            Some("La prueba falló «suma_con_dos» — esperado: igual a 5, obtenido: 6")
        );

        // El corte se ubica en la definición de la prueba.
        let c = &r.diagnosticos[1];
        assert_eq!(c.linea, 3);
        assert!(c
            .resumen
            .as_deref()
            .unwrap()
            .contains("Violación de segmento"));
        assert!(c.resumen.as_deref().unwrap().contains("«cadena_nula»"));

        assert!(r.diagnosticos[2]
            .resumen
            .as_deref()
            .unwrap()
            .contains("obtenido: \"chau\""));
    }

    #[test]
    fn prueba_del_cursor_y_binario() {
        let t = "#include \"p1_test.h\"\n\nTEST(suma)\n{\n    ASSERT_TRUE(1);\n}\n\nTEST( resta )\n{\n}\n\nint main(void)\n{\n    RUN_TEST(suma);\n}\n";
        assert_eq!(prueba_en(t, 4).as_deref(), Some("suma"));
        assert_eq!(prueba_en(t, 8).as_deref(), Some("resta"));
        assert_eq!(prueba_en(t, 13).as_deref(), Some("suma"), "sobre RUN_TEST");
        assert_eq!(prueba_en(t, 0), None);
        assert_eq!(
            binario_de_pruebas("CC ?= gcc\nTEST_TARGET = pruebas\n"),
            "pruebas"
        );
        assert_eq!(binario_de_pruebas("all:\n"), "test_bin");
    }

    #[test]
    fn todas_pasan_y_salida_ajena() {
        let ok = "  [CORRIENDO] a ... PASÓ (0.01 ms)\n  [CORRIENDO] b ... \u{1b}[1;32mPASÓ\u{1b}[0m (0.01 ms)\n";
        let r = interpretar(ok, Path::new("/tp"));
        assert_eq!((r.pasan, r.fallan), (2, 0));
        assert!(r.diagnosticos.is_empty());
        let otra = interpretar("ok 1 - algo\n", Path::new("/tp"));
        assert!(!otra.reconocida);
    }
}
