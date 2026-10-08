//! Estado de la aplicación y despacho de eventos.
//!
//! Un único canal de [`Aviso`] despierta el bucle: eventos de la terminal (hilo de
//! entrada), salida de procesos (compilación, ripley, verificación, mem-trace), respuestas
//! de plugins, despertares del deck y de GDB, y señales. Tras procesar todo lo pendiente
//! se redibuja una sola vez; sin eventos, el bucle duerme hasta el próximo temporizador.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use jonesy_core::catscan::Traza;
use jonesy_core::gdb::{EventoGdb, Gdb, Marco, Parada, Variable};
use jonesy_core::proceso::{self, Salida};
use jonesy_core::sesion::{ArchivoAbierto, EstadoExamen, Sesion};
use jonesy_core::{
    diagnostico, entrega, Buffer, Delta, Diagnostico, Lenguaje, Origen, PosicionDoc, Proyecto,
    Severidad, TamPaneles,
};
use jonesy_git::{Commit, Repo};
use jonesy_plugins::{
    ComandoPlugin, Contexto, Pedido as PedidoPlugin, Registro, Respuesta, Servicio, Supervisor,
};
use jonesy_telemetry::{Telemetria, TelemetryEvent};
use jonesy_terminal::{bytes_de_tecla, Deck, PtyInferior, Tecla};
use ratatui::crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;
use ratatui::text::Line;

use crate::accion::{self, Accion};
use crate::layout::{Areas, Modo, Vista};
use crate::markdown::{self, Documento};
use crate::navegador::{Explorador, FilaProyecto, PestanaNav, VistaProyecto};
use crate::paleta;
use crate::resaltado;
use crate::sistema::{self, Aviso};
use crate::tema::{self, Simbolos};

pub const AUTOGUARDADO: Duration = Duration::from_secs(30);
pub const DEBOUNCE_PREVIEW: Duration = Duration::from_millis(120);
pub const SONDEO_GIT: Duration = Duration::from_secs(2);
pub const AVISOS_MIN: [u64; 3] = [30, 15, 5];
/// Las herramientas lentas (ripley tarda ~10 s) corren este tiempo después del último
/// guardado: guardar seguido no las relanza cada vez.
pub const DEMORA_LENTAS: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone)]
pub struct Opciones {
    pub raiz: PathBuf,
    pub archivos: Vec<PathBuf>,
    pub examen: bool,
    pub ascii: bool,
    /// Telemetría fuera del modo examen (en examen siempre está activa).
    pub telemetria: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Foco {
    Editor,
    Consigna,
    Inferior,
    Lateral,
    Doc,
    Navegador,
    /// La vista previa de Markdown (para recorrer sus enlaces con el teclado).
    Preview,
}

impl Foco {
    fn nombre(self) -> &'static str {
        match self {
            Foco::Editor => "editor",
            Foco::Consigna => "consigna",
            Foco::Inferior => "inferior",
            Foco::Lateral => "lateral",
            Foco::Doc => "quickdoc",
            Foco::Navegador => "navegador",
            Foco::Preview => "vista previa",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pestana {
    Problemas,
    Salida,
    Terminal,
    Ejecucion,
    Depurador,
}

impl Pestana {
    pub const TODAS: [Pestana; 5] = [
        Pestana::Problemas,
        Pestana::Salida,
        Pestana::Terminal,
        Pestana::Ejecucion,
        Pestana::Depurador,
    ];

    pub fn titulo(self) -> &'static str {
        match self {
            Pestana::Problemas => "Problemas",
            Pestana::Salida => "Salida",
            Pestana::Terminal => "Terminal",
            Pestana::Ejecucion => "Ejecución",
            Pestana::Depurador => "Depurador",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lateral {
    Git,
    Reglas,
    Variables,
    CatScan,
}

#[derive(Debug, Clone)]
pub enum QuePaleta {
    Accion(Accion),
    Plugin(ComandoPlugin),
    /// Objetivo de make: (índice del módulo, objetivo).
    Objetivo(usize, String),
    /// Comando de tp.sh por índice.
    Tp(usize),
    /// Acción de una integración: (nombre, índice de la acción).
    Herramienta(String, usize),
}

#[derive(Debug, Clone)]
pub struct OpcionPaleta {
    pub titulo: String,
    pub atajo: String,
    pub que: QuePaleta,
}

/// Fila del diálogo Abrir proyecto.
#[derive(Debug, Clone, PartialEq)]
pub struct EntradaDir {
    pub etiqueta: String,
    pub destino: PathBuf,
    /// true: abre `destino` como proyecto; false: entra a navegarlo.
    pub abrir: bool,
    /// Tiene Makefile, .git o .jonesy.toml.
    pub es_proyecto: bool,
}

/// Directorio con pinta de proyecto.
pub fn parece_proyecto(d: &Path) -> bool {
    [
        "Makefile",
        "makefile",
        ".git",
        jonesy_core::proyecto::ARCHIVO_CONFIG,
        "tp.sh",
    ]
    .iter()
    .any(|n| d.join(n).exists())
}

/// Entradas del diálogo para `dir`: abrir aquí, recientes (sólo al empezar), subir y subcarpetas.
pub fn entradas_dir(dir: &Path, con_recientes: bool) -> Vec<EntradaDir> {
    let mut v = vec![EntradaDir {
        etiqueta: format!("Abrir {}", dir.display()),
        destino: dir.to_path_buf(),
        abrir: true,
        es_proyecto: parece_proyecto(dir),
    }];
    if con_recientes {
        for r in jonesy_core::sesion::recientes::cargar() {
            v.push(EntradaDir {
                etiqueta: format!("reciente: {}", r.display()),
                destino: r.clone(),
                abrir: true,
                es_proyecto: true,
            });
        }
    }
    if let Some(p) = dir.parent() {
        v.push(EntradaDir {
            etiqueta: "../".into(),
            destino: p.to_path_buf(),
            abrir: false,
            es_proyecto: false,
        });
    }
    let mut subs: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|it| {
            it.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.is_dir()
                        && !p
                            .file_name()
                            .is_some_and(|n| n.to_string_lossy().starts_with('.'))
                })
                .collect()
        })
        .unwrap_or_default();
    subs.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    for d in subs {
        let nombre = d
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        v.push(EntradaDir {
            etiqueta: format!("{nombre}/"),
            es_proyecto: parece_proyecto(&d),
            destino: d,
            abrir: false,
        });
    }
    v
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirmacion {
    Salir,
    CerrarBuffer,
    Entregar,
    /// Abrir el repositorio recién clonado (`App::proyecto_pendiente`).
    AbrirClonado,
    /// Restaurar las copias de recuperación de un corte anterior (`App::recuperables`).
    Recuperar,
    /// Borrar el archivo o la carpeta de `App::borrar_pendiente`.
    Borrar,
    /// git reset --soft HEAD~1.
    DeshacerCommit,
}

pub enum Modal {
    Paleta {
        filtro: String,
        sel: usize,
        opciones: Vec<OpcionPaleta>,
    },
    Abrir {
        filtro: String,
        sel: usize,
        archivos: Vec<String>,
        /// Índices (en `archivos`) marcados con Tab para abrir varios a la vez.
        marcados: BTreeSet<usize>,
    },
    Buscar {
        texto: String,
    },
    IrALinea {
        texto: String,
    },
    Confirmar {
        titulo: String,
        mensaje: String,
        que: Confirmacion,
    },
    Commit {
        mensaje: String,
    },
    Visor {
        titulo: String,
        lineas: Vec<Line<'static>>,
        desplazamiento: usize,
    },
    Menu {
        indice: usize,
        sel: usize,
    },
    /// Argumentos para un comando (tp.sh add-lib …) que corre en el deck.
    Argumentos {
        titulo: String,
        comando: String,
        texto: String,
    },
    /// Nuevo nombre (ruta relativa a la raíz) para un archivo o carpeta.
    Renombrar {
        origen: PathBuf,
        texto: String,
    },
    /// Nombre del archivo (o carpeta) nuevo, relativo a la raíz.
    NuevoArchivo {
        texto: String,
        carpeta: bool,
    },
    /// Git → Clonar: URL y carpeta donde crear el clon; `campo` 0 = URL, 1 = destino.
    Clonar {
        url: String,
        destino: String,
        campo: usize,
    },
    /// Archivo → Abrir proyecto: navegador de directorios.
    AbrirProyecto {
        dir: PathBuf,
        filtro: String,
        sel: usize,
        entradas: Vec<EntradaDir>,
    },
    /// Diferencias entre dos versiones, lado a lado o unificadas (s alterna).
    Diff {
        titulo: String,
        filas: Vec<jonesy_core::diff::FilaDiff>,
        desplazamiento: usize,
        lado_a_lado: bool,
        /// Columnas desplazadas con ←/→ para leer líneas largas.
        desplazamiento_h: usize,
        /// Lenguaje para el resaltado (en diffs de varios archivos, el de cada uno).
        lenguaje: Lenguaje,
    },
    /// Herramientas → Integraciones: fila elegida.
    Integraciones {
        sel: usize,
    },
    /// Ver → Tamaños de paneles: fila elegida.
    Paneles {
        sel: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nivel {
    Info,
    Ok,
    Aviso,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Luego {
    Nada,
    Ejecutar,
    Depurar,
}

#[derive(Debug, Clone)]
enum Tarea {
    Compilar {
        luego: Luego,
    },
    /// Integración que corre al guardar (ripley, gaff, kaneda…).
    Integracion {
        nombre: String,
        archivo: PathBuf,
    },
    /// Acción del menú Herramientas que termina en un visor, en diagnósticos o recargando
    /// el archivo.
    AccionHerramienta {
        integracion: String,
        titulo: String,
        archivo: PathBuf,
        modo: jonesy_core::integraciones::Modo,
    },
    Verificar,
    Explicar {
        codigo: String,
        enlace: Option<String>,
    },
    MemTrace {
        traza: PathBuf,
    },
    /// Objetivo de make lanzado desde la vista de proyecto o la paleta.
    Objetivo {
        comando: String,
    },
    /// Autoformato sobre una copia temporal del buffer.
    Formatear {
        archivo: PathBuf,
        temporal: PathBuf,
        version: u64,
    },
}

pub struct BufferAbierto {
    pub buf: Buffer,
    pub cache: resaltado::Cache,
    /// Vista previa de Markdown: (versión renderizada, ancho, documento).
    pub preview: Option<(u64, u16, Documento)>,
    pub preview_desplazamiento: usize,
    /// Enlace elegido con Tab en la vista previa.
    pub preview_enlace: Option<usize>,
    pub ultimo_cambio: Instant,
}

impl BufferAbierto {
    fn nuevo(buf: Buffer) -> BufferAbierto {
        BufferAbierto {
            buf,
            cache: Default::default(),
            preview: None,
            preview_desplazamiento: 0,
            preview_enlace: None,
            ultimo_cambio: Instant::now(),
        }
    }
}

pub struct Consigna {
    pub ruta: PathBuf,
    pub fuente: String,
    pub doc: Option<(u16, Documento)>,
    pub desplazamiento: usize,
    /// Enlace elegido con Tab (RF-8.3).
    pub enlace: Option<usize>,
}

pub struct Depuracion {
    pub gdb: Gdb,
    /// PTY propio del programa depurado (None en Windows: comparte la consola de GDB).
    pub pty: Option<PtyInferior>,
    pub marcos: Vec<Marco>,
    pub variables: Vec<Variable>,
    pub ubicacion: Option<(PathBuf, usize)>,
    pub consola: Vec<String>,
    pub entrada: String,
    /// true: el teclado va a la consola de GDB; false: al programa.
    pub foco_consola: bool,
    pub estado: String,
    pub traza: Option<Traza>,
    pub paso_traza: Option<usize>,
    pub tam_pty: (u16, u16),
}

pub struct EstadoExamenUi {
    pub estado: EstadoExamen,
    pub avisos_dados: BTreeSet<u64>,
    pub ultimo_autoguardado: Instant,
    pub entregando: bool,
}

#[derive(Default)]
pub struct PanelGit {
    pub sel: usize,
    pub log: Option<Vec<Commit>>,
    pub sel_log: usize,
    /// Commit marcado con m en el log: Enter sobre otro muestra el diff entre ambos.
    pub marcado: Option<String>,
}

/// Zonas clicables registradas en el último dibujo.
#[derive(Default)]
pub struct Zonas {
    pub menu: Vec<(Rect, usize)>,
    pub botones: Vec<(Rect, Accion)>,
    pub pestanas_buffers: Vec<(Rect, usize)>,
    /// Pestañas Archivos / Proyecto de la barra izquierda.
    pub pestanas_nav: Vec<(Rect, PestanaNav)>,
    /// Lista de la barra izquierda: (área, primera fila visible).
    pub lista_nav: Option<(Rect, usize)>,
    /// Botón × de cada pestaña.
    pub cierres_buffers: Vec<(Rect, usize)>,
    /// Flechas ‹ › de desborde de la barra de pestañas (desplazamiento −1 / +1).
    pub desborde_pestanas: Vec<(Rect, isize)>,
    pub pestanas_inferior: Vec<(Rect, Pestana)>,
    pub editor: Option<ZonaEditor>,
    pub preview: Option<Rect>,
    pub lista_inferior: Option<(Rect, usize)>,
    pub lista_lateral: Option<(Rect, usize)>,
    pub deck: Option<Rect>,
    pub items_menu: Vec<(Rect, Accion)>,
    pub modal_items: Option<(Rect, usize)>,
    pub botones_depurador: Vec<(Rect, Accion)>,
}

#[derive(Clone, Copy)]
pub struct ZonaEditor {
    pub texto: Rect,
    pub gutter: Rect,
}

pub struct App {
    pub proyecto: Proyecto,
    pub sim: Simbolos,
    pub buffers: Vec<BufferAbierto>,
    pub activo: usize,
    pub consigna: Option<Consigna>,
    pub consigna_visible: bool,
    pub foco: Foco,
    pub pestana: Pestana,
    pub panel_inferior: bool,
    /// Tamaños de los paneles (mouse, Ver → Tamaños de paneles, `.jonesy.toml`).
    pub paneles: TamPaneles,
    /// Barra izquierda visible y su pestaña.
    pub navegador: Option<PestanaNav>,
    pub explorador: Explorador,
    pub vista_proyecto: VistaProyecto,
    /// Releer la estructura cuando termine el comando del deck (tp.sh add-lib …).
    refrescar_estructura: bool,
    /// Opciones de arranque (para reabrir otro proyecto con las mismas).
    pub opciones: Opciones,
    /// Documentación del proyecto (`doc/api.json` de corbel) y su vigencia.
    pub doc: Option<jonesy_core::documentacion::DocProyecto>,
    pub estado_doc: jonesy_core::documentacion::EstadoDoc,
    doc_modificado: Option<std::time::SystemTime>,
    /// Cómo generar la documentación (se recalcula al cargarla y al releer la estructura).
    comando_doc: Option<String>,
    /// Última vista elegida en los diffs (lado a lado o unificada).
    pub preferir_lado_a_lado: bool,
    /// Archivo o carpeta a borrar si se confirma.
    borrar_pendiente: Option<PathBuf>,
    /// Copias de recuperación pendientes de restaurar o descartar: (archivo, copia).
    pub recuperables: Vec<(PathBuf, PathBuf)>,
    /// Destino de un `git clone` en curso en el deck.
    clonando: Option<PathBuf>,
    /// Proyecto a abrir si se confirma (después de clonar).
    proyecto_pendiente: Option<PathBuf>,
    pub lateral: Option<Lateral>,
    pub quickdoc_visible: bool,
    pub quickdoc_posicion: PosicionDoc,
    pub vista: Vista,
    pub preview_markdown: bool,
    pub modal: Option<Modal>,
    pub mensaje: Option<(String, Nivel, Instant)>,
    pub portapapeles: String,
    pub busqueda: String,

    pub diag_compilador: Vec<Diagnostico>,
    /// Hallazgos de las integraciones: (herramienta, archivo) → diagnósticos.
    pub diag_herramientas: HashMap<(String, PathBuf), Vec<Diagnostico>>,
    /// Herramientas del ecosistema integradas como plugins (de fábrica, usuario, proyecto).
    pub integraciones: Vec<jonesy_core::integraciones::Integracion>,
    pub errores_integraciones: Vec<String>,
    pub diag_plugins: HashMap<PathBuf, Vec<Diagnostico>>,
    pub sel_problema: usize,
    pub sel_regla: usize,
    pub salida: Vec<(String, bool)>,
    pub titulo_salida: String,
    pub salida_desplazamiento: usize,
    /// Integraciones que no se pudieron lanzar (código 127): no se reintentan.
    integraciones_ausentes: BTreeSet<String>,
    /// Archivos guardados mientras una integración ya los analizaba: se repite una vez.
    integraciones_pendientes: BTreeSet<(String, PathBuf)>,
    /// Integraciones lentas programadas para después del último guardado.
    integraciones_programadas: HashMap<(String, PathBuf), Instant>,

    pub terminal: Option<Deck>,
    pub ejecucion: Option<Deck>,
    pub depuracion: Option<Depuracion>,
    pub breakpoints: BTreeSet<(PathBuf, usize)>,

    pub repo: Option<Repo>,
    pub git_estado: Option<jonesy_git::Estado>,
    git_marca: Option<(std::time::SystemTime, std::time::SystemTime)>,
    ultimo_sondeo_git: Instant,
    pub git: PanelGit,

    pub examen: Option<EstadoExamenUi>,
    pub telemetria: Option<Telemetria>,
    ultimo_cursor: (String, usize, usize),
    ultimo_cursor_registrado: Instant,

    pub registro: Registro,
    plugins: Option<Servicio>,
    pub comandos_plugins: Vec<ComandoPlugin>,

    procesos: HashMap<u64, Tarea>,
    siguiente_id: u64,
    tx: Sender<Aviso>,

    pub areas: Areas,
    pub zonas: Zonas,
    pub tam: (u16, u16),
    arrastre: Option<Arrastre>,
    ultimo_clic: Option<(Instant, u16, u16)>,
    esc_pendiente: Option<Instant>,
    /// La rueda del mouse movió la vista: no forzar el cursor a la vista hasta la próxima tecla.
    pub desplazado_por_mouse: bool,
    /// Primera pestaña visible cuando no entran todas.
    pub primera_pestana: usize,
    /// Funciones declaradas en el buffer activo (quick-doc): (buffer, versión, funciones).
    pub funciones_usuario: Option<(usize, u64, Vec<jonesy_doc::Funcion>)>,
    pub salir: bool,
    pub inicio: Instant,
}

#[derive(Clone, Copy)]
enum Arrastre {
    Seleccion,
    Separador(crate::layout::Separador),
}

impl App {
    pub fn nueva(op: Opciones, tx: Sender<Aviso>) -> App {
        let proyecto = Proyecto::abrir(&op.raiz);
        let ascii = op.ascii || proyecto.config.interfaz.ascii || !tema::locale_utf8();
        let sim = if ascii {
            Simbolos::ascii()
        } else {
            Simbolos::unicode()
        };
        let dir_estado = proyecto.dir_estado();
        let examen = op.examen.then(|| {
            let estado = EstadoExamen::cargar(&dir_estado).unwrap_or_else(|| EstadoExamen {
                inicio_unix: jonesy_core::ahora_unix(),
                duracion_min: proyecto.config.examen.duracion_min,
                ..Default::default()
            });
            let _ = std::fs::create_dir_all(&dir_estado);
            let _ = estado.guardar(&dir_estado);
            EstadoExamenUi {
                estado,
                avisos_dados: BTreeSet::new(),
                ultimo_autoguardado: Instant::now(),
                entregando: false,
            }
        });
        let entregado = examen.as_ref().is_some_and(|e| e.estado.entregado);
        let telemetria = if (op.examen || op.telemetria) && !entregado {
            let _ = std::fs::create_dir_all(&dir_estado);
            Telemetria::abrir(dir_estado.join("telemetry.log")).ok()
        } else {
            None
        };
        if let Some(t) = &telemetria {
            t.registrar(TelemetryEvent::SessionStart {
                user: jonesy_core::usuario(),
                exam_mode: op.examen,
            });
        }
        let permitidos = proyecto.config.examen.plugins_permitidos.clone();
        let sup = Supervisor::nuevo(
            &dir_estado.join("plugins"),
            op.examen.then_some(permitidos.as_slice()),
        );
        let rechazados = sup.rechazados.len();
        let plugins = sup.hay_plugins().then(|| {
            let tx = tx.clone();
            Servicio::iniciar(sup, move |r| {
                let _ = tx.send(Aviso::Plugins(r));
            })
        });
        let repo = Repo::abrir(&proyecto.raiz);
        let consigna = proyecto.consigna().and_then(|ruta| {
            let fuente = std::fs::read_to_string(&ruta).ok()?;
            Some(Consigna {
                ruta,
                fuente,
                doc: None,
                desplazamiento: 0,
                enlace: None,
            })
        });
        let mut app = App {
            sim,
            buffers: vec![],
            activo: 0,
            consigna_visible: consigna.is_some(),
            consigna,
            foco: Foco::Editor,
            pestana: Pestana::Problemas,
            panel_inferior: false,
            paneles: proyecto.config.interfaz.paneles.acotar(),
            navegador: None,
            explorador: Explorador::default(),
            vista_proyecto: VistaProyecto::default(),
            refrescar_estructura: false,
            opciones: op.clone(),
            doc: None,
            estado_doc: jonesy_core::documentacion::EstadoDoc::Ausente,
            doc_modificado: None,
            comando_doc: None,
            preferir_lado_a_lado: true,
            recuperables: vec![],
            borrar_pendiente: None,
            clonando: None,
            proyecto_pendiente: None,
            lateral: None,
            quickdoc_visible: true,
            quickdoc_posicion: proyecto.config.interfaz.quickdoc,
            vista: Vista::Codigo,
            preview_markdown: false,
            modal: None,
            mensaje: None,
            portapapeles: String::new(),
            busqueda: String::new(),
            diag_compilador: vec![],
            diag_herramientas: HashMap::new(),
            integraciones: vec![],
            errores_integraciones: vec![],
            diag_plugins: HashMap::new(),
            sel_problema: 0,
            sel_regla: 0,
            salida: vec![],
            titulo_salida: String::new(),
            salida_desplazamiento: 0,
            integraciones_ausentes: BTreeSet::new(),
            integraciones_pendientes: BTreeSet::new(),
            integraciones_programadas: HashMap::new(),
            terminal: None,
            ejecucion: None,
            depuracion: None,
            breakpoints: BTreeSet::new(),
            repo,
            git_estado: None,
            git_marca: None,
            ultimo_sondeo_git: Instant::now(),
            git: PanelGit::default(),
            examen,
            telemetria,
            ultimo_cursor: (String::new(), usize::MAX, usize::MAX),
            ultimo_cursor_registrado: Instant::now(),
            registro: Registro::de_fabrica(),
            plugins,
            comandos_plugins: vec![],
            procesos: HashMap::new(),
            siguiente_id: 1,
            tx,
            areas: Areas::default(),
            zonas: Zonas::default(),
            tam: (80, 24),
            arrastre: None,
            ultimo_clic: None,
            esc_pendiente: None,
            desplazado_por_mouse: false,
            primera_pestana: 0,
            funciones_usuario: None,
            salir: false,
            inicio: Instant::now(),
            proyecto,
        };
        app.cargar_integraciones();
        app.restaurar_sesion(&op.archivos);
        app.ofrecer_recuperacion();
        app.cargar_doc();
        app.refrescar_git();
        if let Some(e) = app.proyecto.error_config.clone() {
            app.avisar(e, Nivel::Error);
        } else if entregado {
            app.avisar(
                "El examen ya fue entregado: los archivos quedan de solo lectura.",
                Nivel::Aviso,
            );
            for b in &mut app.buffers {
                b.buf.solo_lectura = true;
            }
        } else if rechazados > 0 {
            app.avisar(
                format!(
                    "Modo examen: {rechazados} plugin(s) sin firma de la cátedra no se cargaron."
                ),
                Nivel::Aviso,
            );
        } else if app.examen.is_some() {
            app.avisar(
                "Modo examen: la sesión se registra y se guarda automáticamente.",
                Nivel::Info,
            );
        }
        app
    }

    // ------------------------------------------------------------ sesión

    fn restaurar_sesion(&mut self, archivos: &[PathBuf]) {
        let dir = self.proyecto.dir_estado();
        if let Some(s) = Sesion::cargar(&dir) {
            for a in &s.archivos {
                let ruta = self.proyecto.raiz.join(&a.ruta);
                if ruta.is_file() && self.abrir_archivo(&ruta, false).is_some() {
                    let b = &mut self.buffers.last_mut().unwrap().buf;
                    b.cursor = a.cursor.min(b.largo());
                    b.desplazamiento = a.desplazamiento;
                }
            }
            self.activo = s.activo.min(self.buffers.len().saturating_sub(1));
            self.quickdoc_visible = s.quickdoc_visible;
            if let Some(p) = s.quickdoc_posicion {
                self.quickdoc_posicion = p;
            }
            self.vista = Vista::de_nombre(&s.vista_compacta);
            if let Some(p) = s.paneles {
                self.paneles = p.acotar();
            }
            self.navegador = s.izquierdo.as_deref().and_then(PestanaNav::de_nombre);
            for rel in &s.expandidos {
                self.explorador
                    .expandidos
                    .insert(self.proyecto.raiz.join(rel));
            }
            for (nombre, activa) in &s.integraciones {
                if let Some(x) = self.integraciones.iter_mut().find(|x| x.nombre == *nombre) {
                    x.activa = *activa;
                }
            }
            self.preview_markdown = s.preview_markdown;
            for (rel, linea) in s.breakpoints {
                self.breakpoints
                    .insert((self.proyecto.raiz.join(rel), linea));
            }
        }
        for a in archivos {
            let ruta = if a.is_absolute() {
                a.clone()
            } else {
                std::env::current_dir().unwrap_or_default().join(a)
            };
            if let Some(i) = self.abrir_archivo(&ruta, true) {
                self.activo = i;
            }
        }
        if self.buffers.is_empty() {
            let raiz = self.proyecto.raiz.clone();
            let candidatos = self.proyecto.archivos(5000);
            let consigna = self.consigna.as_ref().map(|c| c.ruta.clone());
            let elegido = [raiz.join("main.c"), raiz.join("src/main.c")]
                .into_iter()
                .find(|p| p.is_file())
                .or_else(|| {
                    candidatos
                        .iter()
                        .find(|p| p.file_name().is_some_and(|n| n == "main.c"))
                        .cloned()
                })
                .or_else(|| {
                    candidatos
                        .iter()
                        .find(|p| p.extension().is_some_and(|e| e == "c"))
                        .cloned()
                })
                .or_else(|| {
                    candidatos
                        .iter()
                        .find(|p| Some(*p) != consigna.as_ref())
                        .cloned()
                });
            if let Some(p) = elegido {
                self.abrir_archivo(&p, false);
            } else {
                self.buffers
                    .push(BufferAbierto::nuevo(Buffer::desde_texto("")));
            }
        }
    }

    pub fn guardar_sesion(&self) {
        let dir = self.proyecto.dir_estado();
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let s = Sesion {
            archivos: self
                .buffers
                .iter()
                .filter_map(|b| {
                    let ruta = b.buf.ruta.as_ref()?;
                    Some(ArchivoAbierto {
                        ruta: self.proyecto.relativa(ruta),
                        cursor: b.buf.cursor,
                        desplazamiento: b.buf.desplazamiento,
                    })
                })
                .collect(),
            activo: self.activo,
            quickdoc_visible: self.quickdoc_visible,
            quickdoc_posicion: Some(self.quickdoc_posicion),
            vista_compacta: self.vista.nombre().into(),
            preview_markdown: self.preview_markdown,
            breakpoints: self
                .breakpoints
                .iter()
                .map(|(p, l)| (self.proyecto.relativa(p), *l))
                .collect(),
            paneles: Some(self.paneles),
            izquierdo: self.navegador.map(|p| p.nombre().to_string()),
            expandidos: self
                .explorador
                .expandidos
                .iter()
                .map(|p| self.proyecto.relativa(p))
                .collect(),
            integraciones: self
                .integraciones
                .iter()
                .map(|x| (x.nombre.clone(), x.activa))
                .collect(),
        };
        let _ = s.guardar(&dir);
    }

    // ------------------------------------------------------------ utilidades

    pub fn avisar(&mut self, texto: impl Into<String>, nivel: Nivel) {
        self.mensaje = Some((texto.into(), nivel, Instant::now()));
    }

    fn tel(&self, e: TelemetryEvent) {
        if let Some(t) = &self.telemetria {
            t.registrar(e);
        }
    }

    pub fn modo(&self) -> Modo {
        Modo::de_tamano(self.tam.0, self.tam.1)
    }

    pub fn buf(&self) -> &BufferAbierto {
        &self.buffers[self.activo]
    }

    pub fn buf_mut(&mut self) -> &mut BufferAbierto {
        &mut self.buffers[self.activo]
    }

    fn id_buffer(&self, i: usize) -> String {
        self.buffers[i].buf.id(&self.proyecto.raiz)
    }

    fn proximo_id(&mut self) -> u64 {
        let id = self.siguiente_id;
        self.siguiente_id += 1;
        id
    }

    fn lanzar(&mut self, comando: &str, tarea: Tarea) -> u64 {
        let id = self.proximo_id();
        self.procesos.insert(id, tarea);
        let tx = self.tx.clone();
        let (txp, rxp) = std::sync::mpsc::channel::<Salida>();
        std::thread::spawn(move || {
            for s in rxp {
                if tx.send(Aviso::Proceso(s)).is_err() {
                    break;
                }
            }
        });
        proceso::lanzar(id, comando, &self.proyecto.raiz, &[], txp);
        id
    }

    pub fn examen_entregado(&self) -> bool {
        self.examen.as_ref().is_some_and(|e| e.estado.entregado)
    }

    /// Todos los diagnósticos, por archivo y línea.
    pub fn diagnosticos(&self) -> Vec<&Diagnostico> {
        let mut v: Vec<&Diagnostico> = self
            .diag_compilador
            .iter()
            .chain(self.diag_herramientas.values().flatten())
            .chain(self.diag_plugins.values().flatten())
            .collect();
        // Varias herramientas informan lo mismo (ripley incluye las reglas de gaff y de
        // kaneda): la misma regla en la misma línea se muestra una vez, prefiriendo la
        // versión con categoría y enlace al apunte.
        let clave = |d: &Diagnostico| d.codigo.clone().unwrap_or_else(|| d.mensaje.clone());
        v.sort_by(|a, b| {
            (
                &a.archivo,
                a.linea,
                clave(a),
                a.enlace.is_none(),
                a.categoria.is_none(),
            )
                .cmp(&(
                    &b.archivo,
                    b.linea,
                    clave(b),
                    b.enlace.is_none(),
                    b.categoria.is_none(),
                ))
        });
        v.dedup_by(|b, a| a.archivo == b.archivo && a.linea == b.linea && clave(a) == clave(b));
        v.sort_by(|a, b| {
            (&a.archivo, a.linea, a.severidad).cmp(&(&b.archivo, b.linea, b.severidad))
        });
        v
    }

    /// Diagnósticos de una línea (0-based) del buffer activo.
    pub fn diagnosticos_linea(&self, ruta: &Path, linea: usize) -> Vec<&Diagnostico> {
        self.diagnosticos()
            .into_iter()
            .filter(|d| {
                d.linea == linea + 1 && d.archivo.as_deref().is_some_and(|a| mismo_archivo(a, ruta))
            })
            .collect()
    }

    // ------------------------------------------------------------ buffers

    /// Abre (o activa) un archivo; devuelve su índice.
    pub fn abrir_archivo(&mut self, ruta: &Path, activar: bool) -> Option<usize> {
        let ruta = jonesy_core::canonica(ruta);
        if let Some(i) = self
            .buffers
            .iter()
            .position(|b| b.buf.ruta.as_deref() == Some(&ruta))
        {
            if activar {
                self.cambiar_buffer(i);
            }
            return Some(i);
        }
        let mut buf = if ruta.exists() {
            match Buffer::abrir(&ruta) {
                Ok(b) => b,
                Err(e) => {
                    self.avisar(
                        format!("No se pudo abrir {}: {e}", ruta.display()),
                        Nivel::Error,
                    );
                    return None;
                }
            }
        } else {
            let mut b = Buffer::desde_texto("");
            b.lenguaje = Lenguaje::de_ruta(&ruta);
            b.ruta = Some(ruta.clone());
            b
        };
        if self.examen.is_some() {
            let protegido = self
                .consigna
                .as_ref()
                .is_some_and(|c| mismo_archivo(&c.ruta, &ruta))
                || ruta
                    .file_name()
                    .is_some_and(|n| n == jonesy_core::proyecto::ARCHIVO_CONFIG)
                || ruta.starts_with(self.proyecto.dir_estado());
            if protegido || self.examen_entregado() {
                buf.solo_lectura = true;
            }
        }
        // Un buffer vacío sin ruta (el inicial) se reemplaza.
        if self.buffers.len() == 1
            && self.buffers[0].buf.ruta.is_none()
            && !self.buffers[0].buf.modificado()
        {
            self.buffers.clear();
        }
        self.buffers.push(BufferAbierto::nuevo(buf));
        let i = self.buffers.len() - 1;
        if activar {
            self.cambiar_buffer(i);
        }
        Some(i)
    }

    fn cambiar_buffer(&mut self, i: usize) {
        if i < self.buffers.len() && i != self.activo {
            self.activo = i;
            self.tel(TelemetryEvent::FocusChange {
                panel: format!("buffer:{}", self.id_buffer(i)),
                focused: true,
            });
        }
        self.foco = Foco::Editor;
        if self.modo() == Modo::Compacto {
            self.vista = Vista::Codigo;
        }
    }

    /// Aplica una edición al buffer activo y la registra en la telemetría.
    fn editar(&mut self, f: impl FnOnce(&mut Buffer) -> Option<Delta>) {
        if self.buf().buf.solo_lectura {
            self.avisar("Archivo de solo lectura.", Nivel::Aviso);
            return;
        }
        let id = self.id_buffer(self.activo);
        let b = self.buf_mut();
        if let Some(d) = f(&mut b.buf) {
            b.ultimo_cambio = Instant::now();
            self.tel(TelemetryEvent::TextDelta {
                buffer_id: id,
                offset: d.offset,
                inserted: d.insertado,
                deleted_len: d.borrados,
            });
        }
    }

    pub fn guardar(&mut self, i: usize) -> bool {
        let id = self.id_buffer(i);
        let b = &mut self.buffers[i].buf;
        if b.ruta.is_none() {
            self.avisar(
                "Este buffer no tiene archivo: abrí uno con Ctrl+O.",
                Nivel::Aviso,
            );
            return false;
        }
        if b.solo_lectura {
            return false;
        }
        match b.guardar() {
            Ok(bytes) => {
                self.tel(TelemetryEvent::BufferSave {
                    buffer_id: id,
                    total_bytes: bytes,
                });
                let ruta = self.buffers[i].buf.ruta.clone().unwrap();
                self.al_guardar(&ruta, i);
                true
            }
            Err(e) => {
                self.avisar(format!("No se pudo guardar: {e}"), Nivel::Error);
                false
            }
        }
    }

    fn guardar_todo(&mut self) -> bool {
        let mut ok = true;
        for i in 0..self.buffers.len() {
            if self.buffers[i].buf.modificado()
                && self.buffers[i].buf.ruta.is_some()
                && !self.buffers[i].buf.solo_lectura
            {
                ok &= self.guardar(i);
            }
        }
        ok
    }

    /// ripley, extensiones nativas y plugins Python tras guardar.
    fn al_guardar(&mut self, ruta: &Path, i: usize) {
        if ruta.extension().is_some_and(|e| e == "h" || e == "c") && self.doc.is_some() {
            self.reevaluar_doc();
        }
        let texto = self.buffers[i].buf.contenido();
        let nativos = self.registro.diagnosticar(ruta, &texto);
        self.diag_plugins.insert(ruta.to_path_buf(), nativos);
        if let Some(p) = &self.plugins {
            p.pedir(PedidoPlugin::AlGuardar {
                archivo: ruta.to_path_buf(),
                texto,
            });
        }
        let al_guardar: Vec<(String, bool)> = self
            .integraciones_activas()
            .filter(|x| x.cuando == jonesy_core::integraciones::Cuando::Guardar && x.aplica_a(ruta))
            .map(|x| (x.nombre.clone(), x.lento))
            .collect();
        for (nombre, lento) in al_guardar {
            if lento {
                self.integraciones_programadas
                    .insert((nombre, ruta.to_path_buf()), Instant::now() + DEMORA_LENTAS);
            } else {
                self.lanzar_integracion(&nombre, ruta);
            }
        }
        self.refrescar_git();
    }

    // ------------------------------------------------------------ integraciones

    /// Carga las integraciones (de fábrica, del usuario y del proyecto) y les aplica la
    /// configuración de `.jonesy.toml` (incluida la sección `[ripley]` de antes).
    pub fn cargar_integraciones(&mut self) {
        let (mut v, errores) = jonesy_core::integraciones::cargar(
            &self.proyecto.dir_estado(),
            &self.proyecto.config.integraciones,
        );
        let r = &self.proyecto.config.ripley;
        if let Some(x) = v.iter_mut().find(|x| x.nombre == "ripley") {
            if !r.activo {
                x.activa = false;
            }
            if !r.comando.trim().is_empty() && r.comando != "ripley" {
                x.binario = r
                    .comando
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_string();
                x.comando = x.comando.replacen("ripley", &r.comando, 1);
                x.explicar = x.explicar.replacen("ripley", &r.comando, 1);
            }
        }
        self.integraciones = v;
        self.errores_integraciones = errores;
    }

    /// Las que corren: activas, con su binario instalado y permitidas en examen.
    pub fn integraciones_activas(
        &self,
    ) -> impl Iterator<Item = &jonesy_core::integraciones::Integracion> {
        let examen = self.examen.is_some();
        self.integraciones.iter().filter(move |x| {
            x.activa
                && (!examen || x.examen)
                && !self.integraciones_ausentes.contains(&x.nombre)
                && x.disponible()
        })
    }

    /// Algunas herramientas tardan varios segundos (ripley compila y aplica todas las
    /// reglas): si ya está analizando el archivo, se encola una sola repetición.
    fn lanzar_integracion(&mut self, nombre: &str, ruta: &Path) {
        let en_curso = self.procesos.values().any(
            |t| matches!(t, Tarea::Integracion { nombre: n, archivo } if n == nombre && archivo == ruta),
        );
        if en_curso {
            self.integraciones_pendientes
                .insert((nombre.to_string(), ruta.to_path_buf()));
            return;
        }
        let Some(x) = self.integraciones.iter().find(|x| x.nombre == nombre) else {
            return;
        };
        let modulo = self
            .proyecto
            .estructura
            .modulo_de(ruta)
            .map(|m| m.dir.clone());
        let cmd = jonesy_core::integraciones::expandir(
            &x.comando,
            &self.proyecto.raiz,
            Some(ruta),
            modulo.as_deref(),
            None,
        );
        self.lanzar(
            &cmd,
            Tarea::Integracion {
                nombre: nombre.to_string(),
                archivo: ruta.to_path_buf(),
            },
        );
    }

    fn fin_integracion(
        &mut self,
        nombre: String,
        archivo: PathBuf,
        codigo: Option<i32>,
        salida: String,
    ) {
        if codigo == Some(127) {
            // No está instalada (o el comando configurado no existe): no se insiste.
            self.integraciones_ausentes.insert(nombre);
            return;
        }
        if self
            .integraciones_pendientes
            .remove(&(nombre.clone(), archivo.clone()))
        {
            // Se guardó de nuevo mientras analizaba: este resultado ya es viejo.
            self.lanzar_integracion(&nombre, &archivo);
            return;
        }
        let Some(x) = self
            .integraciones
            .iter()
            .find(|x| x.nombre == nombre)
            .cloned()
        else {
            return;
        };
        match jonesy_core::integraciones::interpretar(&x, &salida, &archivo, &self.proyecto.raiz) {
            Ok(d) => {
                self.diag_herramientas.insert((nombre, archivo), d);
            }
            Err(e) => {
                self.diag_herramientas.remove(&(nombre.clone(), archivo));
                self.avisar(
                    format!("{nombre}: no se pudo leer su salida ({e})."),
                    Nivel::Aviso,
                );
            }
        }
    }

    /// Acciones de las integraciones activas que aplican al archivo activo:
    /// (integración, índice de la acción, título).
    pub fn acciones_disponibles(&self) -> Vec<(String, usize, String)> {
        let Some(ruta) = self.activo_ruta() else {
            return vec![];
        };
        self.integraciones_activas()
            .flat_map(|x| {
                x.acciones
                    .iter()
                    .enumerate()
                    .filter(|(_, a)| a.aplica_a(x, &ruta))
                    .map(|(n, a)| (x.nombre.clone(), n, a.titulo.clone()))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// Corre una acción del menú Herramientas sobre el archivo activo.
    pub fn correr_accion_herramienta(&mut self, integracion: &str, n: usize) {
        use jonesy_core::integraciones::Modo;
        let Some(ruta) = self.activo_ruta() else {
            self.avisar("Abrí un archivo para usar las herramientas.", Nivel::Aviso);
            return;
        };
        let Some(x) = self
            .integraciones
            .iter()
            .find(|x| x.nombre == integracion)
            .cloned()
        else {
            return;
        };
        let Some(a) = x.acciones.get(n).cloned() else {
            return;
        };
        if a.modo == Modo::Recargar && self.buf().buf.solo_lectura {
            self.avisar("Archivo de solo lectura.", Nivel::Aviso);
            return;
        }
        // Las herramientas leen el archivo del disco.
        self.guardar_todo();
        let modulo = self
            .proyecto
            .estructura
            .modulo_de(&ruta)
            .map(|m| m.dir.clone());
        let cmd = jonesy_core::integraciones::expandir(
            &a.comando,
            &self.proyecto.raiz,
            Some(&ruta),
            modulo.as_deref(),
            None,
        );
        self.tel(TelemetryEvent::CommandExec {
            command: format!("{integracion}: {}", a.titulo),
            exit_code: None,
        });
        match a.modo {
            Modo::Terminal => self.correr_en_deck(cmd),
            Modo::Panel => self.correr_en_panel(cmd),
            Modo::Visor | Modo::Recargar | Modo::Diagnosticos => {
                self.avisar(format!("{}…", a.titulo), Nivel::Info);
                self.lanzar(
                    &cmd,
                    Tarea::AccionHerramienta {
                        integracion: integracion.to_string(),
                        titulo: a.titulo,
                        archivo: ruta,
                        modo: a.modo,
                    },
                );
            }
        }
    }

    fn fin_accion_herramienta(
        &mut self,
        integracion: String,
        titulo: String,
        archivo: PathBuf,
        modo: jonesy_core::integraciones::Modo,
        codigo: Option<i32>,
        salida: String,
    ) {
        use jonesy_core::integraciones::Modo;
        match modo {
            Modo::Recargar => {
                if codigo != Some(0) {
                    let detalle = salida
                        .lines()
                        .rev()
                        .find(|l| !l.trim().is_empty())
                        .unwrap_or("")
                        .to_string();
                    self.avisar(format!("{titulo}: falló ({detalle})"), Nivel::Error);
                    return;
                }
                self.recargar_desde_disco(&archivo, &titulo);
            }
            Modo::Diagnosticos => {
                let Some(x) = self
                    .integraciones
                    .iter()
                    .find(|x| x.nombre == integracion)
                    .cloned()
                else {
                    return;
                };
                match jonesy_core::integraciones::interpretar(
                    &x,
                    &salida,
                    &archivo,
                    &self.proyecto.raiz,
                ) {
                    Ok(d) => {
                        let n = d.len();
                        self.diag_herramientas.insert((integracion, archivo), d);
                        self.panel_inferior = true;
                        self.pestana = Pestana::Problemas;
                        self.avisar(
                            format!("{titulo}: {n} hallazgo(s)."),
                            if n == 0 { Nivel::Ok } else { Nivel::Aviso },
                        );
                    }
                    Err(e) => self.avisar(format!("{titulo}: {e}"), Nivel::Error),
                }
            }
            _ => {
                let ancho = self.tam.0.saturating_sub(8).clamp(40, 120);
                let mut lineas = crate::vt::ansi_a_lineas(&salida, ancho);
                if lineas.is_empty() {
                    lineas.push(Line::from(format!(
                        "(sin salida; código {})",
                        codigo.map(|c| c.to_string()).unwrap_or("?".into())
                    )));
                }
                self.modal = Some(Modal::Visor {
                    titulo,
                    lineas,
                    desplazamiento: 0,
                });
            }
        }
    }

    /// Después de una herramienta que modificó el archivo en disco (gaff fix, corbel
    /// scaffold), el buffer toma el contenido nuevo como una edición (Ctrl+Z la deshace).
    fn recargar_desde_disco(&mut self, archivo: &Path, titulo: &str) {
        let Ok(nuevo) = std::fs::read_to_string(archivo) else {
            return;
        };
        let Some(i) = self
            .buffers
            .iter()
            .position(|b| b.buf.ruta.as_deref() == Some(archivo))
        else {
            return;
        };
        if self.buffers[i].buf.contenido() == nuevo {
            self.avisar(format!("{titulo}: no hubo cambios."), Nivel::Info);
            return;
        }
        let anterior = self.activo;
        self.activo = i;
        let (l, c) = self.buf().buf.linea_columna();
        self.editar(|b| {
            let d = b.reemplazar_todo(&nuevo);
            let pos = b.linea_columna_a_pos(l, c);
            b.mover_a(pos, false);
            b.marcar_guardado();
            d
        });
        self.activo = anterior;
        self.avisar(format!("{titulo}: listo (Ctrl+Z lo deshace)."), Nivel::Ok);
    }

    /// Nombres de las tareas en segundo plano, para la barra de estado.
    pub fn tareas_en_curso(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .procesos
            .values()
            .map(|t| match t {
                Tarea::Compilar { .. } => "compilando".into(),
                Tarea::Integracion { nombre, .. } => nombre.clone(),
                Tarea::AccionHerramienta { integracion, .. } => integracion.clone(),
                Tarea::Verificar => "verificando".into(),
                Tarea::Explicar { .. } => "explicando".into(),
                Tarea::MemTrace { .. } => "mem-trace".into(),
                Tarea::Objetivo { .. } => "make".into(),
                Tarea::Formatear { .. } => "formateando".into(),
            })
            .collect();
        v.extend(
            self.integraciones_programadas
                .keys()
                .map(|(n, _)| n.clone()),
        );
        v.sort();
        v.dedup();
        v
    }

    /// Cierra la pestaña `i` (pidiendo confirmación si tiene cambios).
    pub fn cerrar_pestana(&mut self, i: usize) {
        if i < self.buffers.len() {
            self.activo = i;
            self.cerrar_buffer(false);
        }
    }

    fn cerrar_buffer(&mut self, forzar: bool) {
        if self.buf().buf.modificado() && !forzar {
            self.modal = Some(Modal::Confirmar {
                titulo: "Cerrar archivo".into(),
                mensaje: format!(
                    "{} tiene cambios sin guardar. ¿Cerrar igual? (s/n)",
                    self.buf().buf.nombre()
                ),
                que: Confirmacion::CerrarBuffer,
            });
            return;
        }
        self.buffers.remove(self.activo);
        if self.buffers.is_empty() {
            self.buffers
                .push(BufferAbierto::nuevo(Buffer::desde_texto("")));
        }
        self.activo = self.activo.min(self.buffers.len() - 1);
    }

    fn ir_a(&mut self, ruta: &Path, linea: usize, columna: usize) {
        if let Some(i) = self.abrir_archivo(ruta, true) {
            let b = &mut self.buffers[i].buf;
            let pos = b.linea_columna_a_pos(linea.saturating_sub(1), columna.saturating_sub(1));
            b.mover_a(pos, false);
            centrar(b, 10);
        }
    }

    // ------------------------------------------------------------ git

    pub fn refrescar_git(&mut self) {
        if let Some(r) = &self.repo {
            self.git_marca = r.marca();
            self.git_estado = r.estado().ok();
            self.ultimo_sondeo_git = Instant::now();
            if let Some(e) = &self.git_estado {
                self.git.sel = self.git.sel.min(e.entradas.len().saturating_sub(1));
            }
        }
    }

    fn sondear_git(&mut self) {
        if self.ultimo_sondeo_git.elapsed() < SONDEO_GIT {
            return;
        }
        self.ultimo_sondeo_git = Instant::now();
        self.sondear_doc();
        if let Some(r) = &self.repo {
            let marca = r.marca();
            if marca != self.git_marca {
                self.refrescar_git();
            }
        }
    }

    fn git_accion(&mut self, a: Accion) {
        let Some(repo) = self.repo.clone() else {
            self.avisar("El proyecto no está en un repositorio git.", Nivel::Aviso);
            return;
        };
        let r = match a {
            Accion::GitStageTodo => repo
                .agregar_todo()
                .map(|_| "Todo agregado al stage.".to_string()),
            Accion::GitUnstageTodo => repo
                .quitar_todo_del_stage()
                .map(|_| "Stage vaciado.".to_string()),
            Accion::GitCommit => {
                if self
                    .git_estado
                    .as_ref()
                    .is_some_and(|e| e.cantidad_en_stage() == 0)
                {
                    self.avisar(
                        "No hay cambios en el stage (Espacio o 'a' para agregar).",
                        Nivel::Aviso,
                    );
                    return;
                }
                self.modal = Some(Modal::Commit {
                    mensaje: String::new(),
                });
                return;
            }
            Accion::GitLog => {
                self.lateral = Some(Lateral::Git);
                self.foco = Foco::Lateral;
                if self.git.log.is_some() {
                    self.git.log = None;
                    return;
                }
                match repo.log(50) {
                    Ok(l) => {
                        self.git.log = Some(l);
                        self.git.sel_log = 0;
                    }
                    Err(e) => self.avisar(e.to_string(), Nivel::Error),
                }
                return;
            }
            _ => return,
        };
        match r {
            Ok(m) => self.avisar(m, Nivel::Ok),
            Err(e) => self.avisar(e.to_string(), Nivel::Error),
        }
        self.refrescar_git();
    }

    fn git_alternar_stage(&mut self) {
        let (Some(repo), Some(e)) = (self.repo.clone(), self.git_estado.clone()) else {
            return;
        };
        let Some(entrada) = e.entradas.get(self.git.sel) else {
            return;
        };
        let r = if entrada.en_stage() && !entrada.sin_stage() {
            repo.quitar_del_stage(&entrada.ruta)
        } else {
            repo.agregar(&entrada.ruta)
        };
        if let Err(e) = r {
            self.avisar(e.to_string(), Nivel::Error);
        }
        self.refrescar_git();
    }

    /// Enter en el panel de git: el diff del archivo o del commit, lado a lado.
    fn git_mostrar(&mut self) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        if let Some(log) = &self.git.log {
            let Some(c) = log.get(self.git.sel_log).cloned() else {
                return;
            };
            // Con otro commit marcado (m): el diff entre los dos, del más viejo al más nuevo.
            if let Some(m) = self.git.marcado.clone().filter(|m| *m != c.hash) {
                let pos = |h: &str| log.iter().position(|x| x.hash == h).unwrap_or(0);
                let (viejo, nuevo) = if pos(&m) > pos(&c.hash) {
                    (m, c.hash.clone())
                } else {
                    (c.hash.clone(), m)
                };
                match repo.diff_commits(&viejo, &nuevo) {
                    Ok(t) => {
                        let filas = jonesy_core::diff::desde_unificado(&t);
                        if filas.is_empty() {
                            self.avisar(
                                format!("Sin diferencias entre {viejo} y {nuevo}."),
                                Nivel::Info,
                            );
                        } else {
                            self.abrir_diff(format!("{viejo} → {nuevo}"), filas);
                        }
                    }
                    Err(e) => self.avisar(e.to_string(), Nivel::Error),
                }
                return;
            }
            match repo.mostrar(&c.hash) {
                Ok(t) => {
                    let filas = jonesy_core::diff::desde_unificado(&t);
                    if filas.is_empty() {
                        // Commits sin cambios de texto (merge, binarios): el texto tal cual.
                        self.modal = Some(Modal::Visor {
                            titulo: format!("git show {}", c.hash),
                            lineas: lineas_diff(&t),
                            desplazamiento: 0,
                        });
                    } else {
                        self.abrir_diff(format!("{} {}", c.hash, c.resumen), filas);
                    }
                }
                Err(e) => self.avisar(e.to_string(), Nivel::Error),
            }
            return;
        }
        let Some(e) = self
            .git_estado
            .as_ref()
            .and_then(|e| e.entradas.get(self.git.sel))
            .cloned()
        else {
            return;
        };
        let bytes = std::fs::read(repo.dir.join(&e.ruta)).unwrap_or_default();
        if jonesy_core::diff::es_binario(&bytes) {
            self.avisar(
                format!(
                    "{} es un archivo binario: no se compara como texto.",
                    e.ruta
                ),
                Nivel::Info,
            );
            return;
        }
        let actual = String::from_utf8_lossy(&bytes).into_owned();
        // Un renombre (en el stage) se compara contra el nombre anterior en HEAD.
        if let Some(origen) = e.origen.clone() {
            let viejo = repo.contenido(&origen, false).unwrap_or_default();
            let nuevo = repo.contenido(&e.ruta, true).unwrap_or(actual);
            let filas = jonesy_core::diff::comparar(&viejo, &nuevo, 3);
            if filas.is_empty() {
                self.avisar(
                    format!("{origen} → {}: sólo cambió el nombre.", e.ruta),
                    Nivel::Info,
                );
            } else {
                self.abrir_diff(format!("{} (renombrado de {origen})", e.ruta), filas);
            }
            return;
        }
        // Sólo en el stage: HEAD contra el índice. Con cambios sin stage: índice contra el
        // árbol de trabajo (lo que mostraría git diff). Nuevo: vacío contra el archivo.
        let (titulo, viejo, nuevo) = if e.en_stage() && !e.sin_stage() {
            (
                format!("{} (HEAD → stage)", e.ruta),
                repo.contenido(&e.ruta, false).unwrap_or_default(),
                repo.contenido(&e.ruta, true).unwrap_or_default(),
            )
        } else {
            let base = repo
                .contenido(&e.ruta, true)
                .or_else(|| repo.contenido(&e.ruta, false))
                .unwrap_or_default();
            (format!("{} (stage → archivo)", e.ruta), base, actual)
        };
        let filas = jonesy_core::diff::comparar(&viejo, &nuevo, 3);
        if filas.is_empty() {
            self.avisar(
                format!("{}: sin diferencias de texto.", e.ruta),
                Nivel::Info,
            );
            return;
        }
        self.abrir_diff(titulo, filas);
    }

    /// Compara el buffer activo (aunque no esté guardado) con su versión en git (HEAD) o,
    /// fuera de un repositorio, con el archivo en disco.
    fn diff_archivo(&mut self) {
        let Some(ruta) = self.activo_ruta() else {
            self.avisar("Este buffer no tiene archivo.", Nivel::Aviso);
            return;
        };
        let actual = self.buf().buf.contenido();
        let en_git = self
            .repo
            .as_ref()
            .and_then(|r| r.relativa(&ruta).and_then(|rel| r.contenido(&rel, false)));
        let (titulo, viejo) = match en_git {
            Some(head) => (
                format!("{} (HEAD → editor)", self.proyecto.relativa(&ruta)),
                head,
            ),
            None => (
                format!("{} (disco → editor)", self.proyecto.relativa(&ruta)),
                std::fs::read_to_string(&ruta).unwrap_or_default(),
            ),
        };
        let filas = jonesy_core::diff::comparar(&viejo, &actual, 3);
        if filas.is_empty() {
            self.avisar("Sin cambios.", Nivel::Info);
            return;
        }
        self.abrir_diff(titulo, filas);
    }

    fn abrir_diff(&mut self, titulo: String, filas: Vec<jonesy_core::diff::FilaDiff>) {
        // El lenguaje sale del nombre del archivo en el título («main.c (HEAD → editor)»).
        let archivo = titulo
            .split(" (")
            .next()
            .unwrap_or(&titulo)
            .rsplit(" → ")
            .next()
            .unwrap_or(&titulo);
        let lenguaje = Lenguaje::de_ruta(Path::new(archivo.trim()));
        self.modal = Some(Modal::Diff {
            titulo,
            filas,
            desplazamiento: 0,
            lado_a_lado: self.preferir_lado_a_lado,
            desplazamiento_h: 0,
            lenguaje,
        });
    }

    /// Confirma deshacer el último commit; se niega si ya está en el remoto (push).
    fn pedir_deshacer_commit(&mut self) {
        let Some(e) = self.git_estado.clone() else {
            self.avisar("El proyecto no está en un repositorio git.", Nivel::Aviso);
            return;
        };
        if e.commit.is_none() {
            self.avisar("Todavía no hay commits.", Nivel::Aviso);
            return;
        }
        if e.upstream.is_some() && e.adelante == 0 {
            self.avisar(
                "El último commit ya está en el remoto (push): deshacerlo reescribiría la historia publicada. Hacé un commit nuevo con la corrección.",
                Nivel::Aviso,
            );
            return;
        }
        let resumen = self
            .repo
            .as_ref()
            .and_then(|r| r.log(1).ok())
            .and_then(|l| l.into_iter().next())
            .map(|c| format!("«{}» ({})", c.resumen, c.hash))
            .unwrap_or_default();
        self.modal = Some(Modal::Confirmar {
            titulo: "Deshacer el último commit".into(),
            mensaje: format!("¿Deshacer el commit {resumen}? Sus cambios quedan en el stage, listos para otro commit. (s/n)"),
            que: Confirmacion::DeshacerCommit,
        });
    }

    fn deshacer_commit(&mut self) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match repo.deshacer_ultimo_commit() {
            Ok(_) => {
                self.tel(TelemetryEvent::CommandExec {
                    command: "git reset --soft HEAD~1".into(),
                    exit_code: Some(0),
                });
                self.avisar(
                    "Commit deshecho: los cambios quedaron en el stage.",
                    Nivel::Ok,
                );
            }
            Err(e) => {
                let m = e.to_string();
                let m = if m.contains("HEAD~1") || m.contains("unknown revision") {
                    "Es el primer commit del repositorio: no hay uno anterior al cual volver."
                        .to_string()
                } else {
                    m
                };
                self.avisar(m, Nivel::Error);
            }
        }
        self.git.log = None;
        self.refrescar_git();
    }

    fn git_commit(&mut self, mensaje: &str) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        if mensaje.trim().is_empty() {
            self.avisar("El mensaje de commit está vacío.", Nivel::Aviso);
            return;
        }
        match repo.commit(mensaje) {
            Ok(salida) => {
                self.tel(TelemetryEvent::CommandExec {
                    command: "git commit".into(),
                    exit_code: Some(0),
                });
                self.avisar(
                    salida
                        .lines()
                        .next()
                        .unwrap_or("Commit creado.")
                        .to_string(),
                    Nivel::Ok,
                );
                self.modal = None;
                self.git.log = None;
            }
            Err(e) => {
                self.tel(TelemetryEvent::CommandExec {
                    command: "git commit".into(),
                    exit_code: Some(1),
                });
                self.avisar(e.to_string(), Nivel::Error);
            }
        }
        self.refrescar_git();
    }

    // ------------------------------------------------------------ construir

    fn activo_ruta(&self) -> Option<PathBuf> {
        self.buf().buf.ruta.clone()
    }

    pub fn compilar(&mut self, luego: Luego) {
        if self
            .procesos
            .values()
            .any(|t| matches!(t, Tarea::Compilar { .. }))
        {
            self.avisar("Ya hay una compilación en curso.", Nivel::Aviso);
            return;
        }
        self.guardar_todo();
        let activo = self.activo_ruta();
        let comando = if luego == Luego::Depurar
            && !self.proyecto.config.depurar.compilar.trim().is_empty()
        {
            Some(self.proyecto.config.depurar.compilar.clone())
        } else {
            self.proyecto.comando_compilar(activo.as_deref())
        };
        let Some(comando) = comando else {
            self.avisar(
                "No hay qué compilar: abrí un .c o configurá [compilar] en .jonesy.toml.",
                Nivel::Aviso,
            );
            return;
        };
        self.salida = vec![(format!("$ {comando}"), false)];
        self.titulo_salida = "compilación".into();
        self.salida_desplazamiento = 0;
        self.diag_compilador.clear();
        self.panel_inferior = true;
        self.pestana = Pestana::Salida;
        self.avisar(format!("Compilando: {comando}"), Nivel::Info);
        self.lanzar(&comando, Tarea::Compilar { luego });
    }

    fn ejecutar_en_deck(&mut self) {
        let activo = self.activo_ruta();
        let Some(cmd) = self.proyecto.comando_ejecutar(activo.as_deref()) else {
            self.avisar(
                "No hay qué ejecutar: abrí un .c o configurá [ejecutar] en .jonesy.toml.",
                Nivel::Aviso,
            );
            return;
        };
        self.correr_en_deck(cmd);
    }

    /// Corre un comando de shell en la pestaña Ejecución (con E/S interactiva) y deja la
    /// salida visible al terminar.
    pub fn correr_en_deck(&mut self, cmd: String) {
        self.guardar_todo();
        let (interprete, opcion) = proceso::interprete();
        // Con sh se agrega el código de salida al final; cmd sólo corre el comando.
        let envoltorio = if interprete == "sh" {
            format!("{cmd}\nc=$?\nprintf '\\n\\033[2m[proceso terminado con código %d]\\033[0m\\n' \"$c\"\n")
        } else {
            cmd.clone()
        };
        let (filas, cols) = self.tam_panel_inferior();
        let tx = self.tx.clone();
        match Deck::iniciar(
            Some(interprete),
            &[opcion.into(), envoltorio],
            &self.proyecto.raiz,
            filas,
            cols,
            move || {
                let _ = tx.send(Aviso::Despertar);
            },
        ) {
            Ok(mut d) => {
                d.titulo = cmd.clone();
                self.ejecucion = Some(d);
                self.panel_inferior = true;
                self.pestana = Pestana::Ejecucion;
                self.foco = Foco::Inferior;
                if self.modo() == Modo::Compacto {
                    self.vista = Vista::Terminal;
                }
                self.tel(TelemetryEvent::CommandExec {
                    command: cmd,
                    exit_code: None,
                });
            }
            Err(e) => self.avisar(format!("No se pudo crear la terminal: {e}"), Nivel::Error),
        }
    }

    pub fn tam_panel_inferior(&self) -> (u16, u16) {
        match self.areas.inferior {
            Some(r) => (r.height.saturating_sub(2).max(4), r.width.max(20)),
            None => (
                self.paneles.inferior.saturating_sub(2).max(4),
                self.tam.0.max(20),
            ),
        }
    }

    fn abrir_terminal(&mut self) {
        let vivo = self.terminal.as_mut().is_some_and(|d| d.vivo());
        if !vivo {
            let shell =
                Some(self.proyecto.config.interfaz.shell.clone()).filter(|s| !s.trim().is_empty());
            let (filas, cols) = self.tam_panel_inferior();
            let tx = self.tx.clone();
            match Deck::iniciar(
                shell.as_deref(),
                &[],
                &self.proyecto.raiz,
                filas,
                cols,
                move || {
                    let _ = tx.send(Aviso::Despertar);
                },
            ) {
                Ok(d) => self.terminal = Some(d),
                Err(e) => {
                    self.avisar(format!("No se pudo abrir la terminal: {e}"), Nivel::Error);
                    return;
                }
            }
        }
        self.panel_inferior = true;
        self.pestana = Pestana::Terminal;
        self.cambiar_foco(Foco::Inferior);
        if self.modo() == Modo::Compacto {
            self.vista = Vista::Terminal;
        }
    }

    fn cambiar_foco(&mut self, f: Foco) {
        if self.foco != f {
            self.tel(TelemetryEvent::FocusChange {
                panel: self.foco.nombre().into(),
                focused: false,
            });
            self.foco = f;
            self.tel(TelemetryEvent::FocusChange {
                panel: f.nombre().into(),
                focused: true,
            });
        }
    }

    // ------------------------------------------------------------ depurador

    fn depurar(&mut self) {
        if let Some(d) = &mut self.depuracion {
            if !d.gdb.corriendo {
                let _ = d.gdb.continuar();
            }
            return;
        }
        self.compilar(Luego::Depurar);
    }

    fn programa_a_depurar(&self) -> Option<PathBuf> {
        let activo = self.activo_ruta();
        self.proyecto
            .programa_a_depurar(activo.as_deref())
            .filter(|p| p.is_file())
    }

    fn iniciar_depuracion(&mut self) {
        let Some(programa) = self.programa_a_depurar() else {
            self.avisar(
                "No encontré el ejecutable: configurá [depurar] programa en .jonesy.toml.",
                Nivel::Error,
            );
            return;
        };
        let (filas, cols) = self.tam_panel_inferior();
        let cols_pty = (cols / 2).max(20);
        let tx = self.tx.clone();
        let pty = match PtyInferior::abrir(filas, cols_pty, move || {
            let _ = tx.send(Aviso::Despertar);
        }) {
            Ok(p) => Some(p),
            // Sin PTY separado (Windows) la E/S del programa aparece en la consola de GDB.
            Err(e) if e.kind() == std::io::ErrorKind::Unsupported => None,
            Err(e) => {
                self.avisar(
                    format!("No se pudo crear el PTY del programa: {e}"),
                    Nivel::Error,
                );
                return;
            }
        };
        let tx = self.tx.clone();
        let args = self.proyecto.config.depurar.argumentos.clone();
        let gdbinit = jonesy_core::entorno::detectar(&self.proyecto.config.interfaz.entorno)
            .and_then(|e| e.gdbinit);
        let mut gdb = match Gdb::lanzar_con(
            &programa,
            &args,
            &self.proyecto.raiz,
            pty.as_ref().map(|p| p.ruta()),
            gdbinit.as_deref(),
            move || {
                let _ = tx.send(Aviso::Despertar);
            },
        ) {
            Ok(g) => g,
            Err(e) => {
                self.avisar(format!("No se pudo lanzar gdb: {e}"), Nivel::Error);
                return;
            }
        };
        if self.breakpoints.is_empty() {
            let _ = gdb.enviar("-break-insert -t main", jonesy_core::gdb::Pedido::Otro);
        }
        for (archivo, linea) in &self.breakpoints {
            let _ = gdb.poner_breakpoint(archivo, *linea);
        }
        let _ = gdb.correr();
        self.tel(TelemetryEvent::CommandExec {
            command: format!("gdb {}", self.proyecto.relativa(&programa)),
            exit_code: None,
        });
        self.depuracion = Some(Depuracion {
            gdb,
            pty,
            marcos: vec![],
            variables: vec![],
            ubicacion: None,
            consola: vec![],
            entrada: String::new(),
            foco_consola: false,
            estado: "iniciando".into(),
            traza: None,
            paso_traza: None,
            tam_pty: (filas, cols_pty),
        });
        self.panel_inferior = true;
        self.pestana = Pestana::Depurador;
        if self.lateral.is_none() || self.lateral == Some(Lateral::Reglas) {
            self.lateral = Some(Lateral::Variables);
        }
        self.avisar(
            "Depurando: F5 continuar · F10 paso · F11 entrar · Shift+F11 salir · Shift+F5 detener",
            Nivel::Info,
        );
        self.lanzar_memtrace();
    }

    /// mem-trace en segundo plano para cat-scan (sólo programas de un archivo).
    /// mem-trace en segundo plano para cat-scan: un .c suelto o el main.c de un ejercicio
    /// con Makefile (con sus fuentes y los de sus bibliotecas).
    fn lanzar_memtrace(&mut self) {
        let Some(activo) = self.activo_ruta() else {
            return;
        };
        let dir = self.proyecto.dir_estado().join("memtrace");
        let Some(cmd) = jonesy_core::catscan::comando_memtrace(&self.proyecto, &activo, &dir)
        else {
            return;
        };
        let _ = std::fs::create_dir_all(&dir);
        self.lanzar(
            &cmd,
            Tarea::MemTrace {
                traza: dir.join("trace.json"),
            },
        );
    }

    fn detener_depuracion(&mut self) {
        if let Some(mut d) = self.depuracion.take() {
            d.gdb.terminar();
            self.avisar("Depuración detenida.", Nivel::Info);
        }
        if matches!(self.lateral, Some(Lateral::Variables | Lateral::CatScan)) {
            self.lateral = None;
        }
    }

    fn depurador_paso(&mut self, a: Accion) {
        let Some(d) = &mut self.depuracion else {
            if a == Accion::Paso {
                self.modal = Some(Modal::Menu { indice: 0, sel: 0 });
            }
            return;
        };
        if d.gdb.corriendo {
            if a == Accion::Detener {
                let _ = d.gdb.interrumpir();
            }
            return;
        }
        let _ = match a {
            Accion::Paso => d.gdb.paso(),
            Accion::Entrar => d.gdb.entrar(),
            Accion::SalirFuncion => d.gdb.salir(),
            _ => Ok(()),
        };
    }

    fn eventos_gdb(&mut self) {
        let Some(d) = &mut self.depuracion else {
            return;
        };
        let eventos = d.gdb.recibir();
        let mut ir: Option<(PathBuf, usize)> = None;
        let mut fin = false;
        let mut aviso: Option<(String, Nivel)> = None;
        for e in eventos {
            match e {
                EventoGdb::Detenido(p) => {
                    let marco = match &p {
                        Parada::Breakpoint(m) | Parada::Paso(m) | Parada::Otra(_, m) => {
                            Some(m.clone())
                        }
                        Parada::Senal {
                            nombre,
                            significado,
                            marco,
                        } => {
                            aviso = Some((
                                format!("El programa recibió {nombre} ({significado})."),
                                Nivel::Error,
                            ));
                            d.estado = format!("señal {nombre}");
                            Some(marco.clone())
                        }
                        Parada::Termino(c) => {
                            d.estado = match c {
                                Some(c) => format!("terminó (código {c})"),
                                None => "terminó por una señal".into(),
                            };
                            aviso = Some((
                                format!("El programa {}. F5 lo vuelve a correr.", d.estado),
                                Nivel::Info,
                            ));
                            d.ubicacion = None;
                            d.marcos.clear();
                            d.variables.clear();
                            None
                        }
                    };
                    if let Some(m) = marco {
                        if !matches!(p, Parada::Senal { .. }) {
                            d.estado = format!("detenido en {}()", m.funcion);
                        }
                        if let (Some(a), Some(l)) = (m.archivo.clone(), m.linea) {
                            d.ubicacion = Some((a.clone(), l));
                            ir = Some((a, l));
                        }
                        let _ = d.gdb.pedir_estado();
                    }
                }
                EventoGdb::Corriendo => d.estado = "corriendo".into(),
                EventoGdb::Marcos(m) => {
                    d.marcos = m;
                    if let (Some(t), Some(m0)) = (&d.traza, d.marcos.first()) {
                        if let Some(l) = m0.linea {
                            d.paso_traza = t.alinear(
                                &m0.funcion,
                                l,
                                d.marcos.len(),
                                d.paso_traza.unwrap_or(0),
                            );
                        }
                    }
                }
                EventoGdb::Variables(v) => d.variables = v,
                EventoGdb::Consola(t) => {
                    for l in t.lines() {
                        // Los tabs de GDB («4\tint *p») dejarían celdas sin pisar.
                        d.consola.push(expandir_tabs(l));
                    }
                    if d.consola.len() > 500 {
                        d.consola.drain(..d.consola.len() - 500);
                    }
                }
                EventoGdb::Error(m) => d.consola.push(format!("error: {m}")),
                EventoGdb::BreakpointCreado { .. } => {}
                EventoGdb::Salio => fin = true,
            }
        }
        if let Some((a, l)) = ir {
            if a.is_file() {
                self.ir_a(&a, l, 1);
            }
        }
        if let Some((m, n)) = aviso {
            self.avisar(m, n);
        }
        if fin {
            self.depuracion = None;
            self.avisar("gdb terminó.", Nivel::Info);
        }
    }

    fn alternar_breakpoint(&mut self, linea: usize) {
        let Some(ruta) = self.activo_ruta() else {
            return;
        };
        let clave = (ruta.clone(), linea + 1);
        let poner = !self.breakpoints.remove(&clave);
        if poner {
            self.breakpoints.insert(clave);
        }
        if let Some(d) = &mut self.depuracion {
            let _ = if poner {
                d.gdb.poner_breakpoint(&ruta, linea + 1)
            } else {
                d.gdb.quitar_breakpoint(&ruta, linea + 1)
            };
        }
    }

    // ------------------------------------------------------------ examen

    fn pedir_entrega(&mut self) {
        let Some(e) = &self.examen else {
            self.avisar(
                "La entrega sólo está disponible en modo examen (jonesy --exam).",
                Nivel::Aviso,
            );
            return;
        };
        if e.estado.entregado {
            let msg = format!(
                "Ya entregado: {}",
                e.estado.sha256.clone().unwrap_or_default()
            );
            self.avisar(msg, Nivel::Info);
            return;
        }
        let verificar = self
            .proyecto
            .comando_verificar()
            .unwrap_or_else(|| "(sin verificación configurada)".into());
        self.modal = Some(Modal::Confirmar {
            titulo: "Entregar examen".into(),
            mensaje: format!(
                "Se guardan todos los archivos, se corre «{verificar}», se sella la bitácora y se arma el paquete final. Después ya no vas a poder editar. ¿Confirmás la entrega? (s/n)"
            ),
            que: Confirmacion::Entregar,
        });
    }

    fn entregar(&mut self) {
        self.guardar_todo();
        if let Some(e) = &mut self.examen {
            e.entregando = true;
        }
        match self.proyecto.comando_verificar() {
            Some(cmd) => {
                self.salida = vec![(format!("$ {cmd}"), false)];
                self.titulo_salida = "verificación de la entrega".into();
                self.panel_inferior = true;
                self.pestana = Pestana::Salida;
                self.avisar("Verificando la entrega…", Nivel::Info);
                self.lanzar(&cmd, Tarea::Verificar);
            }
            None => self.finalizar_entrega(None),
        }
    }

    fn finalizar_entrega(&mut self, verificacion: Option<entrega::Verificacion>) {
        if let Some(v) = &verificacion {
            self.tel(TelemetryEvent::CommandExec {
                command: v.comando.clone(),
                exit_code: v.codigo,
            });
        }
        self.tel(TelemetryEvent::SessionEnd {
            reason: "entrega".into(),
        });
        let log = self.proyecto.dir_estado().join("telemetry.log");
        if let Some(t) = self.telemetria.take() {
            t.cerrar();
        }
        let sello = jonesy_telemetry::verificar(&log).ok();
        let telemetria = sello.as_ref().map(|v| {
            (
                log.as_path(),
                Some(v.ultimo_hash.clone()),
                v.registros as usize,
            )
        });
        match entrega::empaquetar(
            &self.proyecto,
            telemetria,
            verificacion.clone(),
            jonesy_core::ahora_unix(),
        ) {
            Ok(p) => {
                let rel = self.proyecto.relativa(&p.ruta);
                if let Some(e) = &mut self.examen {
                    e.entregando = false;
                    e.estado.entregado = true;
                    e.estado.paquete = Some(rel.clone());
                    e.estado.sha256 = Some(p.sha256.clone());
                    let _ = e.estado.guardar(&self.proyecto.dir_estado());
                }
                for b in &mut self.buffers {
                    b.buf.solo_lectura = true;
                }
                let resultado = match &verificacion {
                    Some(v) if v.codigo == Some(0) => "✓ la verificación pasó".to_string(),
                    Some(v) => format!(
                        "✗ la verificación falló (código {})",
                        v.codigo.map(|c| c.to_string()).unwrap_or("?".into())
                    ),
                    None => "sin verificación configurada".to_string(),
                };
                let texto = format!(
                    "# Entrega registrada\n\n- **Resultado:** {resultado}\n- **Paquete:** `{rel}`\n- **SHA-256:** `{}`\n- **Archivos:** {}\n- **Bitácora:** {} registros, cadena {}\n\nGuardá el SHA-256: identifica tu entrega.\n",
                    p.sha256,
                    p.manifiesto.archivos.len(),
                    p.manifiesto.telemetria_registros,
                    if sello.as_ref().is_some_and(|s| s.integra()) { "íntegra" } else { "no verificada" },
                );
                let ancho = self.tam.0.saturating_sub(10).min(90);
                self.modal = Some(Modal::Visor {
                    titulo: "Entrega".into(),
                    lineas: markdown::renderizar(&texto, ancho, &self.sim).lineas,
                    desplazamiento: 0,
                });
            }
            Err(e) => {
                if let Some(ex) = &mut self.examen {
                    ex.entregando = false;
                }
                self.avisar(format!("No se pudo armar el paquete: {e}"), Nivel::Error);
            }
        }
    }

    /// Temporizadores del examen: autoguardado y avisos de tiempo.
    fn tic_examen(&mut self) {
        let Some(e) = &mut self.examen else { return };
        if e.estado.entregado {
            return;
        }
        let autoguardar = e.ultimo_autoguardado.elapsed() >= AUTOGUARDADO;
        let restante = e.estado.restante_s(jonesy_core::ahora_unix());
        let mut aviso = None;
        if let Some(r) = restante {
            for m in AVISOS_MIN {
                if r <= (m * 60) as i64 && r > 0 && e.avisos_dados.insert(m) {
                    aviso = Some((format!("Quedan {m} minutos de examen."), Nivel::Aviso));
                }
            }
            if r <= 0 && e.avisos_dados.insert(0) {
                aviso = Some((
                    "Se terminó el tiempo del examen: entregá con F12.".into(),
                    Nivel::Error,
                ));
            }
        }
        if autoguardar {
            e.ultimo_autoguardado = Instant::now();
            self.guardar_todo();
        }
        if let Some((m, n)) = aviso {
            self.avisar(m, n);
        }
    }

    // ------------------------------------------------------------ salida y señales

    /// Guarda (examen) o deja copias de recuperación, sella la telemetría y la sesión.
    pub fn finalizar(&mut self, razon: &str) {
        if self.examen.is_some() && !self.examen_entregado() {
            self.guardar_todo();
        } else {
            let dir = self.proyecto.dir_estado();
            for b in &self.buffers {
                if let (true, Some(ruta)) = (b.buf.modificado(), &b.buf.ruta) {
                    let destino =
                        jonesy_core::sesion::ruta_recuperacion(&dir, &self.proyecto.relativa(ruta));
                    if let Some(p) = destino.parent() {
                        let _ = std::fs::create_dir_all(p);
                    }
                    let _ = jonesy_core::buffer::guardar_atomico(
                        &destino,
                        b.buf.contenido().as_bytes(),
                    );
                }
            }
        }
        self.guardar_sesion();
        if let Some(d) = &mut self.depuracion {
            d.gdb.terminar();
        }
        if let Some(t) = self.telemetria.take() {
            t.registrar(TelemetryEvent::SessionEnd {
                reason: razon.into(),
            });
            t.cerrar();
        }
    }

    fn pedir_salida(&mut self) {
        let sin_guardar: Vec<String> = self
            .buffers
            .iter()
            .filter(|b| b.buf.modificado())
            .map(|b| b.buf.nombre())
            .collect();
        if sin_guardar.is_empty() || self.examen.is_some() {
            self.salir = true;
        } else {
            self.modal = Some(Modal::Confirmar {
                titulo: "Salir".into(),
                mensaje: format!(
                    "Hay cambios sin guardar en {}. ¿Salir sin guardar? (s/n)",
                    sin_guardar.join(", ")
                ),
                que: Confirmacion::Salir,
            });
        }
    }

    // ------------------------------------------------------------ procesos

    fn al_proceso(&mut self, s: Salida) {
        match s {
            Salida::Linea { id, texto, error } => {
                if matches!(
                    self.procesos.get(&id),
                    Some(Tarea::Compilar { .. } | Tarea::Verificar | Tarea::Objetivo { .. })
                ) {
                    self.salida.push((texto, error));
                }
            }
            Salida::Fin { id, codigo, salida } => {
                let Some(t) = self.procesos.remove(&id) else {
                    return;
                };
                self.fin_proceso(t, codigo, salida);
            }
        }
    }

    fn fin_proceso(&mut self, t: Tarea, codigo: Option<i32>, salida: String) {
        match t {
            Tarea::Compilar { luego } => {
                let comando = self
                    .salida
                    .first()
                    .map(|l| l.0.trim_start_matches("$ ").to_string())
                    .unwrap_or_default();
                self.tel(TelemetryEvent::CommandExec {
                    command: comando,
                    exit_code: codigo,
                });
                self.diag_compilador = diagnostico::parsear_gcc(&salida, &self.proyecto.raiz);
                let errores = self
                    .diag_compilador
                    .iter()
                    .filter(|d| d.severidad == Severidad::Error)
                    .count();
                let advertencias = self.diag_compilador.len() - errores;
                if codigo == Some(0) {
                    self.salida.push((
                        format!("[compilación correcta, {advertencias} advertencia(s)]"),
                        false,
                    ));
                    self.avisar(
                        format!("Compilación correcta ({advertencias} advertencias)."),
                        Nivel::Ok,
                    );
                    if advertencias > 0 && luego == Luego::Nada {
                        self.pestana = Pestana::Problemas;
                    }
                    match luego {
                        Luego::Ejecutar => self.ejecutar_en_deck(),
                        Luego::Depurar => self.iniciar_depuracion(),
                        Luego::Nada => {}
                    }
                } else {
                    self.salida.push((
                        format!("[la compilación falló: código {}]", codigo.unwrap_or(-1)),
                        true,
                    ));
                    if errores > 0 {
                        self.avisar(format!("La compilación falló: {errores} error(es). Enter en Problemas salta al error."), Nivel::Error);
                        self.pestana = Pestana::Problemas;
                        self.sel_problema = 0;
                    } else {
                        // Sin errores reconocibles (make, un comando que no existe...): la salida explica.
                        self.avisar(
                            "La compilación falló: mirá la pestaña Salida.",
                            Nivel::Error,
                        );
                        self.pestana = Pestana::Salida;
                    }
                }
            }
            Tarea::Integracion { nombre, archivo } => {
                self.fin_integracion(nombre, archivo, codigo, salida)
            }
            Tarea::AccionHerramienta {
                integracion,
                titulo,
                archivo,
                modo,
            } => self.fin_accion_herramienta(integracion, titulo, archivo, modo, codigo, salida),
            Tarea::Verificar => {
                let comando = self
                    .salida
                    .first()
                    .map(|l| l.0.trim_start_matches("$ ").to_string())
                    .unwrap_or_default();
                self.salida.push((
                    format!("[verificación terminada: código {}]", codigo.unwrap_or(-1)),
                    codigo != Some(0),
                ));
                let v = entrega::Verificacion {
                    comando,
                    codigo,
                    salida: entrega::cola(&salida, 40),
                };
                self.finalizar_entrega(Some(v));
            }
            Tarea::Explicar {
                codigo: regla,
                enlace,
            } => {
                let ancho = self.tam.0.saturating_sub(8).clamp(40, 100);
                let mut lineas = if codigo == Some(0) && !salida.trim().is_empty() {
                    crate::vt::ansi_a_lineas(&salida, ancho)
                } else {
                    vec![Line::from(format!("No hay una explicación para {regla}."))]
                };
                if let Some(e) = enlace {
                    lineas.insert(0, Line::raw(""));
                    lineas.insert(
                        0,
                        Line::styled(
                            format!("Apunte: {e}"),
                            ratatui::style::Style::default().fg(tema::NOTA),
                        ),
                    );
                }
                self.modal = Some(Modal::Visor {
                    titulo: format!("Regla {regla}"),
                    lineas,
                    desplazamiento: 0,
                });
            }
            Tarea::Formatear {
                archivo,
                temporal,
                version,
            } => {
                self.fin_formateo(archivo, temporal, version, codigo, salida);
            }
            Tarea::Objetivo { comando } => {
                self.cargar_doc();
                self.tel(TelemetryEvent::CommandExec {
                    command: comando.clone(),
                    exit_code: codigo,
                });
                self.diag_compilador = diagnostico::parsear_gcc(&salida, &self.proyecto.raiz);
                if codigo == Some(0) {
                    self.salida
                        .push((format!("[«{comando}» terminó bien]"), false));
                    self.avisar(format!("«{comando}» terminó bien."), Nivel::Ok);
                } else {
                    let c = codigo.map(|c| c.to_string()).unwrap_or_else(|| "?".into());
                    self.salida
                        .push((format!("[«{comando}» falló: código {c}]"), true));
                    self.avisar(format!("«{comando}» falló (código {c})."), Nivel::Error);
                    if self
                        .diag_compilador
                        .iter()
                        .any(|d| d.severidad == Severidad::Error)
                    {
                        self.pestana = Pestana::Problemas;
                        self.sel_problema = 0;
                    }
                }
            }
            Tarea::MemTrace { traza } => {
                if codigo == Some(0) {
                    if let (Some(d), Ok(t)) = (&mut self.depuracion, Traza::cargar(&traza)) {
                        if let Some(m0) = d.marcos.first() {
                            if let Some(l) = m0.linea {
                                d.paso_traza = t.alinear(&m0.funcion, l, d.marcos.len(), 0);
                            }
                        }
                        d.traza = Some(t);
                        self.avisar("cat-scan listo: Alt+M muestra la memoria.", Nivel::Ok);
                    }
                }
            }
        }
    }

    fn al_plugin(&mut self, r: Respuesta) {
        match r {
            Respuesta::Diagnosticos { archivo, resultado } => match resultado {
                Ok(d) => {
                    let nativos = self.registro.diagnosticar(
                        &archivo,
                        &std::fs::read_to_string(&archivo).unwrap_or_default(),
                    );
                    self.diag_plugins
                        .insert(archivo, nativos.into_iter().chain(d).collect());
                }
                Err(jonesy_plugins::ErrorPlugin::SinPlugins) => {}
                Err(e) => self.avisar(format!("Plugins: {e}"), Nivel::Aviso),
            },
            Respuesta::Comandos(Ok(c)) => self.comandos_plugins = c,
            Respuesta::Comandos(Err(_)) => {}
            Respuesta::Ejecutado { comando, resultado } => match resultado {
                Ok(t) => {
                    let ancho = self.tam.0.saturating_sub(8).clamp(40, 100);
                    self.modal = Some(Modal::Visor {
                        titulo: comando.titulo(),
                        lineas: markdown::renderizar(&t, ancho, &self.sim).lineas,
                        desplazamiento: 0,
                    });
                }
                Err(e) => self.avisar(format!("{}: {e}", comando.titulo()), Nivel::Error),
            },
        }
    }

    /// F1: explica el código del diagnóstico elegido (en Problemas, en el panel de reglas o
    /// en la línea del cursor) con el comando `explicar` de la herramienta que lo informó.
    fn explicar_regla(&mut self) {
        let d: Option<Diagnostico> = match self.foco {
            Foco::Inferior if self.pestana == Pestana::Problemas => self
                .diagnosticos()
                .get(self.sel_problema)
                .map(|d| (*d).clone()),
            Foco::Lateral if self.lateral == Some(Lateral::Reglas) => self
                .reglas_agrupadas()
                .into_iter()
                .flat_map(|(_, v)| v)
                .nth(self.sel_regla)
                .cloned(),
            _ => self.activo_ruta().and_then(|r| {
                let (l, _) = self.buf().buf.linea_columna();
                self.diagnosticos_linea(&r, l)
                    .into_iter()
                    .find(|d| d.codigo.is_some())
                    .cloned()
            }),
        };
        let Some(d) = d.filter(|d| d.codigo.is_some()) else {
            self.abrir_paleta();
            return;
        };
        let codigo = d.codigo.clone().unwrap_or_default();
        let herramienta = d.herramienta.clone().unwrap_or_else(|| "ripley".into());
        let plantilla = self
            .integraciones
            .iter()
            .find(|x| x.nombre == herramienta)
            .map(|x| x.explicar.clone())
            .unwrap_or_default();
        if plantilla.is_empty() {
            // Sin comando para explicar: el mensaje, la sugerencia y el apunte.
            let mut texto = format!("# {herramienta}: {codigo}\n\n{}\n", d.mensaje);
            if let Some(s) = &d.sugerencia {
                texto.push_str(&format!("\n**Sugerencia:** {s}\n"));
            }
            if let Some(e) = &d.enlace {
                texto.push_str(&format!("\n**Apunte:** {e}\n"));
                if self.proyecto.config.interfaz.osc52 {
                    sistema::osc52(e);
                }
            }
            let ancho = self.tam.0.saturating_sub(10).clamp(40, 100);
            self.modal = Some(Modal::Visor {
                titulo: codigo,
                lineas: markdown::renderizar(&texto, ancho, &self.sim).lineas,
                desplazamiento: 0,
            });
            return;
        }
        let cmd = jonesy_core::integraciones::expandir(
            &plantilla,
            &self.proyecto.raiz,
            None,
            None,
            Some(&codigo),
        );
        self.lanzar(
            &cmd,
            Tarea::Explicar {
                codigo,
                enlace: d.enlace.clone(),
            },
        );
    }

    /// Hallazgos de las herramientas agrupados por la categoría común del ecosistema
    /// (memoria, punteros, estilo…; panel de reglas, RF-3.4).
    pub fn reglas_agrupadas(&self) -> Vec<(String, Vec<&Diagnostico>)> {
        let mut grupos: BTreeMap<String, Vec<&Diagnostico>> = BTreeMap::new();
        for d in self.diagnosticos() {
            if d.origen == Origen::Compilador {
                continue;
            }
            let cat = match (&d.categoria, &d.codigo) {
                (Some(c), _) => jonesy_core::integraciones::titulo_categoria(c).to_string(),
                (None, Some(c)) => diagnostico::categoria_regla(c).to_string(),
                _ => "Extensiones".to_string(),
            };
            grupos.entry(cat).or_default().push(d);
        }
        grupos.into_iter().collect()
    }

    // ------------------------------------------------------------ recuperación

    /// Si quedaron copias de un corte (SIGHUP fuera del examen) distintas de los archivos,
    /// pregunta si restaurarlas (RF-6.3).
    fn ofrecer_recuperacion(&mut self) {
        if self.examen.is_some() {
            return;
        }
        self.recuperables = jonesy_core::sesion::copias_recuperacion(
            &self.proyecto.dir_estado(),
            &self.proyecto.raiz,
        );
        if self.recuperables.is_empty() {
            return;
        }
        let nombres: Vec<String> = self
            .recuperables
            .iter()
            .map(|(a, _)| self.proyecto.relativa(a))
            .collect();
        self.modal = Some(Modal::Confirmar {
            titulo: "Recuperar cambios".into(),
            mensaje: format!(
                "La sesión anterior se cortó con cambios sin guardar en {}. ¿Recuperarlos? (s: se abren con esos cambios, sin guardar · n: se descartan)",
                nombres.join(", ")
            ),
            que: Confirmacion::Recuperar,
        });
    }

    /// Restaura (o descarta) las copias de recuperación y las borra.
    pub fn recuperar(&mut self, restaurar: bool) {
        for (archivo, copia) in std::mem::take(&mut self.recuperables) {
            if restaurar {
                if let (Ok(texto), Some(i)) = (
                    std::fs::read_to_string(&copia),
                    self.abrir_archivo(&archivo, true),
                ) {
                    self.activo = i;
                    self.editar(|b| b.reemplazar_todo(&texto));
                }
            }
            let _ = std::fs::remove_file(&copia);
        }
        if restaurar {
            self.avisar(
                "Cambios recuperados: revisalos y guardá con Ctrl+S.",
                Nivel::Ok,
            );
        }
    }

    // ------------------------------------------------------------ cambio de proyecto

    /// Cierra el proyecto actual (guarda sesión, sella la telemetría, termina procesos) y
    /// abre `raiz` con las mismas opciones de arranque.
    pub fn cambiar_proyecto(&mut self, raiz: PathBuf) {
        if self.examen.is_some() {
            self.avisar(
                "En modo examen no se puede cambiar de proyecto.",
                Nivel::Aviso,
            );
            return;
        }
        let raiz = jonesy_core::canonica(&raiz);
        if !raiz.is_dir() {
            self.avisar(
                format!("No existe el directorio {}", raiz.display()),
                Nivel::Error,
            );
            return;
        }
        let sin_guardar = self.buffers.iter().filter(|b| b.buf.modificado()).count();
        self.finalizar("cambio de proyecto");
        let op = Opciones {
            raiz: raiz.clone(),
            archivos: vec![],
            ..self.opciones.clone()
        };
        let tam = self.tam;
        *self = App::nueva(op, self.tx.clone());
        self.tam = tam;
        jonesy_core::sesion::recientes::registrar(&raiz);
        let aviso = if sin_guardar > 0 {
            format!("Proyecto {} abierto. Los cambios sin guardar quedaron en .jonesy/recuperacion del anterior.", raiz.display())
        } else {
            format!("Proyecto {} abierto.", raiz.display())
        };
        self.avisar(aviso, Nivel::Ok);
    }

    /// Autoformato del buffer activo: se escribe una copia junto al archivo (para que el
    /// formateador encuentre su .clang-format), se formatea en segundo plano y el resultado
    /// se aplica como una edición (se puede deshacer con Ctrl+Z).
    fn formatear(&mut self) {
        let b = &self.buf().buf;
        let Some(ruta) = b.ruta.clone() else {
            self.avisar("Guardá el archivo antes de formatearlo.", Nivel::Aviso);
            return;
        };
        if b.solo_lectura {
            self.avisar("Archivo de solo lectura.", Nivel::Aviso);
            return;
        }
        let configurado = !self.proyecto.config.formato.comando.trim().is_empty();
        if b.lenguaje != Lenguaje::C && !configurado {
            self.avisar(
                "El autoformato es para archivos .c y .h (o configurá [formato] comando).",
                Nivel::Aviso,
            );
            return;
        }
        if self
            .procesos
            .values()
            .any(|t| matches!(t, Tarea::Formatear { .. }))
        {
            return;
        }
        let nombre = ruta
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let temporal = ruta.with_file_name(format!(".jonesy-fmt-{nombre}"));
        if let Err(e) = std::fs::write(&temporal, b.contenido()) {
            self.avisar(
                format!("No se pudo preparar el formateo: {e}"),
                Nivel::Error,
            );
            return;
        }
        let version = b.version;
        let Some(cmd) = self
            .proyecto
            .comando_formato(&self.proyecto.relativa(&temporal))
        else {
            let _ = std::fs::remove_file(&temporal);
            self.avisar(
                "No encontré gaff ni clang-format: instalalos o configurá [formato] comando.",
                Nivel::Aviso,
            );
            return;
        };
        self.lanzar(
            &cmd,
            Tarea::Formatear {
                archivo: ruta,
                temporal,
                version,
            },
        );
    }

    fn fin_formateo(
        &mut self,
        archivo: PathBuf,
        temporal: PathBuf,
        version: u64,
        codigo: Option<i32>,
        salida: String,
    ) {
        let formateado = std::fs::read_to_string(&temporal);
        let _ = std::fs::remove_file(&temporal);
        if codigo != Some(0) {
            let detalle = salida
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("")
                .to_string();
            self.avisar(format!("El formateador falló: {detalle}"), Nivel::Error);
            return;
        }
        let Ok(nuevo) = formateado else { return };
        let Some(i) = self
            .buffers
            .iter()
            .position(|b| b.buf.ruta.as_deref() == Some(archivo.as_path()))
        else {
            return;
        };
        if self.buffers[i].buf.version != version {
            self.avisar(
                "El archivo cambió mientras se formateaba: probá de nuevo.",
                Nivel::Aviso,
            );
            return;
        }
        if self.buffers[i].buf.contenido() == nuevo {
            self.avisar("El archivo ya estaba formateado.", Nivel::Info);
            return;
        }
        let anterior = self.activo;
        self.activo = i;
        let (l, c) = self.buf().buf.linea_columna();
        self.tel(TelemetryEvent::CommandExec {
            command: "autoformato".into(),
            exit_code: Some(0),
        });
        self.editar(|b| {
            let d = b.reemplazar_todo(&nuevo);
            let pos = b.linea_columna_a_pos(l, c);
            b.mover_a(pos, false);
            d
        });
        self.activo = anterior;
        self.avisar("Archivo formateado (Ctrl+Z lo deshace).", Nivel::Ok);
    }

    /// `git clone` en el deck (para que funcionen credenciales y SSH); al terminar se
    /// ofrece abrir el repositorio.
    fn clonar(&mut self, url: &str, destino: &str) {
        let url = url.trim();
        if url.is_empty() {
            self.avisar("Falta la URL del repositorio.", Nivel::Aviso);
            return;
        }
        let Some(base) = expandir_ruta(destino, &self.proyecto.raiz) else {
            self.avisar("Falta la carpeta de destino.", Nivel::Aviso);
            return;
        };
        let carpeta = base.join(nombre_de_repo(url));
        if carpeta.exists() {
            self.avisar(format!("Ya existe {}", carpeta.display()), Nivel::Aviso);
            return;
        }
        let q = jonesy_core::proyecto::shell_quote;
        let cmd = format!("git clone {} {}", q(url), q(&carpeta.to_string_lossy()));
        self.clonando = Some(carpeta);
        self.correr_en_deck(cmd);
    }

    // ------------------------------------------------------------ documentación

    fn ruta_api(&self) -> PathBuf {
        jonesy_core::documentacion::DocProyecto::ruta_api(
            &self.proyecto.raiz,
            &self.proyecto.config.documentacion.dir,
        )
    }

    /// Busca y carga `doc/api.json` (generado con make doc) y calcula su vigencia.
    pub fn cargar_doc(&mut self) {
        use jonesy_core::documentacion::{DocProyecto, EstadoDoc};
        let ruta = self.ruta_api();
        self.comando_doc = self.calcular_comando_doc();
        self.doc_modificado = std::fs::metadata(&ruta).and_then(|m| m.modified()).ok();
        match DocProyecto::cargar(&ruta) {
            Ok(d) => {
                self.doc = Some(d);
                self.reevaluar_doc();
            }
            Err(e) => {
                self.doc = None;
                self.estado_doc = if ruta.exists() {
                    EstadoDoc::Invalida(e)
                } else {
                    EstadoDoc::Ausente
                };
            }
        }
    }

    pub fn reevaluar_doc(&mut self) {
        if let Some(d) = &self.doc {
            self.estado_doc = d.estado(&self.proyecto.raiz, &self.proyecto.estructura);
        }
    }

    /// Comando que genera la documentación: el objetivo doc del Makefile raíz (o del único
    /// módulo que lo tenga) o, sin Makefile que lo defina, corbel directamente.
    pub fn comando_doc(&self) -> Option<String> {
        self.comando_doc.clone()
    }

    fn calcular_comando_doc(&self) -> Option<String> {
        let e = &self.proyecto.estructura;
        if let Some(r) = e.raiz().filter(|r| r.tiene("doc")) {
            return Some(r.comando("doc"));
        }
        let con_doc: Vec<_> = e.modulos.iter().filter(|m| m.tiene("doc")).collect();
        if con_doc.len() == 1 {
            return Some(con_doc[0].comando("doc"));
        }
        jonesy_core::proyecto::en_path("corbel").then(|| "corbel proyecto .".to_string())
    }

    /// Hay que avisar que falta correr make doc (y hay cómo hacerlo).
    pub fn aviso_doc(&self) -> Option<&'static str> {
        use jonesy_core::documentacion::EstadoDoc;
        self.comando_doc()?;
        match self.estado_doc {
            EstadoDoc::AlDia => None,
            EstadoDoc::Ausente => Some("sin doc"),
            EstadoDoc::Desactualizada(_) => Some("doc vieja"),
            EstadoDoc::Invalida(_) => Some("doc inválida"),
        }
    }

    /// Cómo se nombra el comando en el botón: make doc o corbel (sin objetivo doc).
    pub fn etiqueta_comando_doc(&self) -> &'static str {
        match &self.comando_doc {
            Some(c) if c.starts_with("make") => "make doc",
            Some(_) => "corbel",
            None => "",
        }
    }

    fn generar_doc(&mut self) {
        match self.comando_doc() {
            Some(cmd) => self.correr_en_panel(cmd),
            None => self.avisar(
                "No hay objetivo doc en el Makefile ni corbel instalado: corré «./tp.sh sync» o instalá corbel.",
                Nivel::Aviso,
            ),
        }
    }

    /// Sondeo liviano: si `api.json` cambió (terminó un make doc), se recarga.
    fn sondear_doc(&mut self) {
        let actual = std::fs::metadata(self.ruta_api())
            .and_then(|m| m.modified())
            .ok();
        if actual != self.doc_modificado {
            self.cargar_doc();
        }
    }

    // ------------------------------------------------------------ barra izquierda

    /// Muestra la barra izquierda en la pestaña `p`, o la oculta si ya estaba en ella con foco.
    pub fn alternar_navegador(&mut self, p: PestanaNav) {
        if self.navegador == Some(p) && self.foco == Foco::Navegador {
            self.navegador = None;
            self.cambiar_foco(Foco::Editor);
            if self.vista == Vista::Navegador {
                self.vista = Vista::Codigo;
            }
            return;
        }
        self.navegador = Some(p);
        self.cambiar_foco(Foco::Navegador);
        if self.modo() == Modo::Compacto {
            self.vista = Vista::Navegador;
        }
        let activo = self.activo_ruta();
        match p {
            PestanaNav::Archivos => match &activo {
                Some(r) => self.explorador.revelar(r, &self.proyecto.raiz),
                None => self.explorador.reconstruir(&self.proyecto.raiz),
            },
            PestanaNav::Proyecto => {
                if let Some(r) = &activo {
                    self.vista_proyecto.revelar(&self.proyecto.estructura, r);
                }
            }
        }
    }

    /// Vuelve a leer Makefiles, library.json y tp.sh (y el árbol de archivos).
    pub fn refrescar_estructura(&mut self) {
        self.proyecto.estructura = jonesy_core::estructura::Estructura::descubrir(
            &self.proyecto.raiz,
            &self.proyecto.config.modulo,
        );
        self.explorador.reconstruir(&self.proyecto.raiz);
        self.cargar_doc();
    }

    /// Corre un objetivo de make: los interactivos (run) en el deck, el resto con la
    /// salida en el panel y los errores de gcc en Problemas.
    pub fn correr_objetivo(&mut self, modulo: usize, objetivo: &str) {
        let Some(m) = self.proyecto.estructura.modulos.get(modulo).cloned() else {
            return;
        };
        let comando = m.comando(objetivo);
        let interactivo = m
            .objetivos
            .iter()
            .find(|o| o.nombre == objetivo)
            .is_some_and(|o| o.interactivo());
        if interactivo {
            self.correr_en_deck(comando);
            return;
        }
        self.correr_en_panel(comando);
    }

    /// Corre un comando no interactivo con la salida en vivo en el panel y los errores de
    /// gcc y del enlazador en Problemas.
    pub fn correr_en_panel(&mut self, comando: String) {
        if self
            .procesos
            .values()
            .any(|t| matches!(t, Tarea::Objetivo { .. } | Tarea::Compilar { .. }))
        {
            self.avisar("Ya hay un make en curso.", Nivel::Aviso);
            return;
        }
        self.guardar_todo();
        self.salida = vec![(format!("$ {comando}"), false)];
        self.titulo_salida = comando.clone();
        self.salida_desplazamiento = 0;
        self.panel_inferior = true;
        self.pestana = Pestana::Salida;
        self.avisar(format!("Corriendo {comando}"), Nivel::Info);
        self.lanzar(
            &comando,
            Tarea::Objetivo {
                comando: comando.clone(),
            },
        );
    }

    /// Comando de tp.sh: si pide argumentos se preguntan antes; corre en el deck.
    pub fn correr_tp(&mut self, i: usize) {
        let Some(c) = self.proyecto.estructura.tp_sh.get(i).cloned() else {
            return;
        };
        let base = format!("./tp.sh {}", c.comando);
        if c.pide_argumentos() {
            self.modal = Some(Modal::Argumentos {
                titulo: format!("tp.sh {} — {}", c.uso, c.descripcion),
                comando: base,
                texto: String::new(),
            });
        } else {
            self.refrescar_estructura = true;
            self.correr_en_deck(base);
        }
    }

    /// Crea un archivo vacío (o una carpeta) y lo abre.
    fn crear_archivo(&mut self, rel: &str, carpeta: bool) {
        let rel = rel.trim().trim_start_matches('/');
        if rel.is_empty() {
            return;
        }
        let ruta = self.proyecto.raiz.join(rel);
        if !ruta.starts_with(&self.proyecto.raiz) || rel.split('/').any(|c| c == "..") {
            self.avisar(
                "La ruta tiene que quedar dentro del proyecto.",
                Nivel::Aviso,
            );
            return;
        }
        if ruta.exists() {
            if !carpeta && ruta.is_file() {
                self.abrir_archivo(&ruta, true);
            } else {
                self.avisar(format!("Ya existe {rel}."), Nivel::Aviso);
            }
            return;
        }
        let creado = if carpeta {
            std::fs::create_dir_all(&ruta)
        } else {
            ruta.parent()
                .map(std::fs::create_dir_all)
                .unwrap_or(Ok(()))
                .and_then(|_| std::fs::write(&ruta, ""))
        };
        if let Err(e) = creado {
            self.avisar(format!("No se pudo crear {rel}: {e}"), Nivel::Error);
            return;
        }
        if carpeta {
            self.explorador.expandidos.insert(ruta.clone());
            self.explorador.revelar(&ruta, &self.proyecto.raiz);
            self.avisar(format!("Carpeta {rel} creada."), Nivel::Ok);
        } else {
            self.abrir_archivo(&ruta, true);
            self.explorador.revelar(&ruta, &self.proyecto.raiz);
            self.avisar(format!("Archivo {rel} creado."), Nivel::Ok);
        }
        self.refrescar_git();
    }

    /// Pide el nombre nuevo de un archivo o carpeta (F2 en el explorador).
    fn pedir_renombrar(&mut self, ruta: &Path) {
        if self.examen.is_some() {
            self.avisar("En modo examen no se renombran archivos.", Nivel::Aviso);
            return;
        }
        let texto = self.proyecto.relativa(ruta);
        self.modal = Some(Modal::Renombrar {
            origen: ruta.to_path_buf(),
            texto,
        });
    }

    /// Renombra (o mueve) dentro del proyecto y actualiza los buffers abiertos.
    fn renombrar(&mut self, origen: &Path, nuevo: &str) {
        let nuevo = nuevo.trim().trim_start_matches('/');
        let destino = self.proyecto.raiz.join(nuevo);
        if nuevo.is_empty() || nuevo.split('/').any(|c| c == "..") || destino == origen {
            return;
        }
        if destino.exists() {
            self.avisar(format!("Ya existe {nuevo}."), Nivel::Aviso);
            return;
        }
        if let Some(p) = destino.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        if let Err(e) = std::fs::rename(origen, &destino) {
            self.avisar(format!("No se pudo renombrar: {e}"), Nivel::Error);
            return;
        }
        let destino = jonesy_core::canonica(&destino);
        for b in &mut self.buffers {
            if let Some(r) = b.buf.ruta.clone() {
                if let Ok(resto) = r.strip_prefix(origen) {
                    b.buf.ruta = Some(if resto.as_os_str().is_empty() {
                        destino.clone()
                    } else {
                        destino.join(resto)
                    });
                }
            }
        }
        self.explorador.revelar(&destino, &self.proyecto.raiz);
        self.refrescar_estructura();
        self.refrescar_git();
        self.avisar(format!("Renombrado a {nuevo}."), Nivel::Ok);
    }

    /// Pide confirmación para borrar (Supr en el explorador).
    fn pedir_borrar(&mut self, ruta: &Path) {
        if self.examen.is_some() {
            self.avisar("En modo examen no se borran archivos.", Nivel::Aviso);
            return;
        }
        let rel = self.proyecto.relativa(ruta);
        let que = if ruta.is_dir() {
            let n = std::fs::read_dir(ruta).map(|i| i.count()).unwrap_or(0);
            format!("la carpeta {rel} y todo su contenido ({n} elemento(s) adentro)")
        } else {
            format!("el archivo {rel}")
        };
        self.borrar_pendiente = Some(ruta.to_path_buf());
        self.modal = Some(Modal::Confirmar {
            titulo: "Borrar".into(),
            mensaje: format!("¿Borrar {que}? No se puede deshacer desde jonesy (sí desde git, si estaba commiteado). (s/n)"),
            que: Confirmacion::Borrar,
        });
    }

    fn borrar_ruta(&mut self) {
        let Some(ruta) = self.borrar_pendiente.take() else {
            return;
        };
        let r = if ruta.is_dir() {
            std::fs::remove_dir_all(&ruta)
        } else {
            std::fs::remove_file(&ruta)
        };
        if let Err(e) = r {
            self.avisar(format!("No se pudo borrar: {e}"), Nivel::Error);
            return;
        }
        // Las pestañas de lo borrado se cierran (sus cambios sin guardar se pierden con el archivo).
        self.buffers
            .retain(|b| b.buf.ruta.as_ref().is_none_or(|r| !r.starts_with(&ruta)));
        if self.buffers.is_empty() {
            self.buffers
                .push(BufferAbierto::nuevo(Buffer::desde_texto("")));
        }
        self.activo = self.activo.min(self.buffers.len() - 1);
        self.explorador.reconstruir(&self.proyecto.raiz);
        self.refrescar_estructura();
        self.refrescar_git();
        self.avisar(
            format!("Borrado {}.", self.proyecto.relativa(&ruta)),
            Nivel::Ok,
        );
    }

    /// Pide el nombre de un archivo o carpeta nueva, partiendo de `dir`.
    fn pedir_nuevo(&mut self, dir: &Path, carpeta: bool) {
        let mut texto = self.proyecto.relativa(dir);
        if !texto.is_empty() && !texto.ends_with('/') {
            texto.push('/');
        }
        self.modal = Some(Modal::NuevoArchivo { texto, carpeta });
    }

    fn tecla_navegador(&mut self, k: KeyEvent) {
        match k.code {
            KeyCode::Esc => {
                self.cambiar_foco(Foco::Editor);
                if self.vista == Vista::Navegador {
                    self.vista = Vista::Codigo;
                }
                return;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                let otra = match self.navegador {
                    Some(PestanaNav::Archivos) => PestanaNav::Proyecto,
                    _ => PestanaNav::Archivos,
                };
                self.navegador = Some(otra);
                if otra == PestanaNav::Archivos && self.explorador.filas.is_empty() {
                    self.explorador.reconstruir(&self.proyecto.raiz);
                }
                return;
            }
            _ => {}
        }
        let alto = self
            .zonas
            .lista_nav
            .map(|(r, _)| r.height as usize)
            .unwrap_or(10)
            .max(1);
        match self.navegador {
            Some(PestanaNav::Archivos) => {
                let raiz = self.proyecto.raiz.clone();
                let e = &mut self.explorador;
                let n = e.filas.len();
                match k.code {
                    KeyCode::Up => e.sel = e.sel.saturating_sub(1),
                    KeyCode::Down => e.sel = (e.sel + 1).min(n.saturating_sub(1)),
                    KeyCode::PageUp => e.sel = e.sel.saturating_sub(alto),
                    KeyCode::PageDown => e.sel = (e.sel + alto).min(n.saturating_sub(1)),
                    KeyCode::Home => e.sel = 0,
                    KeyCode::End => e.sel = n.saturating_sub(1),
                    KeyCode::Left => match e.seleccionada() {
                        Some(f) if f.es_dir && e.expandidos.contains(&f.ruta) => {
                            e.alternar(e.sel, &raiz)
                        }
                        _ => {
                            if let Some(p) = e.padre(e.sel) {
                                e.sel = p;
                            }
                        }
                    },
                    KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ') => {
                        match e.seleccionada().cloned() {
                            Some(f) if f.es_dir => {
                                if k.code != KeyCode::Right || !e.expandidos.contains(&f.ruta) {
                                    e.alternar(e.sel, &raiz);
                                }
                            }
                            Some(f) if k.code != KeyCode::Right => {
                                self.abrir_archivo(&f.ruta, true);
                                if self.modo() == Modo::Compacto {
                                    self.vista = Vista::Codigo;
                                }
                            }
                            _ => {}
                        }
                    }
                    KeyCode::Char('n') => {
                        let d = e.directorio_destino(&raiz);
                        self.pedir_nuevo(&d, false);
                    }
                    KeyCode::Char('m') => {
                        let d = e.directorio_destino(&raiz);
                        self.pedir_nuevo(&d, true);
                    }
                    KeyCode::F(2) => {
                        if let Some(f) = e.seleccionada().cloned() {
                            self.pedir_renombrar(&f.ruta);
                        }
                    }
                    KeyCode::Delete => {
                        if let Some(f) = e.seleccionada().cloned() {
                            self.pedir_borrar(&f.ruta);
                        }
                    }
                    KeyCode::Char('r') => {
                        self.refrescar_estructura();
                        self.refrescar_git();
                    }
                    _ => {}
                }
            }
            Some(PestanaNav::Proyecto) => {
                let filas = self.vista_proyecto.filas(&self.proyecto.estructura);
                let n = filas.len();
                let v = &mut self.vista_proyecto;
                v.sel = v.sel.min(n.saturating_sub(1));
                match k.code {
                    KeyCode::Up => v.sel = v.sel.saturating_sub(1),
                    KeyCode::Down => v.sel = (v.sel + 1).min(n.saturating_sub(1)),
                    KeyCode::PageUp => v.sel = v.sel.saturating_sub(alto),
                    KeyCode::PageDown => v.sel = (v.sel + alto).min(n.saturating_sub(1)),
                    KeyCode::Home => v.sel = 0,
                    KeyCode::End => v.sel = n.saturating_sub(1),
                    KeyCode::Char('r') => self.refrescar_estructura(),
                    KeyCode::Left => match filas.get(v.sel) {
                        Some(FilaProyecto::Objetivo(m, _)) => {
                            let m = *m;
                            if let Some(i) =
                                filas.iter().position(|f| *f == FilaProyecto::Modulo(m))
                            {
                                v.sel = i;
                            }
                        }
                        Some(f @ (FilaProyecto::Modulo(_) | FilaProyecto::TpSh)) => {
                            let abierto = match f {
                                FilaProyecto::Modulo(i) => v
                                    .desplegados
                                    .contains(&self.proyecto.estructura.modulos[*i].rel),
                                _ => v.tp_desplegado,
                            };
                            if abierto {
                                v.alternar(f, &self.proyecto.estructura);
                            }
                        }
                        _ => {}
                    },
                    KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ') => {
                        let fila = filas.get(v.sel).cloned();
                        self.activar_fila_proyecto(fila, k.code != KeyCode::Right);
                    }
                    _ => {}
                }
            }
            None => self.cambiar_foco(Foco::Editor),
        }
    }

    /// Enter sobre una fila de la vista de proyecto: despliega módulos y corre objetivos.
    fn activar_fila_proyecto(&mut self, fila: Option<FilaProyecto>, ejecutar: bool) {
        match fila {
            Some(f @ (FilaProyecto::Modulo(_) | FilaProyecto::TpSh)) => {
                self.vista_proyecto.alternar(&f, &self.proyecto.estructura)
            }
            Some(FilaProyecto::Objetivo(m, o)) if ejecutar => {
                let nombre = self.proyecto.estructura.modulos[m].objetivos[o]
                    .nombre
                    .clone();
                self.correr_objetivo(m, &nombre);
            }
            Some(FilaProyecto::ComandoTp(i)) if ejecutar => self.correr_tp(i),
            _ => {}
        }
    }

    // ------------------------------------------------------------ paleta y modales

    fn abrir_paleta(&mut self) {
        let mut opciones: Vec<OpcionPaleta> = Accion::todas()
            .iter()
            .filter(|a| self.examen.is_some() || **a != Accion::Entregar)
            .map(|a| {
                let i = a.info();
                OpcionPaleta {
                    titulo: i.titulo.into(),
                    atajo: i.atajo.into(),
                    que: QuePaleta::Accion(*a),
                }
            })
            .collect();
        for c in &self.comandos_plugins {
            opciones.push(OpcionPaleta {
                titulo: c.titulo(),
                atajo: "plugin".into(),
                que: QuePaleta::Plugin(c.clone()),
            });
        }
        for (i, m) in self.proyecto.estructura.modulos.iter().enumerate() {
            let nombre = if m.rel.is_empty() {
                "proyecto".to_string()
            } else {
                m.nombre.clone()
            };
            for o in &m.objetivos {
                opciones.push(OpcionPaleta {
                    titulo: format!("make: {nombre} › {} — {}", o.nombre, o.descripcion),
                    atajo: m.comando(&o.nombre),
                    que: QuePaleta::Objetivo(i, o.nombre.clone()),
                });
            }
        }
        for (nombre, n, titulo) in self.acciones_disponibles() {
            opciones.push(OpcionPaleta {
                titulo: format!("{nombre}: {titulo}"),
                atajo: "herramienta".into(),
                que: QuePaleta::Herramienta(nombre, n),
            });
        }
        for (i, c) in self.proyecto.estructura.tp_sh.iter().enumerate() {
            opciones.push(OpcionPaleta {
                titulo: format!("tp.sh: {} — {}", c.uso, c.descripcion),
                atajo: "tp.sh".into(),
                que: QuePaleta::Tp(i),
            });
        }
        if self.comandos_plugins.is_empty() {
            if let Some(p) = &self.plugins {
                p.pedir(PedidoPlugin::Comandos);
            }
        }
        self.modal = Some(Modal::Paleta {
            filtro: String::new(),
            sel: 0,
            opciones,
        });
    }

    fn abrir_selector(&mut self) {
        let archivos: Vec<String> = self
            .proyecto
            .archivos(2000)
            .iter()
            .map(|p| self.proyecto.relativa(p))
            .collect();
        self.modal = Some(Modal::Abrir {
            filtro: String::new(),
            sel: 0,
            archivos,
            marcados: BTreeSet::new(),
        });
    }

    pub fn accion(&mut self, a: Accion) {
        self.tel(TelemetryEvent::CommandExec {
            command: format!("accion:{a:?}"),
            exit_code: None,
        });
        match a {
            Accion::Abrir => self.abrir_selector(),
            Accion::Guardar => {
                if self.guardar(self.activo) {
                    self.avisar(format!("Guardado {}", self.buf().buf.nombre()), Nivel::Ok);
                }
            }
            Accion::GuardarTodo => {
                if self.guardar_todo() {
                    self.avisar("Todo guardado.", Nivel::Ok);
                }
            }
            Accion::CerrarBuffer => self.cerrar_buffer(false),
            Accion::SiguienteBuffer => {
                let n = self.buffers.len();
                self.cambiar_buffer((self.activo + 1) % n);
            }
            Accion::AnteriorBuffer => {
                let n = self.buffers.len();
                self.cambiar_buffer((self.activo + n - 1) % n);
            }
            Accion::Salir => self.pedir_salida(),
            Accion::Deshacer => self.editar(|b| b.deshacer()),
            Accion::Rehacer => self.editar(|b| b.rehacer()),
            Accion::Cortar => {
                if self.buf().buf.solo_lectura {
                    return;
                }
                let id = self.id_buffer(self.activo);
                if let Some((t, d)) = self.buf_mut().buf.cortar() {
                    self.tel(TelemetryEvent::TextDelta {
                        buffer_id: id,
                        offset: d.offset,
                        inserted: d.insertado,
                        deleted_len: d.borrados,
                    });
                    self.copiar_texto(t);
                }
            }
            Accion::Copiar => {
                if let Some(t) = self.buf().buf.texto_seleccionado() {
                    self.copiar_texto(t);
                }
            }
            Accion::Pegar => {
                let t = self.portapapeles.clone();
                if !t.is_empty() {
                    self.editar(|b| b.insertar(&t));
                }
            }
            Accion::SeleccionarTodo => self.buf_mut().buf.seleccionar_todo(),
            Accion::Buscar => {
                self.modal = Some(Modal::Buscar {
                    texto: self.busqueda.clone(),
                })
            }
            Accion::BuscarSiguiente => self.buscar_siguiente(),
            Accion::IrALinea => {
                self.modal = Some(Modal::IrALinea {
                    texto: String::new(),
                })
            }
            Accion::QuickDoc => {
                if self.modo() == Modo::Compacto {
                    self.vista = if self.vista == Vista::Doc {
                        Vista::Codigo
                    } else {
                        Vista::Doc
                    };
                } else {
                    self.quickdoc_visible = !self.quickdoc_visible;
                }
            }
            Accion::OrientacionQuickDoc => {
                self.quickdoc_posicion = match self.quickdoc_posicion {
                    PosicionDoc::Derecha => PosicionDoc::Inferior,
                    PosicionDoc::Inferior => PosicionDoc::Derecha,
                };
                self.quickdoc_visible = true;
                self.guardar_sesion();
            }
            Accion::Terminal => {
                if self.foco == Foco::Inferior && self.pestana == Pestana::Terminal {
                    self.cambiar_foco(Foco::Editor);
                    if self.modo() == Modo::Compacto {
                        self.vista = Vista::Codigo;
                    }
                } else {
                    self.abrir_terminal();
                }
            }
            Accion::Problemas => self.mostrar_pestana(Pestana::Problemas),
            Accion::SalidaCompilacion => self.mostrar_pestana(Pestana::Salida),
            Accion::PanelGit => self.alternar_lateral(Lateral::Git),
            Accion::PanelReglas => self.alternar_lateral(Lateral::Reglas),
            Accion::PreviewMarkdown => {
                if self.buf().buf.lenguaje == Lenguaje::Markdown {
                    self.preview_markdown = !self.preview_markdown;
                } else {
                    self.avisar("La vista previa es para archivos Markdown.", Nivel::Aviso);
                }
            }
            Accion::VistaConsigna => {
                if self.modo() == Modo::Compacto {
                    self.vista = Vista::Consigna;
                    self.foco = Foco::Consigna;
                } else if self.consigna.is_some() {
                    if self.foco == Foco::Consigna {
                        self.consigna_visible = !self.consigna_visible;
                        self.cambiar_foco(Foco::Editor);
                    } else {
                        self.consigna_visible = true;
                        self.cambiar_foco(Foco::Consigna);
                    }
                }
            }
            Accion::VistaCodigo => {
                self.vista = Vista::Codigo;
                self.cambiar_foco(Foco::Editor);
            }
            Accion::VistaTerminal => {
                if self.modo() == Modo::Compacto {
                    self.vista = Vista::Terminal;
                    self.panel_inferior = true;
                    self.cambiar_foco(Foco::Inferior);
                } else {
                    self.abrir_terminal();
                }
            }
            Accion::VistaDoc => {
                self.vista = Vista::Doc;
                self.quickdoc_visible = true;
            }
            Accion::Compilar => self.compilar(Luego::Nada),
            Accion::Ejecutar => self.compilar(Luego::Ejecutar),
            Accion::Depurar => self.depurar(),
            Accion::Paso | Accion::Entrar | Accion::SalirFuncion => self.depurador_paso(a),
            Accion::Detener => self.detener_depuracion(),
            Accion::AlternarBreakpoint => {
                let (l, _) = self.buf().buf.linea_columna();
                self.alternar_breakpoint(l);
            }
            Accion::CatScan => {
                if self.depuracion.is_none() {
                    self.avisar(
                        "cat-scan muestra la memoria durante la depuración (F5).",
                        Nivel::Aviso,
                    );
                    return;
                }
                self.lateral = Some(if self.lateral == Some(Lateral::CatScan) {
                    Lateral::Variables
                } else {
                    Lateral::CatScan
                });
                if self.depuracion.as_ref().is_some_and(|d| d.traza.is_none()) {
                    self.avisar("La traza de mem-trace todavía no está lista (o el proyecto tiene Makefile).", Nivel::Info);
                }
            }
            Accion::GitDeshacerCommit => self.pedir_deshacer_commit(),
            Accion::GitStageTodo | Accion::GitUnstageTodo | Accion::GitCommit | Accion::GitLog => {
                self.git_accion(a)
            }
            Accion::Entregar => self.pedir_entrega(),
            Accion::ExplicarRegla => self.explicar_regla(),
            Accion::Atajos => {
                let ancho = self.tam.0.saturating_sub(10).clamp(40, 90);
                self.modal = Some(Modal::Visor {
                    titulo: "Atajos".into(),
                    lineas: markdown::renderizar(accion::AYUDA_ATAJOS, ancho, &self.sim).lineas,
                    desplazamiento: 0,
                });
            }
            Accion::AcercaDe => {
                let texto = format!(
                    "# jonesy {}\n\nIDE de terminal para la cátedra de Programación I.\n\n- Proyecto: `{}`\n- Base quick-doc: {} funciones\n- Layout: modo {}\n- Telemetría: {}\n",
                    env!("CARGO_PKG_VERSION"),
                    self.proyecto.raiz.display(),
                    jonesy_doc::Base::embebida().cantidad_funciones(),
                    self.modo().nombre(),
                    if self.telemetria.is_some() { "activa (.jonesy/telemetry.log)" } else { "inactiva" },
                );
                self.modal = Some(Modal::Visor {
                    titulo: "Acerca de".into(),
                    lineas: markdown::renderizar(&texto, 70, &self.sim).lineas,
                    desplazamiento: 0,
                });
            }
            Accion::Paleta => self.abrir_paleta(),
            Accion::TamanosPaneles => self.modal = Some(Modal::Paneles { sel: 0 }),
            Accion::Explorador => self.alternar_navegador(PestanaNav::Archivos),
            Accion::Formatear => self.formatear(),
            Accion::GenerarDoc => self.generar_doc(),
            Accion::DiffArchivo => self.diff_archivo(),
            Accion::Herramientas => {
                let opciones: Vec<OpcionPaleta> = self
                    .acciones_disponibles()
                    .into_iter()
                    .map(|(nombre, n, titulo)| OpcionPaleta {
                        titulo,
                        atajo: nombre.clone(),
                        que: QuePaleta::Herramienta(nombre, n),
                    })
                    .collect();
                if opciones.is_empty() {
                    self.avisar("No hay herramientas para este archivo (Herramientas → Integraciones muestra cuáles están instaladas).", Nivel::Aviso);
                    return;
                }
                self.modal = Some(Modal::Paleta {
                    filtro: String::new(),
                    sel: 0,
                    opciones,
                });
            }
            Accion::Integraciones => self.modal = Some(Modal::Integraciones { sel: 0 }),
            Accion::GitClonar => {
                if self.examen.is_some() {
                    self.avisar("En modo examen no se puede clonar.", Nivel::Aviso);
                    return;
                }
                let destino = self
                    .proyecto
                    .raiz
                    .parent()
                    .unwrap_or(&self.proyecto.raiz)
                    .display()
                    .to_string();
                self.modal = Some(Modal::Clonar {
                    url: String::new(),
                    destino,
                    campo: 0,
                });
            }
            Accion::AbrirProyecto => {
                if self.examen.is_some() {
                    self.avisar(
                        "En modo examen no se puede cambiar de proyecto.",
                        Nivel::Aviso,
                    );
                    return;
                }
                let dir = self
                    .proyecto
                    .raiz
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| self.proyecto.raiz.clone());
                self.modal = Some(Modal::AbrirProyecto {
                    entradas: entradas_dir(&dir, true),
                    dir,
                    filtro: String::new(),
                    sel: 0,
                });
            }
            Accion::EstructuraProyecto => self.alternar_navegador(PestanaNav::Proyecto),
            Accion::NuevoArchivo => {
                let dir = if self.foco == Foco::Navegador
                    && self.navegador == Some(PestanaNav::Archivos)
                {
                    self.explorador.directorio_destino(&self.proyecto.raiz)
                } else {
                    self.activo_ruta()
                        .and_then(|r| r.parent().map(Path::to_path_buf))
                        .unwrap_or_else(|| self.proyecto.raiz.clone())
                };
                self.pedir_nuevo(&dir, false);
            }
        }
    }

    fn mostrar_pestana(&mut self, p: Pestana) {
        if self.panel_inferior && self.pestana == p && self.foco == Foco::Inferior {
            self.panel_inferior = false;
            self.cambiar_foco(Foco::Editor);
            return;
        }
        self.panel_inferior = true;
        self.pestana = p;
        self.cambiar_foco(Foco::Inferior);
        if self.modo() == Modo::Compacto {
            self.vista = Vista::Terminal;
        }
    }

    fn alternar_lateral(&mut self, l: Lateral) {
        if self.lateral == Some(l) {
            self.lateral = None;
            self.cambiar_foco(Foco::Editor);
        } else {
            self.lateral = Some(l);
            self.cambiar_foco(Foco::Lateral);
            if l == Lateral::Git {
                self.refrescar_git();
            }
        }
    }

    fn copiar_texto(&mut self, t: String) {
        if self.proyecto.config.interfaz.osc52 {
            sistema::osc52(&t);
        }
        self.portapapeles = t;
    }

    fn buscar_siguiente(&mut self) {
        if self.busqueda.is_empty() {
            self.modal = Some(Modal::Buscar {
                texto: String::new(),
            });
            return;
        }
        let patron = self.busqueda.clone();
        let b = &mut self.buf_mut().buf;
        let desde = b.seleccion().map(|(_, f)| f).unwrap_or(b.cursor);
        match b.buscar(&patron, desde) {
            Some(p) => {
                b.mover_a(p, false);
                b.mover_a(p + patron.chars().count(), true);
                centrar(b, 8);
            }
            None => self.avisar(format!("No se encontró «{patron}»."), Nivel::Aviso),
        }
    }

    // ------------------------------------------------------------ bucle

    pub fn bucle<B: ratatui::backend::Backend>(
        &mut self,
        term: &mut ratatui::Terminal<B>,
        rx: Receiver<Aviso>,
    ) -> anyhow::Result<()> {
        term.draw(|f| crate::dibujo::dibujar(f, self))?;
        let mut ultimo_dibujo = Instant::now();
        while !self.salir {
            let espera = self.proxima_espera();
            let primero = match rx.recv_timeout(espera) {
                Ok(a) => Some(a),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => break,
            };
            let mut solo_despertar = true;
            let hubo_evento = primero.is_some();
            if let Some(a) = primero {
                solo_despertar &= matches!(a, Aviso::Despertar);
                self.procesar(a);
                while let Ok(a) = rx.try_recv() {
                    solo_despertar &= matches!(a, Aviso::Despertar);
                    self.procesar(a);
                    if self.salir {
                        break;
                    }
                }
            }
            let antes = self.firma_visible();
            self.tic();
            if self.salir {
                break;
            }
            // Sin eventos y sin cambios visibles no se dibuja: ratatui reescribiría el
            // cursor igual y la terminal recibiría bytes en reposo (§10: 0 B/s).
            if !hubo_evento && self.firma_visible() == antes {
                continue;
            }
            // Coalescencia: la salida masiva del deck se dibuja a 60 FPS como máximo.
            if solo_despertar && hubo_evento {
                let desde = ultimo_dibujo.elapsed();
                if desde < Duration::from_millis(16) {
                    std::thread::sleep(Duration::from_millis(16) - desde);
                }
            }
            term.draw(|f| crate::dibujo::dibujar(f, self))?;
            ultimo_dibujo = Instant::now();
        }
        Ok(())
    }

    /// Resumen barato de lo que se ve en pantalla y puede cambiar sin eventos de entrada
    /// (temporizadores, sondeos, procesos): si no cambió, no se redibuja.
    pub fn firma_visible(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.mensaje
            .as_ref()
            .map(|(m, _, _)| m.clone())
            .hash(&mut h);
        self.git_estado
            .as_ref()
            .map(|e| (e.resumen(), e.entradas.len()))
            .hash(&mut h);
        format!("{:?}", self.estado_doc).hash(&mut h);
        self.modal.is_some().hash(&mut h);
        self.procesos.len().hash(&mut h);
        self.diagnosticos().len().hash(&mut h);
        self.depuracion
            .as_ref()
            .map(|d| (d.estado.clone(), d.ubicacion.clone(), d.consola.len()))
            .hash(&mut h);
        // El cronómetro del examen cambia cada segundo.
        if let Some(e) = self.examen.as_ref().filter(|e| !e.estado.entregado) {
            e.estado.restante_s(jonesy_core::ahora_unix()).hash(&mut h);
        }
        // Vista previa de Markdown pendiente de renderizar (debounce cumplido).
        if self.preview_markdown {
            let b = self.buf();
            (b.preview.as_ref().map(|p| p.0) != Some(b.buf.version)
                && b.ultimo_cambio.elapsed() >= DEBOUNCE_PREVIEW)
                .hash(&mut h);
        }
        h.finish()
    }

    fn proxima_espera(&self) -> Duration {
        let mut espera = Duration::from_secs(60);
        if self.examen.as_ref().is_some_and(|e| !e.estado.entregado) {
            espera = espera.min(Duration::from_secs(1));
        }
        if self.repo.is_some() || self.comando_doc().is_some() || self.doc.is_some() {
            espera = espera.min(SONDEO_GIT);
        }
        if let Some((_, _, t)) = &self.mensaje {
            espera = espera.min(
                Duration::from_secs(6)
                    .saturating_sub(t.elapsed())
                    .max(Duration::from_millis(50)),
            );
        }
        if self.preview_markdown {
            let b = self.buf();
            if b.preview.as_ref().is_none_or(|p| p.0 != b.buf.version) {
                espera = espera.min(
                    DEBOUNCE_PREVIEW
                        .saturating_sub(b.ultimo_cambio.elapsed())
                        .max(Duration::from_millis(10)),
                );
            }
        }
        if let Some(t) = self.integraciones_programadas.values().min() {
            espera = espera.min(
                t.saturating_duration_since(Instant::now())
                    .max(Duration::from_millis(10)),
            );
        }
        if !self.procesos.is_empty() {
            espera = espera.min(Duration::from_millis(500));
        }
        espera
    }

    /// Tareas periódicas tras cada lote de eventos.
    pub fn tic(&mut self) {
        if self.clonando.is_some() && !self.ejecucion.as_mut().is_some_and(|d| d.vivo()) {
            let destino = self.clonando.take().unwrap();
            if destino.join(".git").is_dir() {
                self.modal = Some(Modal::Confirmar {
                    titulo: "Repositorio clonado".into(),
                    mensaje: format!(
                        "Se clonó en {}. ¿Abrirlo como proyecto? (s/n)",
                        destino.display()
                    ),
                    que: Confirmacion::AbrirClonado,
                });
                self.proyecto_pendiente = Some(destino);
            } else {
                self.avisar(
                    "git clone no terminó bien: mirá la pestaña Ejecución.",
                    Nivel::Error,
                );
            }
        }
        // tp.sh add-lib/add-ex/sync cambian la estructura: se relee al terminar.
        if self.refrescar_estructura && !self.ejecucion.as_mut().is_some_and(|d| d.vivo()) {
            self.refrescar_estructura = false;
            self.refrescar_estructura();
        }
        let vencidas: Vec<(String, PathBuf)> = self
            .integraciones_programadas
            .iter()
            .filter(|(_, t)| Instant::now() >= **t)
            .map(|(k, _)| k.clone())
            .collect();
        for (nombre, archivo) in vencidas {
            self.integraciones_programadas
                .remove(&(nombre.clone(), archivo.clone()));
            self.lanzar_integracion(&nombre, &archivo);
        }
        self.tic_examen();
        self.sondear_git();
        self.eventos_gdb();
        if self.mensaje.as_ref().is_some_and(|(_, n, t)| {
            t.elapsed() > Duration::from_secs(if *n == Nivel::Error { 12 } else { 6 })
        }) {
            self.mensaje = None;
        }
        // CursorMove con un mínimo de 200 ms entre registros.
        if self.telemetria.is_some() {
            let (l, c) = self.buf().buf.linea_columna();
            let id = self.id_buffer(self.activo);
            if (id.clone(), l, c) != self.ultimo_cursor
                && self.ultimo_cursor_registrado.elapsed() >= Duration::from_millis(200)
            {
                self.ultimo_cursor = (id.clone(), l, c);
                self.ultimo_cursor_registrado = Instant::now();
                self.tel(TelemetryEvent::CursorMove {
                    buffer_id: id,
                    line: l + 1,
                    col: c + 1,
                });
            }
        }
    }

    pub fn procesar(&mut self, a: Aviso) {
        match a {
            Aviso::Terminal(ev) => self.evento(ev),
            Aviso::Despertar => {}
            Aviso::Proceso(s) => self.al_proceso(s),
            Aviso::Plugins(r) => self.al_plugin(r),
            Aviso::Senal(s) => {
                self.finalizar(sistema::nombre_senal(s));
                self.salir = true;
            }
        }
    }

    pub fn evento(&mut self, ev: Event) {
        match ev {
            Event::Key(k) if k.kind != KeyEventKind::Release => self.tecla(k),
            Event::Mouse(m) => self.mouse(m),
            Event::Paste(t) => self.pegar(t),
            Event::Resize(w, h) => self.tam = (w, h),
            Event::FocusLost => {
                self.tel(TelemetryEvent::FocusChange {
                    panel: "terminal".into(),
                    focused: false,
                });
                if self.examen.is_some() && !self.examen_entregado() {
                    self.guardar_todo();
                }
            }
            Event::FocusGained => self.tel(TelemetryEvent::FocusChange {
                panel: "terminal".into(),
                focused: true,
            }),
            _ => {}
        }
    }

    fn pegar(&mut self, t: String) {
        let t = t.replace("\r\n", "\n").replace('\r', "\n");
        match &mut self.modal {
            Some(
                Modal::Buscar { texto }
                | Modal::IrALinea { texto }
                | Modal::Argumentos { texto, .. }
                | Modal::NuevoArchivo { texto, .. }
                | Modal::Renombrar { texto, .. }
                | Modal::Clonar {
                    url: texto,
                    campo: 0,
                    ..
                }
                | Modal::Clonar { destino: texto, .. }
                | Modal::Commit { mensaje: texto },
            ) => {
                texto.push_str(&t);
                return;
            }
            Some(
                Modal::Paleta { filtro, .. }
                | Modal::Abrir { filtro, .. }
                | Modal::AbrirProyecto { filtro, .. },
            ) => {
                filtro.push_str(t.lines().next().unwrap_or(""));
                return;
            }
            Some(_) => return,
            None => {}
        }
        match self.foco {
            Foco::Inferior => {
                let datos = format!("\x1b[200~{t}\x1b[201~");
                match self.pestana {
                    Pestana::Terminal => {
                        if let Some(d) = &mut self.terminal {
                            let _ = d.escribir(datos.as_bytes());
                        }
                    }
                    Pestana::Ejecucion => {
                        if let Some(d) = &mut self.ejecucion {
                            let _ = d.escribir(t.as_bytes());
                        }
                    }
                    Pestana::Depurador => {
                        if let Some(d) = &mut self.depuracion {
                            if d.foco_consola {
                                d.entrada.push_str(t.lines().next().unwrap_or(""));
                            } else {
                                if let Some(p) = &mut d.pty {
                                    let _ = p.escribir(t.as_bytes());
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {
                if self.buf().buf.solo_lectura {
                    self.avisar("Archivo de solo lectura.", Nivel::Aviso);
                    return;
                }
                let id = self.id_buffer(self.activo);
                self.tel(TelemetryEvent::BracketedPaste {
                    buffer_id: id,
                    length: t.chars().count(),
                    content_hash: jonesy_core::sha256_hex(t.as_bytes()),
                });
                // Pegado literal: sin autoindentado (RF-7.2).
                self.editar(|b| b.insertar(&t));
                self.foco = Foco::Editor;
            }
        }
    }

    fn registrar_tecla(&self, k: &KeyEvent) {
        if self.telemetria.is_none() {
            return;
        }
        // Los caracteres sueltos ya quedan en TextDelta; se registran teclas de control.
        if matches!(k.code, KeyCode::Char(_))
            && !k
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return;
        }
        let nombre = match k.code {
            KeyCode::Char(c) => c.to_string(),
            KeyCode::F(n) => format!("F{n}"),
            otro => format!("{otro:?}"),
        };
        self.tel(TelemetryEvent::KeyPress {
            key: nombre,
            modifiers: k.modifiers.bits(),
        });
    }

    pub fn tecla(&mut self, k: KeyEvent) {
        self.registrar_tecla(&k);
        self.desplazado_por_mouse = false;
        if self.modal.is_some() {
            self.tecla_modal(k);
            return;
        }
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let alt = k.modifiers.contains(KeyModifiers::ALT);
        let shift = k.modifiers.contains(KeyModifiers::SHIFT);

        // Pestañas: Ctrl+Tab / Ctrl+Shift+Tab y Alt+1…9 (Alt+0 la última).
        match (k.code, ctrl, alt) {
            (KeyCode::Tab, true, _) => {
                self.accion(Accion::SiguienteBuffer);
                return;
            }
            (KeyCode::BackTab, true, _) => {
                self.accion(Accion::AnteriorBuffer);
                return;
            }
            (KeyCode::Char(c @ '0'..='9'), false, true) => {
                let n = self.buffers.len();
                let i = if c == '0' {
                    n - 1
                } else {
                    (c as usize - '1' as usize).min(n - 1)
                };
                self.cambiar_buffer(i);
                return;
            }
            _ => {}
        }
        // Teclas que siempre son globales (también con el foco en la terminal).
        let global = match (k.code, ctrl, alt, shift) {
            (KeyCode::Char('t'), false, true, _) => Some(Accion::Terminal),
            (KeyCode::F(5), true, _, _) => Some(Accion::Ejecutar),
            (KeyCode::F(5), _, _, true) => Some(Accion::Detener),
            (KeyCode::F(5), _, _, _) => Some(Accion::Depurar),
            (KeyCode::F(11), _, _, true) => Some(Accion::SalirFuncion),
            (KeyCode::F(11), _, _, _) => Some(Accion::Entrar),
            (KeyCode::F(10), _, _, _) => Some(Accion::Paso),
            (KeyCode::F(6), _, _, _) => Some(Accion::Compilar),
            (KeyCode::F(7), _, _, _) => Some(Accion::VistaConsigna),
            (KeyCode::F(8), _, _, _) => Some(Accion::VistaCodigo),
            (KeyCode::F(9), _, _, _) => Some(Accion::VistaTerminal),
            (KeyCode::F(12), _, _, _) => Some(Accion::Entregar),
            (KeyCode::Char('g'), false, true, _) => Some(Accion::PanelGit),
            (KeyCode::Char('e'), false, true, _) => Some(Accion::Explorador),
            (KeyCode::Char('h'), false, true, _) => Some(Accion::Herramientas),
            (KeyCode::Char('y'), false, true, _) => Some(Accion::EstructuraProyecto),
            (KeyCode::Char('n'), true, false, _) => Some(Accion::NuevoArchivo),
            (KeyCode::Char('F'), false, true, _) | (KeyCode::Char('f'), false, true, true) => {
                Some(Accion::Formatear)
            }
            (KeyCode::Char('D'), false, true, _) | (KeyCode::Char('d'), false, true, true) => {
                Some(Accion::DiffArchivo)
            }
            (KeyCode::Char('r'), false, true, _) => Some(Accion::PanelReglas),
            (KeyCode::Char('p'), false, true, _) => Some(Accion::Problemas),
            (KeyCode::Char('o'), false, true, _) => Some(Accion::SalidaCompilacion),
            (KeyCode::Char('m'), false, true, _) => Some(Accion::CatScan),
            (KeyCode::Char('d'), false, true, _) => Some(Accion::QuickDoc),
            (KeyCode::F(4), _, _, true) => Some(Accion::OrientacionQuickDoc),
            (KeyCode::F(4), _, _, _) => Some(Accion::QuickDoc),
            (KeyCode::Char('q'), true, _, _) => Some(Accion::Salir),
            (KeyCode::Char('k'), true, _, _) => Some(Accion::Paleta),
            _ => None,
        };
        if let Some(a) = global {
            // F10 sin depuración abre el menú.
            if a == Accion::Paso && self.depuracion.is_none() {
                self.modal = Some(Modal::Menu { indice: 0, sel: 0 });
                return;
            }
            self.accion(a);
            return;
        }
        match self.foco {
            Foco::Inferior => self.tecla_inferior(k),
            Foco::Lateral => self.tecla_lateral(k),
            Foco::Consigna => self.tecla_consigna(k),
            Foco::Navegador => self.tecla_navegador(k),
            Foco::Preview => self.tecla_preview(k),
            _ => self.tecla_editor(k),
        }
    }

    fn tecla_consigna(&mut self, k: KeyEvent) {
        let alto = self
            .areas
            .consigna
            .map(|r| r.height.saturating_sub(2) as usize)
            .unwrap_or(10)
            .max(1);
        let Some(c) = &mut self.consigna else {
            self.foco = Foco::Editor;
            return;
        };
        let doc = c.doc.as_ref().map(|(_, d)| d);
        match k.code {
            KeyCode::Up => c.desplazamiento = c.desplazamiento.saturating_sub(1),
            KeyCode::Down => c.desplazamiento += 1,
            KeyCode::PageUp => c.desplazamiento = c.desplazamiento.saturating_sub(alto),
            KeyCode::PageDown | KeyCode::Char(' ') => c.desplazamiento += alto,
            KeyCode::Home => c.desplazamiento = 0,
            // Tab / Shift+Tab recorren los enlaces; sin enlaces, Tab vuelve al editor.
            KeyCode::Tab | KeyCode::BackTab if doc.is_some_and(|d| !d.enlaces.is_empty()) => {
                let d = doc.unwrap();
                c.enlace = siguiente_enlace(d, c.enlace, k.code == KeyCode::Tab);
                if let Some(e) = c.enlace.and_then(|i| d.enlaces.get(i)) {
                    c.desplazamiento = a_la_vista(e.linea, c.desplazamiento, alto);
                }
            }
            KeyCode::Enter => {
                let destino = c
                    .enlace
                    .and_then(|i| doc.and_then(|d| d.enlaces.get(i)))
                    .map(|e| e.destino.clone());
                let base = c.ruta.parent().map(Path::to_path_buf).unwrap_or_default();
                if let Some(d) = destino {
                    if let Some(l) = d
                        .strip_prefix('#')
                        .and_then(|ancla| doc.and_then(|x| linea_de_ancla(x, ancla)))
                    {
                        c.desplazamiento = l;
                    } else {
                        self.abrir_enlace(&base, &d);
                    }
                }
            }
            KeyCode::Esc | KeyCode::Tab => self.cambiar_foco(Foco::Editor),
            _ => self.tecla_editor(k),
        }
    }

    /// Vista previa de Markdown con foco: desplazamiento y enlaces con el teclado.
    fn tecla_preview(&mut self, k: KeyEvent) {
        let alto = self
            .zonas
            .preview
            .map(|r| r.height as usize)
            .unwrap_or(10)
            .max(1);
        let ba = &mut self.buffers[self.activo];
        let Some((_, _, doc)) = &ba.preview else {
            self.foco = Foco::Editor;
            return;
        };
        match k.code {
            KeyCode::Up => ba.preview_desplazamiento = ba.preview_desplazamiento.saturating_sub(1),
            KeyCode::Down => ba.preview_desplazamiento += 1,
            KeyCode::PageUp => {
                ba.preview_desplazamiento = ba.preview_desplazamiento.saturating_sub(alto)
            }
            KeyCode::PageDown | KeyCode::Char(' ') => ba.preview_desplazamiento += alto,
            KeyCode::Home => ba.preview_desplazamiento = 0,
            KeyCode::Tab | KeyCode::BackTab if !doc.enlaces.is_empty() => {
                ba.preview_enlace =
                    siguiente_enlace(doc, ba.preview_enlace, k.code == KeyCode::Tab);
                if let Some(e) = ba.preview_enlace.and_then(|i| doc.enlaces.get(i)) {
                    ba.preview_desplazamiento =
                        a_la_vista(e.linea, ba.preview_desplazamiento, alto);
                }
            }
            KeyCode::Enter => {
                let destino = ba
                    .preview_enlace
                    .and_then(|i| doc.enlaces.get(i))
                    .map(|e| e.destino.clone());
                let ancla = destino
                    .as_deref()
                    .and_then(|d| d.strip_prefix('#'))
                    .and_then(|a| linea_de_ancla(doc, a));
                if let Some(l) = ancla {
                    ba.preview_desplazamiento = l;
                } else if let Some(d) = destino {
                    let base = ba
                        .buf
                        .ruta
                        .as_ref()
                        .and_then(|p| p.parent().map(Path::to_path_buf))
                        .unwrap_or_default();
                    self.abrir_enlace(&base, &d);
                }
            }
            _ => {
                self.cambiar_foco(Foco::Editor);
                if k.code != KeyCode::Esc {
                    self.tecla_editor(k);
                }
            }
        }
    }

    fn tecla_lateral(&mut self, k: KeyEvent) {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        if k.code == KeyCode::Esc || k.code == KeyCode::Tab {
            self.cambiar_foco(Foco::Editor);
            return;
        }
        match self.lateral {
            Some(Lateral::Git) => {
                let en_log = self.git.log.is_some();
                let total = if en_log {
                    self.git.log.as_ref().map(|l| l.len()).unwrap_or(0)
                } else {
                    self.git_estado
                        .as_ref()
                        .map(|e| e.entradas.len())
                        .unwrap_or(0)
                };
                let sel = if en_log {
                    &mut self.git.sel_log
                } else {
                    &mut self.git.sel
                };
                match k.code {
                    KeyCode::Up => *sel = sel.saturating_sub(1),
                    KeyCode::Down => *sel = (*sel + 1).min(total.saturating_sub(1)),
                    KeyCode::Char(' ') if !en_log => self.git_alternar_stage(),
                    KeyCode::Char('a') if !ctrl => self.git_accion(Accion::GitStageTodo),
                    KeyCode::Char('u') if !ctrl => self.git_accion(Accion::GitUnstageTodo),
                    KeyCode::Char('c') if !ctrl => self.git_accion(Accion::GitCommit),
                    KeyCode::Char('l') => self.git_accion(Accion::GitLog),
                    KeyCode::Char('z') => self.pedir_deshacer_commit(),
                    KeyCode::Char('m') if en_log => {
                        let h = self
                            .git
                            .log
                            .as_ref()
                            .and_then(|l| l.get(self.git.sel_log))
                            .map(|c| c.hash.clone());
                        self.git.marcado = if self.git.marcado == h { None } else { h };
                    }
                    KeyCode::Char('r') => self.refrescar_git(),
                    KeyCode::Enter => self.git_mostrar(),
                    _ => self.tecla_editor(k),
                }
            }
            Some(Lateral::Reglas) => {
                let total: usize = self.reglas_agrupadas().iter().map(|(_, v)| v.len()).sum();
                match k.code {
                    KeyCode::Up => self.sel_regla = self.sel_regla.saturating_sub(1),
                    KeyCode::Down => {
                        self.sel_regla = (self.sel_regla + 1).min(total.saturating_sub(1))
                    }
                    KeyCode::Enter => {
                        let d = self
                            .reglas_agrupadas()
                            .into_iter()
                            .flat_map(|(_, v)| v)
                            .nth(self.sel_regla)
                            .cloned();
                        if let Some(Diagnostico {
                            archivo: Some(a),
                            linea,
                            columna,
                            ..
                        }) = d
                        {
                            self.ir_a(&a, linea, columna);
                        }
                    }
                    KeyCode::F(1) => self.explicar_regla(),
                    _ => self.tecla_editor(k),
                }
            }
            _ => self.tecla_editor(k),
        }
    }

    fn tecla_inferior(&mut self, k: KeyEvent) {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        // Esc Esc devuelve el foco al editor (un Esc solo va al programa).
        if k.code == KeyCode::Esc {
            if self
                .esc_pendiente
                .is_some_and(|t| t.elapsed() < Duration::from_millis(600))
            {
                self.esc_pendiente = None;
                self.cambiar_foco(Foco::Editor);
                if self.modo() == Modo::Compacto {
                    self.vista = Vista::Codigo;
                }
                return;
            }
            self.esc_pendiente = Some(Instant::now());
            if matches!(self.pestana, Pestana::Problemas | Pestana::Salida) {
                self.cambiar_foco(Foco::Editor);
                return;
            }
        }
        match self.pestana {
            Pestana::Problemas => {
                let total = self.diagnosticos().len();
                match k.code {
                    KeyCode::Up => self.sel_problema = self.sel_problema.saturating_sub(1),
                    KeyCode::Down => {
                        self.sel_problema = (self.sel_problema + 1).min(total.saturating_sub(1))
                    }
                    KeyCode::Enter => self.saltar_a_problema(self.sel_problema),
                    KeyCode::F(1) => self.explicar_regla(),
                    KeyCode::Tab => self.cambiar_foco(Foco::Editor),
                    _ => {}
                }
            }
            Pestana::Salida => match k.code {
                KeyCode::Up => self.salida_desplazamiento += 1,
                KeyCode::Down => {
                    self.salida_desplazamiento = self.salida_desplazamiento.saturating_sub(1)
                }
                KeyCode::PageUp => self.salida_desplazamiento += 10,
                KeyCode::PageDown => {
                    self.salida_desplazamiento = self.salida_desplazamiento.saturating_sub(10)
                }
                KeyCode::Tab => self.cambiar_foco(Foco::Editor),
                _ => {}
            },
            Pestana::Terminal | Pestana::Ejecucion => {
                let deck = if self.pestana == Pestana::Terminal {
                    self.terminal.as_mut()
                } else {
                    self.ejecucion.as_mut()
                };
                let Some(d) = deck else { return };
                if k.modifiers.contains(KeyModifiers::SHIFT)
                    && matches!(k.code, KeyCode::PageUp | KeyCode::PageDown)
                {
                    let actual = d.con_pantalla(|s| s.scrollback());
                    let n = if k.code == KeyCode::PageUp {
                        actual + 10
                    } else {
                        actual.saturating_sub(10)
                    };
                    d.desplazar(n);
                    return;
                }
                d.desplazar(0);
                if let Some(bytes) = bytes_tecla(&k) {
                    let _ = d.escribir(&bytes);
                }
            }
            Pestana::Depurador => {
                let Some(d) = &mut self.depuracion else {
                    return;
                };
                if k.code == KeyCode::Tab {
                    d.foco_consola = !d.foco_consola;
                    return;
                }
                if d.foco_consola {
                    match k.code {
                        KeyCode::Enter => {
                            let cmd = std::mem::take(&mut d.entrada);
                            d.consola.push(format!("(gdb) {cmd}"));
                            let _ = d.gdb.consola(&cmd);
                            let _ = d.gdb.pedir_estado();
                        }
                        KeyCode::Backspace => {
                            d.entrada.pop();
                        }
                        KeyCode::Char(c) if !ctrl => d.entrada.push(c),
                        _ => {}
                    }
                } else if let Some(bytes) = bytes_tecla(&k) {
                    match &mut d.pty {
                        Some(p) => {
                            let _ = p.escribir(&bytes);
                        }
                        None => d.foco_consola = true,
                    }
                }
            }
        }
    }

    fn saltar_a_problema(&mut self, i: usize) {
        let d = self.diagnosticos().get(i).map(|d| (*d).clone());
        if let Some(Diagnostico {
            archivo: Some(a),
            linea,
            columna,
            ..
        }) = d
        {
            self.ir_a(&a, linea, columna);
        }
    }

    fn tecla_editor(&mut self, k: KeyEvent) {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let alt = k.modifiers.contains(KeyModifiers::ALT);
        let shift = k.modifiers.contains(KeyModifiers::SHIFT);
        if self.foco != Foco::Editor && self.foco != Foco::Consigna {
            self.foco = Foco::Editor;
        }
        if self.modo() == Modo::Compacto
            && self.vista != Vista::Codigo
            && self.vista != Vista::Consigna
        {
            if k.code == KeyCode::Esc {
                self.vista = Vista::Codigo;
            }
            return;
        }
        let alto = self
            .zonas
            .editor
            .map(|z| z.texto.height as isize)
            .unwrap_or(20)
            .max(2);
        let accion = match (k.code, ctrl, shift) {
            (KeyCode::Char('s'), true, true) => Some(Accion::GuardarTodo),
            (KeyCode::Char('s'), true, false) | (KeyCode::F(2), _, _) => Some(Accion::Guardar),
            (KeyCode::Char('z'), true, _) => Some(Accion::Deshacer),
            (KeyCode::Char('y'), true, _) => Some(Accion::Rehacer),
            (KeyCode::Char('x'), true, _) => Some(Accion::Cortar),
            (KeyCode::Char('c'), true, _) => Some(Accion::Copiar),
            (KeyCode::Char('v'), true, _) => Some(Accion::Pegar),
            (KeyCode::Char('a'), true, _) => Some(Accion::SeleccionarTodo),
            (KeyCode::Char('f'), true, _) => Some(Accion::Buscar),
            (KeyCode::F(3), _, true) => Some(Accion::BuscarSiguiente),
            (KeyCode::F(3), _, false) => Some(Accion::Buscar),
            (KeyCode::Char('g'), true, _) => Some(Accion::IrALinea),
            (KeyCode::Char('o'), true, _) => Some(Accion::Abrir),
            (KeyCode::Char('w'), true, _) => Some(Accion::CerrarBuffer),
            (KeyCode::Char('b'), true, _) => Some(Accion::AlternarBreakpoint),
            (KeyCode::Char('p'), true, _) => Some(Accion::PreviewMarkdown),
            (KeyCode::PageDown, true, _) => Some(Accion::SiguienteBuffer),
            (KeyCode::PageUp, true, _) => Some(Accion::AnteriorBuffer),
            (KeyCode::F(1), _, _) => Some(Accion::ExplicarRegla),
            _ => None,
        };
        if let Some(a) = accion {
            self.accion(a);
            return;
        }
        if alt {
            if let KeyCode::Char(c) = k.code {
                // Alt+letra abre el menú correspondiente.
                let menus = accion::menus(self.examen.is_some());
                if let Some(i) = menus
                    .iter()
                    .position(|(n, _)| n.to_lowercase().starts_with(c))
                {
                    self.modal = Some(Modal::Menu { indice: i, sel: 0 });
                }
            }
            return;
        }
        let b = &mut self.buffers[self.activo].buf;
        match k.code {
            KeyCode::Left if ctrl => b.palabra_izquierda(shift),
            KeyCode::Right if ctrl => b.palabra_derecha(shift),
            KeyCode::Left => b.izquierda(shift),
            KeyCode::Right => b.derecha(shift),
            KeyCode::Up => b.vertical(-1, shift),
            KeyCode::Down => b.vertical(1, shift),
            KeyCode::PageUp => {
                b.vertical(-alto, shift);
                b.desplazamiento = b.desplazamiento.saturating_sub(alto as usize);
            }
            KeyCode::PageDown => {
                b.vertical(alto, shift);
                b.desplazamiento += alto as usize;
            }
            KeyCode::Home if ctrl => b.mover_a(0, shift),
            KeyCode::End if ctrl => {
                let n = b.largo();
                b.mover_a(n, shift)
            }
            KeyCode::Home => b.inicio_linea(shift),
            KeyCode::End => b.fin_linea(shift),
            KeyCode::Esc => {
                b.ancla = None;
                if self.depuracion.is_some() && self.foco == Foco::Editor {
                    // nada: Esc no detiene la depuración
                }
            }
            KeyCode::Enter => self.editar(|b| b.nueva_linea()),
            KeyCode::Backspace => self.editar(|b| b.borrar_atras()),
            KeyCode::Delete => self.editar(|b| b.borrar_adelante()),
            // En un Makefile las recetas exigen un TAB real; en el resto, 4 espacios.
            KeyCode::Tab => self.editar(|b| {
                let t = if b.lenguaje == Lenguaje::Makefile {
                    "\t"
                } else {
                    "    "
                };
                b.insertar(t)
            }),
            KeyCode::BackTab => self.editar(desindentar),
            KeyCode::Char(c) if !ctrl => {
                let mut s = [0u8; 4];
                let t = c.encode_utf8(&mut s).to_string();
                self.editar(|b| b.insertar(&t));
            }
            _ => {}
        }
    }

    fn tecla_modal(&mut self, k: KeyEvent) {
        let Some(modal) = self.modal.take() else {
            return;
        };
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let alt = k.modifiers.contains(KeyModifiers::ALT);
        let cerrar = k.code == KeyCode::Esc;
        match modal {
            Modal::Paleta {
                mut filtro,
                mut sel,
                opciones,
            } => {
                if cerrar {
                    return;
                }
                let filtradas = paleta::filtrar(
                    &filtro,
                    &opciones
                        .iter()
                        .map(|o| o.titulo.as_str())
                        .collect::<Vec<_>>(),
                );
                match k.code {
                    KeyCode::Enter => {
                        if let Some(o) = filtradas.get(sel).map(|&i| opciones[i].clone()) {
                            self.ejecutar_opcion(o.que);
                        }
                        return;
                    }
                    KeyCode::Up => sel = sel.saturating_sub(1),
                    KeyCode::Down => sel = (sel + 1).min(filtradas.len().saturating_sub(1)),
                    _ => {
                        if editar_campo(&mut filtro, &k) {
                            sel = 0;
                        }
                    }
                }
                self.modal = Some(Modal::Paleta {
                    filtro,
                    sel,
                    opciones,
                });
            }
            Modal::Abrir {
                mut filtro,
                mut sel,
                archivos,
                mut marcados,
            } => {
                if cerrar {
                    return;
                }
                let filtrados = paleta::filtrar(&filtro, &archivos);
                match k.code {
                    KeyCode::Enter => {
                        // Con archivos marcados se abren todos (una pestaña cada uno).
                        let rels: Vec<String> = if marcados.is_empty() {
                            vec![filtrados
                                .get(sel)
                                .map(|&i| archivos[i].clone())
                                .unwrap_or_else(|| filtro.clone())]
                        } else {
                            marcados.iter().map(|&i| archivos[i].clone()).collect()
                        };
                        for rel in rels {
                            let rel = rel.trim();
                            if rel.is_empty() {
                                continue;
                            }
                            let ruta = self.proyecto.raiz.join(rel);
                            if self.examen.is_some() && !ruta.starts_with(&self.proyecto.raiz) {
                                continue;
                            }
                            self.abrir_archivo(&ruta, true);
                        }
                        return;
                    }
                    KeyCode::Tab => {
                        if let Some(&i) = filtrados.get(sel) {
                            if !marcados.remove(&i) {
                                marcados.insert(i);
                            }
                            sel = (sel + 1).min(filtrados.len().saturating_sub(1));
                        }
                    }
                    KeyCode::Up => sel = sel.saturating_sub(1),
                    KeyCode::Down => sel = (sel + 1).min(filtrados.len().saturating_sub(1)),
                    _ => {
                        if editar_campo(&mut filtro, &k) {
                            sel = 0;
                        }
                    }
                }
                self.modal = Some(Modal::Abrir {
                    filtro,
                    sel,
                    archivos,
                    marcados,
                });
            }
            Modal::Buscar { mut texto } => {
                if cerrar {
                    return;
                }
                if k.code == KeyCode::Enter {
                    self.busqueda = texto.clone();
                    self.buscar_siguiente();
                    // Enter repetido busca la siguiente; Esc cierra.
                    if !matches!(self.modal, Some(Modal::Buscar { .. })) {
                        self.modal = Some(Modal::Buscar { texto });
                    }
                    return;
                }
                editar_campo(&mut texto, &k);
                self.modal = Some(Modal::Buscar { texto });
            }
            Modal::IrALinea { mut texto } => {
                if cerrar {
                    return;
                }
                if k.code == KeyCode::Enter {
                    if let Ok(n) = texto.trim().parse::<usize>() {
                        let b = &mut self.buf_mut().buf;
                        let pos = b.linea_columna_a_pos(n.saturating_sub(1), 0);
                        b.mover_a(pos, false);
                        centrar(b, 10);
                    }
                    return;
                }
                if let KeyCode::Char(c) = k.code {
                    if !c.is_ascii_digit() {
                        self.modal = Some(Modal::IrALinea { texto });
                        return;
                    }
                }
                editar_campo(&mut texto, &k);
                self.modal = Some(Modal::IrALinea { texto });
            }
            Modal::Confirmar {
                titulo,
                mensaje,
                que,
            } => match k.code {
                KeyCode::Char('s' | 'S' | 'y' | 'Y') | KeyCode::Enter => match que {
                    Confirmacion::Salir => self.salir = true,
                    Confirmacion::CerrarBuffer => self.cerrar_buffer(true),
                    Confirmacion::Entregar => self.entregar(),
                    Confirmacion::Recuperar => self.recuperar(true),
                    Confirmacion::Borrar => self.borrar_ruta(),
                    Confirmacion::DeshacerCommit => self.deshacer_commit(),
                    Confirmacion::AbrirClonado => {
                        if let Some(p) = self.proyecto_pendiente.take() {
                            self.cambiar_proyecto(p);
                        }
                    }
                },
                KeyCode::Char('n' | 'N') => {
                    if que == Confirmacion::Recuperar {
                        self.recuperar(false);
                    }
                }
                // Esc cierra sin decidir: las copias de recuperación quedan para la próxima.
                KeyCode::Esc => {}
                _ => {
                    self.modal = Some(Modal::Confirmar {
                        titulo,
                        mensaje,
                        que,
                    })
                }
            },
            Modal::Commit { mut mensaje } => {
                if cerrar {
                    return;
                }
                let confirmar = (k.code == KeyCode::Enter && (ctrl || alt))
                    || (k.code == KeyCode::Char('s') && ctrl);
                if confirmar {
                    self.modal = Some(Modal::Commit {
                        mensaje: mensaje.clone(),
                    });
                    self.git_commit(&mensaje);
                    return;
                }
                if k.code == KeyCode::Enter {
                    mensaje.push('\n');
                } else {
                    editar_campo(&mut mensaje, &k);
                }
                self.modal = Some(Modal::Commit { mensaje });
            }
            Modal::Visor {
                titulo,
                lineas,
                mut desplazamiento,
            } => {
                let alto = self.tam.1.saturating_sub(8) as usize;
                match k.code {
                    KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => return,
                    KeyCode::Up => desplazamiento = desplazamiento.saturating_sub(1),
                    KeyCode::Down => desplazamiento += 1,
                    KeyCode::PageUp => desplazamiento = desplazamiento.saturating_sub(alto),
                    KeyCode::PageDown | KeyCode::Char(' ') => desplazamiento += alto,
                    KeyCode::Home => desplazamiento = 0,
                    _ => {}
                }
                let desplazamiento = desplazamiento.min(lineas.len().saturating_sub(1));
                self.modal = Some(Modal::Visor {
                    titulo,
                    lineas,
                    desplazamiento,
                });
            }
            Modal::Argumentos {
                titulo,
                comando,
                mut texto,
            } => {
                if cerrar {
                    return;
                }
                if k.code == KeyCode::Enter {
                    self.refrescar_estructura = true;
                    let completo = format!("{comando} {}", texto.trim()).trim_end().to_string();
                    self.correr_en_deck(completo);
                    return;
                }
                editar_campo(&mut texto, &k);
                self.modal = Some(Modal::Argumentos {
                    titulo,
                    comando,
                    texto,
                });
            }
            Modal::Renombrar { origen, mut texto } => {
                if cerrar {
                    return;
                }
                if k.code == KeyCode::Enter {
                    self.renombrar(&origen, &texto);
                    return;
                }
                editar_campo(&mut texto, &k);
                self.modal = Some(Modal::Renombrar { origen, texto });
            }
            Modal::NuevoArchivo { mut texto, carpeta } => {
                if cerrar {
                    return;
                }
                if k.code == KeyCode::Enter {
                    self.crear_archivo(&texto, carpeta);
                    return;
                }
                editar_campo(&mut texto, &k);
                self.modal = Some(Modal::NuevoArchivo { texto, carpeta });
            }
            Modal::Clonar {
                mut url,
                mut destino,
                mut campo,
            } => {
                if cerrar {
                    return;
                }
                match k.code {
                    KeyCode::Tab | KeyCode::BackTab | KeyCode::Up | KeyCode::Down => {
                        campo = 1 - campo
                    }
                    KeyCode::Enter if campo == 0 => campo = 1,
                    KeyCode::Enter => {
                        self.clonar(&url, &destino);
                        return;
                    }
                    _ => {
                        editar_campo(if campo == 0 { &mut url } else { &mut destino }, &k);
                    }
                }
                self.modal = Some(Modal::Clonar {
                    url,
                    destino,
                    campo,
                });
            }
            Modal::AbrirProyecto {
                dir,
                mut filtro,
                mut sel,
                entradas,
            } => {
                if cerrar {
                    return;
                }
                let etiquetas: Vec<&str> = entradas.iter().map(|e| e.etiqueta.as_str()).collect();
                let visibles = if filtro.is_empty() {
                    (0..entradas.len()).collect()
                } else {
                    paleta::filtrar(&filtro, &etiquetas)
                };
                match k.code {
                    KeyCode::Up => sel = sel.saturating_sub(1),
                    KeyCode::Down => sel = (sel + 1).min(visibles.len().saturating_sub(1)),
                    KeyCode::Enter => {
                        // Una ruta escrita a mano (~/tp, /home/...) se abre directo.
                        let escrita = expandir_ruta(&filtro, &dir);
                        if let Some(r) = escrita.filter(|r| r.is_dir()) {
                            self.cambiar_proyecto(r);
                            return;
                        }
                        let Some(e) = visibles.get(sel).map(|&i| entradas[i].clone()) else {
                            self.modal = Some(Modal::AbrirProyecto {
                                dir,
                                filtro,
                                sel,
                                entradas,
                            });
                            return;
                        };
                        if e.abrir {
                            self.cambiar_proyecto(e.destino);
                            return;
                        }
                        self.modal = Some(Modal::AbrirProyecto {
                            entradas: entradas_dir(&e.destino, false),
                            dir: e.destino,
                            filtro: String::new(),
                            sel: 0,
                        });
                        return;
                    }
                    KeyCode::Backspace if filtro.is_empty() => {
                        if let Some(p) = dir.parent().map(Path::to_path_buf) {
                            self.modal = Some(Modal::AbrirProyecto {
                                entradas: entradas_dir(&p, false),
                                dir: p,
                                filtro,
                                sel: 0,
                            });
                            return;
                        }
                    }
                    _ => {
                        if editar_campo(&mut filtro, &k) {
                            sel = 0;
                        }
                    }
                }
                self.modal = Some(Modal::AbrirProyecto {
                    dir,
                    filtro,
                    sel,
                    entradas,
                });
            }
            Modal::Diff {
                titulo,
                filas,
                mut desplazamiento,
                mut lado_a_lado,
                mut desplazamiento_h,
                lenguaje,
            } => {
                let alto = self.tam.1.saturating_sub(6) as usize;
                let cambios = jonesy_core::diff::inicios_de_cambios(&filas);
                match k.code {
                    KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => return,
                    KeyCode::Up => desplazamiento = desplazamiento.saturating_sub(1),
                    KeyCode::Down => desplazamiento += 1,
                    KeyCode::PageUp => desplazamiento = desplazamiento.saturating_sub(alto),
                    KeyCode::PageDown | KeyCode::Char(' ') => desplazamiento += alto,
                    KeyCode::Home => {
                        desplazamiento = 0;
                        desplazamiento_h = 0;
                    }
                    KeyCode::End => desplazamiento = filas.len(),
                    KeyCode::Left => desplazamiento_h = desplazamiento_h.saturating_sub(8),
                    KeyCode::Right => desplazamiento_h += 8,
                    KeyCode::Char('n') => {
                        if let Some(&i) = cambios.iter().find(|&&i| i > desplazamiento) {
                            desplazamiento = i;
                        }
                    }
                    KeyCode::Char('p') => {
                        if let Some(&i) = cambios.iter().rev().find(|&&i| i < desplazamiento) {
                            desplazamiento = i;
                        }
                    }
                    KeyCode::Char('s') | KeyCode::Char('u') | KeyCode::Tab => {
                        lado_a_lado = !lado_a_lado;
                        self.preferir_lado_a_lado = lado_a_lado;
                    }
                    _ => {}
                }
                let desplazamiento = desplazamiento.min(filas.len().saturating_sub(1));
                self.modal = Some(Modal::Diff {
                    titulo,
                    filas,
                    desplazamiento,
                    lado_a_lado,
                    desplazamiento_h,
                    lenguaje,
                });
            }
            Modal::Integraciones { mut sel } => {
                let n = self.integraciones.len().max(1);
                match k.code {
                    KeyCode::Esc | KeyCode::Enter => {
                        self.guardar_sesion();
                        return;
                    }
                    KeyCode::Up => sel = (sel + n - 1) % n,
                    KeyCode::Down => sel = (sel + 1) % n,
                    KeyCode::Char(' ') => {
                        if let Some(x) = self.integraciones.get_mut(sel) {
                            x.activa = !x.activa;
                            if !x.activa {
                                let nombre = x.nombre.clone();
                                self.diag_herramientas.retain(|(h, _), _| *h != nombre);
                            }
                        }
                    }
                    KeyCode::Char('r') => {
                        // Relee los manifiestos (después de editar .jonesy/integraciones).
                        let estado: Vec<(String, bool)> = self
                            .integraciones
                            .iter()
                            .map(|x| (x.nombre.clone(), x.activa))
                            .collect();
                        self.cargar_integraciones();
                        for (nombre, activa) in estado {
                            if let Some(x) =
                                self.integraciones.iter_mut().find(|x| x.nombre == nombre)
                            {
                                x.activa = activa;
                            }
                        }
                        self.integraciones_ausentes.clear();
                    }
                    _ => {}
                }
                self.modal = Some(Modal::Integraciones { sel });
            }
            Modal::Paneles { mut sel } => {
                let n = CAMPOS_PANELES.len();
                match k.code {
                    KeyCode::Esc | KeyCode::Enter => {
                        self.guardar_sesion();
                        return;
                    }
                    KeyCode::Up => sel = (sel + n - 1) % n,
                    KeyCode::Down => sel = (sel + 1) % n,
                    KeyCode::Left | KeyCode::Char('-') => ajustar_panel(&mut self.paneles, sel, -1),
                    KeyCode::Right | KeyCode::Char('+') => ajustar_panel(&mut self.paneles, sel, 1),
                    KeyCode::Char('r') => {
                        self.paneles = self.proyecto.config.interfaz.paneles.acotar()
                    }
                    _ => {}
                }
                self.modal = Some(Modal::Paneles { sel });
            }
            Modal::Menu {
                mut indice,
                mut sel,
            } => {
                let menus = accion::menus(self.examen.is_some());
                match k.code {
                    KeyCode::Esc | KeyCode::F(10) => return,
                    KeyCode::Left => {
                        indice = (indice + menus.len() - 1) % menus.len();
                        sel = 0;
                    }
                    KeyCode::Right => {
                        indice = (indice + 1) % menus.len();
                        sel = 0;
                    }
                    KeyCode::Up => sel = sel.saturating_sub(1),
                    KeyCode::Down => sel = (sel + 1).min(menus[indice].1.len() - 1),
                    KeyCode::Enter => {
                        let a = menus[indice].1[sel];
                        self.accion(a);
                        return;
                    }
                    _ => {}
                }
                self.modal = Some(Modal::Menu { indice, sel });
            }
        }
    }

    fn ejecutar_opcion(&mut self, q: QuePaleta) {
        match q {
            QuePaleta::Accion(a) => self.accion(a),
            QuePaleta::Objetivo(m, o) => self.correr_objetivo(m, &o),
            QuePaleta::Tp(i) => self.correr_tp(i),
            QuePaleta::Herramienta(nombre, n) => self.correr_accion_herramienta(&nombre, n),
            QuePaleta::Plugin(c) => {
                if let Some(p) = &self.plugins {
                    let b = &self.buf().buf;
                    let (l, col) = b.linea_columna();
                    let ctx = Contexto {
                        archivo: b
                            .ruta
                            .as_ref()
                            .map(|r| r.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                        texto: b.contenido(),
                        linea: l + 1,
                        columna: col + 1,
                    };
                    self.tel(TelemetryEvent::CommandExec {
                        command: format!("plugin:{}", c.titulo()),
                        exit_code: None,
                    });
                    p.pedir(PedidoPlugin::Ejecutar { comando: c, ctx });
                }
            }
        }
    }

    // ------------------------------------------------------------ mouse

    fn mouse(&mut self, m: MouseEvent) {
        let (x, y) = (m.column, m.row);
        let dentro = |r: Rect| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height;
        match m.kind {
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let delta: isize = if m.kind == MouseEventKind::ScrollUp {
                    -3
                } else {
                    3
                };
                if let Some(Modal::Diff {
                    desplazamiento,
                    filas,
                    ..
                }) = &mut self.modal
                {
                    *desplazamiento = (*desplazamiento as isize + delta)
                        .clamp(0, filas.len().saturating_sub(1) as isize)
                        as usize;
                    return;
                }
                if let Some(Modal::Visor {
                    desplazamiento,
                    lineas,
                    ..
                }) = &mut self.modal
                {
                    *desplazamiento = (*desplazamiento as isize + delta)
                        .clamp(0, lineas.len().saturating_sub(1) as isize)
                        as usize;
                    return;
                }
                if self.areas.izquierdo.is_some_and(dentro) {
                    let paso = delta.signum();
                    match self.navegador {
                        Some(PestanaNav::Archivos) => {
                            let n = self.explorador.filas.len().saturating_sub(1);
                            self.explorador.sel =
                                ((self.explorador.sel as isize + paso * 3).max(0) as usize).min(n);
                        }
                        Some(PestanaNav::Proyecto) => {
                            let n = self
                                .vista_proyecto
                                .filas(&self.proyecto.estructura)
                                .len()
                                .saturating_sub(1);
                            let v = &mut self.vista_proyecto;
                            v.sel = ((v.sel as isize + paso * 3).max(0) as usize).min(n);
                        }
                        None => {}
                    }
                } else if self.zonas.preview.is_some_and(dentro) {
                    let b = self.buf_mut();
                    b.preview_desplazamiento =
                        (b.preview_desplazamiento as isize + delta).max(0) as usize;
                } else if self
                    .zonas
                    .editor
                    .is_some_and(|z| dentro(z.texto) || dentro(z.gutter))
                {
                    let b = &mut self.buf_mut().buf;
                    let max = b.lineas().saturating_sub(1);
                    b.desplazamiento =
                        ((b.desplazamiento as isize + delta).max(0) as usize).min(max);
                    self.desplazado_por_mouse = true;
                } else if self.areas.consigna.is_some_and(dentro) {
                    if let Some(c) = &mut self.consigna {
                        c.desplazamiento = (c.desplazamiento as isize + delta).max(0) as usize;
                    }
                } else if self.zonas.deck.is_some_and(dentro) {
                    let deck = match self.pestana {
                        Pestana::Terminal => self.terminal.as_ref(),
                        Pestana::Ejecucion => self.ejecucion.as_ref(),
                        _ => None,
                    };
                    if let Some(d) = deck {
                        let actual = d.con_pantalla(|s| s.scrollback()) as isize;
                        d.desplazar((actual - delta).max(0) as usize);
                    }
                } else if self.areas.inferior.is_some_and(dentro) {
                    match self.pestana {
                        Pestana::Salida => {
                            self.salida_desplazamiento =
                                (self.salida_desplazamiento as isize - delta).max(0) as usize
                        }
                        Pestana::Problemas => {
                            let total = self.diagnosticos().len();
                            self.sel_problema = ((self.sel_problema as isize + delta.signum())
                                .max(0) as usize)
                                .min(total.saturating_sub(1));
                        }
                        _ => {}
                    }
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                self.clic(x, y, m.modifiers.contains(KeyModifiers::SHIFT))
            }
            MouseEventKind::Drag(MouseButton::Left) => match self.arrastre {
                Some(Arrastre::Seleccion) => {
                    if let Some(p) = self.posicion_en_editor(x, y) {
                        self.buf_mut().buf.mover_a(p, true);
                    }
                }
                Some(Arrastre::Separador(sep)) => sep.aplicar(x, y, &mut self.paneles),
                None => {}
            },
            MouseEventKind::Down(MouseButton::Middle) => {
                // Clic del medio sobre una pestaña la cierra, como en los navegadores.
                let pestana = self
                    .zonas
                    .pestanas_buffers
                    .iter()
                    .find(|(r, _)| dentro(*r))
                    .map(|(_, i)| *i);
                if let Some(i) = pestana {
                    self.cerrar_pestana(i);
                }
            }
            MouseEventKind::Up(_) => {
                if matches!(self.arrastre, Some(Arrastre::Separador(_))) {
                    self.guardar_sesion();
                }
                self.arrastre = None;
            }
            _ => {}
        }
    }

    fn clic(&mut self, x: u16, y: u16, shift: bool) {
        let dentro = |r: Rect| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height;
        let doble = self.ultimo_clic.is_some_and(|(t, cx, cy)| {
            t.elapsed() < Duration::from_millis(400) && cx == x && cy == y
        });
        self.ultimo_clic = Some((Instant::now(), x, y));

        // Menú desplegado y modales.
        if let Some(Modal::Menu { .. }) = self.modal {
            if let Some((_, a)) = self
                .zonas
                .items_menu
                .iter()
                .find(|(r, _)| dentro(*r))
                .copied()
            {
                self.modal = None;
                self.accion(a);
                return;
            }
            if let Some((_, i)) = self.zonas.menu.iter().find(|(r, _)| dentro(*r)).copied() {
                self.modal = Some(Modal::Menu { indice: i, sel: 0 });
                return;
            }
            self.modal = None;
            return;
        }
        if let Some(modal) = &mut self.modal {
            if let (
                Some((r, base)),
                Modal::Paleta { sel, .. }
                | Modal::Abrir { sel, .. }
                | Modal::AbrirProyecto { sel, .. },
            ) = (self.zonas.modal_items, &mut *modal)
            {
                if dentro(r) {
                    *sel = base + (y - r.y) as usize;
                    let k = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
                    self.tecla_modal(k);
                    return;
                }
            }
            if !matches!(modal, Modal::Visor { .. }) {
                return;
            }
            self.modal = None;
            return;
        }
        if let Some((_, i)) = self.zonas.menu.iter().find(|(r, _)| dentro(*r)).copied() {
            self.modal = Some(Modal::Menu { indice: i, sel: 0 });
            return;
        }
        if let Some((_, a)) = self
            .zonas
            .botones
            .iter()
            .chain(self.zonas.botones_depurador.iter())
            .find(|(r, _)| dentro(*r))
            .copied()
        {
            self.accion(a);
            return;
        }
        if let Some((_, i)) = self
            .zonas
            .cierres_buffers
            .iter()
            .find(|(r, _)| dentro(*r))
            .copied()
        {
            self.cerrar_pestana(i);
            return;
        }
        if let Some((_, d)) = self
            .zonas
            .desborde_pestanas
            .iter()
            .find(|(r, _)| dentro(*r))
            .copied()
        {
            let n = self.buffers.len().saturating_sub(1);
            self.primera_pestana =
                (self.primera_pestana as isize + d).clamp(0, n as isize) as usize;
            return;
        }
        if let Some((_, i)) = self
            .zonas
            .pestanas_buffers
            .iter()
            .find(|(r, _)| dentro(*r))
            .copied()
        {
            self.cambiar_buffer(i);
            return;
        }
        if let Some((_, p)) = self
            .zonas
            .pestanas_inferior
            .iter()
            .find(|(r, _)| dentro(*r))
            .copied()
        {
            self.pestana = p;
            self.cambiar_foco(Foco::Inferior);
            if p == Pestana::Terminal && self.terminal.is_none() {
                self.abrir_terminal();
            }
            return;
        }
        // Separadores arrastrables entre paneles.
        if let Some(sep) = self
            .areas
            .separadores
            .iter()
            .find(|s| s.toca(x, y))
            .copied()
        {
            self.arrastre = Some(Arrastre::Separador(sep));
            return;
        }
        // Barra izquierda: pestañas y filas.
        if let Some((_, p)) = self
            .zonas
            .pestanas_nav
            .iter()
            .find(|(r, _)| dentro(*r))
            .copied()
        {
            self.navegador = Some(p);
            self.cambiar_foco(Foco::Navegador);
            if p == PestanaNav::Archivos && self.explorador.filas.is_empty() {
                self.explorador.reconstruir(&self.proyecto.raiz);
            }
            return;
        }
        if let Some((r, base)) = self.zonas.lista_nav {
            if dentro(r) {
                self.cambiar_foco(Foco::Navegador);
                let i = base + (y - r.y) as usize;
                match self.navegador {
                    Some(PestanaNav::Archivos) => {
                        if let Some(f) = self.explorador.filas.get(i).cloned() {
                            self.explorador.sel = i;
                            if f.es_dir {
                                self.explorador.alternar(i, &self.proyecto.raiz);
                            } else {
                                // Un clic abre el archivo (en una pestaña nueva si no estaba).
                                self.abrir_archivo(&f.ruta, true);
                                self.foco = Foco::Navegador;
                            }
                        }
                    }
                    Some(PestanaNav::Proyecto) => {
                        let filas = self.vista_proyecto.filas(&self.proyecto.estructura);
                        if let Some(f) = filas.get(i).cloned() {
                            self.vista_proyecto.sel = i;
                            // Módulos se despliegan con un clic; objetivos corren con doble clic.
                            let correr =
                                doble || matches!(f, FilaProyecto::Modulo(_) | FilaProyecto::TpSh);
                            if correr {
                                self.activar_fila_proyecto(Some(f), true);
                            }
                        }
                    }
                    None => {}
                }
                return;
            }
        }
        if self.areas.izquierdo.is_some_and(dentro) {
            self.cambiar_foco(Foco::Navegador);
            return;
        }
        if let Some(z) = self.zonas.editor {
            if dentro(z.gutter) {
                let linea = self.buf().buf.desplazamiento + (y - z.gutter.y) as usize;
                if linea < self.buf().buf.lineas() {
                    self.alternar_breakpoint(linea);
                }
                self.cambiar_foco(Foco::Editor);
                return;
            }
            if dentro(z.texto) {
                self.cambiar_foco(Foco::Editor);
                if let Some(p) = self.posicion_en_editor(x, y) {
                    if doble {
                        self.buf_mut().buf.seleccionar_palabra(p);
                    } else {
                        self.buf_mut().buf.mover_a(p, shift);
                        self.arrastre = Some(Arrastre::Seleccion);
                    }
                }
                return;
            }
        }
        if let Some(r) = self.zonas.preview {
            if dentro(r) {
                self.cambiar_foco(Foco::Preview);
                let b = self.buf();
                let linea = b.preview_desplazamiento + (y - r.y) as usize;
                let col = (x - r.x) as usize;
                let destino = b
                    .preview
                    .as_ref()
                    .and_then(|(_, _, d)| enlace_en(d, linea, col));
                let base = b
                    .buf
                    .ruta
                    .as_ref()
                    .and_then(|p| p.parent().map(Path::to_path_buf));
                if let (Some(d), Some(base)) = (destino, base) {
                    self.abrir_enlace(&base, &d);
                }
                return;
            }
        }
        if let Some(r) = self.areas.consigna {
            if dentro(r) {
                self.cambiar_foco(Foco::Consigna);
                if let Some(c) = &self.consigna {
                    let linea = c.desplazamiento + (y.saturating_sub(r.y + 1)) as usize;
                    let col = x.saturating_sub(r.x + 1) as usize;
                    let destino = c.doc.as_ref().and_then(|(_, d)| enlace_en(d, linea, col));
                    let base = c.ruta.parent().map(Path::to_path_buf);
                    if let (Some(d), Some(base)) = (destino, base) {
                        self.abrir_enlace(&base, &d);
                    }
                }
                return;
            }
        }
        if let Some((r, base)) = self.zonas.lista_inferior {
            if dentro(r) {
                self.cambiar_foco(Foco::Inferior);
                if self.pestana == Pestana::Problemas {
                    self.sel_problema = base + (y - r.y) as usize;
                    self.saltar_a_problema(self.sel_problema);
                }
                return;
            }
        }
        if let Some((r, base)) = self.zonas.lista_lateral {
            if dentro(r) {
                self.cambiar_foco(Foco::Lateral);
                let i = base + (y - r.y) as usize;
                match self.lateral {
                    Some(Lateral::Git) => {
                        if self.git.log.is_some() {
                            self.git.sel_log = i;
                        } else {
                            self.git.sel = i;
                        }
                        if doble {
                            self.git_mostrar();
                        }
                    }
                    Some(Lateral::Reglas) => {
                        self.sel_regla = i;
                        let d = self
                            .reglas_agrupadas()
                            .into_iter()
                            .flat_map(|(_, v)| v)
                            .nth(i)
                            .cloned();
                        if let Some(Diagnostico {
                            archivo: Some(a),
                            linea,
                            columna,
                            ..
                        }) = d
                        {
                            self.ir_a(&a, linea, columna);
                        }
                    }
                    _ => {}
                }
                return;
            }
        }
        if self.zonas.deck.is_some_and(dentro) {
            self.cambiar_foco(Foco::Inferior);
            return;
        }
        if self.areas.inferior.is_some_and(dentro) {
            self.cambiar_foco(Foco::Inferior);
            if self.pestana == Pestana::Depurador {
                if let Some(d) = &mut self.depuracion {
                    // Mitad izquierda: programa; derecha: consola.
                    let r = self.areas.inferior.unwrap();
                    d.foco_consola = x >= r.x + r.width / 2;
                }
            }
            return;
        }
        if self.areas.lateral.is_some_and(dentro) {
            self.cambiar_foco(Foco::Lateral);
            return;
        }
        if self.areas.quickdoc.is_some_and(dentro) {
            self.cambiar_foco(Foco::Doc);
        }
    }

    fn abrir_enlace(&mut self, base: &Path, destino: &str) {
        // Enlace a una sección de la misma consigna (#ancla).
        if let Some(ancla) = destino.strip_prefix('#') {
            if let Some(c) = &mut self.consigna {
                if let Some(l) = c.doc.as_ref().and_then(|(_, d)| linea_de_ancla(d, ancla)) {
                    c.desplazamiento = l;
                    return;
                }
            }
        }
        if !markdown::es_enlace_local(destino) {
            self.avisar(format!("Enlace externo: {destino}"), Nivel::Info);
            return;
        }
        let sin_ancla = destino.split('#').next().unwrap_or(destino);
        let ruta = base.join(sin_ancla);
        if ruta.is_file() {
            self.abrir_archivo(&ruta, true);
        } else {
            self.avisar(format!("No existe {}", ruta.display()), Nivel::Aviso);
        }
    }

    /// Posición de carácter del buffer activo bajo la celda (x, y).
    fn posicion_en_editor(&self, x: u16, y: u16) -> Option<usize> {
        let z = self.zonas.editor?;
        let b = &self.buf().buf;
        let fila = y
            .saturating_sub(z.texto.y)
            .min(z.texto.height.saturating_sub(1)) as usize;
        let linea = (b.desplazamiento + fila).min(b.lineas().saturating_sub(1));
        let objetivo = b.desplazamiento_h + x.saturating_sub(z.texto.x) as usize;
        let chars: Vec<char> = b.linea_texto(linea).chars().collect();
        let col = columna_de_visual(&chars, objetivo);
        Some(b.linea_columna_a_pos(linea, col))
    }
}

// ------------------------------------------------------------------ ayudas

pub fn mismo_archivo(a: &Path, b: &Path) -> bool {
    a == b
        || a.canonicalize()
            .ok()
            .zip(b.canonicalize().ok())
            .is_some_and(|(x, y)| x == y)
}

pub const ANCHO_TAB: usize = 4;

/// Columna visual de cada carácter (tabs a múltiplos de 4, anchos dobles).
pub fn columnas_visuales(chars: &[char]) -> Vec<usize> {
    let mut v = Vec::with_capacity(chars.len() + 1);
    let mut col = 0;
    for &c in chars {
        v.push(col);
        col += ancho_char(c, col);
    }
    v.push(col);
    v
}

pub fn ancho_char(c: char, col: usize) -> usize {
    if c == '\t' {
        ANCHO_TAB - col % ANCHO_TAB
    } else {
        unicode_width::UnicodeWidthChar::width(c)
            .unwrap_or(0)
            .max(1)
    }
}

/// Índice de carácter para una columna visual.
pub fn columna_de_visual(chars: &[char], objetivo: usize) -> usize {
    let cols = columnas_visuales(chars);
    for i in 0..chars.len() {
        if cols[i + 1] > objetivo {
            return if objetivo - cols[i] > (cols[i + 1] - cols[i]) / 2 {
                i + 1
            } else {
                i
            };
        }
    }
    chars.len()
}

fn centrar(b: &mut Buffer, margen: usize) {
    let (l, _) = b.linea_columna();
    if l < b.desplazamiento || l >= b.desplazamiento + margen * 2 {
        b.desplazamiento = l.saturating_sub(margen);
    }
}

fn desindentar(b: &mut Buffer) -> Option<Delta> {
    let (l, _) = b.linea_columna();
    let inicio = b.linea_columna_a_pos(l, 0);
    let linea = b.linea_texto(l);
    let n = linea.chars().take(4).take_while(|c| *c == ' ').count();
    if n == 0 {
        return None;
    }
    let cursor = b.cursor;
    b.mover_a(inicio, false);
    b.mover_a(inicio + n, true);
    let d = b.insertar("");
    let nuevo = cursor.saturating_sub(n).max(inicio);
    b.mover_a(nuevo, false);
    d
}

/// Filas de Ver → Tamaños de paneles: (título, unidad).
pub const CAMPOS_PANELES: [(&str, &str); 6] = [
    ("Barra izquierda (archivos y proyecto)", "columnas"),
    ("Consigna", "% (0 = según el modo)"),
    ("quick-doc a la derecha", "%"),
    ("quick-doc en el dock inferior", "filas"),
    ("Panel lateral derecho (git, reglas, variables)", "%"),
    ("Panel inferior (problemas, terminal, depurador)", "filas"),
];

/// Valor actual de la fila `i` de [`CAMPOS_PANELES`].
pub fn valor_panel(t: &TamPaneles, i: usize) -> u16 {
    [
        t.izquierdo,
        t.consigna,
        t.quickdoc,
        t.dock,
        t.lateral,
        t.inferior,
    ][i]
}

/// Suma `delta` pasos a la fila `i` (columnas y filas de a 1, porcentajes de a 5).
pub fn ajustar_panel(t: &mut TamPaneles, i: usize, delta: i32) {
    let campo = match i {
        0 => &mut t.izquierdo,
        1 => &mut t.consigna,
        2 => &mut t.quickdoc,
        3 => &mut t.dock,
        4 => &mut t.lateral,
        _ => &mut t.inferior,
    };
    let paso = if matches!(i, 1 | 2 | 4) { 5 } else { 1 };
    // La consigna salta de 0 (automático) a 15 y vuelve a 0 por debajo de 15.
    let nuevo = *campo as i32 + delta * paso;
    *campo = if i == 1 && nuevo < 15 {
        if delta > 0 {
            15
        } else {
            0
        }
    } else {
        nuevo.max(0) as u16
    };
    *t = t.acotar();
}

/// Nombre de la carpeta que crea `git clone` para una URL.
pub fn nombre_de_repo(url: &str) -> String {
    let u = url.trim().trim_end_matches('/');
    let ultimo = u.rsplit(['/', ':']).next().unwrap_or(u);
    let n = ultimo.strip_suffix(".git").unwrap_or(ultimo);
    if n.is_empty() {
        "repositorio".into()
    } else {
        n.to_string()
    }
}

/// Índice del enlace siguiente (o anterior) al elegido, circular.
pub fn siguiente_enlace(d: &Documento, actual: Option<usize>, adelante: bool) -> Option<usize> {
    let n = d.enlaces.len();
    if n == 0 {
        return None;
    }
    Some(match (actual, adelante) {
        (None, true) => 0,
        (None, false) => n - 1,
        (Some(i), true) => (i + 1) % n,
        (Some(i), false) => (i + n - 1) % n,
    })
}

/// Desplazamiento que deja `linea` a la vista.
fn a_la_vista(linea: usize, desplazamiento: usize, alto: usize) -> usize {
    if linea < desplazamiento {
        linea
    } else if linea >= desplazamiento + alto {
        linea + 1 - alto
    } else {
        desplazamiento
    }
}

/// Ancla de un encabezado como la arma GitHub: minúsculas, espacios a guiones, sin
/// puntuación.
pub fn ancla_de(titulo: &str) -> String {
    titulo
        .trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
            _ => None,
        })
        .collect()
}

/// Línea del encabezado cuya ancla es `ancla`.
pub fn linea_de_ancla(d: &Documento, ancla: &str) -> Option<usize> {
    let buscada = ancla_de(ancla);
    d.encabezados
        .iter()
        .find(|(_, t)| ancla_de(t) == buscada)
        .map(|(l, _)| *l)
}

/// Ruta escrita en un campo: `~/x`, absoluta o relativa a `base`. None si está vacía.
pub fn expandir_ruta(texto: &str, base: &Path) -> Option<PathBuf> {
    let t = texto.trim();
    if t.is_empty() {
        return None;
    }
    if let Some(resto) = t
        .strip_prefix("~/")
        .or(if t == "~" { Some("") } else { None })
    {
        return std::env::var_os("HOME").map(|h| PathBuf::from(h).join(resto));
    }
    let p = PathBuf::from(t);
    Some(if p.is_absolute() { p } else { base.join(p) })
}

/// Reemplaza tabs por espacios hasta la próxima columna múltiplo de 4.
pub fn expandir_tabs(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut col = 0;
    for c in s.chars() {
        if c == '\t' {
            let n = ANCHO_TAB - col % ANCHO_TAB;
            out.extend(std::iter::repeat_n(' ', n));
            col += n;
        } else {
            out.push(c);
            col += 1;
        }
    }
    out
}

/// Edita un campo de texto de una línea; true si cambió.
pub fn editar_campo(campo: &mut String, k: &KeyEvent) -> bool {
    match k.code {
        KeyCode::Backspace => campo.pop().is_some(),
        KeyCode::Char('u') if k.modifiers.contains(KeyModifiers::CONTROL) => {
            campo.clear();
            true
        }
        KeyCode::Char(c)
            if !k
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            campo.push(c);
            true
        }
        _ => false,
    }
}

/// Bytes para el PTY de una tecla.
pub fn bytes_tecla(k: &KeyEvent) -> Option<Vec<u8>> {
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    let alt = k.modifiers.contains(KeyModifiers::ALT);
    let t = match k.code {
        KeyCode::Char(c) if ctrl => Tecla::Ctrl(c),
        KeyCode::Char(c) if alt => Tecla::Alt(c),
        KeyCode::Char(c) => Tecla::Caracter(c),
        KeyCode::Enter => Tecla::Enter,
        KeyCode::Backspace => Tecla::Retroceso,
        KeyCode::Tab => Tecla::Tab,
        KeyCode::Esc => Tecla::Esc,
        KeyCode::Up => Tecla::Arriba,
        KeyCode::Down => Tecla::Abajo,
        KeyCode::Left => Tecla::Izquierda,
        KeyCode::Right => Tecla::Derecha,
        KeyCode::Home => Tecla::Inicio,
        KeyCode::End => Tecla::Fin,
        KeyCode::PageUp => Tecla::RePag,
        KeyCode::PageDown => Tecla::AvPag,
        KeyCode::Delete => Tecla::Suprimir,
        _ => return None,
    };
    Some(bytes_de_tecla(t))
}

fn enlace_en(d: &Documento, linea: usize, col: usize) -> Option<String> {
    d.enlaces
        .iter()
        .find(|e| e.linea == linea && col >= e.ini && col < e.fin.max(e.ini + 1))
        .map(|e| e.destino.clone())
}

/// Colorea un diff unificado.
pub fn lineas_diff(texto: &str) -> Vec<Line<'static>> {
    use ratatui::style::{Modifier, Style};
    texto
        .lines()
        .map(|l| {
            let estilo = if l.starts_with("+++")
                || l.starts_with("---")
                || l.starts_with("diff ")
                || l.starts_with("index ")
            {
                Style::default().add_modifier(Modifier::BOLD)
            } else if l.starts_with('+') {
                Style::default().fg(tema::OK)
            } else if l.starts_with('-') {
                Style::default().fg(tema::ERROR)
            } else if l.starts_with("@@") {
                Style::default().fg(tema::NOTA)
            } else if l.starts_with("commit ") {
                Style::default()
                    .fg(tema::ADVERTENCIA)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Line::styled(l.replace('\t', "    "), estilo)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nombre_de_repo_como_git() {
        assert_eq!(
            nombre_de_repo("https://github.com/INGCOM-UNRN-P1/plantilla-TP.git"),
            "plantilla-TP"
        );
        assert_eq!(nombre_de_repo("git@github.com:usuario/tp3.git"), "tp3");
        assert_eq!(nombre_de_repo("https://host/x/lista/"), "lista");
    }

    #[test]
    fn expande_tabs_a_multiplos_de_cuatro() {
        assert_eq!(expandir_tabs("4\tint *p;"), "4   int *p;");
        assert_eq!(expandir_tabs("\tx"), "    x");
    }
}
