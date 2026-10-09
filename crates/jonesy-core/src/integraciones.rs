//! Integraciones con las herramientas del ecosistema de la cátedra, como plugins
//! declarativos.
//!
//! Cada integración es un manifiesto TOML (ver `data/integraciones.toml`, las de fábrica)
//! que dice qué binario la habilita, cuándo correrla (al guardar o a mano), sobre qué
//! archivos, cómo leer su salida (el formato común de hallazgos de yutani, ripley, corbel o
//! gcc), cómo explicar uno de sus códigos con F1 y qué acciones agrega al menú
//! Herramientas. Un proyecto agrega o reemplaza integraciones en `.jonesy/integraciones/`
//! y un usuario en `~/.config/jonesy/integraciones/`; `[integraciones] desactivar` en
//! `.jonesy.toml` apaga las que no quiera.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::diagnostico::{Diagnostico, Origen, Severidad};

const DE_FABRICA: &str = include_str!("../data/integraciones.toml");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Cuando {
    /// Al guardar un archivo que coincide con `archivos`.
    Guardar,
    /// Sólo con sus acciones (Alt+H) o la paleta.
    #[default]
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Formato {
    /// `{"hallazgos": [...]}` con la forma común de yutani.
    #[default]
    Hallazgos,
    /// `ripley check --format json` (`ast_findings`).
    Ripley,
    /// `corbel check --json` (`missing`).
    Corbel,
    /// Salida de gcc/clang (`archivo:línea:col: error: …`).
    Gcc,
    /// SARIF 2.1.0: el formato estándar de analizadores (gcc con
    /// `-fdiagnostics-format=sarif-stderr`, clang, cppcheck, CodeQL…).
    Sarif,
}

/// Qué hacer con la salida de una acción.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Modo {
    /// Salida en vivo en el panel inferior.
    #[default]
    Panel,
    /// En la terminal interna (programas interactivos).
    Terminal,
    /// Ventana con la salida, con sus colores.
    Visor,
    /// Modifica el archivo en disco: se guarda antes y se recarga después.
    Recargar,
    /// Interpreta la salida con el `formato` de la integración y la muestra en Problemas.
    Diagnosticos,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Accion {
    pub titulo: String,
    pub comando: String,
    pub modo: Modo,
    /// Globs de archivos (vacío = los de la integración).
    pub archivos: Vec<String>,
    /// Permitida en modo examen. Sin indicarlo, se permiten todas salvo las que
    /// modifican el código (modo «recargar»: gaff fix, corbel scaffold), que harían el
    /// trabajo por el alumno.
    pub examen: Option<bool>,
}

impl Accion {
    pub fn permitida_en_examen(&self) -> bool {
        self.examen.unwrap_or(self.modo != Modo::Recargar)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Integracion {
    pub nombre: String,
    pub descripcion: String,
    /// Programa que tiene que estar en el PATH para habilitarla (vacío = siempre).
    pub binario: String,
    pub cuando: Cuando,
    /// Globs del nombre de archivo (`*.c`).
    pub archivos: Vec<String>,
    /// Comando al guardar, con marcadores (ver [`expandir`]).
    pub comando: String,
    pub formato: Formato,
    /// Comando para explicar un código con F1 (`gaff explain {codigo}`).
    pub explicar: String,
    /// Comando que emite el catálogo de reglas en JSON (`gaff rules --json`): alimenta el
    /// catálogo navegable y la explicación unificada de F1 (ver `catalogo`).
    pub catalogo: String,
    /// Campo booleano de cada hallazgo que, si es true, lo descarta (duplicados entre
    /// herramientas, como `tambien_en_gaff` de spunkmeyer).
    pub omitir_si: String,
    /// Activa por defecto (se puede apagar desde Ver → Integraciones).
    pub activa: bool,
    /// Permitida en modo examen.
    pub examen: bool,
    /// Tarda varios segundos: se indica en la barra mientras corre.
    pub lento: bool,
    #[serde(rename = "accion")]
    pub acciones: Vec<Accion>,
    /// De dónde salió el manifiesto (para mostrarlo); no se lee del TOML.
    #[serde(skip)]
    pub origen: String,
}

impl Default for Integracion {
    fn default() -> Self {
        Integracion {
            nombre: String::new(),
            descripcion: String::new(),
            binario: String::new(),
            cuando: Cuando::Manual,
            archivos: vec![],
            comando: String::new(),
            formato: Formato::Hallazgos,
            explicar: String::new(),
            catalogo: String::new(),
            omitir_si: String::new(),
            activa: true,
            examen: true,
            lento: false,
            acciones: vec![],
            origen: String::new(),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct Archivo {
    integracion: Vec<Integracion>,
}

/// Configuración en `.jonesy.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ConfigIntegraciones {
    /// Nombres de integraciones a no usar en este proyecto.
    pub desactivar: Vec<String>,
    /// Integraciones apagadas por defecto que este proyecto activa (corbel al guardar).
    pub activar: Vec<String>,
}

fn coincide(globs: &[String], ruta: &Path) -> bool {
    let nombre = ruta
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    globs.is_empty() || globs.iter().any(|g| crate::entrega::glob(g, &nombre))
}

impl Integracion {
    pub fn disponible(&self) -> bool {
        self.binario.is_empty() || crate::proyecto::en_path(&self.binario)
    }

    pub fn aplica_a(&self, ruta: &Path) -> bool {
        coincide(&self.archivos, ruta)
    }
}

impl Accion {
    pub fn aplica_a(&self, integracion: &Integracion, ruta: &Path) -> bool {
        if self.archivos.is_empty() {
            integracion.aplica_a(ruta)
        } else {
            coincide(&self.archivos, ruta)
        }
    }
}

/// Las de fábrica.
pub fn de_fabrica() -> Vec<Integracion> {
    let a: Archivo = toml::from_str(DE_FABRICA).expect("data/integraciones.toml es válido");
    a.integracion
        .into_iter()
        .map(|mut i| {
            i.origen = "de fábrica".into();
            i
        })
        .collect()
}

/// Directorio del usuario: `$XDG_CONFIG_HOME/jonesy/integraciones` (o `~/.config`; en
/// Windows `%APPDATA%\jonesy\integraciones`).
pub fn dir_usuario() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    };
    base.map(|b| b.join("jonesy").join("integraciones"))
}

/// Lee los `*.toml` de `dir`; devuelve las integraciones y los errores de lectura.
fn leer_dir(dir: &Path) -> (Vec<Integracion>, Vec<String>) {
    let mut v = vec![];
    let mut errores = vec![];
    let Ok(it) = std::fs::read_dir(dir) else {
        return (v, errores);
    };
    let mut archivos: Vec<PathBuf> = it
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "toml"))
        .collect();
    archivos.sort();
    for p in archivos {
        match std::fs::read_to_string(&p)
            .map_err(|e| e.to_string())
            .and_then(|t| toml::from_str::<Archivo>(&t).map_err(|e| e.to_string()))
        {
            Ok(a) => v.extend(a.integracion.into_iter().map(|mut i| {
                i.origen = p.display().to_string();
                i
            })),
            Err(e) => errores.push(format!("{}: {e}", p.display())),
        }
    }
    (v, errores)
}

/// Todas las integraciones: de fábrica, del usuario y del proyecto (en ese orden; una
/// posterior con el mismo nombre reemplaza a la anterior), con la configuración aplicada.
pub fn cargar(dir_estado: &Path, config: &ConfigIntegraciones) -> (Vec<Integracion>, Vec<String>) {
    let mut todas = de_fabrica();
    let mut errores = vec![];
    let dirs: Vec<PathBuf> = dir_usuario()
        .into_iter()
        .chain([dir_estado.join("integraciones")])
        .collect();
    for d in dirs {
        let (v, e) = leer_dir(&d);
        errores.extend(e);
        for i in v {
            match todas.iter_mut().find(|x| x.nombre == i.nombre) {
                Some(x) => *x = i,
                None => todas.push(i),
            }
        }
    }
    for i in &mut todas {
        if config.desactivar.contains(&i.nombre) {
            i.activa = false;
        }
        if config.activar.contains(&i.nombre) {
            i.activa = true;
        }
    }
    (todas, errores)
}

/// Reemplaza los marcadores de un comando. Los valores van entre comillas de shell.
pub fn expandir(
    plantilla: &str,
    raiz: &Path,
    archivo: Option<&Path>,
    modulo: Option<&Path>,
    codigo: Option<&str>,
) -> String {
    let q = crate::proyecto::shell_quote;
    let rel = |p: &Path| {
        let r = p
            .strip_prefix(raiz)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/");
        if r.is_empty() {
            ".".to_string()
        } else {
            r
        }
    };
    let mut s = plantilla.to_string();
    if let Some(a) = archivo {
        s = s.replace("{archivo}", &q(&rel(a)));
        s = s.replace("{dir}", &q(&rel(a.parent().unwrap_or(raiz))));
    }
    if let Some(m) = modulo.or_else(|| archivo.and_then(Path::parent)) {
        s = s.replace("{modulo}", &q(&rel(m)));
    }
    if let Some(c) = codigo {
        s = s.replace("{codigo}", &q(c));
    }
    s
}

fn severidad(s: &str) -> Severidad {
    match s.trim().to_lowercase().as_str() {
        "error" | "fatal" | "fatal error" | "critico" | "crítico" | "critical" | "high"
        | "alta" => Severidad::Error,
        "advertencia" | "warning" | "warn" | "medium" | "media" => Severidad::Advertencia,
        "info" | "nota" | "note" | "information" => Severidad::Nota,
        _ => Severidad::Estilo,
    }
}

/// JSON desde la primera llave (algunas herramientas imprimen avisos antes).
fn json(salida: &str) -> Result<Value, String> {
    let i = salida.find('{').ok_or("la herramienta no devolvió JSON")?;
    let mut des = serde_json::Deserializer::from_str(&salida[i..]).into_iter::<Value>();
    des.next().ok_or("JSON vacío")?.map_err(|e| e.to_string())
}

fn ruta_de(v: &Value, dir: &Path) -> Option<PathBuf> {
    let a = v.as_str().filter(|s| !s.is_empty())?;
    let p = PathBuf::from(a);
    Some(if p.is_absolute() { p } else { dir.join(p) })
}

/// Interpreta la salida de una integración. `archivo` es el analizado (para hallazgos sin
/// archivo) y `dir` la raíz contra la que se resuelven las rutas relativas.
pub fn interpretar(
    i: &Integracion,
    salida: &str,
    archivo: &Path,
    dir: &Path,
) -> Result<Vec<Diagnostico>, String> {
    let herramienta = Some(i.nombre.clone());
    let mut v = match i.formato {
        Formato::Gcc => crate::diagnostico::parsear_gcc(salida, dir),
        Formato::Sarif => {
            let doc =
                crate::sarif::buscar(salida).ok_or("la salida no tiene un documento SARIF")?;
            crate::sarif::interpretar(&doc, dir).diagnosticos
        }
        Formato::Ripley => crate::diagnostico::parsear_ripley(salida, dir)?,
        Formato::Hallazgos => {
            let datos = json(salida)?;
            let lista = datos
                .get("hallazgos")
                .and_then(Value::as_array)
                .ok_or("la salida no tiene «hallazgos»")?;
            lista
                .iter()
                .filter(|h| i.omitir_si.is_empty() || !h[&i.omitir_si].as_bool().unwrap_or(false))
                .map(|h| Diagnostico {
                    archivo: ruta_de(&h["archivo"], dir).or_else(|| Some(archivo.to_path_buf())),
                    linea: h["linea"].as_u64().unwrap_or(1).max(1) as usize,
                    columna: h["columna"].as_u64().unwrap_or(1).max(1) as usize,
                    severidad: severidad(h["severidad"].as_str().unwrap_or("")),
                    mensaje: h["mensaje"].as_str().unwrap_or("").to_string(),
                    codigo: h["codigo"].as_str().map(str::to_string),
                    sugerencia: h["sugerencia"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .map(str::to_string),
                    origen: Origen::Herramienta,
                    herramienta: herramienta.clone(),
                    categoria: h["categoria"].as_str().map(str::to_string),
                    enlace: h["enlace"].as_str().map(str::to_string),
                    resumen: None,
                })
                .collect()
        }
        Formato::Corbel => {
            let datos = json(salida)?;
            let lista = datos
                .get("missing")
                .and_then(Value::as_array)
                .ok_or("la salida no tiene «missing»")?;
            lista
                .iter()
                .map(|m| {
                    let tipo = m["type"].as_str().unwrap_or("elemento");
                    let nombre = m["name"].as_str().unwrap_or("");
                    Diagnostico {
                        archivo: Some(archivo.to_path_buf()),
                        linea: m["line"].as_u64().unwrap_or(1).max(1) as usize,
                        columna: 1,
                        severidad: Severidad::Estilo,
                        mensaje: format!("Falta documentar {tipo} {nombre}"),
                        codigo: Some("sin-documentar".into()),
                        sugerencia: Some(
                            "Alt+H → Generar el esqueleto de documentación (corbel scaffold)"
                                .into(),
                        ),
                        origen: Origen::Herramienta,
                        herramienta: herramienta.clone(),
                        categoria: Some("documentacion".into()),
                        enlace: None,
                        resumen: None,
                    }
                })
                .collect()
        }
    };
    for d in &mut v {
        if d.herramienta.is_none() {
            d.herramienta = herramienta.clone();
        }
    }
    Ok(v)
}

/// Categorías comunes del ecosistema (yutani.hallazgos.CATEGORIAS) en el orden del programa.
pub const CATEGORIAS: [(&str, &str); 20] = [
    ("sintaxis", "Sintaxis"),
    ("declaraciones", "Declaraciones y prototipos"),
    ("tipos", "Tipos y conversiones"),
    ("enlazado", "Enlazado"),
    ("compilacion", "Compilación y Makefiles"),
    ("control", "Estructuras de control"),
    ("funciones", "Funciones"),
    ("punteros", "Punteros"),
    ("memoria", "Memoria dinámica"),
    ("arreglos", "Arreglos y cadenas"),
    ("archivos", "Entrada, salida y archivos"),
    ("estructuras", "Estructuras"),
    ("tad", "TDAs y encapsulamiento"),
    ("recursion", "Recursión"),
    ("numeros", "Representación numérica"),
    ("documentacion", "Documentación y contratos"),
    ("pruebas", "Pruebas"),
    ("rendimiento", "Rendimiento"),
    ("estilo", "Estilo de la cátedra"),
    ("seguridad", "Seguridad"),
];

/// Título de una categoría común (o la clave tal cual si no es conocida).
pub fn titulo_categoria(clave: &str) -> &str {
    CATEGORIAS
        .iter()
        .find(|(c, _)| *c == clave)
        .map(|(_, t)| *t)
        .unwrap_or(clave)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn de_fabrica_validas() {
        let v = de_fabrica();
        let nombres: Vec<&str> = v.iter().map(|i| i.nombre.as_str()).collect();
        for n in [
            "ripley",
            "gaff",
            "kaneda",
            "spunkmeyer",
            "corbel",
            "daedalus",
            "tetsuo",
            "motoko",
            "callahan",
            "hal",
            "vasquez",
            "holden",
        ] {
            assert!(nombres.contains(&n), "falta {n}");
        }
        let gaff = v.iter().find(|i| i.nombre == "gaff").unwrap();
        assert_eq!(gaff.cuando, Cuando::Guardar);
        assert_eq!(gaff.acciones[0].modo, Modo::Recargar);
        assert!(
            !gaff.acciones[0].permitida_en_examen(),
            "gaff fix corrige por el alumno"
        );
        assert!(gaff.acciones[1].permitida_en_examen());
        assert!(!v.iter().find(|i| i.nombre == "callahan").unwrap().examen);
        let corbel = v.iter().find(|i| i.nombre == "corbel").unwrap();
        assert_eq!(corbel.cuando, Cuando::Manual);
        assert_eq!(corbel.acciones[0].modo, Modo::Diagnosticos);
    }

    #[test]
    fn proyecto_agrega_y_reemplaza() {
        let d = tempfile::tempdir().unwrap();
        let dir = d.path().join("integraciones");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("propias.toml"),
            "[[integracion]]\nnombre = \"gaff\"\ncuando = \"manual\"\n\n[[integracion]]\nnombre = \"cppcheck\"\nbinario = \"cppcheck\"\ncuando = \"guardar\"\narchivos = [\"*.c\"]\ncomando = \"cppcheck --template=gcc {archivo}\"\nformato = \"gcc\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("rota.toml"), "esto no es toml [").unwrap();
        let (v, errores) = cargar(
            d.path(),
            &ConfigIntegraciones {
                desactivar: vec!["kaneda".into()],
                activar: vec!["corbel".into()],
            },
        );
        assert_eq!(errores.len(), 1, "{errores:?}");
        assert_eq!(
            v.iter().find(|i| i.nombre == "gaff").unwrap().cuando,
            Cuando::Manual
        );
        assert!(v
            .iter()
            .any(|i| i.nombre == "cppcheck" && i.formato == Formato::Gcc));
        assert!(!v.iter().find(|i| i.nombre == "kaneda").unwrap().activa);
        assert!(
            v.iter().find(|i| i.nombre == "daedalus").unwrap().activa,
            "activar gana sobre desactivar"
        );
    }

    #[test]
    fn marcadores() {
        let r = Path::new("/tp");
        let c = expandir(
            "gaff check {archivo} --dir {dir} -m {modulo}",
            r,
            Some(Path::new("/tp/ej 1/main.c")),
            None,
            None,
        );
        assert_eq!(c, "gaff check 'ej 1/main.c' --dir 'ej 1' -m 'ej 1'");
        assert_eq!(
            expandir("kaneda explain {codigo}", r, None, None, Some("KAN001")),
            "kaneda explain KAN001"
        );
    }

    #[test]
    fn formato_sarif() {
        let i = Integracion {
            nombre: "cppcheck".into(),
            formato: Formato::Sarif,
            ..Default::default()
        };
        let salida = r#"Checking main.c ...
{"version": "2.1.0", "runs": [{"tool": {"driver": {"name": "Cppcheck", "rules": [{"id": "nullPointer", "helpUri": "https://cwe.mitre.org/data/definitions/476.html"}]}},
 "results": [{"ruleId": "nullPointer", "level": "error", "message": {"text": "Null pointer dereference: p"},
              "locations": [{"physicalLocation": {"artifactLocation": {"uri": "main.c"}, "region": {"startLine": 7, "startColumn": 6}}}]}]}]}
"#;
        let d = interpretar(&i, salida, Path::new("/tp/main.c"), Path::new("/tp")).unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].archivo.as_deref(), Some(Path::new("/tp/main.c")));
        assert_eq!((d[0].linea, d[0].columna), (7, 6));
        assert_eq!(d[0].codigo.as_deref(), Some("nullPointer"));
        assert_eq!(d[0].herramienta.as_deref(), Some("Cppcheck"));
        assert!(interpretar(&i, "sin sarif", Path::new("/tp/main.c"), Path::new("/tp")).is_err());
    }

    #[test]
    fn hallazgos_comunes_y_omitir_duplicados() {
        let mut i = Integracion {
            nombre: "spunkmeyer".into(),
            omitir_si: "tambien_en_gaff".into(),
            ..Default::default()
        };
        let salida = r#"aviso previo
{"schema_version": "1.0", "hallazgos": [
 {"id": "spunkmeyer:0x5008h", "codigo": "0x5008h", "categoria": "seguridad", "severidad": "error",
  "mensaje": "gets prohibida", "archivo": "main.c", "linea": 9, "columna": 5,
  "enlace": "https://ingcom-unrn-p1.github.io/x5008h", "tambien_en_gaff": true},
 {"id": "spunkmeyer:AP003", "codigo": "AP003", "categoria": "funciones", "severidad": "advertencia",
  "mensaje": "función demasiado larga", "archivo": "main.c", "linea": 5, "columna": 1, "sugerencia": "partila"}]}"#;
        let d = interpretar(&i, salida, Path::new("/tp/main.c"), Path::new("/tp")).unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].codigo.as_deref(), Some("AP003"));
        assert_eq!(d[0].severidad, Severidad::Advertencia);
        assert_eq!(d[0].categoria.as_deref(), Some("funciones"));
        assert_eq!(d[0].herramienta.as_deref(), Some("spunkmeyer"));
        assert_eq!(d[0].archivo.as_deref(), Some(Path::new("/tp/main.c")));
        i.omitir_si.clear();
        let d = interpretar(&i, salida, Path::new("/tp/main.c"), Path::new("/tp")).unwrap();
        assert_eq!(
            d[0].enlace.as_deref(),
            Some("https://ingcom-unrn-p1.github.io/x5008h")
        );
    }

    #[test]
    fn corbel_y_errores() {
        let i = Integracion {
            nombre: "corbel".into(),
            formato: Formato::Corbel,
            ..Default::default()
        };
        let d = interpretar(
            &i,
            r#"{"missing": [{"type": "función", "name": "suma", "line": 3}]}"#,
            Path::new("/tp/a.h"),
            Path::new("/tp"),
        )
        .unwrap();
        assert_eq!(d[0].mensaje, "Falta documentar función suma");
        assert_eq!(d[0].linea, 3);
        let g = Integracion {
            nombre: "gaff".into(),
            ..Default::default()
        };
        assert!(interpretar(
            &g,
            "Traceback: algo falló",
            Path::new("a.c"),
            Path::new(".")
        )
        .is_err());
    }

    #[test]
    fn herramientas_reales_si_estan() {
        // Comprueba el contrato con las herramientas instaladas (se saltea si no están).
        let dir = tempfile::tempdir().unwrap();
        let fuente = dir.path().join("main.c");
        std::fs::write(&fuente, "#include <stdio.h>\nint main(void)\n{\n    char b[4];\n    gets(b);\n    return 0;\n}\n").unwrap();
        for i in de_fabrica()
            .into_iter()
            .filter(|i| i.cuando == Cuando::Guardar && i.formato == Formato::Hallazgos)
        {
            if !i.disponible() {
                continue;
            }
            let cmd = expandir(&i.comando, dir.path(), Some(&fuente), None, None);
            let (prog, opcion) = crate::proceso::interprete();
            let out = std::process::Command::new(prog)
                .arg(opcion)
                .arg(&cmd)
                .current_dir(dir.path())
                .env("NO_COLOR", "1")
                .output()
                .unwrap();
            let salida = String::from_utf8_lossy(&out.stdout);
            let d = interpretar(&i, &salida, &fuente, dir.path())
                .unwrap_or_else(|e| panic!("{}: {e}\n{salida}", i.nombre));
            if i.nombre == "kaneda" {
                assert!(
                    d.iter().any(|x| x.linea == 5),
                    "kaneda tendría que marcar gets: {d:?}"
                );
            }
        }
    }
}
