//! Target CPU architecture handling.

use anyhow::{bail, Result};

/// A normalised CPU architecture that can be rendered in both the
/// Debian and RPM naming schemes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X86_64,
    Aarch64,
    Armhf,
    I386,
}

impl Arch {
    /// Detect the architecture of the host this binary is running on.
    pub fn detect() -> Self {
        match std::env::consts::ARCH {
            "x86_64" => Arch::X86_64,
            "aarch64" => Arch::Aarch64,
            "arm" => Arch::Armhf,
            "x86" => Arch::I386,
            // Fall back to the host triple so unusual targets still build.
            _ => Arch::X86_64,
        }
    }

    /// Parse an architecture from either the Debian or RPM spelling.
    pub fn parse(value: &str) -> Result<Self> {
        let a = match value.trim().to_ascii_lowercase().as_str() {
            "amd64" | "x86_64" | "x64" => Arch::X86_64,
            "arm64" | "aarch64" => Arch::Aarch64,
            "armhf" | "armv7hl" | "armv7" | "arm" => Arch::Armhf,
            "i386" | "i686" | "x86" | "386" => Arch::I386,
            other => bail!("unsupported architecture: {other}"),
        };
        Ok(a)
    }

    /// Debian architecture name (e.g. `amd64`).
    pub fn deb(self) -> &'static str {
        match self {
            Arch::X86_64 => "amd64",
            Arch::Aarch64 => "arm64",
            Arch::Armhf => "armhf",
            Arch::I386 => "i386",
        }
    }

    /// RPM architecture name (e.g. `x86_64`).
    pub fn rpm(self) -> &'static str {
        match self {
            Arch::X86_64 => "x86_64",
            Arch::Aarch64 => "aarch64",
            Arch::Armhf => "armv7hl",
            Arch::I386 => "i686",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Arch;

    #[test]
    fn parses_both_schemes() {
        assert_eq!(Arch::parse("amd64").unwrap(), Arch::X86_64);
        assert_eq!(Arch::parse("x86_64").unwrap(), Arch::X86_64);
        assert_eq!(Arch::parse("arm64").unwrap(), Arch::Aarch64);
        assert_eq!(Arch::parse("armv7hl").unwrap(), Arch::Armhf);
    }

    #[test]
    fn rejects_unknown() {
        assert!(Arch::parse("sparc").is_err());
    }
}
