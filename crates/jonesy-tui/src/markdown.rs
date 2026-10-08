//! Renderizador de Markdown a líneas de ratatui (RF-8.1): encabezados con línea divisoria,
//! énfasis, tachado, citas con barra lateral, listas y tareas, tablas GFM con bordes de
//! caja, bloques de código resaltados y enlaces navegables.

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::resaltado::{resaltar_etiqueta, Estado};
use crate::tema::{self, Simbolos};

/// Enlace en la salida: línea, columnas [ini, fin) y destino.
#[derive(Debug, Clone, PartialEq)]
pub struct Enlace {
    pub linea: usize,
    pub ini: usize,
    pub fin: usize,
    pub destino: String,
}

#[derive(Debug, Default)]
pub struct Documento {
    pub lineas: Vec<Line<'static>>,
    pub enlaces: Vec<Enlace>,
    /// Línea de salida de cada encabezado (para navegar).
    pub encabezados: Vec<(usize, String)>,
}

struct Tabla {
    alineaciones: Vec<Alignment>,
    filas: Vec<Vec<String>>,
    celda: String,
    encabezado: usize,
}

struct R<'s> {
    ancho: usize,
    sim: &'s Simbolos,
    doc: Documento,
    actual: Vec<Span<'static>>,
    col: usize,
    estilos: Vec<Style>,
    /// Prefijos por nivel de anidamiento (cita, sangría de lista).
    prefijos: Vec<String>,
    /// Prefijo pendiente para la primera línea de un ítem.
    vineta: Option<String>,
    listas: Vec<Option<u64>>,
    codigo: Option<(String, String)>,
    tabla: Option<Tabla>,
    enlace: Option<(String, usize, usize)>,
    encabezado: Option<(HeadingLevel, String)>,
}

impl R<'_> {
    fn estilo(&self) -> Style {
        self.estilos.last().copied().unwrap_or_default()
    }

    fn empujar_estilo(&mut self, f: impl FnOnce(Style) -> Style) {
        let s = f(self.estilo());
        self.estilos.push(s);
    }

    fn prefijo(&mut self) {
        if !self.actual.is_empty() {
            return;
        }
        let mut p: String = self.prefijos.concat();
        if let Some(v) = self.vineta.take() {
            // La viñeta reemplaza la sangría del último nivel.
            let ultimo = self.prefijos.last().map(|s| s.chars().count()).unwrap_or(0);
            let base: String = p.chars().take(p.chars().count() - ultimo).collect();
            p = format!("{base}{v}");
        }
        if !p.is_empty() {
            self.col = UnicodeWidthStr::width(p.as_str());
            let estilo = if p.contains(self.sim.cita.trim()) { Style::default().fg(tema::ATENUADO) } else { Style::default().fg(tema::ACENTO) };
            self.actual.push(Span::styled(p, estilo));
        } else {
            self.col = 0;
        }
    }

    fn cortar(&mut self) {
        let spans = std::mem::take(&mut self.actual);
        self.doc.lineas.push(Line::from(spans));
        self.col = 0;
    }

    fn linea_en_blanco(&mut self) {
        if !self.actual.is_empty() {
            self.cortar();
        }
        if self.doc.lineas.last().is_some_and(|l| l.width() > 0) {
            let pre = self.prefijos.concat();
            self.doc.lineas.push(Line::from(Span::styled(pre.trim_end().to_string(), Style::default().fg(tema::ATENUADO))));
        }
    }

    /// Agrega texto con ajuste de línea por palabras.
    fn texto(&mut self, t: &str) {
        if let Some(tabla) = &mut self.tabla {
            tabla.celda.push_str(t);
            return;
        }
        if let Some((_, titulo)) = &mut self.encabezado {
            titulo.push_str(t);
        }
        let estilo = self.estilo();
        for (i, parte) in t.split(' ').enumerate() {
            if i > 0 {
                self.prefijo();
                if self.col + 1 < self.ancho {
                    self.actual.push(Span::styled(" ", estilo));
                    self.col += 1;
                }
            }
            if parte.is_empty() {
                continue;
            }
            let w = UnicodeWidthStr::width(parte);
            if self.col + w > self.ancho && self.col > self.prefijos.concat().len() {
                // Saca el espacio final antes de cortar.
                if self.actual.last().is_some_and(|s| s.content == " ") {
                    self.actual.pop();
                }
                self.cortar();
            }
            self.prefijo();
            let linea = self.doc.lineas.len();
            self.actual.push(Span::styled(parte.to_string(), estilo));
            if let Some((_, l, c)) = &mut self.enlace {
                if *l == usize::MAX {
                    *l = linea;
                    *c = self.col;
                }
            }
            self.col += w;
        }
    }

    fn cerrar_enlace(&mut self) {
        if let Some((destino, linea, ini)) = self.enlace.take() {
            if linea != usize::MAX {
                let fin = if linea == self.doc.lineas.len() { self.col } else { self.ancho };
                self.doc.enlaces.push(Enlace { linea, ini, fin, destino });
            }
        }
    }

    fn regla(&mut self, c: char, estilo: Style) {
        let pre = self.prefijos.concat();
        let n = self.ancho.saturating_sub(UnicodeWidthStr::width(pre.as_str()));
        self.doc.lineas.push(Line::from(vec![Span::raw(pre), Span::styled(c.to_string().repeat(n), estilo)]));
    }

    fn bloque_codigo(&mut self, etiqueta: &str, codigo: &str) {
        let pre = self.prefijos.concat();
        let fondo = Style::default().bg(tema::FONDO_CODIGO);
        let interior = self.ancho.saturating_sub(UnicodeWidthStr::width(pre.as_str()) + 2).max(1);
        if !etiqueta.is_empty() {
            self.doc.lineas.push(Line::from(vec![
                Span::raw(pre.clone()),
                Span::styled(format!(" {etiqueta} "), Style::default().fg(tema::ATENUADO).bg(tema::FONDO_CODIGO).add_modifier(Modifier::ITALIC)),
            ]));
        }
        let mut estado = Estado::default();
        for linea in codigo.trim_end_matches('\n').split('\n') {
            let cs: Vec<char> = linea.replace('\t', "    ").chars().collect();
            let tramos = resaltar_etiqueta(etiqueta, &cs, &mut estado);
            let mut spans = vec![Span::raw(pre.clone()), Span::styled(" ", fondo)];
            let mut i = 0;
            let mut usado = 0;
            let poner = |spans: &mut Vec<Span<'static>>, a: usize, b: usize, st: Style, usado: &mut usize| {
                let s: String = cs[a..b].iter().take(interior.saturating_sub(*usado)).collect();
                *usado += UnicodeWidthStr::width(s.as_str());
                if !s.is_empty() {
                    spans.push(Span::styled(s, st));
                }
            };
            for (a, b, clase) in tramos {
                if a > i {
                    poner(&mut spans, i, a, fondo, &mut usado);
                }
                poner(&mut spans, a, b, tema::estilo_clase(clase).bg(tema::FONDO_CODIGO), &mut usado);
                i = b;
            }
            if i < cs.len() {
                poner(&mut spans, i, cs.len(), fondo, &mut usado);
            }
            spans.push(Span::styled(" ".repeat(interior.saturating_sub(usado) + 1), fondo));
            self.doc.lineas.push(Line::from(spans));
        }
    }

    fn tabla(&mut self, t: Tabla) {
        let pre = self.prefijos.concat();
        let ncol = t.filas.iter().map(|f| f.len()).max().unwrap_or(0);
        if ncol == 0 {
            return;
        }
        let mut anchos = vec![1usize; ncol];
        for f in &t.filas {
            for (i, c) in f.iter().enumerate() {
                anchos[i] = anchos[i].max(UnicodeWidthStr::width(c.as_str()));
            }
        }
        // Encoge las columnas más anchas hasta que entre.
        let disponible = self.ancho.saturating_sub(UnicodeWidthStr::width(pre.as_str()) + 1 + ncol * 3).max(ncol);
        while anchos.iter().sum::<usize>() > disponible {
            let (i, _) = anchos.iter().enumerate().max_by_key(|(_, w)| **w).unwrap();
            if anchos[i] <= 3 {
                break;
            }
            anchos[i] -= 1;
        }
        let s = self.sim;
        let borde = Style::default().fg(tema::ATENUADO);
        let separador = |izq: char, medio: char, der: char| {
            let mut x = String::new();
            x.push(izq);
            for (i, w) in anchos.iter().enumerate() {
                x.push_str(&s.h.to_string().repeat(w + 2));
                x.push(if i + 1 == anchos.len() { der } else { medio });
            }
            x
        };
        let mut lineas = vec![Line::from(vec![Span::raw(pre.clone()), Span::styled(separador(s.esq[0], s.cruz[0], s.esq[1]), borde)])];
        for (n, fila) in t.filas.iter().enumerate() {
            let mut spans = vec![Span::raw(pre.clone()), Span::styled(s.v.to_string(), borde)];
            for (i, w) in anchos.iter().enumerate() {
                let texto = fila.get(i).map(String::as_str).unwrap_or("");
                let mut celda: String = String::new();
                for c in texto.chars() {
                    if UnicodeWidthStr::width(celda.as_str()) + unicode_width::UnicodeWidthChar::width(c).unwrap_or(0) > *w {
                        break;
                    }
                    celda.push(c);
                }
                let resto = w - UnicodeWidthStr::width(celda.as_str());
                let (izq, der) = match t.alineaciones.get(i) {
                    Some(Alignment::Right) => (resto, 0),
                    Some(Alignment::Center) => (resto / 2, resto - resto / 2),
                    _ => (0, resto),
                };
                let estilo = if n < t.encabezado { Style::default().add_modifier(Modifier::BOLD).fg(tema::ACENTO) } else { Style::default() };
                spans.push(Span::styled(format!(" {}{}{} ", " ".repeat(izq), celda, " ".repeat(der)), estilo));
                spans.push(Span::styled(s.v.to_string(), borde));
            }
            lineas.push(Line::from(spans));
            if n + 1 == t.encabezado && t.filas.len() > t.encabezado {
                lineas.push(Line::from(vec![Span::raw(pre.clone()), Span::styled(separador(s.cruz[2], s.cruz[4], s.cruz[3]), borde)]));
            }
        }
        lineas.push(Line::from(vec![Span::raw(pre), Span::styled(separador(s.esq[2], s.cruz[1], s.esq[3]), borde)]));
        self.doc.lineas.extend(lineas);
    }

    fn evento(&mut self, e: Event) {
        match e {
            Event::Start(tag) => self.inicio(tag),
            Event::End(tag) => self.fin(tag),
            Event::Text(t) => {
                if let Some((_, buf)) = &mut self.codigo {
                    buf.push_str(&t);
                } else {
                    self.texto(&t);
                }
            }
            Event::Code(t) => {
                if let Some(tabla) = &mut self.tabla {
                    tabla.celda.push_str(&t);
                    return;
                }
                self.empujar_estilo(|s| s.fg(tema::OK).bg(tema::FONDO_CODIGO));
                self.texto(&t);
                self.estilos.pop();
            }
            Event::SoftBreak => self.texto(" "),
            Event::HardBreak => {
                self.cortar();
            }
            Event::Rule => {
                self.linea_en_blanco();
                self.regla(self.sim.h, Style::default().fg(tema::ATENUADO));
            }
            Event::TaskListMarker(hecho) => {
                let marca = self.sim.tarea[hecho as usize];
                let estilo = if hecho { Style::default().fg(tema::OK) } else { Style::default().fg(tema::ATENUADO) };
                self.prefijo();
                self.actual.push(Span::styled(marca.to_string(), estilo));
                self.col += UnicodeWidthStr::width(marca);
            }
            Event::Html(t) | Event::InlineHtml(t) => {
                if !t.trim_start().starts_with("<!--") {
                    self.empujar_estilo(|s| s.fg(tema::ATENUADO));
                    self.texto(t.trim_end());
                    self.estilos.pop();
                }
            }
            Event::InlineMath(t) | Event::DisplayMath(t) => {
                self.empujar_estilo(|s| s.add_modifier(Modifier::ITALIC));
                self.texto(&t);
                self.estilos.pop();
            }
            Event::FootnoteReference(t) => self.texto(&format!("[{t}]")),
        }
    }

    fn inicio(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => {
                if self.listas.is_empty() || self.vineta.is_none() {
                    self.linea_en_blanco();
                }
            }
            Tag::Heading { level, .. } => {
                self.linea_en_blanco();
                let color = match level {
                    HeadingLevel::H1 => tema::ACENTO,
                    HeadingLevel::H2 => tema::NOTA,
                    _ => Color::Rgb(229, 192, 123),
                };
                self.empujar_estilo(|s| s.fg(color).add_modifier(Modifier::BOLD));
                self.encabezado = Some((level, String::new()));
            }
            Tag::BlockQuote(_) => {
                self.linea_en_blanco();
                self.prefijos.push(self.sim.cita.to_string());
                self.empujar_estilo(|s| s.add_modifier(Modifier::ITALIC));
            }
            Tag::CodeBlock(kind) => {
                self.linea_en_blanco();
                let etiqueta = match kind {
                    CodeBlockKind::Fenced(e) => e.split_whitespace().next().unwrap_or("").to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                self.codigo = Some((etiqueta, String::new()));
            }
            Tag::List(inicio) => {
                if self.listas.is_empty() {
                    self.linea_en_blanco();
                } else if !self.actual.is_empty() {
                    self.cortar();
                }
                self.listas.push(inicio);
            }
            Tag::Item => {
                if !self.actual.is_empty() {
                    self.cortar();
                }
                let marca = match self.listas.last_mut() {
                    Some(Some(n)) => {
                        let m = format!("{n}. ");
                        *n += 1;
                        m
                    }
                    _ => self.sim.vineta.to_string(),
                };
                self.prefijos.push(" ".repeat(UnicodeWidthStr::width(marca.as_str())));
                self.vineta = Some(marca);
            }
            Tag::Emphasis => self.empujar_estilo(|s| s.add_modifier(Modifier::ITALIC)),
            Tag::Strong => self.empujar_estilo(|s| s.add_modifier(Modifier::BOLD)),
            Tag::Strikethrough => self.empujar_estilo(|s| s.add_modifier(Modifier::CROSSED_OUT)),
            Tag::Link { dest_url, .. } => {
                self.empujar_estilo(|s| s.fg(tema::NOTA).add_modifier(Modifier::UNDERLINED));
                self.enlace = Some((dest_url.to_string(), usize::MAX, 0));
            }
            Tag::Image { dest_url, .. } => {
                self.empujar_estilo(|s| s.fg(tema::ATENUADO));
                self.texto("[imagen: ");
                self.enlace = Some((dest_url.to_string(), usize::MAX, 0));
            }
            Tag::Table(alineaciones) => {
                self.linea_en_blanco();
                self.tabla = Some(Tabla { alineaciones, filas: vec![], celda: String::new(), encabezado: 0 });
            }
            Tag::TableHead | Tag::TableRow => {
                if let Some(t) = &mut self.tabla {
                    t.filas.push(vec![]);
                }
            }
            Tag::TableCell => {
                if let Some(t) = &mut self.tabla {
                    t.celda.clear();
                }
            }
            _ => {}
        }
    }

    fn fin(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                if !self.actual.is_empty() {
                    self.cortar();
                }
            }
            TagEnd::Heading(level) => {
                self.estilos.pop();
                if !self.actual.is_empty() {
                    self.cortar();
                }
                if let Some((_, titulo)) = self.encabezado.take() {
                    self.doc.encabezados.push((self.doc.lineas.len().saturating_sub(1), titulo));
                }
                match level {
                    HeadingLevel::H1 => self.regla(self.sim.h_grueso, Style::default().fg(tema::ACENTO)),
                    HeadingLevel::H2 => self.regla(self.sim.h, Style::default().fg(tema::NOTA)),
                    _ => {}
                }
            }
            TagEnd::BlockQuote(_) => {
                if !self.actual.is_empty() {
                    self.cortar();
                }
                self.prefijos.pop();
                self.estilos.pop();
            }
            TagEnd::CodeBlock => {
                if let Some((etiqueta, codigo)) = self.codigo.take() {
                    self.bloque_codigo(&etiqueta, &codigo);
                }
            }
            TagEnd::List(_) => {
                if !self.actual.is_empty() {
                    self.cortar();
                }
                self.listas.pop();
            }
            TagEnd::Item => {
                if !self.actual.is_empty() {
                    self.cortar();
                }
                if self.vineta.take().is_some() {
                    // Ítem vacío: igual se muestra la viñeta.
                }
                self.prefijos.pop();
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                self.estilos.pop();
            }
            TagEnd::Link => {
                self.cerrar_enlace();
                self.estilos.pop();
            }
            TagEnd::Image => {
                self.cerrar_enlace();
                self.texto("]");
                self.estilos.pop();
            }
            TagEnd::TableCell => {
                if let Some(t) = &mut self.tabla {
                    let celda = std::mem::take(&mut t.celda);
                    if let Some(f) = t.filas.last_mut() {
                        f.push(celda.trim().to_string());
                    }
                }
            }
            TagEnd::TableHead => {
                if let Some(t) = &mut self.tabla {
                    t.encabezado = t.filas.len();
                }
            }
            TagEnd::Table => {
                if let Some(t) = self.tabla.take() {
                    self.tabla(t);
                }
            }
            _ => {}
        }
    }
}

pub fn renderizar(fuente: &str, ancho: u16, sim: &Simbolos) -> Documento {
    let ancho = (ancho as usize).max(20);
    let mut r = R {
        ancho,
        sim,
        doc: Documento::default(),
        actual: vec![],
        col: 0,
        estilos: vec![Style::default()],
        prefijos: vec![],
        vineta: None,
        listas: vec![],
        codigo: None,
        tabla: None,
        enlace: None,
        encabezado: None,
    };
    let opciones = Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH;
    for e in Parser::new_ext(fuente, opciones) {
        r.evento(e);
    }
    if !r.actual.is_empty() {
        r.cortar();
    }
    // Sin línea en blanco inicial.
    while r.doc.lineas.first().is_some_and(|l| l.width() == 0) {
        r.doc.lineas.remove(0);
        for e in &mut r.doc.enlaces {
            e.linea = e.linea.saturating_sub(1);
        }
        for h in &mut r.doc.encabezados {
            h.0 = h.0.saturating_sub(1);
        }
    }
    r.doc
}

/// Texto plano de una línea (pruebas, copiar).
pub fn texto_linea(l: &Line) -> String {
    l.spans.iter().map(|s| s.content.as_ref()).collect()
}

/// Un destino de enlace es local si no tiene esquema (http:, mailto:...).
pub fn es_enlace_local(destino: &str) -> bool {
    !destino.contains("://") && !destino.starts_with("mailto:") && !destino.starts_with('#')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texto(doc: &Documento) -> Vec<String> {
        doc.lineas.iter().map(texto_linea).collect()
    }

    #[test]
    fn encabezados_y_enfasis() {
        let d = renderizar("# Título\n\nTexto con **negrita** y *cursiva*.\n", 30, &Simbolos::unicode());
        let t = texto(&d);
        assert_eq!(t[0], "Título");
        assert!(t[1].starts_with("━━━━"));
        assert_eq!(t[3], "Texto con negrita y cursiva.");
        let negrita = d.lineas[3].spans.iter().find(|s| s.content == "negrita").unwrap();
        assert!(negrita.style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(d.encabezados, vec![(0, "Título".to_string())]);
    }

    #[test]
    fn ajuste_de_linea_y_citas() {
        let d = renderizar("> una cita bastante larga que debe partirse en varias líneas\n", 24, &Simbolos::unicode());
        let t = texto(&d);
        assert!(t.len() >= 3, "{t:?}");
        assert!(t.iter().all(|l| l.starts_with("│ ")), "{t:?}");
        assert!(t.iter().all(|l| UnicodeWidthStr::width(l.as_str()) <= 24), "{t:?}");
    }

    #[test]
    fn listas_y_tareas() {
        let d = renderizar("- uno\n- [x] hecho\n- [ ] pendiente\n\n1. a\n2. b\n", 40, &Simbolos::unicode());
        let t = texto(&d);
        assert!(t.contains(&"• uno".to_string()), "{t:?}");
        assert!(t.contains(&"• ☑ hecho".to_string()), "{t:?}");
        assert!(t.contains(&"• ☐ pendiente".to_string()), "{t:?}");
        assert!(t.contains(&"1. a".to_string()) && t.contains(&"2. b".to_string()), "{t:?}");
    }

    #[test]
    fn tabla_gfm() {
        let d = renderizar("| Func | Retorno |\n|:-----|--------:|\n| malloc | NULL |\n", 40, &Simbolos::unicode());
        let t = texto(&d);
        assert_eq!(t[0], "┌────────┬─────────┐");
        assert_eq!(t[1], "│ Func   │ Retorno │");
        assert_eq!(t[2], "├────────┼─────────┤");
        assert_eq!(t[3], "│ malloc │    NULL │");
        assert_eq!(t[4], "└────────┴─────────┘");
        let ascii = texto(&renderizar("| a |\n|---|\n| b |\n", 40, &Simbolos::ascii()));
        assert_eq!(ascii[0], "+---+");
    }

    #[test]
    fn codigo_y_enlaces() {
        let d = renderizar("Ver [la guía](guia.md).\n\n```c\nint x = 0;\n```\n", 40, &Simbolos::unicode());
        assert_eq!(d.enlaces.len(), 1);
        let e = &d.enlaces[0];
        assert_eq!(e.destino, "guia.md");
        assert_eq!((e.linea, e.ini), (0, 4));
        let t = texto(&d);
        assert!(t.iter().any(|l| l.contains("int x = 0;")), "{t:?}");
        let linea = d.lineas.iter().find(|l| texto_linea(l).contains("int")).unwrap();
        assert!(linea.spans.iter().any(|s| s.content == "int" && s.style.fg.is_some()));
        assert!(es_enlace_local("guia.md"));
        assert!(!es_enlace_local("https://x.org"));
    }
}
