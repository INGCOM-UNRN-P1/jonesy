//! «Ver la precedencia de una expresión»: la expresión de C con todos sus paréntesis
//! implícitos, los operadores que usa con su nivel, y avisos de las trampas típicas
//! (`x & 1 == 0`, `*p++`, `a = b == c`…).

#[derive(Debug, Clone, PartialEq)]
enum Expr {
    Hoja(String),
    Prefijo(String, Box<Expr>),
    Postfijo(String, Box<Expr>),
    Binario(String, Box<Expr>, Box<Expr>),
    Ternario(Box<Expr>, Box<Expr>, Box<Expr>),
    Llamada(Box<Expr>, Vec<Expr>),
    Indice(Box<Expr>, Box<Expr>),
    Miembro(Box<Expr>, String, String),
    Cast(String, Box<Expr>),
}

const OPERADORES: [&str; 43] = [
    "<<=", ">>=", "->", "++", "--", "<<", ">>", "<=", ">=", "==", "!=", "&&", "||", "+=", "-=",
    "*=", "/=", "%=", "&=", "^=", "|=", "+", "-", "*", "/", "%", "<", ">", "=", "!", "~", "&", "|",
    "^", "?", ":", ",", ".", "(", ")", "[", "]", ";",
];

const TIPOS: [&str; 14] = [
    "int", "char", "short", "long", "float", "double", "unsigned", "signed", "void", "const",
    "struct", "enum", "union", "bool",
];

fn tokenizar(s: &str) -> Result<Vec<String>, String> {
    let c: Vec<char> = s.chars().collect();
    let mut v = vec![];
    let mut i = 0;
    while i < c.len() {
        let x = c[i];
        if x.is_whitespace() {
            i += 1;
        } else if x.is_alphanumeric() || x == '_' {
            let ini = i;
            // Números con punto, exponente o sufijos; identificadores.
            while i < c.len()
                && (c[i].is_alphanumeric()
                    || c[i] == '_'
                    || (c[ini].is_ascii_digit()
                        && (c[i] == '.'
                            || ((c[i] == '+' || c[i] == '-') && matches!(c[i - 1], 'e' | 'E')))))
            {
                i += 1;
            }
            v.push(c[ini..i].iter().collect());
        } else if x == '"' || x == '\'' {
            let ini = i;
            i += 1;
            while i < c.len() && c[i] != x {
                if c[i] == '\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
            v.push(c[ini..i.min(c.len())].iter().collect());
        } else {
            let resto: String = c[i..].iter().take(3).collect();
            match OPERADORES.iter().find(|o| resto.starts_with(*o)) {
                Some(o) => {
                    v.push(o.to_string());
                    i += o.chars().count();
                }
                None => return Err(format!("no entiendo el símbolo «{x}»")),
            }
        }
    }
    Ok(v)
}

/// (fuerza a la izquierda, fuerza a la derecha) de un operador binario: más alto, más
/// ligado. Asociatividad a derecha: la derecha es menor.
fn fuerza(op: &str) -> Option<(u8, u8)> {
    Some(match op {
        "," => (1, 2),
        "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "^=" | "|=" | "<<=" | ">>=" => (4, 3),
        "?" => (6, 5),
        "||" => (7, 8),
        "&&" => (9, 10),
        "|" => (11, 12),
        "^" => (13, 14),
        "&" => (15, 16),
        "==" | "!=" => (17, 18),
        "<" | "<=" | ">" | ">=" => (19, 20),
        "<<" | ">>" => (21, 22),
        "+" | "-" => (23, 24),
        "*" | "/" | "%" => (25, 26),
        _ => return None,
    })
}

const PREFIJO: u8 = 27;

struct Parser {
    t: Vec<String>,
    i: usize,
}

impl Parser {
    fn ver(&self) -> Option<&str> {
        self.t.get(self.i).map(String::as_str)
    }

    fn tomar(&mut self) -> Option<String> {
        let x = self.t.get(self.i).cloned();
        self.i += 1;
        x
    }

    fn esperar(&mut self, s: &str) -> Result<(), String> {
        match self.tomar() {
            Some(x) if x == s => Ok(()),
            Some(x) => Err(format!("esperaba «{s}» y encontré «{x}»")),
            None => Err(format!("falta «{s}»")),
        }
    }

    fn es_tipo(&self, j: usize) -> bool {
        self.t.get(j).is_some_and(|x| {
            TIPOS.contains(&x.as_str())
                || (x.ends_with("_t") && x.chars().all(|c| c.is_alphanumeric() || c == '_'))
        })
    }

    fn expr(&mut self, minimo: u8) -> Result<Expr, String> {
        let mut izq = self.prefijo()?;
        while let Some(op) = self.ver().map(str::to_string) {
            let Some((fi, fd)) = fuerza(&op) else { break };
            if fi < minimo {
                break;
            }
            self.i += 1;
            if op == "?" {
                let a = self.expr(0)?;
                self.esperar(":")?;
                let b = self.expr(fd)?;
                izq = Expr::Ternario(Box::new(izq), Box::new(a), Box::new(b));
            } else {
                let der = self.expr(fd)?;
                izq = Expr::Binario(op, Box::new(izq), Box::new(der));
            }
        }
        Ok(izq)
    }

    fn prefijo(&mut self) -> Result<Expr, String> {
        let Some(x) = self.ver().map(str::to_string) else {
            return Err("la expresión está incompleta".into());
        };
        // Cast: (tipo) expr
        if x == "(" && self.es_tipo(self.i + 1) {
            let mut j = self.i + 1;
            let mut tipo = vec![];
            while j < self.t.len() && self.t[j] != ")" {
                tipo.push(self.t[j].clone());
                j += 1;
            }
            self.i = j + 1;
            let e = self.expr(PREFIJO)?;
            return Ok(Expr::Cast(tipo.join(" ").replace(" *", "*"), Box::new(e)));
        }
        if matches!(
            x.as_str(),
            "!" | "~" | "-" | "+" | "*" | "&" | "++" | "--" | "sizeof"
        ) {
            self.i += 1;
            if x == "sizeof" && self.ver() == Some("(") && self.es_tipo(self.i + 1) {
                let mut tipo = vec![];
                self.i += 1;
                while let Some(y) = self.tomar() {
                    if y == ")" {
                        break;
                    }
                    tipo.push(y);
                }
                return Ok(Expr::Hoja(format!("sizeof({})", tipo.join(" "))));
            }
            let e = self.expr(PREFIJO)?;
            return Ok(Expr::Prefijo(x, Box::new(e)));
        }
        let mut e = if x == "(" {
            self.i += 1;
            let e = self.expr(0)?;
            self.esperar(")")?;
            e
        } else {
            let es_valor = x
                .chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '"' || c == '\'');
            if !es_valor {
                return Err(format!("no esperaba «{x}» acá"));
            }
            self.i += 1;
            Expr::Hoja(x)
        };
        // Postfijos: llamada, índice, miembro, ++ y --.
        loop {
            match self.ver() {
                Some("(") => {
                    self.i += 1;
                    let mut args = vec![];
                    if self.ver() != Some(")") {
                        loop {
                            args.push(self.expr(2)?);
                            if self.ver() == Some(",") {
                                self.i += 1;
                            } else {
                                break;
                            }
                        }
                    }
                    self.esperar(")")?;
                    e = Expr::Llamada(Box::new(e), args);
                }
                Some("[") => {
                    self.i += 1;
                    let i = self.expr(0)?;
                    self.esperar("]")?;
                    e = Expr::Indice(Box::new(e), Box::new(i));
                }
                Some(op @ ("." | "->")) => {
                    let op = op.to_string();
                    self.i += 1;
                    let campo = self.tomar().ok_or("falta el nombre del campo")?;
                    e = Expr::Miembro(Box::new(e), op, campo);
                }
                Some(op @ ("++" | "--")) => {
                    let op = op.to_string();
                    self.i += 1;
                    e = Expr::Postfijo(op, Box::new(e));
                }
                _ => break,
            }
        }
        Ok(e)
    }
}

fn mostrar(e: &Expr, raiz: bool) -> String {
    let envolver = |s: String| if raiz { s } else { format!("({s})") };
    match e {
        Expr::Hoja(s) => s.clone(),
        Expr::Prefijo(op, x) if op == "sizeof" => envolver(format!("sizeof {}", mostrar(x, false))),
        Expr::Prefijo(op, x) => envolver(format!("{op}{}", mostrar(x, false))),
        Expr::Postfijo(op, x) => envolver(format!("{}{op}", mostrar(x, false))),
        Expr::Binario(op, a, b) if op == "," => {
            envolver(format!("{}, {}", mostrar(a, false), mostrar(b, false)))
        }
        Expr::Binario(op, a, b) => {
            envolver(format!("{} {op} {}", mostrar(a, false), mostrar(b, false)))
        }
        Expr::Ternario(c, a, b) => envolver(format!(
            "{} ? {} : {}",
            mostrar(c, false),
            mostrar(a, false),
            mostrar(b, false)
        )),
        Expr::Llamada(f, args) => format!(
            "{}({})",
            mostrar(f, false),
            args.iter()
                .map(|a| mostrar(a, true))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::Indice(x, i) => format!("{}[{}]", mostrar(x, false), mostrar(i, true)),
        Expr::Miembro(x, op, c) => format!("{}{op}{c}", mostrar(x, false)),
        Expr::Cast(t, x) => envolver(format!("({t}) {}", mostrar(x, false))),
    }
}

/// Qué hace un operador y su nivel en la tabla de precedencia (1 = el más ligado).
fn describir(op: &str, prefijo: bool) -> (&'static str, u8) {
    match (op, prefijo) {
        ("++", true) => ("incrementa y después usa el valor", 2),
        ("--", true) => ("decrementa y después usa el valor", 2),
        ("++", false) => ("usa el valor y después incrementa", 1),
        ("--", false) => ("usa el valor y después decrementa", 1),
        ("*", true) => ("lo apuntado por (desreferencia)", 2),
        ("&", true) => ("la dirección de", 2),
        ("-", true) => ("cambio de signo", 2),
        ("+", true) => ("signo positivo", 2),
        ("!", true) => ("negación lógica", 2),
        ("~", true) => ("negación de bits", 2),
        ("sizeof", true) => ("tamaño en bytes", 2),
        ("*", false) => ("multiplicación", 3),
        ("/", false) => ("división", 3),
        ("%", false) => ("resto", 3),
        ("+", false) => ("suma", 4),
        ("-", false) => ("resta", 4),
        ("<<", _) | (">>", _) => ("desplazamiento de bits", 5),
        ("<", _) | ("<=", _) | (">", _) | (">=", _) => ("comparación", 6),
        ("==", _) | ("!=", _) => ("igualdad", 7),
        ("&", false) => ("Y de bits", 8),
        ("^", _) => ("O exclusivo de bits", 9),
        ("|", _) => ("O de bits", 10),
        ("&&", _) => ("Y lógico", 11),
        ("||", _) => ("O lógico", 12),
        ("?", _) => ("condicional", 13),
        (",", _) => ("coma (evalúa ambos, vale el segundo)", 15),
        _ => ("asignación", 14),
    }
}

fn operadores(e: &Expr, v: &mut Vec<(String, bool)>) {
    match e {
        Expr::Hoja(_) => {}
        Expr::Prefijo(op, x) => {
            v.push((op.clone(), true));
            operadores(x, v);
        }
        Expr::Postfijo(op, x) => {
            v.push((op.clone(), false));
            operadores(x, v);
        }
        Expr::Binario(op, a, b) => {
            v.push((op.clone(), false));
            operadores(a, v);
            operadores(b, v);
        }
        Expr::Ternario(c, a, b) => {
            v.push(("?".into(), false));
            operadores(c, v);
            operadores(a, v);
            operadores(b, v);
        }
        Expr::Llamada(f, args) => {
            operadores(f, v);
            args.iter().for_each(|a| operadores(a, v));
        }
        Expr::Indice(x, i) => {
            operadores(x, v);
            operadores(i, v);
        }
        Expr::Miembro(x, _, _) | Expr::Cast(_, x) => operadores(x, v),
    }
}

fn es_comparacion(e: &Expr) -> bool {
    matches!(e, Expr::Binario(op, _, _) if matches!(op.as_str(), "==" | "!=" | "<" | "<=" | ">" | ">="))
}

/// Trampas de precedencia frecuentes en la expresión.
fn trampas(e: &Expr, v: &mut Vec<String>) {
    match e {
        Expr::Binario(op, a, b) => {
            if matches!(op.as_str(), "&" | "|" | "^") && (es_comparacion(a) || es_comparacion(b)) {
                v.push(format!(
                    "Ojo: las comparaciones van antes que `{op}`: `x {op} 1 == 0` es `x {op} (1 == 0)`. Usá paréntesis: `(x {op} 1) == 0`."
                ));
            }
            if matches!(op.as_str(), "<<" | ">>")
                && [a, b]
                    .iter()
                    .any(|x| matches!(x.as_ref(), Expr::Binario(o, _, _) if o == "+" || o == "-"))
            {
                v.push(format!(
                    "Ojo: `+` y `-` van antes que `{op}`: `1 {op} n + 1` es `1 {op} (n + 1)`."
                ));
            }
            if op == "=" && es_comparacion(b) {
                v.push("Ojo: `a = b == c` guarda en `a` el resultado de la comparación (0 o 1), no `b`.".into());
            }
            trampas(a, v);
            trampas(b, v);
        }
        Expr::Prefijo(op, x) => {
            if op == "*" && matches!(x.as_ref(), Expr::Postfijo(o, _) if o == "++" || o == "--") {
                v.push("Ojo: `*p++` incrementa el puntero (lee y avanza), no el valor apuntado; para el valor: `(*p)++`.".into());
            }
            if op == "!" && matches!(x.as_ref(), Expr::Hoja(_)) {
                // !a == b se arma más arriba: se avisa desde el binario.
            }
            trampas(x, v);
        }
        Expr::Postfijo(_, x) | Expr::Miembro(x, _, _) | Expr::Cast(_, x) => trampas(x, v),
        Expr::Ternario(c, a, b) => {
            trampas(c, v);
            trampas(a, v);
            trampas(b, v);
        }
        Expr::Llamada(f, args) => {
            trampas(f, v);
            args.iter().for_each(|a| trampas(a, v));
        }
        Expr::Indice(x, i) => {
            trampas(x, v);
            trampas(i, v);
        }
        Expr::Hoja(_) => {}
    }
    if let Expr::Binario(op, a, _) = e {
        if (op == "==" || op == "!=") && matches!(a.as_ref(), Expr::Prefijo(o, _) if o == "!") {
            v.push(
                "Ojo: `!a == b` es `(!a) == b`; para negar la comparación: `!(a == b)` o `a != b`."
                    .into(),
            );
        }
    }
}

/// La expresión a analizar a partir de una línea de código: sin `return`, `if (…)`,
/// `while (…)`, el `;` final ni la parte declarativa (`int x = …`).
pub fn de_linea(linea: &str) -> String {
    let mut s = linea.trim().to_string();
    if let Some(i) = s.find("//") {
        s.truncate(i);
    }
    let mut s = s.trim().trim_end_matches(['{', ';']).trim().to_string();
    for clave in ["return ", "else if", "if", "while", "switch"] {
        if let Some(resto) = s.strip_prefix(clave) {
            let r = resto.trim();
            if clave == "return " {
                s = r.to_string();
            } else if r.starts_with('(') && r.ends_with(')') {
                s = r[1..r.len() - 1].to_string();
            }
            break;
        }
    }
    // Declaración: lo que está después del primer «=» (que no sea «==»).
    let palabras: Vec<&str> = s.split_whitespace().collect();
    if palabras
        .first()
        .is_some_and(|p| TIPOS.contains(p) || p.ends_with("_t"))
    {
        if let Some(i) = s.find('=').filter(|&i| !s[i..].starts_with("==")) {
            s = s[i + 1..].trim().to_string();
        }
    }
    s
}

/// Resultado del análisis.
#[derive(Debug, Clone, PartialEq)]
pub struct Analisis {
    pub original: String,
    pub con_parentesis: String,
    /// (operador, qué hace, nivel), del más ligado al menos.
    pub operadores: Vec<(String, &'static str, u8)>,
    pub avisos: Vec<String>,
}

pub fn analizar(expresion: &str) -> Result<Analisis, String> {
    let t = tokenizar(expresion.trim().trim_end_matches(';'))?;
    if t.is_empty() {
        return Err("no hay una expresión".into());
    }
    let mut p = Parser { t, i: 0 };
    let e = p.expr(0)?;
    if let Some(x) = p.ver() {
        return Err(format!("sobra «{x}»"));
    }
    let mut ops = vec![];
    operadores(&e, &mut ops);
    let mut operadores: Vec<(String, &'static str, u8)> = ops
        .into_iter()
        .map(|(o, pre)| {
            let (d, n) = describir(&o, pre);
            (if o == "?" { "? :".to_string() } else { o }, d, n)
        })
        .collect();
    operadores.sort_by_key(|x| x.2);
    operadores.dedup();
    let mut avisos = vec![];
    trampas(&e, &mut avisos);
    avisos.dedup();
    Ok(Analisis {
        original: expresion.trim().to_string(),
        con_parentesis: mostrar(&e, true),
        operadores,
        avisos,
    })
}

/// El análisis en Markdown para mostrar en una ventana.
pub fn markdown(a: &Analisis) -> String {
    let mut s = format!(
        "# Precedencia\n\n```c\n{}\n```\n\nCon todos sus paréntesis:\n\n```c\n{}\n```\n",
        a.original, a.con_parentesis
    );
    if !a.operadores.is_empty() {
        s.push_str("\n| Operador | Qué hace | Nivel |\n|---|---|---|\n");
        for (o, d, n) in &a.operadores {
            s.push_str(&format!("| `{o}` | {d} | {n} |\n"));
        }
        s.push_str("\nEl nivel más bajo se aplica primero (Ayuda → Tablas de consulta tiene la tabla completa).\n");
    }
    for w in &a.avisos {
        s.push_str(&format!("\n**{w}**\n"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(e: &str) -> String {
        analizar(e).unwrap().con_parentesis
    }

    #[test]
    fn agrega_los_parentesis_implicitos() {
        assert_eq!(p("a + b * c"), "a + (b * c)");
        assert_eq!(p("a - b - c"), "(a - b) - c");
        assert_eq!(p("a = b = c"), "a = (b = c)");
        assert_eq!(p("x & 1 == 0"), "x & (1 == 0)");
        assert_eq!(p("*p++"), "*(p++)");
        assert_eq!(p("a < b && c || !d"), "((a < b) && c) || (!d)");
        assert_eq!(p("f(a + 1, v[i * 2]) * 2"), "f(a + 1, v[i * 2]) * 2");
        assert_eq!(p("p->sig->valor + 1"), "p->sig->valor + 1");
        assert_eq!(p("(double) suma / n"), "((double) suma) / n");
        assert_eq!(p("x > 0 ? x : -x"), "(x > 0) ? x : (-x)");
        assert_eq!(p("n * sizeof(int)"), "n * sizeof(int)");
        assert_eq!(p("1e-3 + 0x1F"), "1e-3 + 0x1F");
    }

    #[test]
    fn avisa_las_trampas_y_lee_lineas() {
        let a = analizar("x & 1 == 0").unwrap();
        assert!(a.avisos[0].contains("comparaciones van antes"));
        assert!(analizar("*p++").unwrap().avisos[0].contains("incrementa el puntero"));
        assert!(analizar("a = b == c")
            .unwrap()
            .avisos
            .iter()
            .any(|w| w.contains("0 o 1")));
        assert!(analizar("!a == b")
            .unwrap()
            .avisos
            .iter()
            .any(|w| w.contains("!(a == b)")));
        assert_eq!(de_linea("    if (x & 1 == 0) {"), "x & 1 == 0");
        assert_eq!(de_linea("return a + b * c;  // suma"), "a + b * c");
        assert_eq!(de_linea("int total = a + b;"), "a + b");
        assert!(analizar("a + * ").is_err());
        let m = markdown(&analizar("a + b * c").unwrap());
        assert!(m.contains("| `*` | multiplicación | 3 |"));
    }
}
