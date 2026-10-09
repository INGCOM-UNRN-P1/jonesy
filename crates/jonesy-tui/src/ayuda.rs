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
