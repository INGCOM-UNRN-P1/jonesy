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

mod acciones;
mod depurador;
mod examen;
mod git;
mod mouse;
mod teclas;
mod teclas_modal;

pub const AUTOGUARDADO: Duration = Duration::from_secs(30);
pub const DEBOUNCE_PREVIEW: Duration = Duration::from_millis(120);
/// Pausa al escribir antes de analizar el buffer con las integraciones en vivo (gaff).
pub const DEBOUNCE_VIVO: Duration = Duration::from_millis(600);
pub const SONDEO_GIT: Duration = Duration::from_secs(2);
pub const AVISOS_MIN: [u64; 3] = [30, 15, 5];
/// Las herramientas lentas (ripley tarda ~10 s) corren este tiempo después del último
/// guardado: guardar seguido no las relanza cada vez.
pub const DEMORA_LENTAS: Duration = Duration::from_millis(1500);
static ARCHIVO_TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[derive(Debug, Clone)]
pub struct Opciones {
    pub raiz: PathBuf,
    pub archivos: Vec<PathBuf>,
    pub examen: bool,
    pub ascii: bool,
    /// Telemetría fuera del modo examen (en examen siempre está activa).
    pub telemetria: bool,
    /// Mostrar la bienvenida si es la primera vez (el binario; no las pruebas ni el
    /// examen).
    pub bienvenida: bool,
    /// Empezar un simulacro de examen de estos minutos (`jonesy --simulacro [min]`).
    pub simulacro: Option<u64>,
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
    /// Resultados de Editar → Buscar en el proyecto.
    Busqueda,
}

impl Pestana {
    pub const TODAS: [Pestana; 6] = [
        Pestana::Problemas,
        Pestana::Salida,
        Pestana::Terminal,
        Pestana::Ejecucion,
        Pestana::Depurador,
        Pestana::Busqueda,
    ];

    pub fn titulo(self) -> &'static str {
        match self {
            Pestana::Problemas => "Problemas",
            Pestana::Salida => "Salida",
            Pestana::Terminal => "Terminal",
            Pestana::Ejecucion => "Ejecución",
            Pestana::Depurador => "Depurador",
            Pestana::Busqueda => "Búsqueda",
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
    /// Abrir un binario en el visor hexadecimal.
    Hex(PathBuf),
    /// Acción de una integración: (nombre, índice de la acción).
    Herramienta(String, usize),
    /// Ir a una línea (1-based) de un archivo: marcadores.
    Linea(PathBuf, usize),
    /// Entrada para Ctrl+F5 en este ejercicio (None: desde el teclado).
    Entrada(Option<PathBuf>),
    /// Un término del glosario (índice).
    Glosario(usize),
    /// Una regla del catálogo de las herramientas (índice).
    Regla(usize),
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
    /// Volver el archivo de `App::descartar_pendiente` al último commit.
    Descartar,
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
    /// Editar → Renombrar en su ámbito: el nombre nuevo de la variable o función.
    RenombrarSimbolo {
        plan: jonesy_core::renombrar::Plan,
        texto: String,
        error: Option<String>,
    },
    /// Ctrl+Espacio: los nombres que completan la palabra del cursor, que empieza en
    /// `inicio` (posición en caracteres).
    Completar {
        inicio: usize,
        opciones: Vec<jonesy_core::completar::Candidato>,
        sel: usize,
    },
    /// Depurar → Vigilar: la expresión a agregar (o a quitar, si ya está).
    Vigilar {
        texto: String,
    },
    /// Exportar o importar las preferencias: la ruta del archivo.
    RutaPreferencias {
        importar: bool,
        texto: String,
    },
    /// Examen → Pausar: el código del docente (se muestra con asteriscos).
    CodigoPausa {
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
    /// Ayuda → Bienvenida (y al primer inicio): si se muestra en cada inicio y qué falta
    /// del entorno (nombre, para qué, cómo instalarlo).
    Bienvenida {
        siempre: bool,
        faltan: Vec<(String, String, String)>,
    },
    /// Ayuda → Recorrido guiado: paso actual (ver `ayuda::RECORRIDO`).
    Recorrido {
        paso: usize,
    },
    /// Archivo → Simulacro de examen: minutos.
    Simulacro {
        texto: String,
    },
    /// Construir → Comparar con las salidas esperadas: un caso por fila.
    Casos {
        resultados: Vec<jonesy_core::casos::ResultadoCaso>,
        sel: usize,
    },
    /// Construir → Argumentos del programa.
    ArgsPrograma {
        texto: String,
    },
    /// Editar → Buscar en el proyecto (Ctrl+Shift+F).
    BuscarProyecto {
        texto: String,
        mayusculas: bool,
        palabra: bool,
    },
    /// Editar → Buscar y reemplazar (Ctrl+H).
    Reemplazar {
        buscar: String,
        reemplazo: String,
        /// 0: buscar; 1: reemplazar.
        campo: u8,
        mayusculas: bool,
        palabra: bool,
    },
    /// Archivo → Configuración del entorno: solapa (ver `PESTANAS_CONFIG`) y fila.
    Configuracion {
        pestana: usize,
        sel: usize,
    },
    /// Configuración del entorno → Apariencia → Tamaños de paneles: fila elegida.
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
    /// Compilar al guardar: sin abrir el panel ni cambiar de pestaña.
    Guardado,
    Ejecutar,
    Depurar,
    /// Correr los casos de entrada y salida esperada.
    Casos,
}

/// Cómo interpretar la salida de un objetivo de make.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lectura {
    /// Sólo errores de gcc y del enlazador.
    Make,
    /// Construir → Probar: la salida de p1_test.
    Pruebas,
    /// Depurar → Revisar memoria con valgrind: la salida de valgrind.
    Memoria,
}

/// Dónde queda el informe de memoria de un programa que corre en la terminal.
#[derive(Debug, Clone)]
enum InformeMemoria {
    /// valgrind lo escribe en este archivo (`--log-file`).
    Valgrind(PathBuf),
    /// AddressSanitizer lo escribe en la terminal, con la salida del programa.
    Asan,
    /// Construir → Ejecutar con sanitizers (ASan y UBSan): el informe en la terminal, que
    /// tetsuo explica si está; si no, `asan::interpretar`.
    Sanitizers,
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
    /// Acción de una herramienta (Alt+H) que termina en un visor, en diagnósticos o recargando
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
        herramienta: String,
        /// Lo que dijo la herramienta en la línea del alumno.
        hallazgo: String,
    },
    MemTrace {
        traza: PathBuf,
    },
    /// Objetivo de make lanzado desde la vista de proyecto o la paleta.
    Objetivo {
        comando: String,
        /// Cómo leer la salida además de los errores de gcc.
        lectura: Lectura,
    },
    /// Autoformato sobre una copia temporal del buffer.
    Formatear {
        archivo: PathBuf,
        temporal: PathBuf,
        version: u64,
    },
    /// Traducción de lo que imprimió el compilador (`traducir_compilacion`, daedalus) o un
    /// programa con sanitizers (`traducir_sanitizers`, tetsuo).
    Traducir {
        integracion: String,
        sanitizers: bool,
        /// El archivo activo (para los hallazgos sin archivo).
        analizado: PathBuf,
        /// La compilación que se traduce: si hubo otra, el resultado ya es viejo.
        compilacion: u64,
        /// Lo que imprimió el programa (para mostrarlo con el informe).
        texto: String,
    },
    /// Análisis del buffer sin guardar mientras se escribe (`en_vivo`, gaff).
    EnVivo {
        nombre: String,
        archivo: PathBuf,
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
    /// Un binario: la pestaña muestra el visor hexadecimal en lugar del texto.
    pub hex: Option<Box<VisorHex>>,
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
            hex: None,
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

/// Solapas de Archivo → Configuración del entorno.
pub const PESTANAS_CONFIG: [&str; 4] = [
    "Integraciones y linters",
    "Plugins",
    "Compilador y formato",
    "Apariencia",
];

/// Una fila de la configuración: qué dice y qué hace Enter (o Espacio) sobre ella.
pub struct FilaConfig {
    pub titulo: String,
    pub valor: String,
    pub detalle: String,
    pub que: QueConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub enum QueConfig {
    /// Activar o apagar la integración (índice en `App::integraciones`).
    Integracion(usize),
    Accion(Accion),
    /// Abrir un archivo en el editor.
    Abrir(PathBuf),
    /// Abrir `.jonesy.toml` (creándolo desde el ejemplo si no existe).
    EditarConfig,
    /// Abrir `.clang-format` (creándolo con el estilo de la cátedra si no existe).
    EditarClangFormat,
    Tema,
    OrientacionDoc,
    ReleerIntegraciones,
    /// Sólo informa.
    Nada,
}

/// Un simulacro de examen: cronómetro y avisos como en el examen, sin restricciones ni
/// sellado, y un resumen al terminar.
#[derive(Debug, Clone)]
pub struct Simulacro {
    pub inicio: Instant,
    pub duracion: Duration,
    avisos: BTreeSet<u64>,
    compilaciones: u32,
    con_errores: u32,
    ejecuciones: u32,
    pruebas: Option<(usize, usize)>,
}

/// El visor hexadecimal de un archivo binario (Ver → Visor hexadecimal, o abrir un
/// ejecutable, un `.o` o un `.a`).
#[derive(Debug, Clone)]
pub struct VisorHex {
    pub ruta: PathBuf,
    pub datos: Vec<u8>,
    pub resumen: String,
    /// Offset de la primera fila visible.
    pub inicio: usize,
    /// Byte bajo el cursor.
    pub cursor: usize,
    /// Largo de la coincidencia de la última búsqueda (resaltada desde el cursor).
    pub marca: usize,
    /// Información o símbolos en lugar de los bytes: (título, líneas, desplazamiento).
    pub detalle: Option<(String, Vec<String>, usize)>,
    /// Campo abierto: (es «ir a», texto); si no, buscar.
    pub entrada: Option<(bool, String)>,
    pub buscado: Vec<u8>,
    /// Bytes por fila y filas visibles del último dibujo.
    pub ancho: usize,
    pub alto: usize,
}

impl VisorHex {
    fn abrir(ruta: &Path) -> std::io::Result<VisorHex> {
        use std::io::Read;
        let mut datos = vec![];
        std::fs::File::open(ruta)?
            .take(jonesy_core::hex::LIMITE as u64)
            .read_to_end(&mut datos)?;
        Ok(VisorHex {
            resumen: jonesy_core::hex::resumen(ruta, &datos),
            ruta: ruta.to_path_buf(),
            datos,
            inicio: 0,
            cursor: 0,
            marca: 0,
            detalle: None,
            entrada: None,
            buscado: vec![],
            ancho: 16,
            alto: 20,
        })
    }

    /// Mueve el cursor y desplaza para que se vea.
    pub fn ir(&mut self, pos: usize) {
        let ancho = self.ancho.max(1);
        self.cursor = pos.min(self.datos.len().saturating_sub(1));
        let fila = self.cursor / ancho * ancho;
        if fila < self.inicio {
            self.inicio = fila;
        } else if fila >= self.inicio + ancho * self.alto.max(1) {
            self.inicio = fila + ancho - ancho * self.alto.max(1);
        }
    }

    fn mover(&mut self, delta: isize) {
        let p = (self.cursor as isize + delta).max(0) as usize;
        self.marca = 0;
        self.ir(p);
    }
}

/// Lo que espera a que se cargue el catálogo de reglas.
#[derive(Debug, Clone)]
enum PendienteCatalogo {
    Explicar(Box<Diagnostico>),
    Lista,
}

/// Operación con el repositorio remoto (corre en la terminal, por la autenticación).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpRemota {
    Subir,
    Traer,
}

/// Estado de la vigilancia de cuelgues (ver `App::vigilar_cuelgue`).
#[allow(dead_code)]
struct Vigia {
    texto: u64,
    cpu: u64,
    desde: Instant,
    avisado: bool,
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
    /// Se detuvo por una señal dentro de código sin fuente (libc): al llegar los marcos se
    /// salta al primero del proyecto.
    pub buscar_fuente: bool,
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
    /// Entradas elegibles del menú abierto: (zona, índice en `items`).
    pub items_menu: Vec<(Rect, usize)>,
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
    /// Tamaños de los paneles (mouse, Configuración del entorno → Apariencia → Tamaños de paneles, `.jonesy.toml`).
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
    /// Compilar cada vez que se guarda un .c o .h (Alt+B o el botón de la barra).
    pub compilar_al_guardar: bool,
    /// Se guardó un .c o .h con compilar al guardar: se compila en el próximo tic (así
    /// F6, que también guarda, no compila dos veces).
    compilar_tras_guardar: bool,
    /// Se guardó mientras compilaba: otra vez al terminar.
    recompilar_al_terminar: bool,
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
    /// Compilaciones terminadas: la traducción de una vieja se descarta.
    compilaciones: u64,
    /// Versión de cada buffer analizada (o en análisis) en vivo.
    analizado_en_vivo: HashMap<PathBuf, u64>,
    /// Integraciones cuyo análisis en vivo falló (una versión sin `--stdin`): quedan sólo
    /// al guardar.
    en_vivo_rotas: BTreeSet<String>,

    pub terminal: Option<Deck>,
    pub ejecucion: Option<Deck>,
    /// La pestaña Ejecución corre el programa del alumno (Ctrl+F5): al terminar se mira
    /// si se cortó por una señal.
    pub vigilar_programa: bool,
    /// El último programa se cortó por un error (violación de segmento, abort…): F5 lo
    /// depura sin detenerse en main, hasta la línea del fallo.
    pub ultimo_fallo: Option<&'static jonesy_core::fallos::Fallo>,
    /// Posiciones desde donde se saltó a una definición (Alt+← vuelve).
    pub historial_nav: Vec<(PathBuf, usize)>,
    /// Pestaña Esquema: fila elegida y símbolos del buffer activo (por ruta y versión).
    pub esquema_sel: usize,
    /// Última búsqueda en el proyecto: patrón, resultado y fila elegida.
    pub busqueda_proyecto: Option<(String, jonesy_core::busqueda::Resultado)>,
    pub sel_busqueda: usize,
    esquema_cache: Option<(Option<PathBuf>, u64, Vec<jonesy_core::simbolos::Simbolo>)>,
    /// Cuándo se lanzaron las últimas pruebas (para el SARIF de `[pruebas] sarif`).
    inicio_pruebas: Option<std::time::SystemTime>,
    /// El último aviso de error (para Ayuda → Reportar un problema).
    ultimo_error: Option<String>,
    /// Cómo terminó cada prueba en la última corrida (✓/✗ junto a su `TEST`).
    pub resultados_pruebas: HashMap<String, bool>,
    /// Expresiones vigiladas del depurador y su valor en la última parada.
    pub vigiladas: Vec<(String, Option<Result<String, String>>)>,
    /// El nombre bajo el cursor (o el mouse) mientras se depura y su valor.
    pub valor_cursor: Option<(String, Option<Result<String, String>>)>,
    /// Palabra bajo el mouse en el editor (gana a la del cursor hasta la próxima tecla).
    hover: Option<String>,
    /// El ejercicio de deckard de la consigna (si es uno).
    pub deckard: Option<jonesy_core::deckard::Ejercicio>,
    /// Pistas vistas de cada ejercicio de deckard (id → cuántas).
    pub pistas_vistas: HashMap<String, usize>,
    /// Archivo (relativo al repositorio) cuyos cambios se van a descartar.
    descartar_pendiente: Option<String>,
    /// Los atajos de `[atajos]` del proyecto: (tecla, acción, cómo se escribió).
    pub atajos_usuario: Vec<(crate::atajos::Atajo, Accion, String)>,
    /// Última compilación: cuándo terminó y cuántos errores dio (para avisar al entregar).
    ultima_compilacion: Option<(Instant, usize)>,
    /// Últimas pruebas: cuándo, cuántas pasan, cuántas fallan y si compilaron.
    ultimas_pruebas: Option<(Instant, usize, usize, bool)>,
    /// Correr hasta el cursor sin depuración en curso: dónde detenerse al arrancar.
    hasta_linea: Option<(PathBuf, usize)>,
    /// Push o pull corriendo en la terminal (al terminar se interpreta el resultado).
    git_remoto: Option<OpRemota>,
    /// Ver → Maximizar el panel (Alt+Z): el del foco ocupa todo el cuerpo.
    pub maximizado: bool,
    /// Términos del glosario que menciona la explicación abierta (g los ofrece).
    glosario_visor: Vec<usize>,
    /// Catálogo de reglas de las herramientas (se carga la primera vez que hace falta).
    pub catalogo: Option<Vec<jonesy_core::catalogo::Regla>>,
    cargando_catalogo: bool,
    /// Qué hacer cuando llegue el catálogo.
    pendiente_catalogo: Option<PendienteCatalogo>,
    /// Simulacro de examen en curso.
    pub simulacro: Option<Simulacro>,
    /// Último recordatorio de hacer commit.
    ultimo_recordatorio_commit: Instant,
    pub depuracion: Option<Depuracion>,
    pub breakpoints: BTreeSet<(PathBuf, usize)>,
    /// Marcadores de línea (1-based), como los breakpoints.
    pub marcadores: BTreeSet<(PathBuf, usize)>,
    /// Por ejercicio: archivo de entrada para Ctrl+F5 y argumentos del programa.
    pub entradas: HashMap<String, PathBuf>,
    pub argumentos: HashMap<String, String>,
    /// Archivo donde /usr/bin/time deja la medida de la última ejecución.
    medida: Option<PathBuf>,
    /// Revisar la memoria de un programa que lee del teclado: corre en la pestaña
    /// Ejecución y el informe se lee al terminar.
    informe_memoria: Option<InformeMemoria>,
    /// Vigilancia de un programa que usa la CPU sin mostrar nada (¿ciclo infinito?).
    vigia: Option<Vigia>,

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
        tema::usar(
            tema::Tema::de_nombre(&proyecto.config.interfaz.tema).unwrap_or(tema::Tema::Oscuro),
        );
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
            compilar_al_guardar: false,
            compilar_tras_guardar: false,
            recompilar_al_terminar: false,
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
            compilaciones: 0,
            analizado_en_vivo: HashMap::new(),
            en_vivo_rotas: BTreeSet::new(),
            terminal: None,
            ejecucion: None,
            vigilar_programa: false,
            ultimo_fallo: None,
            historial_nav: vec![],
            esquema_sel: 0,
            busqueda_proyecto: None,
            sel_busqueda: 0,
            esquema_cache: None,
            inicio_pruebas: None,
            ultima_compilacion: None,
            ultimo_error: None,
            resultados_pruebas: HashMap::new(),
            vigiladas: vec![],
            valor_cursor: None,
            hover: None,
            atajos_usuario: vec![],
            descartar_pendiente: None,
            deckard: None,
            pistas_vistas: HashMap::new(),
            ultimas_pruebas: None,
            hasta_linea: None,
            git_remoto: None,
            maximizado: false,
            glosario_visor: vec![],
            catalogo: None,
            cargando_catalogo: false,
            pendiente_catalogo: None,
            simulacro: None,
            ultimo_recordatorio_commit: Instant::now(),
            depuracion: None,
            breakpoints: BTreeSet::new(),
            marcadores: BTreeSet::new(),
            entradas: HashMap::new(),
            argumentos: HashMap::new(),
            medida: None,
            informe_memoria: None,
            vigia: None,
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
        app.consigna_de_deckard();
        app.ofrecer_recuperacion();
        if let Some(min) = op.simulacro.filter(|_| !op.examen) {
            app.empezar_simulacro(min);
        }
        // La primera vez (fuera del examen, y si no hay que recuperar nada antes).
        if op.bienvenida
            && !op.examen
            && app.modal.is_none()
            && !jonesy_core::sesion::bienvenida::vista()
        {
            app.mostrar_bienvenida();
        }
        app.cargar_doc();
        app.refrescar_git();
        let (atajos, errores_atajos) = crate::atajos::cargar(&app.proyecto.config.atajos);
        app.atajos_usuario = atajos;
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
        } else if !errores_atajos.is_empty() {
            app.avisar(
                format!("[atajos] de .jonesy.toml: {}", errores_atajos.join("; ")),
                Nivel::Aviso,
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
            if let Some(t) = s.tema.as_deref().and_then(tema::Tema::de_nombre) {
                tema::usar(t);
            }
            for rel in &s.expandidos {
                self.explorador
                    .expandidos
                    .insert(self.proyecto.raiz.join(rel));
            }
            self.pistas_vistas = s.pistas.iter().cloned().collect();
            for (nombre, activa) in &s.integraciones {
                if let Some(x) = self.integraciones.iter_mut().find(|x| x.nombre == *nombre) {
                    x.activa = *activa;
                }
            }
            self.preview_markdown = s.preview_markdown;
            self.compilar_al_guardar = s.compilar_al_guardar;
            for (rel, linea) in s.breakpoints {
                self.breakpoints
                    .insert((self.proyecto.raiz.join(rel), linea));
            }
            for (rel, linea) in s.marcadores {
                self.marcadores
                    .insert((self.proyecto.raiz.join(rel), linea));
            }
            for (clave, rel) in s.entradas {
                self.entradas.insert(clave, self.proyecto.raiz.join(rel));
            }
            self.argumentos.extend(s.argumentos);
        } else if let Some(p) = jonesy_core::preferencias::cargar_usuario() {
            self.aplicar_preferencias(&p);
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
            // La solución modelo de deckard es del docente: nunca se abre por su cuenta.
            let candidatos: Vec<PathBuf> = self
                .proyecto
                .archivos(5000)
                .into_iter()
                .filter(|p| p.file_name().is_none_or(|n| n != "solucion.c"))
                .collect();
            let consigna = self.consigna.as_ref().map(|c| c.ruta.clone());
            let elegido = jonesy_core::deckard::leer(&raiz)
                .and_then(|e| jonesy_core::deckard::archivo_inicial(&e))
                .or_else(|| {
                    [raiz.join("main.c"), raiz.join("src/main.c")]
                        .into_iter()
                        .find(|p| p.is_file())
                })
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
            compilar_al_guardar: self.compilar_al_guardar,
            breakpoints: self
                .breakpoints
                .iter()
                .map(|(p, l)| (self.proyecto.relativa(p), *l))
                .collect(),
            marcadores: self
                .marcadores
                .iter()
                .map(|(p, l)| (self.proyecto.relativa(p), *l))
                .collect(),
            entradas: self
                .entradas
                .iter()
                .map(|(k, p)| (k.clone(), self.proyecto.relativa(p)))
                .collect(),
            argumentos: self
                .argumentos
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
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
            tema: Some(tema::actual().nombre().into()),
            pistas: self
                .pistas_vistas
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect(),
        };
        let _ = s.guardar(&dir);
    }

    // ------------------------------------------------------------ utilidades

    pub fn avisar(&mut self, texto: impl Into<String>, nivel: Nivel) {
        let texto = texto.into();
        if nivel == Nivel::Error {
            self.ultimo_error = Some(texto.clone());
        }
        self.mensaje = Some((texto, nivel, Instant::now()));
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
        self.lanzar_con(comando, tarea, &[])
    }

    fn lanzar_con(&mut self, comando: &str, tarea: Tarea, entorno: &[(&str, &str)]) -> u64 {
        self.lanzar_con_entrada(comando, tarea, entorno, None)
    }

    fn lanzar_con_entrada(
        &mut self,
        comando: &str,
        tarea: Tarea,
        entorno: &[(&str, &str)],
        entrada: Option<String>,
    ) -> u64 {
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
        proceso::lanzar_con_entrada(id, comando, &self.proyecto.raiz, entorno, entrada, txp);
        id
    }

    pub fn examen_en_pausa(&self) -> bool {
        self.examen
            .as_ref()
            .is_some_and(|e| e.estado.pausado_desde.is_some())
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
        // Un ejecutable, un .o, un .a…: el visor hexadecimal, no el editor.
        if ruta.is_file() && jonesy_core::hex::archivo_binario(&ruta) {
            return self.abrir_pestana_hex(&ruta, activar);
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
        self.consigna_de_deckard();
    }

    /// La consigna del ejercicio de deckard del archivo activo (o de la raíz): cambia al
    /// pasar a un archivo de otro ejercicio (una guía con varios).
    pub fn consigna_de_deckard(&mut self) {
        let desde = self
            .buffers
            .get(self.activo)
            .and_then(|b| b.buf.ruta.clone())
            .unwrap_or_else(|| self.proyecto.raiz.clone());
        let Some(e) = jonesy_core::deckard::buscar_desde(&desde, &self.proyecto.raiz)
            .or_else(|| jonesy_core::deckard::leer(&self.proyecto.raiz))
        else {
            return;
        };
        if self
            .deckard
            .as_ref()
            .is_some_and(|d| d.id == e.id && d.dir == e.dir)
        {
            return;
        }
        let primera = self.deckard.is_none();
        self.deckard = Some(e);
        self.refrescar_consigna_deckard(false);
        if primera {
            self.consigna_visible = true;
        }
    }

    /// Arma la consigna con las pistas vistas hasta ahora.
    fn refrescar_consigna_deckard(&mut self, conservar_desplazamiento: bool) {
        let Some(e) = &self.deckard else { return };
        let vistas = self.pistas_vistas.get(&e.id).copied().unwrap_or(0);
        let ruta = ["enunciado.md", "README.md", "ejercicio.yaml"]
            .iter()
            .map(|n| e.dir.join(n))
            .find(|p| p.is_file())
            .unwrap_or_else(|| e.dir.join("enunciado.md"));
        let desplazamiento = if conservar_desplazamiento {
            self.consigna
                .as_ref()
                .map(|c| c.desplazamiento)
                .unwrap_or(0)
        } else {
            0
        };
        self.consigna = Some(Consigna {
            ruta,
            fuente: jonesy_core::deckard::markdown(e, vistas),
            doc: None,
            desplazamiento,
            enlace: None,
        });
    }

    /// Ayuda → Ver la siguiente pista (o p en la consigna): una pista más del ejercicio de
    /// deckard. En el examen queda en la bitácora.
    fn siguiente_pista(&mut self) {
        let Some(e) = self.deckard.clone() else {
            self.avisar(
                "Esta consigna no tiene pistas (las traen los ejercicios de deckard).",
                Nivel::Info,
            );
            return;
        };
        let vistas = self.pistas_vistas.get(&e.id).copied().unwrap_or(0);
        if e.pistas.is_empty() {
            self.avisar("Este ejercicio no tiene pistas.", Nivel::Info);
            return;
        }
        if vistas >= e.pistas.len() {
            self.avisar("Ya viste todas las pistas de este ejercicio.", Nivel::Info);
            return;
        }
        let n = vistas + 1;
        self.pistas_vistas.insert(e.id.clone(), n);
        self.tel(TelemetryEvent::CommandExec {
            command: format!("pista {n} de {} del ejercicio {}", e.pistas.len(), e.id),
            exit_code: Some(0),
        });
        self.refrescar_consigna_deckard(true);
        self.consigna_visible = true;
        self.guardar_sesion();
        let texto: String = e.pistas[vistas].chars().take(120).collect();
        self.avisar(
            format!("Pista {n} de {}: {texto}", e.pistas.len()),
            Nivel::Info,
        );
    }

    /// Aplica una edición al buffer activo y la registra en la telemetría.
    fn editar(&mut self, f: impl FnOnce(&mut Buffer) -> Option<Delta>) {
        if self.examen_en_pausa() {
            self.avisar(
                "El examen está en pausa: no se puede editar hasta que se reanude.",
                Nivel::Aviso,
            );
            return;
        }
        if self.buf().buf.solo_lectura {
            self.avisar("Archivo de solo lectura.", Nivel::Aviso);
            return;
        }
        let id = self.id_buffer(self.activo);
        let b = self.buf_mut();
        if let Some(d) = f(&mut b.buf) {
            b.ultimo_cambio = Instant::now();
            // Breakpoints y marcadores siguen a su línea si se agregan o quitan líneas.
            let corrimiento =
                d.insertado.matches('\n').count() as isize - d.saltos_borrados as isize;
            if corrimiento != 0 {
                if let Some(ruta) = b.buf.ruta.clone() {
                    let linea = b.buf.texto.char_to_line(d.offset.min(b.buf.largo())) + 1;
                    for marcas in [&mut self.breakpoints, &mut self.marcadores] {
                        *marcas = std::mem::take(marcas)
                            .into_iter()
                            .map(|(p, l)| {
                                if p == ruta && l > linea {
                                    (p, (l as isize + corrimiento).max(linea as isize) as usize)
                                } else {
                                    (p, l)
                                }
                            })
                            .collect();
                    }
                }
            }
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
        if self.proyecto.config.formato.limpiar_espacios {
            if let Some(d) = b.limpiar_espacios_finales() {
                self.tel(TelemetryEvent::TextDelta {
                    buffer_id: id.clone(),
                    offset: d.offset,
                    inserted: d.insertado,
                    deleted_len: d.borrados,
                });
            }
        }
        let b = &mut self.buffers[i].buf;
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
    /// Relanza el servicio de plugins Python (después de agregar o editar uno).
    pub fn recargar_plugins(&mut self) {
        let permitidos = self.proyecto.config.examen.plugins_permitidos.clone();
        let sup = Supervisor::nuevo(
            &self.proyecto.dir_estado().join("plugins"),
            self.examen.is_some().then_some(permitidos.as_slice()),
        );
        self.comandos_plugins.clear();
        let dir = self.proyecto.dir_estado().join("plugins");
        self.diag_plugins.retain(|r, _| !r.starts_with(&dir));
        // Soltar el servicio anterior cierra su canal: su hilo termina y mata al worker.
        self.plugins = sup.hay_plugins().then(|| {
            let tx = self.tx.clone();
            Servicio::iniciar(sup, move |r| {
                let _ = tx.send(Aviso::Plugins(r));
            })
        });
        if let Some(p) = &self.plugins {
            p.pedir(PedidoPlugin::Comandos);
        }
    }

    fn al_guardar(&mut self, ruta: &Path, i: usize) {
        if ruta.starts_with(self.proyecto.dir_estado().join("plugins"))
            && ruta.extension().is_some_and(|e| e == "py")
        {
            self.recargar_plugins();
            self.avisar(
                "Plugin guardado y recargado: sus comandos están en la paleta (Ctrl+K).",
                Nivel::Ok,
            );
            return;
        }
        if ruta.extension().is_some_and(|e| e == "h" || e == "c") && self.doc.is_some() {
            self.reevaluar_doc();
        }
        if self.compilar_al_guardar && ruta.extension().is_some_and(|e| e == "h" || e == "c") {
            self.compilar_tras_guardar = true;
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

    // ------------------------------------------------------------ en vivo y traducciones

    /// Mientras se escribe: tras una pausa, el buffer sin guardar va por la entrada estándar
    /// a las integraciones con `en_vivo` (gaff), que no marcan la línea del cursor ni lo que
    /// todavía no está cerrado. Sus hallazgos reemplazan a los del último guardado.
    fn analizar_en_vivo(&mut self) {
        if self.proyecto.config.integraciones.en_vivo == Some(false) || self.examen_en_pausa() {
            return;
        }
        let Some(ruta) = self.activo_ruta() else {
            return;
        };
        let b = self.buf();
        if !b.buf.modificado() || b.hex.is_some() || b.ultimo_cambio.elapsed() < DEBOUNCE_VIVO {
            return;
        }
        let version = b.buf.version;
        if self.analizado_en_vivo.get(&ruta) == Some(&version)
            || self
                .procesos
                .values()
                .any(|t| matches!(t, Tarea::EnVivo { archivo, .. } if *archivo == ruta))
        {
            return;
        }
        let integraciones: Vec<(String, String)> = self
            .integraciones_activas()
            .filter(|x| {
                !x.en_vivo.is_empty()
                    && x.aplica_a(&ruta)
                    && !self.en_vivo_rotas.contains(&x.nombre)
            })
            .map(|x| (x.nombre.clone(), x.en_vivo.clone()))
            .collect();
        if integraciones.is_empty() {
            return;
        }
        let linea = b.buf.linea_columna().0 + 1;
        let texto = b.buf.contenido();
        self.analizado_en_vivo.insert(ruta.clone(), version);
        let modulo = self
            .proyecto
            .estructura
            .modulo_de(&ruta)
            .map(|m| m.dir.clone());
        for (nombre, plantilla) in integraciones {
            let cmd = jonesy_core::integraciones::expandir(
                &plantilla.replace("{linea}", &linea.to_string()),
                &self.proyecto.raiz,
                Some(&ruta),
                modulo.as_deref(),
                None,
            );
            self.lanzar_con_entrada(
                &cmd,
                Tarea::EnVivo {
                    nombre,
                    archivo: ruta.clone(),
                    version,
                },
                &[],
                Some(texto.clone()),
            );
        }
    }

    /// Cuánto falta para el próximo análisis en vivo del buffer activo (None si no hay uno
    /// pendiente).
    fn espera_en_vivo(&self) -> Option<Duration> {
        let ruta = self.activo_ruta()?;
        let b = self.buf();
        let pendiente = b.buf.modificado()
            && self.analizado_en_vivo.get(&ruta) != Some(&b.buf.version)
            && self.proyecto.config.integraciones.en_vivo != Some(false)
            && self.integraciones_activas().any(|x| {
                !x.en_vivo.is_empty()
                    && x.aplica_a(&ruta)
                    && !self.en_vivo_rotas.contains(&x.nombre)
            });
        pendiente.then(|| DEBOUNCE_VIVO.saturating_sub(b.ultimo_cambio.elapsed()))
    }

    fn fin_en_vivo(
        &mut self,
        nombre: String,
        archivo: PathBuf,
        version: u64,
        codigo: Option<i32>,
        salida: String,
    ) {
        if codigo == Some(127) {
            self.integraciones_ausentes.insert(nombre);
            return;
        }
        // Si siguió escribiendo, el próximo análisis trae lo vigente.
        let vigente = self
            .buffers
            .iter()
            .any(|b| b.buf.ruta.as_deref() == Some(archivo.as_path()) && b.buf.version == version);
        let Some(x) = self
            .integraciones
            .iter()
            .find(|x| x.nombre == nombre)
            .cloned()
        else {
            return;
        };
        match jonesy_core::integraciones::interpretar(&x, &salida, &archivo, &self.proyecto.raiz) {
            Ok(d) if vigente => {
                self.diag_herramientas.insert((nombre, archivo), d);
            }
            Ok(_) => {}
            Err(_) => {
                self.en_vivo_rotas.insert(nombre.clone());
                self.avisar(
                    format!("{nombre} no puede analizar mientras escribís (¿una versión anterior a --stdin?): sigue al guardar."),
                    Nivel::Aviso,
                );
            }
        }
    }

    /// La integración activa que traduce la salida del compilador (daedalus) o la de un
    /// programa con sanitizers (tetsuo).
    fn traductora(&self, sanitizers: bool) -> Option<jonesy_core::integraciones::Integracion> {
        self.integraciones_activas()
            .find(|x| {
                if sanitizers {
                    !x.traducir_sanitizers.is_empty()
                } else {
                    !x.traducir_compilacion.is_empty()
                }
            })
            .cloned()
    }

    /// Escribe `texto` en el estado del proyecto y lanza el `traducir_*` de `x` sobre él.
    fn lanzar_traduccion(
        &mut self,
        x: &jonesy_core::integraciones::Integracion,
        sanitizers: bool,
        texto: &str,
    ) -> bool {
        let dir = self.proyecto.dir_estado();
        let _ = std::fs::create_dir_all(&dir);
        let archivo = dir.join(if sanitizers {
            "salida-sanitizers.txt"
        } else {
            "salida-compilacion.txt"
        });
        if std::fs::write(&archivo, texto).is_err() {
            return false;
        }
        let plantilla = if sanitizers {
            &x.traducir_sanitizers
        } else {
            &x.traducir_compilacion
        };
        let activo = self.activo_ruta();
        let cmd = jonesy_core::integraciones::expandir(
            &plantilla.replace(
                "{salida}",
                &jonesy_core::proyecto::shell_quote(&archivo.to_string_lossy()),
            ),
            &self.proyecto.raiz,
            activo.as_deref(),
            None,
            None,
        );
        let analizado = activo.unwrap_or_else(|| self.proyecto.raiz.clone());
        self.lanzar(
            &cmd,
            Tarea::Traducir {
                integracion: x.nombre.clone(),
                sanitizers,
                analizado,
                compilacion: self.compilaciones,
                texto: if sanitizers {
                    texto.to_string()
                } else {
                    String::new()
                },
            },
        );
        true
    }

    /// La compilación falló: su salida va a `traducir_compilacion` (daedalus translate
    /// --primer-error), sin compilar de nuevo, para explicar el error que causa los demás.
    fn traducir_compilacion(&mut self, salida: &str) {
        if let Some(x) = self.traductora(false) {
            self.lanzar_traduccion(&x, false, salida);
        }
    }

    /// Suma la explicación traducida a los errores de gcc de la misma línea: el resumen
    /// (al final de la línea y en Problemas) si el catálogo de jonesy no lo reconoció, y el
    /// porqué para F1.
    fn fin_traducir_compilacion(&mut self, integracion: &str, analizado: &Path, salida: &str) {
        let Some(x) = self
            .integraciones
            .iter()
            .find(|x| x.nombre == integracion)
            .cloned()
        else {
            return;
        };
        // Sin traducción (la herramienta falló) quedan los errores de gcc como estaban.
        let Ok(traducidos) =
            jonesy_core::integraciones::traduccion(&x, salida, analizado, &self.proyecto.raiz)
        else {
            return;
        };
        for t in traducidos {
            let Some(d) = self.diag_compilador.iter_mut().find(|d| {
                d.origen == jonesy_core::Origen::Compilador
                    && d.linea == t.linea
                    && d.archivo.as_deref().map(jonesy_core::canonica) == t.archivo
            }) else {
                continue;
            };
            d.explicacion = Some(markdown_traduccion(&t, &d.mensaje));
            if d.resumen.is_none() {
                d.resumen = Some(t.mensaje.clone());
            }
            d.categoria = d.categoria.take().or(t.categoria);
            d.enlace = d.enlace.take().or(t.enlace);
            d.sugerencia = d.sugerencia.take().or(t.sugerencia);
        }
    }

    /// Acciones de las integraciones activas que aplican al archivo activo:
    /// (integración, índice de la acción, título).
    pub fn acciones_disponibles(&self) -> Vec<(String, usize, String)> {
        let Some(ruta) = self.activo_ruta() else {
            return vec![];
        };
        let examen = self.examen.is_some();
        self.integraciones_activas()
            .flat_map(|x| {
                x.acciones
                    .iter()
                    .enumerate()
                    .filter(|(_, a)| a.aplica_a(x, &ruta) && (!examen || a.permitida_en_examen()))
                    .map(|(n, a)| (x.nombre.clone(), n, a.titulo.clone()))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// Corre una acción de una herramienta (Alt+H) sobre el archivo activo.
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
        if self.examen.is_some() && !a.permitida_en_examen() {
            self.avisar(
                format!("«{}» no está permitida en modo examen.", a.titulo),
                Nivel::Aviso,
            );
            return;
        }
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
            .filter_map(|t| match t {
                Tarea::Compilar { .. } => Some("compilando".into()),
                Tarea::Integracion { nombre, .. } => Some(nombre.clone()),
                Tarea::AccionHerramienta { integracion, .. } => Some(integracion.clone()),
                Tarea::Verificar => Some("verificando".into()),
                Tarea::Explicar { .. } => Some("explicando".into()),
                Tarea::MemTrace { .. } => Some("mem-trace".into()),
                Tarea::Objetivo { .. } => Some("make".into()),
                Tarea::Formatear { .. } => Some("formateando".into()),
                Tarea::Traducir { integracion, .. } => Some(integracion.clone()),
                // Corre a cada pausa al escribir: en la barra sería un parpadeo.
                Tarea::EnVivo { .. } => None,
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
    /// `d` en el panel de git o Git → Descartar: pide confirmación para volver el archivo
    /// al último commit.
    fn pedir_descartar(&mut self, rel: String) {
        let Some(e) = self.git_estado.clone() else {
            self.avisar("El proyecto no está en un repositorio git.", Nivel::Aviso);
            return;
        };
        let Some(entrada) = e.entradas.iter().find(|x| x.ruta == rel) else {
            self.avisar(
                format!("«{rel}» no tiene cambios desde el último commit."),
                Nivel::Info,
            );
            return;
        };
        if entrada.tipo == jonesy_git::TipoEntrada::SinSeguimiento {
            self.avisar(
                format!("«{rel}» es nuevo (nunca se hizo commit): no hay versión anterior. Si sobra, borralo desde el explorador (Supr)."),
                Nivel::Aviso,
            );
            return;
        }
        self.descartar_pendiente = Some(rel.clone());
        self.modal = Some(Modal::Confirmar {
            titulo: "Descartar cambios".into(),
            mensaje: format!(
                "«{rel}» vuelve a como estaba en el último commit y se pierden sus cambios (si está abierto, Ctrl+Z en el editor los recupera mientras no lo cierres). ¿Descartarlos? (s/n)"
            ),
            que: Confirmacion::Descartar,
        });
    }

    fn descartar_cambios(&mut self) {
        let (Some(rel), Some(repo)) = (self.descartar_pendiente.take(), self.repo.clone()) else {
            return;
        };
        match repo.descartar(&rel) {
            Ok(()) => {
                let ruta = jonesy_core::canonica(&repo.dir.join(&rel));
                self.recargar_desde_disco(&ruta, "Descartar");
                self.avisar(
                    format!("«{rel}» volvió a la versión del último commit."),
                    Nivel::Ok,
                );
            }
            Err(e) => self.avisar(format!("No se pudo descartar: {e}"), Nivel::Error),
        }
        self.refrescar_git();
    }

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

    // ------------------------------------------------------------ construir

    pub fn activo_ruta(&self) -> Option<PathBuf> {
        self.buf().buf.ruta.clone()
    }

    pub fn compilar(&mut self, luego: Luego) {
        let guardado = luego == Luego::Guardado;
        if self
            .procesos
            .values()
            .any(|t| matches!(t, Tarea::Compilar { .. }))
        {
            if guardado {
                self.recompilar_al_terminar = true;
            } else {
                self.avisar("Ya hay una compilación en curso.", Nivel::Aviso);
            }
            return;
        }
        if !guardado {
            self.guardar_todo();
            // Ya compila con todo guardado.
            self.compilar_tras_guardar = false;
        }
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
        // Al guardar no se mueve nada: los errores anteriores quedan hasta el resultado
        // y la barra muestra «compilando…».
        if !guardado {
            self.diag_compilador.clear();
            self.panel_inferior = true;
            self.pestana = Pestana::Salida;
            self.avisar(format!("Compilando: {comando}"), Nivel::Info);
        }
        self.lanzar(&comando, Tarea::Compilar { luego });
    }

    /// Clave del ejercicio activo (su módulo o, si es un archivo suelto, el archivo) para
    /// recordar su entrada y sus argumentos.
    pub fn clave_ejercicio(&self) -> Option<String> {
        let activo = self.activo_ruta()?;
        Some(match self.proyecto.modulo_activo(Some(&activo)) {
            Some(m) => m.rel.clone(),
            None => self.proyecto.relativa(&activo),
        })
    }

    /// Directorio del ejercicio activo (donde están sus casos).
    pub fn dir_ejercicio(&self) -> PathBuf {
        let activo = self.activo_ruta();
        match activo
            .as_deref()
            .and_then(|a| self.proyecto.modulo_activo(Some(a)))
        {
            Some(m) => self.proyecto.raiz.join(&m.rel),
            None => activo
                .as_deref()
                .and_then(Path::parent)
                .map(Path::to_path_buf)
                .unwrap_or_else(|| self.proyecto.raiz.clone()),
        }
    }

    /// El ejecutable del ejercicio y dónde correrlo.
    fn programa_y_dir(&self) -> Option<(PathBuf, PathBuf)> {
        let activo = self.activo_ruta();
        let prog = self.proyecto.programa_a_depurar(activo.as_deref())?;
        let dir = match activo
            .as_deref()
            .and_then(|a| self.proyecto.modulo_activo(Some(a)))
        {
            Some(m) => self.proyecto.raiz.join(&m.rel),
            None => self.proyecto.raiz.clone(),
        };
        Some((prog, dir))
    }

    fn argumentos_de_ejercicio(&self) -> Vec<String> {
        self.clave_ejercicio()
            .and_then(|k| self.argumentos.get(&k))
            .map(|a| jonesy_core::casos::partir_argumentos(a))
            .unwrap_or_default()
    }

    fn ejecutar_en_deck(&mut self) {
        let activo = self.activo_ruta();
        let clave = self.clave_ejercicio().unwrap_or_default();
        let entrada = self.entradas.get(&clave).filter(|p| p.is_file()).cloned();
        let redir = entrada
            .as_ref()
            .map(|e| {
                format!(
                    " < {}",
                    jonesy_core::proyecto::shell_quote(&e.to_string_lossy())
                )
            })
            .unwrap_or_default();
        let args: String = self
            .argumentos_de_ejercicio()
            .iter()
            .map(|a| format!(" {}", jonesy_core::proyecto::shell_quote(a)))
            .collect();
        let propio = !self.proyecto.config.ejecutar.comando.trim().is_empty();
        // Ya compiló (Luego::Ejecutar): se corre sólo el programa, medido con
        // /usr/bin/time si está (tiempo y memoria máxima).
        let directo = (!propio)
            .then(|| self.programa_y_dir())
            .flatten()
            .filter(|(p, _)| p.is_file());
        let cmd = match directo {
            Some((prog, dir)) => {
                let medir = jonesy_core::casos::time_gnu()
                    .map(|t| {
                        let id = ARCHIVO_TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let f = std::env::temp_dir()
                            .join(format!("jonesy-medida-{}-{id}.txt", std::process::id()));
                        let _ = std::fs::remove_file(&f);
                        let m = format!(
                            "{t} -f '%e %M' -o {} ",
                            jonesy_core::proyecto::shell_quote(&f.to_string_lossy())
                        );
                        self.medida = Some(f);
                        m
                    })
                    .unwrap_or_default();
                format!(
                    "cd {} && {medir}{}{args}{redir}",
                    jonesy_core::proyecto::shell_quote(&dir.to_string_lossy()),
                    jonesy_core::proyecto::shell_quote(&prog.to_string_lossy())
                )
            }
            None => {
                let Some(cmd) = self.proyecto.comando_ejecutar(activo.as_deref()) else {
                    self.avisar(
                        "No hay qué ejecutar: abrí un .c o configurá [ejecutar] en .jonesy.toml.",
                        Nivel::Aviso,
                    );
                    return;
                };
                format!("{cmd}{redir}")
            }
        };
        self.correr_en_deck(cmd);
        if let Some(s) = &mut self.simulacro {
            s.ejecuciones += 1;
        }
        self.vigilar_programa = self.ejecucion.is_some();
        self.vigia = None;
        self.ultimo_fallo = None;
        if let Some(e) = entrada {
            self.avisar(
                format!(
                    "Entrada desde {} (Construir → Ejecutar con entrada… la cambia).",
                    self.proyecto.relativa(&e)
                ),
                Nivel::Info,
            );
        }
    }

    /// Construir → Comparar con las salidas esperadas (después de compilar).
    fn correr_casos(&mut self) {
        let dir = self.dir_ejercicio();
        let mut casos = jonesy_core::casos::buscar(&dir);
        if casos.is_empty() && dir != self.proyecto.raiz {
            casos = jonesy_core::casos::buscar(&self.proyecto.raiz);
        }
        if casos.is_empty() {
            self.avisar(
                "No hay casos: creá entrada1.txt con su salida1.txt (o x.in y x.out) junto al ejercicio.",
                Nivel::Aviso,
            );
            return;
        }
        let Some((prog, cwd)) = self.programa_y_dir().filter(|(p, _)| p.is_file()) else {
            self.avisar("No encontré el ejecutable del ejercicio.", Nivel::Error);
            return;
        };
        let args = self.argumentos_de_ejercicio();
        let tx = self.tx.clone();
        let config = self.proyecto.config.casos.clone();
        self.avisar(format!("Corriendo {} caso(s)…", casos.len()), Nivel::Info);
        std::thread::spawn(move || {
            let limites = |c: &jonesy_core::casos::Caso| {
                jonesy_core::casos::Limites::de_config(&config, &c.nombre)
            };
            let r = jonesy_core::casos::correr_todos(&prog, &args, &cwd, &casos, &limites);
            let _ = tx.send(Aviso::Casos(r));
        });
    }

    /// Construir → Ejecutar con entrada…: elegir el archivo de entrada de Ctrl+F5.
    fn elegir_archivo_de_entrada(&mut self) {
        let dir = self.dir_ejercicio();
        let mut casos = jonesy_core::casos::buscar(&dir);
        if casos.is_empty() && dir != self.proyecto.raiz {
            casos = jonesy_core::casos::buscar(&self.proyecto.raiz);
        }
        let mut opciones = vec![OpcionPaleta {
            titulo: "Sin archivo: escribo yo en la terminal".into(),
            atajo: String::new(),
            que: QuePaleta::Entrada(None),
        }];
        for c in casos {
            let primera = std::fs::read_to_string(&c.entrada)
                .ok()
                .and_then(|t| t.lines().next().map(str::to_string))
                .unwrap_or_default();
            opciones.push(OpcionPaleta {
                titulo: format!("{}  ({primera}…)", self.proyecto.relativa(&c.entrada)),
                atajo: String::new(),
                que: QuePaleta::Entrada(Some(c.entrada)),
            });
        }
        self.modal = Some(Modal::Paleta {
            filtro: String::new(),
            sel: 0,
            opciones,
        });
    }

    /// Tras correr el programa: la medida de /usr/bin/time (tiempo y memoria máxima).
    fn medida_de_la_ejecucion(&mut self) -> Option<String> {
        let f = self.medida.take()?;
        let texto = std::fs::read_to_string(&f).ok()?;
        let _ = std::fs::remove_file(&f);
        let (seg, kib) = jonesy_core::casos::leer_medida(&texto)?;
        let mem = if kib >= 1024 {
            format!("{:.1} MiB", kib as f64 / 1024.0)
        } else {
            format!("{kib} KiB")
        };
        Some(format!("Terminó en {seg:.2} s · memoria máxima {mem}."))
    }

    /// Un programa que usa la CPU sin mostrar nada desde hace un rato: ¿ciclo infinito?
    fn vigilar_cuelgue(&mut self) {
        #[cfg(target_os = "linux")]
        {
            let Some(d) = &self.ejecucion else { return };
            let Some(pid) = d.pid() else { return };
            let texto = {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                d.texto().hash(&mut h);
                h.finish()
            };
            let cpu = jonesy_core::proceso::cpu_de_descendientes(pid);
            let ahora = Instant::now();
            let v = self.vigia.get_or_insert(Vigia {
                texto,
                cpu,
                desde: ahora,
                avisado: false,
            });
            if v.texto != texto {
                *v = Vigia {
                    texto,
                    cpu,
                    desde: ahora,
                    avisado: false,
                };
                return;
            }
            let pasado = ahora.duration_since(v.desde).as_secs_f64();
            let usado = cpu.saturating_sub(v.cpu) as f64 / 100.0;
            if !v.avisado && pasado >= 5.0 && usado >= 0.8 * pasado {
                v.avisado = true;
                self.avisar(
                    "El programa usa la CPU sin mostrar nada desde hace 5 s: ¿un ciclo infinito? Shift+F5 lo corta.",
                    Nivel::Aviso,
                );
            }
        }
    }

    /// Corre un comando de shell en la pestaña Ejecución (con E/S interactiva) y deja la
    /// salida visible al terminar.
    pub fn correr_en_deck(&mut self, cmd: String) {
        self.guardar_todo();
        let (interprete, opcion) = proceso::interprete();
        // Con sh se agrega el código de salida al final; cmd sólo corre el comando.
        // Si se cortó por una señal, se explica en español (y el código se conserva para
        // que jonesy lo vea).
        let envoltorio = if interprete == "sh" {
            format!(
                "{cmd}\nc=$?\nprintf '\\n\\033[2m[proceso terminado con código %d]\\033[0m\\n' \"$c\"\n{}exit \"$c\"\n",
                jonesy_core::fallos::explicacion_sh()
            )
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

    /// Copia el reporte HTML de la última traza de mem-trace al proyecto
    /// (`memoria-<programa>.html`) y lo abre en el navegador si hay entorno gráfico; si
    /// no (por SSH), copia la ruta al portapapeles.
    fn exportar_memoria(&mut self) {
        let dir = self.proyecto.dir_estado().join("memtrace");
        let reporte = dir.join("reporte.html");
        if !reporte.is_file() {
            self.avisar(
                "Todavía no hay una traza: depurá con F5 y esperá «cat-scan listo».",
                Nivel::Aviso,
            );
            return;
        }
        let programa = jonesy_core::catscan::Traza::cargar(&dir.join("trace.json"))
            .ok()
            .and_then(|t| {
                Path::new(&t.meta.source)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "programa".into());
        let destino = self.proyecto.raiz.join(format!("memoria-{programa}.html"));
        if let Err(e) = std::fs::copy(&reporte, &destino) {
            self.avisar(format!("No se pudo exportar: {e}"), Nivel::Error);
            return;
        }
        let rel = self.proyecto.relativa(&destino);
        let grafico = cfg!(target_os = "macos")
            || cfg!(windows)
            || std::env::var_os("DISPLAY").is_some()
            || std::env::var_os("WAYLAND_DISPLAY").is_some();
        let abridor = if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(windows) {
            "explorer"
        } else {
            "xdg-open"
        };
        if grafico && jonesy_core::proyecto::en_path(abridor) {
            let _ = std::process::Command::new(abridor)
                .arg(&destino)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
            self.avisar(
                format!("Traza exportada a {rel} y abierta en el navegador."),
                Nivel::Ok,
            );
        } else {
            if self.proyecto.config.interfaz.osc52 {
                sistema::osc52(&destino.to_string_lossy());
            }
            self.avisar(
                format!("Traza exportada a {rel} (ruta copiada al portapapeles)."),
                Nivel::Ok,
            );
        }
        self.refrescar_git();
    }

    pub fn alternar_breakpoint(&mut self, linea: usize) {
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

    /// Examen → Pausar: el docente pausa el cronómetro con su código (el SHA-256 está en
    /// `[examen] pausa_sha256`). Reanudar no pide código: sólo le devuelve tiempo a correr.
    fn pedir_pausa_examen(&mut self) {
        let Some(e) = &self.examen else {
            self.avisar("La pausa sólo existe en modo examen.", Nivel::Aviso);
            return;
        };
        if e.estado.entregado {
            self.avisar("El examen ya se entregó.", Nivel::Info);
        } else if e.estado.pausado_desde.is_some() {
            self.alternar_pausa_examen();
        } else if self.proyecto.config.examen.pausa_sha256.trim().is_empty() {
            self.avisar(
                "Este examen no admite pausas (falta [examen] pausa_sha256 en .jonesy.toml).",
                Nivel::Aviso,
            );
        } else {
            self.modal = Some(Modal::CodigoPausa {
                texto: String::new(),
            });
        }
    }

    fn pausar_examen(&mut self, codigo: &str) {
        let esperado = self
            .proyecto
            .config
            .examen
            .pausa_sha256
            .trim()
            .to_lowercase();
        if jonesy_core::sha256_hex(codigo.trim().as_bytes()) != esperado {
            self.tel(TelemetryEvent::CommandExec {
                command: "jonesy: pausa del examen rechazada (código incorrecto)".into(),
                exit_code: Some(1),
            });
            self.avisar(
                "Código incorrecto: la pausa la autoriza el docente.",
                Nivel::Error,
            );
            return;
        }
        self.alternar_pausa_examen();
    }

    fn alternar_pausa_examen(&mut self) {
        self.guardar_todo();
        let dir = self.proyecto.dir_estado();
        let Some(e) = &mut self.examen else { return };
        e.estado.alternar_pausa(jonesy_core::ahora_unix());
        let _ = e.estado.guardar(&dir);
        let (pausa, total) = (e.estado.pausado_desde.is_some(), e.estado.pausado_s);
        self.tel(TelemetryEvent::CommandExec {
            command: if pausa {
                "jonesy: examen en pausa (autorizada por el docente)".into()
            } else {
                format!("jonesy: examen reanudado ({total} s en pausa en total)")
            },
            exit_code: Some(0),
        });
        if pausa {
            self.avisar(
                "Examen en pausa: el cronómetro no corre y no se puede editar. Examen → Reanudar sigue.",
                Nivel::Aviso,
            );
        } else {
            self.avisar(
                "Examen reanudado: el cronómetro sigue corriendo.",
                Nivel::Ok,
            );
        }
    }

    /// El panel con el foco, para sus atajos (`?`). None donde `?` se escribe (la terminal,
    /// el programa, la consola de gdb).
    fn panel_actual(&self) -> Option<crate::ayuda::Panel> {
        use crate::ayuda::Panel;
        Some(match self.foco {
            Foco::Navegador => match self.navegador? {
                PestanaNav::Archivos => Panel::Archivos,
                PestanaNav::Proyecto => Panel::Proyecto,
                PestanaNav::Esquema => Panel::Esquema,
            },
            Foco::Lateral => match self.lateral? {
                Lateral::Git => Panel::Git,
                Lateral::Reglas => Panel::Reglas,
                _ => Panel::Editor,
            },
            Foco::Inferior => match self.pestana {
                Pestana::Problemas => Panel::Problemas,
                Pestana::Salida => Panel::Salida,
                Pestana::Busqueda => Panel::Busqueda,
                Pestana::Depurador
                    if self
                        .depuracion
                        .as_ref()
                        .is_some_and(|d| d.foco_consola || d.pty.is_some()) =>
                {
                    return None
                }
                Pestana::Depurador => Panel::Depurador,
                Pestana::Terminal | Pestana::Ejecucion => return None,
            },
            Foco::Consigna => Panel::Consigna,
            Foco::Preview => Panel::VistaPrevia,
            Foco::Editor | Foco::Doc => Panel::Editor,
        })
    }

    /// Una ruta escrita por el usuario: `~` es el directorio personal y las relativas son
    /// del proyecto.
    fn ruta_usuario_a_path(&self, texto: &str) -> PathBuf {
        let t = texto.trim();
        match (t.strip_prefix("~/"), std::env::var_os("HOME")) {
            (Some(resto), Some(h)) => PathBuf::from(h).join(resto),
            _ if Path::new(t).is_absolute() => PathBuf::from(t),
            _ => self.proyecto.raiz.join(t),
        }
    }

    /// Las preferencias que se ven ahora, con los manifiestos propios del usuario.
    fn preferencias_actuales(&self) -> jonesy_core::preferencias::Preferencias {
        let s = Sesion {
            quickdoc_visible: self.quickdoc_visible,
            quickdoc_posicion: Some(self.quickdoc_posicion),
            preview_markdown: self.preview_markdown,
            compilar_al_guardar: self.compilar_al_guardar,
            paneles: Some(self.paneles),
            integraciones: self
                .integraciones
                .iter()
                .map(|x| (x.nombre.clone(), x.activa))
                .collect(),
            tema: Some(tema::actual().nombre().into()),
            ..Default::default()
        };
        jonesy_core::preferencias::desde_sesion(
            &s,
            jonesy_core::integraciones::dir_usuario().as_deref(),
        )
    }

    fn exportar_preferencias(&mut self, ruta: &Path) {
        let p = self.preferencias_actuales();
        match jonesy_core::preferencias::escribir(&p, ruta) {
            Ok(()) => self.avisar(
                format!(
                    "Preferencias exportadas en {} ({}).",
                    ruta.display(),
                    p.resumen()
                ),
                Nivel::Ok,
            ),
            Err(e) => self.avisar(format!("No pude exportar: {e}"), Nivel::Error),
        }
    }

    fn importar_preferencias(&mut self, ruta: &Path) {
        match jonesy_core::preferencias::importar(ruta) {
            Ok((p, escritos)) => {
                if !escritos.is_empty() {
                    self.aplicar_config(QueConfig::ReleerIntegraciones);
                }
                self.aplicar_preferencias(&p);
                self.guardar_sesion();
                let extra = if escritos.is_empty() {
                    String::new()
                } else {
                    format!(" Manifiestos instalados: {}.", escritos.join(", "))
                };
                self.avisar(
                    format!("Preferencias importadas: {}.{extra}", p.resumen()),
                    Nivel::Ok,
                );
            }
            Err(e) => self.avisar(format!("No pude importar: {e}"), Nivel::Error),
        }
    }

    /// Aplica preferencias importadas (o las del usuario, en un proyecto sin sesión).
    fn aplicar_preferencias(&mut self, p: &jonesy_core::preferencias::Preferencias) {
        if let Some(t) = p.tema.as_deref().and_then(tema::Tema::de_nombre) {
            tema::usar(t);
        }
        if let Some(v) = p.quickdoc_visible {
            self.quickdoc_visible = v;
        }
        if let Some(v) = p.quickdoc_posicion {
            self.quickdoc_posicion = v;
        }
        if let Some(v) = p.preview_markdown {
            self.preview_markdown = v;
        }
        if let Some(v) = p.compilar_al_guardar {
            self.compilar_al_guardar = v;
        }
        if let Some(t) = p.paneles {
            self.paneles = t.acotar();
        }
        for (nombre, activa) in &p.integraciones {
            if let Some(x) = self.integraciones.iter_mut().find(|x| x.nombre == *nombre) {
                x.activa = *activa;
            }
        }
    }

    /// Abre el visor hexadecimal de un archivo.
    pub fn abrir_hex(&mut self, ruta: &Path) {
        self.abrir_archivo(ruta, true);
    }

    /// Una pestaña con el visor hexadecimal: el buffer queda vacío y de solo lectura
    /// (nunca se escribe sobre el binario).
    fn abrir_pestana_hex(&mut self, ruta: &Path, activar: bool) -> Option<usize> {
        let v = match VisorHex::abrir(ruta) {
            Ok(v) => v,
            Err(e) => {
                self.avisar(
                    format!("No se pudo abrir {}: {e}", ruta.display()),
                    Nivel::Error,
                );
                return None;
            }
        };
        let mut buf = Buffer::desde_texto("");
        buf.ruta = Some(ruta.to_path_buf());
        buf.lenguaje = Lenguaje::Texto;
        buf.solo_lectura = true;
        if self.buffers.len() == 1
            && self.buffers[0].buf.ruta.is_none()
            && !self.buffers[0].buf.modificado()
        {
            self.buffers.clear();
        }
        let mut b = BufferAbierto::nuevo(buf);
        b.hex = Some(Box::new(v));
        self.buffers.push(b);
        let i = self.buffers.len() - 1;
        if activar {
            self.cambiar_buffer(i);
        }
        Some(i)
    }

    /// Alt+I: del .c a su .h y del .h a su .c.
    fn ir_a_pareja(&mut self) {
        let Some(ruta) = self.activo_ruta() else {
            return;
        };
        let es_c = matches!(
            ruta.extension().and_then(|e| e.to_str()),
            Some("c" | "h" | "C" | "H")
        );
        if !es_c {
            self.avisar("Alt+I va del .c a su .h (y del .h al .c).", Nivel::Aviso);
            return;
        }
        match self.proyecto.archivo_pareja(&ruta) {
            Some(p) => {
                self.recordar_posicion();
                self.abrir_archivo(&p, true);
                self.cambiar_foco(Foco::Editor);
            }
            None => {
                let stem = ruta
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let otra = if ruta.extension().is_some_and(|e| e == "h" || e == "H") {
                    "c"
                } else {
                    "h"
                };
                self.avisar(
                    format!("No encontré {stem}.{otra} en el proyecto."),
                    Nivel::Aviso,
                );
            }
        }
    }

    /// Ver → Visor hexadecimal: elegir uno de los binarios del proyecto.
    fn elegir_binario(&mut self) {
        let opciones: Vec<OpcionPaleta> = jonesy_core::hex::binarios(&self.proyecto.raiz, 500)
            .into_iter()
            .map(|r| {
                let tipo = std::fs::read(&r)
                    .map(|d| {
                        jonesy_core::hex::resumen(&r, &d[..d.len().min(4096)])
                            .split(" · ")
                            .take(2)
                            .collect::<Vec<_>>()
                            .join(" · ")
                    })
                    .unwrap_or_default();
                OpcionPaleta {
                    titulo: format!("{} — {tipo}", self.proyecto.relativa(&r)),
                    atajo: String::new(),
                    que: QuePaleta::Hex(r),
                }
            })
            .collect();
        if opciones.is_empty() {
            self.avisar(
                "No hay binarios en el proyecto todavía: F6 compila y deja el ejecutable y los .o.",
                Nivel::Aviso,
            );
            return;
        }
        self.modal = Some(Modal::Paleta {
            filtro: String::new(),
            sel: 0,
            opciones,
        });
    }

    fn buscar_hex(&mut self, v: &mut VisorHex, desde: usize) {
        if v.buscado.is_empty() {
            self.avisar("/ busca texto o bytes (7f 45 4c 46).", Nivel::Info);
            return;
        }
        match jonesy_core::hex::buscar(&v.datos, &v.buscado, desde) {
            Some(p) => {
                v.ir(p);
                v.marca = v.buscado.len();
                self.avisar(
                    format!("Encontrado en 0x{p:x} (n busca el siguiente)."),
                    Nivel::Info,
                );
            }
            None => self.avisar("No aparece en el archivo.", Nivel::Aviso),
        }
    }

    /// Ayuda → Atajos de este panel (o `?` en el panel).
    fn atajos_del_panel(&mut self) {
        let p = match (self.foco, self.pestana) {
            (Foco::Inferior, Pestana::Depurador) => crate::ayuda::Panel::Depurador,
            _ => self.panel_actual().unwrap_or(crate::ayuda::Panel::Editor),
        };
        let (titulo, texto) = crate::ayuda::atajos_panel(p);
        let md = if p == crate::ayuda::Panel::Editor {
            texto.to_string()
        } else {
            format!("# {titulo}\n\n{texto}\nTodos los atajos: Ayuda → Atajos de teclado.\n")
        };
        self.glosario_visor.clear();
        let ancho = self.tam.0.saturating_sub(10).clamp(40, 90);
        self.modal = Some(Modal::Visor {
            titulo: titulo.into(),
            lineas: markdown::renderizar(&md, ancho, &self.sim).lineas,
            desplazamiento: 0,
        });
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
                self.compilaciones += 1;
                let errores = self
                    .diag_compilador
                    .iter()
                    .filter(|d| d.severidad == Severidad::Error)
                    .count();
                if codigo != Some(0) && errores > 0 {
                    self.traducir_compilacion(&salida);
                }
                let advertencias = self.diag_compilador.len() - errores;
                self.ultima_compilacion = Some((
                    Instant::now(),
                    if codigo == Some(0) { 0 } else { errores.max(1) },
                ));
                let guardado = luego == Luego::Guardado;
                if std::mem::take(&mut self.recompilar_al_terminar) {
                    self.compilar_tras_guardar = true;
                }
                if let Some(s) = self.simulacro.as_mut().filter(|_| !guardado) {
                    s.compilaciones += 1;
                    if codigo != Some(0) {
                        s.con_errores += 1;
                    }
                }
                if codigo == Some(0) {
                    self.salida.push((
                        format!("[compilación correcta, {advertencias} advertencia(s)]"),
                        false,
                    ));
                    self.avisar(
                        if guardado {
                            format!("Guardado y compila ({advertencias} advertencias).")
                        } else {
                            format!("Compilación correcta ({advertencias} advertencias).")
                        },
                        Nivel::Ok,
                    );
                    if advertencias > 0 && luego == Luego::Nada {
                        self.pestana = Pestana::Problemas;
                    }
                    match luego {
                        Luego::Ejecutar => self.ejecutar_en_deck(),
                        Luego::Casos => self.correr_casos(),
                        Luego::Depurar => self.iniciar_depuracion(),
                        Luego::Nada | Luego::Guardado => {}
                    }
                } else {
                    self.salida.push((
                        format!("[la compilación falló: código {}]", codigo.unwrap_or(-1)),
                        true,
                    ));
                    if errores > 0 {
                        let primero = self
                            .primer_error()
                            .map(|(a, l, _)| {
                                format!(
                                    " Empezá por el primero: {}:{l}.",
                                    self.proyecto.relativa(&a)
                                )
                            })
                            .unwrap_or_default();
                        if guardado {
                            // Sin mover el foco: los errores se ven en el margen y Alt+N
                            // va al siguiente.
                            self.avisar(format!("Guardado, pero no compila: {errores} error(es).{primero} Alt+N va al error."), Nivel::Error);
                        } else {
                            self.avisar(format!("La compilación falló: {errores} error(es).{primero} Enter en Problemas salta."), Nivel::Error);
                            self.pestana = Pestana::Problemas;
                            self.sel_problema = 0;
                        }
                    } else if guardado {
                        self.avisar(
                            "Guardado, pero no compila: mirá la pestaña Salida.",
                            Nivel::Error,
                        );
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
            Tarea::Traducir {
                integracion,
                sanitizers,
                analizado,
                compilacion,
                texto,
            } => {
                if sanitizers {
                    self.fin_traducir_sanitizers(&integracion, &analizado, &texto, &salida);
                } else if compilacion == self.compilaciones {
                    self.fin_traducir_compilacion(&integracion, &analizado, &salida);
                }
            }
            Tarea::EnVivo {
                nombre,
                archivo,
                version,
            } => self.fin_en_vivo(nombre, archivo, version, codigo, salida),
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
                herramienta,
                hallazgo,
            } => {
                // Con el formato de los errores de gcc si se entiende la salida; si no,
                // tal cual.
                if let Some(r) = (codigo == Some(0))
                    .then(|| jonesy_core::explicacion::desde_salida(&herramienta, &regla, &salida))
                    .flatten()
                {
                    let texto = jonesy_core::catalogo::markdown_hallazgo(
                        &r,
                        Some(&hallazgo),
                        enlace.as_deref(),
                    );
                    self.visor_explicacion(format!("Regla {regla}"), &texto);
                    return;
                }
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
                            ratatui::style::Style::default().fg(tema::nota()),
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
            Tarea::Objetivo { comando, lectura } => {
                self.cargar_doc();
                self.tel(TelemetryEvent::CommandExec {
                    command: comando.clone(),
                    exit_code: codigo,
                });
                if lectura == Lectura::Pruebas {
                    let raiz = self.proyecto.raiz.clone();
                    let r = jonesy_core::pruebas::interpretar(&salida, &raiz);
                    // Sin p1_test: SARIF (otros frameworks), en la salida o en el archivo
                    // de [pruebas] sarif.
                    let archivo = self.proyecto.config.pruebas.sarif.trim().to_string();
                    let r = if r.reconocida {
                        Some(r)
                    } else {
                        jonesy_core::pruebas::desde_sarif(
                            &salida,
                            (!archivo.is_empty()).then(|| Path::new(&archivo)),
                            self.inicio_pruebas,
                            &raiz,
                        )
                    };
                    if let Some(r) = r {
                        self.fin_pruebas(r, codigo);
                        return;
                    }
                }
                if lectura == Lectura::Memoria {
                    let mut r = jonesy_core::valgrind::interpretar(&salida, &self.proyecto.raiz);
                    if !r.reconocida {
                        // Sin valgrind: el programa compilado con AddressSanitizer (o un
                        // `make memcheck` con sanitizers). Sin informe y terminado bien,
                        // no hubo errores.
                        r = jonesy_core::asan::interpretar(&salida, &self.proyecto.raiz);
                        r.reconocida |=
                            codigo == Some(0) && !jonesy_core::proyecto::en_path("valgrind");
                    }
                    if r.reconocida {
                        self.fin_memoria(r, &salida);
                        return;
                    }
                }
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
            Respuesta::ErroresPropios(d) => {
                let dir = self.proyecto.dir_estado().join("plugins");
                self.diag_plugins.retain(|r, _| !r.starts_with(&dir));
                if let Some(primero) = d.first() {
                    self.avisar(format!("{} (ver Problemas)", primero.mensaje), Nivel::Error);
                }
                for x in d {
                    if let Some(r) = x.archivo.clone() {
                        self.diag_plugins.entry(r).or_default().push(x);
                    }
                }
            }
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
                let en_linea = self.diagnosticos_linea(&r, l);
                // Primero los errores de gcc explicados (lo que impide compilar).
                en_linea
                    .iter()
                    .find(|d| d.resumen.is_some())
                    .or_else(|| en_linea.iter().find(|d| d.codigo.is_some()))
                    .map(|d| (*d).clone())
            }),
        };
        if let Some(e) = d
            .as_ref()
            .filter(|d| d.resumen.is_some())
            .and_then(|d| jonesy_core::errores_gcc::explicar(&d.mensaje))
        {
            let mut texto = e.texto.clone();
            if let Some((_, a)) = self.arreglo_de(d.as_ref()) {
                texto.push_str(&format!(
                    "\n**Arreglo rápido:** Alt+Enter en la línea hace esto: {}.\n",
                    a.descripcion
                ));
            }
            self.visor_explicacion("Error de compilación".into(), &texto);
            return;
        }
        // Un error de gcc que el catálogo no reconoce, explicado por daedalus.
        if let Some(t) = d
            .as_ref()
            .filter(|d| d.codigo.is_none())
            .and_then(|d| d.explicacion.clone())
        {
            self.visor_explicacion("Error de compilación".into(), &t);
            return;
        }
        let Some(d) = d.filter(|d| d.codigo.is_some()) else {
            self.abrir_paleta();
            return;
        };
        // Primero el catálogo de la herramienta (mismo formato que los errores de gcc).
        match &self.catalogo {
            Some(_) => {
                if self.explicar_desde_catalogo(&d) {
                    return;
                }
            }
            None if self.hay_catalogos() => {
                self.pendiente_catalogo = Some(PendienteCatalogo::Explicar(Box::new(d)));
                self.cargar_catalogo();
                self.avisar("Buscando la regla en el catálogo…", Nivel::Info);
                return;
            }
            None => {}
        }
        self.explicar_con_herramienta(d);
    }

    fn resolver_pendiente_catalogo(&mut self, p: PendienteCatalogo) {
        match p {
            PendienteCatalogo::Lista => self.abrir_catalogo(),
            PendienteCatalogo::Explicar(d) => {
                let explicada = self.explicar_desde_catalogo(&d);
                if !explicada {
                    self.explicar_con_herramienta(*d);
                }
            }
        }
    }

    /// F1 desde el catálogo cargado; false si la regla no está.
    fn explicar_desde_catalogo(&mut self, d: &Diagnostico) -> bool {
        let codigo = d.codigo.clone().unwrap_or_default();
        let Some(r) = self.catalogo.as_ref().and_then(|c| {
            jonesy_core::catalogo::buscar(c, d.herramienta.as_deref(), &codigo).cloned()
        }) else {
            return false;
        };
        let mut r = r;
        if r.sugerencia.is_empty() {
            r.sugerencia = d.sugerencia.clone().unwrap_or_default();
        }
        let texto =
            jonesy_core::catalogo::markdown_hallazgo(&r, Some(&d.mensaje), d.enlace.as_deref());
        self.visor_explicacion(format!("Regla {}", r.codigo), &texto);
        true
    }

    /// ¿Alguna integración disponible declara un catálogo?
    fn hay_catalogos(&self) -> bool {
        self.integraciones
            .iter()
            .any(|x| !x.catalogo.is_empty() && x.disponible())
    }

    /// Carga en otro hilo los catálogos de las integraciones disponibles.
    fn cargar_catalogo(&mut self) {
        if self.cargando_catalogo || self.catalogo.is_some() {
            return;
        }
        self.cargando_catalogo = true;
        let comandos: Vec<(String, String)> = self
            .integraciones
            .iter()
            .filter(|x| !x.catalogo.is_empty() && x.disponible())
            .map(|x| (x.nombre.clone(), x.catalogo.clone()))
            .collect();
        let raiz = self.proyecto.raiz.clone();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let mut reglas = vec![];
            for (nombre, cmd) in comandos {
                let (prog, op) = proceso::interprete();
                if let Ok(o) = std::process::Command::new(prog)
                    .arg(op)
                    .arg(&cmd)
                    .current_dir(&raiz)
                    .env("NO_COLOR", "1")
                    .stdin(std::process::Stdio::null())
                    .output()
                {
                    reglas.extend(jonesy_core::catalogo::desde_json(
                        &nombre,
                        &String::from_utf8_lossy(&o.stdout),
                    ));
                }
            }
            let _ = tx.send(Aviso::Catalogo(reglas));
        });
    }

    /// Ayuda → Reglas de la cátedra: el catálogo navegable.
    pub fn abrir_catalogo(&mut self) {
        let Some(c) = &self.catalogo else {
            if !self.hay_catalogos() {
                self.avisar(
                    "No hay herramientas con catálogo instaladas (gaff, kaneda, spunkmeyer).",
                    Nivel::Aviso,
                );
                return;
            }
            self.pendiente_catalogo = Some(PendienteCatalogo::Lista);
            self.cargar_catalogo();
            self.avisar("Cargando el catálogo de reglas…", Nivel::Info);
            return;
        };
        if c.is_empty() {
            self.avisar("Las herramientas no devolvieron su catálogo.", Nivel::Aviso);
            return;
        }
        let opciones = c
            .iter()
            .enumerate()
            .map(|(i, r)| OpcionPaleta {
                titulo: format!("{} · {}", r.codigo, r.titulo),
                atajo: r.herramienta.clone(),
                que: QuePaleta::Regla(i),
            })
            .collect();
        self.modal = Some(Modal::Paleta {
            filtro: String::new(),
            sel: 0,
            opciones,
        });
    }

    /// F1 con el comando `explain` de la herramienta (o lo que trae el hallazgo).
    fn explicar_con_herramienta(&mut self, d: Diagnostico) {
        let codigo = d.codigo.clone().unwrap_or_default();
        let herramienta = d.herramienta.clone().unwrap_or_else(|| "ripley".into());
        let plantilla = self
            .integraciones
            .iter()
            .find(|x| x.nombre == herramienta)
            .map(|x| x.explicar.clone())
            .unwrap_or_default();
        if plantilla.is_empty() {
            // Sin comando para explicar: lo que trae el hallazgo, con el mismo formato.
            let r = jonesy_core::catalogo::Regla {
                herramienta: herramienta.clone(),
                codigo: codigo.clone(),
                titulo: d.mensaje.clone(),
                descripcion: d.explicacion.clone().unwrap_or_default(),
                sugerencia: d.sugerencia.clone().unwrap_or_default(),
                categoria: d.categoria.clone().unwrap_or_default(),
                ..Default::default()
            };
            if let Some(e) = &d.enlace {
                if self.proyecto.config.interfaz.osc52 {
                    sistema::osc52(e);
                }
            }
            let texto = jonesy_core::catalogo::markdown_hallazgo(&r, None, d.enlace.as_deref());
            self.visor_explicacion(format!("Regla {codigo}"), &texto);
            return;
        }
        let cmd = jonesy_core::integraciones::expandir(
            &plantilla,
            &self.proyecto.raiz,
            None,
            None,
            Some(&codigo),
        );
        // Sin colores y sin cortar renglones, para entender la salida.
        self.lanzar_con(
            &cmd,
            Tarea::Explicar {
                codigo,
                enlace: d.enlace.clone(),
                herramienta,
                hallazgo: d.mensaje.clone(),
            },
            &[("NO_COLOR", "1"), ("COLUMNS", "200")],
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
            bienvenida: false,
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
        if !self.proyecto_usa_doc() {
            return None;
        }
        match self.estado_doc {
            EstadoDoc::AlDia => None,
            EstadoDoc::Ausente => Some("sin doc"),
            EstadoDoc::Desactualizada(_) => Some("doc vieja"),
            EstadoDoc::Invalida(_) => Some("doc inválida"),
        }
    }

    /// Si conviene avisar sobre la documentación en este proyecto: lo dice
    /// `[documentacion] aviso` o, sin indicarlo, que el proyecto la use (algún objetivo
    /// doc, un corbel.toml o la carpeta doc/). Tener corbel instalado no alcanza.
    pub fn proyecto_usa_doc(&self) -> bool {
        if let Some(a) = self.proyecto.config.documentacion.aviso {
            return a;
        }
        let dir = self.ruta_api();
        self.proyecto
            .estructura
            .modulos
            .iter()
            .any(|m| m.tiene("doc"))
            || self.proyecto.raiz.join("corbel.toml").is_file()
            || dir.parent().is_some_and(|d| d.is_dir())
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
            PestanaNav::Esquema => self.revelar_en_esquema(),
        }
    }

    /// Símbolos del buffer activo (se recalculan sólo si cambió).
    pub fn simbolos_activos(&mut self) -> Vec<jonesy_core::simbolos::Simbolo> {
        let b = &self.buf().buf;
        let clave = (b.ruta.clone(), b.version);
        if let Some((r, v, s)) = &self.esquema_cache {
            if (r, *v) == (&clave.0, clave.1) {
                return s.clone();
            }
        }
        let s = if b.lenguaje == jonesy_core::buffer::Lenguaje::C {
            jonesy_core::simbolos::de_texto(&b.contenido())
        } else {
            vec![]
        };
        self.esquema_cache = Some((clave.0, clave.1, s.clone()));
        s
    }

    /// Elige en el esquema el símbolo donde está el cursor.
    fn revelar_en_esquema(&mut self) {
        let linea = self.buf().buf.linea_columna().0;
        let s = self.simbolos_activos();
        self.esquema_sel = s.iter().rposition(|x| x.linea <= linea).unwrap_or(0);
    }

    /// Salta al símbolo `i` del esquema.
    fn ir_a_simbolo(&mut self, i: usize, foco_editor: bool) {
        let s = self.simbolos_activos();
        let Some(x) = s.get(i) else { return };
        self.esquema_sel = i;
        let b = &mut self.buf_mut().buf;
        let pos = b.linea_columna_a_pos(x.linea, x.columna);
        b.mover_a(pos, false);
        centrar(b, 8);
        if foco_editor {
            self.cambiar_foco(Foco::Editor);
            if self.vista == Vista::Navegador {
                self.vista = Vista::Codigo;
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
        self.correr_en_panel_con(comando, Lectura::Make);
    }

    fn correr_en_panel_con(&mut self, comando: String, lectura: Lectura) {
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
                lectura,
            },
        );
    }

    /// Abre el menú `indice`. Los comandos de los plugins se piden al worker recién la
    /// primera vez que hacen falta (como en la paleta): llegan enseguida y el menú se
    /// redibuja.
    pub fn abrir_menu(&mut self, indice: usize) {
        self.modal = Some(Modal::Menu { indice, sel: 0 });
        let es_plugins = accion::menus(self.examen.is_some())
            .get(indice)
            .is_some_and(|m| m.items.contains(&accion::Entrada::ComandosPlugins));
        if es_plugins && self.comandos_plugins.is_empty() {
            if let Some(p) = &self.plugins {
                p.pedir(PedidoPlugin::Comandos);
            }
        }
    }

    /// Los menús de la barra, con las entradas del proyecto en el lugar de
    /// `Entrada::ComandosPlugins` (menú Plugins).
    pub fn menus(&self) -> Vec<accion::Menu> {
        let mut menus = accion::menus(self.examen.is_some());
        let (entradas, _) = self.entradas_plugins();
        for m in &mut menus {
            if let Some(i) = m
                .items
                .iter()
                .position(|e| *e == accion::Entrada::ComandosPlugins)
            {
                m.items.splice(i..=i, entradas.clone());
            }
        }
        menus
    }

    /// Entradas del menú Plugins y a qué lleva cada `Externa` (por su `id`): los comandos
    /// de los plugins Python y las acciones de las herramientas para el archivo activo.
    pub fn entradas_plugins(&self) -> (Vec<accion::Entrada>, Vec<QuePaleta>) {
        use accion::Entrada;
        let mut e = vec![Entrada::Nota("Plugins Python".into())];
        let mut destinos = vec![];
        for c in &self.comandos_plugins {
            e.push(Entrada::Externa {
                titulo: format!("  {}", c.titulo()),
                atajo: String::new(),
                id: destinos.len(),
            });
            destinos.push(QuePaleta::Plugin(c.clone()));
        }
        if self.comandos_plugins.is_empty() {
            e.push(Entrada::Nota(if self.plugins.is_some() {
                "  (sus comandos todavía no llegaron)".into()
            } else {
                "  (ninguno en .jonesy/plugins)".into()
            }));
        }
        e.push(Entrada::Separador);
        e.push(Entrada::Nota("Herramientas para este archivo".into()));
        let acciones = self.acciones_disponibles();
        if acciones.is_empty() {
            e.push(Entrada::Nota(if self.activo_ruta().is_some() {
                "  (ninguna para este archivo)".into()
            } else {
                "  (abrí un archivo)".into()
            }));
        }
        for (nombre, n, titulo) in acciones {
            e.push(Entrada::Externa {
                titulo: format!("  {nombre}: {titulo}"),
                atajo: String::new(),
                id: destinos.len(),
            });
            destinos.push(QuePaleta::Herramienta(nombre, n));
        }
        (e, destinos)
    }

    /// Ejecuta una entrada de menú.
    fn elegir_entrada(&mut self, e: accion::Entrada) {
        match e {
            accion::Entrada::Accion(a) => self.accion(a),
            accion::Entrada::Externa { id, .. } => {
                if let Some(q) = self.entradas_plugins().1.get(id).cloned() {
                    self.ejecutar_opcion(q);
                }
            }
            _ => {}
        }
    }

    /// Filas de una solapa de Archivo → Configuración del entorno.
    pub fn filas_configuracion(&self, pestana: usize) -> Vec<FilaConfig> {
        let fila = |titulo: &str, valor: String, detalle: String, que| FilaConfig {
            titulo: titulo.to_string(),
            valor,
            detalle,
            que,
        };
        let examen = self.examen.is_some();
        match pestana {
            0 => {
                let mut v: Vec<FilaConfig> = self
                    .integraciones
                    .iter()
                    .enumerate()
                    .map(|(k, x)| {
                        let estado = if !x.disponible() {
                            "no instalada"
                        } else if examen && !x.examen {
                            "no en examen"
                        } else if x.activa {
                            "activa"
                        } else {
                            "apagada"
                        };
                        let cuando = match x.cuando {
                            jonesy_core::integraciones::Cuando::Guardar => "al guardar",
                            jonesy_core::integraciones::Cuando::Manual => "a mano",
                        };
                        fila(
                            &x.nombre,
                            estado.into(),
                            format!("{cuando} · {}", x.descripcion),
                            QueConfig::Integracion(k),
                        )
                    })
                    .collect();
                v.push(fila(
                    "Acciones de las herramientas para este archivo…",
                    "Alt+H".into(),
                    "gaff fix, tetsuo, motoko, hal…".into(),
                    QueConfig::Accion(Accion::Herramientas),
                ));
                v.push(fila(
                    "Releer los manifiestos",
                    String::new(),
                    "después de editar .jonesy/integraciones/*.toml".into(),
                    QueConfig::ReleerIntegraciones,
                ));
                v
            }
            1 => {
                let dir = self.proyecto.dir_estado().join("plugins");
                let mut scripts: Vec<PathBuf> = std::fs::read_dir(&dir)
                    .map(|it| {
                        it.filter_map(|e| e.ok().map(|e| e.path()))
                            .filter(|p| p.extension().is_some_and(|e| e == "py"))
                            .collect()
                    })
                    .unwrap_or_default();
                scripts.sort();
                let mut v: Vec<FilaConfig> = scripts
                    .into_iter()
                    .map(|p| {
                        let nombre = p
                            .file_stem()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        let comandos = self
                            .comandos_plugins
                            .iter()
                            .filter(|c| c.plugin == nombre)
                            .count();
                        let errores = self.diag_plugins.get(&p).is_some_and(|d| !d.is_empty());
                        fila(
                            &format!("{nombre}.py"),
                            if errores {
                                "con errores".into()
                            } else {
                                "activo".into()
                            },
                            format!("{comandos} comando(s) en la paleta · Enter lo abre"),
                            QueConfig::Abrir(p),
                        )
                    })
                    .collect();
                if v.is_empty() {
                    v.push(fila(
                        "No hay plugins en .jonesy/plugins",
                        String::new(),
                        String::new(),
                        QueConfig::Nada,
                    ));
                }
                v.push(fila(
                    "Nuevo plugin…",
                    String::new(),
                    "desde una plantilla; guardarlo lo recarga".into(),
                    QueConfig::Accion(Accion::NuevoPlugin),
                ));
                v.push(fila(
                    "Recargar los plugins",
                    String::new(),
                    if examen {
                        "en examen sólo corren los firmados por la cátedra".into()
                    } else {
                        String::new()
                    },
                    QueConfig::Accion(Accion::RecargarPlugins),
                ));
                v
            }
            2 => {
                let activo = self.activo_ruta();
                let rel = activo
                    .as_deref()
                    .map(|a| self.proyecto.relativa(a))
                    .unwrap_or_else(|| "archivo.c".into());
                let compilar = self
                    .proyecto
                    .comando_compilar(activo.as_deref())
                    .unwrap_or_else(|| "(abrí un .c)".into());
                let formato = self
                    .proyecto
                    .comando_formato(&rel)
                    .unwrap_or_else(|| "ni gaff ni clang-format instalados".into());
                let origen = |configurado: bool| {
                    if configurado {
                        "de .jonesy.toml".to_string()
                    } else {
                        "predeterminado según el proyecto".to_string()
                    }
                };
                vec![
                    fila(
                        "Compilar (F6)",
                        compilar,
                        origen(!self.proyecto.config.compilar.comando.trim().is_empty()),
                        QueConfig::EditarConfig,
                    ),
                    fila(
                        "Autoformato",
                        formato,
                        origen(!self.proyecto.config.formato.comando.trim().is_empty()),
                        QueConfig::EditarConfig,
                    ),
                    fila(
                        "Editar .jonesy.toml…",
                        String::new(),
                        "banderas de gcc ([compilar] comando), formateador ([formato] comando)…"
                            .into(),
                        QueConfig::EditarConfig,
                    ),
                    fila(
                        "Editar .clang-format…",
                        String::new(),
                        "estilo de clang-format (si no usás gaff)".into(),
                        QueConfig::EditarClangFormat,
                    ),
                ]
            }
            _ => vec![
                fila(
                    "Tema",
                    tema::actual().nombre().into(),
                    "oscuro · claro · contraste · turbo (Turbo C++) · solarizado · fósforo · daltónico".into(),
                    QueConfig::Tema,
                ),
                fila(
                    "Quick-doc",
                    match self.quickdoc_posicion {
                        jonesy_core::PosicionDoc::Derecha => "a la derecha".into(),
                        jonesy_core::PosicionDoc::Inferior => "abajo".into(),
                    },
                    "Shift+F4".into(),
                    QueConfig::OrientacionDoc,
                ),
                fila(
                    "Tamaños de paneles…",
                    String::new(),
                    "también se arrastran con el mouse".into(),
                    QueConfig::Accion(Accion::TamanosPaneles),
                ),
                fila(
                    "Restablecer disposición de paneles",
                    String::new(),
                    "los tamaños de .jonesy.toml o los predeterminados".into(),
                    QueConfig::Accion(Accion::RestablecerPaneles),
                ),
                fila(
                    "Exportar mis preferencias…",
                    String::new(),
                    "tema, quick-doc, paneles, integraciones y manifiestos propios, a un .toml"
                        .into(),
                    QueConfig::Accion(Accion::ExportarPreferencias),
                ),
                fila(
                    "Importar preferencias…",
                    String::new(),
                    "las de otra máquina; también quedan para los proyectos nuevos".into(),
                    QueConfig::Accion(Accion::ImportarPreferencias),
                ),
            ],
        }
    }

    /// Aplica una fila de la configuración. Devuelve si la pantalla sigue abierta.
    fn aplicar_config(&mut self, que: QueConfig) -> bool {
        match que {
            QueConfig::Integracion(k) => {
                if let Some(x) = self.integraciones.get_mut(k) {
                    x.activa = !x.activa;
                    if !x.activa {
                        let nombre = x.nombre.clone();
                        self.diag_herramientas.retain(|(h, _), _| *h != nombre);
                    }
                }
                true
            }
            QueConfig::ReleerIntegraciones => {
                let estado: Vec<(String, bool)> = self
                    .integraciones
                    .iter()
                    .map(|x| (x.nombre.clone(), x.activa))
                    .collect();
                self.cargar_integraciones();
                for (nombre, activa) in estado {
                    if let Some(x) = self.integraciones.iter_mut().find(|x| x.nombre == nombre) {
                        x.activa = activa;
                    }
                }
                self.integraciones_ausentes.clear();
                true
            }
            QueConfig::Tema => {
                tema::usar(tema::actual().siguiente());
                true
            }
            QueConfig::OrientacionDoc => {
                self.modal = None;
                self.accion(Accion::OrientacionQuickDoc);
                true
            }
            // Acciones que no abren otra pantalla: la configuración sigue abierta.
            QueConfig::Accion(a @ (Accion::RecargarPlugins | Accion::RestablecerPaneles)) => {
                self.accion(a);
                true
            }
            QueConfig::Accion(a) => {
                self.modal = None;
                self.accion(a);
                false
            }
            QueConfig::Abrir(p) => {
                self.modal = None;
                self.abrir_archivo(&p, true);
                false
            }
            QueConfig::EditarConfig => {
                if self.examen.is_some() {
                    self.avisar(
                        "En modo examen no se cambia la configuración del proyecto.",
                        Nivel::Aviso,
                    );
                    return true;
                }
                let ruta = self.proyecto.raiz.join(".jonesy.toml");
                if !ruta.exists() {
                    let _ =
                        std::fs::write(&ruta, include_str!("../../../docs/ejemplo.jonesy.toml"));
                }
                self.modal = None;
                self.abrir_archivo(&ruta, true);
                self.avisar(
                    "Los cambios en .jonesy.toml se aplican al volver a abrir el proyecto.",
                    Nivel::Info,
                );
                false
            }
            QueConfig::EditarClangFormat => {
                let ruta = self.proyecto.raiz.join(".clang-format");
                if !ruta.exists() {
                    let _ = std::fs::write(
                        &ruta,
                        "# Estilo de clang-format (Editar → Autoformatear si no está gaff).\n\
                         BasedOnStyle: LLVM\n\
                         IndentWidth: 4\n\
                         BreakBeforeBraces: Linux\n\
                         AllowShortIfStatementsOnASingleLine: false\n\
                         ColumnLimit: 100\n",
                    );
                }
                self.modal = None;
                self.abrir_archivo(&ruta, true);
                false
            }
            QueConfig::Nada => true,
        }
    }

    /// Construir → Limpiar compilación: `make clean` del módulo (o borrar el ejecutable).
    pub fn limpiar_compilacion(&mut self) {
        let activo = self.activo_ruta();
        match self.proyecto.comando_limpiar(activo.as_deref()) {
            Some(c) => self.correr_en_panel(c),
            None => self.avisar(
                "No hay qué limpiar: el Makefile no tiene un objetivo «clean».",
                Nivel::Aviso,
            ),
        }
    }

    /// La pantalla de bienvenida, con lo que falta del entorno para lo básico (sólo mira
    /// el PATH: es instantáneo; el diagnóstico completo está a una tecla).
    pub fn mostrar_bienvenida(&mut self) {
        let esenciales = [
            (
                "gcc",
                "compilar (F6)",
                "sudo dnf install gcc / sudo apt install gcc",
            ),
            (
                "make",
                "los Makefiles de los TP",
                "sudo dnf install make / sudo apt install make",
            ),
            (
                "gdb",
                "depurar (F5)",
                "sudo dnf install gdb / sudo apt install gdb",
            ),
            (
                "valgrind",
                "revisar la memoria",
                "sudo dnf install valgrind / sudo apt install valgrind",
            ),
            (
                "git",
                "guardar versiones",
                "sudo dnf install git / sudo apt install git",
            ),
        ];
        let faltan = esenciales
            .iter()
            .filter(|(b, _, _)| jonesy_core::doctor::buscar(b).is_none())
            .map(|(b, p, s)| (b.to_string(), p.to_string(), s.to_string()))
            .collect();
        // Por omisión no se repite: la casilla empieza destildada.
        self.modal = Some(Modal::Bienvenida {
            siempre: false,
            faltan,
        });
    }

    /// Abre una explicación en Markdown; si menciona términos del glosario, lo dice al pie
    /// y la tecla g los ofrece.
    pub fn visor_explicacion(&mut self, titulo: String, texto: &str) {
        let terminos = jonesy_core::glosario::en_texto(texto);
        let mut texto = texto.to_string();
        if !terminos.is_empty() {
            let nombres: Vec<String> = terminos
                .iter()
                .map(|i| jonesy_core::glosario::terminos()[*i].termino.clone())
                .collect();
            texto.push_str(&format!("\n**Glosario** (g): {}\n", nombres.join(" · ")));
        }
        self.glosario_visor = terminos;
        let ancho = self.tam.0.saturating_sub(10).clamp(40, 100);
        self.modal = Some(Modal::Visor {
            titulo,
            lineas: markdown::renderizar(&texto, ancho, &self.sim).lineas,
            desplazamiento: 0,
        });
    }

    /// Ayuda → Glosario (o g en una explicación: primero los términos que menciona).
    pub fn abrir_glosario(&mut self) {
        let todos = jonesy_core::glosario::terminos();
        let primero: Vec<usize> = std::mem::take(&mut self.glosario_visor);
        let resto = (0..todos.len()).filter(|i| !primero.contains(i));
        let orden: Vec<usize> = primero.iter().copied().chain(resto).collect();
        let opciones = orden
            .into_iter()
            .map(|i| OpcionPaleta {
                titulo: format!(
                    "{} — {}",
                    todos[i].termino,
                    todos[i].definicion.chars().take(70).collect::<String>()
                ),
                atajo: String::new(),
                que: QuePaleta::Glosario(i),
            })
            .collect();
        self.modal = Some(Modal::Paleta {
            filtro: String::new(),
            sel: 0,
            opciones,
        });
    }

    /// Ayuda → Diagnóstico del entorno: el informe de `jonesy doctor` (con las pruebas
    /// de humo, en otro hilo).
    pub fn diagnostico_entorno(&mut self) {
        let proyecto = jonesy_core::Proyecto::abrir(&self.proyecto.raiz);
        let integraciones = self.integraciones.clone();
        let errores = self.errores_integraciones.clone();
        let tx = self.tx.clone();
        // JONESY_DOCTOR_RAPIDO saltea las pruebas de humo (máquinas lentas, pruebas).
        let humo = std::env::var_os("JONESY_DOCTOR_RAPIDO").is_none();
        std::thread::spawn(move || {
            let informe = jonesy_core::doctor::informe(
                env!("CARGO_PKG_VERSION"),
                jonesy_core::doctor::chequeos(&proyecto, &integraciones, &errores, humo),
            );
            let _ = tx.send(Aviso::Doctor(jonesy_core::doctor::markdown(&informe)));
        });
        self.avisar(
            "Revisando el entorno (compila, depura y revisa un programa de prueba)…",
            Nivel::Info,
        );
    }

    /// Ayuda → Reportar un problema: arma el informe (como `jonesy reporte`), lo guarda en
    /// `.jonesy/` y lo copia.
    pub fn reportar_problema(&mut self) {
        let proyecto = jonesy_core::Proyecto::abrir(&self.proyecto.raiz);
        let integraciones = self.integraciones.clone();
        let errores = self.errores_integraciones.clone();
        let ctx = jonesy_core::reporte::Contexto {
            tamano: Some(self.tam),
            ultimo_error: self.ultimo_error.clone(),
            examen: self.examen.is_some(),
        };
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let r = jonesy_core::reporte::generar(
                env!("CARGO_PKG_VERSION"),
                &proyecto,
                &integraciones,
                &errores,
                &ctx,
            );
            let _ = tx.send(Aviso::Reporte(r));
        });
        self.avisar("Armando el reporte…", Nivel::Info);
    }

    fn mostrar_reporte(&mut self, texto: String) {
        let dir = self.proyecto.dir_estado();
        let ruta = dir.join(format!("reporte-{}.md", jonesy_core::ahora_unix()));
        let guardado = std::fs::create_dir_all(&dir)
            .and_then(|_| std::fs::write(&ruta, &texto))
            .is_ok();
        self.copiar_texto(texto.clone());
        let donde = if guardado {
            format!("Guardado en `{}` y copiado", self.proyecto.relativa(&ruta))
        } else {
            "Copiado".to_string()
        };
        let md = format!(
            "{donde}: pegalo en el aviso del problema (o mandalo a la cátedra).\n\n{texto}"
        );
        let ancho = self.tam.0.saturating_sub(10).clamp(40, 110);
        self.glosario_visor.clear();
        self.modal = Some(Modal::Visor {
            titulo: "Reporte de un problema".into(),
            lineas: markdown::renderizar(&md, ancho, &self.sim).lineas,
            desplazamiento: 0,
        });
    }

    /// Construir → Probar (Ctrl+F6): `make test` del módulo activo, con los fallos de
    /// cada prueba en Problemas.
    pub fn probar(&mut self) {
        let activo = self.activo_ruta();
        let Some(comando) = self.proyecto.comando_probar(activo.as_deref()) else {
            self.avisar(
                "No hay pruebas: el Makefile de este ejercicio (o el de la raíz) no tiene un objetivo «test».",
                Nivel::Aviso,
            );
            return;
        };
        // Un poco antes, por la resolución de las fechas del sistema de archivos.
        self.inicio_pruebas = Some(std::time::SystemTime::now() - Duration::from_secs(2));
        self.correr_en_panel_con(comando, Lectura::Pruebas);
    }

    /// Recordatorio de commit: hay cambios sin confirmar desde hace un rato.
    fn recordar_commit(&mut self) {
        let min = self.proyecto.config.interfaz.recordar_commit_min;
        if self.examen.is_some() || min <= 0.0 {
            return;
        }
        let cada = Duration::from_secs_f64(min * 60.0);
        if self.ultimo_recordatorio_commit.elapsed() < cada {
            return;
        }
        let cambios = self
            .git_estado
            .as_ref()
            .map(|e| e.cantidad_modificados())
            .unwrap_or(0);
        let Some(repo) = self.repo.clone() else {
            return;
        };
        if cambios == 0 {
            return;
        }
        self.ultimo_recordatorio_commit = Instant::now();
        let desde = repo.segundos_desde_ultimo_commit();
        if desde.is_none_or(|s| s as f64 >= min * 60.0) {
            self.avisar(
                format!(
                    "Hay {cambios} archivo(s) cambiados sin commit {}: Alt+G y c para guardar una versión.",
                    match desde {
                        Some(s) if s >= 3600 => format!("desde hace {} h", s / 3600),
                        Some(s) => format!("desde hace {} min", s / 60),
                        None => "(todavía no hay ningún commit)".into(),
                    }
                ),
                Nivel::Aviso,
            );
        }
    }

    /// Empieza un simulacro de `min` minutos.
    pub fn empezar_simulacro(&mut self, min: u64) {
        let min = min.clamp(1, 600);
        self.simulacro = Some(Simulacro {
            inicio: Instant::now(),
            duracion: Duration::from_secs(min * 60),
            avisos: BTreeSet::new(),
            compilaciones: 0,
            con_errores: 0,
            ejecuciones: 0,
            pruebas: None,
        });
        self.avisar(
            format!(
                "Simulacro de {min} min: como en el examen, pero sin entregar. F12 lo termina."
            ),
            Nivel::Info,
        );
    }

    /// Termina el simulacro y muestra el resumen.
    pub fn terminar_simulacro(&mut self) {
        let Some(s) = self.simulacro.take() else {
            self.avisar(
                "No hay un simulacro en curso (Archivo → Simulacro de examen…).",
                Nivel::Aviso,
            );
            return;
        };
        let usado = s
            .inicio
            .elapsed()
            .as_secs()
            .div_ceil(60)
            .min(s.duracion.as_secs() / 60);
        let mut t = format!(
            "# Fin del simulacro\n\n- **Tiempo:** usaste {usado} de {} min.\n- **Compilaciones:** {} ({} con errores).\n- **Ejecuciones:** {}.\n",
            s.duracion.as_secs() / 60,
            s.compilaciones,
            s.con_errores,
            s.ejecuciones
        );
        match s.pruebas {
            Some((p, 0)) => {
                t.push_str(&format!("- **Pruebas:** pasan las {p} (última corrida).\n"))
            }
            Some((p, f)) => t.push_str(&format!(
                "- **Pruebas:** {p} pasan, {f} fallan (última corrida).\n"
            )),
            None => t.push_str("- **Pruebas:** no las corriste (Ctrl+F6).\n"),
        }
        t.push_str(
            "\nEn el examen real (jonesy --exam) los archivos se guardan solos, cada edición queda en la \
             bitácora y **F12 entrega**: el paquete se sella y ya no se puede modificar.\n",
        );
        let ancho = self.tam.0.saturating_sub(10).clamp(40, 90);
        self.modal = Some(Modal::Visor {
            titulo: "Simulacro".into(),
            lineas: markdown::renderizar(&t, ancho, &self.sim).lineas,
            desplazamiento: 0,
        });
    }

    /// Avisos del simulacro (15, 5 y 1 minuto) y fin del tiempo.
    fn tic_simulacro(&mut self) {
        let Some(s) = &mut self.simulacro else { return };
        let resta = s.duracion.saturating_sub(s.inicio.elapsed()).as_secs();
        let mut aviso = None;
        for m in [15u64, 5, 1] {
            if resta <= m * 60 && resta > 0 && s.avisos.insert(m) {
                aviso = Some(format!("Simulacro: quedan {m} minuto(s)."));
            }
        }
        if resta == 0 {
            self.avisar("Se terminó el tiempo del simulacro.", Nivel::Aviso);
            self.terminar_simulacro();
            return;
        }
        if let Some(a) = aviso {
            self.avisar(a, Nivel::Aviso);
        }
    }

    /// Construir → Correr la prueba del cursor (Shift+F6): compila el ejecutable de
    /// pruebas del ejercicio y lo corre con `-k nombre` (filtro de p1_test).
    pub fn probar_aqui(&mut self) {
        let b = &self.buf().buf;
        let Some(nombre) = jonesy_core::pruebas::prueba_en(&b.contenido(), b.linea_columna().0)
        else {
            self.avisar(
                "Poné el cursor dentro de una prueba (TEST(nombre) { … }).",
                Nivel::Aviso,
            );
            return;
        };
        let activo = self.activo_ruta();
        let Some(m) = activo
            .as_deref()
            .and_then(|a| self.proyecto.estructura.modulo_de(a))
            .cloned()
        else {
            self.avisar(
                "Para correr una sola prueba hace falta el Makefile del ejercicio.",
                Nivel::Aviso,
            );
            return;
        };
        let dir = self.proyecto.raiz.join(&m.rel);
        let bin = std::fs::read_to_string(dir.join("Makefile"))
            .map(|t| jonesy_core::pruebas::binario_de_pruebas(&t))
            .unwrap_or_else(|_| "test_bin".into());
        let q = |s: &str| jonesy_core::proyecto::shell_quote(s);
        let comando = format!(
            "make -C {d} {b} && cd {d} && ./{b} -k {n}",
            d = q(&dir.to_string_lossy()),
            b = q(&bin),
            n = q(&nombre)
        );
        self.inicio_pruebas = Some(std::time::SystemTime::now() - Duration::from_secs(2));
        self.correr_en_panel_con(comando, Lectura::Pruebas);
        self.avisar(format!("Corriendo sólo «{nombre}»…"), Nivel::Info);
    }

    /// Depurar → Correr hasta el cursor (Ctrl+F10): sin poner un breakpoint.
    pub fn correr_hasta_cursor(&mut self) {
        let Some(ruta) = self.activo_ruta() else {
            return;
        };
        let linea = self.buf().buf.linea_columna().0 + 1;
        if let Some(d) = &mut self.depuracion {
            if d.gdb.corriendo {
                self.avisar(
                    "El programa está corriendo: esperá a que se detenga.",
                    Nivel::Aviso,
                );
                return;
            }
            let _ = d.gdb.correr_hasta(&ruta, linea);
            return;
        }
        // Sin depuración: arranca y se detiene en la línea del cursor en vez de en main.
        self.hasta_linea = Some((ruta, linea));
        self.compilar(Luego::Depurar);
    }

    /// Depurar → Revisar memoria con valgrind: valgrind sobre las pruebas (`make memcheck`) o sobre
    /// el programa, con cada error y fuga explicados en su línea.
    pub fn revisar_memoria(&mut self) {
        if !jonesy_core::proyecto::en_path("valgrind") {
            self.revisar_memoria_con_asan();
            return;
        }
        let activo = self.activo_ruta();
        if self.revisar_memoria_interactiva(activo.as_deref()) {
            return;
        }
        let Some(comando) = self.proyecto.comando_memoria(activo.as_deref()) else {
            self.avisar(
                "No sé qué revisar: abrí un .c o agregá un objetivo «memcheck» al Makefile.",
                Nivel::Aviso,
            );
            return;
        };
        self.correr_en_panel_con(comando, Lectura::Memoria);
    }

    /// Sin valgrind (Windows, macOS): el programa compilado con AddressSanitizer, si el
    /// compilador lo trae; si no, cat-scan.
    fn revisar_memoria_con_asan(&mut self) {
        let Some(cc) = jonesy_core::asan::compilador() else {
            self.avisar(
                if cfg!(windows) {
                    "En Windows no hay valgrind y el compilador no trae AddressSanitizer (clang de MSYS2 CLANG64 sí). Mientras tanto, cat-scan (F5 y Alt+M) muestra el heap paso a paso."
                } else {
                    "valgrind no está instalado (sudo dnf install valgrind / sudo apt install valgrind) y el compilador no trae AddressSanitizer. Mientras tanto, cat-scan (F5 y Alt+M) muestra el heap paso a paso."
                },
                Nivel::Aviso,
            );
            return;
        };
        let activo = self.activo_ruta();
        let salida = self.proyecto.dir_estado().join("asan");
        let _ = std::fs::create_dir_all(&salida);
        let Some(comando) = activo
            .as_deref()
            .and_then(|a| jonesy_core::asan::comando(&self.proyecto, a, cc, &salida))
        else {
            self.avisar(
                "No sé qué revisar: abrí un .c del programa (el ejercicio con su main.c).",
                Nivel::Aviso,
            );
            return;
        };
        let args: String = self
            .argumentos_de_ejercicio()
            .iter()
            .map(|x| format!(" {}", jonesy_core::proyecto::shell_quote(x)))
            .collect();
        let clave = self.clave_ejercicio().unwrap_or_default();
        let entrada = self.entradas.get(&clave).filter(|p| p.is_file()).cloned();
        let lee = activo
            .as_deref()
            .is_some_and(|a| self.proyecto.programa_lee_entrada(a));
        match entrada {
            Some(e) => {
                let redir = jonesy_core::proyecto::shell_quote(&e.to_string_lossy());
                self.correr_en_panel_con(format!("{comando}{args} < {redir}"), Lectura::Memoria);
            }
            None if lee => {
                self.correr_en_deck(format!("{comando}{args}"));
                if self.ejecucion.is_some() {
                    self.informe_memoria = Some(InformeMemoria::Asan);
                    self.avisar(
                        "El programa corre con AddressSanitizer en la terminal: escribí su entrada; el informe aparece al terminar.",
                        Nivel::Info,
                    );
                }
            }
            None => self.correr_en_panel_con(format!("{comando}{args}"), Lectura::Memoria),
        }
    }

    /// Construir → Ejecutar con sanitizers: el programa compilado con AddressSanitizer y
    /// UndefinedBehaviorSanitizer corre en la pestaña Ejecución (se le puede escribir la
    /// entrada, o va el archivo de Ejecutar con entrada…). El primer acceso inválido o
    /// comportamiento indefinido lo detiene; al terminar, el informe (explicado por tetsuo
    /// si está) queda en Problemas y el cursor en la sentencia que lo causó.
    pub fn ejecutar_con_sanitizers(&mut self) {
        let Some((cc, banderas)) = jonesy_core::asan::compilador_sanitizers() else {
            self.avisar(
                if cfg!(windows) {
                    "El compilador no trae los sanitizers: en Windows, el clang de MSYS2 CLANG64 sí (o WSL). Mientras tanto, Revisar la memoria y cat-scan."
                } else {
                    "El compilador no trae los sanitizers: instalá libasan y libubsan (sudo dnf install libasan libubsan / sudo apt install libasan8 libubsan1)."
                },
                Nivel::Aviso,
            );
            return;
        };
        let activo = self.activo_ruta();
        let salida = self.proyecto.dir_estado().join("asan");
        let _ = std::fs::create_dir_all(&salida);
        let Some(comando) = activo
            .as_deref()
            .and_then(|a| jonesy_core::asan::comando_con(&self.proyecto, a, cc, banderas, &salida))
        else {
            self.avisar(
                "No sé qué ejecutar: abrí un .c del programa (el ejercicio con su main.c).",
                Nivel::Aviso,
            );
            return;
        };
        let args: String = self
            .argumentos_de_ejercicio()
            .iter()
            .map(|x| format!(" {}", jonesy_core::proyecto::shell_quote(x)))
            .collect();
        let clave = self.clave_ejercicio().unwrap_or_default();
        let redir = self
            .entradas
            .get(&clave)
            .filter(|p| p.is_file())
            .map(|e| {
                format!(
                    " < {}",
                    jonesy_core::proyecto::shell_quote(&e.to_string_lossy())
                )
            })
            .unwrap_or_default();
        self.correr_en_deck(format!("{comando}{args}{redir}"));
        if self.ejecucion.is_some() {
            self.informe_memoria = Some(InformeMemoria::Sanitizers);
            self.avisar(
                if banderas.contains("undefined") {
                    "Corre con AddressSanitizer y UBSan: el primer error de memoria o comportamiento indefinido lo detiene y se explica en su línea."
                } else {
                    "Corre con AddressSanitizer: el primer error de memoria lo detiene y se explica en su línea (este compilador no trae UBSan)."
                },
                Nivel::Info,
            );
        }
    }

    /// tetsuo tradujo lo que imprimió el programa con sanitizers.
    fn fin_traducir_sanitizers(
        &mut self,
        integracion: &str,
        analizado: &Path,
        texto: &str,
        salida: &str,
    ) {
        let x = self
            .integraciones
            .iter()
            .find(|x| x.nombre == integracion)
            .cloned();
        let d = x.and_then(|x| {
            jonesy_core::integraciones::traduccion(&x, salida, analizado, &self.proyecto.raiz).ok()
        });
        match d {
            Some(mut d) => {
                for x in &mut d {
                    // Lo que muestra la línea: el título; F1, el porqué y qué hacer.
                    x.resumen = Some(x.mensaje.clone());
                }
                self.fin_sanitizers(d, texto)
            }
            // tetsuo falló: lo que entiende jonesy.
            None => {
                let r = jonesy_core::asan::interpretar(texto, &self.proyecto.raiz);
                self.fin_sanitizers(r.diagnosticos, texto)
            }
        }
    }

    /// El informe de los sanitizers en Problemas, con el cursor en la sentencia que causó el
    /// primer error (la que detuvo al programa).
    fn fin_sanitizers(&mut self, diagnosticos: Vec<Diagnostico>, salida: &str) {
        let mut d = diagnostico::parsear_gcc(salida, &self.proyecto.raiz);
        d.extend(diagnosticos);
        self.diag_compilador = d;
        let errores = self
            .diag_compilador
            .iter()
            .filter(|d| d.severidad == Severidad::Error)
            .count();
        if errores == 0 {
            let texto = "Sanitizers: sin errores de memoria ni comportamiento indefinido.";
            self.salida.push((format!("[{texto}]"), false));
            self.avisar(texto, Nivel::Ok);
            return;
        }
        let causa = self
            .diag_compilador
            .iter()
            .find(|d| d.severidad == Severidad::Error && d.archivo.is_some())
            .map(|d| {
                (
                    d.archivo.clone().unwrap(),
                    d.linea,
                    d.columna,
                    d.resumen.clone(),
                )
            });
        let texto = format!("Sanitizers: {errores} error(es) (mirá Problemas; F1 lo explica).");
        self.salida.push((format!("[{texto}]"), true));
        self.pestana = Pestana::Problemas;
        self.sel_problema = 0;
        if let Some((archivo, linea, columna, resumen)) = causa {
            self.ir_a(&archivo, linea, columna);
            self.foco = Foco::Editor;
            self.avisar(
                format!(
                    "{}:{linea}: {}",
                    self.proyecto.relativa(&archivo),
                    resumen.unwrap_or(texto)
                ),
                Nivel::Error,
            );
        } else {
            self.avisar(texto, Nivel::Error);
        }
    }

    /// valgrind sobre el programa (no `make memcheck`) con sus argumentos y su entrada:
    /// con un archivo de entrada elegido corre en el panel; si el programa lee del teclado
    /// y no hay archivo, en la pestaña Ejecución para poder escribirle (sin teclado
    /// recibiría EOF), y el informe se lee al terminar. false si no corresponde.
    fn revisar_memoria_interactiva(&mut self, activo: Option<&Path>) -> bool {
        let Some(a) = activo else { return false };
        let clave = self.clave_ejercicio().unwrap_or_default();
        let entrada = self.entradas.get(&clave).filter(|p| p.is_file()).cloned();
        if entrada.is_none() && !self.proyecto.programa_lee_entrada(a) {
            return false;
        }
        let id = ARCHIVO_TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let informe =
            std::env::temp_dir().join(format!("jonesy-valgrind-{}-{id}.txt", std::process::id()));
        let _ = std::fs::remove_file(&informe);
        let Some(comando) = self.proyecto.comando_memoria_interactiva(Some(a), &informe) else {
            return false;
        };
        let args: String = self
            .argumentos_de_ejercicio()
            .iter()
            .map(|x| format!(" {}", jonesy_core::proyecto::shell_quote(x)))
            .collect();
        match entrada {
            Some(e) => {
                let Some(comando) = self.proyecto.comando_memoria(Some(a)) else {
                    return false;
                };
                let redir = jonesy_core::proyecto::shell_quote(&e.to_string_lossy());
                self.correr_en_panel_con(format!("{comando}{args} < {redir}"), Lectura::Memoria);
            }
            None => {
                self.correr_en_deck(format!("{comando}{args}"));
                if self.ejecucion.is_some() {
                    self.informe_memoria = Some(InformeMemoria::Valgrind(informe));
                    self.avisar(
                        "El programa corre con valgrind en la terminal: escribí su entrada; el informe aparece al terminar.",
                        Nivel::Info,
                    );
                }
            }
        }
        true
    }

    /// Terminó el programa que corría con valgrind en la terminal: su informe.
    fn fin_memoria_interactiva(&mut self) {
        let Some(informe) = self.informe_memoria.take() else {
            return;
        };
        let texto = self
            .ejecucion
            .as_ref()
            .map(|d| d.texto())
            .unwrap_or_default();
        if matches!(informe, InformeMemoria::Sanitizers) {
            // tetsuo lo explica si está; si no, lo que entiende jonesy (ASan y UBSan).
            match self.traductora(true) {
                Some(x) if self.lanzar_traduccion(&x, true, &texto) => {}
                _ => {
                    let r = jonesy_core::asan::interpretar(&texto, &self.proyecto.raiz);
                    self.fin_sanitizers(r.diagnosticos, &texto);
                }
            }
            return;
        }
        let r = match informe {
            InformeMemoria::Valgrind(archivo) => {
                let log = std::fs::read_to_string(&archivo).unwrap_or_default();
                let _ = std::fs::remove_file(&archivo);
                jonesy_core::valgrind::interpretar(&log, &self.proyecto.raiz)
            }
            InformeMemoria::Sanitizers => unreachable!("se trata antes"),
            InformeMemoria::Asan => {
                let mut r = jonesy_core::asan::interpretar(&texto, &self.proyecto.raiz);
                // Sin informe pero el programa terminó bien: no hubo errores.
                r.reconocida |= texto.contains("[proceso terminado con código 0]");
                r
            }
        };
        if r.reconocida {
            self.fin_memoria(r, &texto);
        } else {
            self.diag_compilador = diagnostico::parsear_gcc(&texto, &self.proyecto.raiz);
            self.avisar(
                "valgrind no llegó a correr el programa: mirá la pestaña Ejecución.",
                Nivel::Error,
            );
        }
    }

    fn fin_memoria(&mut self, r: jonesy_core::valgrind::Resultado, salida: &str) {
        let mut d = diagnostico::parsear_gcc(salida, &self.proyecto.raiz);
        d.extend(r.diagnosticos);
        self.diag_compilador = d;
        let (texto, nivel) = if r.errores == 0 && r.bytes_perdidos == 0 {
            ("Memoria: sin errores ni fugas.".to_string(), Nivel::Ok)
        } else {
            (
                format!(
                    "Memoria: {} error(es), {} bytes perdidos (mirá Problemas).",
                    r.errores, r.bytes_perdidos
                ),
                Nivel::Error,
            )
        };
        self.salida
            .push((format!("[{texto}]"), nivel == Nivel::Error));
        self.avisar(texto, nivel);
        if nivel == Nivel::Error && !self.diag_compilador.is_empty() {
            self.pestana = Pestana::Problemas;
            self.sel_problema = 0;
        }
    }

    fn fin_pruebas(&mut self, r: jonesy_core::pruebas::Resultado, codigo: Option<i32>) {
        if let Some(s) = &mut self.simulacro {
            s.pruebas = Some((r.pasan, r.fallan));
        }
        for (nombre, paso) in &r.por_prueba {
            self.resultados_pruebas.insert(nombre.clone(), *paso);
        }
        self.diag_compilador = r.diagnosticos;
        let compila = !self.diag_compilador.iter().any(|d| {
            d.origen == jonesy_core::Origen::Compilador && d.severidad == Severidad::Error
        });
        self.ultimas_pruebas = Some((
            Instant::now(),
            r.pasan,
            if codigo == Some(0) || r.fallan > 0 {
                r.fallan
            } else {
                r.fallan.max(1)
            },
            compila,
        ));
        let (texto, nivel) = if !compila {
            (
                "Las pruebas no compilan: mirá Problemas.".to_string(),
                Nivel::Error,
            )
        } else if r.fallan == 0 && codigo == Some(0) {
            (format!("Pruebas: pasan las {}.", r.pasan), Nivel::Ok)
        } else {
            (
                format!("Pruebas: {} pasan, {} fallan.", r.pasan, r.fallan),
                Nivel::Error,
            )
        };
        self.salida
            .push((format!("[{texto}]"), nivel == Nivel::Error));
        self.avisar(texto, nivel);
        if nivel == Nivel::Error && !self.diag_compilador.is_empty() {
            self.pestana = Pestana::Problemas;
            self.sel_problema = 0;
        }
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
                .and_then(|_| {
                    std::fs::write(&ruta, plantilla_para(&ruta, &self.proyecto.dir_estado()))
                })
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

    /// Ctrl+R: renombra la variable o función del cursor sólo donde esa declaración vale
    /// (no toca otra variable con el mismo nombre en otra función, ni campos, cadenas o
    /// comentarios).
    fn pedir_renombrar_simbolo(&mut self) {
        let b = &self.buf().buf;
        if b.lenguaje != Lenguaje::C {
            self.avisar(
                "Renombrar en su ámbito funciona en archivos de C.",
                Nivel::Aviso,
            );
            return;
        }
        let pos = b.seleccion().map(|(a, _)| a).unwrap_or(b.cursor);
        match jonesy_core::renombrar::planear(&b.contenido(), pos) {
            Ok(plan) => {
                self.modal = Some(Modal::RenombrarSimbolo {
                    texto: plan.nombre.clone(),
                    plan,
                    error: None,
                })
            }
            Err(e) => self.avisar(e, Nivel::Aviso),
        }
    }

    fn renombrar_simbolo(&mut self, plan: jonesy_core::renombrar::Plan, nuevo: String) {
        let nuevo = nuevo.trim().to_string();
        let fuente = self.buf().buf.contenido();
        if let Err(e) = jonesy_core::renombrar::validar(&fuente, &plan, &nuevo) {
            self.modal = Some(Modal::RenombrarSimbolo {
                plan,
                texto: nuevo,
                error: Some(e),
            });
            return;
        }
        let texto = jonesy_core::renombrar::aplicar(&fuente, &plan, &nuevo);
        // El cursor sigue al mismo lugar del código aunque cambien los largos.
        let cursor = self.buf().buf.cursor;
        let delta = nuevo.chars().count() as isize - plan.nombre.chars().count() as isize;
        let antes = plan.rangos.iter().filter(|(_, f)| *f <= cursor).count() as isize;
        let cursor = (cursor as isize + antes * delta).max(0) as usize;
        self.editar(|b| b.reemplazar_todo(&texto));
        self.buf_mut().buf.mover_a(cursor, false);
        let n = plan.rangos.len();
        self.avisar(
            format!(
                "«{}» → «{nuevo}»: {n} {} en {} (Ctrl+Z deshace).",
                plan.nombre,
                if n == 1 { "cambio" } else { "cambios" },
                plan.alcance
            ),
            Nivel::Ok,
        );
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
            .filter(|a| {
                self.examen.is_some() || !matches!(a, Accion::Entregar | Accion::PausarExamen)
            })
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
        opciones.extend(self.opciones_make("make: "));
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

    /// Los objetivos de todos los Makefile del proyecto: primero los del ejercicio (o la
    /// biblioteca) del archivo activo, después el Makefile general, las bibliotecas y
    /// los demás ejercicios.
    fn opciones_make(&self, prefijo: &str) -> Vec<OpcionPaleta> {
        use jonesy_core::estructura::TipoModulo;
        let activo = self.activo_ruta();
        let actual = activo
            .as_deref()
            .and_then(|r| self.proyecto.modulo_activo(Some(r)))
            .map(|m| m.dir.clone());
        let mut modulos: Vec<(usize, &jonesy_core::estructura::Modulo)> = self
            .proyecto
            .estructura
            .modulos
            .iter()
            .enumerate()
            .collect();
        modulos.sort_by_key(|(i, m)| {
            let grupo = if Some(&m.dir) == actual.as_ref() {
                0
            } else {
                match m.tipo {
                    TipoModulo::Raiz => 1,
                    TipoModulo::Libreria => 2,
                    TipoModulo::Ejercicio => 3,
                    TipoModulo::Otro => 4,
                }
            };
            (grupo, *i)
        });
        let mut v = vec![];
        for (i, m) in modulos {
            let mut nombre = match m.tipo {
                TipoModulo::Raiz => "general".to_string(),
                TipoModulo::Libreria => format!("biblioteca {}", m.nombre),
                _ => m.nombre.clone(),
            };
            if Some(&m.dir) == actual.as_ref() {
                nombre.push_str(" (actual)");
            }
            for o in &m.objetivos {
                let descripcion = if o.descripcion.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", o.descripcion)
                };
                v.push(OpcionPaleta {
                    titulo: format!("{prefijo}{nombre} › {}{descripcion}", o.nombre),
                    atajo: m.comando(&o.nombre),
                    que: QuePaleta::Objetivo(i, o.nombre.clone()),
                });
            }
        }
        v
    }

    /// Construir → Correr un objetivo de make (Shift+F9): la lista de objetivos sola,
    /// para elegir con el teclado.
    fn elegir_objetivo_make(&mut self) {
        let opciones = self.opciones_make("");
        if opciones.is_empty() {
            self.avisar(
                "Este proyecto no tiene Makefile: F6 compila el archivo activo con gcc.",
                Nivel::Aviso,
            );
            return;
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

    /// Recuerda dónde está el cursor (para volver con Alt+←).
    fn recordar_posicion(&mut self) {
        if let Some(r) = self.activo_ruta() {
            let pos = self.buf().buf.cursor;
            if self.historial_nav.last() != Some(&(r.clone(), pos)) {
                self.historial_nav.push((r, pos));
                if self.historial_nav.len() > 50 {
                    self.historial_nav.remove(0);
                }
            }
        }
    }

    /// F12 / Ctrl+clic: la definición de la función, tipo o macro bajo el cursor (en el
    /// buffer, aunque no esté guardado, o en los .c/.h del proyecto).
    pub fn ir_a_definicion(&mut self) {
        let b = &self.buf().buf;
        let Some(nombre) = b.palabra_en(b.cursor).filter(|p| !p.is_empty()) else {
            self.avisar("Poné el cursor sobre un nombre.", Nivel::Aviso);
            return;
        };
        let ruta = b.ruta.clone();
        let texto = b.contenido();
        let actual = ruta.as_deref().map(|r| (r, texto.as_str()));
        match jonesy_core::simbolos::definicion(&nombre, actual, &self.proyecto.raiz) {
            Some((archivo, s)) => {
                self.recordar_posicion();
                self.ir_a(&archivo, s.linea + 1, s.columna + 1);
                self.cambiar_foco(Foco::Editor);
                self.avisar(
                    format!(
                        "{} · {}:{} (Alt+← vuelve)",
                        s.texto,
                        self.proyecto.relativa(&archivo),
                        s.linea + 1
                    ),
                    Nivel::Info,
                );
            }
            None => {
                let base = jonesy_doc::Base::embebida();
                let cabecera = base
                    .funcion(&nombre)
                    .map(|f| f.cabecera.clone())
                    .or_else(|| base.tipo(&nombre).map(|t| t.cabecera.clone()));
                self.avisar(
                    match cabecera {
                        Some(h) => format!(
                            "«{nombre}» es de la biblioteca estándar (<{h}>): el quick-doc (F4) la explica."
                        ),
                        None => format!("No encontré la definición de «{nombre}» en el proyecto."),
                    },
                    Nivel::Aviso,
                );
            }
        }
    }

    /// Alt+←: vuelve a donde se estaba antes de ir a una definición.
    pub fn volver(&mut self) {
        let Some((ruta, pos)) = self.historial_nav.pop() else {
            self.avisar("No hay a dónde volver.", Nivel::Aviso);
            return;
        };
        if self.abrir_archivo(&ruta, true).is_some() {
            let b = &mut self.buf_mut().buf;
            b.mover_a(pos.min(b.largo()), false);
            centrar(b, 8);
            self.cambiar_foco(Foco::Editor);
        }
    }

    /// Busca en todos los archivos y muestra los resultados en la pestaña Búsqueda.
    pub fn buscar_en_proyecto(&mut self, patron: &str, mayusculas: bool, palabra: bool) {
        if patron.is_empty() {
            return;
        }
        // Lo sin guardar también cuenta: se guarda antes (como al compilar).
        self.guardar_todo();
        let r = jonesy_core::busqueda::buscar(&self.proyecto.raiz, patron, mayusculas, palabra);
        let (n, archivos, truncado) = (r.hallazgos.len(), r.archivos, r.truncado);
        self.busqueda_proyecto = Some((patron.to_string(), r));
        self.sel_busqueda = 0;
        self.panel_inferior = true;
        self.pestana = Pestana::Busqueda;
        self.cambiar_foco(Foco::Inferior);
        if self.modo() == Modo::Compacto {
            self.vista = Vista::Terminal;
        }
        self.avisar(
            if n == 0 {
                format!("«{patron}» no aparece en el proyecto.")
            } else {
                format!(
                    "«{patron}»: {n} línea(s) en {archivos} archivo(s){}. Enter abre.",
                    if truncado {
                        " (se muestran las primeras)"
                    } else {
                        ""
                    }
                )
            },
            if n == 0 { Nivel::Aviso } else { Nivel::Info },
        );
    }

    /// Shift+F12: dónde se usa la función o variable del cursor, en todo el proyecto (sin
    /// comentarios ni cadenas). Los resultados van al panel Búsqueda.
    pub fn buscar_referencias(&mut self) {
        let b = &self.buf().buf;
        let Some(nombre) = b.palabra_en(b.cursor).filter(|p| {
            p.chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_')
        }) else {
            self.avisar(
                "Poné el cursor sobre un nombre para ver dónde se usa.",
                Nivel::Aviso,
            );
            return;
        };
        self.guardar_todo();
        let r = jonesy_core::busqueda::referencias(&self.proyecto.raiz, &nombre);
        let (n, archivos) = (r.hallazgos.len(), r.archivos);
        self.busqueda_proyecto = Some((nombre.clone(), r));
        self.sel_busqueda = 0;
        self.panel_inferior = true;
        self.pestana = Pestana::Busqueda;
        self.cambiar_foco(Foco::Inferior);
        if self.modo() == Modo::Compacto {
            self.vista = Vista::Terminal;
        }
        self.avisar(
            if n == 0 {
                format!("«{nombre}» no se usa en los .c y .h del proyecto.")
            } else {
                format!("«{nombre}»: {n} referencia(s) en {archivos} archivo(s). Enter abre.")
            },
            if n == 0 { Nivel::Aviso } else { Nivel::Info },
        );
    }

    /// Editar → Documentar la función: el comentario `/** … */` de la función del cursor,
    /// con el esqueleto de corbel, y el texto del @brief seleccionado para escribirlo.
    fn documentar_funcion(&mut self) {
        let b = &self.buf().buf;
        if b.lenguaje != Lenguaje::C {
            self.avisar("Documentar funciones es para archivos de C.", Nivel::Aviso);
            return;
        }
        let fuente = b.contenido();
        let Some(f) = jonesy_core::documentar::firma_en(&fuente, b.linea_columna().0) else {
            self.avisar(
                "Poné el cursor en una función (o justo arriba).",
                Nivel::Aviso,
            );
            return;
        };
        if jonesy_core::documentar::documentada(&fuente, f.linea) {
            self.avisar(
                format!("«{}» ya tiene su comentario de documentación.", f.nombre),
                Nivel::Info,
            );
            return;
        }
        let texto = jonesy_core::documentar::docblock(&f);
        let pos = b.linea_columna_a_pos(f.linea, 0);
        self.editar(|b| b.reemplazar_rango(pos, pos, &texto));
        // Seleccionado el texto del @brief, para escribir encima.
        if let Some(i) = texto.find("[Descripción breve") {
            let ini = pos + texto[..i].chars().count();
            let largo = texto[i..]
                .find(']')
                .map(|j| texto[i..i + j + 1].chars().count());
            if let Some(largo) = largo {
                let b = &mut self.buf_mut().buf;
                b.mover_a(ini, false);
                b.mover_a(ini + largo, true);
            }
        }
        self.avisar(
            format!(
                "Comentario de «{}»: escribí la descripción; los [ ] que quedan los marca corbel.",
                f.nombre
            ),
            Nivel::Ok,
        );
    }

    /// Construir → Crear una prueba para esta función: el esqueleto de un `TEST` de
    /// p1_test (una variable por parámetro, la llamada y la aserción según lo que
    /// devuelve) en el archivo de pruebas del módulo, con su `RUN_TEST`. Queda sin guardar.
    fn crear_prueba(&mut self) {
        let Some(ruta) = self.activo_ruta() else {
            return;
        };
        let b = &self.buf().buf;
        let Some(f) = jonesy_core::documentar::firma_en(&b.contenido(), b.linea_columna().0)
            .filter(|f| f.nombre != "main" && b.lenguaje == Lenguaje::C)
        else {
            self.avisar(
                "Poné el cursor en una función de C (no en main).",
                Nivel::Aviso,
            );
            return;
        };
        let Some(pruebas) = jonesy_core::esqueleto::archivo_de_pruebas(&self.proyecto, &ruta)
        else {
            self.avisar(
                "No encontré el archivo de pruebas del ejercicio (prueba.c).",
                Nivel::Aviso,
            );
            return;
        };
        if mismo_archivo(&pruebas, &ruta) {
            self.avisar(
                "Estás en el archivo de pruebas: poné el cursor en la función a probar.",
                Nivel::Aviso,
            );
            return;
        }
        if self.abrir_archivo(&pruebas, true).is_none() {
            return;
        }
        let fuente = self.buf().buf.contenido();
        let nombre = jonesy_core::esqueleto::nombre_libre(&fuente, &f.nombre);
        let test = jonesy_core::esqueleto::prueba(&f, &nombre);
        let (texto, linea) = jonesy_core::esqueleto::agregar(&fuente, &test, &nombre);
        self.editar(|b| b.reemplazar_todo(&texto));
        let b = &mut self.buf_mut().buf;
        let pos = b.linea_columna_a_pos(linea, 0);
        b.mover_a(pos, false);
        centrar(b, 4);
        self.cambiar_foco(Foco::Editor);
        self.avisar(
            format!(
                "Prueba «{nombre}» agregada (y su RUN_TEST): completá los valores y lo esperado, y guardá."
            ),
            Nivel::Ok,
        );
    }

    /// Los candidatos para la palabra del cursor (los del archivo y los de la base del
    /// quick-doc). Con `abrir`, abre la lista (o completa directo si hay uno solo); si no,
    /// la actualiza mientras se escribe.
    fn autocompletar(&mut self, abrir: bool) {
        let b = &self.buf().buf;
        let fuente = b.contenido();
        let cursor = b.cursor;
        let chars: Vec<char> = fuente.chars().collect();
        let (inicio, prefijo) = jonesy_core::completar::prefijo(&chars, cursor);
        let base = jonesy_doc::Base::embebida();
        let biblioteca: Vec<(String, String)> = if b.lenguaje == Lenguaje::C {
            base.nombres()
                .filter_map(|n| base.funcion(n).map(|f| (n.to_string(), f.firma.clone())))
                .collect()
        } else {
            vec![]
        };
        let opciones = jonesy_core::completar::candidatos(&fuente, cursor, &biblioteca);
        if opciones.is_empty() || (prefijo.is_empty() && !abrir) {
            self.modal = None;
            if abrir {
                self.avisar(
                    if prefijo.is_empty() {
                        "Escribí el comienzo de un nombre y Ctrl+Espacio lo completa.".to_string()
                    } else {
                        format!("Nada que empiece con «{prefijo}».")
                    },
                    Nivel::Info,
                );
            }
            return;
        }
        if abrir && opciones.len() == 1 && !prefijo.is_empty() {
            let texto = opciones[0].texto.clone();
            self.editar(|b| b.reemplazar_rango(inicio, cursor, &texto));
            return;
        }
        self.modal = Some(Modal::Completar {
            inicio,
            opciones,
            sel: 0,
        });
    }

    /// El arreglo rápido de un diagnóstico de gcc del archivo activo (o del primero de la
    /// línea del cursor que tenga uno).
    fn arreglo_de(
        &self,
        d: Option<&Diagnostico>,
    ) -> Option<(Diagnostico, jonesy_core::arreglos::Arreglo)> {
        let ruta = self.activo_ruta()?;
        let fuente = self.buf().buf.contenido();
        let candidatos: Vec<Diagnostico> = match d {
            Some(d) => vec![d.clone()],
            None => self
                .diagnosticos_linea(&ruta, self.buf().buf.linea_columna().0)
                .into_iter()
                .cloned()
                .collect(),
        };
        candidatos
            .into_iter()
            .filter(|d| d.origen == jonesy_core::Origen::Compilador)
            .find_map(|d| {
                jonesy_core::arreglos::proponer(&fuente, &d.mensaje, d.linea, d.columna)
                    .map(|a| (d, a))
            })
    }

    /// Alt+Enter: aplica el arreglo del error de la línea (falta «;» o un #include, «=» por
    /// «==»). Se deshace con Ctrl+Z; F6 vuelve a compilar para confirmarlo.
    fn arreglo_rapido(&mut self) {
        let Some((d, a)) = self.arreglo_de(None) else {
            let hay = self.activo_ruta().is_some_and(|r| {
                !self
                    .diagnosticos_linea(&r, self.buf().buf.linea_columna().0)
                    .is_empty()
            });
            self.avisar(
                if hay {
                    "Este error no tiene un arreglo automático: F1 lo explica."
                } else {
                    "No hay un error en esta línea (Alt+N va al siguiente)."
                },
                Nivel::Info,
            );
            return;
        };
        self.editar(|b| b.reemplazar_rango(a.inicio, a.fin, &a.texto));
        // El diagnóstico arreglado sale de la lista hasta la próxima compilación.
        self.diag_compilador
            .retain(|x| !(x.mensaje == d.mensaje && x.linea == d.linea && x.archivo == d.archivo));
        self.avisar(
            format!(
                "{} (Ctrl+Z lo deshace; F6 compila de nuevo).",
                a.descripcion
            ),
            Nivel::Ok,
        );
    }

    /// Depurar → Contar malloc y free por función: compila con un envoltorio de malloc y
    /// free, corre el programa (con la entrada elegida para Ctrl+F5) y muestra cuánto pide
    /// y libera cada función y qué quedó sin liberar.
    fn contar_memoria(&mut self) {
        if cfg!(not(target_os = "linux")) {
            self.avisar(
                "El conteo de malloc y free por función es para Linux.",
                Nivel::Aviso,
            );
            return;
        }
        let Some(activo) = self.activo_ruta() else {
            return;
        };
        let Some(fuentes) = jonesy_core::catscan::fuentes_del_programa(&self.proyecto, &activo)
        else {
            self.avisar(
                "Abrí un archivo del ejercicio (que tenga main.c) para contar su memoria.",
                Nivel::Aviso,
            );
            return;
        };
        self.guardar_todo();
        let entrada = self
            .clave_ejercicio()
            .and_then(|c| self.entradas.get(&c).filter(|p| p.is_file()).cloned());
        let args = self.argumentos_de_ejercicio();
        let proyecto = jonesy_core::Proyecto::abrir(&self.proyecto.raiz);
        let dir = self.proyecto.dir_estado().join("conteo");
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let r = jonesy_core::conteo_memoria::contar(
                &proyecto,
                &fuentes,
                entrada.as_deref(),
                &args,
                &dir,
            );
            let _ = tx.send(Aviso::ConteoMemoria(r));
        });
        self.avisar(
            "Compilando y corriendo para contar malloc y free…",
            Nivel::Info,
        );
    }

    /// Depurar → Vigilar: agrega la expresión (y la evalúa si el programa está detenido) o
    /// la quita si ya estaba.
    fn alternar_vigilada(&mut self, expresion: &str) {
        if expresion.is_empty() {
            return;
        }
        if let Some(i) = self.vigiladas.iter().position(|(e, _)| e == expresion) {
            self.vigiladas.remove(i);
            self.avisar(format!("Ya no se vigila «{expresion}»."), Nivel::Info);
            return;
        }
        self.vigiladas.push((expresion.to_string(), None));
        let i = self.vigiladas.len() - 1;
        if let Some(d) = self.depuracion.as_mut().filter(|d| !d.gdb.corriendo) {
            let _ = d.gdb.evaluar(expresion, i);
        }
        self.lateral = Some(Lateral::Variables);
        self.avisar(
            format!("Vigilando «{expresion}»: se recalcula en cada paso (repetirla la quita)."),
            Nivel::Ok,
        );
    }

    /// Con el programa detenido, pide el valor del nombre bajo el cursor (o el mouse) si
    /// cambió (#31). Sale en la barra de estado.
    fn pedir_valor_cursor(&mut self) {
        let Some(d) = self.depuracion.as_mut() else {
            self.valor_cursor = None;
            return;
        };
        if d.gdb.corriendo || d.ubicacion.is_none() {
            return;
        }
        let b = &self.buffers[self.activo].buf;
        let palabra = self
            .hover
            .clone()
            .or_else(|| b.palabra_en(b.cursor))
            .filter(|p| {
                p.chars()
                    .next()
                    .is_some_and(|c| c.is_alphabetic() || c == '_')
                    && !jonesy_core::completar::es_reservada(p)
            });
        let Some(palabra) = palabra else {
            self.valor_cursor = None;
            return;
        };
        if self
            .valor_cursor
            .as_ref()
            .is_some_and(|(n, _)| *n == palabra)
        {
            return;
        }
        let _ = d.gdb.evaluar(&palabra, usize::MAX);
        self.valor_cursor = Some((palabra, None));
    }

    /// Abre el resultado `i` de la búsqueda en el proyecto.
    fn abrir_hallazgo(&mut self, i: usize) {
        let Some(h) = self
            .busqueda_proyecto
            .as_ref()
            .and_then(|(_, r)| r.hallazgos.get(i).cloned())
        else {
            return;
        };
        self.sel_busqueda = i;
        self.recordar_posicion();
        self.ir_a(&h.archivo, h.linea + 1, h.columna + 1);
        let fin = h.rangos[0].1 - h.rangos[0].0;
        let b = &mut self.buf_mut().buf;
        let pos = b.cursor;
        b.mover_a(pos + fin, true);
        b.ancla = Some(pos);
    }

    /// Elige la próxima coincidencia después del cursor (o de la elegida), circular.
    fn siguiente_coincidencia(&mut self, patron: &str, mayusculas: bool, palabra: bool) {
        let b = &mut self.buf_mut().buf;
        let v = b.coincidencias(patron, mayusculas, palabra);
        let desde = b.seleccion().map(|(_, f)| f).unwrap_or(b.cursor);
        match v.iter().find(|(a, _)| *a >= desde).or(v.first()) {
            Some(&(a, z)) => {
                b.mover_a(a, false);
                b.mover_a(z, true);
                centrar(b, 8);
            }
            None => {
                b.mover_a(b.cursor, false);
                self.avisar(format!("No hay más «{patron}»."), Nivel::Aviso);
            }
        }
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
        self.ultimo_fallo.map(|f| f.senal).hash(&mut h);
        self.procesos.len().hash(&mut h);
        self.diagnosticos().len().hash(&mut h);
        self.depuracion
            .as_ref()
            .map(|d| (d.estado.clone(), d.ubicacion.clone(), d.consola.len()))
            .hash(&mut h);
        format!("{:?}{:?}", self.vigiladas, self.valor_cursor).hash(&mut h);
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
        if let Some(t) = self.espera_en_vivo() {
            espera = espera.min(t.max(Duration::from_millis(10)));
        }
        if !self.procesos.is_empty() || self.vigilar_programa {
            espera = espera.min(Duration::from_millis(500));
        }
        espera
    }

    /// Tareas periódicas tras cada lote de eventos.
    pub fn tic(&mut self) {
        // Compilar al guardar; no mientras se depura (el programa en gdb es el anterior).
        if std::mem::take(&mut self.compilar_tras_guardar)
            && self.compilar_al_guardar
            && self.depuracion.is_none()
        {
            self.compilar(Luego::Guardado);
        }
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
        self.analizar_en_vivo();
        if self.vigilar_programa && self.ejecucion.as_mut().is_some_and(|d| d.vivo()) {
            self.vigilar_cuelgue();
        }
        if self.informe_memoria.is_some() && !self.ejecucion.as_mut().is_some_and(|d| d.vivo()) {
            self.fin_memoria_interactiva();
        }
        if self.git_remoto.is_some() && !self.ejecucion.as_mut().is_some_and(|d| d.vivo()) {
            let op = self.git_remoto.take().unwrap();
            self.fin_git_remoto(op);
        }
        self.recordar_commit();
        self.tic_simulacro();
        if self.vigilar_programa && !self.ejecucion.as_mut().is_some_and(|d| d.vivo()) {
            self.vigilar_programa = false;
            self.vigia = None;
            if let Some(m) = self.medida_de_la_ejecucion() {
                self.avisar(m, Nivel::Info);
            }
            let codigo = self.ejecucion.as_mut().and_then(|d| d.codigo_salida());
            if let Some(f) = codigo.and_then(|c| jonesy_core::fallos::de_codigo_salida(c.into())) {
                if f.error {
                    self.ultimo_fallo = Some(f);
                    self.avisar(
                        format!("{} ({}). F5: ver la línea donde falló.", f.titulo, f.senal),
                        Nivel::Error,
                    );
                }
            }
        }
        self.tic_examen();
        self.sondear_git();
        self.eventos_gdb();
        self.pedir_valor_cursor();
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
            Aviso::Casos(resultados) => {
                let total = resultados.len();
                let con = resultados.iter().filter(|r| r.coincide.is_some()).count();
                let bien = resultados
                    .iter()
                    .filter(|r| r.coincide == Some(true))
                    .count();
                self.avisar(
                    format!(
                        "Casos: {bien} de {con} coinciden con la salida esperada ({total} en total)."
                    ),
                    if bien == con { Nivel::Ok } else { Nivel::Error },
                );
                self.modal = Some(Modal::Casos { resultados, sel: 0 });
            }
            Aviso::Catalogo(reglas) => {
                self.cargando_catalogo = false;
                self.catalogo = Some(reglas);
                if let Some(p) = self.pendiente_catalogo.take() {
                    self.resolver_pendiente_catalogo(p);
                }
            }
            Aviso::ConteoMemoria(r) => match r {
                Ok(md) => {
                    let ancho = self.tam.0.saturating_sub(10).clamp(40, 110);
                    self.glosario_visor.clear();
                    self.modal = Some(Modal::Visor {
                        titulo: "Memoria por función".into(),
                        lineas: markdown::renderizar(&md, ancho, &self.sim).lineas,
                        desplazamiento: 0,
                    });
                }
                Err(e) => self.avisar(e, Nivel::Error),
            },
            Aviso::Doctor(texto) => {
                let ancho = self.tam.0.saturating_sub(10).clamp(40, 110);
                self.modal = Some(Modal::Visor {
                    titulo: "Diagnóstico del entorno".into(),
                    lineas: markdown::renderizar(&texto, ancho, &self.sim).lineas,
                    desplazamiento: 0,
                });
            }
            Aviso::Reporte(texto) => self.mostrar_reporte(texto),
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
                | Modal::RenombrarSimbolo { texto, .. }
                | Modal::CodigoPausa { texto }
                | Modal::Vigilar { texto }
                | Modal::RutaPreferencias { texto, .. }
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

    /// Cascada de errores de gcc: el primero de la compilación (archivo y línea) y si hay
    /// más errores en ese archivo después de él (que suelen ser su consecuencia).
    pub fn primer_error(&self) -> Option<(PathBuf, usize, bool)> {
        let errores: Vec<&Diagnostico> = self
            .diag_compilador
            .iter()
            .filter(|d| d.severidad == Severidad::Error && d.archivo.is_some())
            .collect();
        let p = errores.first()?;
        let a = p.archivo.clone()?;
        let mas = errores[1..]
            .iter()
            .any(|d| d.archivo.as_ref() == Some(&a) && d.linea >= p.linea);
        Some((a, p.linea, mas))
    }

    /// Papel de un diagnóstico en una cascada de errores de gcc: `Some(false)` es el
    /// primero («empezá por acá»), `Some(true)` uno posterior en el mismo archivo (puede
    /// ser consecuencia del primero), `None` si no aplica (un solo error, otra fuente…).
    pub fn papel_en_cascada(&self, d: &Diagnostico) -> Option<bool> {
        let (a, l, mas) = self.primer_error()?;
        if !mas
            || d.origen != jonesy_core::Origen::Compilador
            || d.severidad != Severidad::Error
            || d.archivo.as_ref() != Some(&a)
        {
            return None;
        }
        let primero = self
            .diag_compilador
            .iter()
            .find(|x| x.severidad == Severidad::Error && x.archivo.is_some())?;
        if d == primero {
            Some(false)
        } else if d.linea >= l {
            Some(true)
        } else {
            None
        }
    }

    /// Ctrl+F2: pone o quita un marcador en la línea del cursor.
    fn alternar_marcador(&mut self) {
        let Some(ruta) = self.activo_ruta() else {
            return;
        };
        let linea = self.buf().buf.linea_columna().0 + 1;
        let clave = (ruta, linea);
        if !self.marcadores.remove(&clave) {
            self.marcadores.insert(clave);
            self.avisar(
                format!("Marcador en la línea {linea} (Shift+F2 va al siguiente)."),
                Nivel::Info,
            );
        }
        self.guardar_sesion();
    }

    /// Shift+F2: el próximo marcador después del cursor (en este archivo y después en
    /// los demás, en orden), dando la vuelta.
    fn siguiente_marcador(&mut self) {
        if self.marcadores.is_empty() {
            self.avisar("No hay marcadores: Ctrl+F2 pone uno.", Nivel::Aviso);
            return;
        }
        let actual = (
            self.activo_ruta().unwrap_or_default(),
            self.buf().buf.linea_columna().0 + 1,
        );
        let (p, l) = self
            .marcadores
            .iter()
            .find(|m| **m > actual)
            .or_else(|| self.marcadores.iter().next())
            .cloned()
            .unwrap();
        self.recordar_posicion();
        self.ir_a(&p, l, 1);
        self.cambiar_foco(Foco::Editor);
    }

    /// Ver → Marcadores: la lista para elegir uno.
    fn lista_marcadores(&mut self) {
        let opciones: Vec<OpcionPaleta> = self
            .marcadores
            .iter()
            .map(|(p, l)| {
                let texto = std::fs::read_to_string(p)
                    .ok()
                    .and_then(|t| t.lines().nth(l - 1).map(|x| x.trim().to_string()))
                    .unwrap_or_default();
                OpcionPaleta {
                    titulo: format!("{}:{l}  {texto}", self.proyecto.relativa(p)),
                    atajo: String::new(),
                    que: QuePaleta::Linea(p.clone(), *l),
                }
            })
            .collect();
        if opciones.is_empty() {
            self.avisar("No hay marcadores: Ctrl+F2 pone uno.", Nivel::Aviso);
            return;
        }
        self.modal = Some(Modal::Paleta {
            filtro: String::new(),
            sel: 0,
            opciones,
        });
    }

    /// Alt+N / Alt+Shift+N: el problema siguiente o anterior al cursor, sin abrir el panel.
    fn problema_cercano(&mut self, adelante: bool) {
        let mut v: Vec<(PathBuf, usize, usize, String)> = self
            .diagnosticos()
            .into_iter()
            .filter_map(|d| {
                Some((
                    d.archivo.clone()?,
                    d.linea.max(1),
                    d.columna.max(1),
                    d.resumen.clone().unwrap_or_else(|| d.mensaje.clone()),
                ))
            })
            .collect();
        v.sort_by(|a, b| (&a.0, a.1, a.2).cmp(&(&b.0, b.1, b.2)));
        v.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
        if v.is_empty() {
            self.avisar("No hay problemas.", Nivel::Ok);
            return;
        }
        let (fila, col) = self.buf().buf.linea_columna();
        let aqui = (self.activo_ruta().unwrap_or_default(), fila + 1, col + 1);
        let clave = |x: &(PathBuf, usize, usize, String)| (x.0.clone(), x.1, 0usize);
        let elegido = if adelante {
            v.iter()
                .find(|x| clave(x) > (aqui.0.clone(), aqui.1, 0))
                .or(v.first())
        } else {
            v.iter()
                .rev()
                .find(|x| clave(x) < (aqui.0.clone(), aqui.1, 0))
                .or(v.last())
        }
        .cloned()
        .unwrap();
        let n = v.iter().position(|x| x == &elegido).unwrap_or(0) + 1;
        self.ir_a(&elegido.0, elegido.1, elegido.2);
        self.cambiar_foco(Foco::Editor);
        self.avisar(format!("{n}/{}: {}", v.len(), elegido.3), Nivel::Aviso);
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

    // ------------------------------------------------------------ mouse
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

/// La explicación de F1 para un hallazgo traducido (daedalus, tetsuo): título, lo que dijo
/// el compilador o el programa, el porqué, qué hacer y el apunte.
pub fn markdown_traduccion(t: &Diagnostico, original: &str) -> String {
    let mut s = format!("# {}\n\n", t.mensaje);
    let original = original.trim();
    if !original.is_empty() && original != t.mensaje {
        s.push_str(&format!("> `{original}`\n\n"));
    }
    if let Some(e) = &t.explicacion {
        s.push_str(e.trim());
        s.push('\n');
    }
    if let Some(q) = &t.sugerencia {
        s.push_str(&format!("\n**Qué hacer:** {}\n", q.trim()));
    }
    if let Some(e) = &t.enlace {
        s.push_str(&format!("\n**Apunte:** {e}\n"));
    }
    if let Some(h) = &t.herramienta {
        s.push_str(&format!("\n_Explicación de {h}._\n"));
    }
    s
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

/// Filas de Configuración del entorno → Apariencia → Tamaños de paneles: (título, unidad).
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

/// Contenido inicial de un archivo nuevo: los plugins Python arrancan con una plantilla
/// comentada (al_guardar y un comando de ejemplo); el resto, vacíos.
pub fn plantilla_para(ruta: &Path, dir_estado: &Path) -> &'static str {
    const PLANTILLA_PLUGIN: &str = include_str!("../../../docs/plugin-ejemplo.py");
    if ruta.starts_with(dir_estado.join("plugins")) && ruta.extension().is_some_and(|e| e == "py") {
        PLANTILLA_PLUGIN
    } else {
        ""
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
                Style::default().fg(tema::ok())
            } else if l.starts_with('-') {
                Style::default().fg(tema::error())
            } else if l.starts_with("@@") {
                Style::default().fg(tema::nota())
            } else if l.starts_with("commit ") {
                Style::default()
                    .fg(tema::advertencia())
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
