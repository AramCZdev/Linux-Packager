//! Collecting the files that make up a package payload.

use crate::config::Manifest;
use anyhow::{bail, Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// A single file destined for a package, with an absolute installed path.
#[derive(Debug, Clone)]
pub struct PayloadFile {
    /// Absolute in-package path, e.g. `/usr/bin/myapp`.
    pub dest: PathBuf,
    /// Path of the on-disk source file.
    pub source: PathBuf,
    /// Unix permission bits.
    pub mode: u32,
    /// Size in bytes.
    pub size: u64,
}

impl PayloadFile {
    /// The destination without the leading slash, e.g. `usr/bin/myapp`.
    pub fn relative(&self) -> String {
        self.dest
            .to_string_lossy()
            .trim_start_matches('/')
            .to_string()
    }
}

/// Resolve every payload entry in the manifest, reading permissions and sizes
/// from disk. Relative paths are resolved against `base`.
pub fn collect(manifest: &Manifest, base: &Path) -> Result<Vec<PayloadFile>> {
    let mut payload = Vec::new();

    if let Some(binary) = &manifest.binary {
        let source = base.join(&binary.path);
        if !source.is_file() {
            bail!(
                "binary `{}` does not exist (resolved to `{}`)",
                binary.path.display(),
                source.display()
            );
        }
        let install_dir = binary
            .install_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("/usr/bin"));
        let file_name = source.file_name().context("binary path has no file name")?;
        let mode = fs::metadata(&source)?.permissions().mode() & 0o7777;
        let dest = normalize_dest(&install_dir.join(file_name))?;
        payload.push(PayloadFile {
            dest,
            size: fs::metadata(&source)?.len(),
            // Ensure the executable bit survives even if the source lost it.
            mode: mode | 0o111,
            source,
        });
    }

    for entry in &manifest.files {
        let source = base.join(&entry.source);
        if !source.is_file() {
            bail!(
                "file `{}` does not exist (resolved to `{}`)",
                entry.source.display(),
                source.display()
            );
        }
        let metadata = fs::metadata(&source)?;
        let mode = entry
            .mode
            .unwrap_or_else(|| metadata.permissions().mode() & 0o7777);
        payload.push(PayloadFile {
            dest: normalize_dest(&entry.dest)?,
            source,
            mode,
            size: metadata.len(),
        });
    }

    payload.sort_by(|a, b| a.dest.cmp(&b.dest));
    payload.dedup_by(|a, b| a.dest == b.dest);
    Ok(payload)
}

/// All parent directories of the payload, as absolute paths, sorted and
/// de-duplicated (excluding `/`).
pub fn parent_dirs(payload: &[PayloadFile]) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for file in payload {
        let mut current = file.dest.parent();
        while let Some(dir) = current {
            if dir.as_os_str().is_empty() || dir == Path::new("/") {
                break;
            }
            dirs.push(dir.to_path_buf());
            current = dir.parent();
        }
    }
    dirs.sort();
    dirs.dedup();
    dirs
}

fn normalize_dest(path: &Path) -> Result<PathBuf> {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        Path::new("/").join(path)
    };
    let text = joined.to_string_lossy();
    if text.contains("..") {
        bail!("destination path `{}` may not contain `..`", path.display());
    }
    Ok(PathBuf::from(text.into_owned()))
}
