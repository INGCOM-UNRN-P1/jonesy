//! Entrega y modo examen: vista previa, entregar y estado (N-JONESY-02).

use super::*;

impl App {
    pub fn examen_entregado(&self) -> bool {
        self.examen.as_ref().is_some_and(|e| e.estado.entregado)
    }

    pub(super) fn pedir_entrega(&mut self) {
        let Some(e) = &self.examen else {
            self.avisar(
                "La entrega sólo está disponible en modo examen (jonesy --exam).",
                Nivel::Aviso,
            );
            return;
        };
        if e.estado.entregado {
            let msg = format!(
                "Ya entregado: {}",
                e.estado.sha256.clone().unwrap_or_default()
            );
            self.avisar(msg, Nivel::Info);
            return;
        }
        let verificar = self
            .proyecto
            .comando_verificar()
            .unwrap_or_else(|| "(sin verificación configurada)".into());
        let mut mensaje = self.vista_previa_entrega();
        mensaje.push_str(&format!(
            "\nSe guardan todos los archivos, se corre «{verificar}», se sella la bitácora y se arma el paquete. Después ya no vas a poder editar. ¿Confirmás la entrega? (s/n)"
        ));
        self.modal = Some(Modal::Confirmar {
            titulo: "Entregar examen".into(),
            mensaje,
            que: Confirmacion::Entregar,
        });
    }

    /// Qué archivos van en el paquete y cómo quedaron la última compilación y las últimas
    /// pruebas, para revisar antes de confirmar la entrega.
    pub(super) fn vista_previa_entrega(&self) -> String {
        let archivos = entrega::archivos_de_entrega(&self.proyecto);
        let mut s = format!("Van en el paquete ({}):\n", archivos.len());
        const MAX: usize = 12;
        for a in archivos.iter().take(MAX) {
            s.push_str(&format!("  · {a}\n"));
        }
        if archivos.len() > MAX {
            s.push_str(&format!("  … y {} más\n", archivos.len() - MAX));
        }
        if archivos.is_empty() {
            s.push_str("  ⚠ ninguno: revisá [examen] archivos en .jonesy.toml\n");
        }
        // Lo que editaste y no entra en el paquete.
        for b in &self.buffers {
            let Some(ruta) = &b.buf.ruta else { continue };
            if !ruta.starts_with(&self.proyecto.raiz) || b.buf.lenguaje != Lenguaje::C {
                continue;
            }
            let rel = self.proyecto.relativa(ruta);
            if !archivos.contains(&rel) {
                s.push_str(&format!(
                    "  ⚠ «{rel}» está abierto pero NO entra en el paquete\n"
                ));
            }
        }
        let ultimo_cambio = self.buffers.iter().map(|b| b.ultimo_cambio).max();
        let despues = |t: Instant| {
            if ultimo_cambio.is_some_and(|c| c > t) {
                " (pero cambiaste el código después)"
            } else {
                ""
            }
        };
        s.push('\n');
        match self.ultima_compilacion {
            None => s.push_str("· Todavía no compilaste en esta sesión (F6).\n"),
            Some((t, 0)) => s.push_str(&format!("✓ La última compilación anduvo{}.\n", despues(t))),
            Some((t, n)) => s.push_str(&format!(
                "✗ La última compilación FALLÓ ({n} error{}){}: así no compila.\n",
                if n == 1 { "" } else { "es" },
                despues(t)
            )),
        }
        match self.ultimas_pruebas {
            None => s.push_str("· No corriste las pruebas en esta sesión (Ctrl+F6).\n"),
            Some((t, _, _, false)) => {
                s.push_str(&format!("✗ Las pruebas no compilaron{}.\n", despues(t)))
            }
            Some((t, p, 0, true)) => {
                s.push_str(&format!("✓ Pasan las {p} pruebas{}.\n", despues(t)))
            }
            Some((t, p, f, true)) => s.push_str(&format!(
                "✗ FALLAN {f} prueba{} ({p} pasan){}.\n",
                if f == 1 { "" } else { "s" },
                despues(t)
            )),
        }
        s
    }

    pub(super) fn entregar(&mut self) {
        self.guardar_todo();
        if let Some(e) = &mut self.examen {
            e.entregando = true;
        }
        match self.proyecto.comando_verificar() {
            Some(cmd) => {
                self.salida = vec![(format!("$ {cmd}"), false)];
                self.titulo_salida = "verificación de la entrega".into();
                self.panel_inferior = true;
                self.pestana = Pestana::Salida;
                self.avisar("Verificando la entrega…", Nivel::Info);
                self.lanzar(&cmd, Tarea::Verificar);
            }
            None => self.finalizar_entrega(None),
        }
    }

    pub(super) fn finalizar_entrega(&mut self, verificacion: Option<entrega::Verificacion>) {
        if let Some(v) = &verificacion {
            self.tel(TelemetryEvent::CommandExec {
                command: v.comando.clone(),
                exit_code: v.codigo,
            });
        }
        self.tel(TelemetryEvent::SessionEnd {
            reason: "entrega".into(),
        });
        let log = self.proyecto.dir_estado().join("telemetry.log");
        if let Some(t) = self.telemetria.take() {
            t.cerrar();
        }
        let sello = jonesy_telemetry::verificar(&log).ok();
        let telemetria = sello.as_ref().map(|v| {
            (
                log.as_path(),
                Some(v.ultimo_hash.clone()),
                v.registros as usize,
            )
        });
        match entrega::empaquetar(
            &self.proyecto,
            telemetria,
            verificacion.clone(),
            jonesy_core::ahora_unix(),
        ) {
            Ok(p) => {
                let rel = self.proyecto.relativa(&p.ruta);
                if let Some(e) = &mut self.examen {
                    e.entregando = false;
                    e.estado.entregado = true;
                    e.estado.paquete = Some(rel.clone());
                    e.estado.sha256 = Some(p.sha256.clone());
                    let _ = e.estado.guardar(&self.proyecto.dir_estado());
                }
                for b in &mut self.buffers {
                    b.buf.solo_lectura = true;
                }
                let resultado = match &verificacion {
                    Some(v) if v.codigo == Some(0) => "✓ la verificación pasó".to_string(),
                    Some(v) => format!(
                        "✗ la verificación falló (código {})",
                        v.codigo.map(|c| c.to_string()).unwrap_or("?".into())
                    ),
                    None => "sin verificación configurada".to_string(),
                };
                let texto = format!(
                    "# Entrega registrada\n\n- **Resultado:** {resultado}\n- **Paquete:** `{rel}`\n- **SHA-256:** `{}`\n- **Archivos:** {}\n- **Bitácora:** {} registros, cadena {}\n\nGuardá el SHA-256: identifica tu entrega.\n",
                    p.sha256,
                    p.manifiesto.archivos.len(),
                    p.manifiesto.telemetria_registros,
                    if sello.as_ref().is_some_and(|s| s.integra()) { "íntegra" } else { "no verificada" },
                );
                let ancho = self.tam.0.saturating_sub(10).min(90);
                self.modal = Some(Modal::Visor {
                    titulo: "Entrega".into(),
                    lineas: markdown::renderizar(&texto, ancho, &self.sim).lineas,
                    desplazamiento: 0,
                });
            }
            Err(e) => {
                if let Some(ex) = &mut self.examen {
                    ex.entregando = false;
                }
                self.avisar(format!("No se pudo armar el paquete: {e}"), Nivel::Error);
            }
        }
    }

    /// Examen → Verificar estado de entrega.
    pub fn estado_entrega(&mut self) {
        let Some(e) = &self.examen else {
            self.avisar("No estás en modo examen.", Nivel::Aviso);
            return;
        };
        let ahora = jonesy_core::ahora_unix();
        let mut t = String::from("# Estado del examen\n\n");
        let empezo = ahora.saturating_sub(e.estado.inicio_unix) / 60;
        t.push_str(&format!("- **Empezó** hace {empezo} min.\n"));
        match e.estado.restante_s(ahora) {
            Some(r) if r > 0 => t.push_str(&format!(
                "- **Tiempo:** quedan {}:{:02} de {} min.\n",
                r / 3600,
                (r % 3600) / 60,
                e.estado.duracion_min
            )),
            Some(_) => t.push_str("- **Tiempo:** terminado. Entregá con F12.\n"),
            None => t.push_str("- **Tiempo:** sin límite.\n"),
        }
        let sin_guardar: Vec<String> = self
            .buffers
            .iter()
            .filter(|b| b.buf.modificado())
            .map(|b| b.buf.nombre())
            .collect();
        t.push_str(&format!(
            "- **Autoguardado:** cada {} s; hace {} s el último. Sin guardar ahora: {}.\n",
            AUTOGUARDADO.as_secs(),
            e.ultimo_autoguardado.elapsed().as_secs(),
            if sin_guardar.is_empty() {
                "nada".to_string()
            } else {
                sin_guardar.join(", ")
            }
        ));
        let log = self.proyecto.dir_estado().join("telemetry.log");
        match jonesy_telemetry::verificar(&log) {
            Ok(v) if v.integra() => t.push_str(&format!(
                "- **Bitácora:** íntegra, {} registros.\n",
                v.registros
            )),
            Ok(v) => t.push_str(&format!(
                "- **Bitácora:** ⚠ rota ({}).\n",
                v.error
                    .map(|(l, m)| format!("línea {l}: {m}"))
                    .unwrap_or_default()
            )),
            Err(_) => t.push_str("- **Bitácora:** todavía sin registros.\n"),
        }
        if e.estado.entregado {
            t.push_str(&format!(
                "- **Entrega:** hecha. Paquete `{}`, SHA-256 `{}`.\n\n\
                 Quien corrige lo comprueba con `jonesy verificar {}`.\n",
                e.estado.paquete.as_deref().unwrap_or("?"),
                e.estado.sha256.as_deref().unwrap_or("?"),
                e.estado.paquete.as_deref().unwrap_or("<paquete>")
            ));
        } else {
            t.push_str("- **Entrega:** todavía no. Cuando termines, Examen → Entregar (F12).\n");
        }
        let ancho = self.tam.0.saturating_sub(10).clamp(40, 90);
        self.modal = Some(Modal::Visor {
            titulo: "Estado de la entrega".into(),
            lineas: markdown::renderizar(&t, ancho, &self.sim).lineas,
            desplazamiento: 0,
        });
    }
}
