use linux_packager::arch::Arch;
use linux_packager::config::Manifest;
use linux_packager::{deb, files};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

fn write_executable(path: &std::path::Path, contents: &[u8]) {
    fs::write(path, contents).unwrap();
    let mut perms = fs::metadata(path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).unwrap();
}

fn setup() -> (tempfile::TempDir, Manifest) {
    let dir = tempfile::tempdir().unwrap();
    write_executable(&dir.path().join("myapp"), b"#!/bin/sh\necho hi\n");
    fs::create_dir_all(dir.path().join("assets")).unwrap();
    fs::write(dir.path().join("assets/app.toml"), b"key = 'value'\n").unwrap();

    let manifest_text = r#"
[package]
name = "myapp"
version = "1.0.0"
description = "Test app"
maintainer = "Tester <test@example.com>"
homepage = "https://example.com"

[binary]
path = "myapp"

[deb]
section = "utils"
priority = "optional"
depends = ["libc6 (>= 2.31)"]

[[files]]
source = "assets/app.toml"
dest = "/etc/myapp/app.toml"
mode = 0o644
"#;
    fs::write(dir.path().join("LinuxPackager.toml"), manifest_text).unwrap();
    let manifest = Manifest::load(&dir.path().join("LinuxPackager.toml")).unwrap();
    (dir, manifest)
}

#[test]
fn builds_valid_deb_layout() {
    let (dir, manifest) = setup();
    let payload = files::collect(&manifest, dir.path()).unwrap();
    let out = dir.path().join("dist");
    let deb_path = deb::build(&manifest, &payload, &out, Arch::X86_64).unwrap();

    assert_eq!(
        deb_path.file_name().unwrap().to_str().unwrap(),
        "myapp_1.0.0_amd64.deb"
    );

    // Every `.deb` is an ar archive with these three members, in order.
    let mut header = [0u8; 8];
    use std::io::Read;
    fs::File::open(&deb_path)
        .unwrap()
        .read_exact(&mut header)
        .unwrap();
    assert_eq!(&header, b"!<arch>\n");

    if let Ok(output) = Command::new("ar").arg("t").arg(&deb_path).output() {
        if output.status.success() {
            let members: Vec<String> = String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::to_string)
                .collect();
            assert_eq!(members, ["debian-binary", "control.tar.gz", "data.tar.gz"]);
        }
    }
}

#[test]
fn deb_control_is_readable_by_dpkg() {
    if Command::new("dpkg-deb").arg("--version").output().is_err() {
        eprintln!("skipping: dpkg-deb not available");
        return;
    }
    let (dir, manifest) = setup();
    let payload = files::collect(&manifest, dir.path()).unwrap();
    let out = dir.path().join("dist");
    let deb_path = deb::build(&manifest, &payload, &out, Arch::X86_64).unwrap();

    let output = Command::new("dpkg-deb")
        .arg("--field")
        .arg(&deb_path)
        .arg("Package")
        .output()
        .unwrap();
    assert!(output.status.success(), "dpkg-deb rejected the package");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "myapp");

    // Extract and confirm both files land where the manifest asked.
    let extracted = dir.path().join("extracted");
    let status = Command::new("dpkg-deb")
        .arg("--extract")
        .arg(&deb_path)
        .arg(&extracted)
        .status()
        .unwrap();
    assert!(status.success());
    assert!(extracted.join("usr/bin/myapp").is_file());
    assert!(extracted.join("etc/myapp/app.toml").is_file());
}
