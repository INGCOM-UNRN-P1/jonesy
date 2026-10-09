//! Catálogo de acciones: lo comparten el menú (F10), la paleta (Ctrl+K / F1) y los atajos.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Accion {
    // Archivo
    Abrir,
    Guardar,
    GuardarTodo,
    CerrarBuffer,
    SiguienteBuffer,
    AnteriorBuffer,
    Salir,
    // Editar
    Deshacer,
    Rehacer,
    Cortar,
    Copiar,
    Pegar,
    SeleccionarTodo,
    Buscar,
    BuscarSiguiente,
    IrALinea,
    // Ver
    QuickDoc,
    OrientacionQuickDoc,
    Terminal,
    Problemas,
    SalidaCompilacion,
    PanelGit,
    PanelReglas,
    PreviewMarkdown,
    VistaConsigna,
    VistaCodigo,
    VistaTerminal,
    VistaDoc,
    // Construir
    Compilar,
    Ejecutar,
    Probar,
    RevisarMemoria,
    Depurar,
    Paso,
    Entrar,
    SalirFuncion,
    Detener,
    AlternarBreakpoint,
    CatScan,
    // Git
    GitStageTodo,
    GitUnstageTodo,
    GitCommit,
    GitLog,
    // Examen
    Entregar,
    // Ayuda
    ExplicarRegla,
    Atajos,
    AcercaDe,
    Paleta,
    TamanosPaneles,
    CambiarTema,
    Explorador,
    EstructuraProyecto,
    NuevoArchivo,
    AbrirProyecto,
    GitClonar,
    Formatear,
    GenerarDoc,
    DiffArchivo,
    Herramientas,
    Integraciones,
    GitDeshacerCommit,
    ExportarMemoria,
    NuevoPlugin,
    RecargarPlugins,
}

pub struct Info {
    pub titulo: &'static str,
    pub atajo: &'static str,
}

impl Accion {
    pub fn info(self) -> Info {
        use Accion::*;
        let (titulo, atajo) = match self {
            Abrir => ("Abrir archivo…", "Ctrl+O"),
            Guardar => ("Guardar", "Ctrl+S / F2"),
            GuardarTodo => ("Guardar todo", "Ctrl+Shift+S"),
            CerrarBuffer => ("Cerrar pestaña", "Ctrl+W"),
            SiguienteBuffer => ("Pestaña siguiente", "Ctrl+Tab"),
            AnteriorBuffer => ("Pestaña anterior", "Ctrl+Shift+Tab"),
            Salir => ("Salir", "Ctrl+Q"),
            Deshacer => ("Deshacer", "Ctrl+Z"),
            Rehacer => ("Rehacer", "Ctrl+Y"),
            Cortar => ("Cortar", "Ctrl+X"),
            Copiar => ("Copiar", "Ctrl+C"),
            Pegar => ("Pegar", "Ctrl+V"),
            SeleccionarTodo => ("Seleccionar todo", "Ctrl+A"),
            Buscar => ("Buscar…", "F3 / Ctrl+F"),
            BuscarSiguiente => ("Buscar siguiente", "Shift+F3"),
            IrALinea => ("Ir a la línea…", "Ctrl+G"),
            QuickDoc => ("Mostrar/ocultar quick-doc", "F4 / Alt+D"),
            OrientacionQuickDoc => ("Quick-doc: derecha ⟷ inferior", "Shift+F4"),
            Terminal => ("Terminal (deck)", "Alt+T"),
            Problemas => ("Problemas", "Alt+P"),
            SalidaCompilacion => ("Salida de la compilación", "Alt+O"),
            PanelGit => ("Panel de Git", "Alt+G"),
            PanelReglas => ("Panel de reglas de ripley", "Alt+R"),
            PreviewMarkdown => ("Vista previa de Markdown", "Ctrl+P"),
            VistaConsigna => ("Ver consigna", "F7"),
            VistaCodigo => ("Ver código", "F8"),
            VistaTerminal => ("Ver terminal", "F9"),
            VistaDoc => ("Ver documentación", "F4"),
            Compilar => ("Compilar", "F6"),
            Ejecutar => ("Compilar y ejecutar", "Ctrl+F5"),
            Probar => ("Correr las pruebas (make test)", "Ctrl+F6"),
            RevisarMemoria => (
                "Revisar la memoria: fugas y accesos inválidos (valgrind)",
                "",
            ),
            Depurar => ("Depurar / continuar", "F5"),
            Paso => ("Paso (siguiente línea)", "F10"),
            Entrar => ("Entrar en la función", "F11"),
            SalirFuncion => ("Salir de la función", "Shift+F11"),
            Detener => ("Detener la depuración", "Shift+F5"),
            AlternarBreakpoint => ("Poner/quitar breakpoint", "Ctrl+B"),
            CatScan => ("Memoria (cat-scan)", "Alt+M"),
            GitStageTodo => ("Git: agregar todo al stage", "a"),
            GitUnstageTodo => ("Git: quitar todo del stage", "u"),
            GitCommit => ("Git: commit…", "c"),
            GitLog => ("Git: historial", "l"),
            Entregar => ("Entregar examen", "F12"),
            ExplicarRegla => ("Explicar la regla del diagnóstico", "F1"),
            Atajos => ("Atajos de teclado", ""),
            AcercaDe => ("Acerca de jonesy", ""),
            Paleta => ("Paleta de comandos", "Ctrl+K"),
            TamanosPaneles => ("Tamaños de paneles…", ""),
            CambiarTema => ("Cambiar tema (oscuro, claro, contraste)", ""),
            Explorador => ("Explorador de archivos", "Alt+E"),
            EstructuraProyecto => ("Proyecto: bibliotecas, ejercicios y objetivos", "Alt+Y"),
            NuevoArchivo => ("Nuevo archivo…", "Ctrl+N"),
            AbrirProyecto => ("Abrir proyecto o directorio…", ""),
            GitClonar => ("Git: clonar un repositorio…", ""),
            Formatear => ("Autoformato del archivo", "Alt+Shift+F"),
            GenerarDoc => ("Generar la documentación del proyecto (make doc)", ""),
            DiffArchivo => (
                "Comparar con la versión de git (diff lado a lado)",
                "Alt+Shift+D",
            ),
            NuevoPlugin => ("Nuevo plugin Python (.jonesy/plugins)…", ""),
            RecargarPlugins => ("Recargar los plugins Python", ""),
            ExportarMemoria => (
                "Exportar la traza de memoria (reporte HTML de mem-trace)",
                "",
            ),
            GitDeshacerCommit => (
                "Git: deshacer el último commit (sus cambios quedan en el stage)",
                "z",
            ),
            Herramientas => ("Herramientas del ecosistema para este archivo…", "Alt+H"),
            Integraciones => ("Integraciones: qué herramientas corren y cuándo…", ""),
        };
        Info { titulo, atajo }
    }

    /// Acciones de la paleta (todas menos la propia paleta).
    pub fn todas() -> &'static [Accion] {
        use Accion::*;
        &[
            Compilar,
            Ejecutar,
            Probar,
            RevisarMemoria,
            Depurar,
            Paso,
            Entrar,
            SalirFuncion,
            Detener,
            AlternarBreakpoint,
            CatScan,
            Guardar,
            GuardarTodo,
            Abrir,
            CerrarBuffer,
            SiguienteBuffer,
            AnteriorBuffer,
            Buscar,
            BuscarSiguiente,
            IrALinea,
            Deshacer,
            Rehacer,
            Cortar,
            Copiar,
            Pegar,
            SeleccionarTodo,
            QuickDoc,
            OrientacionQuickDoc,
            Terminal,
            Problemas,
            SalidaCompilacion,
            PanelGit,
            PanelReglas,
            PreviewMarkdown,
            VistaConsigna,
            VistaCodigo,
            VistaTerminal,
            VistaDoc,
            GitStageTodo,
            GitUnstageTodo,
            GitCommit,
            GitLog,
            GitClonar,
            GitDeshacerCommit,
            ExportarMemoria,
            NuevoPlugin,
            RecargarPlugins,
            Formatear,
            GenerarDoc,
            DiffArchivo,
            Herramientas,
            Integraciones,
            Entregar,
            ExplicarRegla,
            TamanosPaneles,
            CambiarTema,
            Explorador,
            EstructuraProyecto,
            NuevoArchivo,
            AbrirProyecto,
            Atajos,
            AcercaDe,
            Salir,
        ]
    }
}

/// Menús de la barra superior.
pub fn menus(examen: bool) -> Vec<(&'static str, Vec<Accion>)> {
    use Accion::*;
    let mut m = vec![
        (
            "Archivo",
            vec![
                NuevoArchivo,
                Abrir,
                AbrirProyecto,
                Guardar,
                GuardarTodo,
                CerrarBuffer,
                SiguienteBuffer,
                AnteriorBuffer,
                Salir,
            ],
        ),
        (
            "Editar",
            vec![
                Deshacer,
                Rehacer,
                Cortar,
                Copiar,
                Pegar,
                SeleccionarTodo,
                Buscar,
                BuscarSiguiente,
                IrALinea,
                Formatear,
            ],
        ),
        (
            "Ver",
            vec![
                Explorador,
                EstructuraProyecto,
                QuickDoc,
                OrientacionQuickDoc,
                Terminal,
                Problemas,
                SalidaCompilacion,
                PanelGit,
                PanelReglas,
                PreviewMarkdown,
                VistaConsigna,
                VistaCodigo,
                VistaTerminal,
                TamanosPaneles,
                CambiarTema,
            ],
        ),
        (
            "Construir",
            vec![
                EstructuraProyecto,
                GenerarDoc,
                Compilar,
                Ejecutar,
                Probar,
                RevisarMemoria,
                Depurar,
                Paso,
                Entrar,
                SalirFuncion,
                Detener,
                AlternarBreakpoint,
                CatScan,
                ExportarMemoria,
            ],
        ),
        (
            "Git",
            vec![
                DiffArchivo,
                GitClonar,
                PanelGit,
                GitStageTodo,
                GitUnstageTodo,
                GitCommit,
                GitLog,
                GitDeshacerCommit,
            ],
        ),
    ];
    if examen {
        m.push(("Examen", vec![Entregar]));
    }
    m.push((
        "Herramientas",
        vec![
            Herramientas,
            Formatear,
            GenerarDoc,
            ExplicarRegla,
            Integraciones,
            NuevoPlugin,
            RecargarPlugins,
        ],
    ));
    m.push(("Ayuda", vec![Paleta, ExplicarRegla, Atajos, AcercaDe]));
    m
}

pub const AYUDA_ATAJOS: &str = "\
# Atajos de jonesy

| Acción | Teclas |
|---|---|
| Guardar | Ctrl+S, F2 |
| Deshacer / rehacer | Ctrl+Z / Ctrl+Y |
| Copiar / cortar / pegar | Ctrl+C / Ctrl+X / Ctrl+V |
| Buscar | F3, Ctrl+F (Shift+F3 siguiente) |
| Ir a línea | Ctrl+G |
| Autoformato (gaff o clang-format) | Alt+Shift+F |
| Diff del archivo contra git (lado a lado; s alterna, n/p cambio) | Alt+Shift+D |
| Herramientas del ecosistema (gaff fix, tetsuo, motoko…) | Alt+H |
| Nuevo archivo · abrir (Tab marca varios) | Ctrl+N · Ctrl+O |
| Explorador de archivos · proyecto (make y tp.sh) | Alt+E · Alt+Y (Tab alterna) |
| Pestañas | Ctrl+Tab / Ctrl+Shift+Tab, Alt+1…9, Ctrl+RePág/AvPág; clic del medio o × cierra |
| Paleta de comandos | Ctrl+K, F1 |
| Explicar regla (sobre un diagnóstico) | F1 |
| Quick-doc | F4, Alt+D (Shift+F4 cambia la orientación) |
| Compilar | F6 |
| Compilar y ejecutar en el deck | Ctrl+F5 |
| Correr las pruebas (make test) | Ctrl+F6 |
| Depurar / continuar | F5 (Shift+F5 detiene) |
| Paso / entrar / salir | F10 / F11 / Shift+F11 |
| Breakpoint | Ctrl+B o clic en el margen |
| Memoria (cat-scan) | Alt+M |
| Terminal | Alt+T |
| Problemas / salida | Alt+P / Alt+O |
| Git | Alt+G (Espacio stage, a/u todo, c commit, l log, Enter diff) |
| Reglas de ripley | Alt+R |
| Vista previa de Markdown | Ctrl+P |
| Menú | F10 (fuera de la depuración) o clic |
| Vistas en modo compacto | F7 consigna, F8 código, F9 terminal, F4 doc |
| Entregar examen | F12 |
| Salir | Ctrl+Q |

En el panel de la terminal, `Alt+T` o `Esc Esc` devuelven el foco al editor.
";

/// Tecla Alt+letra que abre el menú `i`, si no la toma antes un atajo global.
fn alt_de_menu(menus: &[(&str, Vec<Accion>)], i: usize) -> Option<char> {
    // Mismo criterio que la TUI: el primer menú cuyo nombre empieza con la letra, y las
    // letras de los atajos globales con Alt ganan.
    const GLOBALES: &[char] = &['t', 'g', 'e', 'h', 'y', 'f', 'd', 'r', 'p', 'o', 'm'];
    let c = menus[i].0.chars().next()?.to_lowercase().next()?;
    let primero = menus
        .iter()
        .position(|(n, _)| n.to_lowercase().starts_with(c));
    (primero == Some(i) && !GLOBALES.contains(&c)).then_some(c)
}

/// La parte generada de `docs/menus.md`: la barra y cada menú con sus atajos.
pub fn documento_menus() -> String {
    let mut s = String::new();
    let menus = menus(true);
    // La barra, como se ve (en examen aparece además «Examen»).
    let barra: String = menus.iter().map(|(n, _)| format!(" {n} ")).collect();
    s.push_str("```text\n");
    s.push_str(&format!(" {barra}\n"));
    s.push_str("```\n\n");
    s.push_str("| Menú | Abre con | Entradas |\n|---|---|---|\n");
    for (i, (nombre, acciones)) in menus.iter().enumerate() {
        let tecla = match alt_de_menu(&menus, i) {
            Some(c) => format!("Alt+{}", c.to_ascii_uppercase()),
            None => "F10 y flechas, o clic".into(),
        };
        s.push_str(&format!("| {nombre} | {tecla} | {} |\n", acciones.len()));
    }
    for (nombre, acciones) in &menus {
        s.push_str(&format!("\n## {nombre}\n\n"));
        if *nombre == "Examen" {
            s.push_str("Sólo en modo examen (`--exam`).\n\n");
        }
        // El menú desplegado, como caja.
        let filas: Vec<(String, &str)> = acciones
            .iter()
            .map(|a| {
                let i = a.info();
                (i.titulo.to_string(), i.atajo)
            })
            .collect();
        let ancho_t = filas
            .iter()
            .map(|(t, _)| t.chars().count())
            .max()
            .unwrap_or(0);
        let ancho_a = filas
            .iter()
            .map(|(_, a)| a.chars().count())
            .max()
            .unwrap_or(0);
        let interior = ancho_t + ancho_a + 4;
        s.push_str("```text\n");
        s.push_str(&format!("┌{}┐\n", "─".repeat(interior)));
        for (t, a) in &filas {
            let relleno_t = ancho_t - t.chars().count();
            let relleno_a = ancho_a - a.chars().count();
            s.push_str(&format!(
                "│ {t}{}  {}{a} │\n",
                " ".repeat(relleno_t),
                " ".repeat(relleno_a)
            ));
        }
        s.push_str(&format!("└{}┘\n", "─".repeat(interior)));
        s.push_str("```\n");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const INICIO: &str = "<!-- menus: inicio (generado por accion::documento_menus) -->\n";
    const FIN: &str = "<!-- menus: fin -->";

    /// docs/menus.md tiene que coincidir con los menús del código. Para regenerarlo:
    /// `JONESY_ACTUALIZAR_DOCS=1 cargo test -p jonesy-tui documento_de_menus`.
    #[test]
    fn documento_de_menus_al_dia() {
        let ruta = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/menus.md");
        let doc = std::fs::read_to_string(&ruta).expect("docs/menus.md");
        let (antes, resto) = doc.split_once(INICIO).expect("marcador de inicio");
        let (_, despues) = resto.split_once(FIN).expect("marcador de fin");
        let nuevo = format!("{antes}{INICIO}\n{}\n{FIN}{despues}", documento_menus());
        if std::env::var_os("JONESY_ACTUALIZAR_DOCS").is_some() {
            std::fs::write(&ruta, &nuevo).unwrap();
            return;
        }
        assert!(
            doc == nuevo,
            "docs/menus.md no coincide con los menús: JONESY_ACTUALIZAR_DOCS=1 cargo test -p jonesy-tui documento_de_menus"
        );
    }

    #[test]
    fn todas_las_acciones_de_los_menus_estan_en_la_paleta() {
        for (_, acciones) in menus(true) {
            for a in acciones {
                assert!(
                    a == Accion::Paleta || Accion::todas().contains(&a),
                    "{a:?} está en un menú pero no en la paleta"
                );
            }
        }
    }
}
