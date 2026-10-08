//! Colores y símbolos. Con `--ascii`, `interfaz.ascii = true` o un locale que no es UTF-8,
//! los caracteres de caja y los íconos se degradan a ASCII.

use ratatui::style::{Color, Modifier, Style};

use crate::resaltado::Clase;

#[derive(Debug, Clone)]
pub struct Simbolos {
    pub ascii: bool,
    pub h: char,
    pub v: char,
    pub esq: [char; 4],  // ┌ ┐ └ ┘
    pub cruz: [char; 5], // ┬ ┴ ├ ┤ ┼
    pub h_grueso: char,
    pub breakpoint: &'static str,
    pub flecha: &'static str,
    pub linea_actual: &'static str,
    pub cita: &'static str,
    pub vineta: &'static str,
    pub tarea: [&'static str; 2],
    pub salto: &'static str,
    pub rama: &'static str,
    pub reloj: &'static str,
}

impl Simbolos {
    pub fn unicode() -> Simbolos {
        Simbolos {
            ascii: false,
            h: '─',
            v: '│',
            esq: ['┌', '┐', '└', '┘'],
            cruz: ['┬', '┴', '├', '┤', '┼'],
            h_grueso: '━',
            breakpoint: "●",
            flecha: "▶",
            linea_actual: "➜",
            cita: "│ ",
            vineta: "• ",
            tarea: ["☐ ", "☑ "],
            salto: "↵",
            rama: "⎇",
            reloj: "⏱",
        }
    }

    pub fn ascii() -> Simbolos {
        Simbolos {
            ascii: true,
            h: '-',
            v: '|',
            esq: ['+', '+', '+', '+'],
            cruz: ['+', '+', '+', '+', '+'],
            h_grueso: '=',
            breakpoint: "*",
            flecha: ">",
            linea_actual: ">",
            cita: "| ",
            vineta: "* ",
            tarea: ["[ ] ", "[x] "],
            salto: "~",
            rama: "git:",
            reloj: "T",
        }
    }

    /// Reemplaza caracteres de caja de un texto ya armado (cat-scan, tablas).
    pub fn degradar(&self, c: char) -> char {
        if !self.ascii {
            return c;
        }
        match c {
            '─' | '━' => '-',
            '│' => '|',
            '┌' | '┐' | '└' | '┘' | '┬' | '┴' | '├' | '┤' | '┼' => '+',
            '►' | '▶' => '>',
            '◄' => '<',
            '▲' => '^',
            '▼' => 'v',
            '⚠' => '!',
            c if c.is_ascii() => c,
            _ => '?',
        }
    }
}

/// Detecta si la terminal habla UTF-8 por las variables de locale.
pub fn locale_utf8() -> bool {
    for v in ["LC_ALL", "LC_CTYPE", "LANG"] {
        if let Ok(s) = std::env::var(v) {
            if !s.is_empty() {
                let s = s.to_ascii_lowercase();
                return s.contains("utf-8") || s.contains("utf8");
            }
        }
    }
    // Sin locale declarado: las terminales modernas son UTF-8.
    true
}

pub const FONDO_BARRA: Color = Color::Rgb(40, 44, 52);
pub const FONDO_SELECCION: Color = Color::Rgb(62, 68, 81);
pub const FONDO_LINEA: Color = Color::Rgb(33, 37, 43);
pub const FONDO_CODIGO: Color = Color::Rgb(36, 40, 47);
pub const ACENTO: Color = Color::Rgb(97, 175, 239);
pub const ATENUADO: Color = Color::Rgb(110, 118, 129);
pub const ERROR: Color = Color::Rgb(224, 108, 117);
pub const ADVERTENCIA: Color = Color::Rgb(229, 192, 123);
pub const NOTA: Color = Color::Rgb(86, 182, 194);
pub const ESTILO: Color = Color::Rgb(198, 120, 221);
pub const OK: Color = Color::Rgb(152, 195, 121);
pub const PELIGRO: Color = Color::Rgb(255, 85, 85);

pub fn estilo_clase(c: Clase) -> Style {
    let s = Style::default();
    match c {
        Clase::Normal => s,
        Clase::Palabra => s.fg(Color::Rgb(198, 120, 221)),
        Clase::Tipo => s.fg(Color::Rgb(229, 192, 123)),
        Clase::Cadena => s.fg(Color::Rgb(152, 195, 121)),
        Clase::Numero => s.fg(Color::Rgb(209, 154, 102)),
        Clase::Comentario => s.fg(ATENUADO).add_modifier(Modifier::ITALIC),
        Clase::Preproc => s.fg(Color::Rgb(224, 108, 117)),
        Clase::Funcion => s.fg(ACENTO),
        Clase::Encabezado => s.fg(ACENTO).add_modifier(Modifier::BOLD),
        Clase::Enfasis => s.add_modifier(Modifier::BOLD),
        Clase::Codigo => s.fg(Color::Rgb(152, 195, 121)),
        Clase::Clave => s.fg(Color::Rgb(224, 108, 117)),
        Clase::Enlace => s.fg(NOTA).add_modifier(Modifier::UNDERLINED),
        Clase::Cita => s.fg(ATENUADO).add_modifier(Modifier::ITALIC),
    }
}

pub fn color_severidad(s: jonesy_core::Severidad) -> Color {
    match s {
        jonesy_core::Severidad::Error => ERROR,
        jonesy_core::Severidad::Advertencia => ADVERTENCIA,
        jonesy_core::Severidad::Nota => NOTA,
        jonesy_core::Severidad::Estilo => ESTILO,
    }
}

pub fn tono_catscan(t: jonesy_core::catscan::Tono) -> Style {
    use jonesy_core::catscan::Tono;
    let s = Style::default();
    match t {
        Tono::Normal => s,
        Tono::Borde => s.fg(ATENUADO),
        Tono::Titulo => s.fg(ACENTO).add_modifier(Modifier::BOLD),
        Tono::Nombre => s.add_modifier(Modifier::BOLD),
        Tono::Tipo => s.fg(Color::Rgb(229, 192, 123)),
        Tono::Valor => s.fg(Color::Rgb(209, 154, 102)),
        Tono::Direccion => s.fg(NOTA),
        Tono::Flecha => s.fg(ACENTO),
        Tono::Vivo => s.fg(OK).add_modifier(Modifier::BOLD),
        Tono::Liberado => s.fg(ATENUADO),
        Tono::Peligro => s.fg(PELIGRO).add_modifier(Modifier::BOLD),
    }
}
