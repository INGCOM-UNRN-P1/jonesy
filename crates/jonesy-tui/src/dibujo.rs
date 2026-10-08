//! Dibujo de la interfaz completa a partir del estado de [`App`].

use std::path::Path;

use jonesy_core::{Lenguaje, Severidad};
use jonesy_doc::{Base, Contexto, Funcion};
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::accion::{self, Accion};
use crate::app::{columnas_visuales, mismo_archivo, App, Foco, Lateral, Modal, Nivel, Pestana, ZonaEditor};
use crate::layout::{self, Modo, Vista};
use crate::markdown;
use crate::paleta;
use crate::resaltado;
use crate::tema::{self, estilo_clase};

pub fn dibujar(f: &mut Frame, app: &mut App) {
    let area = f.area();
    app.tam = (area.width, area.height);
    app.zonas = Default::default();
    let modo = app.modo();
    let pedido = layout::Pedido {
        area,
        hay_consigna: app.consigna.is_some() && (app.consigna_visible || modo == Modo::Compacto),
        quickdoc_visible: app.quickdoc_visible,
        quickdoc_posicion: app.quickdoc_posicion,
        panel_inferior: app.panel_inferior,
        alto_inferior: app.alto_inferior,
        ancho_consigna: app.ancho_consigna,
        vista: app.vista,
        lateral: app.lateral.is_some(),
        ancho_lateral: if app.lateral == Some(Lateral::CatScan) { 45 } else { 30 },
    };
    app.areas = layout::calcular(&pedido);
    let a = app.areas;
    let mut cursor: Option<(u16, u16)> = None;

    dibujar_menu(f, app, a.menu);
    if let Some(r) = a.consigna {
        dibujar_consigna(f, app, r);
    }
    if let Some(r) = a.editor {
        cursor = dibujar_area_editor(f, app, r);
    }
    if let Some(r) = a.inferior {
        if let Some(c) = dibujar_inferior(f, app, r) {
            cursor = Some(c);
        }
    }
    if let Some(r) = a.lateral {
        dibujar_lateral(f, app, r);
    }
    if let Some(r) = a.quickdoc {
        if a.quickdoc_flotante {
            f.render_widget(Clear, r);
        }
        dibujar_quickdoc(f, app, r);
    }
    dibujar_estado(f, app, a.estado);
    if app.modal.is_some() {
        cursor = dibujar_modal(f, app, area);
    }
    if let Some((x, y)) = cursor {
        f.set_cursor_position(Position { x, y });
    }
}

fn bloque<'a>(titulo: impl Into<Line<'a>>, foco: bool) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(if foco { tema::ACENTO } else { tema::ATENUADO }))
        .title(titulo)
}

fn bloque_app<'a>(app: &App, titulo: impl Into<Line<'a>>, foco: bool) -> Block<'a> {
    let b = bloque(titulo, foco);
    if app.sim.ascii {
        b.border_set(ratatui::symbols::border::Set {
            top_left: "+",
            top_right: "+",
            bottom_left: "+",
            bottom_right: "+",
            vertical_left: "|",
            vertical_right: "|",
            horizontal_top: "-",
            horizontal_bottom: "-",
        })
    } else {
        b
    }
}

// ------------------------------------------------------------------ barra de menú

fn dibujar_menu(f: &mut Frame, app: &mut App, r: Rect) {
    let fondo = Style::default().bg(tema::FONDO_BARRA);
    f.render_widget(Block::default().style(fondo), r);
    let menus = accion::menus(app.examen.is_some());
    let abierto = match app.modal {
        Some(Modal::Menu { indice, .. }) => Some(indice),
        _ => None,
    };
    let mut x = r.x + 1;
    for (i, (nombre, _)) in menus.iter().enumerate() {
        let w = nombre.chars().count() as u16 + 2;
        if x + w > r.x + r.width {
            break;
        }
        let estilo = if abierto == Some(i) { Style::default().bg(tema::ACENTO).fg(Color::Black) } else { fondo };
        let zona = Rect::new(x, r.y, w, 1);
        f.render_widget(Paragraph::new(format!(" {nombre} ")).style(estilo), zona);
        app.zonas.menu.push((zona, i));
        x += w;
    }
    // Derecha: cronómetro y entrega en examen, o el nombre del proyecto.
    let mut derecha: Vec<Span> = vec![];
    let mut boton: Option<(String, Accion)> = None;
    if let Some(e) = &app.examen {
        if e.estado.entregado {
            derecha.push(Span::styled(" ENTREGADO ", Style::default().bg(tema::OK).fg(Color::Black).add_modifier(Modifier::BOLD)));
        } else {
            if let Some(s) = e.estado.restante_s(jonesy_core::ahora_unix()) {
                let (h, m, seg) = (s.max(0) / 3600, (s.max(0) % 3600) / 60, s.max(0) % 60);
                let color = if s <= 300 { tema::ERROR } else if s <= 900 { tema::ADVERTENCIA } else { tema::OK };
                derecha.push(Span::styled(format!(" {} {h}:{m:02}:{seg:02} ", app.sim.reloj), fondo.fg(color).add_modifier(Modifier::BOLD)));
            }
            boton = Some((if e.entregando { "[ Entregando… ]".into() } else { "[ Entregar Examen ]".into() }, Accion::Entregar));
        }
    } else {
        let nombre = app.proyecto.raiz.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        derecha.push(Span::styled(format!(" jonesy · {nombre} "), fondo.fg(tema::ATENUADO)));
    }
    let ancho_boton = boton.as_ref().map(|(t, _)| t.chars().count() as u16 + 1).unwrap_or(0);
    let ancho_der: u16 = derecha.iter().map(|s| s.width() as u16).sum::<u16>() + ancho_boton;
    if ancho_der < r.width.saturating_sub(x - r.x) {
        let zona = Rect::new(r.x + r.width - ancho_der, r.y, ancho_der - ancho_boton, 1);
        f.render_widget(Paragraph::new(Line::from(derecha)).style(fondo), zona);
        if let Some((t, a)) = boton {
            let zb = Rect::new(r.x + r.width - ancho_boton, r.y, ancho_boton - 1, 1);
            f.render_widget(Paragraph::new(t).style(Style::default().bg(tema::ADVERTENCIA).fg(Color::Black).add_modifier(Modifier::BOLD)), zb);
            app.zonas.botones.push((zb, a));
        }
    }
}

// ------------------------------------------------------------------ consigna

fn dibujar_consigna(f: &mut Frame, app: &mut App, r: Rect) {
    let foco = app.foco == Foco::Consigna;
    let titulo = if app.examen.is_some() { " Consigna (solo lectura) " } else { " Consigna " };
    let b = bloque_app(app, titulo, foco);
    let interior = b.inner(r);
    f.render_widget(b, r);
    let sim = app.sim.clone();
    let Some(c) = &mut app.consigna else { return };
    if c.doc.as_ref().is_none_or(|(w, _)| *w != interior.width) {
        c.doc = Some((interior.width, markdown::renderizar(&c.fuente, interior.width, &sim)));
    }
    let (_, doc) = c.doc.as_ref().unwrap();
    c.desplazamiento = c.desplazamiento.min(doc.lineas.len().saturating_sub(1));
    let lineas: Vec<Line> = doc.lineas.iter().skip(c.desplazamiento).take(interior.height as usize).cloned().collect();
    f.render_widget(Paragraph::new(lineas), interior);
}

// ------------------------------------------------------------------ editor

fn dibujar_area_editor(f: &mut Frame, app: &mut App, r: Rect) -> Option<(u16, u16)> {
    // Fila de pestañas de buffers.
    let (tabs, cuerpo) = (Rect { height: 1, ..r }, Rect { y: r.y + 1, height: r.height.saturating_sub(1), ..r });
    dibujar_pestanas_buffers(f, app, tabs);
    let md = app.buf().buf.lenguaje == Lenguaje::Markdown;
    if app.preview_markdown && md && cuerpo.width >= 40 {
        let mitad = cuerpo.width / 2;
        let izq = Rect { width: mitad, ..cuerpo };
        let der = Rect { x: cuerpo.x + mitad, width: cuerpo.width - mitad, ..cuerpo };
        let c = dibujar_editor(f, app, izq);
        dibujar_preview(f, app, der);
        c
    } else {
        dibujar_editor(f, app, cuerpo)
    }
}

fn dibujar_pestanas_buffers(f: &mut Frame, app: &mut App, r: Rect) {
    f.render_widget(Block::default().style(Style::default().bg(tema::FONDO_LINEA)), r);
    let mut x = r.x;
    for (i, b) in app.buffers.iter().enumerate() {
        let marca = if b.buf.modificado() { " ●" } else if b.buf.solo_lectura { " 🔒" } else { "" };
        let marca = if app.sim.ascii && !marca.is_empty() { if b.buf.modificado() { " *" } else { " RO" } } else { marca };
        let texto = format!(" {}{} ", b.buf.nombre(), marca);
        let w = UnicodeWidthStr::width(texto.as_str()) as u16;
        if x + w > r.x + r.width {
            break;
        }
        let estilo = if i == app.activo {
            Style::default().bg(tema::FONDO_SELECCION).fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().bg(tema::FONDO_LINEA).fg(tema::ATENUADO)
        };
        let zona = Rect::new(x, r.y, w, 1);
        f.render_widget(Paragraph::new(texto).style(estilo), zona);
        app.zonas.pestanas_buffers.push((zona, i));
        x += w + 1;
    }
}

fn dibujar_editor(f: &mut Frame, app: &mut App, r: Rect) -> Option<(u16, u16)> {
    if r.height == 0 || r.width < 8 {
        return None;
    }
    let foco = app.foco == Foco::Editor && app.modal.is_none();
    let activo = app.activo;
    let ruta = app.buffers[activo].buf.ruta.clone();
    let lineas_total = app.buffers[activo].buf.lineas();
    let digitos = lineas_total.max(1).to_string().len().max(3) as u16;
    let ancho_gutter = 1 + digitos + 2;
    let gutter = Rect { width: ancho_gutter.min(r.width), ..r };
    let texto = Rect { x: r.x + gutter.width, width: r.width - gutter.width, ..r };
    app.zonas.editor = Some(ZonaEditor { texto, gutter });

    // Ubicación del depurador y breakpoints de este archivo.
    let linea_depurador = app.depuracion.as_ref().and_then(|d| d.ubicacion.clone()).and_then(|(a, l)| {
        ruta.as_ref().is_some_and(|r| mismo_archivo(&a, r)).then_some(l - 1)
    });
    let bps: Vec<usize> = app
        .breakpoints
        .iter()
        .filter(|(a, _)| ruta.as_ref().is_some_and(|r| mismo_archivo(a, r)))
        .map(|(_, l)| l - 1)
        .collect();
    let diags: Vec<(usize, Severidad, String)> = match &ruta {
        Some(p) => app
            .diagnosticos()
            .into_iter()
            .filter(|d| d.archivo.as_deref().is_some_and(|a| mismo_archivo(a, p)))
            .map(|d| {
                let codigo = d.codigo.as_deref().map(|c| format!(" [{c}]")).unwrap_or_default();
                (d.linea.saturating_sub(1), d.severidad, format!("{}{codigo}", d.mensaje))
            })
            .collect(),
        None => vec![],
    };
    let mouse = app.desplazado_por_mouse;
    let sim = app.sim.clone();
    let ba = &mut app.buffers[activo];
    let b = &mut ba.buf;
    let alto = texto.height as usize;
    let (cl, cc) = b.linea_columna();
    let cursor_chars: Vec<char> = b.linea_texto(cl).chars().collect();
    let cursor_vis = columnas_visuales(&cursor_chars)[cc.min(cursor_chars.len())];
    if !mouse {
        if cl < b.desplazamiento {
            b.desplazamiento = cl;
        } else if cl >= b.desplazamiento + alto {
            b.desplazamiento = cl + 1 - alto;
        }
        let ancho = texto.width as usize;
        if cursor_vis < b.desplazamiento_h {
            b.desplazamiento_h = cursor_vis;
        } else if cursor_vis >= b.desplazamiento_h + ancho.saturating_sub(1) {
            b.desplazamiento_h = cursor_vis + 2 - ancho.max(2);
        }
    }
    b.desplazamiento = b.desplazamiento.min(lineas_total.saturating_sub(1));
    let desp = b.desplazamiento;
    let desp_h = b.desplazamiento_h;
    let seleccion = b.seleccion();
    let version = b.version;
    let lenguaje = b.lenguaje;
    let mut estado = {
        let bref = &*b;
        ba.cache.estado_en(version, lenguaje, desp, |i| bref.linea_texto(i).chars().collect())
    };
    let b = &ba.buf;
    let buf = f.buffer_mut();
    for fila in 0..alto {
        let y = texto.y + fila as u16;
        let linea = desp + fila;
        // Fondo de la línea.
        let es_actual = linea == cl;
        let es_depurador = linea_depurador == Some(linea);
        let fondo = if es_depurador {
            Color::Rgb(70, 60, 20)
        } else if es_actual && foco {
            tema::FONDO_LINEA
        } else {
            Color::Reset
        };
        for x in r.x..r.x + r.width {
            buf[(x, y)].set_symbol(" ").set_style(Style::default().bg(fondo));
        }
        if linea >= lineas_total {
            buf[(gutter.x + 1, y)].set_symbol("~").set_fg(tema::ATENUADO);
            continue;
        }
        // Gutter: breakpoint, número, marca de diagnóstico.
        if bps.contains(&linea) {
            buf[(gutter.x, y)].set_symbol(sim.breakpoint).set_fg(tema::ERROR);
        }
        if es_depurador {
            buf[(gutter.x, y)].set_symbol(sim.linea_actual).set_fg(tema::ADVERTENCIA);
        }
        let num = format!("{:>w$}", linea + 1, w = digitos as usize);
        let estilo_num = if es_actual { Style::default().fg(Color::White) } else { Style::default().fg(tema::ATENUADO) };
        buf.set_string(gutter.x + 1, y, &num, estilo_num);
        let peor = diags.iter().filter(|d| d.0 == linea).map(|d| d.1).min();
        if let Some(s) = peor {
            buf[(gutter.x + 1 + digitos, y)].set_symbol("▎").set_fg(tema::color_severidad(s));
        }
        // Texto con resaltado y selección.
        let chars: Vec<char> = b.linea_texto(linea).chars().collect();
        let tramos = resaltado::resaltar(lenguaje, &chars, &mut estado);
        let cols = columnas_visuales(&chars);
        let inicio_linea = b.linea_columna_a_pos(linea, 0);
        let mut t = 0;
        for (i, &c) in chars.iter().enumerate() {
            let vis = cols[i];
            if vis < desp_h {
                continue;
            }
            let x = vis - desp_h;
            if x >= texto.width as usize {
                break;
            }
            while t < tramos.len() && tramos[t].1 <= i {
                t += 1;
            }
            let mut estilo = Style::default().bg(fondo);
            if t < tramos.len() && tramos[t].0 <= i {
                estilo = estilo.patch(estilo_clase(tramos[t].2));
            }
            let pos = inicio_linea + i;
            if seleccion.is_some_and(|(a, z)| pos >= a && pos < z) {
                estilo = estilo.bg(tema::FONDO_SELECCION);
            }
            let celda = &mut buf[(texto.x + x as u16, y)];
            if c == '\t' {
                celda.set_symbol(" ").set_style(estilo);
            } else {
                let mut s = [0u8; 4];
                celda.set_symbol(c.encode_utf8(&mut s)).set_style(estilo);
            }
        }
        // Selección que incluye el salto de línea.
        if seleccion.is_some_and(|(a, z)| inicio_linea + chars.len() >= a && inicio_linea + chars.len() < z) {
            let vis = cols[chars.len()];
            if vis >= desp_h && vis - desp_h < texto.width as usize {
                buf[(texto.x + (vis - desp_h) as u16, y)].set_bg(tema::FONDO_SELECCION);
            }
        }
        // Texto virtual: el diagnóstico más grave al final de la línea (RF-3.2).
        if let Some((_, sev, msg)) = diags.iter().filter(|d| d.0 == linea).min_by_key(|d| d.1) {
            let fin = cols[chars.len()].saturating_sub(desp_h) + 3;
            if fin + 4 < texto.width as usize {
                let disponible = texto.width as usize - fin;
                let icono = match sev {
                    Severidad::Error => "■ ",
                    Severidad::Advertencia => "▲ ",
                    _ => "● ",
                };
                let icono = if sim.ascii { "<< " } else { icono };
                let s: String = format!("{icono}{msg}").chars().take(disponible).collect();
                buf.set_string(
                    texto.x + fin as u16,
                    y,
                    s,
                    Style::default().fg(tema::color_severidad(*sev)).bg(fondo).add_modifier(Modifier::ITALIC),
                );
            }
        }
    }
    let _ = estado;
    if foco && cl >= desp && cl < desp + alto && cursor_vis >= desp_h {
        let x = texto.x + (cursor_vis - desp_h) as u16;
        if x < texto.x + texto.width {
            return Some((x, texto.y + (cl - desp) as u16));
        }
    }
    None
}

fn dibujar_preview(f: &mut Frame, app: &mut App, r: Rect) {
    let b = bloque_app(app, " Vista previa ", false);
    let interior = b.inner(r);
    f.render_widget(b, r);
    app.zonas.preview = Some(interior);
    let sim = app.sim.clone();
    let ba = &mut app.buffers[app.activo];
    let version = ba.buf.version;
    let vencido = ba.ultimo_cambio.elapsed() >= crate::app::DEBOUNCE_PREVIEW;
    let desactualizado = ba.preview.as_ref().is_none_or(|(v, w, _)| *v != version || *w != interior.width);
    // Debounce (RF-8.2): no se rerenderiza mientras se tipea.
    if desactualizado && (vencido || ba.preview.is_none()) {
        let doc = markdown::renderizar(&ba.buf.contenido(), interior.width, &sim);
        ba.preview = Some((version, interior.width, doc));
    }
    if let Some((_, _, doc)) = &ba.preview {
        ba.preview_desplazamiento = ba.preview_desplazamiento.min(doc.lineas.len().saturating_sub(1));
        let lineas: Vec<Line> = doc.lineas.iter().skip(ba.preview_desplazamiento).take(interior.height as usize).cloned().collect();
        f.render_widget(Paragraph::new(lineas), interior);
    }
}

// ------------------------------------------------------------------ quick-doc

fn funcion_doc<'a>(app: &'a App, nombre: &str) -> Option<std::borrow::Cow<'a, Funcion>> {
    if let Some(f) = Base::embebida().funcion(nombre) {
        return Some(std::borrow::Cow::Borrowed(f));
    }
    app.funciones_usuario.as_ref().and_then(|(_, _, v)| v.iter().find(|f| f.nombre == nombre)).map(|f| std::borrow::Cow::Owned(f.clone()))
}

pub fn lineas_quickdoc(app: &mut App, ancho: u16) -> (String, Vec<Line<'static>>) {
    let activo = app.activo;
    let b = &app.buffers[activo].buf;
    if b.lenguaje != Lenguaje::C {
        return (" quick-doc ".into(), vec![Line::styled("Ayuda de funciones de C: abrí un archivo .c o .h.", Style::default().fg(tema::ATENUADO))]);
    }
    let version = b.version;
    if app.funciones_usuario.as_ref().is_none_or(|(i, v, _)| *i != activo || *v != version) {
        app.funciones_usuario = Some((activo, version, jonesy_doc::funciones_del_codigo(&b.contenido())));
    }
    let b = &app.buffers[activo].buf;
    // Ventana alrededor del cursor: el lexer no necesita el archivo entero.
    let ini = b.cursor.saturating_sub(4000);
    let fin = (b.cursor + 200).min(b.largo());
    let ventana: Vec<char> = b.texto.slice(ini..fin).chars().collect();
    let ctx = jonesy_doc::contexto(&ventana, b.cursor - ini);
    let (nombre, activo_param) = match &ctx {
        Contexto::Llamada { funcion, parametro } => (funcion.clone(), Some(*parametro)),
        Contexto::Identificador(id) => (id.clone(), None),
        Contexto::Nada => (String::new(), None),
    };
    let atenuado = Style::default().fg(tema::ATENUADO);
    if nombre.is_empty() {
        return (" quick-doc ".into(), vec![Line::styled("Posá el cursor sobre una función o escribí «(» para ver su firma.", atenuado)]);
    }
    if let Some(func) = funcion_doc(app, &nombre) {
        let mut l = vec![];
        // Firma con el parámetro activo resaltado (RF-9.1).
        let mut firma: Vec<Span<'static>> = vec![];
        let texto = func.firma.clone();
        match (activo_param, texto.find('('), texto.rfind(')')) {
            (Some(p), Some(a), Some(z)) if a < z && !func.parametros.is_empty() => {
                firma.push(Span::styled(texto[..=a].to_string(), Style::default().fg(tema::ACENTO)));
                let partes: Vec<&str> = texto[a + 1..z].split(',').collect();
                for (i, parte) in partes.iter().enumerate() {
                    let es_activo = i == p.min(partes.len() - 1) && !(texto.contains("...") && p >= partes.len() && i != partes.len() - 1);
                    let estilo = if es_activo {
                        Style::default().fg(tema::ADVERTENCIA).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                    } else {
                        Style::default()
                    };
                    firma.push(Span::styled(parte.to_string(), estilo));
                    if i + 1 < partes.len() {
                        firma.push(Span::raw(","));
                    }
                }
                firma.push(Span::styled(texto[z..].to_string(), Style::default().fg(tema::ACENTO)));
            }
            _ => firma.push(Span::styled(texto.clone(), Style::default().fg(tema::ACENTO).add_modifier(Modifier::BOLD))),
        }
        l.push(Line::from(firma));
        if !func.cabecera.is_empty() {
            l.push(Line::styled(format!("#include <{}>", func.cabecera.trim_matches(['<', '>'])), atenuado));
        }
        if !func.descripcion.is_empty() {
            l.push(Line::raw(func.descripcion.clone()));
        }
        if !func.retorno.is_empty() {
            l.push(Line::from(vec![Span::styled("Retorno: ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(func.retorno.clone())]));
        }
        for (i, p) in func.parametros.iter().enumerate() {
            let activo = activo_param.is_some_and(|a| a == i);
            let estilo = if activo { Style::default().fg(tema::ADVERTENCIA).add_modifier(Modifier::BOLD) } else { Style::default() };
            l.push(Line::from(vec![
                Span::styled(format!("  {} {}", p.tipo, p.nombre), estilo),
                Span::styled(if p.desc.is_empty() { String::new() } else { format!(" — {}", p.desc) }, if activo { estilo } else { atenuado }),
            ]));
        }
        for t in &func.trampas {
            l.push(Line::styled(format!("{} {t}", if app.sim.ascii { "!" } else { "⚠" }), Style::default().fg(tema::ADVERTENCIA)));
        }
        if !func.reglas.is_empty() {
            l.push(Line::styled(format!("Reglas: {}", func.reglas.join(", ")), Style::default().fg(tema::ESTILO)));
        }
        let _ = ancho;
        return (format!(" {} ", func.nombre), l);
    }
    if let Some(t) = Base::embebida().tipo(&nombre) {
        let mut l = vec![
            Line::styled(t.nombre.clone(), Style::default().fg(tema::ACENTO).add_modifier(Modifier::BOLD)),
            Line::styled(format!("#include <{}>", t.cabecera.trim_matches(['<', '>'])), atenuado),
            Line::raw(t.descripcion.clone()),
        ];
        for tr in &t.trampas {
            l.push(Line::styled(format!("{} {tr}", if app.sim.ascii { "!" } else { "⚠" }), Style::default().fg(tema::ADVERTENCIA)));
        }
        return (format!(" {} ", t.nombre), l);
    }
    (" quick-doc ".into(), vec![Line::styled(format!("Sin documentación para «{nombre}»."), atenuado)])
}

fn dibujar_quickdoc(f: &mut Frame, app: &mut App, r: Rect) {
    let interior_w = r.width.saturating_sub(2);
    let (titulo, lineas) = lineas_quickdoc(app, interior_w);
    let b = bloque_app(app, titulo, app.foco == Foco::Doc);
    let interior = b.inner(r);
    f.render_widget(b, r);
    f.render_widget(Paragraph::new(lineas).wrap(Wrap { trim: false }), interior);
}

// ------------------------------------------------------------------ panel inferior

fn dibujar_inferior(f: &mut Frame, app: &mut App, r: Rect) -> Option<(u16, u16)> {
    let foco = app.foco == Foco::Inferior && app.modal.is_none();
    // Pestañas en la fila superior (sobre el borde).
    let fila = Rect { height: 1, ..r };
    f.render_widget(Block::default().style(Style::default().bg(tema::FONDO_BARRA)), fila);
    let mut x = r.x + 1;
    for p in Pestana::TODAS {
        let mut titulo = p.titulo().to_string();
        match p {
            Pestana::Problemas => {
                let n = app.diagnosticos().len();
                if n > 0 {
                    titulo = format!("{titulo} ({n})");
                }
            }
            Pestana::Depurador if app.depuracion.is_none() => continue,
            Pestana::Ejecucion if app.ejecucion.is_none() => continue,
            _ => {}
        }
        let w = titulo.chars().count() as u16 + 2;
        if x + w > r.x + r.width {
            break;
        }
        let estilo = if p == app.pestana {
            Style::default().bg(if foco { tema::ACENTO } else { tema::FONDO_SELECCION }).fg(if foco { Color::Black } else { Color::White }).add_modifier(Modifier::BOLD)
        } else {
            Style::default().bg(tema::FONDO_BARRA).fg(tema::ATENUADO)
        };
        let zona = Rect::new(x, r.y, w, 1);
        f.render_widget(Paragraph::new(format!(" {titulo} ")).style(estilo), zona);
        app.zonas.pestanas_inferior.push((zona, p));
        x += w + 1;
    }
    let cuerpo = Rect { y: r.y + 1, height: r.height.saturating_sub(1), ..r };
    match app.pestana {
        Pestana::Problemas => {
            dibujar_problemas(f, app, cuerpo);
            None
        }
        Pestana::Salida => {
            let total = app.salida.len();
            let alto = cuerpo.height as usize;
            app.salida_desplazamiento = app.salida_desplazamiento.min(total.saturating_sub(alto));
            let fin = total - app.salida_desplazamiento;
            let ini = fin.saturating_sub(alto);
            let lineas: Vec<Line> = app.salida[ini..fin]
                .iter()
                .map(|(t, e)| Line::styled(t.clone(), if *e { Style::default().fg(tema::ERROR) } else { Style::default() }))
                .collect();
            f.render_widget(Paragraph::new(lineas), cuerpo);
            None
        }
        Pestana::Terminal | Pestana::Ejecucion => {
            app.zonas.deck = Some(cuerpo);
            let es_terminal = app.pestana == Pestana::Terminal;
            let deck = if es_terminal { app.terminal.as_mut() } else { app.ejecucion.as_mut() };
            let Some(d) = deck else {
                f.render_widget(Paragraph::new("Alt+T abre la terminal.").style(Style::default().fg(tema::ATENUADO)), cuerpo);
                return None;
            };
            let (filas, cols) = d.con_pantalla(|s| s.size());
            if (filas, cols) != (cuerpo.height, cuerpo.width) && cuerpo.height > 1 && cuerpo.width > 1 {
                d.redimensionar(cuerpo.height, cuerpo.width);
            }
            d.tomar_pendiente();
            let vivo = d.vivo();
            let c = d.con_pantalla(|s| crate::vt::dibujar_pantalla(s, cuerpo, f.buffer_mut(), foco && vivo));
            if !vivo {
                let aviso = if es_terminal { " [terminal cerrada: Alt+T abre otra] " } else { " [terminado] " };
                let w = aviso.chars().count() as u16;
                if cuerpo.width > w {
                    f.buffer_mut().set_string(cuerpo.x + cuerpo.width - w, r.y, aviso, Style::default().fg(tema::ATENUADO).bg(tema::FONDO_BARRA));
                }
            }
            c
        }
        Pestana::Depurador => dibujar_depurador(f, app, cuerpo, foco),
    }
}

fn icono_severidad(s: Severidad, ascii: bool) -> &'static str {
    match (s, ascii) {
        (Severidad::Error, false) => "✖",
        (Severidad::Advertencia, false) => "▲",
        (Severidad::Nota, false) => "●",
        (Severidad::Estilo, false) => "◆",
        (Severidad::Error, true) => "E",
        (Severidad::Advertencia, true) => "W",
        (Severidad::Nota, true) => "N",
        (Severidad::Estilo, true) => "S",
    }
}

fn dibujar_problemas(f: &mut Frame, app: &mut App, r: Rect) {
    let diags: Vec<_> = app.diagnosticos().into_iter().cloned().collect();
    if diags.is_empty() {
        f.render_widget(Paragraph::new(" Sin problemas. F6 compila; al guardar corre ripley.").style(Style::default().fg(tema::ATENUADO)), r);
        return;
    }
    app.sel_problema = app.sel_problema.min(diags.len() - 1);
    let alto = r.height as usize;
    let base = app.sel_problema.saturating_sub(alto.saturating_sub(1));
    app.zonas.lista_inferior = Some((r, base));
    let lineas: Vec<Line> = diags
        .iter()
        .enumerate()
        .skip(base)
        .take(alto)
        .map(|(i, d)| {
            let ubic = format!(
                "{}:{}:{}",
                d.archivo.as_deref().map(|a| app.proyecto.relativa(a)).unwrap_or_default(),
                d.linea,
                d.columna
            );
            let fondo = if i == app.sel_problema { tema::FONDO_SELECCION } else { Color::Reset };
            let origen = match d.origen {
                jonesy_core::Origen::Compilador => "gcc",
                jonesy_core::Origen::Ripley => "ripley",
                jonesy_core::Origen::Plugin => "plugin",
            };
            Line::from(vec![
                Span::styled(format!(" {} ", icono_severidad(d.severidad, app.sim.ascii)), Style::default().fg(tema::color_severidad(d.severidad)).bg(fondo)),
                Span::styled(format!("{ubic} "), Style::default().fg(tema::ATENUADO).bg(fondo)),
                Span::styled(d.mensaje.clone(), Style::default().bg(fondo)),
                Span::styled(d.codigo.as_deref().map(|c| format!(" [{c}]")).unwrap_or_default(), Style::default().fg(tema::ESTILO).bg(fondo)),
                Span::styled(format!(" ({origen})"), Style::default().fg(tema::ATENUADO).bg(fondo)),
            ])
        })
        .collect();
    f.render_widget(Paragraph::new(lineas), r);
}

fn dibujar_depurador(f: &mut Frame, app: &mut App, r: Rect, foco: bool) -> Option<(u16, u16)> {
    let sim_ascii = app.sim.ascii;
    let Some(d) = &mut app.depuracion else { return None };
    // Barra de botones (RF-4.1).
    let botones: [(&str, &str, Accion); 5] = [
        ("▶ Continuar", "> Continuar", Accion::Depurar),
        ("↷ Paso", "Paso", Accion::Paso),
        ("↓ Entrar", "Entrar", Accion::Entrar),
        ("↑ Salir", "Salir", Accion::SalirFuncion),
        ("■ Detener", "Detener", Accion::Detener),
    ];
    let mut x = r.x;
    for (u, a_, accion) in botones {
        let t = format!("[{}]", if sim_ascii { a_ } else { u });
        let w = UnicodeWidthStr::width(t.as_str()) as u16;
        if x + w > r.x + r.width {
            break;
        }
        let zona = Rect::new(x, r.y, w, 1);
        f.render_widget(Paragraph::new(t).style(Style::default().fg(tema::ACENTO).add_modifier(Modifier::BOLD)), zona);
        app.zonas.botones_depurador.push((zona, accion));
        x += w + 1;
    }
    let estado = format!(" {} ", d.estado);
    if x + estado.chars().count() as u16 <= r.x + r.width {
        f.buffer_mut().set_string(x, r.y, &estado, Style::default().fg(tema::ATENUADO));
    }
    let cuerpo = Rect { y: r.y + 1, height: r.height.saturating_sub(1), ..r };
    let mitad = cuerpo.width / 2;
    let izq = Rect { width: mitad, ..cuerpo };
    let der = Rect { x: cuerpo.x + mitad, width: cuerpo.width - mitad, ..cuerpo };
    let bp = bloque(" programa (E/S) ", foco && !d.foco_consola);
    let ip = bp.inner(izq);
    f.render_widget(bp, izq);
    if d.tam_pty != (ip.height, ip.width) && ip.height > 1 && ip.width > 1 {
        d.pty.redimensionar(ip.height, ip.width);
        d.tam_pty = (ip.height, ip.width);
    }
    d.pty.tomar_pendiente();
    let c1 = d.pty.con_pantalla(|s| crate::vt::dibujar_pantalla(s, ip, f.buffer_mut(), foco && !d.foco_consola));
    let bc = bloque(" consola gdb (Tab alterna) ", foco && d.foco_consola);
    let ic = bc.inner(der);
    f.render_widget(bc, der);
    let alto = ic.height.saturating_sub(1) as usize;
    let ini = d.consola.len().saturating_sub(alto);
    let lineas: Vec<Line> = d.consola[ini..].iter().map(|l| Line::raw(l.clone())).collect();
    f.render_widget(Paragraph::new(lineas), Rect { height: ic.height.saturating_sub(1), ..ic });
    let entrada = format!("(gdb) {}", d.entrada);
    let y = ic.y + ic.height.saturating_sub(1);
    f.buffer_mut().set_string(ic.x, y, &entrada, Style::default().fg(tema::ACENTO));
    if foco && d.foco_consola {
        Some((ic.x + (UnicodeWidthStr::width(entrada.as_str()) as u16).min(ic.width.saturating_sub(1)), y))
    } else {
        c1
    }
}

// ------------------------------------------------------------------ panel lateral

fn dibujar_lateral(f: &mut Frame, app: &mut App, r: Rect) {
    let foco = app.foco == Foco::Lateral;
    match app.lateral {
        Some(Lateral::Git) => dibujar_git(f, app, r, foco),
        Some(Lateral::Reglas) => dibujar_reglas(f, app, r, foco),
        Some(Lateral::Variables) => dibujar_variables(f, app, r, foco),
        Some(Lateral::CatScan) => dibujar_catscan(f, app, r, foco),
        None => {}
    }
}

fn dibujar_git(f: &mut Frame, app: &mut App, r: Rect, foco: bool) {
    let b = bloque_app(app, " Git ", foco);
    let i = b.inner(r);
    f.render_widget(b, r);
    let Some(e) = app.git_estado.clone() else {
        f.render_widget(Paragraph::new("El proyecto no está en un repositorio git.").style(Style::default().fg(tema::ATENUADO)).wrap(Wrap { trim: true }), i);
        return;
    };
    let mut cabecera = vec![Line::from(vec![
        Span::styled(format!("{}{}", app.sim.rama, e.rama.clone().unwrap_or("(sin rama)".into())), Style::default().fg(tema::ACENTO).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!(
                "{}{}",
                if e.adelante > 0 { format!("  ↑{}", e.adelante) } else { String::new() },
                if e.atras > 0 { format!("  ↓{}", e.atras) } else { String::new() }
            ),
            Style::default().fg(tema::ADVERTENCIA),
        ),
    ])];
    let ayuda = if app.git.log.is_some() { "Enter: ver commit · l: volver" } else { "Espacio stage · a/u todo · c commit · l log · Enter diff" };
    cabecera.push(Line::styled(ayuda, Style::default().fg(tema::ATENUADO)));
    let alto_cab = cabecera.len() as u16;
    f.render_widget(Paragraph::new(cabecera).wrap(Wrap { trim: true }), Rect { height: alto_cab.min(i.height), ..i });
    let lista = Rect { y: i.y + alto_cab + 1, height: i.height.saturating_sub(alto_cab + 1), ..i };
    let alto = lista.height as usize;
    if let Some(log) = &app.git.log {
        let sel = app.git.sel_log.min(log.len().saturating_sub(1));
        let base = sel.saturating_sub(alto.saturating_sub(1));
        app.zonas.lista_lateral = Some((lista, base));
        let lineas: Vec<Line> = log
            .iter()
            .enumerate()
            .skip(base)
            .take(alto)
            .map(|(n, c)| {
                let fondo = if n == sel { tema::FONDO_SELECCION } else { Color::Reset };
                Line::from(vec![
                    Span::styled(format!("{} ", c.hash), Style::default().fg(tema::ADVERTENCIA).bg(fondo)),
                    Span::styled(c.resumen.clone(), Style::default().bg(fondo)),
                    Span::styled(format!(" · {}", c.cuando), Style::default().fg(tema::ATENUADO).bg(fondo)),
                ])
            })
            .collect();
        f.render_widget(Paragraph::new(lineas), lista);
        return;
    }
    if e.entradas.is_empty() {
        f.render_widget(Paragraph::new("Árbol de trabajo limpio.").style(Style::default().fg(tema::OK)), lista);
        return;
    }
    let sel = app.git.sel.min(e.entradas.len() - 1);
    let base = sel.saturating_sub(alto.saturating_sub(1));
    app.zonas.lista_lateral = Some((lista, base));
    let lineas: Vec<Line> = e
        .entradas
        .iter()
        .enumerate()
        .skip(base)
        .take(alto)
        .map(|(n, en)| {
            let fondo = if n == sel { tema::FONDO_SELECCION } else { Color::Reset };
            let (marca, color) = if en.en_stage() && !en.sin_stage() {
                ("[S]", tema::OK)
            } else if en.en_stage() {
                ("[±]", tema::ADVERTENCIA)
            } else {
                ("[ ]", tema::ATENUADO)
            };
            Line::from(vec![
                Span::styled(format!("{marca} "), Style::default().fg(color).bg(fondo)),
                Span::styled(format!("{:<10} ", en.describir()), Style::default().fg(tema::ATENUADO).bg(fondo)),
                Span::styled(en.ruta.clone(), Style::default().bg(fondo)),
            ])
        })
        .collect();
    f.render_widget(Paragraph::new(lineas), lista);
}

fn dibujar_reglas(f: &mut Frame, app: &mut App, r: Rect, foco: bool) {
    let b = bloque_app(app, " Reglas (ripley) ", foco);
    let i = b.inner(r);
    f.render_widget(b, r);
    let grupos: Vec<(&'static str, Vec<jonesy_core::Diagnostico>)> =
        app.reglas_agrupadas().into_iter().map(|(c, v)| (c, v.into_iter().cloned().collect())).collect();
    if grupos.is_empty() {
        f.render_widget(
            Paragraph::new("Sin observaciones. ripley corre al guardar un archivo .c (F1 sobre una línea marcada explica la regla).")
                .style(Style::default().fg(tema::ATENUADO))
                .wrap(Wrap { trim: true }),
            i,
        );
        return;
    }
    // Árbol plano: encabezado de categoría + ítems; la selección cuenta sólo ítems.
    let total: usize = grupos.iter().map(|g| g.1.len()).sum();
    app.sel_regla = app.sel_regla.min(total.saturating_sub(1));
    let mut lineas = vec![];
    let mut n = 0;
    let mut fila_sel = 0;
    for (cat, v) in &grupos {
        lineas.push(Line::styled(format!("▾ {cat} ({})", v.len()), Style::default().fg(tema::ACENTO).add_modifier(Modifier::BOLD)));
        for d in v {
            let fondo = if n == app.sel_regla { tema::FONDO_SELECCION } else { Color::Reset };
            if n == app.sel_regla {
                fila_sel = lineas.len();
            }
            let archivo = d.archivo.as_deref().and_then(Path::file_name).map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
            lineas.push(Line::from(vec![
                Span::styled(format!("  {} ", icono_severidad(d.severidad, app.sim.ascii)), Style::default().fg(tema::color_severidad(d.severidad)).bg(fondo)),
                Span::styled(format!("{archivo}:{} ", d.linea), Style::default().fg(tema::ATENUADO).bg(fondo)),
                Span::styled(d.codigo.clone().unwrap_or_default(), Style::default().fg(tema::ESTILO).bg(fondo)),
                Span::styled(format!(" {}", d.mensaje), Style::default().bg(fondo)),
            ]));
            n += 1;
        }
    }
    let alto = i.height as usize;
    let desde = fila_sel.saturating_sub(alto.saturating_sub(1));
    f.render_widget(Paragraph::new(lineas.into_iter().skip(desde).take(alto).collect::<Vec<_>>()), i);
}

fn dibujar_variables(f: &mut Frame, app: &mut App, r: Rect, foco: bool) {
    let b = bloque_app(app, " Marcos y variables ", foco);
    let i = b.inner(r);
    f.render_widget(b, r);
    let Some(d) = &app.depuracion else {
        f.render_widget(Paragraph::new("F5 inicia la depuración.").style(Style::default().fg(tema::ATENUADO)), i);
        return;
    };
    let mut l: Vec<Line> = vec![Line::styled("Pila", Style::default().fg(tema::ACENTO).add_modifier(Modifier::BOLD))];
    for m in &d.marcos {
        let ubic = match (&m.archivo, m.linea) {
            (Some(a), Some(n)) => format!("{}:{n}", a.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default()),
            _ => m.direccion.clone(),
        };
        l.push(Line::from(vec![
            Span::styled(format!(" #{} ", m.nivel), Style::default().fg(tema::ATENUADO)),
            Span::styled(format!("{}()", m.funcion), Style::default().fg(tema::ACENTO)),
            Span::styled(format!(" {ubic}"), Style::default().fg(tema::ATENUADO)),
        ]));
    }
    l.push(Line::raw(""));
    l.push(Line::styled("Variables", Style::default().fg(tema::ACENTO).add_modifier(Modifier::BOLD)));
    for v in &d.variables {
        l.push(Line::from(vec![
            Span::styled(if v.argumento { " ⇢ " } else { "   " }, Style::default().fg(tema::ATENUADO)),
            Span::styled(format!("{} ", v.tipo), estilo_clase(resaltado::Clase::Tipo)),
            Span::styled(v.nombre.clone(), Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" = "),
            Span::styled(v.valor.clone(), estilo_clase(resaltado::Clase::Numero)),
        ]));
    }
    if d.traza.is_some() {
        l.push(Line::raw(""));
        l.push(Line::styled("Alt+M: vista de memoria (cat-scan)", Style::default().fg(tema::ATENUADO)));
    }
    f.render_widget(Paragraph::new(l).wrap(Wrap { trim: false }), i);
}

fn dibujar_catscan(f: &mut Frame, app: &mut App, r: Rect, foco: bool) {
    let sim = app.sim.clone();
    let b = bloque_app(app, " Memoria (cat-scan) ", foco);
    let i = b.inner(r);
    f.render_widget(b, r);
    let Some(d) = &app.depuracion else { return };
    let (Some(t), Some(p)) = (&d.traza, d.paso_traza) else {
        let msg = if d.traza.is_none() { "Esperando la traza de mem-trace…" } else { "Esta parada no figura en la traza de mem-trace." };
        f.render_widget(Paragraph::new(msg).style(Style::default().fg(tema::ATENUADO)), i);
        return;
    };
    let lienzo = jonesy_core::catscan::dibujar(&t.steps[p], i.width as usize);
    let buf = f.buffer_mut();
    for (fila, celdas) in lienzo.filas.iter().enumerate().take(i.height as usize) {
        for (col, c) in celdas.iter().enumerate().take(i.width as usize) {
            let mut s = [0u8; 4];
            let ch = sim.degradar(c.c);
            buf[(i.x + col as u16, i.y + fila as u16)].set_symbol(ch.encode_utf8(&mut s)).set_style(tema::tono_catscan(c.tono));
        }
    }
    let pie = format!(" paso {}/{} ", p + 1, t.steps.len());
    if i.width as usize > pie.len() && r.height > 1 {
        buf.set_string(r.x + r.width - pie.len() as u16 - 1, r.y + r.height - 1, &pie, Style::default().fg(tema::ATENUADO));
    }
}

// ------------------------------------------------------------------ barra de estado

fn dibujar_estado(f: &mut Frame, app: &mut App, r: Rect) {
    let fondo = Style::default().bg(tema::FONDO_BARRA);
    f.render_widget(Block::default().style(fondo), r);
    let b = &app.buf().buf;
    let (l, c) = b.linea_columna();
    let mut izq: Vec<Span> = vec![];
    if app.examen.is_some() {
        izq.push(Span::styled(" EXAMEN ", Style::default().bg(tema::ERROR).fg(Color::Black).add_modifier(Modifier::BOLD)));
    }
    if app.modo() == Modo::Compacto {
        let vista = match app.vista {
            Vista::Consigna => "F7 consigna",
            Vista::Codigo => "F8 código",
            Vista::Terminal => "F9 terminal",
            Vista::Doc => "F4 doc",
        };
        izq.push(Span::styled(format!(" {vista} "), Style::default().bg(tema::FONDO_SELECCION)));
    }
    izq.push(Span::styled(format!(" {}{} ", b.nombre(), if b.modificado() { "*" } else { "" }), fondo.add_modifier(Modifier::BOLD)));
    izq.push(Span::styled(format!("Ln {}, Col {} ", l + 1, c + 1), fondo.fg(tema::ATENUADO)));
    let tareas = app.tareas_en_curso();
    if !tareas.is_empty() {
        izq.push(Span::styled(format!(" {}… ", tareas.join(", ")), fondo.fg(tema::NOTA).add_modifier(Modifier::ITALIC)));
    }
    if let Some((m, n, _)) = &app.mensaje {
        let color = match n {
            Nivel::Info => tema::NOTA,
            Nivel::Ok => tema::OK,
            Nivel::Aviso => tema::ADVERTENCIA,
            Nivel::Error => tema::ERROR,
        };
        izq.push(Span::styled(format!(" {m}"), fondo.fg(color)));
    }
    // Derecha: git y botones (RF-2.1: compilar con un clic).
    let mut botones: Vec<(String, Accion)> = vec![
        ("[F6 Compilar]".into(), Accion::Compilar),
        ("[^F5 Ejecutar]".into(), Accion::Ejecutar),
        (if app.depuracion.is_some() { "[F5 Continuar]" } else { "[F5 Depurar]" }.into(), Accion::Depurar),
        ("[Terminal]".into(), Accion::Terminal),
    ];
    let git = app.git_estado.as_ref().map(|e| e.resumen()).unwrap_or_default();
    if app.repo.is_some() {
        botones.push((format!("[{} {}]", app.sim.rama, git.trim()), Accion::PanelGit));
    }
    let ancho_der: u16 = botones.iter().map(|(t, _)| UnicodeWidthStr::width(t.as_str()) as u16 + 1).sum();
    let ancho_izq = r.width.saturating_sub(ancho_der);
    f.render_widget(Paragraph::new(Line::from(izq)).style(fondo), Rect { width: ancho_izq, ..r });
    if ancho_der <= r.width {
        let mut x = r.x + ancho_izq;
        for (t, a) in botones {
            let w = UnicodeWidthStr::width(t.as_str()) as u16;
            let zona = Rect::new(x, r.y, w, 1);
            f.render_widget(Paragraph::new(t).style(fondo.fg(tema::ACENTO)), zona);
            app.zonas.botones.push((zona, a));
            x += w + 1;
        }
    }
}

// ------------------------------------------------------------------ modales

fn centrado(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 3, w, h)
}

fn dibujar_modal(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let ascii = app.sim.ascii;
    let Some(modal) = &app.modal else { return None };
    match modal {
        Modal::Paleta { filtro, sel, opciones } => {
            let titulos: Vec<&str> = opciones.iter().map(|o| o.titulo.as_str()).collect();
            let filtradas = paleta::filtrar(filtro, &titulos);
            let items: Vec<(String, String)> = filtradas.iter().map(|&i| (opciones[i].titulo.clone(), opciones[i].atajo.clone())).collect();
            let (filtro, sel) = (filtro.clone(), *sel);
            lista_modal(f, app, area, " Comandos ", &filtro, &items, sel)
        }
        Modal::Abrir { filtro, sel, archivos } => {
            let filtrados = paleta::filtrar(filtro, archivos);
            let items: Vec<(String, String)> = filtrados.iter().map(|&i| (archivos[i].clone(), String::new())).collect();
            let (filtro, sel) = (filtro.clone(), *sel);
            let titulo = if items.is_empty() && !filtro.is_empty() { " Abrir (Enter crea el archivo) " } else { " Abrir archivo " };
            lista_modal(f, app, area, titulo, &filtro, &items, sel)
        }
        Modal::Buscar { texto } | Modal::IrALinea { texto } => {
            let titulo = if matches!(modal, Modal::Buscar { .. }) { " Buscar (Enter siguiente · Esc cerrar) " } else { " Ir a la línea " };
            let base = app.zonas.editor.map(|z| z.texto).unwrap_or(area);
            let r = Rect::new(base.x + base.width.saturating_sub(46), base.y, 46.min(base.width), 3);
            f.render_widget(Clear, r);
            let b = bloque(titulo, true);
            let i = b.inner(r);
            f.render_widget(b, r);
            f.render_widget(Paragraph::new(texto.clone()), i);
            Some((i.x + (UnicodeWidthStr::width(texto.as_str()) as u16).min(i.width.saturating_sub(1)), i.y))
        }
        Modal::Confirmar { titulo, mensaje, .. } => {
            let r = centrado(area, 64, 7);
            f.render_widget(Clear, r);
            let b = bloque(format!(" {titulo} "), true).border_style(Style::default().fg(tema::ADVERTENCIA));
            let i = b.inner(r);
            f.render_widget(b, r);
            f.render_widget(Paragraph::new(mensaje.clone()).wrap(Wrap { trim: true }), i);
            None
        }
        Modal::Commit { mensaje } => {
            let r = centrado(area, 76, 12);
            f.render_widget(Clear, r);
            let b = bloque(" Commit (Ctrl+Enter, Alt+Enter o Ctrl+S confirma · Esc cancela) ", true);
            let i = b.inner(r);
            f.render_widget(b, r);
            let mut lineas: Vec<Line> = mensaje.split('\n').map(|l| Line::raw(l.to_string())).collect();
            let aviso = jonesy_git::aviso_mensaje(mensaje);
            let alto = i.height.saturating_sub(if aviso.is_some() { 1 } else { 0 });
            if let Some(a) = aviso {
                f.buffer_mut().set_string(i.x, i.y + i.height - 1, a, Style::default().fg(tema::ADVERTENCIA));
            }
            let n = lineas.len();
            let ultima = lineas.pop().unwrap_or_default();
            let ancho_ultima = ultima.width() as u16;
            lineas.push(ultima);
            f.render_widget(Paragraph::new(lineas), Rect { height: alto, ..i });
            Some((i.x + ancho_ultima.min(i.width.saturating_sub(1)), i.y + (n as u16 - 1).min(alto.saturating_sub(1))))
        }
        Modal::Visor { titulo, lineas, desplazamiento } => {
            let ancho_max = lineas.iter().map(|l| l.width()).max().unwrap_or(20) as u16 + 4;
            let w = ancho_max.clamp(40, area.width.saturating_sub(4));
            let h = (lineas.len() as u16 + 2).clamp(6, area.height.saturating_sub(4));
            let r = centrado(area, w, h);
            f.render_widget(Clear, r);
            let b = bloque(format!(" {titulo} · Esc cierra "), true);
            let i = b.inner(r);
            f.render_widget(b, r);
            let visibles: Vec<Line> = lineas.iter().skip(*desplazamiento).take(i.height as usize).cloned().collect();
            f.render_widget(Paragraph::new(visibles), i);
            if lineas.len() > i.height as usize {
                let pie = format!(" {}/{} ", desplazamiento + 1, lineas.len());
                f.buffer_mut().set_string(r.x + r.width.saturating_sub(pie.len() as u16 + 2), r.y + r.height - 1, pie, Style::default().fg(tema::ATENUADO));
            }
            None
        }
        Modal::Menu { indice, sel } => {
            let (indice, sel) = (*indice, *sel);
            let menus = accion::menus(app.examen.is_some());
            let Some((_, items)) = menus.get(indice) else { return None };
            let x = app.zonas.menu.iter().find(|(_, i)| *i == indice).map(|(r, _)| r.x).unwrap_or(1);
            let ancho = items.iter().map(|a| a.info().titulo.chars().count() + a.info().atajo.chars().count() + 4).max().unwrap_or(20) as u16 + 2;
            let r = Rect::new(x.min(area.width.saturating_sub(ancho)), 1, ancho.min(area.width), (items.len() as u16 + 2).min(area.height.saturating_sub(1)));
            f.render_widget(Clear, r);
            let b = bloque("", true);
            let i = b.inner(r);
            f.render_widget(b, r);
            for (n, a) in items.iter().enumerate().take(i.height as usize) {
                let info = a.info();
                let fila = Rect::new(i.x, i.y + n as u16, i.width, 1);
                let estilo = if n == sel { Style::default().bg(tema::ACENTO).fg(Color::Black) } else { Style::default() };
                let relleno = (i.width as usize).saturating_sub(info.titulo.chars().count() + info.atajo.chars().count() + 2);
                let texto = format!(" {}{}{} ", info.titulo, " ".repeat(relleno), info.atajo);
                f.render_widget(Paragraph::new(texto).style(estilo), fila);
                app.zonas.items_menu.push((fila, *a));
            }
            let _ = ascii;
            None
        }
    }
}

fn lista_modal(f: &mut Frame, app: &mut App, area: Rect, titulo: &str, filtro: &str, items: &[(String, String)], sel: usize) -> Option<(u16, u16)> {
    let r = centrado(area, 70, 18);
    f.render_widget(Clear, r);
    let b = bloque(titulo.to_string(), true);
    let i = b.inner(r);
    f.render_widget(b, r);
    let entrada = format!("> {filtro}");
    f.buffer_mut().set_string(i.x, i.y, &entrada, Style::default().fg(tema::ACENTO));
    let lista = Rect { y: i.y + 1, height: i.height.saturating_sub(1), ..i };
    let alto = lista.height as usize;
    let sel = sel.min(items.len().saturating_sub(1));
    let base = sel.saturating_sub(alto.saturating_sub(1));
    app.zonas.modal_items = Some((lista, base));
    for (n, (t, atajo)) in items.iter().enumerate().skip(base).take(alto) {
        let fila = Rect::new(lista.x, lista.y + (n - base) as u16, lista.width, 1);
        let estilo = if n == sel { Style::default().bg(tema::FONDO_SELECCION).add_modifier(Modifier::BOLD) } else { Style::default() };
        let relleno = (lista.width as usize).saturating_sub(UnicodeWidthStr::width(t.as_str()) + atajo.chars().count() + 2);
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!(" {t}{}", " ".repeat(relleno)), estilo),
                Span::styled(format!("{atajo} "), estilo.fg(tema::ATENUADO)),
            ])),
            fila,
        );
    }
    Some((i.x + (UnicodeWidthStr::width(entrada.as_str()) as u16).min(i.width.saturating_sub(1)), i.y))
}
