//! Memoria en vivo con bishop (Ola 42): la vista de memoria (Alt+M) de la parada actual.
//!
//! cat-scan dibujaba la traza de `mem-trace`, que corre el programa otra vez por su cuenta: si
//! el programa lee de la entrada o depende del azar, la parada del depurador no figura en la
//! traza. Con bishop, la misma sesión de gdb del depurador carga su módulo (`bishop
//! gdb-script` da la ruta), sigue malloc y free sin detener el programa (`bishop-seguir`) y en
//! cada parada escribe la memoria en un JSON (`bishop-memoria`). Acá ese JSON se convierte en
//! un [`Paso`] de cat-scan, así se dibuja igual: marcos a la izquierda, bloques a la derecha,
//! flechas, punteros colgantes y, además, las fugas (bloques vivos que nadie apunta).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use serde_json::{json, Value};

use crate::catscan::{Bloque, Heap, MarcoTraza, Paso, Sitio, VariableTraza};

/// Ruta del módulo de gdb de bishop (`bishop gdb-script`), si bishop está instalado y lo
/// tiene. Se averigua una vez por sesión.
pub fn script_gdb() -> Option<PathBuf> {
    static RUTA: OnceLock<Option<PathBuf>> = OnceLock::new();
    RUTA.get_or_init(|| {
        let bishop = std::env::var_os("JONESY_BISHOP")
            .map(PathBuf::from)
            .or_else(|| crate::doctor::buscar("bishop"))?;
        let o = Command::new(bishop)
            .arg("gdb-script")
            .env("NO_COLOR", "1")
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        let ruta = PathBuf::from(String::from_utf8_lossy(&o.stdout).trim());
        (o.status.success() && ruta.is_file()).then_some(ruta)
    })
    .clone()
}

/// La foto que dejó `bishop-memoria` en `ruta`.
pub fn cargar(ruta: &Path) -> Result<Paso, String> {
    let texto = std::fs::read_to_string(ruta).map_err(|e| format!("{}: {e}", ruta.display()))?;
    let v: Value =
        serde_json::from_str(&texto).map_err(|e| format!("memoria de bishop inválida: {e}"))?;
    Ok(paso_de_bishop(&v))
}

fn direccion(v: &Value) -> u64 {
    v.as_str()
        .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0)
}

fn sitio(v: &Value) -> Option<Sitio> {
    Some(Sitio {
        func: v["funcion"].as_str().unwrap_or("").into(),
        file: v["archivo"].as_str()?.into(),
        line: v["linea"].as_u64().unwrap_or(0) as usize,
    })
}

fn sitio_json(v: &Value) -> Value {
    json!({"file": v["archivo"], "line": v["linea"], "func": v["funcion"]})
}

/// El valor de una variable o campo con la forma de mem-trace (`kind`, `type`, `value` o
/// `addr`/`region`).
fn valor(tipo: &str, texto: &str, es_puntero: bool, bloques: &[Bloque]) -> Value {
    if !es_puntero {
        return json!({"kind": "scalar", "type": tipo, "value": texto});
    }
    let dir = direccion(&json!(texto));
    let region = if dir == 0 {
        "null"
    } else if bloques.iter().any(|b| b.contiene(dir)) {
        "heap"
    } else {
        "otra"
    };
    json!({"kind": "pointer", "type": tipo, "addr": dir, "region": region})
}

/// Un JSON de `bishop-memoria` como paso de cat-scan.
pub fn paso_de_bishop(v: &Value) -> Paso {
    let lista = |v: &Value| v.as_array().cloned().unwrap_or_default();
    let mut sin_numero = 0;
    let mut bloques: Vec<Bloque> = lista(&v["heap"])
        .iter()
        .map(|b| {
            // Un bloque que el seguimiento no numeró (reservado antes o por la biblioteca).
            let id = b["id"].as_u64().unwrap_or_else(|| {
                sin_numero += 1;
                10_000 + sin_numero
            });
            Bloque {
                id,
                addr: direccion(&b["direccion"]),
                size: b["tamanio_bytes"].as_u64().unwrap_or(0),
                status: if b["esta_liberado"].as_bool() == Some(true) {
                    "freed".into()
                } else {
                    "alive".into()
                },
                site: sitio(&b["sitio"]).unwrap_or_default(),
                free_site: sitio(&b["sitio_free"]),
                fuga: b["alcanzable"].as_bool() == Some(false),
                contenido: None,
            }
        })
        .collect();
    // El contenido de cada bloque: sus campos si es un struct, si no el texto de bishop.
    let contenidos: Vec<Option<Value>> = lista(&v["heap"])
        .iter()
        .map(|b| {
            let tipo = b["tipo"].as_str().unwrap_or("");
            if let Some(campos) = b["campos"].as_array() {
                let fields: Vec<Value> = campos
                    .iter()
                    .map(|c| {
                        json!({
                            "name": c["nombre"],
                            "value": valor(
                                c["tipo"].as_str().unwrap_or(""),
                                c["valor"].as_str().unwrap_or(""),
                                c["es_puntero"].as_bool() == Some(true),
                                &bloques,
                            ),
                        })
                    })
                    .collect();
                Some(json!({"kind": "struct", "type": tipo, "fields": fields}))
            } else {
                b["contenido"]
                    .as_str()
                    .filter(|c| !c.is_empty() && b["esta_liberado"].as_bool() != Some(true))
                    .map(|c| json!({"kind": "scalar", "type": tipo, "value": c}))
            }
        })
        .collect();
    for (b, c) in bloques.iter_mut().zip(contenidos) {
        b.contenido = c;
    }
    // bishop da los marcos del más nuevo al más viejo; la pila se dibuja desde main.
    let mut frames: Vec<MarcoTraza> = lista(&v["frames"])
        .iter()
        .map(|m| MarcoTraza {
            func: m["funcion"].as_str().unwrap_or("?").into(),
            file: m["archivo"].as_str().unwrap_or("").into(),
            line: m["linea_actual"].as_u64().unwrap_or(0) as usize,
            locals: lista(&m["variables"])
                .iter()
                .map(|x| VariableTraza {
                    name: x["nombre"].as_str().unwrap_or("?").into(),
                    storage: "local".into(),
                    addr: direccion(&x["direccion"]),
                    val: valor(
                        x["tipo"].as_str().unwrap_or(""),
                        x["valor"].as_str().unwrap_or(""),
                        x["es_puntero"].as_bool() == Some(true),
                        &bloques,
                    ),
                })
                .collect(),
        })
        .collect();
    let (func, line) = frames
        .first()
        .map(|m| (m.func.clone(), m.line))
        .unwrap_or_default();
    frames.reverse();
    let events = lista(&v["eventos"])
        .iter()
        .map(|e| {
            let tipo = e["tipo"].as_str().unwrap_or("?");
            let kind = if tipo == "realloc" && e["movido"].as_bool() == Some(true) {
                "resize"
            } else {
                tipo
            };
            json!({
                "kind": kind,
                "id": e["id"],
                "size": e["tamanio"],
                "site": sitio_json(&e["sitio"]),
                "double_free": e["doble"].as_bool() == Some(true),
                "unknown": e["desconocido"].as_bool() == Some(true),
                "noop": e["nulo"].as_bool() == Some(true),
                "failed": e["falla"].as_bool() == Some(true),
            })
        })
        .collect();
    Paso {
        n: 0,
        func,
        line,
        frames,
        globals: vec![],
        heap: Heap { blocks: bloques },
        events,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn foto() -> Value {
        json!({
            "linea": 26,
            "frames": [
                {"funcion": "agregar", "archivo": "/tp/lista.c", "linea_actual": 9, "variables": [
                    {"nombre": "n", "tipo": "nodo_t *", "direccion": "0x7ffe10", "valor": "0x4050",
                     "es_puntero": true, "direccion_apuntada": "0x4050"}
                ]},
                {"funcion": "main", "archivo": "/tp/main.c", "linea_actual": 26, "variables": [
                    {"nombre": "lista", "tipo": "nodo_t *", "direccion": "0x7ffe40", "valor": "0x4050",
                     "es_puntero": true, "direccion_apuntada": "0x4050"},
                    {"nombre": "copia", "tipo": "char *", "direccion": "0x7ffe48", "valor": "0x5080",
                     "es_puntero": true, "direccion_apuntada": "0x5080"},
                    {"nombre": "i", "tipo": "int", "direccion": "0x7ffe50", "valor": "3", "es_puntero": false}
                ]}
            ],
            "heap": [
                {"id": 2, "direccion": "0x4050", "tamanio_bytes": 16, "tipo": "struct nodo",
                 "contenido": "{dato=1, sig=0x4030}", "sitio": {"archivo": "/tp/lista.c", "linea": 8, "funcion": "agregar"},
                 "campos": [{"nombre": "dato", "tipo": "int", "valor": "1"},
                            {"nombre": "sig", "tipo": "struct nodo *", "valor": "0x4030", "es_puntero": true}],
                 "punteros_salientes": [{"campo": "sig", "destino": "0x4030"}]},
                {"id": 1, "direccion": "0x4030", "tamanio_bytes": 16, "tipo": "struct nodo",
                 "campos": [{"nombre": "dato", "tipo": "int", "valor": "0"},
                            {"nombre": "sig", "tipo": "struct nodo *", "valor": "NULL", "es_puntero": true}],
                 "sitio": {"archivo": "/tp/lista.c", "linea": 8, "funcion": "agregar"}},
                {"id": 4, "direccion": "0x5080", "tamanio_bytes": 5, "contenido": "‹liberado›", "esta_liberado": true,
                 "sitio": {"archivo": "/tp/main.c", "linea": 20}, "sitio_free": {"archivo": "/tp/main.c", "linea": 24}},
                {"id": 5, "direccion": "0x50a0", "tamanio_bytes": 400, "contenido": "", "alcanzable": false,
                 "sitio": {"archivo": "/tp/main.c", "linea": 21}}
            ],
            "eventos": [
                {"tipo": "realloc", "id": 5, "tamanio": 400, "movido": true, "sitio": {"archivo": "/tp/main.c", "linea": 22}},
                {"tipo": "free", "id": 4, "sitio": {"archivo": "/tp/main.c", "linea": 24}},
                {"tipo": "free", "id": 4, "doble": true, "sitio": {"archivo": "/tp/main.c", "linea": 25}}
            ]
        })
    }

    #[test]
    fn convierte_la_foto_de_bishop() {
        let p = paso_de_bishop(&foto());
        assert_eq!((p.func.as_str(), p.line), ("agregar", 9));
        assert_eq!(p.frames[0].func, "main", "la pila empieza en main");
        assert_eq!(p.frames[0].locals[0].val["region"], "heap");
        assert_eq!(p.frames[0].locals[2].val["value"], "3");
        let ids: Vec<u64> = p.heap.blocks.iter().map(|b| b.id).collect();
        assert_eq!(ids, [2, 1, 4, 5]);
        assert!(!p.heap.blocks[2].vivo() && p.heap.blocks[3].fuga);
        assert_eq!(p.heap.blocks[2].free_site.as_ref().unwrap().line, 24);
    }

    #[test]
    fn se_dibuja_con_catscan() {
        let p = paso_de_bishop(&foto());
        let s = crate::catscan::dibujar(&p, 120).a_texto();
        assert!(
            s.contains("main() main.c:26") && s.contains("agregar() lista.c:9"),
            "{s}"
        );
        // Los campos de cada nodo, con la flecha al siguiente aunque ninguna variable lo apunte.
        assert!(s.contains("dato = 1") && s.contains("dato = 0"), "{s}");
        assert!(s.contains("─► #1"), "{s}");
        assert!(s.contains("⚠ colgante"), "{s}");
        assert!(s.contains("#5 0x50a0 fuga"), "{s}");
    }

    #[test]
    fn eventos_en_el_formato_de_catscan() {
        let p = paso_de_bishop(&foto());
        let e: Vec<String> = crate::catscan::eventos_del_paso(&p)
            .into_iter()
            .map(|(_, t)| t)
            .collect();
        assert_eq!(e[0], "realloc #5 en main.c:22: el bloque se movió");
        assert_eq!(e[1], "free(#4) en main.c:24");
        assert!(e[2].starts_with("⚠ free doble de #4 en main.c:25"), "{e:?}");
    }
}
