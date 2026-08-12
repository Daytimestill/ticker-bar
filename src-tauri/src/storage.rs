use std::{
    error::Error,
    fmt, fs,
    io::Write,
    path::{Path, PathBuf},
};

use crate::AppConfig;

pub fn load_config(path: &Path) -> Result<Option<AppConfig>, StorageError> {
    if !path.exists() {
        return Ok(None);
    }

    let contents = fs::read_to_string(path).map_err(|source| StorageError::Read {
        path: path.to_owned(),
        source,
    })?;
    let config =
        crate::config::parse_config_json(&contents).map_err(|source| StorageError::Parse {
            path: path.to_owned(),
            source,
        })?;
    config.validate().map_err(StorageError::Invalid)?;

    Ok(Some(config))
}

pub fn save_config(path: &Path, config: &AppConfig) -> Result<(), StorageError> {
    config.validate().map_err(StorageError::Invalid)?;
    let parent = path.parent().ok_or_else(|| StorageError::MissingParent {
        path: path.to_owned(),
    })?;
    fs::create_dir_all(parent).map_err(|source| StorageError::CreateDirectory {
        path: parent.to_owned(),
        source,
    })?;

    let temporary_path = path.with_extension("json.tmp");
    let contents = serde_json::to_vec_pretty(config).map_err(StorageError::Serialize)?;
    let mut temporary =
        fs::File::create(&temporary_path).map_err(|source| StorageError::Write {
            path: temporary_path.clone(),
            source,
        })?;
    // 配置仅供当前用户使用；提前收紧权限，未来引入券商凭证时不留缺口。
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temporary
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|source| StorageError::Write {
                path: temporary_path.clone(),
                source,
            })?;
    }
    temporary
        .write_all(&contents)
        .and_then(|_| temporary.sync_all())
        .map_err(|source| StorageError::Write {
            path: temporary_path.clone(),
            source,
        })?;
    fs::rename(&temporary_path, path).map_err(|source| StorageError::Commit {
        from: temporary_path,
        to: path.to_owned(),
        source,
    })
}

#[derive(Debug)]
pub enum StorageError {
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    Invalid(crate::DomainError),
    MissingParent {
        path: PathBuf,
    },
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    Serialize(serde_json::Error),
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    Commit {
        from: PathBuf,
        to: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(formatter, "failed to read {}: {source}", path.display())
            }
            Self::Parse { path, source } => {
                write!(formatter, "failed to parse {}: {source}", path.display())
            }
            Self::Invalid(source) => write!(formatter, "invalid config: {source}"),
            Self::MissingParent { path } => {
                write!(formatter, "config path has no parent: {}", path.display())
            }
            Self::CreateDirectory { path, source } => {
                write!(
                    formatter,
                    "failed to create config directory {}: {source}",
                    path.display()
                )
            }
            Self::Serialize(source) => write!(formatter, "failed to serialize config: {source}"),
            Self::Write { path, source } => {
                write!(formatter, "failed to write {}: {source}", path.display())
            }
            Self::Commit { from, to, source } => write!(
                formatter,
                "failed to commit {} to {}: {source}",
                from.display(),
                to.display()
            ),
        }
    }
}

impl Error for StorageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Read { source, .. }
            | Self::CreateDirectory { source, .. }
            | Self::Write { source, .. }
            | Self::Commit { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::Invalid(source) => Some(source),
            Self::Serialize(source) => Some(source),
            Self::MissingParent { .. } => None,
        }
    }
}
