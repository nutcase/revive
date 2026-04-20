//! DSP-3 coprocessor HLE used by SD Gundam GX.
//!
//! DSP-3 uses the same NEC DSP I/O register shape as DSP-1, but its command set
//! is different.  Treating SD Gundam GX as DSP-1 corrupts the CPU-side protocol,
//! so this module keeps the DSP-3 transfer state and command handlers separate.

#[derive(Debug, Clone)]
pub struct Dsp3 {
    dr: u16,
    sr: u16,
    memory_index: usize,
    win_lo: i16,
    win_hi: i16,
    add_lo: i16,
    add_hi: i16,
    byte_phase_low: bool,
    handler: Handler,
    output_words: Vec<u16>,
    output_index: usize,
    codewords: u16,
    outwords: u16,
    symbol: u16,
    bit_count: u16,
    index: usize,
    codes: [u16; 512],
    bits_left: u16,
    req_bits: u16,
    req_data: u16,
    bit_command: u16,
    base_length: u8,
    base_codes: u16,
    base_code: u16,
    code_lengths: [u8; 8],
    code_offsets: [u16; 8],
    lz_code: u16,
    lz_length: u8,
    x: u16,
    y: u16,
    bitmap: [u8; 8],
    bitplane: [u8; 8],
    bm_index: usize,
    bp_index: usize,
    count: u16,
    op3e_x: i16,
    op3e_y: i16,
    op1e_terrain: [i16; 0x2000],
    op1e_cost: [i16; 0x2000],
    op1e_weight: [i16; 0x2000],
    op1e_cell: usize,
    op1e_turn: i16,
    op1e_x: i16,
    op1e_y: i16,
    op1e_min_radius: i16,
    op1e_max_radius: i16,
    op1e_max_search_radius: i16,
    op1e_max_path_radius: i16,
    op1e_lcv_radius: i16,
    op1e_lcv_steps: i16,
    op1e_lcv_turns: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Handler {
    Command,
    MemoryDump,
    Coordinate,
    Decode,
    DecodeA,
    DecodeSymbols,
    DecodeTree,
    DecodeData,
    Convert,
    ConvertA,
    Op03,
    Op06,
    Op07,
    Op07A,
    Op07B,
    Op10,
    AbsorbThenReset,
    Op1cA,
    Op1cB,
    Op1cC,
    Op1e,
    Op1eA1,
    Op1eA2,
    Op1eA3,
    Op1eB,
    Op1eC,
    Op1eC1,
    Op1eC2,
    Op3e,
}

impl Dsp3 {
    pub fn new() -> Self {
        let mut dsp = Self {
            dr: 0,
            sr: 0,
            memory_index: 0,
            win_lo: 0,
            win_hi: 0,
            add_lo: 0,
            add_hi: 0,
            byte_phase_low: true,
            handler: Handler::Command,
            output_words: Vec::new(),
            output_index: 0,
            codewords: 0,
            outwords: 0,
            symbol: 0,
            bit_count: 0,
            index: 0,
            codes: [0; 512],
            bits_left: 0,
            req_bits: 0,
            req_data: 0,
            bit_command: 0xffff,
            base_length: 0,
            base_codes: 0,
            base_code: 0xffff,
            code_lengths: [0; 8],
            code_offsets: [0; 8],
            lz_code: 0,
            lz_length: 0,
            x: 0,
            y: 0,
            bitmap: [0; 8],
            bitplane: [0; 8],
            bm_index: 0,
            bp_index: 0,
            count: 0,
            op3e_x: 0,
            op3e_y: 0,
            op1e_terrain: [0; 0x2000],
            op1e_cost: [0; 0x2000],
            op1e_weight: [0; 0x2000],
            op1e_cell: 0,
            op1e_turn: 0,
            op1e_x: 0,
            op1e_y: 0,
            op1e_min_radius: 0,
            op1e_max_radius: 0,
            op1e_max_search_radius: 0,
            op1e_max_path_radius: 0,
            op1e_lcv_radius: 0,
            op1e_lcv_steps: 0,
            op1e_lcv_turns: 0,
        };
        dsp.reset();
        dsp
    }

    pub fn reset(&mut self) {
        self.dr = 0x0080;
        self.sr = 0x0084;
        self.memory_index = 0;
        self.byte_phase_low = true;
        self.handler = Handler::Command;
        self.output_words.clear();
        self.output_index = 0;
    }

    pub fn read_dr(&mut self) -> u8 {
        if self.sr & 0x04 != 0 {
            let byte = self.dr as u8;
            self.step_handler();
            return byte;
        }

        self.sr ^= 0x10;
        self.byte_phase_low = self.sr & 0x10 == 0;
        if self.sr & 0x10 != 0 {
            self.dr as u8
        } else {
            let byte = (self.dr >> 8) as u8;
            self.step_handler();
            byte
        }
    }

    pub fn read_sr(&self) -> u8 {
        self.sr as u8
    }

    pub fn write_dr(&mut self, byte: u8) {
        if self.sr & 0x04 != 0 {
            self.dr = (self.dr & 0xff00) | byte as u16;
            self.step_handler();
            return;
        }

        self.sr ^= 0x10;
        self.byte_phase_low = self.sr & 0x10 == 0;
        if self.sr & 0x10 != 0 {
            self.dr = (self.dr & 0xff00) | byte as u16;
        } else {
            self.dr = (self.dr & 0x00ff) | ((byte as u16) << 8);
            self.step_handler();
        }
    }

    fn step_handler(&mut self) {
        match self.handler {
            Handler::Command => self.command(),
            Handler::MemoryDump => self.memory_dump_next(),
            Handler::Coordinate => self.coordinate(),
            Handler::Decode => self.decode(),
            Handler::DecodeA => self.decode_a(),
            Handler::DecodeSymbols => self.decode_symbols(),
            Handler::DecodeTree => self.decode_tree(),
            Handler::DecodeData => self.decode_data(),
            Handler::Convert => self.convert(),
            Handler::ConvertA => self.convert_a(),
            Handler::Op03 => self.op03(),
            Handler::Op06 => self.op06(),
            Handler::Op07 => self.op07(),
            Handler::Op07A => self.op07_a(),
            Handler::Op07B => self.op07_b(),
            Handler::Op10 => self.op10(),
            Handler::AbsorbThenReset => self.reset(),
            Handler::Op1cA => {
                self.handler = Handler::Op1cB;
            }
            Handler::Op1cB => {
                self.dr = 0;
                self.handler = Handler::Op1cC;
            }
            Handler::Op1cC => {
                self.dr = 0;
                self.reset();
            }
            Handler::Op1e => self.op1e(),
            Handler::Op1eA1 => self.op1e_a1(),
            Handler::Op1eA2 => self.op1e_a2(),
            Handler::Op1eA3 => self.op1e_a3(),
            Handler::Op1eB => self.op1e_b(),
            Handler::Op1eC => self.op1e_c(),
            Handler::Op1eC1 => self.op1e_c1(),
            Handler::Op1eC2 => self.op1e_c2(),
            Handler::Op3e => self.op3e(),
        }
    }

    fn command(&mut self) {
        if self.dr >= 0x40 {
            return;
        }

        match self.dr {
            0x02 => {
                self.handler = Handler::Coordinate;
                self.sr = 0x0080;
                self.index = 0;
            }
            0x03 => {
                self.handler = Handler::Op03;
                self.sr = 0x0080;
                self.index = 0;
            }
            0x06 => {
                self.handler = Handler::Op06;
                self.sr = 0x0080;
                self.index = 0;
            }
            0x07 => {
                self.handler = Handler::Op07;
            }
            0x0f => {
                // Test memory.
                self.dr = 0;
                self.reset_after_valid_command();
            }
            0x10 => {
                self.handler = Handler::Op10;
                self.sr = 0x0080;
                self.index = 0;
            }
            0x1f => {
                self.output_words = data_rom_words();
                self.output_index = 0;
                self.sr = 0x0080;
                self.handler = Handler::MemoryDump;
                self.memory_dump_next();
            }
            0x18 => {
                self.handler = Handler::Convert;
                self.sr = 0x0080;
                self.index = 0;
            }
            0x38 => {
                self.handler = Handler::Decode;
                self.sr = 0x0080;
                self.index = 0;
            }
            0x0c => {
                self.dr = 0;
                self.handler = Handler::AbsorbThenReset;
                self.sr = 0x0080;
            }
            0x1c => {
                self.handler = Handler::Op1cA;
                self.sr = 0x0080;
            }
            0x1e => {
                self.handler = Handler::Op1e;
                self.sr = 0x0080;
                self.index = 0;
            }
            0x3e => {
                self.handler = Handler::Op3e;
                self.sr = 0x0080;
                self.index = 0;
            }
            _ => {}
        }
    }

    fn reset_after_valid_command(&mut self) {
        self.dr = 0x0080;
        self.sr = 0x0084;
        self.handler = Handler::Command;
        self.byte_phase_low = true;
    }

    fn memory_dump_next(&mut self) {
        if self.output_index < self.output_words.len() {
            self.dr = self.output_words[self.output_index];
            self.output_index += 1;
        } else {
            self.reset();
        }
    }

    fn lo_byte_word(value: u16) -> i16 {
        (value as u8) as i16
    }

    fn hi_byte_word(value: u16) -> i16 {
        ((value >> 8) as u8) as i16
    }

    fn cell_from_xy(&mut self, x: i16, y: i16) -> usize {
        self.dr = (x as u8 as u16) | ((y as u8 as u16) << 8);
        self.op03_compute();
        (self.dr as usize) & 0x1fff
    }

    fn coordinate(&mut self) {
        self.index += 1;

        match self.index {
            3 => {
                if self.dr == 0xffff {
                    self.reset();
                }
            }
            4 => {
                self.x = self.dr;
            }
            5 => {
                self.y = self.dr;
                self.dr = 1;
            }
            6 => {
                self.dr = self.x;
            }
            7 => {
                self.dr = self.y;
                self.index = 0;
            }
            _ => {}
        }
    }

    fn op03_compute(&mut self) {
        let lo = Self::lo_byte_word(self.dr);
        let hi = Self::hi_byte_word(self.dr);
        let ofs = self
            .win_lo
            .wrapping_mul(hi)
            .wrapping_shl(1)
            .wrapping_add(lo.wrapping_shl(1));
        self.dr = (ofs >> 1) as u16;
    }

    fn op03(&mut self) {
        self.op03_compute();
        self.handler = Handler::AbsorbThenReset;
    }

    fn op06(&mut self) {
        self.win_lo = Self::lo_byte_word(self.dr);
        self.win_hi = Self::hi_byte_word(self.dr);
        self.reset();
    }

    fn load_direction_add(&mut self, direction: i16) {
        let data_ofs = ((((direction as i32) << 1) + 0x03b2) as usize) & 0x03ff;
        self.add_hi = DSP3_DATA_ROM[data_ofs] as i16;
        self.add_lo = DSP3_DATA_ROM[data_ofs + 1] as i16;
    }

    fn op07(&mut self) {
        self.load_direction_add(self.dr as i16);
        self.handler = Handler::Op07A;
        self.sr = 0x0080;
    }

    fn step_wrapped(&mut self, lo: i16, hi: i16) -> (i16, i16) {
        let mut hi = hi;
        if lo & 1 != 0 {
            hi = hi.wrapping_add(self.add_lo & 1);
        }

        let mut next_lo = self.add_lo.wrapping_add(lo);
        let mut next_hi = self.add_hi.wrapping_add(hi);

        if next_lo < 0 {
            next_lo = next_lo.wrapping_add(self.win_lo);
        } else if next_lo >= self.win_lo {
            next_lo = next_lo.wrapping_sub(self.win_lo);
        }

        if next_hi < 0 {
            next_hi = next_hi.wrapping_add(self.win_hi);
        } else if next_hi >= self.win_hi {
            next_hi = next_hi.wrapping_sub(self.win_hi);
        }

        self.add_lo = next_lo;
        self.add_hi = next_hi;
        (next_lo, next_hi)
    }

    fn op07_a(&mut self) {
        let lo = Self::lo_byte_word(self.dr);
        let hi = Self::hi_byte_word(self.dr);
        let (next_lo, next_hi) = self.step_wrapped(lo, hi);

        self.dr = (next_lo as u16) | ((next_hi as u16) << 8) | (((next_hi as u16) >> 8) & 0x00ff);
        self.handler = Handler::Op07B;
    }

    fn op07_b(&mut self) {
        let ofs = self
            .win_lo
            .wrapping_mul(self.add_hi)
            .wrapping_shl(1)
            .wrapping_add(self.add_lo.wrapping_shl(1));
        self.dr = (ofs >> 1) as u16;
        self.handler = Handler::AbsorbThenReset;
    }

    fn op10(&mut self) {
        if self.dr == 0xffff {
            self.reset();
        }
    }

    fn op3e(&mut self) {
        self.op3e_x = Self::lo_byte_word(self.dr);
        self.op3e_y = Self::hi_byte_word(self.dr);

        self.op03_compute();
        let cell = (self.dr as usize) & 0x1fff;
        self.op1e_terrain[cell] = 0x00;
        self.op1e_cost[cell] = 0xff;
        self.op1e_weight[cell] = 0;

        self.op1e_max_search_radius = 0;
        self.op1e_max_path_radius = 0;
        self.handler = Handler::AbsorbThenReset;
    }

    fn convert(&mut self) {
        self.count = self.dr;
        self.bm_index = 0;
        self.handler = Handler::ConvertA;
    }

    fn convert_a(&mut self) {
        if self.bm_index < 8 {
            self.bitmap[self.bm_index] = self.dr as u8;
            self.bm_index += 1;
            self.bitmap[self.bm_index] = (self.dr >> 8) as u8;
            self.bm_index += 1;

            if self.bm_index == 8 {
                self.bitplane = [0; 8];
                for i in 0..8 {
                    for j in 0..8 {
                        self.bitplane[j] <<= 1;
                        self.bitplane[j] |= (self.bitmap[i] >> j) & 1;
                    }
                }
                self.bp_index = 0;
                self.count = self.count.wrapping_sub(1);
            }
        }

        if self.bm_index == 8 {
            if self.bp_index == 8 {
                if self.count == 0 {
                    self.reset();
                }
                self.bm_index = 0;
            } else {
                self.dr = self.bitplane[self.bp_index] as u16;
                self.bp_index += 1;
                self.dr |= (self.bitplane[self.bp_index] as u16) << 8;
                self.bp_index += 1;
            }
        }
    }

    fn get_bits(&mut self, count: u8) -> bool {
        if self.bits_left == 0 {
            self.bits_left = count as u16;
            self.req_bits = 0;
        }

        loop {
            if self.bit_count == 0 {
                self.sr = 0x00c0;
                return false;
            }

            self.req_bits <<= 1;
            if self.req_data & 0x8000 != 0 {
                self.req_bits = self.req_bits.wrapping_add(1);
            }
            self.req_data <<= 1;
            self.bit_count = self.bit_count.wrapping_sub(1);
            self.bits_left = self.bits_left.wrapping_sub(1);

            if self.bits_left == 0 {
                return true;
            }
        }
    }

    fn decode(&mut self) {
        self.codewords = self.dr;
        self.handler = Handler::DecodeA;
    }

    fn decode_a(&mut self) {
        self.outwords = self.dr;
        self.handler = Handler::DecodeSymbols;
        self.bit_count = 0;
        self.bits_left = 0;
        self.symbol = 0;
        self.index = 0;
        self.bit_command = 0xffff;
        self.sr = 0x00c0;
    }

    fn decode_symbols(&mut self) {
        self.req_data = self.dr;
        self.bit_count = self.bit_count.wrapping_add(16);

        while self.codewords != 0 {
            if self.bit_command == 0xffff {
                if !self.get_bits(2) {
                    return;
                }
                self.bit_command = self.req_bits;
            }

            match self.bit_command {
                0 => {
                    if !self.get_bits(9) {
                        return;
                    }
                    self.symbol = self.req_bits;
                }
                1 => {
                    self.symbol = self.symbol.wrapping_add(1);
                }
                2 => {
                    if !self.get_bits(1) {
                        return;
                    }
                    self.symbol = self.symbol.wrapping_add(2).wrapping_add(self.req_bits);
                }
                3 => {
                    if !self.get_bits(4) {
                        return;
                    }
                    self.symbol = self.symbol.wrapping_add(4).wrapping_add(self.req_bits);
                }
                _ => {}
            }

            self.bit_command = 0xffff;
            if self.index < self.codes.len() {
                self.codes[self.index] = self.symbol;
            }
            self.index += 1;
            self.codewords = self.codewords.wrapping_sub(1);
        }

        self.index = 0;
        self.symbol = 0;
        self.base_codes = 0;
        self.handler = Handler::DecodeTree;
        if self.bit_count != 0 {
            self.decode_tree();
        }
    }

    fn decode_tree(&mut self) {
        if self.bit_count == 0 {
            self.req_data = self.dr;
            self.bit_count = self.bit_count.wrapping_add(16);
        }

        if self.base_codes == 0 {
            let _ = self.get_bits(1);
            if self.req_bits != 0 {
                self.base_length = 3;
                self.base_codes = 8;
            } else {
                self.base_length = 2;
                self.base_codes = 4;
            }
        }

        while self.base_codes != 0 {
            if !self.get_bits(3) {
                return;
            }

            self.req_bits = self.req_bits.wrapping_add(1);
            if self.index < self.code_lengths.len() {
                self.code_lengths[self.index] = self.req_bits as u8;
                self.code_offsets[self.index] = self.symbol;
            }
            self.index += 1;
            self.symbol = self.symbol.wrapping_add(1u16 << self.req_bits);
            self.base_codes = self.base_codes.wrapping_sub(1);
        }

        self.base_code = 0xffff;
        self.lz_code = 0;
        self.handler = Handler::DecodeData;
        if self.bit_count != 0 {
            self.decode_data();
        }
    }

    fn decode_data(&mut self) {
        if self.bit_count == 0 {
            if self.sr & 0x40 != 0 {
                self.req_data = self.dr;
                self.bit_count = self.bit_count.wrapping_add(16);
            } else {
                self.sr = 0x00c0;
                return;
            }
        }

        if self.lz_code == 1 {
            if !self.get_bits(1) {
                return;
            }
            self.lz_length = if self.req_bits != 0 { 12 } else { 8 };
            self.lz_code = self.lz_code.wrapping_add(1);
        }

        if self.lz_code == 2 {
            if !self.get_bits(self.lz_length) {
                return;
            }
            self.lz_code = 0;
            self.outwords = self.outwords.wrapping_sub(1);
            if self.outwords == 0 {
                self.handler = Handler::AbsorbThenReset;
            }
            self.sr = 0x0080;
            self.dr = self.req_bits;
            return;
        }

        if self.base_code == 0xffff {
            if !self.get_bits(self.base_length) {
                return;
            }
            self.base_code = self.req_bits;
        }

        let base = self.base_code as usize;
        if base >= self.code_lengths.len() {
            self.reset();
            return;
        }
        if !self.get_bits(self.code_lengths[base]) {
            return;
        }

        let code_index = self.code_offsets[base].wrapping_add(self.req_bits) as usize;
        self.symbol = self.codes.get(code_index).copied().unwrap_or(0);
        self.base_code = 0xffff;

        if self.symbol & 0xff00 != 0 {
            self.symbol = self.symbol.wrapping_add(0x7f02);
            self.lz_code = self.lz_code.wrapping_add(1);
        } else {
            self.outwords = self.outwords.wrapping_sub(1);
            if self.outwords == 0 {
                self.handler = Handler::AbsorbThenReset;
            }
        }

        self.sr = 0x0080;
        self.dr = self.symbol;
    }

    fn op1e(&mut self) {
        self.op1e_min_radius = Self::lo_byte_word(self.dr);
        self.op1e_max_radius = Self::hi_byte_word(self.dr);

        if self.op1e_min_radius == 0 {
            self.op1e_min_radius = 1;
        }

        if self.op1e_max_search_radius >= self.op1e_min_radius {
            self.op1e_min_radius = self.op1e_max_search_radius + 1;
        }

        if self.op1e_max_radius > self.op1e_max_search_radius {
            self.op1e_max_search_radius = self.op1e_max_radius;
        }

        self.op1e_lcv_radius = self.op1e_min_radius;
        self.op1e_lcv_steps = self.op1e_min_radius;
        self.op1e_lcv_turns = 6;
        self.op1e_turn = 0;
        self.op1e_x = self.op3e_x;
        self.op1e_y = self.op3e_y;

        for _ in 0..self.op1e_min_radius {
            self.op1e_d(self.op1e_turn);
        }

        self.op1e_a();
    }

    fn op1e_a(&mut self) {
        if self.op1e_lcv_steps == 0 {
            self.op1e_lcv_radius += 1;
            self.op1e_lcv_steps = self.op1e_lcv_radius;
            self.op1e_x = self.op3e_x;
            self.op1e_y = self.op3e_y;

            for _ in 0..self.op1e_lcv_radius {
                self.op1e_d(self.op1e_turn);
            }
        }

        if self.op1e_lcv_radius > self.op1e_max_radius {
            self.op1e_turn += 1;
            self.op1e_lcv_turns -= 1;
            self.op1e_lcv_radius = self.op1e_min_radius;
            self.op1e_lcv_steps = self.op1e_min_radius;
            self.op1e_x = self.op3e_x;
            self.op1e_y = self.op3e_y;

            for _ in 0..self.op1e_min_radius {
                self.op1e_d(self.op1e_turn);
            }
        }

        if self.op1e_lcv_turns == 0 {
            self.dr = 0xffff;
            self.sr = 0x0080;
            self.handler = Handler::Op1eB;
            return;
        }

        self.op1e_cell = self.cell_from_xy(self.op1e_x, self.op1e_y);
        self.sr = 0x0080;
        self.handler = Handler::Op1eA1;
    }

    fn op1e_a1(&mut self) {
        self.sr = 0x0084;
        self.handler = Handler::Op1eA2;
    }

    fn op1e_a2(&mut self) {
        self.op1e_terrain[self.op1e_cell] = (self.dr & 0x00ff) as i16;
        self.sr = 0x0084;
        self.handler = Handler::Op1eA3;
    }

    fn op1e_a3(&mut self) {
        self.op1e_cost[self.op1e_cell] = (self.dr & 0x00ff) as i16;

        if self.op1e_lcv_radius == 1 {
            if self.op1e_terrain[self.op1e_cell] & 1 != 0 {
                self.op1e_weight[self.op1e_cell] = 0xff;
            } else {
                self.op1e_weight[self.op1e_cell] = self.op1e_cost[self.op1e_cell];
            }
        } else {
            self.op1e_weight[self.op1e_cell] = 0xff;
        }

        self.op1e_d(self.op1e_turn + 2);
        self.op1e_lcv_steps -= 1;
        self.sr = 0x0080;
        self.op1e_a();
    }

    fn op1e_b(&mut self) {
        self.op1e_x = self.op3e_x;
        self.op1e_y = self.op3e_y;
        self.op1e_lcv_radius = 1;

        self.op1e_b1();
        self.handler = Handler::Op1eC;
    }

    fn op1e_b1(&mut self) {
        while self.op1e_lcv_radius < self.op1e_max_radius {
            self.op1e_y -= 1;
            self.op1e_lcv_turns = 6;
            self.op1e_turn = 5;

            while self.op1e_lcv_turns != 0 {
                self.op1e_lcv_steps = self.op1e_lcv_radius;

                while self.op1e_lcv_steps != 0 {
                    self.op1e_d1(self.op1e_turn);

                    if 0 <= self.op1e_y
                        && self.op1e_y < self.win_hi
                        && 0 <= self.op1e_x
                        && self.op1e_x < self.win_lo
                    {
                        self.op1e_cell = self.cell_from_xy(self.op1e_x, self.op1e_y);
                        if self.op1e_cost[self.op1e_cell] < 0x80
                            && self.op1e_terrain[self.op1e_cell] < 0x40
                        {
                            self.op1e_b2();
                        }
                    }

                    self.op1e_lcv_steps -= 1;
                }

                self.op1e_turn -= 1;
                if self.op1e_turn == 0 {
                    self.op1e_turn = 6;
                }

                self.op1e_lcv_turns -= 1;
            }

            self.op1e_lcv_radius += 1;
        }
    }

    fn op1e_b2(&mut self) {
        let mut path = 0xff;
        let mut lcv_turns = 6;

        while lcv_turns != 0 {
            let mut x = self.op1e_x;
            let mut y = self.op1e_y;
            self.op1e_d1_at(lcv_turns, &mut x, &mut y);

            if 0 <= y && y < self.win_hi && 0 <= x && x < self.win_lo {
                let cell = self.cell_from_xy(x, y);
                if self.op1e_terrain[cell] < 0x80 || self.op1e_weight[cell] == 0 {
                    if self.op1e_weight[cell] < path {
                        path = self.op1e_weight[cell];
                    }
                }
            }

            lcv_turns -= 1;
        }

        if path != 0xff {
            self.op1e_weight[self.op1e_cell] = path + self.op1e_cost[self.op1e_cell];
        }
    }

    fn op1e_c(&mut self) {
        self.op1e_min_radius = Self::lo_byte_word(self.dr);
        self.op1e_max_radius = Self::hi_byte_word(self.dr);

        if self.op1e_min_radius == 0 {
            self.op1e_min_radius = 1;
        }

        if self.op1e_max_path_radius >= self.op1e_min_radius {
            self.op1e_min_radius = self.op1e_max_path_radius + 1;
        }

        if self.op1e_max_radius > self.op1e_max_path_radius {
            self.op1e_max_path_radius = self.op1e_max_radius;
        }

        self.op1e_lcv_radius = self.op1e_min_radius;
        self.op1e_lcv_steps = self.op1e_min_radius;
        self.op1e_lcv_turns = 6;
        self.op1e_turn = 0;
        self.op1e_x = self.op3e_x;
        self.op1e_y = self.op3e_y;

        for _ in 0..self.op1e_min_radius {
            self.op1e_d(self.op1e_turn);
        }

        self.op1e_c1();
    }

    fn op1e_c1(&mut self) {
        if self.op1e_lcv_steps == 0 {
            self.op1e_lcv_radius += 1;
            self.op1e_lcv_steps = self.op1e_lcv_radius;
            self.op1e_x = self.op3e_x;
            self.op1e_y = self.op3e_y;

            for _ in 0..self.op1e_lcv_radius {
                self.op1e_d(self.op1e_turn);
            }
        }

        if self.op1e_lcv_radius > self.op1e_max_radius {
            self.op1e_turn += 1;
            self.op1e_lcv_turns -= 1;
            self.op1e_lcv_radius = self.op1e_min_radius;
            self.op1e_lcv_steps = self.op1e_min_radius;
            self.op1e_x = self.op3e_x;
            self.op1e_y = self.op3e_y;

            for _ in 0..self.op1e_min_radius {
                self.op1e_d(self.op1e_turn);
            }
        }

        if self.op1e_lcv_turns == 0 {
            self.dr = 0xffff;
            self.sr = 0x0080;
            self.handler = Handler::AbsorbThenReset;
            return;
        }

        self.op1e_cell = self.cell_from_xy(self.op1e_x, self.op1e_y);
        self.sr = 0x0080;
        self.handler = Handler::Op1eC2;
    }

    fn op1e_c2(&mut self) {
        self.dr = self.op1e_weight[self.op1e_cell] as u16;
        self.op1e_d(self.op1e_turn + 2);
        self.op1e_lcv_steps -= 1;
        self.sr = 0x0084;
        self.handler = Handler::Op1eC1;
    }

    fn op1e_d(&mut self, movement: i16) {
        self.load_direction_add(movement);
        let lo = (self.op1e_x as u8) as i16;
        let hi = (self.op1e_y as u8) as i16;
        let (next_lo, next_hi) = self.step_wrapped(lo, hi);
        self.op1e_x = next_lo;
        self.op1e_y = next_hi;
    }

    fn op1e_d1(&mut self, movement: i16) {
        let mut x = self.op1e_x;
        let mut y = self.op1e_y;
        self.op1e_d1_at(movement, &mut x, &mut y);
        self.op1e_x = x;
        self.op1e_y = y;
    }

    fn op1e_d1_at(&mut self, movement: i16, lo: &mut i16, hi: &mut i16) {
        const HI_ADD: [i16; 16] = [
            0x00, 0xff, 0x00, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0xff, 0xff, 0x00, 0x01, 0x00,
            0xff, 0x00,
        ];
        const LO_ADD: [i16; 8] = [0x00, 0x00, 0x01, 0x01, 0x00, 0xff, 0xff, 0x00];

        let movement = movement.clamp(0, 7) as usize;
        self.add_hi = if *lo & 1 != 0 {
            HI_ADD[movement + 8]
        } else {
            HI_ADD[movement]
        };
        self.add_lo = LO_ADD[movement];

        let lo_byte = (*lo as u8) as i16;
        let mut hi_byte = (*hi as u8) as i16;
        if lo_byte & 1 != 0 {
            hi_byte += self.add_lo & 1;
        }

        self.add_lo += lo_byte;
        self.add_hi += hi_byte;
        *lo = self.add_lo;
        *hi = self.add_hi;
    }
}

fn data_rom_words() -> Vec<u16> {
    DSP3_DATA_ROM.to_vec()
}

const DSP3_DATA_ROM: [u16; 1024] = [
    0x8000, 0x4000, 0x2000, 0x1000, 0x0800, 0x0400, 0x0200, 0x0100, 0x0080, 0x0040, 0x0020, 0x0010,
    0x0008, 0x0004, 0x0002, 0x0001, 0x0002, 0x0004, 0x0008, 0x0010, 0x0020, 0x0040, 0x0080, 0x0100,
    0x0000, 0x000f, 0x0400, 0x0200, 0x0140, 0x0400, 0x0200, 0x0040, 0x007d, 0x007e, 0x007e, 0x007b,
    0x007c, 0x007d, 0x007b, 0x007c, 0x0002, 0x0020, 0x0030, 0x0000, 0x000d, 0x0019, 0x0026, 0x0032,
    0x003e, 0x004a, 0x0056, 0x0062, 0x006d, 0x0079, 0x0084, 0x008e, 0x0098, 0x00a2, 0x00ac, 0x00b5,
    0x00be, 0x00c6, 0x00ce, 0x00d5, 0x00dc, 0x00e2, 0x00e7, 0x00ec, 0x00f1, 0x00f5, 0x00f8, 0x00fb,
    0x00fd, 0x00ff, 0x0100, 0x0100, 0x0100, 0x00ff, 0x00fd, 0x00fb, 0x00f8, 0x00f5, 0x00f1, 0x00ed,
    0x00e7, 0x00e2, 0x00dc, 0x00d5, 0x00ce, 0x00c6, 0x00be, 0x00b5, 0x00ac, 0x00a2, 0x0099, 0x008e,
    0x0084, 0x0079, 0x006e, 0x0062, 0x0056, 0x004a, 0x003e, 0x0032, 0x0026, 0x0019, 0x000d, 0x0000,
    0xfff3, 0xffe7, 0xffdb, 0xffce, 0xffc2, 0xffb6, 0xffaa, 0xff9e, 0xff93, 0xff87, 0xff7d, 0xff72,
    0xff68, 0xff5e, 0xff54, 0xff4b, 0xff42, 0xff3a, 0xff32, 0xff2b, 0xff25, 0xff1e, 0xff19, 0xff14,
    0xff0f, 0xff0b, 0xff08, 0xff05, 0xff03, 0xff01, 0xff00, 0xff00, 0xff00, 0xff01, 0xff03, 0xff05,
    0xff08, 0xff0b, 0xff0f, 0xff13, 0xff18, 0xff1e, 0xff24, 0xff2b, 0xff32, 0xff3a, 0xff42, 0xff4b,
    0xff54, 0xff5d, 0xff67, 0xff72, 0xff7c, 0xff87, 0xff92, 0xff9e, 0xffa9, 0xffb5, 0xffc2, 0xffce,
    0xffda, 0xffe7, 0xfff3, 0x002b, 0x007f, 0x0020, 0x00ff, 0xff00, 0xffbe, 0x0000, 0x0044, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0xffc1, 0x0001, 0x0002, 0x0045, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0xffc5, 0x0003, 0x0004, 0x0005, 0x0047, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0xffca, 0x0006, 0x0007, 0x0008,
    0x0009, 0x004a, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0xffd0, 0x000a, 0x000b, 0x000c, 0x000d, 0x000e, 0x004e, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0xffd7, 0x000f, 0x0010, 0x0011, 0x0012, 0x0013, 0x0014, 0x0053, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0xffdf, 0x0015, 0x0016, 0x0017,
    0x0018, 0x0019, 0x001a, 0x001b, 0x0059, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0xffe8, 0x001c, 0x001d, 0x001e, 0x001f, 0x0020, 0x0021, 0x0022,
    0x0023, 0x0060, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0xfff2, 0x0024, 0x0025, 0x0026, 0x0027, 0x0028, 0x0029, 0x002a, 0x002b, 0x002c, 0x0068, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0xfffd, 0x002d, 0x002e, 0x002f,
    0x0030, 0x0031, 0x0032, 0x0033, 0x0034, 0x0035, 0x0036, 0x0071, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0xffc7, 0x0037, 0x0038, 0x0039, 0x003a, 0x003b, 0x003c, 0x003d,
    0x003e, 0x003f, 0x0040, 0x0041, 0x007b, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0xffd4, 0x0000, 0x0001, 0x0002, 0x0003, 0x0004, 0x0005, 0x0006, 0x0007, 0x0008, 0x0009, 0x000a,
    0x000b, 0x0044, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0xffe2, 0x000c, 0x000d, 0x000e,
    0x000f, 0x0010, 0x0011, 0x0012, 0x0013, 0x0014, 0x0015, 0x0016, 0x0017, 0x0018, 0x0050, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0xfff1, 0x0019, 0x001a, 0x001b, 0x001c, 0x001d, 0x001e, 0x001f,
    0x0020, 0x0021, 0x0022, 0x0023, 0x0024, 0x0025, 0x0026, 0x005d, 0x0000, 0x0000, 0x0000, 0x0000,
    0xffcb, 0x0027, 0x0028, 0x0029, 0x002a, 0x002b, 0x002c, 0x002d, 0x002e, 0x002f, 0x0030, 0x0031,
    0x0032, 0x0033, 0x0034, 0x0035, 0x006b, 0x0000, 0x0000, 0x0000, 0xffdc, 0x0000, 0x0001, 0x0002,
    0x0003, 0x0004, 0x0005, 0x0006, 0x0007, 0x0008, 0x0009, 0x000a, 0x000b, 0x000c, 0x000d, 0x000e,
    0x000f, 0x0044, 0x0000, 0x0000, 0xffee, 0x0010, 0x0011, 0x0012, 0x0013, 0x0014, 0x0015, 0x0016,
    0x0017, 0x0018, 0x0019, 0x001a, 0x001b, 0x001c, 0x001d, 0x001e, 0x001f, 0x0020, 0x0054, 0x0000,
    0xffee, 0x0021, 0x0022, 0x0023, 0x0024, 0x0025, 0x0026, 0x0027, 0x0028, 0x0029, 0x002a, 0x002b,
    0x002c, 0x002d, 0x002e, 0x002f, 0x0030, 0x0031, 0x0032, 0x0065, 0xffbe, 0x0000, 0xfeac, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0xffc1, 0x0001, 0x0002, 0xfead, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0xffc5, 0x0003, 0x0004, 0x0005, 0xfeaf, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0xffca, 0x0006, 0x0007, 0x0008,
    0x0009, 0xfeb2, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0xffd0, 0x000a, 0x000b, 0x000c, 0x000d, 0x000e, 0xfeb6, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0xffd7, 0x000f, 0x0010, 0x0011, 0x0012, 0x0013, 0x0014, 0xfebb, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0xffdf, 0x0015, 0x0016, 0x0017,
    0x0018, 0x0019, 0x001a, 0x001b, 0xfec1, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0xffe8, 0x001c, 0x001d, 0x001e, 0x001f, 0x0020, 0x0021, 0x0022,
    0x0023, 0xfec8, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0xfff2, 0x0024, 0x0025, 0x0026, 0x0027, 0x0028, 0x0029, 0x002a, 0x002b, 0x002c, 0xfed0, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0xfffd, 0x002d, 0x002e, 0x002f,
    0x0030, 0x0031, 0x0032, 0x0033, 0x0034, 0x0035, 0x0036, 0xfed9, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0xffc7, 0x0037, 0x0038, 0x0039, 0x003a, 0x003b, 0x003c, 0x003d,
    0x003e, 0x003f, 0x0040, 0x0041, 0xfee3, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0xffd4, 0x0000, 0x0001, 0x0002, 0x0003, 0x0004, 0x0005, 0x0006, 0x0007, 0x0008, 0x0009, 0x000a,
    0x000b, 0xfeac, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0xffe2, 0x000c, 0x000d, 0x000e,
    0x000f, 0x0010, 0x0011, 0x0012, 0x0013, 0x0014, 0x0015, 0x0016, 0x0017, 0x0018, 0xfeb8, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0xfff1, 0x0019, 0x001a, 0x001b, 0x001c, 0x001d, 0x001e, 0x001f,
    0x0020, 0x0021, 0x0022, 0x0023, 0x0024, 0x0025, 0x0026, 0xfec5, 0x0000, 0x0000, 0x0000, 0x0000,
    0xffcb, 0x0027, 0x0028, 0x0029, 0x002a, 0x002b, 0x002c, 0x002d, 0x002e, 0x002f, 0x0030, 0x0031,
    0x0032, 0x0033, 0x0034, 0x0035, 0xfed3, 0x0000, 0x0000, 0x0000, 0xffdc, 0x0000, 0x0001, 0x0002,
    0x0003, 0x0004, 0x0005, 0x0006, 0x0007, 0x0008, 0x0009, 0x000a, 0x000b, 0x000c, 0x000d, 0x000e,
    0x000f, 0xfeac, 0x0000, 0x0000, 0xffee, 0x0010, 0x0011, 0x0012, 0x0013, 0x0014, 0x0015, 0x0016,
    0x0017, 0x0018, 0x0019, 0x001a, 0x001b, 0x001c, 0x001d, 0x001e, 0x001f, 0x0020, 0xfebc, 0x0000,
    0xffee, 0x0021, 0x0022, 0x0023, 0x0024, 0x0025, 0x0026, 0x0027, 0x0028, 0x0029, 0x002a, 0x002b,
    0x002c, 0x002d, 0x002e, 0x002f, 0x0030, 0x0031, 0x0032, 0xfecd, 0x0154, 0x0218, 0x0110, 0x00b0,
    0x00cc, 0x00b0, 0x0088, 0x00b0, 0x0044, 0x00b0, 0x0000, 0x00b0, 0x00fe, 0xff07, 0x0002, 0x00ff,
    0x00f8, 0x0007, 0x00fe, 0x00ee, 0x07ff, 0x0200, 0x00ef, 0xf800, 0x0700, 0x00ee, 0xffff, 0xffff,
    0xffff, 0x0000, 0x0000, 0x0001, 0x0001, 0x0001, 0x0001, 0x0000, 0x0000, 0xffff, 0xffff, 0xffff,
    0xffff, 0x0000, 0x0000, 0x0001, 0x0001, 0x0001, 0x0001, 0x0000, 0x0000, 0xffff, 0xffff, 0x0000,
    0xffff, 0x0001, 0x0000, 0x0001, 0x0001, 0x0000, 0x0000, 0xffff, 0xffff, 0xffff, 0xffff, 0x0000,
    0xffff, 0x0001, 0x0000, 0x0001, 0x0001, 0x0000, 0x0000, 0xffff, 0xffff, 0xffff, 0x0000, 0x0000,
    0x0000, 0x0044, 0x0088, 0x00cc, 0x0110, 0x0154, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff,
    0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff,
    0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff,
    0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff,
    0xffff, 0xffff, 0xffff, 0xffff,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_exposes_completion_marker() {
        let dsp = Dsp3::new();
        assert_eq!(dsp.read_sr(), 0x84);
    }

    #[test]
    fn memory_test_completes_with_dsp3_marker() {
        let mut dsp = Dsp3::new();
        dsp.write_dr(0x0f);
        assert_eq!(dsp.read_dr(), 0x80);
    }

    #[test]
    fn sixteen_bit_transfers_expose_byte_phase_in_status() {
        let mut dsp = Dsp3::new();
        dsp.write_dr(0x18);
        assert_eq!(dsp.read_sr(), 0x80);

        dsp.write_dr(0x34);
        assert_eq!(dsp.read_sr(), 0x90);

        dsp.write_dr(0x12);
        assert_eq!(dsp.read_sr(), 0x80);
    }

    #[test]
    fn sixteen_bit_reads_expose_byte_phase_in_status() {
        let mut dsp = Dsp3::new();
        dsp.write_dr(0x1f);

        assert_eq!(dsp.read_sr(), 0x80);
        assert_eq!(dsp.read_dr(), 0x00);
        assert_eq!(dsp.read_sr(), 0x90);
        assert_eq!(dsp.read_dr(), 0x80);
        assert_eq!(dsp.read_sr(), 0x80);
    }
}
