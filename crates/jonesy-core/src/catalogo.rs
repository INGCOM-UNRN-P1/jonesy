//! Catálogo de reglas de las herramientas del ecosistema (gaff, kaneda, spunkmeyer…), para
//! el catálogo navegable de Ayuda y para que F1 explique cualquier hallazgo con el mismo
//! formato que los errores de gcc: qué es, qué hacer, «así no» y «así sí».
//!
//! Cada integración puede declarar `catalogo = "…--json"`. El JSON trae una lista
//! (`reglas`, `patrones` o `rules`) con, según la herramienta, `codigo`, `titulo` o
//! `nombre`, `descripcion` o `explicacion`, `sugerencia`, `ejemplo_incorrecto`,
//! `ejemplo_correcto`, `categoria`, `alias`, `cwe`, `regla_catedra`.

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Regla {
    pub herramienta: String,
    pub codigo: String,
    pub alias: Vec<String>,
    pub titulo: String,
    pub descripcion: String,
    pub sugerencia: String,
    pub mal: String,
    pub bien: String,
    pub categoria: String,
    /// Datos extra para mostrar (CWE, regla de cátedra relacionada…).
    pub extra: Vec<(String, String)>,
}

fn texto(v: &Value, claves: &[&str]) -> String {
    claves
        .iter()
        .find_map(|k| v.get(*k).and_then(Value::as_str))
        .unwrap_or("")
        .trim()
        .to_string()
}

/// Las reglas del JSON que emite el comando `catalogo` de una herramienta.
pub fn desde_json(herramienta: &str, json: &str) -> Vec<Regla> {
    let inicio = json.find('{').unwrap_or(0);
    let Ok(v) = serde_json::from_str::<Value>(&json[inicio..]) else {
        return vec![];
    };
    let lista = ["reglas", "patrones", "rules", "antipatrones"]
        .iter()
        .find_map(|k| v.get(*k).and_then(Value::as_array))
        .cloned()
        .or_else(|| v.as_array().cloned())
        .unwrap_or_default();
    lista
        .iter()
        .filter_map(|r| {
            let codigo = texto(r, &["codigo", "code", "id"]);
            if codigo.is_empty() {
                return None;
            }
            let mut alias = vec![];
            for k in ["alias", "sp_codigo", "codigo_anterior"] {
                let a = texto(r, &[k]);
                if !a.is_empty() && a != codigo {
                    alias.push(a);
                }
            }
            let mut extra = vec![];
            for (k, nombre) in [
                ("cwe", "CWE"),
                ("regla_catedra", "Regla de cátedra"),
                ("severidad", "Severidad"),
            ] {
                let x = texto(r, &[k]);
                if !x.is_empty() {
                    extra.push((nombre.to_string(), x));
                }
            }
            Some(Regla {
                herramienta: herramienta.to_string(),
                codigo,
                alias,
                titulo: texto(r, &["titulo", "nombre", "title"]),
                descripcion: texto(r, &["descripcion", "explicacion", "description"]),
                sugerencia: texto(r, &["sugerencia", "como_corregir", "fix"]),
                mal: texto(r, &["ejemplo_incorrecto", "mal"]),
                bien: texto(r, &["ejemplo_correcto", "bien"]),
                categoria: texto(r, &["categoria", "category"]),
                extra,
            })
        })
        .collect()
}

/// La regla `codigo` (o un alias), prefiriendo la de `herramienta` si varias comparten
/// el código.
pub fn buscar<'a>(
    reglas: &'a [Regla],
    herramienta: Option<&str>,
    codigo: &str,
) -> Option<&'a Regla> {
    let c = codigo.trim().to_lowercase();
    let coincide =
        |r: &&Regla| r.codigo.to_lowercase() == c || r.alias.iter().any(|a| a.to_lowercase() == c);
    reglas
        .iter()
        .filter(coincide)
        .find(|r| herramienta.is_some_and(|h| h == r.herramienta))
        .or_else(|| reglas.iter().find(coincide))
}

/// La ficha de una regla en Markdown, con el mismo formato que los errores de gcc.
pub fn markdown(r: &Regla, enlace: Option<&str>) -> String {
    markdown_hallazgo(r, None, enlace)
}

/// Como los errores de gcc: título, lo que se marcó en la línea del alumno (si no es
/// el mismo título), explicación, qué hacer y «Así no» / «Así sí».
pub fn markdown_hallazgo(r: &Regla, hallazgo: Option<&str>, enlace: Option<&str>) -> String {
    let mut s = format!("# {} · {}\n\n", r.codigo, r.titulo);
    if let Some(h) = hallazgo
        .map(str::trim)
        .filter(|h| !h.is_empty() && *h != r.titulo)
    {
        s.push_str(&format!("> `{h}`\n\n"));
    }
    let mut meta = vec![r.herramienta.clone()];
    if !r.categoria.is_empty() {
        meta.push(r.categoria.clone());
    }
    s.push_str(&format!("> {}\n\n", meta.join(" · ")));
    if !r.descripcion.is_empty() {
        s.push_str(&r.descripcion);
        s.push('\n');
    }
    if !r.sugerencia.is_empty() {
        s.push_str(&format!("\n**Qué hacer:** {}\n", r.sugerencia));
    }
    if !r.mal.is_empty() {
        s.push_str(&format!(
            "\n**Así no:**\n\n```c\n{}\n```\n",
            r.mal.trim_end()
        ));
    }
    if !r.bien.is_empty() {
        s.push_str(&format!(
            "\n**Así sí:**\n\n```c\n{}\n```\n",
            r.bien.trim_end()
        ));
    }
    for (k, v) in &r.extra {
        s.push_str(&format!("\n**{k}:** {v}"));
    }
    if !r.extra.is_empty() {
        s.push('\n');
    }
    if let Some(e) = enlace {
        s.push_str(&format!("\n**Apunte:** {e}\n"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const GAFF: &str = r#"{"schema_version": "1.0.0", "total_reglas": 1, "reglas": [{"codigo": "0x0001h", "titulo": "La claridad es lo primero", "descripcion": "El código debe ser claro.", "ejemplo_correcto": "int i = 0;", "ejemplo_incorrecto": "int i=0,j=10;", "categoria": "Sintaxis", "codigo_anterior": "0x0000h", "alias": "GAFF_0x0001h", "severidad": "ESTILO"}]}"#;
    const KANEDA: &str = r#"{"reglas": [{"codigo": "KAN001", "titulo": "gets() prohibida", "descripcion": "No verifica límites.", "sugerencia": "Usá fgets.", "cwe": "CWE-242", "regla_catedra": "0x5008h", "ejemplo_incorrecto": "gets(b);", "ejemplo_correcto": "fgets(b, sizeof b, stdin);"}]}"#;
    const SPUNK: &str = r#"aviso previo
{"patrones": [{"codigo": "0x0001h", "alias": "AP006", "nombre": "Punto y coma tras el if", "explicacion": "Crea una sentencia vacía.", "sugerencia": "Sacá el ;", "ejemplo_incorrecto": "if (x);", "ejemplo_correcto": "if (x)"}]}"#;

    #[test]
    fn lee_los_catalogos_de_cada_herramienta() {
        let mut r = desde_json("gaff", GAFF);
        r.extend(desde_json("kaneda", KANEDA));
        r.extend(desde_json("spunkmeyer", SPUNK));
        assert_eq!(r.len(), 3);
        assert_eq!(r[0].alias, vec!["GAFF_0x0001h", "0x0000h"]);
        // Mismo código en dos herramientas: gana la del hallazgo.
        assert_eq!(
            buscar(&r, Some("spunkmeyer"), "0x0001h").unwrap().titulo,
            "Punto y coma tras el if"
        );
        assert_eq!(
            buscar(&r, Some("gaff"), "0x0001h").unwrap().herramienta,
            "gaff"
        );
        assert_eq!(buscar(&r, None, "ap006").unwrap().herramienta, "spunkmeyer");
        let m = markdown(
            buscar(&r, None, "KAN001").unwrap(),
            Some("https://apunte/x"),
        );
        assert!(m.contains("**Así no:**") && m.contains("**Así sí:**"));
        assert!(m.contains("**CWE:** CWE-242") && m.contains("**Apunte:**"));
        assert!(desde_json("x", "no es json").is_empty());
    }
}
