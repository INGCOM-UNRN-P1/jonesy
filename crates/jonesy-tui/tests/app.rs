//! Pruebas de la aplicación sin terminal: TestBackend + eventos sintéticos.

use std::path::Path;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

use jonesy_tui::app::{Foco, Modal, Pestana};
use jonesy_tui::sistema::Aviso;
use jonesy_tui::{App, Opciones};
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;

fn proyecto(archivos: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (n, c) in archivos {
        let p = dir.path().join(n);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, c).unwrap();
    }
    dir
}

fn app(dir: &Path, examen: bool) -> (App, Receiver<Aviso>) {
    let (tx, rx) = channel();
    let op = Opciones {
        raiz: dir.to_path_buf(),
        archivos: vec![],
        examen,
        ascii: false,
        telemetria: false,
    };
    (App::nueva(op, tx), rx)
}

fn pantalla(app: &mut App, w: u16, h: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| jonesy_tui::dibujo::dibujar(f, app)).unwrap();
    let buf = t.backend().buffer().clone();
    (0..h)
        .map(|y| {
            (0..w)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn tecla(app: &mut App, code: KeyCode, m: KeyModifiers) {
    app.evento(Event::Key(KeyEvent::new(code, m)));
}

fn escribir(app: &mut App, s: &str) {
    for c in s.chars() {
        tecla(app, KeyCode::Char(c), KeyModifiers::NONE);
    }
}

/// Procesa avisos hasta que `cond` se cumpla (o 20 s).
fn esperar(app: &mut App, rx: &Receiver<Aviso>, mut cond: impl FnMut(&App) -> bool) {
    let fin = Instant::now() + Duration::from_secs(20);
    while !cond(app) {
        assert!(
            Instant::now() < fin,
            "tiempo agotado esperando la condición"
        );
        if let Ok(a) = rx.recv_timeout(Duration::from_millis(50)) {
            app.procesar(a);
        }
        // Como el bucle real: temporizadores, sondeos y herramientas programadas.
        app.tic();
    }
}

const MAIN: &str =
    "#include <stdio.h>\n\nint main(void)\n{\n    printf(\"hola\\n\");\n    return 0;\n}\n";

#[test]
fn layout_en_los_tres_modos() {
    let dir = proyecto(&[
        ("main.c", MAIN),
        ("consigna.md", "# Ejercicio 1\n\nEscribí un programa.\n"),
    ]);
    let (mut a, _rx) = app(dir.path(), false);
    let p = pantalla(&mut a, 160, 40);
    assert!(p.contains("Consigna"), "{p}");
    assert!(p.contains("Ejercicio 1"));
    assert!(p.contains("main.c"));
    assert!(p.contains("printf"));
    assert!(p.contains("[F6 Compilar]"));
    assert!(a.areas.quickdoc.is_some() && !a.areas.quickdoc_flotante);

    let p = pantalla(&mut a, 100, 30);
    assert!(p.contains("Consigna") && p.contains("printf"));

    let p = pantalla(&mut a, 80, 24);
    assert!(
        !p.contains("Ejercicio 1"),
        "en compacto sólo se ve el código"
    );
    assert!(p.contains("F8 código"));
    tecla(&mut a, KeyCode::F(7), KeyModifiers::NONE);
    let p = pantalla(&mut a, 80, 24);
    assert!(p.contains("Ejercicio 1"));
}

#[test]
fn editar_guardar_deshacer() {
    let dir = proyecto(&[("main.c", MAIN)]);
    let (mut a, _rx) = app(dir.path(), false);
    pantalla(&mut a, 120, 30);
    tecla(&mut a, KeyCode::End, KeyModifiers::CONTROL);
    escribir(&mut a, "// fin");
    assert!(a.buf().buf.modificado());
    tecla(&mut a, KeyCode::Char('s'), KeyModifiers::CONTROL);
    assert!(!a.buf().buf.modificado());
    assert!(std::fs::read_to_string(dir.path().join("main.c"))
        .unwrap()
        .ends_with("// fin"));
    // El deshacer agrupa el tipeo por palabra: «// » y «fin» son dos pasos.
    tecla(&mut a, KeyCode::Char('z'), KeyModifiers::CONTROL);
    assert!(a.buf().buf.contenido().ends_with("// "));
    tecla(&mut a, KeyCode::Char('z'), KeyModifiers::CONTROL);
    assert_eq!(a.buf().buf.contenido(), MAIN);
    tecla(&mut a, KeyCode::Char('y'), KeyModifiers::CONTROL);
    tecla(&mut a, KeyCode::Char('y'), KeyModifiers::CONTROL);
    assert!(a.buf().buf.contenido().ends_with("// fin"));
    // La sesión recuerda archivo y cursor.
    a.guardar_sesion();
    let (b, _rx) = app(dir.path(), false);
    assert_eq!(b.buf().buf.nombre(), "main.c");
    assert_eq!(b.buf().buf.cursor, a.buf().buf.cursor);
}

#[test]
fn quickdoc_muestra_firma_con_parametro_activo() {
    let dir = proyecto(&[(
        "main.c",
        "#include <string.h>\nint main(void)\n{\n    strncpy(a, \n}\n",
    )]);
    let (mut a, _rx) = app(dir.path(), false);
    let pos = a.buf().buf.linea_columna_a_pos(3, 15);
    a.buf_mut().buf.mover_a(pos, false);
    let (titulo, lineas) = jonesy_tui::dibujo::lineas_quickdoc(&mut a, 60);
    assert!(titulo.contains("strncpy"));
    let firma = &lineas[0];
    let activo = firma
        .spans
        .iter()
        .find(|s| {
            s.style
                .add_modifier
                .contains(ratatui::style::Modifier::BOLD)
        })
        .unwrap();
    assert!(activo.content.contains("src"), "{:?}", firma);
}

#[test]
#[cfg(unix)] // usa sh, make, gcc o el deck
fn compila_y_salta_al_error() {
    if std::process::Command::new("gcc")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let roto = "int main(void)\n{\n    return x;\n}\n";
    let dir = proyecto(&[
        ("prueba.c", roto),
        (".jonesy.toml", "[ripley]\nactivo = false\n"),
    ]);
    let (mut a, rx) = app(dir.path(), false);
    pantalla(&mut a, 120, 30);
    tecla(&mut a, KeyCode::F(6), KeyModifiers::NONE);
    esperar(&mut a, &rx, |a| !a.diag_compilador.is_empty());
    assert_eq!(a.pestana, Pestana::Problemas);
    let d = &a.diag_compilador[0];
    assert_eq!(d.linea, 3);
    a.buf_mut().buf.mover_a(0, false);
    a.foco = Foco::Inferior;
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(a.buf().buf.linea_columna().0, 2);
    let p = pantalla(&mut a, 120, 30);
    assert!(p.contains("Problemas ("), "{p}");
    assert!(p.contains("undeclared") || p.contains("no declarad"), "{p}");
}

#[test]
#[cfg(unix)] // usa sh, make, gcc o el deck
fn examen_registra_pegado_y_entrega() {
    let dir = proyecto(&[
        ("main.c", MAIN),
        ("consigna.md", "# Examen\n"),
        ("Makefile", "all:\n\ttrue\ntest:\n\t@echo 'tests ok'\n"),
        (
            ".jonesy.toml",
            "[examen]\nduracion_min = 90\n[ripley]\nactivo = false\n",
        ),
    ]);
    let (mut a, rx) = app(dir.path(), true);
    let p = pantalla(&mut a, 160, 40);
    assert!(p.contains("EXAMEN"));
    assert!(p.contains("Entregar Examen"));
    assert!(p.contains("1:29:") || p.contains("1:30:00"), "{p}");
    // La consigna abierta en el editor es de solo lectura.
    a.abrir_archivo(&dir.path().join("consigna.md"), true);
    assert!(a.buf().buf.solo_lectura);
    tecla(&mut a, KeyCode::Char('w'), KeyModifiers::CONTROL);

    a.evento(Event::Paste("int pegado = 1;\n".into()));
    tecla(&mut a, KeyCode::F(12), KeyModifiers::NONE);
    assert!(matches!(a.modal, Some(Modal::Confirmar { .. })));
    tecla(&mut a, KeyCode::Char('s'), KeyModifiers::NONE);
    esperar(&mut a, &rx, |a| a.examen_entregado());
    let e = &a.examen.as_ref().unwrap().estado;
    let paquete = dir.path().join(e.paquete.as_ref().unwrap());
    assert!(paquete.is_file());
    assert_eq!(
        e.sha256.as_deref().unwrap(),
        jonesy_core::sha256_hex(&std::fs::read(&paquete).unwrap())
    );
    assert!(a.buffers.iter().all(|b| b.buf.solo_lectura));
    let log = dir.path().join(".jonesy/telemetry.log");
    let v = jonesy_telemetry::verificar(&log).unwrap();
    assert!(v.integra(), "{v:?}");
    assert!(v.cerrada);
    assert_eq!(v.pegados, 1);
    assert!(std::fs::read_to_string(dir.path().join("main.c"))
        .unwrap()
        .contains("int pegado = 1;"));
    let texto = std::fs::read_to_string(&log).unwrap();
    assert!(
        texto.contains("\"BufferSave\""),
        "el autoguardado de la entrega queda registrado"
    );
    assert!(texto.contains("make test"));
}

#[test]
fn paleta_filtra_y_ejecuta() {
    let dir = proyecto(&[("notas.md", "# Notas\n\n| a | b |\n|---|---|\n| 1 | 2 |\n")]);
    let (mut a, _rx) = app(dir.path(), false);
    tecla(&mut a, KeyCode::Char('k'), KeyModifiers::CONTROL);
    escribir(&mut a, "vista previa");
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert!(a.preview_markdown);
    std::thread::sleep(Duration::from_millis(150));
    let p = pantalla(&mut a, 120, 30);
    assert!(p.contains("┌───┬───┐"), "{p}");
}

#[test]
fn breakpoint_por_clic_en_el_margen() {
    use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    let dir = proyecto(&[("main.c", MAIN)]);
    let (mut a, _rx) = app(dir.path(), false);
    pantalla(&mut a, 120, 30);
    let z = a.zonas.editor.unwrap();
    let clic = |x, y| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        })
    };
    a.evento(clic(z.gutter.x, z.gutter.y + 4));
    assert_eq!(a.breakpoints.len(), 1);
    assert_eq!(a.breakpoints.iter().next().unwrap().1, 5);
    let p = pantalla(&mut a, 120, 30);
    assert!(p.contains("●"));
    // Clic en el texto ubica el cursor.
    a.evento(clic(z.texto.x + 4, z.texto.y + 4));
    assert_eq!(a.buf().buf.linea_columna(), (4, 4));
}

#[test]
#[cfg(unix)] // usa sh y las herramientas del ecosistema instaladas
fn herramientas_del_ecosistema_al_guardar() {
    let dir = proyecto(&[(
        "main.c",
        "#include <stdio.h>\n#include <stdlib.h>\nint main(void)\n{\n    char b[4];\n    int *p = malloc(4);\n    gets(b);\n    free(p);\n    return 0;\n}\n",
    )]);
    let (mut a, rx) = app(dir.path(), false);
    let activas: Vec<String> = a
        .integraciones_activas()
        .filter(|x| {
            x.cuando == jonesy_core::integraciones::Cuando::Guardar
                && x.archivos.iter().any(|g| g == "*.c")
        })
        .map(|x| x.nombre.clone())
        .collect();
    if activas.is_empty() {
        return;
    }
    tecla(&mut a, KeyCode::Char('s'), KeyModifiers::CONTROL);
    // Un segundo guardado mientras analizan no lanza otras instancias.
    tecla(&mut a, KeyCode::Char('s'), KeyModifiers::CONTROL);
    assert_eq!(
        a.tareas_en_curso(),
        activas
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
    );
    let p = pantalla(&mut a, 160, 30);
    assert!(
        p.contains(&format!("{}…", activas.join(", "))) || p.contains('…'),
        "{p}"
    );
    let fin = Instant::now() + Duration::from_secs(90);
    while !a.tareas_en_curso().is_empty() && Instant::now() < fin {
        if let Ok(av) = rx.recv_timeout(Duration::from_millis(100)) {
            a.procesar(av);
        }
        // Las lentas (ripley) se lanzan después de DEMORA_LENTAS.
        a.tic();
    }
    assert!(
        a.tareas_en_curso().is_empty(),
        "las herramientas no terminaron"
    );
    let d: Vec<_> = a.diag_herramientas.values().flatten().collect();
    // gaff y kaneda informan con su nombre (spunkmeyer omite lo que ya marca gaff).
    for h in ["gaff", "kaneda"] {
        if activas.iter().any(|x| x == h) {
            assert!(
                d.iter().any(|x| x.herramienta.as_deref() == Some(h)),
                "{h} no informó nada"
            );
        }
    }
    // Lo que informan varias herramientas (ripley repite gaff y kaneda) se muestra una vez.
    let vistos = a.diagnosticos();
    let mut claves: Vec<(usize, String)> = vistos
        .iter()
        .map(|x| {
            (
                x.linea,
                x.codigo.clone().unwrap_or_else(|| x.mensaje.clone()),
            )
        })
        .collect();
    let antes = claves.len();
    claves.sort();
    claves.dedup();
    assert_eq!(antes, claves.len(), "hay hallazgos repetidos");
    if activas.iter().any(|h| h == "kaneda") {
        let kan = vistos
            .iter()
            .find(|x| x.codigo.as_deref() == Some("KAN001"))
            .expect("KAN001");
        assert!(
            kan.enlace.is_some(),
            "se prefiere la versión con enlace al apunte"
        );
    }
    if activas.iter().any(|h| h == "kaneda") {
        let g = d
            .iter()
            .find(|x| x.herramienta.as_deref() == Some("kaneda") && x.linea == 7)
            .expect("kaneda marca gets");
        assert_eq!(g.categoria.as_deref(), Some("seguridad"));
        assert!(g.enlace.is_some());
    }
    if activas.iter().any(|h| h == "ripley") {
        assert!(
            d.iter().any(|x| x.codigo.as_deref() == Some("0x3001h")),
            "{d:?}"
        );
    }
    // El panel de hallazgos agrupa por tema del ecosistema.
    a.accion(jonesy_tui::accion::Accion::PanelReglas);
    let p = pantalla(&mut a, 160, 40);
    assert!(
        p.contains("Seguridad") || p.contains("Memoria") || p.contains("Estilo"),
        "{p}"
    );
    // Herramientas → Integraciones muestra el estado de cada una.
    a.accion(jonesy_tui::accion::Accion::Integraciones);
    let p = pantalla(&mut a, 160, 40);
    assert!(
        p.contains("gaff") && p.contains("callahan") && p.contains("activa"),
        "{p}"
    );
    // Apagar la primera saca sus hallazgos y queda en la sesión.
    let primera = a.integraciones[0].nombre.clone();
    tecla(&mut a, KeyCode::Char(' '), KeyModifiers::NONE);
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert!(!a.diag_herramientas.keys().any(|(h, _)| *h == primera));
    let (b, _rx) = app(dir.path(), false);
    assert!(
        !b.integraciones
            .iter()
            .find(|x| x.nombre == primera)
            .unwrap()
            .activa
    );
}

#[test]
fn pestanas_abrir_varios_cerrar_y_atajos() {
    use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    let dir = proyecto(&[
        ("a.c", "int a;\n"),
        ("b.c", "int b;\n"),
        ("c.c", "int c;\n"),
        ("src/a.c", "int sa;\n"),
    ]);
    let (mut a, _rx) = app(dir.path(), false);
    // Ctrl+O, marcar tres con Tab y abrirlos juntos.
    tecla(&mut a, KeyCode::Char('o'), KeyModifiers::CONTROL);
    for _ in 0..3 {
        tecla(&mut a, KeyCode::Tab, KeyModifiers::NONE);
    }
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    let nombres: Vec<String> = a.buffers.iter().map(|b| b.buf.nombre()).collect();
    assert_eq!(a.buffers.len(), 3, "{nombres:?}");
    a.abrir_archivo(&dir.path().join("src/a.c"), true);
    let p = pantalla(&mut a, 120, 30);
    assert!(
        p.contains("a.c (src)") && p.contains("a.c (.)"),
        "nombres repetidos con directorio: {p}"
    );
    // Alt+1 va a la primera pestaña.
    tecla(&mut a, KeyCode::Char('1'), KeyModifiers::ALT);
    assert_eq!(a.activo, 0);
    tecla(&mut a, KeyCode::Tab, KeyModifiers::CONTROL);
    assert_eq!(a.activo, 1);
    // Clic en la × de la primera pestaña la cierra.
    pantalla(&mut a, 120, 30);
    let (r, _) = a.zonas.cierres_buffers[0];
    a.evento(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: r.x,
        row: r.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(a.buffers.len(), 3);
    // Con muchas pestañas en poco ancho aparece el desborde y la activa queda visible.
    for i in 0..12 {
        let n = format!("archivo_largo_{i}.c");
        std::fs::write(dir.path().join(&n), "").unwrap();
        a.abrir_archivo(&dir.path().join(&n), true);
    }
    let p = pantalla(&mut a, 80, 24);
    assert!(p.contains("‹"), "{p}");
    assert!(
        p.contains("archivo_largo_11.c"),
        "la activa a la vista: {p}"
    );
}

#[test]
fn paneles_se_redimensionan_con_mouse_y_opciones() {
    use jonesy_tui::layout::Sep;
    use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    let dir = proyecto(&[("main.c", MAIN), ("consigna.md", "# Consigna\n")]);
    let (mut a, _rx) = app(dir.path(), false);
    tecla(&mut a, KeyCode::Char('p'), KeyModifiers::ALT); // panel inferior
    pantalla(&mut a, 160, 40);
    let raton = |kind, x, y| {
        Event::Mouse(MouseEvent {
            kind,
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        })
    };
    // Arrastrar el separador consigna|editor 16 columnas a la derecha.
    let s = *a
        .areas
        .separadores
        .iter()
        .find(|s| s.sep == Sep::Consigna)
        .unwrap();
    let ancho_antes = a.areas.consigna.unwrap().width;
    a.evento(raton(MouseEventKind::Down(MouseButton::Left), s.pos, 10));
    a.evento(raton(
        MouseEventKind::Drag(MouseButton::Left),
        s.pos + 16,
        10,
    ));
    a.evento(raton(MouseEventKind::Up(MouseButton::Left), s.pos + 16, 10));
    pantalla(&mut a, 160, 40);
    assert_eq!(a.areas.consigna.unwrap().width, ancho_antes + 16);
    // Subir el borde del panel inferior 4 filas.
    let s = *a
        .areas
        .separadores
        .iter()
        .find(|s| s.sep == Sep::Inferior)
        .unwrap();
    let alto_antes = a.areas.inferior.unwrap().height;
    a.evento(raton(MouseEventKind::Down(MouseButton::Left), 60, s.pos));
    a.evento(raton(
        MouseEventKind::Drag(MouseButton::Left),
        60,
        s.pos - 4,
    ));
    a.evento(raton(MouseEventKind::Up(MouseButton::Left), 60, s.pos - 4));
    pantalla(&mut a, 160, 40);
    assert_eq!(a.areas.inferior.unwrap().height, alto_antes + 4);
    // Ver → Tamaños de paneles: el quick-doc (fila 3) crece de a 5 %.
    a.accion(jonesy_tui::accion::Accion::TamanosPaneles);
    let p = pantalla(&mut a, 160, 40);
    assert!(p.contains("Tamaños de paneles"), "{p}");
    tecla(&mut a, KeyCode::Down, KeyModifiers::NONE);
    tecla(&mut a, KeyCode::Down, KeyModifiers::NONE);
    tecla(&mut a, KeyCode::Right, KeyModifiers::NONE);
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(a.paneles.quickdoc, 30);
    // Se recuerdan en la sesión.
    let (b, _rx) = app(dir.path(), false);
    assert_eq!(b.paneles, a.paneles);
}

#[test]
#[cfg(unix)] // usa sh, make, gcc o el deck
fn navegador_de_archivos_y_proyecto_con_objetivos() {
    let dir = proyecto(&[
        ("Makefile", "# Compila todo\nall:\n\t@$(MAKE) -C ejercicios/ej1\n"),
        (
            "ejercicios/ej1/Makefile",
            "PROG_TARGET = programa\n# Compilar el programa\nall:\n\t@echo compilando ej1\n# Corre las pruebas del ejercicio\ntest:\n\t@echo pruebas ok\n",
        ),
        ("ejercicios/ej1/main.c", MAIN),
        ("tp.sh", "echo -e \"  ${VERDE}add-ex <nombre> [libs]${NC}    Agrega un ejercicio.\"\n"),
    ]);
    let (mut a, rx) = app(dir.path(), false);
    // Alt+E: el árbol revela el archivo activo.
    a.abrir_archivo(&dir.path().join("ejercicios/ej1/main.c"), true);
    tecla(&mut a, KeyCode::Char('e'), KeyModifiers::ALT);
    assert_eq!(a.foco, Foco::Navegador);
    let p = pantalla(&mut a, 160, 40);
    assert!(
        p.contains("Archivos") && p.contains("ej1/") && p.contains("main.c"),
        "{p}"
    );
    assert_eq!(a.explorador.seleccionada().unwrap().nombre, "main.c");
    // n crea un archivo en la carpeta seleccionada y lo abre.
    tecla(&mut a, KeyCode::Char('n'), KeyModifiers::NONE);
    escribir(&mut a, "lista.h");
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert!(dir.path().join("ejercicios/ej1/lista.h").is_file());
    assert_eq!(a.buf().buf.nombre(), "lista.h");
    // Tab pasa a Proyecto: el módulo del archivo activo está desplegado con sus objetivos.
    tecla(&mut a, KeyCode::Char('y'), KeyModifiers::ALT);
    let p = pantalla(&mut a, 160, 40);
    assert!(p.contains("EJERCICIOS") && p.contains("▶ test"), "{p}");
    assert!(p.contains("tp.sh"), "{p}");
    // Bajar hasta «test» y correrlo con Enter: la salida va al panel.
    tecla(&mut a, KeyCode::Down, KeyModifiers::NONE);
    tecla(&mut a, KeyCode::Down, KeyModifiers::NONE);
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    esperar(&mut a, &rx, |a| {
        a.salida.iter().any(|(l, _)| l.contains("terminó bien"))
    });
    assert!(a
        .salida
        .iter()
        .any(|(l, _)| l.contains("make -C ejercicios/ej1 test")));
    assert!(a.salida.iter().any(|(l, _)| l == "pruebas ok"));
    // La paleta lista los objetivos de make.
    tecla(&mut a, KeyCode::Char('k'), KeyModifiers::CONTROL);
    escribir(&mut a, "make ej1 test");
    let p = pantalla(&mut a, 160, 40);
    assert!(p.contains("make: ej1 › test"), "{p}");
    tecla(&mut a, KeyCode::Esc, KeyModifiers::NONE);
    // La barra queda en la sesión.
    a.guardar_sesion();
    let (b, _rx) = app(dir.path(), false);
    assert_eq!(
        b.navegador,
        Some(jonesy_tui::navegador::PestanaNav::Proyecto)
    );
}

#[test]
fn abrir_otro_proyecto_desde_el_dialogo() {
    let base = tempfile::tempdir().unwrap();
    std::env::set_var("JONESY_ESTADO", base.path().join("estado"));
    let a_dir = base.path().join("tp1");
    let b_dir = base.path().join("tp2");
    for d in [&a_dir, &b_dir] {
        std::fs::create_dir_all(d).unwrap();
        std::fs::write(d.join("Makefile"), "all:\n\t@true\n").unwrap();
        std::fs::write(d.join("main.c"), MAIN).unwrap();
    }
    let (mut a, _rx) = app(&a_dir, false);
    a.accion(jonesy_tui::accion::Accion::AbrirProyecto);
    let p = pantalla(&mut a, 140, 40);
    assert!(p.contains("tp2/") && p.contains("proyecto"), "{p}");
    // Filtrar «tp2», entrar y abrirlo.
    escribir(&mut a, "tp2");
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(a.proyecto.raiz, jonesy_core::canonica(&b_dir));
    assert_eq!(a.buf().buf.nombre(), "main.c");
    // En examen no se puede.
    let (mut e, _rx) = app(&a_dir, true);
    e.accion(jonesy_tui::accion::Accion::AbrirProyecto);
    assert!(e.modal.is_none());
}

#[test]
#[cfg(unix)] // usa sh, make, gcc o el deck
fn clonar_un_repositorio_y_abrirlo() {
    let git = |dir: &Path, args: &[&str]| {
        assert!(std::process::Command::new("git")
            .args([
                "-c",
                "user.name=prueba",
                "-c",
                "user.email=p@x",
                "-c",
                "init.defaultBranch=main"
            ])
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap()
            .status
            .success());
    };
    let base = tempfile::tempdir().unwrap();
    std::env::set_var("JONESY_ESTADO", base.path().join("estado"));
    let origen = base.path().join("origen");
    std::fs::create_dir_all(&origen).unwrap();
    std::fs::write(origen.join("main.c"), MAIN).unwrap();
    git(&origen, &["init", "-q"]);
    git(&origen, &["add", "-A"]);
    git(&origen, &["commit", "-qm", "inicial"]);
    let actual = base.path().join("actual");
    std::fs::create_dir_all(&actual).unwrap();
    std::fs::write(actual.join("notas.md"), "# x\n").unwrap();
    let destino = base.path().join("clones");
    std::fs::create_dir_all(&destino).unwrap();

    let (mut a, rx) = app(&actual, false);
    pantalla(&mut a, 140, 40);
    a.accion(jonesy_tui::accion::Accion::GitClonar);
    escribir(&mut a, &origen.to_string_lossy());
    tecla(&mut a, KeyCode::Tab, KeyModifiers::NONE);
    tecla(&mut a, KeyCode::Char('u'), KeyModifiers::CONTROL);
    escribir(&mut a, &destino.to_string_lossy());
    let p = pantalla(&mut a, 140, 40);
    assert!(p.contains("clones/origen"), "{p}");
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    let fin = Instant::now() + Duration::from_secs(20);
    while !matches!(a.modal, Some(Modal::Confirmar { .. })) {
        assert!(Instant::now() < fin, "el clon no terminó");
        if let Ok(av) = rx.recv_timeout(Duration::from_millis(50)) {
            a.procesar(av);
        }
        a.tic();
    }
    tecla(&mut a, KeyCode::Char('s'), KeyModifiers::NONE);
    assert_eq!(
        a.proyecto.raiz,
        jonesy_core::canonica(&destino.join("origen"))
    );
    assert_eq!(a.buf().buf.nombre(), "main.c");
}

#[test]
#[cfg(unix)] // usa sh, make, gcc o el deck
fn autoformato_aplica_y_se_deshace() {
    let dir = proyecto(&[
        ("main.c", "int x=1;\nint y=2;\n"),
        (".jonesy.toml", "[formato]\ncomando = \"printf 'int x = 1;\\\\nint y = 2;\\\\n' > {archivo}\"\n[ripley]\nactivo = false\n"),
    ]);
    let (mut a, rx) = app(dir.path(), false);
    let pos = a.buf().buf.linea_columna_a_pos(1, 3);
    a.buf_mut().buf.mover_a(pos, false);
    tecla(
        &mut a,
        KeyCode::Char('F'),
        KeyModifiers::ALT | KeyModifiers::SHIFT,
    );
    esperar(&mut a, &rx, |a| a.tareas_en_curso().is_empty());
    assert_eq!(a.buf().buf.contenido(), "int x = 1;\nint y = 2;\n");
    assert_eq!(
        a.buf().buf.linea_columna(),
        (1, 3),
        "el cursor queda en la misma línea y columna"
    );
    assert!(
        a.buf().buf.modificado(),
        "formatear no guarda: queda como una edición más"
    );
    assert!(
        !dir.path().join(".jonesy-fmt-main.c").exists(),
        "se borra la copia temporal"
    );
    tecla(&mut a, KeyCode::Char('z'), KeyModifiers::CONTROL);
    assert_eq!(a.buf().buf.contenido(), "int x=1;\nint y=2;\n");
}

#[test]
#[cfg(unix)] // usa sh, make, gcc o el deck
fn autoformato_con_clang_format() {
    if std::process::Command::new("clang-format")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let dir = proyecto(&[
        ("main.c", "int main(void){return 0;}\n"),
        (
            ".jonesy.toml",
            "[formato]\ncomando = \"clang-format -i {archivo}\"\n[ripley]\nactivo = false\n",
        ),
    ]);
    let (mut a, rx) = app(dir.path(), false);
    a.accion(jonesy_tui::accion::Accion::Formatear);
    esperar(&mut a, &rx, |a| a.tareas_en_curso().is_empty());
    assert_eq!(a.buf().buf.contenido(), "int main(void) { return 0; }\n");
}

#[test]
#[cfg(unix)] // usa make
fn documentacion_del_proyecto_a_demanda() {
    let h = "int suma(const int *v, size_t n);\n";
    let c = "#include \"arreglos.h\"\nint main(void)\n{\n    return suma(0, 0);\n}\n";
    let dir = proyecto(&[
        (
            "Makefile",
            "# Genera la documentación\ndoc:\n\t@mkdir -p doc && cp api.fuente doc/api.json\n",
        ),
        ("libs/arreglos/Makefile", "all:\n\t@true\n"),
        ("libs/arreglos/arreglos.h", h),
        ("ejercicios/ej1/Makefile", "all:\n\t@true\n"),
        ("ejercicios/ej1/main.c", c),
        (".jonesy.toml", "[ripley]\nactivo = false\n"),
    ]);
    // Lo que generaría corbel proyecto, con las fuentes y sus SHA-256.
    let sha = |rel: &str| jonesy_core::sha256_hex(&std::fs::read(dir.path().join(rel)).unwrap());
    let api = format!(
        r#"{{"schema_version": "1.0.0",
            "fuentes": [{{"archivo": "libs/arreglos/arreglos.h", "sha256": "{}"}}, {{"archivo": "ejercicios/ej1/main.c", "sha256": "{}"}}],
            "modulos": [{{"nombre": "arreglos", "cabeceras": [{{"archivo": "libs/arreglos/arreglos.h", "funciones": [
              {{"nombre": "suma", "firma": "int suma(const int *v, size_t n)", "brief": "Suma los n valores de v.",
                "parametros": [{{"nombre": "v", "descripcion": "los valores"}}, {{"nombre": "n", "descripcion": "cuántos hay"}}],
                "retorno": "La suma.", "pre": ["v != NULL o n == 0"]}}]}}]}}]}}"#,
        sha("libs/arreglos/arreglos.h"),
        sha("ejercicios/ej1/main.c")
    );
    std::fs::write(dir.path().join("api.fuente"), api).unwrap();

    let (mut a, rx) = app(dir.path(), false);
    a.abrir_archivo(&dir.path().join("ejercicios/ej1/main.c"), true);
    let p = pantalla(&mut a, 160, 40);
    assert!(p.contains("[sin doc: make doc]"), "{p}");

    a.accion(jonesy_tui::accion::Accion::GenerarDoc);
    esperar(&mut a, &rx, |a| a.tareas_en_curso().is_empty());
    assert_eq!(a.estado_doc, jonesy_core::documentacion::EstadoDoc::AlDia);
    let p = pantalla(&mut a, 160, 40);
    assert!(!p.contains("make doc]"), "{p}");

    // El quick-doc muestra la documentación de la biblioteca al llamarla desde el ejercicio.
    let pos = a.buf().buf.linea_columna_a_pos(3, 15);
    a.buf_mut().buf.mover_a(pos, false);
    let (titulo, lineas) = jonesy_tui::dibujo::lineas_quickdoc(&mut a, 60);
    let texto: Vec<String> = lineas
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
        .collect();
    assert!(titulo.contains("suma"), "{titulo}");
    assert!(
        texto.iter().any(|l| l.contains("Suma los n valores de v.")),
        "{texto:?}"
    );
    assert!(
        texto
            .iter()
            .any(|l| l.contains("const int * v — los valores")),
        "{texto:?}"
    );
    assert!(
        texto.iter().any(|l| l.contains("Pre: v != NULL")),
        "{texto:?}"
    );
    assert!(
        texto
            .iter()
            .any(|l| l.contains("arreglos.h") && l.contains("doc/")),
        "{texto:?}"
    );

    // Cambiar la cabecera la deja vieja: se avisa en la barra y en el quick-doc.
    a.abrir_archivo(&dir.path().join("libs/arreglos/arreglos.h"), true);
    tecla(&mut a, KeyCode::End, KeyModifiers::CONTROL);
    escribir(&mut a, "int resta(int a, int b);");
    tecla(&mut a, KeyCode::Char('s'), KeyModifiers::CONTROL);
    assert!(matches!(
        a.estado_doc,
        jonesy_core::documentacion::EstadoDoc::Desactualizada(_)
    ));
    let p = pantalla(&mut a, 160, 40);
    assert!(p.contains("[doc vieja: make doc]"), "{p}");
}

#[test]
fn diff_lado_a_lado_desde_git_y_del_buffer() {
    let dir = proyecto(&[(
        "main.c",
        "int main(void)\n{\n    int x = 1;\n    return x;\n}\n",
    )]);
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args([
                "-c",
                "user.name=p",
                "-c",
                "user.email=p@x",
                "-c",
                "init.defaultBranch=main",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(dir.path())
            .output()
            .unwrap()
    };
    if git(&["init", "-q"]).status.code() != Some(0) {
        return;
    }
    git(&["add", "-A"]);
    git(&["commit", "-qm", "inicial"]);
    std::fs::write(
        dir.path().join("main.c"),
        "int main(void)\n{\n    int x = 2;\n    return x;\n}\n",
    )
    .unwrap();

    let (mut a, _rx) = app(dir.path(), false);
    a.refrescar_git();
    tecla(&mut a, KeyCode::Char('g'), KeyModifiers::ALT);
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert!(matches!(
        a.modal,
        Some(Modal::Diff {
            lado_a_lado: true,
            ..
        })
    ));
    let p = pantalla(&mut a, 140, 30);
    // La versión vieja y la nueva en la misma fila.
    let fila = p
        .lines()
        .find(|l| l.contains("int x = 1;"))
        .expect("falta la línea vieja");
    assert!(fila.contains("int x = 2;"), "{fila}");
    assert!(p.contains("+1") && p.contains("−1"), "{p}");
    // s pasa a unificado: la vieja y la nueva en filas distintas, con - y +.
    tecla(&mut a, KeyCode::Char('s'), KeyModifiers::NONE);
    let p = pantalla(&mut a, 140, 30);
    let vieja = p.lines().find(|l| l.contains("int x = 1;")).unwrap();
    assert!(
        vieja.contains('-') && !vieja.contains("int x = 2;"),
        "{vieja}"
    );
    assert!(
        p.lines()
            .any(|l| l.contains('+') && l.contains("int x = 2;")),
        "{p}"
    );
    tecla(&mut a, KeyCode::Esc, KeyModifiers::NONE);

    // Comparar el buffer sin guardar contra HEAD (la vista elegida se recuerda).
    a.abrir_archivo(&dir.path().join("main.c"), true);
    tecla(&mut a, KeyCode::End, KeyModifiers::CONTROL);
    escribir(&mut a, "// sin guardar");
    a.accion(jonesy_tui::accion::Accion::DiffArchivo);
    assert!(matches!(
        a.modal,
        Some(Modal::Diff {
            lado_a_lado: false,
            ..
        })
    ));
    let p = pantalla(&mut a, 140, 30);
    assert!(
        p.contains("HEAD → editor") && p.contains("// sin guardar"),
        "{p}"
    );
}

#[test]
#[cfg(unix)] // los comandos de la integración de prueba usan sh
fn integracion_propia_del_proyecto_con_acciones_y_f1() {
    let manifiesto = r#"
[[integracion]]
nombre = "eco"
descripcion = "Integración de prueba"
cuando = "guardar"
archivos = ["*.c"]
comando = "printf '%s' '{\"hallazgos\": [{\"codigo\": \"ECO1\", \"categoria\": \"memoria\", \"severidad\": \"error\", \"mensaje\": \"falta free\", \"archivo\": \"main.c\", \"linea\": 2, \"columna\": 1, \"enlace\": \"https://ejemplo/memoria\"}]}'"
explicar = "echo explicación de {codigo}"

  [[integracion.accion]]
  titulo = "Reescribir"
  comando = "printf 'int y;\\n' > {archivo}"
  modo = "recargar"

  [[integracion.accion]]
  titulo = "Contar"
  comando = "wc -l < {archivo}"
  modo = "visor"
"#;
    let dir = proyecto(&[
        ("main.c", "int x;\nint z;\n"),
        (".jonesy/integraciones/eco.toml", manifiesto),
        (
            ".jonesy.toml",
            "[integraciones]\ndesactivar = [\"ripley\", \"gaff\", \"kaneda\", \"spunkmeyer\"]\n",
        ),
    ]);
    let (mut a, rx) = app(dir.path(), false);
    assert!(a
        .integraciones
        .iter()
        .any(|x| x.nombre == "eco" && x.origen.contains("eco.toml")));
    // Al guardar corre y su hallazgo queda con categoría y enlace.
    tecla(&mut a, KeyCode::Char('s'), KeyModifiers::CONTROL);
    esperar(&mut a, &rx, |a| a.tareas_en_curso().is_empty());
    let d = a.diagnosticos();
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].herramienta.as_deref(), Some("eco"));
    assert_eq!(d[0].categoria.as_deref(), Some("memoria"));
    // F1 en la línea del hallazgo usa el «explicar» de la integración y muestra el apunte.
    let pos = a.buf().buf.linea_columna_a_pos(1, 0);
    a.buf_mut().buf.mover_a(pos, false);
    tecla(&mut a, KeyCode::F(1), KeyModifiers::NONE);
    esperar(&mut a, &rx, |a| {
        matches!(a.modal, Some(Modal::Visor { .. }))
    });
    let p = pantalla(&mut a, 120, 30);
    assert!(
        p.contains("explicación de ECO1") && p.contains("https://ejemplo/memoria"),
        "{p}"
    );
    tecla(&mut a, KeyCode::Esc, KeyModifiers::NONE);
    // Alt+H lista las acciones; «Contar» abre un visor con la salida.
    tecla(&mut a, KeyCode::Char('h'), KeyModifiers::ALT);
    let p = pantalla(&mut a, 120, 30);
    assert!(p.contains("Reescribir") && p.contains("Contar"), "{p}");
    escribir(&mut a, "contar");
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    esperar(&mut a, &rx, |a| {
        matches!(a.modal, Some(Modal::Visor { .. }))
    });
    let p = pantalla(&mut a, 120, 30);
    assert!(p.contains('2'), "{p}");
    tecla(&mut a, KeyCode::Esc, KeyModifiers::NONE);
    // «Reescribir» modifica el archivo en disco y el buffer lo toma como una edición.
    a.correr_accion_herramienta("eco", 0);
    esperar(&mut a, &rx, |a| {
        a.tareas_en_curso().is_empty() && a.buf().buf.contenido() == "int y;\n"
    });
    assert!(!a.buf().buf.modificado());
    tecla(&mut a, KeyCode::Char('z'), KeyModifiers::CONTROL);
    assert_eq!(a.buf().buf.contenido(), "int x;\nint z;\n");
}

#[test]
fn reposo_sin_redibujos() {
    let dir = proyecto(&[("main.c", MAIN)]);
    let (mut a, _rx) = app(dir.path(), false);
    pantalla(&mut a, 120, 30);
    a.tic();
    let antes = a.firma_visible();
    for _ in 0..3 {
        a.tic();
    }
    assert_eq!(
        antes,
        a.firma_visible(),
        "sin eventos nada visible cambia: no hay que redibujar"
    );
    a.avisar("hola", jonesy_tui::app::Nivel::Info);
    assert_ne!(antes, a.firma_visible());
}

#[test]
fn enlaces_de_markdown_con_teclado() {
    let mut consigna = String::from(
        "# Ejercicio\n\nUsá [la lista](lista.c) y mirá [las entregas](#entregas).\n\n",
    );
    for i in 0..40 {
        consigna.push_str(&format!("Párrafo {i}.\n\n"));
    }
    consigna.push_str("## Entregas\n\nEl viernes.\n");
    let dir = proyecto(&[
        ("main.c", MAIN),
        ("lista.c", "int lista;\n"),
        ("consigna.md", &consigna),
        ("notas.md", "Ver [otra](otra.md).\n"),
        ("otra.md", "# Otra\n"),
    ]);
    let (mut a, _rx) = app(dir.path(), false);
    pantalla(&mut a, 160, 40);
    tecla(&mut a, KeyCode::F(7), KeyModifiers::NONE);
    assert_eq!(a.foco, Foco::Consigna);
    // Tab elige el primer enlace y Enter lo abre en el editor.
    tecla(&mut a, KeyCode::Tab, KeyModifiers::NONE);
    pantalla(&mut a, 160, 40);
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(a.buf().buf.nombre(), "lista.c");
    // El segundo es un ancla: desplaza la consigna hasta «Entregas».
    tecla(&mut a, KeyCode::F(7), KeyModifiers::NONE);
    tecla(&mut a, KeyCode::Tab, KeyModifiers::NONE);
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    let p = pantalla(&mut a, 160, 40);
    assert!(p.contains("Entregas") && p.contains("El viernes."), "{p}");
    assert!(a.consigna.as_ref().unwrap().desplazamiento > 40);
    // Vista previa: clic para darle foco, Tab al enlace, Enter abre otra.md.
    a.abrir_archivo(&dir.path().join("notas.md"), true);
    tecla(&mut a, KeyCode::Char('p'), KeyModifiers::CONTROL);
    std::thread::sleep(Duration::from_millis(150));
    pantalla(&mut a, 160, 40);
    let r = a.zonas.preview.expect("vista previa visible");
    use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    a.evento(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: r.x + r.width - 2,
        row: r.y + r.height - 1,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(a.foco, Foco::Preview);
    tecla(&mut a, KeyCode::Tab, KeyModifiers::NONE);
    let p = pantalla(&mut a, 160, 40);
    assert!(p.contains("Tab enlaces"), "{p}");
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(a.buf().buf.nombre(), "otra.md");
}

#[test]
fn recuperacion_tras_un_corte() {
    let dir = proyecto(&[("main.c", MAIN), ("otro.c", "int otro;\n")]);
    let estado = dir.path().join(".jonesy");
    let copia = jonesy_core::sesion::ruta_recuperacion(&estado, "main.c");
    std::fs::create_dir_all(copia.parent().unwrap()).unwrap();
    std::fs::write(&copia, "int main(void) { return 42; }\n").unwrap();
    // Al abrir se ofrece recuperar; con «s» el buffer queda con esos cambios, sin guardar.
    let (mut a, _rx) = app(dir.path(), false);
    let p = pantalla(&mut a, 120, 30);
    assert!(
        p.contains("Recuperar cambios") && p.contains("main.c"),
        "{p}"
    );
    tecla(&mut a, KeyCode::Char('s'), KeyModifiers::NONE);
    assert_eq!(a.buf().buf.contenido(), "int main(void) { return 42; }\n");
    assert!(a.buf().buf.modificado());
    assert!(!copia.exists());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("main.c")).unwrap(),
        MAIN,
        "no se guarda solo"
    );
    // Con «n» se descarta; con Esc queda para la próxima.
    std::fs::write(&copia, "otra cosa\n").unwrap();
    let (mut b, _rx) = app(dir.path(), false);
    tecla(&mut b, KeyCode::Esc, KeyModifiers::NONE);
    assert!(copia.exists());
    let (mut c, _rx) = app(dir.path(), false);
    tecla(&mut c, KeyCode::Char('n'), KeyModifiers::NONE);
    assert!(!copia.exists());
}

#[test]
fn explorador_renombra_y_borra() {
    let dir = proyecto(&[
        ("main.c", MAIN),
        ("src/lista.c", "int l;\n"),
        ("src/lista.h", "int l;\n"),
    ]);
    let (mut a, _rx) = app(dir.path(), false);
    a.abrir_archivo(&dir.path().join("src/lista.c"), true);
    tecla(&mut a, KeyCode::Char('e'), KeyModifiers::ALT);
    assert_eq!(a.explorador.seleccionada().unwrap().nombre, "lista.c");
    // F2: renombrar; la pestaña abierta sigue al archivo.
    tecla(&mut a, KeyCode::F(2), KeyModifiers::NONE);
    tecla(&mut a, KeyCode::Char('u'), KeyModifiers::CONTROL);
    escribir(&mut a, "src/listas.c");
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert!(dir.path().join("src/listas.c").is_file() && !dir.path().join("src/lista.c").exists());
    assert_eq!(a.buf().buf.nombre(), "listas.c");
    // Supr: borrar con confirmación; la pestaña se cierra.
    tecla(&mut a, KeyCode::Char('e'), KeyModifiers::ALT);
    tecla(&mut a, KeyCode::Char('e'), KeyModifiers::ALT);
    assert_eq!(a.explorador.seleccionada().unwrap().nombre, "listas.c");
    tecla(&mut a, KeyCode::Delete, KeyModifiers::NONE);
    assert!(matches!(a.modal, Some(Modal::Confirmar { .. })));
    tecla(&mut a, KeyCode::Char('s'), KeyModifiers::NONE);
    assert!(!dir.path().join("src/listas.c").exists());
    assert!(a.buffers.iter().all(|b| b.buf.nombre() != "listas.c"));
    // En examen no se puede.
    let (mut e, _rx) = app(dir.path(), true);
    e.abrir_archivo(&dir.path().join("src/lista.h"), true);
    tecla(&mut e, KeyCode::Char('e'), KeyModifiers::ALT);
    tecla(&mut e, KeyCode::Delete, KeyModifiers::NONE);
    assert!(e.modal.is_none());
    assert!(dir.path().join("src/lista.h").exists());
}

#[test]
fn diff_entre_commits_renombres_binarios_y_lineas_largas() {
    let largo = format!("int main(void) {{ return {}; }}\n", "1 + ".repeat(60) + "0");
    let dir = proyecto(&[("main.c", MAIN), ("lista.c", "int lista;\n")]);
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args([
                "-c",
                "user.name=p",
                "-c",
                "user.email=p@x",
                "-c",
                "init.defaultBranch=main",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(dir.path())
            .output()
            .unwrap()
    };
    if git(&["init", "-q"]).status.code() != Some(0) {
        return;
    }
    git(&["add", "-A"]);
    git(&["commit", "-qm", "uno"]);
    std::fs::write(dir.path().join("main.c"), &largo).unwrap();
    git(&["commit", "-qam", "dos"]);
    std::fs::write(dir.path().join("main.c"), "int main(void) { return 3; }\n").unwrap();
    git(&["commit", "-qam", "tres"]);
    let (mut a, _rx) = app(dir.path(), false);
    // Log: m marca «uno», Enter sobre «tres» compara los dos.
    a.refrescar_git();
    tecla(&mut a, KeyCode::Char('g'), KeyModifiers::ALT);
    tecla(&mut a, KeyCode::Char('l'), KeyModifiers::NONE);
    tecla(&mut a, KeyCode::Down, KeyModifiers::NONE);
    tecla(&mut a, KeyCode::Down, KeyModifiers::NONE);
    tecla(&mut a, KeyCode::Char('m'), KeyModifiers::NONE);
    let p = pantalla(&mut a, 160, 40);
    assert!(p.contains("● "), "{p}");
    tecla(&mut a, KeyCode::Up, KeyModifiers::NONE);
    tecla(&mut a, KeyCode::Up, KeyModifiers::NONE);
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    let p = pantalla(&mut a, 160, 40);
    assert!(
        p.contains("return 0;") && p.contains("return 3;"),
        "diff de uno a tres: {p}"
    );
    tecla(&mut a, KeyCode::Esc, KeyModifiers::NONE);
    // Línea larga: → desplaza en horizontal (el log sigue abierto).
    a.git.sel_log = 1; // «dos»
    a.git.marcado = None;
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    let antes = pantalla(&mut a, 100, 30);
    for _ in 0..20 {
        tecla(&mut a, KeyCode::Right, KeyModifiers::NONE);
    }
    let despues = pantalla(&mut a, 100, 30);
    assert_ne!(antes, despues);
    assert!(despues.contains('‹'), "{despues}");
    tecla(&mut a, KeyCode::Esc, KeyModifiers::NONE);
    // Renombre en el stage y binario.
    git(&["mv", "lista.c", "listas.c"]);
    std::fs::write(dir.path().join("foto.bin"), b"\x00\x01\x02").unwrap();
    a.refrescar_git();
    a.git.log = None;
    let estado = a.git_estado.clone().unwrap();
    a.git.sel = estado
        .entradas
        .iter()
        .position(|e| e.ruta == "listas.c")
        .unwrap();
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert!(
        a.mensaje
            .as_ref()
            .is_some_and(|(m, _, _)| m.contains("sólo cambió el nombre")),
        "{:?}",
        a.mensaje
    );
    a.git.sel = estado
        .entradas
        .iter()
        .position(|e| e.ruta == "foto.bin")
        .unwrap();
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert!(
        a.mensaje
            .as_ref()
            .is_some_and(|(m, _, _)| m.contains("binario")),
        "{:?}",
        a.mensaje
    );
}

#[test]
fn deshacer_el_ultimo_commit() {
    let dir = proyecto(&[("main.c", MAIN)]);
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args([
                "-c",
                "user.name=p",
                "-c",
                "user.email=p@x",
                "-c",
                "init.defaultBranch=main",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(dir.path())
            .output()
            .unwrap()
    };
    if git(&["init", "-q"]).status.code() != Some(0) {
        return;
    }
    git(&["add", "-A"]);
    git(&["commit", "-qm", "uno"]);
    std::fs::write(dir.path().join("main.c"), "int main(void) { return 1; }\n").unwrap();
    git(&["commit", "-qam", "dos"]);
    let (mut a, _rx) = app(dir.path(), false);
    a.refrescar_git();
    tecla(&mut a, KeyCode::Char('g'), KeyModifiers::ALT);
    tecla(&mut a, KeyCode::Char('z'), KeyModifiers::NONE);
    let p = pantalla(&mut a, 140, 30);
    assert!(p.contains("«dos»"), "{p}");
    tecla(&mut a, KeyCode::Char('s'), KeyModifiers::NONE);
    let log = String::from_utf8(git(&["log", "--oneline"]).stdout).unwrap();
    assert_eq!(log.lines().count(), 1, "{log}");
    assert!(a
        .git_estado
        .as_ref()
        .unwrap()
        .entradas
        .iter()
        .any(|e| e.en_stage()));
    // El primer commit no tiene anterior: se avisa.
    tecla(&mut a, KeyCode::Char('z'), KeyModifiers::NONE);
    tecla(&mut a, KeyCode::Char('s'), KeyModifiers::NONE);
    assert!(
        a.mensaje
            .as_ref()
            .is_some_and(|(m, _, _)| m.contains("primer commit")),
        "{:?}",
        a.mensaje
    );
}

#[test]
fn aviso_de_documentacion_solo_si_el_proyecto_la_usa() {
    // Sin objetivo doc, corbel.toml ni doc/: no se avisa aunque corbel esté instalado.
    let dir = proyecto(&[("main.c", MAIN)]);
    let (a, _rx) = app(dir.path(), false);
    assert!(a.aviso_doc().is_none());
    // Forzado desde .jonesy.toml (si hay cómo generarla).
    let dir = proyecto(&[
        ("main.c", MAIN),
        ("Makefile", "doc:\n\t@true\n"),
        (".jonesy.toml", "[documentacion]\naviso = false\n"),
    ]);
    let (a, _rx) = app(dir.path(), false);
    assert!(a.aviso_doc().is_none(), "aviso = false lo silencia");
    let dir = proyecto(&[("main.c", MAIN), ("Makefile", "doc:\n\t@true\n")]);
    let (a, _rx) = app(dir.path(), false);
    assert_eq!(a.aviso_doc(), Some("sin doc"));
}

#[test]
fn exportar_la_traza_de_memoria() {
    let dir = proyecto(&[("main.c", MAIN)]);
    let (mut a, _rx) = app(dir.path(), false);
    a.accion(jonesy_tui::accion::Accion::ExportarMemoria);
    assert!(a
        .mensaje
        .as_ref()
        .is_some_and(|(m, _, _)| m.contains("Todavía no hay una traza")));
    let mt = dir.path().join(".jonesy/memtrace");
    std::fs::create_dir_all(&mt).unwrap();
    std::fs::write(mt.join("reporte.html"), "<html>traza</html>").unwrap();
    std::fs::write(
        mt.join("trace.json"),
        r#"{"meta": {"source": "/tp/main.c"}, "steps": []}"#,
    )
    .unwrap();
    // Sin entorno gráfico en la prueba: sólo se copia.
    std::env::remove_var("DISPLAY");
    std::env::remove_var("WAYLAND_DISPLAY");
    a.accion(jonesy_tui::accion::Accion::ExportarMemoria);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("memoria-main.html")).unwrap(),
        "<html>traza</html>"
    );
}

#[test]
#[cfg(unix)] // usa python3
fn nuevo_plugin_python_desde_el_ide() {
    if std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let dir = proyecto(&[("main.c", MAIN)]);
    let (mut a, rx) = app(dir.path(), false);
    a.accion(jonesy_tui::accion::Accion::NuevoPlugin);
    tecla(&mut a, KeyCode::Char('u'), KeyModifiers::CONTROL);
    escribir(&mut a, ".jonesy/plugins/contador.py");
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    let ruta = dir.path().join(".jonesy/plugins/contador.py");
    assert!(
        std::fs::read_to_string(&ruta)
            .unwrap()
            .contains("def comando_contar_lineas"),
        "plantilla"
    );
    assert_eq!(a.buf().buf.nombre(), "contador.py");
    // Guardarlo lo recarga: su comando aparece en la paleta.
    tecla(&mut a, KeyCode::End, KeyModifiers::CONTROL);
    escribir(&mut a, "# mío");
    tecla(&mut a, KeyCode::Char('s'), KeyModifiers::CONTROL);
    esperar(&mut a, &rx, |a| !a.comandos_plugins.is_empty());
    assert!(a
        .comandos_plugins
        .iter()
        .any(|c| c.plugin == "contador" && c.comando == "contar_lineas"));
    // Un error de sintaxis aparece en Problemas, sobre el plugin y en su línea.
    escribir(&mut a, "\ndef roto(:\n");
    let linea_rota = a.buf().buf.contenido().lines().count();
    tecla(&mut a, KeyCode::Char('s'), KeyModifiers::CONTROL);
    esperar(&mut a, &rx, |a| {
        a.diagnosticos()
            .iter()
            .any(|d| d.mensaje.contains("no se pudo cargar"))
    });
    let d = a.diagnosticos();
    let d = d
        .iter()
        .find(|d| d.mensaje.contains("[contador]"))
        .expect("diagnóstico del plugin");
    assert_eq!(d.archivo.as_deref(), Some(ruta.as_path()));
    assert_eq!(d.linea, linea_rota);
    // Corregido, el error desaparece.
    tecla(&mut a, KeyCode::Backspace, KeyModifiers::NONE);
    for _ in 0.."def roto(:".len() {
        tecla(&mut a, KeyCode::Backspace, KeyModifiers::NONE);
    }
    tecla(&mut a, KeyCode::Char('s'), KeyModifiers::CONTROL);
    esperar(&mut a, &rx, |a| a.diagnosticos().is_empty());
    // En examen no se crean plugins.
    let (mut e, _rx) = app(dir.path(), true);
    e.accion(jonesy_tui::accion::Accion::NuevoPlugin);
    assert!(e.modal.is_none());
}

#[test]
fn tema_se_elige_desde_la_config_y_se_cambia_y_recuerda() {
    use jonesy_tui::tema::{self, Tema};
    let d = proyecto(&[
        ("main.c", "int main(void) { return 0; }\n"),
        (".jonesy.toml", "[interfaz]\ntema = \"claro\"\n"),
    ]);
    let (mut a, _rx) = app(d.path(), false);
    assert_eq!(tema::actual(), Tema::Claro);
    assert_eq!(tema::fuerte(), ratatui::style::Color::Black);
    pantalla(&mut a, 120, 40);
    a.accion(jonesy_tui::accion::Accion::CambiarTema);
    assert_eq!(tema::actual(), Tema::Contraste);
    // Contraste sólo usa los 16 colores ANSI.
    assert!(!matches!(tema::acento(), ratatui::style::Color::Rgb(..)));
    drop(a);
    // La sesión recuerda el elegido por encima de la config.
    let (_b, _rx) = app(d.path(), false);
    assert_eq!(tema::actual(), Tema::Contraste);
    tema::usar(Tema::Oscuro);
}
