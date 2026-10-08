//! Entrega asistida del examen (RF-6.5): selección de archivos, paquete `.tar` con la
//! bitácora sellada y un manifiesto, y su SHA-256.
//!
//! El paquete contiene:
//! - los archivos de la entrega (por defecto `*.c`, `*.h`, `*.md` y el Makefile),
//! - `telemetry.log` (la bitácora, ya cerrada con `SessionEnd`),
//! - `entrega.json`: usuario, momento, resultado de la verificación, último hash de la
//!   cadena de telemetría y el SHA-256 de cada archivo.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::proyecto::Proyecto;
use crate::sha256_hex;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArchivoEntregado {
    pub ruta: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Manifiesto {
    pub usuario: String,
    pub momento_unix: u64,
    pub version_jonesy: String,
    pub verificacion: Option<Verificacion>,
    /// Hash del último registro de la telemetría (sella la cadena).
    pub telemetria_ultimo_hash: Option<String>,
    pub telemetria_registros: usize,
    pub archivos: Vec<ArchivoEntregado>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Verificacion {
    pub comando: String,
    pub codigo: Option<i32>,
    /// Últimas líneas de la salida (resumen visible en la entrega).
    pub salida: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Paquete {
    pub ruta: PathBuf,
    pub sha256: String,
    pub manifiesto: Manifiesto,
}

/// Coincidencia de un glob simple (`*` y `?`) contra un nombre de archivo o ruta relativa.
pub fn glob(patron: &str, texto: &str) -> bool {
    fn rec(p: &[char], t: &[char]) -> bool {
        match (p.first(), t.first()) {
            (None, None) => true,
            (Some('*'), _) => rec(&p[1..], t) || (!t.is_empty() && t[0] != '/' && rec(p, &t[1..])),
            (Some('?'), Some(c)) if *c != '/' => rec(&p[1..], &t[1..]),
            (Some(a), Some(b)) if a == b => rec(&p[1..], &t[1..]),
            _ => false,
        }
    }
    let p: Vec<char> = patron.chars().collect();
    let t: Vec<char> = texto.chars().collect();
    rec(&p, &t)
}

/// Archivos que forman la entrega, relativos a la raíz y ordenados.
pub fn archivos_de_entrega(p: &Proyecto) -> Vec<String> {
    let patrones: Vec<String> = if p.config.examen.archivos.is_empty() {
        [
            "*.c",
            "*.h",
            "*.md",
            "Makefile",
            "makefile",
            "src/*.c",
            "src/*.h",
            "test/*.c",
            "tests/*.c",
            "include/*.h",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    } else {
        p.config.examen.archivos.clone()
    };
    let mut rels: Vec<String> = p
        .archivos(5000)
        .iter()
        .map(|a| p.relativa(a))
        .chain(
            ["Makefile".to_string(), "makefile".to_string()]
                .into_iter()
                .filter(|m| p.raiz.join(m).is_file()),
        )
        .filter(|rel| patrones.iter().any(|pat| glob(pat, rel)))
        .collect();
    rels.sort();
    rels.dedup();
    rels
}

/// Arma el paquete en `.jonesy/entrega/`. `telemetria` es la bitácora ya sellada.
pub fn empaquetar(
    p: &Proyecto,
    telemetria: Option<(&Path, Option<String>, usize)>,
    verificacion: Option<Verificacion>,
    ahora_unix: u64,
) -> std::io::Result<Paquete> {
    let dir = p.dir_estado().join("entrega");
    std::fs::create_dir_all(&dir)?;
    let usuario = crate::usuario();
    let mut archivos = vec![];
    for rel in archivos_de_entrega(p) {
        let datos = std::fs::read(p.raiz.join(&rel))?;
        archivos.push(ArchivoEntregado {
            ruta: rel,
            bytes: datos.len() as u64,
            sha256: sha256_hex(&datos),
        });
    }
    let (ultimo_hash, registros) = telemetria
        .as_ref()
        .map(|(_, h, n)| (h.clone(), *n))
        .unwrap_or((None, 0));
    let manifiesto = Manifiesto {
        usuario: usuario.clone(),
        momento_unix: ahora_unix,
        version_jonesy: env!("CARGO_PKG_VERSION").into(),
        verificacion,
        telemetria_ultimo_hash: ultimo_hash,
        telemetria_registros: registros,
        archivos,
    };
    let nombre: String = usuario
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let ruta = dir.join(format!("entrega-{nombre}-{ahora_unix}.tar"));
    let tmp = dir.join(format!(".entrega-{ahora_unix}.tar.tmp"));
    {
        let f = std::fs::File::create(&tmp)?;
        let mut tar = tar::Builder::new(f);
        tar.mode(tar::HeaderMode::Deterministic);
        let agregar = |tar: &mut tar::Builder<std::fs::File>,
                       nombre: &str,
                       datos: &[u8]|
         -> std::io::Result<()> {
            let mut h = tar::Header::new_gnu();
            h.set_size(datos.len() as u64);
            h.set_mode(0o644);
            h.set_mtime(ahora_unix);
            h.set_cksum();
            tar.append_data(&mut h, nombre, datos)
        };
        for a in &manifiesto.archivos {
            agregar(
                &mut tar,
                &format!("entrega/{}", a.ruta),
                &std::fs::read(p.raiz.join(&a.ruta))?,
            )?;
        }
        if let Some((log, _, _)) = &telemetria {
            if let Ok(datos) = std::fs::read(log) {
                agregar(&mut tar, "telemetry.log", &datos)?;
            }
        }
        let json = serde_json::to_vec_pretty(&manifiesto).map_err(std::io::Error::other)?;
        agregar(&mut tar, "entrega.json", &json)?;
        tar.into_inner()?.sync_all()?;
    }
    std::fs::rename(&tmp, &ruta)?;
    let sha256 = sha256_hex(&std::fs::read(&ruta)?);
    std::fs::write(
        ruta.with_extension("tar.sha256"),
        format!(
            "{sha256}  {}\n",
            ruta.file_name().unwrap().to_string_lossy()
        ),
    )?;
    Ok(Paquete {
        ruta,
        sha256,
        manifiesto,
    })
}

/// Recorta una salida larga a sus últimas `n` líneas.
pub fn cola(texto: &str, n: usize) -> String {
    let lineas: Vec<&str> = texto.lines().collect();
    lineas[lineas.len().saturating_sub(n)..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globs() {
        assert!(glob("*.c", "main.c"));
        assert!(!glob("*.c", "src/main.c"));
        assert!(glob("src/*.c", "src/main.c"));
        assert!(glob("Makefile", "Makefile"));
        assert!(glob("t?st.c", "test.c"));
        assert!(!glob("*.c", "main.h"));
    }

    #[test]
    fn empaqueta_con_manifiesto_y_hash() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("main.c"), "int main(void) { return 0; }\n").unwrap();
        std::fs::write(dir.path().join("Makefile"), "all:\n\tgcc main.c\n").unwrap();
        std::fs::write(dir.path().join("notas.txt"), "no va").unwrap();
        std::fs::create_dir(dir.path().join(".jonesy")).unwrap();
        let log = dir.path().join(".jonesy/telemetry.log");
        std::fs::write(&log, "{}\n").unwrap();
        let p = Proyecto::abrir(dir.path());
        assert_eq!(
            archivos_de_entrega(&p),
            vec!["Makefile".to_string(), "main.c".to_string()]
        );
        let v = Verificacion {
            comando: "make test".into(),
            codigo: Some(0),
            salida: "ok".into(),
        };
        let paq = empaquetar(
            &p,
            Some((&log, Some("ab".into()), 1)),
            Some(v),
            1_700_000_000,
        )
        .unwrap();
        assert!(paq.ruta.is_file());
        assert_eq!(paq.sha256, sha256_hex(&std::fs::read(&paq.ruta).unwrap()));
        let mut nombres: Vec<String> = tar::Archive::new(std::fs::File::open(&paq.ruta).unwrap())
            .entries()
            .unwrap()
            .map(|e| e.unwrap().path().unwrap().to_string_lossy().into_owned())
            .collect();
        nombres.sort();
        assert_eq!(
            nombres,
            [
                "entrega.json",
                "entrega/Makefile",
                "entrega/main.c",
                "telemetry.log"
            ]
        );
        assert!(
            std::fs::read_to_string(paq.ruta.with_extension("tar.sha256"))
                .unwrap()
                .starts_with(&paq.sha256)
        );
    }
}
