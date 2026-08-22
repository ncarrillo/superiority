use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use superiority_core::metadata::find_metadata_blobs;

fn usage() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "usage: cargo run -p extract-bsn-metadata -- <SC2-executable> <output.bin>",
    )
}

fn next_path(args: &mut impl Iterator<Item = OsString>) -> Result<PathBuf, io::Error> {
    args.next().map(PathBuf::from).ok_or_else(usage)
}

fn create_parent(path: &Path) -> io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os();
    let _program = args.next();
    let executable = next_path(&mut args)?;
    let output = next_path(&mut args)?;
    if args.next().is_some() {
        return Err(usage().into());
    }

    let executable_data = fs::read(&executable)?;
    let candidates = find_metadata_blobs(&executable_data)?;
    let candidate_count = candidates.len();
    let (offset, metadata) = candidates
        .into_iter()
        .max_by_key(|(_, metadata)| metadata.blob().len())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "the executable contains no valid native BSN metadata",
            )
        })?;

    create_parent(&output)?;
    fs::write(&output, metadata.blob())?;
    println!(
        "Extracted {} bytes and {} types from candidate {offset:#x} ({candidate_count} found) to {}",
        metadata.blob().len(),
        metadata.header.type_count,
        output.display()
    );
    Ok(())
}
