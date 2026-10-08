//! `jonesy replay <telemetry.log>`: reconstrucción paso a paso de una sesión (RF-6.4).
//!
//! Muestra el documento tal como estaba en cada evento, con lo pegado resaltado, la lista
//! de eventos alrededor de la posición y un resumen de la sesión (velocidad, pausas,
//! pegados). La integridad de la cadena SHA-256 se verifica al abrir.

use std::path::Path;
use std::time::{Duration, Instant};

use jonesy_telemetry::replay::{resumir, Reproduccion, Resumen};
use jonesy_telemetry::{TelemetryEvent, TelemetryRecord, Verificacion};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, MouseEventKind};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::tema;

pub struct Reproductor {
    pub rep: Reproduccion,
    pub verificacion: Verificacion,
    pub resumen: Resumen,
    pub ruta: String,
    reproduciendo: bool,
    velocidad: u32,
    ultimo_paso: Instant,
    /// Buffer elegido a mano con Tab (None = el último editado).
    buffer_fijo: Option<String>,
}

pub fn describir(e: &TelemetryEvent) -> (String, Color) {
    match e {
        TelemetryEvent::SessionStart { user, exam_mode } => (
            format!(
                "inicio de sesión de {user}{}",
                if *exam_mode { " (examen)" } else { "" }
            ),
            tema::ACENTO,
        ),
        TelemetryEvent::KeyPress { key, .. } => (format!("tecla {key}"), tema::ATENUADO),
        TelemetryEvent::TextDelta {
            inserted,
            deleted_len,
            ..
        } => {
            let mostrar: String = inserted
                .chars()
                .take(24)
                .map(|c| if c == '\n' { '↵' } else { c })
                .collect();
            if inserted.is_empty() {
                (format!("borra {deleted_len}"), tema::ADVERTENCIA)
            } else if *deleted_len > 0 {
                (
                    format!("reemplaza {deleted_len} por «{mostrar}»"),
                    Color::White,
                )
            } else {
                (format!("escribe «{mostrar}»"), Color::White)
            }
        }
        TelemetryEvent::BracketedPaste { length, .. } => {
            (format!("PEGADO de {length} caracteres"), tema::PELIGRO)
        }
        TelemetryEvent::CursorMove { line, col, .. } => {
            (format!("cursor {line}:{col}"), tema::ATENUADO)
        }
        TelemetryEvent::BufferSave {
            buffer_id,
            total_bytes,
        } => (format!("guarda {buffer_id} ({total_bytes} B)"), tema::OK),
        TelemetryEvent::CommandExec { command, exit_code } => (
            format!(
                "ejecuta {command}{}",
                exit_code.map(|c| format!(" → {c}")).unwrap_or_default()
            ),
            tema::NOTA,
        ),
        TelemetryEvent::FocusChange { panel, focused } => (
            format!("{} {panel}", if *focused { "foco en" } else { "deja" }),
            tema::ATENUADO,
        ),
        TelemetryEvent::SessionEnd { reason } => {
            (format!("fin de sesión ({reason})"), tema::ACENTO)
        }
    }
}

impl Reproductor {
    pub fn abrir(ruta: &Path) -> std::io::Result<Reproductor> {
        let registros: Vec<TelemetryRecord> = jonesy_telemetry::leer(ruta)?;
        let verificacion = jonesy_telemetry::verificar(ruta)?;
        let resumen = resumir(&registros);
        Ok(Reproductor {
            rep: Reproduccion::nueva(registros),
            verificacion,
            resumen,
            ruta: ruta.display().to_string(),
            reproduciendo: false,
            velocidad: 20,
            ultimo_paso: Instant::now(),
            buffer_fijo: None,
        })
    }

    fn buffer_mostrado(&self) -> Option<String> {
        self.buffer_fijo
            .clone()
            .or_else(|| self.rep.buffer_activo.clone())
    }

    /// Procesa una tecla; false para salir.
    pub fn tecla(&mut self, k: KeyCode) -> bool {
        let n = self.rep.registros.len();
        match k {
            KeyCode::Char('q') | KeyCode::Esc => return false,
            KeyCode::Right | KeyCode::Char('l') => {
                self.rep.avanzar();
            }
            KeyCode::Left | KeyCode::Char('h') => self.rep.retroceder(),
            KeyCode::PageDown => {
                for _ in 0..50 {
                    self.rep.avanzar();
                }
            }
            KeyCode::PageUp => self.rep.ir_a(self.rep.posicion.saturating_sub(50)),
            KeyCode::Home => self.rep.ir_a(0),
            KeyCode::End => self.rep.ir_a(n),
            KeyCode::Char(' ') => {
                self.reproduciendo = !self.reproduciendo;
                self.ultimo_paso = Instant::now();
            }
            KeyCode::Char('+') => self.velocidad = (self.velocidad * 2).min(640),
            KeyCode::Char('-') => self.velocidad = (self.velocidad / 2).max(1),
            KeyCode::Char('p') => {
                // Salta al próximo pegado.
                while let Some(r) = self.rep.avanzar() {
                    if matches!(r.event, TelemetryEvent::BracketedPaste { .. }) {
                        self.rep.avanzar();
                        break;
                    }
                }
            }
            KeyCode::Tab => {
                let ids: Vec<String> = self.rep.documentos.keys().cloned().collect();
                if !ids.is_empty() {
                    let actual = self.buffer_mostrado();
                    let i = actual
                        .and_then(|a| ids.iter().position(|x| *x == a))
                        .map(|i| (i + 1) % ids.len())
                        .unwrap_or(0);
                    self.buffer_fijo = Some(ids[i].clone());
                }
            }
            KeyCode::Char('a') => self.buffer_fijo = None,
            _ => {}
        }
        true
    }

    /// Avance automático; devuelve cuánto esperar hasta el próximo paso.
    pub fn tic(&mut self) -> Duration {
        if !self.reproduciendo {
            return Duration::from_secs(3600);
        }
        let intervalo = Duration::from_millis(1000 / self.velocidad as u64);
        while self.ultimo_paso.elapsed() >= intervalo {
            self.ultimo_paso += intervalo;
            if self.rep.avanzar().is_none() {
                self.reproduciendo = false;
                break;
            }
        }
        intervalo.saturating_sub(self.ultimo_paso.elapsed())
    }

    pub fn dibujar(&self, f: &mut Frame) {
        let area = f.area();
        let [cab, cuerpo, pie] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .areas(area);
        let ok = self.verificacion.integra();
        let estado = if ok {
            Span::styled(
                " cadena SHA-256 íntegra ",
                Style::default()
                    .bg(tema::OK)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            let (l, m) = self.verificacion.error.clone().unwrap_or_default();
            Span::styled(
                format!(" CADENA ROTA en la línea {l}: {m} "),
                Style::default()
                    .bg(tema::PELIGRO)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
        };
        let n = self.rep.registros.len();
        let t0 = self.rep.registros.first().map(|r| r.ts_micros).unwrap_or(0);
        let t = self
            .rep
            .registros
            .get(self.rep.posicion.saturating_sub(1))
            .map(|r| r.ts_micros)
            .unwrap_or(t0);
        let seg = t.saturating_sub(t0) / 1_000_000;
        f.render_widget(
            Paragraph::new(vec![
                Line::from(vec![
                    Span::styled(
                        format!(" jonesy replay · {} ", self.ruta),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    estado,
                ]),
                Line::styled(
                    format!(
                        " evento {}/{} · t+{:02}:{:02}:{:02} · {} · velocidad {} ev/s",
                        self.rep.posicion,
                        n,
                        seg / 3600,
                        (seg % 3600) / 60,
                        seg % 60,
                        if self.reproduciendo {
                            "▶ reproduciendo"
                        } else {
                            "⏸ pausa"
                        },
                        self.velocidad
                    ),
                    Style::default().fg(tema::ATENUADO),
                ),
            ]),
            cab,
        );
        let [doc, lado] =
            Layout::horizontal([Constraint::Percentage(62), Constraint::Percentage(38)])
                .areas(cuerpo);
        self.dibujar_documento(f, doc);
        let [eventos, resumen] =
            Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(lado);
        self.dibujar_eventos(f, eventos);
        self.dibujar_resumen(f, resumen);
        f.render_widget(
            Paragraph::new(" ←/→ paso · RePág/AvPág ±50 · Inicio/Fin · Espacio reproducir · +/- velocidad · p próximo pegado · Tab archivo · q salir")
                .style(Style::default().bg(tema::FONDO_BARRA).fg(tema::ATENUADO)),
            pie,
        );
    }

    fn dibujar_documento(&self, f: &mut Frame, r: Rect) {
        let id = self.buffer_mostrado();
        let titulo = format!(
            " {} (resaltado: texto pegado) ",
            id.clone().unwrap_or_else(|| "sin documento".into())
        );
        let b = Block::default()
            .borders(Borders::ALL)
            .title(titulo)
            .border_style(Style::default().fg(tema::ACENTO));
        let i = b.inner(r);
        f.render_widget(b, r);
        let Some(d) = id.and_then(|id| self.rep.documentos.get(&id)) else {
            return;
        };
        // Ubica la última edición para que quede a la vista.
        let ultima = self.rep.registros[..self.rep.posicion]
            .iter()
            .rev()
            .find_map(|r| match &r.event {
                TelemetryEvent::TextDelta {
                    offset, inserted, ..
                } => Some(offset + inserted.chars().count()),
                _ => None,
            });
        let mut lineas: Vec<Line> = vec![];
        let mut actual: Vec<Span> = vec![];
        let mut linea_cursor = 0;
        let mut tramo = String::new();
        let mut pegado_tramo = false;
        let cerrar = |actual: &mut Vec<Span<'static>>, tramo: &mut String, pegado: bool| {
            if !tramo.is_empty() {
                let estilo = if pegado {
                    Style::default()
                        .bg(Color::Rgb(110, 50, 20))
                        .fg(Color::White)
                } else {
                    Style::default()
                };
                actual.push(Span::styled(std::mem::take(tramo), estilo));
            }
        };
        for (k, (&c, &p)) in d.texto.iter().zip(d.pegado.iter()).enumerate() {
            if Some(k) == ultima {
                linea_cursor = lineas.len();
            }
            if c == '\n' {
                cerrar(&mut actual, &mut tramo, pegado_tramo);
                lineas.push(Line::from(std::mem::take(&mut actual)));
                continue;
            }
            if p != pegado_tramo {
                cerrar(&mut actual, &mut tramo, pegado_tramo);
                pegado_tramo = p;
            }
            if c == '\t' {
                tramo.push_str("    ");
            } else {
                tramo.push(c);
            }
        }
        cerrar(&mut actual, &mut tramo, pegado_tramo);
        lineas.push(Line::from(actual));
        if ultima.is_some_and(|u| u >= d.texto.len()) {
            linea_cursor = lineas.len() - 1;
        }
        let alto = i.height as usize;
        let desde = linea_cursor
            .saturating_sub(alto / 2)
            .min(lineas.len().saturating_sub(alto));
        let numeradas: Vec<Line> = lineas
            .into_iter()
            .enumerate()
            .skip(desde)
            .take(alto)
            .map(|(n, mut l)| {
                let estilo = if n == linea_cursor {
                    Style::default().fg(tema::ADVERTENCIA)
                } else {
                    Style::default().fg(tema::ATENUADO)
                };
                l.spans
                    .insert(0, Span::styled(format!("{:>4} ", n + 1), estilo));
                l
            })
            .collect();
        f.render_widget(Paragraph::new(numeradas), i);
    }

    fn dibujar_eventos(&self, f: &mut Frame, r: Rect) {
        let b = Block::default()
            .borders(Borders::ALL)
            .title(" Eventos ")
            .border_style(Style::default().fg(tema::ATENUADO));
        let i = b.inner(r);
        f.render_widget(b, r);
        let alto = i.height as usize;
        let pos = self.rep.posicion;
        let desde = pos.saturating_sub(alto.saturating_sub(2));
        let t0 = self.rep.registros.first().map(|r| r.ts_micros).unwrap_or(0);
        let lineas: Vec<Line> = self
            .rep
            .registros
            .iter()
            .enumerate()
            .skip(desde)
            .take(alto)
            .map(|(k, reg)| {
                let (texto, color) = describir(&reg.event);
                let s = (reg.ts_micros.saturating_sub(t0)) as f64 / 1e6;
                let actual = k + 1 == pos;
                let futuro = k >= pos;
                let estilo = if actual {
                    Style::default()
                        .bg(tema::FONDO_SELECCION)
                        .fg(color)
                        .add_modifier(Modifier::BOLD)
                } else if futuro {
                    Style::default().fg(tema::ATENUADO)
                } else {
                    Style::default().fg(color)
                };
                Line::from(vec![
                    Span::styled(
                        format!("{:>6} {:>8.1}s ", reg.seq, s),
                        Style::default().fg(tema::ATENUADO),
                    ),
                    Span::styled(texto, estilo),
                ])
            })
            .collect();
        f.render_widget(Paragraph::new(lineas), i);
    }

    fn dibujar_resumen(&self, f: &mut Frame, r: Rect) {
        let b = Block::default()
            .borders(Borders::ALL)
            .title(" Resumen de la sesión ")
            .border_style(Style::default().fg(tema::ATENUADO));
        let i = b.inner(r);
        f.render_widget(b, r);
        // Abajo, el ritmo de escritura por minuto (verde) y los pegados (▲ rojo).
        let alto_grafico = if i.height >= 10 { 4 } else { 0 };
        let texto = Rect {
            height: i.height - alto_grafico,
            ..i
        };
        f.render_widget(
            Paragraph::new(lineas_resumen(&self.resumen, &self.verificacion))
                .wrap(Wrap { trim: true }),
            texto,
        );
        if alto_grafico > 0 {
            let ancho = i.width as usize;
            let tipeado =
                jonesy_telemetry::replay::agrupar(&self.resumen.tipeado_por_minuto, ancho);
            let pegado = jonesy_telemetry::replay::agrupar(&self.resumen.pegado_por_minuto, ancho);
            let y = i.y + i.height - alto_grafico;
            let buf = f.buffer_mut();
            buf.set_string(
                i.x,
                y,
                "Escritura por minuto (▲ pegados):",
                Style::default().fg(tema::ATENUADO),
            );
            let maximo = tipeado.iter().copied().max().unwrap_or(0);
            buf.set_string(
                i.x,
                y + 1,
                jonesy_telemetry::replay::sparkline(&tipeado),
                Style::default().fg(tema::OK),
            );
            let marcas: String = pegado
                .iter()
                .map(|&p| if p > 0 { '▲' } else { ' ' })
                .collect();
            buf.set_string(i.x, y + 2, marcas, Style::default().fg(tema::PELIGRO));
            let minutos = self.resumen.tipeado_por_minuto.len();
            buf.set_string(
                i.x,
                y + 3,
                format!("{minutos} min · máximo {maximo} car. por columna"),
                Style::default().fg(tema::ATENUADO),
            );
        }
    }
}

pub fn lineas_resumen(r: &Resumen, v: &Verificacion) -> Vec<Line<'static>> {
    let mut l = vec![
        Line::raw(format!(
            "Duración: {:.0} min · {} registros · {} sesión(es)",
            r.duracion_s / 60.0,
            r.registros,
            v.sesiones
        )),
        Line::raw(format!(
            "Tipeado: {} car. · borrado: {} car.",
            r.caracteres_tipeados, r.caracteres_borrados
        )),
        Line::raw(format!(
            "Velocidad: {:.0} car/min de escritura activa",
            r.velocidad_cpm
        )),
        Line::raw(format!(
            "Guardados: {} · comandos: {}",
            r.guardados,
            r.comandos.len()
        )),
    ];
    let pegado = Style::default()
        .fg(if r.pegados.is_empty() {
            tema::OK
        } else {
            tema::PELIGRO
        })
        .add_modifier(Modifier::BOLD);
    l.push(Line::styled(
        format!(
            "Pegados: {} ({} car.)",
            r.pegados.len(),
            r.caracteres_pegados
        ),
        pegado,
    ));
    for p in r.pegados.iter().take(5) {
        l.push(Line::styled(
            format!(
                "  #{} {} car. en {} · {}…",
                p.seq,
                p.length,
                p.buffer_id,
                &p.content_hash[..12.min(p.content_hash.len())]
            ),
            Style::default().fg(tema::ATENUADO),
        ));
    }
    l.push(Line::raw(format!(
        "Pausas > 30 s: {}",
        r.pausas_largas.len()
    )));
    l.push(Line::raw(format!(
        "Sesión cerrada: {}",
        if v.cerrada {
            "sí"
        } else {
            "no (corte o sesión abierta)"
        }
    )));
    l
}

/// Bucle interactivo del reproductor (usa la terminal actual).
pub fn ejecutar(ruta: &Path) -> anyhow::Result<()> {
    let mut rep = Reproductor::abrir(ruta)?;
    crate::sistema::instalar_gancho_panico();
    crate::sistema::preparar()?;
    let backend = ratatui::backend::CrosstermBackend::new(std::io::stdout());
    let mut term = ratatui::Terminal::new(backend)?;
    let resultado = (|| -> anyhow::Result<()> {
        loop {
            term.draw(|f| rep.dibujar(f))?;
            let espera = rep.tic();
            if event::poll(espera.min(Duration::from_secs(1)))? {
                match event::read()? {
                    Event::Key(k) if k.kind != KeyEventKind::Release => {
                        if !rep.tecla(k.code) {
                            return Ok(());
                        }
                    }
                    Event::Mouse(m) => match m.kind {
                        MouseEventKind::ScrollDown => {
                            rep.tecla(KeyCode::Right);
                        }
                        MouseEventKind::ScrollUp => {
                            rep.tecla(KeyCode::Left);
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
    })();
    crate::sistema::restaurar();
    resultado
}

#[cfg(test)]
mod tests {
    use super::*;
    use jonesy_telemetry::Telemetria;
    use ratatui::backend::TestBackend;

    #[test]
    fn reproduce_y_marca_pegados() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("telemetry.log");
        let t = Telemetria::abrir(&log).unwrap();
        t.registrar(TelemetryEvent::SessionStart {
            user: "ana".into(),
            exam_mode: true,
        });
        t.registrar(TelemetryEvent::TextDelta {
            buffer_id: "a.c".into(),
            offset: 0,
            inserted: "int x;".into(),
            deleted_len: 0,
        });
        t.registrar(TelemetryEvent::BracketedPaste {
            buffer_id: "a.c".into(),
            length: 7,
            content_hash: "ab".repeat(32),
        });
        t.registrar(TelemetryEvent::TextDelta {
            buffer_id: "a.c".into(),
            offset: 6,
            inserted: "\nint y;".into(),
            deleted_len: 0,
        });
        t.registrar(TelemetryEvent::SessionEnd {
            reason: "salida".into(),
        });
        t.cerrar();
        let mut r = Reproductor::abrir(&log).unwrap();
        assert!(r.verificacion.integra());
        r.tecla(KeyCode::End);
        let d = &r.rep.documentos["a.c"];
        assert_eq!(d.como_texto(), "int x;\nint y;");
        assert!(d.pegado[7] && !d.pegado[0]);
        let mut term = ratatui::Terminal::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| r.dibujar(f)).unwrap();
        let pantalla: String = term
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(pantalla.contains("íntegra"));
        assert!(pantalla.contains("Pegados: 1"));
        assert!(
            pantalla.contains("Escritura por minuto"),
            "gráfico de ritmo"
        );
        assert!(pantalla.contains('▲'), "el pegado se marca en el gráfico");
        assert!(pantalla.contains("int y;"));
        assert!(!r.tecla(KeyCode::Char('q')));
    }
}
