use super::super::*;

impl Cpu {
    #[inline]
    pub(in crate::cpu) fn nop(&mut self) -> u8 {
        2
    }

    #[inline]
    pub(in crate::cpu) fn jam(&mut self) -> u8 {
        self.halted = true;
        2
    }
}
