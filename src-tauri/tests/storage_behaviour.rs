use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use tickerbar_core::{AppConfig, DisplayPreset, apply_display_preset, load_config, save_config};

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

fn temporary_path(path: &Path) -> PathBuf {
    path.with_extension("json.tmp")
}
