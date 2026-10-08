//! Git ligero (RF-11): invoca el binario `git` del host, sin `libgit2`.
//!
//! Hereda `~/.gitconfig`, las claves SSH, la firma y los hooks del usuario. Lo interactivo
//! (push, pull, rebase, conflictos) se deriva a la terminal interna.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

/// Largo recomendado de la primera línea de un mensaje de commit.
pub const LARGO_RESUMEN: usize = 72;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entrada {
    pub ruta: String,
    /// Estado en el índice (columna X de porcelain): 'M', 'A', 'D', 'R'… o '.'.
    pub indice: char,
    /// Estado en el árbol de trabajo (columna Y).
    pub arbol: char,
    pub tipo: TipoEntrada,
    /// Ruta original de un renombre.
    pub origen: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoEntrada {
    Cambio,
    SinSeguimiento,
    Ignorado,
    Conflicto,
}

impl Entrada {
    pub fn en_stage(&self) -> bool {
        self.tipo == TipoEntrada::Cambio && self.indice != '.'
    }

    pub fn sin_stage(&self) -> bool {
        matches!(
            self.tipo,
            TipoEntrada::SinSeguimiento | TipoEntrada::Conflicto
        ) || self.arbol != '.'
    }

    /// Descripción corta en español para el panel.
    pub fn describir(&self) -> &'static str {
        match self.tipo {
            TipoEntrada::SinSeguimiento => "nuevo",
            TipoEntrada::Ignorado => "ignorado",
            TipoEntrada::Conflicto => "conflicto",
            TipoEntrada::Cambio => match if self.arbol != '.' {
                self.arbol
            } else {
                self.indice
            } {
                'M' => "modificado",
                'A' => "agregado",
                'D' => "borrado",
                'R' => "renombrado",
                'C' => "copiado",
                'T' => "cambió de tipo",
                _ => "cambiado",
            },
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Estado {
    pub rama: Option<String>,
    pub commit: Option<String>,
    pub adelante: u32,
    pub atras: u32,
    pub entradas: Vec<Entrada>,
}

impl Estado {
    pub fn limpio(&self) -> bool {
        self.entradas
            .iter()
            .all(|e| e.tipo == TipoEntrada::Ignorado)
    }

    pub fn cantidad_modificados(&self) -> usize {
        self.entradas.iter().filter(|e| e.sin_stage()).count()
    }

    pub fn cantidad_en_stage(&self) -> usize {
        self.entradas.iter().filter(|e| e.en_stage()).count()
    }

    /// Texto de la barra de estado: ` main *2 +1 ↑1`.
    pub fn resumen(&self) -> String {
        let mut s = format!(" {}", self.rama.as_deref().unwrap_or("(sin rama)"));
        let m = self.cantidad_modificados();
        let st = self.cantidad_en_stage();
        if m > 0 {
            s.push_str(&format!(" *{m}"));
        }
        if st > 0 {
            s.push_str(&format!(" +{st}"));
        }
        if self.adelante > 0 {
            s.push_str(&format!(" ↑{}", self.adelante));
        }
        if self.atras > 0 {
            s.push_str(&format!(" ↓{}", self.atras));
        }
        s
    }
}

/// Parsea `git status --porcelain=v2 --branch -z`.
pub fn parsear_estado(salida: &str) -> Estado {
    let mut estado = Estado::default();
    let mut campos = salida.split('\0').peekable();
    while let Some(registro) = campos.next() {
        if registro.is_empty() {
            continue;
        }
        if let Some(cabecera) = registro.strip_prefix("# ") {
            let (clave, valor) = cabecera.split_once(' ').unwrap_or((cabecera, ""));
            match clave {
                "branch.head" => estado.rama = (valor != "(detached)").then(|| valor.to_string()),
                "branch.oid" => estado.commit = (valor != "(initial)").then(|| valor.to_string()),
                "branch.ab" => {
                    for parte in valor.split_whitespace() {
                        if let Some(n) = parte.strip_prefix('+') {
                            estado.adelante = n.parse().unwrap_or(0);
                        } else if let Some(n) = parte.strip_prefix('-') {
                            estado.atras = n.parse().unwrap_or(0);
                        }
                    }
                }
                _ => {}
            }
            continue;
        }
        let tipo = registro.chars().next().unwrap_or(' ');
        match tipo {
            '1' | '2' => {
                // 1 XY sub mH mI mW hH hI ruta   |   2 XY sub mH mI mW hH hI Xscore ruta \0 origen
                let n_campos = if tipo == '1' { 9 } else { 10 };
                let partes: Vec<&str> = registro.splitn(n_campos, ' ').collect();
                if partes.len() < n_campos {
                    continue;
                }
                let xy: Vec<char> = partes[1].chars().collect();
                let origen = if tipo == '2' {
                    campos.next().map(String::from)
                } else {
                    None
                };
                estado.entradas.push(Entrada {
                    ruta: partes[n_campos - 1].to_string(),
                    indice: *xy.first().unwrap_or(&'.'),
                    arbol: *xy.get(1).unwrap_or(&'.'),
                    tipo: TipoEntrada::Cambio,
                    origen,
                });
            }
            'u' => {
                let partes: Vec<&str> = registro.splitn(11, ' ').collect();
                if let Some(ruta) = partes.get(10) {
                    estado.entradas.push(Entrada {
                        ruta: ruta.to_string(),
                        indice: 'U',
                        arbol: 'U',
                        tipo: TipoEntrada::Conflicto,
                        origen: None,
                    });
                }
            }
            '?' | '!' => {
                estado.entradas.push(Entrada {
                    ruta: registro[2..].to_string(),
                    indice: '.',
                    arbol: '.',
                    tipo: if tipo == '?' {
                        TipoEntrada::SinSeguimiento
                    } else {
                        TipoEntrada::Ignorado
                    },
                    origen: None,
                });
            }
            _ => {}
        }
    }
    estado
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub hash: String,
    pub autor: String,
    pub cuando: String,
    pub resumen: String,
}

#[derive(Debug, Clone)]
pub struct ErrorGit(pub String);

impl std::fmt::Display for ErrorGit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Repositorio: la raíz donde se invoca `git`.
#[derive(Debug, Clone)]
pub struct Repo {
    pub dir: PathBuf,
}

impl Repo {
    /// El repositorio que contiene `dir`, si `git` está y `dir` está dentro de uno.
    pub fn abrir(dir: &Path) -> Option<Repo> {
        let repo = Repo {
            dir: dir.to_path_buf(),
        };
        let (codigo, salida, _) = repo.git(&["rev-parse", "--show-toplevel"]).ok()?;
        (codigo == 0).then(|| Repo {
            dir: PathBuf::from(salida.trim()),
        })
    }

    fn git(&self, args: &[&str]) -> std::io::Result<(i32, String, String)> {
        let salida = Command::new("git")
            .args(args)
            .current_dir(&self.dir)
            .stdin(Stdio::null())
            // Sin paginador ni editor interactivo: el IDE no los puede atender.
            .env("GIT_PAGER", "cat")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_EDITOR", "true")
            .env("LC_ALL", "C.UTF-8")
            .output()?;
        Ok((
            salida.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&salida.stdout).into_owned(),
            String::from_utf8_lossy(&salida.stderr).into_owned(),
        ))
    }

    /// Ejecuta y reintenta ante un `index.lock` transitorio (otra operación en curso, R-7).
    fn ejecutar(&self, args: &[&str]) -> Result<String, ErrorGit> {
        let mut ultimo = String::new();
        for intento in 0..4 {
            match self.git(args) {
                Ok((0, salida, _)) => return Ok(salida),
                Ok((_, salida, error)) => {
                    let texto = if error.trim().is_empty() {
                        salida
                    } else {
                        error
                    };
                    if texto.contains("index.lock") && intento < 3 {
                        std::thread::sleep(Duration::from_millis(100 * (intento + 1)));
                        ultimo = texto;
                        continue;
                    }
                    return Err(ErrorGit(texto.trim().to_string()));
                }
                Err(e) => return Err(ErrorGit(format!("no se pudo ejecutar git: {e}"))),
            }
        }
        Err(ErrorGit(ultimo.trim().to_string()))
    }

    pub fn estado(&self) -> Result<Estado, ErrorGit> {
        self.ejecutar(&["status", "--porcelain=v2", "--branch", "-z"])
            .map(|s| parsear_estado(&s))
    }

    pub fn agregar(&self, ruta: &str) -> Result<(), ErrorGit> {
        self.ejecutar(&["add", "--", ruta]).map(|_| ())
    }

    pub fn quitar_del_stage(&self, ruta: &str) -> Result<(), ErrorGit> {
        if self.tiene_commits() {
            self.ejecutar(&["restore", "--staged", "--", ruta])
                .map(|_| ())
        } else {
            self.ejecutar(&["rm", "--cached", "-q", "--", ruta])
                .map(|_| ())
        }
    }

    pub fn agregar_todo(&self) -> Result<(), ErrorGit> {
        self.ejecutar(&["add", "-A"]).map(|_| ())
    }

    pub fn quitar_todo_del_stage(&self) -> Result<(), ErrorGit> {
        if self.tiene_commits() {
            self.ejecutar(&["restore", "--staged", "--", "."])
                .map(|_| ())
        } else {
            self.ejecutar(&["rm", "-r", "--cached", "-q", "--", "."])
                .map(|_| ())
        }
    }

    fn tiene_commits(&self) -> bool {
        matches!(
            self.git(&["rev-parse", "--verify", "-q", "HEAD"]),
            Ok((0, _, _))
        )
    }

    /// `git commit -m`: respeta identidad, firma y hooks del host. Devuelve la salida de git.
    pub fn commit(&self, mensaje: &str) -> Result<String, ErrorGit> {
        if mensaje.trim().is_empty() {
            return Err(ErrorGit("El mensaje del commit está vacío.".into()));
        }
        self.ejecutar(&["commit", "-m", mensaje])
    }

    pub fn log(&self, cantidad: usize) -> Result<Vec<Commit>, ErrorGit> {
        let n = format!("-n{cantidad}");
        let salida = match self.ejecutar(&["log", &n, "--pretty=format:%h%x1f%an%x1f%ar%x1f%s%x1e"])
        {
            Ok(s) => s,
            Err(e) if e.0.contains("does not have any commits") => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        Ok(salida
            .split('\x1e')
            .filter_map(|r| {
                let r = r.trim_start_matches('\n');
                let mut p = r.split('\x1f');
                Some(Commit {
                    hash: p.next()?.to_string(),
                    autor: p.next()?.to_string(),
                    cuando: p.next()?.to_string(),
                    resumen: p.next()?.to_string(),
                })
            })
            .filter(|c| !c.hash.is_empty())
            .collect())
    }

    pub fn mostrar(&self, hash: &str) -> Result<String, ErrorGit> {
        self.ejecutar(&["show", "--stat", "--patch", "--no-color", hash])
    }

    /// Contenido de `ruta` (relativa al repositorio) en el índice (`en_indice`) o en
    /// HEAD. None si no existe ahí (archivo nuevo, repositorio sin commits).
    pub fn contenido(&self, ruta: &str, en_indice: bool) -> Option<String> {
        let ruta = ruta.replace('\\', "/");
        let spec = if en_indice {
            format!(":{ruta}")
        } else {
            format!("HEAD:{ruta}")
        };
        match self.git(&["show", &spec]) {
            Ok((0, salida, _)) => Some(salida),
            _ => None,
        }
    }

    /// Ruta de `archivo` relativa a la raíz del repositorio, con `/`.
    pub fn relativa(&self, archivo: &Path) -> Option<String> {
        let dir = self.dir.canonicalize().unwrap_or_else(|_| self.dir.clone());
        let a = archivo
            .canonicalize()
            .unwrap_or_else(|_| archivo.to_path_buf());
        a.strip_prefix(&dir)
            .ok()
            .map(|r| r.to_string_lossy().replace('\\', "/"))
    }

    /// Diff de un archivo: del árbol de trabajo o, con `en_stage`, del índice.
    pub fn diff(&self, ruta: &str, en_stage: bool) -> Result<String, ErrorGit> {
        if en_stage {
            self.ejecutar(&["diff", "--no-color", "--cached", "--", ruta])
        } else {
            self.ejecutar(&["diff", "--no-color", "--", ruta])
        }
    }

    /// Marca de cambios en `.git/HEAD` y `.git/index` para refrescar sin consultar a git.
    pub fn marca(&self) -> Option<(SystemTime, SystemTime)> {
        let git = self.dir.join(".git");
        let m = |p: PathBuf| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        Some((
            m(git.join("HEAD"))?,
            m(git.join("index")).unwrap_or(SystemTime::UNIX_EPOCH),
        ))
    }
}

/// Aviso para el diálogo de commit (RF-11.3).
pub fn aviso_mensaje(mensaje: &str) -> Option<String> {
    let primera = mensaje.lines().next().unwrap_or("");
    let n = primera.chars().count();
    (n > LARGO_RESUMEN).then(|| {
        format!("La primera línea tiene {n} caracteres; conviene que no pase de {LARGO_RESUMEN}.")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsea_porcelain_v2() {
        let salida = "# branch.oid abc123\0# branch.head main\0# branch.ab +2 -1\0\
1 .M N... 100644 100644 100644 aaa bbb lista.c\0\
1 A. N... 000000 100644 100644 000 ccc nuevo.h\0\
2 R. N... 100644 100644 100644 ddd eee R100 b.c\0a.c\0\
? notas.md\0";
        let e = parsear_estado(salida);
        assert_eq!(e.rama.as_deref(), Some("main"));
        assert_eq!((e.adelante, e.atras), (2, 1));
        assert_eq!(e.entradas.len(), 4);
        assert_eq!(e.entradas[0].ruta, "lista.c");
        assert!(e.entradas[0].sin_stage() && !e.entradas[0].en_stage());
        assert!(e.entradas[1].en_stage());
        assert_eq!(e.entradas[2].origen.as_deref(), Some("a.c"));
        assert_eq!(e.entradas[3].tipo, TipoEntrada::SinSeguimiento);
        assert_eq!(e.resumen(), " main *2 +2 ↑2 ↓1");
    }

    #[test]
    fn rutas_con_espacios() {
        let e =
            parsear_estado("1 .M N... 100644 100644 100644 a b mi archivo.c\0? otro archivo.md\0");
        assert_eq!(e.entradas[0].ruta, "mi archivo.c");
        assert_eq!(e.entradas[1].ruta, "otro archivo.md");
    }

    #[test]
    fn flujo_completo_contra_git_real() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        let correr = |args: &[&str]| {
            Command::new("git")
                .args(args)
                .current_dir(d)
                .output()
                .unwrap()
        };
        correr(&["init", "-q", "-b", "main"]);
        correr(&["config", "user.name", "Alumna"]);
        correr(&["config", "user.email", "alumna@example.com"]);
        correr(&["config", "commit.gpgsign", "false"]);
        std::fs::write(d.join("lista.c"), "int x;\n").unwrap();
        let repo = Repo::abrir(d).unwrap();
        let e = repo.estado().unwrap();
        assert_eq!(e.entradas.len(), 1);
        assert_eq!(e.entradas[0].tipo, TipoEntrada::SinSeguimiento);
        repo.agregar("lista.c").unwrap();
        assert!(repo.estado().unwrap().entradas[0].en_stage());
        repo.quitar_del_stage("lista.c").unwrap();
        assert_eq!(
            repo.estado().unwrap().entradas[0].tipo,
            TipoEntrada::SinSeguimiento
        );
        repo.agregar_todo().unwrap();
        repo.commit("Primer commit").unwrap();
        assert!(repo.estado().unwrap().limpio());
        let log = repo.log(10).unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].resumen, "Primer commit");
        assert!(repo.mostrar(&log[0].hash).unwrap().contains("int x;"));
        std::fs::write(d.join("lista.c"), "int y;\n").unwrap();
        assert!(repo.diff("lista.c", false).unwrap().contains("+int y;"));
        assert_eq!(
            repo.contenido("lista.c", false).as_deref(),
            Some("int x;\n")
        );
        repo.agregar("lista.c").unwrap();
        assert_eq!(repo.contenido("lista.c", true).as_deref(), Some("int y;\n"));
        assert_eq!(repo.contenido("no-existe.c", false), None);
        assert_eq!(
            repo.relativa(&d.join("lista.c")).as_deref(),
            Some("lista.c")
        );
    }

    #[test]
    fn aviso_de_largo() {
        assert!(aviso_mensaje("corto").is_none());
        assert!(aviso_mensaje(&"x".repeat(80)).is_some());
    }
}
