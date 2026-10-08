//! Estado de sesión persistido en `.jonesy/session.json` (buffers, cursores y paneles) y
//! copias de recuperación ante un corte de la terminal.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::buffer::guardar_atomico;
use crate::proyecto::PosicionDoc;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Sesion {
    pub archivos: Vec<ArchivoAbierto>,
    pub activo: usize,
    pub quickdoc_visible: bool,
    pub quickdoc_posicion: Option<PosicionDoc>,
    /// Vista del modo compacto: consigna, codigo, terminal, doc.
    pub vista_compacta: String,
    pub preview_markdown: bool,
    pub breakpoints: Vec<(String, usize)>,
    /// Tamaños de paneles ajustados por el usuario.
    pub paneles: Option<crate::proyecto::TamPaneles>,
    /// Barra izquierda visible y su pestaña ("archivos" o "proyecto").
    pub izquierdo: Option<String>,
    /// Directorios expandidos del explorador (relativos a la raíz).
    pub expandidos: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ArchivoAbierto {
    /// Relativa a la raíz del proyecto.
    pub ruta: String,
    pub cursor: usize,
    pub desplazamiento: usize,
}

impl Sesion {
    pub fn ruta(dir_estado: &Path) -> PathBuf {
        dir_estado.join("session.json")
    }

    pub fn cargar(dir_estado: &Path) -> Option<Sesion> {
        let texto = std::fs::read_to_string(Self::ruta(dir_estado)).ok()?;
        serde_json::from_str(&texto).ok()
    }

    pub fn guardar(&self, dir_estado: &Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        guardar_atomico(&Self::ruta(dir_estado), json.as_bytes()).map(|_| ())
    }
}

/// Estado persistente del examen (sobrevive a reinicios: el cronómetro no vuelve a cero).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct EstadoExamen {
    pub inicio_unix: u64,
    pub duracion_min: u64,
    pub entregado: bool,
    pub paquete: Option<String>,
    pub sha256: Option<String>,
}

impl EstadoExamen {
    pub fn ruta(dir_estado: &Path) -> PathBuf {
        dir_estado.join("examen.json")
    }

    pub fn cargar(dir_estado: &Path) -> Option<EstadoExamen> {
        serde_json::from_str(&std::fs::read_to_string(Self::ruta(dir_estado)).ok()?).ok()
    }

    pub fn guardar(&self, dir_estado: &Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        guardar_atomico(&Self::ruta(dir_estado), json.as_bytes()).map(|_| ())
    }

    /// Segundos restantes (negativo si se pasó el tiempo); None sin cronómetro.
    pub fn restante_s(&self, ahora_unix: u64) -> Option<i64> {
        (self.duracion_min > 0)
            .then(|| (self.inicio_unix + self.duracion_min * 60) as i64 - ahora_unix as i64)
    }
}

/// Copias de recuperación: buffers modificados sin guardar al recibir SIGHUP fuera del
/// modo examen (en examen se guardan en el lugar).
pub fn ruta_recuperacion(dir_estado: &Path, relativa: &str) -> PathBuf {
    let nombre: String = relativa
        .chars()
        .map(|c| if c == '/' || c == '\\' { '%' } else { c })
        .collect();
    dir_estado.join("recuperacion").join(nombre)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_y_vuelta() {
        let dir = tempfile::tempdir().unwrap();
        let s = Sesion {
            archivos: vec![ArchivoAbierto {
                ruta: "src/a.c".into(),
                cursor: 12,
                desplazamiento: 3,
            }],
            activo: 0,
            quickdoc_visible: true,
            quickdoc_posicion: Some(PosicionDoc::Inferior),
            vista_compacta: "codigo".into(),
            preview_markdown: false,
            breakpoints: vec![("src/a.c".into(), 7)],
            paneles: Some(crate::proyecto::TamPaneles {
                izquierdo: 30,
                ..Default::default()
            }),
            izquierdo: Some("proyecto".into()),
            expandidos: vec!["src".into()],
        };
        s.guardar(dir.path()).unwrap();
        assert_eq!(Sesion::cargar(dir.path()).unwrap(), s);
    }

    #[test]
    fn cronometro() {
        let e = EstadoExamen {
            inicio_unix: 1000,
            duracion_min: 2,
            ..Default::default()
        };
        assert_eq!(e.restante_s(1060), Some(60));
        assert_eq!(e.restante_s(1200), Some(-80));
        assert_eq!(EstadoExamen::default().restante_s(5), None);
    }

    #[test]
    fn nombre_de_recuperacion() {
        assert_eq!(
            ruta_recuperacion(Path::new("/x"), "src/a.c"),
            PathBuf::from("/x/recuperacion/src%a.c")
        );
    }
}

/// Proyectos abiertos recientemente (los más nuevos primero), guardados fuera del
/// proyecto: `$XDG_STATE_HOME/jonesy/recientes.txt` (o `~/.local/state/…`; en Windows
/// `%LOCALAPPDATA%\jonesy\recientes.txt`).
pub mod recientes {
    use std::path::{Path, PathBuf};

    pub const MAXIMO: usize = 12;

    pub fn archivo() -> Option<PathBuf> {
        if let Ok(d) = std::env::var("JONESY_ESTADO") {
            return Some(PathBuf::from(d).join("recientes.txt"));
        }
        let base = if cfg!(windows) {
            std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
        } else {
            std::env::var_os("XDG_STATE_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        };
        base.map(|b| b.join("jonesy").join("recientes.txt"))
    }

    pub fn cargar() -> Vec<PathBuf> {
        archivo()
            .and_then(|a| std::fs::read_to_string(a).ok())
            .map(|t| {
                t.lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(PathBuf::from)
                    .filter(|p| p.is_dir())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Pone `raiz` primera en la lista.
    pub fn registrar(raiz: &Path) {
        let Some(a) = archivo() else { return };
        let mut v = cargar();
        v.retain(|p| p != raiz);
        v.insert(0, raiz.to_path_buf());
        v.truncate(MAXIMO);
        if let Some(d) = a.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let texto: String = v.iter().map(|p| format!("{}\n", p.display())).collect();
        let _ = crate::buffer::guardar_atomico(&a, texto.as_bytes());
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn ultimos_primero_sin_repetir() {
            let estado = tempfile::tempdir().unwrap();
            let a = tempfile::tempdir().unwrap();
            let b = tempfile::tempdir().unwrap();
            // Variable propia del proceso de prueba: no afecta a otros tests.
            std::env::set_var("JONESY_ESTADO", estado.path());
            super::registrar(a.path());
            super::registrar(b.path());
            super::registrar(a.path());
            assert_eq!(
                super::cargar(),
                vec![a.path().to_path_buf(), b.path().to_path_buf()]
            );
        }
    }
}
