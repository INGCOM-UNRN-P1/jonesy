//! Renombrar una variable o función dentro de su ámbito (Editar → Renombrar, Ctrl+R).
//!
//! Sobre el texto sin comentarios ni cadenas se arman los bloques `{ }` y se reconocen las
//! declaraciones (un nombre precedido por un tipo y seguido de `=`, `;`, `,`, `[` o `)`).
//! Cada aparición del nombre se asocia a la declaración que ve (la más interna que la
//! contiene), así se respetan las variables tapadas por otra del mismo nombre en un bloque
//! interno, los parámetros (sólo en su función) y las variables de un `for`. Lo que no
//! tiene declaración local (funciones, globales, macros, tipos) se renombra en todo el
//! archivo. Los campos de un struct (`p.x`, `q->x`) no se tocan.

use crate::simbolos::limpio;

const CLAVES: &[&str] = &[
    "auto", "break", "case", "char", "const", "continue", "default", "do", "double", "else",
    "enum", "extern", "float", "for", "goto", "if", "inline", "int", "long", "register",
    "restrict", "return", "short", "signed", "sizeof", "static", "struct", "switch", "typedef",
    "union", "unsigned", "void", "volatile", "while", "bool", "true", "false", "NULL",
];

/// Lo que se va a cambiar.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub nombre: String,
    /// Dónde vale: «la función main», «el bloque de la línea 7», «todo el archivo».
    pub alcance: String,
    /// Rangos (inicio, fin) en caracteres de cada aparición.
    pub rangos: Vec<(usize, usize)>,
}

fn es_id(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Un bloque: llaves o el ámbito de los parámetros/variables de `for`.
#[derive(Debug, Clone, Copy)]
struct Bloque {
    ini: usize,
    fin: usize,
}

fn bloques(t: &[char]) -> Vec<Bloque> {
    let mut v = vec![];
    let mut pila = vec![];
    for (i, c) in t.iter().enumerate() {
        match c {
            '{' => pila.push(i),
            '}' => {
                if let Some(a) = pila.pop() {
                    v.push(Bloque { ini: a, fin: i });
                }
            }
            _ => {}
        }
    }
    // Lo que quedó abierto llega hasta el final.
    for a in pila {
        v.push(Bloque {
            ini: a,
            fin: t.len(),
        });
    }
    v
}

/// Siguiente / anterior carácter que no es blanco.
fn siguiente(t: &[char], mut i: usize) -> Option<(usize, char)> {
    while i < t.len() {
        if !t[i].is_whitespace() {
            return Some((i, t[i]));
        }
        i += 1;
    }
    None
}

fn anterior(t: &[char], i: usize) -> Option<(usize, char)> {
    (0..i)
        .rev()
        .find(|&k| !t[k].is_whitespace())
        .map(|k| (k, t[k]))
}

fn palabra_antes(t: &[char], i: usize) -> String {
    let mut k = i;
    while k > 0 && t[k - 1].is_whitespace() {
        k -= 1;
    }
    let fin = k;
    while k > 0 && es_id(t[k - 1]) {
        k -= 1;
    }
    t[k..fin].iter().collect()
}

/// ¿La aparición en `i..i+n` es un campo (`.x`, `->x`)?
fn es_campo(t: &[char], i: usize) -> bool {
    match anterior(t, i) {
        Some((_, '.')) => true,
        Some((k, '>')) => k > 0 && t[k - 1] == '-',
        _ => false,
    }
}

/// ¿Es una declaración? Un tipo (o `*`, o `,` dentro de una declaración) antes y `= ; , [ )`
/// después.
fn es_declaracion(t: &[char], i: usize, n: usize) -> bool {
    let despues = siguiente(t, i + n).map(|x| x.1);
    if !matches!(despues, Some('=' | ';' | ',' | '[' | ')')) {
        return false;
    }
    match anterior(t, i) {
        Some((_, '*')) => {
            // `int *p` sí; `a * b;` no: antes del * tiene que haber un tipo.
            let mut k = i;
            while k > 0 && (t[k - 1].is_whitespace() || t[k - 1] == '*') {
                k -= 1;
            }
            let p = palabra_antes(t, k);
            !p.is_empty() && !matches!(p.as_str(), "return" | "sizeof")
        }
        Some((_, c)) if es_id(c) => {
            let p = palabra_antes(t, i);
            !matches!(p.as_str(), "return" | "sizeof" | "case" | "else" | "goto")
                && !p.chars().next().is_some_and(|c| c.is_ascii_digit())
        }
        _ => false,
    }
}

/// El ámbito de una declaración en `pos`: su bloque `{ }` más interno o, si está entre
/// paréntesis (parámetro o variable de `for`), el cuerpo que sigue al `)`.
fn ambito(t: &[char], bl: &[Bloque], pos: usize) -> Option<Bloque> {
    // ¿Dentro de paréntesis? Se busca el '(' que lo contiene.
    let mut nivel = 0i32;
    let mut k = pos;
    let mut abre = None;
    while k > 0 {
        k -= 1;
        match t[k] {
            ')' => nivel += 1,
            '(' if nivel == 0 => {
                abre = Some(k);
                break;
            }
            '(' => nivel -= 1,
            '{' | '}' | ';' if nivel == 0 => break,
            _ => {}
        }
    }
    if let Some(a) = abre {
        // Su ')' y lo que sigue: '{' (cuerpo) o una sola sentencia.
        let mut nivel = 0i32;
        let mut cierra = None;
        for (j, c) in t.iter().enumerate().skip(a) {
            match c {
                '(' => nivel += 1,
                ')' => {
                    nivel -= 1;
                    if nivel == 0 {
                        cierra = Some(j);
                        break;
                    }
                }
                _ => {}
            }
        }
        let c = cierra?;
        return match siguiente(t, c + 1) {
            Some((j, '{')) => bl
                .iter()
                .find(|b| b.ini == j)
                .map(|b| Bloque { ini: a, fin: b.fin }),
            Some((_, ';')) => Some(Bloque { ini: a, fin: c }),
            Some((j, _)) => {
                let fin = (j..t.len()).find(|&x| t[x] == ';').unwrap_or(t.len());
                Some(Bloque { ini: a, fin })
            }
            None => Some(Bloque { ini: a, fin: c }),
        };
    }
    bl.iter()
        .filter(|b| b.ini < pos && pos < b.fin)
        .min_by_key(|b| b.fin - b.ini)
        .copied()
}

/// Qué cambiaría renombrar lo que está en `pos`.
pub fn planear(fuente: &str, pos: usize) -> Result<Plan, String> {
    let orig: Vec<char> = fuente.chars().collect();
    let t = limpio(&orig);
    if pos > t.len() {
        return Err("el cursor está fuera del texto".into());
    }
    // La palabra bajo (o justo antes de) el cursor.
    let mut ini = pos.min(t.len());
    if ini == t.len() || !es_id(t[ini]) {
        if ini > 0 && es_id(t[ini - 1]) {
            ini -= 1;
        } else {
            return Err(
                "poné el cursor sobre un nombre (en el código, no en un comentario)".into(),
            );
        }
    }
    while ini > 0 && es_id(t[ini - 1]) {
        ini -= 1;
    }
    let mut fin = ini;
    while fin < t.len() && es_id(t[fin]) {
        fin += 1;
    }
    let nombre: String = t[ini..fin].iter().collect();
    if nombre.chars().next().is_some_and(|c| c.is_ascii_digit())
        || CLAVES.contains(&nombre.as_str())
    {
        return Err(format!("«{nombre}» no es un nombre que se pueda renombrar"));
    }
    if es_campo(&t, ini) {
        return Err(format!(
            "«{nombre}» es un campo de un struct: se renombra en su definición"
        ));
    }
    let n = nombre.chars().count();
    let bl = bloques(&t);
    // Todas las apariciones (no campos) y las declaraciones con su ámbito.
    let mut apariciones = vec![];
    let mut i = 0;
    while i + n <= t.len() {
        if t[i..i + n].iter().copied().eq(nombre.chars())
            && (i == 0 || !es_id(t[i - 1]))
            && t.get(i + n).is_none_or(|c| !es_id(*c))
            && !es_campo(&t, i)
        {
            apariciones.push(i);
            i += n;
        } else {
            i += 1;
        }
    }
    let declaraciones: Vec<(usize, Bloque)> = apariciones
        .iter()
        .filter(|&&p| es_declaracion(&t, p, n))
        .filter_map(|&p| ambito(&t, &bl, p).map(|b| (p, b)))
        .collect();
    // La declaración que ve una posición: la más interna que la contiene y está antes.
    let visible = |q: usize| -> Option<usize> {
        declaraciones
            .iter()
            .enumerate()
            .filter(|(_, (p, b))| *p <= q && b.ini <= q && q <= b.fin)
            .min_by_key(|(_, (_, b))| b.fin - b.ini)
            .map(|(k, _)| k)
    };
    let objetivo = visible(ini);
    let rangos: Vec<(usize, usize)> = apariciones
        .into_iter()
        .filter(|&q| visible(q) == objetivo)
        .map(|q| (q, q + n))
        .collect();
    let alcance = match objetivo {
        None => "todo el archivo".to_string(),
        Some(k) => {
            let (_, b) = declaraciones[k];
            let linea = orig[..b.ini].iter().filter(|c| **c == '\n').count() + 1;
            let funcion = crate::simbolos::de_texto(fuente)
                .into_iter()
                .filter(|s| s.tipo == crate::simbolos::Tipo::Funcion)
                .rfind(|s| {
                    let inicio: usize = orig
                        .iter()
                        .enumerate()
                        .filter(|(_, c)| **c == '\n')
                        .nth(s.linea.wrapping_sub(1))
                        .map(|(x, _)| x)
                        .unwrap_or(0);
                    inicio <= b.ini
                });
            match funcion {
                Some(f)
                    if t[b.ini] == '('
                        || orig[..b.ini].iter().filter(|c| **c == '\n').count() <= f.linea + 1 =>
                {
                    format!("la función {}", f.nombre)
                }
                _ => format!("el bloque de la línea {linea}"),
            }
        }
    };
    Ok(Plan {
        nombre,
        alcance,
        rangos,
    })
}

/// Valida un nombre nuevo para el plan: identificador de C, no palabra clave, y que no
/// choque con otro nombre que ya se use en el texto.
pub fn validar(fuente: &str, plan: &Plan, nuevo: &str) -> Result<(), String> {
    let ok = nuevo
        .chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_')
        && nuevo.chars().all(es_id);
    if !ok {
        return Err("un nombre empieza con letra o _ y sigue con letras, dígitos o _".into());
    }
    if CLAVES.contains(&nuevo) {
        return Err(format!("«{nuevo}» es una palabra reservada de C"));
    }
    if nuevo == plan.nombre {
        return Err("es el mismo nombre".into());
    }
    let t: Vec<char> = limpio(&fuente.chars().collect::<Vec<_>>());
    let n = nuevo.chars().count();
    let ya = (0..t.len().saturating_sub(n - 1)).any(|i| {
        t[i..i + n].iter().copied().eq(nuevo.chars())
            && (i == 0 || !es_id(t[i - 1]))
            && t.get(i + n).is_none_or(|c| !es_id(*c))
    });
    if ya {
        return Err(format!(
            "«{nuevo}» ya se usa en este archivo: elegí otro para no confundirlos"
        ));
    }
    Ok(())
}

/// El texto con el plan aplicado.
pub fn aplicar(fuente: &str, plan: &Plan, nuevo: &str) -> String {
    let c: Vec<char> = fuente.chars().collect();
    let mut s = String::with_capacity(fuente.len());
    let mut ult = 0;
    for (a, b) in &plan.rangos {
        s.extend(&c[ult..*a]);
        s.push_str(nuevo);
        ult = *b;
    }
    s.extend(&c[ult..]);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const CODIGO: &str = r#"int total = 0;  /* total global */
int sumar(int a, int b)
{
    int total = a + b;
    {
        int total = 1;
        total++;
    }
    return total;
}

int main(void)
{
    struct p { int total; } x;
    x.total = sumar(1, 2);
    for (int a = 0; a < 3; a++) {
        total += a;
    }
    printf("total %d", total);
    return sumar(total, 1);
}
"#;

    fn en(texto: &str, linea: usize, col: usize) -> usize {
        texto
            .lines()
            .take(linea)
            .map(|l| l.chars().count() + 1)
            .sum::<usize>()
            + col
    }

    fn lineas(plan: &Plan) -> Vec<usize> {
        plan.rangos
            .iter()
            .map(|(a, _)| CODIGO.chars().take(*a).filter(|c| *c == '\n').count())
            .collect()
    }

    #[test]
    fn local_tapada_y_bloques() {
        // «total» de sumar (línea 3): no toca la global, ni la del bloque interno.
        let p = planear(CODIGO, en(CODIGO, 3, 9)).unwrap();
        assert_eq!(lineas(&p), vec![3, 8]);
        assert_eq!(p.alcance, "la función sumar");
        // La del bloque interno: sólo ahí.
        let p = planear(CODIGO, en(CODIGO, 6, 9)).unwrap();
        assert_eq!(lineas(&p), vec![5, 6]);
        // La global: su declaración y los usos de main, no los campos ni la cadena.
        let p = planear(CODIGO, en(CODIGO, 0, 5)).unwrap();
        assert_eq!(lineas(&p), vec![0, 16, 18, 19]);
        assert_eq!(p.alcance, "todo el archivo");
    }

    #[test]
    fn parametros_for_funciones_y_campos() {
        // El parámetro a de sumar: sólo en sumar, no la a del for de main.
        let p = planear(CODIGO, en(CODIGO, 1, 14)).unwrap();
        assert_eq!(lineas(&p), vec![1, 3]);
        // La a del for: su declaración, la condición, el incremento y el cuerpo.
        let p = planear(CODIGO, en(CODIGO, 15, 13)).unwrap();
        assert_eq!(p.rangos.len(), 4);
        // Una función: definición y llamadas.
        let p = planear(CODIGO, en(CODIGO, 1, 5)).unwrap();
        assert_eq!(lineas(&p), vec![1, 14, 19]);
        let nuevo = aplicar(CODIGO, &p, "sumar_dos");
        assert!(
            nuevo.contains("int sumar_dos(int a") && nuevo.contains("return sumar_dos(total, 1)")
        );
        // Un campo no se renombra desde un uso.
        assert!(planear(CODIGO, en(CODIGO, 14, 6)).is_err());
        // En un comentario, no.
        assert!(planear(CODIGO, en(CODIGO, 0, 20)).is_err());
    }

    #[test]
    fn valida_el_nombre_nuevo() {
        let p = planear(CODIGO, en(CODIGO, 3, 9)).unwrap();
        assert!(validar(CODIGO, &p, "suma").is_ok());
        assert!(validar(CODIGO, &p, "2x").is_err());
        assert!(validar(CODIGO, &p, "while").is_err());
        assert!(validar(CODIGO, &p, "sumar")
            .unwrap_err()
            .contains("ya se usa"));
    }
}
