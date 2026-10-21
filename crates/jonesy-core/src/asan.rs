//! Errores de memoria de AddressSanitizer (y LeakSanitizer) explicados en español y
//! ubicados en el código del alumno, como los de valgrind: donde no hay valgrind
//! (Windows con clang, macOS) se compila el programa con `-fsanitize=address`.
//!
//! Cada informe empieza con `==PID==ERROR: AddressSanitizer: <tipo> …`, sigue con la pila
//! (`#0 0x… in main /ruta/m.c:14`) y secciones con su pila («freed by thread T0 here:»,
//! «previously allocated by…»). Las fugas vienen en un informe de LeakSanitizer con un
//! «Direct leak of 12 byte(s) in 1 object(s) allocated from:» por cada una.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::diagnostico::{Diagnostico, Origen, Severidad};
use crate::valgrind::{donde, propio, Marco, Resultado};

/// «#1 0x000000400531 in pedir /home/alumno/tp/m.c:7» (o `C:\tp\m.c:7:5` con clang).
fn marco(linea: &str) -> Option<Marco> {
    let t = linea.trim();
    let t = t.strip_prefix('#')?;
    let (_, resto) = t.split_once(" in ")?;
    let (funcion, ubic) = resto.split_once(' ')?;
    let ubic = ubic.trim();
    // «(/lib64/libc.so.6+0x3680)»: código de una biblioteca, sin archivo.
    if ubic.starts_with('(') {
        return Some(Marco {
            funcion: funcion.to_string(),
            archivo: String::new(),
            linea: 0,
        });
    }
    let mut partes: Vec<&str> = ubic.rsplitn(3, ':').collect();
    partes.reverse();
    let (archivo, linea) = match partes.as_slice() {
        // archivo:línea:columna (clang), salvo una unidad de Windows («C:\tp\m.c:7»).
        [a, l, c] if l.parse::<usize>().is_ok() && c.parse::<usize>().is_ok() => {
            (a.to_string(), l.parse().ok()?)
        }
        [a, b, l] => (format!("{a}:{b}"), l.parse().ok()?),
        [a, l] => (a.to_string(), l.parse().ok()?),
        _ => return None,
    };
    Some(Marco {
        funcion: funcion.to_string(),
        archivo,
        linea,
    })
}

/// Un informe: el tipo, si leyó o escribió y cuánto, la pila y las secciones.
#[derive(Debug, Default)]
struct Informe {
    tipo: String,
    titulo: String,
    acceso: Option<(bool, usize)>,
    pila: Vec<Marco>,
    secciones: Vec<(String, Vec<Marco>)>,
    /// «located 0 bytes after 12-byte region», «'a' (line 17) <== … overflows».
    datos: Vec<String>,
}

/// Saca el prefijo `==PID==` que ASan pone en algunos renglones.
fn sin_pid(l: &str) -> &str {
    if let Some(resto) = l.strip_prefix("==") {
        if let Some((pid, t)) = resto.split_once("==") {
            if pid.chars().all(|c| c.is_ascii_digit()) {
                return t;
            }
        }
    }
    l
}

fn leer(salida: &str) -> Vec<Informe> {
    let mut v: Vec<Informe> = vec![];
    // ¿Los marcos que vienen son de la pila principal (o de la última sección)?
    let mut en_seccion = false;
    for cruda in salida.lines() {
        let l = sin_pid(cruda.trim_end());
        if let Some(resto) = l
            .strip_prefix("ERROR: AddressSanitizer: ")
            .or_else(|| l.strip_prefix("ERROR: LeakSanitizer: "))
        {
            let tipo = resto.split_whitespace().next().unwrap_or("").to_string();
            v.push(Informe {
                tipo: if resto.starts_with("attempting double-free") {
                    "double-free".into()
                } else if resto.starts_with("attempting free on address which was not") {
                    "bad-free".into()
                } else if resto.starts_with("detected memory leaks") {
                    "leaks".into()
                } else {
                    tipo
                },
                titulo: resto.to_string(),
                ..Default::default()
            });
            en_seccion = false;
            continue;
        }
        let Some(inf) = v.last_mut() else { continue };
        let t = l.trim();
        if let Some(m) = marco(t) {
            if en_seccion {
                if let Some((_, p)) = inf.secciones.last_mut() {
                    p.push(m);
                }
            } else {
                inf.pila.push(m);
            }
            continue;
        }
        if t.starts_with("READ of size") || t.starts_with("WRITE of size") {
            let n = t
                .split_whitespace()
                .nth(3)
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
            inf.acceso = Some((t.starts_with("WRITE"), n));
            continue;
        }
        if t.ends_with("here:") || t.ends_with("allocated from:") {
            en_seccion = true;
            inf.secciones.push((t.to_string(), vec![]));
            continue;
        }
        if t.contains("is located") || t.contains("<== Memory access") || t.contains("zero page") {
            inf.datos.push(t.to_string());
        }
    }
    v
}

/// «0 bytes after 12-byte region» → (relación, distancia, tamaño del bloque).
fn region(datos: &[String]) -> Option<(&'static str, usize, usize)> {
    let d = datos.iter().find(|d| d.contains("-byte region"))?;
    let (_, resto) = d.split_once("is located ")?;
    let mut p = resto.split_whitespace();
    let n: usize = p.next()?.parse().ok()?;
    let _bytes = p.next()?;
    let rel = match p.next()? {
        "after" => "después del final",
        "before" => "antes del principio",
        _ => "dentro",
    };
    let tam = d
        .split_whitespace()
        .find_map(|w| w.strip_suffix("-byte"))
        .and_then(|n| n.parse().ok())?;
    Some((rel, n, tam))
}

/// La variable de la pila que se desbordó: «[64, 72) 'a' (line 17) <== …».
fn variable(datos: &[String]) -> Option<String> {
    let d = datos.iter().find(|d| d.contains("<== Memory access"))?;
    let (_, resto) = d.split_once('\'')?;
    let (nombre, _) = resto.split_once('\'')?;
    Some(nombre.to_string())
}

fn seccion<'a>(inf: &'a Informe, empieza: &str) -> Option<&'a [Marco]> {
    inf.secciones
        .iter()
        .find(|(t, _)| t.starts_with(empieza))
        .map(|(_, p)| p.as_slice())
}

fn explicar(inf: &Informe, base: &Path, raiz: &Path) -> Option<String> {
    let que = match inf.acceso {
        Some((true, n)) => format!("Escritura de {n} bytes"),
        Some((false, n)) => format!("Lectura de {n} bytes"),
        None => "Acceso".into(),
    };
    let pedido = seccion(inf, "allocated by")
        .or_else(|| seccion(inf, "previously allocated"))
        .and_then(|p| donde(p, base, raiz))
        .map(|d| format!(" pedido en {d}"))
        .unwrap_or_default();
    let liberado = seccion(inf, "freed by")
        .and_then(|p| donde(p, base, raiz))
        .map(|d| format!(" (liberado en {d})"))
        .unwrap_or_default();
    Some(match inf.tipo.as_str() {
        "heap-buffer-overflow" => {
            let r = region(&inf.datos)
                .map(|(rel, n, tam)| {
                    let distancia = if n == 0 {
                        "justo ".to_string()
                    } else {
                        format!("{n} bytes ")
                    };
                    format!(": {distancia}{rel} de un bloque de {tam} bytes{pedido}")
                })
                .unwrap_or_default();
            format!(
                "{que} fuera de la memoria pedida (¿índice fuera del arreglo?){r}"
            )
        }
        "heap-use-after-free" => format!(
            "{que} en memoria ya liberada: se usa un puntero después de su free{liberado}"
        ),
        "double-free" => format!("free doble: este bloque ya se había liberado{liberado}"),
        "bad-free" => {
            "free de algo que no se pidió con malloc (¿la dirección de una variable o un puntero corrido?)".into()
        }
        "stack-buffer-overflow" | "stack-buffer-underflow" => {
            let v = variable(&inf.datos)
                .map(|v| format!(" «{v}»"))
                .unwrap_or_default();
            format!("{que} fuera del arreglo local{v} (¿índice fuera de rango?)")
        }
        "global-buffer-overflow" => {
            format!("{que} fuera de un arreglo global (¿índice fuera de rango?)")
        }
        "stack-use-after-return" => {
            "Se usa la dirección de una variable local de una función que ya terminó (no devuelvas punteros a locales)".into()
        }
        "stack-use-after-scope" => {
            "Se usa una variable fuera del bloque donde se declaró".into()
        }
        "SEGV" => {
            if inf.datos.iter().any(|d| d.contains("zero page")) {
                "Acceso a través de un puntero NULL (¿malloc devolvió NULL o falta inicializarlo?)".into()
            } else {
                "Acceso a una dirección inválida (¿puntero sin inicializar o ya liberado?)".into()
            }
        }
        "alloc-dealloc-mismatch" => {
            "Se libera con una función que no corresponde a cómo se pidió".into()
        }
        "negative-size-param" | "calloc-overflow" | "allocation-size-too-big" => {
            "Se pide o copia un tamaño negativo o enorme (¿una cuenta que dio negativa?)".into()
        }
        "memcpy-param-overlap" | "strcpy-param-overlap" | "strcat-param-overlap" => {
            "Origen y destino se superponen: usá memmove en vez de memcpy/strcpy".into()
        }
        _ => return None,
    })
}

/// Interpreta la salida de un programa compilado con `-fsanitize=address`.
pub fn interpretar(salida: &str, raiz: &Path) -> Resultado {
    let mut r = Resultado::default();
    let base = raiz.to_path_buf();
    for inf in leer(salida) {
        r.reconocida = true;
        if inf.tipo == "leaks" {
            for (t, pila) in &inf.secciones {
                // «Direct leak of 12 byte(s) in 1 object(s) allocated from:»
                let directa = t.starts_with("Direct");
                let bytes: usize = t
                    .split_whitespace()
                    .nth(3)
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(0);
                let objetos = t
                    .split(" in ")
                    .nth(1)
                    .and_then(|x| x.split_whitespace().next())
                    .unwrap_or("1");
                let cuanto = if objetos == "1" {
                    format!("{bytes} bytes")
                } else {
                    format!("{bytes} bytes en {objetos} bloques")
                };
                if directa {
                    r.bytes_perdidos += bytes;
                }
                r.errores += 1;
                let (archivo, linea) = ubicacion(pila, &base, raiz);
                r.diagnosticos.push(diagnostico(
                    archivo,
                    linea,
                    if directa {
                        Severidad::Error
                    } else {
                        Severidad::Advertencia
                    },
                    t.trim_end_matches(':').to_string(),
                    if directa {
                        format!("Fuga de memoria: {cuanto} pedidos acá nunca se liberaron (falta un free)")
                    } else {
                        format!("Fuga indirecta: {cuanto} apuntados desde otro bloque que se perdió (al liberar la estructura, liberá también sus nodos)")
                    },
                ));
            }
            continue;
        }
        r.errores += 1;
        let Some(resumen) = explicar(&inf, &base, raiz) else {
            continue;
        };
        let (archivo, linea) = ubicacion(&inf.pila, &base, raiz);
        r.diagnosticos.push(diagnostico(
            archivo,
            linea,
            Severidad::Error,
            format!("AddressSanitizer: {}", inf.titulo),
            resumen,
        ));
    }
    for d in ubsan(salida, raiz) {
        r.reconocida = true;
        r.errores += 1;
        r.diagnosticos.push(d);
    }
    r
}

/// Los errores de UndefinedBehaviorSanitizer: una línea «m.c:7:9: runtime error: …» (con
/// `-fno-sanitize-recover`, el programa se detiene en el primero).
fn ubsan(salida: &str, raiz: &Path) -> Vec<Diagnostico> {
    salida
        .lines()
        .filter_map(|l| {
            let (ubic, mensaje) = l.trim().split_once(": runtime error: ")?;
            let mut partes: Vec<&str> = ubic.rsplitn(3, ':').collect();
            partes.reverse();
            let (archivo, linea, columna) = match partes.as_slice() {
                [a, l, c] => (*a, l.parse().ok()?, c.parse().unwrap_or(1)),
                [a, l] => (*a, l.parse().ok()?, 1),
                _ => return None,
            };
            let p = PathBuf::from(archivo);
            let p = if p.is_absolute() { p } else { raiz.join(p) };
            let mut d = diagnostico(
                Some(p),
                linea,
                Severidad::Error,
                format!("UndefinedBehaviorSanitizer: {mensaje}"),
                format!("Comportamiento indefinido: {}", explicar_ubsan(mensaje)),
            );
            d.columna = columna;
            Some(d)
        })
        .collect()
}

fn explicar_ubsan(mensaje: &str) -> String {
    let m = mensaje.to_ascii_lowercase();
    if m.contains("signed integer overflow") {
        "la cuenta se pasa del máximo (o mínimo) de un int con signo: usá un tipo más grande o verificá antes de operar".into()
    } else if m.contains("division by zero") {
        "división por cero: verificá el divisor antes de dividir".into()
    } else if m.contains("out of bounds for type") || m.contains("index") {
        "índice fuera del arreglo: revisá el rango del lazo (de 0 a N-1)".into()
    } else if m.contains("null pointer") {
        "se usa un puntero NULL: verificá que apunte a algo antes de usarlo".into()
    } else if m.contains("shift") {
        "desplazamiento de bits fuera de rango (negativo o de más bits que el tipo)".into()
    } else if m.contains("misaligned") {
        "acceso a una dirección desalineada para el tipo (¿un cast de puntero?)".into()
    } else {
        mensaje.to_string()
    }
}

fn ubicacion(pila: &[Marco], base: &Path, raiz: &Path) -> (Option<PathBuf>, usize) {
    let pila: Vec<Marco> = pila
        .iter()
        .filter(|m| !m.archivo.is_empty())
        .cloned()
        .collect();
    match propio(&pila, base, raiz) {
        Some((p, m)) => (Some(p), m.linea),
        None => (None, 0),
    }
}

fn diagnostico(
    archivo: Option<PathBuf>,
    linea: usize,
    severidad: Severidad,
    mensaje: String,
    resumen: String,
) -> Diagnostico {
    Diagnostico {
        archivo,
        linea,
        columna: 1,
        severidad,
        mensaje,
        resumen: Some(resumen),
        origen: Origen::Herramienta,
        herramienta: Some("asan".into()),
        categoria: Some("memoria".into()),
        ..Default::default()
    }
}

/// El primer compilador (`gcc`, `clang`) que compila y corre un programa con
/// AddressSanitizer en esta máquina; se averigua una vez.
pub fn compilador() -> Option<&'static str> {
    static C: OnceLock<Option<&'static str>> = OnceLock::new();
    *C.get_or_init(|| {
        ["gcc", "clang"]
            .into_iter()
            .find(|cc| crate::proyecto::en_path(cc) && funciona(cc, BANDERAS_ASAN))
    })
}

const BANDERAS_ASAN: &str = "-fsanitize=address";
/// AddressSanitizer y UndefinedBehaviorSanitizer, deteniéndose en el primer error (UBSan sólo
/// avisa y sigue, sin `-fno-sanitize-recover`).
pub const BANDERAS_SANITIZERS: &str = "-fsanitize=address,undefined -fno-sanitize-recover=all";

/// Compilador y banderas para Construir → Ejecutar con sanitizers: ASan y UBSan si el
/// compilador los trae (gcc con libasan y libubsan, clang de MSYS2 CLANG64); si sólo trae
/// ASan, ASan. Se averigua una vez.
pub fn compilador_sanitizers() -> Option<(&'static str, &'static str)> {
    static C: OnceLock<Option<(&'static str, &'static str)>> = OnceLock::new();
    *C.get_or_init(|| {
        let disponibles: Vec<&'static str> = ["gcc", "clang"]
            .into_iter()
            .filter(|cc| crate::proyecto::en_path(cc))
            .collect();
        disponibles
            .iter()
            .find(|cc| funciona(cc, BANDERAS_SANITIZERS))
            .map(|cc| (*cc, BANDERAS_SANITIZERS))
            .or_else(|| compilador().map(|cc| (cc, BANDERAS_ASAN)))
    })
}

fn funciona(cc: &str, banderas: &str) -> bool {
    let d = std::env::temp_dir().join(format!(
        "jonesy-asan-{}-{cc}-{}",
        std::process::id(),
        banderas.len()
    ));
    if std::fs::create_dir_all(&d).is_err() {
        return false;
    }
    let fuente = d.join("prueba.c");
    let programa = d.join(if cfg!(windows) {
        "prueba.exe"
    } else {
        "prueba"
    });
    if std::fs::write(&fuente, "int main(void) { return 0; }\n").is_err() {
        return false;
    }
    let compila = std::process::Command::new(cc)
        .args(banderas.split_whitespace())
        .arg("-g")
        .arg(&fuente)
        .arg("-o")
        .arg(&programa)
        .stdin(std::process::Stdio::null())
        .output()
        .is_ok_and(|o| o.status.success());
    let corre = compila
        && std::process::Command::new(&programa)
            .stdin(std::process::Stdio::null())
            .output()
            .is_ok_and(|o| o.status.success());
    let _ = std::fs::remove_dir_all(&d);
    corre
}

/// Compila el programa del archivo activo con AddressSanitizer (en `salida`) y lo corre
/// en su directorio; el programa queda al final del comando para agregarle argumentos y
/// una redirección. None si no hay programa.
pub fn comando(p: &crate::Proyecto, activo: &Path, cc: &str, salida: &Path) -> Option<String> {
    comando_con(p, activo, cc, BANDERAS_ASAN, salida)
}

/// Como [`comando`], con otras banderas de sanitizers (ver [`compilador_sanitizers`]).
pub fn comando_con(
    p: &crate::Proyecto,
    activo: &Path,
    cc: &str,
    banderas: &str,
    salida: &Path,
) -> Option<String> {
    use crate::proyecto::shell_quote as q;
    let f = crate::catscan::fuentes_del_programa(p, activo)?;
    let programa = salida.join(if cfg!(windows) {
        "programa-asan.exe"
    } else {
        "programa-asan"
    });
    let mut partes = vec![
        cc.to_string(),
        format!("-g -O0 {banderas} -fno-omit-frame-pointer"),
    ];
    for i in &f.includes {
        partes.push(format!("-I {}", q(&i.to_string_lossy())));
    }
    for c in std::iter::once(&f.main).chain(&f.otras) {
        partes.push(q(&c.to_string_lossy()));
    }
    partes.push(format!("-o {}", q(&programa.to_string_lossy())));
    let dir = f.main.parent().unwrap_or(&p.raiz).to_path_buf();
    Some(format!(
        "{} && cd {} && {}",
        partes.join(" "),
        q(&dir.to_string_lossy()),
        q(&programa.to_string_lossy())
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SALIDA: &str = include_str!("../tests/datos/asan_salida.txt");

    #[test]
    fn errores_y_fugas_en_el_codigo_del_alumno() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("m.c"), "/* … */\n").unwrap();
        let r = interpretar(SALIDA, d.path());
        assert!(r.reconocida);
        // Siete corridas: seis errores y un informe con dos fugas.
        assert_eq!(r.errores, 8);
        assert_eq!(r.bytes_perdidos, 16);
        let v: Vec<(usize, &str)> = r
            .diagnosticos
            .iter()
            .map(|d| (d.linea, d.resumen.as_deref().unwrap()))
            .collect();
        let hay = |linea: usize, texto: &str| {
            assert!(
                v.iter().any(|(l, r)| *l == linea && r.contains(texto)),
                "{linea} «{texto}»: {v:#?}"
            )
        };
        hay(
            14,
            "Escritura de 4 bytes fuera de la memoria pedida (¿índice fuera del arreglo?): justo después del final de un bloque de 12 bytes pedido en m.c:7 (pedir())",
        );
        hay(15, "en memoria ya liberada");
        hay(15, "liberado en m.c:15 (main())");
        hay(16, "free doble");
        hay(17, "fuera del arreglo local «a»");
        hay(18, "puntero NULL");
        hay(19, "no se pidió con malloc");
        hay(7, "Fuga de memoria: 12 bytes");
        hay(20, "Fuga de memoria: 4 bytes");
        assert!(r
            .diagnosticos
            .iter()
            .all(|d| d.archivo.is_some() && d.herramienta.as_deref() == Some("asan")));
    }

    #[test]
    fn marcos_de_linux_y_de_windows() {
        let m = marco("#1 0x000000400531 in pedir /home/alumno/tp/m.c:7").unwrap();
        assert_eq!(
            (m.funcion.as_str(), m.archivo.as_str(), m.linea),
            ("pedir", "/home/alumno/tp/m.c", 7)
        );
        let m = marco("#0 0x7ff6 in main C:\\tp\\m.c:14:5").unwrap();
        assert_eq!((m.archivo.as_str(), m.linea), ("C:\\tp\\m.c", 14));
        let m = marco("#0 0x7ff6 in main C:\\tp\\m.c:14").unwrap();
        assert_eq!((m.archivo.as_str(), m.linea), ("C:\\tp\\m.c", 14));
        let m = marco("#2 0x7f in __libc_start_main (/lib64/libc.so.6+0x3797)").unwrap();
        assert!(m.archivo.is_empty());
    }

    #[test]
    fn compilar_con_addresssanitizer() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("m.c"), "int main(void) { return 0; }\n").unwrap();
        let p = crate::Proyecto::abrir(d.path());
        let salida = d.path().join(".jonesy/asan");
        let c = comando(&p, &d.path().join("m.c"), "clang", &salida).unwrap();
        assert!(c.starts_with("clang -g -O0 -fsanitize=address"), "{c}");
        let esperado = if cfg!(windows) {
            "programa-asan.exe"
        } else {
            "programa-asan"
        };
        assert!(
            c.contains("m.c") && c.contains("-o ") && c.trim_end_matches('\'').ends_with(esperado),
            "{c}"
        );
    }

    #[test]
    fn comportamiento_indefinido_de_ubsan() {
        let d = tempfile::tempdir().unwrap();
        let salida = format!(
            "ingresá: 5\n{}/m.c:7:15: runtime error: signed integer overflow: 2147483647 + 1 cannot be represented in type 'int'\n",
            d.path().display()
        );
        let r = interpretar(&salida, d.path());
        assert!(r.reconocida && r.errores == 1);
        let x = &r.diagnosticos[0];
        assert_eq!((x.linea, x.columna), (7, 15));
        assert!(x.resumen.as_deref().unwrap().contains("máximo"));
        assert_eq!(x.archivo.as_deref(), Some(d.path().join("m.c").as_path()));
        let r = interpretar("t.c:3:5: runtime error: division by zero\n", d.path());
        assert!(r.diagnosticos[0]
            .resumen
            .as_deref()
            .unwrap()
            .contains("divisor"));
    }

    #[test]
    fn compilar_con_ubsan() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("m.c"), "int main(void) { return 0; }\n").unwrap();
        let p = crate::Proyecto::abrir(d.path());
        let c = comando_con(
            &p,
            &d.path().join("m.c"),
            "gcc",
            BANDERAS_SANITIZERS,
            &d.path().join(".jonesy/asan"),
        )
        .unwrap();
        assert!(
            c.starts_with("gcc -g -O0 -fsanitize=address,undefined -fno-sanitize-recover=all"),
            "{c}"
        );
    }

    #[test]
    fn sin_informe_no_se_reconoce() {
        let r = interpretar("hola\n[proceso terminado con código 0]\n", Path::new("."));
        assert!(!r.reconocida && r.diagnosticos.is_empty());
    }
}
