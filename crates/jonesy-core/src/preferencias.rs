//! Preferencias portables del usuario (Archivo → Configuración → Exportar / Importar y
//! `jonesy config exportar|importar`): tema, quick-doc, tamaños de paneles, vista previa
//! de Markdown, integraciones encendidas o apagadas y los manifiestos de integraciones
//! propios. Se guardan en un TOML legible para llevarlas a otra máquina; al importarlas
//! quedan además como punto de partida de los proyectos que todavía no tienen sesión.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::proyecto::{PosicionDoc, TamPaneles};
use crate::sesion::Sesion;

pub const VERSION: u32 = 1;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Preferencias {
    pub version: u32,
    pub tema: Option<String>,
    pub quickdoc_visible: Option<bool>,
    pub quickdoc_posicion: Option<PosicionDoc>,
    pub preview_markdown: Option<bool>,
    pub paneles: Option<TamPaneles>,
    /// Integración → encendida.
    pub integraciones: BTreeMap<String, bool>,
    /// Manifiestos de `~/.config/jonesy/integraciones/`.
    pub manifiesto: Vec<Manifiesto>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Manifiesto {
    pub archivo: String,
    pub contenido: String,
}

/// `$JONESY_CONFIG`, o `$XDG_CONFIG_HOME/jonesy` (`~/.config/jonesy`; en Windows
/// `%APPDATA%\jonesy`).
pub fn dir_config() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("JONESY_CONFIG") {
        return Some(PathBuf::from(d));
    }
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    };
    base.map(|b| b.join("jonesy"))
}

/// Donde quedan las preferencias importadas (el punto de partida de proyectos nuevos).
pub fn ruta_usuario() -> Option<PathBuf> {
    dir_config().map(|d| d.join("preferencias.toml"))
}

pub fn cargar_usuario() -> Option<Preferencias> {
    leer(&ruta_usuario()?).ok()
}

pub fn leer(ruta: &Path) -> Result<Preferencias, String> {
    let texto = std::fs::read_to_string(ruta).map_err(|e| format!("{}: {e}", ruta.display()))?;
    let p: Preferencias =
        toml::from_str(&texto).map_err(|e| format!("{}: {}", ruta.display(), e.message()))?;
    if p.version > VERSION {
        return Err(format!(
            "{} es de una versión más nueva de jonesy (formato {}).",
            ruta.display(),
            p.version
        ));
    }
    Ok(p)
}

pub fn escribir(p: &Preferencias, ruta: &Path) -> Result<(), String> {
    let texto = format!(
        "# Preferencias de jonesy (Archivo → Configuración → Importar, o\n# `jonesy config importar {}`).\n{}",
        ruta.file_name().unwrap_or_default().to_string_lossy(),
        toml::to_string_pretty(p).map_err(|e| e.to_string())?
    );
    if let Some(d) = ruta.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    crate::buffer::guardar_atomico(ruta, texto.as_bytes())
        .map(|_| ())
        .map_err(|e| format!("{}: {e}", ruta.display()))
}

/// Las preferencias de una sesión, con los manifiestos de `dir_integraciones`.
pub fn desde_sesion(s: &Sesion, dir_integraciones: Option<&Path>) -> Preferencias {
    let mut manifiesto = vec![];
    if let Some(Ok(it)) = dir_integraciones.map(std::fs::read_dir) {
        let mut rutas: Vec<PathBuf> = it
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "toml"))
            .collect();
        rutas.sort();
        for r in rutas {
            if let Ok(contenido) = std::fs::read_to_string(&r) {
                manifiesto.push(Manifiesto {
                    archivo: r.file_name().unwrap().to_string_lossy().into(),
                    contenido,
                });
            }
        }
    }
    Preferencias {
        version: VERSION,
        tema: s.tema.clone(),
        quickdoc_visible: Some(s.quickdoc_visible),
        quickdoc_posicion: s.quickdoc_posicion,
        preview_markdown: Some(s.preview_markdown),
        paneles: s.paneles,
        integraciones: s.integraciones.iter().cloned().collect(),
        manifiesto,
    }
}

/// Importa un archivo de preferencias: instala sus manifiestos en el directorio del
/// usuario y las deja como punto de partida de los proyectos sin sesión. Devuelve las
/// preferencias y los manifiestos escritos.
pub fn importar(ruta: &Path) -> Result<(Preferencias, Vec<String>), String> {
    let p = leer(ruta)?;
    let escritos = match crate::integraciones::dir_usuario() {
        Some(d) => p.instalar_manifiestos(&d)?,
        None if p.manifiesto.is_empty() => vec![],
        None => return Err("no encuentro el directorio de configuración del usuario".into()),
    };
    if let Some(destino) = ruta_usuario() {
        let base = Preferencias {
            manifiesto: vec![],
            version: VERSION,
            ..p.clone()
        };
        escribir(&base, &destino)?;
    }
    Ok((p, escritos))
}

/// Las preferencias para exportar fuera del IDE: las del usuario, pisadas por la sesión
/// del proyecto si tiene una.
pub fn actuales(dir_estado: Option<&Path>) -> Preferencias {
    let dir_int = crate::integraciones::dir_usuario();
    let mut p = cargar_usuario().unwrap_or_default();
    if let Some(s) = dir_estado.and_then(Sesion::cargar) {
        let de_sesion = desde_sesion(&s, None);
        let mut sesion = Sesion::default();
        p.aplicar_a(&mut sesion);
        de_sesion.aplicar_a(&mut sesion);
        p = desde_sesion(&sesion, None);
    }
    p.version = VERSION;
    p.manifiesto = desde_sesion(&Sesion::default(), dir_int.as_deref()).manifiesto;
    p
}

impl Preferencias {
    /// Pasa a una sesión lo que estas preferencias definen.
    pub fn aplicar_a(&self, s: &mut Sesion) {
        if let Some(t) = &self.tema {
            s.tema = Some(t.clone());
        }
        if let Some(v) = self.quickdoc_visible {
            s.quickdoc_visible = v;
        }
        if self.quickdoc_posicion.is_some() {
            s.quickdoc_posicion = self.quickdoc_posicion;
        }
        if let Some(v) = self.preview_markdown {
            s.preview_markdown = v;
        }
        if self.paneles.is_some() {
            s.paneles = self.paneles;
        }
        for (n, a) in &self.integraciones {
            match s.integraciones.iter_mut().find(|(x, _)| x == n) {
                Some(x) => x.1 = *a,
                None => s.integraciones.push((n.clone(), *a)),
            }
        }
    }

    /// Escribe los manifiestos en `dir`. Uno que ya existe con otro contenido se guarda
    /// antes como `<archivo>.anterior`. Devuelve los archivos escritos.
    pub fn instalar_manifiestos(&self, dir: &Path) -> Result<Vec<String>, String> {
        let mut escritos = vec![];
        for m in &self.manifiesto {
            let nombre = Path::new(&m.archivo)
                .file_name()
                .filter(|n| Path::new(n).extension().is_some_and(|e| e == "toml"))
                .ok_or_else(|| format!("«{}» no es un manifiesto .toml", m.archivo))?;
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            let destino = dir.join(nombre);
            match std::fs::read_to_string(&destino) {
                Ok(actual) if actual == m.contenido => continue,
                Ok(actual) => {
                    let mut copia = destino.clone().into_os_string();
                    copia.push(".anterior");
                    std::fs::write(copia, actual).map_err(|e| e.to_string())?;
                }
                Err(_) => {}
            }
            std::fs::write(&destino, &m.contenido).map_err(|e| e.to_string())?;
            escritos.push(nombre.to_string_lossy().into_owned());
        }
        Ok(escritos)
    }

    /// Resumen de una línea de lo que trae.
    pub fn resumen(&self) -> String {
        let mut v = vec![];
        if let Some(t) = &self.tema {
            v.push(format!("tema {t}"));
        }
        if self.quickdoc_visible.is_some() || self.quickdoc_posicion.is_some() {
            v.push("quick-doc".into());
        }
        if self.paneles.is_some() {
            v.push("paneles".into());
        }
        if !self.integraciones.is_empty() {
            v.push(format!("{} integraciones", self.integraciones.len()));
        }
        if !self.manifiesto.is_empty() {
            v.push(format!("{} manifiestos", self.manifiesto.len()));
        }
        if v.is_empty() {
            "nada".into()
        } else {
            v.join(", ")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exporta_e_importa() {
        let d = tempfile::tempdir().unwrap();
        let origen = d.path().join("origen");
        std::fs::create_dir_all(&origen).unwrap();
        std::fs::write(origen.join("mia.toml"), "nombre = \"mia\"\n").unwrap();
        let s = Sesion {
            tema: Some("turbo".into()),
            quickdoc_visible: true,
            quickdoc_posicion: Some(PosicionDoc::Inferior),
            integraciones: vec![("gaff".into(), false)],
            archivos: vec![Default::default()],
            ..Default::default()
        };
        let p = desde_sesion(&s, Some(&origen));
        let ruta = d.path().join("prefs.toml");
        escribir(&p, &ruta).unwrap();
        let leida = leer(&ruta).unwrap();
        assert_eq!(leida, p);
        assert!(leida.resumen().contains("tema turbo"));

        // Aplicar sólo toca preferencias, no los archivos abiertos.
        let mut otra = Sesion {
            integraciones: vec![("gaff".into(), true), ("kaneda".into(), true)],
            ..Default::default()
        };
        leida.aplicar_a(&mut otra);
        assert_eq!(otra.tema.as_deref(), Some("turbo"));
        assert_eq!(otra.quickdoc_posicion, Some(PosicionDoc::Inferior));
        assert_eq!(
            otra.integraciones,
            vec![("gaff".into(), false), ("kaneda".into(), true)]
        );
        assert!(otra.archivos.is_empty());

        // Los manifiestos: uno distinto que ya estaba queda como .anterior.
        let destino = d.path().join("destino");
        std::fs::create_dir_all(&destino).unwrap();
        std::fs::write(destino.join("mia.toml"), "viejo").unwrap();
        assert_eq!(
            leida.instalar_manifiestos(&destino).unwrap(),
            vec!["mia.toml"]
        );
        assert_eq!(
            std::fs::read_to_string(destino.join("mia.toml.anterior")).unwrap(),
            "viejo"
        );
        assert!(leida.instalar_manifiestos(&destino).unwrap().is_empty());

        std::fs::write(&ruta, "version = 99\n").unwrap();
        assert!(leer(&ruta).unwrap_err().contains("más nueva"));
        let mala = Preferencias {
            manifiesto: vec![Manifiesto {
                archivo: "../../.bashrc".into(),
                contenido: String::new(),
            }],
            ..Default::default()
        };
        assert!(mala.instalar_manifiestos(&destino).is_err());
    }
}
