use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::Path,
    time::UNIX_EPOCH,
};

use crate::{Error, Result};

const MACH_HEADER_64_SIZE: usize = 32;
const MH_MAGIC_64: u32 = 0xfeed_facf;
const CPU_TYPE_X86_64: u32 = 0x0100_0007;
const LC_SYMTAB: u32 = 0x2;
const LC_DYSYMTAB: u32 = 0xb;
const LC_UUID: u32 = 0x1b;
const LC_SEGMENT_64: u32 = 0x19;
const LC_DYLD_INFO: u32 = 0x22;
const LC_DYLD_INFO_ONLY: u32 = 0x8000_0022;
const LINKEDIT_DATA_COMMANDS: &[u32] =
    &[0x1d, 0x1e, 0x26, 0x29, 0x2b, 0x2e, 0x8000_0033, 0x8000_0034];

#[derive(Clone, Copy)]
struct LoadCommand {
    kind: u32,
    offset: usize,
    size: usize,
}

pub(super) fn prepare_analysis_image(source: &Path, destination: &Path) -> Result<()> {
    let source = source.canonicalize().map_err(|error| {
        platform_error(format!(
            "could not resolve the running SC2 executable {}: {error}",
            source.display()
        ))
    })?;
    let source_metadata = source.metadata()?;
    let modified = source_metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let fingerprint = format!(
        "{}\n{}\n{}\n",
        source.display(),
        source_metadata.len(),
        modified
    );
    let fingerprint_path = destination.with_extension("source");
    if destination.is_file()
        && fs::read_to_string(&fingerprint_path).is_ok_and(|saved| saved == fingerprint)
    {
        return Ok(());
    }

    let mut image = fs::read(&source)?;
    sanitize(&mut image)?;

    let parent = destination.parent().ok_or_else(|| {
        platform_error(format!(
            "analysis image {} has no parent directory",
            destination.display()
        ))
    })?;
    fs::create_dir_all(parent)?;
    fs::set_permissions(parent, std::os::unix::fs::PermissionsExt::from_mode(0o700))?;

    let temporary = destination.with_extension("tmp");
    remove_if_present(&temporary)?;
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o700)
        .open(&temporary)?;
    output.write_all(&image)?;
    output.sync_all()?;
    drop(output);
    fs::rename(&temporary, destination)?;

    let fingerprint_temporary = fingerprint_path.with_extension("source.tmp");
    remove_if_present(&fingerprint_temporary)?;
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&fingerprint_temporary)?;
    output.write_all(fingerprint.as_bytes())?;
    output.sync_all()?;
    drop(output);
    fs::rename(fingerprint_temporary, fingerprint_path)?;
    Ok(())
}

fn sanitize(image: &mut Vec<u8>) -> Result<()> {
    if read_u32(image, 0)? != MH_MAGIC_64 {
        return Err(platform_error("SC2 is not a little-endian 64-bit Mach-O"));
    }
    if read_u32(image, 4)? != CPU_TYPE_X86_64 {
        return Err(platform_error("SC2 is not an x86_64 Mach-O"));
    }

    let command_count = usize::try_from(read_u32(image, 16)?).expect("u32 fits in usize");
    let commands_size = usize::try_from(read_u32(image, 20)?).expect("u32 fits in usize");
    let commands_end = MACH_HEADER_64_SIZE
        .checked_add(commands_size)
        .ok_or_else(|| platform_error("SC2 load-command table overflows the image"))?;
    if commands_end > image.len() {
        return Err(platform_error(
            "SC2 load-command table extends beyond the image",
        ));
    }

    let mut commands = Vec::with_capacity(command_count);
    let mut offset = MACH_HEADER_64_SIZE;
    for _ in 0..command_count {
        let kind = read_u32(image, offset)?;
        let size = usize::try_from(read_u32(image, offset + 4)?).expect("u32 fits in usize");
        if size < 8
            || offset
                .checked_add(size)
                .is_none_or(|end| end > commands_end)
        {
            return Err(platform_error(
                "SC2 contains an invalid Mach-O load command",
            ));
        }
        commands.push(LoadCommand { kind, offset, size });
        offset += size;
    }
    if offset != commands_end {
        return Err(platform_error(
            "SC2 load commands do not consume the declared table size",
        ));
    }

    relocate_linkedit(image, &commands)?;

    let mut uuid_count = 0;
    let mut symbol_table_count = 0;
    let mut dynamic_symbol_table_count = 0;
    for command in commands {
        match command.kind {
            LC_UUID => {
                if command.size != 24 {
                    return Err(platform_error("SC2 has an invalid LC_UUID command"));
                }
                uuid_count += 1;
            }
            LC_SYMTAB => {
                if command.size != 24 {
                    return Err(platform_error("SC2 has an invalid LC_SYMTAB command"));
                }
                image[command.offset + 8..command.offset + 24].fill(0);
                symbol_table_count += 1;
            }
            LC_DYSYMTAB => {
                if command.size != 80 {
                    return Err(platform_error("SC2 has an invalid LC_DYSYMTAB command"));
                }
                image[command.offset + 8..command.offset + 80].fill(0);
                dynamic_symbol_table_count += 1;
            }
            _ => {}
        }
    }
    if uuid_count != 1 || symbol_table_count != 1 || dynamic_symbol_table_count != 1 {
        return Err(platform_error(format!(
            "SC2 has unexpected Mach-O metadata counts: UUID={uuid_count}, SYMTAB={symbol_table_count}, DYSYMTAB={dynamic_symbol_table_count}"
        )));
    }
    Ok(())
}

fn relocate_linkedit(image: &mut Vec<u8>, commands: &[LoadCommand]) -> Result<()> {
    let mut image_base = None;
    let mut linkedit = None;
    for command in commands
        .iter()
        .filter(|command| command.kind == LC_SEGMENT_64 && command.size >= 72)
    {
        let file_offset = read_u64(image, command.offset + 40)?;
        let file_size = read_u64(image, command.offset + 48)?;
        let name = &image[command.offset + 8..command.offset + 24];
        if file_offset == 0 && file_size != 0 && image_base.is_none() {
            image_base = Some(read_u64(image, command.offset + 24)?);
        }
        if name.starts_with(b"__LINKEDIT\0") {
            linkedit = Some((*command, file_offset, file_size));
        }
    }
    let image_base = image_base.ok_or_else(|| platform_error("SC2 image base is missing"))?;
    let (command, old_offset, file_size) =
        linkedit.ok_or_else(|| platform_error("SC2 __LINKEDIT segment is missing"))?;
    let virtual_address = read_u64(image, command.offset + 24)?;
    let expected_offset = virtual_address
        .checked_sub(image_base)
        .ok_or_else(|| platform_error("SC2 __LINKEDIT address precedes its image base"))?;
    if expected_offset == old_offset {
        return Ok(());
    }
    if expected_offset < old_offset {
        return Err(platform_error("refusing to move SC2 __LINKEDIT backwards"));
    }

    let old_offset = usize::try_from(old_offset)
        .map_err(|_| platform_error("SC2 __LINKEDIT offset is too large"))?;
    let expected_offset = usize::try_from(expected_offset)
        .map_err(|_| platform_error("SC2 relocated __LINKEDIT offset is too large"))?;
    let file_size = usize::try_from(file_size)
        .map_err(|_| platform_error("SC2 __LINKEDIT size is too large"))?;
    let old_end = old_offset
        .checked_add(file_size)
        .ok_or_else(|| platform_error("SC2 __LINKEDIT range overflows"))?;
    if old_end > image.len() {
        return Err(platform_error("SC2 __LINKEDIT extends beyond the image"));
    }
    let new_end = expected_offset
        .checked_add(file_size)
        .ok_or_else(|| platform_error("relocated SC2 __LINKEDIT range overflows"))?;
    let payload = image[old_offset..old_end].to_vec();
    image.resize(image.len().max(new_end), 0);
    image[expected_offset..new_end].copy_from_slice(&payload);
    write_u64(
        image,
        command.offset + 40,
        u64::try_from(expected_offset).expect("usize fits in u64"),
    )?;

    let delta = expected_offset - old_offset;
    for command in commands {
        if matches!(command.kind, LC_DYLD_INFO | LC_DYLD_INFO_ONLY) {
            if command.size != 48 {
                return Err(platform_error("SC2 has an invalid LC_DYLD_INFO command"));
            }
            for field in (0..10).step_by(2) {
                relocate_u32_offset(
                    image,
                    command.offset + 8 + field * 4,
                    old_offset,
                    old_end,
                    delta,
                )?;
            }
        } else if LINKEDIT_DATA_COMMANDS.contains(&command.kind) {
            if command.size != 16 {
                return Err(platform_error(format!(
                    "SC2 has an invalid linkedit-data command {:#x}",
                    command.kind
                )));
            }
            relocate_u32_offset(image, command.offset + 8, old_offset, old_end, delta)?;
        }
    }
    Ok(())
}

fn relocate_u32_offset(
    image: &mut [u8],
    offset: usize,
    old_start: usize,
    old_end: usize,
    delta: usize,
) -> Result<()> {
    let value = usize::try_from(read_u32(image, offset)?).expect("u32 fits in usize");
    if (old_start..old_end).contains(&value) {
        let relocated = value
            .checked_add(delta)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| platform_error("relocated SC2 linkedit offset exceeds u32"))?;
        write_u32(image, offset, relocated)?;
    }
    Ok(())
}

fn read_u32(image: &[u8], offset: usize) -> Result<u32> {
    let bytes = image
        .get(offset..offset + 4)
        .ok_or_else(|| platform_error("truncated SC2 Mach-O data"))?;
    Ok(u32::from_le_bytes(
        bytes.try_into().expect("slice is four bytes"),
    ))
}

fn read_u64(image: &[u8], offset: usize) -> Result<u64> {
    let bytes = image
        .get(offset..offset + 8)
        .ok_or_else(|| platform_error("truncated SC2 Mach-O data"))?;
    Ok(u64::from_le_bytes(
        bytes.try_into().expect("slice is eight bytes"),
    ))
}

fn write_u32(image: &mut [u8], offset: usize, value: u32) -> Result<()> {
    let bytes = image
        .get_mut(offset..offset + 4)
        .ok_or_else(|| platform_error("truncated SC2 Mach-O data"))?;
    bytes.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn write_u64(image: &mut [u8], offset: usize, value: u64) -> Result<()> {
    let bytes = image
        .get_mut(offset..offset + 8)
        .ok_or_else(|| platform_error("truncated SC2 Mach-O data"))?;
    bytes.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn remove_if_present(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn platform_error(message: impl Into<String>) -> Error {
    Error::Platform(message.into())
}

#[cfg(test)]
mod tests {
    use super::prepare_analysis_image;

    #[test]
    #[ignore = "requires an installed retail SC2 client"]
    fn prepares_the_installed_sc2_image_for_lldb() {
        let source = crate::platform::installed_sc2_executable().unwrap();
        let directory =
            std::env::temp_dir().join(format!("scanner-sweep-macho-test-{}", std::process::id()));
        let destination = directory.join("SC2");
        prepare_analysis_image(&source, &destination).unwrap();
        assert!(destination.is_file());
        assert!(destination.with_extension("source").is_file());
        assert!(destination.metadata().unwrap().len() >= source.metadata().unwrap().len());
        std::fs::remove_dir_all(directory).unwrap();
    }
}
