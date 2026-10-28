//! deck: terminal interna de jonesy.
//!
//! Un proceso (shell o comando) corre sobre un PTY real (`portable-pty`); un hilo lector
//! vuelca su salida en una pantalla virtual `vt100` que la TUI dibuja. Para no redibujar
//! por cada byte, el hilo sólo notifica cuando no hay ya un redibujo pendiente: la TUI
//! llama a [`Deck::tomar_pendiente`] al dibujar, lo que coalesce ráfagas de salida.
//!
//! [`PtyInferior`] abre un PTY sin proceso: GDB lo usa como terminal del programa
//! depurado (`-inferior-tty-set`) para separar su E/S de la consola de GDB.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize, SlavePty};

pub use vt100::Screen;

const SCROLLBACK: usize = 1000;

fn tamano(filas: u16, cols: u16) -> PtySize {
    PtySize {
        rows: filas.max(2),
        cols: cols.max(2),
        pixel_width: 0,
        pixel_height: 0,
    }
}

/// Hilo lector compartido por el deck y el PTY del inferior.
fn leer_en_hilo(
    mut lector: Box<dyn Read + Send>,
    parser: Arc<Mutex<vt100::Parser>>,
    vivo: Arc<AtomicBool>,
    pendiente: Arc<AtomicBool>,
    notificar: Arc<dyn Fn() + Send + Sync>,
    salida: Option<Arc<Mutex<Vec<u8>>>>,
) {
    std::thread::Builder::new()
        .name("deck-lector".into())
        .spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match lector.read(&mut buf) {
                    // Una señal interrumpió la lectura: no es el fin de la salida.
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if let Ok(mut p) = parser.lock() {
                            p.process(&buf[..n]);
                        }
                        if let Some(s) = &salida {
                            if let Ok(mut guard) = s.lock() {
                                if guard.len() < 2 * 1024 * 1024 {
                                    guard.extend_from_slice(&buf[..n]);
                                }
                            }
                        }
                        if !pendiente.swap(true, Ordering::AcqRel) {
                            notificar();
                        }
                    }
                }
            }
            vivo.store(false, Ordering::Release);
            pendiente.store(true, Ordering::Release);
            notificar();
        })
        .expect("no se pudo crear el hilo lector del deck");
}

/// Terminal interna con un proceso hijo.
pub struct Deck {
    parser: Arc<Mutex<vt100::Parser>>,
    escritor: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    hijo: Box<dyn Child + Send + Sync>,
    vivo: Arc<AtomicBool>,
    pendiente: Arc<AtomicBool>,
    salida: Arc<Mutex<Vec<u8>>>,
    pub titulo: String,
    /// Cuándo se vio que el proceso terminó (para esperar a leer su última salida).
    termino: Option<std::time::Instant>,
}

impl Deck {
    /// Lanza `programa args` (o el shell del usuario si `programa` es None) en `dir`.
    pub fn iniciar(
        programa: Option<&str>,
        args: &[String],
        dir: &Path,
        filas: u16,
        cols: u16,
        notificar: impl Fn() + Send + Sync + 'static,
    ) -> std::io::Result<Deck> {
        let sistema = native_pty_system();
        let par = sistema
            .openpty(tamano(filas, cols))
            .map_err(std::io::Error::other)?;
        let prog = programa
            .map(str::to_string)
            .unwrap_or_else(shell_por_defecto);
        let mut cmd = CommandBuilder::new(&prog);
        cmd.args(args);
        cmd.cwd(dir);
        cmd.env("TERM", "xterm-256color");
        cmd.env("JONESY_DECK", "1");
        let hijo = par
            .slave
            .spawn_command(cmd)
            .map_err(std::io::Error::other)?;
        drop(par.slave);
        let lector = par
            .master
            .try_clone_reader()
            .map_err(std::io::Error::other)?;
        let escritor = par.master.take_writer().map_err(std::io::Error::other)?;
        let parser = Arc::new(Mutex::new(vt100::Parser::new(
            filas.max(2),
            cols.max(2),
            SCROLLBACK,
        )));
        let vivo = Arc::new(AtomicBool::new(true));
        let pendiente = Arc::new(AtomicBool::new(false));
        let salida = Arc::new(Mutex::new(Vec::new()));
        leer_en_hilo(
            lector,
            parser.clone(),
            vivo.clone(),
            pendiente.clone(),
            Arc::new(notificar),
            Some(salida.clone()),
        );
        let titulo = std::iter::once(prog.as_str())
            .chain(args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ");
        Ok(Deck {
            parser,
            escritor,
            master: par.master,
            hijo,
            vivo,
            pendiente,
            salida,
            titulo,
            termino: None,
        })
    }

    /// Envía bytes al proceso (teclas ya traducidas a secuencias de terminal).
    pub fn escribir(&mut self, datos: &[u8]) -> std::io::Result<()> {
        self.escritor.write_all(datos)?;
        self.escritor.flush()
    }

    pub fn redimensionar(&mut self, filas: u16, cols: u16) {
        let _ = self.master.resize(tamano(filas, cols));
        if let Ok(mut p) = self.parser.lock() {
            p.set_size(filas.max(2), cols.max(2));
        }
    }

    /// Ejecuta `f` sobre la pantalla virtual actual.
    pub fn con_pantalla<R>(&self, f: impl FnOnce(&Screen) -> R) -> R {
        let p = self.parser.lock().unwrap_or_else(|e| e.into_inner());
        f(p.screen())
    }

    /// Desplaza la vista hacia atrás en el historial (0 = al día).
    pub fn desplazar(&self, lineas: usize) {
        if let Ok(mut p) = self.parser.lock() {
            p.set_scrollback(lineas);
        }
    }

    /// true si hubo salida nueva desde el último dibujo (y limpia la marca).
    pub fn tomar_pendiente(&self) -> bool {
        self.pendiente.swap(false, Ordering::AcqRel)
    }

    /// El proceso sigue corriendo, o terminó pero todavía se está leyendo lo último que
    /// escribió (hasta 1 s). Así, cuando dice que no, la pantalla ya tiene toda la salida.
    pub fn vivo(&mut self) -> bool {
        if matches!(self.hijo.try_wait(), Ok(None)) {
            return true;
        }
        let lector_abierto = self.vivo.load(Ordering::Acquire);
        let desde = *self.termino.get_or_insert_with(std::time::Instant::now);
        lector_abierto && desde.elapsed() < std::time::Duration::from_secs(1)
    }

    /// Código de salida si el proceso terminó.
    pub fn codigo_salida(&mut self) -> Option<u32> {
        self.hijo.try_wait().ok().flatten().map(|e| e.exit_code())
    }

    pub fn terminar(&mut self) {
        let _ = self.hijo.kill();
    }

    /// PID del proceso (para enviarle señales).
    pub fn pid(&self) -> Option<u32> {
        self.hijo.process_id()
    }

    /// Texto visible (sin atributos); útil para pruebas y para copiar.
    pub fn texto(&self) -> String {
        if let Ok(s) = self.salida.lock() {
            if !s.is_empty() {
                return String::from_utf8_lossy(&s).to_string();
            }
        }
        self.con_pantalla(|s| s.contents())
    }
}

impl Drop for Deck {
    fn drop(&mut self) {
        self.terminar();
    }
}

/// PTY sin proceso para la E/S del programa depurado.
#[cfg_attr(not(unix), allow(dead_code))]
pub struct PtyInferior {
    parser: Arc<Mutex<vt100::Parser>>,
    escritor: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    // El esclavo se mantiene abierto para que el PTY no reporte EOF antes de que el
    // inferior lo abra.
    _esclavo: Box<dyn SlavePty + Send>,
    pendiente: Arc<AtomicBool>,
    ruta: String,
}

impl PtyInferior {
    #[cfg(unix)]
    pub fn abrir(
        filas: u16,
        cols: u16,
        notificar: impl Fn() + Send + Sync + 'static,
    ) -> std::io::Result<PtyInferior> {
        let par = native_pty_system()
            .openpty(tamano(filas, cols))
            .map_err(std::io::Error::other)?;
        let fd = par
            .master
            .as_raw_fd()
            .ok_or_else(|| std::io::Error::other("el PTY no expone su descriptor"))?;
        let ruta = ruta_esclavo(fd)?;
        let lector = par
            .master
            .try_clone_reader()
            .map_err(std::io::Error::other)?;
        let escritor = par.master.take_writer().map_err(std::io::Error::other)?;
        let parser = Arc::new(Mutex::new(vt100::Parser::new(
            filas.max(2),
            cols.max(2),
            SCROLLBACK,
        )));
        let pendiente = Arc::new(AtomicBool::new(false));
        leer_en_hilo(
            lector,
            parser.clone(),
            Arc::new(AtomicBool::new(true)),
            pendiente.clone(),
            Arc::new(notificar),
            None,
        );
        Ok(PtyInferior {
            parser,
            escritor,
            master: par.master,
            _esclavo: par.slave,
            pendiente,
            ruta,
        })
    }

    /// En Windows no hay un PTY con ruta para pasarle a GDB: el programa depurado
    /// comparte la consola con GDB.
    #[cfg(not(unix))]
    pub fn abrir(
        _filas: u16,
        _cols: u16,
        _notificar: impl Fn() + Send + Sync + 'static,
    ) -> std::io::Result<PtyInferior> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "en Windows el programa depurado usa la consola de GDB",
        ))
    }

    /// Ruta del dispositivo (/dev/pts/N) para `-inferior-tty-set`.
    pub fn ruta(&self) -> &str {
        &self.ruta
    }

    pub fn escribir(&mut self, datos: &[u8]) -> std::io::Result<()> {
        self.escritor.write_all(datos)?;
        self.escritor.flush()
    }

    pub fn redimensionar(&mut self, filas: u16, cols: u16) {
        let _ = self.master.resize(tamano(filas, cols));
        if let Ok(mut p) = self.parser.lock() {
            p.set_size(filas.max(2), cols.max(2));
        }
    }

    pub fn con_pantalla<R>(&self, f: impl FnOnce(&Screen) -> R) -> R {
        let p = self.parser.lock().unwrap_or_else(|e| e.into_inner());
        f(p.screen())
    }

    pub fn tomar_pendiente(&self) -> bool {
        self.pendiente.swap(false, Ordering::AcqRel)
    }
}

/// Shell del deck: `$SHELL`; en Windows bash (UCRT64/MSYS2, Git Bash) si está en el
/// PATH y si no `%COMSPEC%`.
pub fn shell_por_defecto() -> String {
    if cfg!(windows) {
        if en_path("bash") {
            return "bash".into();
        }
        return std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into());
    }
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into())
}

fn en_path(programa: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| {
        std::env::split_paths(&p)
            .any(|d| d.join(programa).is_file() || d.join(format!("{programa}.exe")).is_file())
    })
}

/// Ruta del esclavo de un PTY a partir del descriptor del maestro.
#[cfg(target_os = "linux")]
fn ruta_esclavo(fd: std::os::unix::io::RawFd) -> std::io::Result<String> {
    let mut buf = [0 as libc::c_char; 128];
    // SAFETY: buf es válido y su largo se pasa explícitamente.
    let r = unsafe { libc::ptsname_r(fd, buf.as_mut_ptr(), buf.len()) };
    if r != 0 {
        return Err(std::io::Error::from_raw_os_error(r));
    }
    // SAFETY: ptsname_r deja una cadena terminada en NUL dentro de buf.
    let c = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) };
    Ok(c.to_string_lossy().into_owned())
}

/// macOS y los BSD no tienen ptsname_r: ptsname usa un búfer estático, así que se
/// serializa con un mutex.
#[cfg(all(unix, not(target_os = "linux")))]
fn ruta_esclavo(fd: std::os::unix::io::RawFd) -> std::io::Result<String> {
    static CANDADO: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _g = CANDADO.lock().unwrap_or_else(|e| e.into_inner());
    // SAFETY: ptsname devuelve NULL o una cadena terminada en NUL válida hasta la
    // próxima llamada, que el mutex impide mientras se copia.
    let p = unsafe { libc::ptsname(fd) };
    if p.is_null() {
        return Err(std::io::Error::last_os_error());
    }
    let c = unsafe { std::ffi::CStr::from_ptr(p) };
    Ok(c.to_string_lossy().into_owned())
}

/// Traduce una tecla a los bytes que espera una terminal xterm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tecla {
    Caracter(char),
    Ctrl(char),
    Alt(char),
    Enter,
    Retroceso,
    Tab,
    Esc,
    Arriba,
    Abajo,
    Izquierda,
    Derecha,
    Inicio,
    Fin,
    RePag,
    AvPag,
    Suprimir,
}

pub fn bytes_de_tecla(t: Tecla) -> Vec<u8> {
    match t {
        Tecla::Caracter(c) => c.to_string().into_bytes(),
        Tecla::Ctrl(c) => {
            let c = c.to_ascii_lowercase();
            if c.is_ascii_lowercase() {
                vec![c as u8 - b'a' + 1]
            } else {
                match c {
                    '[' => vec![0x1b],
                    '\\' => vec![0x1c],
                    ']' => vec![0x1d],
                    ' ' | '@' => vec![0],
                    _ => vec![],
                }
            }
        }
        Tecla::Alt(c) => {
            let mut v = vec![0x1b];
            v.extend(c.to_string().bytes());
            v
        }
        Tecla::Enter => b"\r".to_vec(),
        Tecla::Retroceso => vec![0x7f],
        Tecla::Tab => b"\t".to_vec(),
        Tecla::Esc => vec![0x1b],
        Tecla::Arriba => b"\x1b[A".to_vec(),
        Tecla::Abajo => b"\x1b[B".to_vec(),
        Tecla::Derecha => b"\x1b[C".to_vec(),
        Tecla::Izquierda => b"\x1b[D".to_vec(),
        Tecla::Inicio => b"\x1b[H".to_vec(),
        Tecla::Fin => b"\x1b[F".to_vec(),
        Tecla::RePag => b"\x1b[5~".to_vec(),
        Tecla::AvPag => b"\x1b[6~".to_vec(),
        Tecla::Suprimir => b"\x1b[3~".to_vec(),
    }
}

#[cfg(test)]
// En Windows sólo corren las pruebas que no necesitan sh.
#[cfg_attr(not(unix), allow(dead_code, unused_imports))]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn esperar(mut cond: impl FnMut() -> bool) -> bool {
        let fin = Instant::now() + Duration::from_secs(5);
        while Instant::now() < fin {
            if cond() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    #[test]
    #[cfg(unix)]
    fn ejecuta_comando_y_muestra_salida() {
        let dir = tempfile::tempdir().unwrap();
        let mut deck = Deck::iniciar(
            Some("sh"),
            &["-c".into(), "echo hola deck".into()],
            dir.path(),
            10,
            40,
            || {},
        )
        .unwrap();
        assert!(
            esperar(|| deck.texto().contains("hola deck")),
            "pantalla: {:?}",
            deck.texto()
        );
        assert!(esperar(|| !deck.vivo()));
        assert!(deck.tomar_pendiente());
    }

    #[test]
    #[cfg(unix)]
    fn shell_interactivo_recibe_teclas() {
        let dir = tempfile::tempdir().unwrap();
        let mut deck = Deck::iniciar(Some("sh"), &[], dir.path(), 10, 40, || {}).unwrap();
        deck.escribir(b"echo $((6*7))\r").unwrap();
        assert!(
            esperar(|| deck.texto().contains("42")),
            "pantalla: {:?}",
            deck.texto()
        );
        deck.escribir(&bytes_de_tecla(Tecla::Ctrl('d'))).unwrap();
        assert!(esperar(|| !deck.vivo()));
    }

    #[test]
    #[cfg(unix)]
    fn pty_inferior_tiene_ruta() {
        let pty = PtyInferior::abrir(5, 20, || {}).unwrap();
        assert!(pty.ruta().starts_with("/dev/"));
        std::fs::OpenOptions::new()
            .write(true)
            .open(pty.ruta())
            .unwrap()
            .write_all(b"inferior")
            .unwrap();
        assert!(esperar(|| pty
            .con_pantalla(|s| s.contents())
            .contains("inferior")));
    }

    #[test]
    fn teclas() {
        assert_eq!(bytes_de_tecla(Tecla::Ctrl('c')), vec![3]);
        assert_eq!(bytes_de_tecla(Tecla::Arriba), b"\x1b[A");
        assert_eq!(bytes_de_tecla(Tecla::Alt('x')), b"\x1bx");
    }
}
