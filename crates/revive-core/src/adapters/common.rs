use std::path::Path;

use crate::paths::{readable_state_path, state_path};
use crate::system::{Result, SystemKind};

pub(crate) fn write_byte(memory: &mut [u8], offset: usize, value: u8) -> bool {
    if let Some(slot) = memory.get_mut(offset) {
        *slot = value;
        true
    } else {
        false
    }
}

pub(crate) fn argb8888_u32_frame_as_bgra8888_bytes(frame: &[u32]) -> &[u8] {
    debug_assert!(cfg!(target_endian = "little"));
    unsafe { std::slice::from_raw_parts(frame.as_ptr() as *const u8, std::mem::size_of_val(frame)) }
}

pub(crate) fn save_state_slot(
    system: SystemKind,
    rom_path: &Path,
    slot: u8,
    ext: &str,
    save: impl FnOnce(&Path) -> Result<()>,
) -> Result<()> {
    let path = state_path(system, rom_path, slot, ext);
    save(&path)
}

pub(crate) fn load_state_slot(
    system: SystemKind,
    rom_path: &Path,
    slot: u8,
    ext: &str,
    load: impl FnOnce(&Path) -> Result<()>,
) -> Result<()> {
    let path = readable_state_path(system, rom_path, slot, ext)?;
    load(&path)
}

pub(crate) fn write_file(path: &Path, data: &[u8]) -> Result<()> {
    std::fs::write(path, data).map_err(|err| err.to_string())
}

pub(crate) fn write_optional_file(path: &Path, data: Option<&[u8]>) -> Result<()> {
    if let Some(data) = data {
        write_file(path, data)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argb_pixels_are_exposed_as_bgra_bytes() {
        let pixels = [0xFF11_2233u32, 0x8044_5566u32];
        let bytes = argb8888_u32_frame_as_bgra8888_bytes(&pixels);

        assert_eq!(bytes, &[0x33, 0x22, 0x11, 0xFF, 0x66, 0x55, 0x44, 0x80]);
    }
}
