//! cat-scan: vista semigráfica de stack y heap a partir del `trace.json` de `mem-trace`.
//!
//! `mem-trace run <fuente.c> --outdir <dir>` ejecuta el programa paso a paso bajo GDB y
//! guarda en `trace.json` el estado de cada paso: marcos con sus variables (escalares,
//! punteros con su región y destino, structs) y los bloques del heap con su estado. Acá se
//! carga esa traza, se alinea con la parada actual del depurador y se dibuja en una
//! cuadrícula de caracteres: marcos en cajas a la izquierda, bloques a la derecha y
//! flechas `──►` ruteadas por carriles verticales entre ambos. Los punteros a bloques
//! liberados se marcan como colgantes.

use std::path::Path;

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
        let archivo = Path::new(&self.file).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
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
    pub fn cargar(ruta: &Path) -> Result<Traza, String> {
        let texto = std::fs::read_to_string(ruta).map_err(|e| format!("{}: {e}", ruta.display()))?;
        serde_json::from_str(&texto).map_err(|e| format!("trace.json inválido: {e}"))
    }

    /// Paso que corresponde a una parada del depurador en `funcion` / `linea` con
    /// `profundidad` marcos. Busca hacia adelante desde `desde` (las paradas avanzan) y,
    /// si no encuentra, desde el principio.
    pub fn alinear(&self, funcion: &str, linea: usize, profundidad: usize, desde: usize) -> Option<usize> {
        let coincide = |p: &Paso| {
            p.line == linea && p.func == funcion && (profundidad == 0 || p.frames.len() == profundidad)
        };
        (desde..self.steps.len())
            .chain(0..desde.min(self.steps.len()))
            .find(|&i| coincide(&self.steps[i]))
            .or_else(|| {
                // Sin coincidencia exacta de profundidad (recursión), alcanza con función y línea.
                (0..self.steps.len()).find(|&i| self.steps[i].line == linea && self.steps[i].func == funcion)
            })
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
        Lienzo { filas: vec![vec![Celda { c: ' ', tono: Tono::Normal }; ancho]; alto], ancho }
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
        self.filas.get(fila).and_then(|f| f.get(col)).map(|c| c.c).unwrap_or(' ')
    }

    /// Texto plano (para pruebas y para copiar).
    pub fn a_texto(&self) -> String {
        self.filas.iter().map(|f| f.iter().map(|c| c.c).collect::<String>().trim_end().to_string()).collect::<Vec<_>>().join("\n")
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
        Some("array") => format!("[{}]", v["elements"].as_array().map(|a| a.len()).unwrap_or(0)),
        _ => v.get("value").map(|x| x.to_string()).unwrap_or_else(|| "?".into()),
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
                segmentos.push((valor_corto(v), if colgante { Tono::Peligro } else { Tono::Direccion }));
                if colgante {
                    segmentos.push((" ⚠ colgante".into(), Tono::Peligro));
                }
            }
            None => {
                let nulo = v["region"] == "null";
                segmentos.push((valor_corto(v), if nulo { Tono::Valor } else { Tono::Direccion }));
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
    out.push(LineaCaja { segmentos, apunta_a, colgante });
    if v["kind"] == "struct" {
        for f in v["fields"].as_array().into_iter().flatten() {
            lineas_variable(paso, f["name"].as_str().unwrap_or("?"), &f["value"], sangria + 2, out);
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
    paso.frames.iter().flat_map(|m| m.locals.iter()).chain(paso.globals.iter()).find_map(|var| buscar(&var.val, b, 0))
}

fn cajas(paso: &Paso) -> (Vec<Caja>, Vec<Caja>) {
    let mut pila = vec![];
    if !paso.globals.is_empty() {
        let mut lineas = vec![];
        for g in &paso.globals {
            lineas_variable(paso, &g.name, &g.val, 0, &mut lineas);
        }
        pila.push(Caja { titulo: "globales".into(), tono_titulo: Tono::Titulo, lineas, id_bloque: None });
    }
    for m in &paso.frames {
        let mut lineas = vec![];
        for l in &m.locals {
            lineas_variable(paso, &l.name, &l.val, 0, &mut lineas);
        }
        if lineas.is_empty() {
            lineas.push(LineaCaja { segmentos: vec![("(sin variables)".into(), Tono::Tipo)], apunta_a: None, colgante: false });
        }
        let archivo = Path::new(&m.file).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        pila.push(Caja { titulo: format!("{}() {archivo}:{}", m.func, m.line), tono_titulo: Tono::Titulo, lineas, id_bloque: None });
    }
    let mut heap = vec![];
    for b in &paso.heap.blocks {
        let mut lineas = vec![LineaCaja {
            segmentos: vec![(format!("{} B en ", b.size), Tono::Normal), (b.site.corto(), Tono::Direccion)],
            apunta_a: None,
            colgante: false,
        }];
        if let Some(fs) = &b.free_site {
            lineas.push(LineaCaja {
                segmentos: vec![("free en ".into(), Tono::Liberado), (fs.corto(), Tono::Liberado)],
                apunta_a: None,
                colgante: false,
            });
        } else if let Some(c) = contenido_bloque(paso, b) {
            match c["kind"].as_str() {
                Some("struct") => {
                    for f in c["fields"].as_array().into_iter().flatten() {
                        let mut ls = vec![];
                        lineas_variable(paso, f["name"].as_str().unwrap_or("?"), &f["value"], 0, &mut ls);
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
                    segmentos: vec![(format!("{} ", tipo_de(&c)), Tono::Tipo), (valor_corto(&c), Tono::Valor)],
                    apunta_a: None,
                    colgante: false,
                }),
            }
        }
        let (estado, tono) = if b.vivo() { ("vivo", Tono::Vivo) } else { ("liberado", Tono::Liberado) };
        heap.push(Caja { titulo: format!("#{} 0x{:x} {estado}", b.id, b.addr), tono_titulo: tono, lineas, id_bloque: Some(b.id) });
    }
    (pila, heap)
}

fn ancho_linea(l: &LineaCaja) -> usize {
    l.segmentos.iter().map(|(s, _)| s.chars().count()).sum()
}

/// Dibuja una caja en (fila, col) con ancho interior `ancho`; devuelve la fila siguiente
/// y, por cada línea con puntero, (fila, id de bloque, colgante).
fn dibujar_caja(l: &mut Lienzo, caja: &Caja, fila: usize, col: usize, ancho: usize) -> (usize, Vec<(usize, u64, bool)>) {
    let borde = if caja.tono_titulo == Tono::Liberado { Tono::Liberado } else { Tono::Borde };
    l.poner(fila, col, '┌', borde);
    for i in 0..ancho + 2 {
        l.poner(fila, col + 1 + i, '─', borde);
    }
    l.poner(fila, col + ancho + 3, '┐', borde);
    l.texto(fila, col + 2, &format!(" {} ", caja.titulo), caja.tono_titulo, ancho);
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
        cs.iter().flat_map(|c| c.lineas.iter().map(ancho_linea).chain([c.titulo.chars().count() + 2])).max().unwrap_or(10)
    };
    let carriles = pila.iter().flat_map(|c| c.lineas.iter()).filter(|l| l.apunta_a.is_some()).count().min(8);
    let gutter = 3 + carriles;
    let disponible = ancho.saturating_sub(gutter + 8).max(20);
    let mut ip = max_interior(&pila).min(disponible * 55 / 100).max(12);
    let mut ih = max_interior(&heap).min(disponible.saturating_sub(ip)).max(12);
    if heap.is_empty() {
        ip = max_interior(&pila).min(ancho.saturating_sub(4)).max(12);
        ih = 0;
    }
    let col_heap = ip + 4 + gutter;
    let alto_pila: usize = pila.iter().map(|c| c.lineas.len() + 2).sum::<usize>() + 1;
    let alto_heap: usize = heap.iter().map(|c| c.lineas.len() + 2).sum::<usize>() + 1;
    let ancho_total = if heap.is_empty() { ip + 4 } else { col_heap + ih + 4 };
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
        let Some(&destino) = filas_bloque.get(&id) else { continue };
        let tono = if colgante { Tono::Peligro } else { Tono::Flecha };
        let carril = ip + 5 + (i % carriles.max(1));
        for x in ip + 4..carril {
            trazar(&mut l, fila, x, '─', tono);
        }
        if destino == fila {
            trazar(&mut l, fila, carril, '─', tono);
        } else {
            let (ini, fin) = if destino > fila { (fila, destino) } else { (destino, fila) };
            for y in ini + 1..fin {
                trazar(&mut l, y, carril, '│', tono);
            }
            let (esq_origen, esq_destino) = if destino > fila { ('┐', '└') } else { ('┘', '┌') };
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
        Traza::cargar(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/datos/trace.json")).unwrap()
    }

    #[test]
    fn carga_y_alinea() {
        let t = traza();
        assert_eq!(t.steps.len(), 11);
        assert_eq!(t.alinear("main", 10, 1, 0), Some(7));
        assert_eq!(t.alinear("main", 6, 1, 8), Some(2), "vuelve a buscar desde el principio");
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
        assert!(l.filas.iter().flatten().any(|c| c.tono == Tono::Peligro && c.c == '►'));
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
