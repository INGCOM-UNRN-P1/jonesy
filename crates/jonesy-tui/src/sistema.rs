//! Terminal anfitriona: modo crudo, pantalla alternativa, mouse, bracketed paste y foco;
//! restauración ante pánico; hilo de entrada; señales; OSC 52.

use std::io::Write;
use std::sync::mpsc::Sender;

use ratatui::crossterm::event::{
    self, DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
    EnableFocusChange, EnableMouseCapture, Event,
};
use ratatui::crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::crossterm::{cursor, execute};

/// Mensajes que despiertan el bucle principal.
pub enum Aviso {
    Terminal(Event),
    /// Hay salida nueva del deck, de GDB o del PTY del inferior.
    Despertar,
    Proceso(jonesy_core::proceso::Salida),
    Plugins(jonesy_plugins::Respuesta),
    Senal(i32),
}

pub fn preparar() -> std::io::Result<()> {
    terminal::enable_raw_mode()?;
    let mut out = std::io::stdout();
    execute!(
        out,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste,
        EnableFocusChange,
        cursor::Hide
    )?;
    Ok(())
}

pub fn restaurar() {
    let mut out = std::io::stdout();
    let _ = execute!(
        out,
        DisableFocusChange,
        DisableBracketedPaste,
        DisableMouseCapture,
        LeaveAlternateScreen,
        cursor::Show
    );
    let _ = terminal::disable_raw_mode();
    let _ = out.flush();
}

/// Restaura la terminal antes de imprimir el pánico, para no dejarla inutilizable.
pub fn instalar_gancho_panico() {
    let previo = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restaurar();
        previo(info);
    }));
}

/// Hilo que lee eventos de la terminal y los reenvía al canal.
pub fn hilo_entrada(tx: Sender<Aviso>) {
    std::thread::Builder::new()
        .name("entrada".into())
        .spawn(move || loop {
            match event::read() {
                Ok(ev) => {
                    if tx.send(Aviso::Terminal(ev)).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        })
        .expect("no se pudo crear el hilo de entrada");
}

/// SIGINT, SIGHUP y SIGTERM llegan como avisos (se guardan los buffers y se sella la
/// telemetría antes de salir). Ctrl+C en modo crudo no genera SIGINT.
pub fn hilo_senales(tx: Sender<Aviso>) {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
    let Ok(mut senales) = signal_hook::iterator::Signals::new([SIGINT, SIGHUP, SIGTERM]) else {
        return;
    };
    std::thread::Builder::new()
        .name("senales".into())
        .spawn(move || {
            for s in senales.forever() {
                if tx.send(Aviso::Senal(s)).is_err() {
                    break;
                }
            }
        })
        .ok();
}

/// Secuencia OSC 52 para copiar al portapapeles del sistema a través de la terminal (también
/// funciona por SSH). Se escribe directo a stdout, fuera del búfer de ratatui.
pub fn osc52(texto: &str) {
    use base64::Engine;
    let codificado = base64::engine::general_purpose::STANDARD.encode(texto.as_bytes());
    let mut out = std::io::stdout();
    let _ = write!(out, "\x1b]52;c;{codificado}\x07");
    let _ = out.flush();
}

pub fn secuencia_osc52(texto: &str) -> String {
    use base64::Engine;
    format!(
        "\x1b]52;c;{}\x07",
        base64::engine::general_purpose::STANDARD.encode(texto.as_bytes())
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn osc52_en_base64() {
        assert_eq!(super::secuencia_osc52("hola"), "\x1b]52;c;aG9sYQ==\x07");
    }
}
