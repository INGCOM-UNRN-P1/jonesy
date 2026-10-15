//! Controlador de GDB por la interfaz máquina (GDB/MI3).
//!
//! GDB corre como proceso hijo con `--interpreter=mi3`; un hilo lee su salida, la
//! interpreta con [`parsear_linea`] y la deja en un canal que la interfaz consulta con
//! [`Gdb::recibir`]. La E/S del programa depurado va a otro PTY (`-inferior-tty-set`) para
//! que `scanf`/`printf` no se mezclen con la consola del depurador.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;

// ------------------------------------------------------------------ valores MI

/// Valor MI: cadena C, tupla `{a=..}` o lista `[..]`.
#[derive(Debug, Clone, PartialEq)]
pub enum Valor {
    Cadena(String),
    Tupla(BTreeMap<String, Valor>),
    Lista(Vec<Valor>),
}

impl Valor {
    pub fn get(&self, clave: &str) -> Option<&Valor> {
        match self {
            Valor::Tupla(m) => m.get(clave),
            _ => None,
        }
    }

    pub fn texto(&self, clave: &str) -> Option<&str> {
        match self.get(clave)? {
            Valor::Cadena(s) => Some(s),
            _ => None,
        }
    }

    pub fn lista(&self) -> &[Valor] {
        match self {
            Valor::Lista(v) => v,
            _ => &[],
        }
    }

    pub fn como_texto(&self) -> Option<&str> {
        match self {
            Valor::Cadena(s) => Some(s),
            _ => None,
        }
    }
}

/// Clase de un registro de salida MI.
#[derive(Debug, Clone, PartialEq)]
pub enum Registro {
    /// `^done`, `^running`, `^error`, `^exit`, `^connected` con su token.
    Resultado {
        token: Option<u64>,
        clase: String,
        datos: Valor,
    },
    /// `*stopped`, `*running`.
    Ejecucion { clase: String, datos: Valor },
    /// `=breakpoint-modified`, `=thread-group-exited`, ...
    Notificacion { clase: String, datos: Valor },
    /// `~"texto"`: salida de la consola de GDB.
    Consola(String),
    /// `@"texto"`: salida del programa (cuando no hay tty separada).
    Programa(String),
    /// `&"texto"`: registro interno de GDB (eco de comandos, errores).
    Log(String),
    /// `(gdb)`.
    Indicador,
    /// Línea que no es MI (avisos sueltos).
    Otro(String),
}

struct Lector<'a> {
    s: &'a [u8],
    i: usize,
}

impl Lector<'_> {
    fn ver(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn cadena_c(&mut self) -> Option<String> {
        if self.ver()? != b'"' {
            return None;
        }
        self.i += 1;
        let mut bytes = Vec::new();
        while let Some(c) = self.ver() {
            self.i += 1;
            match c {
                b'"' => return Some(String::from_utf8_lossy(&bytes).into_owned()),
                b'\\' => {
                    let e = self.ver()?;
                    self.i += 1;
                    match e {
                        b'n' => bytes.push(b'\n'),
                        b't' => bytes.push(b'\t'),
                        b'r' => bytes.push(b'\r'),
                        b'e' => bytes.push(0x1b),
                        b'0'..=b'7' => {
                            // Octal de hasta 3 dígitos (GDB escapa así los bytes no ASCII).
                            let mut v = (e - b'0') as u32;
                            for _ in 0..2 {
                                match self.ver() {
                                    Some(d @ b'0'..=b'7') => {
                                        v = v * 8 + (d - b'0') as u32;
                                        self.i += 1;
                                    }
                                    _ => break,
                                }
                            }
                            bytes.push(v as u8);
                        }
                        otro => bytes.push(otro),
                    }
                }
                otro => bytes.push(otro),
            }
        }
        None
    }

    fn nombre(&mut self) -> String {
        let ini = self.i;
        while let Some(c) = self.ver() {
            if c == b'=' || c == b',' || c == b'}' || c == b']' {
                break;
            }
            self.i += 1;
        }
        String::from_utf8_lossy(&self.s[ini..self.i]).into_owned()
    }

    fn valor(&mut self) -> Option<Valor> {
        match self.ver()? {
            b'"' => self.cadena_c().map(Valor::Cadena),
            b'{' => {
                self.i += 1;
                let mut m = BTreeMap::new();
                while self.ver()? != b'}' {
                    let k = self.nombre();
                    if self.ver()? == b'=' {
                        self.i += 1;
                    }
                    m.insert(k, self.valor()?);
                    if self.ver()? == b',' {
                        self.i += 1;
                    }
                }
                self.i += 1;
                Some(Valor::Tupla(m))
            }
            b'[' => {
                self.i += 1;
                let mut v = Vec::new();
                while self.ver()? != b']' {
                    if matches!(self.ver()?, b'"' | b'{' | b'[') {
                        v.push(self.valor()?);
                    } else {
                        // Lista de resultados `[frame={..},frame={..}]`: se guarda el valor.
                        let _ = self.nombre();
                        if self.ver()? == b'=' {
                            self.i += 1;
                        }
                        v.push(self.valor()?);
                    }
                    if self.ver()? == b',' {
                        self.i += 1;
                    }
                }
                self.i += 1;
                Some(Valor::Lista(v))
            }
            _ => None,
        }
    }

    /// `clase,k=v,k=v` → (clase, tupla).
    fn clase_y_resultados(&mut self) -> Option<(String, Valor)> {
        let ini = self.i;
        while let Some(c) = self.ver() {
            if c == b',' {
                break;
            }
            self.i += 1;
        }
        let clase = String::from_utf8_lossy(&self.s[ini..self.i]).into_owned();
        let mut m = BTreeMap::new();
        while self.ver() == Some(b',') {
            self.i += 1;
            let k = self.nombre();
            if self.ver() == Some(b'=') {
                self.i += 1;
            }
            m.insert(k, self.valor()?);
        }
        Some((clase, Valor::Tupla(m)))
    }
}

pub fn parsear_linea(linea: &str) -> Registro {
    let linea = linea.trim_end_matches(['\r', '\n']);
    if linea.trim() == "(gdb)" {
        return Registro::Indicador;
    }
    let bytes = linea.as_bytes();
    let digitos = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
    let token = linea[..digitos].parse().ok();
    let mut l = Lector {
        s: bytes,
        i: digitos,
    };
    let Some(prefijo) = l.ver() else {
        return Registro::Otro(linea.into());
    };
    l.i += 1;
    let r = match prefijo {
        b'^' => l
            .clase_y_resultados()
            .map(|(clase, datos)| Registro::Resultado {
                token,
                clase,
                datos,
            }),
        b'*' => l
            .clase_y_resultados()
            .map(|(clase, datos)| Registro::Ejecucion { clase, datos }),
        b'=' => l
            .clase_y_resultados()
            .map(|(clase, datos)| Registro::Notificacion { clase, datos }),
        b'~' => l.cadena_c().map(Registro::Consola),
        b'@' => l.cadena_c().map(Registro::Programa),
        b'&' => l.cadena_c().map(Registro::Log),
        _ => None,
    };
    r.unwrap_or_else(|| Registro::Otro(linea.into()))
}

// ------------------------------------------------------------------ modelo

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Marco {
    pub nivel: usize,
    pub funcion: String,
    pub archivo: Option<PathBuf>,
    pub linea: Option<usize>,
    pub direccion: String,
}

impl Marco {
    pub fn de_valor(v: &Valor) -> Marco {
        Marco {
            nivel: v.texto("level").and_then(|s| s.parse().ok()).unwrap_or(0),
            funcion: v.texto("func").unwrap_or("??").to_string(),
            archivo: v.texto("fullname").or(v.texto("file")).map(PathBuf::from),
            linea: v.texto("line").and_then(|s| s.parse().ok()),
            direccion: v.texto("addr").unwrap_or("").to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variable {
    pub nombre: String,
    pub tipo: String,
    pub valor: String,
    pub argumento: bool,
}

/// Por qué se detuvo el programa.
#[derive(Debug, Clone, PartialEq)]
pub enum Parada {
    Breakpoint(Marco),
    Paso(Marco),
    Senal {
        nombre: String,
        significado: String,
        marco: Marco,
    },
    Termino(Option<i32>),
    Otra(String, Marco),
}

/// Evento ya interpretado para la interfaz.
#[derive(Debug, Clone, PartialEq)]
pub enum EventoGdb {
    Detenido(Parada),
    Corriendo,
    Marcos(Vec<Marco>),
    Variables(Vec<Variable>),
    Consola(String),
    Error(String),
    BreakpointCreado {
        numero: u32,
        archivo: PathBuf,
        linea: usize,
    },
    /// Nombre y valor completo de cada variable (completa los de `Variables`).
    Valores(Vec<(String, String)>),
    /// El valor de una expresión pedida con `Gdb::evaluar` (o por qué no se pudo).
    Evaluado {
        indice: usize,
        valor: Result<String, String>,
    },
    Salio,
}

pub fn interpretar(r: &Registro, pedido: Option<&Pedido>) -> Vec<EventoGdb> {
    match r {
        Registro::Ejecucion { clase, datos } if clase == "stopped" => {
            let marco = datos.get("frame").map(Marco::de_valor).unwrap_or_default();
            let razon = datos.texto("reason").unwrap_or("");
            let parada = match razon {
                "breakpoint-hit" => Parada::Breakpoint(marco),
                "end-stepping-range" | "function-finished" | "location-reached" => {
                    Parada::Paso(marco)
                }
                "signal-received" => Parada::Senal {
                    nombre: datos.texto("signal-name").unwrap_or("").into(),
                    significado: datos.texto("signal-meaning").unwrap_or("").into(),
                    marco,
                },
                "exited-normally" => Parada::Termino(Some(0)),
                "exited" => Parada::Termino(datos.texto("exit-code").and_then(|c| {
                    i32::from_str_radix(c.trim_start_matches('0'), 8)
                        .ok()
                        .or(Some(0))
                })),
                "exited-signalled" => Parada::Termino(None),
                otra => Parada::Otra(otra.into(), marco),
            };
            vec![EventoGdb::Detenido(parada)]
        }
        Registro::Ejecucion { clase, .. } if clase == "running" => vec![EventoGdb::Corriendo],
        Registro::Consola(t) | Registro::Programa(t) => vec![EventoGdb::Consola(t.clone())],
        Registro::Resultado { clase, datos, .. } if clase == "error" => {
            let msg = datos.texto("msg").unwrap_or("error de GDB").to_string();
            match pedido {
                Some(Pedido::Evaluar(indice)) => vec![EventoGdb::Evaluado {
                    indice: *indice,
                    valor: Err(msg),
                }],
                _ => vec![EventoGdb::Error(msg)],
            }
        }
        Registro::Resultado { clase, .. } if clase == "exit" => vec![EventoGdb::Salio],
        Registro::Resultado { datos, .. } => match pedido {
            Some(Pedido::Marcos) => {
                vec![EventoGdb::Marcos(
                    datos
                        .get("stack")
                        .map(|s| s.lista().iter().map(Marco::de_valor).collect())
                        .unwrap_or_default(),
                )]
            }
            Some(Pedido::Variables) => vec![EventoGdb::Variables(
                datos
                    .get("variables")
                    .map(|v| {
                        v.lista()
                            .iter()
                            .map(|x| Variable {
                                nombre: x.texto("name").unwrap_or("").into(),
                                tipo: x.texto("type").unwrap_or("").into(),
                                valor: x.texto("value").unwrap_or("…").into(),
                                argumento: x.texto("arg") == Some("1"),
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            )],
            Some(Pedido::Evaluar(indice)) => vec![EventoGdb::Evaluado {
                indice: *indice,
                valor: Ok(datos.texto("value").unwrap_or("").to_string()),
            }],
            Some(Pedido::Valores) => vec![EventoGdb::Valores(
                datos
                    .get("variables")
                    .map(|v| {
                        v.lista()
                            .iter()
                            .filter_map(|x| {
                                Some((x.texto("name")?.to_string(), x.texto("value")?.to_string()))
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            )],
            Some(Pedido::Breakpoint) => datos
                .get("bkpt")
                .and_then(|b| {
                    Some(EventoGdb::BreakpointCreado {
                        numero: b.texto("number")?.parse().ok()?,
                        archivo: PathBuf::from(b.texto("fullname").or(b.texto("file"))?),
                        linea: b.texto("line")?.parse().ok()?,
                    })
                })
                .into_iter()
                .collect(),
            _ => vec![],
        },
        _ => vec![],
    }
}

/// Qué se pidió con cada token, para interpretar la respuesta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pedido {
    Marcos,
    Variables,
    Breakpoint,
    /// `-data-evaluate-expression`: el índice dice a qué expresión corresponde.
    Evaluar(usize),
    /// Los valores completos de las variables (arreglos y structs incluidos).
    Valores,
    Otro,
}

// ------------------------------------------------------------------ controlador

pub struct Gdb {
    hijo: Child,
    entrada: ChildStdin,
    eventos: Receiver<Registro>,
    siguiente_token: u64,
    pedidos: BTreeMap<u64, Pedido>,
    /// Breakpoints vivos: (archivo, línea) → número de GDB.
    pub breakpoints: BTreeMap<(PathBuf, usize), u32>,
    pub corriendo: bool,
    pub iniciado: bool,
}

impl Gdb {
    /// Lanza `gdb --interpreter=mi3` sobre `programa`. `tty` es la ruta del PTY donde
    /// correrá el programa depurado.
    pub fn lanzar(
        programa: &Path,
        argumentos: &[String],
        dir: &Path,
        tty: Option<&str>,
        notificar: impl Fn() + Send + Sync + 'static,
    ) -> std::io::Result<Gdb> {
        Self::lanzar_con(programa, argumentos, dir, tty, None, notificar)
    }

    /// Como [`Gdb::lanzar`], cargando además un gdbinit (el del entorno de la cátedra) en
    /// lugar de los del usuario.
    pub fn lanzar_con(
        programa: &Path,
        argumentos: &[String],
        dir: &Path,
        tty: Option<&str>,
        gdbinit: Option<&Path>,
        notificar: impl Fn() + Send + Sync + 'static,
    ) -> std::io::Result<Gdb> {
        let mut cmd = Command::new(std::env::var("JONESY_GDB").unwrap_or_else(|_| "gdb".into()));
        cmd.args(["--interpreter=mi3", "-q", "-nx"]);
        if let Some(g) = gdbinit {
            cmd.arg("-x").arg(g);
        }
        let mut hijo = cmd
            .arg(programa)
            .current_dir(dir)
            .env("LC_ALL", "C")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let entrada = hijo.stdin.take().expect("stdin entubado");
        let salida = hijo.stdout.take().expect("stdout entubado");
        let (tx, rx) = mpsc::channel();
        let notificar = Arc::new(notificar);
        std::thread::Builder::new()
            .name("gdb-mi".into())
            .spawn(move || {
                for linea in BufReader::new(salida).lines() {
                    let Ok(linea) = linea else { break };
                    if tx.send(parsear_linea(&linea)).is_err() {
                        break;
                    }
                    notificar();
                }
                let _ = tx.send(Registro::Resultado {
                    token: None,
                    clase: "exit".into(),
                    datos: Valor::Tupla(BTreeMap::new()),
                });
                notificar();
            })?;
        let mut g = Gdb {
            hijo,
            entrada,
            eventos: rx,
            siguiente_token: 1,
            pedidos: BTreeMap::new(),
            breakpoints: BTreeMap::new(),
            corriendo: false,
            iniciado: false,
        };
        g.enviar("-gdb-set mi-async on", Pedido::Otro)?;
        g.enviar("-gdb-set print pretty off", Pedido::Otro)?;
        g.enviar("-gdb-set confirm off", Pedido::Otro)?;
        if let Some(t) = tty {
            g.enviar(&format!("-inferior-tty-set {t}"), Pedido::Otro)?;
        }
        if !argumentos.is_empty() {
            let args: Vec<String> = argumentos.iter().map(|a| comillas_mi(a)).collect();
            g.enviar(&format!("-exec-arguments {}", args.join(" ")), Pedido::Otro)?;
        }
        Ok(g)
    }

    pub fn enviar(&mut self, comando: &str, pedido: Pedido) -> std::io::Result<u64> {
        let token = self.siguiente_token;
        self.siguiente_token += 1;
        self.pedidos.insert(token, pedido);
        writeln!(self.entrada, "{token}{comando}")?;
        self.entrada.flush()?;
        Ok(token)
    }

    /// Correr hasta `archivo:linea`: un breakpoint temporal y continuar.
    pub fn correr_hasta(&mut self, archivo: &Path, linea: usize) -> std::io::Result<()> {
        self.poner_breakpoint_temporal(archivo, linea)?;
        self.continuar()
    }

    /// Un breakpoint que se borra solo al detenerse (`-break-insert -t`).
    pub fn poner_breakpoint_temporal(
        &mut self,
        archivo: &Path,
        linea: usize,
    ) -> std::io::Result<()> {
        self.enviar(
            &format!(
                "-break-insert -t {}",
                comillas_mi(&format!("{}:{linea}", archivo.display()))
            ),
            Pedido::Otro,
        )
        .map(|_| ())
    }

    pub fn poner_breakpoint(&mut self, archivo: &Path, linea: usize) -> std::io::Result<()> {
        self.enviar(
            &format!(
                "-break-insert {}",
                comillas_mi(&format!("{}:{linea}", archivo.display()))
            ),
            Pedido::Breakpoint,
        )
        .map(|_| ())
    }

    pub fn quitar_breakpoint(&mut self, archivo: &Path, linea: usize) -> std::io::Result<()> {
        let claves: Vec<_> = self
            .breakpoints
            .keys()
            .filter(|(a, l)| *l == linea && (a == archivo || a.file_name() == archivo.file_name()))
            .cloned()
            .collect();
        for k in claves {
            if let Some(n) = self.breakpoints.remove(&k) {
                self.enviar(&format!("-break-delete {n}"), Pedido::Otro)?;
            }
        }
        Ok(())
    }

    pub fn correr(&mut self) -> std::io::Result<()> {
        self.iniciado = true;
        self.enviar("-exec-run", Pedido::Otro).map(|_| ())
    }

    pub fn continuar(&mut self) -> std::io::Result<()> {
        if !self.iniciado {
            return self.correr();
        }
        self.enviar("-exec-continue", Pedido::Otro).map(|_| ())
    }

    pub fn paso(&mut self) -> std::io::Result<()> {
        self.enviar("-exec-next", Pedido::Otro).map(|_| ())
    }

    pub fn entrar(&mut self) -> std::io::Result<()> {
        self.enviar("-exec-step", Pedido::Otro).map(|_| ())
    }

    pub fn salir(&mut self) -> std::io::Result<()> {
        self.enviar("-exec-finish", Pedido::Otro).map(|_| ())
    }

    pub fn interrumpir(&mut self) -> std::io::Result<()> {
        self.enviar("-exec-interrupt", Pedido::Otro).map(|_| ())
    }

    /// Pide marcos y variables del marco actual (las respuestas llegan como eventos).
    pub fn pedir_estado(&mut self) -> std::io::Result<()> {
        self.enviar("-stack-list-frames 0 20", Pedido::Marcos)?;
        // --simple-values trae los tipos; --all-values, el valor de arreglos y structs.
        self.enviar("-stack-list-variables --simple-values", Pedido::Variables)?;
        self.enviar("-stack-list-variables --all-values", Pedido::Valores)?;
        Ok(())
    }

    /// Evalúa una expresión en la parada actual (`indice` vuelve en `EventoGdb::Evaluado`).
    pub fn evaluar(&mut self, expresion: &str, indice: usize) -> std::io::Result<()> {
        self.enviar(
            &format!("-data-evaluate-expression {}", comillas_mi(expresion)),
            Pedido::Evaluar(indice),
        )
        .map(|_| ())
    }

    /// Pasa al marco `nivel` de la pila (sus variables pasan a ser las locales).
    pub fn seleccionar_marco(&mut self, nivel: usize) -> std::io::Result<()> {
        self.enviar(&format!("-stack-select-frame {nivel}"), Pedido::Otro)?;
        self.enviar("-stack-list-variables --simple-values", Pedido::Variables)?;
        self.enviar("-stack-list-variables --all-values", Pedido::Valores)?;
        Ok(())
    }

    /// Comando de consola (`print x`, `info breakpoints`...).
    pub fn consola(&mut self, comando: &str) -> std::io::Result<()> {
        self.enviar(
            &format!("-interpreter-exec console {}", comillas_mi(comando)),
            Pedido::Otro,
        )
        .map(|_| ())
    }

    /// Eventos pendientes, ya interpretados.
    pub fn recibir(&mut self) -> Vec<EventoGdb> {
        let mut eventos = vec![];
        while let Ok(r) = self.eventos.try_recv() {
            let pedido = match &r {
                Registro::Resultado { token: Some(t), .. } => self.pedidos.remove(t),
                _ => None,
            };
            for e in interpretar(&r, pedido.as_ref()) {
                match &e {
                    EventoGdb::Corriendo => self.corriendo = true,
                    EventoGdb::Detenido(p) => {
                        self.corriendo = false;
                        if matches!(p, Parada::Termino(_)) {
                            self.iniciado = false;
                        }
                    }
                    EventoGdb::BreakpointCreado {
                        numero,
                        archivo,
                        linea,
                    } => {
                        self.breakpoints.insert((archivo.clone(), *linea), *numero);
                    }
                    _ => {}
                }
                eventos.push(e);
            }
        }
        eventos
    }

    pub fn terminar(&mut self) {
        let _ = writeln!(self.entrada, "-gdb-exit");
        let _ = self.entrada.flush();
        std::thread::sleep(std::time::Duration::from_millis(50));
        let _ = self.hijo.kill();
        let _ = self.hijo.wait();
    }
}

impl Drop for Gdb {
    fn drop(&mut self) {
        let _ = self.hijo.kill();
        let _ = self.hijo.wait();
    }
}

/// Cadena C para un argumento MI.
/// Cómo mostrar el valor de una variable en el panel.
#[derive(Debug, Clone, PartialEq)]
pub enum Presentacion {
    Texto(String),
    /// Una cadena de C, ya sin comillas ni los `\000` del final.
    Cadena(String),
    /// Un arreglo: cada elemento en orden (los `<repeats N times>` expandidos).
    Tabla(Vec<String>),
}

/// Separa por comas de primer nivel (fuera de llaves y comillas).
fn elementos(interior: &str) -> Vec<String> {
    let (mut v, mut actual, mut nivel, mut en_cadena, mut escape) =
        (vec![], String::new(), 0, false, false);
    for c in interior.chars() {
        if en_cadena {
            actual.push(c);
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                en_cadena = false;
            }
            continue;
        }
        match c {
            '"' => en_cadena = true,
            '{' => nivel += 1,
            '}' => nivel -= 1,
            ',' if nivel == 0 => {
                v.push(actual.trim().to_string());
                actual.clear();
                continue;
            }
            _ => {}
        }
        actual.push(c);
    }
    if !actual.trim().is_empty() {
        v.push(actual.trim().to_string());
    }
    v
}

/// Arreglos como tabla y cadenas como texto (#30); el resto, como lo da gdb.
pub fn presentar(tipo: &str, valor: &str) -> Presentacion {
    let es_char = tipo.contains("char");
    let es_arreglo = tipo.contains('[');
    // `0x4005d4 "hola"` (char *) o `"hola", '\000' <repeats 15 times>` (char [20]).
    if es_char && (es_arreglo || tipo.contains('*')) {
        if let (Some(i), Some(j)) = (valor.find('"'), valor.rfind('"')) {
            if j > i {
                let mut s = valor[i + 1..j].to_string();
                while let Some(r) = s.strip_suffix("\\000") {
                    s = r.to_string();
                }
                return Presentacion::Cadena(s);
            }
        }
    }
    if es_arreglo && valor.starts_with('{') && valor.ends_with('}') && !es_char {
        let mut v = vec![];
        for e in elementos(&valor[1..valor.len() - 1]) {
            match e.split_once(" <repeats ") {
                Some((x, resto)) => {
                    let n: usize = resto
                        .trim_end_matches(" times>")
                        .trim()
                        .parse()
                        .unwrap_or(1);
                    v.extend(std::iter::repeat_n(x.trim().to_string(), n.min(1000)));
                }
                None => v.push(e),
            }
        }
        return Presentacion::Tabla(v);
    }
    Presentacion::Texto(valor.to_string())
}

pub fn comillas_mi(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arreglos_como_tabla_y_cadenas_como_texto() {
        assert_eq!(
            presentar("int [5]", "{1, 2, 0 <repeats 3 times>}"),
            Presentacion::Tabla(vec![
                "1".into(),
                "2".into(),
                "0".into(),
                "0".into(),
                "0".into()
            ])
        );
        assert_eq!(
            presentar("char [10]", "\"hola\", '\\000' <repeats 5 times>"),
            Presentacion::Cadena("hola".into())
        );
        assert_eq!(
            presentar("char *", "0x4006d4 \"hola, mundo\""),
            Presentacion::Cadena("hola, mundo".into())
        );
        assert_eq!(presentar("int", "5"), Presentacion::Texto("5".into()));
        assert_eq!(
            presentar("struct punto [2]", "{{x = 1, y = 2}, {x = 3, y = 4}}"),
            Presentacion::Tabla(vec!["{x = 1, y = 2}".into(), "{x = 3, y = 4}".into()])
        );
    }

    #[test]
    fn evaluar_responde_con_su_indice() {
        let ok = parsear_linea("5^done,value=\"42\"");
        assert_eq!(
            interpretar(&ok, Some(&Pedido::Evaluar(3))),
            vec![EventoGdb::Evaluado {
                indice: 3,
                valor: Ok("42".into())
            }]
        );
        let mal = parsear_linea("6^error,msg=\"No symbol \\\"z\\\" in current context.\"");
        match &interpretar(&mal, Some(&Pedido::Evaluar(1)))[0] {
            EventoGdb::Evaluado {
                indice: 1,
                valor: Err(m),
            } => assert!(m.contains("No symbol")),
            otro => panic!("{otro:?}"),
        }
    }
    use std::time::{Duration, Instant};

    #[test]
    fn parsea_stopped() {
        let r = parsear_linea(
            r#"*stopped,reason="breakpoint-hit",disp="keep",bkptno="1",frame={addr="0x401136",func="main",args=[],file="a.c",fullname="/tmp/a.c",line="4",arch="i386:x86-64"},thread-id="1",stopped-threads="all""#,
        );
        let ev = interpretar(&r, None);
        let EventoGdb::Detenido(Parada::Breakpoint(m)) = &ev[0] else {
            panic!("{ev:?}")
        };
        assert_eq!(m.funcion, "main");
        assert_eq!(m.linea, Some(4));
        assert_eq!(m.archivo.as_deref(), Some(Path::new("/tmp/a.c")));
    }

    #[test]
    fn parsea_variables_y_consola() {
        let r = parsear_linea(
            r#"7^done,variables=[{name="x",arg="1",type="int",value="3"},{name="s",type="char *",value="0x4006 \"hola\""}]"#,
        );
        assert!(matches!(r, Registro::Resultado { token: Some(7), .. }));
        let ev = interpretar(&r, Some(&Pedido::Variables));
        let EventoGdb::Variables(v) = &ev[0] else {
            panic!()
        };
        assert_eq!(v.len(), 2);
        assert!(v[0].argumento);
        assert_eq!(v[1].valor, "0x4006 \"hola\"");
        assert_eq!(
            parsear_linea(r#"~"$1 = 42\n""#),
            Registro::Consola("$1 = 42\n".into())
        );
        assert_eq!(parsear_linea("(gdb) "), Registro::Indicador);
        assert_eq!(
            parsear_linea(r#"~"\303\261""#),
            Registro::Consola("ñ".into())
        );
    }

    #[test]
    fn parsea_lista_de_marcos() {
        let r = parsear_linea(
            r#"3^done,stack=[frame={level="0",addr="0x1",func="f",file="a.c",line="2"},frame={level="1",addr="0x2",func="main",file="a.c",line="9"}]"#,
        );
        let ev = interpretar(&r, Some(&Pedido::Marcos));
        let EventoGdb::Marcos(m) = &ev[0] else {
            panic!()
        };
        assert_eq!(
            m.iter().map(|m| m.funcion.as_str()).collect::<Vec<_>>(),
            ["f", "main"]
        );
        assert_eq!(m[1].nivel, 1);
    }

    #[cfg(unix)]
    fn esperar(g: &mut Gdb, mut pred: impl FnMut(&EventoGdb) -> bool) -> Vec<EventoGdb> {
        let fin = Instant::now() + Duration::from_secs(10);
        let mut todos = vec![];
        while Instant::now() < fin {
            let ev = g.recibir();
            let listo = ev.iter().any(&mut pred);
            todos.extend(ev);
            if listo {
                return todos;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("GDB no respondió a tiempo: {todos:?}");
    }

    #[test]
    #[cfg(unix)]
    fn sesion_real_con_gdb() {
        if Command::new("gdb").arg("--version").output().is_err()
            || Command::new("gcc").arg("--version").output().is_err()
        {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let fuente = dir.path().join("a.c");
        std::fs::write(&fuente, "int doble(int x)\n{\n    return 2 * x;\n}\n\nint main(void)\n{\n    int a = 21;\n    int b = doble(a);\n    return b - 42;\n}\n").unwrap();
        let ok = Command::new("gcc")
            .args(["-g", "-O0", "-o", "a", "a.c"])
            .current_dir(dir.path())
            .status()
            .unwrap();
        assert!(ok.success());
        let mut g = Gdb::lanzar(&dir.path().join("a"), &[], dir.path(), None, || {}).unwrap();
        g.poner_breakpoint(&fuente, 9).unwrap();
        esperar(&mut g, |e| matches!(e, EventoGdb::BreakpointCreado { .. }));
        g.correr().unwrap();
        esperar(
            &mut g,
            |e| matches!(e, EventoGdb::Detenido(Parada::Breakpoint(m)) if m.linea == Some(9)),
        );
        g.pedir_estado().unwrap();
        let ev = esperar(&mut g, |e| matches!(e, EventoGdb::Variables(_)));
        let vars = ev
            .iter()
            .find_map(|e| {
                if let EventoGdb::Variables(v) = e {
                    Some(v.clone())
                } else {
                    None
                }
            })
            .unwrap();
        assert!(
            vars.iter().any(|v| v.nombre == "a" && v.valor == "21"),
            "{vars:?}"
        );
        g.entrar().unwrap();
        esperar(
            &mut g,
            |e| matches!(e, EventoGdb::Detenido(Parada::Paso(m)) if m.funcion == "doble"),
        );
        g.continuar().unwrap();
        esperar(&mut g, |e| {
            matches!(e, EventoGdb::Detenido(Parada::Termino(Some(0))))
        });
        g.terminar();
    }
}
