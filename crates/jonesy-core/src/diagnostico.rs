//! Diagnósticos: los del compilador (GCC/Clang) y los pedagógicos de `ripley`.

use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severidad {
    Error,
    Advertencia,
    Nota,
    Estilo,
}

impl Severidad {
    pub fn etiqueta(self) -> &'static str {
        match self {
            Severidad::Error => "error",
            Severidad::Advertencia => "advertencia",
            Severidad::Nota => "nota",
            Severidad::Estilo => "estilo",
        }
    }

    fn de_texto(s: &str) -> Severidad {
        match s.to_ascii_lowercase().as_str() {
            "error" | "fatal error" | "critico" | "crítico" => Severidad::Error,
            "warning" | "advertencia" => Severidad::Advertencia,
            "note" | "nota" | "info" => Severidad::Nota,
            _ => Severidad::Estilo,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostico {
    pub archivo: Option<PathBuf>,
    /// 1-based, como los imprime el compilador.
    pub linea: usize,
    pub columna: usize,
    pub severidad: Severidad,
    pub mensaje: String,
    /// Código de regla de cátedra (`0x3001h`) en los de ripley.
    pub codigo: Option<String>,
    pub sugerencia: Option<String>,
    pub origen: Origen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origen {
    Compilador,
    Ripley,
    /// Extensiones (nativas o del worker Python).
    Plugin,
}

/// Interpreta la salida de GCC/Clang: líneas `archivo:línea:col: severidad: mensaje`.
/// Las rutas relativas se resuelven contra `dir`.
pub fn parsear_gcc(salida: &str, dir: &Path) -> Vec<Diagnostico> {
    let mut diags = Vec::new();
    // `make -C sub` imprime «Entering directory '…'»: las rutas de gcc que siguen son
    // relativas a ese directorio, no a la raíz.
    let mut pila: Vec<PathBuf> = vec![dir.to_path_buf()];
    for linea in salida.lines() {
        if let Some(d) = directorio_de_make(linea, "Entering directory")
            .or_else(|| directorio_de_make(linea, "Se entra en el directorio"))
        {
            pila.push(d);
            continue;
        }
        if directorio_de_make(linea, "Leaving directory")
            .or_else(|| directorio_de_make(linea, "Se sale del directorio"))
            .is_some()
        {
            if pila.len() > 1 {
                pila.pop();
            }
            continue;
        }
        if let Some(d) =
            parsear_linea_gcc(linea, pila.last().unwrap()).or_else(|| parsear_enlazador(linea))
        {
            diags.push(d);
        }
    }
    diags
}

/// Directorio de una línea `make[1]: Entering directory '/ruta'` (comillas ' ` « o ").
fn directorio_de_make(linea: &str, frase: &str) -> Option<PathBuf> {
    if !linea.starts_with("make") {
        return None;
    }
    let resto = &linea[linea.find(frase)? + frase.len()..];
    let resto = resto
        .trim()
        .trim_start_matches(['\'', '`', '«', '"', '‘', '“']);
    let fin = resto
        .rfind(['\'', '»', '"', '’', '”'])
        .unwrap_or(resto.len());
    Some(PathBuf::from(&resto[..fin]))
}

/// Errores del enlazador, que no tienen archivo:línea: «ld: cannot find -lx» y
/// «undefined reference to `f'». «collect2: error: ld returned…» se omite (repite).
fn parsear_enlazador(linea: &str) -> Option<Diagnostico> {
    let l = linea.trim();
    let mensaje = if let Some(i) = l.find("undefined reference to") {
        // «main.c:(.text+0x1a): undefined reference to `f'» o «/usr/bin/ld: main.o: in function…»
        l[i..].to_string()
    } else {
        let (prefijo, resto) = l.split_once(": ")?;
        let es_ld = prefijo.ends_with("ld")
            || prefijo.ends_with("ld.bfd")
            || prefijo.ends_with("ld.gold")
            || prefijo.ends_with("ld.lld")
            || prefijo.ends_with("ld.exe");
        // «ld: main.o: in function `main':» es contexto de la línea siguiente.
        if !es_ld || resto.starts_with("warning") || resto.contains(": in function") {
            return None;
        }
        resto.to_string()
    };
    let explicacion = if mensaje.starts_with("cannot find -l") {
        Some("la biblioteca no está compilada o falta su ruta (-L): compilala primero")
    } else if mensaje.starts_with("undefined reference") {
        Some("la función se declaró pero su definición no se compiló ni enlazó")
    } else {
        None
    };
    Some(Diagnostico {
        archivo: None,
        linea: 0,
        columna: 0,
        severidad: Severidad::Error,
        mensaje: format!("enlazador: {mensaje}"),
        codigo: None,
        sugerencia: explicacion.map(str::to_string),
        origen: Origen::Compilador,
    })
}

fn parsear_linea_gcc(linea: &str, dir: &Path) -> Option<Diagnostico> {
    // archivo:L:C: tipo: mensaje   o   archivo:L: tipo: mensaje
    let mut partes = linea.splitn(5, ':');
    let archivo = partes.next()?.trim();
    if archivo.is_empty()
        || archivo.contains(' ') && !archivo.ends_with(".c") && !archivo.ends_with(".h")
    {
        return None;
    }
    let l: usize = partes.next()?.trim().parse().ok()?;
    let tercero = partes.next()?;
    let (columna, tipo, resto) = match tercero.trim().parse::<usize>() {
        Ok(c) => (
            c,
            partes.next()?.trim().to_string(),
            partes.next().unwrap_or("").to_string(),
        ),
        Err(_) => {
            let resto: Vec<&str> = partes.collect();
            (1, tercero.trim().to_string(), resto.join(":"))
        }
    };
    let tipo_l = tipo.to_ascii_lowercase();
    if !["error", "warning", "note", "fatal error"].contains(&tipo_l.as_str()) {
        return None;
    }
    let ruta = PathBuf::from(archivo);
    let ruta = if ruta.is_absolute() {
        ruta
    } else {
        dir.join(ruta)
    };
    Some(Diagnostico {
        archivo: Some(ruta),
        linea: l,
        columna,
        severidad: Severidad::de_texto(&tipo_l),
        mensaje: resto.trim().to_string(),
        codigo: None,
        sugerencia: None,
        origen: Origen::Compilador,
    })
}

#[derive(Deserialize)]
struct SalidaRipley {
    #[serde(default)]
    ast_findings: Vec<HallazgoRipley>,
}

/// Un hallazgo de ripley. ripley emite cada campo en inglés y en español a la vez
/// (`file` y `archivo`, `line` y `linea`...): no sirven `#[serde(alias)]` (serde
/// rechaza el campo duplicado), así que se leen por separado y se prefiere el inglés.
#[derive(Deserialize)]
struct HallazgoRipley {
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    archivo: Option<String>,
    #[serde(default)]
    line: Option<usize>,
    #[serde(default)]
    linea: Option<usize>,
    #[serde(default)]
    column: Option<usize>,
    #[serde(default)]
    columna: Option<usize>,
    #[serde(default)]
    severity: Option<String>,
    #[serde(default)]
    severidad: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    mensaje: Option<String>,
    #[serde(default)]
    suggestion: Option<String>,
    #[serde(default)]
    sugerencia: Option<String>,
    #[serde(default)]
    rule_code: Option<String>,
    #[serde(default)]
    codigo: Option<String>,
}

/// Interpreta `ripley check --format json`. Las rutas se resuelven contra `dir`.
pub fn parsear_ripley(json: &str, dir: &Path) -> Result<Vec<Diagnostico>, String> {
    // ripley puede imprimir avisos antes del JSON: se toma desde la primera llave.
    let inicio = json.find('{').ok_or("ripley no devolvió JSON")?;
    let salida: SalidaRipley = serde_json::from_str(&json[inicio..]).map_err(|e| e.to_string())?;
    Ok(salida
        .ast_findings
        .into_iter()
        .map(|h| Diagnostico {
            archivo: h.file.or(h.archivo).map(|f| {
                let p = PathBuf::from(f);
                if p.is_absolute() {
                    p
                } else {
                    dir.join(p)
                }
            }),
            linea: h.line.or(h.linea).unwrap_or(1).max(1),
            columna: h.column.or(h.columna).unwrap_or(1).max(1),
            severidad: Severidad::de_texto(
                h.severity.or(h.severidad).as_deref().unwrap_or("estilo"),
            ),
            mensaje: h.message.or(h.mensaje).unwrap_or_default(),
            codigo: h.rule_code.or(h.codigo),
            sugerencia: h.suggestion.or(h.sugerencia).filter(|s| !s.is_empty()),
            origen: Origen::Ripley,
        })
        .collect())
}

/// Categoría del panel de reglas (RF-3.4) según el código de cátedra.
pub fn categoria_regla(codigo: &str) -> &'static str {
    let c = codigo.trim_start_matches("0x").to_ascii_uppercase();
    match c.chars().next() {
        Some('0') => "Estilo y formato",
        Some('1') => "Control de flujo",
        Some('2') => "Funciones",
        Some('3') => "Memoria",
        Some('4') => "Archivos",
        Some('5') => "Preprocesador y seguridad",
        Some('6') => "Proceso",
        Some('7') => "Robustez",
        Some('8') => "Verificación y pruebas",
        _ => "Otras",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ripley_con_campos_bilingues() {
        // Forma real de ripley 1.2: cada campo en inglés y en español.
        let json = r#"{"version": "1.2.2", "ast_findings": [{"rule_code": "0x3001h", "codigo": "0x3001h",
            "severity": "ERROR", "severidad": "ERROR", "file": "main.c", "archivo": "main.c",
            "line": 4, "linea": 4, "column": 10, "columna": 10,
            "message": "Verificá malloc", "mensaje": "Verificá malloc",
            "suggestion": "if (p == NULL)", "sugerencia": "if (p == NULL)"}]}"#;
        let d = parsear_ripley(json, Path::new("/tp")).unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].codigo.as_deref(), Some("0x3001h"));
        assert_eq!((d[0].linea, d[0].columna), (4, 10));
        assert_eq!(d[0].severidad, Severidad::Error);
        assert_eq!(d[0].archivo.as_deref(), Some(Path::new("/tp/main.c")));
    }

    #[test]
    fn errores_del_enlazador() {
        let salida = "/usr/bin/ld.bfd: cannot find -larreglos: No such file or directory\n\
                      collect2: error: ld returned 1 exit status\n\
                      /usr/bin/ld: main.o: in function `main':\n\
                      main.c:(.text+0x1a): undefined reference to `lista_crear'\n";
        let d = parsear_gcc(salida, Path::new("/tp"));
        assert_eq!(d.len(), 2, "{d:?}");
        assert!(d[0].mensaje.contains("cannot find -larreglos") && d[0].archivo.is_none());
        assert_eq!(d[0].severidad, Severidad::Error);
        assert!(d[1]
            .mensaje
            .contains("undefined reference to `lista_crear'"));
    }

    #[test]
    fn rutas_relativas_al_directorio_de_make() {
        let salida = "make: Entering directory '/tp/ejercicios/ej1'\n\
                      main.c:3:5: error: 'x' undeclared\n\
                      make: Leaving directory '/tp/ejercicios/ej1'\n\
                      lista.c:1:1: warning: algo\n";
        let d = parsear_gcc(salida, Path::new("/tp"));
        assert_eq!(
            d[0].archivo.as_deref(),
            Some(Path::new("/tp/ejercicios/ej1/main.c"))
        );
        assert_eq!(d[1].archivo.as_deref(), Some(Path::new("/tp/lista.c")));
        let es = parsear_gcc("make[1]: se entra en el directorio «/tp/libs/a»\nmake[1]: Se entra en el directorio «/tp/libs/a»\na.c:2:1: error: x\n", Path::new("/tp"));
        assert_eq!(es[0].archivo.as_deref(), Some(Path::new("/tp/libs/a/a.c")));
    }

    #[test]
    fn interpreta_gcc() {
        let salida = "lista.c: In function 'main':\n\
                      lista.c:12:5: error: 'x' undeclared (first use in this function)\n\
                      lista.c:20:10: warning: unused variable 'y' [-Wunused-variable]\n\
                      /usr/include/stdio.h:3:1: note: declared here\n\
                      collect2: error: ld returned 1 exit status\n\
                      Makefile:3: recipe for target 'x' failed\n";
        let d = parsear_gcc(salida, Path::new("/tp"));
        assert_eq!(d.len(), 3);
        assert_eq!(d[0].archivo.as_deref(), Some(Path::new("/tp/lista.c")));
        assert_eq!(
            (d[0].linea, d[0].columna, d[0].severidad),
            (12, 5, Severidad::Error)
        );
        assert_eq!(d[1].severidad, Severidad::Advertencia);
        assert_eq!(
            d[2].archivo.as_deref(),
            Some(Path::new("/usr/include/stdio.h"))
        );
    }

    #[test]
    fn interpreta_ripley() {
        let json = r#"aviso previo
        {"version":"1.2.2","ast_findings":[{"rule_code":"0x3001h","severity":"ERROR","file":"a.c","line":3,"column":21,
        "message":"Asignación sin verificación","suggestion":"Verificá NULL"}]}"#;
        let d = parsear_ripley(json, Path::new("/tp")).unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].codigo.as_deref(), Some("0x3001h"));
        assert_eq!(d[0].severidad, Severidad::Error);
        assert_eq!(d[0].archivo.as_deref(), Some(Path::new("/tp/a.c")));
        assert_eq!(categoria_regla("0x3001h"), "Memoria");
    }
}
