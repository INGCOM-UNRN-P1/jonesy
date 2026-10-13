//! `jonesy config exportar|importar`: llevar las preferencias de una máquina a otra.

use std::process::Command;

#[test]
fn exportar_e_importar_entre_maquinas() {
    let d = tempfile::tempdir().unwrap();
    let origen = d.path().join("origen");
    let destino = d.path().join("destino");
    let proyecto = d.path().join("tp");
    std::fs::create_dir_all(origen.join("integraciones")).unwrap();
    std::fs::create_dir_all(proyecto.join(".jonesy")).unwrap();
    std::fs::write(
        origen.join("integraciones/mia.toml"),
        "nombre = \"mia\"\ncomando = \"true\"\n",
    )
    .unwrap();
    std::fs::write(
        proyecto.join(".jonesy/session.json"),
        r#"{"tema": "turbo", "quickdoc_visible": true, "integraciones": [["gaff", false]]}"#,
    )
    .unwrap();
    let archivo = d.path().join("prefs.toml");
    let jonesy = |config: &std::path::Path, args: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_jonesy"))
            .args(args)
            .env("JONESY_CONFIG", config)
            .current_dir(d.path())
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8_lossy(&o.stdout).into_owned()
    };
    let salida = jonesy(
        &origen,
        &[
            "config",
            "exportar",
            archivo.to_str().unwrap(),
            proyecto.to_str().unwrap(),
        ],
    );
    assert!(
        salida.contains("tema turbo") && salida.contains("1 manifiestos"),
        "{salida}"
    );
    let salida = jonesy(&destino, &["config", "importar", archivo.to_str().unwrap()]);
    assert!(
        salida.contains("manifiesto instalado: mia.toml"),
        "{salida}"
    );
    assert!(destino.join("integraciones/mia.toml").is_file());
    let base = std::fs::read_to_string(destino.join("preferencias.toml")).unwrap();
    assert!(
        base.contains("tema = \"turbo\"") && !base.contains("[[manifiesto]]"),
        "{base}"
    );
    assert!(base.contains("gaff = false"), "{base}");
}
