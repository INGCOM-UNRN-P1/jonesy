//! Informe para reportar un problema de jonesy (`jonesy reporte` y Ayuda → Reportar un
//! problema): versión, sistema, terminal, un doctor rápido, la configuración del proyecto,
//! el último error y el último pánico. No incluye el código del alumno, y el directorio
//! personal aparece como `~`.

use std::path::{Path, PathBuf};

/// Directorio de estado del usuario (`$JONESY_ESTADO`, `$XDG_STATE_HOME/jonesy`…).
pub fn dir_estado_usuario() -> Option<PathBuf> {
    crate::sesion::recientes::archivo().and_then(|a| a.parent().map(Path::to_path_buf))
}

pub fn ruta_panico() -> Option<PathBuf> {
    dir_estado_usuario().map(|d| d.join("ultimo-panico.txt"))
}

/// Guarda un pánico para el próximo reporte (lo llama el gancho de pánico).
pub fn registrar_panico(version: &str, mensaje: &str) {
    let Some(ruta) = ruta_panico() else { return };
    if let Some(d) = ruta.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let traza = std::backtrace::Backtrace::force_capture();
    let _ = std::fs::write(
        ruta,
        format!(
            "jonesy {version} · {} (unix)\n{mensaje}\n\n{traza}\n",
            crate::ahora_unix()
        ),
    );
}

/// Datos que sólo conoce quien lo pide (el IDE sabe el tamaño y el último error).
#[derive(Debug, Default)]
pub struct Contexto {
    pub tamano: Option<(u16, u16)>,
    pub ultimo_error: Option<String>,
    pub examen: bool,
}

fn sistema() -> String {
    let mut s = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);
    if let Ok(t) = std::fs::read_to_string("/etc/os-release") {
        if let Some(n) = t
            .lines()
            .find_map(|l| l.strip_prefix("PRETTY_NAME="))
            .map(|v| v.trim_matches('"'))
        {
            s.push_str(&format!(" · {n}"));
        }
    }
    #[cfg(unix)]
    if let Ok(o) = std::process::Command::new("uname").arg("-r").output() {
        let r = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !r.is_empty() {
            s.push_str(&format!(" · kernel {r}"));
        }
    }
    s
}

/// El informe en Markdown.
pub fn generar(
    version: &str,
    proyecto: &crate::Proyecto,
    integraciones: &[crate::integraciones::Integracion],
    errores_integraciones: &[String],
    ctx: &Contexto,
) -> String {
    let var = |k: &str| std::env::var(k).unwrap_or_else(|_| "—".into());
    let mut s = String::from(
        "# Reporte de jonesy\n\n> Revisalo antes de mandarlo: no incluye tu código, pero sí la configuración del proyecto.\n\n",
    );
    s.push_str(&format!("- **Versión:** jonesy {version}\n"));
    s.push_str(&format!("- **Sistema:** {}\n", sistema()));
    s.push_str(&format!(
        "- **Terminal:** TERM={} · COLORTERM={} · TERM_PROGRAM={}{}\n",
        var("TERM"),
        var("COLORTERM"),
        var("TERM_PROGRAM"),
        ctx.tamano
            .map(|(w, h)| format!(" · {w}×{h}"))
            .unwrap_or_default()
    ));
    s.push_str(&format!(
        "- **Idioma:** LANG={} · LC_ALL={}\n",
        var("LANG"),
        var("LC_ALL")
    ));
    s.push_str(&format!(
        "- **Modo examen:** {}\n",
        if ctx.examen { "sí" } else { "no" }
    ));

    s.push_str("\n## Entorno (doctor rápido)\n\n");
    let chequeos = crate::doctor::chequeos(proyecto, integraciones, errores_integraciones, false);
    for c in &chequeos {
        // La identidad de git es un dato personal: sólo si está.
        let detalle = if c.nombre == "identidad" && c.ok {
            "configurada"
        } else {
            c.detalle.as_str()
        };
        s.push_str(&format!(
            "- {} {} ({}): {detalle}\n",
            if c.ok { "✓" } else { "✗" },
            c.nombre,
            c.nivel,
        ));
    }

    s.push_str("\n## Configuración del proyecto\n\n");
    match std::fs::read_to_string(proyecto.raiz.join(crate::proyecto::ARCHIVO_CONFIG)) {
        Ok(t) => {
            // El hash de la pausa del examen no sale del proyecto.
            let t: Vec<String> = t
                .lines()
                .map(|l| {
                    if l.trim_start().starts_with("pausa_sha256") {
                        "pausa_sha256 = \"(oculto)\"".to_string()
                    } else {
                        l.to_string()
                    }
                })
                .collect();
            s.push_str(&format!("```toml\n{}\n```\n", t.join("\n").trim_end()))
        }
        Err(_) => s.push_str("(sin .jonesy.toml)\n"),
    }

    s.push_str("\n## Último error\n\n");
    s.push_str(
        ctx.ultimo_error
            .as_deref()
            .unwrap_or("(ninguno en esta sesión)"),
    );
    s.push('\n');

    s.push_str("\n## Último pánico\n\n");
    match ruta_panico().and_then(|r| std::fs::read_to_string(r).ok()) {
        Some(p) => {
            // La traza completa es larga: alcanza con el principio.
            let corto: Vec<&str> = p.lines().take(40).collect();
            s.push_str(&format!("```\n{}\n```\n", corto.join("\n")));
        }
        None => s.push_str("(ninguno registrado)\n"),
    }
    s.push_str(
        "\n## Qué pasó\n\nContá qué hacías, qué esperabas y qué pasó (y si se repite, cómo).\n",
    );
    anonimizar(&s)
}

/// El directorio personal como `~` (y el usuario, fuera).
fn anonimizar(s: &str) -> String {
    match std::env::var("HOME") {
        Ok(h) if h.len() > 1 => s.replace(&h, "~"),
        _ => s.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn informe_con_todo_y_sin_el_home() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            d.path().join(".jonesy.toml"),
            "[ripley]\nactivo = false\n[examen]\npausa_sha256 = \"abc123\"\n",
        )
        .unwrap();
        let p = crate::Proyecto::abrir(d.path());
        let r = generar(
            "9.9.9",
            &p,
            &[],
            &[],
            &Contexto {
                tamano: Some((120, 40)),
                ultimo_error: Some("La compilación falló".into()),
                examen: false,
            },
        );
        assert!(r.contains("jonesy 9.9.9") && r.contains("120×40"), "{r}");
        assert!(r.contains("[ripley]") && r.contains("La compilación falló"));
        assert!(r.contains("gcc"), "{r}");
        assert!(!r.contains("abc123") && r.contains("(oculto)"));
        if let Ok(h) = std::env::var("HOME") {
            assert!(!r.contains(&h) || h.len() <= 1);
        }
    }
}
