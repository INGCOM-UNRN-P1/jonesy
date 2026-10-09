//! §12.6: un corte de la conexión (SIGHUP) no pierde trabajo. Se lanza el binario real
//! en una terminal virtual, se escribe en el editor y se le manda SIGHUP.
#![cfg(unix)]

use std::path::Path;
use std::time::{Duration, Instant};

use jonesy_terminal::Deck;

fn esperar(que: &str, mut cond: impl FnMut() -> bool) {
    let fin = Instant::now() + Duration::from_secs(20);
    while !cond() {
        assert!(Instant::now() < fin, "tiempo agotado esperando {que}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Abre jonesy sobre `main.c`, escribe `texto`, le manda SIGHUP y espera que termine.
fn cortar(dir: &Path, args: &[&str], texto: &str) {
    // Estado del usuario propio, con la bienvenida ya vista (si no, lo tipeado iría a
    // esa pantalla y no al editor).
    let estado = dir.join(".estado-usuario");
    std::fs::create_dir_all(&estado).unwrap();
    std::fs::write(estado.join("bienvenida-vista"), "").unwrap();
    std::env::set_var("JONESY_ESTADO", &estado);
    let mut a: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    a.push("main.c".into());
    let mut deck =
        Deck::iniciar(Some(env!("CARGO_BIN_EXE_jonesy")), &a, dir, 40, 120, || {}).unwrap();
    esperar("la interfaz", || deck.texto().contains("int main"));
    deck.escribir(texto.as_bytes()).unwrap();
    esperar("el texto en el editor", || deck.texto().contains(texto));
    let pid = deck.pid().expect("pid") as libc::pid_t;
    assert_eq!(unsafe { libc::kill(pid, libc::SIGHUP) }, 0);
    esperar("que jonesy termine", || !deck.vivo());
}

fn proyecto() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("main.c"), "int main(void) { return 0; }\n").unwrap();
    d
}

fn bitacora_sellada_por_sighup(dir: &Path) {
    let log = dir.join(".jonesy/telemetry.log");
    let v = jonesy_telemetry::verificar(&log).unwrap();
    assert!(v.integra(), "{:?}", v.error);
    assert!(v.cerrada, "la bitácora termina con SessionEnd");
    let ultima = std::fs::read_to_string(&log).unwrap();
    let ultima = ultima.lines().last().unwrap();
    assert!(ultima.contains("SIGHUP"), "{ultima}");
}

#[test]
fn fuera_del_examen_deja_copia_de_recuperacion_y_sesion() {
    let d = proyecto();
    cortar(d.path(), &["--telemetria"], "QWERTY");
    // El archivo no se toca: lo sin guardar queda en .jonesy/recuperacion.
    let original = std::fs::read_to_string(d.path().join("main.c")).unwrap();
    assert!(!original.contains("QWERTY"));
    let copia = std::fs::read_to_string(d.path().join(".jonesy/recuperacion/main.c")).unwrap();
    assert!(copia.contains("QWERTY"), "{copia}");
    let sesion = std::fs::read_to_string(d.path().join(".jonesy/session.json")).unwrap();
    assert!(sesion.contains("main.c"));
    bitacora_sellada_por_sighup(d.path());
}

#[test]
fn en_examen_guarda_los_archivos() {
    let d = proyecto();
    cortar(d.path(), &["--exam"], "ZXCVB");
    let guardado = std::fs::read_to_string(d.path().join("main.c")).unwrap();
    assert!(guardado.contains("ZXCVB"), "{guardado}");
    assert!(d.path().join(".jonesy/session.json").is_file());
    bitacora_sellada_por_sighup(d.path());
}
