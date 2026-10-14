//! Depurar → Contar malloc y free por función: corre el programa compilado con
//! `-Wl,--wrap=malloc,…` y un envoltorio que anota cada pedido y cada liberación con la
//! dirección desde donde se llamó; `addr2line` la traduce a función y línea. A
//! diferencia de mem-trace, cuenta en todos los `.c` del ejercicio y sus bibliotecas.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::catscan::FuentesPrograma;

/// El envoltorio: `--wrap` sólo cambia las llamadas del programa (no las de la libc, ni
/// las que hace fprintf), así que no se cuenta a sí mismo.
pub const ENVOLTORIO: &str = r#"#include <stdio.h>
#include <stdlib.h>

void *__real_malloc(size_t n);
void __real_free(void *p);
void *__real_calloc(size_t n, size_t t);
void *__real_realloc(void *p, size_t n);

static FILE *jonesy_conteo(void)
{
    static FILE *f = NULL;
    static int abierto = 0;
    if (!abierto)
    {
        abierto = 1;
        f = fopen(JONESY_CONTEO_LOG, "w");
        if (f != NULL)
        {
            setvbuf(f, NULL, _IOLBF, 0);
        }
    }
    return f;
}

void *__wrap_malloc(size_t n)
{
    void *p = __real_malloc(n);
    FILE *f = jonesy_conteo();
    if (f != NULL)
    {
        fprintf(f, "m %p %zu %p\n", p, n, __builtin_return_address(0));
    }
    return p;
}

void *__wrap_calloc(size_t n, size_t t)
{
    void *p = __real_calloc(n, t);
    FILE *f = jonesy_conteo();
    if (f != NULL)
    {
        fprintf(f, "m %p %zu %p\n", p, n * t, __builtin_return_address(0));
    }
    return p;
}

void *__wrap_realloc(void *viejo, size_t n)
{
    void *p = __real_realloc(viejo, n);
    FILE *f = jonesy_conteo();
    if (f != NULL)
    {
        fprintf(f, "r %p %p %zu %p\n", viejo, p, n, __builtin_return_address(0));
    }
    return p;
}

void __wrap_free(void *p)
{
    FILE *f = jonesy_conteo();
    if (f != NULL)
    {
        fprintf(f, "f %p %p\n", p, __builtin_return_address(0));
    }
    __real_free(p);
}
"#;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PorFuncion {
    pub funcion: String,
    /// Dónde está (archivo:línea del primer pedido o liberación).
    pub lugar: String,
    pub pedidos: usize,
    pub bytes: u64,
    pub liberaciones: usize,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Informe {
    pub funciones: Vec<PorFuncion>,
    /// (función, archivo:línea, bloques, bytes) de lo que quedó sin liberar.
    pub sin_liberar: Vec<(String, String, usize, u64)>,
    /// free de un puntero que no venía de malloc (o ya liberado).
    pub frees_raros: Vec<(String, String)>,
    pub pedidos_fallidos: usize,
}

/// Dirección de retorno → (función, archivo:línea).
pub type Ubicaciones = HashMap<u64, (String, String)>;

fn hex(s: &str) -> Option<u64> {
    if s == "(nil)" {
        return Some(0);
    }
    u64::from_str_radix(s.trim_start_matches("0x"), 16).ok()
}

/// Interpreta el registro del envoltorio. `ubicar` traduce direcciones de retorno a
/// (función, archivo:línea).
pub fn analizar(registro: &str, ubicar: &dyn Fn(&[u64]) -> Ubicaciones) -> Informe {
    enum Ev {
        Pide(u64, u64, u64),
        Libera(u64, u64),
        Realloc(u64, u64, u64, u64),
    }
    let eventos: Vec<Ev> = registro
        .lines()
        .filter_map(|l| {
            let p: Vec<&str> = l.split_whitespace().collect();
            match p.as_slice() {
                ["m", ptr, n, ret] => Some(Ev::Pide(hex(ptr)?, n.parse().ok()?, hex(ret)?)),
                ["f", ptr, ret] => Some(Ev::Libera(hex(ptr)?, hex(ret)?)),
                ["r", v, nuevo, n, ret] => Some(Ev::Realloc(
                    hex(v)?,
                    hex(nuevo)?,
                    n.parse().ok()?,
                    hex(ret)?,
                )),
                _ => None,
            }
        })
        .collect();
    let mut direcciones: Vec<u64> = eventos
        .iter()
        .map(|e| match e {
            Ev::Pide(_, _, r) | Ev::Libera(_, r) | Ev::Realloc(_, _, _, r) => *r,
        })
        .collect();
    direcciones.sort_unstable();
    direcciones.dedup();
    let lugares = ubicar(&direcciones);
    let lugar = |r: u64| {
        lugares
            .get(&r)
            .cloned()
            .unwrap_or_else(|| ("?".into(), format!("0x{r:x}")))
    };
    let mut por: BTreeMap<String, PorFuncion> = BTreeMap::new();
    let mut vivos: HashMap<u64, (u64, u64)> = HashMap::new();
    let mut inf = Informe::default();
    let anotar = |r: u64, pide: Option<u64>, por: &mut BTreeMap<String, PorFuncion>| {
        let (f, l) = lugar(r);
        let e = por.entry(f.clone()).or_insert_with(|| PorFuncion {
            funcion: f,
            lugar: l,
            ..Default::default()
        });
        match pide {
            Some(n) => {
                e.pedidos += 1;
                e.bytes += n;
            }
            None => e.liberaciones += 1,
        }
    };
    for ev in eventos {
        match ev {
            Ev::Pide(p, n, r) => {
                anotar(r, Some(n), &mut por);
                if p == 0 {
                    inf.pedidos_fallidos += 1;
                } else {
                    vivos.insert(p, (n, r));
                }
            }
            Ev::Libera(p, r) => {
                if p == 0 {
                    continue; // free(NULL) no hace nada
                }
                anotar(r, None, &mut por);
                if vivos.remove(&p).is_none() {
                    inf.frees_raros.push(lugar(r));
                }
            }
            Ev::Realloc(viejo, nuevo, n, r) => {
                anotar(r, Some(n), &mut por);
                if viejo != 0 && nuevo != 0 {
                    vivos.remove(&viejo);
                }
                if nuevo != 0 {
                    vivos.insert(nuevo, (n, r));
                } else if n > 0 {
                    inf.pedidos_fallidos += 1;
                }
            }
        }
    }
    let mut sin: BTreeMap<(String, String), (usize, u64)> = BTreeMap::new();
    for (n, r) in vivos.values() {
        let e = sin.entry(lugar(*r)).or_default();
        e.0 += 1;
        e.1 += n;
    }
    inf.sin_liberar = sin
        .into_iter()
        .map(|((f, l), (b, n))| (f, l, b, n))
        .collect();
    inf.funciones = por.into_values().collect();
    inf.funciones
        .sort_by_key(|a| std::cmp::Reverse(a.pedidos + a.liberaciones));
    inf
}

/// `addr2line -f`: función y archivo:línea de cada dirección de retorno (se busca la
/// anterior, que es la de la llamada).
pub fn ubicar_con_addr2line(programa: &Path, raiz: &Path, direcciones: &[u64]) -> Ubicaciones {
    let mut m = HashMap::new();
    if direcciones.is_empty() {
        return m;
    }
    let Ok(o) = Command::new("addr2line")
        .arg("-f")
        .arg("-e")
        .arg(programa)
        .args(
            direcciones
                .iter()
                .map(|d| format!("0x{:x}", d.saturating_sub(1))),
        )
        .output()
    else {
        return m;
    };
    let texto = String::from_utf8_lossy(&o.stdout);
    let lineas: Vec<&str> = texto.lines().collect();
    for (i, d) in direcciones.iter().enumerate() {
        let (Some(f), Some(l)) = (lineas.get(2 * i), lineas.get(2 * i + 1)) else {
            break;
        };
        let l = l.split(" (").next().unwrap_or(l);
        let l = Path::new(l)
            .strip_prefix(raiz)
            .map(|x| x.to_string_lossy().into_owned())
            .unwrap_or_else(|_| l.to_string());
        m.insert(*d, (f.to_string(), l));
    }
    m
}

pub fn markdown(inf: &Informe, programa: &str) -> String {
    let mut s = format!("# malloc y free por función\n\nPrograma: `{programa}`\n\n");
    if inf.funciones.is_empty() {
        s.push_str("El programa no pidió memoria dinámica (ni malloc, ni calloc, ni realloc).\n");
        return s;
    }
    s.push_str("| Función | Dónde | Pide | Bytes | Libera |\n|---|---|---|---|---|\n");
    for f in &inf.funciones {
        s.push_str(&format!(
            "| `{}` | {} | {} | {} | {} |\n",
            f.funcion, f.lugar, f.pedidos, f.bytes, f.liberaciones
        ));
    }
    s.push('\n');
    if inf.sin_liberar.is_empty() {
        s.push_str("✓ Todo lo que se pidió se liberó.\n");
    } else {
        let bloques: usize = inf.sin_liberar.iter().map(|x| x.2).sum();
        let bytes: u64 = inf.sin_liberar.iter().map(|x| x.3).sum();
        s.push_str(&format!(
            "**Sin liberar al terminar:** {bloques} bloque(s), {bytes} bytes (fugas). Se pidieron en:\n\n"
        ));
        for (f, l, b, n) in &inf.sin_liberar {
            s.push_str(&format!("- `{f}` ({l}): {b} bloque(s), {n} bytes\n"));
        }
        s.push_str("\nCada malloc necesita su free cuando ya no se usa el bloque.\n");
    }
    if !inf.frees_raros.is_empty() {
        s.push_str("\n**free de algo que no venía de malloc (o ya liberado):**\n\n");
        for (f, l) in &inf.frees_raros {
            s.push_str(&format!("- `{f}` ({l})\n"));
        }
    }
    if inf.pedidos_fallidos > 0 {
        s.push_str(&format!(
            "\n{} pedido(s) devolvieron NULL (no había memoria).\n",
            inf.pedidos_fallidos
        ));
    }
    s
}

/// Compila con el envoltorio en `dir_trabajo`, corre el programa y arma el informe.
pub fn contar(
    p: &crate::Proyecto,
    fuentes: &FuentesPrograma,
    entrada: Option<&Path>,
    args: &[String],
    dir_trabajo: &Path,
) -> Result<String, String> {
    std::fs::create_dir_all(dir_trabajo).map_err(|e| e.to_string())?;
    let envoltorio = dir_trabajo.join("envoltorio.c");
    let registro = dir_trabajo.join("conteo.log");
    let programa: PathBuf = dir_trabajo.join("programa");
    std::fs::write(&envoltorio, ENVOLTORIO).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&registro);
    let mut gcc = Command::new("gcc");
    gcc.args(["-std=c11", "-g", "-O0", "-no-pie"])
        .arg(format!(
            "-DJONESY_CONTEO_LOG=\"{}\"",
            registro
                .display()
                .to_string()
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
        ))
        .arg("-Wl,--wrap=malloc,--wrap=free,--wrap=calloc,--wrap=realloc");
    for i in &fuentes.includes {
        gcc.arg("-I").arg(i);
    }
    gcc.arg(&fuentes.main)
        .args(&fuentes.otras)
        .arg(&envoltorio)
        .arg("-o")
        .arg(&programa)
        .arg("-lm");
    let o = gcc
        .output()
        .map_err(|e| format!("no pude correr gcc: {e}"))?;
    if !o.status.success() {
        let err = String::from_utf8_lossy(&o.stderr);
        return Err(format!(
            "No compila (corregilo con F6 primero):\n{}",
            err.lines().take(8).collect::<Vec<_>>().join("\n")
        ));
    }
    let dir_programa = fuentes.main.parent().unwrap_or(&p.raiz);
    let corrida = crate::casos::correr_con(
        &programa,
        args,
        dir_programa,
        entrada,
        crate::casos::Limites {
            tiempo: Duration::from_secs(10),
            memoria_mb: None,
        },
    )
    .map_err(|e| e.to_string())?;
    let texto = std::fs::read_to_string(&registro).unwrap_or_default();
    let inf = analizar(&texto, &|d| ubicar_con_addr2line(&programa, &p.raiz, d));
    let mut md = markdown(&inf, &p.relativa(&fuentes.main));
    if corrida.colgado {
        md.push_str("\nEl programa no terminó en 10 s y se cortó: el conteo es hasta ahí.\n");
    } else if let Some(c) = corrida.codigo.filter(|c| *c != 0) {
        md.push_str(&format!("\nEl programa terminó con código {c}.\n"));
    }
    if entrada.is_none() {
        md.push_str("\nCorrió sin entrada (si lee del teclado, elegí un archivo con Construir → Ejecutar con entrada).\n");
    }
    Ok(md)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuenta_por_funcion_y_encuentra_fugas() {
        let registro = "m 0x10 12 0x401000\nm 0x20 8 0x401000\nf 0x10 0x402000\nr 0x20 0x30 16 0x403000\nf (nil) 0x402000\nf 0x99 0x402000\nm (nil) 1000 0x401000\n";
        let ubicar = |d: &[u64]| {
            d.iter()
                .map(|x| {
                    let f = match *x {
                        0x401000 => ("crear", "lista.c:10"),
                        0x402000 => ("main", "main.c:20"),
                        _ => ("agrandar", "lista.c:30"),
                    };
                    (*x, (f.0.to_string(), f.1.to_string()))
                })
                .collect()
        };
        let inf = analizar(registro, &ubicar);
        let crear = inf.funciones.iter().find(|f| f.funcion == "crear").unwrap();
        assert_eq!((crear.pedidos, crear.bytes), (3, 1020));
        let main = inf.funciones.iter().find(|f| f.funcion == "main").unwrap();
        assert_eq!(main.liberaciones, 2, "free(NULL) no cuenta");
        // El bloque agrandado con realloc no se liberó.
        assert_eq!(
            inf.sin_liberar,
            vec![("agrandar".into(), "lista.c:30".into(), 1, 16)]
        );
        assert_eq!(inf.frees_raros.len(), 1);
        assert_eq!(inf.pedidos_fallidos, 1);
        let md = markdown(&inf, "main.c");
        assert!(
            md.contains("| `crear` | lista.c:10 | 3 | 1020 | 0 |"),
            "{md}"
        );
        assert!(
            md.contains("Sin liberar al terminar:** 1 bloque(s), 16 bytes"),
            "{md}"
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn con_un_programa_real() {
        if Command::new("gcc").arg("--version").output().is_err()
            || Command::new("addr2line").arg("--version").output().is_err()
        {
            return;
        }
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            d.path().join("copia.c"),
            "#include <stdlib.h>\nint *copiar(int n)\n{\n    return malloc(n * sizeof(int));\n}\n",
        )
        .unwrap();
        std::fs::write(
            d.path().join("main.c"),
            "#include <stdlib.h>\nint *copiar(int n);\nint main(void)\n{\n    int *a = copiar(3);\n    int *b = copiar(5);\n    free(a);\n    (void)b;\n    return 0;\n}\n",
        )
        .unwrap();
        let p = crate::Proyecto::abrir(d.path());
        let f = FuentesPrograma {
            main: d.path().join("main.c"),
            otras: vec![d.path().join("copia.c")],
            includes: vec![d.path().to_path_buf()],
        };
        let md = contar(&p, &f, None, &[], &d.path().join(".jonesy/conteo")).unwrap();
        assert!(md.contains("| `copiar` | copia.c:4 | 2 | 32 | 0 |"), "{md}");
        assert!(md.contains("| `main` | main.c:7 | 0 | 0 | 1 |"), "{md}");
        assert!(md.contains("1 bloque(s), 20 bytes"), "{md}");
    }
}
