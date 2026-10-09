//! Errores y advertencias de gcc explicados para quien empieza con C (RNF-C.1).
//!
//! El catálogo está en [`data/errores_gcc.toml`](../data/errores_gcc.toml): cada entrada
//! reconoce mensajes de gcc con patrones simples (`{}` captura hasta el texto siguiente) y
//! da un resumen de una línea, una explicación y un ejemplo mal/bien. Reconoce los mensajes
//! en inglés y en español (gcc localizado traduce una parte).

use std::sync::OnceLock;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Entrada {
    pub id: String,
    patrones: Vec<String>,
    resumen: String,
    #[serde(default)]
    explicacion: String,
    #[serde(default)]
    mal: String,
    #[serde(default)]
    bien: String,
    /// Capturas (desde 1) que no pueden contener `*`.
    #[serde(default)]
    sin_puntero: Vec<usize>,
    /// Notas de gcc que no aportan (las repite otra entrada): se descartan.
    #[serde(default)]
    pub ocultar: bool,
}

#[derive(Deserialize)]
struct Catalogo {
    error: Vec<Entrada>,
}

/// Un mensaje de gcc reconocido.
#[derive(Debug, Clone, PartialEq)]
pub struct Explicacion {
    pub id: String,
    /// Una línea en español, con los nombres del mensaje.
    pub resumen: String,
    /// La explicación completa en Markdown (para F1).
    pub texto: String,
    /// Es una nota que repite a otra: no vale la pena mostrarla.
    pub ocultar: bool,
}

const DATOS: &str = include_str!("../data/errores_gcc.toml");

pub fn catalogo() -> &'static [Entrada] {
    static C: OnceLock<Vec<Entrada>> = OnceLock::new();
    C.get_or_init(|| {
        toml::from_str::<Catalogo>(DATOS)
            .expect("data/errores_gcc.toml es válido (lo verifica un test)")
            .error
    })
}

/// Comillas tipográficas y acento grave (las usa gcc según el locale y ld) a `'`.
fn normalizar(mensaje: &str) -> String {
    mensaje
        .chars()
        .map(|c| match c {
            '‘' | '’' | '`' | '´' => '\'',
            '“' | '”' => '"',
            c => c,
        })
        .collect()
}

/// Compara `patron` (texto con `{}` que capturan) contra `texto`. El primer tramo literal
/// se busca en cualquier lugar; cada captura llega hasta la primera aparición del tramo
/// siguiente, y la última, si el patrón termina en `{}`, hasta el final.
pub fn coincidir(patron: &str, texto: &str) -> Option<Vec<String>> {
    let tramos: Vec<&str> = patron.split("{}").collect();
    let primero = tramos[0];
    let mut desde = 0;
    while desde <= texto.len() {
        let inicio = desde + texto[desde..].find(primero)?;
        if let Some(c) = capturar(&tramos[1..], &texto[inicio + primero.len()..]) {
            return Some(c);
        }
        // Siguiente aparición del primer tramo (avanzando un carácter).
        desde = inicio + texto[inicio..].chars().next().map_or(1, char::len_utf8);
        if primero.is_empty() {
            return None;
        }
    }
    None
}

fn capturar(tramos: &[&str], mut resto: &str) -> Option<Vec<String>> {
    let mut capturas = vec![];
    for (i, t) in tramos.iter().enumerate() {
        let ultimo = i == tramos.len() - 1;
        if t.is_empty() && ultimo {
            capturas.push(resto.trim().to_string());
            resto = "";
            continue;
        }
        let fin = resto.find(t)?;
        capturas.push(resto[..fin].to_string());
        resto = &resto[fin + t.len()..];
    }
    let _ = resto;
    Some(capturas)
}

/// Qué incluir para tener `nombre` (función o tipo de la base del quick-doc).
pub fn cabecera_de(nombre: &str) -> Option<String> {
    let base = jonesy_doc::Base::embebida();
    base.funcion(nombre)
        .map(|f| f.cabecera.clone())
        .or_else(|| base.tipo(nombre).map(|t| t.cabecera.clone()))
        .filter(|c| !c.is_empty() && c.ends_with(".h"))
}

fn sugerencia_include(nombre: Option<&str>) -> String {
    match nombre.and_then(cabecera_de) {
        Some(h) => format!("agregá «#include <{h}>» al principio del archivo"),
        None => {
            "si es tuya, poné su prototipo antes (o incluí su .h); si no, revisá el nombre".into()
        }
    }
}

fn expandir(plantilla: &str, capturas: &[String]) -> String {
    let mut s = plantilla.to_string();
    if s.contains("{include}") {
        s = s.replace(
            "{include}",
            &sugerencia_include(capturas.first().map(String::as_str)),
        );
    }
    for (i, c) in capturas.iter().enumerate() {
        s = s.replace(&format!("{{{}}}", i + 1), c.trim());
    }
    s
}

/// Reconoce un mensaje de gcc o del enlazador (sin «archivo:línea:col: error:»).
pub fn explicar(mensaje: &str) -> Option<Explicacion> {
    let m = normalizar(mensaje);
    for e in catalogo() {
        for p in &e.patrones {
            let Some(capturas) = coincidir(p, &m) else {
                continue;
            };
            if e.sin_puntero
                .iter()
                .any(|&i| capturas.get(i - 1).is_some_and(|c| c.contains('*')))
            {
                continue;
            }
            let resumen = expandir(&e.resumen, &capturas);
            let mut texto = format!("# {resumen}\n\n> `{}`\n\n", mensaje.trim());
            texto.push_str(e.explicacion.trim());
            texto.push('\n');
            if !e.mal.trim().is_empty() {
                texto.push_str(&format!(
                    "\n**Así no:**\n\n```c\n{}\n```\n",
                    e.mal.trim_end()
                ));
            }
            if !e.bien.trim().is_empty() {
                texto.push_str(&format!(
                    "\n**Así sí:**\n\n```c\n{}\n```\n",
                    e.bien.trim_end()
                ));
            }
            return Some(Explicacion {
                id: e.id.clone(),
                resumen,
                texto,
                ocultar: e.ocultar,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_catalogo_es_valido_y_sin_ids_repetidos() {
        let c = catalogo();
        assert!(c.len() > 40);
        let mut ids: Vec<&str> = c.iter().map(|e| e.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), c.len(), "ids repetidos");
        for e in c {
            assert!(!e.patrones.is_empty(), "{} sin patrones", e.id);
            assert!(
                e.ocultar || !e.resumen.is_empty() && !e.explicacion.is_empty(),
                "{} sin resumen o explicación",
                e.id
            );
        }
    }

    #[test]
    fn patrones_con_capturas() {
        assert_eq!(
            coincidir(
                "'{}' undeclared",
                "'x' undeclared (first use in this function)"
            ),
            Some(vec!["x".into()])
        );
        assert_eq!(
            coincidir(
                "'{}' has no member named '{}'",
                "'struct p' has no member named 'y'"
            ),
            Some(vec!["struct p".into(), "y".into()])
        );
        assert_eq!(
            coincidir(
                "{}: No such file or directory",
                "lista.h: No such file or directory"
            ),
            Some(vec!["lista.h".into()])
        );
        assert_eq!(
            coincidir("cannot find -l{}", "cannot find -lnada"),
            Some(vec!["nada".into()])
        );
        assert!(coincidir("expected ';' before", "expected ')' before ';'").is_none());
    }

    /// Mensajes reales de gcc 16 en inglés (LC_MESSAGES=C) y el id que les corresponde.
    #[test]
    fn mensajes_reales() {
        let casos = [
            ("expected ',' or ';' before 'return'", "falta-punto-y-coma"),
            ("expected '=', ',', ';', 'asm' or '__attribute__' before 'v'", "falta-punto-y-coma"),
            ("'x' undeclared (first use in this function)", "no-declarada"),
            ("each undeclared identifier is reported only once for each function it appears in", "no-declarada-ruido"),
            ("implicit declaration of function 'printf' [-Wimplicit-function-declaration]", "declaracion-implicita"),
            ("incompatible implicit declaration of built-in function 'malloc' [-Wbuiltin-declaration-mismatch]", "builtin-incompatible"),
            ("include '<stdlib.h>' or provide a declaration of 'malloc'", "nota-include"),
            ("expected declaration or statement at end of input", "llave-sin-cerrar"),
            ("stray '\\342' in program", "caracter-extrano"),
            ("missing terminating \" character", "comilla-sin-cerrar"),
            ("conflicting types for 'f'; have 'float(int)'", "tipos-en-conflicto"),
            ("too few arguments to function 'f'; expected 2, have 1", "pocos-argumentos"),
            ("too many arguments to function 'f'; expected 1, have 2", "muchos-argumentos"),
            ("initialization of 'int *' from 'int' makes pointer from integer without a cast [-Wint-conversion]", "puntero-desde-entero"),
            ("initialization of 'int' from 'int *' makes integer from pointer without a cast [-Wint-conversion]", "entero-desde-puntero"),
            ("passing argument 1 of 'f' from incompatible pointer type [-Wincompatible-pointer-types]", "puntero-incompatible"),
            ("format '%d' expects argument of type 'int', but argument 2 has type 'double' [-Wformat=]", "formato-tipo"),
            ("format '%d' expects argument of type 'int *', but argument 2 has type 'int' [-Wformat=]", "formato-espera-puntero"),
            ("format '%s' expects argument of type 'char *', but argument 2 has type 'char (*)[10]' [-Wformat=]", "formato-arreglo-con-ampersand"),
            ("too many arguments for format [-Wformat-extra-args]", "formato-muchos-argumentos"),
            ("unused variable 'x' [-Wunused-variable]", "variable-sin-uso"),
            ("unused parameter 'b' [-Wunused-parameter]", "parametro-sin-uso"),
            ("variable 's' set but not used [-Wunused-but-set-variable=]", "asignada-sin-usar"),
            ("'x' is used uninitialized [-Wuninitialized]", "sin-inicializar"),
            ("control reaches end of non-void function [-Wreturn-type]", "sin-return"),
            ("suggest parentheses around assignment used as truth value [-Wparentheses]", "asignacion-en-condicion"),
            ("comparison between pointer and integer", "puntero-con-entero"),
            ("comparison with string literal results in unspecified behavior [-Waddress]", "comparar-cadenas"),
            ("redefinition of 'a'", "redefinicion"),
            ("'a' redeclared as different kind of symbol", "redefinicion"),
            ("'return' with a value, in function returning void [-Wreturn-mismatch]", "return-con-valor"),
            ("'return' with no value, in function returning non-void [-Wreturn-mismatch]", "return-sin-valor"),
            ("subscripted value is neither array nor pointer nor vector", "subindice"),
            ("'q' is a pointer; did you mean to use '->'?", "flecha"),
            ("invalid type argument of '->' (have 'struct p')", "punto"),
            ("invalid use of undefined type 'struct nodo'", "tipo-incompleto"),
            ("lista.h: No such file or directory", "include-no-encontrado"),
            ("unknown type name 'bool'", "tipo-desconocido"),
            ("'bool' is defined in header '<stdbool.h>'; this is probably fixable by adding '#include <stdbool.h>'", "nota-header-tipo"),
            ("lvalue required as left operand of assignment", "lvalue"),
            ("assignment to expression with array type", "asignar-arreglo"),
            ("assignment of read-only variable 'a'", "solo-lectura"),
            ("passing argument 1 of 'strlen' makes pointer from integer without a cast [-Wint-conversion]", "puntero-desde-entero"),
            ("division by zero [-Wdiv-by-zero]", "division-cero"),
            ("comparison of integer expressions of different signedness: 'int' and 'unsigned int' [-Wsign-compare]", "signos-distintos"),
            ("statement with no effect [-Wunused-value]", "sin-efecto"),
            ("this 'if' clause does not guard... [-Wmisleading-indentation]", "indentacion"),
            ("...this statement, but the latter is misleadingly indented as if it were guarded by the 'if'", "indentacion-ruido"),
            ("suggest braces around empty body in an 'if' statement [-Wempty-body]", "cuerpo-vacio"),
            ("excess elements in array initializer", "exceso-inicializador"),
            ("variable-sized object may not be initialized except with an empty initializer", "vla-inicializada"),
            ("'else' without a previous 'if'", "else-sin-if"),
            ("break statement not within loop or switch", "break-fuera"),
            ("return type defaults to 'int' [-Wimplicit-int]", "tipo-de-retorno"),
            ("this statement may fall through [-Wimplicit-fallthrough=]", "caida-switch"),
            ("function returns address of local variable [-Wreturn-local-addr]", "direccion-local"),
            ("'free' called on unallocated object 'x' [-Wfree-nonheap-object]", "free-no-heap"),
            ("argument to 'sizeof' in 'memset' call is the same expression as the destination; did you mean to provide an explicit length? [-Wsizeof-pointer-memaccess]", "sizeof-puntero"),
            ("expected expression before ';' token", "expresion-esperada"),
            ("expected ')' before ';' token", "falta-parentesis"),
            ("expected '}' before 'else'", "falta-algo"),
            ("enlazador: undefined reference to `f'", "referencia-indefinida"),
            ("enlazador: undefined reference to `main'", "sin-main"),
            ("enlazador: multiple definition of `comun'; /tmp/x.o:a.c:(.text+0x0): first defined here", "definicion-multiple"),
            ("enlazador: cannot find -lnoexiste: No such file or directory", "biblioteca-no-encontrada"),
        ];
        let mut fallas = vec![];
        for (m, id) in casos {
            match explicar(m) {
                Some(e) if e.id == id => {}
                otro => fallas.push(format!(
                    "{m:?}: esperaba {id}, dio {:?}",
                    otro.map(|e| e.id)
                )),
            }
        }
        assert!(fallas.is_empty(), "{}", fallas.join("\n"));
    }

    /// Los mismos errores con gcc 16 en es_AR.UTF-8 (parte de los mensajes quedan en inglés).
    #[test]
    fn mensajes_reales_en_espanol() {
        let casos = [
            ("se pasa el argumento 1 de ‘f’ desde un tipo de puntero incompatible [-Wincompatible-pointer-types]", "puntero-incompatible"),
            ("el paso del argumento 1 de ‘strlen’ crea un puntero desde un entero sin una conversión [-Wint-conversion]", "puntero-desde-entero"),
            ("asignación a expresión con tipo matriz", "asignar-arreglo"),
            ("se define la variable ‘s’ pero no se usa [-Wunused-but-set-variable=]", "asignada-sin-usar"),
            ("se sugieren paréntesis alrededor de la asignación usada como valor verdadero [-Wparentheses]", "asignacion-en-condicion"),
            ("la declaración break no está dentro de un bucle o switch", "break-fuera"),
            ("falta el carácter terminando \"", "comilla-sin-cerrar"),
            ("la comparación con una literal de cadena resulta en una conducta no especificada [-Waddress]", "comparar-cadenas"),
            ("asignación de la variable de sólo lectura ‘a’", "solo-lectura"),
            ("división por cero [-Wdiv-by-zero]", "division-cero"),
            ("‘else’ sin un ‘if’ previo", "else-sin-if"),
            ("exceso de elementos en el inicializador de matriz", "exceso-inicializador"),
            ("formato ‘%d’ expects argument of type ‘int’, but argument 2 has type ‘double’ [-Wformat=]", "formato-tipo"),
            ("formato ‘%d’ expects argument of type ‘int *’, but argument 2 has type ‘int’ [-Wformat=]", "formato-espera-puntero"),
            ("formato ‘%s’ expects argument of type ‘char *’, but argument 2 has type ‘char (*)[10]’ [-Wformat=]", "formato-arreglo-con-ampersand"),
            ("declaración implícita de la función ‘malloc’ [-Wimplicit-function-declaration]", "declaracion-implicita"),
            ("declaración implícita incompatible de la función interna ‘malloc’ [-Wbuiltin-declaration-mismatch]", "builtin-incompatible"),
            ("incluya ‘<stdlib.h>’ o proporcione una declaración de ‘malloc’", "nota-include"),
            ("lista.h: No existe el fichero o el directorio", "include-no-encontrado"),
            ("uso no válido del tipo indefinido ‘struct nodo’", "tipo-incompleto"),
            ("se requiere un l-valor como operando izquierdo de la asignación", "lvalue"),
            ("‘q’ es un puntero; ¿pretendía utilizar ‘->’?", "flecha"),
            ("argumento de tipo no válido de ‘->’ (se tiene ‘struct p’)", "punto"),
            ("‘x’ no se declaró aquí (primer uso en esta función)", "no-declarada"),
            ("cada identificador sin declarar se reporta sólo una vez para cada función en el que aparece", "no-declarada-ruido"),
            ("variable ‘x’ sin usar [-Wunused-variable]", "variable-sin-uso"),
            ("parámetro ‘b’ sin uso [-Wunused-parameter]", "parametro-sin-uso"),
            ("comparación entre puntero y entero", "puntero-con-entero"),
            ("la inicialización de ‘int *’ desde ‘int’ crea un puntero desde un entero sin una conversión [-Wint-conversion]", "puntero-desde-entero"),
            ("la inicialización de ‘int’ desde ‘int *’ crea un entero desde un puntero sin una conversión [-Wint-conversion]", "entero-desde-puntero"),
            ("se sugieren llaves alrededor del cuerpo vacío en una declaración ‘if’ [-Wempty-body]", "cuerpo-vacio"),
            ("redefinición de ‘a’", "redefinicion"),
            ("la función devuelve la dirección de una variable local [-Wreturn-local-addr]", "direccion-local"),
            ("‘return’ con valor, en una función que devuelve void [-Wreturn-mismatch]", "return-con-valor"),
            ("‘return’ sin valores, en una función que no devuelve void [-Wreturn-mismatch]", "return-sin-valor"),
            ("demasiados argumentos para el formato [-Wformat-extra-args]", "formato-muchos-argumentos"),
            ("declaración sin efecto [-Wunused-value]", "sin-efecto"),
            ("el control alcanza el final de una función que no es void [-Wreturn-type]", "sin-return"),
            ("el argumento de ‘sizeof’ en la llamada ‘memset’ es la misma expresión que el destino; ¿pretendía proporcionar una longitud explícita? [-Wsizeof-pointer-memaccess]", "sizeof-puntero"),
            ("‘a’ redeclarado como un tipo diferente de símbolo", "redefinicion"),
            ("‘\\342’ parásito en el programa", "caracter-extrano"),
            ("el valor del subíndice no es ni matriz ni puntero ni vector", "subindice"),
            ("esta sentencia puede fracasar [-Wimplicit-fallthrough=]", "caida-switch"),
            ("nombre de tipo ‘bool’ desconocido", "tipo-desconocido"),
            ("el tipo de devolución por defecto es ‘int’ [-Wimplicit-int]", "tipo-de-retorno"),
            ("enlazador: referencia a `f' sin definir", "referencia-indefinida"),
            ("enlazador: referencia a `main' sin definir", "sin-main"),
            ("enlazador: definiciones múltiples de `comun'; /tmp/x.o:a.c:(.text+0x0): primero se definió aquí", "definicion-multiple"),
            ("enlazador: no se puede encontrar -lnoexiste: No existe el fichero o el directorio", "biblioteca-no-encontrada"),
        ];
        let mut fallas = vec![];
        for (m, id) in casos {
            match explicar(m) {
                Some(e) if e.id == id => {}
                otro => fallas.push(format!(
                    "{m:?}: esperaba {id}, dio {:?}",
                    otro.map(|e| e.id)
                )),
            }
        }
        assert!(fallas.is_empty(), "{}", fallas.join("\n"));
        let e = explicar("declaración implícita de la función ‘printf’").unwrap();
        assert!(e.resumen.contains("«printf»") && e.resumen.contains("<stdio.h>"));
    }

    #[test]
    fn resumen_con_nombres_e_include() {
        let e = explicar("implicit declaration of function 'malloc'").unwrap();
        assert_eq!(
            e.resumen,
            "«malloc» se usa sin estar declarada: agregá «#include <stdlib.h>» al principio del archivo"
        );
        let e = explicar("implicit declaration of function 'mi_funcion'").unwrap();
        assert!(e.resumen.contains("prototipo"), "{}", e.resumen);
        let e = explicar("unknown type name 'size_t'").unwrap();
        assert!(
            e.resumen.contains("<stddef.h>")
                || e.resumen.contains("<stdlib.h>")
                || e.resumen.contains("<stdio.h>"),
            "{}",
            e.resumen
        );
        let e = explicar("‘x’ undeclared (first use in this function)").unwrap();
        assert!(e.resumen.starts_with("«x» no está declarada"));
        assert!(e.texto.contains("**Así sí:**") && e.texto.contains("```c"));
        assert!(explicar("algo que gcc no dice").is_none());
    }
}
