//! Proyección de la pantalla virtual `vt100` a la cuadrícula de ratatui, y salida con
//! colores ANSI (ripley explain) convertida a líneas.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

fn color(c: vt100::Color) -> Option<Color> {
    match c {
        vt100::Color::Default => None,
        vt100::Color::Idx(i) => Some(Color::Indexed(i)),
        vt100::Color::Rgb(r, g, b) => Some(Color::Rgb(r, g, b)),
    }
}

fn estilo(celda: &vt100::Cell) -> Style {
    let mut s = Style::default();
    if let Some(c) = color(celda.fgcolor()) {
        s = s.fg(c);
    }
    if let Some(c) = color(celda.bgcolor()) {
        s = s.bg(c);
    }
    if celda.bold() {
        s = s.add_modifier(Modifier::BOLD);
    }
    if celda.italic() {
        s = s.add_modifier(Modifier::ITALIC);
    }
    if celda.underline() {
        s = s.add_modifier(Modifier::UNDERLINED);
    }
    if celda.inverse() {
        s = s.add_modifier(Modifier::REVERSED);
    }
    s
}

/// Copia la pantalla al área; devuelve la posición del cursor si es visible.
pub fn dibujar_pantalla(
    pantalla: &vt100::Screen,
    area: Rect,
    buf: &mut Buffer,
    mostrar_cursor: bool,
) -> Option<(u16, u16)> {
    let (filas, cols) = pantalla.size();
    for f in 0..filas.min(area.height) {
        let mut c = 0;
        while c < cols.min(area.width) {
            let Some(celda) = pantalla.cell(f, c) else {
                break;
            };
            let x = area.x + c;
            let y = area.y + f;
            let contenido = celda.contents();
            let destino = &mut buf[(x, y)];
            if contenido.is_empty() {
                destino.set_symbol(" ");
            } else {
                destino.set_symbol(&contenido);
            }
            destino.set_style(estilo(celda));
            c += if celda.is_wide() { 2 } else { 1 };
        }
    }
    let (cf, cc) = pantalla.cursor_position();
    (mostrar_cursor
        && !pantalla.hide_cursor()
        && cf < area.height
        && cc < area.width
        && pantalla.scrollback() == 0)
        .then_some((area.x + cc, area.y + cf))
}

/// Interpreta texto con secuencias ANSI y lo devuelve como líneas con estilo.
pub fn ansi_a_lineas(texto: &str, ancho: u16) -> Vec<Line<'static>> {
    let lineas_fuente = texto.lines().count().max(1);
    let alto = (lineas_fuente * 3).clamp(10, 2000) as u16;
    let mut p = vt100::Parser::new(alto, ancho.max(20), 0);
    p.process(texto.replace('\n', "\r\n").as_bytes());
    let pantalla = p.screen();
    let (filas, cols) = pantalla.size();
    let mut salida = vec![];
    for f in 0..filas {
        let mut spans: Vec<Span<'static>> = vec![];
        let mut c = 0;
        while c < cols {
            let Some(celda) = pantalla.cell(f, c) else {
                break;
            };
            let s = estilo(celda);
            let contenido = if celda.contents().is_empty() {
                " ".to_string()
            } else {
                celda.contents()
            };
            match spans.last_mut() {
                Some(ult) if ult.style == s => ult.content.to_mut().push_str(&contenido),
                _ => spans.push(Span::styled(contenido, s)),
            }
            c += if celda.is_wide() { 2 } else { 1 };
        }
        // Recorta los espacios finales sin fondo.
        while let Some(ult) = spans.last_mut() {
            let recortado = ult.content.trim_end().to_string();
            if ult.style.bg.is_some() || recortado.len() == ult.content.len() {
                break;
            }
            if recortado.is_empty() {
                spans.pop();
            } else {
                ult.content = recortado.into();
                break;
            }
        }
        salida.push(Line::from(spans));
    }
    while salida.last().is_some_and(|l| l.width() == 0) {
        salida.pop();
    }
    salida
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_con_colores() {
        let l = ansi_a_lineas("\x1b[1;31mError\x1b[0m: algo\nsegunda", 40);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].spans[0].content, "Error");
        assert_eq!(l[0].spans[0].style.fg, Some(Color::Indexed(1)));
        assert!(l[0].spans[0].style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(l[1].spans[0].content, "segunda");
    }
}
