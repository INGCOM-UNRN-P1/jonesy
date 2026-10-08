//! `jonesy doctor`: qué herramientas del entorno y del ecosistema están disponibles, con
//! el sobre JSON común del ecosistema (`yutani.doctor`, LINEAMIENTOS §3.2):
//!
//! ```json
//! {"schema_version": "1.0.0", "herramienta": "jonesy", "version": "…", "ok": true,
//!  "chequeos": [{"nombre", "requerido", "ok", "detalle", "proposito", "sugerencia"}]}
//! ```

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;

pub const SCHEMA_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Chequeo {
    pub nombre: String,
    pub requerido: bool,
    pub ok: bool,
    pub detalle: String,
    pub proposito: String,
    pub sugerencia: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Informe {
    pub schema_version: String,
    pub herramienta: String,
    pub version: String,
    pub ok: bool,
    pub chequeos: Vec<Chequeo>,
}

/// Ruta de `nombre` en el PATH (también `.exe` en Windows).
pub fn buscar(nombre: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|d| {
        [d.join(nombre), d.join(format!("{nombre}.exe"))]
            .into_iter()
            .find(|p| p.is_file())
    })
}

/// Primera línea de `programa --version`, con un tiempo límite (algunas herramientas
/// arrancan lento o esperan entrada).
fn version(ruta: &Path, limite: Duration) -> String {
    let Ok(mut hijo) = Command::new(ruta)
        .arg("--version")
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    else {
        return "versión desconocida".into();
    };
    let fin = Instant::now() + limite;
    loop {
        match hijo.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < fin => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = hijo.kill();
                let _ = hijo.wait();
                return "versión desconocida (no respondió)".into();
            }
        }
    }
    let salida = hijo.wait_with_output().ok();
    let texto = salida
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stdout).into_owned();
            if s.trim().is_empty() {
                String::from_utf8_lossy(&o.stderr).into_owned()
            } else {
                s
            }
        })
        .unwrap_or_default();
    texto
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("versión desconocida")
        .to_string()
}

/// Chequeo de un binario: si está en el PATH y su versión.
pub fn binario(nombre: &str, proposito: &str, requerido: bool, sugerencia: &str) -> Chequeo {
    match buscar(nombre) {
        None => Chequeo {
            nombre: nombre.into(),
            requerido,
            ok: false,
            detalle: "No encontrado en el PATH".into(),
            proposito: proposito.into(),
            sugerencia: sugerencia.into(),
        },
        Some(ruta) => Chequeo {
            nombre: nombre.into(),
            requerido,
            ok: true,
            detalle: format!(
                "{} ({})",
                version(&ruta, Duration::from_secs(5)),
                ruta.display()
            ),
            proposito: proposito.into(),
            sugerencia: String::new(),
        },
    }
}

pub fn informe(version: &str, chequeos: Vec<Chequeo>) -> Informe {
    Informe {
        schema_version: SCHEMA_VERSION.into(),
        herramienta: "jonesy".into(),
        version: version.into(),
        ok: chequeos.iter().filter(|c| c.requerido).all(|c| c.ok),
        chequeos,
    }
}

/// Sugerencia de instalación para una herramienta del ecosistema.
pub fn sugerencia_ecosistema(nombre: &str) -> String {
    format!("uv tool install git+https://github.com/INGCOM-UNRN-P1/{nombre}")
}

/// Los chequeos de jonesy en `raiz`: el entorno de compilación y depuración, y las
/// integraciones del ecosistema (de fábrica y las del proyecto).
pub fn chequeos(raiz: &Path, integraciones: &[crate::integraciones::Integracion]) -> Vec<Chequeo> {
    let mut v = vec![
        binario(
            "gcc",
            "Compilar (F6)",
            true,
            "Instalá gcc (sudo dnf install gcc / sudo apt install gcc)",
        ),
        binario(
            "make",
            "Makefiles del proyecto",
            true,
            "Instalá make (sudo dnf install make / sudo apt install make)",
        ),
        binario("git", "Panel de git, diff y clonar", false, "Instalá git"),
        binario(
            "gdb",
            "Depurador (F5)",
            false,
            "Instalá gdb (sudo dnf install gdb / sudo apt install gdb)",
        ),
        binario(
            "mem-trace",
            "cat-scan: vista de la memoria al depurar",
            false,
            &sugerencia_ecosistema("mem-trace"),
        ),
        binario(
            "python3",
            "Plugins Python (.jonesy/plugins)",
            false,
            "Instalá Python 3",
        ),
    ];
    let entorno = crate::entorno::detectar("");
    v.push(Chequeo {
        nombre: "entorno".into(),
        requerido: false,
        ok: entorno.is_some(),
        detalle: match &entorno {
            Some(e) => format!(
                "{}{}",
                e.raiz.display(),
                if e.gdbinit.is_some() {
                    " (con gdbinit de cátedra)"
                } else {
                    ""
                }
            ),
            None => "No encontrado (P1_ENTORNO, ~/p1/entorno)".into(),
        },
        proposito: "Comandos y variables de la cátedra en la terminal y el depurador".into(),
        sugerencia: if entorno.is_some() {
            String::new()
        } else {
            "https://github.com/INGCOM-UNRN-P1/entorno".into()
        },
    });
    let mut vistos: Vec<String> = vec!["gcc".into(), "make".into(), "git".into(), "gdb".into()];
    for i in integraciones {
        if i.binario.is_empty() || vistos.contains(&i.binario) {
            continue;
        }
        vistos.push(i.binario.clone());
        let proposito = if i.nombre == i.binario {
            i.descripcion.clone()
        } else {
            format!("{} ({})", i.descripcion, i.nombre)
        };
        let mut c = binario(
            &i.binario,
            &proposito,
            false,
            &sugerencia_ecosistema(&i.binario),
        );
        if c.ok && !i.activa {
            c.detalle.push_str(&format!(
                " · apagada: [integraciones] activar = [\"{}\"]",
                i.nombre
            ));
        }
        v.push(c);
    }
    let _ = raiz;
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sobre_comun() {
        let c = vec![
            binario("sh", "shell", true, ""),
            binario("no-existe-jonesy-xyz", "nada", false, "instalalo"),
        ];
        assert!(c[0].ok && !c[1].ok);
        assert_eq!(c[1].sugerencia, "instalalo");
        let i = informe("0.1.0", c);
        assert!(i.ok, "lo que falta no es requerido");
        let json = serde_json::to_value(&i).unwrap();
        assert_eq!(json["schema_version"], "1.0.0");
        assert_eq!(json["herramienta"], "jonesy");
        for k in [
            "nombre",
            "requerido",
            "ok",
            "detalle",
            "proposito",
            "sugerencia",
        ] {
            assert!(json["chequeos"][0].get(k).is_some(), "falta {k}");
        }
        let mal = informe("0.1.0", vec![binario("no-existe-jonesy-xyz", "", true, "")]);
        assert!(!mal.ok);
    }
}
