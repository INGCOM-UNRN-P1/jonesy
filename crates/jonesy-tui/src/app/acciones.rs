//! Ejecución de las acciones (atajos, menú y paleta) sobre la app (N-JONESY-02).

use super::*;

impl App {
    pub fn accion(&mut self, a: Accion) {
        self.tel(TelemetryEvent::CommandExec {
            command: format!("accion:{a:?}"),
            exit_code: None,
        });
        match a {
            Accion::Abrir => self.abrir_selector(),
            Accion::Guardar => {
                if self.guardar(self.activo) {
                    self.avisar(format!("Guardado {}", self.buf().buf.nombre()), Nivel::Ok);
                }
            }
            Accion::GuardarTodo => {
                if self.guardar_todo() {
                    self.avisar("Todo guardado.", Nivel::Ok);
                }
            }
            Accion::CerrarBuffer => self.cerrar_buffer(false),
            Accion::SiguienteBuffer => {
                let n = self.buffers.len();
                self.cambiar_buffer((self.activo + 1) % n);
            }
            Accion::AnteriorBuffer => {
                let n = self.buffers.len();
                self.cambiar_buffer((self.activo + n - 1) % n);
            }
            Accion::Salir => self.pedir_salida(),
            Accion::Deshacer => self.editar(|b| b.deshacer()),
            Accion::Rehacer => self.editar(|b| b.rehacer()),
            Accion::Cortar => {
                if self.buf().buf.solo_lectura {
                    return;
                }
                let id = self.id_buffer(self.activo);
                if let Some((t, d)) = self.buf_mut().buf.cortar() {
                    self.tel(TelemetryEvent::TextDelta {
                        buffer_id: id,
                        offset: d.offset,
                        inserted: d.insertado,
                        deleted_len: d.borrados,
                    });
                    self.copiar_texto(t);
                }
            }
            Accion::Copiar => {
                if let Some(t) = self.buf().buf.texto_seleccionado() {
                    self.copiar_texto(t);
                }
            }
            Accion::Pegar => {
                let t = self.portapapeles.clone();
                if !t.is_empty() {
                    self.editar(|b| b.insertar(&t));
                }
            }
            Accion::SeleccionarTodo => self.buf_mut().buf.seleccionar_todo(),
            Accion::Buscar => {
                self.modal = Some(Modal::Buscar {
                    texto: self.busqueda.clone(),
                })
            }
            Accion::BuscarSiguiente => self.buscar_siguiente(),
            Accion::BuscarEnProyecto => {
                let texto = self
                    .buf()
                    .buf
                    .texto_seleccionado()
                    .filter(|t| !t.contains('\n'))
                    .or_else(|| self.busqueda_proyecto.as_ref().map(|(p, _)| p.clone()))
                    .or_else(|| {
                        let b = &self.buf().buf;
                        b.palabra_en(b.cursor)
                    })
                    .unwrap_or_default();
                self.modal = Some(Modal::BuscarProyecto {
                    texto,
                    mayusculas: false,
                    palabra: false,
                });
            }
            Accion::Reemplazar => {
                let buscar = self
                    .buf()
                    .buf
                    .texto_seleccionado()
                    .filter(|t| !t.contains('\n'))
                    .unwrap_or_else(|| self.busqueda.clone());
                self.modal = Some(Modal::Reemplazar {
                    buscar,
                    reemplazo: String::new(),
                    campo: 0,
                    mayusculas: false,
                    palabra: false,
                });
            }
            Accion::IrALinea => {
                self.modal = Some(Modal::IrALinea {
                    texto: String::new(),
                })
            }
            Accion::QuickDoc => {
                if self.modo() == Modo::Compacto {
                    self.vista = if self.vista == Vista::Doc {
                        Vista::Codigo
                    } else {
                        Vista::Doc
                    };
                } else {
                    self.quickdoc_visible = !self.quickdoc_visible;
                }
            }
            Accion::OrientacionQuickDoc => {
                self.quickdoc_posicion = match self.quickdoc_posicion {
                    PosicionDoc::Derecha => PosicionDoc::Inferior,
                    PosicionDoc::Inferior => PosicionDoc::Derecha,
                };
                self.quickdoc_visible = true;
                self.guardar_sesion();
            }
            Accion::Terminal => {
                if self.foco == Foco::Inferior && self.pestana == Pestana::Terminal {
                    self.cambiar_foco(Foco::Editor);
                    if self.modo() == Modo::Compacto {
                        self.vista = Vista::Codigo;
                    }
                } else {
                    self.abrir_terminal();
                }
            }
            Accion::Problemas => self.mostrar_pestana(Pestana::Problemas),
            Accion::SalidaCompilacion => self.mostrar_pestana(Pestana::Salida),
            Accion::PanelGit => self.alternar_lateral(Lateral::Git),
            Accion::PanelReglas => self.alternar_lateral(Lateral::Reglas),
            Accion::CompilarAlGuardar => {
                self.compilar_al_guardar = !self.compilar_al_guardar;
                self.avisar(
                    if self.compilar_al_guardar {
                        "Compilar al guardar: activado (Ctrl+S guarda y compila; Alt+B lo apaga)."
                    } else {
                        "Compilar al guardar: desactivado."
                    },
                    Nivel::Info,
                );
            }
            Accion::PreviewMarkdown => {
                if self.buf().buf.lenguaje == Lenguaje::Markdown {
                    self.preview_markdown = !self.preview_markdown;
                } else {
                    self.avisar("La vista previa es para archivos Markdown.", Nivel::Aviso);
                }
            }
            Accion::VistaConsigna => {
                if self.modo() == Modo::Compacto {
                    self.vista = Vista::Consigna;
                    self.foco = Foco::Consigna;
                } else if self.consigna.is_some() {
                    if self.foco == Foco::Consigna {
                        self.consigna_visible = !self.consigna_visible;
                        self.cambiar_foco(Foco::Editor);
                    } else {
                        self.consigna_visible = true;
                        self.cambiar_foco(Foco::Consigna);
                    }
                }
            }
            Accion::VistaCodigo => {
                self.vista = Vista::Codigo;
                self.cambiar_foco(Foco::Editor);
            }
            Accion::VistaTerminal => {
                if self.modo() == Modo::Compacto {
                    self.vista = Vista::Terminal;
                    self.panel_inferior = true;
                    self.cambiar_foco(Foco::Inferior);
                } else {
                    self.abrir_terminal();
                }
            }
            Accion::VistaDoc => {
                self.vista = Vista::Doc;
                self.quickdoc_visible = true;
            }
            Accion::Compilar => self.compilar(Luego::Nada),
            Accion::Ejecutar => self.compilar(Luego::Ejecutar),
            Accion::Probar => self.probar(),
            Accion::ProbarAqui => self.probar_aqui(),
            Accion::GitSubir => self.git_remoto(OpRemota::Subir),
            Accion::MaximizarPanel => {
                self.maximizado = !self.maximizado;
                if self.maximizado {
                    self.avisar("Panel maximizado: Alt+Z vuelve.", Nivel::Info);
                }
            }
            Accion::Simulacro => {
                if self.examen.is_some() {
                    self.avisar("Ya estás en un examen.", Nivel::Aviso);
                } else {
                    self.modal = Some(Modal::Simulacro {
                        texto: self.proyecto.config.examen.duracion_min.max(1).to_string(),
                    });
                }
            }
            Accion::TerminarSimulacro => self.terminar_simulacro(),
            Accion::GitTraer => self.git_remoto(OpRemota::Traer),
            Accion::CorrerHastaCursor => self.correr_hasta_cursor(),
            Accion::RevisarMemoria => self.revisar_memoria(),
            Accion::Depurar => self.depurar(),
            Accion::Paso | Accion::Entrar | Accion::SalirFuncion => self.depurador_paso(a),
            Accion::Detener => self.detener_depuracion(),
            Accion::AlternarBreakpoint => {
                let (l, _) = self.buf().buf.linea_columna();
                self.alternar_breakpoint(l);
            }
            Accion::CatScan => {
                if self.depuracion.is_none() {
                    self.avisar(
                        "cat-scan muestra la memoria durante la depuración (F5).",
                        Nivel::Aviso,
                    );
                    return;
                }
                self.lateral = Some(if self.lateral == Some(Lateral::CatScan) {
                    Lateral::Variables
                } else {
                    Lateral::CatScan
                });
                if self.depuracion.as_ref().is_some_and(|d| d.traza.is_none()) {
                    self.avisar("La traza de mem-trace todavía no está lista (o el proyecto tiene Makefile).", Nivel::Info);
                }
            }
            Accion::GitDeshacerCommit => self.pedir_deshacer_commit(),
            Accion::ExportarMemoria => self.exportar_memoria(),
            Accion::NuevoPlugin => {
                if self.examen.is_some() {
                    self.avisar(
                        "En modo examen sólo corren los plugins firmados por la cátedra.",
                        Nivel::Aviso,
                    );
                    return;
                }
                self.modal = Some(Modal::NuevoArchivo {
                    texto: ".jonesy/plugins/mi_plugin.py".into(),
                    carpeta: false,
                });
            }
            Accion::RecargarPlugins => {
                self.recargar_plugins();
                let n = if self.plugins.is_some() {
                    "listos"
                } else {
                    "no hay plugins"
                };
                self.avisar(format!("Plugins Python recargados ({n})."), Nivel::Ok);
            }
            Accion::GitStageTodo | Accion::GitUnstageTodo | Accion::GitCommit | Accion::GitLog => {
                self.git_accion(a)
            }
            Accion::Entregar => self.pedir_entrega(),
            Accion::ExplicarRegla => self.explicar_regla(),
            Accion::Atajos => {
                let ancho = self.tam.0.saturating_sub(10).clamp(40, 90);
                let mut texto = accion::AYUDA_ATAJOS.to_string();
                if !self.atajos_usuario.is_empty() {
                    texto.push_str("\n## Tus atajos ([atajos] de .jonesy.toml)\n\n| Teclas | Acción |\n|---|---|\n");
                    for (_, a, tecla) in &self.atajos_usuario {
                        texto.push_str(&format!("| {tecla} | {} |\n", a.info().titulo));
                    }
                } else {
                    texto.push_str("\nPara sumar los tuyos: `[atajos]` en .jonesy.toml, por ejemplo `\"ctrl+e\" = \"Compilar\"`.\n");
                }
                self.modal = Some(Modal::Visor {
                    titulo: "Atajos".into(),
                    lineas: markdown::renderizar(&texto, ancho, &self.sim).lineas,
                    desplazamiento: 0,
                });
            }
            Accion::AtajosPanel => self.atajos_del_panel(),
            Accion::ExportarPreferencias | Accion::ImportarPreferencias => {
                let importar = a == Accion::ImportarPreferencias;
                if importar && self.examen.is_some() {
                    self.avisar("En modo examen no se importan preferencias.", Nivel::Aviso);
                    return;
                }
                let texto = std::env::var_os("HOME")
                    .map(|h| {
                        PathBuf::from(h)
                            .join("jonesy-preferencias.toml")
                            .display()
                            .to_string()
                    })
                    .unwrap_or_else(|| "jonesy-preferencias.toml".into());
                self.modal = Some(Modal::RutaPreferencias { importar, texto });
            }
            Accion::AcercaDe => {
                let texto = format!(
                    "# jonesy {}\n\nIDE de terminal para la cátedra de Programación I.\n\n- Proyecto: `{}`\n- Base quick-doc: {} funciones\n- Layout: modo {}\n- Telemetría: {}\n",
                    env!("CARGO_PKG_VERSION"),
                    self.proyecto.raiz.display(),
                    jonesy_doc::Base::embebida().cantidad_funciones(),
                    self.modo().nombre(),
                    if self.telemetria.is_some() { "activa (.jonesy/telemetry.log)" } else { "inactiva" },
                );
                self.modal = Some(Modal::Visor {
                    titulo: "Acerca de".into(),
                    lineas: markdown::renderizar(&texto, 70, &self.sim).lineas,
                    desplazamiento: 0,
                });
            }
            Accion::Paleta => self.abrir_paleta(),
            Accion::TamanosPaneles => self.modal = Some(Modal::Paneles { sel: 0 }),
            Accion::CambiarTema => {
                let t = tema::actual().siguiente();
                tema::usar(t);
                self.guardar_sesion();
                self.avisar(format!("Tema {}", t.nombre()), Nivel::Info);
            }
            Accion::Explorador => self.alternar_navegador(PestanaNav::Archivos),
            Accion::Formatear => self.formatear(),
            Accion::GenerarDoc => self.generar_doc(),
            Accion::DiffArchivo => self.diff_archivo(),
            Accion::Herramientas => {
                let opciones: Vec<OpcionPaleta> = self
                    .acciones_disponibles()
                    .into_iter()
                    .map(|(nombre, n, titulo)| OpcionPaleta {
                        titulo,
                        atajo: nombre.clone(),
                        que: QuePaleta::Herramienta(nombre, n),
                    })
                    .collect();
                if opciones.is_empty() {
                    self.avisar("No hay herramientas para este archivo (Archivo → Configuración del entorno muestra cuáles están instaladas).", Nivel::Aviso);
                    return;
                }
                self.modal = Some(Modal::Paleta {
                    filtro: String::new(),
                    sel: 0,
                    opciones,
                });
            }
            Accion::Integraciones => self.modal = Some(Modal::Configuracion { pestana: 0, sel: 0 }),
            Accion::Configuracion => self.modal = Some(Modal::Configuracion { pestana: 0, sel: 0 }),
            Accion::ConfigurarPlugins => {
                self.modal = Some(Modal::Configuracion { pestana: 1, sel: 0 })
            }
            Accion::LimpiarCompilacion => self.limpiar_compilacion(),
            Accion::RestablecerPaneles => {
                self.paneles = self.proyecto.config.interfaz.paneles.acotar();
                self.guardar_sesion();
                self.avisar(
                    "Paneles con los tamaños de .jonesy.toml (o los predeterminados).",
                    Nivel::Ok,
                );
            }
            Accion::GuiaRapida => {
                let ancho = self.tam.0.saturating_sub(10).clamp(40, 90);
                self.modal = Some(Modal::Visor {
                    titulo: "Guía rápida".into(),
                    lineas: markdown::renderizar(accion::GUIA_RAPIDA, ancho, &self.sim).lineas,
                    desplazamiento: 0,
                });
            }
            Accion::EstadoEntrega => self.estado_entrega(),
            Accion::Doctor => self.diagnostico_entorno(),
            Accion::ReportarProblema => self.reportar_problema(),
            Accion::ObjetivoMake => self.elegir_objetivo_make(),
            Accion::VisorHex => self.elegir_binario(),
            Accion::ArchivoPareja => self.ir_a_pareja(),
            Accion::Referencias => self.buscar_referencias(),
            Accion::DocumentarFuncion => self.documentar_funcion(),
            Accion::CrearPrueba => self.crear_prueba(),
            Accion::Autocompletar => self.autocompletar(true),
            Accion::ArregloRapido => self.arreglo_rapido(),
            Accion::GitDescartar => {
                let rel = self
                    .activo_ruta()
                    .and_then(|r| self.repo.as_ref().and_then(|repo| repo.relativa(&r)));
                match rel {
                    Some(r) => self.pedir_descartar(r),
                    None => {
                        self.avisar("Este archivo no está en un repositorio git.", Nivel::Aviso)
                    }
                }
            }
            Accion::ConteoMemoria => self.contar_memoria(),
            Accion::SiguientePista => self.siguiente_pista(),
            Accion::Vigilar => {
                let b = &self.buf().buf;
                let texto = b
                    .texto_seleccionado()
                    .filter(|t| !t.contains('\n'))
                    .or_else(|| b.palabra_en(b.cursor))
                    .unwrap_or_default();
                self.modal = Some(Modal::Vigilar { texto });
            }
            Accion::Glosario => self.abrir_glosario(),
            Accion::CatalogoReglas => self.abrir_catalogo(),
            Accion::Recorrido => self.modal = Some(Modal::Recorrido { paso: 0 }),
            Accion::RenombrarSimbolo => self.pedir_renombrar_simbolo(),
            Accion::PausarExamen => self.pedir_pausa_examen(),
            Accion::Precedencia => {
                let b = &self.buf().buf;
                let expr = b
                    .texto_seleccionado()
                    .filter(|t| !t.trim().is_empty())
                    .unwrap_or_else(|| {
                        jonesy_core::precedencia::de_linea(&b.linea_texto(b.linea_columna().0))
                    });
                match jonesy_core::precedencia::analizar(&expr) {
                    Ok(a) => self.visor_explicacion(
                        "Precedencia".into(),
                        &jonesy_core::precedencia::markdown(&a),
                    ),
                    Err(e) => self.avisar(
                        format!("No pude leer la expresión «{}»: {e}.", expr.trim()),
                        Nivel::Aviso,
                    ),
                }
            }
            Accion::Tablas => {
                self.glosario_visor.clear();
                let ancho = self.tam.0.saturating_sub(10).clamp(40, 100);
                self.modal = Some(Modal::Visor {
                    titulo: "Tablas de consulta".into(),
                    lineas: markdown::renderizar(&crate::ayuda::tablas(), ancho, &self.sim).lineas,
                    desplazamiento: 0,
                });
            }
            Accion::Comentar => self.editar(|b| b.alternar_comentario()),
            Accion::IrADefinicion => self.ir_a_definicion(),
            Accion::AlternarMarcador => self.alternar_marcador(),
            Accion::EjecutarConEntrada => self.elegir_archivo_de_entrada(),
            Accion::CompararSalidas => self.compilar(Luego::Casos),
            Accion::ArgumentosPrograma => {
                let texto = self
                    .clave_ejercicio()
                    .and_then(|k| self.argumentos.get(&k).cloned())
                    .unwrap_or_default();
                self.modal = Some(Modal::ArgsPrograma { texto });
            }
            Accion::SiguienteMarcador => self.siguiente_marcador(),
            Accion::Marcadores => self.lista_marcadores(),
            Accion::SiguienteProblema => self.problema_cercano(true),
            Accion::AnteriorProblema => self.problema_cercano(false),
            Accion::Volver => self.volver(),
            Accion::DuplicarLinea => self.editar(|b| b.duplicar_lineas()),
            Accion::SubirLinea => self.editar(|b| b.mover_lineas(false)),
            Accion::BajarLinea => self.editar(|b| b.mover_lineas(true)),
            Accion::LimpiarEspacios => self.editar(|b| b.limpiar_espacios_finales()),
            Accion::Bienvenida => self.mostrar_bienvenida(),
            Accion::GitClonar => {
                if self.examen.is_some() {
                    self.avisar("En modo examen no se puede clonar.", Nivel::Aviso);
                    return;
                }
                let destino = self
                    .proyecto
                    .raiz
                    .parent()
                    .unwrap_or(&self.proyecto.raiz)
                    .display()
                    .to_string();
                self.modal = Some(Modal::Clonar {
                    url: String::new(),
                    destino,
                    campo: 0,
                });
            }
            Accion::AbrirProyecto => {
                if self.examen.is_some() {
                    self.avisar(
                        "En modo examen no se puede cambiar de proyecto.",
                        Nivel::Aviso,
                    );
                    return;
                }
                let dir = self
                    .proyecto
                    .raiz
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| self.proyecto.raiz.clone());
                self.modal = Some(Modal::AbrirProyecto {
                    entradas: entradas_dir(&dir, true),
                    dir,
                    filtro: String::new(),
                    sel: 0,
                });
            }
            Accion::EstructuraProyecto => self.alternar_navegador(PestanaNav::Proyecto),
            Accion::Esquema => self.alternar_navegador(PestanaNav::Esquema),
            Accion::NuevoArchivo => {
                let dir = if self.foco == Foco::Navegador
                    && self.navegador == Some(PestanaNav::Archivos)
                {
                    self.explorador.directorio_destino(&self.proyecto.raiz)
                } else {
                    self.activo_ruta()
                        .and_then(|r| r.parent().map(Path::to_path_buf))
                        .unwrap_or_else(|| self.proyecto.raiz.clone())
                };
                self.pedir_nuevo(&dir, false);
            }
        }
    }
}
