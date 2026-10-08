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
        };
        Info { titulo, atajo }
    }

    /// Acciones de la paleta (todas menos la propia paleta).
    pub fn todas() -> &'static [Accion] {
        use Accion::*;
        &[
            Compilar,
            Ejecutar,
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
            Entregar,
            ExplicarRegla,
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
                Abrir,
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
            ],
        ),
        (
            "Ver",
            vec![
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
            ],
        ),
        (
            "Construir",
            vec![
                Compilar,
                Ejecutar,
                Depurar,
                Paso,
                Entrar,
                SalirFuncion,
                Detener,
                AlternarBreakpoint,
                CatScan,
            ],
        ),
        (
            "Git",
            vec![PanelGit, GitStageTodo, GitUnstageTodo, GitCommit, GitLog],
        ),
    ];
    if examen {
        m.push(("Examen", vec![Entregar]));
    }
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
| Abrir archivo (Tab marca varios) | Ctrl+O |
| Pestañas | Ctrl+Tab / Ctrl+Shift+Tab, Alt+1…9, Ctrl+RePág/AvPág; clic del medio o × cierra |
| Paleta de comandos | Ctrl+K, F1 |
| Explicar regla (sobre un diagnóstico) | F1 |
| Quick-doc | F4, Alt+D (Shift+F4 cambia la orientación) |
| Compilar | F6 |
| Compilar y ejecutar en el deck | Ctrl+F5 |
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
