//! Máquina de estados del layout adaptativo (§9.2).
//!
//! - Espacioso (≥ 120 × 28): consigna 25 % | editor 50 % | quick-doc 25 % (o dock inferior).
//! - Estándar (90–119 columnas, > 24 filas): consigna 35 % | editor 65 %, quick-doc en un
//!   dock inferior de 3 filas o como ventana flotante con F4.
//! - Compacto (≤ 89 columnas o ≤ 24 filas): una vista a pantalla completa por vez
//!   (F7 consigna, F8 código, F9 terminal, F4 doc; la barra izquierda también ocupa toda
//!   la pantalla).
//!
//! La barra izquierda (archivos y estructura del proyecto), el panel lateral derecho y el
//! panel inferior se abren bajo demanda. Todos los tamaños vienen de [`TamPaneles`] y cada
//! división expone un [`Separador`] que se arrastra con el mouse.

use jonesy_core::{PosicionDoc, TamPaneles};
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
    /// La barra izquierda (explorador o proyecto).
    Navegador,
}

impl Vista {
    pub fn nombre(self) -> &'static str {
        match self {
            Vista::Consigna => "consigna",
            Vista::Codigo => "codigo",
            Vista::Terminal => "terminal",
            Vista::Doc => "doc",
            Vista::Navegador => "navegador",
        }
    }

    pub fn de_nombre(s: &str) -> Vista {
        match s {
            "consigna" => Vista::Consigna,
            "terminal" => Vista::Terminal,
            "doc" => Vista::Doc,
            "navegador" => Vista::Navegador,
            _ => Vista::Codigo,
        }
    }
}

/// Qué tamaño cambia al arrastrar un separador.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sep {
    Izquierdo,
    Consigna,
    QuickDoc,
    Dock,
    Lateral,
    Inferior,
}

/// Una división arrastrable: línea vertical (columna `pos`) u horizontal (fila `pos`)
/// que parte la región `base`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Separador {
    pub sep: Sep,
    pub vertical: bool,
    pub pos: u16,
    pub base: Rect,
}

impl Separador {
    /// true si la celda (x, y) está sobre el separador (con una celda de tolerancia).
    pub fn toca(&self, x: u16, y: u16) -> bool {
        let b = self.base;
        if self.vertical {
            (x == self.pos || x + 1 == self.pos) && y >= b.y && y < b.y + b.height
        } else {
            (y == self.pos || y + 1 == self.pos) && x >= b.x && x < b.x + b.width
        }
    }

    /// Nuevo tamaño al soltar el separador en (x, y).
    pub fn aplicar(&self, x: u16, y: u16, t: &mut TamPaneles) {
        let b = self.base;
        let pct = |n: u16, total: u16| (n as u32 * 100 / total.max(1) as u32) as u16;
        match self.sep {
            Sep::Izquierdo => t.izquierdo = x.saturating_sub(b.x),
            Sep::Consigna => t.consigna = pct(x.saturating_sub(b.x), b.width).max(1),
            Sep::QuickDoc => t.quickdoc = pct((b.x + b.width).saturating_sub(x), b.width),
            Sep::Dock => t.dock = (b.y + b.height).saturating_sub(y),
            Sep::Lateral => t.lateral = pct((b.x + b.width).saturating_sub(x), b.width),
            Sep::Inferior => t.inferior = (b.y + b.height).saturating_sub(y),
        }
        *t = t.acotar();
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
    pub vista: Vista,
    /// Panel lateral derecho (git, reglas, variables, cat-scan).
    pub lateral: bool,
    /// Ancho mínimo del panel lateral en % (cat-scan necesita más).
    pub lateral_minimo: u16,
    /// Barra izquierda (explorador y proyecto).
    pub izquierdo: bool,
    pub tam: TamPaneles,
}

/// Rectángulos resultantes; `None` = no visible.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Areas {
    pub menu: Rect,
    pub izquierdo: Option<Rect>,
    pub consigna: Option<Rect>,
    pub editor: Option<Rect>,
    pub quickdoc: Option<Rect>,
    /// quick-doc como ventana flotante (estándar con F4 y sin dock).
    pub quickdoc_flotante: bool,
    pub inferior: Option<Rect>,
    pub lateral: Option<Rect>,
    pub estado: Rect,
    pub separadores: Vec<Separador>,
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
    let t = p.tam.acotar();
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
            Vista::Navegador if p.izquierdo => r.izquierdo = Some(cuerpo),
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

    // Barra izquierda de ancho fijo en columnas.
    let mut resto = cuerpo;
    if p.izquierdo {
        let w = t.izquierdo.min(cuerpo.width / 2);
        let (izq, der) = partir_h(cuerpo, w);
        r.izquierdo = Some(izq);
        r.separadores.push(Separador {
            sep: Sep::Izquierdo,
            vertical: true,
            pos: der.x,
            base: cuerpo,
        });
        resto = der;
    }

    // Panel lateral derecho.
    let mut centro = resto;
    if p.lateral {
        let pct = t.lateral.max(p.lateral_minimo);
        let w = (resto.width as u32 * pct as u32 / 100) as u16;
        let w = w.max(28).min(resto.width * 6 / 10);
        let (c, l) = partir_h(resto, resto.width - w);
        centro = c;
        r.lateral = Some(l);
        r.separadores.push(Separador {
            sep: Sep::Lateral,
            vertical: true,
            pos: l.x,
            base: resto,
        });
    }

    // Panel inferior.
    let mut arriba = centro;
    if p.panel_inferior {
        let alto = t.inferior.clamp(4, centro.height.saturating_sub(6).max(4));
        let (ar, ab) = partir_v(centro, centro.height - alto);
        arriba = ar;
        r.inferior = Some(ab);
        r.separadores.push(Separador {
            sep: Sep::Inferior,
            vertical: false,
            pos: ab.y,
            base: centro,
        });
    }

    let doc_derecha = modo == Modo::Espacioso
        && p.quickdoc_posicion == PosicionDoc::Derecha
        && p.quickdoc_visible;
    let pct_consigna = match (t.consigna, modo) {
        (0, Modo::Espacioso) => 25,
        (0, _) => 35,
        (c, _) => c,
    };
    let mut editor = arriba;
    if p.hay_consigna {
        let w = (arriba.width as u32 * pct_consigna as u32 / 100) as u16;
        let (c, e) = partir_h(arriba, w);
        r.consigna = Some(c);
        r.separadores.push(Separador {
            sep: Sep::Consigna,
            vertical: true,
            pos: e.x,
            base: arriba,
        });
        editor = e;
    }
    if doc_derecha {
        let w = (arriba.width as u32 * t.quickdoc as u32 / 100) as u16;
        let (e, d) = partir_h(editor, editor.width.saturating_sub(w));
        editor = e;
        r.quickdoc = Some(d);
        r.separadores.push(Separador {
            sep: Sep::QuickDoc,
            vertical: true,
            pos: d.x,
            base: arriba,
        });
    } else if p.quickdoc_visible {
        if modo == Modo::Espacioso || p.quickdoc_posicion == PosicionDoc::Inferior {
            let base = editor;
            let alto = t.dock.min(editor.height.saturating_sub(4));
            let (e, d) = partir_v(editor, editor.height.saturating_sub(alto));
            editor = e;
            r.quickdoc = Some(d);
            r.separadores.push(Separador {
                sep: Sep::Dock,
                vertical: false,
                pos: d.y,
                base,
            });
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
            vista: Vista::Codigo,
            lateral: false,
            lateral_minimo: 0,
            izquierdo: false,
            tam: TamPaneles::default(),
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
        assert_eq!(a.separadores.len(), 2);
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
        p.izquierdo = true;
        p.vista = Vista::Navegador;
        assert_eq!(calcular(&p).izquierdo.unwrap().width, 80);
    }

    #[test]
    fn paneles_izquierdo_inferior_y_lateral() {
        let mut p = pedido(160, 40);
        p.panel_inferior = true;
        p.lateral = true;
        p.izquierdo = true;
        let a = calcular(&p);
        let izq = a.izquierdo.unwrap();
        assert_eq!(izq.width, 28);
        assert_eq!(a.consigna.unwrap().x, 28);
        let inf = a.inferior.unwrap();
        assert_eq!(inf.height, 10);
        assert_eq!(a.lateral.unwrap().width, 39); // 30 % de 132
        assert_eq!(a.editor.unwrap().y + a.editor.unwrap().height, inf.y);
        assert_eq!(a.separadores.len(), 5);
    }

    #[test]
    fn tamanos_configurables_y_arrastre() {
        let mut p = pedido(160, 40);
        p.izquierdo = true;
        p.panel_inferior = true;
        p.tam = TamPaneles {
            izquierdo: 40,
            consigna: 30,
            quickdoc: 20,
            inferior: 15,
            ..Default::default()
        };
        let a = calcular(&p);
        assert_eq!(a.izquierdo.unwrap().width, 40);
        assert_eq!(a.consigna.unwrap().width, 36); // 30 % de 120
        assert_eq!(a.quickdoc.unwrap().width, 24);
        assert_eq!(a.inferior.unwrap().height, 15);
        // Arrastrar el separador de la consigna 12 columnas a la derecha.
        let s = *a
            .separadores
            .iter()
            .find(|s| s.sep == Sep::Consigna)
            .unwrap();
        assert!(s.toca(s.pos, 10));
        let mut t = p.tam;
        s.aplicar(s.pos + 12, 10, &mut t);
        assert_eq!(t.consigna, 40);
        // El inferior: subir 5 filas lo agranda en 5.
        let s = *a
            .separadores
            .iter()
            .find(|s| s.sep == Sep::Inferior)
            .unwrap();
        s.aplicar(0, s.pos - 5, &mut t);
        assert_eq!(t.inferior, 20);
        // Los valores se acotan.
        let s = *a
            .separadores
            .iter()
            .find(|s| s.sep == Sep::Izquierdo)
            .unwrap();
        s.aplicar(2, 5, &mut t);
        assert_eq!(t.izquierdo, 14);
    }
}
