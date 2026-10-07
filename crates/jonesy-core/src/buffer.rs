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
    Texto,
}

impl Lenguaje {
    pub fn de_ruta(ruta: &Path) -> Lenguaje {
        match ruta.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
            Some("c" | "h") => Lenguaje::C,
            Some("md" | "markdown") => Lenguaje::Markdown,
            Some("yaml" | "yml") => Lenguaje::Yaml,
            Some("toml") => Lenguaje::Toml,
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
            Some(r) => r.strip_prefix(raiz).unwrap_or(r).to_string_lossy().into_owned(),
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
        let sangria = texto.chars().take_while(|c| *c == ' ' || *c == '\t').count();
        // Inicio «inteligente»: primero la sangría, después la columna 0.
        let destino = if col == sangria { 0 } else { sangria };
        self.columna_deseada = None;
        self.mover_a(self.texto.line_to_char(linea) + destino, extender);
    }

    pub fn fin_linea(&mut self, extender: bool) {
        let (linea, _) = self.linea_columna();
        self.columna_deseada = None;
        self.mover_a(self.texto.line_to_char(linea) + self.largo_linea(linea), extender);
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

    fn aplicar(&mut self, offset: usize, borrar: usize, insertar: &str, cursor_despues: usize, agrupable: bool) -> Option<Delta> {
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
        Some(Delta { offset, insertado: insertar.to_string(), borrados: borrar })
    }

    /// Inserta en el cursor (reemplazando la selección).
    pub fn insertar(&mut self, texto: &str) -> Option<Delta> {
        let (ini, fin) = self.seleccion().unwrap_or((self.cursor, self.cursor));
        let destino = ini + texto.chars().count();
        self.aplicar(ini, fin - ini, texto, destino, fin == ini)
    }

    /// Enter con sangría automática (copia la de la línea y suma un nivel tras `{`).
    pub fn nueva_linea(&mut self) -> Option<Delta> {
        let (linea, col) = self.linea_columna();
        let texto = self.linea_texto(linea);
        let sangria: String = texto.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
        let antes: String = texto.chars().take(col).collect();
        let mut extra = String::new();
        if self.lenguaje == Lenguaje::C && antes.trim_end().ends_with('{') {
            extra.push_str("    ");
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
        // Borra una sangría completa de 4 espacios de una vez.
        let (linea, col) = self.linea_columna();
        let antes: String = self.linea_texto(linea).chars().take(col).collect();
        let n = if !antes.is_empty() && antes.chars().all(|c| c == ' ') && col % 4 == 0 { 4 } else { 1 };
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
        let delta = Delta { offset: e.offset, insertado: e.borrado.clone(), borrados: n };
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
        let delta = Delta { offset: e.offset, insertado: e.insertado.clone(), borrados: n };
        self.deshacer.push(e);
        Some(delta)
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
        let ruta = self
            .ruta
            .clone()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "el buffer no tiene ruta"))?;
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

/// Escritura atómica: `.<nombre>.jonesy-tmp` + fsync + rename (+ fsync del directorio).
pub fn guardar_atomico(ruta: &Path, datos: &[u8]) -> std::io::Result<usize> {
    let dir = ruta.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let nombre = ruta.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
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
    use super::*;

    #[test]
    fn insertar_borrar_y_deshacer() {
        let mut b = Buffer::desde_texto("int x;\n");
        b.mover_a(4, false);
        let d = b.insertar("y").unwrap();
        assert_eq!(d, Delta { offset: 4, insertado: "y".into(), borrados: 0 });
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
