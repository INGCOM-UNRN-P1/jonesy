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
    /// Terminó el diagnóstico del entorno (Ayuda → Diagnóstico del entorno): Markdown.
    Doctor(String),
    /// Está listo el reporte de Ayuda → Reportar un problema: Markdown.
    Reporte(String),
    /// Terminaron los casos de Construir → Comparar con las salidas esperadas.
    Casos(Vec<jonesy_core::casos::ResultadoCaso>),
    /// Terminó Depurar → Contar malloc y free por función: el informe o el error.
    ConteoMemoria(Result<String, String>),
    /// Se cargaron los catálogos de reglas de las herramientas.
    Catalogo(Vec<jonesy_core::catalogo::Regla>),
}

pub fn preparar() -> std::io::Result<()> {
    terminal::enable_raw_mode()?;
    let mut out = std::io::stdout();
    execute!(out, EnterAlternateScreen, EnableMouseCapture, cursor::Hide)?;
    // La consola clásica de Windows no implementa bracketed paste ni eventos de foco:
    // si fallan se sigue sin ellos.
    let _ = execute!(out, EnableBracketedPaste);
    let _ = execute!(out, EnableFocusChange);
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
        // Queda para el próximo `jonesy reporte`.
        jonesy_core::reporte::registrar_panico(env!("CARGO_PKG_VERSION"), &info.to_string());
        previo(info);
    }));
}

/// Hilo que lee eventos de la terminal y los reenvía al canal.
pub fn hilo_entrada(tx: Sender<Aviso>) {
    std::thread::Builder::new()
        .name("entrada".into())
        .spawn(move || {
            while let Ok(ev) = event::read() {
                if tx.send(Aviso::Terminal(ev)).is_err() {
                    break;
                }
            }
        })
        .expect("no se pudo crear el hilo de entrada");
}

/// SIGINT, SIGHUP y SIGTERM llegan como avisos (se guardan los buffers y se sella la
/// telemetría antes de salir). Ctrl+C en modo crudo no genera SIGINT.
#[cfg(unix)]
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

/// En Windows cerrar la consola termina el proceso sin señales capturables; el
/// autoguardado del examen cubre ese caso.
#[cfg(not(unix))]
pub fn hilo_senales(_tx: Sender<Aviso>) {}

/// Nombre de una señal para la telemetría (SessionEnd).
pub fn nombre_senal(s: i32) -> &'static str {
    #[cfg(unix)]
    {
        use signal_hook::consts::{SIGHUP, SIGINT};
        match s {
            SIGHUP => "SIGHUP",
            SIGINT => "SIGINT",
            _ => "SIGTERM",
        }
    }
    #[cfg(not(unix))]
    {
        let _ = s;
        "señal"
    }
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
