use super::{BRAM_FORMAT_HEADER, BRAM_SIZE, TIMER_CONTROL_START};

/// A `bool` wrapper that is invisible to bincode serialization.
/// Encodes as zero bytes; decodes as `false`.  Used for transient render
/// state that must survive struct derivation but not save-state files.
#[derive(Clone, Copy, Default)]
pub(super) struct TransientBool(pub(super) bool);

impl bincode::Encode for TransientBool {
    fn encode<E: bincode::enc::Encoder>(
        &self,
        _encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        Ok(()) // write nothing
    }
}

impl<Context> bincode::Decode<Context> for TransientBool {
    fn decode<D: bincode::de::Decoder>(
        _decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        Ok(Self(false))
    }
}

impl<'de, Context> bincode::BorrowDecode<'de, Context> for TransientBool {
    fn borrow_decode<D: bincode::de::BorrowDecoder<'de>>(
        _decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        Ok(Self(false))
    }
}

impl core::ops::Deref for TransientBool {
    type Target = bool;
    fn deref(&self) -> &bool {
        &self.0
    }
}

impl core::ops::DerefMut for TransientBool {
    fn deref_mut(&mut self) -> &mut bool {
        &mut self.0
    }
}

/// A `u64` wrapper excluded from save-state serialization.
#[derive(Clone, Copy, Default)]
pub(super) struct TransientU64(pub(super) u64);

impl bincode::Encode for TransientU64 {
    fn encode<E: bincode::enc::Encoder>(
        &self,
        _encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        Ok(())
    }
}

impl<Context> bincode::Decode<Context> for TransientU64 {
    fn decode<D: bincode::de::Decoder>(
        _decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        Ok(Self(0))
    }
}

impl<'de, Context> bincode::BorrowDecode<'de, Context> for TransientU64 {
    fn borrow_decode<D: bincode::de::BorrowDecoder<'de>>(
        _decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        Ok(Self(0))
    }
}

impl core::ops::Deref for TransientU64 {
    type Target = u64;
    fn deref(&self) -> &u64 {
        &self.0
    }
}

impl core::ops::DerefMut for TransientU64 {
    fn deref_mut(&mut self) -> &mut u64 {
        &mut self.0
    }
}

/// A `usize` wrapper excluded from save-state serialization.
#[derive(Clone, Copy, Default)]
pub(super) struct TransientUsize(pub(super) usize);

impl bincode::Encode for TransientUsize {
    fn encode<E: bincode::enc::Encoder>(
        &self,
        _encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        Ok(())
    }
}

impl<Context> bincode::Decode<Context> for TransientUsize {
    fn decode<D: bincode::de::Decoder>(
        _decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        Ok(Self(0))
    }
}

impl<'de, Context> bincode::BorrowDecode<'de, Context> for TransientUsize {
    fn borrow_decode<D: bincode::de::BorrowDecoder<'de>>(
        _decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        Ok(Self(0))
    }
}

impl core::ops::Deref for TransientUsize {
    type Target = usize;
    fn deref(&self) -> &usize {
        &self.0
    }
}

impl core::ops::DerefMut for TransientUsize {
    fn deref_mut(&mut self) -> &mut usize {
        &mut self.0
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct PaletteFlickerEvent {
    pub(super) row: usize,
    pub(super) x: usize,
    pub(super) len: usize,
}

#[derive(Clone, Default)]
pub(super) struct TransientPaletteFlicker(pub(super) Vec<PaletteFlickerEvent>);

impl bincode::Encode for TransientPaletteFlicker {
    fn encode<E: bincode::enc::Encoder>(
        &self,
        _encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        Ok(())
    }
}

impl<Context> bincode::Decode<Context> for TransientPaletteFlicker {
    fn decode<D: bincode::de::Decoder>(
        _decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        Ok(Self::default())
    }
}

impl<'de, Context> bincode::BorrowDecode<'de, Context> for TransientPaletteFlicker {
    fn borrow_decode<D: bincode::de::BorrowDecoder<'de>>(
        _decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        Ok(Self::default())
    }
}

impl core::ops::Deref for TransientPaletteFlicker {
    type Target = [PaletteFlickerEvent];
    fn deref(&self) -> &[PaletteFlickerEvent] {
        &self.0
    }
}

impl core::ops::DerefMut for TransientPaletteFlicker {
    fn deref_mut(&mut self) -> &mut [PaletteFlickerEvent] {
        &mut self.0
    }
}

/// A BRAM wrapper that is intentionally excluded from save-state encoding.
/// Old save states (before BRAM support) remain decodable because this field
/// consumes zero bytes on decode.
#[derive(Clone)]
pub(super) struct TransientBram(pub(super) Vec<u8>);

impl Default for TransientBram {
    fn default() -> Self {
        let mut bram = vec![0; BRAM_SIZE];
        bram[..BRAM_FORMAT_HEADER.len()].copy_from_slice(&BRAM_FORMAT_HEADER);
        Self(bram)
    }
}

impl bincode::Encode for TransientBram {
    fn encode<E: bincode::enc::Encoder>(
        &self,
        _encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        Ok(())
    }
}

impl<Context> bincode::Decode<Context> for TransientBram {
    fn decode<D: bincode::de::Decoder>(
        _decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        Ok(Self::default())
    }
}

impl<'de, Context> bincode::BorrowDecode<'de, Context> for TransientBram {
    fn borrow_decode<D: bincode::de::BorrowDecoder<'de>>(
        _decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        Ok(Self::default())
    }
}

impl core::ops::Deref for TransientBram {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.0
    }
}

impl core::ops::DerefMut for TransientBram {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

#[derive(Clone, Copy, bincode::Encode, bincode::Decode)]
pub(super) enum VdcPort {
    Control,
    Data,
}

#[derive(Clone, Copy, Debug, bincode::Encode, bincode::Decode)]
pub(super) enum BankMapping {
    Ram { base: usize },
    Rom { base: usize },
    CartRam { base: usize },
    Hardware,
    Bram,
}

#[derive(Clone, Copy, bincode::Encode, bincode::Decode)]
pub(super) enum ControlRegister {
    TimerCounter,
    TimerControl,
    IrqMask,
    IrqStatus,
}

#[derive(Clone, Copy, bincode::Encode, bincode::Decode)]
pub(super) struct IoPort {
    pub(super) output: u8,
    pub(super) direction: u8,
    pub(super) enable: u8,
    pub(super) select: u8,
    pub(super) input: u8,
}

#[derive(Clone, Copy, bincode::Encode, bincode::Decode)]
pub(super) struct Timer {
    pub(super) reload: u8,
    pub(super) counter: u8,
    pub(super) prescaler: u32,
    pub(super) enabled: bool,
}

impl Timer {
    pub(super) fn new() -> Self {
        Self {
            reload: 0,
            counter: 0,
            prescaler: 0,
            enabled: false,
        }
    }

    pub(super) fn reset(&mut self) {
        *self = Self::new();
    }

    pub(super) fn write_reload(&mut self, value: u8) {
        self.reload = value & 0x7F;
    }

    pub(super) fn read_counter(&self) -> u8 {
        self.counter & 0x7F
    }

    pub(super) fn write_control(&mut self, value: u8) {
        let start = value & TIMER_CONTROL_START != 0;
        if start && !self.enabled {
            self.enabled = true;
            self.counter = self.reload;
            self.prescaler = 0;
        } else if !start {
            self.enabled = false;
        }
    }

    pub(super) fn control(&self) -> u8 {
        if self.enabled { TIMER_CONTROL_START } else { 0 }
    }

    pub(super) fn tick(&mut self, cycles: u32, high_speed: bool) -> bool {
        if !self.enabled {
            return false;
        }

        let divider = if high_speed { 1024 } else { 256 };
        self.prescaler += cycles;
        let mut fired = false;

        while self.prescaler >= divider as u32 {
            self.prescaler -= divider as u32;
            if self.counter == 0 {
                self.counter = self.reload;
                fired = true;
            } else {
                self.counter = self.counter.wrapping_sub(1) & 0x7F;
            }
        }

        fired
    }
}

impl IoPort {
    pub(super) fn new() -> Self {
        Self {
            output: 0,
            direction: 0,
            enable: 0,
            select: 0,
            input: 0xFF,
        }
    }

    pub(super) fn reset(&mut self) {
        *self = Self::new();
    }

    pub(super) fn read(&self, offset: usize) -> Option<u8> {
        match offset & 0x03FF {
            0x0000 => Some(self.read_joypad_data()),
            0x0002 => Some(self.direction),
            0x0004 => Some(self.input),
            0x0005 => Some(self.enable),
            0x0006 => Some(self.select),
            _ => None,
        }
    }

    pub(super) fn write(&mut self, offset: usize, value: u8) -> bool {
        match offset & 0x03FF {
            0x0000 => {
                self.output = value;
                // CLR low resets the 6-pad scan index on hardware.
                if value & 0x02 == 0 {
                    self.select = 0;
                }
                true
            }
            0x0002 => {
                self.direction = value;
                true
            }
            0x0004 => {
                self.input = value;
                true
            }
            0x0005 => {
                self.enable = value;
                true
            }
            0x0006 => {
                self.select = value;
                true
            }
            _ => false,
        }
    }

    pub(super) fn read_joypad_data(&self) -> u8 {
        // PC Engine joypad reads one nibble at a time.
        // SEL=1 -> d-pad nibble (lower 4 bits of input)
        // SEL=0 -> button nibble (upper 4 bits of input)
        let sel = (self.output & 0x01) != 0;
        let nibble = if sel {
            self.input & 0x0F // d-pad: Up(0) Right(1) Down(2) Left(3)
        } else {
            (self.input >> 4) & 0x0F // buttons: I(0) II(1) Sel(2) Run(3)
        };
        0xF0 | nibble
    }
}
