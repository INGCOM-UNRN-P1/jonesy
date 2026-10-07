//! Bitácora forense local de `jonesy` (§9.1 de la especificación).
//!
//! El archivo `.jonesy/telemetry.log` es JSON Lines append-only: cada línea es un
//! [`TelemetryRecord`] con su número de secuencia, el SHA-256 de la línea anterior
//! (`prev_hash`) y el SHA-256 del evento (`payload_hash`). Alterar, borrar o reordenar una
//! línea rompe la cadena y [`verificar`] lo detecta.
//!
//! La escritura no bloquea el teclado: [`Telemetria`] manda los eventos por un canal a un
//! hilo de E/S que vacía el búfer cada 32 eventos o tras 500 ms de inactividad, y con
//! `flush` + `sync_data` inmediatos ante un guardado, un cambio de foco o una señal.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub mod replay;

/// Hash del «registro anterior» del primer registro de una bitácora.
pub const HASH_INICIAL: &str = "0000000000000000000000000000000000000000000000000000000000000000";
/// Eventos acumulados que fuerzan un vaciado del búfer.
pub const LOTE: usize = 32;
/// Inactividad que fuerza un vaciado del búfer.
pub const VENTANA: Duration = Duration::from_millis(500);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TelemetryRecord {
    pub seq: u64,
    pub ts_micros: u64,
    pub prev_hash: String,
    pub event: TelemetryEvent,
    pub payload_hash: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum TelemetryEvent {
    SessionStart { user: String, exam_mode: bool },
    KeyPress { key: String, modifiers: u8 },
    TextDelta { buffer_id: String, offset: usize, inserted: String, deleted_len: usize },
    BracketedPaste { buffer_id: String, length: usize, content_hash: String },
    CursorMove { buffer_id: String, line: usize, col: usize },
    BufferSave { buffer_id: String, total_bytes: usize },
    CommandExec { command: String, exit_code: Option<i32> },
    FocusChange { panel: String, focused: bool },
    SessionEnd { reason: String },
}

impl TelemetryEvent {
    /// Los eventos que piden `flush` + `sync_data` inmediato (§9.1, punto 3).
    pub fn fuerza_sincronizacion(&self) -> bool {
        matches!(
            self,
            TelemetryEvent::BufferSave { .. }
                | TelemetryEvent::FocusChange { .. }
                | TelemetryEvent::SessionEnd { .. }
                | TelemetryEvent::SessionStart { .. }
        )
    }
}

/// SHA-256 en hexadecimal.
pub fn sha256_hex(datos: &[u8]) -> String {
    hex::encode(Sha256::digest(datos))
}

/// JSON canónico de un valor: el de serde, sin espacios y con los campos en el orden de
/// declaración (determinista para estas estructuras).
pub fn json_canonico<T: Serialize>(valor: &T) -> String {
    serde_json::to_string(valor).expect("los eventos de telemetría siempre se pueden serializar")
}

pub fn ahora_micros() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_micros() as u64).unwrap_or(0)
}

/// Estado de la cadena: lo que hace falta para agregar el siguiente registro.
#[derive(Debug, Clone, PartialEq)]
pub struct Cadena {
    pub siguiente_seq: u64,
    pub ultimo_hash: String,
}

impl Default for Cadena {
    fn default() -> Self {
        Cadena { siguiente_seq: 0, ultimo_hash: HASH_INICIAL.to_string() }
    }
}

impl Cadena {
    /// Arma el registro siguiente y devuelve la línea JSON a escribir (sin `\n`).
    pub fn sellar(&mut self, event: TelemetryEvent, ts_micros: u64) -> String {
        let payload_hash = sha256_hex(json_canonico(&event).as_bytes());
        let registro = TelemetryRecord {
            seq: self.siguiente_seq,
            ts_micros,
            prev_hash: self.ultimo_hash.clone(),
            event,
            payload_hash,
        };
        let linea = json_canonico(&registro);
        self.ultimo_hash = sha256_hex(linea.as_bytes());
        self.siguiente_seq += 1;
        linea
    }

    /// Continúa una bitácora existente (reingreso tras un corte): toma la última línea válida.
    pub fn desde_archivo(ruta: &Path) -> std::io::Result<Cadena> {
        let mut cadena = Cadena::default();
        if !ruta.exists() {
            return Ok(cadena);
        }
        for linea in BufReader::new(File::open(ruta)?).lines() {
            let linea = linea?;
            if linea.trim().is_empty() {
                continue;
            }
            if let Ok(registro) = serde_json::from_str::<TelemetryRecord>(&linea) {
                cadena.siguiente_seq = registro.seq + 1;
                cadena.ultimo_hash = sha256_hex(linea.as_bytes());
            }
        }
        Ok(cadena)
    }
}

enum Mensaje {
    Evento(TelemetryEvent, u64),
    Sincronizar(Option<Sender<()>>),
    Cerrar,
}

/// Escritor asíncrono de la bitácora.
pub struct Telemetria {
    tx: Sender<Mensaje>,
    hilo: Option<JoinHandle<()>>,
    ruta: PathBuf,
}

impl Telemetria {
    /// Abre (o continúa) la bitácora en `ruta` y arranca el hilo de E/S.
    pub fn abrir(ruta: impl Into<PathBuf>) -> std::io::Result<Telemetria> {
        let ruta = ruta.into();
        if let Some(dir) = ruta.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let cadena = Cadena::desde_archivo(&ruta)?;
        let archivo = OpenOptions::new().create(true).append(true).open(&ruta)?;
        let (tx, rx) = mpsc::channel();
        let hilo = std::thread::Builder::new()
            .name("jonesy-telemetria".into())
            .spawn(move || bucle_escritura(rx, archivo, cadena))?;
        Ok(Telemetria { tx, hilo: Some(hilo), ruta })
    }

    pub fn ruta(&self) -> &Path {
        &self.ruta
    }

    /// Encola un evento (no bloquea).
    pub fn registrar(&self, evento: TelemetryEvent) {
        let _ = self.tx.send(Mensaje::Evento(evento, ahora_micros()));
    }

    /// Vacía el búfer y sincroniza el archivo; espera a que termine.
    pub fn sincronizar(&self) {
        let (tx, rx) = mpsc::channel();
        if self.tx.send(Mensaje::Sincronizar(Some(tx))).is_ok() {
            let _ = rx.recv_timeout(Duration::from_secs(5));
        }
    }

    /// Cierra la bitácora: vacía, sincroniza y espera al hilo.
    pub fn cerrar(mut self) {
        self.terminar();
    }

    fn terminar(&mut self) {
        let _ = self.tx.send(Mensaje::Cerrar);
        if let Some(hilo) = self.hilo.take() {
            let _ = hilo.join();
        }
    }
}

impl Drop for Telemetria {
    fn drop(&mut self) {
        self.terminar();
    }
}

fn bucle_escritura(rx: Receiver<Mensaje>, archivo: File, mut cadena: Cadena) {
    let mut salida = BufWriter::new(archivo);
    let mut pendientes = 0usize;
    let vaciar = |salida: &mut BufWriter<File>, sincronizar: bool| {
        let _ = salida.flush();
        if sincronizar {
            let _ = salida.get_ref().sync_data();
        }
    };
    loop {
        let mensaje = if pendientes > 0 { rx.recv_timeout(VENTANA) } else { rx.recv().map_err(|_| RecvTimeoutError::Disconnected) };
        match mensaje {
            Ok(Mensaje::Evento(evento, ts)) => {
                let sincronizar = evento.fuerza_sincronizacion();
                let linea = cadena.sellar(evento, ts);
                let _ = writeln!(salida, "{linea}");
                pendientes += 1;
                if sincronizar {
                    vaciar(&mut salida, true);
                    pendientes = 0;
                } else if pendientes >= LOTE {
                    vaciar(&mut salida, false);
                    pendientes = 0;
                }
            }
            Ok(Mensaje::Sincronizar(ack)) => {
                vaciar(&mut salida, true);
                pendientes = 0;
                if let Some(ack) = ack {
                    let _ = ack.send(());
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                vaciar(&mut salida, false);
                pendientes = 0;
            }
            Ok(Mensaje::Cerrar) | Err(RecvTimeoutError::Disconnected) => {
                vaciar(&mut salida, true);
                return;
            }
        }
    }
}

/// Resultado de verificar una bitácora.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Verificacion {
    pub registros: u64,
    pub ultimo_hash: String,
    /// Primer problema encontrado (línea 1-based y descripción); None si la cadena es íntegra.
    pub error: Option<(usize, String)>,
    pub pegados: usize,
    pub caracteres_pegados: usize,
    pub sesiones: usize,
    pub cerrada: bool,
}

impl Verificacion {
    pub fn integra(&self) -> bool {
        self.error.is_none()
    }
}

/// Verifica la cadena completa: secuencia, `prev_hash` y `payload_hash` de cada línea.
pub fn verificar(ruta: &Path) -> std::io::Result<Verificacion> {
    let mut v = Verificacion {
        registros: 0,
        ultimo_hash: HASH_INICIAL.to_string(),
        error: None,
        pegados: 0,
        caracteres_pegados: 0,
        sesiones: 0,
        cerrada: false,
    };
    let mut esperado_seq = 0u64;
    for (i, linea) in BufReader::new(File::open(ruta)?).lines().enumerate() {
        let linea = linea?;
        let n = i + 1;
        if linea.trim().is_empty() {
            continue;
        }
        let registro: TelemetryRecord = match serde_json::from_str(&linea) {
            Ok(r) => r,
            Err(e) => {
                v.error = Some((n, format!("la línea no es un registro válido: {e}")));
                return Ok(v);
            }
        };
        if registro.seq != esperado_seq {
            v.error = Some((n, format!("secuencia {} donde se esperaba {esperado_seq}", registro.seq)));
            return Ok(v);
        }
        if registro.prev_hash != v.ultimo_hash {
            v.error = Some((n, "el hash del registro anterior no coincide: la bitácora fue alterada".into()));
            return Ok(v);
        }
        if sha256_hex(json_canonico(&registro.event).as_bytes()) != registro.payload_hash {
            v.error = Some((n, "el hash del evento no coincide con su contenido".into()));
            return Ok(v);
        }
        match &registro.event {
            TelemetryEvent::BracketedPaste { length, .. } => {
                v.pegados += 1;
                v.caracteres_pegados += length;
            }
            TelemetryEvent::SessionStart { .. } => {
                v.sesiones += 1;
                v.cerrada = false;
            }
            TelemetryEvent::SessionEnd { .. } => v.cerrada = true,
            _ => {}
        }
        v.ultimo_hash = sha256_hex(linea.as_bytes());
        v.registros += 1;
        esperado_seq += 1;
    }
    Ok(v)
}

/// Lee todos los registros (sin verificar la cadena).
pub fn leer(ruta: &Path) -> std::io::Result<Vec<TelemetryRecord>> {
    let mut registros = Vec::new();
    for linea in BufReader::new(File::open(ruta)?).lines() {
        let linea = linea?;
        if let Ok(r) = serde_json::from_str(&linea) {
            registros.push(r);
        }
    }
    Ok(registros)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn delta(texto: &str, offset: usize) -> TelemetryEvent {
        TelemetryEvent::TextDelta { buffer_id: "a.c".into(), offset, inserted: texto.into(), deleted_len: 0 }
    }

    #[test]
    fn la_cadena_se_verifica_y_detecta_alteraciones() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("telemetry.log");
        {
            let t = Telemetria::abrir(&ruta).unwrap();
            t.registrar(TelemetryEvent::SessionStart { user: "ana".into(), exam_mode: true });
            for i in 0..40 {
                t.registrar(delta("x", i));
            }
            t.registrar(TelemetryEvent::BracketedPaste { buffer_id: "a.c".into(), length: 120, content_hash: "ab".into() });
            t.registrar(TelemetryEvent::SessionEnd { reason: "entrega".into() });
            t.cerrar();
        }
        let v = verificar(&ruta).unwrap();
        assert!(v.integra(), "{:?}", v.error);
        assert_eq!(v.registros, 43);
        assert_eq!((v.pegados, v.caracteres_pegados, v.cerrada), (1, 120, true));

        // Cambiar un carácter de un evento rompe la verificación.
        let texto = std::fs::read_to_string(&ruta).unwrap().replacen("\"offset\":5", "\"offset\":6", 1);
        std::fs::write(&ruta, texto).unwrap();
        let v = verificar(&ruta).unwrap();
        assert_eq!(v.error.as_ref().map(|e| e.0), Some(7));
    }

    #[test]
    fn borrar_una_linea_rompe_la_cadena() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("t.log");
        let t = Telemetria::abrir(&ruta).unwrap();
        for i in 0..5 {
            t.registrar(delta("y", i));
        }
        t.cerrar();
        let lineas: Vec<String> = std::fs::read_to_string(&ruta).unwrap().lines().map(String::from).collect();
        let sin_una: Vec<&str> = lineas.iter().enumerate().filter(|(i, _)| *i != 2).map(|(_, l)| l.as_str()).collect();
        std::fs::write(&ruta, sin_una.join("\n") + "\n").unwrap();
        assert!(!verificar(&ruta).unwrap().integra());
    }

    #[test]
    fn reabrir_continua_la_cadena() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("t.log");
        Telemetria::abrir(&ruta).unwrap().registrar(delta("a", 0));
        {
            let t = Telemetria::abrir(&ruta).unwrap();
            t.registrar(delta("b", 1));
            t.sincronizar();
        }
        let v = verificar(&ruta).unwrap();
        assert!(v.integra(), "{:?}", v.error);
        assert_eq!(v.registros, 2);
    }

    #[test]
    fn el_lote_se_vacia_sin_esperar_al_cierre() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("t.log");
        let t = Telemetria::abrir(&ruta).unwrap();
        for i in 0..LOTE {
            t.registrar(delta("z", i));
        }
        // Sin sincronizar: el lote de 32 o la ventana de 500 ms vacían solos.
        std::thread::sleep(VENTANA + Duration::from_millis(300));
        assert_eq!(std::fs::read_to_string(&ruta).unwrap().lines().count(), LOTE);
        t.cerrar();
    }
}
