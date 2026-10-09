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

/// Estado del usuario (proyectos recientes, bienvenida vista) en un directorio propio de
/// las pruebas, para no tocar el real.
fn estado_de_prueba() {
    static ESTADO: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    let d = ESTADO.get_or_init(|| tempfile::tempdir().unwrap());
    std::env::set_var("JONESY_ESTADO", d.path());
}

fn app(dir: &Path, examen: bool) -> (App, Receiver<Aviso>) {
    estado_de_prueba();
    let (tx, rx) = channel();
    let op = Opciones {
        raiz: dir.to_path_buf(),
        archivos: vec![],
        examen,
        ascii: false,
        telemetria: false,
        bienvenida: false,
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
    // El error de gcc llega en inglés (LC_MESSAGES=C) y se muestra explicado en español,
    // en Problemas y al final de la línea.
    assert!(p.contains("«x» no está declarada"), "{p}");
    assert!(p.contains("F1 explica"), "{p}");
    // F1 sobre el problema abre la explicación con el ejemplo.
    a.foco = Foco::Inferior;
    tecla(&mut a, KeyCode::F(1), KeyModifiers::NONE);
    assert!(matches!(a.modal, Some(Modal::Visor { .. })));
    let p = pantalla(&mut a, 120, 40);
    assert!(p.contains("Error de compilación"), "{p}");
    assert!(p.contains("undeclared"), "el mensaje original, citado: {p}");
    assert!(p.contains("Así sí"), "{p}");
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
    // Archivo → Configuración del entorno: la primera solapa muestra el estado de cada
    // integración.
    a.accion(jonesy_tui::accion::Accion::Configuracion);
    let p = pantalla(&mut a, 160, 40);
    assert!(
        p.contains("Integraciones y linters")
            && p.contains("gaff")
            && p.contains("callahan")
            && p.contains("activa"),
        "{p}"
    );
    // Apagar la primera saca sus hallazgos y queda en la sesión.
    let primera = a.integraciones[0].nombre.clone();
    tecla(&mut a, KeyCode::Char(' '), KeyModifiers::NONE);
    tecla(&mut a, KeyCode::Esc, KeyModifiers::NONE);
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
    // Configuración del entorno → Apariencia → Tamaños de paneles: el quick-doc (fila 3) crece de a 5 %.
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

/// Clic izquierdo en (x, y).
fn clic(app: &mut App, x: u16, y: u16) {
    use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    app.evento(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }));
}

/// Posición (columna, fila) de `texto` en la pantalla.
fn buscar(p: &str, texto: &str) -> Option<(u16, u16)> {
    p.lines().enumerate().find_map(|(y, l)| {
        let i = l.find(texto)?;
        Some((l[..i].chars().count() as u16, y as u16))
    })
}

#[test]
fn menu_con_el_mouse() {
    use jonesy_tui::navegador::PestanaNav;
    let dir = proyecto(&[("main.c", MAIN)]);
    let (mut a, _rx) = app(dir.path(), false);
    let p = pantalla(&mut a, 160, 40);
    // Clic en un título abre su menú.
    let (x, y) = buscar(p.lines().next().unwrap(), "Archivo").unwrap();
    clic(&mut a, x + 1, y);
    assert!(matches!(a.modal, Some(Modal::Menu { .. })));
    let p = pantalla(&mut a, 160, 40);
    assert!(p.contains("Nuevo archivo"), "{p}");
    // Con un menú abierto, clic en otro título cambia de menú.
    let (x, y) = buscar(p.lines().next().unwrap(), "Ver").unwrap();
    clic(&mut a, x + 1, y);
    let p = pantalla(&mut a, 160, 40);
    assert!(!p.contains("Nuevo archivo"));
    // Clic en una entrada ejecuta su acción y cierra el menú.
    let (x, y) = buscar(&p, "Explorador de archivos").expect(&p);
    clic(&mut a, x + 2, y);
    assert!(a.modal.is_none());
    assert_eq!(a.navegador, Some(PestanaNav::Archivos));
    // Clic fuera del menú lo cierra sin hacer nada.
    let p = pantalla(&mut a, 160, 40);
    let (x, y) = buscar(p.lines().next().unwrap(), "Ver").unwrap();
    clic(&mut a, x + 1, y);
    assert!(matches!(a.modal, Some(Modal::Menu { .. })));
    pantalla(&mut a, 160, 40);
    clic(&mut a, 150, 30);
    assert!(a.modal.is_none());
    assert_eq!(a.navegador, Some(PestanaNav::Archivos));
}

#[test]
fn compacto_con_la_barra_izquierda() {
    use jonesy_tui::layout::Vista;
    let dir = proyecto(&[
        ("main.c", MAIN),
        ("otro.c", "int otra_funcion(void) { return 7; }\n"),
    ]);
    let (mut a, _rx) = app(dir.path(), false);
    let p = pantalla(&mut a, 80, 24);
    assert!(p.contains("printf"));
    // La barra izquierda ocupa toda la pantalla.
    a.accion(jonesy_tui::accion::Accion::Explorador);
    let p = pantalla(&mut a, 80, 24);
    assert_eq!(a.vista, Vista::Navegador);
    assert!(p.contains("otro.c") && !p.contains("printf"), "{p}");
    let izq = a.areas.izquierdo.expect("barra izquierda");
    assert_eq!(izq.width, 80);
    // Esc vuelve al código.
    tecla(&mut a, KeyCode::Esc, KeyModifiers::NONE);
    let p = pantalla(&mut a, 80, 24);
    assert_eq!(a.vista, Vista::Codigo);
    assert!(p.contains("printf"));
    // Abrir un archivo desde la barra muestra el editor con ese archivo.
    a.accion(jonesy_tui::accion::Accion::Explorador);
    let p = pantalla(&mut a, 80, 24);
    let (x, y) = buscar(&p, "otro.c").unwrap();
    clic(&mut a, x, y);
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    let p = pantalla(&mut a, 80, 24);
    assert_eq!(a.buf().buf.nombre(), "otro.c");
    assert!(p.contains("otra_funcion"), "{p}");
    // Al agrandar la terminal la barra queda al costado del editor.
    a.accion(jonesy_tui::accion::Accion::Explorador);
    let p = pantalla(&mut a, 160, 40);
    let izq = a.areas.izquierdo.expect("barra izquierda");
    assert!(izq.width < 80 && a.areas.editor.is_some());
    assert!(p.contains("otra_funcion"));
}

#[test]
#[cfg(unix)] // sh, gcc y gdb
fn programa_que_revienta_se_explica_y_f5_va_a_la_linea() {
    for herramienta in ["gcc", "gdb"] {
        if std::process::Command::new(herramienta)
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
    }
    // El NULL llega en tiempo de ejecución y el fallo ocurre dentro de strlen (libc).
    let codigo = "#include <stdio.h>\n#include <stdlib.h>\n#include <string.h>\n\nsize_t largo(const char *s)\n{\n    return strlen(s);\n}\n\nint main(void)\n{\n    char *s = getenv(\"JONESY_NO_EXISTE\");\n    printf(\"%zu\\n\", largo(s));\n    return 0;\n}\n";
    let dir = proyecto(&[
        ("main.c", codigo),
        (".jonesy.toml", "[ripley]\nactivo = false\n"),
    ]);
    let (mut a, rx) = app(dir.path(), false);
    pantalla(&mut a, 120, 40);
    a.accion(jonesy_tui::accion::Accion::Ejecutar);
    esperar(&mut a, &rx, |a| a.ultimo_fallo.is_some());
    assert_eq!(a.ultimo_fallo.unwrap().senal, "SIGSEGV");
    let p = pantalla(&mut a, 120, 40);
    assert!(p.contains("Violación de segmento"), "{p}");
    assert!(p.contains("[F5 Ver dónde falló]"), "{p}");
    // F5 no se detiene en main: corre hasta el fallo y muestra la línea del proyecto
    // (la 7, dentro de largo), no la de strlen.
    tecla(&mut a, KeyCode::F(5), KeyModifiers::NONE);
    esperar(&mut a, &rx, |a| {
        a.depuracion
            .as_ref()
            .and_then(|d| d.ubicacion.as_ref())
            .is_some_and(|(_, l)| *l == 7)
    });
    let d = a.depuracion.as_ref().unwrap();
    assert!(
        d.estado.contains("SIGSEGV") && d.estado.contains("largo"),
        "{}",
        d.estado
    );
    assert!(d.consola.iter().any(|l| l.contains("puntero NULL")));
    assert_eq!(
        a.buf().buf.linea_columna().0,
        6,
        "el editor salta a la línea"
    );
    a.accion(jonesy_tui::accion::Accion::Detener);
}

#[test]
#[cfg(unix)] // make y sh
fn probar_muestra_los_fallos_en_su_linea() {
    if std::process::Command::new("make")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    // Un ejercicio cuyo «make test» imprime lo que imprime p1_test.
    let receta = "test:\n\t@printf '  [CORRIENDO] suma ... PASÓ (0.01 ms)\\n  [CORRIENDO] resta ... \\n  [FALLO] prueba.c:4: ASSERT_INT_EQ(1, resta(3, 1))\\n    en test: resta\\n    esperado: igual a 1\\n    obtenido: 2\\n  [RESULTADO] FALLÓ (0.01 ms)\\n'; exit 1\n";
    let dir = proyecto(&[
        ("ejercicios/ej1/Makefile", receta),
        (
            "ejercicios/ej1/prueba.c",
            "#include \"p1_test.h\"\n\nTEST(resta)\n{\n    ASSERT_INT_EQ(1, resta(3, 1));\n}\n",
        ),
        (".jonesy.toml", "[ripley]\nactivo = false\n"),
    ]);
    let (mut a, rx) = app(dir.path(), false);
    a.abrir_archivo(&dir.path().join("ejercicios/ej1/prueba.c"), true);
    let p = pantalla(&mut a, 140, 40);
    assert!(p.contains("[^F6 Probar]"), "{p}");
    tecla(&mut a, KeyCode::F(6), KeyModifiers::CONTROL);
    esperar(&mut a, &rx, |a| {
        a.mensaje
            .as_ref()
            .is_some_and(|(m, _, _)| m.starts_with("Pruebas:"))
    });
    assert_eq!(a.mensaje.as_ref().unwrap().0, "Pruebas: 1 pasan, 1 fallan.");
    assert_eq!(a.pestana, Pestana::Problemas);
    let d = &a.diag_compilador[0];
    assert_eq!(d.linea, 4);
    assert!(d
        .archivo
        .as_deref()
        .unwrap()
        .ends_with("ejercicios/ej1/prueba.c"));
    assert_eq!(
        d.resumen.as_deref(),
        Some("La prueba falló «resta» — esperado: igual a 1, obtenido: 2")
    );
    // En pantalla (otras herramientas instaladas pueden sumar hallazgos antes en la lista).
    let p = pantalla(&mut a, 140, 40);
    assert!(p.contains("La prueba falló «resta»"), "{p}");
    // Sin objetivo test, se avisa en vez de correr nada.
    let vacio = proyecto(&[("main.c", MAIN)]);
    let (mut b, _rx) = app(vacio.path(), false);
    b.accion(jonesy_tui::accion::Accion::Probar);
    assert!(b.mensaje.as_ref().unwrap().0.starts_with("No hay pruebas"));
}

#[test]
#[cfg(unix)] // gcc y valgrind
fn revisar_memoria_explica_errores_y_fugas_en_su_linea() {
    for herramienta in ["gcc", "valgrind"] {
        if std::process::Command::new(herramienta)
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
    }
    let codigo = "#include <stdlib.h>\n\nint main(void)\n{\n    int *v = malloc(3 * sizeof *v);\n    v[3] = 1;\n    return 0;\n}\n";
    let dir = proyecto(&[
        ("main.c", codigo),
        (".jonesy.toml", "[ripley]\nactivo = false\n"),
    ]);
    let (mut a, rx) = app(dir.path(), false);
    a.abrir_archivo(&dir.path().join("main.c"), true);
    a.accion(jonesy_tui::accion::Accion::RevisarMemoria);
    esperar(&mut a, &rx, |a| {
        a.mensaje
            .as_ref()
            .is_some_and(|(m, _, _)| m.starts_with("Memoria:"))
    });
    assert_eq!(
        a.mensaje.as_ref().unwrap().0,
        "Memoria: 2 error(es), 12 bytes perdidos (mirá Problemas)."
    );
    let d: Vec<(usize, String)> = a
        .diag_compilador
        .iter()
        .map(|d| (d.linea, d.resumen.clone().unwrap_or_default()))
        .collect();
    assert!(
        d.iter()
            .any(|(l, r)| *l == 6 && r.starts_with("Escritura fuera de la memoria pedida")),
        "{d:#?}"
    );
    assert!(
        d.iter()
            .any(|(l, r)| *l == 5 && r.starts_with("Fuga de memoria: 12 bytes")),
        "{d:#?}"
    );
    let p = pantalla(&mut a, 140, 40);
    assert!(p.contains("(valgrind)"), "{p}");
}

#[test]
fn menus_por_letra_y_separadores() {
    use jonesy_tui::accion::Accion;
    let dir = proyecto(&[("main.c", MAIN)]);
    let (mut a, _rx) = app(dir.path(), false);
    let p = pantalla(&mut a, 140, 40);
    let barra = p.lines().next().unwrap();
    assert!(barra.contains("Construir  Depurar  Git"), "{barra}");
    assert!(
        !barra.contains("Herramientas"),
        "Herramientas pasó a la configuración"
    );
    let indice = |a: &App| match a.modal {
        Some(Modal::Menu { indice, sel }) => Some((indice, sel)),
        _ => None,
    };
    // Alt+D abre Depurar (el quick-doc queda en F4).
    tecla(&mut a, KeyCode::Char('d'), KeyModifiers::ALT);
    assert_eq!(indice(&a), Some((4, 0)));
    let p = pantalla(&mut a, 140, 40);
    assert!(p.contains("Iniciar depuración / continuar"), "{p}");
    // Con un menú abierto, la letra de otro lo abre: g → Git.
    tecla(&mut a, KeyCode::Char('g'), KeyModifiers::NONE);
    assert_eq!(indice(&a), Some((5, 0)));
    tecla(&mut a, KeyCode::Esc, KeyModifiers::NONE);
    // F10 y la letra: Editar, cuyo Alt+E es del explorador.
    tecla(&mut a, KeyCode::F(10), KeyModifiers::NONE);
    tecla(&mut a, KeyCode::Char('e'), KeyModifiers::NONE);
    assert_eq!(indice(&a), Some((1, 0)));
    tecla(&mut a, KeyCode::Esc, KeyModifiers::NONE);
    tecla(&mut a, KeyCode::Char('e'), KeyModifiers::ALT);
    assert!(
        a.modal.is_none() && a.navegador.is_some(),
        "Alt+E sigue siendo el explorador"
    );
    // ↓ saltea los separadores: en Archivo, después de «Abrir proyecto» viene «Guardar».
    tecla(&mut a, KeyCode::Char('a'), KeyModifiers::ALT);
    for _ in 0..3 {
        tecla(&mut a, KeyCode::Down, KeyModifiers::NONE);
    }
    let (i, sel) = indice(&a).unwrap();
    let menus = jonesy_tui::accion::menus(false);
    assert_eq!(
        menus[i].items[sel],
        jonesy_tui::accion::Entrada::Accion(Accion::Guardar)
    );
    let p = pantalla(&mut a, 140, 40);
    assert!(p.contains("Configuración del entorno…"), "{p}");
    assert!(p.contains("├") || p.contains("─────"), "separadores: {p}");
}

#[test]
fn configuracion_del_entorno_por_solapas() {
    use jonesy_tui::tema::{self, Tema};
    let dir = proyecto(&[("main.c", MAIN)]);
    let (mut a, _rx) = app(dir.path(), false);
    a.abrir_archivo(&dir.path().join("main.c"), true);
    a.accion(jonesy_tui::accion::Accion::Configuracion);
    // Plugins.
    tecla(&mut a, KeyCode::Right, KeyModifiers::NONE);
    let p = pantalla(&mut a, 140, 40);
    assert!(
        p.contains("Nuevo plugin…") && p.contains("Recargar los plugins"),
        "{p}"
    );
    // Apariencia: Enter sobre Tema lo cambia y la pantalla sigue abierta.
    tecla(&mut a, KeyCode::Char('4'), KeyModifiers::NONE);
    let antes = tema::actual();
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert_ne!(tema::actual(), antes);
    assert!(matches!(
        a.modal,
        Some(Modal::Configuracion { pestana: 3, .. })
    ));
    tema::usar(Tema::Oscuro);
    // Compilador y formato: muestra el comando de F6 y abre .jonesy.toml (lo crea).
    tecla(&mut a, KeyCode::Char('3'), KeyModifiers::NONE);
    let p = pantalla(&mut a, 140, 40);
    assert!(p.contains("Compilar (F6)") && p.contains("gcc"), "{p}");
    tecla(&mut a, KeyCode::Down, KeyModifiers::NONE);
    tecla(&mut a, KeyCode::Down, KeyModifiers::NONE);
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert!(a.modal.is_none());
    assert!(dir.path().join(".jonesy.toml").is_file());
    assert_eq!(a.buf().buf.nombre(), ".jonesy.toml");
    // En examen no se edita la configuración del proyecto.
    let (mut e, _rx) = app(dir.path(), true);
    e.accion(jonesy_tui::accion::Accion::Configuracion);
    tecla(&mut e, KeyCode::Char('3'), KeyModifiers::NONE);
    tecla(&mut e, KeyCode::Down, KeyModifiers::NONE);
    tecla(&mut e, KeyCode::Down, KeyModifiers::NONE);
    tecla(&mut e, KeyCode::Enter, KeyModifiers::NONE);
    assert!(e.mensaje.as_ref().unwrap().0.contains("modo examen"));
}

#[test]
fn guia_estado_de_entrega_y_limpiar() {
    use jonesy_tui::accion::Accion;
    let dir = proyecto(&[("main.c", MAIN)]);
    let (mut a, _rx) = app(dir.path(), false);
    a.accion(Accion::GuiaRapida);
    let p = pantalla(&mut a, 120, 50);
    assert!(
        p.contains("Guía rápida") && p.contains("Compilar — F6"),
        "{p}"
    );
    a.modal = None;
    // Un archivo suelto sin Makefile: limpiar borra su ejecutable.
    a.abrir_archivo(&dir.path().join("main.c"), true);
    a.accion(Accion::LimpiarCompilacion);
    assert!(
        a.salida
            .first()
            .is_some_and(|(l, _)| l.contains("rm -f main")),
        "{:?}",
        a.salida.first()
    );
    // Estado de la entrega en examen.
    let (mut e, _rx) = app(dir.path(), true);
    e.accion(Accion::EstadoEntrega);
    let p = pantalla(&mut e, 120, 40);
    assert!(
        p.contains("Estado de la entrega") && p.contains("todavía no"),
        "{p}"
    );
}

#[test]
fn tema_turbo_pinta_como_turbo_c() {
    use jonesy_tui::tema::{self, Tema};
    use ratatui::style::Color;
    let dir = proyecto(&[
        ("main.c", MAIN),
        (".jonesy.toml", "[interfaz]\ntema = \"turbo\"\n"),
    ]);
    let (mut a, _rx) = app(dir.path(), false);
    assert_eq!(tema::actual(), Tema::Turbo);
    let mut t = Terminal::new(TestBackend::new(120, 30)).unwrap();
    t.draw(|f| jonesy_tui::dibujo::dibujar(f, &mut a)).unwrap();
    let buf = t.backend().buffer().clone();
    // Barra de menú gris con texto negro y la letra mnemónica roja.
    assert_eq!(buf[(5, 0)].bg, Color::Rgb(170, 170, 170));
    assert_eq!(buf[(2, 0)].symbol(), "A");
    assert_eq!(buf[(2, 0)].fg, Color::Rgb(170, 0, 0));
    assert_eq!(buf[(4, 0)].fg, Color::Rgb(0, 0, 0));
    // Editor azul.
    let z = a.zonas.editor.unwrap();
    assert_eq!(
        buf[(z.texto.x + 40, z.texto.y + 10)].bg,
        Color::Rgb(0, 0, 170)
    );
    // Menú abierto: la entrada elegida en verde.
    tecla(&mut a, KeyCode::Char('a'), KeyModifiers::ALT);
    t.draw(|f| jonesy_tui::dibujo::dibujar(f, &mut a)).unwrap();
    let buf = t.backend().buffer().clone();
    assert_eq!(buf[(5, 2)].bg, Color::Rgb(0, 170, 0));
    // Los nombres con o sin tilde.
    assert_eq!(Tema::de_nombre("fósforo"), Some(Tema::Fosforo));
    tema::usar(Tema::Oscuro);
}

#[test]
fn menu_plugins_lista_y_ejecuta_los_comandos() {
    if std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let dir = proyecto(&[
        ("main.c", MAIN),
        (
            ".jonesy/plugins/hola.py",
            "def comando_saludar(ctx):\n    return \"# Hola\\n\\ndesde el plugin\"\n",
        ),
    ]);
    let (mut a, rx) = app(dir.path(), false);
    a.abrir_archivo(&dir.path().join("main.c"), true);
    // Alt+L abre Plugins: pide los comandos al worker y se completa cuando llegan. La
    // selección va al primer comando (los títulos de sección no se eligen).
    tecla(&mut a, KeyCode::Char('l'), KeyModifiers::ALT);
    esperar(&mut a, &rx, |a| !a.comandos_plugins.is_empty());
    let p = pantalla(&mut a, 140, 40);
    let barra = p.lines().next().unwrap();
    assert!(barra.contains("Git  Plugins  Ayuda"), "{barra}");
    assert!(
        p.contains("Plugins Python") && p.contains("hola: saludar"),
        "{p}"
    );
    assert!(p.contains("Herramientas para este archivo"), "{p}");
    assert!(
        p.contains("Configurar plugins…") && p.contains("Recargar los plugins"),
        "{p}"
    );
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    esperar(&mut a, &rx, |a| {
        matches!(a.modal, Some(Modal::Visor { .. }))
    });
    let p = pantalla(&mut a, 140, 40);
    assert!(p.contains("desde el plugin"), "{p}");
    // Configurar plugins abre la solapa Plugins de la configuración.
    a.modal = None;
    a.accion(jonesy_tui::accion::Accion::ConfigurarPlugins);
    assert!(matches!(
        a.modal,
        Some(Modal::Configuracion { pestana: 1, .. })
    ));
    // Sin plugins, el menú lo dice.
    let vacio = proyecto(&[("main.c", MAIN)]);
    let (mut b, _rx) = app(vacio.path(), false);
    tecla(&mut b, KeyCode::Char('l'), KeyModifiers::ALT);
    let p = pantalla(&mut b, 140, 40);
    assert!(p.contains("(ninguno en .jonesy/plugins)"), "{p}");
}

#[test]
fn atajos_de_la_barra_izquierda_se_ven_completos() {
    use jonesy_tui::navegador::PestanaNav;
    let dir = proyecto(&[
        ("main.c", MAIN),
        (
            "Makefile",
            "## Compila todo el trabajo práctico con sus bibliotecas y ejercicios\nall:\n\t@true\n",
        ),
    ]);
    let (mut a, _rx) = app(dir.path(), false);
    a.accion(jonesy_tui::accion::Accion::Explorador);
    let p = pantalla(&mut a, 120, 30);
    let ancho = a.areas.izquierdo.unwrap().width;
    assert!(ancho <= 34, "barra angosta: {ancho}");
    for atajo in [
        "n nuevo",
        "m carpeta",
        "F2 renombrar",
        "Supr borrar",
        "r relee",
    ] {
        assert!(p.contains(atajo), "falta «{atajo}»:\n{p}");
    }
    a.navegador = Some(PestanaNav::Proyecto);
    let p = pantalla(&mut a, 120, 30);
    for atajo in ["Enter corre", "→← despliega", "r relee"] {
        assert!(p.contains(atajo), "falta «{atajo}»:\n{p}");
    }
    // Y el pie entra en una barra todavía más angosta, con más líneas.
    let lineas = jonesy_tui::dibujo::pie_atajos(
        &[
            "n nuevo",
            "m carpeta",
            "F2 renombrar",
            "Supr borrar",
            "r relee",
        ],
        20,
    );
    assert!(lineas.iter().all(|l| l.chars().count() <= 20), "{lineas:?}");
    assert_eq!(lineas.join(" · ").matches(" · ").count(), 4);
}

#[test]
#[cfg(unix)] // make y sh
fn pruebas_de_otros_frameworks_en_sarif() {
    if std::process::Command::new("make")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let sarif = r#"{"version": "2.1.0", "runs": [{"tool": {"driver": {"name": "cmocka"}}, "results": [{"kind": "pass", "message": {"text": "ok"}}, {"kind": "fail", "level": "error", "message": {"text": "esperado 4, obtenido 5"}, "properties": {"test": "suma"}, "locations": [{"physicalLocation": {"artifactLocation": {"uri": "prueba.c"}, "region": {"startLine": 3}}}]}]}]}"#;
    // 1. El SARIF en la salida de make test.
    let receta = format!("test:\n\t@echo '{sarif}'; exit 1\n");
    let dir = proyecto(&[
        ("Makefile", receta.as_str()),
        ("prueba.c", "#include <assert.h>\n\nvoid suma(void) {}\n"),
        (".jonesy.toml", "[ripley]\nactivo = false\n"),
    ]);
    let (mut a, rx) = app(dir.path(), false);
    a.abrir_archivo(&dir.path().join("prueba.c"), true);
    a.accion(jonesy_tui::accion::Accion::Probar);
    esperar(&mut a, &rx, |a| {
        a.mensaje
            .as_ref()
            .is_some_and(|(m, _, _)| m.starts_with("Pruebas:"))
    });
    assert_eq!(a.mensaje.as_ref().unwrap().0, "Pruebas: 1 pasan, 1 fallan.");
    let d = &a.diag_compilador[0];
    assert_eq!(d.linea, 3);
    assert_eq!(d.resumen.as_deref(), Some("La prueba falló «suma»"));
    assert_eq!(d.herramienta.as_deref(), Some("cmocka"));

    // 2. Otro comando ([pruebas] comando) que deja el SARIF en un archivo.
    let guion = format!("printf '%s' '{sarif}' > resultados.sarif\necho listo\n");
    let dir = proyecto(&[
        ("prueba.c", "#include <assert.h>\n\nvoid suma(void) {}\n"),
        ("pruebas.sh", guion.as_str()),
        (
            ".jonesy.toml",
            "[ripley]\nactivo = false\n[pruebas]\ncomando = \"sh pruebas.sh\"\nsarif = \"resultados.sarif\"\n",
        ),
    ]);
    let (mut a, rx) = app(dir.path(), false);
    a.abrir_archivo(&dir.path().join("prueba.c"), true);
    a.accion(jonesy_tui::accion::Accion::Probar);
    esperar(&mut a, &rx, |a| {
        a.mensaje
            .as_ref()
            .is_some_and(|(m, _, _)| m.starts_with("Pruebas:"))
    });
    assert_eq!(a.mensaje.as_ref().unwrap().0, "Pruebas: 1 pasan, 1 fallan.");
}

#[test]
fn diagnostico_del_entorno_desde_ayuda() {
    let dir = proyecto(&[("main.c", MAIN)]);
    let (mut a, rx) = app(dir.path(), false);
    a.accion(jonesy_tui::accion::Accion::Doctor);
    esperar(&mut a, &rx, |a| {
        matches!(a.modal, Some(Modal::Visor { .. }))
    });
    let p = pantalla(&mut a, 130, 60);
    assert!(p.contains("Diagnóstico del entorno"), "{p}");
    assert!(p.contains("Compilación") && p.contains("Proyecto"), "{p}");
}

#[test]
fn bienvenida_la_primera_vez_y_desde_ayuda() {
    // El estado del usuario (marca de «ya vista») en un directorio propio de la prueba:
    // un subdirectorio para no depender de lo que hagan las demás.
    estado_de_prueba();
    let base = std::path::PathBuf::from(std::env::var_os("JONESY_ESTADO").unwrap());
    let propio = base.join("bienvenida");
    std::fs::create_dir_all(&propio).unwrap();
    std::env::set_var("JONESY_ESTADO", &propio);
    let dir = proyecto(&[("main.c", MAIN)]);
    let abrir = |examen: bool| {
        let (tx, rx) = channel();
        let op = Opciones {
            raiz: dir.path().to_path_buf(),
            archivos: vec![],
            examen,
            ascii: false,
            telemetria: false,
            bienvenida: true,
        };
        (App::nueva(op, tx), rx)
    };
    let (mut a, _rx) = abrir(false);
    assert!(
        matches!(a.modal, Some(Modal::Bienvenida { .. })),
        "primera vez"
    );
    let p = pantalla(&mut a, 120, 40);
    assert!(p.contains("Te damos la bienvenida a jonesy"), "{p}");
    assert!(p.contains("Ctrl+F6") && p.contains("Tu entorno"), "{p}");
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert!(a.modal.is_none());
    // Ya vista: no vuelve a aparecer…
    let (b, _rx) = abrir(false);
    assert!(b.modal.is_none());
    // …salvo que se pida verla siempre (Espacio), y queda en Ayuda.
    let (mut c, _rx) = abrir(false);
    c.accion(jonesy_tui::accion::Accion::Bienvenida);
    tecla(&mut c, KeyCode::Char(' '), KeyModifiers::NONE);
    assert!(matches!(
        c.modal,
        Some(Modal::Bienvenida { siempre: true, .. })
    ));
    tecla(&mut c, KeyCode::Enter, KeyModifiers::NONE);
    let (d, _rx) = abrir(false);
    assert!(matches!(d.modal, Some(Modal::Bienvenida { .. })));
    // En examen nunca.
    let (e, _rx) = abrir(true);
    assert!(!matches!(e.modal, Some(Modal::Bienvenida { .. })));
    std::env::set_var("JONESY_ESTADO", &base);
}

#[test]
fn edicion_pares_comentarios_lineas_y_espacios() {
    let dir = proyecto(&[
        ("main.c", "int a;   \nint b;\n"),
        (
            ".jonesy.toml",
            "[ripley]\nactivo = false\n[formato]\nlimpiar_espacios = true\n",
        ),
    ]);
    let (mut a, _rx) = app(dir.path(), false);
    a.abrir_archivo(&dir.path().join("main.c"), true);
    // Los espacios del final se ven (el cursor está en otra línea).
    let pos = a.buf().buf.linea_columna_a_pos(1, 0);
    a.buf_mut().buf.mover_a(pos, false);
    let p = pantalla(&mut a, 100, 20);
    assert!(p.contains("int a;···"), "{p}");
    // Paréntesis que se cierran solos y se saltean.
    tecla(&mut a, KeyCode::End, KeyModifiers::CONTROL);
    escribir(&mut a, "f(x)");
    assert!(a.buf().buf.contenido().ends_with("f(x)"));
    // La pareja del cursor se resalta.
    tecla(&mut a, KeyCode::Left, KeyModifiers::NONE);
    let mut t = Terminal::new(TestBackend::new(100, 20)).unwrap();
    t.draw(|f| jonesy_tui::dibujo::dibujar(f, &mut a)).unwrap();
    let z = a.zonas.editor.unwrap();
    let buf = t.backend().buffer().clone();
    let fila = z.texto.y + 2;
    let subrayadas = (z.texto.x..z.texto.x + 10)
        .filter(|&x| {
            buf[(x, fila)]
                .modifier
                .contains(ratatui::style::Modifier::UNDERLINED)
        })
        .count();
    assert_eq!(subrayadas, 2, "( y ) resaltados");
    // Ctrl+/ comenta y descomenta; Ctrl+D duplica; Alt+↑ sube la línea.
    tecla(&mut a, KeyCode::Char('/'), KeyModifiers::CONTROL);
    assert!(a.buf().buf.contenido().ends_with("// f(x)"));
    tecla(&mut a, KeyCode::Char('7'), KeyModifiers::CONTROL);
    assert!(a.buf().buf.contenido().ends_with("\nf(x)"));
    tecla(&mut a, KeyCode::Char('d'), KeyModifiers::CONTROL);
    tecla(&mut a, KeyCode::Up, KeyModifiers::ALT);
    assert_eq!(a.buf().buf.contenido(), "int a;   \nint b;\nf(x)\nf(x)");
    tecla(&mut a, KeyCode::Up, KeyModifiers::ALT);
    assert_eq!(a.buf().buf.contenido(), "int a;   \nf(x)\nint b;\nf(x)");
    // Al guardar se limpian los espacios del final ([formato] limpiar_espacios).
    tecla(&mut a, KeyCode::Char('s'), KeyModifiers::CONTROL);
    let guardado = std::fs::read_to_string(dir.path().join("main.c")).unwrap();
    assert_eq!(guardado, "int a;\nf(x)\nint b;\nf(x)");
}

#[test]
fn buscar_y_reemplazar_una_por_una_y_todas() {
    let dir = proyecto(&[
        ("main.c", "int max = 1;\nint y = max + MAX;\n"),
        (".jonesy.toml", "[ripley]\nactivo = false\n"),
    ]);
    let (mut a, _rx) = app(dir.path(), false);
    a.abrir_archivo(&dir.path().join("main.c"), true);
    a.buf_mut().buf.mover_a(0, false);
    tecla(&mut a, KeyCode::Char('h'), KeyModifiers::CONTROL);
    escribir(&mut a, "max");
    tecla(&mut a, KeyCode::Tab, KeyModifiers::NONE);
    escribir(&mut a, "tope");
    tecla(&mut a, KeyCode::Char('m'), KeyModifiers::ALT);
    let p = pantalla(&mut a, 120, 30);
    assert!(
        p.contains("2 coincidencia(s)") && p.contains("[x] mayúsculas"),
        "{p}"
    );
    // Enter elige la primera; Enter de nuevo la reemplaza y elige la siguiente.
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        a.buf().buf.contenido(),
        "int tope = 1;\nint y = max + MAX;\n"
    );
    assert_eq!(a.buf().buf.texto_seleccionado().as_deref(), Some("max"));
    // F3 la saltea; Alt+T reemplaza todas (sin distinguir mayúsculas).
    tecla(&mut a, KeyCode::F(3), KeyModifiers::NONE);
    tecla(&mut a, KeyCode::Char('m'), KeyModifiers::ALT);
    tecla(&mut a, KeyCode::Char('t'), KeyModifiers::ALT);
    assert_eq!(
        a.buf().buf.contenido(),
        "int tope = 1;\nint y = tope + tope;\n"
    );
    assert!(a.mensaje.as_ref().unwrap().0.starts_with("2 reemplazo(s)"));
    // Un solo Ctrl+Z deshace el «todas».
    tecla(&mut a, KeyCode::Esc, KeyModifiers::NONE);
    tecla(&mut a, KeyCode::Char('z'), KeyModifiers::CONTROL);
    assert_eq!(
        a.buf().buf.contenido(),
        "int tope = 1;\nint y = max + MAX;\n"
    );
}

#[test]
fn definicion_volver_y_esquema() {
    use jonesy_tui::navegador::PestanaNav;
    let principal = "#include \"lib.h\"\n\nint main(void)\n{\n    return suma(1, 2);\n}\n";
    let dir = proyecto(&[
        ("main.c", principal),
        ("lib.h", "int suma(int a, int b);\n"),
        (
            "lib.c",
            "#include \"lib.h\"\n\nint suma(int a, int b)\n{\n    return a + b;\n}\n",
        ),
        (".jonesy.toml", "[ripley]\nactivo = false\n"),
    ]);
    let (mut a, _rx) = app(dir.path(), false);
    a.abrir_archivo(&dir.path().join("main.c"), true);
    let pos = a.buf().buf.linea_columna_a_pos(4, 12);
    a.buf_mut().buf.mover_a(pos, false);
    // F12 va a la definición (en lib.c, no al prototipo de lib.h).
    tecla(&mut a, KeyCode::F(12), KeyModifiers::NONE);
    assert_eq!(a.buf().buf.nombre(), "lib.c");
    assert_eq!(a.buf().buf.linea_columna().0, 2);
    // Alt+← vuelve.
    tecla(&mut a, KeyCode::Left, KeyModifiers::ALT);
    assert_eq!(a.buf().buf.nombre(), "main.c");
    assert_eq!(a.buf().buf.cursor, pos);
    // Una función de la biblioteca estándar: lo dice.
    a.buf_mut()
        .buf
        .reemplazar_todo("int main(void) { printf(\"x\"); }\n");
    a.buf_mut().buf.mover_a(19, false);
    tecla(&mut a, KeyCode::F(12), KeyModifiers::NONE);
    assert!(a
        .mensaje
        .as_ref()
        .unwrap()
        .0
        .contains("biblioteca estándar"));
    // Ctrl+clic sobre «main» la encuentra en el mismo buffer (sin guardar).
    pantalla(&mut a, 120, 30);
    let z = a.zonas.editor.unwrap();
    use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    a.evento(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: z.texto.x + 5,
        row: z.texto.y,
        modifiers: KeyModifiers::CONTROL,
    }));
    assert!(
        a.mensaje.as_ref().unwrap().0.contains("main.c:1"),
        "{:?}",
        a.mensaje
    );
    // Esquema (Alt+S): las funciones del archivo; Enter salta.
    a.abrir_archivo(&dir.path().join("lib.c"), true);
    tecla(&mut a, KeyCode::Char('s'), KeyModifiers::ALT);
    assert_eq!(a.navegador, Some(PestanaNav::Esquema));
    let p = pantalla(&mut a, 120, 30);
    assert!(p.contains("Esquema") && p.contains("suma"), "{p}");
    a.buf_mut().buf.mover_a(0, false);
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(a.buf().buf.linea_columna().0, 2);
    assert_eq!(a.foco, Foco::Editor);
}

#[test]
fn buscar_en_el_proyecto_con_panel_de_resultados() {
    let dir = proyecto(&[
        (
            "main.c",
            "int total = 0;\nint main(void) { return total; }\n",
        ),
        ("libs/a/a.c", "int subtotal(void);\n"),
        (".jonesy.toml", "[ripley]\nactivo = false\n"),
    ]);
    let (mut a, _rx) = app(dir.path(), false);
    a.abrir_archivo(&dir.path().join("main.c"), true);
    tecla(&mut a, KeyCode::F(3), KeyModifiers::CONTROL);
    tecla(&mut a, KeyCode::Char('u'), KeyModifiers::CONTROL);
    escribir(&mut a, "total");
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(a.pestana, Pestana::Busqueda);
    let p = pantalla(&mut a, 120, 30);
    assert!(p.contains("Búsqueda (3)"), "{p}");
    assert!(p.contains("libs/a/a.c:1"), "{p}");
    // Palabra completa: subtotal no cuenta.
    tecla(&mut a, KeyCode::F(3), KeyModifiers::CONTROL);
    tecla(&mut a, KeyCode::Char('p'), KeyModifiers::ALT);
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    let p = pantalla(&mut a, 120, 30);
    assert!(p.contains("Búsqueda (2)"), "{p}");
    // Enter abre el resultado y elige lo encontrado.
    tecla(&mut a, KeyCode::Down, KeyModifiers::NONE);
    tecla(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(a.buf().buf.linea_columna().0, 1);
    assert_eq!(a.buf().buf.texto_seleccionado().as_deref(), Some("total"));
}
