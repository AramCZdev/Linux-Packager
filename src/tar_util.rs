//! Small helpers for writing tar / tar.gz archives.

use anyhow::Result;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::io::Write;
use tar::{Builder, EntryType, GnuHeader, Header};

/// Serialize a tar archive into memory using the supplied closure.
pub fn tar_bytes<F>(fill: F) -> Result<Vec<u8>>
where
    F: FnOnce(&mut Builder<Vec<u8>>) -> Result<()>,
{
    let mut builder = Builder::new(Vec::new());
    fill(&mut builder)?;
    Ok(builder.into_inner()?)
}

/// Serialize a gzip-compressed tar archive into memory.
pub fn tar_gz_bytes<F>(fill: F) -> Result<Vec<u8>>
where
    F: FnOnce(&mut Builder<GzEncoder<Vec<u8>>>) -> Result<()>,
{
    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut builder = Builder::new(encoder);
    fill(&mut builder)?;
    let encoder = builder.into_inner()?;
    Ok(encoder.finish()?)
}

/// Append a regular file entry with an exact name (no path canonicalisation).
pub fn append_file<W: Write>(
    builder: &mut Builder<W>,
    name: &str,
    mode: u32,
    data: &[u8],
) -> Result<()> {
    let mut header = base_header(name, mode);
    header.set_entry_type(EntryType::Regular);
    header.set_size(data.len() as u64);
    header.set_cksum();
    builder.append(&header, data)?;
    Ok(())
}

/// Append a directory entry with an exact name.
pub fn append_dir<W: Write>(builder: &mut Builder<W>, name: &str, mode: u32) -> Result<()> {
    let mut header = base_header(name, mode);
    header.set_entry_type(EntryType::Directory);
    header.set_size(0);
    header.set_cksum();
    builder.append(&header, std::io::empty())?;
    Ok(())
}

/// Build a GNU header with the name written verbatim (preserving `./`).
fn base_header(name: &str, mode: u32) -> Header {
    let mut header = Header::new_gnu();
    header.set_mode(mode);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    write_name(&mut header, name);
    header
}

/// Write `name` directly into the GNU header name field so the archive keeps
/// the exact spelling (e.g. a leading `./`) rather than a normalised form.
fn write_name(header: &mut Header, name: &str) {
    let gnu: &mut GnuHeader = header.as_gnu_mut().expect("gnu header");
    let bytes = name.as_bytes();
    let field = &mut gnu.name;
    field.fill(0);
    let n = bytes.len().min(field.len());
    field[..n].copy_from_slice(&bytes[..n]);
}
