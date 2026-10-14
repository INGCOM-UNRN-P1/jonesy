//! Núcleo de `jonesy`: buffers, proyecto, sesión, procesos y diagnósticos.
//!
//! No depende de la terminal: lo usan la TUI (`jonesy-tui`) y los modos sin pantalla
//! (`--headless`, benchmarks) del binario.

pub mod buffer;
pub mod busqueda;
pub mod casos;
pub mod catalogo;
pub mod catscan;
pub mod diagnostico;
pub mod diff;
pub mod doctor;
pub mod documentacion;
pub mod documentar;
pub mod entorno;
pub mod entrega;
pub mod errores_gcc;
pub mod estructura;
pub mod fallos;
pub mod gdb;
pub mod glosario;
pub mod hex;
pub mod integraciones;
pub mod precedencia;
pub mod preferencias;
pub mod proceso;
pub mod proyecto;
pub mod pruebas;
pub mod renombrar;
pub mod reporte;
pub mod sarif;
pub mod sesion;
pub mod simbolos;
pub mod valgrind;

pub use buffer::{Buffer, Delta, Lenguaje};
pub use diagnostico::{Diagnostico, Origen, Severidad};
pub use proyecto::{Config, PosicionDoc, Proyecto, TamPaneles};

/// Ruta absoluta canónica. En Windows `canonicalize` devuelve `\\?\C:\…`, que no se
/// compara bien con las rutas de git (`C:/…`) ni se muestra prolija: se quita el prefijo.
pub fn canonica(p: &std::path::Path) -> std::path::PathBuf {
    let c = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    #[cfg(windows)]
    {
        let s = c.to_string_lossy();
        if let Some(resto) = s.strip_prefix(r"\\?\") {
            if !resto.starts_with("UNC") {
                return std::path::PathBuf::from(resto);
            }
        }
    }
    c
}

/// SHA-256 en hexadecimal.
pub fn sha256_hex(datos: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    ::hex::encode(Sha256::digest(datos))
}

/// Usuario del sistema (para la telemetría y la entrega).
pub fn usuario() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "desconocido".into())
}

pub fn ahora_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
