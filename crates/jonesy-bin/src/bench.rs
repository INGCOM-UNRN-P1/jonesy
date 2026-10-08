//! Benchmarks de §10: miden en el propio binario y comparan con el presupuesto.

use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use jonesy_telemetry::{Telemetria, TelemetryEvent};
use jonesy_tui::{App, Opciones};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn informar(nombre: &str, valor: f64, unidad: &str, presupuesto: f64) -> bool {
    let ok = valor <= presupuesto;
    println!(
        "{nombre}: {valor:.3} {unidad} (presupuesto ≤ {presupuesto} {unidad}) {}",
        if ok { "OK" } else { "EXCEDE" }
    );
    ok
}

fn salida(ok: bool) -> anyhow::Result<ExitCode> {
    Ok(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

/// RSS actual y pico (VmRSS, VmHWM) en MiB de un proceso.
pub fn rss_mib(pid: Option<u32>) -> (f64, f64) {
    let ruta = match pid {
        Some(p) => format!("/proc/{p}/status"),
        None => "/proc/self/status".into(),
    };
    let texto = std::fs::read_to_string(ruta).unwrap_or_default();
    let campo = |n: &str| {
        texto
            .lines()
            .find(|l| l.starts_with(n))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<f64>().ok())
            .map(|kb| kb / 1024.0)
            .unwrap_or(0.0)
    };
    (campo("VmRSS:"), campo("VmHWM:"))
}

fn app_sin_terminal(dir: &Path) -> (App, std::sync::mpsc::Receiver<jonesy_tui::sistema::Aviso>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let op = Opciones {
        raiz: dir.to_path_buf(),
        archivos: vec![],
        examen: false,
        ascii: false,
        telemetria: false,
    };
    (App::nueva(op, tx), rx)
}

pub fn arranque(dir: &Path) -> anyhow::Result<ExitCode> {
    let t = Instant::now();
    let (mut app, _rx) = app_sin_terminal(dir);
    let mut term = Terminal::new(TestBackend::new(120, 30))?;
    term.draw(|f| jonesy_tui::dibujo::dibujar(f, &mut app))?;
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    salida(informar(
        "arranque (proyecto + primer cuadro)",
        ms,
        "ms",
        50.0,
    ))
}

pub fn sesion(dir: &Path, segundos: u64) -> anyhow::Result<ExitCode> {
    let (mut app, rx) = app_sin_terminal(dir);
    let mut term = Terminal::new(TestBackend::new(140, 40))?;
    app.quickdoc_visible = true;
    app.accion(jonesy_tui::accion::Accion::PanelGit);
    app.accion(jonesy_tui::accion::Accion::Terminal);
    let fin = Instant::now() + Duration::from_secs(segundos);
    let mut cuadros = 0;
    while Instant::now() < fin {
        while let Ok(a) = rx.try_recv() {
            app.procesar(a);
        }
        term.draw(|f| jonesy_tui::dibujo::dibujar(f, &mut app))?;
        cuadros += 1;
        std::thread::sleep(Duration::from_millis(100));
    }
    let (rss, pico) = rss_mib(None);
    println!("sesión: {cuadros} cuadros en {segundos} s; RSS actual {rss:.1} MiB");
    salida(informar(
        "RSS pico (consigna + C + git + quick-doc + terminal)",
        pico,
        "MiB",
        30.0,
    ))
}

pub fn doc_lookup(funcion: &str) -> anyhow::Result<ExitCode> {
    let dir = tempfile::tempdir()?;
    let codigo = format!(
        "#include <stdlib.h>\n\nint main(void)\n{{\n    int *p = {funcion}(sizeof(int) * 4, "
    );
    std::fs::write(dir.path().join("main.c"), &codigo)?;
    let (mut app, _rx) = app_sin_terminal(dir.path());
    let fin = app.buf().buf.largo();
    app.buf_mut().buf.mover_a(fin, false);
    let mut term = Terminal::new(TestBackend::new(140, 40))?;
    let n = 200;
    let t = Instant::now();
    let mut titulo = String::new();
    for i in 0..n {
        // Cada iteración simula un cambio de versión (tipeo) y redibuja el panel.
        app.buf_mut().buf.version += 1;
        let (tt, _) = jonesy_tui::dibujo::lineas_quickdoc(&mut app, 40);
        if i == 0 {
            titulo = tt;
        }
    }
    let por_consulta = t.elapsed().as_secs_f64() * 1000.0 / n as f64;
    let t = Instant::now();
    term.draw(|f| jonesy_tui::dibujo::dibujar(f, &mut app))?;
    let cuadro = t.elapsed().as_secs_f64() * 1000.0;
    println!("símbolo detectado: {}", titulo.trim());
    let ok = informar(
        "quick-doc: detección + documentación",
        por_consulta,
        "ms",
        70.0,
    );
    salida(
        informar("quick-doc: cuadro completo", cuadro, "ms", 70.0)
            && ok
            && !titulo.contains("quick-doc"),
    )
}

pub fn telemetria() -> anyhow::Result<ExitCode> {
    let dir = tempfile::tempdir()?;
    let log = dir.path().join("telemetry.log");
    let t = Telemetria::abrir(&log)?;
    let n = 20_000;
    let mut tiempos = Vec::with_capacity(n);
    for i in 0..n {
        let e = TelemetryEvent::TextDelta {
            buffer_id: "main.c".into(),
            offset: i,
            inserted: "x".into(),
            deleted_len: 0,
        };
        let ini = Instant::now();
        t.registrar(e);
        tiempos.push(ini.elapsed());
    }
    let ini = Instant::now();
    t.cerrar();
    let cierre = ini.elapsed().as_secs_f64() * 1000.0;
    tiempos.sort();
    let p50 = tiempos[n / 2].as_secs_f64() * 1000.0;
    let p99 = tiempos[n * 99 / 100].as_secs_f64() * 1000.0;
    let max = tiempos[n - 1].as_secs_f64() * 1000.0;
    let v = jonesy_telemetry::verificar(&log)?;
    println!("{n} eventos; p50 {p50:.4} ms · p99 {p99:.4} ms · máx {max:.3} ms · vaciado final {cierre:.1} ms · cadena {}", if v.integra() { "íntegra" } else { "ROTA" });
    let ok = informar("telemetría: latencia p99 por evento", p99, "ms", 1.0);
    salida(informar("telemetría: latencia máxima por evento", max, "ms", 2.0) && ok && v.integra())
}

pub fn git_status(dir: &Path) -> anyhow::Result<ExitCode> {
    let Some(repo) = jonesy_git::Repo::abrir(dir) else {
        anyhow::bail!("{} no está en un repositorio git", dir.display())
    };
    let _ = repo.estado();
    let mut tiempos = vec![];
    let mut entradas = 0;
    for _ in 0..21 {
        let t = Instant::now();
        entradas = repo.estado().map(|e| e.entradas.len()).unwrap_or(0);
        tiempos.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    tiempos.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!("{entradas} entradas en el estado");
    salida(informar(
        "git status --porcelain=v2 (mediana de 21)",
        tiempos[10],
        "ms",
        30.0,
    ))
}

const MUESTRA_MD: &str = "# Trabajo práctico 3: listas enlazadas

Implementá las operaciones de **lista simplemente enlazada** respetando las
reglas de estilo de la cátedra. Usá `malloc` y verificá *siempre* el retorno.

> Recordá: cada `malloc` tiene su `free`.

## Entregables

- [x] `lista.h` con los prototipos
- [ ] `lista.c` con la implementación
- [ ] pruebas en `test_lista.c`

| Función | Retorno | Complejidad |
|:--------|:-------:|------------:|
| `lista_crear` | `lista_t *` | O(1) |
| `lista_insertar` | `bool` | O(1) |
| `lista_buscar` | `nodo_t *` | O(n) |

```c
nodo_t *nodo = malloc(sizeof(nodo_t));
if (nodo == NULL) {
    return false;
}
```

1. Compilá con `make`.
2. Corré `make test`.
";

pub fn markdown(archivo: Option<&Path>) -> anyhow::Result<ExitCode> {
    let fuente = match archivo {
        Some(p) => std::fs::read_to_string(p)?,
        None => MUESTRA_MD.to_string(),
    };
    let sim = jonesy_tui::tema::Simbolos::unicode();
    let t = Instant::now();
    let doc = jonesy_tui::markdown::renderizar(&fuente, 100, &sim);
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    println!(
        "{} líneas renderizadas, {} enlaces",
        doc.lineas.len(),
        doc.enlaces.len()
    );
    salida(informar(
        "primer render de Markdown con estilos",
        ms,
        "ms",
        15.0,
    ))
}

pub fn plugins(dir: &Path) -> anyhow::Result<ExitCode> {
    let proyecto = jonesy_core::Proyecto::abrir(dir);
    let mut dir_plugins = proyecto.dir_estado().join("plugins");
    let _temporal;
    if !jonesy_plugins::Supervisor::nuevo(&dir_plugins, None).hay_plugins() {
        // Sin plugins en el proyecto se usa uno de ejemplo, para medir el worker.
        let t = tempfile::tempdir()?;
        std::fs::write(
            t.path().join("ejemplo.py"),
            "def al_guardar(archivo, texto):\n    return []\n",
        )?;
        dir_plugins = t.path().to_path_buf();
        _temporal = t;
    }
    let tel_dir = tempfile::tempdir()?;
    let tel = Telemetria::abrir(tel_dir.path().join("telemetry.log"))?;
    tel.registrar(TelemetryEvent::SessionStart {
        user: "bench".into(),
        exam_mode: false,
    });
    let (_, base) = rss_mib(None);
    let mut sup = jonesy_plugins::Supervisor::nuevo(&dir_plugins, None);
    let t = Instant::now();
    let r = sup.al_guardar(Path::new("main.c"), "int main(void) { return 0; }\n");
    let arranque = t.elapsed().as_secs_f64() * 1000.0;
    if let Err(e) = r {
        anyhow::bail!("el worker falló: {e}");
    }
    let (_, propio) = rss_mib(None);
    let (worker, _) = rss_mib(sup.pid());
    tel.cerrar();
    println!("worker lanzado y primera llamada en {arranque:.1} ms; RSS jonesy {propio:.1} MiB (antes {base:.1}) + worker {worker:.1} MiB");
    salida(informar(
        "RSS jonesy + worker Python",
        propio + worker,
        "MiB",
        50.0,
    ))
}
