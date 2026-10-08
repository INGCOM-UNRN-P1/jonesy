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
    /// Error visible (en Makefiles: receta sangrada con espacios en vez de TAB).
    Invalido,
}

/// Estado al inicio de una línea.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Estado {
    pub comentario_bloque: bool,
    pub cercado: bool,
    /// Makefile: la línea anterior fue una regla o una receta (siguen recetas).
    pub en_regla: bool,
    /// La línea anterior terminó en `\` (continuación).
    pub continua: bool,
}

/// Tramos (inicio, fin) en índices de carácter.
pub type Tramos = Vec<(usize, usize, Clase)>;

const PALABRAS_C: &[&str] = &[
    "auto",
    "break",
    "case",
    "const",
    "continue",
    "default",
    "do",
    "else",
    "enum",
    "extern",
    "for",
    "goto",
    "if",
    "inline",
    "register",
    "restrict",
    "return",
    "sizeof",
    "static",
    "struct",
    "switch",
    "typedef",
    "union",
    "volatile",
    "while",
    "_Alignas",
    "_Alignof",
    "_Atomic",
    "_Generic",
    "_Noreturn",
    "_Static_assert",
    "_Thread_local",
    "NULL",
    "true",
    "false",
];

const TIPOS_C: &[&str] = &[
    "void",
    "char",
    "short",
    "int",
    "long",
    "float",
    "double",
    "signed",
    "unsigned",
    "_Bool",
    "bool",
    "size_t",
    "ssize_t",
    "FILE",
    "int8_t",
    "int16_t",
    "int32_t",
    "int64_t",
    "uint8_t",
    "uint16_t",
    "uint32_t",
    "uint64_t",
    "uintptr_t",
    "intptr_t",
    "ptrdiff_t",
    "pid_t",
    "off_t",
    "time_t",
    "clock_t",
];

const PALABRAS_BASH: &[&str] = &[
    "if", "then", "else", "elif", "fi", "for", "while", "until", "do", "done", "case", "esac",
    "in", "function", "return", "local", "export", "set", "echo", "cd", "exit",
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
            let f = cs[i..]
                .iter()
                .position(|&c| c == '>')
                .map(|x| i + x + 1)
                .unwrap_or(cs.len());
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
    let ini = cs
        .iter()
        .position(|c| !c.is_whitespace())
        .unwrap_or(cs.len());
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
        if (c.is_ascii_digit() || c == '-')
            && (i == 0 || !es_ident(cs[i - 1]))
            && cs
                .get(i + 1)
                .is_none_or(|n| n.is_ascii_digit() || c.is_ascii_digit())
        {
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
                let f = cs[i + 1..]
                    .iter()
                    .position(|&c| c == '`')
                    .map(|p| i + p + 2)
                    .unwrap_or(cs.len());
                t.push((i, f, Clase::Codigo));
                i = f;
            }
            '*' | '_' => {
                let marca = cs[i];
                let doble = cs.get(i + 1) == Some(&marca);
                let largo = if doble { 2 } else { 1 };
                let cierre = (i + largo + 1..cs.len())
                    .find(|&j| cs[j] == marca && (!doble || cs.get(j + 1) == Some(&marca)));
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
                        let f = cs[j..]
                            .iter()
                            .position(|&c| c == ')')
                            .map(|p| j + p + 1)
                            .unwrap_or(cs.len());
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

const DIRECTIVAS_MAKE: &[&str] = &[
    "include", "-include", "sinclude", "ifeq", "ifneq", "ifdef", "ifndef", "else", "endif",
    "define", "endef", "export", "unexport", "override", "vpath", "undefine", "private",
];

/// Fin de una referencia `$(...)` / `${...}` que empieza en `i` (con anidamiento).
fn fin_referencia(cs: &[char], i: usize) -> usize {
    let (abre, cierra) = match cs.get(i + 1) {
        Some('(') => ('(', ')'),
        Some('{') => ('{', '}'),
        // $@ $< $^ $? $* $+ $% y $X de un carácter; $$ es un $ para la shell.
        Some(_) => return (i + 2).min(cs.len()),
        None => return cs.len(),
    };
    let mut nivel = 0;
    let mut j = i + 1;
    while j < cs.len() {
        if cs[j] == abre {
            nivel += 1;
        } else if cs[j] == cierra {
            nivel -= 1;
            if nivel == 0 {
                return j + 1;
            }
        }
        j += 1;
    }
    cs.len()
}

/// Referencias a variables, cadenas y comentarios en `cs[desde..]` (valores, recetas y
/// prerrequisitos). En recetas además se resaltan palabras clave de la shell.
fn tramos_make_resto(cs: &[char], desde: usize, receta: bool, t: &mut Tramos) {
    let mut i = desde;
    while i < cs.len() {
        let c = cs[i];
        if c == '#' && (i == 0 || cs[i - 1] != '\\') {
            t.push((i, cs.len(), Clase::Comentario));
            return;
        }
        if c == '$' {
            let f = fin_referencia(cs, i);
            let clase = if cs.get(i + 1) == Some(&'$') {
                Clase::Clave
            } else {
                Clase::Tipo
            };
            t.push((i, f, clase));
            i = f;
            continue;
        }
        if receta && (c == '"' || c == '\'') {
            let f = literal(cs, i, c);
            t.push((i, f, Clase::Cadena));
            i = f;
            continue;
        }
        if receta && es_ident(c) && (i == 0 || !es_ident(cs[i - 1])) {
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
}

/// Makefile: directivas, asignaciones, reglas (objetivos y prerrequisitos), recetas con
/// sus prefijos `@`/`-`/`+`, referencias `$(VAR)` y comentarios. Una receta sangrada con
/// espacios en lugar de TAB (el «missing separator» de make) se marca como inválida.
fn resaltar_make(cs: &[char], e: &mut Estado) -> Tramos {
    let mut t = vec![];
    let continuacion = e.continua;
    e.continua = cs.last() == Some(&'\\');
    let vacia = cs.iter().all(|c| c.is_whitespace());
    if vacia {
        return t;
    }
    // Continuación de la línea anterior: mismo tratamiento que un valor.
    if continuacion {
        tramos_make_resto(cs, 0, e.en_regla, &mut t);
        return t;
    }
    let primero = cs.iter().position(|c| !c.is_whitespace()).unwrap_or(0);
    if cs[0] == '\t' {
        if cs[primero] == '#' {
            t.push((0, cs.len(), Clase::Comentario));
            return t;
        }
        if e.en_regla {
            let mut i = primero;
            while i < cs.len() && matches!(cs[i], '@' | '-' | '+') {
                i += 1;
            }
            if i > primero {
                t.push((primero, i, Clase::Palabra));
            }
            tramos_make_resto(cs, i, true, &mut t);
            return t;
        }
    } else if primero > 0 && e.en_regla && cs[primero] != '#' {
        // Sangría con espacios donde make espera un TAB.
        t.push((0, primero, Clase::Invalido));
        tramos_make_resto(cs, primero, true, &mut t);
        return t;
    }
    if cs[primero] == '#' {
        t.push((primero, cs.len(), Clase::Comentario));
        return t;
    }
    // Directiva (ifeq, include, define…).
    let mut f = primero;
    while f < cs.len() && !cs[f].is_whitespace() && cs[f] != '(' {
        f += 1;
    }
    let palabra: String = cs[primero..f].iter().collect();
    if DIRECTIVAS_MAKE.contains(&palabra.as_str()) {
        t.push((primero, f, Clase::Preproc));
        tramos_make_resto(cs, f, false, &mut t);
        e.en_regla = false;
        return t;
    }
    // Asignación: NOMBRE = | := | ::= | ?= | += | != valor (antes de cualquier ':' de regla).
    let texto: String = cs.iter().collect();
    let asignacion = texto.char_indices().find_map(|(b, c)| match c {
        '=' => Some(b),
        ':' if !texto[b..].starts_with(":=") && !texto[b..].starts_with("::=") => Some(usize::MAX),
        _ => None,
    });
    if let Some(b) = asignacion.filter(|b| *b != usize::MAX) {
        let i = texto[..b].chars().count();
        let mut fin_nombre = i;
        while fin_nombre > primero
            && matches!(cs[fin_nombre - 1], ':' | '?' | '+' | '!' | ' ' | '\t')
        {
            fin_nombre -= 1;
        }
        t.push((primero, fin_nombre, Clase::Clave));
        tramos_make_resto(cs, i + 1, false, &mut t);
        e.en_regla = false;
        return t;
    }
    // Regla: objetivos: prerrequisitos [; receta]
    if let Some(dos) = cs.iter().position(|&c| c == ':') {
        let mut i = primero;
        while i < dos {
            if cs[i] == '$' {
                let f = fin_referencia(cs, i).min(dos);
                t.push((i, f, Clase::Tipo));
                i = f;
                continue;
            }
            if !cs[i].is_whitespace() {
                let mut f = i;
                while f < dos && !cs[f].is_whitespace() && cs[f] != '$' {
                    f += 1;
                }
                let clase = if cs[i] == '.' {
                    Clase::Palabra
                } else {
                    Clase::Funcion
                };
                t.push((i, f, clase));
                i = f;
                continue;
            }
            i += 1;
        }
        tramos_make_resto(cs, dos + 1, false, &mut t);
        e.en_regla = true;
        return t;
    }
    tramos_make_resto(cs, primero, false, &mut t);
    t
}

/// Tramos de una línea. Para Markdown con bloque cercado, los bloques se colorean como código.
pub fn resaltar(lenguaje: Lenguaje, linea: &[char], estado: &mut Estado) -> Tramos {
    match lenguaje {
        Lenguaje::C => resaltar_c(linea, estado),
        Lenguaje::Markdown => resaltar_markdown(linea, estado),
        Lenguaje::Yaml => resaltar_clave_valor(linea, ':'),
        Lenguaje::Toml => resaltar_clave_valor(linea, '='),
        Lenguaje::Makefile => resaltar_make(linea, estado),
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
        "make" | "makefile" | "mk" => resaltar_make(linea, estado),
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
    pub fn estado_en(
        &mut self,
        version: u64,
        lenguaje: Lenguaje,
        linea: usize,
        obtener: impl Fn(usize) -> Vec<char>,
    ) -> Estado {
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
        resaltar(l, &cs, &mut Estado::default())
            .into_iter()
            .map(|(a, b, c)| (cs[a..b].iter().collect(), c))
            .collect()
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
        assert_eq!(
            clases(Lenguaje::C, "#include <stdio.h>")[1],
            ("<stdio.h>".into(), Clase::Cadena)
        );
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
        assert_eq!(
            clases(Lenguaje::Markdown, "# Título")[0].1,
            Clase::Encabezado
        );
        let r = clases(Lenguaje::Markdown, "usar `malloc` y **validar**");
        assert_eq!(
            r,
            vec![
                ("`malloc`".into(), Clase::Codigo),
                ("**validar**".into(), Clase::Enfasis)
            ]
        );
        let r = clases(Lenguaje::Toml, "comando = \"make\" # x");
        assert_eq!(r[0], ("comando ".into(), Clase::Clave));
        assert_eq!(r[1].1, Clase::Cadena);
        assert_eq!(clases(Lenguaje::Yaml, "activo: true")[1].1, Clase::Palabra);
    }

    #[test]
    fn makefile() {
        let mut e = Estado::default();
        let mut linea = |s: &str| {
            let cs: Vec<char> = s.chars().collect();
            resaltar(Lenguaje::Makefile, &cs, &mut e)
                .into_iter()
                .map(|(a, b, c)| (cs[a..b].iter().collect::<String>(), c))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            linea("CFLAGS ?= -Wall $(EXTRA) # opciones"),
            vec![
                ("CFLAGS".into(), Clase::Clave),
                ("$(EXTRA)".into(), Clase::Tipo),
                ("# opciones".into(), Clase::Comentario),
            ]
        );
        assert_eq!(
            linea(".PHONY: run"),
            vec![(".PHONY".into(), Clase::Palabra),]
        );
        assert_eq!(
            linea("$(PROG): main.o $(OBJS)"),
            vec![
                ("$(PROG)".into(), Clase::Tipo),
                ("$(OBJS)".into(), Clase::Tipo),
            ]
        );
        let r = linea("\t@echo \"hola $@\" && ./$(PROG)");
        assert_eq!(r[0], ("@".into(), Clase::Palabra));
        assert_eq!(r[1], ("echo".into(), Clase::Palabra));
        assert_eq!(r[2].1, Clase::Cadena);
        assert!(r.contains(&("$(PROG)".into(), Clase::Tipo)));
        // Receta sangrada con espacios: inválida.
        let r = linea("    rm -f *.o");
        assert_eq!(r[0], ("    ".into(), Clase::Invalido));
        assert_eq!(linea("ifneq ($(X),)")[0], ("ifneq".into(), Clase::Preproc));
        assert_eq!(linea("all: prog")[0], ("all".into(), Clase::Funcion));
        // Continuación con barra: no es un error de sangría.
        linea("SRCS = a.c \\");
        assert_eq!(linea("    b.c $(X)"), vec![("$(X)".into(), Clase::Tipo)]);
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
