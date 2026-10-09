//! Buffer de texto sobre cuerda (`ropey`): cursor, selección, deshacer y guardado atómico.
//!
//! Todas las posiciones son índices de carácter (no de byte). Cada edición devuelve un
//! [`Delta`] con el mismo contrato que el evento `TextDelta` de la telemetría.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use ropey::Rope;

/// Cambio aplicado al texto: en `offset` se borraron `borrados` caracteres y se insertó `insertado`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delta {
    pub offset: usize,
    pub insertado: String,
    pub borrados: usize,
}

#[derive(Debug, Clone)]
struct Edicion {
    offset: usize,
    borrado: String,
    insertado: String,
    cursor_antes: usize,
    cursor_despues: usize,
    momento: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lenguaje {
    C,
    Markdown,
    Yaml,
    Toml,
    /// Makefile, makefile, GNUmakefile y *.mk.
    Makefile,
    Texto,
}

impl Lenguaje {
    pub fn de_ruta(ruta: &Path) -> Lenguaje {
        let nombre = ruta
            .file_name()
            .map(|n| n.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if matches!(nombre.as_str(), "makefile" | "gnumakefile") || nombre.starts_with("makefile.")
        {
            return Lenguaje::Makefile;
        }
        match ruta
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref()
        {
            Some("c" | "h") => Lenguaje::C,
            Some("md" | "markdown") => Lenguaje::Markdown,
            Some("yaml" | "yml") => Lenguaje::Yaml,
            Some("toml") => Lenguaje::Toml,
            Some("mk") => Lenguaje::Makefile,
            _ => Lenguaje::Texto,
        }
    }
}

/// Un buffer abierto en el editor.
#[derive(Debug, Clone)]
pub struct Buffer {
    pub ruta: Option<PathBuf>,
    pub texto: Rope,
    /// Posición del cursor (índice de carácter).
    pub cursor: usize,
    /// Extremo fijo de la selección, si la hay.
    pub ancla: Option<usize>,
    /// Columna deseada al moverse verticalmente.
    pub columna_deseada: Option<usize>,
    pub solo_lectura: bool,
    pub lenguaje: Lenguaje,
    /// Primera línea visible (la ajusta la vista).
    pub desplazamiento: usize,
    pub desplazamiento_h: usize,
    deshacer: Vec<Edicion>,
    rehacer: Vec<Edicion>,
    version_guardada: u64,
    pub version: u64,
}

impl Default for Buffer {
    fn default() -> Self {
        Buffer::desde_texto("")
    }
}

impl Buffer {
    pub fn desde_texto(texto: &str) -> Buffer {
        Buffer {
            ruta: None,
            texto: Rope::from_str(texto),
            cursor: 0,
            ancla: None,
            columna_deseada: None,
            solo_lectura: false,
            lenguaje: Lenguaje::Texto,
            desplazamiento: 0,
            desplazamiento_h: 0,
            deshacer: Vec::new(),
            rehacer: Vec::new(),
            version_guardada: 0,
            version: 0,
        }
    }

    /// Abre un archivo; si no existe, empieza vacío con esa ruta.
    pub fn abrir(ruta: impl Into<PathBuf>) -> std::io::Result<Buffer> {
        let ruta = ruta.into();
        let contenido = match fs::read(&ruta) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).replace("\r\n", "\n"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e),
        };
        let mut b = Buffer::desde_texto(&contenido);
        b.lenguaje = Lenguaje::de_ruta(&ruta);
        b.ruta = Some(ruta);
        Ok(b)
    }

    /// Identificador estable para la telemetría: la ruta relativa al proyecto o el nombre.
    pub fn id(&self, raiz: &Path) -> String {
        match &self.ruta {
            Some(r) => r
                .strip_prefix(raiz)
                .unwrap_or(r)
                .to_string_lossy()
                .into_owned(),
            None => "[sin nombre]".into(),
        }
    }

    pub fn nombre(&self) -> String {
        self.ruta
            .as_ref()
            .and_then(|r| r.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "[sin nombre]".into())
    }

    pub fn modificado(&self) -> bool {
        self.version != self.version_guardada
    }

    pub fn largo(&self) -> usize {
        self.texto.len_chars()
    }

    pub fn lineas(&self) -> usize {
        self.texto.len_lines()
    }

    /// (línea, columna) del cursor, 0-based.
    pub fn linea_columna(&self) -> (usize, usize) {
        self.pos_a_linea_columna(self.cursor)
    }

    pub fn pos_a_linea_columna(&self, pos: usize) -> (usize, usize) {
        let pos = pos.min(self.largo());
        let linea = self.texto.char_to_line(pos);
        (linea, pos - self.texto.line_to_char(linea))
    }

    /// Largo de la línea sin el salto final.
    pub fn largo_linea(&self, linea: usize) -> usize {
        if linea >= self.lineas() {
            return 0;
        }
        let l = self.texto.line(linea);
        let n = l.len_chars();
        if n > 0 && l.char(n - 1) == '\n' {
            n - 1
        } else {
            n
        }
    }

    pub fn linea_texto(&self, linea: usize) -> String {
        if linea >= self.lineas() {
            return String::new();
        }
        let s = self.texto.line(linea).to_string();
        s.strip_suffix('\n').map(str::to_string).unwrap_or(s)
    }

    pub fn linea_columna_a_pos(&self, linea: usize, columna: usize) -> usize {
        let linea = linea.min(self.lineas().saturating_sub(1));
        self.texto.line_to_char(linea) + columna.min(self.largo_linea(linea))
    }

    // ------------------------------------------------------------------ selección

    pub fn seleccion(&self) -> Option<(usize, usize)> {
        let a = self.ancla?;
        if a == self.cursor {
            return None;
        }
        Some((a.min(self.cursor), a.max(self.cursor)))
    }

    pub fn texto_seleccionado(&self) -> Option<String> {
        let (a, b) = self.seleccion()?;
        Some(self.texto.slice(a..b).to_string())
    }

    pub fn seleccionar_todo(&mut self) {
        self.ancla = Some(0);
        self.cursor = self.largo();
    }

    /// Selecciona la palabra (identificador) que contiene `pos` (doble clic).
    pub fn seleccionar_palabra(&mut self, pos: usize) {
        let (ini, fin) = self.limites_palabra(pos);
        self.ancla = Some(ini);
        self.cursor = fin;
    }

    pub fn limites_palabra(&self, pos: usize) -> (usize, usize) {
        let es = |c: char| c.is_alphanumeric() || c == '_';
        let n = self.largo();
        let pos = pos.min(n);
        let mut ini = pos;
        while ini > 0 && es(self.texto.char(ini - 1)) {
            ini -= 1;
        }
        let mut fin = pos;
        while fin < n && es(self.texto.char(fin)) {
            fin += 1;
        }
        (ini, fin)
    }

    pub fn palabra_en(&self, pos: usize) -> Option<String> {
        let (a, b) = self.limites_palabra(pos);
        (a < b).then(|| self.texto.slice(a..b).to_string())
    }

    // ------------------------------------------------------------------ movimiento

    /// Mueve el cursor a `pos`; con `extender` mantiene/crea la selección.
    pub fn mover_a(&mut self, pos: usize, extender: bool) {
        if extender {
            if self.ancla.is_none() {
                self.ancla = Some(self.cursor);
            }
        } else {
            self.ancla = None;
        }
        self.cursor = pos.min(self.largo());
    }

    pub fn izquierda(&mut self, extender: bool) {
        if !extender {
            if let Some((a, _)) = self.seleccion() {
                self.mover_a(a, false);
                return;
            }
        }
        self.columna_deseada = None;
        self.mover_a(self.cursor.saturating_sub(1), extender);
    }

    pub fn derecha(&mut self, extender: bool) {
        if !extender {
            if let Some((_, b)) = self.seleccion() {
                self.mover_a(b, false);
                return;
            }
        }
        self.columna_deseada = None;
        self.mover_a(self.cursor + 1, extender);
    }

    pub fn vertical(&mut self, delta: isize, extender: bool) {
        let (linea, col) = self.linea_columna();
        let deseada = *self.columna_deseada.get_or_insert(col);
        let max = self.lineas().saturating_sub(1) as isize;
        let nueva = (linea as isize + delta).clamp(0, max) as usize;
        let pos = self.linea_columna_a_pos(nueva, deseada);
        self.mover_a(pos, extender);
        self.columna_deseada = Some(deseada);
    }

    pub fn inicio_linea(&mut self, extender: bool) {
        let (linea, col) = self.linea_columna();
        let texto = self.linea_texto(linea);
        let sangria = texto
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .count();
        // Inicio «inteligente»: primero la sangría, después la columna 0.
        let destino = if col == sangria { 0 } else { sangria };
        self.columna_deseada = None;
        self.mover_a(self.texto.line_to_char(linea) + destino, extender);
    }

    pub fn fin_linea(&mut self, extender: bool) {
        let (linea, _) = self.linea_columna();
        self.columna_deseada = None;
        self.mover_a(
            self.texto.line_to_char(linea) + self.largo_linea(linea),
            extender,
        );
    }

    pub fn palabra_izquierda(&mut self, extender: bool) {
        let mut pos = self.cursor;
        while pos > 0 && !es_palabra(self.texto.char(pos - 1)) {
            pos -= 1;
        }
        while pos > 0 && es_palabra(self.texto.char(pos - 1)) {
            pos -= 1;
        }
        self.columna_deseada = None;
        self.mover_a(pos, extender);
    }

    pub fn palabra_derecha(&mut self, extender: bool) {
        let n = self.largo();
        let mut pos = self.cursor;
        while pos < n && !es_palabra(self.texto.char(pos)) {
            pos += 1;
        }
        while pos < n && es_palabra(self.texto.char(pos)) {
            pos += 1;
        }
        self.columna_deseada = None;
        self.mover_a(pos, extender);
    }

    // ------------------------------------------------------------------ edición

    fn aplicar(
        &mut self,
        offset: usize,
        borrar: usize,
        insertar: &str,
        cursor_despues: usize,
        agrupable: bool,
    ) -> Option<Delta> {
        if self.solo_lectura {
            return None;
        }
        let borrado = self.texto.slice(offset..offset + borrar).to_string();
        self.texto.remove(offset..offset + borrar);
        self.texto.insert(offset, insertar);
        let ahora = Instant::now();
        let cursor_antes = self.cursor;
        // Agrupa tipeo consecutivo (caracteres sueltos, < 1 s) en una sola unidad de deshacer.
        let agrupado = agrupable
            && borrar == 0
            && insertar.chars().count() == 1
            && !insertar.contains('\n')
            && self.deshacer.last().is_some_and(|ult| {
                ult.borrado.is_empty()
                    && ult.offset + ult.insertado.chars().count() == offset
                    && !ult.insertado.ends_with(' ')
                    && ahora.duration_since(ult.momento).as_millis() < 1000
            });
        if agrupado {
            let ult = self.deshacer.last_mut().unwrap();
            ult.insertado.push_str(insertar);
            ult.cursor_despues = cursor_despues;
            ult.momento = ahora;
        } else {
            self.deshacer.push(Edicion {
                offset,
                borrado,
                insertado: insertar.to_string(),
                cursor_antes,
                cursor_despues,
                momento: ahora,
            });
        }
        self.rehacer.clear();
        self.cursor = cursor_despues;
        self.ancla = None;
        self.columna_deseada = None;
        self.version += 1;
        Some(Delta {
            offset,
            insertado: insertar.to_string(),
            borrados: borrar,
        })
    }

    /// Inserta en el cursor (reemplazando la selección).
    pub fn insertar(&mut self, texto: &str) -> Option<Delta> {
        let (ini, fin) = self.seleccion().unwrap_or((self.cursor, self.cursor));
        let destino = ini + texto.chars().count();
        self.aplicar(ini, fin - ini, texto, destino, fin == ini)
    }

    /// Escribe un carácter tipeado en el editor (no los pegados): en C cierra solos los
    /// paréntesis, corchetes, llaves y comillas, saltea el cierre si ya está, envuelve la
    /// selección y desindenta la línea al escribir `}` al principio.
    pub fn escribir(&mut self, c: char) -> Option<Delta> {
        if self.lenguaje != Lenguaje::C {
            return self.insertar(&c.to_string());
        }
        let cierre = match c {
            '(' => Some(')'),
            '[' => Some(']'),
            '{' => Some('}'),
            '"' | '\'' => Some(c),
            _ => None,
        };
        let siguiente = self.caracter(self.cursor);
        let anterior = self.cursor.checked_sub(1).and_then(|p| self.caracter(p));
        // Con selección: envolverla.
        if let (Some(cierre), Some((a, b))) = (cierre, self.seleccion()) {
            let dentro: String = self.texto.slice(a..b).to_string();
            let nuevo = format!("{c}{dentro}{cierre}");
            let d = self.aplicar(a, b - a, &nuevo, b + 1, false);
            self.ancla = Some(a + 1);
            return d;
        }
        // El cierre ya está justo después (lo puso la apertura): se saltea.
        if matches!(c, ')' | ']' | '}' | '"' | '\'')
            && siguiente == Some(c)
            && self.seleccion().is_none()
        {
            let es_comilla = c == '"' || c == '\'';
            if !es_comilla || anterior != Some('\\') {
                self.mover_a(self.cursor + 1, false);
                return None;
            }
        }
        // `}` al principio de la línea: un nivel menos de sangría.
        if c == '}' && self.seleccion().is_none() {
            let (linea, col) = self.linea_columna();
            let texto_linea = self.linea_texto(linea);
            // En una línea vacía justo antes del `}` que se cerró solo: se usa ese.
            if texto_linea.trim().is_empty() && linea + 1 < self.lineas() {
                let sig = self.linea_texto(linea + 1);
                if sig.trim_start().starts_with('}') {
                    let ini = self.texto.line_to_char(linea);
                    let largo = texto_linea.chars().count() + 1;
                    let llave = sig.chars().take_while(|x| x.is_whitespace()).count();
                    return self.aplicar(ini, largo, "", ini + llave + 1, false);
                }
            }
            let antes: String = texto_linea.chars().take(col).collect();
            if !antes.is_empty() && antes.chars().all(|x| x == ' ') {
                let quitar = antes.len().min(4);
                let pos = self.cursor - quitar;
                return self.aplicar(pos, quitar, "}", pos + 1, false);
            }
        }
        if let Some(cierre) = cierre {
            let libre = siguiente.is_none_or(|x| x.is_whitespace() || ")]};,".contains(x));
            let es_comilla = c == '"' || c == '\'';
            let palabra_antes =
                anterior.is_some_and(|x| x.is_alphanumeric() || x == '_' || x == '\\');
            if libre && !(es_comilla && palabra_antes) {
                let par = format!("{c}{cierre}");
                return self.aplicar(self.cursor, 0, &par, self.cursor + 1, false);
            }
        }
        self.insertar(&c.to_string())
    }

    fn caracter(&self, pos: usize) -> Option<char> {
        (pos < self.largo()).then(|| self.texto.char(pos))
    }

    /// Enter con sangría automática (copia la de la línea y suma un nivel tras `{`).
    pub fn nueva_linea(&mut self) -> Option<Delta> {
        let (linea, col) = self.linea_columna();
        let texto = self.linea_texto(linea);
        let sangria: String = texto
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();
        let antes: String = texto.chars().take(col).collect();
        let mut extra = String::new();
        if self.lenguaje == Lenguaje::C && antes.trim_end().ends_with('{') {
            extra.push_str("    ");
            // Entre `{` y `}`: el cierre baja a su propia línea.
            if self.seleccion().is_none() && self.caracter(self.cursor) == Some('}') {
                let t = format!("\n{sangria}    \n{sangria}");
                let destino = self.cursor + 1 + sangria.chars().count() + 4;
                return self.aplicar(self.cursor, 0, &t, destino, false);
            }
        }
        // Makefile: después de «objetivo: prerrequisitos» viene la receta, que exige TAB.
        if self.lenguaje == Lenguaje::Makefile && sangria.is_empty() && es_regla_make(&antes) {
            extra.push('\t');
        }
        self.insertar(&format!("\n{sangria}{extra}"))
    }

    pub fn borrar_atras(&mut self) -> Option<Delta> {
        if let Some((a, b)) = self.seleccion() {
            return self.aplicar(a, b - a, "", a, false);
        }
        if self.cursor == 0 {
            return None;
        }
        // Entre un par vacío que se cerró solo («(|)»): se borran los dos.
        if self.lenguaje == Lenguaje::C {
            let par = (self.caracter(self.cursor - 1), self.caracter(self.cursor));
            if matches!(
                par,
                (Some('('), Some(')'))
                    | (Some('['), Some(']'))
                    | (Some('{'), Some('}'))
                    | (Some('"'), Some('"'))
                    | (Some('\''), Some('\''))
            ) {
                let pos = self.cursor - 1;
                return self.aplicar(pos, 2, "", pos, false);
            }
        }
        // Borra una sangría completa de 4 espacios de una vez.
        let (linea, col) = self.linea_columna();
        let antes: String = self.linea_texto(linea).chars().take(col).collect();
        let n = if !antes.is_empty() && antes.chars().all(|c| c == ' ') && col % 4 == 0 {
            4
        } else {
            1
        };
        let n = n.min(self.cursor);
        let pos = self.cursor - n;
        self.aplicar(pos, n, "", pos, false)
    }

    pub fn borrar_adelante(&mut self) -> Option<Delta> {
        if let Some((a, b)) = self.seleccion() {
            return self.aplicar(a, b - a, "", a, false);
        }
        if self.cursor >= self.largo() {
            return None;
        }
        self.aplicar(self.cursor, 1, "", self.cursor, false)
    }

    pub fn cortar(&mut self) -> Option<(String, Delta)> {
        let texto = self.texto_seleccionado()?;
        let delta = self.borrar_atras()?;
        Some((texto, delta))
    }

    /// Reemplaza todo el contenido (formateadores externos, recargas).
    pub fn reemplazar_todo(&mut self, texto: &str) -> Option<Delta> {
        let n = self.largo();
        let cursor = self.cursor.min(texto.chars().count());
        self.aplicar(0, n, texto, cursor, false)
    }

    pub fn deshacer(&mut self) -> Option<Delta> {
        let e = self.deshacer.pop()?;
        let n = e.insertado.chars().count();
        self.texto.remove(e.offset..e.offset + n);
        self.texto.insert(e.offset, &e.borrado);
        self.cursor = e.cursor_antes.min(self.largo());
        self.ancla = None;
        self.version += 1;
        let delta = Delta {
            offset: e.offset,
            insertado: e.borrado.clone(),
            borrados: n,
        };
        self.rehacer.push(e);
        Some(delta)
    }

    pub fn rehacer(&mut self) -> Option<Delta> {
        let e = self.rehacer.pop()?;
        let n = e.borrado.chars().count();
        self.texto.remove(e.offset..e.offset + n);
        self.texto.insert(e.offset, &e.insertado);
        self.cursor = e.cursor_despues.min(self.largo());
        self.ancla = None;
        self.version += 1;
        let delta = Delta {
            offset: e.offset,
            insertado: e.insertado.clone(),
            borrados: n,
        };
        self.deshacer.push(e);
        Some(delta)
    }

    /// Las líneas que abarca la selección (o la del cursor): (primera, última).
    pub fn lineas_elegidas(&self) -> (usize, usize) {
        match self.seleccion() {
            Some((a, b)) => {
                let la = self.texto.char_to_line(a);
                // Una selección que termina al principio de una línea no la incluye.
                let mut lb = self.texto.char_to_line(b);
                if lb > la && self.texto.line_to_char(lb) == b {
                    lb -= 1;
                }
                (la, lb)
            }
            None => {
                let l = self.linea_columna().0;
                (l, l)
            }
        }
    }

    /// Reemplaza las líneas `la..=lb` (sin el salto final de la última) por `nuevo`, en
    /// una sola unidad de deshacer, y deja elegidas las líneas resultantes.
    fn reemplazar_lineas(
        &mut self,
        la: usize,
        lb: usize,
        nuevo: &str,
        cursor: usize,
    ) -> Option<Delta> {
        let ini = self.texto.line_to_char(la);
        let fin = ini + (la..=lb).map(|l| self.largo_linea(l) + 1).sum::<usize>() - 1;
        let fin = fin.min(self.largo());
        self.aplicar(ini, fin - ini, nuevo, cursor, false)
    }

    /// Prefijo de comentario de línea del lenguaje.
    fn prefijo_comentario(&self) -> Option<&'static str> {
        match self.lenguaje {
            Lenguaje::C => Some("//"),
            Lenguaje::Makefile | Lenguaje::Toml | Lenguaje::Yaml => Some("#"),
            _ => None,
        }
    }

    /// Ctrl+/: comenta las líneas elegidas o, si ya lo están todas, las descomenta.
    pub fn alternar_comentario(&mut self) -> Option<Delta> {
        let pre = self.prefijo_comentario()?;
        let (la, lb) = self.lineas_elegidas();
        let lineas: Vec<String> = (la..=lb).map(|l| self.linea_texto(l)).collect();
        let con_texto = lineas.iter().filter(|l| !l.trim().is_empty());
        let todas = con_texto.clone().all(|l| l.trim_start().starts_with(pre));
        let sangria = con_texto
            .map(|l| l.chars().take_while(|c| *c == ' ' || *c == '\t').count())
            .min()
            .unwrap_or(0);
        let nuevas: Vec<String> = lineas
            .iter()
            .map(|l| {
                if l.trim().is_empty() {
                    return l.clone();
                }
                if todas {
                    let i = l.find(pre).unwrap();
                    let resto = &l[i + pre.len()..];
                    let resto = resto.strip_prefix(' ').unwrap_or(resto);
                    format!("{}{resto}", &l[..i])
                } else {
                    let (a, b): (String, String) = (
                        l.chars().take(sangria).collect(),
                        l.chars().skip(sangria).collect(),
                    );
                    format!("{a}{pre} {b}")
                }
            })
            .collect();
        let tenia_seleccion = self.seleccion().is_some();
        let (_, col) = self.linea_columna();
        let fila_cursor = self.linea_columna().0;
        let nuevo = nuevas.join("\n");
        let ini = self.texto.line_to_char(la);
        let d = self.reemplazar_lineas(la, lb, &nuevo, ini)?;
        if tenia_seleccion {
            self.ancla = Some(ini);
            self.cursor = ini + nuevo.chars().count();
        } else {
            let largo = self.largo_linea(fila_cursor);
            let delta = nuevas[fila_cursor - la].chars().count() as isize
                - lineas[fila_cursor - la].chars().count() as isize;
            let col = (col as isize + delta).clamp(0, largo as isize) as usize;
            self.cursor = self.linea_columna_a_pos(fila_cursor, col);
        }
        Some(d)
    }

    /// Alt+↑/↓: mueve las líneas elegidas una posición.
    pub fn mover_lineas(&mut self, abajo: bool) -> Option<Delta> {
        let (la, lb) = self.lineas_elegidas();
        let ultima = self.lineas().saturating_sub(1);
        if (!abajo && la == 0) || (abajo && lb >= ultima) {
            return None;
        }
        let (_, col) = self.linea_columna();
        let fila = self.linea_columna().0;
        let seleccion = self.seleccion().is_some();
        let bloque: Vec<String> = (la..=lb).map(|l| self.linea_texto(l)).collect();
        let (desde, hasta, nuevo, nueva_la) = if abajo {
            let otra = self.linea_texto(lb + 1);
            let mut v = vec![otra];
            v.extend(bloque.iter().cloned());
            (la, lb + 1, v.join("\n"), la + 1)
        } else {
            let otra = self.linea_texto(la - 1);
            let mut v = bloque.clone();
            v.push(otra);
            (la - 1, lb, v.join("\n"), la - 1)
        };
        let d = self.reemplazar_lineas(desde, hasta, &nuevo, 0)?;
        let nueva_lb = nueva_la + (lb - la);
        if seleccion {
            self.ancla = Some(self.texto.line_to_char(nueva_la));
            self.cursor = self.linea_columna_a_pos(nueva_lb, self.largo_linea(nueva_lb));
        } else {
            let f = nueva_la + (fila - la);
            self.cursor = self.linea_columna_a_pos(f, col.min(self.largo_linea(f)));
        }
        Some(d)
    }

    /// Ctrl+D: duplica las líneas elegidas debajo.
    pub fn duplicar_lineas(&mut self) -> Option<Delta> {
        let (la, lb) = self.lineas_elegidas();
        let (fila, col) = self.linea_columna();
        let bloque: Vec<String> = (la..=lb).map(|l| self.linea_texto(l)).collect();
        let texto = bloque.join("\n");
        let nuevo = format!("{texto}\n{texto}");
        let d = self.reemplazar_lineas(la, lb, &nuevo, 0)?;
        let f = fila + (lb - la + 1);
        self.cursor = self.linea_columna_a_pos(f, col);
        Some(d)
    }

    /// Posición de la llave, paréntesis o corchete que hace pareja con el que está en el
    /// cursor (o justo antes). Saltea cadenas, caracteres y comentarios de C.
    pub fn pareja(&self) -> Option<(usize, usize)> {
        const LIMITE: usize = 20_000;
        let es = |c: char| "()[]{}".contains(c);
        let pos = [Some(self.cursor), self.cursor.checked_sub(1)]
            .into_iter()
            .flatten()
            .find(|p| self.caracter(*p).is_some_and(es))?;
        let c = self.caracter(pos)?;
        let (abre, cierra, adelante) = match c {
            '(' => ('(', ')', true),
            '[' => ('[', ']', true),
            '{' => ('{', '}', true),
            ')' => ('(', ')', false),
            ']' => ('[', ']', false),
            _ => ('{', '}', false),
        };
        // Zonas a saltear (cadenas y comentarios), calculadas sobre una ventana.
        let ini = if adelante {
            pos
        } else {
            pos.saturating_sub(LIMITE)
        };
        let fin = if adelante {
            (pos + LIMITE).min(self.largo())
        } else {
            pos + 1
        };
        let base = if adelante { ini.saturating_sub(0) } else { ini };
        let texto: Vec<char> = self.texto.slice(base..fin).chars().collect();
        let mut codigo = vec![true; texto.len()];
        let mut i = 0;
        while i < texto.len() {
            match texto[i] {
                '"' | '\'' => {
                    let q = texto[i];
                    let mut j = i + 1;
                    while j < texto.len() && texto[j] != q && texto[j] != '\n' {
                        if texto[j] == '\\' {
                            j += 1;
                        }
                        j += 1;
                    }
                    codigo[i..=j.min(texto.len() - 1)].fill(false);
                    i = j + 1;
                }
                '/' if texto.get(i + 1) == Some(&'/') => {
                    let mut j = i;
                    while j < texto.len() && texto[j] != '\n' {
                        codigo[j] = false;
                        j += 1;
                    }
                    i = j;
                }
                '/' if texto.get(i + 1) == Some(&'*') => {
                    let mut j = i;
                    while j < texto.len() && !(texto[j] == '*' && texto.get(j + 1) == Some(&'/')) {
                        codigo[j] = false;
                        j += 1;
                    }
                    codigo[j.min(texto.len())..(j + 2).min(texto.len())].fill(false);
                    i = j + 2;
                }
                _ => i += 1,
            }
        }
        let rel = pos - base;
        if !codigo[rel] {
            return None;
        }
        let mut nivel = 0i32;
        let rango: Box<dyn Iterator<Item = usize>> = if adelante {
            Box::new(rel..texto.len())
        } else {
            Box::new((0..=rel).rev())
        };
        for k in rango {
            if !codigo[k] {
                continue;
            }
            if texto[k] == abre {
                nivel += if adelante { 1 } else { -1 };
            } else if texto[k] == cierra {
                nivel += if adelante { -1 } else { 1 };
            }
            if nivel == 0 {
                return Some((pos, base + k));
            }
        }
        None
    }

    /// Saca los espacios y tabs del final de cada línea (al guardar, si se pide). En
    /// Markdown no: dos espacios al final son un salto de línea.
    pub fn limpiar_espacios_finales(&mut self) -> Option<Delta> {
        if self.lenguaje == Lenguaje::Markdown {
            return None;
        }
        let texto = self.contenido();
        let limpio: String = texto
            .split('\n')
            .map(|l| l.trim_end_matches([' ', '\t']))
            .collect::<Vec<_>>()
            .join("\n");
        if limpio == texto {
            return None;
        }
        let (fila, col) = self.linea_columna();
        let d = self.aplicar(0, self.largo(), &limpio, 0, false);
        let f = fila.min(self.lineas().saturating_sub(1));
        self.cursor = self.linea_columna_a_pos(f, col.min(self.largo_linea(f)));
        d
    }

    // ------------------------------------------------------------------ búsqueda

    /// Próxima aparición de `patron` desde el cursor (circular, sin distinguir mayúsculas).
    pub fn buscar(&self, patron: &str, desde: usize) -> Option<usize> {
        if patron.is_empty() {
            return None;
        }
        let texto = self.texto.to_string().to_lowercase();
        let patron = patron.to_lowercase();
        let byte_desde = self.texto.char_to_byte(desde.min(self.largo()));
        let pos_byte = texto[byte_desde..]
            .find(&patron)
            .map(|p| p + byte_desde)
            .or_else(|| texto.find(&patron))?;
        Some(self.texto.byte_to_char(pos_byte))
    }

    // ------------------------------------------------------------------ disco

    /// Guarda de forma atómica: archivo temporal en el mismo directorio, fsync y rename.
    pub fn guardar(&mut self) -> std::io::Result<usize> {
        let ruta = self.ruta.clone().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "el buffer no tiene ruta")
        })?;
        let bytes = guardar_atomico(&ruta, self.texto.to_string().as_bytes())?;
        self.version_guardada = self.version;
        Ok(bytes)
    }

    pub fn marcar_guardado(&mut self) {
        self.version_guardada = self.version;
    }

    pub fn contenido(&self) -> String {
        self.texto.to_string()
    }
}

fn es_palabra(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Línea de regla de un Makefile (`objetivo: prerrequisitos`), no una asignación.
fn es_regla_make(linea: &str) -> bool {
    let l = linea.trim_end();
    if l.is_empty() || l.starts_with('#') || l.starts_with(char::is_whitespace) {
        return false;
    }
    match (l.find(':'), l.find('=')) {
        (Some(d), Some(e)) => d < e && !l[d..].starts_with(":=") && !l[d..].starts_with("::="),
        (Some(_), None) => true,
        _ => false,
    }
}

/// Escritura atómica: `.<nombre>.jonesy-tmp` + fsync + rename (+ fsync del directorio).
pub fn guardar_atomico(ruta: &Path, datos: &[u8]) -> std::io::Result<usize> {
    let dir = ruta
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let nombre = ruta
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{nombre}.jonesy-tmp"));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(datos)?;
        f.sync_all()?;
    }
    if let Ok(meta) = fs::metadata(ruta) {
        let _ = fs::set_permissions(&tmp, meta.permissions());
    }
    fs::rename(&tmp, ruta)?;
    if let Ok(d) = fs::File::open(dir) {
        let _ = d.sync_all();
    }
    Ok(datos.len())
}

#[cfg(test)]
mod tests {

    #[test]
    fn makefile_enter_tras_regla_agrega_tab() {
        let mut b = Buffer::desde_texto("programa: main.o");
        b.lenguaje = Lenguaje::Makefile;
        b.mover_a(b.largo(), false);
        b.nueva_linea();
        assert_eq!(b.contenido(), "programa: main.o\n\t");
        b.insertar("gcc -o $@ $^");
        b.nueva_linea();
        assert_eq!(
            b.contenido(),
            "programa: main.o\n\tgcc -o $@ $^\n\t",
            "la receta sigue con TAB"
        );
        let mut v = Buffer::desde_texto("CC := gcc");
        v.lenguaje = Lenguaje::Makefile;
        v.mover_a(v.largo(), false);
        v.nueva_linea();
        assert_eq!(
            v.contenido(),
            "CC := gcc\n",
            "una asignación no abre receta"
        );
        assert_eq!(
            Lenguaje::de_ruta(Path::new("libs/a/Makefile")),
            Lenguaje::Makefile
        );
        assert_eq!(
            Lenguaje::de_ruta(Path::new("reglas.mk")),
            Lenguaje::Makefile
        );
    }

    use super::*;

    #[test]
    fn insertar_borrar_y_deshacer() {
        let mut b = Buffer::desde_texto("int x;\n");
        b.mover_a(4, false);
        let d = b.insertar("y").unwrap();
        assert_eq!(
            d,
            Delta {
                offset: 4,
                insertado: "y".into(),
                borrados: 0
            }
        );
        assert_eq!(b.contenido(), "int yx;\n");
        b.borrar_adelante();
        assert_eq!(b.contenido(), "int y;\n");
        b.deshacer();
        b.deshacer();
        assert_eq!(b.contenido(), "int x;\n");
        b.rehacer();
        assert_eq!(b.contenido(), "int yx;\n");
    }

    #[test]
    fn el_tipeo_seguido_se_deshace_junto() {
        let mut b = Buffer::desde_texto("");
        for c in "hola".chars() {
            b.insertar(&c.to_string());
        }
        b.deshacer();
        assert_eq!(b.contenido(), "");
    }

    #[test]
    fn nueva_linea_con_sangria() {
        let mut b = Buffer::desde_texto("int main(void) {");
        b.lenguaje = Lenguaje::C;
        b.mover_a(b.largo(), false);
        b.nueva_linea();
        assert_eq!(b.contenido(), "int main(void) {\n    ");
        b.insertar("x;");
        b.nueva_linea();
        assert_eq!(b.contenido(), "int main(void) {\n    x;\n    ");
        b.borrar_atras();
        assert_eq!(b.contenido(), "int main(void) {\n    x;\n");
    }

    fn c(texto: &str) -> Buffer {
        let mut b = Buffer::desde_texto(texto);
        b.lenguaje = Lenguaje::C;
        b.mover_a(b.largo(), false);
        b
    }

    fn tipear(b: &mut Buffer, t: &str) {
        for ch in t.chars() {
            if ch == '\n' {
                b.nueva_linea();
            } else {
                b.escribir(ch);
            }
        }
    }

    #[test]
    fn cierra_pares_y_saltea_el_cierre() {
        let mut b = c("");
        tipear(&mut b, "f(a[1]");
        assert_eq!(b.contenido(), "f(a[1])");
        tipear(&mut b, ")");
        assert_eq!(b.contenido(), "f(a[1])", "el ) ya estaba: se saltea");
        tipear(&mut b, ";\nputs(\"hola");
        assert_eq!(b.contenido(), "f(a[1]);\nputs(\"hola\")");
        tipear(&mut b, "\");");
        assert_eq!(b.contenido(), "f(a[1]);\nputs(\"hola\");");
        // Antes de una palabra no se cierra; tras una letra, la comilla no se duplica.
        let mut b = c("x");
        b.mover_a(0, false);
        tipear(&mut b, "(");
        assert_eq!(b.contenido(), "(x");
        let mut b = c("don");
        tipear(&mut b, "'");
        assert_eq!(b.contenido(), "don'");
        // Backspace en un par vacío borra los dos.
        let mut b = c("");
        tipear(&mut b, "(");
        b.borrar_atras();
        assert_eq!(b.contenido(), "");
        // Con selección, la envuelve.
        let mut b = c("a + b");
        b.seleccionar_todo();
        b.escribir('(');
        assert_eq!(b.contenido(), "(a + b)");
        // En Markdown no.
        let mut m = Buffer::desde_texto("");
        m.escribir('(');
        assert_eq!(m.contenido(), "(");
    }

    #[test]
    fn llaves_con_sangria() {
        let mut b = c("int main(void)\n");
        tipear(&mut b, "{\nreturn 0;\n}");
        assert_eq!(b.contenido(), "int main(void)\n{\n    return 0;\n}");
        // Escribir } al principio de una línea sangrada la desindenta.
        let mut b = c("if (x) {\n    y();\n    ");
        tipear(&mut b, "}");
        assert_eq!(b.contenido(), "if (x) {\n    y();\n}");
    }

    #[test]
    fn comentar_mover_y_duplicar() {
        let mut b = c("int a;\n    int b;\n\nint c;");
        b.mover_a(0, false);
        b.mover_a(b.linea_columna_a_pos(1, 3), true);
        b.alternar_comentario();
        assert_eq!(b.contenido(), "// int a;\n//     int b;\n\nint c;");
        b.alternar_comentario();
        assert_eq!(b.contenido(), "int a;\n    int b;\n\nint c;");
        b.deshacer();
        assert_eq!(
            b.contenido(),
            "// int a;\n//     int b;\n\nint c;",
            "una sola unidad de deshacer"
        );
        let mut b = c("uno\ndos\ntres");
        b.mover_a(0, false);
        b.mover_lineas(true);
        assert_eq!(b.contenido(), "dos\nuno\ntres");
        assert_eq!(b.linea_columna().0, 1);
        b.mover_lineas(false);
        assert_eq!(b.contenido(), "uno\ndos\ntres");
        assert!(b.mover_lineas(false).is_none());
        b.duplicar_lineas();
        assert_eq!(b.contenido(), "uno\nuno\ndos\ntres");
        assert_eq!(b.linea_columna().0, 1);
        let mut m = c("PROG = a\nall:");
        m.lenguaje = Lenguaje::Makefile;
        m.seleccionar_todo();
        m.alternar_comentario();
        assert_eq!(m.contenido(), "# PROG = a\n# all:");
    }

    #[test]
    fn pareja_saltea_cadenas_y_comentarios() {
        let b = c("f(a, \")\", /* ) */ g(b));");
        let mut b = b;
        b.mover_a(1, false); // sobre el primer (
        let (a, z) = b.pareja().unwrap();
        assert_eq!((a, z), (1, 22), "saltea \")\" y /* ) */");
        b.mover_a(23, false); // justo después del ) final
        assert_eq!(b.pareja(), Some((22, 1)));
        let mut b = c("{ x }");
        b.mover_a(2, false);
        assert!(b.pareja().is_none());
    }

    #[test]
    fn limpia_espacios_finales() {
        let mut b = c("int a;   \n\tint b;\t\n");
        assert!(b.limpiar_espacios_finales().is_some());
        assert_eq!(b.contenido(), "int a;\n\tint b;\n");
        assert!(b.limpiar_espacios_finales().is_none());
    }

    #[test]
    fn movimiento_vertical_recuerda_la_columna() {
        let mut b = Buffer::desde_texto("abcdef\nab\nabcdef");
        b.mover_a(5, false);
        b.vertical(1, false);
        assert_eq!(b.linea_columna(), (1, 2));
        b.vertical(1, false);
        assert_eq!(b.linea_columna(), (2, 5));
    }

    #[test]
    fn seleccion_y_reemplazo() {
        let mut b = Buffer::desde_texto("hola mundo");
        b.seleccionar_palabra(6);
        assert_eq!(b.texto_seleccionado().as_deref(), Some("mundo"));
        b.insertar("che");
        assert_eq!(b.contenido(), "hola che");
    }

    #[test]
    fn solo_lectura_no_cambia() {
        let mut b = Buffer::desde_texto("consigna");
        b.solo_lectura = true;
        assert!(b.insertar("x").is_none());
        assert_eq!(b.contenido(), "consigna");
    }

    #[test]
    fn guardado_atomico_y_modificado() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("a.c");
        let mut b = Buffer::abrir(&ruta).unwrap();
        b.insertar("int x;\n");
        assert!(b.modificado());
        assert_eq!(b.guardar().unwrap(), 7);
        assert!(!b.modificado());
        assert_eq!(fs::read_to_string(&ruta).unwrap(), "int x;\n");
        assert!(!dir.path().join(".a.c.jonesy-tmp").exists());
    }

    #[test]
    fn buscar_es_circular() {
        let b = Buffer::desde_texto("abc ABC abc");
        assert_eq!(b.buscar("abc", 1), Some(4));
        assert_eq!(b.buscar("abc", 9), Some(0));
    }
}
