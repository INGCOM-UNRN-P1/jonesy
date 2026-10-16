//! Teclado fuera de los modales: despacho por foco (editor, navegador, paneles, terminal, hexadecimal) (N-JONESY-02).

use super::*;

impl App {
    /// Teclas del visor hexadecimal (pestaña activa). Las globales (F-teclas, Alt, menús)
    /// ya se atendieron antes.
    pub(super) fn tecla_hex(&mut self, v: &mut VisorHex, k: KeyEvent) {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        // Campo de «ir a» o de búsqueda.
        if let Some((ir, mut texto)) = v.entrada.take() {
            match k.code {
                KeyCode::Esc => {}
                KeyCode::Enter if ir => match jonesy_core::hex::offset(&texto) {
                    Some(o) if o < v.datos.len() => {
                        v.marca = 0;
                        v.detalle = None;
                        v.ir(o);
                    }
                    _ => self.avisar(
                        format!(
                            "«{texto}» no es un offset del archivo (0 a 0x{:x}).",
                            v.datos.len().saturating_sub(1)
                        ),
                        Nivel::Aviso,
                    ),
                },
                KeyCode::Enter => {
                    v.buscado = jonesy_core::hex::patron(&texto);
                    v.detalle = None;
                    let desde = v.cursor;
                    self.buscar_hex(v, desde);
                }
                _ => {
                    editar_campo(&mut texto, &k);
                    v.entrada = Some((ir, texto));
                }
            }
            return;
        }
        let pagina = (v.ancho * v.alto.max(1)) as isize;
        if let Some((_, lineas, desp)) = &mut v.detalle {
            let n = lineas.len().saturating_sub(1);
            match k.code {
                KeyCode::Esc | KeyCode::Char('i' | 's' | '?') => v.detalle = None,
                KeyCode::Up => *desp = desp.saturating_sub(1),
                KeyCode::Down => *desp = (*desp + 1).min(n),
                KeyCode::PageUp => *desp = desp.saturating_sub(v.alto.max(1)),
                KeyCode::PageDown => *desp = (*desp + v.alto.max(1)).min(n),
                KeyCode::Char('g') => v.entrada = Some((true, String::new())),
                _ => {}
            }
            return;
        }
        match k.code {
            KeyCode::Left => v.mover(-1),
            KeyCode::Right => v.mover(1),
            KeyCode::Up => v.mover(-(v.ancho as isize)),
            KeyCode::Down => v.mover(v.ancho as isize),
            KeyCode::PageUp => v.mover(-pagina),
            KeyCode::PageDown => v.mover(pagina),
            KeyCode::Home if ctrl => v.mover(isize::MIN / 2),
            KeyCode::End if ctrl => v.mover(isize::MAX / 2),
            KeyCode::Home => {
                let p = v.cursor / v.ancho.max(1) * v.ancho.max(1);
                v.ir(p);
            }
            KeyCode::End => {
                let p = v.cursor / v.ancho.max(1) * v.ancho.max(1) + v.ancho - 1;
                v.ir(p);
            }
            KeyCode::Char('g') => v.entrada = Some((true, String::new())),
            KeyCode::Char('/' | 'f') => v.entrada = Some((false, String::new())),
            KeyCode::Char('n') => {
                let desde = v.cursor + 1;
                self.buscar_hex(v, desde);
            }
            KeyCode::Char('i') => {
                let info = jonesy_core::hex::informacion(&v.ruta, &v.datos);
                v.detalle = Some(("Información".into(), info, 0));
            }
            KeyCode::Char('s') => match jonesy_core::hex::simbolos(&v.ruta) {
                Ok(s) => v.detalle = Some(("Símbolos (nm)".into(), s, 0)),
                Err(e) => self.avisar(e, Nivel::Aviso),
            },
            KeyCode::Char('?') => {
                v.detalle = Some((
                    "Atajos del visor hexadecimal".into(),
                    crate::ayuda::ATAJOS_HEX
                        .lines()
                        .map(str::to_string)
                        .collect(),
                    0,
                ))
            }
            KeyCode::Char(_) | KeyCode::Enter | KeyCode::Backspace | KeyCode::Delete => self
                .avisar(
                    "Es un archivo binario: se mira, no se edita (? muestra las teclas del visor).",
                    Nivel::Info,
                ),
            _ => {}
        }
    }

    pub(super) fn tecla_navegador(&mut self, k: KeyEvent) {
        match k.code {
            KeyCode::Esc => {
                self.cambiar_foco(Foco::Editor);
                if self.vista == Vista::Navegador {
                    self.vista = Vista::Codigo;
                }
                return;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                let adelante = k.code == KeyCode::Tab;
                let otra = match (self.navegador, adelante) {
                    (Some(PestanaNav::Archivos), true) => PestanaNav::Proyecto,
                    (Some(PestanaNav::Proyecto), true) => PestanaNav::Esquema,
                    (Some(PestanaNav::Esquema), false) => PestanaNav::Proyecto,
                    (Some(PestanaNav::Archivos), false) => PestanaNav::Esquema,
                    _ => PestanaNav::Archivos,
                };
                self.navegador = Some(otra);
                if otra == PestanaNav::Archivos && self.explorador.filas.is_empty() {
                    self.explorador.reconstruir(&self.proyecto.raiz);
                }
                if otra == PestanaNav::Esquema {
                    self.revelar_en_esquema();
                }
                return;
            }
            _ => {}
        }
        let alto = self
            .zonas
            .lista_nav
            .map(|(r, _)| r.height as usize)
            .unwrap_or(10)
            .max(1);
        match self.navegador {
            Some(PestanaNav::Esquema) => {
                let n = self.simbolos_activos().len();
                let s = &mut self.esquema_sel;
                match k.code {
                    KeyCode::Up => *s = s.saturating_sub(1),
                    KeyCode::Down => *s = (*s + 1).min(n.saturating_sub(1)),
                    KeyCode::PageUp => *s = s.saturating_sub(alto),
                    KeyCode::PageDown => *s = (*s + alto).min(n.saturating_sub(1)),
                    KeyCode::Home => *s = 0,
                    KeyCode::End => *s = n.saturating_sub(1),
                    KeyCode::Enter | KeyCode::Right | KeyCode::Char(' ') => {
                        let i = *s;
                        self.ir_a_simbolo(i, k.code == KeyCode::Enter);
                    }
                    _ => {}
                }
            }
            Some(PestanaNav::Archivos) => {
                let raiz = self.proyecto.raiz.clone();
                let e = &mut self.explorador;
                let n = e.filas.len();
                match k.code {
                    KeyCode::Up => e.sel = e.sel.saturating_sub(1),
                    KeyCode::Down => e.sel = (e.sel + 1).min(n.saturating_sub(1)),
                    KeyCode::PageUp => e.sel = e.sel.saturating_sub(alto),
                    KeyCode::PageDown => e.sel = (e.sel + alto).min(n.saturating_sub(1)),
                    KeyCode::Home => e.sel = 0,
                    KeyCode::End => e.sel = n.saturating_sub(1),
                    KeyCode::Left => match e.seleccionada() {
                        Some(f) if f.es_dir && e.expandidos.contains(&f.ruta) => {
                            e.alternar(e.sel, &raiz)
                        }
                        _ => {
                            if let Some(p) = e.padre(e.sel) {
                                e.sel = p;
                            }
                        }
                    },
                    KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ') => {
                        match e.seleccionada().cloned() {
                            Some(f) if f.es_dir => {
                                if k.code != KeyCode::Right || !e.expandidos.contains(&f.ruta) {
                                    e.alternar(e.sel, &raiz);
                                }
                            }
                            Some(f) if k.code != KeyCode::Right => {
                                self.abrir_archivo(&f.ruta, true);
                                if self.modo() == Modo::Compacto {
                                    self.vista = Vista::Codigo;
                                }
                            }
                            _ => {}
                        }
                    }
                    KeyCode::Char('n') => {
                        let d = e.directorio_destino(&raiz);
                        self.pedir_nuevo(&d, false);
                    }
                    KeyCode::Char('m') => {
                        let d = e.directorio_destino(&raiz);
                        self.pedir_nuevo(&d, true);
                    }
                    KeyCode::F(2) => {
                        if let Some(f) = e.seleccionada().cloned() {
                            self.pedir_renombrar(&f.ruta);
                        }
                    }
                    KeyCode::Delete => {
                        if let Some(f) = e.seleccionada().cloned() {
                            self.pedir_borrar(&f.ruta);
                        }
                    }
                    KeyCode::Char('r') => {
                        self.refrescar_estructura();
                        self.refrescar_git();
                    }
                    _ => {}
                }
            }
            Some(PestanaNav::Proyecto) => {
                let filas = self.vista_proyecto.filas(&self.proyecto.estructura);
                let n = filas.len();
                let v = &mut self.vista_proyecto;
                v.sel = v.sel.min(n.saturating_sub(1));
                match k.code {
                    KeyCode::Up => v.sel = v.sel.saturating_sub(1),
                    KeyCode::Down => v.sel = (v.sel + 1).min(n.saturating_sub(1)),
                    KeyCode::PageUp => v.sel = v.sel.saturating_sub(alto),
                    KeyCode::PageDown => v.sel = (v.sel + alto).min(n.saturating_sub(1)),
                    KeyCode::Home => v.sel = 0,
                    KeyCode::End => v.sel = n.saturating_sub(1),
                    KeyCode::Char('r') => self.refrescar_estructura(),
                    KeyCode::Left => match filas.get(v.sel) {
                        Some(FilaProyecto::Objetivo(m, _)) => {
                            let m = *m;
                            if let Some(i) =
                                filas.iter().position(|f| *f == FilaProyecto::Modulo(m))
                            {
                                v.sel = i;
                            }
                        }
                        Some(f @ (FilaProyecto::Modulo(_) | FilaProyecto::TpSh)) => {
                            let abierto = match f {
                                FilaProyecto::Modulo(i) => v
                                    .desplegados
                                    .contains(&self.proyecto.estructura.modulos[*i].rel),
                                _ => v.tp_desplegado,
                            };
                            if abierto {
                                v.alternar(f, &self.proyecto.estructura);
                            }
                        }
                        _ => {}
                    },
                    KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ') => {
                        let fila = filas.get(v.sel).cloned();
                        self.activar_fila_proyecto(fila, k.code != KeyCode::Right);
                    }
                    _ => {}
                }
            }
            None => self.cambiar_foco(Foco::Editor),
        }
    }

    pub fn tecla(&mut self, k: KeyEvent) {
        self.registrar_tecla(&k);
        self.hover = None;
        self.desplazado_por_mouse = false;
        if self.modal.is_some() {
            self.tecla_modal(k);
            return;
        }
        // Los atajos propios de [atajos] ganan a los de fábrica.
        if let Some(a) = self
            .atajos_usuario
            .iter()
            .find(|(t, _, _)| t.coincide(&k))
            .map(|(_, a, _)| *a)
        {
            self.accion(a);
            return;
        }
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let alt = k.modifiers.contains(KeyModifiers::ALT);
        let shift = k.modifiers.contains(KeyModifiers::SHIFT);

        // Pestañas: Ctrl+Tab / Ctrl+Shift+Tab y Alt+1…9 (Alt+0 la última).
        match (k.code, ctrl, alt) {
            (KeyCode::Tab, true, _) => {
                self.accion(Accion::SiguienteBuffer);
                return;
            }
            (KeyCode::BackTab, true, _) => {
                self.accion(Accion::AnteriorBuffer);
                return;
            }
            (KeyCode::Char(c @ '0'..='9'), false, true) => {
                let n = self.buffers.len();
                let i = if c == '0' {
                    n - 1
                } else {
                    (c as usize - '1' as usize).min(n - 1)
                };
                self.cambiar_buffer(i);
                return;
            }
            _ => {}
        }
        // Teclas que siempre son globales (también con el foco en la terminal).
        let global = match (k.code, ctrl, alt, shift) {
            (KeyCode::Char('t'), false, true, _) => Some(Accion::Terminal),
            (KeyCode::F(5), true, _, _) => Some(Accion::Ejecutar),
            (KeyCode::F(5), _, _, true) => Some(Accion::Detener),
            (KeyCode::F(5), _, _, _) => Some(Accion::Depurar),
            (KeyCode::F(11), _, _, true) => Some(Accion::SalirFuncion),
            (KeyCode::F(11), _, _, _) => Some(Accion::Entrar),
            (KeyCode::F(10), true, _, _) => Some(Accion::CorrerHastaCursor),
            (KeyCode::F(10), _, _, _) => Some(Accion::Paso),
            (KeyCode::F(6), true, _, _) => Some(Accion::Probar),
            (KeyCode::F(6), _, _, true) => Some(Accion::ProbarAqui),
            (KeyCode::F(6), _, _, _) => Some(Accion::Compilar),
            (KeyCode::F(7), _, _, _) => Some(Accion::VistaConsigna),
            (KeyCode::F(8), _, _, _) => Some(Accion::VistaCodigo),
            // Shift+F9 llega como F21 en algunas terminales.
            (KeyCode::F(9), _, _, true) | (KeyCode::F(21), _, _, _) => Some(Accion::ObjetivoMake),
            (KeyCode::F(9), _, _, _) => Some(Accion::VistaTerminal),
            // F12: entregar en el examen, terminar el simulacro; fuera, ir a la definición.
            (KeyCode::F(12), _, _, true) | (KeyCode::F(24), _, _, _) => Some(Accion::Referencias),
            (KeyCode::F(12), _, _, _) => Some(if self.examen.is_some() {
                Accion::Entregar
            } else if self.simulacro.is_some() {
                Accion::TerminarSimulacro
            } else {
                Accion::IrADefinicion
            }),
            (KeyCode::Char('z'), false, true, _) => Some(Accion::MaximizarPanel),
            (KeyCode::Char('g'), false, true, _) => Some(Accion::PanelGit),
            (KeyCode::Char('e'), false, true, _) => Some(Accion::Explorador),
            (KeyCode::Char('h'), false, true, _) => Some(Accion::Herramientas),
            (KeyCode::Char('y'), false, true, _) => Some(Accion::EstructuraProyecto),
            (KeyCode::Char('s'), false, true, _) => Some(Accion::Esquema),
            (KeyCode::Char('i'), false, true, _) => Some(Accion::ArchivoPareja),
            (KeyCode::Char('n'), false, true, false) => Some(Accion::SiguienteProblema),
            (KeyCode::Char('N'), false, true, _) | (KeyCode::Char('n'), false, true, true) => {
                Some(Accion::AnteriorProblema)
            }
            (KeyCode::F(2), true, _, _) => Some(Accion::AlternarMarcador),
            (KeyCode::F(2), _, _, true) => Some(Accion::SiguienteMarcador),
            // Ctrl+Shift+F sólo llega en algunas terminales y Ctrl+F3 (`ESC[1;5R`) crossterm
            // lo toma por la respuesta de posición del cursor y lo descarta: Alt+F llega en
            // todas (salvo en la terminal, donde la shell lo usa para avanzar una palabra).
            (KeyCode::Char('F'), true, _, _) | (KeyCode::Char('f'), true, _, true) => {
                Some(Accion::BuscarEnProyecto)
            }
            (KeyCode::Char('f'), false, true, false)
                if !(self.foco == Foco::Inferior
                    && matches!(self.pestana, Pestana::Terminal | Pestana::Ejecucion)) =>
            {
                Some(Accion::BuscarEnProyecto)
            }
            (KeyCode::F(3), true, _, _) | (KeyCode::F(27), _, _, _) => {
                Some(Accion::BuscarEnProyecto)
            }
            (KeyCode::Char('b'), false, true, false)
                if !(self.foco == Foco::Inferior
                    && matches!(self.pestana, Pestana::Terminal | Pestana::Ejecucion)) =>
            {
                Some(Accion::CompilarAlGuardar)
            }
            (KeyCode::Char('n'), true, false, _) => Some(Accion::NuevoArchivo),
            (KeyCode::Char('F'), false, true, _) | (KeyCode::Char('f'), false, true, true) => {
                Some(Accion::Formatear)
            }
            (KeyCode::Char('D'), false, true, _) | (KeyCode::Char('d'), false, true, true) => {
                Some(Accion::DiffArchivo)
            }
            (KeyCode::Char('r'), false, true, _) => Some(Accion::PanelReglas),
            (KeyCode::Char('p'), false, true, _) => Some(Accion::Problemas),
            (KeyCode::Char('o'), false, true, _) => Some(Accion::SalidaCompilacion),
            (KeyCode::Char('m'), false, true, _) => Some(Accion::CatScan),
            (KeyCode::F(4), _, _, true) => Some(Accion::OrientacionQuickDoc),
            (KeyCode::F(4), _, _, _) => Some(Accion::QuickDoc),
            (KeyCode::Char('q'), true, _, _) => Some(Accion::Salir),
            (KeyCode::Char('k'), true, _, _) => Some(Accion::Paleta),
            // Ctrl+, llega sólo en terminales que lo distinguen de «,» (protocolo de teclado
            // extendido); Alt+, llega en todas.
            (KeyCode::Char(','), true, _, _) | (KeyCode::Char(','), false, true, _) => {
                Some(Accion::Configuracion)
            }
            _ => None,
        };
        // Alt+letra abre el menú de esa letra (subrayada en la barra), salvo dentro de la
        // terminal, donde la shell usa Alt+D, Alt+C…
        let en_terminal = self.foco == Foco::Inferior
            && matches!(self.pestana, Pestana::Terminal | Pestana::Ejecucion);
        if let (None, KeyCode::Char(c), true, false, false) =
            (global, k.code, alt, ctrl, en_terminal)
        {
            let menus = accion::menus(self.examen.is_some());
            if let Some(i) = menus.iter().position(|m| m.tecla == c) {
                self.abrir_menu(i);
                return;
            }
        }
        if let Some(a) = global {
            // F10 sin depuración abre el menú.
            if a == Accion::Paso && self.depuracion.is_none() {
                self.abrir_menu(0);
                return;
            }
            self.accion(a);
            return;
        }
        // `?` en un panel que no es de texto: sus atajos.
        if k.code == KeyCode::Char('?')
            && !k
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
            && self.foco != Foco::Editor
            && self.panel_actual() != Some(crate::ayuda::Panel::Editor)
            && self.panel_actual().is_some()
        {
            self.atajos_del_panel();
            return;
        }
        match self.foco {
            Foco::Inferior => self.tecla_inferior(k),
            Foco::Lateral => self.tecla_lateral(k),
            Foco::Consigna => self.tecla_consigna(k),
            Foco::Navegador => self.tecla_navegador(k),
            Foco::Preview => self.tecla_preview(k),
            _ => self.tecla_editor(k),
        }
    }

    pub(super) fn tecla_consigna(&mut self, k: KeyEvent) {
        let alto = self
            .areas
            .consigna
            .map(|r| r.height.saturating_sub(2) as usize)
            .unwrap_or(10)
            .max(1);
        let Some(c) = &mut self.consigna else {
            self.foco = Foco::Editor;
            return;
        };
        let doc = c.doc.as_ref().map(|(_, d)| d);
        match k.code {
            KeyCode::Up => c.desplazamiento = c.desplazamiento.saturating_sub(1),
            KeyCode::Down => c.desplazamiento += 1,
            KeyCode::PageUp => c.desplazamiento = c.desplazamiento.saturating_sub(alto),
            KeyCode::PageDown | KeyCode::Char(' ') => c.desplazamiento += alto,
            KeyCode::Home => c.desplazamiento = 0,
            // Tab / Shift+Tab recorren los enlaces; sin enlaces, Tab vuelve al editor.
            KeyCode::Tab | KeyCode::BackTab if doc.is_some_and(|d| !d.enlaces.is_empty()) => {
                let d = doc.unwrap();
                c.enlace = siguiente_enlace(d, c.enlace, k.code == KeyCode::Tab);
                if let Some(e) = c.enlace.and_then(|i| d.enlaces.get(i)) {
                    c.desplazamiento = a_la_vista(e.linea, c.desplazamiento, alto);
                }
            }
            KeyCode::Enter => {
                let destino = c
                    .enlace
                    .and_then(|i| doc.and_then(|d| d.enlaces.get(i)))
                    .map(|e| e.destino.clone());
                let base = c.ruta.parent().map(Path::to_path_buf).unwrap_or_default();
                if let Some(d) = destino {
                    if let Some(l) = d
                        .strip_prefix('#')
                        .and_then(|ancla| doc.and_then(|x| linea_de_ancla(x, ancla)))
                    {
                        c.desplazamiento = l;
                    } else {
                        self.abrir_enlace(&base, &d);
                    }
                }
            }
            KeyCode::Esc | KeyCode::Tab => self.cambiar_foco(Foco::Editor),
            KeyCode::Char('p') if self.deckard.is_some() => self.siguiente_pista(),
            _ => self.tecla_editor(k),
        }
    }

    /// Vista previa de Markdown con foco: desplazamiento y enlaces con el teclado.
    pub(super) fn tecla_preview(&mut self, k: KeyEvent) {
        let alto = self
            .zonas
            .preview
            .map(|r| r.height as usize)
            .unwrap_or(10)
            .max(1);
        let ba = &mut self.buffers[self.activo];
        let Some((_, _, doc)) = &ba.preview else {
            self.foco = Foco::Editor;
            return;
        };
        match k.code {
            KeyCode::Up => ba.preview_desplazamiento = ba.preview_desplazamiento.saturating_sub(1),
            KeyCode::Down => ba.preview_desplazamiento += 1,
            KeyCode::PageUp => {
                ba.preview_desplazamiento = ba.preview_desplazamiento.saturating_sub(alto)
            }
            KeyCode::PageDown | KeyCode::Char(' ') => ba.preview_desplazamiento += alto,
            KeyCode::Home => ba.preview_desplazamiento = 0,
            KeyCode::Tab | KeyCode::BackTab if !doc.enlaces.is_empty() => {
                ba.preview_enlace =
                    siguiente_enlace(doc, ba.preview_enlace, k.code == KeyCode::Tab);
                if let Some(e) = ba.preview_enlace.and_then(|i| doc.enlaces.get(i)) {
                    ba.preview_desplazamiento =
                        a_la_vista(e.linea, ba.preview_desplazamiento, alto);
                }
            }
            KeyCode::Enter => {
                let destino = ba
                    .preview_enlace
                    .and_then(|i| doc.enlaces.get(i))
                    .map(|e| e.destino.clone());
                let ancla = destino
                    .as_deref()
                    .and_then(|d| d.strip_prefix('#'))
                    .and_then(|a| linea_de_ancla(doc, a));
                if let Some(l) = ancla {
                    ba.preview_desplazamiento = l;
                } else if let Some(d) = destino {
                    let base = ba
                        .buf
                        .ruta
                        .as_ref()
                        .and_then(|p| p.parent().map(Path::to_path_buf))
                        .unwrap_or_default();
                    self.abrir_enlace(&base, &d);
                }
            }
            _ => {
                self.cambiar_foco(Foco::Editor);
                if k.code != KeyCode::Esc {
                    self.tecla_editor(k);
                }
            }
        }
    }

    pub(super) fn tecla_lateral(&mut self, k: KeyEvent) {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        if k.code == KeyCode::Esc || k.code == KeyCode::Tab {
            self.cambiar_foco(Foco::Editor);
            return;
        }
        match self.lateral {
            Some(Lateral::Git) => {
                let en_log = self.git.log.is_some();
                let total = if en_log {
                    self.git.log.as_ref().map(|l| l.len()).unwrap_or(0)
                } else {
                    self.git_estado
                        .as_ref()
                        .map(|e| e.entradas.len())
                        .unwrap_or(0)
                };
                let sel = if en_log {
                    &mut self.git.sel_log
                } else {
                    &mut self.git.sel
                };
                match k.code {
                    KeyCode::Up => *sel = sel.saturating_sub(1),
                    KeyCode::Down => *sel = (*sel + 1).min(total.saturating_sub(1)),
                    KeyCode::Char(' ') if !en_log => self.git_alternar_stage(),
                    KeyCode::Char('a') if !ctrl => self.git_accion(Accion::GitStageTodo),
                    KeyCode::Char('u') if !ctrl => self.git_accion(Accion::GitUnstageTodo),
                    KeyCode::Char('c') if !ctrl => self.git_accion(Accion::GitCommit),
                    KeyCode::Char('l') => self.git_accion(Accion::GitLog),
                    KeyCode::Char('z') => self.pedir_deshacer_commit(),
                    KeyCode::Char('d') if !en_log => {
                        let ruta = self
                            .git_estado
                            .as_ref()
                            .and_then(|e| e.entradas.get(self.git.sel))
                            .map(|e| e.ruta.clone());
                        if let Some(r) = ruta {
                            self.pedir_descartar(r);
                        }
                    }
                    KeyCode::Char('s') if !ctrl => self.git_remoto(OpRemota::Subir),
                    KeyCode::Char('b') if !ctrl => self.git_remoto(OpRemota::Traer),
                    KeyCode::Char('m') if en_log => {
                        let h = self
                            .git
                            .log
                            .as_ref()
                            .and_then(|l| l.get(self.git.sel_log))
                            .map(|c| c.hash.clone());
                        self.git.marcado = if self.git.marcado == h { None } else { h };
                    }
                    KeyCode::Char('r') => self.refrescar_git(),
                    KeyCode::Enter => self.git_mostrar(),
                    _ => self.tecla_editor(k),
                }
            }
            Some(Lateral::Reglas) => {
                let total: usize = self.reglas_agrupadas().iter().map(|(_, v)| v.len()).sum();
                match k.code {
                    KeyCode::Up => self.sel_regla = self.sel_regla.saturating_sub(1),
                    KeyCode::Down => {
                        self.sel_regla = (self.sel_regla + 1).min(total.saturating_sub(1))
                    }
                    KeyCode::Enter => {
                        let d = self
                            .reglas_agrupadas()
                            .into_iter()
                            .flat_map(|(_, v)| v)
                            .nth(self.sel_regla)
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
                    KeyCode::F(1) => self.explicar_regla(),
                    _ => self.tecla_editor(k),
                }
            }
            _ => self.tecla_editor(k),
        }
    }

    pub(super) fn tecla_inferior(&mut self, k: KeyEvent) {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        // Esc Esc devuelve el foco al editor (un Esc solo va al programa).
        if k.code == KeyCode::Esc {
            if self
                .esc_pendiente
                .is_some_and(|t| t.elapsed() < Duration::from_millis(600))
            {
                self.esc_pendiente = None;
                self.cambiar_foco(Foco::Editor);
                if self.modo() == Modo::Compacto {
                    self.vista = Vista::Codigo;
                }
                return;
            }
            self.esc_pendiente = Some(Instant::now());
            if matches!(self.pestana, Pestana::Problemas | Pestana::Salida) {
                self.cambiar_foco(Foco::Editor);
                return;
            }
        }
        match self.pestana {
            Pestana::Busqueda => {
                let total = self
                    .busqueda_proyecto
                    .as_ref()
                    .map(|(_, r)| r.hallazgos.len())
                    .unwrap_or(0);
                match k.code {
                    KeyCode::Up => self.sel_busqueda = self.sel_busqueda.saturating_sub(1),
                    KeyCode::Down => {
                        self.sel_busqueda = (self.sel_busqueda + 1).min(total.saturating_sub(1))
                    }
                    KeyCode::PageUp => self.sel_busqueda = self.sel_busqueda.saturating_sub(10),
                    KeyCode::PageDown => {
                        self.sel_busqueda = (self.sel_busqueda + 10).min(total.saturating_sub(1))
                    }
                    KeyCode::Enter => {
                        let i = self.sel_busqueda;
                        self.abrir_hallazgo(i);
                        self.cambiar_foco(Foco::Editor);
                    }
                    KeyCode::Esc | KeyCode::Tab => self.cambiar_foco(Foco::Editor),
                    _ => {}
                }
            }
            Pestana::Problemas => {
                let total = self.diagnosticos().len();
                match k.code {
                    KeyCode::Up => self.sel_problema = self.sel_problema.saturating_sub(1),
                    KeyCode::Down => {
                        self.sel_problema = (self.sel_problema + 1).min(total.saturating_sub(1))
                    }
                    KeyCode::Enter if k.modifiers.contains(KeyModifiers::ALT) => {
                        self.saltar_a_problema(self.sel_problema);
                        self.cambiar_foco(Foco::Editor);
                        self.arreglo_rapido();
                    }
                    KeyCode::Enter => self.saltar_a_problema(self.sel_problema),
                    KeyCode::F(1) => self.explicar_regla(),
                    KeyCode::Tab => self.cambiar_foco(Foco::Editor),
                    _ => {}
                }
            }
            Pestana::Salida => match k.code {
                KeyCode::Up => self.salida_desplazamiento += 1,
                KeyCode::Down => {
                    self.salida_desplazamiento = self.salida_desplazamiento.saturating_sub(1)
                }
                KeyCode::PageUp => self.salida_desplazamiento += 10,
                KeyCode::PageDown => {
                    self.salida_desplazamiento = self.salida_desplazamiento.saturating_sub(10)
                }
                KeyCode::Tab => self.cambiar_foco(Foco::Editor),
                _ => {}
            },
            Pestana::Terminal | Pestana::Ejecucion => {
                let deck = if self.pestana == Pestana::Terminal {
                    self.terminal.as_mut()
                } else {
                    self.ejecucion.as_mut()
                };
                let Some(d) = deck else { return };
                if k.modifiers.contains(KeyModifiers::SHIFT)
                    && matches!(k.code, KeyCode::PageUp | KeyCode::PageDown)
                {
                    let actual = d.con_pantalla(|s| s.scrollback());
                    let n = if k.code == KeyCode::PageUp {
                        actual + 10
                    } else {
                        actual.saturating_sub(10)
                    };
                    d.desplazar(n);
                    return;
                }
                d.desplazar(0);
                if let Some(bytes) = bytes_tecla(&k) {
                    let _ = d.escribir(&bytes);
                }
            }
            Pestana::Depurador => {
                let Some(d) = &mut self.depuracion else {
                    return;
                };
                if k.code == KeyCode::Tab {
                    d.foco_consola = !d.foco_consola;
                    return;
                }
                if d.foco_consola {
                    match k.code {
                        KeyCode::Enter => {
                            let cmd = std::mem::take(&mut d.entrada);
                            d.consola.push(format!("(gdb) {cmd}"));
                            let _ = d.gdb.consola(&cmd);
                            let _ = d.gdb.pedir_estado();
                        }
                        KeyCode::Backspace => {
                            d.entrada.pop();
                        }
                        KeyCode::Char(c) if !ctrl => d.entrada.push(c),
                        _ => {}
                    }
                } else if let Some(bytes) = bytes_tecla(&k) {
                    match &mut d.pty {
                        Some(p) => {
                            let _ = p.escribir(&bytes);
                        }
                        None => d.foco_consola = true,
                    }
                }
            }
        }
    }

    pub(super) fn tecla_editor(&mut self, k: KeyEvent) {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let alt = k.modifiers.contains(KeyModifiers::ALT);
        let shift = k.modifiers.contains(KeyModifiers::SHIFT);
        if self.foco != Foco::Editor && self.foco != Foco::Consigna {
            self.foco = Foco::Editor;
        }
        if self.modo() == Modo::Compacto
            && self.vista != Vista::Codigo
            && self.vista != Vista::Consigna
        {
            if k.code == KeyCode::Esc {
                self.vista = Vista::Codigo;
            }
            return;
        }
        let alto = self
            .zonas
            .editor
            .map(|z| z.texto.height as isize)
            .unwrap_or(20)
            .max(2);
        let accion = match (k.code, ctrl, shift) {
            (KeyCode::Char('s'), true, true) => Some(Accion::GuardarTodo),
            (KeyCode::Char('s'), true, false) | (KeyCode::F(2), _, _) => Some(Accion::Guardar),
            (KeyCode::Char('z'), true, _) => Some(Accion::Deshacer),
            (KeyCode::Char('y'), true, _) => Some(Accion::Rehacer),
            (KeyCode::Char('x'), true, _) => Some(Accion::Cortar),
            (KeyCode::Char('c'), true, _) => Some(Accion::Copiar),
            (KeyCode::Char('v'), true, _) => Some(Accion::Pegar),
            (KeyCode::Char('a'), true, _) => Some(Accion::SeleccionarTodo),
            (KeyCode::Char('f'), true, _) => Some(Accion::Buscar),
            // Ctrl+H: algunas terminales lo mandan como Ctrl+Backspace.
            (KeyCode::Char('h'), true, _) => Some(Accion::Reemplazar),
            (KeyCode::Backspace, true, _) => Some(Accion::Reemplazar),
            // Shift+F3 (`ESC[1;2R`) sólo llega con el protocolo de teclado extendido (o como
            // F15); Alt+W, como en nano, llega en todas.
            (KeyCode::F(3), _, true) | (KeyCode::F(15), _, _) => Some(Accion::BuscarSiguiente),
            (KeyCode::Char('w'), false, false) if alt => Some(Accion::BuscarSiguiente),
            (KeyCode::F(3), _, false) => Some(Accion::Buscar),
            (KeyCode::Char('g'), true, _) => Some(Accion::IrALinea),
            (KeyCode::Char('o'), true, _) => Some(Accion::Abrir),
            (KeyCode::Char('w'), true, _) => Some(Accion::CerrarBuffer),
            (KeyCode::Char('b'), true, _) => Some(Accion::AlternarBreakpoint),
            // Ctrl+/ llega como Ctrl+/ o como Ctrl+7 según la terminal.
            (KeyCode::Char('/' | '7' | '_'), true, _) => Some(Accion::Comentar),
            (KeyCode::Char('d'), true, _) => Some(Accion::DuplicarLinea),
            // Ctrl+Espacio llega como Ctrl+Espacio o como Ctrl+@ (NUL) según la terminal.
            (KeyCode::Char(' ' | '@'), true, _) => Some(Accion::Autocompletar),
            (KeyCode::Char('r'), true, _) => Some(Accion::RenombrarSimbolo),
            (KeyCode::Char('p'), true, _) => Some(Accion::PreviewMarkdown),
            (KeyCode::PageDown, true, _) => Some(Accion::SiguienteBuffer),
            (KeyCode::PageUp, true, _) => Some(Accion::AnteriorBuffer),
            (KeyCode::F(1), _, _) => Some(Accion::ExplicarRegla),
            _ => None,
        };
        // Pestaña de un binario: las teclas van al visor hexadecimal. Ctrl+F y Ctrl+G
        // buscan y van a un offset; cerrar, abrir y cambiar de pestaña siguen andando.
        if self.buffers[self.activo].hex.is_some() {
            let idx = self.activo;
            match accion {
                Some(Accion::Buscar | Accion::IrALinea) => {
                    if let Some(v) = &mut self.buffers[idx].hex {
                        v.entrada = Some((accion == Some(Accion::IrALinea), String::new()));
                    }
                }
                Some(
                    a @ (Accion::CerrarBuffer
                    | Accion::Abrir
                    | Accion::SiguienteBuffer
                    | Accion::AnteriorBuffer
                    | Accion::ExplicarRegla
                    | Accion::GuardarTodo),
                ) => self.accion(a),
                Some(_) => {}
                None if alt => {
                    if k.code == KeyCode::Left {
                        self.accion(Accion::Volver);
                    }
                }
                None => {
                    if let Some(mut v) = self.buffers[idx].hex.take() {
                        self.tecla_hex(&mut v, k);
                        if let Some(b) = self.buffers.get_mut(idx) {
                            b.hex = Some(v);
                        }
                    }
                }
            }
            return;
        }
        if let Some(a) = accion {
            self.accion(a);
            return;
        }
        if alt {
            match k.code {
                KeyCode::Up => self.accion(Accion::SubirLinea),
                KeyCode::Enter => self.accion(Accion::ArregloRapido),
                KeyCode::Down => self.accion(Accion::BajarLinea),
                KeyCode::Left => self.accion(Accion::Volver),
                _ => {}
            }
            return;
        }
        let b = &mut self.buffers[self.activo].buf;
        match k.code {
            KeyCode::Left if ctrl => b.palabra_izquierda(shift),
            KeyCode::Right if ctrl => b.palabra_derecha(shift),
            KeyCode::Left => b.izquierda(shift),
            KeyCode::Right => b.derecha(shift),
            KeyCode::Up => b.vertical(-1, shift),
            KeyCode::Down => b.vertical(1, shift),
            KeyCode::PageUp => {
                b.vertical(-alto, shift);
                b.desplazamiento = b.desplazamiento.saturating_sub(alto as usize);
            }
            KeyCode::PageDown => {
                b.vertical(alto, shift);
                b.desplazamiento += alto as usize;
            }
            KeyCode::Home if ctrl => b.mover_a(0, shift),
            KeyCode::End if ctrl => {
                let n = b.largo();
                b.mover_a(n, shift)
            }
            KeyCode::Home => b.inicio_linea(shift),
            KeyCode::End => b.fin_linea(shift),
            KeyCode::Esc => {
                b.ancla = None;
                if self.depuracion.is_some() && self.foco == Foco::Editor {
                    // nada: Esc no detiene la depuración
                }
            }
            KeyCode::Enter => self.editar(|b| b.nueva_linea()),
            KeyCode::Backspace => self.editar(|b| b.borrar_atras()),
            KeyCode::Delete => self.editar(|b| b.borrar_adelante()),
            // En un Makefile las recetas exigen un TAB real; en el resto, 4 espacios.
            KeyCode::Tab => self.editar(|b| {
                let t = if b.lenguaje == Lenguaje::Makefile {
                    "\t"
                } else {
                    "    "
                };
                b.insertar(t)
            }),
            KeyCode::BackTab => self.editar(desindentar),
            // Lo tipeado (no lo pegado): cierra pares y desindenta `}` (en C).
            KeyCode::Char(c) if !ctrl => self.editar(|b| b.escribir(c)),
            _ => {}
        }
    }
}
