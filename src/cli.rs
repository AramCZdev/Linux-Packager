//! Command-line interface definition.

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use crate::config::DEFAULT_MANIFEST;

#[derive(Debug, Parser)]
#[command(
    name = "linux-packager",
    version,
    about = "Package Linux executables into .deb and .rpm formats",
    propagate_version = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Build packages from a manifest file.
    Build(BuildArgs),
    /// Write a starter manifest to begin from.
    Init(InitArgs),
}

#[derive(Debug, Args)]
pub struct BuildArgs {
    /// Path to the manifest file.
    #[arg(short, long, default_value = DEFAULT_MANIFEST)]
    pub manifest: PathBuf,

    /// Package format(s) to produce. Defaults to all supported formats.
    #[arg(short, long, value_enum)]
    pub format: Vec<Format>,

    /// Directory the finished packages are written to.
    #[arg(short, long, default_value = "dist")]
    pub output: PathBuf,

    /// Base directory for relative paths in the manifest.
    /// Defaults to the directory containing the manifest.
    #[arg(long)]
    pub root: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct InitArgs {
    /// Where to write the manifest.
    #[arg(default_value = DEFAULT_MANIFEST)]
    pub path: PathBuf,

    /// Overwrite the file if it already exists.
    #[arg(short, long)]
    pub force: bool,
}

/// Supported output package formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    /// Debian package.
    Deb,
    /// RPM package.
    Rpm,
}
