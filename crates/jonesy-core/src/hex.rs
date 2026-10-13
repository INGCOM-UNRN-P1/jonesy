//! Visor hexadecimal: reconoce archivos binarios (ejecutables, objetos `.o`, bibliotecas
//! `.a` y `.so`, imágenes…), describe su formato (cabecera ELF, secciones, miembros de un
//! `.a`) y arma las filas offset · bytes · ASCII.

use std::path::{Path, PathBuf};

/// Hasta cuánto se lee de un archivo para mostrarlo.
pub const LIMITE: usize = 16 * 1024 * 1024;

const EXTENSIONES: &[&str] = &[
    "o", "a", "so", "out", "exe", "obj", "lib", "dll", "bin", "elf", "dylib",
];

/// ¿Es un binario? Por la extensión, el número mágico (ELF, `ar`) o un byte nulo al
/// principio.
pub fn es_binario(ruta: &Path, inicio: &[u8]) -> bool {
    let ext = ruta
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    ext.as_deref().is_some_and(|e| EXTENSIONES.contains(&e))
        || inicio.starts_with(b"\x7fELF")
        || inicio.starts_with(b"!<arch>\n")
        || inicio.iter().take(8000).any(|b| *b == 0)
}

/// ¿Es un binario el archivo de `ruta`? (Lee sólo el principio.)
pub fn archivo_binario(ruta: &Path) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(ruta) else {
        return false;
    };
    let mut b = vec![0u8; 8000];
    let n = f.read(&mut b).unwrap_or(0);
    n > 0 && es_binario(ruta, &b[..n])
}

/// Los binarios del proyecto (sin `.git` ni `.jonesy`), para elegir uno.
pub fn binarios(raiz: &Path, limite: usize) -> Vec<PathBuf> {
    let mut v = vec![];
    let mut pila = vec![raiz.to_path_buf()];
    let mut vistos = 0;
    while let Some(d) = pila.pop() {
        let Ok(it) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in it.flatten() {
            let nombre = e.file_name().to_string_lossy().into_owned();
            if nombre.starts_with('.') || nombre == "target" {
                continue;
            }
            let r = e.path();
            if r.is_dir() {
                pila.push(r);
            } else {
                vistos += 1;
                if vistos > 5000 {
                    break;
                }
                if archivo_binario(&r) {
                    v.push(r);
                }
            }
        }
        if v.len() >= limite {
            break;
        }
    }
    v.sort();
    v
}

/// Una fila: `00000010  7f 45 4c 46 …  |.ELF…|`, con `ancho` bytes por fila.
pub fn fila(datos: &[u8], offset: usize, ancho: usize) -> String {
    let fin = (offset + ancho).min(datos.len());
    let trozo = &datos[offset.min(fin)..fin];
    let mut s = format!("{offset:08x}  ");
    for i in 0..ancho {
        match trozo.get(i) {
            Some(b) => s.push_str(&format!("{b:02x} ")),
            None => s.push_str("   "),
        }
        if i % 8 == 7 && i + 1 < ancho {
            s.push(' ');
        }
    }
    s.push(' ');
    s.push('|');
    s.extend(trozo.iter().map(|b| visible(*b)));
    s.push('|');
    s.extend(std::iter::repeat_n(' ', ancho - trozo.len()));
    s
}

/// El carácter que representa un byte en la columna ASCII.
pub fn visible(b: u8) -> char {
    if (0x20..0x7f).contains(&b) {
        b as char
    } else {
        '.'
    }
}

/// Bytes por fila que entran en `columnas`.
pub fn ancho_para(columnas: usize) -> usize {
    if columnas >= 78 {
        16
    } else {
        8
    }
}

/// Lo que se busca: bytes en hexadecimal (`7f 45 4c`, `7f454c`) o texto (entre
/// comillas si parece hexadecimal: `"cafe"`).
pub fn patron(texto: &str) -> Vec<u8> {
    if let Some(t) = texto
        .strip_prefix('"')
        .map(|t| t.strip_suffix('"').unwrap_or(t))
    {
        return t.as_bytes().to_vec();
    }
    let limpio: String = texto.chars().filter(|c| !c.is_whitespace()).collect();
    let es_hex = !limpio.is_empty()
        && limpio.len() & 1 == 0
        && limpio.chars().all(|c| c.is_ascii_hexdigit())
        && (texto.contains(' ') || limpio.len() >= 4 || texto.starts_with("0x"));
    let limpio = limpio.trim_start_matches("0x");
    if es_hex {
        (0..limpio.len())
            .step_by(2)
            .filter_map(|i| u8::from_str_radix(&limpio[i..i + 2], 16).ok())
            .collect()
    } else {
        texto.as_bytes().to_vec()
    }
}

/// La próxima aparición de `p` desde `desde` (dando la vuelta al final).
pub fn buscar(datos: &[u8], p: &[u8], desde: usize) -> Option<usize> {
    if p.is_empty() || p.len() > datos.len() {
        return None;
    }
    let ultimo = datos.len() - p.len();
    (desde.min(ultimo + 1)..=ultimo)
        .chain(0..desde.min(ultimo + 1))
        .find(|&i| &datos[i..i + p.len()] == p)
}

/// Un offset escrito por el usuario: `0x1f0`, `1f0h`, `$1f0` o decimal.
pub fn offset(texto: &str) -> Option<usize> {
    let t = texto.trim().to_lowercase();
    if let Some(h) = t
        .strip_prefix("0x")
        .or_else(|| t.strip_prefix('$'))
        .or_else(|| t.strip_suffix('h'))
    {
        return usize::from_str_radix(h, 16).ok();
    }
    t.parse()
        .ok()
        .or_else(|| usize::from_str_radix(&t, 16).ok())
}

/// Qué es el archivo, en una línea.
pub fn resumen(ruta: &Path, d: &[u8]) -> String {
    let tam = tamano(d.len());
    if let Some(e) = Elf::leer(d) {
        let so = ruta
            .extension()
            .is_some_and(|x| x == "so" || x.to_string_lossy().starts_with("so"));
        return format!(
            "ELF de {} bits · {} · {} · {tam}",
            e.bits,
            e.tipo(so),
            e.maquina()
        );
    }
    if d.starts_with(b"!<arch>\n") {
        let n = miembros_ar(d).iter().filter(|m| !m.especial).count();
        let objetos = if n == 1 { "objeto" } else { "objetos" };
        return format!("biblioteca estática (archivo ar) · {n} {objetos} · {tam}");
    }
    let tipo = if d.starts_with(b"MZ") {
        "ejecutable de Windows (PE)"
    } else if d.starts_with(&[0xcf, 0xfa, 0xed, 0xfe]) || d.starts_with(&[0xce, 0xfa, 0xed, 0xfe]) {
        "ejecutable de macOS (Mach-O)"
    } else if d.starts_with(b"\x89PNG") {
        "imagen PNG"
    } else if d.starts_with(&[0xff, 0xd8, 0xff]) {
        "imagen JPEG"
    } else if d.starts_with(b"GIF8") {
        "imagen GIF"
    } else if d.starts_with(b"%PDF") {
        "documento PDF"
    } else if d.starts_with(b"PK\x03\x04") {
        "archivo comprimido ZIP"
    } else if d.starts_with(&[0x1f, 0x8b]) {
        "archivo comprimido gzip"
    } else if d.starts_with(b"SQLite format 3") {
        "base de datos SQLite"
    } else {
        "datos binarios"
    };
    format!("{tipo} · {tam}")
}

fn tamano(n: usize) -> String {
    if n < 1024 {
        format!("{n} bytes")
    } else if n < 1024 * 1024 {
        format!("{:.1} KiB ({n} bytes)", n as f64 / 1024.0)
    } else {
        format!("{:.1} MiB ({n} bytes)", n as f64 / 1048576.0)
    }
}

/// La información detallada (Markdown simple, una línea por renglón): cabecera,
/// secciones de un ELF o miembros de un `.a`, con su offset para ir con `g`.
pub fn informacion(ruta: &Path, d: &[u8]) -> Vec<String> {
    let mut v = vec![resumen(ruta, d), String::new()];
    if let Some(e) = Elf::leer(d) {
        v.push(format!(
            "Orden de bytes: {} · punto de entrada: 0x{:x}",
            if e.le { "little endian" } else { "big endian" },
            e.entrada
        ));
        v.push(String::new());
        v.push("Secciones (offset en el archivo · tamaño · qué es):".into());
        for s in e.secciones(d) {
            if s.nombre.is_empty() {
                continue;
            }
            v.push(format!(
                "  {:<20} 0x{:08x}  {:>8}  {}",
                s.nombre,
                s.offset,
                s.tamano,
                explicar_seccion(&s.nombre)
            ));
        }
        v.push(String::new());
        v.push("s muestra los símbolos (nm): T función definida acá, U usada pero definida en otro lado (la busca el enlazador), D/B variables globales.".into());
    } else if d.starts_with(b"!<arch>\n") {
        v.push("Objetos que contiene (offset · tamaño):".into());
        for m in miembros_ar(d) {
            if !m.especial {
                v.push(format!(
                    "  {:<28} 0x{:08x}  {:>8}",
                    m.nombre, m.offset, m.tamano
                ));
            }
        }
        v.push(String::new());
        v.push("Una biblioteca estática es un paquete de .o: el enlazador toma de acá los que definen lo que el programa usa. s muestra sus símbolos.".into());
    }
    if d.len() >= LIMITE {
        v.push(String::new());
        v.push(format!(
            "El archivo es grande: se muestran los primeros {}.",
            tamano(LIMITE)
        ));
    }
    v
}

fn explicar_seccion(n: &str) -> &'static str {
    match n {
        ".text" => "el código de máquina de las funciones",
        ".data" => "globales y static con valor inicial",
        ".bss" => "globales y static en cero (no ocupan lugar en el archivo)",
        ".rodata" => "constantes y cadenas literales",
        ".symtab" => "la tabla de símbolos (nombres de funciones y variables)",
        ".strtab" => "los nombres de los símbolos",
        ".dynsym" | ".dynstr" => "símbolos para el enlace dinámico",
        ".plt" | ".plt.got" | ".plt.sec" => "saltos a funciones de bibliotecas compartidas",
        ".got" => "direcciones resueltas al cargar el programa",
        ".init" | ".fini" | ".init_array" | ".fini_array" => "código de arranque y cierre",
        ".interp" => "qué cargador dinámico usa",
        ".dynamic" => "información para el cargador dinámico",
        ".comment" => "versión del compilador",
        ".eh_frame" | ".eh_frame_hdr" => "información para desenrollar la pila",
        n if n.starts_with(".debug") => "información de depuración (-g) para gdb",
        n if n.starts_with(".rela") || n.starts_with(".rel") => {
            "reubicaciones: lugares que el enlazador completa"
        }
        n if n.starts_with(".note") => "notas (ABI, id de compilación)",
        _ => "",
    }
}

struct Elf {
    bits: u8,
    le: bool,
    tipo: u16,
    maquina: u16,
    entrada: u64,
    shoff: u64,
    shentsize: u16,
    shnum: u16,
    shstrndx: u16,
}

pub struct Seccion {
    pub nombre: String,
    pub offset: u64,
    pub tamano: u64,
}

fn u16_de(d: &[u8], i: usize, le: bool) -> Option<u16> {
    let b: [u8; 2] = d.get(i..i + 2)?.try_into().ok()?;
    Some(if le {
        u16::from_le_bytes(b)
    } else {
        u16::from_be_bytes(b)
    })
}

fn u32_de(d: &[u8], i: usize, le: bool) -> Option<u32> {
    let b: [u8; 4] = d.get(i..i + 4)?.try_into().ok()?;
    Some(if le {
        u32::from_le_bytes(b)
    } else {
        u32::from_be_bytes(b)
    })
}

fn u64_de(d: &[u8], i: usize, le: bool) -> Option<u64> {
    let b: [u8; 8] = d.get(i..i + 8)?.try_into().ok()?;
    Some(if le {
        u64::from_le_bytes(b)
    } else {
        u64::from_be_bytes(b)
    })
}

impl Elf {
    fn leer(d: &[u8]) -> Option<Elf> {
        if !d.starts_with(b"\x7fELF") {
            return None;
        }
        let bits = match d.get(4)? {
            1 => 32,
            2 => 64,
            _ => return None,
        };
        let le = *d.get(5)? != 2;
        let tipo = u16_de(d, 16, le)?;
        let maquina = u16_de(d, 18, le)?;
        Some(if bits == 64 {
            Elf {
                bits,
                le,
                tipo,
                maquina,
                entrada: u64_de(d, 24, le)?,
                shoff: u64_de(d, 0x28, le)?,
                shentsize: u16_de(d, 0x3a, le)?,
                shnum: u16_de(d, 0x3c, le)?,
                shstrndx: u16_de(d, 0x3e, le)?,
            }
        } else {
            Elf {
                bits,
                le,
                tipo,
                maquina,
                entrada: u32_de(d, 24, le)? as u64,
                shoff: u32_de(d, 0x20, le)? as u64,
                shentsize: u16_de(d, 0x2e, le)?,
                shnum: u16_de(d, 0x30, le)?,
                shstrndx: u16_de(d, 0x32, le)?,
            }
        })
    }

    fn tipo(&self, so: bool) -> &'static str {
        match self.tipo {
            1 => "objeto reubicable (.o, falta enlazar)",
            2 => "ejecutable",
            3 if so => "biblioteca compartida (.so)",
            3 => "ejecutable (PIE)",
            4 => "volcado de memoria (core)",
            _ => "tipo desconocido",
        }
    }

    fn maquina(&self) -> String {
        match self.maquina {
            0x3e => "x86-64".into(),
            0x03 => "x86 (32 bits)".into(),
            0xb7 => "ARM de 64 bits (AArch64)".into(),
            0x28 => "ARM".into(),
            0xf3 => "RISC-V".into(),
            m => format!("máquina 0x{m:x}"),
        }
    }

    /// (nombre, tipo, offset, tamaño) de la sección `i`.
    fn cabecera(&self, d: &[u8], i: usize) -> Option<(u32, u64, u64)> {
        let base = usize::try_from(self.shoff).ok()? + i * self.shentsize as usize;
        let le = self.le;
        if self.bits == 64 {
            Some((
                u32_de(d, base, le)?,
                u64_de(d, base + 24, le)?,
                u64_de(d, base + 32, le)?,
            ))
        } else {
            Some((
                u32_de(d, base, le)?,
                u32_de(d, base + 16, le)? as u64,
                u32_de(d, base + 20, le)? as u64,
            ))
        }
    }

    fn secciones(&self, d: &[u8]) -> Vec<Seccion> {
        if self.shoff == 0 || self.shnum == 0 || self.shnum > 4096 {
            return vec![];
        }
        let nombres = self
            .cabecera(d, self.shstrndx as usize)
            .map(|(_, o, _)| o as usize);
        (0..self.shnum as usize)
            .filter_map(|i| {
                let (n, offset, tamano) = self.cabecera(d, i)?;
                let nombre = nombres
                    .and_then(|base| d.get(base + n as usize..))
                    .map(|r| {
                        let fin = r.iter().position(|b| *b == 0).unwrap_or(0);
                        String::from_utf8_lossy(&r[..fin]).into_owned()
                    })
                    .unwrap_or_default();
                Some(Seccion {
                    nombre,
                    offset,
                    tamano,
                })
            })
            .collect()
    }
}

pub struct Miembro {
    pub nombre: String,
    pub offset: usize,
    pub tamano: usize,
    /// Tabla de símbolos o de nombres largos (no es un objeto).
    pub especial: bool,
}

/// Los miembros de un archivo `ar` (biblioteca estática `.a`).
pub fn miembros_ar(d: &[u8]) -> Vec<Miembro> {
    let mut v = vec![];
    let mut largos: &[u8] = &[];
    let mut i = 8;
    while i + 60 <= d.len() && v.len() < 10_000 {
        let h = &d[i..i + 60];
        if &h[58..60] != b"`\n" {
            break;
        }
        let crudo = String::from_utf8_lossy(&h[..16]).trim_end().to_string();
        let tamano: usize = String::from_utf8_lossy(&h[48..58])
            .trim()
            .parse()
            .unwrap_or(0);
        let datos = i + 60;
        let especial = crudo == "/" || crudo == "//" || crudo.starts_with("__.SYMDEF");
        if crudo == "//" {
            largos = d.get(datos..datos + tamano).unwrap_or(&[]);
        }
        let nombre = match crudo.strip_prefix('/') {
            Some(n) if !especial => n
                .parse::<usize>()
                .ok()
                .and_then(|o| largos.get(o..))
                .map(|r| {
                    let fin = r
                        .iter()
                        .position(|b| *b == b'/' || *b == b'\n')
                        .unwrap_or(r.len());
                    String::from_utf8_lossy(&r[..fin]).into_owned()
                })
                .unwrap_or(crudo.clone()),
            _ => crudo.trim_end_matches('/').to_string(),
        };
        v.push(Miembro {
            nombre,
            offset: datos,
            tamano,
            especial,
        });
        i = datos + tamano + tamano % 2;
    }
    v
}

/// Los símbolos del binario con `nm` (si está), explicados.
pub fn simbolos(ruta: &Path) -> Result<Vec<String>, String> {
    let o = std::process::Command::new("nm")
        .args(["-C", "--no-sort"])
        .arg(ruta)
        .output()
        .map_err(|_| "nm no está instalado (viene con binutils)".to_string())?;
    let texto = String::from_utf8_lossy(&o.stdout);
    let mut v = vec![
        "Símbolos (nm): T función definida · U usada, definida en otro lado · D/B/R variable global (con valor, en cero, constante) · t/d/b locales (static)".to_string(),
        String::new(),
    ];
    for l in texto.lines() {
        v.push(l.to_string());
    }
    if v.len() == 2 {
        let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
        v.push(if err.is_empty() {
            "(sin símbolos: el binario está «stripped»)".into()
        } else {
            err
        });
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filas_busqueda_y_offsets() {
        let d: Vec<u8> = b"\x7fELF hola\x00\x01mundo".to_vec();
        let f = fila(&d, 0, 16);
        assert!(
            f.starts_with("00000000  7f 45 4c 46 20 68 6f 6c  61 00 01 6d"),
            "{f}"
        );
        assert!(f.ends_with("|.ELF hola..mundo|"), "{f}");
        assert_eq!(fila(&d, 16, 8).len(), fila(&d, 0, 8).len());
        assert_eq!(patron("7f 45"), vec![0x7f, 0x45]);
        assert_eq!(patron("mundo"), b"mundo".to_vec());
        assert_eq!(patron("cafe"), vec![0xca, 0xfe]);
        assert_eq!(patron("\"cafe\""), b"cafe".to_vec());
        assert_eq!(buscar(&d, &patron("mundo"), 0), Some(11));
        // Da la vuelta.
        assert_eq!(buscar(&d, b"ELF", 5), Some(1));
        assert_eq!(buscar(&d, b"zzz", 0), None);
        assert_eq!(offset("0x10"), Some(16));
        assert_eq!(offset("10h"), Some(16));
        assert_eq!(offset("32"), Some(32));
        assert_eq!(ancho_para(60), 8);
    }

    #[test]
    fn reconoce_binarios() {
        assert!(es_binario(Path::new("x.o"), b"cualquier"));
        assert!(es_binario(Path::new("prog"), b"\x7fELF\x02"));
        assert!(es_binario(Path::new("datos"), b"ab\x00cd"));
        assert!(!es_binario(Path::new("main.c"), b"int main(void) {}\n"));
        assert!(resumen(Path::new("x.png"), b"\x89PNG\r\n").contains("PNG"));
    }

    #[test]
    #[cfg(unix)]
    fn elf_y_ar_reales() {
        let d = tempfile::tempdir().unwrap();
        let c = d.path().join("f.c");
        std::fs::write(&c, "int global = 3;\nint doble(int x) { return 2 * x; }\n").unwrap();
        let o = d.path().join("f.o");
        let ok = std::process::Command::new("gcc")
            .args(["-c", "-o"])
            .arg(&o)
            .arg(&c)
            .status()
            .is_ok_and(|s| s.success());
        if !ok {
            return; // sin gcc
        }
        let datos = std::fs::read(&o).unwrap();
        assert!(resumen(&o, &datos).contains("objeto reubicable"));
        let info = informacion(&o, &datos).join("\n");
        assert!(
            info.contains(".text") && info.contains("código de máquina"),
            "{info}"
        );
        let a = d.path().join("libf.a");
        let ar = std::process::Command::new("ar")
            .arg("rcs")
            .arg(&a)
            .arg(&o)
            .status()
            .is_ok_and(|s| s.success());
        if ar {
            let datos = std::fs::read(&a).unwrap();
            assert!(
                resumen(&a, &datos).contains("1 objeto ·"),
                "{}",
                resumen(&a, &datos)
            );
            assert!(informacion(&a, &datos).join("\n").contains("f.o"));
        }
        if let Ok(s) = simbolos(&o) {
            let s = s.join("\n");
            assert!(s.contains("doble") || s.contains("nm"), "{s}");
        }
    }
}
