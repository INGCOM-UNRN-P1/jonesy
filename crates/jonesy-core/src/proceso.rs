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
    // Se decide una vez: el PATH no cambia durante la sesión.
    static INTERPRETE: std::sync::OnceLock<(&'static str, &'static str)> =
        std::sync::OnceLock::new();
    *INTERPRETE.get_or_init(|| {
        if cfg!(windows) && !crate::proyecto::en_path("sh") {
            ("cmd", "/C")
        } else {
            ("sh", "-c")
        }
    })
}

/// Tiempo de CPU (en ticks, 1/100 s) de los descendientes de `pid` (Linux, /proc).
pub fn cpu_de_descendientes(pid: u32) -> u64 {
    let Ok(it) = std::fs::read_dir("/proc") else {
        return 0;
    };
    let mut hijos: std::collections::HashMap<u32, Vec<u32>> = Default::default();
    let mut cpu: std::collections::HashMap<u32, u64> = Default::default();
    for e in it.flatten() {
        let Some(p) = e.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else {
            continue;
        };
        let Ok(t) = std::fs::read_to_string(format!("/proc/{p}/stat")) else {
            continue;
        };
        let Some(resto) = t.rsplit_once(')').map(|x| x.1) else {
            continue;
        };
        let campos: Vec<&str> = resto.split_whitespace().collect();
        let (Some(ppid), Some(u), Some(s)) = (
            campos.get(1).and_then(|x| x.parse::<u32>().ok()),
            campos.get(11).and_then(|x| x.parse::<u64>().ok()),
            campos.get(12).and_then(|x| x.parse::<u64>().ok()),
        ) else {
            continue;
        };
        hijos.entry(ppid).or_default().push(p);
        cpu.insert(p, u + s);
    }
    let mut total = 0;
    let mut pila = hijos.get(&pid).cloned().unwrap_or_default();
    while let Some(p) = pila.pop() {
        total += cpu.get(&p).copied().unwrap_or(0);
        pila.extend(hijos.get(&p).cloned().unwrap_or_default());
    }
    total
}

/// Lanza `comando` con el [`interprete`] en `dir`. Devuelve enseguida; el progreso llega por `tx`.
/// `Fin` incluye toda la salida combinada, para parsear diagnósticos o JSON.
pub fn lanzar(id: u64, comando: &str, dir: &Path, entorno: &[(&str, &str)], tx: Sender<Salida>) {
    lanzar_con_entrada(id, comando, dir, entorno, None, tx)
}

/// Como [`lanzar`], escribiéndole `entrada` por la entrada estándar (el buffer sin guardar
/// para un análisis en vivo, `gaff check --stdin`).
pub fn lanzar_con_entrada(
    id: u64,
    comando: &str,
    dir: &Path,
    entorno: &[(&str, &str)],
    entrada: Option<String>,
    tx: Sender<Salida>,
) {
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
            .stdin(if entrada.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
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
        if let (Some(texto), Some(mut stdin)) = (entrada, hijo.stdin.take()) {
            // En otro hilo: si el programa no lee todo, escribir no bloquea a los lectores.
            lectores.push(std::thread::spawn(move || {
                use std::io::Write;
                let _ = stdin.write_all(texto.as_bytes());
            }));
        }
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

    #[test]
    #[cfg(unix)] // cat
    fn escribe_la_entrada() {
        let (tx, rx) = std::sync::mpsc::channel();
        let texto = "int main(void)\n{\n".repeat(20_000);
        lanzar_con_entrada(8, "cat", Path::new("."), &[], Some(texto.clone()), tx);
        loop {
            if let Salida::Fin { codigo, salida, .. } = rx.recv().unwrap() {
                assert_eq!(codigo, Some(0));
                assert_eq!(salida, texto);
                break;
            }
        }
    }
}
