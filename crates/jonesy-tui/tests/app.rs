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
fn ripley_al_guardar() {
    if std::process::Command::new("ripley")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let dir = proyecto(&[("main.c", "#include <stdlib.h>\nint main(void)\n{\n    int *p = malloc(4);\n    free(p);\n    return 0;\n}\n")]);
    let (mut a, rx) = app(dir.path(), false);
    tecla(&mut a, KeyCode::Char('s'), KeyModifiers::CONTROL);
    // Un segundo guardado mientras analiza no lanza otra instancia.
    tecla(&mut a, KeyCode::Char('s'), KeyModifiers::CONTROL);
    assert_eq!(a.tareas_en_curso(), vec!["ripley"]);
    assert!(pantalla(&mut a, 120, 30).contains("ripley…"));
    let fin = Instant::now() + Duration::from_secs(60);
    while a.diag_ripley.is_empty() && Instant::now() < fin {
        if let Ok(av) = rx.recv_timeout(Duration::from_millis(100)) {
            a.procesar(av);
        }
    }
    let d: Vec<_> = a.diag_ripley.values().flatten().collect();
    assert!(
        d.iter().any(|d| d.codigo.as_deref() == Some("0x3001h")),
        "{d:?}"
    );
    assert!(a.tareas_en_curso().is_empty());
    let p = pantalla(&mut a, 120, 30);
    assert!(
        p.contains("■ ") && p.contains("0x3001h"),
        "texto virtual de ripley: {p}"
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
