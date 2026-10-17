use anyhow::{bail, Context, Result};
use clap::Parser;
use linux_packager::arch::Arch;
use linux_packager::cli::{BuildArgs, Cli, Command, Format, InitArgs};
use linux_packager::config::{self, Manifest};
use linux_packager::{deb, files, rpm};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Init(args) => init(args),
        Command::Build(args) => build(args),
    }
}

fn init(args: InitArgs) -> Result<()> {
    if args.path.exists() && !args.force {
        bail!(
            "`{}` already exists (use --force to overwrite)",
            args.path.display()
        );
    }
    std::fs::write(&args.path, config::template())
        .with_context(|| format!("failed to write `{}`", args.path.display()))?;
    println!("wrote {}", args.path.display());
    Ok(())
}

fn build(args: BuildArgs) -> Result<()> {
    let manifest = Manifest::load(&args.manifest)?;

    let base = match &args.root {
        Some(root) => root.clone(),
        None => args
            .manifest
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(".")),
    };

    let arch = match &manifest.package.arch {
        Some(value) => Arch::parse(value)?,
        None => Arch::detect(),
    };

    let payload = files::collect(&manifest, &base)?;
    if payload.is_empty() {
        bail!("manifest resolved to an empty payload");
    }

    // An explicit --format wins; otherwise build every format marked enabled.
    let formats = if args.format.is_empty() {
        let mut formats = Vec::new();
        if manifest.deb.enabled {
            formats.push(Format::Deb);
        }
        if manifest.rpm.enabled {
            formats.push(Format::Rpm);
        }
        if formats.is_empty() {
            bail!("all formats are disabled — nothing to build (check `enabled` in [deb]/[rpm])");
        }
        formats
    } else {
        args.format.clone()
    };

    for format in formats {
        let produced = match format {
            Format::Deb => deb::build(&manifest, &payload, &args.output, arch)?,
            Format::Rpm => rpm::build(&manifest, &payload, &args.output, arch)?,
        };
        println!("built {}", produced.display());
    }
    Ok(())
}
