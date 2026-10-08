//! Barra izquierda: explorador de archivos y estructura del proyecto.
//!
//! - **Archivos**: árbol de directorios con carpetas desplegables, el archivo activo
//!   resaltado y el estado de git de cada archivo.
//! - **Proyecto**: módulos con Makefile agrupados en bibliotecas, ejercicios y otros, con
//!   sus objetivos (y lo que hace cada uno) listos para correr, y los comandos de `tp.sh`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use jonesy_core::estructura::{Estructura, TipoModulo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PestanaNav {
    Archivos,
    Proyecto,
}

impl PestanaNav {
    pub fn nombre(self) -> &'static str {
        match self {
            PestanaNav::Archivos => "archivos",
            PestanaNav::Proyecto => "proyecto",
        }
    }

    pub fn de_nombre(s: &str) -> Option<PestanaNav> {
        match s {
            "archivos" => Some(PestanaNav::Archivos),
            "proyecto" => Some(PestanaNav::Proyecto),
            _ => None,
        }
    }
}

// ------------------------------------------------------------------ archivos

/// Directorios que no se muestran (estado interno de herramientas).
const OCULTOS: [&str; 3] = [".git", ".jonesy", "target"];

#[derive(Debug, Clone, PartialEq)]
pub struct FilaArchivo {
    pub ruta: PathBuf,
    pub nombre: String,
    pub nivel: usize,
    pub es_dir: bool,
}

#[derive(Debug, Default)]
pub struct Explorador {
    /// Directorios desplegados (absolutos).
    pub expandidos: BTreeSet<PathBuf>,
    pub sel: usize,
    pub filas: Vec<FilaArchivo>,
}

fn listar(dir: &Path) -> Vec<(PathBuf, String, bool)> {
    let mut v: Vec<(PathBuf, String, bool)> = std::fs::read_dir(dir)
        .map(|it| {
            it.flatten()
                .filter_map(|e| {
                    let nombre = e.file_name().to_string_lossy().into_owned();
                    if OCULTOS.contains(&nombre.as_str()) {
                        return None;
                    }
                    let ruta = e.path();
                    let es_dir = ruta.is_dir();
                    Some((ruta, nombre, es_dir))
                })
                .collect()
        })
        .unwrap_or_default();
    // Carpetas primero, después archivos; sin distinguir mayúsculas.
    v.sort_by_key(|e| (!e.2, e.1.to_lowercase()));
    v
}

impl Explorador {
    /// Recalcula las filas visibles a partir de los directorios desplegados.
    pub fn reconstruir(&mut self, raiz: &Path) {
        fn agregar(e: &Explorador, dir: &Path, nivel: usize, filas: &mut Vec<FilaArchivo>) {
            if filas.len() > 5000 {
                return;
            }
            for (ruta, nombre, es_dir) in listar(dir) {
                let abierto = es_dir && e.expandidos.contains(&ruta);
                filas.push(FilaArchivo {
                    ruta: ruta.clone(),
                    nombre,
                    nivel,
                    es_dir,
                });
                if abierto {
                    agregar(e, &ruta, nivel + 1, filas);
                }
            }
        }
        let mut filas = vec![];
        agregar(self, raiz, 0, &mut filas);
        self.filas = filas;
        self.sel = self.sel.min(self.filas.len().saturating_sub(1));
    }

    pub fn seleccionada(&self) -> Option<&FilaArchivo> {
        self.filas.get(self.sel)
    }

    /// Despliega o pliega la carpeta de la fila `i`.
    pub fn alternar(&mut self, i: usize, raiz: &Path) {
        let Some(f) = self.filas.get(i) else { return };
        if !f.es_dir {
            return;
        }
        let ruta = f.ruta.clone();
        if !self.expandidos.remove(&ruta) {
            self.expandidos.insert(ruta);
        }
        self.reconstruir(raiz);
    }

    /// Despliega las carpetas que contienen `ruta` y la selecciona.
    pub fn revelar(&mut self, ruta: &Path, raiz: &Path) {
        let mut d = ruta.parent();
        while let Some(dir) = d {
            if dir == raiz || !dir.starts_with(raiz) {
                break;
            }
            self.expandidos.insert(dir.to_path_buf());
            d = dir.parent();
        }
        self.reconstruir(raiz);
        if let Some(i) = self.filas.iter().position(|f| f.ruta == ruta) {
            self.sel = i;
        }
    }

    /// Fila de la carpeta que contiene a la fila `i` (para ← sobre un archivo).
    pub fn padre(&self, i: usize) -> Option<usize> {
        let nivel = self.filas.get(i)?.nivel;
        (0..i)
            .rev()
            .find(|&j| self.filas[j].nivel + 1 == nivel && self.filas[j].es_dir)
    }

    /// Directorio donde crear algo nuevo: la carpeta seleccionada o la del archivo seleccionado.
    pub fn directorio_destino(&self, raiz: &Path) -> PathBuf {
        match self.seleccionada() {
            Some(f) if f.es_dir => f.ruta.clone(),
            Some(f) => f
                .ruta
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| raiz.to_path_buf()),
            None => raiz.to_path_buf(),
        }
    }
}

/// Estado de git por ruta (relativa al repo) → letra a mostrar.
pub fn marcas_git(estado: Option<&jonesy_git::Estado>, repo: &Path) -> BTreeMap<PathBuf, char> {
    let mut m = BTreeMap::new();
    if let Some(e) = estado {
        for en in &e.entradas {
            let letra = match en.tipo {
                jonesy_git::TipoEntrada::SinSeguimiento => '?',
                jonesy_git::TipoEntrada::Conflicto => '!',
                jonesy_git::TipoEntrada::Ignorado => continue,
                jonesy_git::TipoEntrada::Cambio => {
                    if en.arbol != '.' {
                        en.arbol
                    } else {
                        en.indice
                    }
                }
            };
            let ruta = repo.join(en.ruta.trim_end_matches('/'));
            // Las carpetas muestran que tienen algo cambiado adentro.
            let mut d = ruta.parent();
            while let Some(dir) = d {
                if !dir.starts_with(repo) || dir == repo {
                    break;
                }
                m.entry(dir.to_path_buf()).or_insert('•');
                d = dir.parent();
            }
            m.insert(ruta, letra);
        }
    }
    m
}

// ------------------------------------------------------------------ proyecto

#[derive(Debug, Clone, PartialEq)]
pub enum FilaProyecto {
    Grupo(TipoModulo),
    Modulo(usize),
    Objetivo(usize, usize),
    TpSh,
    ComandoTp(usize),
}

#[derive(Debug, Default)]
pub struct VistaProyecto {
    pub sel: usize,
    /// Módulos desplegados, por directorio relativo ("" es la raíz).
    pub desplegados: BTreeSet<String>,
    pub tp_desplegado: bool,
}

impl VistaProyecto {
    pub fn filas(&self, e: &Estructura) -> Vec<FilaProyecto> {
        let mut v = vec![];
        let mut grupo: Option<TipoModulo> = None;
        for (i, m) in e.modulos.iter().enumerate() {
            if grupo != Some(m.tipo) {
                grupo = Some(m.tipo);
                v.push(FilaProyecto::Grupo(m.tipo));
            }
            v.push(FilaProyecto::Modulo(i));
            if self.desplegados.contains(&m.rel) {
                v.extend((0..m.objetivos.len()).map(|o| FilaProyecto::Objetivo(i, o)));
            }
        }
        if !e.tp_sh.is_empty() {
            v.push(FilaProyecto::TpSh);
            if self.tp_desplegado {
                v.extend((0..e.tp_sh.len()).map(FilaProyecto::ComandoTp));
            }
        }
        v
    }

    /// Despliega o pliega la fila (módulo o tp.sh).
    pub fn alternar(&mut self, fila: &FilaProyecto, e: &Estructura) {
        match fila {
            FilaProyecto::Modulo(i) => {
                let rel = e.modulos[*i].rel.clone();
                if !self.desplegados.remove(&rel) {
                    self.desplegados.insert(rel);
                }
            }
            FilaProyecto::TpSh => self.tp_desplegado = !self.tp_desplegado,
            _ => {}
        }
    }

    /// Despliega el módulo del archivo activo y lo selecciona.
    pub fn revelar(&mut self, e: &Estructura, ruta: &Path) {
        if let Some(m) = e.modulo_de(ruta) {
            self.desplegados.insert(m.rel.clone());
            let rel = m.rel.clone();
            if let Some(i) = self
                .filas(e)
                .iter()
                .position(|f| matches!(f, FilaProyecto::Modulo(j) if e.modulos[*j].rel == rel))
            {
                self.sel = i;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arbol() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for f in [
            "main.c",
            "src/lista.c",
            "src/lista.h",
            "Zeta.md",
            ".git/HEAD",
            ".jonesy/session.json",
        ] {
            let p = d.path().join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "").unwrap();
        }
        d
    }

    #[test]
    fn arbol_con_carpetas_primero_y_ocultos_filtrados() {
        let d = arbol();
        let mut e = Explorador::default();
        e.reconstruir(d.path());
        let nombres: Vec<&str> = e.filas.iter().map(|f| f.nombre.as_str()).collect();
        assert_eq!(nombres, ["src", "main.c", "Zeta.md"]);
        e.alternar(0, d.path());
        let nombres: Vec<(&str, usize)> = e
            .filas
            .iter()
            .map(|f| (f.nombre.as_str(), f.nivel))
            .collect();
        assert_eq!(
            nombres,
            [
                ("src", 0),
                ("lista.c", 1),
                ("lista.h", 1),
                ("main.c", 0),
                ("Zeta.md", 0)
            ]
        );
        assert_eq!(e.padre(2), Some(0));
        e.sel = 2;
        assert_eq!(e.directorio_destino(d.path()), d.path().join("src"));
    }

    #[test]
    fn revelar_despliega_hasta_el_archivo() {
        let d = arbol();
        let mut e = Explorador::default();
        e.revelar(&d.path().join("src/lista.h"), d.path());
        assert_eq!(e.seleccionada().unwrap().nombre, "lista.h");
    }

    #[test]
    fn filas_del_proyecto() {
        use jonesy_core::estructura::{Modulo, Objetivo};
        let m = |rel: &str, tipo, objs: &[&str]| Modulo {
            nombre: rel.rsplit('/').next().unwrap().into(),
            dir: PathBuf::from("/tp").join(rel),
            rel: rel.into(),
            tipo,
            objetivos: objs
                .iter()
                .map(|o| Objetivo {
                    nombre: o.to_string(),
                    descripcion: String::new(),
                })
                .collect(),
            programa: None,
            pruebas: None,
            dependencias: vec![],
            version: None,
            repositorio: None,
        };
        let e = Estructura {
            modulos: vec![
                m("", TipoModulo::Raiz, &["all", "test"]),
                m("libs/a", TipoModulo::Libreria, &["all"]),
                m(
                    "ejercicios/e1",
                    TipoModulo::Ejercicio,
                    &["all", "run", "test"],
                ),
            ],
            tp_sh: vec![],
        };
        let mut v = VistaProyecto::default();
        assert_eq!(v.filas(&e).len(), 6); // 3 grupos + 3 módulos plegados
        v.revelar(&e, Path::new("/tp/ejercicios/e1/main.c"));
        let f = v.filas(&e);
        assert_eq!(f[v.sel], FilaProyecto::Modulo(2));
        assert_eq!(f.len(), 9);
        assert_eq!(f[8], FilaProyecto::Objetivo(2, 2));
    }
}
