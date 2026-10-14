//! Colores y símbolos. Con `--ascii`, `interfaz.ascii = true` o un locale que no es UTF-8,
//! los caracteres de caja y los íconos se degradan a ASCII.

use std::cell::Cell;

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

/// Tema de colores (`[interfaz] tema` o Archivo → Configuración del entorno → Apariencia).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tema {
    /// El de siempre, para terminales de fondo oscuro con color verdadero.
    Oscuro,
    /// Para terminales de fondo claro.
    Claro,
    /// Sólo los 16 colores ANSI: terminales sin color verdadero o más contraste.
    Contraste,
    /// Como Turbo C++ de Borland: editor azul con texto amarillo, barra gris con letras
    /// rojas y selección verde.
    Turbo,
    /// Solarized oscuro: fondo azul petróleo y colores de bajo contraste.
    Solarizado,
    /// Monitor de fósforo verde: todo en verdes sobre negro.
    Fosforo,
    /// Apto para daltonismo (paleta de Okabe-Ito): errores en naranja y aciertos en
    /// azul, nunca rojo contra verde; el diff, azul y naranja.
    Daltonico,
}

impl Tema {
    pub const TODOS: [Tema; 7] = [
        Tema::Oscuro,
        Tema::Claro,
        Tema::Contraste,
        Tema::Turbo,
        Tema::Solarizado,
        Tema::Fosforo,
        Tema::Daltonico,
    ];

    pub fn nombre(self) -> &'static str {
        match self {
            Tema::Oscuro => "oscuro",
            Tema::Claro => "claro",
            Tema::Contraste => "contraste",
            Tema::Turbo => "turbo",
            Tema::Solarizado => "solarizado",
            Tema::Fosforo => "fosforo",
            Tema::Daltonico => "daltonico",
        }
    }

    pub fn de_nombre(s: &str) -> Option<Tema> {
        let s = s.trim().to_lowercase().replace('ó', "o");
        Tema::TODOS.into_iter().find(|t| t.nombre() == s)
    }

    pub fn siguiente(self) -> Tema {
        let i = Tema::TODOS.iter().position(|t| *t == self).unwrap_or(0);
        Tema::TODOS[(i + 1) % Tema::TODOS.len()]
    }

    fn paleta(self) -> &'static Paleta {
        match self {
            Tema::Oscuro => &OSCURO,
            Tema::Claro => &CLARO,
            Tema::Contraste => &CONTRASTE,
            Tema::Turbo => &TURBO,
            Tema::Solarizado => &SOLARIZADO,
            Tema::Fosforo => &FOSFORO,
            Tema::Daltonico => &DALTONICO,
        }
    }
}

/// Colores de un tema.
#[derive(Debug)]
pub struct Paleta {
    /// Fondo de toda la pantalla (Reset: el de la terminal).
    pub fondo: Color,
    /// Texto común (Reset: el de la terminal).
    pub texto: Color,
    /// Texto sobre las barras (menú, estado, pestañas).
    pub texto_barra: Color,
    /// Texto secundario sobre las barras.
    pub atenuado_barra: Color,
    /// Letra mnemónica de cada menú (Reset: sólo subrayada).
    pub tecla_menu: Color,
    /// Fondo del menú abierto y de la entrada elegida.
    pub fondo_menu_sel: Color,
    pub fondo_barra: Color,
    pub fondo_seleccion: Color,
    pub fondo_linea: Color,
    pub fondo_codigo: Color,
    pub acento: Color,
    pub atenuado: Color,
    pub error: Color,
    pub advertencia: Color,
    pub nota: Color,
    pub estilo: Color,
    pub ok: Color,
    pub peligro: Color,
    /// Texto resaltado sobre el fondo de la terminal o de la selección.
    pub fuerte: Color,
    /// Texto sobre el color de acento.
    pub sobre_acento: Color,
    pub numero: Color,
    pub linea_depurador: Color,
    pub borrado: Color,
    pub borrado_fuerte: Color,
    pub agregado: Color,
    pub agregado_fuerte: Color,
    pub relleno: Color,
    pub pegado: Color,
}

const OSCURO: Paleta = Paleta {
    fondo: Color::Reset,
    texto: Color::Reset,
    texto_barra: Color::Reset,
    atenuado_barra: Color::Rgb(110, 118, 129),
    tecla_menu: Color::Reset,
    fondo_menu_sel: Color::Rgb(97, 175, 239),
    fondo_barra: Color::Rgb(40, 44, 52),
    fondo_seleccion: Color::Rgb(62, 68, 81),
    fondo_linea: Color::Rgb(33, 37, 43),
    fondo_codigo: Color::Rgb(36, 40, 47),
    acento: Color::Rgb(97, 175, 239),
    atenuado: Color::Rgb(110, 118, 129),
    error: Color::Rgb(224, 108, 117),
    advertencia: Color::Rgb(229, 192, 123),
    nota: Color::Rgb(86, 182, 194),
    estilo: Color::Rgb(198, 120, 221),
    ok: Color::Rgb(152, 195, 121),
    peligro: Color::Rgb(255, 85, 85),
    fuerte: Color::White,
    sobre_acento: Color::Black,
    numero: Color::Rgb(209, 154, 102),
    linea_depurador: Color::Rgb(70, 60, 20),
    borrado: Color::Rgb(62, 28, 30),
    borrado_fuerte: Color::Rgb(125, 40, 45),
    agregado: Color::Rgb(26, 56, 34),
    agregado_fuerte: Color::Rgb(40, 110, 55),
    relleno: Color::Rgb(55, 58, 64),
    pegado: Color::Rgb(110, 50, 20),
};

/// Okabe-Ito: azul (0,114,178), celeste (86,180,233), naranja (230,159,0), bermellón
/// (213,94,0), amarillo (240,228,66), púrpura rojizo (204,121,167).
const DALTONICO: Paleta = Paleta {
    fondo: Color::Reset,
    texto: Color::Reset,
    texto_barra: Color::Reset,
    atenuado_barra: Color::Rgb(120, 125, 135),
    tecla_menu: Color::Reset,
    fondo_menu_sel: Color::Rgb(86, 180, 233),
    fondo_barra: Color::Rgb(40, 44, 52),
    fondo_seleccion: Color::Rgb(60, 66, 80),
    fondo_linea: Color::Rgb(33, 37, 43),
    fondo_codigo: Color::Rgb(36, 40, 47),
    acento: Color::Rgb(86, 180, 233),
    atenuado: Color::Rgb(120, 125, 135),
    error: Color::Rgb(213, 94, 0),
    advertencia: Color::Rgb(240, 228, 66),
    nota: Color::Rgb(86, 180, 233),
    estilo: Color::Rgb(204, 121, 167),
    ok: Color::Rgb(0, 114, 178),
    peligro: Color::Rgb(230, 159, 0),
    fuerte: Color::White,
    sobre_acento: Color::Black,
    numero: Color::Rgb(230, 159, 0),
    linea_depurador: Color::Rgb(70, 62, 10),
    borrado: Color::Rgb(70, 38, 0),
    borrado_fuerte: Color::Rgb(140, 70, 0),
    agregado: Color::Rgb(0, 40, 75),
    agregado_fuerte: Color::Rgb(0, 80, 140),
    relleno: Color::Rgb(55, 58, 64),
    pegado: Color::Rgb(90, 60, 120),
};

const CLARO: Paleta = Paleta {
    fondo: Color::Reset,
    texto: Color::Reset,
    texto_barra: Color::Reset,
    atenuado_barra: Color::Rgb(110, 119, 129),
    tecla_menu: Color::Reset,
    fondo_menu_sel: Color::Rgb(9, 105, 218),
    fondo_barra: Color::Rgb(232, 234, 239),
    fondo_seleccion: Color::Rgb(200, 216, 242),
    fondo_linea: Color::Rgb(243, 244, 247),
    fondo_codigo: Color::Rgb(240, 241, 244),
    acento: Color::Rgb(9, 105, 218),
    atenuado: Color::Rgb(110, 119, 129),
    error: Color::Rgb(207, 34, 46),
    advertencia: Color::Rgb(154, 103, 0),
    nota: Color::Rgb(27, 124, 131),
    estilo: Color::Rgb(130, 80, 223),
    ok: Color::Rgb(26, 127, 55),
    peligro: Color::Rgb(200, 0, 0),
    fuerte: Color::Black,
    sobre_acento: Color::White,
    numero: Color::Rgb(149, 56, 0),
    linea_depurador: Color::Rgb(255, 240, 180),
    borrado: Color::Rgb(255, 235, 233),
    borrado_fuerte: Color::Rgb(255, 190, 190),
    agregado: Color::Rgb(230, 255, 236),
    agregado_fuerte: Color::Rgb(170, 242, 188),
    relleno: Color::Rgb(208, 210, 215),
    pegado: Color::Rgb(255, 205, 160),
};

const CONTRASTE: Paleta = Paleta {
    fondo: Color::Reset,
    texto: Color::Reset,
    texto_barra: Color::White,
    atenuado_barra: Color::Gray,
    tecla_menu: Color::LightYellow,
    fondo_menu_sel: Color::LightCyan,
    fondo_barra: Color::DarkGray,
    fondo_seleccion: Color::Blue,
    fondo_linea: Color::Reset,
    fondo_codigo: Color::Reset,
    acento: Color::LightCyan,
    atenuado: Color::Gray,
    error: Color::LightRed,
    advertencia: Color::LightYellow,
    nota: Color::Cyan,
    estilo: Color::LightMagenta,
    ok: Color::LightGreen,
    peligro: Color::Red,
    fuerte: Color::White,
    sobre_acento: Color::Black,
    numero: Color::Yellow,
    linea_depurador: Color::Magenta,
    borrado: Color::Red,
    borrado_fuerte: Color::LightRed,
    agregado: Color::Green,
    agregado_fuerte: Color::LightGreen,
    relleno: Color::DarkGray,
    pegado: Color::Red,
};

/// Turbo C++ 3.0: los 16 colores de la paleta CGA.
const TURBO: Paleta = Paleta {
    fondo: Color::Rgb(0, 0, 170),
    texto: Color::Rgb(255, 255, 85),
    texto_barra: Color::Rgb(0, 0, 0),
    atenuado_barra: Color::Rgb(85, 85, 85),
    tecla_menu: Color::Rgb(170, 0, 0),
    fondo_menu_sel: Color::Rgb(0, 170, 0),
    fondo_barra: Color::Rgb(170, 170, 170),
    fondo_seleccion: Color::Rgb(0, 170, 170),
    fondo_linea: Color::Rgb(0, 0, 170),
    fondo_codigo: Color::Rgb(0, 0, 128),
    acento: Color::Rgb(85, 255, 255),
    atenuado: Color::Rgb(170, 170, 170),
    error: Color::Rgb(255, 85, 85),
    advertencia: Color::Rgb(255, 255, 85),
    nota: Color::Rgb(85, 255, 85),
    estilo: Color::Rgb(255, 255, 255),
    ok: Color::Rgb(85, 255, 255),
    peligro: Color::Rgb(255, 85, 85),
    fuerte: Color::Rgb(255, 255, 255),
    sobre_acento: Color::Rgb(0, 0, 0),
    numero: Color::Rgb(85, 255, 255),
    linea_depurador: Color::Rgb(0, 170, 170),
    borrado: Color::Rgb(128, 0, 0),
    borrado_fuerte: Color::Rgb(170, 0, 0),
    agregado: Color::Rgb(0, 100, 0),
    agregado_fuerte: Color::Rgb(0, 170, 0),
    relleno: Color::Rgb(0, 0, 128),
    pegado: Color::Rgb(170, 0, 170),
};

/// Solarized (Ethan Schoonover), variante oscura.
const SOLARIZADO: Paleta = Paleta {
    fondo: Color::Rgb(0, 43, 54),
    texto: Color::Rgb(131, 148, 150),
    texto_barra: Color::Rgb(147, 161, 161),
    atenuado_barra: Color::Rgb(88, 110, 117),
    tecla_menu: Color::Rgb(181, 137, 0),
    fondo_menu_sel: Color::Rgb(38, 139, 210),
    fondo_barra: Color::Rgb(7, 54, 66),
    fondo_seleccion: Color::Rgb(7, 54, 66),
    fondo_linea: Color::Rgb(7, 54, 66),
    fondo_codigo: Color::Rgb(7, 54, 66),
    acento: Color::Rgb(38, 139, 210),
    atenuado: Color::Rgb(88, 110, 117),
    error: Color::Rgb(220, 50, 47),
    advertencia: Color::Rgb(181, 137, 0),
    nota: Color::Rgb(42, 161, 152),
    estilo: Color::Rgb(108, 113, 196),
    ok: Color::Rgb(133, 153, 0),
    peligro: Color::Rgb(220, 50, 47),
    fuerte: Color::Rgb(238, 232, 213),
    sobre_acento: Color::Rgb(0, 43, 54),
    numero: Color::Rgb(203, 75, 22),
    linea_depurador: Color::Rgb(88, 70, 0),
    borrado: Color::Rgb(70, 30, 30),
    borrado_fuerte: Color::Rgb(130, 40, 40),
    agregado: Color::Rgb(30, 60, 30),
    agregado_fuerte: Color::Rgb(60, 110, 40),
    relleno: Color::Rgb(7, 54, 66),
    pegado: Color::Rgb(110, 50, 20),
};

/// Monitor monocromo de fósforo verde.
const FOSFORO: Paleta = Paleta {
    fondo: Color::Rgb(0, 0, 0),
    texto: Color::Rgb(51, 255, 51),
    texto_barra: Color::Rgb(0, 0, 0),
    atenuado_barra: Color::Rgb(0, 80, 0),
    tecla_menu: Color::Rgb(0, 0, 0),
    fondo_menu_sel: Color::Rgb(170, 255, 170),
    fondo_barra: Color::Rgb(0, 190, 0),
    fondo_seleccion: Color::Rgb(0, 90, 0),
    fondo_linea: Color::Rgb(0, 25, 0),
    fondo_codigo: Color::Rgb(0, 20, 0),
    acento: Color::Rgb(150, 255, 150),
    atenuado: Color::Rgb(0, 140, 0),
    error: Color::Rgb(200, 255, 200),
    advertencia: Color::Rgb(120, 255, 120),
    nota: Color::Rgb(80, 220, 80),
    estilo: Color::Rgb(170, 255, 170),
    ok: Color::Rgb(100, 230, 100),
    peligro: Color::Rgb(220, 255, 220),
    fuerte: Color::Rgb(200, 255, 200),
    sobre_acento: Color::Rgb(0, 0, 0),
    numero: Color::Rgb(120, 255, 120),
    linea_depurador: Color::Rgb(0, 70, 0),
    borrado: Color::Rgb(0, 40, 0),
    borrado_fuerte: Color::Rgb(0, 70, 0),
    agregado: Color::Rgb(0, 60, 0),
    agregado_fuerte: Color::Rgb(0, 110, 0),
    relleno: Color::Rgb(0, 30, 0),
    pegado: Color::Rgb(0, 120, 0),
};

thread_local! {
    // Por hilo: se dibuja siempre desde el hilo principal, y así las pruebas (que corren
    // en paralelo) no se pisan el tema.
    static TEMA: Cell<Tema> = const { Cell::new(Tema::Oscuro) };
}

/// Cambia el tema (los colores se leen al dibujar).
pub fn usar(t: Tema) {
    TEMA.with(|c| c.set(t));
}

pub fn actual() -> Tema {
    TEMA.with(Cell::get)
}

pub fn paleta() -> &'static Paleta {
    actual().paleta()
}

macro_rules! colores {
    ($($nombre:ident),* $(,)?) => {
        $(
            #[inline]
            pub fn $nombre() -> Color {
                paleta().$nombre
            }
        )*
    };
}

colores!(
    fondo,
    texto,
    texto_barra,
    atenuado_barra,
    tecla_menu,
    fondo_menu_sel,
    fondo_barra,
    fondo_seleccion,
    fondo_linea,
    fondo_codigo,
    acento,
    atenuado,
    error,
    advertencia,
    nota,
    estilo,
    ok,
    peligro,
    fuerte,
    sobre_acento,
    numero,
    linea_depurador,
    borrado,
    borrado_fuerte,
    agregado,
    agregado_fuerte,
    relleno,
    pegado,
);

pub fn estilo_clase(c: Clase) -> Style {
    let s = Style::default();
    match c {
        Clase::Normal => s,
        Clase::Palabra => s.fg(estilo()),
        Clase::Tipo => s.fg(advertencia()),
        Clase::Cadena => s.fg(ok()),
        Clase::Numero => s.fg(numero()),
        Clase::Comentario => s.fg(atenuado()).add_modifier(Modifier::ITALIC),
        Clase::Preproc => s.fg(error()),
        Clase::Funcion => s.fg(acento()),
        Clase::Encabezado => s.fg(acento()).add_modifier(Modifier::BOLD),
        Clase::Enfasis => s.add_modifier(Modifier::BOLD),
        Clase::Codigo => s.fg(ok()),
        Clase::Clave => s.fg(error()),
        Clase::Enlace => s.fg(nota()).add_modifier(Modifier::UNDERLINED),
        Clase::Cita => s.fg(atenuado()).add_modifier(Modifier::ITALIC),
        Clase::Invalido => s.bg(error()).fg(sobre_acento()),
    }
}

pub fn color_severidad(s: jonesy_core::Severidad) -> Color {
    match s {
        jonesy_core::Severidad::Error => error(),
        jonesy_core::Severidad::Advertencia => advertencia(),
        jonesy_core::Severidad::Nota => nota(),
        jonesy_core::Severidad::Estilo => estilo(),
    }
}

pub fn tono_catscan(t: jonesy_core::catscan::Tono) -> Style {
    use jonesy_core::catscan::Tono;
    let s = Style::default();
    match t {
        Tono::Normal => s,
        Tono::Borde => s.fg(atenuado()),
        Tono::Titulo => s.fg(acento()).add_modifier(Modifier::BOLD),
        Tono::Nombre => s.add_modifier(Modifier::BOLD),
        Tono::Tipo => s.fg(advertencia()),
        Tono::Valor => s.fg(numero()),
        Tono::Direccion => s.fg(nota()),
        Tono::Flecha => s.fg(acento()),
        Tono::Vivo => s.fg(ok()).add_modifier(Modifier::BOLD),
        Tono::Liberado => s.fg(atenuado()),
        Tono::Peligro => s.fg(peligro()).add_modifier(Modifier::BOLD),
    }
}
