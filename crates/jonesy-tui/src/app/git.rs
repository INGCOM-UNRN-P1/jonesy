//! Panel de git: stage, commit, diff y push/pull (N-JONESY-02).

use super::*;

impl App {
    pub(super) fn git_accion(&mut self, a: Accion) {
        let Some(repo) = self.repo.clone() else {
            self.avisar("El proyecto no está en un repositorio git.", Nivel::Aviso);
            return;
        };
        let r = match a {
            Accion::GitStageTodo => repo
                .agregar_todo()
                .map(|_| "Todo agregado al stage.".to_string()),
            Accion::GitUnstageTodo => repo
                .quitar_todo_del_stage()
                .map(|_| "Stage vaciado.".to_string()),
            Accion::GitCommit => {
                if self
                    .git_estado
                    .as_ref()
                    .is_some_and(|e| e.cantidad_en_stage() == 0)
                {
                    self.avisar(
                        "No hay cambios en el stage (Espacio o 'a' para agregar).",
                        Nivel::Aviso,
                    );
                    return;
                }
                self.modal = Some(Modal::Commit {
                    mensaje: String::new(),
                });
                return;
            }
            Accion::GitLog => {
                self.lateral = Some(Lateral::Git);
                self.foco = Foco::Lateral;
                if self.git.log.is_some() {
                    self.git.log = None;
                    return;
                }
                match repo.log(50) {
                    Ok(l) => {
                        self.git.log = Some(l);
                        self.git.sel_log = 0;
                    }
                    Err(e) => self.avisar(e.to_string(), Nivel::Error),
                }
                return;
            }
            _ => return,
        };
        match r {
            Ok(m) => self.avisar(m, Nivel::Ok),
            Err(e) => self.avisar(e.to_string(), Nivel::Error),
        }
        self.refrescar_git();
    }

    pub(super) fn git_alternar_stage(&mut self) {
        let (Some(repo), Some(e)) = (self.repo.clone(), self.git_estado.clone()) else {
            return;
        };
        let Some(entrada) = e.entradas.get(self.git.sel) else {
            return;
        };
        let r = if entrada.en_stage() && !entrada.sin_stage() {
            repo.quitar_del_stage(&entrada.ruta)
        } else {
            repo.agregar(&entrada.ruta)
        };
        if let Err(e) = r {
            self.avisar(e.to_string(), Nivel::Error);
        }
        self.refrescar_git();
    }

    /// Enter en el panel de git: el diff del archivo o del commit, lado a lado.
    pub(super) fn git_mostrar(&mut self) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        if let Some(log) = &self.git.log {
            let Some(c) = log.get(self.git.sel_log).cloned() else {
                return;
            };
            // Con otro commit marcado (m): el diff entre los dos, del más viejo al más nuevo.
            if let Some(m) = self.git.marcado.clone().filter(|m| *m != c.hash) {
                let pos = |h: &str| log.iter().position(|x| x.hash == h).unwrap_or(0);
                let (viejo, nuevo) = if pos(&m) > pos(&c.hash) {
                    (m, c.hash.clone())
                } else {
                    (c.hash.clone(), m)
                };
                match repo.diff_commits(&viejo, &nuevo) {
                    Ok(t) => {
                        let filas = jonesy_core::diff::desde_unificado(&t);
                        if filas.is_empty() {
                            self.avisar(
                                format!("Sin diferencias entre {viejo} y {nuevo}."),
                                Nivel::Info,
                            );
                        } else {
                            self.abrir_diff(format!("{viejo} → {nuevo}"), filas);
                        }
                    }
                    Err(e) => self.avisar(e.to_string(), Nivel::Error),
                }
                return;
            }
            match repo.mostrar(&c.hash) {
                Ok(t) => {
                    let filas = jonesy_core::diff::desde_unificado(&t);
                    if filas.is_empty() {
                        // Commits sin cambios de texto (merge, binarios): el texto tal cual.
                        self.modal = Some(Modal::Visor {
                            titulo: format!("git show {}", c.hash),
                            lineas: lineas_diff(&t),
                            desplazamiento: 0,
                        });
                    } else {
                        self.abrir_diff(format!("{} {}", c.hash, c.resumen), filas);
                    }
                }
                Err(e) => self.avisar(e.to_string(), Nivel::Error),
            }
            return;
        }
        let Some(e) = self
            .git_estado
            .as_ref()
            .and_then(|e| e.entradas.get(self.git.sel))
            .cloned()
        else {
            return;
        };
        let bytes = std::fs::read(repo.dir.join(&e.ruta)).unwrap_or_default();
        if jonesy_core::diff::es_binario(&bytes) {
            self.avisar(
                format!(
                    "{} es un archivo binario: no se compara como texto.",
                    e.ruta
                ),
                Nivel::Info,
            );
            return;
        }
        let actual = String::from_utf8_lossy(&bytes).into_owned();
        // Un renombre (en el stage) se compara contra el nombre anterior en HEAD.
        if let Some(origen) = e.origen.clone() {
            let viejo = repo.contenido(&origen, false).unwrap_or_default();
            let nuevo = repo.contenido(&e.ruta, true).unwrap_or(actual);
            let filas = jonesy_core::diff::comparar(&viejo, &nuevo, 3);
            if filas.is_empty() {
                self.avisar(
                    format!("{origen} → {}: sólo cambió el nombre.", e.ruta),
                    Nivel::Info,
                );
            } else {
                self.abrir_diff(format!("{} (renombrado de {origen})", e.ruta), filas);
            }
            return;
        }
        // Sólo en el stage: HEAD contra el índice. Con cambios sin stage: índice contra el
        // árbol de trabajo (lo que mostraría git diff). Nuevo: vacío contra el archivo.
        let (titulo, viejo, nuevo) = if e.en_stage() && !e.sin_stage() {
            (
                format!("{} (HEAD → stage)", e.ruta),
                repo.contenido(&e.ruta, false).unwrap_or_default(),
                repo.contenido(&e.ruta, true).unwrap_or_default(),
            )
        } else {
            let base = repo
                .contenido(&e.ruta, true)
                .or_else(|| repo.contenido(&e.ruta, false))
                .unwrap_or_default();
            (format!("{} (stage → archivo)", e.ruta), base, actual)
        };
        let filas = jonesy_core::diff::comparar(&viejo, &nuevo, 3);
        if filas.is_empty() {
            self.avisar(
                format!("{}: sin diferencias de texto.", e.ruta),
                Nivel::Info,
            );
            return;
        }
        self.abrir_diff(titulo, filas);
    }

    pub(super) fn git_commit(&mut self, mensaje: &str) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        if let Some((_, m)) = jonesy_git::revisar_mensaje(mensaje)
            .into_iter()
            .find(|(bloquea, _)| *bloquea)
        {
            self.avisar(m, Nivel::Aviso);
            return;
        }
        match repo.commit(mensaje) {
            Ok(salida) => {
                self.tel(TelemetryEvent::CommandExec {
                    command: "git commit".into(),
                    exit_code: Some(0),
                });
                self.avisar(
                    salida
                        .lines()
                        .next()
                        .unwrap_or("Commit creado.")
                        .to_string(),
                    Nivel::Ok,
                );
                self.modal = None;
                self.git.log = None;
            }
            Err(e) => {
                self.tel(TelemetryEvent::CommandExec {
                    command: "git commit".into(),
                    exit_code: Some(1),
                });
                self.avisar(e.to_string(), Nivel::Error);
            }
        }
        self.refrescar_git();
    }

    /// Git → Subir (push) o Traer (pull): en la terminal, por si pide usuario o clave.
    pub fn git_remoto(&mut self, op: OpRemota) {
        let Some(e) = self.git_estado.clone() else {
            self.avisar("El proyecto no está en un repositorio git.", Nivel::Aviso);
            return;
        };
        let cmd = match op {
            OpRemota::Subir if e.upstream.is_none() => "git push -u origin HEAD".to_string(),
            OpRemota::Subir => "git push".to_string(),
            OpRemota::Traer => {
                if e.cantidad_modificados() > 0 {
                    self.avisar(
                        "Hay cambios sin commit: si chocan con lo que traés, git no va a poder. Mejor hacé commit antes.",
                        Nivel::Aviso,
                    );
                }
                "git pull --no-rebase --no-edit".to_string()
            }
        };
        self.git_remoto = Some(op);
        self.correr_en_deck(cmd);
    }

    /// Al terminar un push o pull: qué pasó y qué hacer.
    pub(super) fn fin_git_remoto(&mut self, op: OpRemota) {
        let texto = self
            .ejecucion
            .as_ref()
            .map(|d| d.texto().to_lowercase())
            .unwrap_or_default();
        let codigo = self.ejecucion.as_mut().and_then(|d| d.codigo_salida());
        self.refrescar_git();
        let conflictos: Vec<String> = self
            .git_estado
            .as_ref()
            .map(|e| {
                e.entradas
                    .iter()
                    .filter(|x| x.tipo == jonesy_git::TipoEntrada::Conflicto)
                    .map(|x| x.ruta.clone())
                    .collect()
            })
            .unwrap_or_default();
        if !conflictos.is_empty() {
            let lista: String = conflictos.iter().map(|c| format!("- `{c}`\n")).collect();
            let texto = format!(
                "# Conflicto al traer los cambios\n\n\
                 Vos y el repositorio cambiaron las mismas líneas de:\n\n{lista}\n\
                 Para resolverlo:\n\n\
                 1. Abrí cada archivo y buscá las marcas `<<<<<<<`, `=======` y `>>>>>>>`.\n\
                 2. Entre `<<<<<<<` y `=======` está tu versión; entre `=======` y `>>>>>>>`, la que llegó.\n\
                 3. Dejá el código como tiene que quedar y borrá las tres marcas.\n\
                 4. Guardá, agregalo al stage (Espacio en el panel de git) y hacé commit (c).\n\n\
                 Si preferís deshacer el intento: `git merge --abort` en la terminal.\n"
            );
            let ancho = self.tam.0.saturating_sub(10).clamp(40, 90);
            self.modal = Some(Modal::Visor {
                titulo: "Conflicto".into(),
                lineas: markdown::renderizar(&texto, ancho, &self.sim).lineas,
                desplazamiento: 0,
            });
            return;
        }
        let rechazado = ["rejected", "rechazad", "fetch first", "non-fast-forward"]
            .iter()
            .any(|p| texto.contains(p));
        match (op, codigo == Some(0), rechazado) {
            (OpRemota::Subir, _, true) => self.avisar(
                "El repositorio tiene cambios que vos no tenés: primero traé los cambios (b en el panel de git) y después subí.",
                Nivel::Error,
            ),
            (OpRemota::Subir, true, _) => self.avisar("Cambios subidos.", Nivel::Ok),
            (OpRemota::Traer, true, _) => self.avisar("Cambios traídos.", Nivel::Ok),
            (_, false, _) => self.avisar(
                "git no pudo terminar: mirá la pestaña Ejecución (¿conexión, usuario o permisos?).",
                Nivel::Error,
            ),
        }
    }
}
