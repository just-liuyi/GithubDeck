use serde_json::Value;
use std::fs;
use std::path::PathBuf;

#[test]
fn tauri_branding_preserves_internal_identity() {
    let config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json");
    let config: Value = serde_json::from_str(
        &fs::read_to_string(config_path).expect("tauri.conf.json should be readable"),
    )
    .expect("tauri.conf.json should contain valid JSON");

    assert_eq!(config["productName"], "GithubDeck");
    assert_eq!(config["mainBinaryName"], "GithubDeck");
    assert_eq!(config["app"]["windows"][0]["title"], "GithubDeck");
    assert_eq!(config["identifier"], "com.feijizai.ghdeck");
}
