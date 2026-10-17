//! Verifies the `rpmbuild` orchestration without requiring a real rpm toolchain
//! by putting a stub `rpmbuild` on `PATH`. This file is its own process, so
//! mutating `PATH` cannot race with other tests.

use linux_packager::arch::Arch;
use linux_packager::config::Manifest;
use linux_packager::files::PayloadFile;
use linux_packager::rpm;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

#[test]
fn invokes_rpmbuild_and_collects_output() {
    let dir = tempfile::tempdir().unwrap();

    // A fake rpmbuild that honours `--define _topdir <dir>` and drops a file
    // where the real tool would.
    let bin_dir = dir.path().join("fakebin");
    fs::create_dir_all(&bin_dir).unwrap();
    let fake = bin_dir.join("rpmbuild");
    fs::write(
        &fake,
        r#"#!/bin/sh
topdir=""
while [ $# -gt 0 ]; do
  case "$1" in
    _topdir*) topdir="${1#_topdir }" ;;
  esac
  shift
done
mkdir -p "$topdir/RPMS/x86_64"
echo "fake rpm" > "$topdir/RPMS/x86_64/myapp-1.0.0-1.x86_64.rpm"
exit 0
"#,
    )
    .unwrap();
    let mut perms = fs::metadata(&fake).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&fake, perms).unwrap();

    let old_path = std::env::var_os("PATH").unwrap_or_default();
    std::env::set_var(
        "PATH",
        format!("{}:{}", bin_dir.display(), old_path.to_string_lossy()),
    );

    let exe = dir.path().join("myapp");
    fs::write(&exe, b"binary").unwrap();

    let manifest: Manifest = toml::from_str(
        r#"
[package]
name = "myapp"
version = "1.0.0"
description = "Test app"
license = "MIT"

[binary]
path = "myapp"
"#,
    )
    .unwrap();

    let payload = vec![PayloadFile {
        dest: PathBuf::from("/usr/bin/myapp"),
        source: exe,
        mode: 0o755,
        size: 6,
    }];

    let out = dir.path().join("dist");
    let result = rpm::build(&manifest, &payload, &out, Arch::X86_64).unwrap();

    assert_eq!(
        result.file_name().unwrap().to_str().unwrap(),
        "myapp-1.0.0-1.x86_64.rpm"
    );
    assert!(result.exists());

    std::env::set_var("PATH", old_path);
}
