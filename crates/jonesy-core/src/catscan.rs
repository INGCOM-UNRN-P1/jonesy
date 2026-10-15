//! cat-scan: vista semigráfica de stack y heap a partir del `trace.json` de `mem-trace`.
//!
//! `mem-trace run <fuente.c> --outdir <dir>` ejecuta el programa paso a paso bajo GDB y
//! guarda en `trace.json` el estado de cada paso: marcos con sus variables (escalares,
//! punteros con su región y destino, structs) y los bloques del heap con su estado. Acá se
//! carga esa traza, se alinea con la parada actual del depurador y se dibuja en una
//! cuadrícula de caracteres: marcos en cajas a la izquierda, bloques a la derecha y
//! flechas `──►` ruteadas por carriles verticales entre ambos. Los punteros a bloques
//! liberados se marcan como colgantes.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Traza {
    pub meta: Meta,
    pub steps: Vec<Paso>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Meta {
    pub source: String,
    pub exit_code: Option<i64>,
    pub signal: Option<String>,
    pub warnings: Vec<Value>,
    /// Estado del heap al terminar: los bloques vivos son fugas.
    pub final_heap: Heap,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Paso {
    pub n: usize,
    pub func: String,
    pub line: usize,
    pub frames: Vec<MarcoTraza>,
    pub globals: Vec<VariableTraza>,
    pub heap: Heap,
    pub events: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct MarcoTraza {
    pub func: String,
    pub file: String,
    pub line: usize,
    pub locals: Vec<VariableTraza>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct VariableTraza {
    pub name: String,
    pub storage: String,
    pub addr: u64,
    pub val: Value,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Heap {
    pub blocks: Vec<Bloque>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Bloque {
    pub id: u64,
    pub addr: u64,
    pub size: u64,
    pub status: String,
    pub site: Sitio,
    pub free_site: Option<Sitio>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Sitio {
    pub func: String,
    pub file: String,
    pub line: usize,
}

impl Sitio {
    fn corto(&self) -> String {
        let archivo = Path::new(&self.file)
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default();
        format!("{archivo}:{}", self.line)
    }
}

impl Bloque {
    pub fn vivo(&self) -> bool {
        self.status == "alive"
    }

    pub fn contiene(&self, dir: u64) -> bool {
        dir >= self.addr && dir < self.addr + self.size.max(1)
    }
}

impl Traza {
    /// Carga `trace.json`; si mem-trace corrió sobre los fuentes juntados (ver
    /// [`comando_memtrace`]), cada línea vuelve a su archivo.
    pub fn cargar(ruta: &Path) -> Result<Traza, String> {
        let texto =
            std::fs::read_to_string(ruta).map_err(|e| format!("{}: {e}", ruta.display()))?;
        let mut t: Traza =
            serde_json::from_str(&texto).map_err(|e| format!("trace.json inválido: {e}"))?;
        let tramos: Option<Vec<Tramo>> = ruta
            .parent()
            .and_then(|d| std::fs::read_to_string(d.join(AMALGAMA)).ok())
            .and_then(|j| serde_json::from_str(&j).ok());
        if let Some(tramos) = tramos.filter(|_| t.meta.source.ends_with("programa.c")) {
            t.desamalgamar(&tramos);
        }
        Ok(t)
    }

    /// Paso que corresponde a una parada del depurador en `funcion` / `linea` con
    /// `profundidad` marcos. Busca hacia adelante desde `desde` (las paradas avanzan) y,
    /// si no encuentra, desde el principio.
    pub fn alinear(
        &self,
        funcion: &str,
        linea: usize,
        profundidad: usize,
        desde: usize,
    ) -> Option<usize> {
        let coincide = |p: &Paso| {
            p.line == linea
                && p.func == funcion
                && (profundidad == 0 || p.frames.len() == profundidad)
        };
        (desde..self.steps.len())
            .chain(0..desde.min(self.steps.len()))
            .find(|&i| coincide(&self.steps[i]))
            .or_else(|| {
                // Sin coincidencia exacta de profundidad (recursión), alcanza con función y línea.
                (0..self.steps.len())
                    .find(|&i| self.steps[i].line == linea && self.steps[i].func == funcion)
            })
    }
}

// ------------------------------------------------------------------ eventos

fn sitio_de(v: &Value) -> String {
    let archivo = v["file"]
        .as_str()
        .and_then(|f| Path::new(f).file_name())
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    match v["line"].as_u64() {
        Some(l) if !archivo.is_empty() => format!(" en {archivo}:{l}"),
        _ => String::new(),
    }
}

/// Qué pasó con el heap en este paso (malloc, free, realloc), con los problemas en tono
/// de peligro: free doble, free de un puntero que no vino de malloc, malloc que falló.
pub fn eventos_del_paso(paso: &Paso) -> Vec<(Tono, String)> {
    paso.events
        .iter()
        .map(|e| {
            let sitio = sitio_de(&e["site"]);
            let id = e["id"]
                .as_u64()
                .map(|i| format!("#{i}"))
                .unwrap_or_default();
            let kind = e["kind"].as_str().unwrap_or("?");
            match kind {
                "free" if e["double_free"].as_bool() == Some(true) => (
                    Tono::Peligro,
                    format!("⚠ free doble de {id}{sitio}: ese bloque ya se había liberado"),
                ),
                "free" if e["unknown"].as_bool() == Some(true) => (
                    Tono::Peligro,
                    format!("⚠ free de un puntero que no viene de malloc{sitio}"),
                ),
                "free" if e["noop"].as_bool() == Some(true) => {
                    (Tono::Normal, format!("free(NULL){sitio}: no hace nada"))
                }
                "free" => (Tono::Liberado, format!("free({id}){sitio}")),
                "resize" => (
                    Tono::Vivo,
                    format!("realloc {id}{sitio}: el bloque se movió"),
                ),
                _ if e["failed"].as_bool() == Some(true) => (
                    Tono::Peligro,
                    format!(
                        "⚠ {kind}({}) devolvió NULL{sitio}",
                        e["size"].as_u64().unwrap_or(0)
                    ),
                ),
                _ => (
                    Tono::Vivo,
                    format!("{kind}({}) → {id}{sitio}", e["size"].as_u64().unwrap_or(0)),
                ),
            }
        })
        .collect()
}

/// Cómo terminó el programa: señal, fugas (bloques vivos al final) y avisos de mem-trace.
pub fn resumen_final(t: &Traza) -> Vec<(Tono, String)> {
    let mut v = vec![];
    if let Some(s) = t.meta.signal.as_deref().filter(|s| !s.is_empty()) {
        v.push((
            Tono::Peligro,
            format!("⚠ el programa terminó por la señal {s}"),
        ));
    } else if let Some(c) = t.meta.exit_code {
        v.push((Tono::Normal, format!("el programa terminó con código {c}")));
    }
    let fugas: Vec<&Bloque> = t
        .meta
        .final_heap
        .blocks
        .iter()
        .filter(|b| b.vivo())
        .collect();
    let bytes: u64 = fugas.iter().map(|b| b.size).sum();
    for b in &fugas {
        v.push((
            Tono::Peligro,
            format!(
                "⚠ fuga: #{} de {} B reservado en {} nunca se liberó",
                b.id,
                b.size,
                b.site.corto()
            ),
        ));
    }
    if fugas.is_empty() && !t.meta.final_heap.blocks.is_empty() {
        v.push((Tono::Vivo, "sin fugas: todo lo reservado se liberó".into()));
    } else if !fugas.is_empty() {
        v.push((
            Tono::Peligro,
            format!("{} bloque(s) sin liberar, {bytes} B en total", fugas.len()),
        ));
    }
    for w in &t.meta.warnings {
        let texto = w
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| w.to_string());
        v.push((Tono::Tipo, format!("aviso de mem-trace: {texto}")));
    }
    v
}

/// ¿Define `main`? (Las pruebas con otro nombre que `prueba.c` tienen su propio `main`, y
/// compilarlas junto al programa da «multiple definition of main».)
fn define_main(ruta: &Path) -> bool {
    let Ok(texto) = std::fs::read_to_string(ruta) else {
        return false;
    };
    let limpio: String = crate::simbolos::limpio(&texto.chars().collect::<Vec<_>>())
        .into_iter()
        .collect();
    limpio.lines().any(|l| {
        let l = l.trim_start();
        ["int ", "void "].iter().any(|tipo| {
            l.strip_prefix(tipo)
                .map(str::trim_start)
                .and_then(|r| r.strip_prefix("main"))
                .is_some_and(|r| r.trim_start().starts_with('('))
        })
    })
}

/// Los fuentes del programa del archivo activo: su `main.c`, los demás `.c` del ejercicio
/// y de las bibliotecas que usa (sin pruebas ni otros `main`), y los directorios de
/// include. Para un archivo suelto sin Makefile, sólo ese archivo.
#[derive(Debug, Clone, PartialEq)]
pub struct FuentesPrograma {
    pub main: PathBuf,
    pub otras: Vec<PathBuf>,
    pub includes: Vec<PathBuf>,
}

fn fuentes_de(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|it| {
            it.flatten()
                .map(|e| e.path())
                .filter(|x| x.extension().is_some_and(|e| e == "c"))
                .filter(|x| {
                    let n = x
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    !matches!(n.as_str(), "main.c" | "prueba.c" | "holden_mocks.c")
                        && !n.starts_with('.')
                        && !define_main(x)
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

pub fn fuentes_del_programa(p: &crate::Proyecto, activo: &Path) -> Option<FuentesPrograma> {
    match p.modulo_activo(Some(activo)) {
        Some(m) => {
            let main = m.dir.join("main.c");
            if !main.is_file() {
                return None;
            }
            let mut f = FuentesPrograma {
                main,
                otras: fuentes_de(&m.dir),
                includes: vec![m.dir.clone()],
            };
            for lib in p.bibliotecas_de(m) {
                f.includes.push(if lib.dir.join("include").is_dir() {
                    lib.dir.join("include")
                } else {
                    lib.dir.clone()
                });
                let src = if lib.dir.join("src").is_dir() {
                    lib.dir.join("src")
                } else {
                    lib.dir.clone()
                };
                f.otras.extend(fuentes_de(&src));
            }
            Some(f)
        }
        None if activo.extension().is_some_and(|e| e == "c") && p.makefile().is_none() => {
            Some(FuentesPrograma {
                main: activo.to_path_buf(),
                otras: vec![],
                includes: activo
                    .parent()
                    .map(|d| vec![d.to_path_buf()])
                    .unwrap_or_default(),
            })
        }
        None => None,
    }
}

/// Comando de mem-trace para el programa del archivo activo: un `.c` suelto, o el
/// `main.c` de un ejercicio con Makefile junto con los demás fuentes del módulo y los de
/// las bibliotecas que usa. None si no hay programa.
///
/// mem-trace sigue sólo el archivo que recibe: lo que se pide con malloc en otro `.c`
/// no aparecía y los punteros a esos bloques salían «inválidos». Por eso los fuentes se
/// juntan en uno solo (`salida/programa.c`) y [`Traza::cargar`] devuelve cada línea a su
/// archivo con `salida/amalgama.json`. Si algún header propio no tiene guarda (incluirlo
/// dos veces en un mismo archivo no compilaría), se pasan por separado con `--cc-arg`.
pub fn comando_memtrace(p: &crate::Proyecto, activo: &Path, salida: &Path) -> Option<String> {
    use crate::proyecto::shell_quote as q;
    let rel = |x: &Path| p.relativa(x).replace('\\', "/");
    let f = fuentes_del_programa(p, activo)?;
    let mut includes = f.includes.clone();
    for o in &f.otras {
        if let Some(d) = o.parent().filter(|d| !includes.iter().any(|i| i == d)) {
            includes.push(d.to_path_buf());
        }
    }
    let mut args: Vec<String> = includes
        .iter()
        .map(|i| format!("-I {}", q(&rel(i))))
        .collect();
    let fuente = if f.otras.is_empty() {
        f.main.clone()
    } else {
        match amalgamar(&f, salida, &includes) {
            Some(a) => a,
            None => {
                for o in &f.otras {
                    args.push(format!("--cc-arg={}", q(&rel(o))));
                }
                f.main.clone()
            }
        }
    };
    if fuente == f.main {
        let _ = std::fs::remove_file(salida.join(AMALGAMA));
    }
    Some(format!(
        "mem-trace run {} {} --outdir {} -o {} --max-steps 3000",
        q(&rel(&fuente)),
        args.join(" "),
        q(&rel(salida)),
        q(&rel(&salida.join("reporte.html")))
    ))
}

const AMALGAMA: &str = "amalgama.json";

/// Un tramo del archivo juntado: desde qué línea (1…) y cuántas vienen de `archivo`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, Deserialize)]
struct Tramo {
    desde: usize,
    lineas: usize,
    archivo: String,
}

/// Escribe `salida/programa.c` con `main.c` y los demás fuentes uno detrás de otro, y
/// `salida/amalgama.json` con de dónde viene cada tramo. None si no conviene juntarlos.
fn amalgamar(f: &FuentesPrograma, salida: &Path, includes: &[PathBuf]) -> Option<PathBuf> {
    let sin_guarda = includes.iter().any(|d| {
        std::fs::read_dir(d)
            .into_iter()
            .flatten()
            .flatten()
            .any(|e| {
                let p = e.path();
                p.extension().is_some_and(|x| x == "h")
                    && std::fs::read_to_string(&p).is_ok_and(|t| {
                        !t.contains("#pragma once")
                            && !(t.contains("#ifndef") && t.contains("#define"))
                    })
            })
    });
    if sin_guarda {
        return None;
    }
    let mut texto = String::new();
    let mut tramos = vec![];
    let mut linea = 1;
    for archivo in std::iter::once(&f.main).chain(&f.otras) {
        let mut t = std::fs::read_to_string(archivo).ok()?;
        if !t.ends_with('\n') {
            t.push('\n');
        }
        let n = t.lines().count();
        tramos.push(Tramo {
            desde: linea,
            lineas: n,
            archivo: archivo.to_string_lossy().into_owned(),
        });
        texto.push_str(&t);
        linea += n;
    }
    std::fs::create_dir_all(salida).ok()?;
    let destino = salida.join("programa.c");
    std::fs::write(&destino, texto).ok()?;
    std::fs::write(salida.join(AMALGAMA), serde_json::to_string(&tramos).ok()?).ok()?;
    Some(destino)
}

/// La línea `l` del archivo juntado: en qué archivo y línea original está.
fn original(tramos: &[Tramo], l: usize) -> Option<(&str, usize)> {
    tramos
        .iter()
        .find(|t| l >= t.desde && l < t.desde + t.lineas)
        .map(|t| (t.archivo.as_str(), l - t.desde + 1))
}

impl Traza {
    /// Devuelve cada paso, marco, bloque y evento a su archivo y línea originales.
    fn desamalgamar(&mut self, tramos: &[Tramo]) {
        let juntado = Path::new(&self.meta.source)
            .file_name()
            .map(|n| n.to_os_string());
        let es_juntado = |f: &str| {
            !f.is_empty() && Path::new(f).file_name().map(|n| n.to_os_string()) == juntado
        };
        let arreglar = |file: &mut String, line: &mut usize| {
            if es_juntado(file) {
                if let Some((a, l)) = original(tramos, *line) {
                    *file = a.to_string();
                    *line = l;
                }
            }
        };
        let arreglar_heap = |h: &mut Heap| {
            for b in &mut h.blocks {
                arreglar(&mut b.site.file, &mut b.site.line);
                if let Some(s) = &mut b.free_site {
                    arreglar(&mut s.file, &mut s.line);
                }
            }
        };
        for p in &mut self.steps {
            // La línea del paso es la del marco de arriba.
            let arriba_juntado = p.frames.first().is_none_or(|m| es_juntado(&m.file));
            if arriba_juntado {
                if let Some((_, l)) = original(tramos, p.line) {
                    p.line = l;
                }
            }
            for m in &mut p.frames {
                arreglar(&mut m.file, &mut m.line);
            }
            arreglar_heap(&mut p.heap);
            for e in &mut p.events {
                desamalgamar_valor(e, &es_juntado, tramos);
            }
        }
        arreglar_heap(&mut self.meta.final_heap);
        if let Some(t) = tramos.first() {
            self.meta.source = t.archivo.clone();
        }
    }
}

/// Los `{"file": …, "line": …}` de un evento (en cualquier profundidad).
fn desamalgamar_valor(v: &mut Value, es_juntado: &dyn Fn(&str) -> bool, tramos: &[Tramo]) {
    match v {
        Value::Object(o) => {
            let archivo = o.get("file").and_then(Value::as_str).map(str::to_string);
            let linea = o.get("line").and_then(Value::as_u64);
            if let (Some(f), Some(l)) = (archivo, linea) {
                if es_juntado(&f) {
                    if let Some((a, l)) = original(tramos, l as usize) {
                        o.insert("file".into(), Value::from(a));
                        o.insert("line".into(), Value::from(l));
                    }
                }
            }
            for (_, x) in o.iter_mut() {
                desamalgamar_valor(x, es_juntado, tramos);
            }
        }
        Value::Array(a) => {
            for x in a {
                desamalgamar_valor(x, es_juntado, tramos);
            }
        }
        _ => {}
    }
}

// ------------------------------------------------------------------ dibujo

/// Tono semántico de cada celda; la TUI lo traduce a colores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tono {
    #[default]
    Normal,
    Borde,
    Titulo,
    Nombre,
    Tipo,
    Valor,
    Direccion,
    Flecha,
    Vivo,
    Liberado,
    Peligro,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Celda {
    pub c: char,
    pub tono: Tono,
}

/// Cuadrícula resultante: filas de celdas del mismo ancho.
#[derive(Debug, Clone, Default)]
pub struct Lienzo {
    pub filas: Vec<Vec<Celda>>,
    pub ancho: usize,
}

impl Lienzo {
    fn nuevo(ancho: usize, alto: usize) -> Lienzo {
        Lienzo {
            filas: vec![
                vec![
                    Celda {
                        c: ' ',
                        tono: Tono::Normal
                    };
                    ancho
                ];
                alto
            ],
            ancho,
        }
    }

    fn poner(&mut self, fila: usize, col: usize, c: char, tono: Tono) {
        if let Some(f) = self.filas.get_mut(fila) {
            if let Some(x) = f.get_mut(col) {
                *x = Celda { c, tono };
            }
        }
    }

    fn texto(&mut self, fila: usize, col: usize, s: &str, tono: Tono, max: usize) {
        for (i, c) in s.chars().take(max).enumerate() {
            self.poner(fila, col + i, c, tono);
        }
    }

    fn get(&self, fila: usize, col: usize) -> char {
        self.filas
            .get(fila)
            .and_then(|f| f.get(col))
            .map(|c| c.c)
            .unwrap_or(' ')
    }

    /// Texto plano (para pruebas y para copiar).
    pub fn a_texto(&self) -> String {
        self.filas
            .iter()
            .map(|f| {
                f.iter()
                    .map(|c| c.c)
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Una línea de contenido dentro de una caja, con el puntero que sale de ella (si hay).
struct LineaCaja {
    segmentos: Vec<(String, Tono)>,
    apunta_a: Option<u64>,
    colgante: bool,
}

struct Caja {
    titulo: String,
    tono_titulo: Tono,
    lineas: Vec<LineaCaja>,
    id_bloque: Option<u64>,
}

fn tipo_de(v: &Value) -> String {
    v["type"].as_str().unwrap_or("").to_string()
}

fn valor_corto(v: &Value) -> String {
    match v["kind"].as_str() {
        Some("scalar") => match &v["value"] {
            Value::String(s) => s.clone(),
            Value::Null => "?".into(),
            otro => otro.to_string(),
        },
        Some("pointer") => match v["region"].as_str() {
            Some("null") => "NULL".into(),
            _ => format!("0x{:x}", v["addr"].as_u64().unwrap_or(0)),
        },
        Some("struct") => "{…}".into(),
        Some("array") => format!(
            "[{}]",
            v["elements"].as_array().map(|a| a.len()).unwrap_or(0)
        ),
        _ => v
            .get("value")
            .map(|x| x.to_string())
            .unwrap_or_else(|| "?".into()),
    }
}

fn bloque_de(paso: &Paso, dir: u64) -> Option<&Bloque> {
    paso.heap.blocks.iter().find(|b| b.contiene(dir))
}

/// Líneas para una variable (y sus campos si es struct por valor).
fn lineas_variable(paso: &Paso, nombre: &str, v: &Value, sangria: usize, out: &mut Vec<LineaCaja>) {
    let pre = " ".repeat(sangria);
    let tipo = tipo_de(v);
    let mut segmentos = vec![
        (format!("{pre}{tipo} "), Tono::Tipo),
        (nombre.to_string(), Tono::Nombre),
        (" = ".into(), Tono::Normal),
    ];
    let mut apunta_a = None;
    let mut colgante = false;
    if v["kind"] == "pointer" {
        let dir = v["addr"].as_u64().unwrap_or(0);
        match bloque_de(paso, dir) {
            Some(b) => {
                apunta_a = Some(b.id);
                colgante = !b.vivo();
                segmentos.push((
                    valor_corto(v),
                    if colgante {
                        Tono::Peligro
                    } else {
                        Tono::Direccion
                    },
                ));
                if colgante {
                    segmentos.push((" ⚠ colgante".into(), Tono::Peligro));
                }
            }
            None => {
                let nulo = v["region"] == "null";
                segmentos.push((
                    valor_corto(v),
                    if nulo { Tono::Valor } else { Tono::Direccion },
                ));
                if !nulo && v["region"].as_str() == Some("heap") {
                    // Apunta al heap pero a ningún bloque conocido: liberado y reciclado o basura.
                    colgante = true;
                    segmentos.push((" ⚠ inválido".into(), Tono::Peligro));
                }
            }
        }
    } else {
        segmentos.push((valor_corto(v), Tono::Valor));
    }
    out.push(LineaCaja {
        segmentos,
        apunta_a,
        colgante,
    });
    if v["kind"] == "struct" {
        for f in v["fields"].as_array().into_iter().flatten() {
            lineas_variable(
                paso,
                f["name"].as_str().unwrap_or("?"),
                &f["value"],
                sangria + 2,
                out,
            );
        }
    }
}

/// Contenido de un bloque: lo que se ve a través del primer puntero que lo apunta.
fn contenido_bloque(paso: &Paso, b: &Bloque) -> Option<Value> {
    fn buscar(v: &Value, b: &Bloque, prof: usize) -> Option<Value> {
        if prof > 6 {
            return None;
        }
        if v["kind"] == "pointer" && v["addr"].as_u64() == Some(b.addr) && !v["target"].is_null() {
            return Some(v["target"].clone());
        }
        for f in v["fields"].as_array().into_iter().flatten() {
            if let Some(x) = buscar(&f["value"], b, prof + 1) {
                return Some(x);
            }
        }
        buscar(&v["target"], b, prof + 1)
    }
    paso.frames
        .iter()
        .flat_map(|m| m.locals.iter())
        .chain(paso.globals.iter())
        .find_map(|var| buscar(&var.val, b, 0))
}

fn cajas(paso: &Paso) -> (Vec<Caja>, Vec<Caja>) {
    let mut pila = vec![];
    if !paso.globals.is_empty() {
        let mut lineas = vec![];
        for g in &paso.globals {
            lineas_variable(paso, &g.name, &g.val, 0, &mut lineas);
        }
        pila.push(Caja {
            titulo: "globales".into(),
            tono_titulo: Tono::Titulo,
            lineas,
            id_bloque: None,
        });
    }
    for m in &paso.frames {
        let mut lineas = vec![];
        for l in &m.locals {
            lineas_variable(paso, &l.name, &l.val, 0, &mut lineas);
        }
        if lineas.is_empty() {
            lineas.push(LineaCaja {
                segmentos: vec![("(sin variables)".into(), Tono::Tipo)],
                apunta_a: None,
                colgante: false,
            });
        }
        let archivo = Path::new(&m.file)
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default();
        pila.push(Caja {
            titulo: format!("{}() {archivo}:{}", m.func, m.line),
            tono_titulo: Tono::Titulo,
            lineas,
            id_bloque: None,
        });
    }
    let mut heap = vec![];
    for b in &paso.heap.blocks {
        let mut lineas = vec![LineaCaja {
            segmentos: vec![
                (format!("{} B en ", b.size), Tono::Normal),
                (b.site.corto(), Tono::Direccion),
            ],
            apunta_a: None,
            colgante: false,
        }];
        if let Some(fs) = &b.free_site {
            lineas.push(LineaCaja {
                segmentos: vec![
                    ("free en ".into(), Tono::Liberado),
                    (fs.corto(), Tono::Liberado),
                ],
                apunta_a: None,
                colgante: false,
            });
        } else if let Some(c) = contenido_bloque(paso, b) {
            match c["kind"].as_str() {
                Some("struct") => {
                    for f in c["fields"].as_array().into_iter().flatten() {
                        let mut ls = vec![];
                        lineas_variable(
                            paso,
                            f["name"].as_str().unwrap_or("?"),
                            &f["value"],
                            0,
                            &mut ls,
                        );
                        for mut l in ls {
                            // Punteros entre bloques: se indican con texto (las flechas
                            // ruteadas son sólo stack → heap).
                            if let Some(id) = l.apunta_a.take() {
                                l.segmentos.push((format!(" ─► #{id}"), Tono::Flecha));
                            }
                            lineas.push(l);
                        }
                    }
                }
                _ => lineas.push(LineaCaja {
                    segmentos: vec![
                        (format!("{} ", tipo_de(&c)), Tono::Tipo),
                        (valor_corto(&c), Tono::Valor),
                    ],
                    apunta_a: None,
                    colgante: false,
                }),
            }
        }
        let (estado, tono) = if b.vivo() {
            ("vivo", Tono::Vivo)
        } else {
            ("liberado", Tono::Liberado)
        };
        heap.push(Caja {
            titulo: format!("#{} 0x{:x} {estado}", b.id, b.addr),
            tono_titulo: tono,
            lineas,
            id_bloque: Some(b.id),
        });
    }
    (pila, heap)
}

fn ancho_linea(l: &LineaCaja) -> usize {
    l.segmentos.iter().map(|(s, _)| s.chars().count()).sum()
}

/// Dibuja una caja en (fila, col) con ancho interior `ancho`; devuelve la fila siguiente
/// y, por cada línea con puntero, (fila, id de bloque, colgante).
fn dibujar_caja(
    l: &mut Lienzo,
    caja: &Caja,
    fila: usize,
    col: usize,
    ancho: usize,
) -> (usize, Vec<(usize, u64, bool)>) {
    let borde = if caja.tono_titulo == Tono::Liberado {
        Tono::Liberado
    } else {
        Tono::Borde
    };
    l.poner(fila, col, '┌', borde);
    for i in 0..ancho + 2 {
        l.poner(fila, col + 1 + i, '─', borde);
    }
    l.poner(fila, col + ancho + 3, '┐', borde);
    l.texto(
        fila,
        col + 2,
        &format!(" {} ", caja.titulo),
        caja.tono_titulo,
        ancho,
    );
    let mut f = fila + 1;
    let mut salidas = vec![];
    for linea in &caja.lineas {
        l.poner(f, col, '│', borde);
        let mut x = col + 2;
        let mut resto = ancho;
        for (s, tono) in &linea.segmentos {
            let n = s.chars().count().min(resto);
            l.texto(f, x, s, *tono, n);
            x += n;
            resto -= n;
        }
        l.poner(f, col + ancho + 3, '│', borde);
        if let Some(id) = linea.apunta_a {
            salidas.push((f, id, linea.colgante));
        }
        f += 1;
    }
    l.poner(f, col, '└', borde);
    for i in 0..ancho + 2 {
        l.poner(f, col + 1 + i, '─', borde);
    }
    l.poner(f, col + ancho + 3, '┘', borde);
    (f + 1, salidas)
}

fn trazar(l: &mut Lienzo, fila: usize, col: usize, c: char, tono: Tono) {
    let previo = l.get(fila, col);
    let nuevo = match (previo, c) {
        ('│', '─') | ('─', '│') | ('┼', _) => '┼',
        _ => c,
    };
    l.poner(fila, col, nuevo, tono);
}

/// Dibuja el paso en un lienzo de `ancho` columnas como máximo.
pub fn dibujar(paso: &Paso, ancho: usize) -> Lienzo {
    let (pila, heap) = cajas(paso);
    let max_interior = |cs: &[Caja]| {
        cs.iter()
            .flat_map(|c| {
                c.lineas
                    .iter()
                    .map(ancho_linea)
                    .chain([c.titulo.chars().count() + 2])
            })
            .max()
            .unwrap_or(10)
    };
    let carriles = pila
        .iter()
        .flat_map(|c| c.lineas.iter())
        .filter(|l| l.apunta_a.is_some())
        .count()
        .min(8);
    let gutter = 3 + carriles;
    let disponible = ancho.saturating_sub(gutter + 8).max(20);
    let mut ip = max_interior(&pila).min(disponible * 55 / 100).max(12);
    let mut ih = max_interior(&heap)
        .min(disponible.saturating_sub(ip))
        .max(12);
    if heap.is_empty() {
        ip = max_interior(&pila).min(ancho.saturating_sub(4)).max(12);
        ih = 0;
    }
    let col_heap = ip + 4 + gutter;
    let alto_pila: usize = pila.iter().map(|c| c.lineas.len() + 2).sum::<usize>() + 1;
    let alto_heap: usize = heap.iter().map(|c| c.lineas.len() + 2).sum::<usize>() + 1;
    let ancho_total = if heap.is_empty() {
        ip + 4
    } else {
        col_heap + ih + 4
    };
    let mut l = Lienzo::nuevo(ancho_total, alto_pila.max(alto_heap) + 1);

    l.texto(0, 0, "STACK", Tono::Titulo, ip + 4);
    if !heap.is_empty() {
        l.texto(0, col_heap, "HEAP", Tono::Titulo, ih + 4);
    }
    let mut f = 1;
    let mut salidas = vec![];
    for c in &pila {
        let (sig, s) = dibujar_caja(&mut l, c, f, 0, ip);
        salidas.extend(s);
        f = sig;
    }
    let mut filas_bloque = std::collections::BTreeMap::new();
    let mut f = 1;
    for c in &heap {
        if let Some(id) = c.id_bloque {
            filas_bloque.insert(id, f);
        }
        f = dibujar_caja(&mut l, c, f, col_heap, ih).0;
    }
    // Flechas: sale por el borde derecho de la caja, baja o sube por su carril y entra
    // por la izquierda del bloque a la altura de su título.
    for (i, (fila, id, colgante)) in salidas.into_iter().enumerate() {
        let Some(&destino) = filas_bloque.get(&id) else {
            continue;
        };
        let tono = if colgante {
            Tono::Peligro
        } else {
            Tono::Flecha
        };
        let carril = ip + 5 + (i % carriles.max(1));
        for x in ip + 4..carril {
            trazar(&mut l, fila, x, '─', tono);
        }
        if destino == fila {
            trazar(&mut l, fila, carril, '─', tono);
        } else {
            let (ini, fin) = if destino > fila {
                (fila, destino)
            } else {
                (destino, fila)
            };
            for y in ini + 1..fin {
                trazar(&mut l, y, carril, '│', tono);
            }
            let (esq_origen, esq_destino) = if destino > fila {
                ('┐', '└')
            } else {
                ('┘', '┌')
            };
            l.poner(fila, carril, esq_origen, tono);
            l.poner(destino, carril, esq_destino, tono);
        }
        for x in carril + 1..col_heap - 1 {
            trazar(&mut l, destino, x, '─', tono);
        }
        l.poner(destino, col_heap - 1, '►', tono);
    }
    l
}

#[cfg(test)]
mod tests {
    use super::*;

    fn traza() -> Traza {
        Traza::cargar(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/datos/trace.json"))
            .unwrap()
    }

    #[test]
    fn carga_y_alinea() {
        let t = traza();
        assert_eq!(t.steps.len(), 11);
        assert_eq!(t.alinear("main", 10, 1, 0), Some(7));
        assert_eq!(
            t.alinear("main", 6, 1, 8),
            Some(2),
            "vuelve a buscar desde el principio"
        );
        assert_eq!(t.alinear("otra", 6, 1, 0), None);
    }

    #[test]
    fn dibuja_flechas_al_heap() {
        let t = traza();
        let l = dibujar(&t.steps[6], 100); // main:9, ambos bloques vivos
        let s = l.a_texto();
        assert!(s.contains("STACK") && s.contains("HEAP"), "{s}");
        assert!(s.contains("main() p.c:9"), "{s}");
        assert!(s.contains("#1 0x") && s.contains("#2 0x"), "{s}");
        assert_eq!(s.matches('►').count(), 2, "{s}");
        assert!(s.contains("sig = NULL"), "{s}");
        assert!(!s.contains("colgante"), "{s}");
    }

    #[test]
    fn marca_puntero_colgante() {
        let t = traza();
        let paso = &t.steps[8]; // tras free(a), antes de a[0] = 1
        let l = dibujar(paso, 100);
        let s = l.a_texto();
        assert!(s.contains("⚠ colgante"), "{s}");
        assert!(s.contains("liberado"), "{s}");
        assert!(l
            .filas
            .iter()
            .flatten()
            .any(|c| c.tono == Tono::Peligro && c.c == '►'));
    }

    #[test]
    fn eventos_y_resumen() {
        let t = traza();
        let e = eventos_del_paso(&t.steps[1]);
        assert!(e[0].1.starts_with("malloc(8) → #1 en p.c:5"), "{e:?}");
        let e = eventos_del_paso(&t.steps[7]);
        assert_eq!(e[0], (Tono::Liberado, "free(#1) en p.c:9".to_string()));
        let r = resumen_final(&t);
        assert!(r.iter().any(|(_, s)| s.contains("sin fugas")), "{r:?}");
        let mut con_fuga = t.clone();
        con_fuga.meta.final_heap.blocks[1].status = "alive".into();
        let r = resumen_final(&con_fuga);
        assert!(
            r.iter()
                .any(|(tono, s)| *tono == Tono::Peligro && s.contains("fuga: #2 de 16 B")),
            "{r:?}"
        );
        let doble = Paso {
            events: vec![
                serde_json::json!({"kind": "free", "id": 1, "double_free": true, "site": {"file": "a.c", "line": 7}}),
            ],
            ..Default::default()
        };
        assert!(eventos_del_paso(&doble)[0]
            .1
            .contains("free doble de #1 en a.c:7"));
    }

    #[test]
    fn traza_real_con_free_doble_y_fuga() {
        // mem-trace sobre un ejercicio de plantilla-TP con Makefile.
        let t = Traza::cargar(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/datos/trace_doble_free.json"),
        )
        .unwrap();
        let ultimo = t.steps.last().unwrap();
        let e = eventos_del_paso(ultimo);
        assert!(
            e.iter()
                .any(|(tono, s)| *tono == Tono::Peligro
                    && s.contains("free doble de #1 en main.c:9")),
            "{e:?}"
        );
        let r = resumen_final(&t);
        assert!(r.iter().any(|(_, s)| s.contains("señal SIGABRT")), "{r:?}");
        assert!(
            r.iter()
                .any(|(_, s)| s.contains("fuga: #2 de 4 B reservado en main.c:7")),
            "{r:?}"
        );
    }

    #[test]
    fn la_memoria_pedida_en_otro_archivo_del_ejercicio() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let ej = r.join("ejercicios/ej3");
        std::fs::create_dir_all(&ej).unwrap();
        std::fs::write(r.join("Makefile"), "all:\n").unwrap();
        std::fs::write(ej.join("Makefile"), "all:\n\tgcc main.c copia.c -o main\n").unwrap();
        std::fs::write(
            ej.join("copia.h"),
            "#ifndef COPIA_H\n#define COPIA_H\nint *copiar(int n);\n#endif\n",
        )
        .unwrap();
        std::fs::write(
            ej.join("copia.c"),
            "#include <stdlib.h>\n#include \"copia.h\"\n\nint *copiar(int n)\n{\n    int *c = malloc(n * sizeof *c);\n    return c;\n}\n",
        )
        .unwrap();
        std::fs::write(
            ej.join("main.c"),
            "#include <stdlib.h>\n#include \"copia.h\"\n\nint main(void)\n{\n    int *c = copiar(3);\n    c[0] = 1;\n    free(c);\n    return 0;\n}\n",
        )
        .unwrap();
        let p = crate::Proyecto::abrir(r);
        let r = &p.raiz;
        let ej = r.join("ejercicios/ej3");
        let salida = r.join(".jonesy/memtrace");
        let cmd = comando_memtrace(&p, &ej.join("main.c"), &salida).unwrap();
        // Los fuentes van juntos en uno: mem-trace sigue sólo el archivo que recibe.
        assert!(
            cmd.contains("programa.c") && !cmd.contains("--cc-arg"),
            "{cmd}"
        );
        let juntado = std::fs::read_to_string(salida.join("programa.c")).unwrap();
        assert!(juntado.contains("int main(void)") && juntado.contains("int *copiar(int n)"));
        // Una traza sobre el archivo juntado vuelve a los archivos originales.
        let traza = serde_json::json!({
            "meta": {"source": salida.join("programa.c"), "final_heap": {"blocks": []}},
            "steps": [{
                "n": 1, "func": "copiar", "line": 17,
                "frames": [
                    {"func": "copiar", "file": salida.join("programa.c"), "line": 17, "locals": []},
                    {"func": "main", "file": salida.join("programa.c"), "line": 6, "locals": []}
                ],
                "heap": {"blocks": [{"id": 1, "addr": 4096, "size": 12, "status": "alive",
                    "site": {"func": "copiar", "file": salida.join("programa.c"), "line": 16}}]},
                "events": [{"kind": "malloc", "id": 1, "site": {"file": salida.join("programa.c"), "line": 16}}]
            }]
        });
        std::fs::write(salida.join("trace.json"), traza.to_string()).unwrap();
        let t = Traza::cargar(&salida.join("trace.json")).unwrap();
        let paso = &t.steps[0];
        assert_eq!(paso.line, 7);
        assert!(paso.frames[0].file.ends_with("copia.c") && paso.frames[0].line == 7);
        assert!(paso.frames[1].file.ends_with("main.c") && paso.frames[1].line == 6);
        let b = &paso.heap.blocks[0];
        assert!(
            b.site.file.ends_with("copia.c") && b.site.line == 6,
            "{b:?}"
        );
        assert!(
            eventos_del_paso(paso)[0].1.contains("copia.c:6"),
            "{:?}",
            eventos_del_paso(paso)
        );
        assert!(t.meta.source.ends_with("main.c"));
        // Con un header propio sin guarda, por separado.
        std::fs::write(ej.join("copia.h"), "int *copiar(int n);\n").unwrap();
        let cmd = comando_memtrace(&p, &ej.join("main.c"), &salida).unwrap();
        assert!(
            cmd.contains("--cc-arg=") && !cmd.contains("programa.c"),
            "{cmd}"
        );
        assert!(!salida.join("amalgama.json").exists());
        // Con mem-trace instalado, de punta a punta: el bloque de copiar() en copia.c.
        if !crate::proyecto::en_path("mem-trace") || !crate::proyecto::en_path("gdb") {
            return;
        }
        std::fs::write(
            ej.join("copia.h"),
            "#ifndef COPIA_H\n#define COPIA_H\nint *copiar(int n);\n#endif\n",
        )
        .unwrap();
        let cmd = comando_memtrace(&p, &ej.join("main.c"), &salida).unwrap();
        let o = std::process::Command::new("sh")
            .args(["-c", &cmd])
            .current_dir(r)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let t = Traza::cargar(&salida.join("trace.json")).unwrap();
        let bloques: Vec<&Bloque> = t.steps.iter().flat_map(|p| &p.heap.blocks).collect();
        assert!(
            bloques
                .iter()
                .any(|b| b.site.file.ends_with("copia.c") && b.site.line == 6),
            "{bloques:?}"
        );
    }

    #[test]
    fn angosto_no_rompe() {
        let t = traza();
        for p in &t.steps {
            let l = dibujar(p, 30);
            assert!(l.filas.iter().all(|f| f.len() == l.ancho));
        }
    }
}
