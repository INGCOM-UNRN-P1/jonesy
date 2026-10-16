//! Dibujo de los modales: una función por modal, que despacha `dibujar_modal` (N-JONESY-02).

use super::*;

pub(super) fn dibujar_modal(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(modal) = &app.modal else { return None };
    match modal {
        Modal::Paleta { .. } => modal_paleta(f, app, area),
        Modal::Abrir { .. } => modal_abrir(f, app, area),
        Modal::Argumentos { .. } => modal_argumentos(f, app, area),
        Modal::Renombrar { .. } => modal_renombrar(f, app, area),
        Modal::RenombrarSimbolo { .. } => modal_renombrar_simbolo(f, app, area),
        Modal::Completar { .. } => modal_completar(f, app, area),
        Modal::Vigilar { .. } => modal_vigilar(f, app, area),
        Modal::RutaPreferencias { .. } => modal_ruta_preferencias(f, app, area),
        Modal::CodigoPausa { .. } => modal_codigo_pausa(f, app, area),
        Modal::NuevoArchivo { .. } => modal_nuevo_archivo(f, app, area),
        Modal::Buscar { .. } | Modal::IrALinea { .. } => modal_buscar_o_ir_a_linea(f, app, area),
        Modal::Recorrido { .. } => modal_recorrido(f, app, area),
        Modal::Simulacro { .. } => modal_simulacro(f, app, area),
        Modal::ArgsPrograma { .. } => modal_args_programa(f, app, area),
        Modal::Casos { .. } => modal_casos(f, app, area),
        Modal::BuscarProyecto { .. } => modal_buscar_proyecto(f, app, area),
        Modal::Reemplazar { .. } => modal_reemplazar(f, app, area),
        Modal::Confirmar { .. } => modal_confirmar(f, app, area),
        Modal::Commit { .. } => modal_commit(f, app, area),
        Modal::Visor { .. } => modal_visor(f, app, area),
        Modal::Clonar { .. } => modal_clonar(f, app, area),
        Modal::AbrirProyecto { .. } => modal_abrir_proyecto(f, app, area),
        Modal::Diff { .. } => modal_diff(f, app, area),
        Modal::Bienvenida { .. } => modal_bienvenida(f, app, area),
        Modal::Configuracion { .. } => modal_configuracion(f, app, area),
        Modal::Paneles { .. } => modal_paneles(f, app, area),
        Modal::Menu { .. } => modal_menu(f, app, area),
    }
}

fn modal_paleta(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(Modal::Paleta {
        filtro,
        sel,
        opciones,
    }) = &app.modal
    else {
        return None;
    };
    let titulos: Vec<&str> = opciones.iter().map(|o| o.titulo.as_str()).collect();
    let filtradas = paleta::filtrar(filtro, &titulos);
    let items: Vec<(String, String)> = filtradas
        .iter()
        .map(|&i| (opciones[i].titulo.clone(), opciones[i].atajo.clone()))
        .collect();
    let (filtro, sel) = (filtro.clone(), *sel);
    let solo_make = !opciones.is_empty()
        && opciones
            .iter()
            .all(|o| matches!(o.que, crate::app::QuePaleta::Objetivo(..)));
    let titulo = if solo_make {
        " Objetivos de make (escribí para filtrar · Enter corre) "
    } else {
        " Comandos "
    };
    lista_modal(f, app, area, titulo, &filtro, &items, sel)
}

fn modal_abrir(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(Modal::Abrir {
        filtro,
        sel,
        archivos,
        marcados,
    }) = &app.modal
    else {
        return None;
    };
    let filtrados = paleta::filtrar(filtro, archivos);
    let items: Vec<(String, String)> = filtrados
        .iter()
        .map(|&i| {
            let marca = if marcados.contains(&i) {
                "● marcado"
            } else {
                ""
            };
            (archivos[i].clone(), marca.to_string())
        })
        .collect();
    let (filtro, sel) = (filtro.clone(), *sel);
    let titulo = if items.is_empty() && !filtro.is_empty() {
        " Abrir (Enter crea el archivo) "
    } else {
        " Abrir archivo (Tab marca varios) "
    };
    lista_modal(f, app, area, titulo, &filtro, &items, sel)
}

fn modal_argumentos(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(Modal::Argumentos {
        titulo,
        texto,
        comando,
    }) = &app.modal
    else {
        return None;
    };
    let r = centrado(area, 76, 6);
    limpiar(f, r);
    let b = bloque(
        format!(" {comando} · Enter corre en la terminal · Esc cancela "),
        true,
    );
    let i = b.inner(r);
    f.render_widget(b, r);
    f.render_widget(
        Paragraph::new(titulo.clone())
            .style(Style::default().fg(tema::atenuado()))
            .wrap(Wrap { trim: true }),
        Rect { height: 2, ..i },
    );
    let y = i.y + i.height.saturating_sub(1);
    let linea = format!("> {texto}");
    f.buffer_mut()
        .set_string(i.x, y, &linea, Style::default().fg(tema::acento()));
    Some((
        i.x + (UnicodeWidthStr::width(linea.as_str()) as u16).min(i.width.saturating_sub(1)),
        y,
    ))
}

fn modal_renombrar(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(Modal::Renombrar { texto, .. }) = &app.modal else {
        return None;
    };
    let r = centrado(area, 64, 3);
    limpiar(f, r);
    let b = bloque(" Renombrar o mover (ruta relativa al proyecto) ", true);
    let i = b.inner(r);
    f.render_widget(b, r);
    f.render_widget(Paragraph::new(texto.clone()), i);
    Some((
        i.x + (UnicodeWidthStr::width(texto.as_str()) as u16).min(i.width.saturating_sub(1)),
        i.y,
    ))
}

fn modal_renombrar_simbolo(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(Modal::RenombrarSimbolo { plan, texto, error }) = &app.modal else {
        return None;
    };
    let r = centrado(area, 64, 5);
    limpiar(f, r);
    let n = plan.rangos.len();
    let b = bloque(
        format!(" Renombrar «{}» · Enter aplica · Esc cancela ", plan.nombre),
        true,
    );
    let i = b.inner(r);
    f.render_widget(b, r);
    let (nota, color) = match error {
        Some(e) => (e.clone(), tema::error()),
        None => (
            format!(
                "{n} {} en {}",
                if n == 1 { "aparición" } else { "apariciones" },
                plan.alcance
            ),
            tema::atenuado(),
        ),
    };
    f.render_widget(
        Paragraph::new(nota)
            .style(Style::default().fg(color))
            .wrap(Wrap { trim: true }),
        Rect { height: 2, ..i },
    );
    let y = i.y + i.height.saturating_sub(1);
    f.buffer_mut()
        .set_string(i.x, y, texto, Style::default().fg(tema::acento()));
    Some((
        i.x + (UnicodeWidthStr::width(texto.as_str()) as u16).min(i.width.saturating_sub(1)),
        y,
    ))
}

fn modal_completar(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(Modal::Completar { opciones, sel, .. }) = &app.modal else {
        return None;
    };
    // Debajo del cursor (o arriba si no entra), dentro del editor.
    let z = app.zonas.editor.map(|z| z.texto).unwrap_or(area);
    let b = &app.buf().buf;
    let (l, c) = b.linea_columna();
    let y_cursor = z.y + l.saturating_sub(b.desplazamiento) as u16;
    let x_cursor = z.x + c.saturating_sub(b.desplazamiento_h) as u16;
    let x = x_cursor.min(area.x + area.width.saturating_sub(20));
    let visibles = opciones.len().min(8) as u16;
    let ancho_texto = opciones
        .iter()
        .map(|o| o.texto.chars().count())
        .max()
        .unwrap_or(4);
    let ancho_detalle = opciones
        .iter()
        .map(|o| o.detalle.chars().count())
        .max()
        .unwrap_or(0);
    let w = ((ancho_texto + ancho_detalle + 5) as u16)
        .min(70)
        .min(area.x + area.width - x);
    let h = visibles + 2;
    let y = if y_cursor + 1 + h <= area.y + area.height {
        y_cursor + 1
    } else {
        y_cursor.saturating_sub(h)
    };
    let r = Rect::new(x, y, w, h);
    limpiar(f, r);
    let bloque_c = bloque(
        format!(" {}/{} · Enter completa ", sel + 1, opciones.len()),
        true,
    );
    let i = bloque_c.inner(r);
    f.render_widget(bloque_c, r);
    let desde = sel.saturating_sub(visibles as usize - 1);
    let lineas: Vec<Line> = opciones
        .iter()
        .enumerate()
        .skip(desde)
        .take(visibles as usize)
        .map(|(k, o)| {
            let estilo = if k == *sel {
                Style::default()
                    .bg(tema::fondo_seleccion())
                    .fg(tema::fuerte())
            } else {
                Style::default().fg(tema::texto())
            };
            Line::from(vec![
                Span::styled(format!("{:<w$} ", o.texto, w = ancho_texto), estilo),
                Span::styled(o.detalle.clone(), Style::default().fg(tema::atenuado())),
            ])
        })
        .collect();
    f.render_widget(Paragraph::new(lineas), i);
    Some((x_cursor, y_cursor))
}

fn modal_vigilar(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(Modal::Vigilar { texto }) = &app.modal else {
        return None;
    };
    let r = centrado(area, 64, 3);
    limpiar(f, r);
    let b = bloque(
        " Vigilar una expresión (x, v[i], *p, p->campo) · Enter ",
        true,
    );
    let i = b.inner(r);
    f.render_widget(b, r);
    f.render_widget(Paragraph::new(texto.clone()), i);
    Some((
        i.x + (UnicodeWidthStr::width(texto.as_str()) as u16).min(i.width.saturating_sub(1)),
        i.y,
    ))
}

fn modal_ruta_preferencias(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(Modal::RutaPreferencias { importar, texto }) = &app.modal else {
        return None;
    };
    let r = centrado(area, 76, 3);
    limpiar(f, r);
    let b = bloque(
        if *importar {
            " Importar preferencias desde (Enter · Esc cancela) "
        } else {
            " Exportar preferencias a (Enter · Esc cancela) "
        },
        true,
    );
    let i = b.inner(r);
    f.render_widget(b, r);
    f.render_widget(Paragraph::new(texto.clone()), i);
    Some((
        i.x + (UnicodeWidthStr::width(texto.as_str()) as u16).min(i.width.saturating_sub(1)),
        i.y,
    ))
}

fn modal_codigo_pausa(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(Modal::CodigoPausa { texto }) = &app.modal else {
        return None;
    };
    let r = centrado(area, 56, 3);
    limpiar(f, r);
    let b = bloque(" Código del docente para pausar · Esc cancela ", true)
        .border_style(Style::default().fg(tema::advertencia()));
    let i = b.inner(r);
    f.render_widget(b, r);
    let oculto = "*".repeat(texto.chars().count());
    f.render_widget(Paragraph::new(oculto.clone()), i);
    Some((
        i.x + (oculto.len() as u16).min(i.width.saturating_sub(1)),
        i.y,
    ))
}

fn modal_nuevo_archivo(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(Modal::NuevoArchivo { texto, carpeta }) = &app.modal else {
        return None;
    };
    let titulo = if *carpeta {
        " Nueva carpeta (ruta relativa al proyecto) "
    } else {
        " Nuevo archivo (ruta relativa al proyecto) "
    };
    let r = centrado(area, 64, 3);
    limpiar(f, r);
    let b = bloque(titulo, true);
    let i = b.inner(r);
    f.render_widget(b, r);
    f.render_widget(Paragraph::new(texto.clone()), i);
    Some((
        i.x + (UnicodeWidthStr::width(texto.as_str()) as u16).min(i.width.saturating_sub(1)),
        i.y,
    ))
}

fn modal_buscar_o_ir_a_linea(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(modal) = &app.modal else {
        return None;
    };
    let (Modal::Buscar { texto } | Modal::IrALinea { texto }) = modal else {
        return None;
    };
    let titulo = if matches!(modal, Modal::Buscar { .. }) {
        " Buscar (Enter siguiente · Esc cerrar) "
    } else {
        " Ir a la línea "
    };
    let base = app.zonas.editor.map(|z| z.texto).unwrap_or(area);
    let r = Rect::new(
        base.x + base.width.saturating_sub(46),
        base.y,
        46.min(base.width),
        3,
    );
    limpiar(f, r);
    let b = bloque(titulo, true);
    let i = b.inner(r);
    f.render_widget(b, r);
    f.render_widget(Paragraph::new(texto.clone()), i);
    Some((
        i.x + (UnicodeWidthStr::width(texto.as_str()) as u16).min(i.width.saturating_sub(1)),
        i.y,
    ))
}

fn modal_recorrido(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(Modal::Recorrido { paso }) = &app.modal else {
        return None;
    };
    use crate::ayuda::{Zona, RECORRIDO};
    let (zona, titulo, texto, oculta) = RECORRIDO[(*paso).min(RECORRIDO.len() - 1)];
    let a = &app.areas;
    let rect = match zona {
        Zona::Menu => Some(a.menu),
        Zona::Editor => a.editor,
        Zona::Consigna => a.consigna,
        Zona::QuickDoc => a.quickdoc,
        Zona::Estado => Some(a.estado),
        Zona::Inferior => a.inferior,
        Zona::Izquierdo => a.izquierdo,
        Zona::Ninguna => None,
    };
    // La zona, marcada con un borde grueso.
    if let Some(z) = rect {
        let marco = Rect {
            x: z.x,
            y: z.y,
            width: z.width.min(area.width.saturating_sub(z.x)),
            height: z.height.max(1).min(area.height.saturating_sub(z.y)),
        };
        f.render_widget(
            Block::default()
                .borders(if marco.height >= 2 {
                    ratatui::widgets::Borders::ALL
                } else {
                    ratatui::widgets::Borders::BOTTOM | ratatui::widgets::Borders::TOP
                })
                .border_type(ratatui::widgets::BorderType::Thick)
                .border_style(
                    Style::default()
                        .fg(tema::advertencia())
                        .add_modifier(Modifier::BOLD),
                ),
            marco,
        );
    }
    // La explicación, cerca de la zona sin taparla si se puede.
    let explicacion = if rect.is_none() && !oculta.is_empty() {
        format!("{texto}\n\n{oculta}")
    } else {
        texto.to_string()
    };
    let w = 60.min(area.width.saturating_sub(4));
    let alto_txt = explicacion.chars().count() as u16 / w.saturating_sub(4).max(1) + 6;
    let h = alto_txt.min(area.height.saturating_sub(2));
    let (x, y) = match rect {
        Some(z) if z.y + z.height + h < area.height => (z.x + 2, z.y + z.height),
        Some(z) if z.y > h => (z.x + 2, z.y - h),
        _ => (
            area.width.saturating_sub(w) / 2,
            area.height.saturating_sub(h) / 2,
        ),
    };
    let r = Rect::new(x.min(area.width.saturating_sub(w)), y, w, h);
    limpiar(f, r);
    let b = bloque(
        format!(" {} · {}/{} ", titulo, paso + 1, RECORRIDO.len()),
        true,
    );
    let i = b.inner(r);
    f.render_widget(b, r);
    let l: Vec<Line> = vec![
        Line::raw(explicacion),
        Line::raw(""),
        Line::styled(
            "Enter o → sigue · ← vuelve · Esc termina",
            Style::default().fg(tema::atenuado()),
        ),
    ];
    f.render_widget(Paragraph::new(l).wrap(Wrap { trim: false }), i);
    None
}

fn modal_simulacro(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(Modal::Simulacro { texto }) = &app.modal else {
        return None;
    };
    let r = centrado(area, 64, 6);
    limpiar(f, r);
    let b = bloque(" Simulacro de examen · Enter empieza · Esc cancela ", true);
    let i = b.inner(r);
    f.render_widget(b, r);
    f.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::raw("Minutos: "),
                Span::styled(texto.clone(), Style::default().add_modifier(Modifier::BOLD)),
            ]),
            Line::raw(""),
            Line::styled(
                "Cronómetro y avisos como en el examen, sin restricciones ni entrega.",
                Style::default().fg(tema::atenuado()),
            ),
            Line::styled(
                "F12 lo termina y muestra cómo te fue.",
                Style::default().fg(tema::atenuado()),
            ),
        ]),
        i,
    );
    Some((i.x + 9 + texto.chars().count() as u16, i.y))
}

fn modal_args_programa(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(Modal::ArgsPrograma { texto }) = &app.modal else {
        return None;
    };
    let r = centrado(area, 64, 5);
    limpiar(f, r);
    let b = bloque(
        " Argumentos del programa · Enter guarda · Esc cancela ",
        true,
    );
    let i = b.inner(r);
    f.render_widget(b, r);
    f.render_widget(
        Paragraph::new(vec![
            Line::raw(texto.clone()),
            Line::raw(""),
            Line::styled(
                "Para este ejercicio; los usan Ctrl+F5 y F5. Vacío: sin argumentos.",
                Style::default().fg(tema::atenuado()),
            ),
        ]),
        i,
    );
    Some((
        i.x + (UnicodeWidthStr::width(texto.as_str()) as u16).min(i.width.saturating_sub(1)),
        i.y,
    ))
}

fn modal_casos(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(Modal::Casos { resultados, sel }) = &app.modal else {
        return None;
    };
    let alto = (resultados.len() as u16 + 4).min(area.height.saturating_sub(2));
    let r = centrado(area, 80, alto);
    limpiar(f, r);
    let b = bloque(" Casos · Enter muestra la diferencia · Esc cierra ", true);
    let i = b.inner(r);
    f.render_widget(b, r);
    let mut lineas: Vec<Line> = resultados
        .iter()
        .enumerate()
        .map(|(n, x)| {
            let fondo = if n == *sel {
                tema::fondo_seleccion()
            } else {
                tema::fondo()
            };
            let st = Style::default().bg(fondo);
            let limites =
                jonesy_core::casos::Limites::de_config(&app.proyecto.config.casos, &x.caso.nombre);
            let (marca, color, detalle) = match (x.coincide, x.corrida.colgado) {
                (_, true) => (
                    "✗",
                    tema::error(),
                    format!(
                        "superó el tiempo ({} s): ¿un ciclo infinito o muy lento?",
                        limites.tiempo.as_secs_f64()
                    ),
                ),
                _ if x.corrida.sin_memoria => (
                    "✗",
                    tema::error(),
                    format!(
                        "superó la memoria ({} MiB)",
                        limites.memoria_mb.unwrap_or(0)
                    ),
                ),
                (Some(true), _) => ("✓", tema::ok(), "coincide".to_string()),
                (Some(false), _) => (
                    "✗",
                    tema::error(),
                    format!("{} línea(s) distintas", x.distintas),
                ),
                (None, _) => ("·", tema::atenuado(), "sin salida esperada".to_string()),
            };
            let codigo = match (x.corrida.codigo, limites.memoria_mb) {
                (Some(0) | None, _) => String::new(),
                (Some(c), Some(mb)) if !x.corrida.sin_memoria => {
                    format!(" · código {c} (con el límite de {mb} MiB: ¿malloc devolvió NULL?)")
                }
                (Some(c), _) => format!(" · código {c}"),
            };
            Line::from(vec![
                Span::styled(format!(" {marca} "), st.fg(color)),
                Span::styled(
                    format!("{:<24}", x.caso.nombre),
                    st.add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{detalle}{codigo}"), st),
                Span::styled(
                    format!(
                        "  {:.2} s{}",
                        x.corrida.tiempo.as_secs_f64(),
                        x.corrida
                            .memoria_kb
                            .map(|kb| format!(" · {:.1} MiB", kb as f64 / 1024.0))
                            .unwrap_or_default()
                    ),
                    st.fg(tema::atenuado()),
                ),
            ])
        })
        .collect();
    lineas.push(Line::raw(""));
    lineas.push(Line::styled(
        " Se comparan sin espacios al final de las líneas ni líneas vacías al final.",
        Style::default().fg(tema::atenuado()),
    ));
    f.render_widget(Paragraph::new(lineas), i);
    None
}

fn modal_buscar_proyecto(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(Modal::BuscarProyecto {
        texto,
        mayusculas,
        palabra,
    }) = &app.modal
    else {
        return None;
    };
    let r = centrado(area, 64, 5);
    limpiar(f, r);
    let b = bloque(" Buscar en el proyecto · Enter busca · Esc cierra ", true);
    let i = b.inner(r);
    f.render_widget(b, r);
    let marca = |on: bool| if on { "[x]" } else { "[ ]" };
    f.render_widget(
        Paragraph::new(vec![
            Line::raw(texto.clone()),
            Line::raw(""),
            Line::styled(
                format!(
                    "{} mayúsculas (Alt+M)  {} palabra completa (Alt+P)",
                    marca(*mayusculas),
                    marca(*palabra)
                ),
                Style::default().fg(tema::nota()),
            ),
        ]),
        i,
    );
    Some((
        i.x + (UnicodeWidthStr::width(texto.as_str()) as u16).min(i.width.saturating_sub(1)),
        i.y,
    ))
}

fn modal_reemplazar(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(Modal::Reemplazar {
        buscar,
        reemplazo,
        campo,
        mayusculas,
        palabra,
    }) = &app.modal
    else {
        return None;
    };
    let n = app
        .buf()
        .buf
        .coincidencias(buscar, *mayusculas, *palabra)
        .len();
    let base = app.zonas.editor.map(|z| z.texto).unwrap_or(area);
    let w = 60.min(base.width);
    let r = Rect::new(base.x + base.width.saturating_sub(w), base.y, w, 6);
    limpiar(f, r);
    let b = bloque(format!(" Buscar y reemplazar · {n} coincidencia(s) "), true);
    let i = b.inner(r);
    f.render_widget(b, r);
    let marca = |on: bool| if on { "[x]" } else { "[ ]" };
    let rotulo = |k: u8| {
        if *campo == k {
            Style::default()
                .fg(tema::acento())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(tema::atenuado())
        }
    };
    let lineas = vec![
        Line::from(vec![
            Span::styled("Buscar:     ", rotulo(0)),
            Span::raw(buscar.clone()),
        ]),
        Line::from(vec![
            Span::styled("Reemplazar: ", rotulo(1)),
            Span::raw(reemplazo.clone()),
        ]),
        Line::styled(
            format!(
                "{} mayúsculas (Alt+M)  {} palabra completa (Alt+P)",
                marca(*mayusculas),
                marca(*palabra)
            ),
            Style::default().fg(tema::nota()),
        ),
        Line::styled(
            "Enter reemplaza y sigue · F3 saltea · Alt+T todas · Tab campo",
            Style::default().fg(tema::atenuado()),
        ),
    ];
    f.render_widget(Paragraph::new(lineas), i);
    let texto = if *campo == 0 { buscar } else { reemplazo };
    Some((
        (i.x + 12 + UnicodeWidthStr::width(texto.as_str()) as u16)
            .min(i.x + i.width.saturating_sub(1)),
        i.y + *campo as u16,
    ))
}

fn modal_confirmar(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(Modal::Confirmar {
        titulo, mensaje, ..
    }) = &app.modal
    else {
        return None;
    };
    // Alto según el texto (la entrega muestra la lista de archivos).
    let filas: usize = mensaje
        .split('\n')
        .map(|l| UnicodeWidthStr::width(l).div_ceil(62).max(1))
        .sum();
    let alto = (filas as u16 + 2).clamp(7, area.height.saturating_sub(2).max(7));
    let r = centrado(area, 64, alto);
    limpiar(f, r);
    let b =
        bloque(format!(" {titulo} "), true).border_style(Style::default().fg(tema::advertencia()));
    let i = b.inner(r);
    f.render_widget(b, r);
    f.render_widget(Paragraph::new(mensaje.clone()).wrap(Wrap { trim: true }), i);
    None
}

fn modal_commit(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(Modal::Commit { mensaje }) = &app.modal else {
        return None;
    };
    let r = centrado(area, 76, 12);
    limpiar(f, r);
    let b = bloque(
        " Commit (Ctrl+Enter, Alt+Enter o Ctrl+S confirma · Esc cancela) ",
        true,
    );
    let i = b.inner(r);
    f.render_widget(b, r);
    let mut lineas: Vec<Line> = mensaje
        .split('\n')
        .map(|l| Line::raw(l.to_string()))
        .collect();
    // Plantilla mientras está vacío; revisión del mensaje al pie.
    if mensaje.is_empty() {
        let guia = Style::default().fg(tema::atenuado());
        let y0 = i.y;
        f.buffer_mut().set_string(
            i.x,
            y0,
            "Qué cambió, en una línea (sin punto final, hasta 72 caracteres)",
            guia,
        );
        f.buffer_mut().set_string(
            i.x,
            y0 + 2,
            "Por qué, si hace falta (después de una línea en blanco)",
            guia,
        );
    }
    let revision: Vec<(bool, String)> = if mensaje.is_empty() {
        vec![]
    } else {
        jonesy_git::revisar_mensaje(mensaje)
    };
    let alto = i.height.saturating_sub(revision.len() as u16);
    for (k, (bloquea, m)) in revision.iter().enumerate() {
        f.buffer_mut().set_string(
            i.x,
            i.y + alto + k as u16,
            format!("{} {m}", if *bloquea { "✗" } else { "!" }),
            Style::default().fg(if *bloquea {
                tema::error()
            } else {
                tema::advertencia()
            }),
        );
    }
    let n = lineas.len();
    let ultima = lineas.pop().unwrap_or_default();
    let ancho_ultima = ultima.width() as u16;
    lineas.push(ultima);
    f.render_widget(Paragraph::new(lineas), Rect { height: alto, ..i });
    Some((
        i.x + ancho_ultima.min(i.width.saturating_sub(1)),
        i.y + (n as u16 - 1).min(alto.saturating_sub(1)),
    ))
}

fn modal_visor(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(Modal::Visor {
        titulo,
        lineas,
        desplazamiento,
    }) = &app.modal
    else {
        return None;
    };
    let ancho_max = lineas.iter().map(|l| l.width()).max().unwrap_or(20) as u16 + 4;
    let w = ancho_max.clamp(40, area.width.saturating_sub(4));
    let h = (lineas.len() as u16 + 2).clamp(6, area.height.saturating_sub(4));
    let r = centrado(area, w, h);
    limpiar(f, r);
    let b = bloque(format!(" {titulo} · Esc cierra "), true);
    let i = b.inner(r);
    f.render_widget(b, r);
    let visibles: Vec<Line> = lineas
        .iter()
        .skip(*desplazamiento)
        .take(i.height as usize)
        .cloned()
        .collect();
    f.render_widget(Paragraph::new(visibles), i);
    if lineas.len() > i.height as usize {
        let pie = format!(" {}/{} ", desplazamiento + 1, lineas.len());
        f.buffer_mut().set_string(
            r.x + r.width.saturating_sub(pie.len() as u16 + 2),
            r.y + r.height - 1,
            pie,
            Style::default().fg(tema::atenuado()),
        );
    }
    None
}

fn modal_clonar(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(Modal::Clonar {
        url,
        destino,
        campo,
    }) = &app.modal
    else {
        return None;
    };
    let r = centrado(area, 78, 8);
    limpiar(f, r);
    let b = bloque(
        " Clonar repositorio · Tab cambia de campo · Enter clona · Esc cancela ",
        true,
    );
    let i = b.inner(r);
    f.render_widget(b, r);
    let carpeta = crate::app::expandir_ruta(destino, &app.proyecto.raiz)
        .map(|d| {
            d.join(crate::app::nombre_de_repo(url))
                .display()
                .to_string()
        })
        .unwrap_or_default();
    let campos = [("URL:    ", url.as_str()), ("Carpeta:", destino.as_str())];
    let mut cursor = None;
    for (n, (etiqueta, valor)) in campos.iter().enumerate() {
        let y = i.y + n as u16 * 2;
        let activo = n == *campo;
        f.buffer_mut()
            .set_string(i.x, y, etiqueta, Style::default().fg(tema::atenuado()));
        let estilo = if activo {
            Style::default()
                .fg(tema::acento())
                .bg(tema::fondo_seleccion())
        } else {
            Style::default()
        };
        let x0 = i.x + 9;
        let ancho = i.width.saturating_sub(9);
        f.buffer_mut()
            .set_string(x0, y, format!("{:<w$}", valor, w = ancho as usize), estilo);
        if activo {
            cursor = Some((
                x0 + (UnicodeWidthStr::width(*valor) as u16).min(ancho.saturating_sub(1)),
                y,
            ));
        }
    }
    f.buffer_mut().set_string(
        i.x,
        i.y + 4,
        format!("Se crea: {carpeta}"),
        Style::default().fg(tema::nota()),
    );
    f.buffer_mut().set_string(
        i.x,
        i.y + 5,
        "Corre en la terminal interna: ahí podés escribir usuario, token o la clave SSH.",
        Style::default().fg(tema::atenuado()),
    );
    cursor
}

fn modal_abrir_proyecto(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(Modal::AbrirProyecto {
        dir,
        filtro,
        sel,
        entradas,
    }) = &app.modal
    else {
        return None;
    };
    let etiquetas: Vec<&str> = entradas.iter().map(|e| e.etiqueta.as_str()).collect();
    let visibles: Vec<usize> = if filtro.is_empty() {
        (0..entradas.len()).collect()
    } else {
        paleta::filtrar(filtro, &etiquetas)
    };
    let items: Vec<(String, String)> = visibles
        .iter()
        .map(|&i| {
            let e = &entradas[i];
            let marca = if e.abrir {
                "Enter abre"
            } else if e.es_proyecto {
                "proyecto"
            } else {
                ""
            };
            (e.etiqueta.clone(), marca.to_string())
        })
        .collect();
    let (filtro, sel) = (filtro.clone(), *sel);
    let titulo = format!(
        " Abrir proyecto · {} · ← borra sube · escribí una ruta o filtrá ",
        dir.display()
    );
    lista_modal(f, app, area, &titulo, &filtro, &items, sel)
}

fn modal_diff(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(Modal::Diff {
        titulo,
        filas,
        desplazamiento,
        lado_a_lado,
        desplazamiento_h,
        lenguaje,
    }) = &app.modal
    else {
        return None;
    };
    let (titulo, desplazamiento, lado_a_lado) = (titulo.clone(), *desplazamiento, *lado_a_lado);
    let filas = filas.clone();
    let ctx = (*lenguaje, *desplazamiento_h);
    dibujar_diff(f, area, &titulo, &filas, desplazamiento, lado_a_lado, ctx);
    None
}

fn modal_bienvenida(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(Modal::Bienvenida { siempre, faltan }) = &app.modal else {
        return None;
    };
    let alto = (24 + faltan.len() as u16).min(area.height.saturating_sub(2));
    let r = centrado(area, 78, alto);
    limpiar(f, r);
    let b = bloque(" Bienvenida ", true);
    let i = b.inner(r);
    f.render_widget(b, r);
    let titulo = Style::default()
        .fg(tema::acento())
        .add_modifier(Modifier::BOLD);
    let tecla = Style::default()
        .fg(tema::advertencia())
        .add_modifier(Modifier::BOLD);
    let normal = Style::default().fg(tema::texto());
    let tenue = Style::default().fg(tema::atenuado());
    let mut l: Vec<Line> = vec![
        Line::styled(" Te damos la bienvenida a jonesy", titulo),
        Line::styled(
            " El IDE de Programación I: escribir, compilar, probar y depurar C en la terminal.",
            normal,
        ),
        Line::raw(""),
        Line::styled(" El ciclo de un ejercicio", titulo),
    ];
    let filas = [
        ("F7 · F8", "leer la consigna · volver al código"),
        (
            "Ctrl+S",
            "guardar (las herramientas revisan el estilo al guardar)",
        ),
        ("F6", "compilar: los errores aparecen explicados en español"),
        ("Ctrl+F5", "compilar y ejecutar en la terminal de abajo"),
        ("Ctrl+F6", "correr las pruebas del ejercicio"),
        ("F5", "depurar paso a paso (Ctrl+B pone un breakpoint)"),
        ("F1", "explicar el error o la regla donde está el cursor"),
        ("Ctrl+K", "buscar cualquier acción por su nombre"),
        ("F10 / Alt", "los menús (la letra subrayada abre cada uno)"),
    ];
    for (t, d) in filas {
        l.push(Line::from(vec![
            Span::styled(format!("   {t:<10}"), tecla),
            Span::styled(d, normal),
        ]));
    }
    l.push(Line::raw(""));
    l.push(Line::styled(" Tu entorno", titulo));
    if faltan.is_empty() {
        l.push(Line::styled(
            "   ✓ Están gcc, make, gdb, valgrind y git.",
            Style::default().fg(tema::ok()),
        ));
    } else {
        for (b, para, como) in faltan.iter() {
            l.push(Line::from(vec![
                Span::styled(
                    format!("   ✗ falta {b}"),
                    Style::default().fg(tema::error()),
                ),
                Span::styled(format!(" (para {para}): "), normal),
                Span::styled(como.clone(), tenue),
            ]));
        }
    }
    l.push(Line::raw(""));
    l.push(Line::from(vec![
        Span::styled(format!(" [{}] ", if *siempre { "x" } else { " " }), tecla),
        Span::styled(
            "mostrar esta pantalla cada vez que abro jonesy (Espacio)",
            normal,
        ),
    ]));
    l.push(Line::raw(""));
    l.push(Line::from(vec![
        Span::styled(" Enter", tecla),
        Span::styled(" empezar · ", tenue),
        Span::styled("G", tecla),
        Span::styled(" guía rápida · ", tenue),
        Span::styled("D", tecla),
        Span::styled(" diagnóstico · ", tenue),
        Span::styled("R", tecla),
        Span::styled(" recorrido por la pantalla", tenue),
    ]));
    f.render_widget(Paragraph::new(l).wrap(Wrap { trim: false }), i);
    None
}

fn modal_configuracion(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(Modal::Configuracion { pestana, sel }) = &app.modal else {
        return None;
    };
    let (pestana, sel) = (*pestana, *sel);
    let filas = app.filas_configuracion(pestana);
    let alto = (filas.len() as u16 + 7).clamp(12, area.height.saturating_sub(2));
    let r = centrado(area, 110, alto);
    limpiar(f, r);
    let b = bloque(" Configuración del entorno · Esc cierra ", true);
    let i = b.inner(r);
    f.render_widget(b, r);
    // Solapas.
    let mut solapas: Vec<Span> = vec![Span::raw(" ")];
    for (k, nombre) in crate::app::PESTANAS_CONFIG.iter().enumerate() {
        let st = if k == pestana {
            Style::default()
                .bg(tema::acento())
                .fg(tema::sobre_acento())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(tema::atenuado())
        };
        solapas.push(Span::styled(format!(" {} {nombre} ", k + 1), st));
        solapas.push(Span::raw(" "));
    }
    let mut lineas: Vec<Line> = vec![Line::from(solapas), Line::raw("")];
    let ancho_t = filas
        .iter()
        .map(|x| x.titulo.chars().count())
        .max()
        .unwrap_or(10)
        .min(48);
    let ancho_v = filas
        .iter()
        .map(|x| x.valor.chars().count())
        .max()
        .unwrap_or(0)
        .min(46);
    let primera = i.y + lineas.len() as u16;
    for (k, x) in filas.iter().enumerate() {
        let fondo = if k == sel {
            tema::fondo_seleccion()
        } else {
            tema::fondo()
        };
        let st = Style::default().bg(fondo);
        let color_valor = match x.valor.as_str() {
            "activa" | "activo" => tema::ok(),
            "apagada" | "con errores" => tema::advertencia(),
            "no instalada" | "no en examen" => tema::atenuado(),
            _ => tema::nota(),
        };
        let titulo: String = x.titulo.chars().take(ancho_t).collect();
        let valor: String = x.valor.chars().take(ancho_v).collect();
        lineas.push(Line::from(vec![
            Span::styled(
                format!(" {titulo:<ancho_t$}  "),
                st.add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("{valor:<ancho_v$}  "), st.fg(color_valor)),
            Span::styled(x.detalle.clone(), st.fg(tema::atenuado())),
        ]));
    }
    app.zonas.modal_items = Some((Rect::new(i.x, primera, i.width, filas.len() as u16), 0));
    lineas.push(Line::raw(""));
    lineas.push(Line::styled(
        " ←→ o 1-4 solapa · ↑↓ elegir · Enter o Espacio aplica · clic en una fila",
        Style::default().fg(tema::atenuado()),
    ));
    if pestana == 0 {
        for e in &app.errores_integraciones {
            lineas.push(Line::styled(
                format!(" ⚠ {e}"),
                Style::default().fg(tema::error()),
            ));
        }
        lineas.push(Line::styled(
            " Integraciones nuevas: .jonesy/integraciones/*.toml o ~/.config/jonesy/integraciones/ (docs/integraciones.md).",
            Style::default().fg(tema::atenuado()),
        ));
    }
    f.render_widget(Paragraph::new(lineas), i);
    None
}

fn modal_paneles(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let Some(Modal::Paneles { sel }) = &app.modal else {
        return None;
    };
    let sel = *sel;
    let r = centrado(area, 72, crate::app::CAMPOS_PANELES.len() as u16 + 5);
    limpiar(f, r);
    let b = bloque(" Tamaños de paneles ", true);
    let i = b.inner(r);
    f.render_widget(b, r);
    let mut lineas: Vec<Line> = crate::app::CAMPOS_PANELES
        .iter()
        .enumerate()
        .map(|(n, (titulo, unidad))| {
            let v = crate::app::valor_panel(&app.paneles, n);
            let estilo = if n == sel {
                Style::default()
                    .bg(tema::fondo_seleccion())
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Line::from(vec![
                Span::styled(format!(" {titulo:<48}"), estilo),
                Span::styled(format!("◂ {v:>3} ▸ "), estilo.fg(tema::acento())),
                Span::styled(unidad.to_string(), estilo.fg(tema::atenuado())),
            ])
        })
        .collect();
    lineas.push(Line::raw(""));
    lineas.push(Line::styled(
        " ↑↓ elegir · ←→ ajustar · r valores de .jonesy.toml · Enter cerrar · también se arrastran con el mouse",
        Style::default().fg(tema::atenuado()),
    ));
    f.render_widget(Paragraph::new(lineas).wrap(Wrap { trim: false }), i);
    None
}

fn modal_menu(f: &mut Frame, app: &mut App, area: Rect) -> Option<(u16, u16)> {
    let ascii = app.sim.ascii;
    let Some(Modal::Menu { indice, sel }) = &app.modal else {
        return None;
    };
    let indice = *indice;
    let menus = app.menus();
    let items = &menus.get(indice)?.items;
    let sel = accion::ajustar_sel(items, *sel);
    let x = app
        .zonas
        .menu
        .iter()
        .find(|(_, i)| *i == indice)
        .map(|(r, _)| r.x)
        .unwrap_or(1);
    let ancho = items
        .iter()
        .map(|e| e.titulo().chars().count() + e.atajo().chars().count() + 4)
        .max()
        .unwrap_or(20)
        .clamp(20, 90) as u16
        + 2;
    let r = Rect::new(
        x.min(area.width.saturating_sub(ancho)),
        1,
        ancho.min(area.width),
        (items.len() as u16 + 2).min(area.height.saturating_sub(1)),
    );
    limpiar(f, r);
    // El desplegable, con los colores de la barra (gris con texto negro en turbo).
    let b = bloque("", true).style(
        Style::default()
            .bg(tema::fondo_barra())
            .fg(tema::texto_barra()),
    );
    let i = b.inner(r);
    f.render_widget(b, r);
    let raya = app.sim.h.to_string().repeat(i.width as usize);
    // Si no entra (el menú Plugins puede ser largo), se desplaza con la selección.
    let alto = i.height as usize;
    let base = sel.saturating_sub(alto.saturating_sub(1));
    for (n, e) in items.iter().enumerate().skip(base).take(alto) {
        let fila = Rect::new(i.x, i.y + (n - base) as u16, i.width, 1);
        let barra = Style::default()
            .bg(tema::fondo_barra())
            .fg(tema::texto_barra());
        match e {
            accion::Entrada::Separador => {
                f.render_widget(
                    Paragraph::new(raya.clone()).style(barra.fg(tema::atenuado_barra())),
                    fila,
                );
                continue;
            }
            accion::Entrada::Nota(t) => {
                f.render_widget(
                    Paragraph::new(format!(" {t}")).style(barra.fg(tema::atenuado_barra())),
                    fila,
                );
                continue;
            }
            _ => {}
        }
        let estilo = if n == sel {
            Style::default()
                .bg(tema::fondo_menu_sel())
                .fg(tema::sobre_acento())
        } else {
            barra
        };
        let (titulo, atajo) = (e.titulo(), e.atajo());
        let disponible = (i.width as usize).saturating_sub(atajo.chars().count() + 3);
        let titulo: String = titulo.chars().take(disponible).collect();
        let relleno =
            (i.width as usize).saturating_sub(titulo.chars().count() + atajo.chars().count() + 2);
        let texto = format!(" {titulo}{}{atajo} ", " ".repeat(relleno));
        f.render_widget(Paragraph::new(texto).style(estilo), fila);
        app.zonas.items_menu.push((fila, n));
    }
    let _ = ascii;
    None
}
