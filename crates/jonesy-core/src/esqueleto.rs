//! Construir → Crear una prueba para esta función: el esqueleto de un `TEST` de
//! `p1_test` para la función del cursor, con una variable por parámetro, la llamada y
//! la aserción que corresponde al tipo que devuelve, más su `RUN_TEST` en el `main` del
//! archivo de pruebas.

use std::path::{Path, PathBuf};

use crate::documentar::Firma;

/// El archivo de pruebas del módulo de `ruta`: `prueba.c`, `pruebas.c` o `test*.c` en
/// el mismo directorio, en el del módulo o en su `test/` / `tests/`.
pub fn archivo_de_pruebas(p: &crate::Proyecto, ruta: &Path) -> Option<PathBuf> {
    let mut dirs = vec![];
    if let Some(d) = ruta.parent() {
        dirs.push(d.to_path_buf());
        if let Some(arriba) = d.parent() {
            dirs.push(arriba.to_path_buf());
        }
    }
    if let Some(m) = p.estructura.modulo_de(ruta) {
        dirs.push(m.dir.clone());
    }
    let mut con_sub = vec![];
    for d in &dirs {
        con_sub.push(d.clone());
        con_sub.push(d.join("test"));
        con_sub.push(d.join("tests"));
    }
    for d in con_sub {
        for n in ["prueba.c", "pruebas.c"] {
            if d.join(n).is_file() {
                return Some(d.join(n));
            }
        }
        let Ok(it) = std::fs::read_dir(&d) else {
            continue;
        };
        let mut otros: Vec<PathBuf> = it
            .flatten()
            .map(|e| e.path())
            .filter(|x| {
                x.extension().is_some_and(|e| e == "c")
                    && x.file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with("test"))
            })
            .collect();
        otros.sort();
        if let Some(x) = otros.into_iter().next() {
            return Some(x);
        }
    }
    None
}

/// Una variable de prueba por parámetro y cómo se la pasa a la función.
fn preparar(decl: &str, nombre: &str) -> (String, String) {
    let d = decl.replace("const ", "");
    let d = d.trim();
    let es_char = d.starts_with("char");
    if let Some(i) = d.find('[') {
        // Un arreglo: con unos valores de ejemplo.
        let tipo = d[..i].trim().trim_end_matches(nombre).trim();
        return (format!("{tipo} {nombre}[] = {{0}};"), nombre.into());
    }
    if d.contains("(*") {
        return (format!("/* {decl}: pasá una función */"), "NULL".into());
    }
    let estrellas = d.matches('*').count();
    let tipo = d.replace('*', " ");
    let tipo = tipo.trim().trim_end_matches(nombre).trim().to_string();
    match (estrellas, es_char) {
        (0, _) => {
            let valor = if tipo.contains("double") || tipo.contains("float") {
                "0.0"
            } else if tipo == "bool" {
                "false"
            } else {
                "0"
            };
            let primitivo = [
                "int", "long", "short", "char", "double", "float", "size_t", "unsigned", "bool",
                "signed",
            ]
            .iter()
            .any(|t| tipo.split_whitespace().any(|x| x == *t));
            if !primitivo {
                (format!("{tipo} {nombre} = {{0}};"), nombre.into())
            } else {
                (format!("{tipo} {nombre} = {valor};"), nombre.into())
            }
        }
        // Una cadena.
        (1, true) => (format!("char {nombre}[] = \"\";"), nombre.into()),
        // Un puntero a un valor: la variable y su dirección.
        (1, false)
            if [
                "int", "long", "double", "float", "short", "size_t", "unsigned", "bool",
            ]
            .iter()
            .any(|t| tipo.contains(t)) =>
        {
            let valor = if tipo.contains("double") || tipo.contains("float") {
                "0.0"
            } else {
                "0"
            };
            (format!("{tipo} {nombre} = {valor};"), format!("&{nombre}"))
        }
        _ => (
            format!("{tipo} {}{nombre} = NULL;", "*".repeat(estrellas)),
            nombre.into(),
        ),
    }
}

/// La aserción según el tipo que devuelve la función.
fn asercion(retorno: &str, llamada: &str) -> String {
    let r = retorno
        .replace("static ", "")
        .replace("inline ", "")
        .replace("const ", "");
    let r = r.trim();
    if r == "void" {
        return format!(
            "    {llamada};\n    /* Verificá lo que cambió (los parámetros por puntero, el arreglo…). */\n    ASSERT_INT_EQ(0 /* esperado */, 0 /* obtenido */);"
        );
    }
    let a = if r.contains("char") && r.contains('*') {
        format!("ASSERT_STR_EQ(\"\" /* esperado */, {llamada});")
    } else if r.contains('*') {
        format!("ASSERT_PTR_NOT_NULL({llamada});")
    } else if r == "bool" {
        format!("ASSERT_TRUE({llamada});")
    } else if r.contains("double") || r.contains("float") {
        format!("ASSERT_DOUBLE_EQ(0.0 /* esperado */, {llamada}, 1e-9);")
    } else if r.contains("unsigned") || r.contains("size_t") {
        format!("ASSERT_UINT_EQ(0 /* esperado */, {llamada});")
    } else {
        format!("ASSERT_INT_EQ(0 /* esperado */, {llamada});")
    };
    format!("    {a}")
}

/// El `TEST(…)` para `f`, llamado `nombre`.
pub fn prueba(f: &Firma, nombre: &str) -> String {
    let mut cuerpo = vec![format!("    SUBCASE(\"Caso básico de {}\");", f.nombre)];
    let mut args = vec![];
    for decl in &f.declaraciones {
        let Some(n) = decl
            .split(['*', ' ', '['])
            .rev()
            .find(|x| !x.is_empty() && x.chars().all(|c| c.is_alphanumeric() || c == '_'))
        else {
            continue;
        };
        if decl == "..." {
            continue;
        }
        let n = if let Some(i) = decl.find("(*") {
            decl[i + 2..]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect()
        } else {
            n.to_string()
        };
        let (linea, arg) = preparar(decl, &n);
        cuerpo.push(format!("    {linea}"));
        args.push(arg);
    }
    let llamada = format!("{}({})", f.nombre, args.join(", "));
    cuerpo.push(asercion(&f.retorno, &llamada));
    format!("TEST({nombre})\n{{\n{}\n}}\n", cuerpo.join("\n"))
}

/// Un nombre de prueba que no esté en `fuente`.
pub fn nombre_libre(fuente: &str, funcion: &str) -> String {
    let base = format!("prueba_{funcion}");
    (1..)
        .map(|i| {
            if i == 1 {
                base.clone()
            } else {
                format!("{base}_{i}")
            }
        })
        .find(|n| !fuente.contains(&format!("TEST({n})")))
        .unwrap()
}

/// El archivo de pruebas con la prueba nueva antes de `main` y su `RUN_TEST` después
/// del último. Devuelve el texto y la línea (0-based) del `TEST` nuevo.
pub fn agregar(fuente: &str, prueba: &str, nombre: &str) -> (String, usize) {
    let lineas: Vec<&str> = fuente.lines().collect();
    let main = lineas
        .iter()
        .position(|l| {
            let l = l.trim_start();
            l.starts_with("int main") || l.starts_with("int\nmain")
        })
        .unwrap_or(lineas.len());
    let mut v: Vec<String> = lineas[..main].iter().map(|s| s.to_string()).collect();
    if v.last().is_some_and(|l| !l.trim().is_empty()) {
        v.push(String::new());
    }
    let linea_test = v.len();
    v.extend(prueba.trim_end().lines().map(str::to_string));
    v.push(String::new());
    let mut resto: Vec<String> = lineas[main..].iter().map(|s| s.to_string()).collect();
    let run = format!("RUN_TEST({nombre});");
    if let Some(i) = resto
        .iter()
        .rposition(|l| l.trim_start().starts_with("RUN_TEST"))
    {
        let sangria: String = resto[i].chars().take_while(|c| c.is_whitespace()).collect();
        resto.insert(i + 1, format!("{sangria}{run}"));
    } else if let Some(i) = resto
        .iter()
        .rposition(|l| l.trim_start().starts_with("return"))
    {
        let sangria: String = resto[i].chars().take_while(|c| c.is_whitespace()).collect();
        resto.insert(i, format!("{sangria}{run}"));
    } else {
        v.push(format!("/* Agregá {run} en el main de las pruebas. */"));
    }
    v.extend(resto);
    (v.join("\n") + "\n", linea_test)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documentar::firma_en;

    #[test]
    fn esqueletos_segun_la_firma() {
        let f = firma_en("int vector_suma(const int v[], size_t n)\n{\n}\n", 0).unwrap();
        let t = prueba(&f, "prueba_vector_suma");
        assert!(
            t.starts_with(
                "TEST(prueba_vector_suma)\n{\n    SUBCASE(\"Caso básico de vector_suma\");"
            ),
            "{t}"
        );
        assert!(t.contains("    int v[] = {0};\n    size_t n = 0;\n"), "{t}");
        assert!(
            t.contains("ASSERT_INT_EQ(0 /* esperado */, vector_suma(v, n));"),
            "{t}"
        );

        let f = firma_en("void intercambiar(int *a, int *b);\n", 0).unwrap();
        let t = prueba(&f, "p");
        assert!(
            t.contains("int a = 0;") && t.contains("intercambiar(&a, &b);"),
            "{t}"
        );

        let f = firma_en("double promedio(const int v[], size_t n);\n", 0).unwrap();
        assert!(
            prueba(&f, "p").contains("ASSERT_DOUBLE_EQ(0.0 /* esperado */, promedio(v, n), 1e-9);")
        );
        let f = firma_en("char *duplicar(const char *s);\n", 0).unwrap();
        let t = prueba(&f, "p");
        assert!(
            t.contains("char s[] = \"\";")
                && t.contains("ASSERT_STR_EQ(\"\" /* esperado */, duplicar(s));"),
            "{t}"
        );
        let f = firma_en("int largo(lista_t lista, struct punto p);\n", 0).unwrap();
        let t = prueba(&f, "p");
        assert!(
            t.contains("lista_t lista = {0};") && t.contains("struct punto p = {0};"),
            "{t}"
        );
        let f = firma_en("bool es_par(int n);\n", 0).unwrap();
        assert!(prueba(&f, "p").contains("ASSERT_TRUE(es_par(n));"));
    }

    #[test]
    fn agrega_el_test_y_su_run_test() {
        let fuente = "#include \"p1_test.h\"\n\nTEST(prueba_a)\n{\n}\n\nint main(int argc, char **argv)\n{\n    RUN_TEST(prueba_a);\n    return TEST_REPORT();\n}\n";
        assert_eq!(nombre_libre(fuente, "a"), "prueba_a_2");
        let (t, l) = agregar(fuente, "TEST(prueba_b)\n{\n}\n", "prueba_b");
        assert_eq!(t.lines().nth(l), Some("TEST(prueba_b)"));
        assert!(
            t.contains("    RUN_TEST(prueba_a);\n    RUN_TEST(prueba_b);\n    return"),
            "{t}"
        );
        assert!(t.find("TEST(prueba_b)").unwrap() < t.find("int main").unwrap());
    }
}
