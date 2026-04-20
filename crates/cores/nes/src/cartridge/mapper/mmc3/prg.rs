#[derive(Clone, Copy)]
pub(super) struct PrgSlot {
    pub(super) bank: usize,
    pub(super) offset: usize,
}

pub(super) fn resolve_prg_slot(
    addr: u16,
    prg_mode: bool,
    bank_6: usize,
    bank_7: usize,
    second_last: usize,
    last: usize,
) -> Option<PrgSlot> {
    match addr {
        0x8000..=0x9FFF => Some(PrgSlot {
            bank: if prg_mode { second_last } else { bank_6 },
            offset: (addr - 0x8000) as usize,
        }),
        0xA000..=0xBFFF => Some(PrgSlot {
            bank: bank_7,
            offset: (addr - 0xA000) as usize,
        }),
        0xC000..=0xDFFF => Some(PrgSlot {
            bank: if prg_mode { bank_6 } else { second_last },
            offset: (addr - 0xC000) as usize,
        }),
        0xE000..=0xFFFF => Some(PrgSlot {
            bank: last,
            offset: (addr - 0xE000) as usize,
        }),
        _ => None,
    }
}
