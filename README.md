# Linux Packager

An app that packages Linux executables into installable package formats.

Given a single executable (and optionally extra files), Linux Packager produces
native installers from one declarative manifest:

- **`.deb`** — built entirely in Rust, no `dpkg-deb` required.
- **`.rpm`** — generated as a standard `.spec` and assembled with `rpmbuild`.

## Download

Grab the portable binary from the latest [Release](../../releases). It is fully
static — no dependencies, no install step:

```sh
chmod +x linux-packager
sudo mv linux-packager /usr/local/bin/
```

## Install from source

```sh
cargo install --path .
```

## Usage

Create a starter manifest:

```sh
linux-packager init
```

That writes a `LinuxPackager.toml`:

```toml
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
enabled = false
group = "Applications/System"
# requires = ["glibc"]

# [[files]]
# source = "assets/config.toml"
# dest = "/etc/myapp/config.toml"
# mode = 0o644
```

Build every supported format into `dist/`:

```sh
linux-packager build
```

Choose formats, output directory, or a manifest explicitly:

```sh
linux-packager build --manifest path/to/LinuxPackager.toml \
                     --format deb --format rpm \
                     --output dist
```

Relative paths in the manifest are resolved against the manifest's directory
(override with `--root`).

## Manifest reference

The manifest is a `LinuxPackager.toml` sitting next to your project. Only
`[package]` and `[binary]` are required — everything else has defaults.

Unknown keys are rejected, so a typo fails loudly instead of silently building
a mis-configured package.

### `[package]`

Identity and metadata of the package.

| Key | Required | Default | Used by | Description |
| --- | --- | --- | --- | --- |
| `name` | yes | — | deb, rpm | Package name; also part of the output filename. |
| `version` | yes | — | deb, rpm | Package version (e.g. `0.1.0`); also part of the output filename. |
| `description` | yes | — | deb, rpm | First line is the short summary (`Description:` / `Summary:`); any extra lines become the long description. |
| `license` | no | `Proprietary` | rpm | SPDX identifier, e.g. `MIT`, `GPL-3.0-or-later`. |
| `maintainer` | no | — | deb | `Name <email@example.com>`, written to the deb `Maintainer:` field. |
| `homepage` | no | — | deb, rpm | Project website — `Homepage:` for deb, `URL:` for rpm. |
| `arch` | no | host | deb, rpm | Target architecture. Both spellings work: `amd64`/`x86_64`, `arm64`/`aarch64`, `armhf`/`armv7hl`, `i386`/`i686`. |
| `release` | no | `1` | rpm | RPM release number; the built file is `name-version-release.arch.rpm`. |

```toml
[package]
name = "ghostab"
version = "3.0.2"
description = "A ghost-themed tab manager"
license = "MIT"
maintainer = "Aram <aram@example.com>"
homepage = "https://example.com"
# arch = "amd64"     # defaults to the host you build on
# release = "1"
```

### `[binary]`

The executable to ship.

| Key | Required | Default | Description |
| --- | --- | --- | --- |
| `path` | yes | — | Path to the executable, relative to the manifest. It must already exist — build it first. |
| `install_dir` | no | `/usr/bin` | Where the executable is installed. The filename is kept from `path`, so `bin/ghostab` → `/usr/bin/ghostab`. |

```toml
[binary]
path = "target/release/ghostab"
# install_dir = "/usr/local/bin"   # avoids clashing with an existing package
```

### `[[files]]`

Install extra files (configs, icons, man pages…). Repeat the block for each
file.

| Key | Required | Default | Description |
| --- | --- | --- | --- |
| `source` | yes | — | File to ship, relative to the manifest. |
| `dest` | yes | — | Absolute path inside the package, e.g. `/etc/ghostab/config.toml`. Must start with `/` and may not contain `..`. |
| `mode` | no | source's mode | Unix permissions in octal, e.g. `0o644`. |

```toml
[[files]]
source = "assets/config.toml"
dest = "/etc/ghostab/config.toml"
mode = 0o644

[[files]]
source = "assets/icon.png"
dest = "/usr/share/pixmaps/ghostab.png"
```

### `[deb]` — Debian options

| Key | Required | Default | Description |
| --- | --- | --- | --- |
| `enabled` | no | `true` | Set `false` to skip `.deb` output in a plain `build`. |
| `depends` | no | `[]` | Debian dependency strings, e.g. `["libc6 (>= 2.31)", "libgtk-3-0"]`. |
| `recommends` | no | `[]` | Soft dependencies, installed by apt but not required. |
| `suggests` | no | `[]` | Packages that pair well with this one; never auto-installed. |
| `section` | no | — | The Debian category (this is Debian's "group"): `utils`, `net`, `www`, `devel`, `games`, `misc`, `graphics`, `sound`, `editors`, `mail`, `httpd`, … |
| `priority` | no | — | `required`, `important`, `standard`, `optional`, `extra`. Most apps use `optional`. |

```toml
[deb]
enabled = true
section = "net"
priority = "optional"
depends = ["libc6 (>= 2.31)"]
```

### `[rpm]` — RPM options

| Key | Required | Default | Description |
| --- | --- | --- | --- |
| `enabled` | no | `true` | Set `false` to skip `.rpm` output in a plain `build`. |
| `requires` | no | `[]` | RPM dependency strings, e.g. `["glibc >= 2.31"]`. |
| `group` | no | — | The RPM category: `Applications/System`, `Development/Tools`, `Games/Entertainment`, `System Environment/Libraries`, … |

```toml
[rpm]
enabled = false
group = "Applications/System"
requires = ["glibc >= 2.31"]
```

### Enabling / disabling formats

`enabled` decides what `linux-packager build` produces when you don't pass
`--format`. An explicit `--format deb` / `--format rpm` overrides it.

```toml
[deb]
enabled = true    # produce .deb

[rpm]
enabled = false   # skip .rpm
```

### Field mapping

Where each manifest value ends up in the built packages:

| Manifest | `.deb` control | `.rpm` spec |
| --- | --- | --- |
| `package.name` | `Package` | `Name` |
| `package.version` | `Version` | `Version` |
| `package.description` (1st line) | `Description` | `Summary` |
| `package.description` (rest) | `Description` (continuation) | `%description` |
| `package.license` | — | `License` |
| `package.maintainer` | `Maintainer` | — |
| `package.homepage` | `Homepage` | `URL` |
| `package.release` | — | `Release` |
| `package.arch` | `Architecture` | `BuildArch` |
| `binary` / `[[files]]` | `data.tar` entries | `%files` entries |
| `deb.depends` | `Depends` | — |
| `deb.section` | `Section` | — |
| `deb.priority` | `Priority` | — |
| `rpm.requires` | — | `Requires` |
| `rpm.group` | — | `Group` |

### Path rules

- Relative paths (`binary.path`, `files[].source`) resolve against the
  manifest's directory. Override the base with `--root`.
- `files[].dest` must be absolute (`/etc/...`), not `./etc/...`.
- Package **name** collisions matter at install time: if a file (e.g.
  `/usr/bin/ghostab`) is already owned by another installed package, `dpkg -i`
  refuses to overwrite it. Use a distinct `install_dir`, or match the existing
  package's `name` + a higher `version` to upgrade cleanly.

## Building RPMs

`.rpm` output requires `rpmbuild` on `PATH` (install `rpm` / `rpm-build`).
Building `.deb` has no external dependencies.

## Development

```sh
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```
