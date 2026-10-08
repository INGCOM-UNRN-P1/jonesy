//! Estructura del proyecto: módulos con Makefile (bibliotecas y ejercicios) y sus objetivos.
//!
//! Los TP de la cátedra (plantilla-TP) no traen un archivo que describa su estructura: el
//! Makefile raíz descubre `libs/*` y `ejercicios/*`, y cada uno tiene su Makefile con
//! objetivos comentados (`all`, `run`, `test`, `memcheck`, `fallos`, `clean`). Acá se lee
//! todo eso para mostrarlo en la interfaz:
//!
//! - el Makefile de la raíz y de cada subdirectorio (hasta dos niveles) que tenga uno,
//!   con la descripción de cada objetivo tomada del comentario que lo precede (o de un
//!   `## descripción` en la misma línea) y variables útiles (`PROG_TARGET`, `TEST_TARGET`,
//!   `LIB_NAME`);
//! - `library.json` de las bibliotecas estructuradas (nombre, versión, repositorio);
//! - los comandos de `tp.sh` (el gestor del TP), leídos de su ayuda.
//!
//! `.jonesy.toml` puede agregar descripciones o módulos con `[[modulo]]` (ver
//! [`ModuloConfig`]).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TipoModulo {
    /// El Makefile de la raíz del proyecto.
    Raiz,
    Libreria,
    Ejercicio,
    #[default]
    Otro,
}

impl TipoModulo {
    pub fn titulo(self) -> &'static str {
        match self {
            TipoModulo::Raiz => "Proyecto",
            TipoModulo::Libreria => "Bibliotecas",
            TipoModulo::Ejercicio => "Ejercicios",
            TipoModulo::Otro => "Otros módulos",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Objetivo {
    pub nombre: String,
    pub descripcion: String,
}

impl Objetivo {
    /// Objetivos que necesitan una terminal (leen de stdin): se corren en el deck.
    pub fn interactivo(&self) -> bool {
        self.nombre == "run"
            || self.nombre.starts_with("run-")
            || self.nombre.starts_with("ejecutar")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Modulo {
    pub nombre: String,
    /// Directorio absoluto.
    pub dir: PathBuf,
    /// Relativo a la raíz ("" para la raíz).
    pub rel: String,
    pub tipo: TipoModulo,
    pub objetivos: Vec<Objetivo>,
    /// Ejecutable del programa (`PROG_TARGET`), para ejecutar y depurar.
    pub programa: Option<String>,
    /// Ejecutable de las pruebas (`TEST_TARGET`).
    pub pruebas: Option<String>,
    /// Bibliotecas que usa (`LIB_NAME` de un ejercicio).
    pub dependencias: Vec<String>,
    /// Versión de `library.json`.
    pub version: Option<String>,
    pub repositorio: Option<String>,
}

impl Modulo {
    /// Comando de shell que corre `objetivo` en este módulo, desde la raíz del proyecto.
    pub fn comando(&self, objetivo: &str) -> String {
        if self.rel.is_empty() {
            format!("make {}", crate::proyecto::shell_quote(objetivo))
        } else {
            format!(
                "make -C {} {}",
                crate::proyecto::shell_quote(&self.rel),
                crate::proyecto::shell_quote(objetivo)
            )
        }
    }

    pub fn tiene(&self, objetivo: &str) -> bool {
        self.objetivos.iter().any(|o| o.nombre == objetivo)
    }

    /// Ruta del ejecutable del programa, si se conoce.
    pub fn ruta_programa(&self) -> Option<PathBuf> {
        self.programa.as_ref().map(|p| self.dir.join(p))
    }
}

/// Comando de `tp.sh`.
#[derive(Debug, Clone, PartialEq)]
pub struct ComandoTp {
    /// Primera palabra (`add-lib`).
    pub comando: String,
    /// Uso completo (`add-lib <nombre> [url-git]`).
    pub uso: String,
    pub descripcion: String,
}

impl ComandoTp {
    /// true si el comando pide argumentos obligatorios (`<...>`).
    pub fn pide_argumentos(&self) -> bool {
        self.uso.contains('<')
    }
}

/// Módulo declarado o descrito a mano en `.jonesy.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ModuloConfig {
    /// Directorio relativo a la raíz.
    pub dir: String,
    pub nombre: String,
    pub tipo: Option<TipoModulo>,
    /// Ejecutable para ejecutar y depurar.
    pub programa: String,
    /// Descripciones de objetivos: `objetivos = { test = "Corre las pruebas de la lista" }`.
    pub objetivos: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Estructura {
    pub modulos: Vec<Modulo>,
    pub tp_sh: Vec<ComandoTp>,
}

const MAKEFILES: [&str; 3] = ["Makefile", "makefile", "GNUmakefile"];
const NO_EXPLORAR: [&str; 6] = ["build", "target", "node_modules", "testing", "obj", "bin"];

fn makefile_en(dir: &Path) -> Option<PathBuf> {
    MAKEFILES.iter().map(|n| dir.join(n)).find(|p| p.is_file())
}

/// Descripción por defecto de los objetivos habituales de la cátedra.
pub fn descripcion_conocida(objetivo: &str) -> Option<&'static str> {
    Some(match objetivo {
        "all" => "Compila todo",
        "run" => "Ejecuta el programa",
        "test" => "Compila y corre las pruebas",
        "memcheck" => "Corre las pruebas con Valgrind",
        "fallos" => "Inyección de fallos con vasquez",
        "clean" => "Borra objetos y ejecutables",
        "librerias" => "Compila las bibliotecas",
        "ejercicios" => "Compila los ejercicios",
        "install" => "Instala",
        "format" => "Formatea el código",
        "lint" => "Revisa el estilo",
        _ => return None,
    })
}

fn es_nombre_de_objetivo(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && !s.starts_with('-')
}

/// Asignaciones simples `NOMBRE [?:+]= valor` del Makefile (sin expandir).
pub fn variables_makefile(texto: &str) -> Vec<(String, String)> {
    texto
        .lines()
        .filter(|l| !l.starts_with(['\t', '#']))
        .filter_map(|l| {
            let i = l.find('=')?;
            let (izq, der) = l.split_at(i);
            let nombre = izq.trim_end_matches([':', '?', '+', ' ']).trim();
            (es_nombre_de_objetivo(nombre) && nombre.chars().any(|c| c.is_ascii_uppercase()))
                .then(|| (nombre.to_string(), der[1..].trim().to_string()))
        })
        .collect()
}

/// Objetivos de un Makefile con su descripción, en orden de aparición.
pub fn objetivos_makefile(texto: &str) -> Vec<Objetivo> {
    let mut salida: Vec<Objetivo> = vec![];
    let mut comentario: Vec<String> = vec![];
    for linea in texto.lines() {
        if let Some(c) = linea.strip_prefix('#') {
            let c = c.trim_start_matches('#').trim();
            if !c.is_empty() {
                comentario.push(c.to_string());
            }
            continue;
        }
        if linea.trim().is_empty() || linea.starts_with(['\t', ' ']) {
            if linea.trim().is_empty() {
                comentario.clear();
            }
            continue;
        }
        let Some(i) = linea.find(':') else {
            comentario.clear();
            continue;
        };
        let (izq, der) = linea.split_at(i);
        // Asignaciones (`:=`, `::=`) y variables, no reglas.
        if der.starts_with(":=") || der.starts_with("::=") || izq.contains('=') {
            comentario.clear();
            continue;
        }
        let en_linea = der.split_once("##").map(|(_, d)| d.trim().to_string());
        let descripcion = en_linea
            .filter(|d| !d.is_empty())
            .or_else(|| comentario.last().cloned())
            .unwrap_or_default();
        // `.PHONY: run` suele ir entre el comentario y la regla: no corta el bloque.
        if izq.trim() == ".PHONY" {
            continue;
        }
        for nombre in izq.split_whitespace() {
            if !es_nombre_de_objetivo(nombre) {
                continue;
            }
            let descripcion = if descripcion.is_empty() {
                descripcion_conocida(nombre).unwrap_or("").to_string()
            } else {
                descripcion.clone()
            };
            match salida.iter_mut().find(|o| o.nombre == nombre) {
                Some(o) if o.descripcion.is_empty() => o.descripcion = descripcion,
                Some(_) => {}
                None => salida.push(Objetivo {
                    nombre: nombre.to_string(),
                    descripcion,
                }),
            }
        }
        comentario.clear();
    }
    salida
}

/// Comandos de `tp.sh`, leídos de las líneas de ayuda `echo -e "  ${VERDE}uso${NC}  descripción"`.
pub fn comandos_tp_sh(texto: &str) -> Vec<ComandoTp> {
    let mut v: Vec<ComandoTp> = vec![];
    for linea in texto.lines() {
        let l = linea.trim();
        if !l.starts_with("echo") {
            continue;
        }
        let Some(ini) = l.find("${VERDE}") else {
            continue;
        };
        let resto = &l[ini + "${VERDE}".len()..];
        let Some(fin) = resto.find("${NC}") else {
            continue;
        };
        let uso = resto[..fin].trim();
        let despues = resto[fin + "${NC}".len()..].trim_end_matches(['"', ';']);
        // Sólo líneas de ayuda: el comando va seguido de su descripción tras varios espacios.
        if !despues.starts_with("  ")
            || uso.is_empty()
            || !uso.chars().next().unwrap().is_ascii_lowercase()
        {
            continue;
        }
        let comando = uso.split_whitespace().next().unwrap_or("").to_string();
        if v.iter().any(|c| c.comando == comando) {
            continue;
        }
        v.push(ComandoTp {
            comando,
            uso: uso.to_string(),
            descripcion: despues.trim().to_string(),
        });
    }
    v
}

fn tipo_de(raiz: &Path, dir: &Path, makefile: &str) -> TipoModulo {
    let padre = dir
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_lowercase());
    let padre = padre.as_deref().unwrap_or("");
    if dir == raiz {
        TipoModulo::Raiz
    } else if matches!(padre, "libs" | "lib" | "librerias" | "bibliotecas")
        || dir.join("library.json").is_file()
        || makefile.contains("lib$(")
        || makefile.contains("ar rcs")
    {
        TipoModulo::Libreria
    } else if matches!(padre, "ejercicios" | "ejercicio" | "programas")
        || dir.join("main.c").is_file()
    {
        TipoModulo::Ejercicio
    } else {
        TipoModulo::Otro
    }
}

fn leer_modulo(raiz: &Path, dir: &Path) -> Option<Modulo> {
    let makefile = makefile_en(dir)?;
    let texto = std::fs::read_to_string(&makefile).ok()?;
    let rel = dir
        .strip_prefix(raiz)
        .unwrap_or(dir)
        .to_string_lossy()
        .replace('\\', "/");
    let vars = variables_makefile(&texto);
    let var = |n: &str| {
        vars.iter()
            .rev()
            .find(|(k, _)| k == n)
            .map(|(_, v)| v.clone())
            .filter(|v| !v.is_empty() && !v.contains('$'))
    };
    let tipo = tipo_de(raiz, dir, &texto);
    let mut nombre = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (mut version, mut repositorio) = (None, None);
    if let Ok(j) = std::fs::read_to_string(dir.join("library.json")) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&j) {
            if let Some(n) = v["name"].as_str() {
                nombre = n.to_string();
            }
            version = v["version"].as_str().map(str::to_string);
            repositorio = v["repository"].as_str().map(str::to_string);
        }
    }
    let dependencias = if tipo == TipoModulo::Ejercicio {
        var("LIB_NAME")
            .map(|v| v.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default()
    } else {
        vec![]
    };
    Some(Modulo {
        nombre,
        dir: dir.to_path_buf(),
        rel,
        tipo,
        objetivos: objetivos_makefile(&texto),
        programa: var("PROG_TARGET")
            .or_else(|| var("TARGET"))
            .or_else(|| var("PROG")),
        pruebas: var("TEST_TARGET"),
        dependencias,
        version,
        repositorio,
    })
}

fn subdirectorios(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|it| {
            it.flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .filter(|p| {
                    let n = p
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    !n.starts_with('.') && !NO_EXPLORAR.contains(&n.as_str())
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

impl Estructura {
    /// Recorre la raíz y hasta dos niveles de subdirectorios buscando Makefiles.
    pub fn descubrir(raiz: &Path, extra: &[ModuloConfig]) -> Estructura {
        let mut modulos = vec![];
        if let Some(m) = leer_modulo(raiz, raiz) {
            modulos.push(m);
        }
        for d1 in subdirectorios(raiz) {
            if let Some(m) = leer_modulo(raiz, &d1) {
                modulos.push(m);
            }
            for d2 in subdirectorios(&d1) {
                if let Some(m) = leer_modulo(raiz, &d2) {
                    modulos.push(m);
                }
            }
        }
        for c in extra {
            let dir = raiz.join(&c.dir);
            let i = match modulos.iter().position(|m| m.dir == dir) {
                Some(i) => i,
                None => match leer_modulo(raiz, &dir) {
                    Some(m) => {
                        modulos.push(m);
                        modulos.len() - 1
                    }
                    None => continue,
                },
            };
            let m = &mut modulos[i];
            if !c.nombre.is_empty() {
                m.nombre = c.nombre.clone();
            }
            if let Some(t) = c.tipo {
                m.tipo = t;
            }
            if !c.programa.is_empty() {
                m.programa = Some(c.programa.clone());
            }
            for (obj, desc) in &c.objetivos {
                match m.objetivos.iter_mut().find(|o| &o.nombre == obj) {
                    Some(o) => o.descripcion = desc.clone(),
                    None => m.objetivos.push(Objetivo {
                        nombre: obj.clone(),
                        descripcion: desc.clone(),
                    }),
                }
            }
        }
        modulos.sort_by(|a, b| (a.tipo, &a.rel).cmp(&(b.tipo, &b.rel)));
        let tp_sh = std::fs::read_to_string(raiz.join("tp.sh"))
            .map(|t| comandos_tp_sh(&t))
            .unwrap_or_default();
        Estructura { modulos, tp_sh }
    }

    /// Módulo más profundo (no la raíz) cuyo directorio contiene `ruta`; si no hay, la raíz.
    pub fn modulo_de(&self, ruta: &Path) -> Option<&Modulo> {
        self.modulos
            .iter()
            .filter(|m| m.tipo != TipoModulo::Raiz && ruta.starts_with(&m.dir))
            .max_by_key(|m| m.dir.components().count())
            .or_else(|| self.raiz())
    }

    pub fn raiz(&self) -> Option<&Modulo> {
        self.modulos.iter().find(|m| m.tipo == TipoModulo::Raiz)
    }

    pub fn vacia(&self) -> bool {
        self.modulos.is_empty() && self.tp_sh.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAKE_EJERCICIO: &str = "\
CC ?= gcc
LIB_NAME ?= arreglos cadenas
PROG_TARGET = programa
TEST_TARGET = test_bin

# Compilar ambos: programa y tests
all: $(PROG_TARGET) $(TEST_TARGET)

# Compilar el programa principal
$(PROG_TARGET): main.o
\t$(CC) -o $@ $^

# Regla genérica para compilar archivos .o a partir de .c
%.o: %.c
\t$(CC) -c $<

# Ejecutar el programa principal
.PHONY: run
run: $(PROG_TARGET)
\t./$(PROG_TARGET)

.PHONY: test
test: $(TEST_TARGET) ## Corre prueba.c
\t./$(TEST_TARGET)

clean:
\trm -f *.o
";

    #[test]
    fn objetivos_con_descripciones() {
        let o = objetivos_makefile(MAKE_EJERCICIO);
        let nombres: Vec<&str> = o.iter().map(|o| o.nombre.as_str()).collect();
        assert_eq!(nombres, ["all", "run", "test", "clean"]);
        assert_eq!(o[0].descripcion, "Compilar ambos: programa y tests");
        assert_eq!(o[1].descripcion, "Ejecutar el programa principal");
        assert_eq!(
            o[2].descripcion, "Corre prueba.c",
            "## en la misma línea gana"
        );
        assert_eq!(
            o[3].descripcion, "Borra objetos y ejecutables",
            "descripción conocida"
        );
        assert!(o[1].interactivo() && !o[2].interactivo());
    }

    #[test]
    fn variables_simples() {
        let v = variables_makefile(MAKE_EJERCICIO);
        assert!(v.contains(&("PROG_TARGET".into(), "programa".into())));
        assert!(v.contains(&("LIB_NAME".into(), "arreglos cadenas".into())));
    }

    #[test]
    fn comandos_de_tp_sh() {
        let t = r#"
mostrar_ayuda() {
    echo -e "  ${VERDE}sync${NC}                            Sincroniza los Makefiles."
    echo -e "  ${VERDE}add-lib <nombre> [url-git]${NC}      Agrega una librería."
    echo -e "Makefile principal -> ${VERDE}Regenerado${NC}"
}"#;
        let c = comandos_tp_sh(t);
        assert_eq!(c.len(), 2);
        assert_eq!(c[1].comando, "add-lib");
        assert_eq!(c[1].uso, "add-lib <nombre> [url-git]");
        assert!(c[1].pide_argumentos() && !c[0].pide_argumentos());
    }

    #[test]
    fn descubre_un_tp_de_la_catedra() {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        let escribir = |rel: &str, t: &str| {
            let p = r.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, t).unwrap();
        };
        escribir(
            "Makefile",
            "# Compila todo\nall: librerias\n\nlibrerias:\n\t@true\n",
        );
        escribir(
            "libs/arreglos/Makefile",
            "LIBRARY_NAME = lib$(LIB_NAME).a\nall: $(LIBRARY_NAME)\ntest:\n\t./test_bin\n",
        );
        escribir("libs/matematica/Makefile", "all:\n\t@true\n");
        escribir(
            "libs/matematica/library.json",
            r#"{"name": "matematica", "version": "1.1.0"}"#,
        );
        escribir("ejercicios/ejercicio1/Makefile", MAKE_EJERCICIO);
        escribir(
            "ejercicios/ejercicio1/main.c",
            "int main(void) { return 0; }\n",
        );
        escribir(
            "tp.sh",
            "echo -e \"  ${VERDE}build${NC}     Compila todo el proyecto.\"\n",
        );
        let e = Estructura::descubrir(r, &[]);
        let resumen: Vec<(TipoModulo, &str)> =
            e.modulos.iter().map(|m| (m.tipo, m.rel.as_str())).collect();
        assert_eq!(
            resumen,
            [
                (TipoModulo::Raiz, ""),
                (TipoModulo::Libreria, "libs/arreglos"),
                (TipoModulo::Libreria, "libs/matematica"),
                (TipoModulo::Ejercicio, "ejercicios/ejercicio1"),
            ]
        );
        let ej = &e.modulos[3];
        assert_eq!(ej.programa.as_deref(), Some("programa"));
        assert_eq!(ej.dependencias, ["arreglos", "cadenas"]);
        assert_eq!(ej.comando("test"), "make -C ejercicios/ejercicio1 test");
        assert_eq!(e.modulos[0].comando("all"), "make all");
        assert_eq!(e.modulos[2].version.as_deref(), Some("1.1.0"));
        assert_eq!(e.tp_sh[0].comando, "build");
        let m = e
            .modulo_de(&r.join("ejercicios/ejercicio1/main.c"))
            .unwrap();
        assert_eq!(m.rel, "ejercicios/ejercicio1");
        assert_eq!(
            e.modulo_de(&r.join("notas.md")).unwrap().tipo,
            TipoModulo::Raiz
        );
        // .jonesy.toml puede describir objetivos y forzar el programa.
        let extra = ModuloConfig {
            dir: "ejercicios/ejercicio1".into(),
            programa: "tp".into(),
            objetivos: [("test".to_string(), "Pruebas de la lista".to_string())].into(),
            ..Default::default()
        };
        let e = Estructura::descubrir(r, &[extra]);
        let ej = e
            .modulos
            .iter()
            .find(|m| m.rel == "ejercicios/ejercicio1")
            .unwrap();
        assert_eq!(ej.programa.as_deref(), Some("tp"));
        assert_eq!(
            ej.objetivos
                .iter()
                .find(|o| o.nombre == "test")
                .unwrap()
                .descripcion,
            "Pruebas de la lista"
        );
    }

    #[test]
    fn plantilla_tp_real() {
        let Some(home) = std::env::var_os("HOME") else {
            return;
        };
        let p = Path::new(&home).join("dev/tools/plantillas/plantilla-TP");
        if !p.is_dir() {
            return;
        }
        let e = Estructura::descubrir(&p, &[]);
        assert!(e.modulos.iter().any(|m| m.tipo == TipoModulo::Libreria));
        let ej = e
            .modulos
            .iter()
            .find(|m| m.tipo == TipoModulo::Ejercicio)
            .expect("hay ejercicios");
        assert!(ej.tiene("run") && ej.tiene("test") && ej.tiene("memcheck"));
        assert!(e.tp_sh.iter().any(|c| c.comando == "add-ex"));
        let raiz = e.raiz().unwrap();
        assert!(raiz.tiene("test") && raiz.tiene("librerias"));
    }
}
