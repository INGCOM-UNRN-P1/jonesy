//! Sistema de extensiones de jonesy (RF-10).
//!
//! - Nivel 1, nativo: los traits [`EditorPlugin`], [`DiagnosticProvider`], [`WidgetView`]
//!   y [`TelemetryHook`] se implementan en Rust y se registran en tiempo de compilación
//!   en un [`Registro`].
//! - Nivel 2, Python fuera de proceso: [`Supervisor`] lanza `jonesy-worker-py` (embebido en
//!   el binario) y le habla JSON-RPC por pipes. El worker sólo se lanza si hay archivos en
//!   `.jonesy/plugins/*.py` (carga diferida); cada llamada tiene tiempo límite y, si vence o
//!   el worker muere, se lo descarta y se relanza en la llamada siguiente.
//!
//! En modo examen sólo se cargan los scripts cuyo SHA-256 figura en la lista de la cátedra.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use jonesy_core::{Diagnostico, Origen, Severidad};
use jonesy_telemetry::TelemetryEvent;
use serde::Deserialize;
use serde_json::{json, Value};

/// Código del worker, embebido para que el binario sea autocontenido.
pub const WORKER_PY: &str = include_str!("../../../tools/jonesy-worker-py/jonesy_worker.py");

pub const TIEMPO_LIMITE: Duration = Duration::from_secs(2);

// ---------------------------------------------------------------- nivel 1: Rust

/// Contexto que reciben los comandos de los plugins.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Contexto {
    pub archivo: String,
    pub texto: String,
    pub linea: usize,
    pub columna: usize,
}

/// Un comando que un plugin agrega a la paleta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComandoPlugin {
    pub plugin: String,
    pub comando: String,
}

impl ComandoPlugin {
    pub fn titulo(&self) -> String {
        format!("{}: {}", self.plugin, self.comando.replace('_', " "))
    }
}

pub trait EditorPlugin: Send {
    fn nombre(&self) -> &str;
    fn comandos(&self) -> Vec<String>;
    /// Devuelve el texto a mostrar al usuario.
    fn ejecutar(&mut self, comando: &str, ctx: &Contexto) -> Result<String, String>;
}

pub trait DiagnosticProvider: Send {
    fn nombre(&self) -> &str;
    fn diagnosticar(&mut self, archivo: &Path, texto: &str) -> Vec<Diagnostico>;
}

/// Panel informativo simple: líneas de texto ya formateadas para el ancho dado.
pub trait WidgetView: Send {
    fn titulo(&self) -> &str;
    fn lineas(&self, ancho: u16) -> Vec<String>;
}

pub trait TelemetryHook: Send {
    fn evento(&mut self, evento: &TelemetryEvent);
}

/// Extensiones nativas ensambladas en tiempo de compilación.
#[derive(Default)]
pub struct Registro {
    pub editores: Vec<Box<dyn EditorPlugin>>,
    pub diagnosticos: Vec<Box<dyn DiagnosticProvider>>,
    pub vistas: Vec<Box<dyn WidgetView>>,
    pub telemetria: Vec<Box<dyn TelemetryHook>>,
}

impl Registro {
    /// Registro con las extensiones que trae jonesy de fábrica.
    pub fn de_fabrica() -> Registro {
        let mut r = Registro::default();
        r.diagnosticos.push(Box::new(SangriaMezclada));
        r
    }

    pub fn diagnosticar(&mut self, archivo: &Path, texto: &str) -> Vec<Diagnostico> {
        self.diagnosticos
            .iter_mut()
            .flat_map(|d| d.diagnosticar(archivo, texto))
            .collect()
    }

    pub fn notificar(&mut self, evento: &TelemetryEvent) {
        for h in &mut self.telemetria {
            h.evento(evento);
        }
    }
}

/// Extensión nativa de ejemplo: marca tabulaciones mezcladas con espacios en la sangría,
/// que en la cátedra se indenta con 4 espacios.
pub struct SangriaMezclada;

impl DiagnosticProvider for SangriaMezclada {
    fn nombre(&self) -> &str {
        "sangria"
    }

    fn diagnosticar(&mut self, archivo: &Path, texto: &str) -> Vec<Diagnostico> {
        let es_c = matches!(
            archivo.extension().and_then(|e| e.to_str()),
            Some("c" | "h")
        );
        if !es_c {
            return vec![];
        }
        texto
            .lines()
            .enumerate()
            .filter_map(|(i, l)| {
                let sangria: String = l.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
                (sangria.contains('\t') && sangria.contains(' ')).then(|| Diagnostico {
                    archivo: Some(archivo.to_path_buf()),
                    linea: i + 1,
                    columna: 1,
                    severidad: Severidad::Estilo,
                    mensaje: "sangría con tabulaciones y espacios mezclados".into(),
                    codigo: None,
                    sugerencia: Some("indentar con 4 espacios".into()),
                    origen: Origen::Plugin,
                    ..Default::default()
                })
            })
            .collect()
    }
}

// ---------------------------------------------------------------- nivel 2: Python

#[derive(Debug, Clone, PartialEq)]
pub enum ErrorPlugin {
    /// No hay plugins que cargar (el worker no se lanza).
    SinPlugins,
    /// No se pudo lanzar python3.
    Lanzamiento(String),
    /// La llamada superó el tiempo límite; el worker fue reiniciado.
    TiempoAgotado,
    /// El worker terminó inesperadamente.
    Caido,
    /// El plugin devolvió un error JSON-RPC.
    Remoto(String),
}

impl std::fmt::Display for ErrorPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorPlugin::SinPlugins => write!(f, "no hay plugins en .jonesy/plugins"),
            ErrorPlugin::Lanzamiento(e) => write!(f, "no se pudo lanzar el worker de Python: {e}"),
            ErrorPlugin::TiempoAgotado => write!(
                f,
                "el plugin superó el tiempo límite; se reinició el worker"
            ),
            ErrorPlugin::Caido => write!(f, "el worker de Python terminó inesperadamente"),
            ErrorPlugin::Remoto(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ErrorPlugin {}

struct Worker {
    hijo: Child,
    entrada: ChildStdin,
    respuestas: Receiver<Value>,
    _script: tempfile_lite::Temporal,
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.hijo.kill();
        let _ = self.hijo.wait();
    }
}

/// Un archivo temporal mínimo (evita depender de `tempfile` en producción).
mod tempfile_lite {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};

    pub struct Temporal(PathBuf);

    impl Temporal {
        pub fn escribir(contenido: &str) -> std::io::Result<Temporal> {
            static CONTADOR: AtomicU32 = AtomicU32::new(0);
            let n = CONTADOR.fetch_add(1, Ordering::Relaxed);
            let ruta =
                std::env::temp_dir().join(format!("jonesy-worker-{}-{n}.py", std::process::id()));
            std::fs::write(&ruta, contenido)?;
            Ok(Temporal(ruta))
        }

        pub fn ruta(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Temporal {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct InfoPlugin {
    pub nombre: String,
    #[serde(default)]
    pub comandos: Vec<String>,
    #[serde(default)]
    pub al_guardar: bool,
}

/// Error de un plugin: al cargarlo (sintaxis, import) o en su `al_guardar`.
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorDePlugin {
    pub plugin: String,
    pub ruta: PathBuf,
    /// Línea del script donde ocurrió (1 si no se sabe).
    pub linea: usize,
    pub mensaje: String,
}

fn errores_de_json(v: &Value) -> Vec<ErrorDePlugin> {
    v.as_array()
        .map(|v| {
            v.iter()
                .map(|e| ErrorDePlugin {
                    plugin: e["plugin"].as_str().unwrap_or("").to_string(),
                    ruta: PathBuf::from(e["ruta"].as_str().unwrap_or("")),
                    linea: e["linea"].as_u64().unwrap_or(1).max(1) as usize,
                    mensaje: e["error"].as_str().unwrap_or("").to_string(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Supervisor del worker Python.
pub struct Supervisor {
    scripts: Vec<PathBuf>,
    /// Scripts descartados por no estar en la lista permitida (modo examen).
    pub rechazados: Vec<PathBuf>,
    pub plugins: Vec<InfoPlugin>,
    /// Errores de carga reportados por el worker.
    pub errores_carga: Vec<ErrorDePlugin>,
    /// Errores del último `al_guardar` de cada plugin.
    pub errores_ejecucion: Vec<ErrorDePlugin>,
    worker: Option<Worker>,
    siguiente_id: u64,
    pub tiempo_limite: Duration,
    pub reinicios: u32,
    python: String,
}

/// Intérprete de Python: `$JONESY_PYTHON`, `python3` o, si no está (Windows), `python`.
pub fn python_por_defecto() -> String {
    if let Ok(p) = std::env::var("JONESY_PYTHON") {
        return p;
    }
    if jonesy_core::proyecto::en_path("python3") || !jonesy_core::proyecto::en_path("python") {
        "python3".into()
    } else {
        "python".into()
    }
}

/// SHA-256 de un archivo en hexadecimal.
pub fn sha256_archivo(ruta: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    Ok(hex::encode(Sha256::digest(std::fs::read(ruta)?)))
}

impl Supervisor {
    /// Busca `dir_plugins/*.py`. Con `permitidos` (modo examen) sólo acepta los scripts
    /// cuyo SHA-256 esté en la lista.
    pub fn nuevo(dir_plugins: &Path, permitidos: Option<&[String]>) -> Supervisor {
        let mut scripts: Vec<PathBuf> = std::fs::read_dir(dir_plugins)
            .map(|it| {
                it.filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.extension().is_some_and(|e| e == "py") && p.is_file())
                    .collect()
            })
            .unwrap_or_default();
        scripts.sort();
        let mut rechazados = vec![];
        if let Some(lista) = permitidos {
            let (ok, no): (Vec<_>, Vec<_>) = scripts.into_iter().partition(|p| {
                sha256_archivo(p)
                    .map(|h| lista.iter().any(|x| x.eq_ignore_ascii_case(&h)))
                    .unwrap_or(false)
            });
            scripts = ok;
            rechazados = no;
        }
        Supervisor {
            scripts,
            rechazados,
            plugins: vec![],
            errores_carga: vec![],
            errores_ejecucion: vec![],
            worker: None,
            siguiente_id: 1,
            tiempo_limite: TIEMPO_LIMITE,
            reinicios: 0,
            python: python_por_defecto(),
        }
    }

    pub fn hay_plugins(&self) -> bool {
        !self.scripts.is_empty()
    }

    pub fn activo(&self) -> bool {
        self.worker.is_some()
    }

    pub fn pid(&self) -> Option<u32> {
        self.worker.as_ref().map(|w| w.hijo.id())
    }

    fn lanzar(&mut self) -> Result<(), ErrorPlugin> {
        let script = tempfile_lite::Temporal::escribir(WORKER_PY)
            .map_err(|e| ErrorPlugin::Lanzamiento(e.to_string()))?;
        let mut hijo = Command::new(&self.python)
            .arg("-I")
            .arg(script.ruta())
            .env("PYTHONUTF8", "1")
            .env("PYTHONIOENCODING", "utf-8")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| ErrorPlugin::Lanzamiento(e.to_string()))?;
        let entrada = hijo.stdin.take().expect("stdin entubado");
        let salida = hijo.stdout.take().expect("stdout entubado");
        let (tx, rx): (Sender<Value>, Receiver<Value>) = mpsc::channel();
        std::thread::Builder::new()
            .name("worker-py".into())
            .spawn(move || {
                for linea in BufReader::new(salida).lines() {
                    let Ok(linea) = linea else { break };
                    if let Ok(v) = serde_json::from_str::<Value>(&linea) {
                        if tx.send(v).is_err() {
                            break;
                        }
                    }
                }
            })
            .map_err(|e| ErrorPlugin::Lanzamiento(e.to_string()))?;
        self.worker = Some(Worker {
            hijo,
            entrada,
            respuestas: rx,
            _script: script,
        });
        let rutas: Vec<String> = self
            .scripts
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        let r = self.enviar("cargar", json!({ "rutas": rutas }))?;
        self.plugins = serde_json::from_value(r["plugins"].clone()).unwrap_or_default();
        self.errores_carga = errores_de_json(&r["errores"]);
        Ok(())
    }

    fn enviar(&mut self, metodo: &str, params: Value) -> Result<Value, ErrorPlugin> {
        let id = self.siguiente_id;
        self.siguiente_id += 1;
        let limite = self.tiempo_limite;
        let w = self.worker.as_mut().ok_or(ErrorPlugin::Caido)?;
        let pedido = json!({"jsonrpc": "2.0", "id": id, "method": metodo, "params": params});
        let escrito = writeln!(w.entrada, "{pedido}").and_then(|_| w.entrada.flush());
        if escrito.is_err() {
            self.descartar();
            return Err(ErrorPlugin::Caido);
        }
        let fin = std::time::Instant::now() + limite;
        loop {
            let resta = fin.saturating_duration_since(std::time::Instant::now());
            let w = self.worker.as_mut().ok_or(ErrorPlugin::Caido)?;
            match w.respuestas.recv_timeout(resta) {
                Ok(v) if v["id"].as_u64() == Some(id) => {
                    if let Some(e) = v.get("error") {
                        return Err(ErrorPlugin::Remoto(
                            e["message"].as_str().unwrap_or("error").to_string(),
                        ));
                    }
                    return Ok(v["result"].clone());
                }
                Ok(_) => continue, // respuesta vieja de una llamada que venció
                Err(RecvTimeoutError::Timeout) => {
                    self.descartar();
                    return Err(ErrorPlugin::TiempoAgotado);
                }
                Err(RecvTimeoutError::Disconnected) => {
                    self.descartar();
                    return Err(ErrorPlugin::Caido);
                }
            }
        }
    }

    fn descartar(&mut self) {
        if self.worker.take().is_some() {
            self.reinicios += 1;
        }
    }

    /// Llama a un método del worker; lo lanza (y carga los plugins) si hace falta.
    pub fn llamar(&mut self, metodo: &str, params: Value) -> Result<Value, ErrorPlugin> {
        if !self.hay_plugins() {
            return Err(ErrorPlugin::SinPlugins);
        }
        if self.worker.is_none() {
            self.lanzar()?;
        }
        self.enviar(metodo, params)
    }

    /// Diagnósticos de los plugins con `al_guardar`.
    pub fn al_guardar(
        &mut self,
        archivo: &Path,
        texto: &str,
    ) -> Result<Vec<Diagnostico>, ErrorPlugin> {
        let r = self.llamar(
            "al_guardar",
            json!({"archivo": archivo.to_string_lossy(), "texto": texto}),
        )?;
        self.errores_ejecucion = errores_de_json(&r["errores"]);
        Ok(r["diagnosticos"]
            .as_array()
            .map(|v| {
                v.iter()
                    .filter_map(|d| diagnostico_de_json(archivo, d))
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Los errores de los plugins como diagnósticos sobre sus propios scripts, para
    /// verlos en Problemas mientras se edita el plugin.
    pub fn diagnosticos_propios(&self) -> Vec<Diagnostico> {
        self.errores_carga
            .iter()
            .map(|e| (e, "no se pudo cargar"))
            .chain(
                self.errores_ejecucion
                    .iter()
                    .map(|e| (e, "falló al_guardar")),
            )
            .filter(|(e, _)| !e.ruta.as_os_str().is_empty())
            .map(|(e, cuando)| Diagnostico {
                archivo: Some(e.ruta.clone()),
                linea: e.linea,
                columna: 1,
                severidad: Severidad::Error,
                mensaje: format!("[{}] {cuando}: {}", e.plugin, e.mensaje),
                origen: Origen::Plugin,
                ..Default::default()
            })
            .collect()
    }

    pub fn comandos(&mut self) -> Result<Vec<ComandoPlugin>, ErrorPlugin> {
        let r = self.llamar("comandos", json!({}))?;
        Ok(r.as_array()
            .map(|v| {
                v.iter()
                    .map(|c| ComandoPlugin {
                        plugin: c["plugin"].as_str().unwrap_or("").to_string(),
                        comando: c["comando"].as_str().unwrap_or("").to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    pub fn ejecutar(
        &mut self,
        comando: &ComandoPlugin,
        ctx: &Contexto,
    ) -> Result<String, ErrorPlugin> {
        let r = self.llamar(
            "ejecutar",
            json!({"plugin": comando.plugin, "comando": comando.comando, "ctx": ctx}),
        )?;
        Ok(r["texto"].as_str().unwrap_or("").to_string())
    }
}

fn diagnostico_de_json(archivo: &Path, d: &Value) -> Option<Diagnostico> {
    let severidad = match d["severidad"].as_str().unwrap_or("advertencia") {
        "error" => Severidad::Error,
        "nota" => Severidad::Nota,
        "estilo" => Severidad::Estilo,
        _ => Severidad::Advertencia,
    };
    let plugin = d["plugin"].as_str().unwrap_or("plugin");
    Some(Diagnostico {
        archivo: Some(archivo.to_path_buf()),
        linea: d["linea"].as_u64().unwrap_or(1).max(1) as usize,
        columna: d["columna"].as_u64().unwrap_or(1).max(1) as usize,
        severidad,
        mensaje: format!("[{plugin}] {}", d["mensaje"].as_str()?),
        codigo: d["codigo"].as_str().map(str::to_string),
        sugerencia: d["sugerencia"].as_str().map(str::to_string),
        origen: Origen::Plugin,
        ..Default::default()
    })
}

/// Servicio que corre el supervisor en un hilo propio para no bloquear la interfaz.
pub enum Pedido {
    AlGuardar {
        archivo: PathBuf,
        texto: String,
    },
    Comandos,
    Ejecutar {
        comando: ComandoPlugin,
        ctx: Contexto,
    },
}

#[derive(Debug)]
pub enum Respuesta {
    Diagnosticos {
        archivo: PathBuf,
        resultado: Result<Vec<Diagnostico>, ErrorPlugin>,
    },
    Comandos(Result<Vec<ComandoPlugin>, ErrorPlugin>),
    Ejecutado {
        comando: ComandoPlugin,
        resultado: Result<String, ErrorPlugin>,
    },
    /// Cambiaron los errores de los propios plugins (ver `diagnosticos_propios`).
    ErroresPropios(Vec<Diagnostico>),
}

pub struct Servicio {
    tx: Sender<Pedido>,
}

impl Servicio {
    pub fn iniciar(
        mut sup: Supervisor,
        responder: impl Fn(Respuesta) + Send + 'static,
    ) -> Servicio {
        let (tx, rx) = mpsc::channel::<Pedido>();
        std::thread::Builder::new()
            .name("plugins".into())
            .spawn(move || {
                let mut errores = vec![];
                for pedido in rx {
                    let r = match pedido {
                        Pedido::AlGuardar { archivo, texto } => {
                            let resultado = sup.al_guardar(&archivo, &texto);
                            Respuesta::Diagnosticos { archivo, resultado }
                        }
                        Pedido::Comandos => Respuesta::Comandos(sup.comandos()),
                        Pedido::Ejecutar { comando, ctx } => {
                            let resultado = sup.ejecutar(&comando, &ctx);
                            Respuesta::Ejecutado { comando, resultado }
                        }
                    };
                    responder(r);
                    let ahora = sup.diagnosticos_propios();
                    if ahora != errores {
                        errores = ahora;
                        responder(Respuesta::ErroresPropios(errores.clone()));
                    }
                }
            })
            .expect("no se pudo crear el hilo de plugins");
        Servicio { tx }
    }

    pub fn pedir(&self, p: Pedido) {
        let _ = self.tx.send(p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hay_python() -> bool {
        Command::new(python_por_defecto())
            .arg("--version")
            .output()
            .is_ok()
    }

    fn proyecto(plugins: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("plugins");
        std::fs::create_dir(&p).unwrap();
        for (nombre, codigo) in plugins {
            std::fs::write(p.join(nombre), codigo).unwrap();
        }
        dir
    }

    const TODO: &str = r#"
def al_guardar(archivo, texto):
    return [{"linea": i + 1, "columna": l.find("TODO") + 1, "mensaje": "queda un TODO", "severidad": "nota"}
            for i, l in enumerate(texto.splitlines()) if "TODO" in l]

def comando_contar_lineas(ctx):
    return f"{len(ctx['texto'].splitlines())} líneas"
"#;

    #[test]
    fn sin_plugins_no_lanza_worker() {
        let dir = proyecto(&[]);
        let mut s = Supervisor::nuevo(&dir.path().join("plugins"), None);
        assert_eq!(
            s.al_guardar(Path::new("a.c"), "x"),
            Err(ErrorPlugin::SinPlugins)
        );
        assert!(!s.activo());
    }

    #[test]
    fn diagnosticos_y_comandos() {
        if !hay_python() {
            return;
        }
        let dir = proyecto(&[("todo.py", TODO)]);
        let mut s = Supervisor::nuevo(&dir.path().join("plugins"), None);
        let d = s
            .al_guardar(Path::new("a.c"), "int x;\n// TODO: algo\n")
            .unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!((d[0].linea, d[0].columna), (2, 4));
        assert!(d[0].mensaje.contains("[todo]"));
        assert_eq!(s.plugins[0].comandos, vec!["contar_lineas".to_string()]);
        let cmds = s.comandos().unwrap();
        let ctx = Contexto {
            texto: "a\nb\nc".into(),
            ..Default::default()
        };
        assert_eq!(s.ejecutar(&cmds[0], &ctx).unwrap(), "3 líneas");
    }

    #[test]
    fn bucle_infinito_reinicia_el_worker() {
        if !hay_python() {
            return;
        }
        let dir = proyecto(&[("colgado.py", "def comando_colgar(ctx):\n    while True: pass\n\ndef comando_ok(ctx):\n    return 'ok'\n")]);
        let mut s = Supervisor::nuevo(&dir.path().join("plugins"), None);
        s.tiempo_limite = Duration::from_millis(500);
        let colgar = ComandoPlugin {
            plugin: "colgado".into(),
            comando: "colgar".into(),
        };
        assert_eq!(
            s.ejecutar(&colgar, &Contexto::default()),
            Err(ErrorPlugin::TiempoAgotado)
        );
        assert!(!s.activo());
        s.tiempo_limite = TIEMPO_LIMITE;
        let ok = ComandoPlugin {
            plugin: "colgado".into(),
            comando: "ok".into(),
        };
        assert_eq!(s.ejecutar(&ok, &Contexto::default()).unwrap(), "ok");
        assert_eq!(s.reinicios, 1);
    }

    #[test]
    fn plugin_roto_reporta_error_sin_tumbar_a_los_demas() {
        if !hay_python() {
            return;
        }
        let dir = proyecto(&[
            ("a_roto.py", "import modulo_inexistente\n"),
            ("todo.py", TODO),
        ]);
        let mut s = Supervisor::nuevo(&dir.path().join("plugins"), None);
        assert!(s.al_guardar(Path::new("a.c"), "TODO").unwrap().len() == 1);
        assert_eq!(s.errores_carga.len(), 1);
        assert_eq!(s.errores_carga[0].plugin, "a_roto");
        assert_eq!(s.errores_carga[0].linea, 1);
        assert!(s.errores_carga[0].mensaje.contains("modulo_inexistente"));
    }

    #[test]
    fn errores_de_un_plugin_con_su_linea() {
        if !hay_python() {
            return;
        }
        let dir = proyecto(&[
            ("sintaxis.py", "x = 1\n\ndef f(:\n    pass\n"),
            (
                "revienta.py",
                "def al_guardar(archivo, texto):\n    return 1 / 0\n",
            ),
        ]);
        let mut s = Supervisor::nuevo(&dir.path().join("plugins"), None);
        s.al_guardar(Path::new("a.c"), "").unwrap();
        let d = s.diagnosticos_propios();
        assert_eq!(d.len(), 2, "{d:#?}");
        let sintaxis = d.iter().find(|d| d.mensaje.contains("[sintaxis]")).unwrap();
        assert_eq!(sintaxis.linea, 3);
        assert!(sintaxis.archivo.as_ref().unwrap().ends_with("sintaxis.py"));
        let revienta = d.iter().find(|d| d.mensaje.contains("[revienta]")).unwrap();
        assert_eq!(revienta.linea, 2);
        assert!(revienta.mensaje.contains("ZeroDivisionError"));
    }

    #[test]
    fn modo_examen_filtra_por_sha256() {
        let dir = proyecto(&[("bueno.py", TODO), ("malo.py", "x = 1\n")]);
        let p = dir.path().join("plugins");
        let hash = sha256_archivo(&p.join("bueno.py")).unwrap();
        let s = Supervisor::nuevo(&p, Some(&[hash]));
        assert_eq!(s.scripts, vec![p.join("bueno.py")]);
        assert_eq!(s.rechazados, vec![p.join("malo.py")]);
    }

    #[test]
    fn extension_nativa_de_sangria() {
        let mut r = Registro::de_fabrica();
        let d = r.diagnosticar(Path::new("a.c"), "int main(void)\n{\n\t  return 0;\n}\n");
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].linea, 3);
        assert!(r.diagnosticar(Path::new("a.md"), "\t  x").is_empty());
    }
}
