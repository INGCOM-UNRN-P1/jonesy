//! Símbolos de un archivo C: funciones (definiciones y prototipos), structs, enums,
//! typedefs y macros, con su línea. Alimenta el esquema del archivo (barra izquierda),
//! «ir a la definición» (F12 o Ctrl+clic) y la búsqueda de definiciones en el proyecto.
//!
//! No es un parser de C: recorre el texto salteando comentarios, cadenas y directivas
//! del preprocesador (salvo `#define`), lleva la profundidad de llaves y reconoce los
//! patrones de nivel superior. Alcanza para el código de la materia.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    Funcion,
    Prototipo,
    Struct,
    Enum,
    Typedef,
    Macro,
}

impl Tipo {
    pub fn icono(self) -> &'static str {
        match self {
            Tipo::Funcion => "ƒ",
            Tipo::Prototipo => "·",
            Tipo::Struct => "S",
            Tipo::Enum => "E",
            Tipo::Typedef => "T",
            Tipo::Macro => "#",
        }
    }

    /// Las definiciones primero al buscar (una función antes que su prototipo).
    fn prioridad(self) -> u8 {
        match self {
            Tipo::Funcion => 0,
            Tipo::Struct | Tipo::Enum | Tipo::Typedef | Tipo::Macro => 1,
            Tipo::Prototipo => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Simbolo {
    pub nombre: String,
    pub tipo: Tipo,
    /// 0-based.
    pub linea: usize,
    pub columna: usize,
    /// La línea donde aparece, sin sangría (para mostrar la firma).
    pub texto: String,
}

const RESERVADAS: &[&str] = &[
    "if", "while", "for", "switch", "return", "sizeof", "do", "else", "case", "defined",
];

fn es_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// El texto con comentarios, cadenas y directivas (salvo #define) reemplazados por
/// espacios: mismas posiciones, nada que confunda al reconocer símbolos.
pub fn limpio(fuente: &[char]) -> Vec<char> {
    let mut t = fuente.to_vec();
    let mut i = 0;
    let mut inicio_linea = true;
    while i < t.len() {
        let c = t[i];
        if c == '\n' {
            inicio_linea = true;
            i += 1;
            continue;
        }
        if inicio_linea && c == '#' {
            // Directiva: se conserva sólo `#define NOMBRE`.
            let fin = (i..t.len())
                .find(|&j| t[j] == '\n' && t[j - 1] != '\\')
                .unwrap_or(t.len());
            let linea: String = t[i..fin].iter().collect();
            let es_define = linea
                .trim_start_matches('#')
                .trim_start()
                .starts_with("define");
            if !es_define {
                t[i..fin].iter_mut().for_each(|x| {
                    if *x != '\n' {
                        *x = ' '
                    }
                });
            }
            i = fin;
            continue;
        }
        if !c.is_whitespace() {
            inicio_linea = false;
        }
        match c {
            '/' if t.get(i + 1) == Some(&'/') => {
                while i < t.len() && t[i] != '\n' {
                    t[i] = ' ';
                    i += 1;
                }
            }
            '/' if t.get(i + 1) == Some(&'*') => {
                while i < t.len() && !(t[i] == '*' && t.get(i + 1) == Some(&'/')) {
                    if t[i] != '\n' {
                        t[i] = ' ';
                    }
                    i += 1;
                }
                for _ in 0..2 {
                    if i < t.len() {
                        t[i] = ' ';
                        i += 1;
                    }
                }
            }
            '"' | '\'' => {
                let q = c;
                t[i] = ' ';
                i += 1;
                while i < t.len() && t[i] != q && t[i] != '\n' {
                    if t[i] == '\\' {
                        t[i] = ' ';
                        i += 1;
                    }
                    if i < t.len() {
                        t[i] = ' ';
                        i += 1;
                    }
                }
                if i < t.len() && t[i] == q {
                    t[i] = ' ';
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    t
}

/// Los símbolos de un texto en C, en orden de aparición.
pub fn de_texto(fuente: &str) -> Vec<Simbolo> {
    let orig: Vec<char> = fuente.chars().collect();
    let t = limpio(&orig);
    // Inicio de cada línea, para pasar de posición a (línea, columna).
    let mut inicios = vec![0usize];
    for (i, c) in orig.iter().enumerate() {
        if *c == '\n' {
            inicios.push(i + 1);
        }
    }
    let ubicar = |pos: usize| {
        let l = inicios.partition_point(|&x| x <= pos) - 1;
        (l, pos - inicios[l])
    };
    let texto_linea = |l: usize| -> String {
        let fin = inicios.get(l + 1).map(|x| x - 1).unwrap_or(orig.len());
        orig[inicios[l]..fin]
            .iter()
            .collect::<String>()
            .trim()
            .to_string()
    };
    let mut v: Vec<Simbolo> = vec![];
    let mut agregar = |nombre: String, tipo: Tipo, pos: usize| {
        let (linea, columna) = ubicar(pos);
        v.push(Simbolo {
            nombre,
            tipo,
            linea,
            columna,
            texto: texto_linea(linea),
        });
    };
    let palabra_en = |i: usize| -> (String, usize) {
        let mut j = i;
        while j < t.len() && es_ident(t[j]) {
            j += 1;
        }
        (t[i..j].iter().collect(), j)
    };
    let saltar_blancos = |mut i: usize| {
        while i < t.len() && t[i].is_whitespace() {
            i += 1;
        }
        i
    };
    let mut nivel = 0usize;
    let mut i = 0;
    // Inicio de la declaración de nivel superior en curso (para typedef/struct).
    let mut inicio_decl = 0;
    // La llave del nivel superior es parte de una declaración que sigue hasta un `;`
    // (typedef struct {…} t; struct x {…}; int v[] = {…};), no el cuerpo de una función.
    let mut llave_de_declaracion = false;
    while i < t.len() {
        let c = t[i];
        match c {
            '{' => {
                // `struct nombre {` / `enum nombre {` en el nivel superior.
                if nivel == 0 {
                    let decl: String = t[inicio_decl..i].iter().collect();
                    let palabras: Vec<&str> = decl.split_whitespace().collect();
                    llave_de_declaracion = decl.contains('=')
                        || palabras
                            .iter()
                            .any(|p| matches!(*p, "typedef" | "struct" | "enum" | "union"))
                            && !decl.trim_end().ends_with(')');
                    if let Some(k) = palabras.iter().position(|p| *p == "struct" || *p == "enum") {
                        if let Some(n) = palabras.get(k + 1).filter(|n| n.chars().all(es_ident)) {
                            let tipo = if palabras[k] == "struct" {
                                Tipo::Struct
                            } else {
                                Tipo::Enum
                            };
                            if let Some(p) = buscar_palabra(&t, inicio_decl, i, n) {
                                agregar(n.to_string(), tipo, p);
                            }
                        }
                    }
                }
                nivel += 1;
                i += 1;
            }
            '}' => {
                nivel = nivel.saturating_sub(1);
                i += 1;
                if nivel == 0 && !llave_de_declaracion {
                    inicio_decl = i;
                }
            }
            ';' => {
                if nivel == 0 {
                    // typedef … Nombre;
                    let decl: String = t[inicio_decl..i].iter().collect();
                    let d = decl.trim_start();
                    if d.starts_with("typedef") {
                        let nombre: String = d
                            .trim_end()
                            .chars()
                            .rev()
                            .take_while(|c| es_ident(*c))
                            .collect::<Vec<_>>()
                            .into_iter()
                            .rev()
                            .collect();
                        if !nombre.is_empty() && !d.trim_end().ends_with(')') {
                            if let Some(p) = buscar_palabra_ultima(&t, inicio_decl, i, &nombre) {
                                agregar(nombre, Tipo::Typedef, p);
                            }
                        }
                    }
                    inicio_decl = i + 1;
                    llave_de_declaracion = false;
                }
                i += 1;
            }
            '#' => {
                // #define NOMBRE (las demás directivas ya son espacios).
                let j = saltar_blancos(i + 1);
                let (pal, fin) = palabra_en(j);
                if pal == "define" {
                    let k = saltar_blancos(fin);
                    let (nombre, _) = palabra_en(k);
                    if !nombre.is_empty() {
                        agregar(nombre, Tipo::Macro, k);
                    }
                }
                while i < t.len() && t[i] != '\n' {
                    i += 1;
                }
                inicio_decl = i;
            }
            c if nivel == 0 && es_ident(c) && (i == 0 || !es_ident(t[i - 1])) => {
                let (pal, fin) = palabra_en(i);
                let j = saltar_blancos(fin);
                if t.get(j) == Some(&'(')
                    && !RESERVADAS.contains(&pal.as_str())
                    && !pal.chars().next().unwrap().is_ascii_digit()
                {
                    // Paréntesis de la lista de parámetros.
                    let mut p = 0i32;
                    let mut k = j;
                    while k < t.len() {
                        match t[k] {
                            '(' => p += 1,
                            ')' => {
                                p -= 1;
                                if p == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        k += 1;
                    }
                    let despues = saltar_blancos(k + 1);
                    // Debe haber un tipo antes (no es una llamada suelta ni un macro).
                    let antes: String = t[inicio_decl..i].iter().collect();
                    let con_tipo = antes.split_whitespace().any(|w| w.chars().any(es_ident))
                        || antes.contains('*');
                    let es_puntero_a_funcion = antes.trim_end().ends_with("(*");
                    if con_tipo && !es_puntero_a_funcion && !antes.contains('=') {
                        match t.get(despues) {
                            Some('{') => agregar(pal.clone(), Tipo::Funcion, i),
                            Some(';') | Some(',') => agregar(pal.clone(), Tipo::Prototipo, i),
                            _ => {}
                        }
                    }
                    i = fin;
                } else {
                    i = fin;
                }
            }
            _ => i += 1,
        }
    }
    v
}

fn buscar_palabra(t: &[char], desde: usize, hasta: usize, nombre: &str) -> Option<usize> {
    let n: Vec<char> = nombre.chars().collect();
    (desde..hasta.saturating_sub(n.len() - 1)).find(|&i| {
        t[i..i + n.len()] == n[..]
            && (i == 0 || !es_ident(t[i - 1]))
            && t.get(i + n.len()).is_none_or(|c| !es_ident(*c))
    })
}

fn buscar_palabra_ultima(t: &[char], desde: usize, hasta: usize, nombre: &str) -> Option<usize> {
    let n: Vec<char> = nombre.chars().collect();
    (desde..hasta.saturating_sub(n.len() - 1)).rev().find(|&i| {
        t[i..i + n.len()] == n[..]
            && (i == 0 || !es_ident(t[i - 1]))
            && t.get(i + n.len()).is_none_or(|c| !es_ident(*c))
    })
}

/// Archivos C del proyecto (`.c` y `.h`), salteando lo que no es fuente.
pub fn archivos_c(raiz: &Path) -> Vec<PathBuf> {
    let mut v = vec![];
    recorrer(raiz, &mut v, 0, &|p| {
        p.extension().is_some_and(|e| e == "c" || e == "h")
    });
    v
}

/// Recorre `dir` (hasta 6 niveles y 5000 archivos) salteando ocultos, build y target.
pub fn recorrer(dir: &Path, v: &mut Vec<PathBuf>, prof: usize, filtro: &dyn Fn(&Path) -> bool) {
    if prof > 6 || v.len() > 5000 {
        return;
    }
    let Ok(it) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entradas: Vec<PathBuf> = it.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entradas.sort();
    for p in entradas {
        let nombre = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if nombre.starts_with('.')
            || matches!(nombre.as_str(), "target" | "node_modules" | "__pycache__")
        {
            continue;
        }
        if p.is_dir() {
            recorrer(&p, v, prof + 1, filtro);
        } else if filtro(&p) {
            v.push(p);
        }
    }
}

/// Dónde está definido `nombre`: primero en `actual` (archivo y texto del buffer, que
/// puede no estar guardado), después en los `.c`/`.h` del proyecto. Las definiciones de
/// funciones ganan a los prototipos.
pub fn definicion(
    nombre: &str,
    actual: Option<(&Path, &str)>,
    raiz: &Path,
) -> Option<(PathBuf, Simbolo)> {
    let mut candidatos: Vec<(PathBuf, Simbolo)> = vec![];
    if let Some((ruta, texto)) = actual {
        candidatos.extend(
            de_texto(texto)
                .into_iter()
                .filter(|s| s.nombre == nombre)
                .map(|s| (ruta.to_path_buf(), s)),
        );
    }
    if !candidatos.iter().any(|(_, s)| s.tipo == Tipo::Funcion) {
        for p in archivos_c(raiz) {
            if actual.is_some_and(|(r, _)| r == p) {
                continue;
            }
            let Ok(texto) = std::fs::read_to_string(&p) else {
                continue;
            };
            if !texto.contains(nombre) {
                continue;
            }
            candidatos.extend(
                de_texto(&texto)
                    .into_iter()
                    .filter(|s| s.nombre == nombre)
                    .map(|s| (p.clone(), s)),
            );
        }
    }
    candidatos.sort_by_key(|(_, s)| s.tipo.prioridad());
    candidatos.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CODIGO: &str = r#"#include <stdio.h>
#define MAXIMO 10

typedef struct nodo {
    int valor;
    struct nodo *sig;
} nodo_t;

enum color { ROJO, VERDE };

int suma(int a, int b);
static char *
duplicar(const char *s,
         size_t n)
{
    if (s == NULL) { return NULL; } /* suma(x) */
    printf("main(void) {");
    return strncpy(malloc(n), s, n);
}

int suma(int a, int b) { return a + b; }

int main(void)
{
    return suma(1, 2);
}
"#;

    #[test]
    fn reconoce_funciones_tipos_y_macros() {
        let s = de_texto(CODIGO);
        let v: Vec<(&str, Tipo, usize)> = s
            .iter()
            .map(|x| (x.nombre.as_str(), x.tipo, x.linea))
            .collect();
        assert_eq!(
            v,
            vec![
                ("MAXIMO", Tipo::Macro, 1),
                ("nodo", Tipo::Struct, 3),
                ("nodo_t", Tipo::Typedef, 6),
                ("color", Tipo::Enum, 8),
                ("suma", Tipo::Prototipo, 10),
                ("duplicar", Tipo::Funcion, 12),
                ("suma", Tipo::Funcion, 20),
                ("main", Tipo::Funcion, 22),
            ]
        );
        assert_eq!(s[5].texto, "duplicar(const char *s,");
        assert_eq!(s[6].columna, 4);
    }

    #[test]
    fn definicion_en_el_buffer_y_en_el_proyecto() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("libs/lista")).unwrap();
        std::fs::write(d.path().join("libs/lista/lista.h"), "int largo(void *l);\n").unwrap();
        std::fs::write(
            d.path().join("libs/lista/lista.c"),
            "#include \"lista.h\"\n\nint largo(void *l)\n{\n    return 0;\n}\n",
        )
        .unwrap();
        std::fs::create_dir_all(d.path().join(".git")).unwrap();
        std::fs::write(d.path().join(".git/x.c"), "int largo(void) { }\n").unwrap();
        let main = d.path().join("main.c");
        let texto = "int largo(void *l);\nint main(void) { return largo(0); }\n";
        let (ruta, s) = definicion("largo", Some((&main, texto)), d.path()).unwrap();
        assert_eq!(
            ruta,
            d.path().join("libs/lista/lista.c"),
            "la definición gana al prototipo"
        );
        assert_eq!((s.linea, s.tipo), (2, Tipo::Funcion));
        // Lo del buffer sin guardar también cuenta.
        let (ruta, s) = definicion("main", Some((&main, texto)), d.path()).unwrap();
        assert_eq!((ruta, s.linea), (main.clone(), 1));
        assert!(definicion("nada", Some((&main, texto)), d.path()).is_none());
    }
}
