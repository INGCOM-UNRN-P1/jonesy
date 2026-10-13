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
    Configuracion,
    LimpiarCompilacion,
    RestablecerPaneles,
    GuiaRapida,
    EstadoEntrega,
    ConfigurarPlugins,
    Doctor,
    Bienvenida,
    Comentar,
    DuplicarLinea,
    SubirLinea,
    BajarLinea,
    LimpiarEspacios,
    Reemplazar,
    IrADefinicion,
    Volver,
    Esquema,
    BuscarEnProyecto,
    AlternarMarcador,
    SiguienteMarcador,
    Marcadores,
    SiguienteProblema,
    AnteriorProblema,
    EjecutarConEntrada,
    CompararSalidas,
    ArgumentosPrograma,
    ProbarAqui,
    CorrerHastaCursor,
    GitSubir,
    GitTraer,
    MaximizarPanel,
    Simulacro,
    TerminarSimulacro,
    Glosario,
    Tablas,
    CatalogoReglas,
    Precedencia,
    Recorrido,
    RenombrarSimbolo,
    PausarExamen,
    AtajosPanel,
    ExportarPreferencias,
    ImportarPreferencias,
    ReportarProblema,
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
            QuickDoc => ("Documentación contextual (quick-doc)", "F4"),
            OrientacionQuickDoc => ("Quick-doc: derecha ⟷ inferior", "Shift+F4"),
            Terminal => ("Terminal (deck)", "Alt+T"),
            Problemas => ("Problemas y diagnósticos", "Alt+P"),
            SalidaCompilacion => ("Salida de la compilación", "Alt+O"),
            PanelGit => ("Abrir panel interactivo de Git", "Alt+G"),
            PanelReglas => ("Panel de reglas pedagógicas (ripley)", "Alt+R"),
            PreviewMarkdown => ("Vista previa de Markdown", "Ctrl+P"),
            VistaConsigna => ("Modo consigna", "F7"),
            VistaCodigo => ("Modo código", "F8"),
            VistaTerminal => ("Modo terminal integrada", "F9"),
            VistaDoc => ("Ver documentación", "F4"),
            Compilar => ("Compilar archivo o proyecto", "F6"),
            Ejecutar => ("Compilar y ejecutar", "Ctrl+F5"),
            Probar => ("Correr pruebas (make test)", "Ctrl+F6"),
            RevisarMemoria => ("Revisar memoria con valgrind (fugas y accesos)", ""),
            Depurar => ("Iniciar depuración / continuar", "F5"),
            Paso => ("Paso a paso (siguiente línea)", "F10"),
            Entrar => ("Entrar en la función (step into)", "F11"),
            SalirFuncion => ("Salir de la función (step out)", "Shift+F11"),
            Detener => ("Detener depuración", "Shift+F5"),
            AlternarBreakpoint => ("Poner / quitar breakpoint", "Ctrl+B"),
            CatScan => ("Inspección visual de memoria (cat-scan)", "Alt+M"),
            GitStageTodo => ("Git: agregar todo al stage", ""),
            GitUnstageTodo => ("Git: quitar todo del stage", ""),
            GitCommit => ("Confirmar cambios (commit)…", ""),
            GitLog => ("Historial de cambios (log)", ""),
            Entregar => ("Entregar examen…", "F12"),
            ExplicarRegla => ("Explicar error o regla actual", "F1"),
            Atajos => ("Atajos de teclado", ""),
            AtajosPanel => ("Atajos de este panel", "? en el panel"),
            ExportarPreferencias => ("Exportar mis preferencias…", ""),
            ImportarPreferencias => ("Importar preferencias…", ""),
            AcercaDe => ("Acerca de jonesy", ""),
            Paleta => ("Paleta de comandos", "Ctrl+K"),
            TamanosPaneles => ("Tamaños de paneles…", ""),
            CambiarTema => ("Cambiar tema (oscuro, claro, contraste, turbo…)", ""),
            Explorador => ("Explorador de archivos", "Alt+E"),
            EstructuraProyecto => ("Ver ejercicios y objetivos del proyecto", "Alt+Y"),
            NuevoArchivo => ("Nuevo archivo…", "Ctrl+N"),
            AbrirProyecto => ("Abrir proyecto o directorio…", ""),
            GitClonar => ("Clonar repositorio…", ""),
            Formatear => ("Autoformatear código (gaff o clang-format)", "Alt+Shift+F"),
            GenerarDoc => ("Generar documentación (make doc)", ""),
            DiffArchivo => ("Comparar cambios (diff lado a lado)", "Alt+Shift+D"),
            NuevoPlugin => ("Nuevo plugin Python (.jonesy/plugins)…", ""),
            RecargarPlugins => ("Recargar los plugins Python", ""),
            ExportarMemoria => ("Exportar traza de memoria (reporte HTML)", ""),
            GitDeshacerCommit => ("Deshacer último commit (mantener en stage)", ""),
            Herramientas => ("Herramientas del ecosistema para este archivo…", "Alt+H"),
            Integraciones => ("Configurar integraciones (qué corre y cuándo)…", ""),
            Configuracion => ("Configuración del entorno…", "Ctrl+,"),
            LimpiarCompilacion => ("Limpiar compilación (make clean)", ""),
            RestablecerPaneles => ("Restablecer disposición de paneles", ""),
            GuiaRapida => ("Guía rápida para estudiantes de C", ""),
            EstadoEntrega => ("Verificar estado de entrega", ""),
            ConfigurarPlugins => ("Configurar plugins…", ""),
            Doctor => ("Diagnóstico del entorno (jonesy doctor)", ""),
            ReportarProblema => ("Reportar un problema de jonesy…", ""),
            Bienvenida => ("Bienvenida", ""),
            Comentar => ("Comentar / descomentar líneas", "Ctrl+/"),
            DuplicarLinea => ("Duplicar línea", "Ctrl+D"),
            SubirLinea => ("Mover línea arriba", "Alt+↑"),
            BajarLinea => ("Mover línea abajo", "Alt+↓"),
            LimpiarEspacios => ("Quitar espacios al final de las líneas", ""),
            Reemplazar => ("Buscar y reemplazar…", "Ctrl+H"),
            IrADefinicion => ("Ir a la definición", "F12 / Ctrl+clic"),
            Volver => ("Volver a donde estaba", "Alt+←"),
            Esquema => ("Esquema del archivo (funciones y tipos)", "Alt+S"),
            BuscarEnProyecto => ("Buscar en el proyecto…", "Ctrl+Shift+F"),
            AlternarMarcador => ("Poner / quitar marcador", "Ctrl+F2"),
            SiguienteMarcador => ("Ir al marcador siguiente", "Shift+F2"),
            Marcadores => ("Marcadores…", ""),
            SiguienteProblema => ("Ir al problema siguiente", "Alt+N"),
            AnteriorProblema => ("Ir al problema anterior", "Alt+Shift+N"),
            EjecutarConEntrada => (
                "Ejecutar con entrada… (archivo de entrada para Ctrl+F5)",
                "",
            ),
            CompararSalidas => ("Comparar con las salidas esperadas", ""),
            ArgumentosPrograma => ("Argumentos del programa…", ""),
            ProbarAqui => ("Correr la prueba del cursor", "Shift+F6"),
            CorrerHastaCursor => ("Correr hasta el cursor", "Ctrl+F10"),
            GitSubir => ("Subir cambios (push)", ""),
            GitTraer => ("Traer cambios (pull)", ""),
            MaximizarPanel => ("Maximizar el panel actual (y volver)", "Alt+Z"),
            Simulacro => ("Simulacro de examen…", ""),
            TerminarSimulacro => ("Terminar el simulacro", "F12"),
            Glosario => ("Glosario de C…", ""),
            Tablas => ("Tablas de consulta (tipos, printf, ASCII, precedencia)", ""),
            CatalogoReglas => ("Reglas de la cátedra (catálogo con ejemplos)…", ""),
            Precedencia => (
                "Ver la precedencia de la expresión (paréntesis implícitos)",
                "",
            ),
            Recorrido => ("Recorrido guiado por la pantalla", ""),
            RenombrarSimbolo => ("Renombrar en su ámbito…", "Ctrl+R"),
            PausarExamen => ("Pausar / reanudar el cronómetro (código del docente)…", ""),
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
            AtajosPanel,
            ExportarPreferencias,
            ImportarPreferencias,
            AcercaDe,
            Configuracion,
            LimpiarCompilacion,
            RestablecerPaneles,
            GuiaRapida,
            EstadoEntrega,
            ConfigurarPlugins,
            Doctor,
            ReportarProblema,
            Bienvenida,
            Comentar,
            DuplicarLinea,
            SubirLinea,
            BajarLinea,
            LimpiarEspacios,
            Reemplazar,
            IrADefinicion,
            Volver,
            Esquema,
            BuscarEnProyecto,
            AlternarMarcador,
            SiguienteMarcador,
            Marcadores,
            SiguienteProblema,
            AnteriorProblema,
            EjecutarConEntrada,
            CompararSalidas,
            ArgumentosPrograma,
            ProbarAqui,
            CorrerHastaCursor,
            GitSubir,
            GitTraer,
            MaximizarPanel,
            Simulacro,
            TerminarSimulacro,
            Glosario,
            Tablas,
            CatalogoReglas,
            Precedencia,
            Recorrido,
            RenombrarSimbolo,
            PausarExamen,
            Salir,
        ]
    }
}

/// Un menú de la barra superior.
pub struct Menu {
    pub nombre: &'static str,
    /// Letra mnemónica (subrayada en la barra): Alt+letra si no la toma un atajo global, o
    /// la letra sola con un menú abierto (F10 y letra).
    pub tecla: char,
    pub items: Vec<Entrada>,
}

/// Una entrada de un menú.
#[derive(Debug, Clone, PartialEq)]
pub enum Entrada {
    Accion(Accion),
    Separador,
    /// Lugar de los comandos de los plugins y de las acciones de las herramientas del
    /// proyecto: la interfaz lo reemplaza por entradas `Externa` al abrir el menú.
    ComandosPlugins,
    /// Entrada armada en tiempo de ejecución; `id` indica cuál a quien la armó.
    Externa {
        titulo: String,
        atajo: String,
        id: usize,
    },
    /// Texto que no se elige (encabezado o «ninguno»).
    Nota(String),
}

impl Entrada {
    /// Se puede elegir con el teclado o el mouse.
    pub fn elegible(&self) -> bool {
        matches!(self, Entrada::Accion(_) | Entrada::Externa { .. })
    }

    pub fn titulo(&self) -> String {
        match self {
            Entrada::Accion(a) => a.info().titulo.into(),
            Entrada::Externa { titulo, .. } | Entrada::Nota(titulo) => titulo.clone(),
            Entrada::ComandosPlugins => "‹comandos de los plugins›".into(),
            Entrada::Separador => String::new(),
        }
    }

    pub fn atajo(&self) -> String {
        match self {
            Entrada::Accion(a) => a.info().atajo.into(),
            Entrada::Externa { atajo, .. } => atajo.clone(),
            _ => String::new(),
        }
    }
}

/// La entrada elegible más cercana a `sel` (hacia abajo, o la primera).
pub fn ajustar_sel(items: &[Entrada], sel: usize) -> usize {
    if items.get(sel).is_some_and(Entrada::elegible) {
        return sel;
    }
    (sel..items.len())
        .chain(0..sel)
        .find(|&i| items[i].elegible())
        .unwrap_or(0)
}

impl Menu {
    /// Las acciones fijas, sin separadores ni entradas armadas en tiempo de ejecución.
    pub fn acciones(&self) -> impl Iterator<Item = Accion> + '_ {
        self.items.iter().filter_map(|e| match e {
            Entrada::Accion(a) => Some(*a),
            _ => None,
        })
    }
}

/// Letras con Alt que son atajos globales (paneles): ganan sobre los menús.
pub const ALT_GLOBALES: &[char] = &['t', 'g', 'e', 'h', 'y', 's', 'n', 'z', 'r', 'p', 'o', 'm'];

/// Menús de la barra superior, ordenados según el ciclo de trabajo del alumno: archivos,
/// edición, vistas, construir (compilar, correr, probar), depurar (paso a paso y
/// memoria), git y los plugins del proyecto. La configuración del entorno (integraciones, plugins, compilador,
/// apariencia) está en Archivo → Configuración del entorno.
pub fn menus(examen: bool) -> Vec<Menu> {
    use Accion::*;
    let menu = |nombre, tecla, items: Vec<Entrada>| Menu {
        nombre,
        tecla,
        items,
    };
    const SEP: Entrada = Entrada::Separador;
    use Entrada::Accion as A;
    let mut m = vec![
        menu(
            "Archivo",
            'a',
            vec![
                A(NuevoArchivo),
                A(Abrir),
                A(AbrirProyecto),
                SEP,
                A(Guardar),
                A(GuardarTodo),
                SEP,
                A(CerrarBuffer),
                A(SiguienteBuffer),
                A(AnteriorBuffer),
                SEP,
                A(Simulacro),
                A(Configuracion),
                SEP,
                A(Salir),
            ],
        ),
        menu(
            "Editar",
            'e',
            vec![
                A(Deshacer),
                A(Rehacer),
                SEP,
                A(Cortar),
                A(Copiar),
                A(Pegar),
                A(SeleccionarTodo),
                SEP,
                A(Buscar),
                A(BuscarSiguiente),
                A(Reemplazar),
                A(BuscarEnProyecto),
                A(IrALinea),
                A(IrADefinicion),
                A(Volver),
                SEP,
                A(AlternarMarcador),
                A(SiguienteMarcador),
                A(Marcadores),
                SEP,
                A(Comentar),
                A(DuplicarLinea),
                A(SubirLinea),
                A(BajarLinea),
                SEP,
                A(Formatear),
                A(LimpiarEspacios),
                A(RenombrarSimbolo),
                A(Precedencia),
            ],
        ),
        menu(
            "Ver",
            'v',
            vec![
                A(VistaCodigo),
                A(VistaConsigna),
                A(VistaTerminal),
                SEP,
                A(Explorador),
                A(Esquema),
                A(Problemas),
                A(SiguienteProblema),
                A(AnteriorProblema),
                A(SalidaCompilacion),
                A(Terminal),
                A(PanelGit),
                A(PanelReglas),
                SEP,
                A(QuickDoc),
                A(PreviewMarkdown),
                SEP,
                A(MaximizarPanel),
                A(RestablecerPaneles),
            ],
        ),
        menu(
            "Construir",
            'c',
            vec![
                A(Compilar),
                A(Ejecutar),
                A(EjecutarConEntrada),
                A(ArgumentosPrograma),
                A(CompararSalidas),
                A(Probar),
                A(ProbarAqui),
                SEP,
                A(LimpiarCompilacion),
                A(GenerarDoc),
                SEP,
                A(EstructuraProyecto),
            ],
        ),
        menu(
            "Depurar",
            'd',
            vec![
                A(Depurar),
                A(Detener),
                A(AlternarBreakpoint),
                SEP,
                A(Paso),
                A(Entrar),
                A(SalirFuncion),
                A(CorrerHastaCursor),
                SEP,
                A(RevisarMemoria),
                A(CatScan),
                A(ExportarMemoria),
            ],
        ),
        menu(
            "Git",
            'g',
            vec![
                A(PanelGit),
                SEP,
                A(DiffArchivo),
                A(GitLog),
                SEP,
                A(GitCommit),
                A(GitDeshacerCommit),
                SEP,
                A(GitTraer),
                A(GitSubir),
                SEP,
                A(GitClonar),
            ],
        ),
        menu(
            "Plugins",
            'l',
            vec![
                Entrada::ComandosPlugins,
                SEP,
                A(ConfigurarPlugins),
                A(Integraciones),
                A(NuevoPlugin),
                A(RecargarPlugins),
            ],
        ),
    ];
    if examen {
        m.push(menu(
            "Examen",
            'x',
            vec![A(Entregar), A(EstadoEntrega), SEP, A(PausarExamen)],
        ));
    }
    m.push(menu(
        "Ayuda",
        'y',
        vec![
            A(Paleta),
            A(ExplicarRegla),
            SEP,
            A(Bienvenida),
            A(Recorrido),
            A(GuiaRapida),
            A(Glosario),
            A(Tablas),
            A(CatalogoReglas),
            A(Atajos),
            A(AtajosPanel),
            A(Doctor),
            A(ReportarProblema),
            A(AcercaDe),
        ],
    ));
    m
}

/// Guía rápida para quien empieza (Ayuda → Guía rápida).
pub const GUIA_RAPIDA: &str = "\
# Guía rápida

El ciclo de trabajo de un ejercicio, de punta a punta.

## 1. Leer la consigna

**F7** muestra la consigna; **F8** vuelve al código. En pantallas chicas se ve una cosa por
vez: F7 consigna, F8 código, F9 terminal.

## 2. Escribir el código

Mientras escribís una llamada (`printf(`, `malloc(`), el panel **quick-doc** (F4) muestra
qué recibe la función, qué devuelve y sus trampas comunes. **Ctrl+S** guarda; al guardar,
las herramientas de la cátedra revisan el estilo y te marcan lo que encuentran al final de
la línea.

## 3. Compilar — F6

Si hay errores, aparecen en **Problemas** explicados en español. Enter sobre uno lleva a la
línea; **F1** abre la explicación completa con un ejemplo de cómo arreglarlo. Empezá
siempre por el **primer** error: los siguientes suelen ser consecuencia.

## 4. Ejecutar — Ctrl+F5

El programa corre en la terminal de abajo, donde podés escribirle la entrada. Si se corta
(una *violación de segmento*, por ejemplo), jonesy te dice por qué y **F5** te lleva a la
línea donde falló.

## 5. Probar — Ctrl+F6

Corre las pruebas del ejercicio (`make test`). Cada prueba que falla aparece en Problemas
con lo que se esperaba y lo que se obtuvo.

## 6. Depurar — F5

Poné un *breakpoint* con **Ctrl+B** (o clic en el margen) y **F5**. Después: **F10** avanza
una línea, **F11** entra en la función, **Shift+F11** sale. A la derecha ves las
variables; **Alt+M** dibuja la memoria (stack y heap) con flechas.

## 7. Revisar la memoria

**Depurar → Revisar memoria con valgrind** busca lo que no corta el programa pero está
mal: leer o escribir fuera de un arreglo, usar memoria después de `free`, fugas.

## 8. Guardar el trabajo en git

**Alt+G** abre el panel de git: Espacio agrega un archivo, `c` confirma (commit) con un
mensaje.

## Si no sabés dónde está algo

**Ctrl+K** abre la paleta: escribí lo que querés hacer («compilar», «memoria», «tema») y
Enter.
";

pub const AYUDA_ATAJOS: &str = "\
# Atajos de jonesy

| Acción | Teclas |
|---|---|
| Guardar | Ctrl+S, F2 |
| Deshacer / rehacer | Ctrl+Z / Ctrl+Y |
| Copiar / cortar / pegar | Ctrl+C / Ctrl+X / Ctrl+V |
| Buscar | F3, Ctrl+F (Shift+F3 siguiente) |
| Buscar y reemplazar (Tab campo, Alt+M mayúsculas, Alt+P palabra, Enter reemplaza, F3 saltea, Alt+T todas) | Ctrl+H |
| Buscar en el proyecto (resultados en el panel Búsqueda) | Ctrl+Shift+F o Ctrl+F3 |
| Marcador: poner o quitar · siguiente | Ctrl+F2 · Shift+F2 |
| Problema siguiente · anterior | Alt+N · Alt+Shift+N |
| Correr la prueba del cursor · correr hasta el cursor | Shift+F6 · Ctrl+F10 |
| Maximizar el panel actual (y volver) | Alt+Z |
| Ir a línea | Ctrl+G |
| Ir a la definición · volver | F12 (fuera del examen) o Ctrl+clic · Alt+← |
| Esquema del archivo (funciones, tipos, macros) | Alt+S |
| Comentar / descomentar · duplicar línea | Ctrl+/ · Ctrl+D |
| Renombrar la variable o función del cursor (sólo donde vale) | Ctrl+R |
| Mover línea arriba / abajo | Alt+↑ / Alt+↓ |
| Autoformato (gaff o clang-format) | Alt+Shift+F |
| Diff del archivo contra git (lado a lado; s alterna, n/p cambio) | Alt+Shift+D |
| Herramientas del ecosistema (gaff fix, tetsuo, motoko…) | Alt+H |
| Nuevo archivo · abrir (Tab marca varios) | Ctrl+N · Ctrl+O |
| Explorador de archivos · proyecto (make y tp.sh) | Alt+E · Alt+Y (Tab alterna) |
| Pestañas | Ctrl+Tab / Ctrl+Shift+Tab, Alt+1…9, Ctrl+RePág/AvPág; clic del medio o × cierra |
| Paleta de comandos | Ctrl+K, F1 |
| Explicar regla (sobre un diagnóstico) | F1 |
| Quick-doc | F4 (Shift+F4 cambia la orientación) |
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
| Menú | Alt+letra subrayada, F10 y la letra, o clic |
| Configuración del entorno | Archivo → Configuración (Ctrl+, si la terminal lo transmite) |
| Vistas en modo compacto | F7 consigna, F8 código, F9 terminal, F4 doc |
| Entregar examen | F12 |
| Salir | Ctrl+Q |

En el panel de la terminal, `Alt+T` o `Esc Esc` devuelven el foco al editor.
";

/// Cómo se abre un menú con el teclado.
pub fn como_se_abre(m: &Menu) -> String {
    let letra = m.tecla.to_ascii_uppercase();
    if ALT_GLOBALES.contains(&m.tecla) {
        format!("F10 y {letra}")
    } else {
        format!("Alt+{letra} (o F10 y {letra})")
    }
}

/// El nombre con la letra mnemónica marcada entre corchetes: «Constr[u]ir».
fn con_mnemonico(m: &Menu) -> String {
    let mut s = String::new();
    let mut marcada = false;
    for c in m.nombre.chars() {
        if !marcada && c.to_lowercase().next() == Some(m.tecla) {
            s.push_str(&format!("[{c}]"));
            marcada = true;
        } else {
            s.push(c);
        }
    }
    s
}

/// La parte generada de `docs/menus.md`: la barra y cada menú con sus atajos.
pub fn documento_menus() -> String {
    let mut s = String::new();
    let menus = menus(true);
    // La barra, como se ve (en examen aparece además «Examen»).
    let barra: String = menus.iter().map(|m| format!(" {} ", m.nombre)).collect();
    s.push_str("```text\n");
    s.push_str(&format!(" {barra}\n"));
    s.push_str("```\n\n");
    s.push_str("| Menú | Letra | Se abre con | Entradas |\n|---|---|---|---|\n");
    for m in &menus {
        s.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            m.nombre,
            con_mnemonico(m),
            como_se_abre(m),
            m.acciones().count()
        ));
    }
    for m in &menus {
        s.push_str(&format!("\n## {}\n\n", m.nombre));
        if m.nombre == "Examen" {
            s.push_str("Sólo en modo examen (`--exam`).\n\n");
        }
        // El menú desplegado, como caja. Las entradas del proyecto se muestran con un
        // ejemplo de cómo se ven.
        let filas: Vec<Option<(String, String)>> = m
            .items
            .iter()
            .flat_map(|e| match e {
                Entrada::Separador => vec![None],
                Entrada::ComandosPlugins => vec![
                    Some(("Plugins Python".to_string(), String::new())),
                    Some(("  ‹plugin›: ‹comando›".to_string(), String::new())),
                    None,
                    Some((
                        "Herramientas para este archivo".to_string(),
                        "Alt+H".to_string(),
                    )),
                    Some(("  ‹herramienta›: ‹acción›".to_string(), String::new())),
                ],
                e => vec![Some((e.titulo(), e.atajo()))],
            })
            .collect();
        let ancho_t = filas
            .iter()
            .flatten()
            .map(|(t, _)| t.chars().count())
            .max()
            .unwrap_or(0);
        let ancho_a = filas
            .iter()
            .flatten()
            .map(|(_, a)| a.chars().count())
            .max()
            .unwrap_or(0);
        let interior = ancho_t + ancho_a + 4;
        s.push_str("```text\n");
        s.push_str(&format!("┌{}┐\n", "─".repeat(interior)));
        for f in &filas {
            match f {
                None => s.push_str(&format!("├{}┤\n", "─".repeat(interior))),
                Some((t, a)) => {
                    let relleno_t = ancho_t - t.chars().count();
                    let relleno_a = ancho_a - a.chars().count();
                    s.push_str(&format!(
                        "│ {t}{}  {}{a} │\n",
                        " ".repeat(relleno_t),
                        " ".repeat(relleno_a)
                    ));
                }
            }
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
        for m in menus(true) {
            for a in m.acciones() {
                assert!(
                    a == Accion::Paleta || Accion::todas().contains(&a),
                    "{a:?} está en un menú pero no en la paleta"
                );
            }
        }
    }

    #[test]
    fn menus_sin_letras_repetidas_ni_acciones_duplicadas() {
        let m = menus(true);
        let mut letras: Vec<char> = m.iter().map(|m| m.tecla).collect();
        letras.sort();
        letras.dedup();
        assert_eq!(letras.len(), m.len(), "dos menús con la misma letra");
        for x in &m {
            assert!(
                x.nombre.to_lowercase().contains(x.tecla),
                "la letra de {} no está en su nombre",
                x.nombre
            );
            // Sin separadores al principio, al final ni seguidos.
            let sep = |e: &Entrada| *e == Entrada::Separador;
            assert!(!sep(x.items.first().unwrap()) && !sep(x.items.last().unwrap()));
            assert!(!x.items.windows(2).any(|w| sep(&w[0]) && sep(&w[1])));
        }
        // Cada acción en un solo lugar (salvo el panel de git, que encabeza su menú y
        // también está entre los paneles de Ver).
        let mut todas: Vec<Accion> = m
            .iter()
            .flat_map(|x| x.acciones())
            .filter(|a| *a != Accion::PanelGit)
            .collect();
        let n = todas.len();
        todas.sort_by_key(|a| format!("{a:?}"));
        todas.dedup();
        assert_eq!(todas.len(), n, "una acción aparece en dos menús");
        // Los atajos de una letra son del panel de git, no del menú.
        for a in m.iter().flat_map(|x| x.acciones()) {
            assert!(
                a.info().atajo.chars().count() != 1,
                "{a:?} muestra un atajo local"
            );
        }
    }
}
