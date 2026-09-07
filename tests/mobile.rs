#![cfg(feature = "embedded-git")]
use serde_json::json;
use synchrogit::mobile::{Engine, Request};

#[test]
fn mobile_config_roundtrips_and_rejects_invalid_settings() {
    let root = tempfile::tempdir().unwrap();
    let mut engine = Engine::new().unwrap();
    let settings = json!({"defaults": {"interval": "20s", "git-timeout": "45s"}, "repo": [
        {"name": "notes", "path": root.path().to_str().unwrap(), "auto-push": false, "ignore": ["cache/**"]}
    ]});
    let source = engine
        .call(Request::EncodeConfig {
            settings: settings.clone(),
        })
        .unwrap()["config"]
        .as_str()
        .unwrap()
        .to_owned();
    let decoded = engine
        .call(Request::DecodeConfig { config: source })
        .unwrap();
    assert_eq!(decoded, settings);
    let mut invalid = settings;
    invalid["defaults"]["interval"] = json!("0s");
    assert!(
        engine
            .call(Request::EncodeConfig { settings: invalid })
            .unwrap_err()
            .to_string()
            .contains("greater than zero")
    );
    assert!(
        !engine.call(Request::Status).unwrap()["running"]
            .as_bool()
            .unwrap()
    );
    assert!(engine.call(Request::Sync).is_err());
}
