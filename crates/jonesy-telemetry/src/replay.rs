//! Reconstrucción de una sesión a partir de la bitácora (`jonesy replay`).
//!
//! Aplica los `TextDelta` en orden sobre un documento por buffer. Cada carácter recuerda si
//! llegó por un pegado (`BracketedPaste` seguido de su `TextDelta`), para que la vista de
//! replay lo muestre en rojo.

use std::collections::BTreeMap;

use crate::{TelemetryEvent, TelemetryRecord};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Documento {
    pub texto: Vec<char>,
    /// Por carácter: verdadero si llegó en un pegado.
    pub pegado: Vec<bool>,
}

impl Documento {
    fn aplicar(&mut self, offset: usize, insertado: &str, borrados: usize, es_pegado: bool) {
        let offset = offset.min(self.texto.len());
        let fin = (offset + borrados).min(self.texto.len());
        self.texto.drain(offset..fin);
        self.pegado.drain(offset..fin);
        let nuevos: Vec<char> = insertado.chars().collect();
        let n = nuevos.len();
        self.texto.splice(offset..offset, nuevos);
        self.pegado
            .splice(offset..offset, std::iter::repeat_n(es_pegado, n));
    }

    pub fn como_texto(&self) -> String {
        self.texto.iter().collect()
    }
}

/// Reproductor paso a paso.
pub struct Reproduccion {
    pub registros: Vec<TelemetryRecord>,
    /// Cantidad de registros ya aplicados.
    pub posicion: usize,
    pub documentos: BTreeMap<String, Documento>,
    pub buffer_activo: Option<String>,
    pegado_pendiente: Option<(String, usize)>,
}

impl Reproduccion {
    pub fn nueva(registros: Vec<TelemetryRecord>) -> Self {
        Reproduccion {
            registros,
            posicion: 0,
            documentos: BTreeMap::new(),
            buffer_activo: None,
            pegado_pendiente: None,
        }
    }

    pub fn terminada(&self) -> bool {
        self.posicion >= self.registros.len()
    }

    /// Aplica el registro siguiente y lo devuelve.
    pub fn avanzar(&mut self) -> Option<&TelemetryRecord> {
        let registro = self.registros.get(self.posicion)?.clone();
        self.posicion += 1;
        match &registro.event {
            TelemetryEvent::BracketedPaste {
                buffer_id, length, ..
            } => {
                self.pegado_pendiente = Some((buffer_id.clone(), *length));
            }
            TelemetryEvent::TextDelta {
                buffer_id,
                offset,
                inserted,
                deleted_len,
            } => {
                let es_pegado = match &self.pegado_pendiente {
                    Some((id, _)) if id == buffer_id && !inserted.is_empty() => {
                        self.pegado_pendiente = None;
                        true
                    }
                    _ => false,
                };
                self.documentos
                    .entry(buffer_id.clone())
                    .or_default()
                    .aplicar(*offset, inserted, *deleted_len, es_pegado);
                self.buffer_activo = Some(buffer_id.clone());
            }
            TelemetryEvent::CursorMove { buffer_id, .. }
            | TelemetryEvent::BufferSave { buffer_id, .. } => {
                self.documentos.entry(buffer_id.clone()).or_default();
                self.buffer_activo = Some(buffer_id.clone());
            }
            _ => {}
        }
        self.registros.get(self.posicion - 1)
    }

    /// Vuelve un paso atrás (reaplica desde el principio: las sesiones son chicas).
    pub fn retroceder(&mut self) {
        let objetivo = self.posicion.saturating_sub(1);
        self.ir_a(objetivo);
    }

    pub fn ir_a(&mut self, posicion: usize) {
        let registros = std::mem::take(&mut self.registros);
        *self = Reproduccion::nueva(registros);
        while self.posicion < posicion && !self.terminada() {
            self.avanzar();
        }
    }
}

/// Un pegado detectado, para el resumen de auditoría.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Pegado {
    pub seq: u64,
    pub ts_micros: u64,
    pub buffer_id: String,
    pub length: usize,
    pub content_hash: String,
}

/// Resumen de una sesión: ritmo de escritura, pausas y pegados.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Resumen {
    pub registros: usize,
    pub duracion_s: f64,
    pub caracteres_tipeados: usize,
    pub caracteres_borrados: usize,
    pub caracteres_pegados: usize,
    /// Caracteres tipeados (sin pegados) por minuto de sesión activa.
    pub velocidad_cpm: f64,
    /// Pausas sin eventos de más de 30 s: (desde, segundos).
    pub pausas_largas: Vec<(u64, f64)>,
    pub pegados: Vec<Pegado>,
    pub guardados: usize,
    pub comandos: Vec<(String, Option<i32>)>,
    /// Caracteres tipeados (sin pegados) en cada minuto de la sesión.
    pub tipeado_por_minuto: Vec<u32>,
    /// Caracteres pegados en cada minuto de la sesión.
    pub pegado_por_minuto: Vec<u32>,
}

pub const PAUSA_LARGA_S: f64 = 30.0;

/// Agrupa una serie en `ancho` columnas (sumando) para dibujarla.
pub fn agrupar(valores: &[u32], ancho: usize) -> Vec<u32> {
    if ancho == 0 || valores.is_empty() {
        return vec![];
    }
    if valores.len() <= ancho {
        return valores.to_vec();
    }
    let por = valores.len().div_ceil(ancho);
    valores.chunks(por).map(|c| c.iter().sum()).collect()
}

/// Gráfico de una línea con ▁▂▃▄▅▆▇█ (cero = espacio).
pub fn sparkline(valores: &[u32]) -> String {
    const NIVELES: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let max = valores.iter().copied().max().unwrap_or(0).max(1);
    valores
        .iter()
        .map(|&v| {
            if v == 0 {
                ' '
            } else {
                NIVELES[((v as usize * 8 - 1) / max as usize).min(7)]
            }
        })
        .collect()
}

pub fn resumir(registros: &[TelemetryRecord]) -> Resumen {
    let mut r = Resumen {
        registros: registros.len(),
        duracion_s: 0.0,
        caracteres_tipeados: 0,
        caracteres_borrados: 0,
        caracteres_pegados: 0,
        velocidad_cpm: 0.0,
        pausas_largas: Vec::new(),
        pegados: Vec::new(),
        guardados: 0,
        comandos: Vec::new(),
        tipeado_por_minuto: Vec::new(),
        pegado_por_minuto: Vec::new(),
    };
    let t0 = registros.first().map(|r| r.ts_micros).unwrap_or(0);
    let minutos = registros
        .last()
        .map(|u| (u.ts_micros.saturating_sub(t0) / 60_000_000) as usize + 1)
        .unwrap_or(0);
    r.tipeado_por_minuto = vec![0; minutos];
    r.pegado_por_minuto = vec![0; minutos];
    let minuto = |ts: u64| (ts.saturating_sub(t0) / 60_000_000) as usize;
    if let (Some(primero), Some(ultimo)) = (registros.first(), registros.last()) {
        r.duracion_s = (ultimo.ts_micros.saturating_sub(primero.ts_micros)) as f64 / 1e6;
    }
    let mut activo_s = 0.0;
    let mut pegado_pendiente = false;
    for (i, reg) in registros.iter().enumerate() {
        if i > 0 {
            let hueco = reg.ts_micros.saturating_sub(registros[i - 1].ts_micros) as f64 / 1e6;
            if hueco > PAUSA_LARGA_S {
                r.pausas_largas.push((registros[i - 1].ts_micros, hueco));
            } else {
                activo_s += hueco;
            }
        }
        match &reg.event {
            TelemetryEvent::BracketedPaste {
                buffer_id,
                length,
                content_hash,
            } => {
                r.caracteres_pegados += length;
                r.pegado_por_minuto[minuto(reg.ts_micros)] += *length as u32;
                r.pegados.push(Pegado {
                    seq: reg.seq,
                    ts_micros: reg.ts_micros,
                    buffer_id: buffer_id.clone(),
                    length: *length,
                    content_hash: content_hash.clone(),
                });
                pegado_pendiente = true;
            }
            TelemetryEvent::TextDelta {
                inserted,
                deleted_len,
                ..
            } => {
                r.caracteres_borrados += deleted_len;
                if pegado_pendiente && !inserted.is_empty() {
                    pegado_pendiente = false;
                } else {
                    let n = inserted.chars().count();
                    r.caracteres_tipeados += n;
                    r.tipeado_por_minuto[minuto(reg.ts_micros)] += n as u32;
                }
            }
            TelemetryEvent::BufferSave { .. } => r.guardados += 1,
            TelemetryEvent::CommandExec { command, exit_code } => {
                r.comandos.push((command.clone(), *exit_code))
            }
            _ => {}
        }
    }
    if activo_s > 0.0 {
        r.velocidad_cpm = r.caracteres_tipeados as f64 / (activo_s / 60.0);
    }
    r
}

#[cfg(test)]
mod tests {

    #[test]
    fn graficos_por_minuto() {
        assert_eq!(sparkline(&[0, 1, 8, 4]), " ▁█▄");
        assert_eq!(agrupar(&[1, 2, 3, 4, 5], 2), vec![6, 9]);
        assert_eq!(agrupar(&[1, 2], 5), vec![1, 2]);
    }

    use super::*;
    use crate::Cadena;

    fn registros(eventos: Vec<(u64, TelemetryEvent)>) -> Vec<TelemetryRecord> {
        let mut cadena = Cadena::default();
        eventos
            .into_iter()
            .map(|(ts, e)| serde_json::from_str(&cadena.sellar(e, ts)).unwrap())
            .collect()
    }

    fn delta(offset: usize, ins: &str, del: usize) -> TelemetryEvent {
        TelemetryEvent::TextDelta {
            buffer_id: "a.c".into(),
            offset,
            inserted: ins.into(),
            deleted_len: del,
        }
    }

    #[test]
    fn reconstruye_el_texto_y_marca_los_pegados() {
        let regs = registros(vec![
            (0, delta(0, "int x;", 0)),
            (1_000_000, delta(4, "", 1)),
            (2_000_000, delta(4, "y", 0)),
            (
                3_000_000,
                TelemetryEvent::BracketedPaste {
                    buffer_id: "a.c".into(),
                    length: 9,
                    content_hash: "h".into(),
                },
            ),
            (3_000_100, delta(6, "\nint z;\n", 0)),
        ]);
        let mut rep = Reproduccion::nueva(regs.clone());
        while rep.avanzar().is_some() {}
        let doc = &rep.documentos["a.c"];
        assert_eq!(doc.como_texto(), "int y;\nint z;\n");
        assert!(doc.pegado[6..].iter().all(|p| *p) && doc.pegado[..6].iter().all(|p| !*p));

        rep.retroceder();
        assert_eq!(rep.documentos["a.c"].como_texto(), "int y;");

        let r = resumir(&regs);
        assert_eq!(
            (
                r.caracteres_tipeados,
                r.caracteres_borrados,
                r.caracteres_pegados
            ),
            (7, 1, 9)
        );
        assert_eq!(r.pegados.len(), 1);
    }

    #[test]
    fn detecta_pausas_largas() {
        let regs = registros(vec![(0, delta(0, "a", 0)), (45_000_000, delta(1, "b", 0))]);
        let r = resumir(&regs);
        assert_eq!(r.pausas_largas.len(), 1);
        assert!((r.pausas_largas[0].1 - 45.0).abs() < 1e-6);
    }
}
