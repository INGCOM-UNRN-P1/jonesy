//! Casos de entrada y salida esperada de un ejercicio, y la medición de una corrida.
//!
//! Un caso es un archivo de entrada con su salida esperada, junto al ejercicio o en su
//! carpeta `casos/`:
//!
//! - `entrada1.txt` ↔ `salida1.txt` (también `entrada.txt` ↔ `salida.txt`);
//! - `nombre.in` ↔ `nombre.out`.
//!
//! Sirven para Ctrl+F5 con la entrada desde un archivo (sin tipearla cada vez) y para
//! Construir → Comparar con las salidas esperadas, que corre todos y compara.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub struct Caso {
    pub nombre: String,
    pub entrada: PathBuf,
    pub esperada: Option<PathBuf>,
}

/// Los casos de `dir` (y de `dir/casos`), ordenados por nombre.
pub fn buscar(dir: &Path) -> Vec<Caso> {
    let mut v = vec![];
    for d in [dir.to_path_buf(), dir.join("casos")] {
        let Ok(it) = std::fs::read_dir(&d) else {
            continue;
        };
        let mut archivos: Vec<PathBuf> = it.filter_map(|e| e.ok().map(|e| e.path())).collect();
        archivos.sort();
        for p in archivos {
            let Some(nombre) = p.file_name().map(|n| n.to_string_lossy().into_owned()) else {
                continue;
            };
            let esperada = if let Some(resto) = nombre.strip_prefix("entrada") {
                Some(d.join(format!("salida{resto}")))
            } else {
                nombre
                    .strip_suffix(".in")
                    .map(|base| d.join(format!("{base}.out")))
            };
            let Some(esperada) = esperada else { continue };
            if !p.is_file() {
                continue;
            }
            let caso = nombre
                .strip_suffix(".txt")
                .or_else(|| nombre.strip_suffix(".in"))
                .unwrap_or(&nombre)
                .to_string();
            v.push(Caso {
                nombre: if d == dir {
                    caso
                } else {
                    format!("casos/{caso}")
                },
                entrada: p,
                esperada: esperada.is_file().then_some(esperada),
            });
        }
    }
    v
}

/// Resultado de correr el programa con una entrada.
#[derive(Debug, Clone, PartialEq)]
pub struct Corrida {
    pub salida: String,
    pub codigo: Option<i32>,
    pub tiempo: Duration,
    /// Se cortó por pasar el tiempo límite (¿ciclo infinito?).
    pub colgado: bool,
}

/// Corre `programa args` en `dir` con `entrada` como stdin y un tiempo límite.
pub fn correr(
    programa: &Path,
    args: &[String],
    dir: &Path,
    entrada: Option<&Path>,
    limite: Duration,
) -> std::io::Result<Corrida> {
    let mut cmd = Command::new(programa);
    cmd.args(args)
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let datos = match entrada {
        Some(e) => Some(std::fs::read(e)?),
        None => None,
    };
    cmd.stdin(if datos.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    let inicio = Instant::now();
    let mut hijo = cmd.spawn()?;
    if let (Some(d), Some(mut stdin)) = (datos, hijo.stdin.take()) {
        // En otro hilo: un programa que no lee toda la entrada no debe trabar esto.
        std::thread::spawn(move || {
            let _ = stdin.write_all(&d);
        });
    }
    // La salida se lee en otro hilo para que un programa que escribe mucho no se trabe.
    let mut out = hijo.stdout.take().expect("stdout entubado");
    let mut err = hijo.stderr.take().expect("stderr entubado");
    let lector = std::thread::spawn(move || {
        let mut s = Vec::new();
        let _ = std::io::Read::read_to_end(&mut out, &mut s);
        let mut e = Vec::new();
        let _ = std::io::Read::read_to_end(&mut err, &mut e);
        s.extend(e);
        s
    });
    let mut colgado = false;
    let codigo = loop {
        match hijo.try_wait()? {
            Some(estado) => break estado.code().or_else(|| senal(&estado)),
            None if inicio.elapsed() >= limite => {
                let _ = hijo.kill();
                let _ = hijo.wait();
                colgado = true;
                break None;
            }
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    };
    let tiempo = inicio.elapsed();
    let salida = String::from_utf8_lossy(&lector.join().unwrap_or_default()).into_owned();
    Ok(Corrida {
        salida,
        codigo,
        tiempo,
        colgado,
    })
}

#[cfg(unix)]
fn senal(e: &std::process::ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    e.signal().map(|s| 128 + s)
}

#[cfg(not(unix))]
fn senal(_: &std::process::ExitStatus) -> Option<i32> {
    None
}

/// Normaliza para comparar: sin espacios al final de cada línea ni líneas vacías al final
/// (lo que no se ve en la consola no debería hacer fallar un caso).
pub fn normalizar(s: &str) -> String {
    let lineas: Vec<&str> = s.lines().map(|l| l.trim_end()).collect();
    let mut fin = lineas.len();
    while fin > 0 && lineas[fin - 1].is_empty() {
        fin -= 1;
    }
    lineas[..fin].join("\n")
}

/// Cuántas líneas difieren entre lo obtenido y lo esperado (ya normalizados).
pub fn lineas_distintas(obtenida: &str, esperada: &str) -> usize {
    let a: Vec<&str> = obtenida.lines().collect();
    let b: Vec<&str> = esperada.lines().collect();
    (0..a.len().max(b.len()))
        .filter(|&i| a.get(i) != b.get(i))
        .count()
}

/// Lo que dejó `/usr/bin/time -f '%e %M'`: (segundos, KiB de memoria máxima).
pub fn leer_medida(texto: &str) -> Option<(f64, u64)> {
    let ultima = texto.lines().rev().find(|l| !l.trim().is_empty())?;
    let mut p = ultima.split_whitespace();
    Some((p.next()?.parse().ok()?, p.next()?.parse().ok()?))
}

/// `/usr/bin/time` de GNU (acepta `-f`), para medir tiempo y memoria máxima.
pub fn time_gnu() -> Option<&'static str> {
    static T: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let ok = *T.get_or_init(|| {
        Path::new("/usr/bin/time").is_file()
            && Command::new("/usr/bin/time")
                .args(["-f", "%e", "true"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success())
    });
    ok.then_some("/usr/bin/time")
}

/// Parte una línea de argumentos como la shell: espacios separan, comillas simples o
/// dobles agrupan.
pub fn partir_argumentos(s: &str) -> Vec<String> {
    let mut v = vec![];
    let mut actual = String::new();
    let mut hay = false;
    let mut comilla: Option<char> = None;
    for c in s.chars() {
        match (comilla, c) {
            (Some(q), c) if c == q => comilla = None,
            (Some(_), c) => actual.push(c),
            (None, '"' | '\'') => {
                comilla = Some(c);
                hay = true;
            }
            (None, c) if c.is_whitespace() => {
                if hay || !actual.is_empty() {
                    v.push(std::mem::take(&mut actual));
                    hay = false;
                }
            }
            (None, c) => actual.push(c),
        }
    }
    if hay || !actual.is_empty() {
        v.push(actual);
    }
    v
}

/// Resultado de un caso al compararlo con su salida esperada.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultadoCaso {
    pub caso: Caso,
    pub corrida: Corrida,
    /// None si el caso no tiene salida esperada.
    pub coincide: Option<bool>,
    pub esperada: String,
    pub distintas: usize,
}

/// Corre todos los casos con `programa` y compara cada salida con la esperada.
pub fn correr_todos(
    programa: &Path,
    args: &[String],
    dir: &Path,
    casos: &[Caso],
    limite: Duration,
) -> Vec<ResultadoCaso> {
    casos
        .iter()
        .map(|c| {
            let corrida =
                correr(programa, args, dir, Some(&c.entrada), limite).unwrap_or(Corrida {
                    salida: String::new(),
                    codigo: None,
                    tiempo: Duration::ZERO,
                    colgado: false,
                });
            let esperada = c
                .esperada
                .as_ref()
                .and_then(|e| std::fs::read_to_string(e).ok())
                .unwrap_or_default();
            let (coincide, distintas) = match &c.esperada {
                None => (None, 0),
                Some(_) => {
                    let (a, b) = (normalizar(&corrida.salida), normalizar(&esperada));
                    let n = lineas_distintas(&a, &b);
                    (Some(n == 0 && !corrida.colgado), n)
                }
            };
            ResultadoCaso {
                caso: c.clone(),
                corrida,
                coincide,
                esperada,
                distintas,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn argumentos_con_comillas() {
        assert_eq!(
            super::partir_argumentos(r#"-n 3 "dos palabras" 'x y' fin"#),
            vec!["-n", "3", "dos palabras", "x y", "fin"]
        );
        assert!(super::partir_argumentos("  ").is_empty());
    }

    use super::*;

    #[test]
    fn encuentra_los_casos() {
        let d = tempfile::tempdir().unwrap();
        for (n, t) in [
            ("entrada1.txt", "1 2"),
            ("salida1.txt", "3"),
            ("entrada2.txt", "5 5"),
            ("main.c", ""),
            ("casos/borde.in", "0 0"),
            ("casos/borde.out", "0"),
        ] {
            let p = d.path().join(n);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, t).unwrap();
        }
        let c = buscar(d.path());
        let nombres: Vec<&str> = c.iter().map(|x| x.nombre.as_str()).collect();
        assert_eq!(nombres, vec!["entrada1", "entrada2", "casos/borde"]);
        assert!(c[0].esperada.is_some() && c[1].esperada.is_none() && c[2].esperada.is_some());
    }

    #[test]
    fn normaliza_y_compara() {
        assert_eq!(normalizar("3  \n\n\n"), "3");
        assert_eq!(lineas_distintas("a\nb\nc", "a\nx"), 2);
        assert_eq!(
            leer_medida("Command terminated by signal 11\n0.09 1320\n"),
            Some((0.09, 1320))
        );
    }

    #[test]
    #[cfg(unix)]
    fn corre_con_entrada_y_tiempo_limite() {
        let d = tempfile::tempdir().unwrap();
        let entrada = d.path().join("e.txt");
        std::fs::write(&entrada, "hola\n").unwrap();
        let c = correr(
            Path::new("/bin/cat"),
            &[],
            d.path(),
            Some(&entrada),
            Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!(
            (c.salida.as_str(), c.codigo, c.colgado),
            ("hola\n", Some(0), false)
        );
        let c = correr(
            Path::new("/bin/sleep"),
            &["5".into()],
            d.path(),
            None,
            Duration::from_millis(200),
        )
        .unwrap();
        assert!(c.colgado);
    }
}
