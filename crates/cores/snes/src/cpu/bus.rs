//! Trait representing the minimal bus interface required by the 65C816 core.

pub trait CpuBus {
    fn read_u8(&mut self, addr: u32) -> u8;
    fn write_u8(&mut self, addr: u32, value: u8);

    /// CPU命令の開始/終了フック（任意）。
    ///
    /// バス側で「命令内サイクル」相当の周辺機器（例: $4202-$4206 の乗除算）を
    /// より正確に進めたい場合に使う。既定では何もしない。
    fn begin_cpu_instruction(&mut self) {}
    fn end_cpu_instruction(&mut self, _cycles: u8) {}

    fn opcode_memory_penalty(&mut self, _addr: u32) -> u8 {
        0
    }

    /// Returns true once if a general DMA (MDMAEN) started immediately after the last opcode fetch.
    ///
    /// This is used to model the SNES timing note that MDMA begins after the *next opcode fetch*.
    /// When true, the CPU core can defer executing the fetched instruction until after the DMA
    /// stall time has elapsed (matching real hardware behavior more closely).
    fn take_dma_start_event(&mut self) -> bool {
        false
    }
    fn poll_nmi(&mut self) -> bool {
        false
    }
    fn read_u16(&mut self, addr: u32) -> u16 {
        let lo = self.read_u8(addr) as u16;
        let hi = self.read_u8(addr.wrapping_add(1)) as u16;
        (hi << 8) | lo
    }
    fn write_u16(&mut self, addr: u32, value: u16) {
        self.write_u8(addr, (value & 0xFF) as u8);
        self.write_u8(addr.wrapping_add(1), (value >> 8) as u8);
    }
    fn acknowledge_nmi(&mut self) {}
    fn poll_irq(&mut self) -> bool;
    fn is_superfx_irq_asserted(&self) -> bool {
        false
    }
    fn is_timer_irq_pending(&self) -> bool {
        false
    }

    /// 任意: CPUが直近で実行中のPCをバス側へ通知するためのフック。
    /// 既定では何もしない。
    fn set_last_cpu_pc(&mut self, _pc24: u32) {}
    fn set_last_cpu_exec_pc(&mut self, _pc24: u32) {}
    fn set_last_cpu_state(&mut self, _a: u16, _x: u16, _y: u16, _db: u8, _pb: u8, _p: u8) {}
}
