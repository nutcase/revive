use super::super::*;

impl Cpu {
    #[inline]
    pub(in crate::cpu) fn clc(&mut self) -> u8 {
        self.status.remove(StatusFlags::CARRY);
        2
    }

    #[inline]
    pub(in crate::cpu) fn sec(&mut self) -> u8 {
        self.status.insert(StatusFlags::CARRY);
        2
    }

    #[inline]
    pub(in crate::cpu) fn cli(&mut self) -> u8 {
        self.status.remove(StatusFlags::INTERRUPT_DISABLE);
        2
    }

    #[inline]
    pub(in crate::cpu) fn sei(&mut self) -> u8 {
        self.status.insert(StatusFlags::INTERRUPT_DISABLE);
        2
    }

    #[inline]
    pub(in crate::cpu) fn clv(&mut self) -> u8 {
        self.status.remove(StatusFlags::OVERFLOW);
        2
    }

    #[inline]
    pub(in crate::cpu) fn cld(&mut self) -> u8 {
        self.status.remove(StatusFlags::DECIMAL);
        2
    }

    #[inline]
    pub(in crate::cpu) fn sed(&mut self) -> u8 {
        self.status.insert(StatusFlags::DECIMAL);
        2
    }
}
