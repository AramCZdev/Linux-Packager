//! `.rpm` builder.
//!
//! Writing the RPM binary format by hand is error-prone, so we generate a
//! standard `.spec` plus a source tarball and delegate the actual assembly to
//! `rpmbuild`. This keeps the produced packages byte-for-byte compatible with
//! the distribution toolchain.

use crate::arch::Arch;
use crate::config::Manifest;
use crate::files::{self, PayloadFile};
use crate::tar_util;
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Build an `.rpm` and return the path to the written file.
pub fn build(
    manifest: &Manifest,
    payload: &[PayloadFile],
    output_dir: &Path,
    arch: Arch,
) -> Result<PathBuf> {
    let rpmbuild = find_rpmbuild().context(
        "`rpmbuild` was not found on PATH; it is required to build .rpm packages \
         (install the `rpm`/`rpm-build` package for your distro)",
    )?;

    fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create output dir `{}`", output_dir.display()))?;

    let topdir = tempfile::tempdir().context("failed to create temporary build directory")?;
    for sub in ["SOURCES", "SPECS", "BUILD", "RPMS", "SRPMS", "BUILDROOT"] {
        fs::create_dir_all(topdir.path().join(sub))?;
    }

    let name = &manifest.package.name;
    let version = &manifest.package.version;
    let source_name = format!("{name}-{version}");

    write_source_tarball(&topdir.path().join("SOURCES"), &source_name, payload)?;

    let spec_path = topdir.path().join("SPECS").join(format!("{name}.spec"));
    fs::write(&spec_path, render_spec(manifest, payload, arch))
        .with_context(|| format!("failed to write spec `{}`", spec_path.display()))?;

    let status = Command::new(&rpmbuild)
        .arg("--define")
        .arg(format!("_topdir {}", topdir.path().display()))
        .arg("-bb")
        .arg(&spec_path)
        .status()
        .context("failed to launch rpmbuild")?;
    if !status.success() {
        bail!("rpmbuild failed with status {status}");
    }

    let rpm_dir = topdir.path().join("RPMS").join(arch.rpm());
    let produced = fs::read_dir(&rpm_dir)
        .with_context(|| format!("rpmbuild produced no output in `{}`", rpm_dir.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|ext| ext == "rpm"))
        .with_context(|| format!("no .rpm found in `{}`", rpm_dir.display()))?;

    let dest = output_dir.join(
        produced
            .file_name()
            .context("produced rpm has no file name")?,
    );
    fs::copy(&produced, &dest)
        .with_context(|| format!("failed to copy rpm to `{}`", dest.display()))?;
    Ok(dest)
}

/// Package the payload into a `<name>-<version>/` prefixed source tarball.
fn write_source_tarball(
    sources_dir: &Path,
    source_name: &str,
    payload: &[PayloadFile],
) -> Result<()> {
    let archive = tar_util::tar_gz_bytes(|builder| {
        tar_util::append_dir(builder, &format!("{source_name}/"), 0o755)?;
        for dir in files::parent_dirs(payload) {
            let name = format!(
                "{source_name}/{}/",
                dir.to_string_lossy().trim_start_matches('/')
            );
            tar_util::append_dir(builder, &name, 0o755)?;
        }
        for file in payload {
            let name = format!("{source_name}/{}", file.relative());
            tar_util::append_file(builder, &name, file.mode, &fs::read(&file.source)?)?;
        }
        Ok(())
    })?;

    let path = sources_dir.join(format!("{source_name}.tar.gz"));
    fs::write(&path, archive)
        .with_context(|| format!("failed to write source tarball `{}`", path.display()))?;
    Ok(())
}

/// Render a complete `.spec` file describing the package.
fn render_spec(manifest: &Manifest, payload: &[PayloadFile], arch: Arch) -> String {
    let package = &manifest.package;
    let mut spec = String::new();

    spec.push_str(&format!("Name:           {}\n", package.name));
    spec.push_str(&format!("Version:        {}\n", package.version));
    spec.push_str(&format!(
        "Release:        {}%{{?dist}}\n",
        manifest.rpm_release()
    ));
    spec.push_str(&format!("Summary:        {}\n", package.summary()));
    spec.push_str(&format!(
        "License:        {}\n",
        package.license.as_deref().unwrap_or("Proprietary")
    ));
    if let Some(homepage) = &package.homepage {
        spec.push_str(&format!("URL:            {homepage}\n"));
    }
    spec.push_str(&format!("BuildArch:      {}\n", arch.rpm()));
    if let Some(group) = &manifest.rpm.group {
        spec.push_str(&format!("Group:          {group}\n"));
    }
    for requirement in &manifest.rpm.requires {
        spec.push_str(&format!("Requires:       {requirement}\n"));
    }
    spec.push('\n');

    spec.push_str("%description\n");
    spec.push_str(package.description.trim_end());
    spec.push_str("\n\n");

    spec.push_str("%prep\n%setup -q\n\n");
    spec.push_str("%build\n\n");
    spec.push_str("%install\n");
    spec.push_str("rm -rf %{buildroot}\n");
    spec.push_str("mkdir -p %{buildroot}\n");
    spec.push_str("cp -a . %{buildroot}/\n\n");

    spec.push_str("%files\n%defattr(-,root,root,-)\n");
    for file in payload {
        spec.push_str(&file.dest.to_string_lossy());
        spec.push('\n');
    }

    spec.push_str("\n%changelog\n");
    spec.push_str(&format!(
        "* {} - {}-{}\n- Package built by linux-packager\n",
        build_date(),
        package.version,
        manifest.rpm_release()
    ));
    spec
}

/// Current date formatted as `Day Mon DD YYYY` for the changelog entry.
fn build_date() -> String {
    let output = Command::new("date").arg("+%a %b %d %Y").output();
    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        _ => "Thu Jan 01 1970".to_string(),
    }
}

/// Locate `rpmbuild` by walking `PATH`.
fn find_rpmbuild() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join("rpmbuild"))
        .find(|candidate| candidate.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::PayloadFile;
    use std::path::PathBuf;

    fn sample_manifest() -> Manifest {
        let toml = r#"
[package]
name = "myapp"
version = "1.0.0"
description = "Test app\n\nSecond paragraph"
license = "MIT"

[binary]
path = "myapp"

[rpm]
group = "Applications/System"
requires = ["glibc >= 2.31"]
"#;
        toml::from_str(toml).unwrap()
    }

    fn sample_payload() -> Vec<PayloadFile> {
        vec![PayloadFile {
            dest: PathBuf::from("/usr/bin/myapp"),
            source: PathBuf::from("/tmp/does-not-matter"),
            mode: 0o755,
            size: 10,
        }]
    }

    #[test]
    fn spec_has_expected_fields() {
        let spec = render_spec(&sample_manifest(), &sample_payload(), Arch::X86_64);
        assert!(spec.contains("Name:           myapp\n"));
        assert!(spec.contains("Version:        1.0.0\n"));
        assert!(spec.contains("Release:        1%{?dist}\n"));
        assert!(spec.contains("BuildArch:      x86_64\n"));
        assert!(spec.contains("Requires:       glibc >= 2.31\n"));
        assert!(spec.contains("%files\n"));
        assert!(spec.contains("/usr/bin/myapp\n"));
        assert!(spec.contains("%changelog\n"));
    }

    #[test]
    fn source_tarball_contains_relative_layout() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("myapp");
        std::fs::write(&src, b"binary").unwrap();
        let payload = vec![PayloadFile {
            dest: PathBuf::from("/usr/bin/myapp"),
            source: src,
            mode: 0o755,
            size: 6,
        }];
        let out = dir.path().join("SOURCES");
        std::fs::create_dir_all(&out).unwrap();
        write_source_tarball(&out, "myapp-1.0.0", &payload).unwrap();

        let tarball = out.join("myapp-1.0.0.tar.gz");
        assert!(tarball.exists());

        let file = std::fs::File::open(&tarball).unwrap();
        let gz = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(gz);
        let names: Vec<String> = archive
            .entries()
            .unwrap()
            .map(|entry| {
                entry
                    .unwrap()
                    .path()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert!(names.iter().any(|n| n == "myapp-1.0.0/usr/bin/myapp"));
    }
}
