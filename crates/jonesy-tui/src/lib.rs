//! Interfaz de terminal de jonesy.

pub mod accion;
pub mod app;
pub mod atajos;
pub mod ayuda;
pub mod dibujo;
pub mod layout;
pub mod markdown;
pub mod navegador;
pub mod paleta;
pub mod reproductor;
pub mod resaltado;
pub mod sistema;
pub mod tema;
pub mod vt;

pub use app::{App, Opciones};

/// Corre el IDE en la terminal actual hasta que el usuario sale.
pub fn ejecutar(op: Opciones) -> anyhow::Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    jonesy_core::sesion::recientes::registrar(&op.raiz);
    // Entorno de la cátedra: sus comandos y variables para el deck, la compilación y las
    // herramientas (antes de crear hilos, porque modifica el entorno del proceso).
    let config = jonesy_core::Proyecto::abrir(&op.raiz)
        .config
        .interfaz
        .entorno;
    if let Some(e) = jonesy_core::entorno::detectar(&config) {
        e.aplicar();
    }
    let mut app = App::nueva(op, tx.clone());
    sistema::instalar_gancho_panico();
    sistema::preparar()?;
    sistema::hilo_entrada(tx.clone());
    sistema::hilo_senales(tx);
    let backend = ratatui::backend::CrosstermBackend::new(std::io::stdout());
    let resultado = ratatui::Terminal::new(backend)
        .map_err(anyhow::Error::from)
        .and_then(|mut t| {
            if let Ok(s) = t.size() {
                app.tam = (s.width, s.height);
            }
            app.bucle(&mut t, rx)
        });
    if !app.salir || resultado.is_err() {
        app.finalizar("error");
    } else {
        app.finalizar("salida");
    }
    sistema::restaurar();
    resultado
}
