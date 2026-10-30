//! Cuántos colores tiene la terminal, para adaptar los temas en una terminal pura (la consola
//! de Linux, un `screen` o una terminal de 256 colores) en lugar de mandar color verdadero que
//! se ve mal o no se ve.
//!
//! `[interfaz] colores` lo fija ("24bit", "256" o "16"); vacío o "auto" lo deduce de
//! `COLORTERM`, `TERM`, `TERM_PROGRAM` y `WT_SESSION`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profundidad {
    /// Color verdadero (24 bits).
    Verdadero,
    /// La paleta de 256 colores de xterm.
    Paleta256,
    /// Los 16 colores ANSI (consola de Linux, terminales viejas).
    Ansi16,
}

impl Profundidad {
    pub fn describir(self) -> &'static str {
        match self {
            Profundidad::Verdadero => "color verdadero (24 bits)",
            Profundidad::Paleta256 => "256 colores",
            Profundidad::Ansi16 => "16 colores",
        }
    }

    fn de_config(s: &str) -> Option<Profundidad> {
        match s.trim().to_lowercase().as_str() {
            "24bit" | "24" | "truecolor" | "verdadero" => Some(Profundidad::Verdadero),
            "256" => Some(Profundidad::Paleta256),
            "16" | "8" | "ansi" => Some(Profundidad::Ansi16),
            _ => None,
        }
    }
}

/// La profundidad de color: la de `config` si la fija, si no la que anuncia el entorno (`var`
/// devuelve una variable de entorno).
pub fn detectar(config: &str, var: impl Fn(&str) -> Option<String>) -> Profundidad {
    if let Some(p) = Profundidad::de_config(config) {
        return p;
    }
    let colorterm = var("COLORTERM").unwrap_or_default().to_lowercase();
    if colorterm == "truecolor" || colorterm == "24bit" || var("WT_SESSION").is_some() {
        return Profundidad::Verdadero;
    }
    let term = var("TERM").unwrap_or_default().to_lowercase();
    let programa = var("TERM_PROGRAM").unwrap_or_default();
    if matches!(
        programa.as_str(),
        "iTerm.app" | "vscode" | "WezTerm" | "ghostty"
    ) {
        return Profundidad::Verdadero;
    }
    if term.contains("256color") || programa == "Apple_Terminal" {
        return Profundidad::Paleta256;
    }
    if term.is_empty() {
        // Windows sin TERM: la consola actual entiende color verdadero.
        return if cfg!(windows) {
            Profundidad::Verdadero
        } else {
            Profundidad::Ansi16
        };
    }
    if term.contains("direct") || term.contains("truecolor") {
        return Profundidad::Verdadero;
    }
    // linux, vt100, vt220, ansi, xterm a secas, screen, tmux sin 256color…
    Profundidad::Ansi16
}

/// La del entorno de este proceso.
pub fn detectar_entorno(config: &str) -> Profundidad {
    detectar(config, |v| std::env::var(v).ok().filter(|s| !s.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn con<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |v| {
            vars.iter()
                .find(|(k, _)| *k == v)
                .map(|(_, x)| x.to_string())
        }
    }

    #[test]
    fn deduce_del_entorno() {
        assert_eq!(
            detectar(
                "",
                con(&[("COLORTERM", "truecolor"), ("TERM", "xterm-256color")])
            ),
            Profundidad::Verdadero
        );
        assert_eq!(
            detectar("auto", con(&[("TERM", "xterm-256color")])),
            Profundidad::Paleta256
        );
        assert_eq!(detectar("", con(&[("TERM", "linux")])), Profundidad::Ansi16);
        assert_eq!(
            detectar("", con(&[("TERM", "screen")])),
            Profundidad::Ansi16
        );
        assert_eq!(
            detectar(
                "",
                con(&[("TERM", "xterm"), ("TERM_PROGRAM", "Apple_Terminal")])
            ),
            Profundidad::Paleta256
        );
        assert_eq!(
            detectar("", con(&[("TERM", "xterm"), ("WT_SESSION", "1")])),
            Profundidad::Verdadero
        );
    }

    #[test]
    fn la_configuracion_manda() {
        assert_eq!(
            detectar("16", con(&[("COLORTERM", "truecolor")])),
            Profundidad::Ansi16
        );
        assert_eq!(
            detectar("256", con(&[("TERM", "linux")])),
            Profundidad::Paleta256
        );
        assert_eq!(
            detectar("24bit", con(&[("TERM", "linux")])),
            Profundidad::Verdadero
        );
    }
}
