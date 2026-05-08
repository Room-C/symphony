use std::path::{Component, Path, PathBuf};

use regex::Regex;

use crate::error::{Result, SymphonyError};

pub fn sanitize_workspace_key(identifier: &str) -> String {
    let re = Regex::new(r"[^A-Za-z0-9._-]").unwrap();
    let sanitized = re.replace_all(identifier, "_").to_string();
    if sanitized.is_empty() {
        "issue".to_string()
    } else {
        sanitized
    }
}

pub fn ensure_workspace_child(root: &Path, key: &str) -> Result<PathBuf> {
    if key.is_empty() || key.contains(std::path::MAIN_SEPARATOR) {
        return Err(SymphonyError::WorkspaceSafety {
            message: format!("invalid workspace key {key:?}"),
        });
    }
    let root = normalize_absolute(root)?;
    let path = normalize_absolute(&root.join(key))?;
    ensure_prefix(&root, &path)?;
    Ok(path)
}

pub fn ensure_cwd(expected_workspace: &Path, cwd: &Path) -> Result<()> {
    let expected = normalize_absolute(expected_workspace)?;
    let actual = normalize_absolute(cwd)?;
    if expected != actual {
        return Err(SymphonyError::WorkspaceSafety {
            message: format!(
                "coding-agent cwd must be workspace path; expected {}, got {}",
                expected.display(),
                actual.display()
            ),
        });
    }
    Ok(())
}

pub fn ensure_prefix(root: &Path, path: &Path) -> Result<()> {
    if !path.starts_with(root) {
        return Err(SymphonyError::WorkspaceSafety {
            message: format!(
                "workspace path {} is outside root {}",
                path.display(),
                root.display()
            ),
        });
    }
    Ok(())
}

pub fn normalize_absolute(path: &Path) -> Result<PathBuf> {
    let mut absolute = if path.is_absolute() {
        PathBuf::from(path)
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    absolute = normalized;
    Ok(absolute)
}
