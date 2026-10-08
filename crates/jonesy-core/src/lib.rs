//! Núcleo de `jonesy`: buffers, proyecto, sesión, procesos y diagnósticos.
//!
//! No depende de la terminal: lo usan la TUI (`jonesy-tui`) y los modos sin pantalla
//! (`--headless`, benchmarks) del binario.

pub mod buffer;
pub mod catscan;
pub mod diagnostico;
pub mod entrega;
pub mod estructura;
pub mod gdb;
pub mod proceso;
pub mod proyecto;
pub mod sesion;

pub use buffer::{Buffer, Delta, Lenguaje};
pub use diagnostico::{Diagnostico, Origen, Severidad};
pub use proyecto::{Config, PosicionDoc, Proyecto, TamPaneles};

/// SHA-256 en hexadecimal.
pub fn sha256_hex(datos: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(datos))
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
