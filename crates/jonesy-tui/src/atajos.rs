//! Atajos propios de `[atajos]` en `.jonesy.toml` (para terminales que no pasan las
//! F-teclas, o por gusto): `"ctrl+e" = "Compilar"`. Ganan a los de fábrica.

use std::collections::BTreeMap;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::accion::Accion;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Atajo {
    pub codigo: KeyCode,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl Atajo {
    /// `ctrl+e`, `alt+shift+f`, `f9`, `ctrl+f5`, `ctrl+enter`…
    pub fn parsear(spec: &str) -> Result<Atajo, String> {
        let mut a = Atajo {
            codigo: KeyCode::Null,
            ctrl: false,
            alt: false,
            shift: false,
        };
        let partes: Vec<String> = spec.split('+').map(|p| p.trim().to_lowercase()).collect();
        let (tecla, mods) = partes.split_last().ok_or("vacío")?;
        for m in mods {
            match m.as_str() {
                "ctrl" | "control" => a.ctrl = true,
                "alt" | "meta" => a.alt = true,
                "shift" | "mayús" | "mayus" => a.shift = true,
                otro => return Err(format!("modificador desconocido «{otro}»")),
            }
        }
        a.codigo = match tecla.as_str() {
            "enter" | "intro" => KeyCode::Enter,
            "tab" => KeyCode::Tab,
            "esc" => KeyCode::Esc,
            "espacio" | "space" => KeyCode::Char(' '),
            "inicio" | "home" => KeyCode::Home,
            "fin" | "end" => KeyCode::End,
            "repag" | "pageup" => KeyCode::PageUp,
            "avpag" | "pagedown" => KeyCode::PageDown,
            "supr" | "delete" => KeyCode::Delete,
            "insert" | "insertar" => KeyCode::Insert,
            t if t.len() > 1 && t.starts_with('f') && t[1..].parse::<u8>().is_ok() => {
                let n: u8 = t[1..].parse().unwrap();
                if !(1..=12).contains(&n) {
                    return Err(format!("«{t}»: las teclas de función van de f1 a f12"));
                }
                KeyCode::F(n)
            }
            t if t.chars().count() == 1 => KeyCode::Char(t.chars().next().unwrap()),
            t => return Err(format!("tecla desconocida «{t}»")),
        };
        Ok(a)
    }

    pub fn coincide(&self, k: &KeyEvent) -> bool {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let alt = k.modifiers.contains(KeyModifiers::ALT);
        let mut shift = k.modifiers.contains(KeyModifiers::SHIFT);
        let codigo = match k.code {
            // Una letra mayúscula es Shift + la letra.
            KeyCode::Char(c) if c.is_uppercase() => {
                shift = true;
                KeyCode::Char(c.to_lowercase().next().unwrap_or(c))
            }
            KeyCode::BackTab => {
                shift = true;
                KeyCode::Tab
            }
            otro => otro,
        };
        codigo == self.codigo && ctrl == self.ctrl && alt == self.alt && shift == self.shift
    }
}

fn normalizar(s: &str) -> String {
    s.trim()
        .trim_end_matches('…')
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// La acción por su nombre interno (`Compilar`, `IrALinea`) o por su título.
pub fn accion_de(nombre: &str) -> Option<Accion> {
    let n = normalizar(nombre);
    Accion::todas()
        .iter()
        .copied()
        .find(|a| normalizar(&format!("{a:?}")) == n || normalizar(a.info().titulo) == n)
}

/// Los atajos válidos de `[atajos]` y los errores de los que no lo son.
pub fn cargar(mapa: &BTreeMap<String, String>) -> (Vec<(Atajo, Accion, String)>, Vec<String>) {
    let mut v = vec![];
    let mut errores = vec![];
    for (tecla, nombre) in mapa {
        match (Atajo::parsear(tecla), accion_de(nombre)) {
            (Ok(a), Some(acc)) => v.push((a, acc, tecla.clone())),
            (Err(e), _) => errores.push(format!("«{tecla}»: {e}")),
            (_, None) => errores.push(format!(
                "«{tecla}»: no hay una acción «{nombre}» (ver docs/menus.md)"
            )),
        }
    }
    (v, errores)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atajos_propios() {
        let mut m = BTreeMap::new();
        m.insert("ctrl+e".to_string(), "Compilar".to_string());
        m.insert(
            "Alt+Shift+P".to_string(),
            "Correr pruebas (make test)".to_string(),
        );
        m.insert("f13".to_string(), "Compilar".to_string());
        m.insert("ctrl+k".to_string(), "no-existe".to_string());
        let (v, e) = cargar(&m);
        assert_eq!(v.len(), 2, "{e:?}");
        assert_eq!(e.len(), 2, "{e:?}");
        let compilar = v.iter().find(|x| x.1 == Accion::Compilar).unwrap();
        assert!(compilar
            .0
            .coincide(&KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL)));
        assert!(!compilar
            .0
            .coincide(&KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT)));
        let probar = v.iter().find(|x| x.1 == Accion::Probar).unwrap();
        assert!(probar
            .0
            .coincide(&KeyEvent::new(KeyCode::Char('P'), KeyModifiers::ALT)));
        assert_eq!(accion_de("ir a la línea"), Some(Accion::IrALinea));
        assert_eq!(accion_de("IrALinea"), Some(Accion::IrALinea));
    }
}
