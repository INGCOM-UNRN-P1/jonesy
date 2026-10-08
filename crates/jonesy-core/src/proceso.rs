//! Procesos en segundo plano (compilar, ripley, verificar la entrega) sin bloquear la interfaz.
//!
//! Cada línea de stdout/stderr y el código de salida llegan por un canal; quien lo consume
//! (el bucle de la TUI) decide cuándo redibujar.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;

#[derive(Debug, Clone, PartialEq)]
pub enum Salida {
    Linea {
        id: u64,
        texto: String,
        error: bool,
    },
    Fin {
        id: u64,
        codigo: Option<i32>,
        salida: String,
    },
}

/// Intérprete para una línea de comandos: `sh -c` en Linux, macOS y en Windows con
/// UCRT64/MSYS2 o Git Bash; `cmd /C` en Windows sin `sh` en el PATH.
pub fn interprete() -> (&'static str, &'static str) {
    if cfg!(windows) && !crate::proyecto::en_path("sh") {
        ("cmd", "/C")
    } else {
        ("sh", "-c")
    }
}

/// Lanza `comando` con el [`interprete`] en `dir`. Devuelve enseguida; el progreso llega por `tx`.
/// `Fin` incluye toda la salida combinada, para parsear diagnósticos o JSON.
pub fn lanzar(id: u64, comando: &str, dir: &Path, entorno: &[(&str, &str)], tx: Sender<Salida>) {
    let comando = comando.to_string();
    let dir = dir.to_path_buf();
    let entorno: Vec<(String, String)> = entorno
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    std::thread::spawn(move || {
        let (programa, opcion) = interprete();
        let mut cmd = Command::new(programa);
        cmd.arg(opcion)
            .arg(&comando)
            .current_dir(&dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Salida sin colores ni barras de progreso: se muestra en un panel, no en una terminal.
        cmd.env("NO_COLOR", "1")
            .env("TERM", "dumb")
            .env_remove("FORCE_COLOR")
            .env("COLUMNS", "120");
        for (k, v) in &entorno {
            cmd.env(k, v);
        }
        let mut hijo = match cmd.spawn() {
            Ok(h) => h,
            Err(e) => {
                let texto = format!("No se pudo ejecutar «{comando}»: {e}");
                let _ = tx.send(Salida::Linea {
                    id,
                    texto: texto.clone(),
                    error: true,
                });
                let _ = tx.send(Salida::Fin {
                    id,
                    codigo: None,
                    salida: texto,
                });
                return;
            }
        };
        let (tx_lineas, rx_lineas) = std::sync::mpsc::channel::<(String, bool)>();
        let mut lectores = Vec::new();
        if let Some(out) = hijo.stdout.take() {
            let t = tx_lineas.clone();
            lectores.push(std::thread::spawn(move || {
                for l in BufReader::new(out).lines().map_while(Result::ok) {
                    let _ = t.send((l, false));
                }
            }));
        }
        if let Some(err) = hijo.stderr.take() {
            let t = tx_lineas.clone();
            lectores.push(std::thread::spawn(move || {
                for l in BufReader::new(err).lines().map_while(Result::ok) {
                    let _ = t.send((l, true));
                }
            }));
        }
        drop(tx_lineas);
        let mut todo = String::new();
        for (texto, error) in rx_lineas {
            todo.push_str(&texto);
            todo.push('\n');
            let _ = tx.send(Salida::Linea { id, texto, error });
        }
        for l in lectores {
            let _ = l.join();
        }
        let codigo = hijo.wait().ok().and_then(|s| s.code());
        let _ = tx.send(Salida::Fin {
            id,
            codigo,
            salida: todo,
        });
    });
}

/// Ejecuta y espera (para comandos cortos: git, ripley explain).
pub fn ejecutar(
    programa: &str,
    args: &[&str],
    dir: &Path,
) -> std::io::Result<(i32, String, String)> {
    let salida = Command::new(programa)
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .env_remove("FORCE_COLOR")
        .output()?;
    Ok((
        salida.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&salida.stdout).into_owned(),
        String::from_utf8_lossy(&salida.stderr).into_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lanza_y_junta_la_salida() {
        let (tx, rx) = std::sync::mpsc::channel();
        lanzar(
            7,
            "echo hola&& echo error 1>&2&& exit 3",
            Path::new("."),
            &[],
            tx,
        );
        let mut lineas = 0;
        loop {
            match rx.recv().unwrap() {
                Salida::Linea { id, .. } => {
                    assert_eq!(id, 7);
                    lineas += 1;
                }
                Salida::Fin { codigo, salida, .. } => {
                    assert_eq!(codigo, Some(3));
                    assert!(salida.contains("hola") && salida.contains("error"));
                    break;
                }
            }
        }
        assert_eq!(lineas, 2);
    }
}
