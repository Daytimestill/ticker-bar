use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use tickerbar_core::{
    AppConfig, CONFIG_SCHEMA_VERSION, DisplayPreset, StorageError, apply_display_preset,
    load_config, save_config,
};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after the Unix epoch")
            .as_nanos();
        Self(std::env::temp_dir().join(format!("tickerbar-{name}-{unique}")))
    }

    fn config_path(&self) -> PathBuf {
        self.0.join("nested").join("config.json")
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn missing_config_is_not_an_error() {
    let directory = TestDirectory::new("missing");

    let loaded = load_config(&directory.config_path()).expect("missing config should be accepted");

    assert_eq!(loaded, None);
}

#[test]
fn saves_and_loads_the_complete_configuration() {
    let directory = TestDirectory::new("roundtrip");
    let path = directory.config_path();
    let default_config = AppConfig::default();
    let config = AppConfig {
        display: tickerbar_core::DisplayConfig {
            items: apply_display_preset(DisplayPreset::Position),
            separator: " · ".into(),
            ..default_config.display
        },
        ..default_config
    };

    save_config(&path, &config).expect("config should save");
    let restored = load_config(&path)
        .expect("config should load")
        .expect("saved config should exist");

    assert_eq!(restored, config);
    assert!(!temporary_path(&path).exists());
}

#[cfg(unix)]
#[test]
fn saves_the_config_readable_only_by_the_current_user() {
    use std::os::unix::fs::PermissionsExt;

    let directory = TestDirectory::new("permissions");
    let path = directory.config_path();

    save_config(&path, &AppConfig::default()).expect("config should save");

    let mode = fs::metadata(&path)
        .expect("saved config should exist")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn reports_malformed_json_instead_of_silently_resetting_preferences() {
    let directory = TestDirectory::new("malformed");
    let path = directory.config_path();
    fs::create_dir_all(path.parent().expect("config should have a parent"))
        .expect("test directory should be created");
    fs::write(&path, "{not-json").expect("fixture should be written");

    let error = load_config(&path).expect_err("malformed config should be reported");

    assert!(error.to_string().contains("parse"));
}

#[test]
fn rejects_a_newer_schema_without_touching_the_original_config() {
    let directory = TestDirectory::new("newer-schema");
    let path = directory.config_path();
    fs::create_dir_all(path.parent().expect("config should have a parent"))
        .expect("test directory should be created");
    let mut json = serde_json::to_value(AppConfig::default()).expect("fixture should serialize");
    json["schemaVersion"] = serde_json::json!(CONFIG_SCHEMA_VERSION + 1);
    json["display"]["items"][0]["metric"] = serde_json::json!("futureMetric");
    let original = serde_json::to_vec_pretty(&json).expect("fixture should serialize");
    fs::write(&path, &original).expect("fixture should be written");

    let error = load_config(&path).expect_err("newer config must not be opened by an older app");

    match error {
        StorageError::UnsupportedSchema {
            found, supported, ..
        } => {
            assert_eq!(found, CONFIG_SCHEMA_VERSION + 1);
            assert_eq!(supported, CONFIG_SCHEMA_VERSION);
        }
        other => panic!("expected unsupported schema error, got {other}"),
    }
    assert_eq!(
        fs::read(&path).expect("original config should remain readable"),
        original,
        "a downgrade must never rewrite the newer config"
    );
}

#[test]
fn saving_over_an_existing_config_keeps_the_previous_version_as_a_backup() {
    let directory = TestDirectory::new("backup");
    let path = directory.config_path();
    let original = AppConfig::default();
    save_config(&path, &original).expect("first config should save");

    let mut updated = original.clone();
    updated.tray_throttle_ms = 8_000;
    save_config(&path, &updated).expect("updated config should save");

    let backup = load_config(&backup_path(&path))
        .expect("backup should load")
        .expect("backup should exist");
    assert_eq!(backup, original);
    assert_eq!(
        load_config(&path).expect("current config should load"),
        Some(updated)
    );
}

fn temporary_path(path: &Path) -> PathBuf {
    path.with_extension("json.tmp")
}

fn backup_path(path: &Path) -> PathBuf {
    path.with_extension("json.bak")
}
