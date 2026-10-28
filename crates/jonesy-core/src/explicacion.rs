//! La salida de `<herramienta> explain <código>` (paneles de rich, viñetas, bloques
//! ```c```) pasada a una [`Regla`], para que F1 la muestre con el mismo formato que
//! los errores de gcc y las reglas del catálogo: título, explicación, qué hacer,
//! «Así no» / «Así sí».

use crate::catalogo::Regla;

/// Qué se está juntando mientras se recorre la salida.
#[derive(Clone, Copy, PartialEq)]
enum Seccion {
    Descripcion,
    Sugerencia,
    Mal,
    Bien,
    Nada,
}

/// Interpreta la salida de `explain`; `None` si no se reconoce nada útil (la TUI
/// muestra entonces la salida tal cual).
pub fn desde_salida(herramienta: &str, codigo: &str, salida: &str) -> Option<Regla> {
    let mut r = Regla {
        herramienta: herramienta.to_string(),
        codigo: codigo.to_string(),
        ..Default::default()
    };
    let mut descripcion: Vec<String> = vec![];
    let mut sugerencia: Vec<String> = vec![];
    let mut mal: Vec<String> = vec![];
    let mut bien: Vec<String> = vec![];
    let mut seccion = Seccion::Descripcion;
    // El primer renglón de un panel con título «Regla X» es el título de la regla.
    let mut titulo_en_el_panel = false;
    let mut en_bloque = false;
    // Paneles de código de rich (`Syntax` con números de línea).
    let mut panel_de_codigo = false;
    for cruda in sin_ansi(salida).lines() {
        let (linea, marco) = sin_marco(cruda);
        if let Some(t) = marco {
            // Encabezado de un panel: «╭── ❌ Código Incorrecto ──╮».
            panel_de_codigo = false;
            en_bloque = false;
            if t.is_empty() {
                continue;
            }
            match encabezado(&t) {
                Some(s @ (Seccion::Mal | Seccion::Bien)) => {
                    seccion = s;
                    panel_de_codigo = true;
                }
                Some(s) => seccion = s,
                None => {
                    titulo_en_el_panel = r.titulo.is_empty();
                    seccion = Seccion::Descripcion;
                }
            }
            continue;
        }
        let t = linea.trim();
        if t.starts_with("```") {
            en_bloque = !en_bloque;
            continue;
        }
        if en_bloque || panel_de_codigo {
            let codigo = if panel_de_codigo {
                sin_numero_de_linea(&linea)
            } else {
                linea.clone()
            };
            match seccion {
                Seccion::Mal => mal.push(codigo),
                Seccion::Bien => bien.push(codigo),
                _ => descripcion.push(format!("    {codigo}")),
            }
            continue;
        }
        if t.is_empty() {
            // Un renglón en blanco cierra la sugerencia y los ejemplos sin bloque.
            match seccion {
                Seccion::Sugerencia | Seccion::Mal | Seccion::Bien => {
                    seccion = Seccion::Descripcion
                }
                Seccion::Descripcion => descripcion.push(String::new()),
                Seccion::Nada => {}
            }
            continue;
        }
        if titulo_en_el_panel {
            r.titulo = t.to_string();
            titulo_en_el_panel = false;
            continue;
        }
        // «📖 Regla P1 0x3001h: Siempre verificá…» (ripley).
        if r.titulo.is_empty() {
            if let Some((_, titulo)) = t
                .split_once(codigo)
                .and_then(|(_, resto)| resto.split_once(':'))
            {
                if !titulo.trim().is_empty() && t.find(codigo) < t.find(':') {
                    r.titulo = titulo.trim().to_string();
                    continue;
                }
            }
        }
        let vineta = t.trim_start_matches(['•', '-', '*']).trim();
        if let Some((clave, valor)) = vineta.split_once(':') {
            let valor = valor.trim();
            match clave.trim().to_lowercase().as_str() {
                "categoría" | "categoria" => {
                    r.categoria = valor.to_string();
                    continue;
                }
                "severidad" => {
                    r.extra.push(("Severidad".into(), valor.to_string()));
                    continue;
                }
                "descripción" | "descripcion" => {
                    descripcion.push(valor.to_string());
                    seccion = Seccion::Descripcion;
                    continue;
                }
                // «CWE-242: https://…», «Regla 0x5008h del apunte: https://…».
                c if valor.starts_with("http") && c.chars().count() < 40 => {
                    r.extra.push((clave.trim().to_string(), valor.to_string()));
                    continue;
                }
                _ => {}
            }
        }
        // «🔍 Explicación didáctica:» anuncia; «La explicación es…» es texto.
        if let Some(s) = encabezado(t).filter(|s| *s != Seccion::Descripcion || t.ends_with(':')) {
            seccion = s;
            // «Cómo corregirlo: Reemplazá…» trae el texto en el mismo renglón.
            if let Some((_, resto)) = t.split_once(':') {
                let resto = resto.trim();
                if !resto.is_empty() && s == Seccion::Sugerencia {
                    sugerencia.push(resto.to_string());
                }
            }
            continue;
        }
        match seccion {
            Seccion::Descripcion => descripcion.push(t.to_string()),
            Seccion::Sugerencia => sugerencia.push(t.to_string()),
            Seccion::Mal => mal.push(linea.trim_end().to_string()),
            Seccion::Bien => bien.push(linea.trim_end().to_string()),
            Seccion::Nada => {}
        }
    }
    r.descripcion = parrafos(&descripcion);
    r.sugerencia = sugerencia.join(" ");
    r.mal = ejemplo(&mal);
    r.bien = ejemplo(&bien);
    if r.titulo.is_empty() && r.descripcion.is_empty() {
        return None;
    }
    Some(r)
}

/// El tipo de sección que anuncia un renglón o el título de un panel.
fn encabezado(t: &str) -> Option<Seccion> {
    let l = t.to_lowercase();
    // Renglones que anuncian algo aunque sean largos («Cómo corregirlo: …»).
    if l.contains(":ignore") || l.starts_with("capacidad de autofix") {
        return Some(Seccion::Nada);
    }
    if l.starts_with("cómo corregir") || l.starts_with("qué hacer") {
        return Some(Seccion::Sugerencia);
    }
    if l.chars().count() >= 60 {
        return None;
    }
    if l.contains("incorrect") || l.contains("antes:") || t.starts_with('✗') || t.starts_with('❌')
    {
        return Some(Seccion::Mal);
    }
    if l.contains("correct") || l.contains("después:") || t.starts_with('✓') || t.starts_with('✅')
    {
        return Some(Seccion::Bien);
    }
    if l.contains("sugerencia") {
        return Some(Seccion::Sugerencia);
    }
    if l.contains("explicación") || l.contains("justificación") {
        return Some(Seccion::Descripcion);
    }
    None
}

/// Saca los bordes de un panel de rich. Devuelve el contenido y, si el renglón es un
/// borde superior o inferior, el título que trae (vacío si no tiene).
fn sin_marco(l: &str) -> (String, Option<String>) {
    let t = l.trim();
    if t.starts_with(['╭', '╰', '┌', '└', '+']) && t.chars().filter(|c| *c == '─').count() > 3
    {
        let titulo: String = t
            .trim_matches(['╭', '╮', '╰', '╯', '┌', '┐', '└', '┘', '+'])
            .trim_matches('─')
            .trim()
            .to_string();
        return (String::new(), Some(titulo));
    }
    if let Some(dentro) = t.strip_prefix('│').or_else(|| t.strip_prefix('┃')) {
        let dentro = dentro.trim_end().trim_end_matches(['│', '┃']).trim_end();
        // El contenido empieza después del espacio de margen.
        return (dentro.strip_prefix(' ').unwrap_or(dentro).to_string(), None);
    }
    (l.trim_end().to_string(), None)
}

/// «  12 int x;» → «int x;» (paneles de código con números de línea).
fn sin_numero_de_linea(l: &str) -> String {
    let t = l.trim_start();
    let digitos = t.chars().take_while(char::is_ascii_digit).count();
    if digitos > 0 && t[digitos..].starts_with(' ') {
        t[digitos + 1..].trim_end().to_string()
    } else {
        l.trim_end().to_string()
    }
}

fn parrafos(v: &[String]) -> String {
    let mut s = String::new();
    for l in v {
        if l.is_empty() {
            if !s.is_empty() && !s.ends_with("\n\n") {
                s.push_str("\n\n");
            }
        } else {
            if !s.is_empty() && !s.ends_with('\n') {
                s.push(if l.starts_with("    ") { '\n' } else { ' ' });
            }
            s.push_str(l);
        }
    }
    s.trim().to_string()
}

/// El código de un ejemplo; los de relleno («/* Antipatrón … */») no cuentan.
fn ejemplo(v: &[String]) -> String {
    let s = v.join("\n").trim_matches('\n').to_string();
    let t = s.trim();
    if t.len() >= 4 && t.starts_with("/*") && t.ends_with("*/") && !t[2..t.len() - 2].contains("*/")
    {
        return String::new();
    }
    s
}

pub(crate) fn sin_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\u{1b}' {
            if it.peek() == Some(&'[') {
                it.next();
                for d in it.by_ref() {
                    if ('@'..='~').contains(&d) {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const RIPLEY: &str = "\
📖 Regla P1 0x3001h: Siempre verificá la asignación exitosa de memoria dinámica
  • Categoría: Punteros y Memoria
  • Severidad: ERROR
  • Descripción: Comprobar siempre if (ptr == NULL) tras llamar a malloc, calloc o realloc.

╭──────────────── 💡 Justificación Pedagógica de Cátedra ────────────────╮
│ malloc devuelve NULL cuando no hay memoria.                             │
╰─────────────────────────────────────────────────────────────────────────╯
╭──────────────── ❌ Código Incorrecto (Antipatrón) ────────────────╮
│   1 /* Antipatrón o incumplimiento de la norma */                  │
╰────────────────────────────────────────────────────────────────────╯
╭──────────── ✅ Código Correcto (Refactorización sugerida) ────────────╮
│   1 int *p = malloc(sizeof *p);                                        │
│   2 if (p == NULL) {                                                   │
│   3     return 1;                                                      │
│   4 }                                                                  │
╰────────────────────────────────────────────────────────────────────────╯
";

    const GAFF: &str = "\
╭──────────────────── 📘 Regla 0x0001h ────────────────────╮
│ La claridad y prolijidad son de máxima importancia        │
│                                                           │
│ El código debe ser claro y fácil de entender para         │
│ cualquier lector.                                         │
│                                                           │
│ ✓ Ejemplo Correcto:                                       │
│ ```c                                                      │
│ int i = 0;                                                │
│ while (i < limite) {                                      │
│     i++;                                                  │
│ }                                                         │
│ ```                                                       │
│                                                           │
│ ✗ Ejemplo Incorrecto:                                     │
│ ```c                                                      │
│ for (int i=0,j=10;i<j;i++,j--) { }                        │
│ ```                                                       │
│                                                           │
│ Capacidad de Autofix: No                                  │
╰───────────────────────────────────────────────────────────╯
";

    const KANEDA: &str = "\
╭──────────────────── KANEDA · KAN001 ────────────────────╮
│ Uso de la función prohibida 'gets()' (CRITICO)           │
│                                                          │
│ 'gets()' no verifica los límites del búfer destino.      │
│                                                          │
│ Cómo corregirlo: Reemplazá 'gets(buf)' por 'fgets(...)'. │
│                                                          │
│ ✗ Antes:                                                 │
│ char nombre[50];                                         │
│ gets(nombre);                                            │
│                                                          │
│ ✓ Después:                                               │
│ char nombre[50];                                         │
│ fgets(nombre, sizeof(nombre), stdin);                    │
│                                                          │
│ CWE-242: https://cwe.mitre.org/data/definitions/242.html │
│                                                          │
│ Si en una línea es correcto: // kaneda:ignore KAN001     │
╰──────────────────────────────────────────────────────────╯
";

    #[test]
    fn ripley_con_vinetas_y_paneles_de_codigo() {
        let r = desde_salida("ripley", "0x3001h", RIPLEY).unwrap();
        assert_eq!(
            r.titulo,
            "Siempre verificá la asignación exitosa de memoria dinámica"
        );
        assert_eq!(r.categoria, "Punteros y Memoria");
        assert!(r.descripcion.starts_with("Comprobar siempre"), "{r:?}");
        assert!(r.descripcion.contains("malloc devuelve NULL"), "{r:?}");
        // El ejemplo de relleno no se muestra; el bueno, sin números de línea.
        assert_eq!(r.mal, "");
        assert_eq!(
            r.bien,
            "int *p = malloc(sizeof *p);\nif (p == NULL) {\n    return 1;\n}"
        );
        assert!(r.extra.contains(&("Severidad".into(), "ERROR".into())));
    }

    #[test]
    fn gaff_con_bloques_de_codigo() {
        let r = desde_salida("gaff", "0x0001h", GAFF).unwrap();
        assert_eq!(
            r.titulo,
            "La claridad y prolijidad son de máxima importancia"
        );
        assert_eq!(
            r.descripcion,
            "El código debe ser claro y fácil de entender para cualquier lector."
        );
        assert_eq!(r.bien, "int i = 0;\nwhile (i < limite) {\n    i++;\n}");
        assert_eq!(r.mal, "for (int i=0,j=10;i<j;i++,j--) { }");
    }

    #[test]
    fn kaneda_con_antes_y_despues() {
        let r = desde_salida("kaneda", "KAN001", KANEDA).unwrap();
        assert_eq!(r.titulo, "Uso de la función prohibida 'gets()' (CRITICO)");
        assert_eq!(r.sugerencia, "Reemplazá 'gets(buf)' por 'fgets(...)'.");
        assert_eq!(r.mal, "char nombre[50];\ngets(nombre);");
        assert_eq!(
            r.bien,
            "char nombre[50];\nfgets(nombre, sizeof(nombre), stdin);"
        );
        assert!(!r.descripcion.contains("ignore"), "{r:?}");
        assert!(r
            .extra
            .iter()
            .any(|(k, v)| k == "CWE-242" && v.starts_with("https://")));
        // Con el mismo formato que los errores de gcc.
        let md = crate::catalogo::markdown(&r, None);
        assert!(
            md.contains("**Así no:**") && md.contains("**Así sí:**"),
            "{md}"
        );
    }

    #[test]
    fn texto_suelto_y_salida_vacia() {
        let r = desde_salida(
            "x",
            "X1",
            "\u{1b}[1mX1\u{1b}[0m: no uses gets\nLa explicación es corta\n",
        )
        .unwrap();
        assert_eq!(r.titulo, "no uses gets");
        assert_eq!(r.descripcion, "La explicación es corta");
        assert!(desde_salida("x", "X1", "\n\n").is_none());
    }
}
