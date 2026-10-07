//! Resaltado de sintaxis por gramáticas estáticas (RF-1.4): C11/C17, Markdown, YAML, TOML
//! y Bash (para bloques de código en Markdown). Trabaja línea por línea con un estado que
//! arrastra comentarios de bloque y bloques cercados; [`Cache`] guarda el estado al inicio
//! de cada línea para no reescanear el archivo en cada cuadro.

use jonesy_core::Lenguaje;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clase {
    Normal,
    Palabra,
    Tipo,
    Cadena,
    Numero,
    Comentario,
    Preproc,
    Funcion,
    Encabezado,
    Enfasis,
    Codigo,
    Clave,
    Enlace,
    Cita,
}

/// Estado al inicio de una línea.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Estado {
    pub comentario_bloque: bool,
    pub cercado: bool,
}

/// Tramos (inicio, fin) en índices de carácter.
pub type Tramos = Vec<(usize, usize, Clase)>;

const PALABRAS_C: &[&str] = &[
    "auto", "break", "case", "const", "continue", "default", "do", "else", "enum", "extern", "for", "goto", "if",
    "inline", "register", "restrict", "return", "sizeof", "static", "struct", "switch", "typedef", "union",
    "volatile", "while", "_Alignas", "_Alignof", "_Atomic", "_Generic", "_Noreturn", "_Static_assert",
    "_Thread_local", "NULL", "true", "false",
];

const TIPOS_C: &[&str] = &[
    "void", "char", "short", "int", "long", "float", "double", "signed", "unsigned", "_Bool", "bool", "size_t",
    "ssize_t", "FILE", "int8_t", "int16_t", "int32_t", "int64_t", "uint8_t", "uint16_t", "uint32_t", "uint64_t",
    "uintptr_t", "intptr_t", "ptrdiff_t", "pid_t", "off_t", "time_t", "clock_t",
];

const PALABRAS_BASH: &[&str] = &[
    "if", "then", "else", "elif", "fi", "for", "while", "until", "do", "done", "case", "esac", "in", "function",
    "return", "local", "export", "set", "echo", "cd", "exit",
];

fn es_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn literal(cs: &[char], i: usize, comillas: char) -> usize {
    let mut j = i + 1;
    while j < cs.len() {
        if cs[j] == '\\' {
            j += 2;
            continue;
        }
        if cs[j] == comillas {
            return j + 1;
        }
        j += 1;
    }
    cs.len()
}

fn numero(cs: &[char], i: usize) -> usize {
    let mut j = i;
    while j < cs.len() && (cs[j].is_ascii_alphanumeric() || cs[j] == '.' || cs[j] == '_') {
        j += 1;
    }
    j
}

fn resaltar_c(cs: &[char], estado: &mut Estado) -> Tramos {
    let mut t = vec![];
    let mut i = 0;
    if estado.comentario_bloque {
        match buscar(cs, 0, "*/") {
            Some(f) => {
                t.push((0, f + 2, Clase::Comentario));
                i = f + 2;
                estado.comentario_bloque = false;
            }
            None => return vec![(0, cs.len(), Clase::Comentario)],
        }
    }
    let primero = cs.iter().position(|c| !c.is_whitespace());
    if primero.is_some_and(|p| p >= i && cs[p] == '#') {
        // Directiva: todo en Preproc salvo cadenas <...> y "..." y comentarios.
        let p = primero.unwrap();
        let mut j = p + 1;
        while j < cs.len() && (cs[j] == ' ' || es_ident(cs[j])) {
            j += 1;
        }
        t.push((p, j, Clase::Preproc));
        i = j;
        if i < cs.len() && cs[i] == '<' {
            let f = cs[i..].iter().position(|&c| c == '>').map(|x| i + x + 1).unwrap_or(cs.len());
            t.push((i, f, Clase::Cadena));
            i = f;
        }
    }
    while i < cs.len() {
        let c = cs[i];
        if c == '/' && cs.get(i + 1) == Some(&'/') {
            t.push((i, cs.len(), Clase::Comentario));
            break;
        }
        if c == '/' && cs.get(i + 1) == Some(&'*') {
            match buscar(cs, i + 2, "*/") {
                Some(f) => {
                    t.push((i, f + 2, Clase::Comentario));
                    i = f + 2;
                }
                None => {
                    t.push((i, cs.len(), Clase::Comentario));
                    estado.comentario_bloque = true;
                    break;
                }
            }
            continue;
        }
        if c == '"' || c == '\'' {
            let f = literal(cs, i, c);
            t.push((i, f, Clase::Cadena));
            i = f;
            continue;
        }
        if c.is_ascii_digit() && (i == 0 || !es_ident(cs[i - 1])) {
            let f = numero(cs, i);
            t.push((i, f, Clase::Numero));
            i = f;
            continue;
        }
        if es_ident(c) {
            let mut f = i;
            while f < cs.len() && es_ident(cs[f]) {
                f += 1;
            }
            let w: String = cs[i..f].iter().collect();
            let clase = if PALABRAS_C.contains(&w.as_str()) {
                Clase::Palabra
            } else if TIPOS_C.contains(&w.as_str()) || w.ends_with("_t") {
                Clase::Tipo
            } else if cs[f..].iter().find(|c| !c.is_whitespace()) == Some(&'(') {
                Clase::Funcion
            } else {
                Clase::Normal
            };
            if clase != Clase::Normal {
                t.push((i, f, clase));
            }
            i = f;
            continue;
        }
        i += 1;
    }
    t
}

fn buscar(cs: &[char], desde: usize, aguja: &str) -> Option<usize> {
    let a: Vec<char> = aguja.chars().collect();
    (desde..cs.len().saturating_sub(a.len() - 1)).find(|&i| cs[i..].starts_with(&a))
}

fn resaltar_bash(cs: &[char]) -> Tramos {
    let mut t = vec![];
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c == '#' && (i == 0 || cs[i - 1].is_whitespace()) {
            t.push((i, cs.len(), Clase::Comentario));
            break;
        }
        if c == '"' || c == '\'' {
            let f = literal(cs, i, c);
            t.push((i, f, Clase::Cadena));
            i = f;
            continue;
        }
        if c == '$' {
            let mut f = i + 1;
            while f < cs.len() && (es_ident(cs[f]) || "{}?@#".contains(cs[f])) {
                f += 1;
            }
            t.push((i, f, Clase::Clave));
            i = f;
            continue;
        }
        if es_ident(c) {
            let mut f = i;
            while f < cs.len() && (es_ident(cs[f]) || cs[f] == '-') {
                f += 1;
            }
            let w: String = cs[i..f].iter().collect();
            if PALABRAS_BASH.contains(&w.as_str()) {
                t.push((i, f, Clase::Palabra));
            }
            i = f;
            continue;
        }
        i += 1;
    }
    t
}

fn resaltar_clave_valor(cs: &[char], separador: char) -> Tramos {
    let mut t = vec![];
    let ini = cs.iter().position(|c| !c.is_whitespace()).unwrap_or(cs.len());
    if ini == cs.len() {
        return t;
    }
    if cs[ini] == '#' {
        return vec![(ini, cs.len(), Clase::Comentario)];
    }
    if separador == '=' && cs[ini] == '[' {
        return vec![(ini, cs.len(), Clase::Encabezado)];
    }
    let mut i = ini;
    if separador == ':' && cs[ini] == '-' {
        t.push((ini, ini + 1, Clase::Palabra));
        i = ini + 1;
    }
    // Clave hasta el separador (fuera de comillas).
    if let Some(p) = cs[i..].iter().position(|&c| c == separador).map(|p| p + i) {
        if !cs[i..p].iter().any(|c| *c == '"' || *c == '\'') {
            t.push((i, p, Clase::Clave));
            i = p + 1;
        }
    }
    while i < cs.len() {
        let c = cs[i];
        if c == '#' && (i == 0 || cs[i - 1].is_whitespace()) {
            t.push((i, cs.len(), Clase::Comentario));
            break;
        }
        if c == '"' || c == '\'' {
            let f = literal(cs, i, c);
            t.push((i, f, Clase::Cadena));
            i = f;
            continue;
        }
        if (c.is_ascii_digit() || c == '-') && (i == 0 || !es_ident(cs[i - 1])) && cs.get(i + 1).is_none_or(|n| n.is_ascii_digit() || c.is_ascii_digit()) {
            let f = numero(cs, i + 1);
            t.push((i, f, Clase::Numero));
            i = f;
            continue;
        }
        if es_ident(c) {
            let mut f = i;
            while f < cs.len() && es_ident(cs[f]) {
                f += 1;
            }
            let w: String = cs[i..f].iter().collect();
            if matches!(w.as_str(), "true" | "false" | "null" | "yes" | "no") {
                t.push((i, f, Clase::Palabra));
            }
            i = f;
            continue;
        }
        i += 1;
    }
    t
}

fn resaltar_markdown(cs: &[char], estado: &mut Estado) -> Tramos {
    let texto: String = cs.iter().collect();
    let recortado = texto.trim_start();
    if recortado.starts_with("```") || recortado.starts_with("~~~") {
        estado.cercado = !estado.cercado;
        return vec![(0, cs.len(), Clase::Codigo)];
    }
    if estado.cercado {
        return vec![(0, cs.len(), Clase::Codigo)];
    }
    if recortado.starts_with('#') {
        return vec![(0, cs.len(), Clase::Encabezado)];
    }
    if recortado.starts_with('>') {
        return vec![(0, cs.len(), Clase::Cita)];
    }
    let mut t = vec![];
    let ini = cs.len() - recortado.chars().count();
    if recortado.starts_with("- ") || recortado.starts_with("* ") || recortado.starts_with("+ ") {
        t.push((ini, ini + 1, Clase::Palabra));
    } else if let Some(p) = recortado.find(". ") {
        if p > 0 && recortado[..p].chars().all(|c| c.is_ascii_digit()) {
            t.push((ini, ini + p + 1, Clase::Palabra));
        }
    }
    let mut i = 0;
    while i < cs.len() {
        match cs[i] {
            '`' => {
                let f = cs[i + 1..].iter().position(|&c| c == '`').map(|p| i + p + 2).unwrap_or(cs.len());
                t.push((i, f, Clase::Codigo));
                i = f;
            }
            '*' | '_' => {
                let marca = cs[i];
                let doble = cs.get(i + 1) == Some(&marca);
                let largo = if doble { 2 } else { 1 };
                let cierre = (i + largo + 1..cs.len()).find(|&j| cs[j] == marca && (!doble || cs.get(j + 1) == Some(&marca)));
                match cierre {
                    Some(j) if !(marca == '_' && i > 0 && es_ident(cs[i - 1])) => {
                        t.push((i, j + largo, Clase::Enfasis));
                        i = j + largo;
                    }
                    _ => i += 1,
                }
            }
            '[' => {
                let cierre = cs[i..].iter().position(|&c| c == ']').map(|p| i + p);
                match cierre {
                    Some(j) if cs.get(j + 1) == Some(&'(') => {
                        let f = cs[j..].iter().position(|&c| c == ')').map(|p| j + p + 1).unwrap_or(cs.len());
                        t.push((i, f, Clase::Enlace));
                        i = f;
                    }
                    _ => i += 1,
                }
            }
            _ => i += 1,
        }
    }
    t
}

/// Tramos de una línea. Para Markdown con bloque cercado, los bloques se colorean como código.
pub fn resaltar(lenguaje: Lenguaje, linea: &[char], estado: &mut Estado) -> Tramos {
    match lenguaje {
        Lenguaje::C => resaltar_c(linea, estado),
        Lenguaje::Markdown => resaltar_markdown(linea, estado),
        Lenguaje::Yaml => resaltar_clave_valor(linea, ':'),
        Lenguaje::Toml => resaltar_clave_valor(linea, '='),
        Lenguaje::Texto => vec![],
    }
}

/// Resaltado de un bloque de código de Markdown según su etiqueta (```c, ```bash...).
pub fn resaltar_etiqueta(etiqueta: &str, linea: &[char], estado: &mut Estado) -> Tramos {
    match etiqueta.trim().to_ascii_lowercase().as_str() {
        "c" | "h" | "c11" | "c17" => resaltar_c(linea, estado),
        "bash" | "sh" | "shell" | "console" | "zsh" => resaltar_bash(linea),
        "yaml" | "yml" => resaltar_clave_valor(linea, ':'),
        "toml" | "ini" => resaltar_clave_valor(linea, '='),
        _ => vec![],
    }
}

/// Estados al inicio de cada línea, calculados de forma perezosa por versión del buffer.
#[derive(Debug, Default)]
pub struct Cache {
    version: u64,
    estados: Vec<Estado>,
}

impl Cache {
    /// Estado al inicio de `linea`, con `obtener(i)` devolviendo la línea i.
    pub fn estado_en(&mut self, version: u64, lenguaje: Lenguaje, linea: usize, obtener: impl Fn(usize) -> Vec<char>) -> Estado {
        if self.version != version {
            self.version = version;
            self.estados.clear();
        }
        if self.estados.is_empty() {
            self.estados.push(Estado::default());
        }
        while self.estados.len() <= linea {
            let i = self.estados.len() - 1;
            let mut e = self.estados[i];
            resaltar(lenguaje, &obtener(i), &mut e);
            self.estados.push(e);
        }
        self.estados[linea]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clases(l: Lenguaje, s: &str) -> Vec<(String, Clase)> {
        let cs: Vec<char> = s.chars().collect();
        resaltar(l, &cs, &mut Estado::default()).into_iter().map(|(a, b, c)| (cs[a..b].iter().collect(), c)).collect()
    }

    #[test]
    fn c_basico() {
        let r = clases(Lenguaje::C, "int x = strlen(\"hola\"); // fin");
        assert_eq!(
            r,
            vec![
                ("int".into(), Clase::Tipo),
                ("strlen".into(), Clase::Funcion),
                ("\"hola\"".into(), Clase::Cadena),
                ("// fin".into(), Clase::Comentario)
            ]
        );
        assert_eq!(clases(Lenguaje::C, "#include <stdio.h>")[1], ("<stdio.h>".into(), Clase::Cadena));
        assert!(clases(Lenguaje::C, "x = 0x1F;").contains(&("0x1F".into(), Clase::Numero)));
    }

    #[test]
    fn comentario_de_bloque_multilinea() {
        let mut e = Estado::default();
        let l1: Vec<char> = "int a; /* abre".chars().collect();
        let l2: Vec<char> = "sigue */ return a;".chars().collect();
        resaltar(Lenguaje::C, &l1, &mut e);
        assert!(e.comentario_bloque);
        let t = resaltar(Lenguaje::C, &l2, &mut e);
        assert_eq!(t[0], (0, 8, Clase::Comentario));
        assert_eq!(t[1].2, Clase::Palabra);
        assert!(!e.comentario_bloque);
    }

    #[test]
    fn markdown_y_toml() {
        assert_eq!(clases(Lenguaje::Markdown, "# Título")[0].1, Clase::Encabezado);
        let r = clases(Lenguaje::Markdown, "usar `malloc` y **validar**");
        assert_eq!(r, vec![("`malloc`".into(), Clase::Codigo), ("**validar**".into(), Clase::Enfasis)]);
        let r = clases(Lenguaje::Toml, "comando = \"make\" # x");
        assert_eq!(r[0], ("comando ".into(), Clase::Clave));
        assert_eq!(r[1].1, Clase::Cadena);
        assert_eq!(clases(Lenguaje::Yaml, "activo: true")[1].1, Clase::Palabra);
    }

    #[test]
    fn cache_arrastra_estado() {
        let lineas = ["/*", "x", "*/", "int y;"];
        let mut c = Cache::default();
        let e = c.estado_en(1, Lenguaje::C, 2, |i| lineas[i].chars().collect());
        assert!(e.comentario_bloque);
        let e = c.estado_en(1, Lenguaje::C, 3, |i| lineas[i].chars().collect());
        assert!(!e.comentario_bloque);
    }
}
