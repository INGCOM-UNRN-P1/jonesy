//! jonesy: IDE de terminal para la cátedra de Programación I.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod bench;
mod verificar;

const AYUDA: &str = "\
jonesy — IDE de terminal para Programación I

Uso:
  jonesy [opciones] [directorio | archivos...]
  jonesy replay <telemetry.log> [--resumen | --json]
  jonesy verificar <telemetry.log | entrega.tar> [--json]
  jonesy doctor [--rapido] [--json] [directorio]
                     salud del entorno: compilar, depurar, memoria, git, terminal,
                     proyecto y ecosistema (--rapido saltea las pruebas de humo)
  jonesy reporte [directorio] [--salida archivo.md]
                     informe para reportar un problema de jonesy: versión, sistema,
                     terminal, doctor rápido, .jonesy.toml, último pánico
  jonesy config exportar [archivo] [directorio]
                     guarda tema, quick-doc, paneles, integraciones y manifiestos
                     propios (jonesy-preferencias.toml) para llevarlos a otra máquina
  jonesy config importar <archivo>
                     los instala como punto de partida de los proyectos

Opciones:
  --exam             modo examen (también JONESY_EXAM_MODE=1)
  --ascii            caracteres ASCII en lugar de los de caja
  --telemetria       registra la bitácora también fuera del examen
  --simulacro [min]  simulacro de examen: cronómetro y avisos, sin entregar (90 min)
  -h, --help         esta ayuda
  -V, --version      versión

Benchmarks (§10 de la especificación; salen con código 1 si exceden el presupuesto):
  --headless --benchmark-startup [dir]
  --benchmark-session <dir> [--segundos N]
  --benchmark-doc-lookup <función>
  --benchmark-telemetry-write
  --benchmark-git-status <dir>
  --benchmark-markdown [archivo.md]
  --benchmark-python-plugins <dir>

Dentro del IDE: F1/Ctrl+K paleta, F10 menú, Ayuda → Atajos de teclado.
";

/// Raíz del proyecto para un archivo: el ancestro más cercano con .jonesy.toml o .git.
fn raiz_de(archivo: &Path) -> PathBuf {
    let absoluto = std::fs::canonicalize(archivo)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default().join(archivo));
    let inicio = absoluto.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut dir = Some(inicio.as_path());
    while let Some(d) = dir {
        if d.join(jonesy_core::proyecto::ARCHIVO_CONFIG).is_file() || d.join(".git").exists() {
            return d.to_path_buf();
        }
        dir = d.parent();
    }
    inicio
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match correr(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("jonesy: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn correr(args: &[String]) -> anyhow::Result<ExitCode> {
    let tiene = |f: &str| args.iter().any(|a| a == f);
    let valor = |f: &str| {
        args.iter()
            .position(|a| a == f)
            .and_then(|i| args.get(i + 1))
            .filter(|v| !v.starts_with("--"))
            .cloned()
    };
    if tiene("-h") || tiene("--help") {
        print!("{AYUDA}");
        return Ok(ExitCode::SUCCESS);
    }
    if tiene("-V") || tiene("--version") {
        println!("jonesy {}", env!("CARGO_PKG_VERSION"));
        return Ok(ExitCode::SUCCESS);
    }
    match args.first().map(String::as_str) {
        Some("replay") => {
            let Some(log) = args.get(1) else {
                anyhow::bail!("uso: jonesy replay <telemetry.log>")
            };
            let log = Path::new(log);
            if tiene("--resumen") || tiene("--json") {
                let registros = jonesy_telemetry::leer(log)?;
                let v = jonesy_telemetry::verificar(log)?;
                let r = jonesy_telemetry::replay::resumir(&registros);
                if tiene("--json") {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &serde_json::json!({"verificacion": v, "resumen": r})
                        )?
                    );
                } else {
                    imprimir_resumen(&r, &v);
                }
                return Ok(if v.integra() {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(1)
                });
            }
            jonesy_tui::reproductor::ejecutar(log)?;
            return Ok(ExitCode::SUCCESS);
        }
        Some("doctor") => {
            let raiz = args
                .iter()
                .skip(1)
                .find(|a| !a.starts_with("--"))
                .map(PathBuf::from)
                .unwrap_or(std::env::current_dir()?);
            let proyecto = jonesy_core::Proyecto::abrir(&raiz);
            let (integraciones, errores) = jonesy_core::integraciones::cargar(
                &proyecto.dir_estado(),
                &proyecto.config.integraciones,
            );
            let informe = jonesy_core::doctor::informe(
                env!("CARGO_PKG_VERSION"),
                jonesy_core::doctor::chequeos(
                    &proyecto,
                    &integraciones,
                    &errores,
                    !tiene("--rapido"),
                ),
            );
            if tiene("--json") {
                println!("{}", serde_json::to_string_pretty(&informe)?);
            } else {
                use std::io::IsTerminal;
                let color =
                    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
                print!(
                    "{}",
                    jonesy_core::doctor::texto(&informe, &proyecto.raiz, color)
                );
            }
            return Ok(if informe.ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            });
        }
        Some("reporte") => {
            let salida = valor("--salida");
            let raiz = args
                .iter()
                .skip(1)
                .filter(|a| !a.starts_with("--") && Some(*a) != salida.as_ref())
                .map(PathBuf::from)
                .next()
                .unwrap_or(std::env::current_dir()?);
            let proyecto = jonesy_core::Proyecto::abrir(&raiz);
            let (integraciones, errores) = jonesy_core::integraciones::cargar(
                &proyecto.dir_estado(),
                &proyecto.config.integraciones,
            );
            let texto = jonesy_core::reporte::generar(
                env!("CARGO_PKG_VERSION"),
                &proyecto,
                &integraciones,
                &errores,
                &jonesy_core::reporte::Contexto::default(),
            );
            match salida {
                Some(s) => {
                    std::fs::write(&s, &texto)?;
                    println!(
                        "Reporte guardado en {s}: revisalo y adjuntalo al aviso del problema."
                    );
                }
                None => print!("{texto}"),
            }
            return Ok(ExitCode::SUCCESS);
        }
        Some("config") => {
            let libres: Vec<&String> = args
                .iter()
                .skip(2)
                .filter(|a| !a.starts_with("--"))
                .collect();
            match args.get(1).map(String::as_str) {
                Some("exportar") => {
                    let archivo = libres
                        .first()
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("jonesy-preferencias.toml"));
                    let raiz = libres
                        .get(1)
                        .map(PathBuf::from)
                        .unwrap_or(std::env::current_dir()?);
                    let proyecto = jonesy_core::Proyecto::abrir(&raiz);
                    let p = jonesy_core::preferencias::actuales(Some(&proyecto.dir_estado()));
                    jonesy_core::preferencias::escribir(&p, &archivo)
                        .map_err(anyhow::Error::msg)?;
                    println!("Exportado en {}: {}.", archivo.display(), p.resumen());
                }
                Some("importar") => {
                    let Some(archivo) = libres.first() else {
                        anyhow::bail!("uso: jonesy config importar <archivo>")
                    };
                    let (p, escritos) = jonesy_core::preferencias::importar(Path::new(archivo))
                        .map_err(anyhow::Error::msg)?;
                    println!("Importado: {}.", p.resumen());
                    for e in escritos {
                        println!("  manifiesto instalado: {e}");
                    }
                    println!("Los proyectos sin sesión arrancan con estas preferencias; en uno ya abierto, Archivo → Configuración → Importar.");
                }
                _ => anyhow::bail!(
                    "uso: jonesy config exportar [archivo] [directorio] | importar <archivo>"
                ),
            }
            return Ok(ExitCode::SUCCESS);
        }
        Some("verificar") => {
            let Some(log) = args.get(1) else {
                anyhow::bail!("uso: jonesy verificar <telemetry.log | entrega.tar> [--json]")
            };
            // Un paquete de entrega: hash, archivos, bitácora y sello.
            if log.ends_with(".tar") {
                let informe = verificar::verificar_paquete(Path::new(log))?;
                if tiene("--json") {
                    println!("{}", serde_json::to_string_pretty(&informe)?);
                } else {
                    println!(
                        "Entrega de {} · {}",
                        informe.usuario.as_deref().unwrap_or("?"),
                        informe.paquete
                    );
                    for c in &informe.chequeos {
                        println!(
                            "{} {}: {}",
                            if c.ok { "✓" } else { "✗" },
                            c.nombre,
                            c.detalle
                        );
                    }
                    println!(
                        "Pegados: {} ({} caracteres)",
                        informe.pegados, informe.caracteres_pegados
                    );
                    println!(
                        "{}",
                        if informe.ok {
                            "La entrega es íntegra."
                        } else {
                            "La entrega NO es íntegra."
                        }
                    );
                }
                return Ok(if informe.ok {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(1)
                });
            }
            let v = jonesy_telemetry::verificar(Path::new(log))?;
            match &v.error {
                None => println!(
                    "✓ cadena íntegra: {} registros, último hash {}",
                    v.registros, v.ultimo_hash
                ),
                Some((l, m)) => println!("✗ cadena rota en la línea {l}: {m}"),
            }
            return Ok(if v.integra() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            });
        }
        _ => {}
    }

    // Benchmarks.
    if tiene("--benchmark-startup") {
        let dir = posicional(args)
            .map(PathBuf::from)
            .unwrap_or(std::env::current_dir()?);
        return bench::arranque(&dir);
    }
    if let Some(dir) = valor("--benchmark-session") {
        let segundos = valor("--segundos")
            .and_then(|s| s.parse().ok())
            .unwrap_or(10);
        return bench::sesion(Path::new(&dir), segundos);
    }
    if let Some(f) = valor("--benchmark-doc-lookup") {
        return bench::doc_lookup(&f);
    }
    if tiene("--benchmark-telemetry-write") {
        return bench::telemetria();
    }
    if let Some(dir) = valor("--benchmark-git-status") {
        return bench::git_status(Path::new(&dir));
    }
    if tiene("--benchmark-markdown") {
        return bench::markdown(valor("--benchmark-markdown").as_deref().map(Path::new));
    }
    if let Some(dir) = valor("--benchmark-python-plugins") {
        return bench::plugins(Path::new(&dir));
    }

    let examen = tiene("--exam")
        || std::env::var("JONESY_EXAM_MODE")
            .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    // El número que sigue a --simulacro son minutos, no un archivo.
    let minutos_simulacro = args
        .iter()
        .position(|a| a == "--simulacro")
        .and_then(|i| args.get(i + 1))
        .filter(|v| v.parse::<u64>().is_ok());
    let posicionales: Vec<&String> = args
        .iter()
        .filter(|a| !a.starts_with("--") && Some(*a) != minutos_simulacro)
        .collect();
    let (raiz, archivos) = match posicionales.first() {
        None => (std::env::current_dir()?, vec![]),
        Some(p) if Path::new(p).is_dir() => (PathBuf::from(p), vec![]),
        Some(p) => (
            raiz_de(Path::new(p)),
            posicionales.iter().map(PathBuf::from).collect(),
        ),
    };
    if !raiz.is_dir() {
        anyhow::bail!("no existe el directorio {}", raiz.display());
    }
    jonesy_tui::ejecutar(jonesy_tui::Opciones {
        raiz,
        archivos,
        examen,
        ascii: tiene("--ascii"),
        telemetria: tiene("--telemetria"),
        bienvenida: true,
        // --simulacro [minutos]: práctica de examen con cronómetro, sin entregar.
        simulacro: tiene("--simulacro").then(|| {
            valor("--simulacro")
                .and_then(|v| v.parse().ok())
                .unwrap_or(90)
        }),
    })?;
    Ok(ExitCode::SUCCESS)
}

fn posicional(args: &[String]) -> Option<&str> {
    args.iter()
        .map(String::as_str)
        .find(|a| !a.starts_with("--"))
}

fn imprimir_resumen(r: &jonesy_telemetry::replay::Resumen, v: &jonesy_telemetry::Verificacion) {
    match &v.error {
        None => println!("Cadena SHA-256: íntegra ({} registros)", v.registros),
        Some((l, m)) => println!("Cadena SHA-256: ROTA en la línea {l}: {m}"),
    }
    println!(
        "Sesiones: {} · cerrada: {}",
        v.sesiones,
        if v.cerrada { "sí" } else { "no" }
    );
    println!("Duración: {:.1} min", r.duracion_s / 60.0);
    println!(
        "Tipeado: {} car. · borrado: {} car. · velocidad: {:.0} car/min",
        r.caracteres_tipeados, r.caracteres_borrados, r.velocidad_cpm
    );
    println!(
        "Pegados: {} ({} car.)",
        r.pegados.len(),
        r.caracteres_pegados
    );
    for p in &r.pegados {
        println!(
            "  seq {} · {} car. en {} · sha256 {}",
            p.seq, p.length, p.buffer_id, p.content_hash
        );
    }
    println!("Pausas de más de 30 s: {}", r.pausas_largas.len());
    if !r.tipeado_por_minuto.is_empty() {
        let ancho = 60;
        let t = jonesy_telemetry::replay::agrupar(&r.tipeado_por_minuto, ancho);
        let p = jonesy_telemetry::replay::agrupar(&r.pegado_por_minuto, ancho);
        println!("Escritura por minuto ({} min):", r.tipeado_por_minuto.len());
        println!("  |{}|", jonesy_telemetry::replay::sparkline(&t));
        let marcas: String = p.iter().map(|&x| if x > 0 { '^' } else { ' ' }).collect();
        if marcas.trim().is_empty() {
            println!("  sin pegados");
        } else {
            println!("  |{marcas}| ^ = pegado");
        }
    }
    println!(
        "Guardados: {} · comandos: {}",
        r.guardados,
        r.comandos.len()
    );
    for (c, codigo) in &r.comandos {
        if !c.starts_with("accion:") {
            println!(
                "  {c}{}",
                codigo.map(|x| format!(" → {x}")).unwrap_or_default()
            );
        }
    }
}
