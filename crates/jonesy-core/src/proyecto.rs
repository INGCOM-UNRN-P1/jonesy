//! Proyecto abierto: raíz, configuración `.jonesy.toml` y tareas de compilación/ejecución.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const ARCHIVO_CONFIG: &str = ".jonesy.toml";
pub const DIR_ESTADO: &str = ".jonesy";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub compilar: Tarea,
    pub ejecutar: Tarea,
    pub depurar: Depurar,
    pub examen: Examen,
    pub interfaz: Interfaz,
    pub ripley: Ripley,
    /// Descripciones o módulos extra para la vista de estructura (`[[modulo]]`).
    pub modulo: Vec<crate::estructura::ModuloConfig>,
    pub formato: Formato,
    pub documentacion: Documentacion,
    /// Herramientas del ecosistema integradas como plugins.
    pub integraciones: crate::integraciones::ConfigIntegraciones,
    pub pruebas: Pruebas,
    /// Atajos propios: `"ctrl+e" = "Compilar"` (tecla → acción; ganan a los de fábrica).
    pub atajos: std::collections::BTreeMap<String, String>,
}

/// `[pruebas]`: cómo correr y leer las pruebas (Construir → Correr pruebas).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Pruebas {
    /// Comando de las pruebas (vacío = el objetivo `test` del Makefile del módulo).
    pub comando: String,
    /// Archivo SARIF que dejan las pruebas, relativo a la raíz (vacío = buscarlo en la
    /// salida). Se usa cuando la salida no es de `p1_test`.
    pub sarif: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Documentacion {
    /// Directorio donde `make doc` (corbel) deja `api.json` (vacío = `doc`).
    pub dir: String,
    /// Avisar en la barra cuando falta o quedó vieja: true siempre, false nunca; sin
    /// indicarlo, sólo si el proyecto la usa (objetivo doc, corbel.toml o doc/).
    pub aviso: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Formato {
    /// Comando que formatea en el lugar el archivo `{archivo}` (vacío = `gaff format` si
    /// está instalado, si no `clang-format -i`).
    pub comando: String,
    /// Sacar los espacios y tabs del final de las líneas al guardar (no en Markdown).
    pub limpiar_espacios: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Tarea {
    /// Comando de shell; vacío = el predeterminado según el proyecto.
    pub comando: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Depurar {
    /// Comando que compila con símbolos de depuración (vacío = el predeterminado).
    pub compilar: String,
    /// Ejecutable a depurar (vacío = el predeterminado).
    pub programa: String,
    /// Argumentos del programa.
    pub argumentos: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
#[derive(Default)]
pub struct Examen {
    /// Enunciado de solo lectura (Markdown).
    pub consigna: String,
    /// Duración total en minutos (0 = sin cronómetro).
    pub duracion_min: u64,
    /// Comando que verifica la entrega (vacío = `make test` si el Makefile lo define).
    pub verificar: String,
    /// SHA-256 de los plugins Python que la cátedra autoriza en modo examen.
    pub plugins_permitidos: Vec<String>,
    /// Archivos que forman la entrega (globs simples `*.c`); vacío = *.c, *.h, *.md, Makefile.
    pub archivos: Vec<String>,
    /// SHA-256 del código con el que el docente pausa el cronómetro (vacío = sin pausa).
    pub pausa_sha256: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PosicionDoc {
    #[default]
    Derecha,
    Inferior,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Interfaz {
    pub quickdoc: PosicionDoc,
    /// Emitir OSC 52 al copiar (portapapeles del sistema en sesiones remotas).
    pub osc52: bool,
    /// Forzar caracteres ASCII en lugar de los de caja.
    pub ascii: bool,
    /// Recordar hacer commit tras estos minutos con cambios sin confirmar (0: nunca).
    pub recordar_commit_min: f64,
    /// Tema de colores: "oscuro" (por defecto), "claro", "contraste" (16 colores ANSI),
    /// "turbo" (como Turbo C++ de Borland), "solarizado" o "fosforo" (monitor verde).
    /// El elegido desde Archivo → Configuración del entorno → Apariencia queda en la sesión y tiene prioridad.
    pub tema: String,
    /// Shell de la terminal interna (vacío = $SHELL o bash).
    pub shell: String,
    /// Instalación del entorno de la cátedra (vacío = buscarla en P1_ENTORNO o
    /// ~/p1/entorno; "no" = no usarla).
    pub entorno: String,
    /// Tamaños iniciales de los paneles (`[interfaz.paneles]`); los cambios hechos con el
    /// mouse o desde Configuración del entorno → Apariencia → Tamaños de paneles se guardan en la sesión.
    pub paneles: TamPaneles,
}

/// Tamaños de los paneles redimensionables.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct TamPaneles {
    /// Ancho de la barra izquierda (explorador y proyecto), en columnas.
    pub izquierdo: u16,
    /// Ancho de la consigna en % (0 = según el modo: 25 % espacioso, 35 % estándar).
    pub consigna: u16,
    /// Ancho del quick-doc a la derecha, en %.
    pub quickdoc: u16,
    /// Alto del quick-doc en el dock inferior, en filas (incluye el borde).
    pub dock: u16,
    /// Ancho del panel lateral derecho (git, reglas, variables), en %.
    pub lateral: u16,
    /// Alto del panel inferior (problemas, terminal, depurador), en filas.
    pub inferior: u16,
}

impl Default for TamPaneles {
    fn default() -> Self {
        TamPaneles {
            izquierdo: 32,
            consigna: 0,
            quickdoc: 25,
            dock: 4,
            lateral: 30,
            inferior: 10,
        }
    }
}

impl TamPaneles {
    /// Ajusta cada valor a su rango válido.
    pub fn acotar(self) -> TamPaneles {
        TamPaneles {
            izquierdo: self.izquierdo.clamp(14, 80),
            consigna: if self.consigna == 0 {
                0
            } else {
                self.consigna.clamp(15, 60)
            },
            quickdoc: self.quickdoc.clamp(15, 50),
            dock: self.dock.clamp(3, 20),
            lateral: self.lateral.clamp(20, 60),
            inferior: self.inferior.clamp(4, 40),
        }
    }
}

impl Default for Interfaz {
    fn default() -> Self {
        Interfaz {
            quickdoc: PosicionDoc::Derecha,
            osc52: true,
            ascii: false,
            tema: String::new(),
            recordar_commit_min: 30.0,
            shell: String::new(),
            entorno: String::new(),
            paneles: TamPaneles::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Ripley {
    /// Correr `ripley check` al guardar archivos C.
    pub activo: bool,
    pub comando: String,
}

impl Default for Ripley {
    fn default() -> Self {
        Ripley {
            activo: true,
            comando: "ripley".into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Proyecto {
    pub raiz: PathBuf,
    pub config: Config,
    /// Error al leer `.jonesy.toml`, para mostrarlo en la barra de estado.
    pub error_config: Option<String>,
    /// Módulos con Makefile (bibliotecas, ejercicios) y comandos de `tp.sh`.
    pub estructura: crate::estructura::Estructura,
}

impl Proyecto {
    /// Abre el proyecto en `raiz` (un directorio). Si `.jonesy.toml` no existe, usa los valores por defecto.
    pub fn abrir(raiz: impl Into<PathBuf>) -> Proyecto {
        let raiz = raiz.into();
        let raiz = crate::canonica(&raiz);
        let (config, error_config) = match std::fs::read_to_string(raiz.join(ARCHIVO_CONFIG)) {
            Ok(texto) => match toml::from_str::<Config>(&texto) {
                Ok(c) => (c, None),
                Err(e) => (Config::default(), Some(format!("{ARCHIVO_CONFIG}: {e}"))),
            },
            Err(_) => (Config::default(), None),
        };
        let estructura = crate::estructura::Estructura::descubrir(&raiz, &config.modulo);
        Proyecto {
            raiz,
            config,
            error_config,
            estructura,
        }
    }

    pub fn dir_estado(&self) -> PathBuf {
        self.raiz.join(DIR_ESTADO)
    }

    pub fn makefile(&self) -> Option<PathBuf> {
        ["Makefile", "makefile", "GNUmakefile"]
            .iter()
            .map(|n| self.raiz.join(n))
            .find(|p| p.is_file())
    }

    /// Objetivos declarados en el Makefile (líneas `objetivo:` sin sangría).
    pub fn objetivos_make(&self) -> Vec<String> {
        let Some(ruta) = self.makefile() else {
            return Vec::new();
        };
        let Ok(texto) = std::fs::read_to_string(ruta) else {
            return Vec::new();
        };
        texto
            .lines()
            .filter(|l| !l.starts_with(['\t', ' ', '#', '.']))
            .filter_map(|l| {
                let (antes, despues) = l.split_once(':')?;
                (!despues.starts_with('=') && !antes.contains('=') && !antes.contains('$')).then(
                    || {
                        antes
                            .split_whitespace()
                            .map(String::from)
                            .collect::<Vec<_>>()
                    },
                )
            })
            .flatten()
            .collect()
    }

    /// Comando de compilación (F6) para el archivo activo.
    pub fn comando_compilar(&self, activo: Option<&Path>) -> Option<String> {
        if !self.config.compilar.comando.trim().is_empty() {
            return Some(self.config.compilar.comando.clone());
        }
        // El módulo (biblioteca o ejercicio con Makefile) del archivo activo.
        if let Some(m) = self.modulo_activo(activo) {
            // Un módulo enlaza sus bibliotecas (y las de ellas): se compilan antes.
            let mut pasos: Vec<String> = self
                .bibliotecas_de(m)
                .iter()
                .map(|lib| format!("make -C {}", shell_quote(&lib.rel)))
                .collect();
            pasos.push(format!("make -C {}", shell_quote(&m.rel)));
            return Some(pasos.join(" && "));
        }
        if self.makefile().is_some() {
            return Some("make".into());
        }
        let archivo = activo.filter(|p| p.extension().is_some_and(|e| e == "c"))?;
        let rel = self.relativa(archivo);
        let stem = archivo.file_stem()?.to_string_lossy().into_owned();
        Some(format!(
            "gcc -std=c11 -Wall -Wextra -g -O0 -o {} {}",
            shell_quote(&stem),
            shell_quote(&rel)
        ))
    }

    /// Comando de ejecución (Ctrl+F5): compilar y correr.
    pub fn comando_ejecutar(&self, activo: Option<&Path>) -> Option<String> {
        if !self.config.ejecutar.comando.trim().is_empty() {
            return Some(self.config.ejecutar.comando.clone());
        }
        let compilar = self.comando_compilar(activo)?;
        if let Some(m) = self.modulo_activo(activo) {
            if m.tiene("run") {
                return Some(m.comando("run"));
            }
            if let Some(p) = &m.programa {
                // Desde el directorio del módulo, para que encuentre sus archivos de datos.
                return Some(format!(
                    "{compilar} && cd {} && ./{}",
                    shell_quote(&m.rel),
                    shell_quote(p)
                ));
            }
        }
        if self.makefile().is_some() {
            if self.objetivos_make().iter().any(|o| o == "run") {
                return Some("make run".into());
            }
            return Some(compilar);
        }
        let stem = activo?.file_stem()?.to_string_lossy().into_owned();
        Some(format!("{compilar} && ./{}", shell_quote(&stem)))
    }

    /// Pruebas del archivo activo: el objetivo `test` de su módulo o, si no, el de la raíz.
    /// Usa la estructura ya leída (se puede llamar al dibujar).
    pub fn comando_probar(&self, activo: Option<&Path>) -> Option<String> {
        use crate::estructura::TipoModulo;
        if !self.config.pruebas.comando.trim().is_empty() {
            return Some(self.config.pruebas.comando.clone());
        }
        activo
            .and_then(|a| self.estructura.modulo_de(a))
            .filter(|m| m.tiene("test"))
            .or_else(|| {
                self.estructura
                    .modulos
                    .iter()
                    .find(|m| m.tipo == TipoModulo::Raiz && m.tiene("test"))
            })
            .map(|m| m.comando("test"))
    }

    /// Limpiar lo compilado: el objetivo `clean` del módulo (o de la raíz); en un archivo
    /// suelto sin Makefile, borrar su ejecutable.
    pub fn comando_limpiar(&self, activo: Option<&Path>) -> Option<String> {
        use crate::estructura::TipoModulo;
        if let Some(m) = activo
            .and_then(|a| self.estructura.modulo_de(a))
            .filter(|m| m.tiene("clean"))
            .or_else(|| {
                self.estructura
                    .modulos
                    .iter()
                    .find(|m| m.tipo == TipoModulo::Raiz && m.tiene("clean"))
            })
        {
            return Some(m.comando("clean"));
        }
        if self.makefile().is_some() || crate::proceso::interprete().0 != "sh" {
            return None;
        }
        let stem = activo?.file_stem()?.to_string_lossy().into_owned();
        Some(format!("rm -f {}", shell_quote(&stem)))
    }

    /// Revisar la memoria con valgrind: el objetivo `memcheck` del módulo (o de la raíz);
    /// si no hay, compilar y correr el programa del archivo activo bajo valgrind.
    pub fn comando_memoria(&self, activo: Option<&Path>) -> Option<String> {
        use crate::estructura::TipoModulo;
        let modulo = activo.and_then(|a| self.estructura.modulo_de(a));
        if let Some(m) = modulo.filter(|m| m.tiene("memcheck")).or_else(|| {
            self.estructura
                .modulos
                .iter()
                .find(|m| m.tipo == TipoModulo::Raiz && m.tiene("memcheck"))
        }) {
            return Some(m.comando("memcheck"));
        }
        let valgrind = "valgrind --leak-check=full --error-exitcode=1";
        let compilar = self.comando_compilar(activo)?;
        if let Some(m) = self.modulo_activo(activo) {
            let p = m.programa.as_ref()?;
            return Some(format!(
                "{compilar} && cd {} && {valgrind} ./{}",
                shell_quote(&m.rel),
                shell_quote(p)
            ));
        }
        if self.makefile().is_some() {
            return None;
        }
        let stem = activo?.file_stem()?.to_string_lossy().into_owned();
        Some(format!("{compilar} && {valgrind} ./{}", shell_quote(&stem)))
    }

    /// Módulo con Makefile que contiene al archivo activo (no la raíz).
    pub fn modulo_activo(&self, activo: Option<&Path>) -> Option<&crate::estructura::Modulo> {
        let m = self.estructura.modulo_de(activo?)?;
        (m.tipo != crate::estructura::TipoModulo::Raiz).then_some(m)
    }

    /// Bibliotecas que usa un módulo (LIB_NAME de un ejercicio, `dependencies` de
    /// library.json o `DEPENDENCIAS` del Makefile de una biblioteca), incluidas las
    /// dependencias de esas bibliotecas, en orden de compilación (primero las de abajo).
    pub fn bibliotecas_de(&self, m: &crate::estructura::Modulo) -> Vec<&crate::estructura::Modulo> {
        fn visitar<'a>(
            p: &'a Proyecto,
            nombres: &[String],
            orden: &mut Vec<&'a crate::estructura::Modulo>,
            camino: &mut Vec<String>,
        ) {
            for n in nombres {
                let Some(lib) = p.estructura.modulos.iter().find(|x| {
                    x.tipo == crate::estructura::TipoModulo::Libreria
                        && (x.nombre == *n || x.dir.file_name().is_some_and(|f| f == n.as_str()))
                }) else {
                    continue;
                };
                // Ciclos (a usa b y b usa a): se cortan.
                if camino.contains(&lib.rel) || orden.iter().any(|x| x.rel == lib.rel) {
                    continue;
                }
                camino.push(lib.rel.clone());
                visitar(p, &lib.dependencias, orden, camino);
                camino.pop();
                orden.push(lib);
            }
        }
        let mut orden = vec![];
        visitar(self, &m.dependencias, &mut orden, &mut vec![m.rel.clone()]);
        orden
    }

    /// Ejecutable a depurar: `[depurar] programa`, el del módulo activo o el `.c` sin extensión.
    pub fn programa_a_depurar(&self, activo: Option<&Path>) -> Option<PathBuf> {
        let cfg = &self.config.depurar.programa;
        if !cfg.trim().is_empty() {
            return Some(self.raiz.join(cfg));
        }
        if let Some(p) = self.modulo_activo(activo).and_then(|m| m.ruta_programa()) {
            return Some(con_exe(p));
        }
        let activo = activo?;
        let stem = activo.file_stem()?.to_string_lossy().into_owned();
        let dir = activo.parent().unwrap_or(&self.raiz);
        [dir.join(&stem), self.raiz.join(&stem)]
            .into_iter()
            .map(con_exe)
            .find(|p| p.is_file())
            .or(Some(con_exe(dir.join(stem))))
    }

    /// Comando de autoformato para el archivo `archivo` (relativo a la raíz, se formatea
    /// en el lugar). None si no hay formateador.
    pub fn comando_formato(&self, archivo: &str) -> Option<String> {
        let plantilla = if !self.config.formato.comando.trim().is_empty() {
            self.config.formato.comando.clone()
        } else if en_path("gaff") {
            "gaff format {archivo}".to_string()
        } else if en_path("clang-format") {
            "clang-format -i {archivo}".to_string()
        } else {
            return None;
        };
        let q = shell_quote(archivo);
        Some(if plantilla.contains("{archivo}") {
            plantilla.replace("{archivo}", &q)
        } else {
            format!("{plantilla} {q}")
        })
    }

    /// Comando que verifica la entrega del examen.
    pub fn comando_verificar(&self) -> Option<String> {
        if !self.config.examen.verificar.trim().is_empty() {
            return Some(self.config.examen.verificar.clone());
        }
        if self.objetivos_make().iter().any(|o| o == "test") {
            return Some("make test".into());
        }
        // Sin objetivo test: el corrector local del entorno de la cátedra, si está.
        en_path("verificar").then(|| "verificar .".into())
    }

    /// Consigna del examen: la configurada o `consigna.md` / `enunciado.md` en la raíz.
    pub fn consigna(&self) -> Option<PathBuf> {
        if !self.config.examen.consigna.is_empty() {
            return Some(self.raiz.join(&self.config.examen.consigna));
        }
        ["consigna.md", "enunciado.md", "CONSIGNA.md"]
            .iter()
            .map(|n| self.raiz.join(n))
            .find(|p| p.is_file())
    }

    pub fn relativa(&self, ruta: &Path) -> String {
        ruta.strip_prefix(&self.raiz)
            .unwrap_or(ruta)
            .to_string_lossy()
            .into_owned()
    }

    /// El par de un fuente de C: el `.h` de un `.c` y el `.c` de un `.h` (Alt+I). Busca en
    /// el mismo directorio, después en `include/` o `src/` del módulo (o del directorio de
    /// arriba) y, si no, en todo el proyecto por nombre.
    pub fn archivo_pareja(&self, ruta: &Path) -> Option<PathBuf> {
        let ext = ruta.extension()?.to_str()?.to_ascii_lowercase();
        let otra = match ext.as_str() {
            "c" => "h",
            "h" => "c",
            _ => return None,
        };
        let stem = ruta.file_stem()?.to_string_lossy().into_owned();
        let nombre = format!("{stem}.{otra}");
        let dir = ruta.parent()?;
        let mut candidatos = vec![dir.join(&nombre)];
        let base_modulo = self
            .estructura
            .modulo_de(ruta)
            .map(|m| m.dir.clone())
            .unwrap_or_else(|| self.raiz.clone());
        for base in [dir.parent().map(Path::to_path_buf), Some(base_modulo)]
            .into_iter()
            .flatten()
        {
            for sub in ["include", "src", "inc", "lib"] {
                candidatos.push(base.join(sub).join(&nombre));
            }
            candidatos.push(base.join(&nombre));
        }
        if let Some(c) = candidatos.into_iter().find(|c| c.is_file()) {
            return Some(c);
        }
        let mut en_proyecto: Vec<PathBuf> = self
            .archivos(5000)
            .into_iter()
            .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy() == nombre))
            .collect();
        // El más cercano al archivo (más componentes en común).
        en_proyecto.sort_by_key(|p| {
            std::cmp::Reverse(
                p.components()
                    .zip(ruta.components())
                    .take_while(|(a, b)| a == b)
                    .count(),
            )
        });
        en_proyecto.into_iter().next()
    }

    /// Archivos del proyecto (para abrir con Ctrl+O), sin directorios ocultos ni binarios conocidos.
    pub fn archivos(&self, limite: usize) -> Vec<PathBuf> {
        let mut salida = Vec::new();
        let mut pila = vec![self.raiz.clone()];
        while let Some(dir) = pila.pop() {
            let Ok(entradas) = std::fs::read_dir(&dir) else {
                continue;
            };
            let mut entradas: Vec<_> = entradas.flatten().collect();
            entradas.sort_by_key(|e| e.file_name());
            for e in entradas {
                let nombre = e.file_name().to_string_lossy().into_owned();
                if nombre.starts_with('.') || nombre == "target" || nombre == "build" {
                    continue;
                }
                let ruta = e.path();
                if ruta.is_dir() {
                    pila.push(ruta);
                } else if es_editable(&ruta) {
                    salida.push(ruta);
                    if salida.len() >= limite {
                        return salida;
                    }
                }
            }
        }
        salida.sort();
        salida
    }
}

pub fn es_editable(ruta: &Path) -> bool {
    let nombre = ruta
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if nombre == "makefile" || nombre == "gnumakefile" {
        return true;
    }
    matches!(
        ruta.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some(
            "mk" | "c"
                | "h"
                | "md"
                | "txt"
                | "toml"
                | "yaml"
                | "yml"
                | "in"
                | "out"
                | "json"
                | "py"
                | "sh"
                | "gift"
        )
    )
}

/// En Windows gcc agrega `.exe` al ejecutable: si existe, se usa esa ruta.
fn con_exe(p: PathBuf) -> PathBuf {
    if cfg!(windows) && !p.is_file() {
        let exe = p.with_extension("exe");
        if exe.is_file() {
            return exe;
        }
    }
    p
}

/// true si `programa` está en el PATH (también `programa.exe` en Windows).
pub fn en_path(programa: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|d| {
        d.join(programa).is_file() || (cfg!(windows) && d.join(format!("{programa}.exe")).is_file())
    })
}

/// Comillas simples para `sh -c`.
pub fn shell_quote(s: &str) -> String {
    comillas_para(crate::proceso::interprete().0, s)
}

/// Comillas para el intérprete dado: simples para sh; dobles para cmd (Windows sin sh),
/// que no entiende las simples.
pub fn comillas_para(interprete: &str, s: &str) -> String {
    let seguro = |c: char| {
        c.is_ascii_alphanumeric() || "._-/+".contains(c) || (interprete == "cmd" && c == '\\')
    };
    if !s.is_empty() && s.chars().all(seguro) {
        s.to_string()
    } else if interprete == "cmd" {
        format!("\"{}\"", s.replace('"', "\\\""))
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn comillas_segun_el_interprete() {
        assert_eq!(comillas_para("sh", "src/a.c"), "src/a.c");
        assert_eq!(comillas_para("sh", "mi tp/a.c"), "'mi tp/a.c'");
        assert_eq!(comillas_para("sh", "o'neil.c"), "'o'\\''neil.c'");
        assert_eq!(comillas_para("cmd", "mi tp\\a.c"), "\"mi tp\\a.c\"");
        assert_eq!(comillas_para("cmd", "src\\a.c"), "src\\a.c");
    }

    use super::*;

    #[test]
    fn pareja_de_c_y_h() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        for f in [
            "lista.c",
            "lista.h",
            "libs/pila/src/pila.c",
            "libs/pila/include/pila.h",
            "ejercicios/ej1/cola.c",
            "otros/cola.h",
        ] {
            std::fs::create_dir_all(r.join(f).parent().unwrap()).unwrap();
            std::fs::write(r.join(f), "").unwrap();
        }
        let p = Proyecto::abrir(r);
        assert_eq!(
            p.archivo_pareja(&r.join("lista.c")),
            Some(r.join("lista.h"))
        );
        assert_eq!(
            p.archivo_pareja(&r.join("lista.h")),
            Some(r.join("lista.c"))
        );
        assert_eq!(
            p.archivo_pareja(&r.join("libs/pila/src/pila.c")),
            Some(r.join("libs/pila/include/pila.h"))
        );
        assert_eq!(
            p.archivo_pareja(&r.join("libs/pila/include/pila.h")),
            Some(r.join("libs/pila/src/pila.c"))
        );
        // En otro lugar del proyecto, por nombre.
        assert_eq!(
            p.archivo_pareja(&r.join("ejercicios/ej1/cola.c")),
            Some(r.join("otros/cola.h"))
        );
        assert_eq!(p.archivo_pareja(&r.join("notas.md")), None);
    }

    #[test]
    fn comando_de_formato() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(".jonesy.toml"),
            "[formato]\ncomando = \"astyle --style=allman {archivo}\"\n",
        )
        .unwrap();
        let p = Proyecto::abrir(dir.path());
        assert_eq!(
            p.comando_formato("src/a b.c").unwrap(),
            "astyle --style=allman 'src/a b.c'"
        );
        assert!(en_path("sh"));
        assert!(!en_path("no-existe-este-programa-jonesy"));
    }

    #[test]
    fn compilar_y_ejecutar_por_modulo() {
        let dir = tempfile::tempdir().unwrap();
        let ej = dir.path().join("ejercicios/ej1");
        std::fs::create_dir_all(&ej).unwrap();
        std::fs::write(dir.path().join("Makefile"), "all:\n\t@true\n").unwrap();
        std::fs::write(
            ej.join("Makefile"),
            "PROG_TARGET = programa\nall:\n\t@true\nrun:\n\t./programa\n",
        )
        .unwrap();
        std::fs::write(ej.join("main.c"), "int main(void) { return 0; }\n").unwrap();
        let p = Proyecto::abrir(dir.path());
        let main = p.raiz.join("ejercicios/ej1/main.c");
        assert_eq!(
            p.comando_compilar(Some(&main)).unwrap(),
            "make -C ejercicios/ej1"
        );
        // Con LIB_NAME, primero las bibliotecas que usa.
        std::fs::create_dir_all(dir.path().join("libs/arreglos")).unwrap();
        std::fs::write(dir.path().join("libs/arreglos/Makefile"), "all:\n\t@true\n").unwrap();
        std::fs::write(
            ej.join("Makefile"),
            "LIB_NAME ?= arreglos\nPROG_TARGET = programa\nall:\n\t@true\nrun:\n\t./programa\n",
        )
        .unwrap();
        let p2 = Proyecto::abrir(dir.path());
        assert_eq!(
            p2.comando_compilar(Some(&main)).unwrap(),
            "make -C libs/arreglos && make -C ejercicios/ej1"
        );
        // Dependencias transitivas: arreglos usa memoria (DEPENDENCIAS) y memoria va antes.
        std::fs::create_dir_all(dir.path().join("libs/memoria")).unwrap();
        std::fs::write(dir.path().join("libs/memoria/Makefile"), "all:\n\t@true\n").unwrap();
        std::fs::write(
            dir.path().join("libs/arreglos/Makefile"),
            "DEPENDENCIAS = memoria\nall:\n\t@true\n",
        )
        .unwrap();
        let p3 = Proyecto::abrir(dir.path());
        assert_eq!(
            p3.comando_compilar(Some(&main)).unwrap(),
            "make -C libs/memoria && make -C libs/arreglos && make -C ejercicios/ej1"
        );
        // mem-trace del ejercicio: su main.c, sus fuentes y los de las bibliotecas.
        std::fs::write(ej.join("lista.c"), "").unwrap();
        // Unas pruebas con otro nombre tienen su propio main: no se compilan con el programa.
        std::fs::write(
            ej.join("pruebas.c"),
            "/* int main(void) en un comentario no cuenta */\nint main(int argc, char *argv[])\n{\n    return 0;\n}\n",
        )
        .unwrap();
        std::fs::write(
            ej.join("utiles.c"),
            "/* int main(void) */\nint doble(int x) { return 2 * x; }\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("libs/arreglos/arreglos.c"), "").unwrap();
        let mt = crate::catscan::comando_memtrace(&p3, &main, &p3.raiz.join(".jonesy/memtrace"))
            .unwrap();
        assert!(mt.starts_with("mem-trace run ejercicios/ej1/main.c -I ejercicios/ej1 --cc-arg=ejercicios/ej1/lista.c"), "{mt}");
        assert!(!mt.contains("pruebas.c") && mt.contains("utiles.c"), "{mt}");
        assert!(
            mt.contains("-I libs/memoria") && mt.contains("--cc-arg=libs/arreglos/arreglos.c"),
            "{mt}"
        );
        assert_eq!(
            p.comando_ejecutar(Some(&main)).unwrap(),
            "make -C ejercicios/ej1 run"
        );
        assert_eq!(
            p.programa_a_depurar(Some(&main)).unwrap(),
            p.raiz.join("ejercicios/ej1/programa")
        );
        // Fuera de un módulo, el Makefile de la raíz.
        assert_eq!(
            p.comando_compilar(Some(&p.raiz.join("notas.md"))).unwrap(),
            "make"
        );
    }

    #[test]
    fn compilar_sin_makefile_usa_gcc_con_el_archivo_activo() {
        let dir = tempfile::tempdir().unwrap();
        let p = Proyecto::abrir(dir.path());
        let a = p.raiz.join("lista.c");
        assert_eq!(
            p.comando_compilar(Some(&a)).unwrap(),
            "gcc -std=c11 -Wall -Wextra -g -O0 -o lista lista.c"
        );
        assert_eq!(
            p.comando_ejecutar(Some(&a)).unwrap(),
            "gcc -std=c11 -Wall -Wextra -g -O0 -o lista lista.c && ./lista"
        );
        assert!(p.comando_compilar(Some(&p.raiz.join("notas.md"))).is_none());
    }

    #[test]
    fn makefile_y_config() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Makefile"),
            "CC=gcc\nall: prog\nrun: all\n\t./prog\ntest:\n\t./t\n",
        )
        .unwrap();
        let p = Proyecto::abrir(dir.path());
        assert_eq!(p.objetivos_make(), vec!["all", "run", "test"]);
        assert_eq!(p.comando_compilar(None).unwrap(), "make");
        assert_eq!(p.comando_ejecutar(None).unwrap(), "make run");
        assert_eq!(p.comando_verificar().unwrap(), "make test");

        std::fs::write(
            dir.path().join(ARCHIVO_CONFIG),
            "[compilar]\ncomando = \"make -j2\"\n[examen]\nconsigna = \"enunciado.md\"\nduracion_min = 90\n[interfaz]\nquickdoc = \"inferior\"\n",
        )
        .unwrap();
        let p = Proyecto::abrir(dir.path());
        assert!(p.error_config.is_none());
        assert_eq!(p.comando_compilar(None).unwrap(), "make -j2");
        assert_eq!(p.config.examen.duracion_min, 90);
        assert_eq!(p.config.interfaz.quickdoc, PosicionDoc::Inferior);
        assert_eq!(p.consigna().unwrap(), p.raiz.join("enunciado.md"));
    }

    #[test]
    fn config_invalida_se_informa() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(ARCHIVO_CONFIG), "[compilar\n").unwrap();
        assert!(Proyecto::abrir(dir.path()).error_config.is_some());
    }

    #[test]
    fn comillas() {
        assert_eq!(shell_quote("a.c"), "a.c");
        assert_eq!(shell_quote("mi tp.c"), "'mi tp.c'");
    }
}
