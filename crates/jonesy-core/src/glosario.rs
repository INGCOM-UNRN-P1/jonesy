//! Glosario de C para quien empieza (Ayuda → Glosario). Las explicaciones de errores
//! ofrecen los términos que mencionan (tecla g en la ventana).

use std::sync::OnceLock;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Termino {
    pub termino: String,
    #[serde(default)]
    pub alias: Vec<String>,
    pub definicion: String,
    #[serde(default)]
    pub ejemplo: String,
    #[serde(default)]
    pub ver: Vec<String>,
}

#[derive(Deserialize)]
struct Archivo {
    termino: Vec<Termino>,
}

const DATOS: &str = include_str!("../data/glosario.toml");

pub fn terminos() -> &'static [Termino] {
    static T: OnceLock<Vec<Termino>> = OnceLock::new();
    T.get_or_init(|| {
        toml::from_str::<Archivo>(DATOS)
            .expect("data/glosario.toml es válido (lo verifica un test)")
            .termino
    })
}

/// El término con ese nombre o alias (sin distinguir mayúsculas).
pub fn buscar(nombre: &str) -> Option<usize> {
    let n = nombre.to_lowercase();
    terminos().iter().position(|t| {
        t.termino.to_lowercase() == n || t.alias.iter().any(|a| a.to_lowercase() == n)
    })
}

fn de_palabra(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// ¿Aparece `palabra` en `texto` como palabra completa?
fn aparece(texto: &str, palabra: &str) -> bool {
    if palabra.chars().count() < 3 {
        return false;
    }
    let t: Vec<char> = texto.chars().collect();
    let p: Vec<char> = palabra.to_lowercase().chars().collect();
    if p.len() > t.len() {
        return false;
    }
    (0..=t.len() - p.len()).any(|i| {
        t[i..i + p.len()]
            .iter()
            .zip(&p)
            .all(|(a, b)| a.to_lowercase().next() == Some(*b))
            && (i == 0 || !de_palabra(t[i - 1]) || !de_palabra(p[0]))
            && t.get(i + p.len())
                .is_none_or(|c| !de_palabra(*c) || !de_palabra(p[p.len() - 1]))
    })
}

/// Los términos del glosario que menciona un texto, en el orden del glosario.
pub fn en_texto(texto: &str) -> Vec<usize> {
    terminos()
        .iter()
        .enumerate()
        .filter(|(_, t)| aparece(texto, &t.termino) || t.alias.iter().any(|a| aparece(texto, a)))
        .map(|(i, _)| i)
        .collect()
}

/// La ficha de un término en Markdown.
pub fn markdown(i: usize) -> String {
    let t = &terminos()[i];
    let mut s = format!("# {}\n\n{}\n", t.termino, t.definicion);
    if !t.ejemplo.is_empty() {
        s.push_str(&format!("\n```c\n{}\n```\n", t.ejemplo.trim_end()));
    }
    if !t.alias.is_empty() {
        s.push_str(&format!("\n**También:** {}\n", t.alias.join(", ")));
    }
    if !t.ver.is_empty() {
        s.push_str(&format!("\n**Ver:** {}\n", t.ver.join(" · ")));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_glosario_es_valido_y_sus_referencias_existen() {
        assert!(terminos().len() >= 35);
        for t in terminos() {
            for v in &t.ver {
                assert!(
                    buscar(v).is_some(),
                    "«{}» remite a «{v}», que no está",
                    t.termino
                );
            }
        }
    }

    #[test]
    fn reconoce_terminos_en_un_texto() {
        let texto =
            "Fuga de memoria: 10 bytes pedidos con malloc nunca se liberaron (falta un free).";
        let nombres: Vec<&str> = en_texto(texto)
            .into_iter()
            .map(|i| terminos()[i].termino.as_str())
            .collect();
        assert!(nombres.contains(&"fuga de memoria"), "{nombres:?}");
        assert!(
            nombres.contains(&"malloc") && nombres.contains(&"free"),
            "{nombres:?}"
        );
        assert!(!nombres.contains(&"stack"));
        assert_eq!(
            buscar("SEGFAULT").map(|i| terminos()[i].termino.as_str()),
            Some("violación de segmento")
        );
        assert!(markdown(buscar("NULL").unwrap()).contains("```c"));
    }
}
