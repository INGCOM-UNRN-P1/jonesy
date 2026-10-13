//! Textos de ayuda que se arman en el momento: las tablas de consulta (con los tamaños de
//! los tipos de esta máquina) y los atajos de cada panel.

use std::os::raw::{c_char, c_double, c_float, c_int, c_long, c_longlong, c_short};

/// Ayuda → Tablas de consulta: tipos, especificadores de printf/scanf, secuencias de
/// escape, precedencia de operadores y ASCII.
pub fn tablas() -> String {
    let mut s = String::from("# Tablas de consulta\n\n## Tipos (en esta máquina)\n\n");
    s.push_str("| Tipo | Bytes | Rango | printf | scanf |\n|---|---|---|---|---|\n");
    let filas: [(&str, usize, String, &str, &str); 11] = [
        (
            "char",
            std::mem::size_of::<c_char>(),
            format!("{} a {}", c_char::MIN, c_char::MAX),
            "%c",
            " %c",
        ),
        (
            "short",
            std::mem::size_of::<c_short>(),
            format!("{} a {}", c_short::MIN, c_short::MAX),
            "%hd",
            "%hd",
        ),
        (
            "int",
            std::mem::size_of::<c_int>(),
            format!("{} a {}", c_int::MIN, c_int::MAX),
            "%d",
            "%d",
        ),
        (
            "unsigned int",
            std::mem::size_of::<c_int>(),
            format!("0 a {}", u32::MAX),
            "%u",
            "%u",
        ),
        (
            "long",
            std::mem::size_of::<c_long>(),
            format!("{} a {}", c_long::MIN, c_long::MAX),
            "%ld",
            "%ld",
        ),
        (
            "long long",
            std::mem::size_of::<c_longlong>(),
            format!("{} a {}", c_longlong::MIN, c_longlong::MAX),
            "%lld",
            "%lld",
        ),
        (
            "size_t",
            std::mem::size_of::<usize>(),
            format!("0 a {}", usize::MAX),
            "%zu",
            "%zu",
        ),
        (
            "float",
            std::mem::size_of::<c_float>(),
            "≈ 7 dígitos, ±3.4e38".into(),
            "%f",
            "%f",
        ),
        (
            "double",
            std::mem::size_of::<c_double>(),
            "≈ 15 dígitos, ±1.8e308".into(),
            "%f",
            "%lf",
        ),
        (
            "char *",
            std::mem::size_of::<*const u8>(),
            "cadena terminada en '\\0'".into(),
            "%s",
            "%s (sin &)",
        ),
        (
            "void *",
            std::mem::size_of::<*const u8>(),
            "una dirección".into(),
            "%p",
            "—",
        ),
    ];
    for (t, b, r, p, sc) in filas {
        s.push_str(&format!("| `{t}` | {b} | {r} | `{p}` | `{sc}` |\n"));
    }
    s.push_str(
        "\n`sizeof` da los bytes; en `scanf` todo lleva `&` salvo las cadenas. `%x` hexadecimal, \
`%o` octal, `%e` notación científica, `%%` un `%`. `%5d` ocupa 5 lugares, `%.2f` deja 2 decimales.\n",
    );
    s.push_str(
        "\n## Secuencias de escape\n\n| Escribís | Es |\n|---|---|\n\
| `\\n` | salto de línea |\n| `\\t` | tabulación |\n| `\\\\` | barra invertida |\n\
| `\\\"` | comilla doble |\n| `\\'` | comilla simple |\n| `\\0` | carácter nulo (fin de cadena) |\n",
    );
    s.push_str(
        "\n## Precedencia de operadores (de mayor a menor)\n\n| Nivel | Operadores | Asociatividad |\n|---|---|---|\n",
    );
    for (n, (ops, asoc)) in PRECEDENCIA.iter().enumerate() {
        s.push_str(&format!("| {} | {ops} | {asoc} |\n", n + 1));
    }
    s.push_str("\nEditar → Ver la precedencia de la expresión muestra los paréntesis implícitos de una línea.\n");
    s.push_str("\n## ASCII imprimible\n\n| Dec | Car | Dec | Car | Dec | Car | Dec | Car |\n|---|---|---|---|---|---|---|---|\n");
    let codigos: Vec<u8> = (32u8..127).collect();
    let filas = codigos.len().div_ceil(4);
    for f in 0..filas {
        let mut l = String::from("|");
        for c in 0..4 {
            match codigos.get(c * filas + f) {
                Some(&b) => {
                    let car = match b {
                        b' ' => "espacio".to_string(),
                        b'|' => "\\|".to_string(),
                        b'`' => "acento".to_string(),
                        _ => format!("`{}`", b as char),
                    };
                    l.push_str(&format!(" {b} | {car} |"));
                }
                None => l.push_str("  |  |"),
            }
        }
        s.push_str(&l);
        s.push('\n');
    }
    s.push_str("\n`'0'` es 48 y `'a'` es 97: `c - '0'` convierte un dígito en su valor.\n");
    s
}

/// Paneles con atajos propios (tecla `?` con el foco en el panel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Archivos,
    Proyecto,
    Esquema,
    Git,
    Reglas,
    Problemas,
    Salida,
    Busqueda,
    Depurador,
    Consigna,
    VistaPrevia,
    Editor,
}

/// Título y atajos de un panel, en Markdown.
pub fn atajos_panel(p: Panel) -> (&'static str, &'static str) {
    match p {
        Panel::Archivos => (
            "Atajos: archivos",
            "| Tecla | Hace |\n|---|---|\n\
| ↑ ↓ RePág AvPág Inicio Fin | moverse |\n\
| Enter · Espacio | abrir el archivo o desplegar la carpeta |\n\
| → · ← | desplegar · plegar (o ir a la carpeta de arriba) |\n\
| n · m | archivo nuevo · carpeta nueva |\n\
| F2 | renombrar o mover |\n\
| Supr | borrar (pide confirmación) |\n\
| r | releer el disco y git |\n\
| Tab · Shift+Tab | pestaña proyecto · esquema |\n\
| Esc | volver al editor |\n",
        ),
        Panel::Proyecto => (
            "Atajos: proyecto",
            "| Tecla | Hace |\n|---|---|\n\
| ↑ ↓ RePág AvPág Inicio Fin | moverse |\n\
| Enter · Espacio | desplegar el ejercicio o correr el objetivo de make (o el comando de tp.sh) |\n\
| → · ← | desplegar · plegar |\n\
| r | releer los Makefile y tp.sh |\n\
| Tab · Shift+Tab | pestaña esquema · archivos |\n\
| Esc | volver al editor |\n",
        ),
        Panel::Esquema => (
            "Atajos: esquema",
            "| Tecla | Hace |\n|---|---|\n\
| ↑ ↓ RePág AvPág Inicio Fin | moverse por las funciones, tipos y macros |\n\
| Enter | ir a la definición y volver al editor |\n\
| → · Espacio | mostrarla sin dejar el esquema |\n\
| Tab · Shift+Tab | pestaña archivos · proyecto |\n\
| Esc | volver al editor |\n",
        ),
        Panel::Git => (
            "Atajos: git",
            "| Tecla | Hace |\n|---|---|\n\
| ↑ ↓ | moverse |\n\
| Espacio | agregar o sacar el archivo del próximo commit (stage) |\n\
| a · u | agregar todo · sacar todo |\n\
| c | commit |\n\
| Enter | ver el cambio (diff) del archivo o del commit |\n\
| l | historial (log); m marca un commit para comparar |\n\
| z | deshacer el último commit (los cambios quedan) |\n\
| s · b | subir (push) · bajar (pull) |\n\
| r | refrescar |\n\
| Esc · Tab | volver al editor |\n",
        ),
        Panel::Reglas => (
            "Atajos: reglas",
            "| Tecla | Hace |\n|---|---|\n\
| ↑ ↓ | moverse por los hallazgos, agrupados por regla |\n\
| Enter | ir a la línea |\n\
| F1 | explicar la regla («así no / así sí») |\n\
| Esc · Tab | volver al editor |\n",
        ),
        Panel::Problemas => (
            "Atajos: problemas",
            "| Tecla | Hace |\n|---|---|\n\
| ↑ ↓ | moverse |\n\
| Enter | ir a la línea del problema |\n\
| F1 | explicar el error o la regla |\n\
| Alt+N · Alt+Shift+N | siguiente · anterior (desde el editor) |\n\
| Esc · Tab | volver al editor |\n",
        ),
        Panel::Salida => (
            "Atajos: salida",
            "| Tecla | Hace |\n|---|---|\n\
| ↑ ↓ RePág AvPág | desplazar la salida |\n\
| Esc · Tab | volver al editor |\n",
        ),
        Panel::Busqueda => (
            "Atajos: búsqueda",
            "| Tecla | Hace |\n|---|---|\n\
| ↑ ↓ RePág AvPág | moverse por los resultados |\n\
| Enter | abrir el archivo en esa línea |\n\
| Esc · Tab | volver al editor |\n",
        ),
        Panel::Depurador => (
            "Atajos: depurador",
            "| Tecla | Hace |\n|---|---|\n\
| F5 · Shift+F5 | continuar · detener |\n\
| F10 · F11 · Shift+F11 | paso · entrar a la función · salir de la función |\n\
| Ctrl+F10 | correr hasta el cursor |\n\
| Ctrl+B | breakpoint en la línea del editor |\n\
| Tab | alternar entre el programa y la consola de gdb |\n\
| Esc Esc | volver al editor |\n",
        ),
        Panel::Consigna => (
            "Atajos: consigna",
            "| Tecla | Hace |\n|---|---|\n\
| ↑ ↓ RePág AvPág Inicio | desplazar |\n\
| Tab · Shift+Tab | recorrer los enlaces (sin enlaces, Tab vuelve al editor) |\n\
| Enter | seguir el enlace |\n\
| F7 · F8 | mostrar la consigna · volver al código (pantallas chicas) |\n\
| Esc | volver al editor |\n",
        ),
        Panel::VistaPrevia => (
            "Atajos: vista previa",
            "| Tecla | Hace |\n|---|---|\n\
| ↑ ↓ RePág AvPág Inicio | desplazar |\n\
| Tab · Shift+Tab | recorrer los enlaces |\n\
| Enter | seguir el enlace |\n\
| Ctrl+P | cerrar la vista previa |\n\
| Esc | volver al editor |\n",
        ),
        Panel::Editor => ("Atajos de jonesy", crate::accion::AYUDA_ATAJOS),
    }
}

/// Zonas de la pantalla que recorre el recorrido guiado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zona {
    Menu,
    Editor,
    Consigna,
    QuickDoc,
    Estado,
    Inferior,
    Izquierdo,
    Ninguna,
}

/// Pasos del recorrido guiado (Ayuda → Recorrido guiado): zona, título, qué es y, si la
/// zona no está abierta, cómo abrirla.
pub const RECORRIDO: [(Zona, &str, &str, &str); 8] = [
    (
        Zona::Menu,
        "Los menús",
        "Todas las acciones del IDE. Se abren con un clic, con Alt y la letra subrayada o con F10 y la letra. Ctrl+K busca cualquier acción por su nombre.",
        "",
    ),
    (
        Zona::Editor,
        "El editor",
        "Acá escribís el código. Ctrl+S guarda; al guardar, las herramientas de la cátedra revisan el estilo y marcan lo que encuentran al final de la línea. F1 explica el error o la regla de la línea.",
        "",
    ),
    (
        Zona::Consigna,
        "La consigna",
        "El enunciado del ejercicio, al lado del código. En pantallas chicas: F7 la muestra y F8 vuelve al código.",
        "Este proyecto no tiene consigna (un archivo consigna.md o README.md).",
    ),
    (
        Zona::QuickDoc,
        "La documentación (quick-doc)",
        "Mientras escribís una llamada, muestra qué recibe la función, qué devuelve y sus trampas. F4 la muestra u oculta.",
        "Está oculta: F4 la muestra.",
    ),
    (
        Zona::Estado,
        "La barra de estado",
        "Dónde está el cursor, los avisos y los botones del ciclo: F6 compilar, Ctrl+F5 ejecutar, Ctrl+F6 probar, F5 depurar. Se usan con un clic o con su tecla.",
        "",
    ),
    (
        Zona::Inferior,
        "El panel de abajo",
        "Problemas (los errores explicados), la salida de la compilación, la terminal, la ejecución del programa y el depurador. Alt+P problemas, Alt+T terminal.",
        "Está cerrado: se abre solo al compilar o ejecutar, o con Alt+P y Alt+T.",
    ),
    (
        Zona::Izquierdo,
        "La barra izquierda",
        "Los archivos (Alt+E), las bibliotecas y ejercicios del proyecto con sus objetivos de make (Alt+Y) y el esquema de funciones del archivo (Alt+S).",
        "Está cerrada: Alt+E la abre con los archivos, Alt+Y con el proyecto, Alt+S con el esquema.",
    ),
    (
        Zona::Ninguna,
        "Listo",
        "Ayuda → Guía rápida explica el ciclo completo de un ejercicio, Ayuda → Glosario los términos de C y Ayuda → Recorrido guiado repite este paseo.",
        "",
    ),
];

/// Niveles de precedencia de C, de mayor a menor, con su asociatividad.
pub const PRECEDENCIA: [(&str, &str); 15] = [
    ("`()` `[]` `->` `.` `x++` `x--`", "→"),
    (
        "`!` `~` `-x` `+x` `*p` `&x` `++x` `--x` `sizeof` `(tipo)`",
        "←",
    ),
    ("`*` `/` `%`", "→"),
    ("`+` `-`", "→"),
    ("`<<` `>>`", "→"),
    ("`<` `<=` `>` `>=`", "→"),
    ("`==` `!=`", "→"),
    ("`&`", "→"),
    ("`^`", "→"),
    ("`|`", "→"),
    ("`&&`", "→"),
    ("`||`", "→"),
    ("`? :`", "←"),
    ("`=` `+=` `-=` `*=` `/=` `%=` …", "←"),
    ("`,`", "→"),
];

#[cfg(test)]
mod tests {
    #[test]
    fn tablas_completas() {
        let t = super::tablas();
        assert!(t.contains("| `int` | 4 |"), "{t}");
        assert!(t.contains("`%zu`") && t.contains("\\0") && t.contains("| 65 | `A` |"));
    }
}
