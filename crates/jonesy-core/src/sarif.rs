//! SARIF 2.1.0 (Static Analysis Results Interchange Format): el formato JSON estándar de
//! resultados de analizadores y frameworks de pruebas (gcc con
//! `-fdiagnostics-format=sarif-stderr`, clang, cppcheck, CodeQL, conversores de JUnit…).
//!
//! jonesy lo lee para las pruebas que no usan `p1_test` (Construir → Correr pruebas) y para
//! las integraciones con `formato = "sarif"`. De cada resultado toma el nivel, el mensaje,
//! la regla (con su descripción y su enlace) y la primera ubicación; los resultados con
//! `kind = "pass"` cuentan como pruebas que pasan.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::diagnostico::{Diagnostico, Origen, Severidad};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Resultado {
    pub diagnosticos: Vec<Diagnostico>,
    /// Resultados con `kind = "pass"`.
    pub pasan: usize,
    /// Resultados que fallan (`kind = "fail"`, el valor por omisión).
    pub fallan: usize,
    /// Herramientas que produjeron el informe (`tool.driver.name`).
    pub herramientas: Vec<String>,
}

/// Busca un documento SARIF en un texto: entero, o el primer objeto JSON que empiece en
/// una línea (puede haber salida de make antes o después).
pub fn buscar(texto: &str) -> Option<Value> {
    let es_sarif = |v: &Value| v.get("runs").is_some_and(Value::is_array);
    if let Ok(v) = serde_json::from_str::<Value>(texto.trim()) {
        return es_sarif(&v).then_some(v);
    }
    let mut desde = 0;
    while let Some(i) = texto[desde..].find('{') {
        let inicio = desde + i;
        let al_principio = inicio == 0 || texto[..inicio].ends_with('\n');
        if al_principio {
            let mut flujo =
                serde_json::Deserializer::from_str(&texto[inicio..]).into_iter::<Value>();
            if let Some(Ok(v)) = flujo.next() {
                if es_sarif(&v) {
                    return Some(v);
                }
            }
        }
        desde = inicio + 1;
    }
    None
}

fn texto_de(v: &Value) -> Option<String> {
    v.get("text")
        .or_else(|| v.get("markdown"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Resuelve el `uri` de un artefacto: `file://` absoluto, relativo a un `uriBaseId`
/// declarado en `originalUriBaseIds`, o relativo a `dir`.
fn ruta(loc: &Value, bases: &Value, dir: &Path) -> Option<PathBuf> {
    let art = &loc["physicalLocation"]["artifactLocation"];
    let uri = art["uri"].as_str()?;
    let decodificar = |s: &str| s.replace("%20", " ");
    if let Some(abs) = uri.strip_prefix("file://") {
        return Some(PathBuf::from(decodificar(abs)));
    }
    let base = art["uriBaseId"]
        .as_str()
        .and_then(|id| bases[id]["uri"].as_str())
        .and_then(|b| b.strip_prefix("file://"))
        .map(|b| PathBuf::from(decodificar(b)))
        .unwrap_or_else(|| dir.to_path_buf());
    Some(base.join(decodificar(uri)))
}

/// Interpreta un documento SARIF. `dir` resuelve las rutas relativas sin base.
pub fn interpretar(doc: &Value, dir: &Path) -> Resultado {
    let mut r = Resultado::default();
    for run in doc["runs"].as_array().into_iter().flatten() {
        let driver = &run["tool"]["driver"];
        let herramienta = driver["name"].as_str().unwrap_or("sarif").to_string();
        if !r.herramientas.contains(&herramienta) {
            r.herramientas.push(herramienta.clone());
        }
        let reglas = driver["rules"].as_array().cloned().unwrap_or_default();
        let regla = |id: &str, indice: Option<u64>| -> Option<&Value> {
            indice
                .and_then(|i| reglas.get(i as usize))
                .or_else(|| reglas.iter().find(|x| x["id"].as_str() == Some(id)))
        };
        let bases = &run["originalUriBaseIds"];
        for res in run["results"].as_array().into_iter().flatten() {
            let kind = res["kind"].as_str().unwrap_or("fail");
            match kind {
                "pass" => {
                    r.pasan += 1;
                    continue;
                }
                "notApplicable" | "informational" => {}
                _ => r.fallan += 1,
            }
            let id = res["ruleId"].as_str().unwrap_or("");
            let def = regla(id, res["ruleIndex"].as_u64());
            let nivel = res["level"]
                .as_str()
                .or_else(|| def.and_then(|d| d["defaultConfiguration"]["level"].as_str()))
                .unwrap_or("warning");
            let severidad = match nivel {
                "error" => Severidad::Error,
                "warning" => Severidad::Advertencia,
                _ => Severidad::Nota,
            };
            let mensaje = texto_de(&res["message"])
                .or_else(|| def.and_then(|d| texto_de(&d["shortDescription"])))
                .unwrap_or_else(|| id.to_string());
            let loc = res["locations"].get(0).unwrap_or(&Value::Null);
            let region = &loc["physicalLocation"]["region"];
            let descripcion = def
                .and_then(|d| {
                    texto_de(&d["fullDescription"]).or_else(|| texto_de(&d["shortDescription"]))
                })
                .filter(|d| *d != mensaje);
            let mut d = Diagnostico {
                archivo: ruta(loc, bases, dir),
                linea: region["startLine"].as_u64().unwrap_or(1).max(1) as usize,
                columna: region["startColumn"].as_u64().unwrap_or(1).max(1) as usize,
                severidad,
                mensaje,
                codigo: (!id.is_empty()).then(|| id.to_string()),
                sugerencia: descripcion,
                origen: Origen::Herramienta,
                herramienta: Some(herramienta.clone()),
                enlace: def.and_then(|d| d["helpUri"].as_str()).map(str::to_string),
                ..Default::default()
            };
            // Los mensajes de gcc en SARIF son los mismos: se explican igual.
            if let Some(e) = crate::errores_gcc::explicar(&d.mensaje).filter(|e| !e.ocultar) {
                d.resumen = Some(e.resumen);
            }
            if kind == "fail" && d.codigo.is_none() && d.resumen.is_none() {
                d.resumen = res["properties"]["test"]
                    .as_str()
                    .map(|t| format!("La prueba falló «{t}»"));
            }
            r.diagnosticos.push(d);
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lo que emite gcc 16 con -fdiagnostics-format=sarif-stderr (recortado).
    fn gcc(base: &Path) -> String {
        format!(
            r#"{{"$schema": "https://docs.oasis-open.org/sarif/sarif/v2.1.0/errata01/os/schemas/sarif-schema-2.1.0.json",
 "version": "2.1.0",
 "runs": [{{"tool": {{"driver": {{"name": "GNU C11", "rules": []}}}},
            "originalUriBaseIds": {{"PWD": {{"uri": "file://{}/"}}}},
            "results": [{{"ruleId": "error", "level": "error",
                         "message": {{"text": "‘y’ undeclared (first use in this function)"}},
                         "locations": [{{"physicalLocation": {{"artifactLocation": {{"uri": "mal.c", "uriBaseId": "PWD"}},
                                                               "region": {{"startLine": 4, "startColumn": 5, "endColumn": 6}}}}}}]}}]}}]}}"#,
            base.display()
        )
    }

    #[test]
    fn diagnosticos_de_gcc() {
        let doc = buscar(&gcc(Path::new("/tp/ej1"))).unwrap();
        let r = interpretar(&doc, Path::new("/otro"));
        assert_eq!(r.herramientas, vec!["GNU C11"]);
        assert_eq!(r.fallan, 1);
        let d = &r.diagnosticos[0];
        assert_eq!(d.archivo.as_deref(), Some(Path::new("/tp/ej1/mal.c")));
        assert_eq!((d.linea, d.columna, d.severidad), (4, 5, Severidad::Error));
        // Explicado en español como cualquier error de gcc.
        assert!(d
            .resumen
            .as_deref()
            .unwrap()
            .starts_with("«y» no está declarada"));
    }

    #[test]
    fn resultados_de_pruebas_con_reglas_y_salida_alrededor() {
        let sarif = r#"{"version": "2.1.0", "runs": [{
  "tool": {"driver": {"name": "pruebas-c", "rules": [
     {"id": "PILA-01", "shortDescription": {"text": "apilar y desapilar"},
      "fullDescription": {"text": "Lo apilado sale en orden inverso"},
      "helpUri": "https://apunte/pilas"}]}},
  "results": [
    {"ruleId": "PILA-01", "kind": "pass", "message": {"text": "ok"}},
    {"ruleId": "PILA-01", "ruleIndex": 0, "kind": "fail", "level": "error",
     "message": {"text": "esperado 3, obtenido 2"},
     "locations": [{"physicalLocation": {"artifactLocation": {"uri": "prueba.c"},
                                         "region": {"startLine": 12}}}]},
    {"kind": "fail", "message": {"text": "se colgó"}, "properties": {"test": "vacia"},
     "locations": [{"physicalLocation": {"artifactLocation": {"uri": "file:///tp/prueba.c"},
                                         "region": {"startLine": 30, "startColumn": 2}}}]}
  ]}]}"#;
        let salida = format!("make: Entering directory '/tp'\n./test_bin --sarif\n{sarif}\nmake: Leaving directory '/tp'\n");
        let doc = buscar(&salida).expect("encuentra el SARIF entre la salida de make");
        let r = interpretar(&doc, Path::new("/tp/ej"));
        assert_eq!((r.pasan, r.fallan), (1, 2));
        let a = &r.diagnosticos[0];
        assert_eq!(a.archivo.as_deref(), Some(Path::new("/tp/ej/prueba.c")));
        assert_eq!(a.linea, 12);
        assert_eq!(a.codigo.as_deref(), Some("PILA-01"));
        assert_eq!(
            a.sugerencia.as_deref(),
            Some("Lo apilado sale en orden inverso")
        );
        assert_eq!(a.enlace.as_deref(), Some("https://apunte/pilas"));
        let b = &r.diagnosticos[1];
        assert_eq!(b.severidad, Severidad::Advertencia, "sin level: warning");
        assert_eq!(b.resumen.as_deref(), Some("La prueba falló «vacia»"));
        assert_eq!((b.linea, b.columna), (30, 2));
        assert!(buscar("nada de json\n{\"a\": 1}\n").is_none());
    }
}
