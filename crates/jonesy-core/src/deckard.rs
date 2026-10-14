//! Consignas de deckard (la herramienta de la cátedra que arma los ejercicios y guías).
//!
//! Un ejercicio llega al alumno de dos formas:
//!
//! - la carpeta del banco: `ejercicio.yaml` (título, tema, nivel de Bloom, minutos,
//!   pistas, funciones), `enunciado.md`, `tests/*.in` ↔ `*.out` y `solucion.c` (que es
//!   del docente: jonesy no la abre por su cuenta);
//! - el starter kit (`deckard` starter_zip): `metadata.json`, `README.md` con el
//!   enunciado y las pistas en `<details>`, `{id}.c` y `Makefile`.
//!
//! jonesy arma con eso la consigna: una cabecera con el tema, el nivel y el tiempo
//! estimado, el enunciado, las funciones a implementar y las pistas de a una, cuando se
//! piden.

use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Funcion {
    pub nombre: String,
    pub retorno: String,
    pub parametros: String,
    pub descripcion: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ejercicio {
    pub dir: PathBuf,
    pub id: String,
    pub titulo: String,
    pub tema: String,
    /// Nivel de Bloom (1 recordar … 5 evaluar).
    pub bloom: Option<u8>,
    pub minutos: Option<u32>,
    pub enunciado: String,
    pub pistas: Vec<String>,
    pub funciones: Vec<Funcion>,
    pub clave_entrega: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Yaml {
    id: String,
    titulo: String,
    tema: String,
    bloom: Option<serde_yaml::Value>,
    minutos_estimados: Option<u32>,
    minutos: Option<u32>,
    enunciado: Option<String>,
    enunciado_md: Option<String>,
    pistas: Vec<String>,
    funciones: Vec<FuncionYaml>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct FuncionYaml {
    nombre: String,
    retorno: Option<String>,
    parametros: Option<String>,
    descripcion: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Metadata {
    ejercicio_id: String,
    titulo: String,
    tema: String,
    bloom: Option<serde_json::Value>,
    minutos_estimados: Option<u32>,
    clave_entrega: Option<String>,
    generator: String,
}

const NIVELES: [&str; 5] = ["Recordar", "Comprender", "Aplicar", "Analizar", "Evaluar"];

/// El nivel de Bloom como número: `3`, `"APLICAR"` o `"aplicar"`.
fn nivel(v: Option<String>) -> Option<u8> {
    let s = v?.trim().to_lowercase();
    s.parse::<u8>()
        .ok()
        .filter(|n| (1..=5).contains(n))
        .or_else(|| {
            NIVELES
                .iter()
                .position(|n| n.to_lowercase() == s)
                .map(|i| i as u8 + 1)
        })
}

pub fn nombre_bloom(n: u8) -> &'static str {
    NIVELES
        .get(n.saturating_sub(1) as usize)
        .copied()
        .unwrap_or("?")
}

/// Lee el ejercicio de deckard de `dir`, si hay uno.
pub fn leer(dir: &Path) -> Option<Ejercicio> {
    let yaml = dir.join("ejercicio.yaml");
    if yaml.is_file() {
        let y: Yaml = serde_yaml::from_str(&std::fs::read_to_string(&yaml).ok()?).ok()?;
        if y.id.is_empty() && y.titulo.is_empty() {
            return None;
        }
        let enunciado = std::fs::read_to_string(dir.join("enunciado.md"))
            .ok()
            .or(y.enunciado_md)
            .or(y.enunciado)
            .unwrap_or_default();
        let bloom = y.bloom.and_then(|b| match b {
            serde_yaml::Value::Number(n) => n.as_u64().map(|n| n.to_string()),
            serde_yaml::Value::String(s) => Some(s),
            _ => None,
        });
        return Some(Ejercicio {
            dir: dir.to_path_buf(),
            id: y.id,
            titulo: y.titulo,
            tema: y.tema,
            bloom: nivel(bloom),
            minutos: y.minutos_estimados.or(y.minutos),
            enunciado,
            pistas: y
                .pistas
                .into_iter()
                .filter(|p| !p.trim().is_empty())
                .collect(),
            funciones: y
                .funciones
                .into_iter()
                .filter(|f| !f.nombre.is_empty())
                .map(|f| Funcion {
                    nombre: f.nombre,
                    retorno: f.retorno.unwrap_or_else(|| "void".into()),
                    parametros: f.parametros.unwrap_or_default(),
                    descripcion: f.descripcion.unwrap_or_default(),
                })
                .collect(),
            clave_entrega: None,
        });
    }
    let meta = dir.join("metadata.json");
    if meta.is_file() {
        let m: Metadata = serde_json::from_str(&std::fs::read_to_string(&meta).ok()?).ok()?;
        if m.ejercicio_id.is_empty() || !m.generator.to_lowercase().contains("deckard") {
            return None;
        }
        let readme = std::fs::read_to_string(dir.join("README.md")).unwrap_or_default();
        let (enunciado, pistas) = partir_readme(&readme);
        let bloom = m.bloom.and_then(|b| match b {
            serde_json::Value::Number(n) => n.as_u64().map(|n| n.to_string()),
            serde_json::Value::String(s) => Some(s),
            _ => None,
        });
        return Some(Ejercicio {
            dir: dir.to_path_buf(),
            id: m.ejercicio_id,
            titulo: m.titulo,
            tema: m.tema,
            bloom: nivel(bloom),
            minutos: m.minutos_estimados,
            enunciado,
            pistas,
            funciones: vec![],
            clave_entrega: m.clave_entrega,
        });
    }
    None
}

/// El enunciado (la sección «## Enunciado», o todo salvo las pistas y las instrucciones)
/// y las pistas (`<details><summary>Pista N</summary> … </details>`) del README del
/// starter kit.
fn partir_readme(readme: &str) -> (String, Vec<String>) {
    let mut pistas = vec![];
    let mut resto = vec![];
    let mut en_pista: Option<Vec<&str>> = None;
    for l in readme.lines() {
        let t = l.trim();
        if t.starts_with("<details>") {
            en_pista = Some(vec![]);
            continue;
        }
        if t.starts_with("</details>") {
            if let Some(p) = en_pista.take() {
                let texto = p.join("\n").trim().to_string();
                if !texto.is_empty() {
                    pistas.push(texto);
                }
            }
            continue;
        }
        match &mut en_pista {
            Some(p) => {
                if !t.starts_with("<summary>") {
                    p.push(l);
                }
            }
            None => resto.push(l),
        }
    }
    // Del resto, la sección del enunciado (sin el título, los datos ni las instrucciones).
    let texto = resto.join("\n");
    let enunciado = match texto.find("## Enunciado") {
        Some(i) => {
            let cuerpo = &texto[i + "## Enunciado".len()..];
            let fin = cuerpo.find("\n## ").unwrap_or(cuerpo.len());
            cuerpo[..fin].trim().to_string()
        }
        None => texto.trim().to_string(),
    };
    (enunciado, pistas)
}

/// El ejercicio de deckard que contiene a `ruta` (subiendo hasta `raiz`).
pub fn buscar_desde(ruta: &Path, raiz: &Path) -> Option<Ejercicio> {
    let mut dir = if ruta.is_dir() {
        Some(ruta)
    } else {
        ruta.parent()
    };
    while let Some(d) = dir {
        if let Some(e) = leer(d) {
            return Some(e);
        }
        if d == raiz || !d.starts_with(raiz) {
            break;
        }
        dir = d.parent();
    }
    None
}

/// La consigna en Markdown, con las primeras `vistas` pistas a la vista.
pub fn markdown(e: &Ejercicio, vistas: usize) -> String {
    let mut s = format!(
        "# {}\n\n",
        if e.titulo.is_empty() {
            &e.id
        } else {
            &e.titulo
        }
    );
    let mut datos = vec![];
    if !e.tema.is_empty() {
        datos.push(format!("**Tema:** {}", e.tema));
    }
    if let Some(b) = e.bloom {
        datos.push(format!("**Nivel:** {} ({b} de 5)", nombre_bloom(b)));
    }
    if let Some(m) = e.minutos {
        datos.push(format!("**Tiempo estimado:** {m} min"));
    }
    if !datos.is_empty() {
        s.push_str(&format!("> {}\n\n", datos.join(" · ")));
    }
    // El enunciado sin repetir el título.
    let enunciado = e.enunciado.trim();
    let enunciado = match enunciado.strip_prefix('#') {
        Some(r)
            if r.trim_start_matches('#')
                .trim_start()
                .starts_with(e.titulo.as_str())
                && !e.titulo.is_empty() =>
        {
            r.split_once('\n').map(|x| x.1).unwrap_or("").trim()
        }
        _ => enunciado,
    };
    s.push_str(enunciado);
    s.push('\n');
    if !e.funciones.is_empty() {
        s.push_str("\n## Funciones a implementar\n\n");
        for f in &e.funciones {
            s.push_str(&format!("- `{} {}({})`", f.retorno, f.nombre, f.parametros));
            if !f.descripcion.is_empty() {
                s.push_str(&format!(": {}", f.descripcion));
            }
            s.push('\n');
        }
    }
    if !e.pistas.is_empty() {
        let n = e.pistas.len();
        s.push_str(&format!("\n## Pistas ({} de {n})\n\n", vistas.min(n)));
        for (i, p) in e.pistas.iter().take(vistas).enumerate() {
            s.push_str(&format!("{}. {}\n", i + 1, p.trim()));
        }
        if vistas < n {
            s.push_str(&format!(
                "\nQuedan {} pista(s): **p** en la consigna (o Ayuda → Ver la siguiente pista) muestra otra. Probá un rato antes de pedirla.\n",
                n - vistas
            ));
        }
    }
    if let Some(c) = &e.clave_entrega {
        s.push_str(&format!("\n**Clave de entrega:** `{c}`\n"));
    }
    s
}

/// El archivo para empezar a trabajar: el `{id}.c` del starter, `starter.c` o `main.c`
/// (nunca `solucion.c`, que es del docente).
pub fn archivo_inicial(e: &Ejercicio) -> Option<PathBuf> {
    [
        format!("{}.c", e.id),
        "starter.c".to_string(),
        "main.c".to_string(),
    ]
    .into_iter()
    .map(|n| e.dir.join(n))
    .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ejercicio_del_banco() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            d.path().join("ejercicio.yaml"),
            "id: suma-vector\ntitulo: Sumar un vector\ntema: arreglos\nbloom: 3\nminutos_estimados: 20\npistas:\n- Recorré el vector una vez,\n  acumulando.\n- Ojo con n == 0.\nfunciones:\n- nombre: sumar\n  retorno: int\n  parametros: const int *v, size_t n\n",
        )
        .unwrap();
        std::fs::write(
            d.path().join("enunciado.md"),
            "# Sumar un vector\n\nDevolver la suma.\n",
        )
        .unwrap();
        std::fs::write(
            d.path().join("solucion.c"),
            "int main(void) { return 0; }\n",
        )
        .unwrap();
        let e = leer(d.path()).unwrap();
        assert_eq!(
            (e.id.as_str(), e.bloom, e.minutos),
            ("suma-vector", Some(3), Some(20))
        );
        assert_eq!(
            e.pistas,
            vec!["Recorré el vector una vez, acumulando.", "Ojo con n == 0."]
        );
        let md = markdown(&e, 0);
        assert!(md.starts_with("# Sumar un vector\n\n> **Tema:** arreglos · **Nivel:** Aplicar (3 de 5) · **Tiempo estimado:** 20 min\n\nDevolver la suma."), "{md}");
        assert!(md.contains("- `int sumar(const int *v, size_t n)`"), "{md}");
        assert!(
            md.contains("## Pistas (0 de 2)") && !md.contains("Ojo con"),
            "{md}"
        );
        let md = markdown(&e, 1);
        assert!(
            md.contains("1. Recorré el vector") && md.contains("Quedan 1 pista"),
            "{md}"
        );
        assert_eq!(archivo_inicial(&e), None, "nunca solucion.c");
        // Desde un archivo de una subcarpeta.
        std::fs::create_dir_all(d.path().join("tests")).unwrap();
        assert!(buscar_desde(&d.path().join("tests/caso_01.in"), d.path()).is_some());
    }

    #[test]
    fn starter_kit() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            d.path().join("metadata.json"),
            r#"{"ejercicio_id": "invertir", "titulo": "Invertir", "tema": "arreglos", "bloom": "APLICAR", "minutos_estimados": 25, "clave_entrega": "DKD-1234-5678-9ABC", "generator": "Deckard Cátedra Toolchain"}"#,
        )
        .unwrap();
        std::fs::write(
            d.path().join("README.md"),
            "# Invertir\n\n- **Tema:** `arreglos`\n\n## Enunciado\n\nLeer N enteros e imprimirlos al revés.\n\n## Pistas de Resolución\n\n<details><summary>Pista 1</summary>\n\nEl último índice es n - 1.\n</details>\n\n## Instrucciones de Compilación y Entrega\n\nmake test\n",
        )
        .unwrap();
        std::fs::write(d.path().join("invertir.c"), "").unwrap();
        let e = leer(d.path()).unwrap();
        assert_eq!(e.bloom, Some(3));
        assert_eq!(e.enunciado, "Leer N enteros e imprimirlos al revés.");
        assert_eq!(e.pistas, vec!["El último índice es n - 1."]);
        assert_eq!(e.clave_entrega.as_deref(), Some("DKD-1234-5678-9ABC"));
        assert_eq!(archivo_inicial(&e), Some(d.path().join("invertir.c")));
        assert!(markdown(&e, 0).contains("**Clave de entrega:** `DKD-1234-5678-9ABC`"));
    }

    #[test]
    fn otros_proyectos_no_son_de_deckard() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            d.path().join("metadata.json"),
            r#"{"ejercicio_id": "x", "generator": "otra"}"#,
        )
        .unwrap();
        assert!(leer(d.path()).is_none());
    }
}
