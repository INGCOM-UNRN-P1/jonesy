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
