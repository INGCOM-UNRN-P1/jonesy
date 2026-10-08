//! `quick-doc` (RF-9): ayuda contextual offline de funciones C.
//!
//! - [`Base`]: libc, POSIX y tipos de cátedra embebidos en el binario (`data/libc.toml`),
//!   más las funciones que el alumno declara en sus propios archivos.
//! - [`contexto`]: autómata que, desde la posición del cursor, decide si el alumno está
//!   dentro de una llamada (y en qué parámetro) o sobre un identificador. Ignora strings,
//!   caracteres y comentarios; no necesita un servidor LSP.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Parametro {
    pub tipo: String,
    pub nombre: String,
    #[serde(default)]
    pub desc: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Funcion {
    pub nombre: String,
    pub cabecera: String,
    pub firma: String,
    #[serde(default)]
    pub descripcion: String,
    #[serde(default)]
    pub retorno: String,
    #[serde(default)]
    pub parametros: Vec<Parametro>,
    #[serde(default)]
    pub trampas: Vec<String>,
    #[serde(default)]
    pub reglas: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Tipo {
    pub nombre: String,
    pub cabecera: String,
    pub descripcion: String,
    #[serde(default)]
    pub trampas: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Archivo {
    #[serde(default)]
    funcion: Vec<Funcion>,
    #[serde(default)]
    tipo: Vec<Tipo>,
}

/// Base de conocimiento.
#[derive(Debug, Default)]
pub struct Base {
    funciones: HashMap<String, Funcion>,
    tipos: HashMap<String, Tipo>,
}

const DATOS: &str = include_str!("../data/libc.toml");

impl Base {
    /// La base embebida (se interpreta una vez por proceso).
    pub fn embebida() -> &'static Base {
        static BASE: OnceLock<Base> = OnceLock::new();
        BASE.get_or_init(|| {
            let archivo: Archivo =
                toml::from_str(DATOS).expect("data/libc.toml es válido (lo verifica un test)");
            Base {
                funciones: archivo
                    .funcion
                    .into_iter()
                    .map(|f| (f.nombre.clone(), f))
                    .collect(),
                tipos: archivo
                    .tipo
                    .into_iter()
                    .map(|t| (t.nombre.clone(), t))
                    .collect(),
            }
        })
    }

    pub fn funcion(&self, nombre: &str) -> Option<&Funcion> {
        self.funciones.get(nombre)
    }

    pub fn tipo(&self, nombre: &str) -> Option<&Tipo> {
        self.tipos.get(nombre)
    }

    pub fn cantidad_funciones(&self) -> usize {
        self.funciones.len()
    }

    pub fn nombres(&self) -> impl Iterator<Item = &str> {
        self.funciones.keys().map(String::as_str)
    }
}

/// Lo que hay que mostrar para la posición del cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Contexto {
    /// Dentro de los paréntesis de una llamada: función y parámetro activo (0-based).
    Llamada {
        funcion: String,
        parametro: usize,
    },
    /// Sobre un identificador.
    Identificador(String),
    Nada,
}

const MAX_VENTANA: usize = 4000;

fn es_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Determina el contexto en `pos` (índice de carácter) de `texto`.
///
/// Prioriza la llamada abierta más cercana (ayuda de firma mientras se tipea); si el cursor
/// no está dentro de una, usa el identificador bajo el cursor.
pub fn contexto(texto: &[char], pos: usize) -> Contexto {
    let pos = pos.min(texto.len());
    if let Some(c) = llamada_abierta(texto, pos) {
        // Sobre el nombre de otra función llamada adentro (`printf("%zu", str|len(s))`)
        // gana esa función; sobre un argumento cualquiera, la firma de la llamada.
        if nombre_de_llamada_en(texto, pos) {
            if let Some(id) = identificador_en(texto, pos) {
                return Contexto::Identificador(id);
            }
        }
        return Contexto::Llamada {
            funcion: c.0,
            parametro: c.1,
        };
    }
    identificador_en(texto, pos)
        .map(Contexto::Identificador)
        .unwrap_or(Contexto::Nada)
}

/// Verdadero si el cursor está sobre un identificador seguido de `(`.
fn nombre_de_llamada_en(texto: &[char], pos: usize) -> bool {
    if !texto.get(pos).is_some_and(|c| es_ident(*c)) && !(pos > 0 && es_ident(texto[pos - 1])) {
        return false;
    }
    let mut fin = pos;
    while fin < texto.len() && es_ident(texto[fin]) {
        fin += 1;
    }
    while fin < texto.len() && texto[fin] == ' ' {
        fin += 1;
    }
    texto.get(fin) == Some(&'(')
}

fn identificador_en(texto: &[char], pos: usize) -> Option<String> {
    let mut ini = pos;
    while ini > 0 && es_ident(texto[ini - 1]) {
        ini -= 1;
    }
    let mut fin = pos;
    while fin < texto.len() && es_ident(texto[fin]) {
        fin += 1;
    }
    if ini == fin || texto[ini].is_ascii_digit() {
        return None;
    }
    Some(texto[ini..fin].iter().collect())
}

/// Recorre hacia adelante una ventana previa al cursor marcando qué caracteres son código
/// (no strings, chars ni comentarios) y, con una pila, encuentra el `(` sin cerrar.
fn llamada_abierta(texto: &[char], pos: usize) -> Option<(String, usize)> {
    let inicio = inicio_seguro(texto, pos);
    // Pila de paréntesis abiertos: (posición del '(', comas vistas en su nivel).
    let mut pila: Vec<(usize, usize)> = Vec::new();
    let mut i = inicio;
    while i < pos {
        let c = texto[i];
        let sig = texto.get(i + 1).copied();
        match c {
            '/' if sig == Some('/') => {
                while i < pos && texto[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            '/' if sig == Some('*') => {
                i += 2;
                while i + 1 < pos && !(texto[i] == '*' && texto[i + 1] == '/') {
                    i += 1;
                }
                i += 2;
                continue;
            }
            '"' | '\'' => {
                let cierre = c;
                i += 1;
                while i < pos && texto[i] != cierre && texto[i] != '\n' {
                    if texto[i] == '\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i += 1;
                continue;
            }
            '(' => pila.push((i, 0)),
            ')' => {
                pila.pop();
            }
            ',' => {
                if let Some(tope) = pila.last_mut() {
                    tope.1 += 1;
                }
            }
            ';' | '{' | '}' => pila.clear(),
            _ => {}
        }
        i += 1;
    }
    // La llamada más interna con nombre de función delante (no if/while/for…).
    for &(abre, comas) in pila.iter().rev() {
        let mut j = abre;
        while j > 0 && texto[j - 1].is_whitespace() {
            j -= 1;
        }
        let fin = j;
        while j > 0 && es_ident(texto[j - 1]) {
            j -= 1;
        }
        if j == fin {
            continue;
        }
        let nombre: String = texto[j..fin].iter().collect();
        if matches!(
            nombre.as_str(),
            "if" | "while" | "for" | "switch" | "return" | "sizeof"
        ) {
            continue;
        }
        return Some((nombre, comas));
    }
    None
}

/// Comienzo de la ventana: el inicio de la línea que está MAX_VENTANA caracteres atrás
/// (las llamadas no cruzan funciones, y `;`/`{` vacían la pila igual).
fn inicio_seguro(texto: &[char], pos: usize) -> usize {
    if pos <= MAX_VENTANA {
        return 0;
    }
    let mut i = pos - MAX_VENTANA;
    while i < pos && texto[i] != '\n' {
        i += 1;
    }
    i
}

/// Funciones declaradas o definidas en el código del alumno (`tipo nombre(params)`),
/// para mostrar sus firmas en quick-doc aunque no estén en la base.
pub fn funciones_del_codigo(codigo: &str) -> Vec<Funcion> {
    let mut salida = Vec::new();
    let mut vistos = std::collections::HashSet::new();
    for linea in codigo.lines() {
        let l = linea.trim();
        if l.starts_with('#') || l.starts_with("//") || l.starts_with('*') || l.starts_with("/*") {
            continue;
        }
        let Some(abre) = l.find('(') else { continue };
        let Some(cierra) = l.rfind(')') else { continue };
        if cierra < abre {
            continue;
        }
        let antes = l[..abre].trim_end();
        let palabras: Vec<&str> = antes.split_whitespace().collect();
        if palabras.len() < 2 {
            continue;
        }
        let nombre = palabras.last().unwrap().trim_start_matches('*');
        if nombre.is_empty()
            || !nombre.chars().all(es_ident)
            || nombre.chars().next().unwrap().is_ascii_digit()
        {
            continue;
        }
        let primero = palabras[0];
        if matches!(
            primero,
            "return" | "if" | "while" | "for" | "switch" | "else" | "case" | "do" | "sizeof"
        ) {
            continue;
        }
        if l[..abre].contains('=') {
            continue;
        }
        let resto = l[cierra + 1..].trim();
        if !(resto.is_empty() || resto == "{" || resto == ";") {
            continue;
        }
        if !vistos.insert(nombre.to_string()) {
            continue;
        }
        let params_txt = &l[abre + 1..cierra];
        let parametros = if params_txt.trim().is_empty() || params_txt.trim() == "void" {
            Vec::new()
        } else {
            params_txt
                .split(',')
                .map(|p| {
                    let p = p.trim();
                    let corte = p
                        .rfind(|c: char| c == ' ' || c == '*')
                        .map(|i| i + 1)
                        .unwrap_or(0);
                    Parametro {
                        tipo: p[..corte].trim().to_string(),
                        nombre: p[corte..].trim().to_string(),
                        desc: String::new(),
                    }
                })
                .collect()
        };
        salida.push(Funcion {
            nombre: nombre.to_string(),
            cabecera: "(tu código)".into(),
            firma: l[..=cierra].to_string(),
            descripcion: "Función declarada en el proyecto.".into(),
            retorno: String::new(),
            parametros,
            trampas: Vec::new(),
            reglas: Vec::new(),
        });
    }
    salida
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(codigo: &str) -> Contexto {
        let pos = codigo.find('|').unwrap();
        let limpio: Vec<char> = codigo.replacen('|', "", 1).chars().collect();
        contexto(&limpio, codigo[..pos].chars().count())
    }

    #[test]
    fn la_base_embebida_es_valida() {
        let b = Base::embebida();
        assert!(b.cantidad_funciones() >= 80, "{}", b.cantidad_funciones());
        let m = b.funcion("malloc").unwrap();
        assert_eq!(m.firma, "void *malloc(size_t size)");
        assert!(m.reglas.contains(&"0x3001h".to_string()));
        for f in b.funciones.values() {
            assert!(
                !f.firma.is_empty() && f.firma.contains(&f.nombre),
                "{}",
                f.nombre
            );
        }
        assert!(b.tipo("size_t").is_some());
    }

    #[test]
    fn parametro_activo_mientras_se_tipea() {
        assert_eq!(
            ctx("p = malloc(|"),
            Contexto::Llamada {
                funcion: "malloc".into(),
                parametro: 0
            }
        );
        assert_eq!(
            ctx("fgets(buf, sizeof buf, |"),
            Contexto::Llamada {
                funcion: "fgets".into(),
                parametro: 2
            }
        );
        assert_eq!(
            ctx("printf(\"%d, %d\", a|"),
            Contexto::Llamada {
                funcion: "printf".into(),
                parametro: 1
            }
        );
        assert_eq!(ctx("x = strlen(s); y = (|"), Contexto::Nada);
        assert_eq!(
            ctx("if (strcmp(a, b|"),
            Contexto::Llamada {
                funcion: "strcmp".into(),
                parametro: 1
            }
        );
        assert_eq!(
            ctx("memcpy(d, f(x, y), |"),
            Contexto::Llamada {
                funcion: "memcpy".into(),
                parametro: 2
            }
        );
        assert_eq!(
            ctx("// malloc(\n free(|"),
            Contexto::Llamada {
                funcion: "free".into(),
                parametro: 0
            }
        );
        assert_eq!(
            ctx("printf(\"(,\", |"),
            Contexto::Llamada {
                funcion: "printf".into(),
                parametro: 1
            }
        );
    }

    #[test]
    fn identificador_bajo_el_cursor() {
        assert_eq!(
            ctx("int n = str|len(s);"),
            Contexto::Identificador("strlen".into())
        );
        assert_eq!(
            ctx("printf(\"%zu\", str|len(s));"),
            Contexto::Identificador("strlen".into())
        );
        assert_eq!(
            ctx("memcpy(d, sr|c, n);"),
            Contexto::Llamada {
                funcion: "memcpy".into(),
                parametro: 1
            }
        );
        assert_eq!(ctx("FI|LE *f;"), Contexto::Identificador("FILE".into()));
        assert_eq!(ctx("x = 3|;"), Contexto::Nada);
    }

    #[test]
    fn funciones_del_alumno() {
        let codigo = "#include <stdio.h>\nint suma(int a, int b);\nstatic nodo_t *crear_nodo(int v) {\n  return NULL;\n}\nint main(void)\n{\n  if (x) { y = f(2); }\n}\n";
        let f = funciones_del_codigo(codigo);
        let nombres: Vec<&str> = f.iter().map(|f| f.nombre.as_str()).collect();
        assert_eq!(nombres, vec!["suma", "crear_nodo", "main"]);
        assert_eq!(
            f[0].parametros[1],
            Parametro {
                tipo: "int".into(),
                nombre: "b".into(),
                desc: String::new()
            }
        );
        assert_eq!(f[1].parametros[0].nombre, "v");
        assert!(f[2].parametros.is_empty());
    }
}
