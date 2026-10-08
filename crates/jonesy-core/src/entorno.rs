//! Entorno de la cátedra (`INGCOM-UNRN-P1/entorno`) para la terminal interna, las
//! herramientas y el depurador (RF-5.3).
//!
//! `entorno` instala los comandos de cátedra (`verificar`, `entregar`, `ripley`…) y deja
//! sus variables en `unix/p1-env.sh`, que la shell carga desde `~/.bashrc` o desde
//! `unix/entrar`. jonesy no depende de que la shell que lo lanzó lo haya cargado: si
//! encuentra la instalación aplica lo mismo que ese script (variables `P1_ENTORNO`,
//! `P1_DEV` y los `bin` al principio del PATH), así el deck, la compilación y las
//! integraciones ven las mismas herramientas que la terminal de la cátedra. El depurador
//! usa además su `config/gdb/gdbinit`.
//!
//! En Windows la instalación portable deja `PORTABLE_ROOT`; se agrega `PORTABLE_ROOT/bin`.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct EntornoCatedra {
    /// Raíz de la instalación (`~/p1/entorno` o `P1_ENTORNO`).
    pub raiz: PathBuf,
    /// Directorios a anteponer al PATH.
    pub bins: Vec<PathBuf>,
    /// Variables a definir.
    pub variables: Vec<(String, String)>,
    /// `config/gdb/gdbinit`, si está.
    pub gdbinit: Option<PathBuf>,
}

/// Busca la instalación: `config` (de `.jonesy.toml`, `"no"` la desactiva), `P1_ENTORNO`,
/// `~/p1/entorno` o, en Windows, `PORTABLE_ROOT`.
pub fn detectar(config: &str) -> Option<EntornoCatedra> {
    if config.trim() == "no" {
        return None;
    }
    let candidatos: Vec<PathBuf> = if !config.trim().is_empty() {
        vec![PathBuf::from(config.trim())]
    } else {
        let mut v = vec![];
        if let Some(p) = std::env::var_os("P1_ENTORNO") {
            v.push(PathBuf::from(p));
        }
        if let Some(h) = std::env::var_os("HOME") {
            v.push(PathBuf::from(h).join("p1").join("entorno"));
        }
        v
    };
    for raiz in candidatos {
        if raiz.join("unix").join("bin").is_dir() {
            return Some(desde_unix(&raiz));
        }
    }
    if cfg!(windows) {
        if let Some(p) = std::env::var_os("PORTABLE_ROOT")
            .map(PathBuf::from)
            .filter(|p| p.is_dir())
        {
            let gdbinit =
                Some(p.join("config").join("gdb").join("gdbinit")).filter(|g| g.is_file());
            return Some(EntornoCatedra {
                bins: vec![p.join("bin")],
                variables: vec![("PORTABLE_ROOT".into(), p.to_string_lossy().into_owned())],
                gdbinit,
                raiz: p,
            });
        }
    }
    None
}

fn desde_unix(raiz: &Path) -> EntornoCatedra {
    // Igual que unix/p1-env.sh: P1_DEV por defecto es el hermano «dev» de la instalación.
    let dev = std::env::var_os("P1_DEV")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            raiz.parent()
                .map(|p| p.join("dev"))
                .unwrap_or_else(|| raiz.join("dev"))
        });
    let gdbinit = Some(raiz.join("config").join("gdb").join("gdbinit")).filter(|g| g.is_file());
    EntornoCatedra {
        raiz: raiz.to_path_buf(),
        bins: vec![
            raiz.join("unix").join("bin"),
            raiz.join("local").join("bin"),
        ],
        variables: vec![
            ("P1_ENTORNO".into(), raiz.to_string_lossy().into_owned()),
            ("P1_DEV".into(), dev.to_string_lossy().into_owned()),
        ],
        gdbinit,
    }
}

impl EntornoCatedra {
    /// PATH con los `bin` de la cátedra adelante (sin duplicarlos).
    pub fn path(&self, actual: &std::ffi::OsStr) -> std::ffi::OsString {
        let mut dirs: Vec<PathBuf> = self.bins.clone();
        for d in std::env::split_paths(actual) {
            if !dirs.contains(&d) {
                dirs.push(d);
            }
        }
        std::env::join_paths(dirs).unwrap_or_else(|_| actual.to_os_string())
    }

    /// Aplica las variables al proceso de jonesy (los hijos las heredan). Se llama al
    /// arrancar, antes de crear hilos.
    pub fn aplicar(&self) {
        let actual = std::env::var_os("PATH").unwrap_or_default();
        std::env::set_var("PATH", self.path(&actual));
        for (k, v) in &self.variables {
            std::env::set_var(k, v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instalacion_unix() {
        let d = tempfile::tempdir().unwrap();
        let raiz = d.path().join("p1").join("entorno");
        std::fs::create_dir_all(raiz.join("unix/bin")).unwrap();
        std::fs::create_dir_all(raiz.join("config/gdb")).unwrap();
        std::fs::write(raiz.join("config/gdb/gdbinit"), "set print pretty on\n").unwrap();
        let e = detectar(&raiz.to_string_lossy()).unwrap();
        assert_eq!(e.bins[0], raiz.join("unix/bin"));
        assert_eq!(
            e.gdbinit.as_deref(),
            Some(raiz.join("config/gdb/gdbinit").as_path())
        );
        assert!(e
            .variables
            .iter()
            .any(|(k, v)| k == "P1_ENTORNO" && v.ends_with("entorno")));
        let path = e.path(std::ffi::OsStr::new("/usr/bin"));
        let dirs: Vec<PathBuf> = std::env::split_paths(&path).collect();
        assert_eq!(dirs[0], raiz.join("unix/bin"));
        assert_eq!(dirs.last().unwrap(), Path::new("/usr/bin"));
        // Aplicar dos veces no duplica.
        let dos = e.path(&path);
        assert_eq!(std::env::split_paths(&dos).count(), dirs.len());
    }

    #[test]
    fn desactivado_o_ausente() {
        assert!(detectar("no").is_none());
        assert!(detectar("/no/existe/entorno").is_none());
    }
}
