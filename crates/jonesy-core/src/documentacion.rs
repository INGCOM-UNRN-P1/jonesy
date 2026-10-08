//! Documentación del proyecto generada por `corbel proyecto` (`make doc`).
//!
//! `doc/` no se versiona: se genera a demanda. jonesy la busca al abrir el proyecto, usa
//! `doc/api.json` en el quick-doc y avisa cuando falta o quedó vieja respecto del código
//! (RF-9 de `corbel/especificacion_documentacion_proyecto.md`). Para saber si quedó vieja
//! compara los SHA-256 de `fuentes` con los archivos actuales y busca `.h`/`.c` nuevos en
//! los módulos; si el `api.json` no trae `fuentes`, compara fechas de modificación.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Deserialize;

use crate::estructura::Estructura;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct ApiJson {
    schema_version: String,
    fuentes: Vec<Fuente>,
    modulos: Vec<ModuloApi>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct Fuente {
    archivo: String,
    sha256: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct ModuloApi {
    nombre: String,
    cabeceras: Vec<CabeceraApi>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct CabeceraApi {
    archivo: String,
    funciones: Vec<FuncionApi>,
    tipos: Vec<TipoApi>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct ParametroApi {
    nombre: String,
    descripcion: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct FuncionApi {
    nombre: String,
    firma: String,
    brief: String,
    descripcion: String,
    parametros: Vec<ParametroApi>,
    retorno: String,
    pre: Vec<String>,
    post: Vec<String>,
    ver: Vec<String>,
    declaracion: String,
    documentada: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct TipoApi {
    nombre: String,
    clase: String,
    opaco: bool,
    brief: String,
    declaracion: String,
}

/// Una función documentada del proyecto.
#[derive(Debug, Clone, PartialEq)]
pub struct FuncionDoc {
    pub nombre: String,
    pub modulo: String,
    /// Cabecera relativa a la raíz (`libs/arreglos/arreglos.h`).
    pub cabecera: String,
    pub firma: String,
    pub brief: String,
    pub descripcion: String,
    /// (nombre, descripción).
    pub parametros: Vec<(String, String)>,
    pub retorno: String,
    pub pre: Vec<String>,
    pub post: Vec<String>,
    pub ver: Vec<String>,
    /// `archivo:línea` de la declaración.
    pub declaracion: String,
    pub documentada: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TipoDoc {
    pub nombre: String,
    pub modulo: String,
    pub cabecera: String,
    pub clase: String,
    pub opaco: bool,
    pub brief: String,
    pub declaracion: String,
}

/// Estado de la documentación respecto del código.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EstadoDoc {
    /// No hay `api.json`.
    Ausente,
    AlDia,
    /// Quedó vieja: el motivo nombra el primer archivo que cambió.
    Desactualizada(String),
    /// Existe pero no se pudo leer (JSON roto o de otra versión mayor).
    Invalida(String),
}

impl EstadoDoc {
    /// true si conviene avisar que hay que correr `make doc`.
    pub fn pide_generar(&self) -> bool {
        !matches!(self, EstadoDoc::AlDia)
    }
}

#[derive(Debug, Clone, Default)]
pub struct DocProyecto {
    pub ruta: PathBuf,
    pub funciones: HashMap<String, FuncionDoc>,
    pub tipos: HashMap<String, TipoDoc>,
    fuentes: Vec<(String, String)>,
    pub modificado: Option<SystemTime>,
}

/// Versión mayor del esquema de `api.json` que entiende jonesy.
pub const ESQUEMA_MAYOR: &str = "1";

impl DocProyecto {
    /// Ruta de `api.json` para el directorio de documentación `dir` (relativo a `raiz`).
    pub fn ruta_api(raiz: &Path, dir: &str) -> PathBuf {
        raiz.join(if dir.trim().is_empty() {
            "doc"
        } else {
            dir.trim()
        })
        .join("api.json")
    }

    pub fn cargar(ruta: &Path) -> Result<DocProyecto, String> {
        let texto = std::fs::read_to_string(ruta).map_err(|e| e.to_string())?;
        let api: ApiJson =
            serde_json::from_str(&texto).map_err(|e| format!("api.json inválido: {e}"))?;
        let mayor = api.schema_version.split('.').next().unwrap_or("");
        if !api.schema_version.is_empty() && mayor != ESQUEMA_MAYOR {
            return Err(format!(
                "api.json usa el esquema {} (jonesy entiende {ESQUEMA_MAYOR}.x)",
                api.schema_version
            ));
        }
        let mut doc = DocProyecto {
            ruta: ruta.to_path_buf(),
            fuentes: api
                .fuentes
                .into_iter()
                .map(|f| (f.archivo, f.sha256))
                .collect(),
            modificado: std::fs::metadata(ruta).and_then(|m| m.modified()).ok(),
            ..Default::default()
        };
        for m in api.modulos {
            for c in m.cabeceras {
                for f in c.funciones {
                    let documentada = f.documentada.unwrap_or(!f.brief.is_empty());
                    doc.funciones.insert(
                        f.nombre.clone(),
                        FuncionDoc {
                            nombre: f.nombre,
                            modulo: m.nombre.clone(),
                            cabecera: c.archivo.clone(),
                            firma: f.firma,
                            brief: f.brief,
                            descripcion: f.descripcion,
                            parametros: f
                                .parametros
                                .into_iter()
                                .map(|p| (p.nombre, p.descripcion))
                                .collect(),
                            retorno: f.retorno,
                            pre: f.pre,
                            post: f.post,
                            ver: f.ver,
                            declaracion: f.declaracion,
                            documentada,
                        },
                    );
                }
                for t in c.tipos {
                    doc.tipos.insert(
                        t.nombre.clone(),
                        TipoDoc {
                            nombre: t.nombre,
                            modulo: m.nombre.clone(),
                            cabecera: c.archivo.clone(),
                            clase: t.clase,
                            opaco: t.opaco,
                            brief: t.brief,
                            declaracion: t.declaracion,
                        },
                    );
                }
            }
        }
        Ok(doc)
    }

    /// Compara la documentación con el código actual del proyecto.
    pub fn estado(&self, raiz: &Path, estructura: &Estructura) -> EstadoDoc {
        let actuales = fuentes_actuales(raiz, estructura);
        if self.fuentes.is_empty() {
            // Sin `fuentes` (corbel viejo): por fecha de modificación.
            let Some(generado) = self.modificado else {
                return EstadoDoc::AlDia;
            };
            for rel in &actuales {
                let nuevo = std::fs::metadata(raiz.join(rel))
                    .and_then(|m| m.modified())
                    .is_ok_and(|t| t > generado);
                if nuevo {
                    return EstadoDoc::Desactualizada(format!(
                        "{rel} cambió después de generar la documentación"
                    ));
                }
            }
            return EstadoDoc::AlDia;
        }
        let registradas: BTreeSet<&str> = self.fuentes.iter().map(|(a, _)| a.as_str()).collect();
        for (rel, sha) in &self.fuentes {
            match std::fs::read(raiz.join(rel)) {
                Err(_) => return EstadoDoc::Desactualizada(format!("se borró {rel}")),
                Ok(datos) if !sha.is_empty() && crate::sha256_hex(&datos) != *sha => {
                    return EstadoDoc::Desactualizada(format!("{rel} cambió"));
                }
                Ok(_) => {}
            }
        }
        if let Some(nuevo) = actuales.iter().find(|a| !registradas.contains(a.as_str())) {
            return EstadoDoc::Desactualizada(format!("{nuevo} es nuevo"));
        }
        EstadoDoc::AlDia
    }
}

/// `.h` y `.c` de los módulos del proyecto (relativos, con `/`), en el directorio del
/// módulo y en `include/` y `src/`. Sin módulos, los de la raíz.
pub fn fuentes_actuales(raiz: &Path, estructura: &Estructura) -> BTreeSet<String> {
    let mut dirs: Vec<PathBuf> = estructura.modulos.iter().map(|m| m.dir.clone()).collect();
    if dirs.is_empty() {
        dirs.push(raiz.to_path_buf());
    }
    let mut v = BTreeSet::new();
    for d in dirs {
        for sub in [d.clone(), d.join("include"), d.join("src")] {
            let Ok(it) = std::fs::read_dir(&sub) else {
                continue;
            };
            for e in it.flatten() {
                let p = e.path();
                let es_fuente = p.extension().is_some_and(|x| x == "c" || x == "h");
                let nombre = p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                // Los mocks generados y las copias temporales del IDE no cuentan.
                if es_fuente
                    && p.is_file()
                    && !nombre.starts_with('.')
                    && nombre != "holden_mocks.c"
                {
                    let rel = p
                        .strip_prefix(raiz)
                        .unwrap_or(&p)
                        .to_string_lossy()
                        .replace('\\', "/");
                    v.insert(rel);
                }
            }
        }
    }
    v
}

/// Tipos de los parámetros a partir de la firma (`int *f(const char *s, size_t n)` →
/// `["const char *", "size_t"]`), para mostrarlos junto a la descripción.
pub fn tipos_de_parametros(firma: &str) -> Vec<String> {
    let (Some(a), Some(z)) = (firma.find('('), firma.rfind(')')) else {
        return vec![];
    };
    let dentro = firma[a + 1..z].trim();
    if dentro.is_empty() || dentro == "void" {
        return vec![];
    }
    dentro
        .split(',')
        .map(|p| {
            let p = p.trim();
            // El tipo es todo menos el último identificador (el nombre del parámetro).
            let fin = p.trim_end_matches(|c: char| {
                c.is_alphanumeric() || c == '_' || c == '[' || c == ']'
            });
            let t = fin.trim();
            if t.is_empty() {
                p.to_string()
            } else {
                t.to_string()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proyecto() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let lib = d.path().join("libs/arreglos");
        std::fs::create_dir_all(&lib).unwrap();
        std::fs::write(lib.join("Makefile"), "all:\n\t@true\n").unwrap();
        std::fs::write(
            lib.join("arreglos.h"),
            "int suma(const int *v, size_t n);\n",
        )
        .unwrap();
        std::fs::write(
            lib.join("arreglos.c"),
            "int suma(const int *v, size_t n) { return 0; }\n",
        )
        .unwrap();
        d
    }

    fn api(d: &Path, fuentes: &[&str]) -> PathBuf {
        let fuentes: Vec<String> = fuentes
            .iter()
            .map(|f| {
                let sha = crate::sha256_hex(&std::fs::read(d.join(f)).unwrap());
                format!(r#"{{"archivo": "{f}", "sha256": "{sha}"}}"#)
            })
            .collect();
        let json = format!(
            r#"{{"schema_version": "1.0.0", "fuentes": [{}], "modulos": [{{"nombre": "arreglos", "cabeceras": [{{
                "archivo": "libs/arreglos/arreglos.h",
                "tipos": [{{"nombre": "arreglo_t", "clase": "typedef", "opaco": true, "brief": "Arreglo dinámico."}}],
                "funciones": [{{"nombre": "suma", "firma": "int suma(const int *v, size_t n)", "brief": "Suma los elementos.",
                  "parametros": [{{"nombre": "v", "descripcion": "los valores"}}, {{"nombre": "n", "descripcion": "cuántos"}}],
                  "retorno": "La suma.", "pre": ["v != NULL"], "declaracion": "libs/arreglos/arreglos.h:1"}}]}}]}}]}}"#,
            fuentes.join(",")
        );
        let ruta = d.join("doc/api.json");
        std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
        std::fs::write(&ruta, json).unwrap();
        ruta
    }

    #[test]
    fn carga_funciones_y_tipos() {
        let d = proyecto();
        let ruta = api(
            d.path(),
            &["libs/arreglos/arreglos.h", "libs/arreglos/arreglos.c"],
        );
        let doc = DocProyecto::cargar(&ruta).unwrap();
        let f = &doc.funciones["suma"];
        assert_eq!(f.brief, "Suma los elementos.");
        assert_eq!(f.modulo, "arreglos");
        assert_eq!(f.parametros[1], ("n".to_string(), "cuántos".to_string()));
        assert!(f.documentada);
        assert!(doc.tipos["arreglo_t"].opaco);
    }

    #[test]
    fn detecta_cambios_borrados_y_nuevos() {
        let d = proyecto();
        let est = Estructura::descubrir(d.path(), &[]);
        let ruta = api(
            d.path(),
            &["libs/arreglos/arreglos.h", "libs/arreglos/arreglos.c"],
        );
        let doc = DocProyecto::cargar(&ruta).unwrap();
        assert_eq!(doc.estado(d.path(), &est), EstadoDoc::AlDia);
        std::fs::write(
            d.path().join("libs/arreglos/arreglos.h"),
            "int suma(const int *v, size_t n);\nint resta(int a, int b);\n",
        )
        .unwrap();
        assert!(
            matches!(doc.estado(d.path(), &est), EstadoDoc::Desactualizada(m) if m.contains("arreglos.h cambió"))
        );
        let doc = DocProyecto::cargar(&api(
            d.path(),
            &["libs/arreglos/arreglos.h", "libs/arreglos/arreglos.c"],
        ))
        .unwrap();
        std::fs::write(d.path().join("libs/arreglos/extra.c"), "").unwrap();
        assert!(
            matches!(doc.estado(d.path(), &est), EstadoDoc::Desactualizada(m) if m.contains("extra.c es nuevo"))
        );
        std::fs::remove_file(d.path().join("libs/arreglos/extra.c")).unwrap();
        std::fs::remove_file(d.path().join("libs/arreglos/arreglos.c")).unwrap();
        assert!(
            matches!(doc.estado(d.path(), &est), EstadoDoc::Desactualizada(m) if m.contains("se borró"))
        );
    }

    #[test]
    fn esquema_de_otra_version_mayor() {
        let d = tempfile::tempdir().unwrap();
        let ruta = d.path().join("api.json");
        std::fs::write(&ruta, r#"{"schema_version": "2.0.0", "modulos": []}"#).unwrap();
        assert!(DocProyecto::cargar(&ruta)
            .unwrap_err()
            .contains("esquema 2.0.0"));
    }

    #[test]
    fn tipos_desde_la_firma() {
        assert_eq!(
            tipos_de_parametros("int suma(const int *v, size_t n)"),
            ["const int *", "size_t"]
        );
        assert!(tipos_de_parametros("void f(void)").is_empty());
        assert_eq!(tipos_de_parametros("void g(char buf[])"), ["char"]);
    }
}
