//! Máquina de estados del layout adaptativo (§9.2).
//!
//! - Espacioso (≥ 120 × 28): consigna 25 % | editor 50 % | quick-doc 25 % (o dock inferior).
//! - Estándar (90–119 columnas, > 24 filas): consigna 35 % | editor 65 %, quick-doc en un
//!   dock inferior de 3 filas o como ventana flotante con F4.
//! - Compacto (≤ 89 columnas o ≤ 24 filas): una vista a pantalla completa por vez
//!   (F7 consigna, F8 código, F9 terminal, F4 doc).
//!
//! El panel inferior (deck, diagnósticos, git, depurador) se abre bajo demanda en los modos
//! espacioso y estándar; en compacto ocupa la vista "terminal".

use jonesy_core::PosicionDoc;
use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    Espacioso,
    Estandar,
    Compacto,
}

impl Modo {
    pub fn de_tamano(ancho: u16, alto: u16) -> Modo {
        if ancho <= 89 || alto <= 24 {
            Modo::Compacto
        } else if ancho >= 120 && alto >= 28 {
            Modo::Espacioso
        } else {
            Modo::Estandar
        }
    }

    pub fn nombre(self) -> &'static str {
        match self {
            Modo::Espacioso => "espacioso",
            Modo::Estandar => "estándar",
            Modo::Compacto => "compacto",
        }
    }
}

/// Vista a pantalla completa del modo compacto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vista {
    Consigna,
    Codigo,
    Terminal,
    Doc,
}

impl Vista {
    pub fn nombre(self) -> &'static str {
        match self {
            Vista::Consigna => "consigna",
            Vista::Codigo => "codigo",
            Vista::Terminal => "terminal",
            Vista::Doc => "doc",
        }
    }

    pub fn de_nombre(s: &str) -> Vista {
        match s {
            "consigna" => Vista::Consigna,
            "terminal" => Vista::Terminal,
            "doc" => Vista::Doc,
            _ => Vista::Codigo,
        }
    }
}

/// Entrada del cálculo de layout.
#[derive(Debug, Clone, Copy)]
pub struct Pedido {
    pub area: Rect,
    pub hay_consigna: bool,
    pub quickdoc_visible: bool,
    pub quickdoc_posicion: PosicionDoc,
    pub panel_inferior: bool,
    /// Alto del panel inferior en filas (redimensionable con el mouse).
    pub alto_inferior: u16,
    /// Porcentaje del ancho para la consigna (redimensionable con el mouse).
    pub ancho_consigna: Option<u16>,
    pub vista: Vista,
    /// Panel lateral derecho extra (git, reglas, variables, cat-scan).
    pub lateral: bool,
    /// Porcentaje del ancho para el panel lateral (cat-scan necesita más).
    pub ancho_lateral: u16,
}

/// Rectángulos resultantes; `None` = no visible.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Areas {
    pub menu: Rect,
    pub consigna: Option<Rect>,
    pub editor: Option<Rect>,
    pub quickdoc: Option<Rect>,
    /// quick-doc como ventana flotante (estándar con F4 y sin dock).
    pub quickdoc_flotante: bool,
    pub inferior: Option<Rect>,
    pub lateral: Option<Rect>,
    pub estado: Rect,
    /// Separadores arrastrables: (columna de la consigna | editor), (fila editor / inferior).
    pub sep_vertical: Option<u16>,
    pub sep_horizontal: Option<u16>,
}

fn partir_h(r: Rect, izq: u16) -> (Rect, Rect) {
    let izq = izq.min(r.width);
    (
        Rect { width: izq, ..r },
        Rect {
            x: r.x + izq,
            width: r.width - izq,
            ..r
        },
    )
}

fn partir_v(r: Rect, arriba: u16) -> (Rect, Rect) {
    let arriba = arriba.min(r.height);
    (
        Rect {
            height: arriba,
            ..r
        },
        Rect {
            y: r.y + arriba,
            height: r.height - arriba,
            ..r
        },
    )
}

pub fn calcular(p: &Pedido) -> Areas {
    let a = p.area;
    let modo = Modo::de_tamano(a.width, a.height);
    let (menu, resto) = partir_v(a, 1);
    let (cuerpo, estado) = partir_v(resto, resto.height.saturating_sub(1));
    let mut r = Areas {
        menu,
        estado,
        ..Default::default()
    };

    if modo == Modo::Compacto {
        match p.vista {
            Vista::Consigna if p.hay_consigna => r.consigna = Some(cuerpo),
            Vista::Terminal => r.inferior = Some(cuerpo),
            Vista::Doc => r.quickdoc = Some(cuerpo),
            _ => {
                if p.lateral {
                    r.lateral = Some(cuerpo);
                } else {
                    r.editor = Some(cuerpo);
                }
            }
        }
        return r;
    }

    // Panel lateral derecho: por defecto 30 % del ancho, mín. 28 columnas.
    let mut centro = cuerpo;
    if p.lateral {
        let w = (cuerpo.width * p.ancho_lateral.clamp(20, 60) / 100)
            .max(28)
            .min(cuerpo.width * 6 / 10);
        let (c, l) = partir_h(cuerpo, cuerpo.width - w);
        centro = c;
        r.lateral = Some(l);
    }

    // Panel inferior bajo demanda.
    let mut arriba = centro;
    if p.panel_inferior {
        let alto = p
            .alto_inferior
            .clamp(4, centro.height.saturating_sub(6).max(4));
        let (ar, ab) = partir_v(centro, centro.height - alto);
        arriba = ar;
        r.inferior = Some(ab);
        r.sep_horizontal = Some(ab.y);
    }

    let doc_derecha = modo == Modo::Espacioso
        && p.quickdoc_posicion == PosicionDoc::Derecha
        && p.quickdoc_visible;
    let (pct_consigna, pct_doc) = match modo {
        Modo::Espacioso => (25, if doc_derecha { 25 } else { 0 }),
        _ => (35, 0),
    };
    let pct_consigna = p.ancho_consigna.unwrap_or(pct_consigna).clamp(15, 60);
    let mut editor = arriba;
    if p.hay_consigna {
        let w = arriba.width * pct_consigna / 100;
        let (c, e) = partir_h(arriba, w);
        r.consigna = Some(c);
        r.sep_vertical = Some(e.x);
        editor = e;
    }
    if pct_doc > 0 {
        let w = arriba.width * pct_doc / 100;
        let (e, d) = partir_h(editor, editor.width.saturating_sub(w));
        editor = e;
        r.quickdoc = Some(d);
    } else if p.quickdoc_visible {
        if modo == Modo::Espacioso || p.quickdoc_posicion == PosicionDoc::Inferior {
            // Dock inferior de 3 filas (+ borde) bajo el código.
            let (e, d) = partir_v(editor, editor.height.saturating_sub(4));
            editor = e;
            r.quickdoc = Some(d);
        } else {
            // Estándar con preferencia "derecha": ventana flotante sobre el editor.
            r.quickdoc_flotante = true;
            let w = (editor.width * 6 / 10).max(40).min(editor.width);
            let h = (editor.height / 2).max(8).min(editor.height);
            r.quickdoc = Some(Rect {
                x: editor.x + editor.width - w,
                y: editor.y,
                width: w,
                height: h,
            });
        }
    }
    r.editor = Some(editor);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pedido(w: u16, h: u16) -> Pedido {
        Pedido {
            area: Rect::new(0, 0, w, h),
            hay_consigna: true,
            quickdoc_visible: true,
            quickdoc_posicion: PosicionDoc::Derecha,
            panel_inferior: false,
            alto_inferior: 10,
            ancho_consigna: None,
            vista: Vista::Codigo,
            lateral: false,
            ancho_lateral: 30,
        }
    }

    #[test]
    fn modos() {
        assert_eq!(Modo::de_tamano(120, 28), Modo::Espacioso);
        assert_eq!(Modo::de_tamano(120, 27), Modo::Estandar);
        assert_eq!(Modo::de_tamano(100, 30), Modo::Estandar);
        assert_eq!(Modo::de_tamano(89, 50), Modo::Compacto);
        assert_eq!(Modo::de_tamano(200, 24), Modo::Compacto);
        assert_eq!(Modo::de_tamano(80, 24), Modo::Compacto);
    }

    #[test]
    fn espacioso_tres_columnas() {
        let a = calcular(&pedido(160, 40));
        let (c, e, d) = (a.consigna.unwrap(), a.editor.unwrap(), a.quickdoc.unwrap());
        assert_eq!(c.width, 40);
        assert_eq!(d.width, 40);
        assert_eq!(e.width, 80);
        assert_eq!(c.x + c.width, e.x);
        assert_eq!(e.x + e.width, d.x);
        assert_eq!(a.menu.height, 1);
        assert_eq!(a.estado.y, 39);
    }

    #[test]
    fn estandar_dock_o_flotante() {
        let mut p = pedido(100, 30);
        let a = calcular(&p);
        assert_eq!(a.consigna.unwrap().width, 35);
        assert!(a.quickdoc_flotante);
        p.quickdoc_posicion = PosicionDoc::Inferior;
        let a = calcular(&p);
        assert!(!a.quickdoc_flotante);
        let (e, d) = (a.editor.unwrap(), a.quickdoc.unwrap());
        assert_eq!(d.height, 4);
        assert_eq!(e.y + e.height, d.y);
        assert_eq!(e.width, 65);
    }

    #[test]
    fn compacto_una_vista() {
        let mut p = pedido(80, 24);
        let a = calcular(&p);
        assert_eq!(a.editor.unwrap().width, 80);
        assert!(a.consigna.is_none() && a.quickdoc.is_none());
        p.vista = Vista::Consigna;
        assert_eq!(calcular(&p).consigna.unwrap().height, 22);
        p.vista = Vista::Terminal;
        assert!(calcular(&p).inferior.is_some());
    }

    #[test]
    fn panel_inferior_y_lateral() {
        let mut p = pedido(160, 40);
        p.panel_inferior = true;
        p.lateral = true;
        let a = calcular(&p);
        let inf = a.inferior.unwrap();
        assert_eq!(inf.height, 10);
        assert_eq!(a.lateral.unwrap().width, 48);
        assert_eq!(a.editor.unwrap().y + a.editor.unwrap().height, inf.y);
    }
}
