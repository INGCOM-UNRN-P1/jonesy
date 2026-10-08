//! `jonesy verificar <paquete.tar>`: comprueba un paquete de entrega del examen.
//!
//! Es lo que tiene que invocar quien corrige (dredd o un docente) para confiar en una
//! entrega: el SHA-256 del paquete contra su `.sha256`, el de cada archivo contra
//! `entrega.json`, la cadena de la bitácora y que su último hash y su cantidad de
//! registros sean los que quedaron sellados en el manifiesto. Con `--json` emite el
//! sobre común del ecosistema (`schema_version`, `herramienta`, `comando`, `ok`, …).

use std::io::Read;
use std::path::Path;

use jonesy_core::entrega::Manifiesto;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Chequeo {
    pub nombre: String,
    pub ok: bool,
    pub detalle: String,
}

#[derive(Debug, Serialize)]
pub struct Informe {
    pub schema_version: &'static str,
    pub herramienta: &'static str,
    pub comando: &'static str,
    pub ok: bool,
    pub paquete: String,
    pub sha256: String,
    pub usuario: Option<String>,
    pub pegados: usize,
    pub caracteres_pegados: usize,
    pub chequeos: Vec<Chequeo>,
}

fn chequeo(v: &mut Vec<Chequeo>, nombre: &str, ok: bool, detalle: impl Into<String>) {
    v.push(Chequeo {
        nombre: nombre.into(),
        ok,
        detalle: detalle.into(),
    });
}

pub fn verificar_paquete(ruta: &Path) -> anyhow::Result<Informe> {
    let datos = std::fs::read(ruta)?;
    let sha = jonesy_core::sha256_hex(&datos);
    let mut c = vec![];
    // 1. Hash del paquete.
    let archivo_sha = ruta.with_extension("tar.sha256");
    match std::fs::read_to_string(&archivo_sha) {
        Ok(t) => {
            let esperado = t.split_whitespace().next().unwrap_or("").to_lowercase();
            chequeo(
                &mut c,
                "sha256 del paquete",
                esperado == sha,
                format!("{sha} (esperado {esperado})"),
            );
        }
        Err(_) => chequeo(
            &mut c,
            "sha256 del paquete",
            true,
            format!("{sha} (sin {} para comparar)", archivo_sha.display()),
        ),
    }
    // 2. Contenido.
    let mut manifiesto: Option<Manifiesto> = None;
    let mut fuentes: Vec<(String, String)> = vec![];
    let mut bitacora: Option<Vec<u8>> = None;
    let mut tar = tar::Archive::new(&datos[..]);
    for e in tar.entries()? {
        let mut e = e?;
        let nombre = e.path()?.to_string_lossy().replace('\\', "/");
        let mut contenido = vec![];
        e.read_to_end(&mut contenido)?;
        if nombre == "entrega.json" {
            manifiesto = Some(serde_json::from_slice(&contenido)?);
        } else if nombre == "telemetry.log" {
            bitacora = Some(contenido);
        } else if let Some(rel) = nombre.strip_prefix("entrega/") {
            fuentes.push((rel.to_string(), jonesy_core::sha256_hex(&contenido)));
        }
    }
    let Some(m) = manifiesto else {
        chequeo(
            &mut c,
            "manifiesto",
            false,
            "el paquete no tiene entrega.json",
        );
        return Ok(cerrar(ruta, sha, None, c, 0, 0));
    };
    let mut distintos = vec![];
    for a in &m.archivos {
        match fuentes.iter().find(|(r, _)| *r == a.ruta) {
            None => distintos.push(format!("falta {}", a.ruta)),
            Some((_, h)) if *h != a.sha256 => distintos.push(format!("{} no coincide", a.ruta)),
            _ => {}
        }
    }
    for (r, _) in &fuentes {
        if !m.archivos.iter().any(|a| a.ruta == *r) {
            distintos.push(format!("{r} no está en el manifiesto"));
        }
    }
    chequeo(
        &mut c,
        "archivos contra el manifiesto",
        distintos.is_empty(),
        if distintos.is_empty() {
            format!("{} archivo(s)", m.archivos.len())
        } else {
            distintos.join("; ")
        },
    );
    // 3. Bitácora.
    let (mut pegados, mut caracteres) = (0, 0);
    match bitacora {
        None => chequeo(
            &mut c,
            "bitácora",
            m.telemetria_registros == 0,
            "el paquete no trae telemetry.log",
        ),
        Some(b) => {
            let tmp = tempfile::NamedTempFile::new()?;
            std::fs::write(tmp.path(), &b)?;
            let v = jonesy_telemetry::verificar(tmp.path())?;
            pegados = v.pegados;
            caracteres = v.caracteres_pegados;
            let detalle = match &v.error {
                None => format!("{} registros íntegros", v.registros),
                Some((l, e)) => format!("rota en la línea {l}: {e}"),
            };
            chequeo(&mut c, "cadena de la bitácora", v.integra(), detalle);
            chequeo(
                &mut c,
                "bitácora sellada en el manifiesto",
                m.telemetria_ultimo_hash.as_deref() == Some(v.ultimo_hash.as_str())
                    && m.telemetria_registros == v.registros as usize,
                format!(
                    "último hash {} y {} registros (manifiesto: {} y {})",
                    v.ultimo_hash,
                    v.registros,
                    m.telemetria_ultimo_hash.as_deref().unwrap_or("—"),
                    m.telemetria_registros
                ),
            );
            chequeo(
                &mut c,
                "sesión cerrada",
                v.cerrada,
                if v.cerrada {
                    "termina con SessionEnd"
                } else {
                    "no termina con SessionEnd"
                },
            );
        }
    }
    Ok(cerrar(ruta, sha, Some(m.usuario), c, pegados, caracteres))
}

fn cerrar(
    ruta: &Path,
    sha: String,
    usuario: Option<String>,
    chequeos: Vec<Chequeo>,
    pegados: usize,
    caracteres: usize,
) -> Informe {
    Informe {
        schema_version: "1.0.0",
        herramienta: "jonesy",
        comando: "verificar",
        ok: chequeos.iter().all(|c| c.ok),
        paquete: ruta.display().to_string(),
        sha256: sha,
        usuario,
        pegados,
        caracteres_pegados: caracteres,
        chequeos,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jonesy_telemetry::{Telemetria, TelemetryEvent};

    fn paquete() -> (tempfile::TempDir, std::path::PathBuf) {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("main.c"), "int main(void) { return 0; }\n").unwrap();
        std::fs::create_dir_all(d.path().join(".jonesy")).unwrap();
        let log = d.path().join(".jonesy/telemetry.log");
        let t = Telemetria::abrir(&log).unwrap();
        t.registrar(TelemetryEvent::SessionStart {
            user: "ana".into(),
            exam_mode: true,
        });
        t.registrar(TelemetryEvent::BracketedPaste {
            buffer_id: "main.c".into(),
            length: 5,
            content_hash: "x".into(),
        });
        t.registrar(TelemetryEvent::SessionEnd {
            reason: "entrega".into(),
        });
        t.cerrar();
        let v = jonesy_telemetry::verificar(&log).unwrap();
        let p = jonesy_core::Proyecto::abrir(d.path());
        let paq = jonesy_core::entrega::empaquetar(
            &p,
            Some((&log, Some(v.ultimo_hash), v.registros as usize)),
            None,
            1_700_000_000,
        )
        .unwrap();
        (d, paq.ruta)
    }

    #[test]
    fn paquete_integro() {
        let (_d, ruta) = paquete();
        let i = verificar_paquete(&ruta).unwrap();
        assert!(i.ok, "{:#?}", i.chequeos);
        assert_eq!(i.pegados, 1);
        assert_eq!(i.chequeos.len(), 5);
    }

    #[test]
    fn paquete_alterado() {
        let (_d, ruta) = paquete();
        // Otro contenido con el mismo nombre: no coincide con el .sha256.
        let mut datos = std::fs::read(&ruta).unwrap();
        let n = datos.len();
        datos[n / 3] ^= 0x01;
        std::fs::write(&ruta, &datos).unwrap();
        let i = verificar_paquete(&ruta);
        // Según qué byte cambió, falla el tar o alguno de los chequeos; nunca da ok.
        assert!(i.map(|i| !i.ok).unwrap_or(true));
    }
}
