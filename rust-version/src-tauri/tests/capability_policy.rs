use serde_json::Value;
use std::fs;
use std::path::PathBuf;

const RELEASES_URL: &str = "https://github.com/cli/cli/releases/";

#[test]
fn opener_permission_is_limited_to_the_fixed_releases_url() {
    let capability_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("capabilities")
        .join("default.json");
    let capability: Value = serde_json::from_str(
        &fs::read_to_string(capability_path).expect("capability should be readable"),
    )
    .expect("capability should contain valid JSON");

    assert_eq!(capability["windows"], serde_json::json!(["main"]));
    let permissions = capability["permissions"]
        .as_array()
        .expect("permissions should be an array");
    assert!(permissions
        .iter()
        .any(|permission| permission == "core:default"));

    let opener_permissions: Vec<&Value> = permissions
        .iter()
        .filter(|permission| {
            permission
                .as_str()
                .is_some_and(|value| value.starts_with("opener:"))
                || permission
                    .get("identifier")
                    .and_then(Value::as_str)
                    .is_some_and(|value| value.starts_with("opener:"))
        })
        .collect();

    assert_eq!(
        opener_permissions,
        vec![&serde_json::json!({
            "identifier": "opener:allow-open-url",
            "allow": [{ "url": RELEASES_URL }]
        })]
    );

    let serialized = serde_json::to_string(&capability).unwrap();
    assert!(!serialized.contains("opener:default"));
    assert!(!serialized.contains('*'));
    assert!(!serialized.contains("http://"));
    for forbidden in ["shell", "path", "reveal", "allow-open-path"] {
        assert!(!serialized.to_ascii_lowercase().contains(forbidden));
    }
}
