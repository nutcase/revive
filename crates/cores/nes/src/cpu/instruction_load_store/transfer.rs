use super::super::*;

impl Cpu {
    #[inline]
    pub(in crate::cpu) fn txa(&mut self) -> u8 {
        self.a = self.x;
        self.set_zero_negative_flags(self.a);
        2
    }

    #[inline]
    pub(in crate::cpu) fn tya(&mut self) -> u8 {
        self.a = self.y;
        self.set_zero_negative_flags(self.a);
        2
    }

    #[inline]
    pub(in crate::cpu) fn txs(&mut self) -> u8 {
        self.sp = self.x;
        2
    }

    #[inline]
    pub(in crate::cpu) fn tay(&mut self) -> u8 {
        self.y = self.a;
        self.set_zero_negative_flags(self.y);
        2
    }

    #[inline]
    pub(in crate::cpu) fn tax(&mut self) -> u8 {
        self.x = self.a;
        self.set_zero_negative_flags(self.x);
        2
    }

    #[inline]
    pub(in crate::cpu) fn tsx(&mut self) -> u8 {
        self.x = self.sp;
        self.set_zero_negative_flags(self.x);
        2
    }
}
