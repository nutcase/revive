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
