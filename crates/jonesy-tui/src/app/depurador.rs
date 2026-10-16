//! Depurador sobre GDB/MI: iniciar, pasos y eventos (N-JONESY-02).

use super::*;

impl App {
    pub(super) fn depurar(&mut self) {
        if let Some(d) = &mut self.depuracion {
            if !d.gdb.corriendo {
                let _ = d.gdb.continuar();
            }
            return;
        }
        self.compilar(Luego::Depurar);
    }

    pub(super) fn programa_a_depurar(&self) -> Option<PathBuf> {
        let activo = self.activo_ruta();
        self.proyecto
            .programa_a_depurar(activo.as_deref())
            .filter(|p| p.is_file())
    }

    pub(super) fn iniciar_depuracion(&mut self) {
        let Some(programa) = self.programa_a_depurar() else {
            self.avisar(
                "No encontré el ejecutable: configurá [depurar] programa en .jonesy.toml.",
                Nivel::Error,
            );
            return;
        };
        let (filas, cols) = self.tam_panel_inferior();
        let cols_pty = (cols / 2).max(20);
        let tx = self.tx.clone();
        let pty = match PtyInferior::abrir(filas, cols_pty, move || {
            let _ = tx.send(Aviso::Despertar);
        }) {
            Ok(p) => Some(p),
            // Sin PTY separado (Windows) la E/S del programa aparece en la consola de GDB.
            Err(e) if e.kind() == std::io::ErrorKind::Unsupported => None,
            Err(e) => {
                self.avisar(
                    format!("No se pudo crear el PTY del programa: {e}"),
                    Nivel::Error,
                );
                return;
            }
        };
        let tx = self.tx.clone();
        // Los argumentos recordados del ejercicio, si no hay en .jonesy.toml.
        let args = if self.proyecto.config.depurar.argumentos.is_empty() {
            self.argumentos_de_ejercicio()
        } else {
            self.proyecto.config.depurar.argumentos.clone()
        };
        let gdbinit = jonesy_core::entorno::detectar(&self.proyecto.config.interfaz.entorno)
            .and_then(|e| e.gdbinit);
        // Después de un corte, sin breakpoints, se corre de largo hasta el fallo.
        let hasta_el_fallo = self.ultimo_fallo.take().is_some() && self.breakpoints.is_empty();
        let mut gdb = match Gdb::lanzar_con(
            &programa,
            &args,
            &self.proyecto.raiz,
            pty.as_ref().map(|p| p.ruta()),
            gdbinit.as_deref(),
            move || {
                let _ = tx.send(Aviso::Despertar);
            },
        ) {
            Ok(g) => g,
            Err(e) => {
                self.avisar(format!("No se pudo lanzar gdb: {e}"), Nivel::Error);
                return;
            }
        };
        if let Some((archivo, linea)) = self.hasta_linea.take() {
            let _ = gdb.poner_breakpoint_temporal(&archivo, linea);
        } else if self.breakpoints.is_empty() && !hasta_el_fallo {
            let _ = gdb.enviar("-break-insert -t main", jonesy_core::gdb::Pedido::Otro);
        }
        for (archivo, linea) in &self.breakpoints {
            let _ = gdb.poner_breakpoint(archivo, *linea);
        }
        let _ = gdb.correr();
        self.tel(TelemetryEvent::CommandExec {
            command: format!("gdb {}", self.proyecto.relativa(&programa)),
            exit_code: None,
        });
        self.depuracion = Some(Depuracion {
            gdb,
            pty,
            marcos: vec![],
            variables: vec![],
            ubicacion: None,
            consola: vec![],
            entrada: String::new(),
            foco_consola: false,
            estado: "iniciando".into(),
            traza: None,
            paso_traza: None,
            tam_pty: (filas, cols_pty),
            buscar_fuente: false,
        });
        self.panel_inferior = true;
        self.pestana = Pestana::Depurador;
        if self.lateral.is_none() || self.lateral == Some(Lateral::Reglas) {
            self.lateral = Some(Lateral::Variables);
        }
        self.avisar(
            "Depurando: F5 continuar · F10 paso · F11 entrar · Shift+F11 salir · Shift+F5 detener",
            Nivel::Info,
        );
        self.lanzar_memtrace();
    }

    pub(super) fn detener_depuracion(&mut self) {
        // Sin depuración: Shift+F5 corta el programa que corre en la terminal (Ctrl+C).
        if self.depuracion.is_none() {
            let mut cortado = false;
            if let Some(d) = self.ejecucion.as_mut() {
                if d.vivo() {
                    let _ = d.escribir(b"\x03");
                    cortado = true;
                }
            }
            if cortado {
                self.avisar("Programa cortado (Ctrl+C).", Nivel::Info);
            }
            return;
        }
        if let Some(mut d) = self.depuracion.take() {
            d.gdb.terminar();
            self.avisar("Depuración detenida.", Nivel::Info);
        }
        if matches!(self.lateral, Some(Lateral::Variables | Lateral::CatScan)) {
            self.lateral = None;
        }
    }

    pub(super) fn depurador_paso(&mut self, a: Accion) {
        let Some(d) = &mut self.depuracion else {
            if a == Accion::Paso {
                self.abrir_menu(0);
            }
            return;
        };
        if d.gdb.corriendo {
            if a == Accion::Detener {
                let _ = d.gdb.interrumpir();
            }
            return;
        }
        let _ = match a {
            Accion::Paso => d.gdb.paso(),
            Accion::Entrar => d.gdb.entrar(),
            Accion::SalirFuncion => d.gdb.salir(),
            _ => Ok(()),
        };
    }

    pub(super) fn eventos_gdb(&mut self) {
        let raiz = self.proyecto.raiz.clone();
        let del_proyecto = |m: &jonesy_core::gdb::Marco| {
            m.linea.is_some()
                && m.archivo
                    .as_ref()
                    .is_some_and(|a| a.starts_with(&raiz) && a.is_file())
        };
        let Some(d) = &mut self.depuracion else {
            return;
        };
        let eventos = d.gdb.recibir();
        let mut ir: Option<(PathBuf, usize)> = None;
        let mut fin = false;
        let mut aviso: Option<(String, Nivel)> = None;
        for e in eventos {
            match e {
                EventoGdb::Detenido(p) => {
                    let marco = match &p {
                        Parada::Breakpoint(m) | Parada::Paso(m) | Parada::Otra(_, m) => {
                            Some(m.clone())
                        }
                        Parada::Senal {
                            nombre,
                            significado,
                            marco,
                        } => {
                            let fallo = jonesy_core::fallos::de_senal(nombre);
                            aviso = Some((
                                match fallo {
                                    Some(f) => format!("{} ({nombre}).", f.titulo),
                                    None => {
                                        format!("El programa recibió {nombre} ({significado}).")
                                    }
                                },
                                Nivel::Error,
                            ));
                            if let Some(f) = fallo.filter(|f| !f.explicacion.is_empty()) {
                                d.consola.push(String::new());
                                d.consola.push(format!("{nombre}: {}", f.titulo));
                                // Sin el marcado de Markdown: la consola es texto plano.
                                d.consola.extend(
                                    f.explicacion
                                        .lines()
                                        .map(|l| l.replace("**", "").replace('`', "")),
                                );
                            }
                            d.estado = format!("señal {nombre}");
                            // Dentro de libc (strlen(NULL), free…): se busca la línea del
                            // proyecto que la llamó cuando lleguen los marcos.
                            d.buscar_fuente = !del_proyecto(marco);
                            Some(marco.clone())
                        }
                        Parada::Termino(c) => {
                            d.estado = match c {
                                Some(c) => format!("terminó (código {c})"),
                                None => "terminó por una señal".into(),
                            };
                            aviso = Some((
                                format!("El programa {}. F5 lo vuelve a correr.", d.estado),
                                Nivel::Info,
                            ));
                            d.ubicacion = None;
                            d.marcos.clear();
                            d.variables.clear();
                            None
                        }
                    };
                    if let Some(m) = marco {
                        if !matches!(p, Parada::Senal { .. }) {
                            d.estado = format!("detenido en {}()", m.funcion);
                        }
                        if let (Some(a), Some(l)) = (m.archivo.clone(), m.linea) {
                            d.ubicacion = Some((a.clone(), l));
                            ir = Some((a, l));
                        }
                        let _ = d.gdb.pedir_estado();
                        for (i, (e, _)) in self.vigiladas.iter().enumerate() {
                            let _ = d.gdb.evaluar(e, i);
                        }
                        // El valor bajo el cursor se vuelve a pedir en esta parada.
                        self.valor_cursor = None;
                    }
                }
                EventoGdb::Corriendo => d.estado = "corriendo".into(),
                EventoGdb::Marcos(m) => {
                    d.marcos = m;
                    if std::mem::take(&mut d.buscar_fuente) {
                        if let Some(m) = d.marcos.iter().find(|m| del_proyecto(m)).cloned() {
                            let (a, l) = (m.archivo.clone().unwrap(), m.linea.unwrap());
                            d.ubicacion = Some((a.clone(), l));
                            ir = Some((a, l));
                            let _ = d.gdb.seleccionar_marco(m.nivel);
                            d.estado = format!("{} en {}()", d.estado, m.funcion);
                        }
                    }
                    if let (Some(t), Some(m0)) = (&d.traza, d.marcos.first()) {
                        if let Some(l) = m0.linea {
                            d.paso_traza = t.alinear(
                                &m0.funcion,
                                l,
                                d.marcos.len(),
                                d.paso_traza.unwrap_or(0),
                            );
                        }
                    }
                }
                EventoGdb::Variables(v) => d.variables = v,
                EventoGdb::Valores(valores) => {
                    for (nombre, valor) in valores {
                        if let Some(x) = d.variables.iter_mut().find(|x| x.nombre == nombre) {
                            x.valor = valor;
                        }
                    }
                }
                EventoGdb::Consola(t) => {
                    for l in t.lines() {
                        // Los tabs de GDB («4\tint *p») dejarían celdas sin pisar.
                        d.consola.push(expandir_tabs(l));
                    }
                    if d.consola.len() > 500 {
                        d.consola.drain(..d.consola.len() - 500);
                    }
                }
                EventoGdb::Error(m) => d.consola.push(format!("error: {m}")),
                EventoGdb::BreakpointCreado { .. } => {}
                EventoGdb::Evaluado { indice, valor } => {
                    if indice == usize::MAX {
                        if let Some((_, v)) = &mut self.valor_cursor {
                            *v = Some(valor);
                        }
                    } else if let Some((_, v)) = self.vigiladas.get_mut(indice) {
                        *v = Some(valor);
                    }
                }
                EventoGdb::Salio => fin = true,
            }
        }
        if let Some((a, l)) = ir {
            if a.is_file() {
                self.ir_a(&a, l, 1);
            }
        }
        if let Some((m, n)) = aviso {
            self.avisar(m, n);
        }
        if fin {
            self.depuracion = None;
            self.avisar("gdb terminó.", Nivel::Info);
        }
    }
}
