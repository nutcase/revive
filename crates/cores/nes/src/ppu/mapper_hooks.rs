use crate::cartridge::Cartridge;

pub(super) fn uses_latch_fetches(cartridge: Option<&Cartridge>) -> bool {
    cartridge.map(Cartridge::uses_mmc2_latches).unwrap_or(false)
}

pub(super) fn tick_mmc5_scanline(cartridge: Option<&Cartridge>) {
    if let Some(cart) = cartridge {
        if cart.uses_mmc5() {
            cart.mmc5_scanline_tick();
        }
    }
}

pub(super) fn end_mmc5_frame(cartridge: Option<&Cartridge>) {
    if let Some(cart) = cartridge {
        if cart.uses_mmc5() {
            cart.mmc5_end_frame();
        }
    }
}

pub(super) fn mmc5_split_bg_fetch(
    cartridge: Option<&Cartridge>,
    x: u8,
    y: u8,
    fine_x: u8,
) -> Option<(u8, u8, u8)> {
    cartridge.and_then(|cart| cart.mmc5_split_bg_fetch(x, y, fine_x))
}
