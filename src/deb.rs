//! Pure-Rust `.deb` (Debian binary package) writer.
//!
//! A `.deb` is an `ar` archive with three members, in order:
//! `debian-binary`, `control.tar.gz` and `data.tar.gz`. We assemble all three
//! in memory so no external tooling (`dpkg-deb`) is required.

use crate::arch::Arch;
use crate::config::Manifest;
use crate::files::{self, PayloadFile};
use crate::tar_util;
use anyhow::{Context, Result};
use md5::{Digest, Md5};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Build a `.deb` and return the path to the written file.
pub fn build(
    manifest: &Manifest,
    payload: &[PayloadFile],
    output_dir: &Path,
    arch: Arch,
) -> Result<PathBuf> {
    fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create output dir `{}`", output_dir.display()))?;

    let control = render_control(manifest, payload, arch);
    let md5sums = render_md5sums(payload)?;

    let control_tar = tar_util::tar_gz_bytes(|builder| {
        tar_util::append_dir(builder, "./", 0o755)?;
        tar_util::append_file(builder, "./control", 0o644, control.as_bytes())?;
        tar_util::append_file(builder, "./md5sums", 0o644, md5sums.as_bytes())?;
        Ok(())
    })?;

    let data_tar = tar_util::tar_gz_bytes(|builder| {
        for dir in files::parent_dirs(payload) {
            let name = format!("./{}/", dir.to_string_lossy().trim_start_matches('/'));
            tar_util::append_dir(builder, &name, 0o755)?;
        }
        for file in payload {
            let name = format!("./{}", file.relative());
            tar_util::append_file(builder, &name, file.mode, &fs::read(&file.source)?)?;
        }
        Ok(())
    })?;

    let mut archive = Vec::new();
    archive.write_all(b"!<arch>\n")?;
    append_ar(&mut archive, "debian-binary", b"2.0\n")?;
    append_ar(&mut archive, "control.tar.gz", &control_tar)?;
    append_ar(&mut archive, "data.tar.gz", &data_tar)?;

    let filename = format!(
        "{}_{}_{}.deb",
        manifest.package.name,
        manifest.package.version,
        arch.deb()
    );
    let path = output_dir.join(filename);
    fs::write(&path, &archive).with_context(|| format!("failed to write `{}`", path.display()))?;
    Ok(path)
}

/// Render the `control` metadata file for a package.
fn render_control(manifest: &Manifest, payload: &[PayloadFile], arch: Arch) -> String {
    let package = &manifest.package;
    let installed_kib: u64 = payload.iter().map(|f| f.size).sum::<u64>().div_ceil(1024);

    let mut out = String::new();
    out.push_str(&format!("Package: {}\n", package.name));
    out.push_str(&format!("Version: {}\n", package.version));
    out.push_str(&format!("Architecture: {}\n", arch.deb()));
    if let Some(maintainer) = &package.maintainer {
        out.push_str(&format!("Maintainer: {maintainer}\n"));
    }
    out.push_str(&format!("Installed-Size: {installed_kib}\n"));
    if let Some(section) = &manifest.deb.section {
        out.push_str(&format!("Section: {section}\n"));
    }
    if let Some(priority) = &manifest.deb.priority {
        out.push_str(&format!("Priority: {priority}\n"));
    }
    for (label, items) in [
        ("Depends", &manifest.deb.depends),
        ("Recommends", &manifest.deb.recommends),
        ("Suggests", &manifest.deb.suggests),
    ] {
        if !items.is_empty() {
            out.push_str(&format!("{label}: {}\n", items.join(", ")));
        }
    }
    if let Some(homepage) = &package.homepage {
        out.push_str(&format!("Homepage: {homepage}\n"));
    }
    out.push_str(&debian_description(&package.description));
    out.push_str("Original-Maintainer: linux-packager\n");
    out
}

/// Format a multi-line description using Debian's continuation convention.
fn debian_description(description: &str) -> String {
    let mut lines = description.lines();
    let summary = lines.next().unwrap_or("").trim();
    let mut out = format!("Description: {summary}\n");
    for line in lines {
        if line.trim().is_empty() {
            out.push_str(" .\n");
        } else {
            out.push_str(&format!(" {}\n", line.trim()));
        }
    }
    out
}

fn render_md5sums(payload: &[PayloadFile]) -> Result<String> {
    let mut out = String::new();
    for file in payload {
        let mut hasher = Md5::new();
        hasher.update(fs::read(&file.source)?);
        let digest = hasher.finalize();
        out.push_str(&format!("{:x}  {}\n", digest, file.relative()));
    }
    Ok(out)
}

/// Append one member to an `ar` archive in the common (`!<arch>`) format.
fn append_ar(out: &mut Vec<u8>, name: &str, data: &[u8]) -> Result<()> {
    anyhow::ensure!(name.len() <= 16, "ar member name too long: {name}");
    let header = format!(
        "{:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n",
        name,
        0, // mtime
        0, // uid
        0, // gid
        "100644",
        data.len()
    );
    anyhow::ensure!(header.len() == 60, "invalid ar header length");
    out.write_all(header.as_bytes())?;
    out.write_all(data)?;
    if data.len() % 2 == 1 {
        out.write_all(b"\n")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ar_header_is_60_bytes() {
        let mut out = Vec::new();
        append_ar(&mut out, "debian-binary", b"2.0\n").unwrap();
        // 60-byte header followed by 4 bytes of (even-length) data.
        assert_eq!(out.len(), 60 + 4);
        assert_eq!(&out[..14], b"debian-binary ");
        assert_eq!(&out[58..60], b"`\n");
    }

    #[test]
    fn description_indents_continuation_lines() {
        let rendered = debian_description("summary\n\nsecond line");
        assert_eq!(rendered, "Description: summary\n .\n second line\n");
    }
}
