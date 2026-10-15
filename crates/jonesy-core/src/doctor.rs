//! `jonesy doctor`: la salud del entorno. No sólo qué programas están en el PATH: también
//! si funcionan (gcc compila y corre un programa, gdb lo depura, valgrind lo revisa), si
//! git puede hacer commits, si la terminal muestra bien la interfaz y si el proyecto se
//! puede usar. Agrupado por categoría y con tres niveles: lo requerido (sin eso jonesy no
//! sirve), lo recomendado (sin eso falta algo importante) y lo opcional.
//!
//! Emite el sobre JSON común del ecosistema (`yutani.doctor`, LINEAMIENTOS §3.2):
//!
//! ```json
//! {"schema_version": "1.0.0", "herramienta": "jonesy", "version": "…", "ok": true,
//!  "chequeos": [{"nombre", "requerido", "ok", "detalle", "proposito", "sugerencia"}]}
//! ```

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;

pub const SCHEMA_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Chequeo {
    pub nombre: String,
    /// Área: Compilación, Depuración, Memoria, Git, Terminal, Python, Proyecto…
    pub categoria: String,
    /// "requerido", "recomendado" u "opcional".
    pub nivel: String,
    pub requerido: bool,
    pub ok: bool,
    pub detalle: String,
    pub proposito: String,
    pub sugerencia: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Informe {
    pub schema_version: String,
    pub herramienta: String,
    pub version: String,
    pub ok: bool,
    pub chequeos: Vec<Chequeo>,
}

/// Ruta de `nombre` en el PATH (también `.exe` en Windows).
pub fn buscar(nombre: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|d| {
        [d.join(nombre), d.join(format!("{nombre}.exe"))]
            .into_iter()
            .find(|p| p.is_file())
    })
}

/// Primera línea de `programa --version`, con un tiempo límite (algunas herramientas
/// arrancan lento o esperan entrada).
/// La versión que informa `ruta --version`, o por qué no arranca: termina con error sin
/// decir nada o con un traceback de Python (un paquete roto, un entorno que no es el suyo).
fn version(ruta: &Path, limite: Duration) -> Result<String, String> {
    let Ok(mut hijo) = Command::new(ruta)
        .arg("--version")
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    else {
        return Ok("versión desconocida".into());
    };
    let fin = Instant::now() + limite;
    loop {
        match hijo.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < fin => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = hijo.kill();
                let _ = hijo.wait();
                return Ok("versión desconocida (no respondió)".into());
            }
        }
    }
    let Ok(o) = hijo.wait_with_output() else {
        return Ok("versión desconocida".into());
    };
    let salida = String::from_utf8_lossy(&o.stdout).into_owned();
    let errores = String::from_utf8_lossy(&o.stderr).into_owned();
    let roto = errores.contains("Traceback (most recent call last)")
        || salida.contains("Traceback (most recent call last)")
        || (!o.status.success() && salida.trim().is_empty());
    if roto {
        // La última línea del error suele decir la causa (ModuleNotFoundError: …).
        let causa = errores
            .lines()
            .chain(salida.lines())
            .map(str::trim)
            .rfind(|l| !l.is_empty())
            .unwrap_or("terminó con error")
            .chars()
            .take(120)
            .collect();
        return Err(causa);
    }
    let texto = if salida.trim().is_empty() {
        errores
    } else {
        salida
    };
    Ok(texto
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("versión desconocida")
        .to_string())
}

/// Chequeo de un binario: si está en el PATH y su versión.
pub fn binario(nombre: &str, proposito: &str, requerido: bool, sugerencia: &str) -> Chequeo {
    match buscar(nombre) {
        None => Chequeo {
            nombre: nombre.into(),
            categoria: String::new(),
            nivel: nivel_de(requerido).into(),
            requerido,
            ok: false,
            detalle: "No encontrado en el PATH".into(),
            proposito: proposito.into(),
            sugerencia: sugerencia.into(),
        },
        Some(ruta) => match version(&ruta, Duration::from_secs(5)) {
            Ok(v) => Chequeo {
                nombre: nombre.into(),
                categoria: String::new(),
                nivel: nivel_de(requerido).into(),
                requerido,
                ok: true,
                detalle: format!("{v} ({})", ruta.display()),
                proposito: proposito.into(),
                sugerencia: String::new(),
            },
            Err(causa) => Chequeo {
                nombre: nombre.into(),
                categoria: String::new(),
                nivel: nivel_de(requerido).into(),
                requerido,
                ok: false,
                detalle: format!("está en {} pero no arranca: {causa}", ruta.display()),
                proposito: proposito.into(),
                sugerencia: format!(
                    "Reinstalalo, o revisá el PATH: {} puede ser otra copia ({})",
                    ruta.display(),
                    sugerencia
                ),
            },
        },
    }
}

fn nivel_de(requerido: bool) -> &'static str {
    if requerido {
        "requerido"
    } else {
        "opcional"
    }
}

impl Chequeo {
    fn en(mut self, categoria: &str) -> Chequeo {
        self.categoria = categoria.into();
        self
    }

    /// Recomendado: no impide usar jonesy, pero falta algo importante.
    fn recomendado(mut self) -> Chequeo {
        self.requerido = false;
        self.nivel = "recomendado".into();
        self
    }

    fn nuevo(
        nombre: &str,
        nivel: &str,
        ok: bool,
        detalle: impl Into<String>,
        proposito: &str,
    ) -> Chequeo {
        Chequeo {
            nombre: nombre.into(),
            categoria: String::new(),
            nivel: nivel.into(),
            requerido: nivel == "requerido",
            ok,
            detalle: detalle.into(),
            proposito: proposito.into(),
            sugerencia: String::new(),
        }
    }

    fn sugerir(mut self, s: impl Into<String>) -> Chequeo {
        if !self.ok {
            self.sugerencia = s.into();
        }
        self
    }
}

pub fn informe(version: &str, chequeos: Vec<Chequeo>) -> Informe {
    Informe {
        schema_version: SCHEMA_VERSION.into(),
        herramienta: "jonesy".into(),
        version: version.into(),
        ok: chequeos.iter().filter(|c| c.requerido).all(|c| c.ok),
        chequeos,
    }
}

/// Sugerencia de instalación para una herramienta del ecosistema.
pub fn sugerencia_ecosistema(nombre: &str) -> String {
    format!("uv tool install git+https://github.com/INGCOM-UNRN-P1/{nombre}")
}

/// Orden de las categorías en el informe.
pub const CATEGORIAS: [&str; 9] = [
    "Compilación",
    "Depuración",
    "Memoria",
    "Git",
    "Terminal",
    "Python",
    "Proyecto",
    "Cátedra",
    "Ecosistema",
];

/// Corre un programa con un tiempo límite; (código, stdout+stderr).
fn correr(cmd: &mut Command, limite: Duration) -> Option<(Option<i32>, String)> {
    let mut hijo = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("LC_MESSAGES", "C")
        .spawn()
        .ok()?;
    let fin = Instant::now() + limite;
    loop {
        match hijo.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < fin => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = hijo.kill();
                let _ = hijo.wait();
                return None;
            }
        }
    }
    let o = hijo.wait_with_output().ok()?;
    let mut texto = String::from_utf8_lossy(&o.stdout).into_owned();
    texto.push_str(&String::from_utf8_lossy(&o.stderr));
    Some((o.status.code(), texto))
}

fn primera_linea(t: &str) -> String {
    t.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .chars()
        .take(120)
        .collect()
}

/// Pruebas de humo: compilar y correr un programa, depurarlo y revisarlo con valgrind.
fn humo(v: &mut Vec<Chequeo>) {
    let Ok(dir) = tempdir_simple() else {
        return;
    };
    let fuente = dir.join("hola.c");
    let programa = dir.join(if cfg!(windows) { "hola.exe" } else { "hola" });
    let _ = std::fs::write(
        &fuente,
        "#include <stdio.h>\n#include <stdlib.h>\n\nint main(void)\n{\n    int *p = malloc(sizeof *p);\n    if (p == NULL) {\n        return 1;\n    }\n    *p = 42;\n    printf(\"hola %d\\n\", *p);\n    free(p);\n    return 0;\n}\n",
    );
    let compila = buscar("gcc").and_then(|gcc| {
        correr(
            Command::new(gcc)
                .args(["-std=c11", "-Wall", "-Wextra", "-g", "-O0", "-o"])
                .arg(&programa)
                .arg(&fuente),
            Duration::from_secs(30),
        )
    });
    let compilo = matches!(compila, Some((Some(0), _))) && programa.is_file();
    let corre = compilo
        && correr(&mut Command::new(&programa), Duration::from_secs(10))
            .is_some_and(|(c, t)| c == Some(0) && t.contains("hola 42"));
    v.push(
        Chequeo::nuevo(
            "gcc compila y corre",
            "requerido",
            corre,
            match (&compila, compilo, corre) {
                (None, _, _) => "gcc no respondió".to_string(),
                (_, false, _) => format!(
                    "no compiló un programa de prueba: {}",
                    compila
                        .as_ref()
                        .map(|(_, t)| primera_linea(t))
                        .unwrap_or_default()
                ),
                (_, true, false) => "compiló, pero el programa no corrió bien".to_string(),
                _ => "compiló y corrió un programa con malloc/printf (C11, -Wall -Wextra -g)"
                    .to_string(),
            },
            "F6 y Ctrl+F5",
        )
        .en("Compilación")
        .sugerir("Revisá la instalación de gcc y de la biblioteca de C (glibc-devel / libc6-dev)"),
    );
    if !compilo {
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    if let Some(gdb) = buscar("gdb") {
        let r = correr(
            Command::new(gdb)
                .args(["-batch", "-nx", "-ex", "run", "-ex", "bt"])
                .arg(&programa),
            Duration::from_secs(30),
        );
        let ok = r
            .as_ref()
            .is_some_and(|(_, t)| t.contains("hola 42") && t.contains("exited normally"));
        v.push(
            Chequeo::nuevo(
                "gdb depura",
                "recomendado",
                ok,
                if ok {
                    "corrió el programa de prueba bajo gdb".to_string()
                } else {
                    format!(
                        "gdb no pudo correr el programa: {}",
                        r.map(|(_, t)| primera_linea(&t)).unwrap_or("no respondió".into())
                    )
                },
                "F5: depurar paso a paso",
            )
            .en("Depuración")
            .sugerir("Revisá ptrace (abajo) o los permisos; en contenedores hace falta --cap-add=SYS_PTRACE"),
        );
    }
    if let Some(vg) = buscar("valgrind") {
        let r = correr(
            Command::new(vg)
                .args(["-q", "--leak-check=full", "--error-exitcode=9"])
                .arg(&programa),
            Duration::from_secs(60),
        );
        let ok = matches!(r, Some((Some(0), _)));
        v.push(
            Chequeo::nuevo(
                "valgrind revisa",
                "recomendado",
                ok,
                if ok {
                    "revisó el programa de prueba sin errores".to_string()
                } else {
                    format!(
                        "valgrind falló: {}",
                        r.map(|(_, t)| primera_linea(&t)).unwrap_or("no respondió".into())
                    )
                },
                "Depurar → Revisar memoria con valgrind",
            )
            .en("Memoria")
            .sugerir("Puede faltar la información de depuración de glibc (debuginfo) o el sistema no es compatible"),
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Un directorio temporal propio (sin depender de tempfile).
fn tempdir_simple() -> std::io::Result<PathBuf> {
    let d = std::env::temp_dir().join(format!(
        "jonesy-doctor-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&d)?;
    Ok(d)
}

fn git_config(clave: &str) -> Option<String> {
    let o = Command::new(buscar("git")?)
        .args(["config", "--get", clave])
        .stdin(Stdio::null())
        .output()
        .ok()?;
    let v = String::from_utf8_lossy(&o.stdout).trim().to_string();
    (!v.is_empty()).then_some(v)
}

/// Los chequeos de salud del entorno para `proyecto`. `humo` corre además las pruebas que
/// compilan, depuran y revisan un programa (unos segundos).
pub fn chequeos(
    proyecto: &crate::Proyecto,
    integraciones: &[crate::integraciones::Integracion],
    errores_integraciones: &[String],
    con_humo: bool,
) -> Vec<Chequeo> {
    let raiz = &proyecto.raiz;
    let mut v = vec![
        binario(
            "gcc",
            "Compilar (F6)",
            true,
            "Instalá gcc (sudo dnf install gcc / sudo apt install gcc)",
        )
        .en("Compilación"),
        binario(
            "make",
            "Makefiles del proyecto",
            true,
            "Instalá make (sudo dnf install make / sudo apt install make)",
        )
        .en("Compilación"),
        binario(
            "gdb",
            "Depurador (F5)",
            false,
            "Instalá gdb (sudo dnf install gdb / sudo apt install gdb)",
        )
        .en("Depuración")
        .recomendado(),
        binario(
            "mem-trace",
            "cat-scan: vista de la memoria al depurar",
            false,
            &sugerencia_ecosistema("mem-trace"),
        )
        .en("Depuración"),
        binario(
            "valgrind",
            "Revisar memoria: fugas y accesos inválidos",
            false,
            "Instalá valgrind (sudo dnf install valgrind / sudo apt install valgrind); en Windows no existe",
        )
        .en("Memoria")
        .recomendado(),
        binario("git", "Panel de git, diff y clonar", false, "Instalá git")
            .en("Git")
            .recomendado(),
        binario(
            "python3",
            "Plugins Python (.jonesy/plugins)",
            false,
            "Instalá Python 3",
        )
        .en("Python"),
    ];
    // ptrace: con yama en 2 o 3, gdb no puede depurar.
    if let Ok(t) = std::fs::read_to_string("/proc/sys/kernel/yama/ptrace_scope") {
        let n: u8 = t.trim().parse().unwrap_or(0);
        v.push(
            Chequeo::nuevo(
                "ptrace",
                "recomendado",
                n <= 1,
                format!("kernel.yama.ptrace_scope = {n}"),
                "gdb necesita poder seguir al programa",
            )
            .en("Depuración")
            .sugerir("sudo sysctl kernel.yama.ptrace_scope=1"),
        );
    }
    if con_humo {
        humo(&mut v);
    }
    // Git: sin nombre y correo no se puede hacer commit.
    if buscar("git").is_some() {
        let nombre = git_config("user.name");
        let correo = git_config("user.email");
        v.push(
            Chequeo::nuevo(
                "identidad",
                "recomendado",
                nombre.is_some() && correo.is_some(),
                match (&nombre, &correo) {
                    (Some(n), Some(c)) => format!("{n} <{c}>"),
                    (None, None) => "sin user.name ni user.email".into(),
                    (None, _) => "sin user.name".into(),
                    (_, None) => "sin user.email".into(),
                },
                "Firmar los commits",
            )
            .en("Git")
            .sugerir("git config --global user.name \"Tu Nombre\" && git config --global user.email tu@correo"),
        );
    }
    // Terminal.
    let locale = ["LC_ALL", "LC_CTYPE", "LANG"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()));
    let utf8 = locale.as_deref().is_none_or(|l| {
        let l = l.to_ascii_lowercase();
        l.contains("utf-8") || l.contains("utf8")
    });
    v.push(
        Chequeo::nuevo(
            "UTF-8",
            "recomendado",
            utf8,
            locale.unwrap_or_else(|| "sin locale (se supone UTF-8)".into()),
            "Tildes, cajas y flechas en la interfaz",
        )
        .en("Terminal")
        .sugerir("export LANG=es_AR.UTF-8 (o jonesy --ascii)"),
    );
    let term = std::env::var("TERM").unwrap_or_default();
    v.push(
        Chequeo::nuevo(
            "TERM",
            "recomendado",
            !term.is_empty() && term != "dumb",
            if term.is_empty() {
                "sin TERM".into()
            } else {
                term.clone()
            },
            "Colores, mouse y teclas de función",
        )
        .en("Terminal")
        .sugerir("export TERM=xterm-256color"),
    );
    let colorterm = std::env::var("COLORTERM").unwrap_or_default();
    let color_verdadero = colorterm == "truecolor" || colorterm == "24bit";
    v.push(
        Chequeo::nuevo(
            "color",
            "opcional",
            color_verdadero,
            if color_verdadero {
                "color verdadero (24 bits)".to_string()
            } else {
                format!("COLORTERM={colorterm:?}: puede que la terminal sólo tenga 256 colores")
            },
            "Los temas oscuro, claro, turbo… usan color verdadero",
        )
        .en("Terminal")
        .sugerir(
            "Si los colores se ven mal, usá el tema contraste ([interfaz] tema = \"contraste\")",
        ),
    );
    // Python: versión mínima para el worker.
    if let Some(py) = buscar("python3").or_else(|| buscar("python")) {
        let ver = version(&py, Duration::from_secs(5)).unwrap_or_default();
        let menor = ver.split_whitespace().nth(1).and_then(|n| {
            let mut p = n.split('.');
            Some((
                p.next()?.parse::<u32>().ok()?,
                p.next()?.parse::<u32>().ok()?,
            ))
        });
        if let Some((ma, mi)) = menor {
            v.push(
                Chequeo::nuevo(
                    "versión",
                    "opcional",
                    (ma, mi) >= (3, 8),
                    ver.clone(),
                    "El worker de plugins necesita Python ≥ 3.8",
                )
                .en("Python")
                .sugerir("Instalá una versión más nueva de Python 3 o fijá JONESY_PYTHON"),
            );
        }
    }
    // Proyecto.
    v.push(
        Chequeo::nuevo(
            ".jonesy.toml",
            "recomendado",
            proyecto.error_config.is_none(),
            match &proyecto.error_config {
                Some(e) => e.clone(),
                None if raiz.join(crate::proyecto::ARCHIVO_CONFIG).is_file() => "válido".into(),
                None => "no hay (se usan los valores por defecto)".into(),
            },
            "Configuración del proyecto",
        )
        .en("Proyecto")
        .sugerir("Corregí el archivo: mientras tenga errores se usan los valores por defecto"),
    );
    let prueba = raiz.join(".jonesy-doctor-escritura");
    let escribe = std::fs::write(&prueba, b"").is_ok();
    let _ = std::fs::remove_file(&prueba);
    v.push(
        Chequeo::nuevo(
            "escritura",
            "requerido",
            escribe,
            if escribe {
                format!("se puede escribir en {}", raiz.display())
            } else {
                format!("no se puede escribir en {}", raiz.display())
            },
            "Guardar archivos, la sesión y la bitácora",
        )
        .en("Proyecto")
        .sugerir("Abrí un directorio tuyo (o revisá los permisos)"),
    );
    let modulos = proyecto.estructura.modulos.len();
    v.push(
        Chequeo::nuevo(
            "estructura",
            "opcional",
            true,
            if modulos > 0 {
                format!("{modulos} módulo(s) con Makefile")
            } else {
                "sin Makefile: F6 compila el archivo activo con gcc".into()
            },
            "Bibliotecas, ejercicios y objetivos de make",
        )
        .en("Proyecto"),
    );
    for e in errores_integraciones {
        v.push(
            Chequeo::nuevo(
                "integración",
                "recomendado",
                false,
                e.clone(),
                "Manifiesto de integración",
            )
            .en("Proyecto")
            .sugerir("Corregí el manifiesto (ver docs/integraciones.md)"),
        );
    }
    // Cátedra.
    let entorno = crate::entorno::detectar(&proyecto.config.interfaz.entorno);
    v.push(
        Chequeo::nuevo(
            "entorno",
            "opcional",
            entorno.is_some(),
            match &entorno {
                Some(e) => format!(
                    "{}{}",
                    e.raiz.display(),
                    if e.gdbinit.is_some() {
                        " (con gdbinit de cátedra)"
                    } else {
                        ""
                    }
                ),
                None => "No encontrado (P1_ENTORNO, ~/p1/entorno)".into(),
            },
            "Comandos y variables de la cátedra en la terminal y el depurador",
        )
        .en("Cátedra")
        .sugerir("https://github.com/INGCOM-UNRN-P1/entorno"),
    );
    // Ecosistema: las herramientas de las integraciones.
    let mut vistos: Vec<String> = ["gcc", "make", "git", "gdb", "valgrind", "python3"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    for i in integraciones {
        if i.binario.is_empty() || vistos.contains(&i.binario) {
            continue;
        }
        vistos.push(i.binario.clone());
        let proposito = if i.nombre == i.binario {
            i.descripcion.clone()
        } else {
            format!("{} ({})", i.descripcion, i.nombre)
        };
        let mut c = binario(
            &i.binario,
            &proposito,
            false,
            &sugerencia_ecosistema(&i.binario),
        )
        .en("Ecosistema");
        if c.ok && !i.activa {
            c.detalle.push_str(&format!(
                " · apagada: [integraciones] activar = [\"{}\"]",
                i.nombre
            ));
        }
        v.push(c);
    }
    // En el orden de las categorías.
    v.sort_by_key(|c| {
        CATEGORIAS
            .iter()
            .position(|x| *x == c.categoria)
            .unwrap_or(99)
    });
    v
}

/// El informe en Markdown, para mostrarlo dentro del IDE.
pub fn markdown(informe: &Informe) -> String {
    let mut s = String::from("# Diagnóstico del entorno\n");
    let mut categoria = "";
    for x in &informe.chequeos {
        if x.categoria != categoria {
            categoria = &x.categoria;
            s.push_str(&format!("\n## {categoria}\n\n"));
        }
        let marca = match (x.ok, x.nivel.as_str()) {
            (true, _) => "✓",
            (false, "requerido") => "✗",
            (false, "recomendado") => "!",
            _ => "·",
        };
        s.push_str(&format!("- {marca} **{}** — {}\n", x.nombre, x.detalle));
        if !x.ok && !x.sugerencia.is_empty() {
            s.push_str(&format!("  → `{}`\n", x.sugerencia));
        }
    }
    let faltan = informe
        .chequeos
        .iter()
        .filter(|x| !x.ok && x.nivel == "requerido")
        .count();
    s.push_str(if faltan > 0 {
        "\n**Falta lo necesario (✗).** Desde una terminal: `jonesy doctor`.\n"
    } else {
        "\nTodo lo necesario está. Las marcas **!** son recomendaciones.\n"
    });
    s
}

/// Texto legible del informe, agrupado por categoría (lo usan la CLI y Ayuda →
/// Diagnóstico del entorno). `color` agrega colores ANSI.
pub fn texto(informe: &Informe, raiz: &Path, color: bool) -> String {
    let c = |codigo: &str, t: &str| {
        if color {
            format!("\x1b[{codigo}m{t}\x1b[0m")
        } else {
            t.to_string()
        }
    };
    let mut s = format!("jonesy {} · {}\n", informe.version, raiz.display());
    let mut categoria = "";
    for x in &informe.chequeos {
        if x.categoria != categoria {
            categoria = &x.categoria;
            s.push_str(&format!("\n{}\n", c("1", categoria)));
        }
        let marca = match (x.ok, x.nivel.as_str()) {
            (true, _) => c("32", "✓"),
            (false, "requerido") => c("31", "✗"),
            (false, "recomendado") => c("33", "!"),
            _ => c("2", "·"),
        };
        s.push_str(&format!("  {marca} {:<20} {}\n", x.nombre, x.detalle));
        if !x.ok && !x.sugerencia.is_empty() {
            s.push_str(&format!("    {} {}\n", c("36", "→"), x.sugerencia));
        }
    }
    let faltan = informe
        .chequeos
        .iter()
        .filter(|x| !x.ok && x.nivel == "requerido")
        .count();
    let avisos = informe
        .chequeos
        .iter()
        .filter(|x| !x.ok && x.nivel == "recomendado")
        .count();
    s.push('\n');
    s.push_str(&match (faltan, avisos) {
        (0, 0) => c(
            "32",
            "Todo en orden: el entorno tiene lo necesario y lo recomendado.",
        ),
        (0, n) => c(
            "33",
            &format!("Se puede usar jonesy; {n} recomendación(es) (!) para tenerlo completo."),
        ),
        (n, _) => c(
            "31",
            &format!("Falta lo necesario ({n} ✗): resolvelo antes de usar jonesy."),
        ),
    });
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sobre_comun() {
        let c = vec![
            binario("cargo", "compilador", true, ""),
            binario("no-existe-jonesy-xyz", "nada", false, "instalalo"),
        ];
        assert!(c[0].ok && !c[1].ok);
        assert_eq!(c[1].sugerencia, "instalalo");
        let i = informe("0.1.0", c);
        assert!(i.ok, "lo que falta no es requerido");
        let json = serde_json::to_value(&i).unwrap();
        assert_eq!(json["schema_version"], "1.0.0");
        assert_eq!(json["herramienta"], "jonesy");
        for k in [
            "nombre",
            "requerido",
            "ok",
            "detalle",
            "proposito",
            "sugerencia",
        ] {
            assert!(json["chequeos"][0].get(k).is_some(), "falta {k}");
        }
        let mal = informe("0.1.0", vec![binario("no-existe-jonesy-xyz", "", true, "")]);
        assert!(!mal.ok);
        assert_eq!(json["chequeos"][0]["nivel"], "requerido");
    }

    #[test]
    #[cfg(unix)]
    fn una_herramienta_rota_no_pasa_por_buena() {
        use std::os::unix::fs::PermissionsExt;
        let d = std::env::temp_dir().join(format!("jonesy-doctor-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let script = |nombre: &str, cuerpo: &str| {
            let p = d.join(nombre);
            std::fs::write(&p, format!("#!/bin/sh\n{cuerpo}\n")).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
            p
        };
        let roto = script(
            "roto",
            "echo 'Traceback (most recent call last):' >&2\necho \"ModuleNotFoundError: No module named 'yutani'\" >&2\nexit 1",
        );
        let sano = script("sano", "echo 'sano 1.0'");
        let mudo = script("mudo", "exit 2");
        let limite = Duration::from_secs(5);
        assert_eq!(
            version(&roto, limite).unwrap_err(),
            "ModuleNotFoundError: No module named 'yutani'"
        );
        assert_eq!(version(&sano, limite).unwrap(), "sano 1.0");
        assert!(version(&mudo, limite).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn salud_del_entorno_con_humo() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join(".jonesy.toml"), "esto [no es toml").unwrap();
        let p = crate::Proyecto::abrir(d.path());
        let v = chequeos(&p, &[], &["rota.toml: error".into()], true);
        let buscar = |n: &str| {
            v.iter()
                .find(|c| c.nombre == n)
                .unwrap_or_else(|| panic!("falta {n}"))
        };
        // El .jonesy.toml roto se informa con su error.
        let cfg = buscar(".jonesy.toml");
        assert!(!cfg.ok && cfg.detalle.contains(".jonesy.toml"));
        assert!(buscar("escritura").ok);
        assert!(!buscar("integración").ok);
        // Con gcc instalado, la prueba de humo compila y corre.
        if super::buscar("gcc").is_some() {
            let g = buscar("gcc compila y corre");
            assert!(g.ok, "{}", g.detalle);
        }
        // Ordenados por categoría.
        let orden: Vec<usize> = v
            .iter()
            .map(|c| CATEGORIAS.iter().position(|x| *x == c.categoria).unwrap())
            .collect();
        assert!(orden.windows(2).all(|w| w[0] <= w[1]), "{orden:?}");
        let t = texto(&informe("0.1.0", v), d.path(), false);
        assert!(t.contains("Compilación") && t.contains("Proyecto"));
        assert!(t.contains("→ Corregí el archivo"));
    }
}
