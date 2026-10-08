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
use jonesy_core::{diagnostico, entrega, Buffer, Delta, Diagnostico, Lenguaje, Origen, PosicionDoc, Proyecto, Severidad};
use jonesy_git::{Commit, Repo};
use jonesy_plugins::{ComandoPlugin, Contexto, Pedido as PedidoPlugin, Registro, Respuesta, Servicio, Supervisor};
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
use crate::paleta;
use crate::resaltado;
use crate::sistema::{self, Aviso};
use crate::tema::{self, Simbolos};

pub const AUTOGUARDADO: Duration = Duration::from_secs(30);
pub const DEBOUNCE_PREVIEW: Duration = Duration::from_millis(120);
pub const SONDEO_GIT: Duration = Duration::from_secs(2);
pub const AVISOS_MIN: [u64; 3] = [30, 15, 5];

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
}

impl Foco {
    fn nombre(self) -> &'static str {
        match self {
            Foco::Editor => "editor",
            Foco::Consigna => "consigna",
            Foco::Inferior => "inferior",
            Foco::Lateral => "lateral",
            Foco::Doc => "quickdoc",
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
    pub const TODAS: [Pestana; 5] = [Pestana::Problemas, Pestana::Salida, Pestana::Terminal, Pestana::Ejecucion, Pestana::Depurador];

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
}

#[derive(Debug, Clone)]
pub struct OpcionPaleta {
    pub titulo: String,
    pub atajo: String,
    pub que: QuePaleta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirmacion {
    Salir,
    CerrarBuffer,
    Entregar,
}

pub enum Modal {
    Paleta { filtro: String, sel: usize, opciones: Vec<OpcionPaleta> },
    Abrir { filtro: String, sel: usize, archivos: Vec<String> },
    Buscar { texto: String },
    IrALinea { texto: String },
    Confirmar { titulo: String, mensaje: String, que: Confirmacion },
    Commit { mensaje: String },
    Visor { titulo: String, lineas: Vec<Line<'static>>, desplazamiento: usize },
    Menu { indice: usize, sel: usize },
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
    Compilar { luego: Luego },
    Ripley { archivo: PathBuf },
    Verificar,
    Explicar { codigo: String },
    MemTrace { traza: PathBuf },
}

pub struct BufferAbierto {
    pub buf: Buffer,
    pub cache: resaltado::Cache,
    /// Vista previa de Markdown: (versión renderizada, ancho, documento).
    pub preview: Option<(u64, u16, Documento)>,
    pub preview_desplazamiento: usize,
    pub ultimo_cambio: Instant,
}

impl BufferAbierto {
    fn nuevo(buf: Buffer) -> BufferAbierto {
        BufferAbierto { buf, cache: Default::default(), preview: None, preview_desplazamiento: 0, ultimo_cambio: Instant::now() }
    }
}

pub struct Consigna {
    pub ruta: PathBuf,
    pub fuente: String,
    pub doc: Option<(u16, Documento)>,
    pub desplazamiento: usize,
}

pub struct Depuracion {
    pub gdb: Gdb,
    pub pty: PtyInferior,
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
}

/// Zonas clicables registradas en el último dibujo.
#[derive(Default)]
pub struct Zonas {
    pub menu: Vec<(Rect, usize)>,
    pub botones: Vec<(Rect, Accion)>,
    pub pestanas_buffers: Vec<(Rect, usize)>,
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
    pub alto_inferior: u16,
    pub ancho_consigna: Option<u16>,
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
    pub diag_ripley: HashMap<PathBuf, Vec<Diagnostico>>,
    pub diag_plugins: HashMap<PathBuf, Vec<Diagnostico>>,
    pub sel_problema: usize,
    pub sel_regla: usize,
    pub salida: Vec<(String, bool)>,
    pub titulo_salida: String,
    pub salida_desplazamiento: usize,
    ripley_disponible: bool,

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
    /// Funciones declaradas en el buffer activo (quick-doc): (buffer, versión, funciones).
    pub funciones_usuario: Option<(usize, u64, Vec<jonesy_doc::Funcion>)>,
    pub salir: bool,
    pub inicio: Instant,
}

#[derive(Clone, Copy)]
enum Arrastre {
    Seleccion,
    SeparadorVertical,
    SeparadorHorizontal,
}

impl App {
    pub fn nueva(op: Opciones, tx: Sender<Aviso>) -> App {
        let proyecto = Proyecto::abrir(&op.raiz);
        let ascii = op.ascii || proyecto.config.interfaz.ascii || !tema::locale_utf8();
        let sim = if ascii { Simbolos::ascii() } else { Simbolos::unicode() };
        let dir_estado = proyecto.dir_estado();
        let examen = op.examen.then(|| {
            let estado = EstadoExamen::cargar(&dir_estado).unwrap_or_else(|| EstadoExamen {
                inicio_unix: jonesy_core::ahora_unix(),
                duracion_min: proyecto.config.examen.duracion_min,
                ..Default::default()
            });
            let _ = std::fs::create_dir_all(&dir_estado);
            let _ = estado.guardar(&dir_estado);
            EstadoExamenUi { estado, avisos_dados: BTreeSet::new(), ultimo_autoguardado: Instant::now(), entregando: false }
        });
        let entregado = examen.as_ref().is_some_and(|e| e.estado.entregado);
        let telemetria = if (op.examen || op.telemetria) && !entregado {
            let _ = std::fs::create_dir_all(&dir_estado);
            Telemetria::abrir(dir_estado.join("telemetry.log")).ok()
        } else {
            None
        };
        if let Some(t) = &telemetria {
            t.registrar(TelemetryEvent::SessionStart { user: jonesy_core::usuario(), exam_mode: op.examen });
        }
        let permitidos = proyecto.config.examen.plugins_permitidos.clone();
        let sup = Supervisor::nuevo(&dir_estado.join("plugins"), op.examen.then_some(permitidos.as_slice()));
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
            Some(Consigna { ruta, fuente, doc: None, desplazamiento: 0 })
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
            alto_inferior: 10,
            ancho_consigna: None,
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
            diag_ripley: HashMap::new(),
            diag_plugins: HashMap::new(),
            sel_problema: 0,
            sel_regla: 0,
            salida: vec![],
            titulo_salida: String::new(),
            salida_desplazamiento: 0,
            ripley_disponible: proyecto.config.ripley.activo,
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
            funciones_usuario: None,
            salir: false,
            inicio: Instant::now(),
            proyecto,
        };
        app.restaurar_sesion(&op.archivos);
        app.refrescar_git();
        if let Some(e) = app.proyecto.error_config.clone() {
            app.avisar(e, Nivel::Error);
        } else if entregado {
            app.avisar("El examen ya fue entregado: los archivos quedan de solo lectura.", Nivel::Aviso);
            for b in &mut app.buffers {
                b.buf.solo_lectura = true;
            }
        } else if rechazados > 0 {
            app.avisar(format!("Modo examen: {rechazados} plugin(s) sin firma de la cátedra no se cargaron."), Nivel::Aviso);
        } else if app.examen.is_some() {
            app.avisar("Modo examen: la sesión se registra y se guarda automáticamente.", Nivel::Info);
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
            self.preview_markdown = s.preview_markdown;
            for (rel, linea) in s.breakpoints {
                self.breakpoints.insert((self.proyecto.raiz.join(rel), linea));
            }
        }
        for a in archivos {
            let ruta = if a.is_absolute() { a.clone() } else { std::env::current_dir().unwrap_or_default().join(a) };
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
                .or_else(|| candidatos.iter().find(|p| p.file_name().is_some_and(|n| n == "main.c")).cloned())
                .or_else(|| candidatos.iter().find(|p| p.extension().is_some_and(|e| e == "c")).cloned())
                .or_else(|| candidatos.iter().find(|p| Some(*p) != consigna.as_ref()).cloned());
            if let Some(p) = elegido {
                self.abrir_archivo(&p, false);
            } else {
                self.buffers.push(BufferAbierto::nuevo(Buffer::desde_texto("")));
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
                    Some(ArchivoAbierto { ruta: self.proyecto.relativa(ruta), cursor: b.buf.cursor, desplazamiento: b.buf.desplazamiento })
                })
                .collect(),
            activo: self.activo,
            quickdoc_visible: self.quickdoc_visible,
            quickdoc_posicion: Some(self.quickdoc_posicion),
            vista_compacta: self.vista.nombre().into(),
            preview_markdown: self.preview_markdown,
            breakpoints: self.breakpoints.iter().map(|(p, l)| (self.proyecto.relativa(p), *l)).collect(),
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
            .chain(self.diag_ripley.values().flatten())
            .chain(self.diag_plugins.values().flatten())
            .collect();
        v.sort_by(|a, b| (&a.archivo, a.linea, a.severidad).cmp(&(&b.archivo, b.linea, b.severidad)));
        v
    }

    /// Diagnósticos de una línea (0-based) del buffer activo.
    pub fn diagnosticos_linea(&self, ruta: &Path, linea: usize) -> Vec<&Diagnostico> {
        self.diagnosticos()
            .into_iter()
            .filter(|d| d.linea == linea + 1 && d.archivo.as_deref().is_some_and(|a| mismo_archivo(a, ruta)))
            .collect()
    }

    // ------------------------------------------------------------ buffers

    /// Abre (o activa) un archivo; devuelve su índice.
    pub fn abrir_archivo(&mut self, ruta: &Path, activar: bool) -> Option<usize> {
        let ruta = ruta.canonicalize().unwrap_or_else(|_| ruta.to_path_buf());
        if let Some(i) = self.buffers.iter().position(|b| b.buf.ruta.as_deref() == Some(&ruta)) {
            if activar {
                self.cambiar_buffer(i);
            }
            return Some(i);
        }
        let mut buf = if ruta.exists() {
            match Buffer::abrir(&ruta) {
                Ok(b) => b,
                Err(e) => {
                    self.avisar(format!("No se pudo abrir {}: {e}", ruta.display()), Nivel::Error);
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
            let protegido = self.consigna.as_ref().is_some_and(|c| mismo_archivo(&c.ruta, &ruta))
                || ruta.file_name().is_some_and(|n| n == jonesy_core::proyecto::ARCHIVO_CONFIG)
                || ruta.starts_with(self.proyecto.dir_estado());
            if protegido || self.examen_entregado() {
                buf.solo_lectura = true;
            }
        }
        // Un buffer vacío sin ruta (el inicial) se reemplaza.
        if self.buffers.len() == 1 && self.buffers[0].buf.ruta.is_none() && !self.buffers[0].buf.modificado() {
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
            self.tel(TelemetryEvent::FocusChange { panel: format!("buffer:{}", self.id_buffer(i)), focused: true });
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
            self.tel(TelemetryEvent::TextDelta { buffer_id: id, offset: d.offset, inserted: d.insertado, deleted_len: d.borrados });
        }
    }

    pub fn guardar(&mut self, i: usize) -> bool {
        let id = self.id_buffer(i);
        let b = &mut self.buffers[i].buf;
        if b.ruta.is_none() {
            self.avisar("Este buffer no tiene archivo: abrí uno con Ctrl+O.", Nivel::Aviso);
            return false;
        }
        if b.solo_lectura {
            return false;
        }
        match b.guardar() {
            Ok(bytes) => {
                self.tel(TelemetryEvent::BufferSave { buffer_id: id, total_bytes: bytes });
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
            if self.buffers[i].buf.modificado() && self.buffers[i].buf.ruta.is_some() && !self.buffers[i].buf.solo_lectura {
                ok &= self.guardar(i);
            }
        }
        ok
    }

    /// ripley, extensiones nativas y plugins Python tras guardar.
    fn al_guardar(&mut self, ruta: &Path, i: usize) {
        let texto = self.buffers[i].buf.contenido();
        let nativos = self.registro.diagnosticar(ruta, &texto);
        self.diag_plugins.insert(ruta.to_path_buf(), nativos);
        if let Some(p) = &self.plugins {
            p.pedir(PedidoPlugin::AlGuardar { archivo: ruta.to_path_buf(), texto });
        }
        let es_c = matches!(self.buffers[i].buf.lenguaje, Lenguaje::C);
        if es_c && self.ripley_disponible {
            let rel = self.proyecto.relativa(ruta);
            let cmd = format!(
                "{} check {} --format json",
                self.proyecto.config.ripley.comando,
                jonesy_core::proyecto::shell_quote(&rel)
            );
            self.lanzar(&cmd, Tarea::Ripley { archivo: ruta.to_path_buf() });
        }
        self.refrescar_git();
    }

    fn cerrar_buffer(&mut self, forzar: bool) {
        if self.buf().buf.modificado() && !forzar {
            self.modal = Some(Modal::Confirmar {
                titulo: "Cerrar archivo".into(),
                mensaje: format!("{} tiene cambios sin guardar. ¿Cerrar igual? (s/n)", self.buf().buf.nombre()),
                que: Confirmacion::CerrarBuffer,
            });
            return;
        }
        self.buffers.remove(self.activo);
        if self.buffers.is_empty() {
            self.buffers.push(BufferAbierto::nuevo(Buffer::desde_texto("")));
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
            Accion::GitStageTodo => repo.agregar_todo().map(|_| "Todo agregado al stage.".to_string()),
            Accion::GitUnstageTodo => repo.quitar_todo_del_stage().map(|_| "Stage vaciado.".to_string()),
            Accion::GitCommit => {
                if self.git_estado.as_ref().is_some_and(|e| e.cantidad_en_stage() == 0) {
                    self.avisar("No hay cambios en el stage (Espacio o 'a' para agregar).", Nivel::Aviso);
                    return;
                }
                self.modal = Some(Modal::Commit { mensaje: String::new() });
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
        let (Some(repo), Some(e)) = (self.repo.clone(), self.git_estado.clone()) else { return };
        let Some(entrada) = e.entradas.get(self.git.sel) else { return };
        let r = if entrada.en_stage() && !entrada.sin_stage() { repo.quitar_del_stage(&entrada.ruta) } else { repo.agregar(&entrada.ruta) };
        if let Err(e) = r {
            self.avisar(e.to_string(), Nivel::Error);
        }
        self.refrescar_git();
    }

    fn git_mostrar(&mut self) {
        let Some(repo) = self.repo.clone() else { return };
        let (titulo, texto) = if let Some(log) = &self.git.log {
            let Some(c) = log.get(self.git.sel_log) else { return };
            (format!("git show {}", c.hash), repo.mostrar(&c.hash))
        } else {
            let Some(e) = self.git_estado.as_ref().and_then(|e| e.entradas.get(self.git.sel)).cloned() else { return };
            if e.tipo == jonesy_git::TipoEntrada::SinSeguimiento {
                let ruta = repo.dir.join(&e.ruta);
                if ruta.is_file() {
                    self.abrir_archivo(&ruta, true);
                }
                return;
            }
            let en_stage = e.en_stage() && !e.sin_stage();
            (format!("diff {}", e.ruta), repo.diff(&e.ruta, en_stage))
        };
        match texto {
            Ok(t) => self.modal = Some(Modal::Visor { titulo, lineas: lineas_diff(&t), desplazamiento: 0 }),
            Err(e) => self.avisar(e.to_string(), Nivel::Error),
        }
    }

    fn git_commit(&mut self, mensaje: &str) {
        let Some(repo) = self.repo.clone() else { return };
        if mensaje.trim().is_empty() {
            self.avisar("El mensaje de commit está vacío.", Nivel::Aviso);
            return;
        }
        match repo.commit(mensaje) {
            Ok(salida) => {
                self.tel(TelemetryEvent::CommandExec { command: "git commit".into(), exit_code: Some(0) });
                self.avisar(salida.lines().next().unwrap_or("Commit creado.").to_string(), Nivel::Ok);
                self.modal = None;
                self.git.log = None;
            }
            Err(e) => {
                self.tel(TelemetryEvent::CommandExec { command: "git commit".into(), exit_code: Some(1) });
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
        if self.procesos.values().any(|t| matches!(t, Tarea::Compilar { .. })) {
            self.avisar("Ya hay una compilación en curso.", Nivel::Aviso);
            return;
        }
        self.guardar_todo();
        let activo = self.activo_ruta();
        let comando = if luego == Luego::Depurar && !self.proyecto.config.depurar.compilar.trim().is_empty() {
            Some(self.proyecto.config.depurar.compilar.clone())
        } else {
            self.proyecto.comando_compilar(activo.as_deref())
        };
        let Some(comando) = comando else {
            self.avisar("No hay qué compilar: abrí un .c o configurá [compilar] en .jonesy.toml.", Nivel::Aviso);
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
            self.avisar("No hay qué ejecutar: abrí un .c o configurá [ejecutar] en .jonesy.toml.", Nivel::Aviso);
            return;
        };
        self.guardar_todo();
        let envoltorio = format!("{cmd}\nc=$?\nprintf '\\n\\033[2m[proceso terminado con código %d]\\033[0m\\n' \"$c\"\n");
        let (filas, cols) = self.tam_panel_inferior();
        let tx = self.tx.clone();
        match Deck::iniciar(Some("sh"), &["-c".into(), envoltorio], &self.proyecto.raiz, filas, cols, move || {
            let _ = tx.send(Aviso::Despertar);
        }) {
            Ok(mut d) => {
                d.titulo = cmd.clone();
                self.ejecucion = Some(d);
                self.panel_inferior = true;
                self.pestana = Pestana::Ejecucion;
                self.foco = Foco::Inferior;
                if self.modo() == Modo::Compacto {
                    self.vista = Vista::Terminal;
                }
                self.tel(TelemetryEvent::CommandExec { command: cmd, exit_code: None });
            }
            Err(e) => self.avisar(format!("No se pudo crear la terminal: {e}"), Nivel::Error),
        }
    }

    pub fn tam_panel_inferior(&self) -> (u16, u16) {
        match self.areas.inferior {
            Some(r) => (r.height.saturating_sub(2).max(4), r.width.max(20)),
            None => (self.alto_inferior.saturating_sub(2).max(4), self.tam.0.max(20)),
        }
    }

    fn abrir_terminal(&mut self) {
        let vivo = self.terminal.as_mut().is_some_and(|d| d.vivo());
        if !vivo {
            let shell = Some(self.proyecto.config.interfaz.shell.clone()).filter(|s| !s.trim().is_empty());
            let (filas, cols) = self.tam_panel_inferior();
            let tx = self.tx.clone();
            match Deck::iniciar(shell.as_deref(), &[], &self.proyecto.raiz, filas, cols, move || {
                let _ = tx.send(Aviso::Despertar);
            }) {
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
            self.tel(TelemetryEvent::FocusChange { panel: self.foco.nombre().into(), focused: false });
            self.foco = f;
            self.tel(TelemetryEvent::FocusChange { panel: f.nombre().into(), focused: true });
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
        let cfg = &self.proyecto.config.depurar.programa;
        if !cfg.trim().is_empty() {
            return Some(self.proyecto.raiz.join(cfg));
        }
        let activo = self.activo_ruta()?;
        let stem = activo.file_stem()?.to_string_lossy().into_owned();
        let candidato = self.proyecto.raiz.join(&stem);
        candidato.is_file().then_some(candidato)
    }

    fn iniciar_depuracion(&mut self) {
        let Some(programa) = self.programa_a_depurar() else {
            self.avisar("No encontré el ejecutable: configurá [depurar] programa en .jonesy.toml.", Nivel::Error);
            return;
        };
        let (filas, cols) = self.tam_panel_inferior();
        let cols_pty = (cols / 2).max(20);
        let tx = self.tx.clone();
        let pty = match PtyInferior::abrir(filas, cols_pty, move || {
            let _ = tx.send(Aviso::Despertar);
        }) {
            Ok(p) => p,
            Err(e) => {
                self.avisar(format!("No se pudo crear el PTY del programa: {e}"), Nivel::Error);
                return;
            }
        };
        let tx = self.tx.clone();
        let args = self.proyecto.config.depurar.argumentos.clone();
        let mut gdb = match Gdb::lanzar(&programa, &args, &self.proyecto.raiz, Some(pty.ruta()), move || {
            let _ = tx.send(Aviso::Despertar);
        }) {
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
        self.tel(TelemetryEvent::CommandExec { command: format!("gdb {}", self.proyecto.relativa(&programa)), exit_code: None });
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
        self.avisar("Depurando: F5 continuar · F10 paso · F11 entrar · Shift+F11 salir · Shift+F5 detener", Nivel::Info);
        self.lanzar_memtrace();
    }

    /// mem-trace en segundo plano para cat-scan (sólo programas de un archivo).
    fn lanzar_memtrace(&mut self) {
        let Some(activo) = self.activo_ruta() else { return };
        if activo.extension().is_none_or(|e| e != "c") || self.proyecto.makefile().is_some() {
            return;
        }
        let dir = self.proyecto.dir_estado().join("memtrace");
        let _ = std::fs::create_dir_all(&dir);
        let rel = self.proyecto.relativa(&activo);
        let q = jonesy_core::proyecto::shell_quote;
        let cmd = format!(
            "mem-trace run {} --outdir {} -o {} --max-steps 3000",
            q(&rel),
            q(&self.proyecto.relativa(&dir)),
            q(&self.proyecto.relativa(&dir.join("reporte.html")))
        );
        self.lanzar(&cmd, Tarea::MemTrace { traza: dir.join("trace.json") });
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
        let Some(d) = &mut self.depuracion else { return };
        let eventos = d.gdb.recibir();
        let mut ir: Option<(PathBuf, usize)> = None;
        let mut fin = false;
        let mut aviso: Option<(String, Nivel)> = None;
        for e in eventos {
            match e {
                EventoGdb::Detenido(p) => {
                    let marco = match &p {
                        Parada::Breakpoint(m) | Parada::Paso(m) | Parada::Otra(_, m) => Some(m.clone()),
                        Parada::Senal { nombre, significado, marco } => {
                            aviso = Some((format!("El programa recibió {nombre} ({significado})."), Nivel::Error));
                            d.estado = format!("señal {nombre}");
                            Some(marco.clone())
                        }
                        Parada::Termino(c) => {
                            d.estado = match c {
                                Some(c) => format!("terminó (código {c})"),
                                None => "terminó por una señal".into(),
                            };
                            aviso = Some((format!("El programa {}. F5 lo vuelve a correr.", d.estado), Nivel::Info));
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
                            d.paso_traza = t.alinear(&m0.funcion, l, d.marcos.len(), d.paso_traza.unwrap_or(0));
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
        let Some(ruta) = self.activo_ruta() else { return };
        let clave = (ruta.clone(), linea + 1);
        let poner = !self.breakpoints.remove(&clave);
        if poner {
            self.breakpoints.insert(clave);
        }
        if let Some(d) = &mut self.depuracion {
            let _ = if poner { d.gdb.poner_breakpoint(&ruta, linea + 1) } else { d.gdb.quitar_breakpoint(&ruta, linea + 1) };
        }
    }

    // ------------------------------------------------------------ examen

    fn pedir_entrega(&mut self) {
        let Some(e) = &self.examen else {
            self.avisar("La entrega sólo está disponible en modo examen (jonesy --exam).", Nivel::Aviso);
            return;
        };
        if e.estado.entregado {
            let msg = format!("Ya entregado: {}", e.estado.sha256.clone().unwrap_or_default());
            self.avisar(msg, Nivel::Info);
            return;
        }
        let verificar = self.proyecto.comando_verificar().unwrap_or_else(|| "(sin verificación configurada)".into());
        self.modal = Some(Modal::Confirmar {
            titulo: "Entregar examen".into(),
            mensaje: format!(
                "Se guardan todos los archivos, se corre «{verificar}», se sella la bitácora y se arma el paquete final. Después no se puede editar. ¿Entregar? (s/n)"
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
            self.tel(TelemetryEvent::CommandExec { command: v.comando.clone(), exit_code: v.codigo });
        }
        self.tel(TelemetryEvent::SessionEnd { reason: "entrega".into() });
        let log = self.proyecto.dir_estado().join("telemetry.log");
        if let Some(t) = self.telemetria.take() {
            t.cerrar();
        }
        let sello = jonesy_telemetry::verificar(&log).ok();
        let telemetria = sello.as_ref().map(|v| (log.as_path(), Some(v.ultimo_hash.clone()), v.registros as usize));
        match entrega::empaquetar(&self.proyecto, telemetria, verificacion.clone(), jonesy_core::ahora_unix()) {
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
                    Some(v) => format!("✗ la verificación falló (código {})", v.codigo.map(|c| c.to_string()).unwrap_or("?".into())),
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
                aviso = Some(("Se terminó el tiempo del examen: entregá con F12.".into(), Nivel::Error));
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
                    let destino = jonesy_core::sesion::ruta_recuperacion(&dir, &self.proyecto.relativa(ruta));
                    if let Some(p) = destino.parent() {
                        let _ = std::fs::create_dir_all(p);
                    }
                    let _ = jonesy_core::buffer::guardar_atomico(&destino, b.buf.contenido().as_bytes());
                }
            }
        }
        self.guardar_sesion();
        if let Some(d) = &mut self.depuracion {
            d.gdb.terminar();
        }
        if let Some(t) = self.telemetria.take() {
            t.registrar(TelemetryEvent::SessionEnd { reason: razon.into() });
            t.cerrar();
        }
    }

    fn pedir_salida(&mut self) {
        let sin_guardar: Vec<String> = self.buffers.iter().filter(|b| b.buf.modificado()).map(|b| b.buf.nombre()).collect();
        if sin_guardar.is_empty() || self.examen.is_some() {
            self.salir = true;
        } else {
            self.modal = Some(Modal::Confirmar {
                titulo: "Salir".into(),
                mensaje: format!("Hay cambios sin guardar en {}. ¿Salir sin guardar? (s/n)", sin_guardar.join(", ")),
                que: Confirmacion::Salir,
            });
        }
    }

    // ------------------------------------------------------------ procesos

    fn al_proceso(&mut self, s: Salida) {
        match s {
            Salida::Linea { id, texto, error } => {
                if matches!(self.procesos.get(&id), Some(Tarea::Compilar { .. } | Tarea::Verificar)) {
                    self.salida.push((texto, error));
                }
            }
            Salida::Fin { id, codigo, salida } => {
                let Some(t) = self.procesos.remove(&id) else { return };
                self.fin_proceso(t, codigo, salida);
            }
        }
    }

    fn fin_proceso(&mut self, t: Tarea, codigo: Option<i32>, salida: String) {
        match t {
            Tarea::Compilar { luego } => {
                let comando = self.salida.first().map(|l| l.0.trim_start_matches("$ ").to_string()).unwrap_or_default();
                self.tel(TelemetryEvent::CommandExec { command: comando, exit_code: codigo });
                self.diag_compilador = diagnostico::parsear_gcc(&salida, &self.proyecto.raiz);
                let errores = self.diag_compilador.iter().filter(|d| d.severidad == Severidad::Error).count();
                let advertencias = self.diag_compilador.len() - errores;
                if codigo == Some(0) {
                    self.salida.push((format!("[compilación correcta, {advertencias} advertencia(s)]"), false));
                    self.avisar(format!("Compilación correcta ({advertencias} advertencias)."), Nivel::Ok);
                    if advertencias > 0 && luego == Luego::Nada {
                        self.pestana = Pestana::Problemas;
                    }
                    match luego {
                        Luego::Ejecutar => self.ejecutar_en_deck(),
                        Luego::Depurar => self.iniciar_depuracion(),
                        Luego::Nada => {}
                    }
                } else {
                    self.salida.push((format!("[la compilación falló: código {}]", codigo.unwrap_or(-1)), true));
                    self.avisar(format!("La compilación falló: {errores} error(es). Enter en Problemas salta al error."), Nivel::Error);
                    if !self.diag_compilador.is_empty() {
                        self.pestana = Pestana::Problemas;
                        self.sel_problema = 0;
                    }
                }
            }
            Tarea::Ripley { archivo } => {
                if codigo == Some(127) {
                    self.ripley_disponible = false;
                    return;
                }
                match diagnostico::parsear_ripley(&salida, &self.proyecto.raiz) {
                    Ok(mut d) => {
                        for x in &mut d {
                            if x.archivo.is_none() {
                                x.archivo = Some(archivo.clone());
                            }
                        }
                        self.diag_ripley.insert(archivo, d);
                    }
                    Err(_) => {
                        self.diag_ripley.remove(&archivo);
                    }
                }
            }
            Tarea::Verificar => {
                let comando = self.salida.first().map(|l| l.0.trim_start_matches("$ ").to_string()).unwrap_or_default();
                self.salida.push((format!("[verificación terminada: código {}]", codigo.unwrap_or(-1)), codigo != Some(0)));
                let v = entrega::Verificacion { comando, codigo, salida: entrega::cola(&salida, 40) };
                self.finalizar_entrega(Some(v));
            }
            Tarea::Explicar { codigo: regla } => {
                let ancho = self.tam.0.saturating_sub(8).clamp(40, 100);
                let lineas = if codigo == Some(0) && !salida.trim().is_empty() {
                    crate::vt::ansi_a_lineas(&salida, ancho)
                } else {
                    vec![Line::from(format!("ripley no tiene una explicación para {regla}."))]
                };
                self.modal = Some(Modal::Visor { titulo: format!("Regla {regla}"), lineas, desplazamiento: 0 });
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
                    let nativos = self.registro.diagnosticar(&archivo, &std::fs::read_to_string(&archivo).unwrap_or_default());
                    self.diag_plugins.insert(archivo, nativos.into_iter().chain(d).collect());
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

    fn explicar_regla(&mut self) {
        let codigo = match self.foco {
            Foco::Inferior if self.pestana == Pestana::Problemas => {
                self.diagnosticos().get(self.sel_problema).and_then(|d| d.codigo.clone())
            }
            Foco::Lateral if self.lateral == Some(Lateral::Reglas) => {
                self.reglas_agrupadas().into_iter().flat_map(|(_, v)| v).nth(self.sel_regla).and_then(|d| d.codigo.clone())
            }
            _ => self.activo_ruta().and_then(|r| {
                let (l, _) = self.buf().buf.linea_columna();
                self.diagnosticos_linea(&r, l).into_iter().find_map(|d| d.codigo.clone())
            }),
        };
        match codigo {
            Some(c) => {
                let cmd = format!("{} explain {}", self.proyecto.config.ripley.comando, jonesy_core::proyecto::shell_quote(&c));
                self.lanzar(&cmd, Tarea::Explicar { codigo: c });
            }
            None => self.abrir_paleta(),
        }
    }

    /// Diagnósticos de ripley agrupados por categoría (panel de reglas, RF-3.4).
    pub fn reglas_agrupadas(&self) -> Vec<(&'static str, Vec<&Diagnostico>)> {
        let mut grupos: BTreeMap<&'static str, Vec<&Diagnostico>> = BTreeMap::new();
        for d in self.diagnosticos() {
            if d.origen == Origen::Compilador {
                continue;
            }
            let cat = d.codigo.as_deref().map(diagnostico::categoria_regla).unwrap_or("Extensiones");
            grupos.entry(cat).or_default().push(d);
        }
        grupos.into_iter().collect()
    }

    // ------------------------------------------------------------ paleta y modales

    fn abrir_paleta(&mut self) {
        let mut opciones: Vec<OpcionPaleta> = Accion::todas()
            .iter()
            .filter(|a| self.examen.is_some() || **a != Accion::Entregar)
            .map(|a| {
                let i = a.info();
                OpcionPaleta { titulo: i.titulo.into(), atajo: i.atajo.into(), que: QuePaleta::Accion(*a) }
            })
            .collect();
        for c in &self.comandos_plugins {
            opciones.push(OpcionPaleta { titulo: c.titulo(), atajo: "plugin".into(), que: QuePaleta::Plugin(c.clone()) });
        }
        if self.comandos_plugins.is_empty() {
            if let Some(p) = &self.plugins {
                p.pedir(PedidoPlugin::Comandos);
            }
        }
        self.modal = Some(Modal::Paleta { filtro: String::new(), sel: 0, opciones });
    }

    fn abrir_selector(&mut self) {
        let archivos: Vec<String> = self.proyecto.archivos(2000).iter().map(|p| self.proyecto.relativa(p)).collect();
        self.modal = Some(Modal::Abrir { filtro: String::new(), sel: 0, archivos });
    }

    pub fn accion(&mut self, a: Accion) {
        self.tel(TelemetryEvent::CommandExec { command: format!("accion:{a:?}"), exit_code: None });
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
                    self.tel(TelemetryEvent::TextDelta { buffer_id: id, offset: d.offset, inserted: d.insertado, deleted_len: d.borrados });
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
            Accion::Buscar => self.modal = Some(Modal::Buscar { texto: self.busqueda.clone() }),
            Accion::BuscarSiguiente => self.buscar_siguiente(),
            Accion::IrALinea => self.modal = Some(Modal::IrALinea { texto: String::new() }),
            Accion::QuickDoc => {
                if self.modo() == Modo::Compacto {
                    self.vista = if self.vista == Vista::Doc { Vista::Codigo } else { Vista::Doc };
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
                    self.avisar("cat-scan muestra la memoria durante la depuración (F5).", Nivel::Aviso);
                    return;
                }
                self.lateral = Some(if self.lateral == Some(Lateral::CatScan) { Lateral::Variables } else { Lateral::CatScan });
                if self.depuracion.as_ref().is_some_and(|d| d.traza.is_none()) {
                    self.avisar("La traza de mem-trace todavía no está lista (o el proyecto tiene Makefile).", Nivel::Info);
                }
            }
            Accion::GitStageTodo | Accion::GitUnstageTodo | Accion::GitCommit | Accion::GitLog => self.git_accion(a),
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
            self.modal = Some(Modal::Buscar { texto: String::new() });
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

    pub fn bucle<B: ratatui::backend::Backend>(&mut self, term: &mut ratatui::Terminal<B>, rx: Receiver<Aviso>) -> anyhow::Result<()> {
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
            self.tic();
            if self.salir {
                break;
            }
            // Coalescencia: la salida masiva del deck se dibuja a 60 FPS como máximo.
            if solo_despertar {
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

    fn proxima_espera(&self) -> Duration {
        let mut espera = Duration::from_secs(60);
        if self.examen.as_ref().is_some_and(|e| !e.estado.entregado) {
            espera = espera.min(Duration::from_secs(1));
        }
        if self.repo.is_some() {
            espera = espera.min(SONDEO_GIT);
        }
        if let Some((_, _, t)) = &self.mensaje {
            espera = espera.min(Duration::from_secs(6).saturating_sub(t.elapsed()).max(Duration::from_millis(50)));
        }
        if self.preview_markdown {
            let b = self.buf();
            if b.preview.as_ref().is_none_or(|p| p.0 != b.buf.version) {
                espera = espera.min(DEBOUNCE_PREVIEW.saturating_sub(b.ultimo_cambio.elapsed()).max(Duration::from_millis(10)));
            }
        }
        if !self.procesos.is_empty() {
            espera = espera.min(Duration::from_millis(500));
        }
        espera
    }

    /// Tareas periódicas tras cada lote de eventos.
    fn tic(&mut self) {
        self.tic_examen();
        self.sondear_git();
        self.eventos_gdb();
        if self.mensaje.as_ref().is_some_and(|(_, n, t)| t.elapsed() > Duration::from_secs(if *n == Nivel::Error { 12 } else { 6 })) {
            self.mensaje = None;
        }
        // CursorMove con un mínimo de 200 ms entre registros.
        if self.telemetria.is_some() {
            let (l, c) = self.buf().buf.linea_columna();
            let id = self.id_buffer(self.activo);
            if (id.clone(), l, c) != self.ultimo_cursor && self.ultimo_cursor_registrado.elapsed() >= Duration::from_millis(200) {
                self.ultimo_cursor = (id.clone(), l, c);
                self.ultimo_cursor_registrado = Instant::now();
                self.tel(TelemetryEvent::CursorMove { buffer_id: id, line: l + 1, col: c + 1 });
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
                let nombre = match s {
                    signal_hook::consts::SIGHUP => "SIGHUP",
                    signal_hook::consts::SIGINT => "SIGINT",
                    _ => "SIGTERM",
                };
                self.finalizar(nombre);
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
                self.tel(TelemetryEvent::FocusChange { panel: "terminal".into(), focused: false });
                if self.examen.is_some() && !self.examen_entregado() {
                    self.guardar_todo();
                }
            }
            Event::FocusGained => self.tel(TelemetryEvent::FocusChange { panel: "terminal".into(), focused: true }),
            _ => {}
        }
    }

    fn pegar(&mut self, t: String) {
        let t = t.replace("\r\n", "\n").replace('\r', "\n");
        match &mut self.modal {
            Some(Modal::Buscar { texto } | Modal::IrALinea { texto } | Modal::Commit { mensaje: texto }) => {
                texto.push_str(&t);
                return;
            }
            Some(Modal::Paleta { filtro, .. } | Modal::Abrir { filtro, .. }) => {
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
                                let _ = d.pty.escribir(t.as_bytes());
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
        if matches!(k.code, KeyCode::Char(_)) && !k.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) {
            return;
        }
        let nombre = match k.code {
            KeyCode::Char(c) => c.to_string(),
            KeyCode::F(n) => format!("F{n}"),
            otro => format!("{otro:?}"),
        };
        self.tel(TelemetryEvent::KeyPress { key: nombre, modifiers: k.modifiers.bits() });
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
            _ => self.tecla_editor(k),
        }
    }

    fn tecla_consigna(&mut self, k: KeyEvent) {
        let Some(c) = &mut self.consigna else {
            self.foco = Foco::Editor;
            return;
        };
        let alto = self.areas.consigna.map(|r| r.height.saturating_sub(2) as usize).unwrap_or(10);
        match k.code {
            KeyCode::Up => c.desplazamiento = c.desplazamiento.saturating_sub(1),
            KeyCode::Down => c.desplazamiento += 1,
            KeyCode::PageUp => c.desplazamiento = c.desplazamiento.saturating_sub(alto),
            KeyCode::PageDown | KeyCode::Char(' ') => c.desplazamiento += alto,
            KeyCode::Home => c.desplazamiento = 0,
            KeyCode::Esc | KeyCode::Tab => self.cambiar_foco(Foco::Editor),
            _ => self.tecla_editor(k),
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
                    self.git_estado.as_ref().map(|e| e.entradas.len()).unwrap_or(0)
                };
                let sel = if en_log { &mut self.git.sel_log } else { &mut self.git.sel };
                match k.code {
                    KeyCode::Up => *sel = sel.saturating_sub(1),
                    KeyCode::Down => *sel = (*sel + 1).min(total.saturating_sub(1)),
                    KeyCode::Char(' ') if !en_log => self.git_alternar_stage(),
                    KeyCode::Char('a') if !ctrl => self.git_accion(Accion::GitStageTodo),
                    KeyCode::Char('u') if !ctrl => self.git_accion(Accion::GitUnstageTodo),
                    KeyCode::Char('c') if !ctrl => self.git_accion(Accion::GitCommit),
                    KeyCode::Char('l') => self.git_accion(Accion::GitLog),
                    KeyCode::Char('r') => self.refrescar_git(),
                    KeyCode::Enter => self.git_mostrar(),
                    _ => self.tecla_editor(k),
                }
            }
            Some(Lateral::Reglas) => {
                let total: usize = self.reglas_agrupadas().iter().map(|(_, v)| v.len()).sum();
                match k.code {
                    KeyCode::Up => self.sel_regla = self.sel_regla.saturating_sub(1),
                    KeyCode::Down => self.sel_regla = (self.sel_regla + 1).min(total.saturating_sub(1)),
                    KeyCode::Enter => {
                        let d = self.reglas_agrupadas().into_iter().flat_map(|(_, v)| v).nth(self.sel_regla).cloned();
                        if let Some(Diagnostico { archivo: Some(a), linea, columna, .. }) = d {
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
            if self.esc_pendiente.is_some_and(|t| t.elapsed() < Duration::from_millis(600)) {
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
                    KeyCode::Down => self.sel_problema = (self.sel_problema + 1).min(total.saturating_sub(1)),
                    KeyCode::Enter => self.saltar_a_problema(self.sel_problema),
                    KeyCode::F(1) => self.explicar_regla(),
                    KeyCode::Tab => self.cambiar_foco(Foco::Editor),
                    _ => {}
                }
            }
            Pestana::Salida => match k.code {
                KeyCode::Up => self.salida_desplazamiento += 1,
                KeyCode::Down => self.salida_desplazamiento = self.salida_desplazamiento.saturating_sub(1),
                KeyCode::PageUp => self.salida_desplazamiento += 10,
                KeyCode::PageDown => self.salida_desplazamiento = self.salida_desplazamiento.saturating_sub(10),
                KeyCode::Tab => self.cambiar_foco(Foco::Editor),
                _ => {}
            },
            Pestana::Terminal | Pestana::Ejecucion => {
                let deck = if self.pestana == Pestana::Terminal { self.terminal.as_mut() } else { self.ejecucion.as_mut() };
                let Some(d) = deck else { return };
                if k.modifiers.contains(KeyModifiers::SHIFT) && matches!(k.code, KeyCode::PageUp | KeyCode::PageDown) {
                    let actual = d.con_pantalla(|s| s.scrollback());
                    let n = if k.code == KeyCode::PageUp { actual + 10 } else { actual.saturating_sub(10) };
                    d.desplazar(n);
                    return;
                }
                d.desplazar(0);
                if let Some(bytes) = bytes_tecla(&k) {
                    let _ = d.escribir(&bytes);
                }
            }
            Pestana::Depurador => {
                let Some(d) = &mut self.depuracion else { return };
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
                    let _ = d.pty.escribir(&bytes);
                }
            }
        }
    }

    fn saltar_a_problema(&mut self, i: usize) {
        let d = self.diagnosticos().get(i).map(|d| (*d).clone());
        if let Some(Diagnostico { archivo: Some(a), linea, columna, .. }) = d {
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
        if self.modo() == Modo::Compacto && self.vista != Vista::Codigo && self.vista != Vista::Consigna {
            if k.code == KeyCode::Esc {
                self.vista = Vista::Codigo;
            }
            return;
        }
        let alto = self.zonas.editor.map(|z| z.texto.height as isize).unwrap_or(20).max(2);
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
                if let Some(i) = menus.iter().position(|(n, _)| n.to_lowercase().starts_with(c)) {
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
            KeyCode::Tab => self.editar(|b| b.insertar("    ")),
            KeyCode::BackTab => self.editar(|b| desindentar(b)),
            KeyCode::Char(c) if !ctrl => {
                let mut s = [0u8; 4];
                let t = c.encode_utf8(&mut s).to_string();
                self.editar(|b| b.insertar(&t));
            }
            _ => {}
        }
    }

    fn tecla_modal(&mut self, k: KeyEvent) {
        let Some(modal) = self.modal.take() else { return };
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let alt = k.modifiers.contains(KeyModifiers::ALT);
        let cerrar = k.code == KeyCode::Esc;
        match modal {
            Modal::Paleta { mut filtro, mut sel, opciones } => {
                if cerrar {
                    return;
                }
                let filtradas = paleta::filtrar(&filtro, &opciones.iter().map(|o| o.titulo.as_str()).collect::<Vec<_>>());
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
                self.modal = Some(Modal::Paleta { filtro, sel, opciones });
            }
            Modal::Abrir { mut filtro, mut sel, archivos } => {
                if cerrar {
                    return;
                }
                let filtrados = paleta::filtrar(&filtro, &archivos);
                match k.code {
                    KeyCode::Enter => {
                        let rel = filtrados.get(sel).map(|&i| archivos[i].clone()).unwrap_or_else(|| filtro.clone());
                        if !rel.trim().is_empty() {
                            let ruta = self.proyecto.raiz.join(rel.trim());
                            if self.examen.is_some() && !ruta.exists() && !ruta.starts_with(&self.proyecto.raiz) {
                                return;
                            }
                            self.abrir_archivo(&ruta, true);
                        }
                        return;
                    }
                    KeyCode::Up => sel = sel.saturating_sub(1),
                    KeyCode::Down => sel = (sel + 1).min(filtrados.len().saturating_sub(1)),
                    _ => {
                        if editar_campo(&mut filtro, &k) {
                            sel = 0;
                        }
                    }
                }
                self.modal = Some(Modal::Abrir { filtro, sel, archivos });
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
            Modal::Confirmar { titulo, mensaje, que } => match k.code {
                KeyCode::Char('s' | 'S' | 'y' | 'Y') | KeyCode::Enter => match que {
                    Confirmacion::Salir => self.salir = true,
                    Confirmacion::CerrarBuffer => self.cerrar_buffer(true),
                    Confirmacion::Entregar => self.entregar(),
                },
                KeyCode::Char('n' | 'N') | KeyCode::Esc => {}
                _ => self.modal = Some(Modal::Confirmar { titulo, mensaje, que }),
            },
            Modal::Commit { mut mensaje } => {
                if cerrar {
                    return;
                }
                let confirmar = (k.code == KeyCode::Enter && (ctrl || alt)) || (k.code == KeyCode::Char('s') && ctrl);
                if confirmar {
                    self.modal = Some(Modal::Commit { mensaje: mensaje.clone() });
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
            Modal::Visor { titulo, lineas, mut desplazamiento } => {
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
                self.modal = Some(Modal::Visor { titulo, lineas, desplazamiento });
            }
            Modal::Menu { mut indice, mut sel } => {
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
            QuePaleta::Plugin(c) => {
                if let Some(p) = &self.plugins {
                    let b = &self.buf().buf;
                    let (l, col) = b.linea_columna();
                    let ctx = Contexto {
                        archivo: b.ruta.as_ref().map(|r| r.to_string_lossy().into_owned()).unwrap_or_default(),
                        texto: b.contenido(),
                        linea: l + 1,
                        columna: col + 1,
                    };
                    self.tel(TelemetryEvent::CommandExec { command: format!("plugin:{}", c.titulo()), exit_code: None });
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
                let delta: isize = if m.kind == MouseEventKind::ScrollUp { -3 } else { 3 };
                if let Some(Modal::Visor { desplazamiento, lineas, .. }) = &mut self.modal {
                    *desplazamiento = (*desplazamiento as isize + delta).clamp(0, lineas.len().saturating_sub(1) as isize) as usize;
                    return;
                }
                if self.zonas.preview.is_some_and(dentro) {
                    let b = self.buf_mut();
                    b.preview_desplazamiento = (b.preview_desplazamiento as isize + delta).max(0) as usize;
                } else if self.zonas.editor.is_some_and(|z| dentro(z.texto) || dentro(z.gutter)) {
                    let b = &mut self.buf_mut().buf;
                    let max = b.lineas().saturating_sub(1);
                    b.desplazamiento = ((b.desplazamiento as isize + delta).max(0) as usize).min(max);
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
                        Pestana::Salida => self.salida_desplazamiento = (self.salida_desplazamiento as isize - delta).max(0) as usize,
                        Pestana::Problemas => {
                            let total = self.diagnosticos().len();
                            self.sel_problema = ((self.sel_problema as isize + delta.signum()).max(0) as usize).min(total.saturating_sub(1));
                        }
                        _ => {}
                    }
                }
            }
            MouseEventKind::Down(MouseButton::Left) => self.clic(x, y, m.modifiers.contains(KeyModifiers::SHIFT)),
            MouseEventKind::Drag(MouseButton::Left) => match self.arrastre {
                Some(Arrastre::Seleccion) => {
                    if let Some(p) = self.posicion_en_editor(x, y) {
                        self.buf_mut().buf.mover_a(p, true);
                    }
                }
                Some(Arrastre::SeparadorVertical) => {
                    let ancho = self.tam.0.max(1);
                    self.ancho_consigna = Some(((x as u32 * 100 / ancho as u32) as u16).clamp(15, 60));
                }
                Some(Arrastre::SeparadorHorizontal) => {
                    let alto_total = self.tam.1.saturating_sub(2);
                    self.alto_inferior = alto_total.saturating_sub(y.saturating_sub(1)).clamp(4, alto_total.saturating_sub(6).max(4));
                }
                None => {}
            },
            MouseEventKind::Up(_) => self.arrastre = None,
            _ => {}
        }
    }

    fn clic(&mut self, x: u16, y: u16, shift: bool) {
        let dentro = |r: Rect| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height;
        let doble = self.ultimo_clic.is_some_and(|(t, cx, cy)| t.elapsed() < Duration::from_millis(400) && cx == x && cy == y);
        self.ultimo_clic = Some((Instant::now(), x, y));

        // Menú desplegado y modales.
        if let Some(Modal::Menu { .. }) = self.modal {
            if let Some((_, a)) = self.zonas.items_menu.iter().find(|(r, _)| dentro(*r)).copied() {
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
            if let (Some((r, base)), Modal::Paleta { sel, .. } | Modal::Abrir { sel, .. }) = (self.zonas.modal_items, &mut *modal) {
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
        if let Some((_, a)) = self.zonas.botones.iter().chain(self.zonas.botones_depurador.iter()).find(|(r, _)| dentro(*r)).copied() {
            self.accion(a);
            return;
        }
        if let Some((_, i)) = self.zonas.pestanas_buffers.iter().find(|(r, _)| dentro(*r)).copied() {
            self.cambiar_buffer(i);
            return;
        }
        if let Some((_, p)) = self.zonas.pestanas_inferior.iter().find(|(r, _)| dentro(*r)).copied() {
            self.pestana = p;
            self.cambiar_foco(Foco::Inferior);
            if p == Pestana::Terminal && self.terminal.is_none() {
                self.abrir_terminal();
            }
            return;
        }
        // Separadores arrastrables.
        if self.areas.sep_vertical.is_some_and(|sx| x == sx || x + 1 == sx) && self.areas.consigna.is_some_and(|c| y >= c.y && y < c.y + c.height) {
            self.arrastre = Some(Arrastre::SeparadorVertical);
            return;
        }
        if self.areas.sep_horizontal.is_some_and(|sy| y == sy) {
            self.arrastre = Some(Arrastre::SeparadorHorizontal);
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
                let b = self.buf();
                let linea = b.preview_desplazamiento + (y - r.y) as usize;
                let col = (x - r.x) as usize;
                let destino = b.preview.as_ref().and_then(|(_, _, d)| enlace_en(d, linea, col));
                let base = b.buf.ruta.as_ref().and_then(|p| p.parent().map(Path::to_path_buf));
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
                        let d = self.reglas_agrupadas().into_iter().flat_map(|(_, v)| v).nth(i).cloned();
                        if let Some(Diagnostico { archivo: Some(a), linea, columna, .. }) = d {
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
        let fila = y.saturating_sub(z.texto.y).min(z.texto.height.saturating_sub(1)) as usize;
        let linea = (b.desplazamiento + fila).min(b.lineas().saturating_sub(1));
        let objetivo = b.desplazamiento_h + x.saturating_sub(z.texto.x) as usize;
        let chars: Vec<char> = b.linea_texto(linea).chars().collect();
        let col = columna_de_visual(&chars, objetivo);
        Some(b.linea_columna_a_pos(linea, col))
    }
}

// ------------------------------------------------------------------ ayudas

pub fn mismo_archivo(a: &Path, b: &Path) -> bool {
    a == b || a.canonicalize().ok().zip(b.canonicalize().ok()).is_some_and(|(x, y)| x == y)
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
        unicode_width::UnicodeWidthChar::width(c).unwrap_or(0).max(1)
    }
}

/// Índice de carácter para una columna visual.
pub fn columna_de_visual(chars: &[char], objetivo: usize) -> usize {
    let cols = columnas_visuales(chars);
    for i in 0..chars.len() {
        if cols[i + 1] > objetivo {
            return if objetivo - cols[i] > (cols[i + 1] - cols[i]) / 2 { i + 1 } else { i };
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
        KeyCode::Char(c) if !k.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
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
    d.enlaces.iter().find(|e| e.linea == linea && col >= e.ini && col < e.fin.max(e.ini + 1)).map(|e| e.destino.clone())
}

/// Colorea un diff unificado.
pub fn lineas_diff(texto: &str) -> Vec<Line<'static>> {
    use ratatui::style::{Modifier, Style};
    texto
        .lines()
        .map(|l| {
            let estilo = if l.starts_with("+++") || l.starts_with("---") || l.starts_with("diff ") || l.starts_with("index ") {
                Style::default().add_modifier(Modifier::BOLD)
            } else if l.starts_with('+') {
                Style::default().fg(tema::OK)
            } else if l.starts_with('-') {
                Style::default().fg(tema::ERROR)
            } else if l.starts_with("@@") {
                Style::default().fg(tema::NOTA)
            } else if l.starts_with("commit ") {
                Style::default().fg(tema::ADVERTENCIA).add_modifier(Modifier::BOLD)
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
    fn expande_tabs_a_multiplos_de_cuatro() {
        assert_eq!(expandir_tabs("4\tint *p;"), "4   int *p;");
        assert_eq!(expandir_tabs("\tx"), "    x");
    }
}
