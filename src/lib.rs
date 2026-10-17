//! Linux Packager: turn a Linux executable (and extra files) into installable
//! `.deb` and `.rpm` packages from a single declarative manifest.

pub mod arch;
pub mod cli;
pub mod config;
pub mod deb;
pub mod files;
pub mod rpm;
pub mod tar_util;
