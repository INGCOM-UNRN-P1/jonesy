//! Teclas de los modales: un método por modal, que despacha `tecla_modal` (N-JONESY-02).

use super::*;

impl App {
    pub(super) fn tecla_modal(&mut self, k: KeyEvent) {
        let Some(modal) = self.modal.take() else {
            return;
        };
        match modal {
            m @ Modal::Paleta { .. } => self.tecla_paleta(m, k),
            m @ Modal::Abrir { .. } => self.tecla_abrir(m, k),
            m @ Modal::Buscar { .. } => self.tecla_buscar(m, k),
            m @ Modal::IrALinea { .. } => self.tecla_ir_a_linea(m, k),
            m @ Modal::Confirmar { .. } => self.tecla_confirmar(m, k),
            m @ Modal::Commit { .. } => self.tecla_commit(m, k),
            m @ Modal::Visor { .. } => self.tecla_visor(m, k),
            m @ Modal::Argumentos { .. } => self.tecla_argumentos(m, k),
            m @ Modal::Renombrar { .. } => self.tecla_renombrar(m, k),
            m @ Modal::RenombrarSimbolo { .. } => self.tecla_renombrar_simbolo(m, k),
            m @ Modal::Completar { .. } => self.tecla_completar(m, k),
            m @ Modal::RutaPreferencias { .. } => self.tecla_ruta_preferencias(m, k),
            m @ Modal::Vigilar { .. } => self.tecla_vigilar(m, k),
            m @ Modal::CodigoPausa { .. } => self.tecla_codigo_pausa(m, k),
            m @ Modal::NuevoArchivo { .. } => self.tecla_nuevo_archivo(m, k),
            m @ Modal::Clonar { .. } => self.tecla_clonar(m, k),
            m @ Modal::AbrirProyecto { .. } => self.tecla_abrir_proyecto(m, k),
            m @ Modal::Diff { .. } => self.tecla_diff(m, k),
            m @ Modal::Recorrido { .. } => self.tecla_recorrido(m, k),
            m @ Modal::Simulacro { .. } => self.tecla_simulacro(m, k),
            m @ Modal::ArgsPrograma { .. } => self.tecla_args_programa(m, k),
            m @ Modal::Casos { .. } => self.tecla_casos(m, k),
            m @ Modal::BuscarProyecto { .. } => self.tecla_buscar_proyecto(m, k),
            m @ Modal::Reemplazar { .. } => self.tecla_reemplazar(m, k),
            m @ Modal::Bienvenida { .. } => self.tecla_bienvenida(m, k),
            m @ Modal::Configuracion { .. } => self.tecla_configuracion(m, k),
            m @ Modal::Paneles { .. } => self.tecla_paneles(m, k),
            m @ Modal::Menu { .. } => self.tecla_menu(m, k),
        }
    }

    fn tecla_paleta(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::Paleta {
            mut filtro,
            mut sel,
            opciones,
        } = modal
        else {
            return;
        };
        let cerrar = k.code == KeyCode::Esc;
        if cerrar {
            return;
        }
        let filtradas = paleta::filtrar(
            &filtro,
            &opciones
                .iter()
                .map(|o| o.titulo.as_str())
                .collect::<Vec<_>>(),
        );
        match k.code {
            KeyCode::Enter => {
                if let Some(o) = filtradas.get(sel).map(|&i| opciones[i].clone()) {
                    self.ejecutar_opcion(o.que);
                }
                return;
            }
            KeyCode::Up => sel = sel.saturating_sub(1),
            KeyCode::Down => sel = (sel + 1).min(filtradas.len().saturating_sub(1)),
            _ => {
                if editar_campo(&mut filtro, &k) {
                    sel = 0;
                }
            }
        }
        self.modal = Some(Modal::Paleta {
            filtro,
            sel,
            opciones,
        });
    }

    fn tecla_abrir(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::Abrir {
            mut filtro,
            mut sel,
            archivos,
            mut marcados,
        } = modal
        else {
            return;
        };
        let cerrar = k.code == KeyCode::Esc;
        if cerrar {
            return;
        }
        let filtrados = paleta::filtrar(&filtro, &archivos);
        match k.code {
            KeyCode::Enter => {
                // Con archivos marcados se abren todos (una pestaña cada uno).
                let rels: Vec<String> = if marcados.is_empty() {
                    vec![filtrados
                        .get(sel)
                        .map(|&i| archivos[i].clone())
                        .unwrap_or_else(|| filtro.clone())]
                } else {
                    marcados.iter().map(|&i| archivos[i].clone()).collect()
                };
                for rel in rels {
                    let rel = rel.trim();
                    if rel.is_empty() {
                        continue;
                    }
                    let ruta = self.proyecto.raiz.join(rel);
                    if self.examen.is_some() && !ruta.starts_with(&self.proyecto.raiz) {
                        continue;
                    }
                    self.abrir_archivo(&ruta, true);
                }
                return;
            }
            KeyCode::Tab => {
                if let Some(&i) = filtrados.get(sel) {
                    if !marcados.remove(&i) {
                        marcados.insert(i);
                    }
                    sel = (sel + 1).min(filtrados.len().saturating_sub(1));
                }
            }
            KeyCode::Up => sel = sel.saturating_sub(1),
            KeyCode::Down => sel = (sel + 1).min(filtrados.len().saturating_sub(1)),
            _ => {
                if editar_campo(&mut filtro, &k) {
                    sel = 0;
                }
            }
        }
        self.modal = Some(Modal::Abrir {
            filtro,
            sel,
            archivos,
            marcados,
        });
    }

    fn tecla_buscar(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::Buscar { mut texto } = modal else {
            return;
        };
        let cerrar = k.code == KeyCode::Esc;
        if cerrar {
            return;
        }
        if k.code == KeyCode::Enter {
            self.busqueda = texto.clone();
            self.buscar_siguiente();
            // Enter repetido busca la siguiente; Esc cierra.
            if !matches!(self.modal, Some(Modal::Buscar { .. })) {
                self.modal = Some(Modal::Buscar { texto });
            }
            return;
        }
        editar_campo(&mut texto, &k);
        self.modal = Some(Modal::Buscar { texto });
    }

    fn tecla_ir_a_linea(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::IrALinea { mut texto } = modal else {
            return;
        };
        let cerrar = k.code == KeyCode::Esc;
        if cerrar {
            return;
        }
        if k.code == KeyCode::Enter {
            if let Ok(n) = texto.trim().parse::<usize>() {
                let b = &mut self.buf_mut().buf;
                let pos = b.linea_columna_a_pos(n.saturating_sub(1), 0);
                b.mover_a(pos, false);
                centrar(b, 10);
            }
            return;
        }
        if let KeyCode::Char(c) = k.code {
            if !c.is_ascii_digit() {
                self.modal = Some(Modal::IrALinea { texto });
                return;
            }
        }
        editar_campo(&mut texto, &k);
        self.modal = Some(Modal::IrALinea { texto });
    }

    fn tecla_confirmar(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::Confirmar {
            titulo,
            mensaje,
            que,
        } = modal
        else {
            return;
        };
        match k.code {
            KeyCode::Char('s' | 'S' | 'y' | 'Y') | KeyCode::Enter => match que {
                Confirmacion::Salir => self.salir = true,
                Confirmacion::CerrarBuffer => self.cerrar_buffer(true),
                Confirmacion::Entregar => self.entregar(),
                Confirmacion::Recuperar => self.recuperar(true),
                Confirmacion::Borrar => self.borrar_ruta(),
                Confirmacion::DeshacerCommit => self.deshacer_commit(),
                Confirmacion::Descartar => self.descartar_cambios(),
                Confirmacion::AbrirClonado => {
                    if let Some(p) = self.proyecto_pendiente.take() {
                        self.cambiar_proyecto(p);
                    }
                }
            },
            KeyCode::Char('n' | 'N') => {
                if que == Confirmacion::Recuperar {
                    self.recuperar(false);
                }
            }
            // Esc cierra sin decidir: las copias de recuperación quedan para la próxima.
            KeyCode::Esc => {}
            _ => {
                self.modal = Some(Modal::Confirmar {
                    titulo,
                    mensaje,
                    que,
                })
            }
        }
    }

    fn tecla_commit(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::Commit { mut mensaje } = modal else {
            return;
        };
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let alt = k.modifiers.contains(KeyModifiers::ALT);
        let cerrar = k.code == KeyCode::Esc;
        if cerrar {
            return;
        }
        let confirmar =
            (k.code == KeyCode::Enter && (ctrl || alt)) || (k.code == KeyCode::Char('s') && ctrl);
        if confirmar {
            self.modal = Some(Modal::Commit {
                mensaje: mensaje.clone(),
            });
            self.git_commit(&mensaje);
            return;
        }
        if k.code == KeyCode::Enter {
            mensaje.push('\n');
        } else {
            editar_campo(&mut mensaje, &k);
        }
        self.modal = Some(Modal::Commit { mensaje });
    }

    fn tecla_visor(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::Visor {
            titulo,
            lineas,
            mut desplazamiento,
        } = modal
        else {
            return;
        };
        let alto = self.tam.1.saturating_sub(8) as usize;
        match k.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => return,
            // El glosario, empezando por los términos que menciona esta ventana.
            KeyCode::Char('g') => {
                self.abrir_glosario();
                return;
            }
            KeyCode::Up => desplazamiento = desplazamiento.saturating_sub(1),
            KeyCode::Down => desplazamiento += 1,
            KeyCode::PageUp => desplazamiento = desplazamiento.saturating_sub(alto),
            KeyCode::PageDown | KeyCode::Char(' ') => desplazamiento += alto,
            KeyCode::Home => desplazamiento = 0,
            _ => {}
        }
        let desplazamiento = desplazamiento.min(lineas.len().saturating_sub(1));
        self.modal = Some(Modal::Visor {
            titulo,
            lineas,
            desplazamiento,
        });
    }

    fn tecla_argumentos(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::Argumentos {
            titulo,
            comando,
            mut texto,
        } = modal
        else {
            return;
        };
        let cerrar = k.code == KeyCode::Esc;
        if cerrar {
            return;
        }
        if k.code == KeyCode::Enter {
            self.refrescar_estructura = true;
            let completo = format!("{comando} {}", texto.trim()).trim_end().to_string();
            self.correr_en_deck(completo);
            return;
        }
        editar_campo(&mut texto, &k);
        self.modal = Some(Modal::Argumentos {
            titulo,
            comando,
            texto,
        });
    }

    fn tecla_renombrar(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::Renombrar { origen, mut texto } = modal else {
            return;
        };
        let cerrar = k.code == KeyCode::Esc;
        if cerrar {
            return;
        }
        if k.code == KeyCode::Enter {
            self.renombrar(&origen, &texto);
            return;
        }
        editar_campo(&mut texto, &k);
        self.modal = Some(Modal::Renombrar { origen, texto });
    }

    fn tecla_renombrar_simbolo(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::RenombrarSimbolo {
            plan, mut texto, ..
        } = modal
        else {
            return;
        };
        let cerrar = k.code == KeyCode::Esc;
        if cerrar {
            return;
        }
        if k.code == KeyCode::Enter {
            self.renombrar_simbolo(plan, texto);
            return;
        }
        editar_campo(&mut texto, &k);
        let error = (!texto.is_empty() && texto != plan.nombre)
            .then(|| {
                jonesy_core::renombrar::validar(&self.buf().buf.contenido(), &plan, &texto).err()
            })
            .flatten();
        self.modal = Some(Modal::RenombrarSimbolo { plan, texto, error });
    }

    fn tecla_completar(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::Completar {
            inicio,
            opciones,
            mut sel,
        } = modal
        else {
            return;
        };
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let alt = k.modifiers.contains(KeyModifiers::ALT);
        let n = opciones.len();
        match k.code {
            KeyCode::Esc => {}
            KeyCode::Up => {
                sel = (sel + n - 1) % n;
                self.modal = Some(Modal::Completar {
                    inicio,
                    opciones,
                    sel,
                });
            }
            KeyCode::Down => {
                sel = (sel + 1) % n;
                self.modal = Some(Modal::Completar {
                    inicio,
                    opciones,
                    sel,
                });
            }
            KeyCode::Enter | KeyCode::Tab => {
                let texto = opciones[sel].texto.clone();
                let cursor = self.buf().buf.cursor;
                self.editar(|b| b.reemplazar_rango(inicio, cursor, &texto));
            }
            // Seguir escribiendo filtra la lista.
            KeyCode::Char(c) if !ctrl && !alt && (c.is_alphanumeric() || c == '_') => {
                self.editar(|b| b.escribir(c));
                self.autocompletar(false);
            }
            KeyCode::Backspace => {
                self.editar(|b| b.borrar_atras());
                if self.buf().buf.cursor > inicio {
                    self.autocompletar(false);
                }
            }
            // Cualquier otra tecla cierra la lista y hace lo suyo en el editor.
            _ => self.tecla_editor(k),
        }
    }

    fn tecla_ruta_preferencias(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::RutaPreferencias {
            importar,
            mut texto,
        } = modal
        else {
            return;
        };
        let cerrar = k.code == KeyCode::Esc;
        if cerrar {
            return;
        }
        if k.code == KeyCode::Enter {
            let ruta = self.ruta_usuario_a_path(&texto);
            if importar {
                self.importar_preferencias(&ruta);
            } else {
                self.exportar_preferencias(&ruta);
            }
            return;
        }
        editar_campo(&mut texto, &k);
        self.modal = Some(Modal::RutaPreferencias { importar, texto });
    }

    fn tecla_vigilar(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::Vigilar { mut texto } = modal else {
            return;
        };
        let cerrar = k.code == KeyCode::Esc;
        if cerrar {
            return;
        }
        if k.code == KeyCode::Enter {
            self.alternar_vigilada(texto.trim());
            return;
        }
        editar_campo(&mut texto, &k);
        self.modal = Some(Modal::Vigilar { texto });
    }

    fn tecla_codigo_pausa(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::CodigoPausa { mut texto } = modal else {
            return;
        };
        let cerrar = k.code == KeyCode::Esc;
        if cerrar {
            return;
        }
        if k.code == KeyCode::Enter {
            self.pausar_examen(&texto);
            return;
        }
        editar_campo(&mut texto, &k);
        self.modal = Some(Modal::CodigoPausa { texto });
    }

    fn tecla_nuevo_archivo(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::NuevoArchivo { mut texto, carpeta } = modal else {
            return;
        };
        let cerrar = k.code == KeyCode::Esc;
        if cerrar {
            return;
        }
        if k.code == KeyCode::Enter {
            self.crear_archivo(&texto, carpeta);
            return;
        }
        editar_campo(&mut texto, &k);
        self.modal = Some(Modal::NuevoArchivo { texto, carpeta });
    }

    fn tecla_clonar(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::Clonar {
            mut url,
            mut destino,
            mut campo,
        } = modal
        else {
            return;
        };
        let cerrar = k.code == KeyCode::Esc;
        if cerrar {
            return;
        }
        match k.code {
            KeyCode::Tab | KeyCode::BackTab | KeyCode::Up | KeyCode::Down => campo = 1 - campo,
            KeyCode::Enter if campo == 0 => campo = 1,
            KeyCode::Enter => {
                self.clonar(&url, &destino);
                return;
            }
            _ => {
                editar_campo(if campo == 0 { &mut url } else { &mut destino }, &k);
            }
        }
        self.modal = Some(Modal::Clonar {
            url,
            destino,
            campo,
        });
    }

    fn tecla_abrir_proyecto(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::AbrirProyecto {
            dir,
            mut filtro,
            mut sel,
            entradas,
        } = modal
        else {
            return;
        };
        let cerrar = k.code == KeyCode::Esc;
        if cerrar {
            return;
        }
        let etiquetas: Vec<&str> = entradas.iter().map(|e| e.etiqueta.as_str()).collect();
        let visibles = if filtro.is_empty() {
            (0..entradas.len()).collect()
        } else {
            paleta::filtrar(&filtro, &etiquetas)
        };
        match k.code {
            KeyCode::Up => sel = sel.saturating_sub(1),
            KeyCode::Down => sel = (sel + 1).min(visibles.len().saturating_sub(1)),
            KeyCode::Enter => {
                // Una ruta escrita a mano (~/tp, /home/...) se abre directo.
                let escrita = expandir_ruta(&filtro, &dir);
                if let Some(r) = escrita.filter(|r| r.is_dir()) {
                    self.cambiar_proyecto(r);
                    return;
                }
                let Some(e) = visibles.get(sel).map(|&i| entradas[i].clone()) else {
                    self.modal = Some(Modal::AbrirProyecto {
                        dir,
                        filtro,
                        sel,
                        entradas,
                    });
                    return;
                };
                if e.abrir {
                    self.cambiar_proyecto(e.destino);
                    return;
                }
                self.modal = Some(Modal::AbrirProyecto {
                    entradas: entradas_dir(&e.destino, false),
                    dir: e.destino,
                    filtro: String::new(),
                    sel: 0,
                });
                return;
            }
            KeyCode::Backspace if filtro.is_empty() => {
                if let Some(p) = dir.parent().map(Path::to_path_buf) {
                    self.modal = Some(Modal::AbrirProyecto {
                        entradas: entradas_dir(&p, false),
                        dir: p,
                        filtro,
                        sel: 0,
                    });
                    return;
                }
            }
            _ => {
                if editar_campo(&mut filtro, &k) {
                    sel = 0;
                }
            }
        }
        self.modal = Some(Modal::AbrirProyecto {
            dir,
            filtro,
            sel,
            entradas,
        });
    }

    fn tecla_diff(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::Diff {
            titulo,
            filas,
            mut desplazamiento,
            mut lado_a_lado,
            mut desplazamiento_h,
            lenguaje,
        } = modal
        else {
            return;
        };
        let alto = self.tam.1.saturating_sub(6) as usize;
        let cambios = jonesy_core::diff::inicios_de_cambios(&filas);
        match k.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => return,
            KeyCode::Up => desplazamiento = desplazamiento.saturating_sub(1),
            KeyCode::Down => desplazamiento += 1,
            KeyCode::PageUp => desplazamiento = desplazamiento.saturating_sub(alto),
            KeyCode::PageDown | KeyCode::Char(' ') => desplazamiento += alto,
            KeyCode::Home => {
                desplazamiento = 0;
                desplazamiento_h = 0;
            }
            KeyCode::End => desplazamiento = filas.len(),
            KeyCode::Left => desplazamiento_h = desplazamiento_h.saturating_sub(8),
            KeyCode::Right => desplazamiento_h += 8,
            KeyCode::Char('n') => {
                if let Some(&i) = cambios.iter().find(|&&i| i > desplazamiento) {
                    desplazamiento = i;
                }
            }
            KeyCode::Char('p') => {
                if let Some(&i) = cambios.iter().rev().find(|&&i| i < desplazamiento) {
                    desplazamiento = i;
                }
            }
            KeyCode::Char('s') | KeyCode::Char('u') | KeyCode::Tab => {
                lado_a_lado = !lado_a_lado;
                self.preferir_lado_a_lado = lado_a_lado;
            }
            _ => {}
        }
        let desplazamiento = desplazamiento.min(filas.len().saturating_sub(1));
        self.modal = Some(Modal::Diff {
            titulo,
            filas,
            desplazamiento,
            lado_a_lado,
            desplazamiento_h,
            lenguaje,
        });
    }

    fn tecla_recorrido(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::Recorrido { mut paso } = modal else {
            return;
        };
        let n = crate::ayuda::RECORRIDO.len();
        match k.code {
            KeyCode::Esc => return,
            KeyCode::Enter | KeyCode::Right | KeyCode::Char(' ') => {
                if paso + 1 >= n {
                    return;
                }
                paso += 1;
            }
            KeyCode::Left | KeyCode::Backspace => paso = paso.saturating_sub(1),
            _ => {}
        }
        self.modal = Some(Modal::Recorrido { paso });
    }

    fn tecla_simulacro(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::Simulacro { mut texto } = modal else {
            return;
        };
        let cerrar = k.code == KeyCode::Esc;
        if cerrar {
            return;
        }
        if k.code == KeyCode::Enter {
            match texto.trim().parse::<u64>() {
                Ok(m) if m > 0 => self.empezar_simulacro(m),
                _ => self.avisar("Poné los minutos (por ejemplo 90).", Nivel::Aviso),
            }
            return;
        }
        if let KeyCode::Char(c) = k.code {
            if c.is_ascii_digit() {
                texto.push(c);
            }
        } else {
            editar_campo(&mut texto, &k);
        }
        self.modal = Some(Modal::Simulacro { texto });
    }

    fn tecla_args_programa(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::ArgsPrograma { mut texto } = modal else {
            return;
        };
        let cerrar = k.code == KeyCode::Esc;
        if cerrar {
            return;
        }
        if k.code == KeyCode::Enter {
            if let Some(c) = self.clave_ejercicio() {
                if texto.trim().is_empty() {
                    self.argumentos.remove(&c);
                } else {
                    self.argumentos.insert(c, texto.trim().to_string());
                }
                self.guardar_sesion();
                self.avisar(
                    "Argumentos guardados para este ejercicio (Ctrl+F5 y F5).",
                    Nivel::Ok,
                );
            }
            return;
        }
        editar_campo(&mut texto, &k);
        self.modal = Some(Modal::ArgsPrograma { texto });
    }

    fn tecla_casos(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::Casos {
            resultados,
            mut sel,
        } = modal
        else {
            return;
        };
        let n = resultados.len().max(1);
        match k.code {
            KeyCode::Esc => return,
            KeyCode::Up => sel = (sel + n - 1) % n,
            KeyCode::Down => sel = (sel + 1) % n,
            KeyCode::Enter => {
                if let Some(r) = resultados.get(sel) {
                    let esperada = jonesy_core::casos::normalizar(&r.esperada);
                    let obtenida = jonesy_core::casos::normalizar(&r.corrida.salida);
                    self.modal = Some(Modal::Diff {
                        titulo: format!(
                            "{}: salida esperada ⟷ obtenida (sin espacios al final)",
                            r.caso.nombre
                        ),
                        filas: jonesy_core::diff::comparar(&esperada, &obtenida, 1000),
                        desplazamiento: 0,
                        lado_a_lado: true,
                        desplazamiento_h: 0,
                        lenguaje: Lenguaje::Texto,
                    });
                    return;
                }
            }
            _ => {}
        }
        self.modal = Some(Modal::Casos { resultados, sel });
    }

    fn tecla_buscar_proyecto(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::BuscarProyecto {
            mut texto,
            mut mayusculas,
            mut palabra,
        } = modal
        else {
            return;
        };
        let alt = k.modifiers.contains(KeyModifiers::ALT);
        let cerrar = k.code == KeyCode::Esc;
        if cerrar {
            return;
        }
        match (k.code, alt) {
            (KeyCode::Char('m' | 'M'), true) => mayusculas = !mayusculas,
            (KeyCode::Char('p' | 'P'), true) => palabra = !palabra,
            (KeyCode::Enter, _) => {
                self.buscar_en_proyecto(&texto, mayusculas, palabra);
                return;
            }
            _ => {
                editar_campo(&mut texto, &k);
            }
        }
        self.modal = Some(Modal::BuscarProyecto {
            texto,
            mayusculas,
            palabra,
        });
    }

    fn tecla_reemplazar(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::Reemplazar {
            mut buscar,
            mut reemplazo,
            mut campo,
            mut mayusculas,
            mut palabra,
        } = modal
        else {
            return;
        };
        let alt = k.modifiers.contains(KeyModifiers::ALT);
        let cerrar = k.code == KeyCode::Esc;
        if cerrar {
            return;
        }
        match (k.code, alt) {
            (KeyCode::Tab | KeyCode::BackTab, _) => campo = 1 - campo,
            (KeyCode::Char('m' | 'M'), true) => mayusculas = !mayusculas,
            (KeyCode::Char('p' | 'P'), true) => palabra = !palabra,
            (KeyCode::Char('t' | 'T'), true) => {
                let (b, r) = (buscar.clone(), reemplazo.clone());
                let mut n = 0;
                self.editar(|buf| {
                    buf.reemplazar_todas(&b, &r, mayusculas, palabra)
                        .map(|(d, k)| {
                            n = k;
                            d
                        })
                });
                self.avisar(
                    if n == 0 {
                        format!("No se encontró «{buscar}».")
                    } else {
                        format!("{n} reemplazo(s) de «{buscar}» por «{reemplazo}».")
                    },
                    if n == 0 { Nivel::Aviso } else { Nivel::Ok },
                );
            }
            (KeyCode::Enter, _) => {
                // Si la coincidencia elegida es la actual, se reemplaza.
                let b = &self.buf().buf;
                let actual = b
                    .seleccion()
                    .filter(|s| b.coincidencias(&buscar, mayusculas, palabra).contains(s));
                if let Some((a, z)) = actual {
                    let r = reemplazo.clone();
                    self.editar(|buf| buf.reemplazar_rango(a, z, &r));
                }
                self.siguiente_coincidencia(&buscar, mayusculas, palabra);
            }
            (KeyCode::F(3), _) => {
                self.siguiente_coincidencia(&buscar, mayusculas, palabra);
            }
            _ => {
                let c = if campo == 0 {
                    &mut buscar
                } else {
                    &mut reemplazo
                };
                editar_campo(c, &k);
            }
        }
        self.modal = Some(Modal::Reemplazar {
            buscar,
            reemplazo,
            campo,
            mayusculas,
            palabra,
        });
    }

    fn tecla_bienvenida(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::Bienvenida { siempre, faltan } = modal else {
            return;
        };
        match k.code {
            KeyCode::Char(' ') => {
                self.modal = Some(Modal::Bienvenida {
                    siempre: !siempre,
                    faltan,
                })
            }
            KeyCode::Char('g' | 'G') => {
                jonesy_core::sesion::bienvenida::recordar(siempre);
                self.accion(Accion::GuiaRapida);
            }
            KeyCode::Char('d' | 'D') => {
                jonesy_core::sesion::bienvenida::recordar(siempre);
                self.accion(Accion::Doctor);
            }
            KeyCode::Char('r' | 'R') => {
                jonesy_core::sesion::bienvenida::recordar(siempre);
                self.modal = Some(Modal::Recorrido { paso: 0 });
            }
            KeyCode::Enter | KeyCode::Esc => jonesy_core::sesion::bienvenida::recordar(siempre),
            _ => {
                self.modal = Some(Modal::Bienvenida { siempre, faltan });
            }
        }
    }

    fn tecla_configuracion(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::Configuracion {
            mut pestana,
            mut sel,
        } = modal
        else {
            return;
        };
        let filas = self.filas_configuracion(pestana);
        let n = filas.len().max(1);
        match k.code {
            KeyCode::Esc => {
                self.guardar_sesion();
                return;
            }
            KeyCode::Left | KeyCode::BackTab => {
                pestana = (pestana + PESTANAS_CONFIG.len() - 1) % PESTANAS_CONFIG.len();
                sel = 0;
            }
            KeyCode::Right | KeyCode::Tab => {
                pestana = (pestana + 1) % PESTANAS_CONFIG.len();
                sel = 0;
            }
            KeyCode::Up => sel = (sel + n - 1) % n,
            KeyCode::Down => sel = (sel + 1) % n,
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(f) = filas.get(sel) {
                    // Si la acción abre otra cosa, la configuración se cierra.
                    if !self.aplicar_config(f.que.clone()) {
                        self.guardar_sesion();
                        return;
                    }
                }
            }
            KeyCode::Char(c @ '1'..='4') => {
                pestana = c as usize - '1' as usize;
                sel = 0;
            }
            _ => {}
        }
        self.modal = Some(Modal::Configuracion { pestana, sel });
    }

    fn tecla_paneles(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::Paneles { mut sel } = modal else {
            return;
        };
        let n = CAMPOS_PANELES.len();
        match k.code {
            KeyCode::Esc | KeyCode::Enter => {
                self.guardar_sesion();
                return;
            }
            KeyCode::Up => sel = (sel + n - 1) % n,
            KeyCode::Down => sel = (sel + 1) % n,
            KeyCode::Left | KeyCode::Char('-') => ajustar_panel(&mut self.paneles, sel, -1),
            KeyCode::Right | KeyCode::Char('+') => ajustar_panel(&mut self.paneles, sel, 1),
            KeyCode::Char('r') => self.paneles = self.proyecto.config.interfaz.paneles.acotar(),
            _ => {}
        }
        self.modal = Some(Modal::Paneles { sel });
    }

    fn tecla_menu(&mut self, modal: Modal, k: KeyEvent) {
        let Modal::Menu {
            mut indice,
            mut sel,
        } = modal
        else {
            return;
        };
        let menus = self.menus();
        let items = &menus[indice].items;
        sel = accion::ajustar_sel(items, sel);
        match k.code {
            KeyCode::Esc | KeyCode::F(10) => return,
            KeyCode::Left => {
                indice = (indice + menus.len() - 1) % menus.len();
                sel = 0;
            }
            KeyCode::Right => {
                indice = (indice + 1) % menus.len();
                sel = 0;
            }
            // Los separadores no se eligen.
            KeyCode::Up => {
                if let Some(i) = (0..sel).rev().find(|&i| items[i].elegible()) {
                    sel = i;
                }
            }
            KeyCode::Down => {
                if let Some(i) = (sel + 1..items.len()).find(|&i| items[i].elegible()) {
                    sel = i;
                }
            }
            KeyCode::Enter => {
                if let Some(e) = items.get(sel).filter(|e| e.elegible()).cloned() {
                    self.elegir_entrada(e);
                    return;
                }
            }
            // La letra subrayada de otro menú lo abre (F10 y letra).
            KeyCode::Char(c) => {
                let c = c.to_ascii_lowercase();
                if let Some(i) = menus.iter().position(|m| m.tecla == c) {
                    indice = i;
                    sel = 0;
                }
            }
            _ => {}
        }
        self.abrir_menu(indice);
        if let Some(Modal::Menu { sel: s, .. }) = &mut self.modal {
            *s = sel;
        }
    }

    pub(super) fn ejecutar_opcion(&mut self, q: QuePaleta) {
        match q {
            QuePaleta::Accion(a) => self.accion(a),
            QuePaleta::Objetivo(m, o) => self.correr_objetivo(m, &o),
            QuePaleta::Hex(r) => self.abrir_hex(&r),
            QuePaleta::Tp(i) => self.correr_tp(i),
            QuePaleta::Herramienta(nombre, n) => self.correr_accion_herramienta(&nombre, n),
            QuePaleta::Entrada(e) => {
                if let Some(k) = self.clave_ejercicio() {
                    match &e {
                        Some(p) => {
                            self.entradas.insert(k, p.clone());
                        }
                        None => {
                            self.entradas.remove(&k);
                        }
                    }
                    self.guardar_sesion();
                }
                self.avisar(
                    match e {
                        Some(p) => format!(
                            "Ctrl+F5 va a leer la entrada de {}.",
                            self.proyecto.relativa(&p)
                        ),
                        None => "Ctrl+F5 lee la entrada del teclado.".into(),
                    },
                    Nivel::Ok,
                );
            }
            QuePaleta::Regla(i) => {
                if let Some(r) = self.catalogo.as_ref().and_then(|c| c.get(i)).cloned() {
                    let texto = jonesy_core::catalogo::markdown(&r, None);
                    self.visor_explicacion(format!("Regla {}", r.codigo), &texto);
                }
            }
            QuePaleta::Glosario(i) => {
                let t = jonesy_core::glosario::terminos()[i].termino.clone();
                self.visor_explicacion(
                    format!("Glosario: {t}"),
                    &jonesy_core::glosario::markdown(i),
                );
                // Desde una ficha, g vuelve al glosario completo.
                self.glosario_visor.clear();
            }
            QuePaleta::Linea(p, l) => {
                self.recordar_posicion();
                self.ir_a(&p, l, 1);
                self.cambiar_foco(Foco::Editor);
            }
            QuePaleta::Plugin(c) => {
                if let Some(p) = &self.plugins {
                    let b = &self.buf().buf;
                    let (l, col) = b.linea_columna();
                    let ctx = Contexto {
                        archivo: b
                            .ruta
                            .as_ref()
                            .map(|r| r.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                        texto: b.contenido(),
                        linea: l + 1,
                        columna: col + 1,
                    };
                    self.tel(TelemetryEvent::CommandExec {
                        command: format!("plugin:{}", c.titulo()),
                        exit_code: None,
                    });
                    p.pedir(PedidoPlugin::Ejecutar { comando: c, ctx });
                }
            }
        }
    }
}
