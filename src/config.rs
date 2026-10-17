//! Manifest (`LinuxPackager.toml`) parsing and defaults.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

/// The default file name looked for when none is given on the command line.
pub const DEFAULT_MANIFEST: &str = "LinuxPackager.toml";

/// Root of a parsed manifest.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub package: Package,
    #[serde(default)]
    pub binary: Option<Binary>,
    #[serde(default)]
    pub deb: DebOptions,
    #[serde(default)]
    pub rpm: RpmOptions,
    /// Extra files installed alongside the binary.
    #[serde(default)]
    pub files: Vec<FileEntry>,
}

/// `[package]` table: identity and metadata of the produced package.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    pub name: String,
    pub version: String,
    /// One line summary; the first line is used as the RPM `Summary`.
    pub description: String,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub maintainer: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    /// Overrides host detection. Accepts Debian or RPM spellings.
    #[serde(default)]
    pub arch: Option<String>,
    /// RPM release number (defaults to `1`).
    #[serde(default)]
    pub release: Option<String>,
}

impl Package {
    /// Short single-line description.
    pub fn summary(&self) -> &str {
        self.description.lines().next().unwrap_or("").trim()
    }
}

/// `[binary]` table describing the executable to ship.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binary {
    /// Path to the executable, relative to the manifest directory.
    pub path: PathBuf,
    /// Directory the binary is installed into (defaults to `/usr/bin`).
    #[serde(default)]
    pub install_dir: Option<PathBuf>,
}

/// `[[files]]` entry installing an arbitrary file to an absolute path.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileEntry {
    /// Source file, relative to the manifest directory.
    pub source: PathBuf,
    /// Absolute destination path inside the package (e.g. `/etc/app/x.toml`).
    pub dest: PathBuf,
    /// Unix mode in octal (e.g. `0o644`). Defaults to the source's mode.
    #[serde(default)]
    pub mode: Option<u32>,
}

/// `[deb]` table of Debian-specific options.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebOptions {
    /// Whether `.deb` output is built (default `true`).
    #[serde(default = "default_enabled", deserialize_with = "deserialize_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub depends: Vec<String>,
    #[serde(default)]
    pub recommends: Vec<String>,
    #[serde(default)]
    pub suggests: Vec<String>,
    #[serde(default)]
    pub section: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
}

impl Default for DebOptions {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            depends: Vec::new(),
            recommends: Vec::new(),
            suggests: Vec::new(),
            section: None,
            priority: None,
        }
    }
}

/// `[rpm]` table of RPM-specific options.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RpmOptions {
    /// Whether `.rpm` output is built (default `true`).
    #[serde(default = "default_enabled", deserialize_with = "deserialize_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub group: Option<String>,
}

impl Default for RpmOptions {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            requires: Vec::new(),
            group: None,
        }
    }
}

fn default_enabled() -> bool {
    true
}

/// Accept `enabled = true` or `enabled = "true"` (and the `false` forms).
fn deserialize_enabled<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Enabled {
        Bool(bool),
        Text(String),
    }

    match Enabled::deserialize(deserializer)? {
        Enabled::Bool(value) => Ok(value),
        Enabled::Text(value) => match value.trim().to_ascii_lowercase().as_str() {
            "true" => Ok(true),
            "false" => Ok(false),
            other => Err(D::Error::custom(format!(
                "`enabled` must be true/false, got {other:?}"
            ))),
        },
    }
}

impl Manifest {
    /// Read and validate a manifest from disk.
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("failed to read manifest `{}`", path.display()))?;
        let manifest: Manifest = toml::from_str(&text)
            .with_context(|| format!("failed to parse manifest `{}`", path.display()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    fn validate(&self) -> Result<()> {
        if self.package.name.trim().is_empty() {
            bail!("`package.name` must not be empty");
        }
        if self.package.version.trim().is_empty() {
            bail!("`package.version` must not be empty");
        }
        if self.package.description.trim().is_empty() {
            bail!("`package.description` must not be empty");
        }
        if self.binary.is_none() && self.files.is_empty() {
            bail!("nothing to package: provide a `[binary]` and/or `[[files]]` entries");
        }
        Ok(())
    }

    /// Full RPM release string, defaulting to `1`.
    pub fn rpm_release(&self) -> &str {
        self.package.release.as_deref().unwrap_or("1")
    }
}

/// A starter manifest written by `linux-packager init`.
pub fn template() -> String {
    r#"# Linux Packager manifest.
[package]
name = "myapp"
version = "0.1.0"
description = "A short one-line summary of myapp"
license = "MIT"
maintainer = "Your Name <you@example.com>"
homepage = "https://example.com"
# arch = "amd64"   # defaults to the host architecture
# release = "1"    # RPM release number

[binary]
path = "target/release/myapp"
# install_dir = "/usr/bin"

[deb]
enabled = true
section = "utils"
priority = "optional"
# depends = ["libc6 (>= 2.31)"]

[rpm]
enabled = true
group = "Applications/System"
# requires = ["glibc"]

# [[files]]
# source = "assets/config.toml"
# dest = "/etc/myapp/config.toml"
# mode = 0o644
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(toml: &str) -> Manifest {
        toml::from_str(toml).unwrap()
    }

    const BASE: &str = r#"
[package]
name = "myapp"
version = "1.0.0"
description = "desc"
"#;

    #[test]
    fn enabled_defaults_to_true() {
        let m = manifest(BASE);
        assert!(m.deb.enabled);
        assert!(m.rpm.enabled);
    }

    #[test]
    fn enabled_accepts_string_forms() {
        let m = manifest(&format!(
            "{BASE}\n[deb]\nenabled = \"false\"\n[rpm]\nenabled = \"true\"\n"
        ));
        assert!(!m.deb.enabled);
        assert!(m.rpm.enabled);
    }

    #[test]
    fn enabled_accepts_bool_forms() {
        let m = manifest(&format!(
            "{BASE}\n[deb]\nenabled = false\n[rpm]\nenabled = true\n"
        ));
        assert!(!m.deb.enabled);
        assert!(m.rpm.enabled);
    }

    #[test]
    fn enabled_rejects_other_strings() {
        let bad = toml::from_str::<Manifest>(&format!("{BASE}\n[deb]\nenabled = \"yes\"\n"));
        assert!(bad.is_err());
    }
}
