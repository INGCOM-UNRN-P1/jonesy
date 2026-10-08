//! Búsqueda difusa para la paleta de comandos (RF-1.5) y el selector de archivos.

/// Puntaje de `patron` contra `texto` (subsecuencia sin distinguir mayúsculas) o None si
/// no coincide. Premia letras consecutivas, inicio de palabra y prefijo.
pub fn puntaje(patron: &str, texto: &str) -> Option<i64> {
    if patron.is_empty() {
        return Some(0);
    }
    let t: Vec<char> = texto.chars().flat_map(char::to_lowercase).collect();
    let p: Vec<char> = patron.chars().flat_map(char::to_lowercase).filter(|c| !c.is_whitespace()).collect();
    let mut puntos = 0i64;
    let mut j = 0;
    let mut anterior: Option<usize> = None;
    for (i, c) in t.iter().enumerate() {
        if j < p.len() && *c == p[j] {
            puntos += 1;
            if anterior == Some(i.wrapping_sub(1)) {
                puntos += 5;
            }
            if i == 0 || !t[i - 1].is_alphanumeric() {
                puntos += 8;
            }
            if i == j {
                puntos += 3;
            }
            anterior = Some(i);
            j += 1;
        }
    }
    (j == p.len()).then(|| puntos * 100 - t.len() as i64)
}

/// Índices de `opciones` que coinciden, mejores primero.
pub fn filtrar<S: AsRef<str>>(patron: &str, opciones: &[S]) -> Vec<usize> {
    let mut v: Vec<(i64, usize)> =
        opciones.iter().enumerate().filter_map(|(i, o)| puntaje(patron, o.as_ref()).map(|p| (p, i))).collect();
    v.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    v.into_iter().map(|(_, i)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subsecuencia() {
        assert!(puntaje("cmp", "Compilar").is_some());
        assert!(puntaje("xyz", "Compilar").is_none());
        assert_eq!(puntaje("", "x"), Some(0));
    }

    #[test]
    fn ordena_por_relevancia() {
        let opciones = ["Git: commit", "Compilar (F6)", "Cerrar buffer", "Copiar"];
        assert_eq!(filtrar("comp", &opciones)[0], 1);
        assert_eq!(filtrar("gc", &opciones)[0], 0);
        assert_eq!(filtrar("cerr", &opciones), vec![2]);
    }
}
