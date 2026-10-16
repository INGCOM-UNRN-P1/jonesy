//! Mouse: clics, rueda, arrastre y enlaces sobre los paneles (N-JONESY-02).

use super::*;

impl App {
    pub(super) fn mouse(&mut self, m: MouseEvent) {
        let (x, y) = (m.column, m.row);
        let dentro = |r: Rect| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height;
        match m.kind {
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let delta: isize = if m.kind == MouseEventKind::ScrollUp {
                    -3
                } else {
                    3
                };
                if let Some(Modal::Diff {
                    desplazamiento,
                    filas,
                    ..
                }) = &mut self.modal
                {
                    *desplazamiento = (*desplazamiento as isize + delta)
                        .clamp(0, filas.len().saturating_sub(1) as isize)
                        as usize;
                    return;
                }
                if let Some(Modal::Visor {
                    desplazamiento,
                    lineas,
                    ..
                }) = &mut self.modal
                {
                    *desplazamiento = (*desplazamiento as isize + delta)
                        .clamp(0, lineas.len().saturating_sub(1) as isize)
                        as usize;
                    return;
                }
                if let (Some(z), Some(v)) = (self.zonas.editor, &mut self.buffers[self.activo].hex)
                {
                    if dentro(z.texto) {
                        match &mut v.detalle {
                            Some((_, lineas, d)) => {
                                *d = (*d as isize + delta)
                                    .clamp(0, lineas.len().saturating_sub(1) as isize)
                                    as usize
                            }
                            None => {
                                let ancho = v.ancho.max(1);
                                let ultimo = v.datos.len().saturating_sub(1) / ancho * ancho;
                                v.inicio = (v.inicio as isize + delta * ancho as isize)
                                    .clamp(0, ultimo as isize)
                                    as usize;
                            }
                        }
                        return;
                    }
                }
                if self.areas.izquierdo.is_some_and(dentro) {
                    let paso = delta.signum();
                    match self.navegador {
                        Some(PestanaNav::Archivos) => {
                            let n = self.explorador.filas.len().saturating_sub(1);
                            self.explorador.sel =
                                ((self.explorador.sel as isize + paso * 3).max(0) as usize).min(n);
                        }
                        Some(PestanaNav::Proyecto) => {
                            let n = self
                                .vista_proyecto
                                .filas(&self.proyecto.estructura)
                                .len()
                                .saturating_sub(1);
                            let v = &mut self.vista_proyecto;
                            v.sel = ((v.sel as isize + paso * 3).max(0) as usize).min(n);
                        }
                        Some(PestanaNav::Esquema) => {
                            let n = self.simbolos_activos().len().saturating_sub(1);
                            self.esquema_sel =
                                ((self.esquema_sel as isize + paso * 3).max(0) as usize).min(n);
                        }
                        None => {}
                    }
                } else if self.zonas.preview.is_some_and(dentro) {
                    let b = self.buf_mut();
                    b.preview_desplazamiento =
                        (b.preview_desplazamiento as isize + delta).max(0) as usize;
                } else if self
                    .zonas
                    .editor
                    .is_some_and(|z| dentro(z.texto) || dentro(z.gutter))
                {
                    let b = &mut self.buf_mut().buf;
                    let max = b.lineas().saturating_sub(1);
                    b.desplazamiento =
                        ((b.desplazamiento as isize + delta).max(0) as usize).min(max);
                    self.desplazado_por_mouse = true;
                } else if self.areas.consigna.is_some_and(dentro) {
                    if let Some(c) = &mut self.consigna {
                        c.desplazamiento = (c.desplazamiento as isize + delta).max(0) as usize;
                    }
                } else if self.zonas.deck.is_some_and(dentro) {
                    let deck = match self.pestana {
                        Pestana::Terminal => self.terminal.as_ref(),
                        Pestana::Ejecucion => self.ejecucion.as_ref(),
                        _ => None,
                    };
                    if let Some(d) = deck {
                        let actual = d.con_pantalla(|s| s.scrollback()) as isize;
                        d.desplazar((actual - delta).max(0) as usize);
                    }
                } else if self.areas.inferior.is_some_and(dentro) {
                    match self.pestana {
                        Pestana::Salida => {
                            self.salida_desplazamiento =
                                (self.salida_desplazamiento as isize - delta).max(0) as usize
                        }
                        Pestana::Problemas => {
                            let total = self.diagnosticos().len();
                            self.sel_problema = ((self.sel_problema as isize + delta.signum())
                                .max(0) as usize)
                                .min(total.saturating_sub(1));
                        }
                        _ => {}
                    }
                }
            }
            MouseEventKind::Down(MouseButton::Left) => self.clic(x, y, m.modifiers),
            // Mientras se depura, el valor de lo que está bajo el mouse (#31).
            MouseEventKind::Moved if self.depuracion.is_some() => {
                let palabra = self
                    .zonas
                    .editor
                    .filter(|z| dentro(z.texto))
                    .and_then(|_| self.posicion_en_editor(x, y))
                    .and_then(|p| self.buf().buf.palabra_en(p));
                self.hover = palabra;
            }
            MouseEventKind::Drag(MouseButton::Left) => match self.arrastre {
                Some(Arrastre::Seleccion) => {
                    if let Some(p) = self.posicion_en_editor(x, y) {
                        self.buf_mut().buf.mover_a(p, true);
                    }
                }
                Some(Arrastre::Separador(sep)) => sep.aplicar(x, y, &mut self.paneles),
                None => {}
            },
            MouseEventKind::Down(MouseButton::Middle) => {
                // Clic del medio sobre una pestaña la cierra, como en los navegadores.
                let pestana = self
                    .zonas
                    .pestanas_buffers
                    .iter()
                    .find(|(r, _)| dentro(*r))
                    .map(|(_, i)| *i);
                if let Some(i) = pestana {
                    self.cerrar_pestana(i);
                }
            }
            MouseEventKind::Up(_) => {
                if matches!(self.arrastre, Some(Arrastre::Separador(_))) {
                    self.guardar_sesion();
                }
                self.arrastre = None;
            }
            _ => {}
        }
    }

    pub(super) fn clic(&mut self, x: u16, y: u16, modificadores: KeyModifiers) {
        let shift = modificadores.contains(KeyModifiers::SHIFT);
        let ctrl = modificadores.contains(KeyModifiers::CONTROL);
        let dentro = |r: Rect| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height;
        let doble = self.ultimo_clic.is_some_and(|(t, cx, cy)| {
            t.elapsed() < Duration::from_millis(400) && cx == x && cy == y
        });
        self.ultimo_clic = Some((Instant::now(), x, y));

        // Menú desplegado y modales.
        if let Some(Modal::Menu { .. }) = self.modal {
            if let (Some((_, n)), Some(Modal::Menu { indice, .. })) = (
                self.zonas
                    .items_menu
                    .iter()
                    .find(|(r, _)| dentro(*r))
                    .copied(),
                &self.modal,
            ) {
                let indice = *indice;
                self.modal = None;
                if let Some(e) = self
                    .menus()
                    .get(indice)
                    .and_then(|m| m.items.get(n).cloned())
                {
                    self.elegir_entrada(e);
                }
                return;
            }
            if let Some((_, i)) = self.zonas.menu.iter().find(|(r, _)| dentro(*r)).copied() {
                self.abrir_menu(i);
                return;
            }
            self.modal = None;
            return;
        }
        if let Some(modal) = &mut self.modal {
            if let (
                Some((r, base)),
                Modal::Paleta { sel, .. }
                | Modal::Abrir { sel, .. }
                | Modal::AbrirProyecto { sel, .. }
                | Modal::Configuracion { sel, .. },
            ) = (self.zonas.modal_items, &mut *modal)
            {
                if dentro(r) {
                    *sel = base + (y - r.y) as usize;
                    let k = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
                    self.tecla_modal(k);
                    return;
                }
            }
            if !matches!(modal, Modal::Visor { .. }) {
                return;
            }
            self.modal = None;
            return;
        }
        if let Some((_, i)) = self.zonas.menu.iter().find(|(r, _)| dentro(*r)).copied() {
            self.abrir_menu(i);
            return;
        }
        if let Some((_, a)) = self
            .zonas
            .botones
            .iter()
            .chain(self.zonas.botones_depurador.iter())
            .find(|(r, _)| dentro(*r))
            .copied()
        {
            self.accion(a);
            return;
        }
        if let Some((_, i)) = self
            .zonas
            .cierres_buffers
            .iter()
            .find(|(r, _)| dentro(*r))
            .copied()
        {
            self.cerrar_pestana(i);
            return;
        }
        if let Some((_, d)) = self
            .zonas
            .desborde_pestanas
            .iter()
            .find(|(r, _)| dentro(*r))
            .copied()
        {
            let n = self.buffers.len().saturating_sub(1);
            self.primera_pestana =
                (self.primera_pestana as isize + d).clamp(0, n as isize) as usize;
            return;
        }
        if let Some((_, i)) = self
            .zonas
            .pestanas_buffers
            .iter()
            .find(|(r, _)| dentro(*r))
            .copied()
        {
            self.cambiar_buffer(i);
            return;
        }
        if let Some((_, p)) = self
            .zonas
            .pestanas_inferior
            .iter()
            .find(|(r, _)| dentro(*r))
            .copied()
        {
            self.pestana = p;
            self.cambiar_foco(Foco::Inferior);
            if p == Pestana::Terminal && self.terminal.is_none() {
                self.abrir_terminal();
            }
            return;
        }
        // Separadores arrastrables entre paneles.
        if let Some(sep) = self
            .areas
            .separadores
            .iter()
            .find(|s| s.toca(x, y))
            .copied()
        {
            self.arrastre = Some(Arrastre::Separador(sep));
            return;
        }
        // Barra izquierda: pestañas y filas.
        if let Some((_, p)) = self
            .zonas
            .pestanas_nav
            .iter()
            .find(|(r, _)| dentro(*r))
            .copied()
        {
            self.navegador = Some(p);
            self.cambiar_foco(Foco::Navegador);
            if p == PestanaNav::Archivos && self.explorador.filas.is_empty() {
                self.explorador.reconstruir(&self.proyecto.raiz);
            }
            return;
        }
        if let Some((r, base)) = self.zonas.lista_nav {
            if dentro(r) {
                self.cambiar_foco(Foco::Navegador);
                let i = base + (y - r.y) as usize;
                match self.navegador {
                    Some(PestanaNav::Archivos) => {
                        if let Some(f) = self.explorador.filas.get(i).cloned() {
                            self.explorador.sel = i;
                            if f.es_dir {
                                self.explorador.alternar(i, &self.proyecto.raiz);
                            } else {
                                // Un clic abre el archivo (en una pestaña nueva si no estaba).
                                self.abrir_archivo(&f.ruta, true);
                                self.foco = Foco::Navegador;
                            }
                        }
                    }
                    Some(PestanaNav::Esquema) => {
                        // Un clic salta al símbolo (el foco queda en la barra).
                        self.ir_a_simbolo(i, false);
                        self.foco = Foco::Navegador;
                    }
                    Some(PestanaNav::Proyecto) => {
                        let filas = self.vista_proyecto.filas(&self.proyecto.estructura);
                        if let Some(f) = filas.get(i).cloned() {
                            self.vista_proyecto.sel = i;
                            // Módulos se despliegan con un clic; objetivos corren con doble clic.
                            let correr =
                                doble || matches!(f, FilaProyecto::Modulo(_) | FilaProyecto::TpSh);
                            if correr {
                                self.activar_fila_proyecto(Some(f), true);
                            }
                        }
                    }
                    None => {}
                }
                return;
            }
        }
        if self.areas.izquierdo.is_some_and(dentro) {
            self.cambiar_foco(Foco::Navegador);
            return;
        }
        if let Some(z) = self.zonas.editor {
            if dentro(z.gutter) {
                let linea = self.buf().buf.desplazamiento + (y - z.gutter.y) as usize;
                if linea < self.buf().buf.lineas() {
                    self.alternar_breakpoint(linea);
                }
                self.cambiar_foco(Foco::Editor);
                return;
            }
            if dentro(z.texto) {
                self.cambiar_foco(Foco::Editor);
                if let Some(p) = self.posicion_en_editor(x, y) {
                    // Ctrl+clic: ir a la definición de lo que está bajo el mouse.
                    if ctrl {
                        self.buf_mut().buf.mover_a(p, false);
                        self.ir_a_definicion();
                        return;
                    }
                    if doble {
                        self.buf_mut().buf.seleccionar_palabra(p);
                    } else {
                        self.buf_mut().buf.mover_a(p, shift);
                        self.arrastre = Some(Arrastre::Seleccion);
                    }
                }
                return;
            }
        }
        if let Some(r) = self.zonas.preview {
            if dentro(r) {
                self.cambiar_foco(Foco::Preview);
                let b = self.buf();
                let linea = b.preview_desplazamiento + (y - r.y) as usize;
                let col = (x - r.x) as usize;
                let destino = b
                    .preview
                    .as_ref()
                    .and_then(|(_, _, d)| enlace_en(d, linea, col));
                let base = b
                    .buf
                    .ruta
                    .as_ref()
                    .and_then(|p| p.parent().map(Path::to_path_buf));
                if let (Some(d), Some(base)) = (destino, base) {
                    self.abrir_enlace(&base, &d);
                }
                return;
            }
        }
        if let Some(r) = self.areas.consigna {
            if dentro(r) {
                self.cambiar_foco(Foco::Consigna);
                if let Some(c) = &self.consigna {
                    let linea = c.desplazamiento + (y.saturating_sub(r.y + 1)) as usize;
                    let col = x.saturating_sub(r.x + 1) as usize;
                    let destino = c.doc.as_ref().and_then(|(_, d)| enlace_en(d, linea, col));
                    let base = c.ruta.parent().map(Path::to_path_buf);
                    if let (Some(d), Some(base)) = (destino, base) {
                        self.abrir_enlace(&base, &d);
                    }
                }
                return;
            }
        }
        if let Some((r, base)) = self.zonas.lista_inferior {
            if dentro(r) {
                self.cambiar_foco(Foco::Inferior);
                if self.pestana == Pestana::Problemas {
                    self.sel_problema = base + (y - r.y) as usize;
                    self.saltar_a_problema(self.sel_problema);
                }
                if self.pestana == Pestana::Busqueda {
                    self.abrir_hallazgo(base + (y - r.y) as usize);
                    self.foco = Foco::Inferior;
                }
                return;
            }
        }
        if let Some((r, base)) = self.zonas.lista_lateral {
            if dentro(r) {
                self.cambiar_foco(Foco::Lateral);
                let i = base + (y - r.y) as usize;
                match self.lateral {
                    Some(Lateral::Git) => {
                        if self.git.log.is_some() {
                            self.git.sel_log = i;
                        } else {
                            self.git.sel = i;
                        }
                        if doble {
                            self.git_mostrar();
                        }
                    }
                    Some(Lateral::Reglas) => {
                        self.sel_regla = i;
                        let d = self
                            .reglas_agrupadas()
                            .into_iter()
                            .flat_map(|(_, v)| v)
                            .nth(i)
                            .cloned();
                        if let Some(Diagnostico {
                            archivo: Some(a),
                            linea,
                            columna,
                            ..
                        }) = d
                        {
                            self.ir_a(&a, linea, columna);
                        }
                    }
                    _ => {}
                }
                return;
            }
        }
        if self.zonas.deck.is_some_and(dentro) {
            self.cambiar_foco(Foco::Inferior);
            return;
        }
        if self.areas.inferior.is_some_and(dentro) {
            self.cambiar_foco(Foco::Inferior);
            if self.pestana == Pestana::Depurador {
                if let Some(d) = &mut self.depuracion {
                    // Mitad izquierda: programa; derecha: consola.
                    let r = self.areas.inferior.unwrap();
                    d.foco_consola = x >= r.x + r.width / 2;
                }
            }
            return;
        }
        if self.areas.lateral.is_some_and(dentro) {
            self.cambiar_foco(Foco::Lateral);
            return;
        }
        if self.areas.quickdoc.is_some_and(dentro) {
            self.cambiar_foco(Foco::Doc);
        }
    }

    pub(super) fn abrir_enlace(&mut self, base: &Path, destino: &str) {
        // Enlace a una sección de la misma consigna (#ancla).
        if let Some(ancla) = destino.strip_prefix('#') {
            if let Some(c) = &mut self.consigna {
                if let Some(l) = c.doc.as_ref().and_then(|(_, d)| linea_de_ancla(d, ancla)) {
                    c.desplazamiento = l;
                    return;
                }
            }
        }
        if !markdown::es_enlace_local(destino) {
            self.avisar(format!("Enlace externo: {destino}"), Nivel::Info);
            return;
        }
        let sin_ancla = destino.split('#').next().unwrap_or(destino);
        let ruta = base.join(sin_ancla);
        if ruta.is_file() {
            self.abrir_archivo(&ruta, true);
        } else {
            self.avisar(format!("No existe {}", ruta.display()), Nivel::Aviso);
        }
    }

    /// Posición de carácter del buffer activo bajo la celda (x, y).
    pub(super) fn posicion_en_editor(&self, x: u16, y: u16) -> Option<usize> {
        let z = self.zonas.editor?;
        let b = &self.buf().buf;
        let fila = y
            .saturating_sub(z.texto.y)
            .min(z.texto.height.saturating_sub(1)) as usize;
        let linea = (b.desplazamiento + fila).min(b.lineas().saturating_sub(1));
        let objetivo = b.desplazamiento_h + x.saturating_sub(z.texto.x) as usize;
        let chars: Vec<char> = b.linea_texto(linea).chars().collect();
        let col = columna_de_visual(&chars, objetivo);
        Some(b.linea_columna_a_pos(linea, col))
    }
}
