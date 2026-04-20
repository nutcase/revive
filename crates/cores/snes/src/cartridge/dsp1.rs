//! DSP-1 coprocessor emulation (NEC μPD77C25) — HLE approach.
//!
//! Provides 3D math operations used by games like Pilotwings, Super Mario Kart,
//! and others for Mode 7 perspective projection, rotation, and trigonometry.
//!
//! Reference: snes9x DSP-1 HLE implementation (byte-based I/O protocol).

#![allow(clippy::precedence)]

/// DSP-1 address mapping variant.
///   TypeA (Pilotwings, small LoROM): DR at $20-$3F/$A0-$BF:$8000-$BFFF, SR at $C000-$FFFF
///   TypeB (Super Mario Kart, large LoROM): DR at $60-$6F/$E0-$EF:$0000-$3FFF, SR at $4000-$7FFF
///   HiRom (Super F1 Circus Gaiden): DR at $00-$1F/$80-$9F:$6000-$6FFF, SR at $7000-$7FFF
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Dsp1Mapping {
    TypeA,
    TypeB,
    HiRom,
}

/// MaxAZS_Exp table from snes9x — used for zenith angle clipping in Op02.
const MAX_AZS_EXP: [i16; 16] = [
    0x38b4, 0x38b7, 0x38ba, 0x38be, 0x38c0, 0x38c4, 0x38c7, 0x38ca, 0x38ce, 0x38d0, 0x38d4, 0x38d7,
    0x38da, 0x38dd, 0x38e0, 0x38e4,
];

const DSP1_OUTPUT_BUFFER_LEN: usize = 2048;

#[derive(Debug, Clone)]
pub struct Dsp1 {
    pub mapping: Dsp1Mapping,

    // I/O state
    waiting4command: bool,
    first_parameter: bool,
    command: u8,
    in_count: usize,
    in_index: usize,
    out_count: usize,
    out_index: usize,
    parameters: Vec<u8>,
    output: Vec<u8>,

    // Attitude matrices (3 sets: A, B, C)
    matrix_a: [[i16; 3]; 3],
    matrix_b: [[i16; 3]; 3],
    matrix_c: [[i16; 3]; 3],

    // Projection state (set by Op02/Parameter)
    centre_x: i16,
    centre_y: i16,
    v_offset: i16,
    vplane_c: i16,
    vplane_e: i16,
    sin_aas: i16,
    cos_aas: i16,
    sin_azs: i16,
    cos_azs: i16,
    // Clipped zenith trig (SinAZS/CosAZS in snes9x — uppercase)
    sin_azs_clipped: i16,
    cos_azs_clipped: i16,
    sec_azs_c1: i16,
    sec_azs_e1: i16,
    sec_azs_c2: i16,
    sec_azs_e2: i16,
    nx: i16,
    ny: i16,
    nz: i16,
    gx: i16,
    gy: i16,
    gz: i16,
    c_les: i16,
    e_les: i16,
    g_les: i16,

    // Op0A raster state
    op0a_vs: i16,
    op0a_a: i16,
    op0a_b: i16,
    op0a_c: i16,
    op0a_d: i16,
}

impl Dsp1 {
    pub fn new(rom_size: usize) -> Self {
        Self::with_mapping(if rom_size <= 0x100000 {
            Dsp1Mapping::TypeA
        } else {
            Dsp1Mapping::TypeB
        })
    }

    pub fn new_hirom() -> Self {
        Self::with_mapping(Dsp1Mapping::HiRom)
    }

    fn with_mapping(mapping: Dsp1Mapping) -> Self {
        Dsp1 {
            mapping,
            waiting4command: true,
            first_parameter: true,
            command: 0,
            in_count: 0,
            in_index: 0,
            out_count: 0,
            out_index: 0,
            parameters: vec![0u8; 512],
            output: vec![0u8; DSP1_OUTPUT_BUFFER_LEN],

            matrix_a: [[0; 3]; 3],
            matrix_b: [[0; 3]; 3],
            matrix_c: [[0; 3]; 3],

            centre_x: 0,
            centre_y: 0,
            v_offset: 0,
            vplane_c: 0,
            vplane_e: 0,
            sin_aas: 0,
            cos_aas: 0,
            sin_azs: 0,
            cos_azs: 0,
            sin_azs_clipped: 0,
            cos_azs_clipped: 0,
            sec_azs_c1: 0,
            sec_azs_e1: 0,
            sec_azs_c2: 0,
            sec_azs_e2: 0,
            nx: 0,
            ny: 0,
            nz: 0,
            gx: 0,
            gy: 0,
            gz: 0,
            c_les: 0,
            e_les: 0,
            g_les: 0,

            op0a_vs: 0,
            op0a_a: 0,
            op0a_b: 0,
            op0a_c: 0,
            op0a_d: 0,
        }
    }

    // -----------------------------------------------------------------------
    // SR — Status Register
    //   bit 7 (RQM): 1 = ready for data transfer
    //   bit 6 (DRS): 0 = output available (read DR), 1 = input expected (write DR)
    // -----------------------------------------------------------------------
    pub fn read_sr(&self) -> u8 {
        // NEC μPD77C25 SR format:
        //   bit 7 (0x80): RQM - Request for Master (1 = ready for transfer)
        //   bit 6 (0x40): DRS - Data Register Status (0 = output available, 1 = input expected)
        if self.out_count > 0 {
            // Output available: RQM=1, DRS=0 → game should read DR
            0x80
        } else {
            // Waiting for command or parameters: RQM=1, DRS=1 → game should write DR
            0x80 | 0x40
        }
    }

    // -----------------------------------------------------------------------
    // DR read (GetByte)
    // -----------------------------------------------------------------------
    pub fn read_dr(&mut self) -> u8 {
        if self.out_count > 0 {
            let t = if self.out_index < self.output.len() {
                self.output[self.out_index]
            } else {
                0
            };
            self.out_index += 1;
            self.out_count -= 1;

            if self.out_count == 0 {
                // Op0A/1A streaming: auto-repeat raster
                if self.command == 0x0A || self.command == 0x1A {
                    self.exec_op0a();
                    self.out_count = 8;
                    self.out_index = 0;
                    self.write_word_out(0, self.op0a_a);
                    self.write_word_out(2, self.op0a_b);
                    self.write_word_out(4, self.op0a_c);
                    self.write_word_out(6, self.op0a_d);
                } else {
                    self.waiting4command = true;
                }
            }
            t
        } else {
            // snes9x returns 0xff when no output pending
            0xFF
        }
    }

    // -----------------------------------------------------------------------
    // DR write (SetByte)
    // -----------------------------------------------------------------------
    pub fn write_dr(&mut self, byte: u8) {
        // snes9x: any write while output is pending consumes one output byte.
        // This allows the game to flush pending output before sending a new command.
        if self.out_count > 0 {
            self.out_count -= 1;
            self.out_index += 1;
            return;
        }

        if self.waiting4command {
            self.command = byte;
            self.in_index = 0;
            self.waiting4command = false;
            self.first_parameter = true;

            let (in_words, _out_bytes) = command_params(byte);
            self.in_count = in_words * 2; // convert words to bytes

            if self.in_count == 0 {
                // Command 0x80: NOP/reset
                self.waiting4command = true;
                self.first_parameter = true;
            }
        } else {
            self.first_parameter = false;

            if self.in_index < self.parameters.len() {
                self.parameters[self.in_index] = byte;
            }
            self.in_index += 1;
            self.in_count -= 1;

            if self.in_count == 0 {
                self.execute_command();
            }
        }
    }

    // -----------------------------------------------------------------------
    // Read/write helpers for parameter/output buffers
    // -----------------------------------------------------------------------
    fn read_word_in(&self, offset: usize) -> i16 {
        let lo = self.parameters[offset] as u16;
        let hi = self.parameters[offset + 1] as u16;
        (lo | (hi << 8)) as i16
    }

    fn write_word_out(&mut self, offset: usize, value: i16) {
        let v = value as u16;
        self.output[offset] = (v & 0xFF) as u8;
        self.output[offset + 1] = ((v >> 8) & 0xFF) as u8;
    }

    // -----------------------------------------------------------------------
    // Command execution
    // -----------------------------------------------------------------------
    fn execute_command(&mut self) {
        // Per snes9x: set waiting4command=true BEFORE executing, so the DSP is
        // immediately ready for the next command regardless of output availability.
        self.waiting4command = true;
        self.out_index = 0;
        self.out_count = 0;

        if std::env::var_os("TRACE_DSP1_CMD").is_some() {
            eprintln!(
                "[DSP1-CMD] cmd=0x{:02X} in_count={}",
                self.command, self.in_count
            );
        }

        match self.command {
            // Multiply
            0x00 => {
                let a = self.read_word_in(0) as i32;
                let b = self.read_word_in(2) as i32;
                let r = ((a * b) >> 15) as i16;
                self.write_word_out(0, r);
                self.out_count = 2;
            }
            // Inverse
            0x10 | 0x30 => {
                let coeff = self.read_word_in(0);
                let exp = self.read_word_in(2);
                let (rc, re) = dsp1_inverse(coeff, exp);
                self.write_word_out(0, rc);
                self.write_word_out(2, re);
                self.out_count = 4;
            }
            // Multiply (variant)
            0x20 => {
                let a = self.read_word_in(0) as i32;
                let b = self.read_word_in(2) as i32;
                let r = ((a * b) >> 15) as i16;
                self.write_word_out(0, r);
                self.out_count = 2;
            }
            // Sin/Cos
            0x04 | 0x24 => {
                let angle = self.read_word_in(0);
                let radius = self.read_word_in(2);
                let s = ((self.dsp_sin(angle) as i32 * radius as i32) >> 15) as i16;
                let c = ((self.dsp_cos(angle) as i32 * radius as i32) >> 15) as i16;
                self.write_word_out(0, s);
                self.write_word_out(2, c);
                self.out_count = 4;
            }
            // Radius squared
            0x08 => {
                let x = self.read_word_in(0) as i32;
                let y = self.read_word_in(2) as i32;
                let z = self.read_word_in(4) as i32;
                let r2 = x * x + y * y + z * z;
                let ll = (r2 & 0xFFFF) as i16;
                let lh = ((r2 >> 16) & 0xFFFF) as i16;
                self.write_word_out(0, ll);
                self.write_word_out(2, lh);
                self.out_count = 4;
            }
            // Range
            0x18 => {
                let x = self.read_word_in(0) as i32;
                let y = self.read_word_in(2) as i32;
                let z = self.read_word_in(4) as i32;
                let r = self.read_word_in(6) as i32;
                let dx = x - z;
                let dy = y - r;
                let dist = ((dx as f64 * dx as f64 + dy as f64 * dy as f64).sqrt()) as i16;
                self.write_word_out(0, dist);
                self.out_count = 2;
            }
            // Distance
            0x28 => {
                let x = self.read_word_in(0) as f64;
                let y = self.read_word_in(2) as f64;
                let z = self.read_word_in(4) as f64;
                let dist = (x * x + y * y + z * z).sqrt() as i16;
                self.write_word_out(0, dist);
                self.out_count = 2;
            }
            // Range (variant)
            0x38 => {
                let x = self.read_word_in(0) as i32;
                let y = self.read_word_in(2) as i32;
                let z = self.read_word_in(4) as i32;
                let r = self.read_word_in(6) as i32;
                let dx = x - z;
                let dy = y - r;
                let dist = ((dx as f64 * dx as f64 + dy as f64 * dy as f64).sqrt()) as i16;
                self.write_word_out(0, dist);
                self.out_count = 2;
            }
            // 2D Rotate
            0x0C | 0x2C => {
                let angle = self.read_word_in(0);
                let x = self.read_word_in(2) as i32;
                let y = self.read_word_in(4) as i32;
                let s = self.dsp_sin(angle) as i32;
                let c = self.dsp_cos(angle) as i32;
                let rx = ((c * x >> 15) + (-s * y >> 15)) as i16;
                let ry = ((s * x >> 15) + (c * y >> 15)) as i16;
                self.write_word_out(0, rx);
                self.write_word_out(2, ry);
                self.out_count = 4;
            }
            // 3D Polar Rotate
            0x1C | 0x3C => {
                self.exec_op1c();
            }
            // Parameter (projection setup)
            0x02 | 0x12 | 0x22 | 0x32 => {
                self.exec_parameter();
            }
            // Raster
            0x0A => {
                self.op0a_vs = self.read_word_in(0);
                self.exec_op0a();
                self.out_count = 8;
                self.write_word_out(0, self.op0a_a);
                self.write_word_out(2, self.op0a_b);
                self.write_word_out(4, self.op0a_c);
                self.write_word_out(6, self.op0a_d);
            }
            0x1A | 0x2A | 0x3A => {
                self.command = 0x1A; // normalize for auto-repeat
                self.op0a_vs = self.read_word_in(0);
                self.exec_op0a();
                self.out_count = 8;
                self.write_word_out(0, self.op0a_a);
                self.write_word_out(2, self.op0a_b);
                self.write_word_out(4, self.op0a_c);
                self.write_word_out(6, self.op0a_d);
            }
            // Project object
            0x06 | 0x16 | 0x26 | 0x36 => {
                self.exec_project();
            }
            // Target
            0x0E | 0x1E | 0x2E | 0x3E => {
                self.exec_target();
            }
            // Attitude matrix A
            0x01 | 0x05 | 0x31 | 0x35 => {
                self.exec_attitude_a();
            }
            // Attitude matrix B
            0x11 | 0x15 => {
                self.exec_attitude_b();
            }
            // Attitude matrix C
            0x21 | 0x25 => {
                self.exec_attitude_c();
            }
            // Objective (matrix A * vec)
            0x0D | 0x09 | 0x39 | 0x3D => {
                self.exec_objective(&self.matrix_a.clone());
            }
            // Objective (matrix B)
            0x1D | 0x19 => {
                self.exec_objective(&self.matrix_b.clone());
            }
            // Objective (matrix C)
            0x2D | 0x29 => {
                self.exec_objective(&self.matrix_c.clone());
            }
            // Subjective (vec * matrix A^T)
            0x03 | 0x33 => {
                self.exec_subjective(&self.matrix_a.clone());
            }
            0x13 => {
                self.exec_subjective(&self.matrix_b.clone());
            }
            0x23 => {
                self.exec_subjective(&self.matrix_c.clone());
            }
            // Scalar product
            0x0B | 0x3B => {
                self.exec_scalar(&self.matrix_a.clone());
            }
            0x1B => {
                self.exec_scalar(&self.matrix_b.clone());
            }
            0x2B => {
                self.exec_scalar(&self.matrix_c.clone());
            }
            // Gyroscope
            0x14 | 0x34 => {
                self.exec_gyrate();
            }
            // Memory test
            0x07 | 0x0F => {
                self.write_word_out(0, 0);
                self.out_count = 2;
            }
            // Size query
            0x27 | 0x2F => {
                self.write_word_out(0, 0x0100);
                self.out_count = 2;
            }
            // ROM dump (1F etc.)
            0x1F | 0x17 | 0x37 | 0x3F => {
                for i in 0..2048 {
                    self.output[i] = 0;
                }
                self.out_count = 2048;
            }
            // Unknown / NOP
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // Trig
    // -----------------------------------------------------------------------
    fn dsp_sin(&self, angle: i16) -> i16 {
        dsp1_sin(angle)
    }

    fn dsp_cos(&self, angle: i16) -> i16 {
        dsp1_cos(angle)
    }

    // -----------------------------------------------------------------------
    // Op01/11/21 — Attitude matrix
    // -----------------------------------------------------------------------
    fn build_attitude(&self) -> [[i16; 3]; 3] {
        let m = (self.read_word_in(0) as i32) >> 1; // halve m
        let zr = self.read_word_in(2);
        let yr = self.read_word_in(4);
        let xr = self.read_word_in(6);

        let sz = self.dsp_sin(zr) as i32;
        let cz = self.dsp_cos(zr) as i32;
        let sy = self.dsp_sin(yr) as i32;
        let cy = self.dsp_cos(yr) as i32;
        let sx = self.dsp_sin(xr) as i32;
        let cx = self.dsp_cos(xr) as i32;

        let mut mat = [[0i16; 3]; 3];
        mat[0][0] = ((m * cz >> 15) * cy >> 15) as i16;
        mat[0][1] = (-((m * sz >> 15) * cy >> 15)) as i16;
        mat[0][2] = (m * sy >> 15) as i16;

        mat[1][0] = (((m * sz >> 15) * cx >> 15) + (((m * cz >> 15) * sx >> 15) * sy >> 15)) as i16;
        mat[1][1] = (((m * cz >> 15) * cx >> 15) - (((m * sz >> 15) * sx >> 15) * sy >> 15)) as i16;
        mat[1][2] = (-((m * sx >> 15) * cy >> 15)) as i16;

        mat[2][0] = (((m * sz >> 15) * sx >> 15) - (((m * cz >> 15) * cx >> 15) * sy >> 15)) as i16;
        mat[2][1] = (((m * cz >> 15) * sx >> 15) + (((m * sz >> 15) * cx >> 15) * sy >> 15)) as i16;
        mat[2][2] = ((m * cx >> 15) * cy >> 15) as i16;

        mat
    }

    fn exec_attitude_a(&mut self) {
        self.matrix_a = self.build_attitude();
        if std::env::var_os("TRACE_DSP1").is_some() {
            eprintln!(
                "[DSP1-ATTITUDE-A] m={} zr={} yr={} xr={} mat={:?}",
                self.read_word_in(0),
                self.read_word_in(2),
                self.read_word_in(4),
                self.read_word_in(6),
                self.matrix_a
            );
        }
    }

    fn exec_attitude_b(&mut self) {
        self.matrix_b = self.build_attitude();
    }

    fn exec_attitude_c(&mut self) {
        self.matrix_c = self.build_attitude();
    }

    // -----------------------------------------------------------------------
    // Op02 — Parameter (projection setup) — matches snes9x DSP1_Parameter
    // -----------------------------------------------------------------------
    fn exec_parameter(&mut self) {
        let fx = self.read_word_in(0) as i32;
        let fy = self.read_word_in(2) as i32;
        let fz = self.read_word_in(4) as i32;
        let lfe = self.read_word_in(6) as i32;
        let les = self.read_word_in(8);
        let aas = self.read_word_in(10);
        let azs = self.read_word_in(12);

        // Store Sine and Cosine of Azimuth and Zenith angles
        self.sin_aas = self.dsp_sin(aas);
        self.cos_aas = self.dsp_cos(aas);
        self.sin_azs = self.dsp_sin(azs);
        self.cos_azs = self.dsp_cos(azs);

        // Normal vector: snes9x multiplies Nz by 0x7FFF
        self.nx = (self.sin_azs as i32 * -(self.sin_aas as i32) >> 15) as i16;
        self.ny = (self.sin_azs as i32 * self.cos_aas as i32 >> 15) as i16;
        self.nz = (self.cos_azs as i32 * 0x7FFF >> 15) as i16;

        let lfe_nx = (lfe * self.nx as i32) >> 15;
        let lfe_ny = (lfe * self.ny as i32) >> 15;
        let lfe_nz = (lfe * self.nz as i32) >> 15;

        // Centre of projection
        self.centre_x = (fx + lfe_nx) as i16;
        self.centre_y = (fy + lfe_ny) as i16;
        let centre_z = (fz + lfe_nz) as i16;

        let les32 = les as i32;
        let les_nx = (les32 * self.nx as i32) >> 15;
        let les_ny = (les32 * self.ny as i32) >> 15;
        let les_nz = (les32 * self.nz as i32) >> 15;

        // Eye position
        self.gx = (self.centre_x as i32 - les_nx) as i16;
        self.gy = (self.centre_y as i32 - les_ny) as i16;
        self.gz = (centre_z as i32 - les_nz) as i16;

        // Normalize Les
        self.e_les = 0;
        self.c_les = les;
        dsp1_normalize(&mut self.c_les, &mut self.e_les);
        self.g_les = les;

        // Normalize CentreZ
        let mut c = centre_z;
        let mut e: i16 = 0;
        dsp1_normalize(&mut c, &mut e);

        self.vplane_c = c;
        self.vplane_e = e;

        // Determine clip boundary for zenith angle
        let idx = (-e).clamp(0, 15) as usize;
        let max_azs = MAX_AZS_EXP[idx];

        // Clip zenith angle
        let mut azs_clipped = azs;
        if azs_clipped < 0 {
            let limit = -max_azs + 1;
            if azs_clipped < limit {
                azs_clipped = limit;
            }
        } else if azs_clipped > max_azs {
            azs_clipped = max_azs;
        }

        // Store clipped sin/cos
        self.sin_azs_clipped = self.dsp_sin(azs_clipped);
        self.cos_azs_clipped = self.dsp_cos(azs_clipped);

        // Compute SecAZS_C1/E1 = 1/cos(clipped AZS)
        let (sc1, se1) = dsp1_inverse(self.cos_azs_clipped, 0);
        self.sec_azs_c1 = sc1;
        self.sec_azs_e1 = se1;

        // Centre correction: adjust CentreX/CentreY based on clipping
        let mut cnorm = c;
        let mut enorm = e;
        {
            let product = (cnorm as i32 * self.sec_azs_c1 as i32 >> 15) as i16;
            cnorm = product;
            dsp1_normalize(&mut cnorm, &mut enorm);
        }
        enorm += self.sec_azs_e1;

        let correction =
            (dsp1_truncate(cnorm, enorm) as i32 * self.sin_azs_clipped as i32 >> 15) as i16;

        self.centre_x =
            (self.centre_x as i32 + (correction as i32 * self.sin_aas as i32 >> 15)) as i16;
        self.centre_y =
            (self.centre_y as i32 - (correction as i32 * self.cos_aas as i32 >> 15)) as i16;

        // Compute Vof (vertical offset due to clipping)
        let mut vof: i16 = 0;
        if azs != azs_clipped || azs == max_azs {
            let mut azs_diff = azs;
            if azs_diff == -32768 {
                azs_diff = -32767;
            }
            let mut diff = azs_diff - max_azs;
            if diff >= 0 {
                diff -= 1;
            }
            let aux = !(diff << 2);
            // Polynomial approximation using DSP1ROM constants
            // DSP1ROM[0x0328] = 0x14AC, DSP1ROM[0x0327] = 0x6488
            let tmp = (aux as i32 * 0x14ACi32 >> 15) as i16;
            let tmp2 = ((tmp as i32 * aux as i32 >> 15) + 0x6488) as i16;
            vof = (vof as i32 - (((tmp2 as i32 * aux as i32) >> 15) * les32 >> 15)) as i16;

            // Correct CosAZS for clipping
            let csq = (aux as i32 * aux as i32 >> 15) as i16;
            // DSP1ROM[0x0324] = 0x0A26, DSP1ROM[0x0325] = 0x277A
            let aux2 = ((csq as i32 * 0x0A26i32 >> 15) + 0x277A) as i16;
            self.cos_azs_clipped = (self.cos_azs_clipped as i32
                + ((csq as i32 * aux2 as i32 >> 15) * self.cos_azs_clipped as i32 >> 15))
                as i16;
        }

        // VOffset = Les * cos(clipped AZS)
        self.v_offset = (les32 * self.cos_azs_clipped as i32 >> 15) as i16;

        // Compute Vva
        let (csec, esec) = dsp1_inverse(self.sin_azs_clipped, 0);
        let mut cv = self.v_offset;
        let mut ev = esec;
        dsp1_normalize(&mut cv, &mut ev);
        let product = (cv as i32 * csec as i32 >> 15) as i16;
        cv = product;
        dsp1_normalize(&mut cv, &mut ev);
        if cv == -32768 {
            cv >>= 1;
            ev += 1;
        }
        let vva = dsp1_truncate(-cv, ev);

        // SecAZS_C2/E2 = 1/cos(clipped AZS) — for Raster
        let (sc2, se2) = dsp1_inverse(self.cos_azs_clipped, 0);
        self.sec_azs_c2 = sc2;
        self.sec_azs_e2 = se2;

        if std::env::var_os("TRACE_DSP1").is_some() {
            eprintln!("[DSP1-PARAM] cmd=0x{:02X} fx={} fy={} fz={} lfe={} les={} aas={} azs={} → vof={} vva={} cx={} cy={} nx={} ny={} nz={} gx={} gy={} gz={}",
                self.command, fx, fy, fz, lfe, les, aas, azs, vof, vva, self.centre_x, self.centre_y,
                self.nx, self.ny, self.nz, self.gx, self.gy, self.gz);
        }

        // Outputs
        self.write_word_out(0, vof);
        self.write_word_out(2, vva);
        self.write_word_out(4, self.centre_x);
        self.write_word_out(6, self.centre_y);
        self.out_count = 8;
    }

    // -----------------------------------------------------------------------
    // Op0A — Raster (per-scanline Mode 7 coefficients)
    // -----------------------------------------------------------------------
    fn exec_op0a(&mut self) {
        dsp1_raster(
            self.op0a_vs,
            self.sin_azs, // original (unclipped) SinAzs
            self.v_offset,
            self.vplane_c,
            self.vplane_e,
            self.sec_azs_c2,
            self.sec_azs_e2,
            self.cos_aas,
            self.sin_aas,
            &mut self.op0a_a,
            &mut self.op0a_b,
            &mut self.op0a_c,
            &mut self.op0a_d,
        );
        self.op0a_vs = self.op0a_vs.wrapping_add(1);
    }

    // -----------------------------------------------------------------------
    // Op06 — Project object (3D to 2D) — matches snes9x DSP1_Project
    // -----------------------------------------------------------------------
    fn exec_project(&mut self) {
        let x = self.read_word_in(0);
        let y = self.read_word_in(2);
        let z = self.read_word_in(4);

        let (mut px, mut e4) = dsp1_normalize_double(x as i32 - self.gx as i32);
        let (mut py, mut e) = dsp1_normalize_double(y as i32 - self.gy as i32);
        let (mut pz, mut e3) = dsp1_normalize_double(z as i32 - self.gz as i32);

        px >>= 1;
        e4 -= 1;
        py >>= 1;
        e -= 1;
        pz >>= 1;
        e3 -= 1;

        let mut ref_e = if e < e3 { e } else { e3 };
        ref_e = if ref_e < e4 { ref_e } else { e4 };

        px = dsp1_shift_r(px, e4 - ref_e);
        py = dsp1_shift_r(py, e - ref_e);
        pz = dsp1_shift_r(pz, e3 - ref_e);

        let c11 = -((px as i32 * self.nx as i32) >> 15) as i16;
        let c8 = -((py as i32 * self.ny as i32) >> 15) as i16;
        let c9 = -((pz as i32 * self.nz as i32) >> 15) as i16;
        let c12 = (c11 as i32 + c8 as i32 + c9 as i32) as i16;

        let mut aux4 = c12 as i32;
        ref_e = 16 - ref_e;
        if ref_e >= 0 {
            aux4 <<= ref_e;
        } else {
            aux4 >>= -ref_e;
        }
        if aux4 == -1 {
            aux4 = 0;
        }
        aux4 >>= 1;

        let aux = (self.g_les as u16 as i32) + aux4;

        let (c10, mut e2) = dsp1_normalize_double(aux);
        e2 = 15 - e2;

        let (c4, mut e4) = dsp1_inverse(c10, 0);
        let c2 = ((c4 as i32 * self.c_les as i32) >> 15) as i16;

        // H
        let c16 = ((px as i32 * ((self.cos_aas as i32 * 0x7fff) >> 15)) >> 15) as i16;
        let c20 = ((py as i32 * ((self.sin_aas as i32 * 0x7fff) >> 15)) >> 15) as i16;
        let c17 = (c16 as i32 + c20 as i32) as i16;
        let c18 = ((c17 as i32 * c2 as i32) >> 15) as i16;
        let mut e7: i16 = 0;
        let mut c19 = c18;
        dsp1_normalize(&mut c19, &mut e7);
        let h = dsp1_truncate(c19, self.e_les - e2 + ref_e + e7);

        // V
        let c21 =
            ((px as i32 * ((self.cos_azs as i32 * -(self.sin_aas as i32)) >> 15)) >> 15) as i16;
        let c22 = ((py as i32 * ((self.cos_azs as i32 * self.cos_aas as i32) >> 15)) >> 15) as i16;
        let c23 = ((pz as i32 * ((-(self.sin_azs as i32) * 0x7fff) >> 15)) >> 15) as i16;
        let c24 = (c21 as i32 + c22 as i32 + c23 as i32) as i16;
        let c26 = ((c24 as i32 * c2 as i32) >> 15) as i16;
        let mut e6: i16 = 0;
        let mut c25 = c26;
        dsp1_normalize(&mut c25, &mut e6);
        let v = dsp1_truncate(c25, self.e_les - e2 + ref_e + e6);

        // M — e4 from dsp1_inverse is modified by normalize
        let mut c6 = c2;
        dsp1_normalize(&mut c6, &mut e4);
        let m = dsp1_truncate(c6, e4 + self.e_les - e2 - 7);

        if std::env::var_os("TRACE_DSP1").is_some() {
            eprintln!(
                "[DSP1-PROJECT] in=({},{},{}) gx={} gy={} gz={} -> h={} v={} m={}",
                x, y, z, self.gx, self.gy, self.gz, h, v, m
            );
        }

        self.write_word_out(0, h);
        self.write_word_out(2, v);
        self.write_word_out(4, m);
        self.out_count = 6;
    }

    // -----------------------------------------------------------------------
    // Op0E — Target (screen coords to world offsets)
    // -----------------------------------------------------------------------
    fn exec_target(&mut self) {
        let h = self.read_word_in(0) as i32;
        let v = self.read_word_in(2) as i32;

        let sa = self.sin_aas as i32;
        let ca = self.cos_aas as i32;
        let sz = self.sin_azs as i32;

        // Inverse projection
        let x = (((-sa * h) >> 15) + (((ca * v) >> 15) * sz >> 15)) as i16;
        let y = (((ca * h) >> 15) + (((sa * v) >> 15) * sz >> 15)) as i16;

        self.write_word_out(0, x);
        self.write_word_out(2, y);
        self.out_count = 4;
    }

    // -----------------------------------------------------------------------
    // Op0D/1D/2D — Objective: matrix * vector
    // -----------------------------------------------------------------------
    fn exec_objective(&mut self, mat: &[[i16; 3]; 3]) {
        let x = self.read_word_in(0) as i32;
        let y = self.read_word_in(2) as i32;
        let z = self.read_word_in(4) as i32;

        let rx = ((mat[0][0] as i32 * x >> 15)
            + (mat[0][1] as i32 * y >> 15)
            + (mat[0][2] as i32 * z >> 15)) as i16;
        let ry = ((mat[1][0] as i32 * x >> 15)
            + (mat[1][1] as i32 * y >> 15)
            + (mat[1][2] as i32 * z >> 15)) as i16;
        let rz = ((mat[2][0] as i32 * x >> 15)
            + (mat[2][1] as i32 * y >> 15)
            + (mat[2][2] as i32 * z >> 15)) as i16;

        self.write_word_out(0, rx);
        self.write_word_out(2, ry);
        self.write_word_out(4, rz);
        self.out_count = 6;
    }

    // -----------------------------------------------------------------------
    // Op03/13/23 — Subjective: vector * matrix^T
    // -----------------------------------------------------------------------
    fn exec_subjective(&mut self, mat: &[[i16; 3]; 3]) {
        let x = self.read_word_in(0) as i32;
        let y = self.read_word_in(2) as i32;
        let z = self.read_word_in(4) as i32;

        let rx = ((mat[0][0] as i32 * x >> 15)
            + (mat[1][0] as i32 * y >> 15)
            + (mat[2][0] as i32 * z >> 15)) as i16;
        let ry = ((mat[0][1] as i32 * x >> 15)
            + (mat[1][1] as i32 * y >> 15)
            + (mat[2][1] as i32 * z >> 15)) as i16;
        let rz = ((mat[0][2] as i32 * x >> 15)
            + (mat[1][2] as i32 * y >> 15)
            + (mat[2][2] as i32 * z >> 15)) as i16;

        self.write_word_out(0, rx);
        self.write_word_out(2, ry);
        self.write_word_out(4, rz);
        self.out_count = 6;
    }

    // -----------------------------------------------------------------------
    // Op0B/1B/2B — Scalar product (dot product with matrix row 0)
    // -----------------------------------------------------------------------
    fn exec_scalar(&mut self, mat: &[[i16; 3]; 3]) {
        let x = self.read_word_in(0) as i32;
        let y = self.read_word_in(2) as i32;
        let z = self.read_word_in(4) as i32;

        let s = ((mat[0][0] as i32 * x >> 15)
            + (mat[0][1] as i32 * y >> 15)
            + (mat[0][2] as i32 * z >> 15)) as i16;
        self.write_word_out(0, s);
        self.out_count = 2;
    }

    // -----------------------------------------------------------------------
    // Op1C — Polar 3D rotate
    // -----------------------------------------------------------------------
    fn exec_op1c(&mut self) {
        let az = self.read_word_in(0);
        let ay = self.read_word_in(2);
        let ax = self.read_word_in(4);
        let x = self.read_word_in(6) as i32;
        let y = self.read_word_in(8) as i32;
        let z = self.read_word_in(10) as i32;

        let sz = self.dsp_sin(az) as i32;
        let cz = self.dsp_cos(az) as i32;
        let sy = self.dsp_sin(ay) as i32;
        let cy = self.dsp_cos(ay) as i32;
        let sx = self.dsp_sin(ax) as i32;
        let cx = self.dsp_cos(ax) as i32;

        let m00 = (cy * cz) >> 15;
        let m01 = -(cy * sz) >> 15;
        let m02 = sy;
        let m10 = (sx * sy * cz >> 15 >> 15) + (cx * sz >> 15);
        let m11 = (cx * cz >> 15) - (sx * sy * sz >> 15 >> 15);
        let m12 = -(sx * cy) >> 15;
        let m20 = -(cx * sy * cz >> 15 >> 15) + (sx * sz >> 15);
        let m21 = (sx * cz >> 15) + (cx * sy * sz >> 15 >> 15);
        let m22 = (cx * cy) >> 15;

        let rx = ((m00 * x >> 15) + (m01 * y >> 15) + (m02 * z >> 15)) as i16;
        let ry = ((m10 * x >> 15) + (m11 * y >> 15) + (m12 * z >> 15)) as i16;
        let rz = ((m20 * x >> 15) + (m21 * y >> 15) + (m22 * z >> 15)) as i16;

        self.write_word_out(0, rx);
        self.write_word_out(2, ry);
        self.write_word_out(4, rz);
        self.out_count = 6;
    }

    // -----------------------------------------------------------------------
    // Op14 — Gyroscope
    // -----------------------------------------------------------------------
    fn exec_gyrate(&mut self) {
        let az = self.read_word_in(0);
        let ay = self.read_word_in(2);
        let ax = self.read_word_in(4);
        let u = self.read_word_in(6) as i32;
        let f = self.read_word_in(8) as i32;
        let l = self.read_word_in(10) as i32;

        let sz = self.dsp_sin(az) as i32;
        let cz = self.dsp_cos(az) as i32;
        let sy = self.dsp_sin(ay) as i32;
        let cy = self.dsp_cos(ay) as i32;
        let sx = self.dsp_sin(ax) as i32;
        let cx = self.dsp_cos(ax) as i32;

        let nx = ((cy * cz >> 15) * u >> 15) + (sy * f >> 15) + ((cy * -sz >> 15) * l >> 15);
        let ny = (((sx * -sy * cz >> 15 >> 15) + (cx * sz >> 15)) * u >> 15)
            + ((sx * cy >> 15) * f >> 15)
            + (((cx * cz >> 15) - (sx * -sy * sz >> 15 >> 15)) * l >> 15);
        let nz = (((cx * sy * cz >> 15 >> 15) + (sx * sz >> 15)) * u >> 15)
            + ((-cx * cy >> 15) * f >> 15)
            + (((sx * cz >> 15) + (cx * sy * sz >> 15 >> 15)) * l >> 15);

        let ny_f = ny as f64 / 32768.0;
        let nx_f = nx as f64 / 32768.0;
        let nz_f = nz as f64 / 32768.0;

        let new_ay = (ny_f.atan2(nx_f) * 32768.0 / std::f64::consts::PI) as i16;
        let horiz = (nx_f * nx_f + ny_f * ny_f).sqrt();
        let new_ax = (nz_f.atan2(horiz) * 32768.0 / std::f64::consts::PI) as i16;

        self.write_word_out(0, az);
        self.write_word_out(2, new_ay);
        self.write_word_out(4, new_ax);
        self.write_word_out(6, u as i16);
        self.write_word_out(8, f as i16);
        self.write_word_out(10, l as i16);
        self.out_count = 12;
    }
}

// ---------------------------------------------------------------------------
// Command parameter table: (in_count_words, out_count_bytes)
// ---------------------------------------------------------------------------
fn command_params(cmd: u8) -> (usize, usize) {
    match cmd {
        0x00 => (2, 2),                         // Multiply
        0x10 | 0x30 => (2, 4),                  // Inverse
        0x20 => (2, 2),                         // Multiply variant
        0x04 | 0x24 => (2, 4),                  // Sin/Cos
        0x08 => (3, 4),                         // Radius squared
        0x18 | 0x38 => (4, 2),                  // Range
        0x28 => (3, 2),                         // Distance
        0x0C | 0x2C => (3, 4),                  // 2D Rotate
        0x1C | 0x3C => (6, 6),                  // 3D Polar Rotate
        0x02 | 0x12 | 0x22 | 0x32 => (7, 8),    // Parameter
        0x0A | 0x1A | 0x2A | 0x3A => (1, 8),    // Raster
        0x06 | 0x16 | 0x26 | 0x36 => (3, 6),    // Project
        0x0E | 0x1E | 0x2E | 0x3E => (2, 4),    // Target
        0x01 | 0x05 | 0x31 | 0x35 => (4, 0),    // Attitude A
        0x11 | 0x15 => (4, 0),                  // Attitude B
        0x21 | 0x25 => (4, 0),                  // Attitude C
        0x0D | 0x09 | 0x39 | 0x3D => (3, 6),    // Objective A
        0x1D | 0x19 => (3, 6),                  // Objective B
        0x2D | 0x29 => (3, 6),                  // Objective C
        0x03 | 0x33 => (3, 6),                  // Subjective A
        0x13 => (3, 6),                         // Subjective B
        0x23 => (3, 6),                         // Subjective C
        0x0B | 0x3B => (3, 2),                  // Scalar A
        0x1B => (3, 2),                         // Scalar B
        0x2B => (3, 2),                         // Scalar C
        0x14 | 0x34 => (6, 12),                 // Gyroscope
        0x07 | 0x0F => (1, 2),                  // Memory test
        0x27 | 0x2F => (1, 2),                  // Size query
        0x1F | 0x17 | 0x37 | 0x3F => (1, 2048), // ROM dump
        0x80 => (0, 0),                         // NOP/Reset
        _ => (0, 0),
    }
}

// ---------------------------------------------------------------------------
// DSP-1 math helpers — matches snes9x semantics
// ---------------------------------------------------------------------------

/// Normalize: shift coefficient left until bit 14 is set, decrementing exponent.
/// Matches snes9x DSP1_Normalize(m, &C, &E) — modifies C and E in-place.
fn dsp1_normalize(c: &mut i16, e: &mut i16) {
    let m = *c;
    if m == 0 {
        // No normalization for zero
        return;
    }

    // Count leading redundant bits (sign extension)
    let mut shift: i16 = 0;
    if m < 0 {
        let mut test = m as u16;
        // Count leading 1-bits (after sign bit)
        while test & 0x4000 != 0 && shift < 15 {
            test <<= 1;
            shift += 1;
        }
    } else {
        let mut test = m as u16;
        // Count leading 0-bits (after sign bit)
        while test & 0x4000 == 0 && shift < 15 {
            test <<= 1;
            shift += 1;
        }
    }

    if shift > 0 {
        *c = ((*c as i32) << shift) as i16;
    }
    *e -= shift;
}

/// NormalizeDouble: normalize a 32-bit product into (coefficient, exponent).
/// Matches snes9x DSP1_NormalizeDouble exactly.
fn dsp1_normalize_double(product: i32) -> (i16, i16) {
    let n = (product & 0x7fff) as i16;
    let m = (product >> 15) as i16;
    let mut e: i16 = 0;
    let mut i: i16 = 0x4000;

    if m < 0 {
        while (m & i) != 0 && i != 0 {
            i >>= 1;
            e += 1;
        }
    } else {
        while (m & i) == 0 && i != 0 {
            i >>= 1;
            e += 1;
        }
    }

    let coefficient;
    if e > 0 {
        // DSP1ROM[0x0021 + e] = 1 << (e - 1)
        let shift_left = 1i32 << (e - 1);
        let mut c = ((m as i32 * shift_left) << 1) as i16;

        if e < 15 {
            // DSP1ROM[0x0040 - e] = 1 << (14 - e)
            let shift_right = 1i32 << (14 - e);
            c = (c as i32 + ((n as i32 * shift_right) >> 15)) as i16;
        } else {
            // e == 15: normalize n
            i = 0x4000;
            if m < 0 {
                while (n & i) != 0 && i != 0 {
                    i >>= 1;
                    e += 1;
                }
            } else {
                while (n & i) == 0 && i != 0 {
                    i >>= 1;
                    e += 1;
                }
            }
            if e > 15 {
                // DSP1ROM[0x0012 + e] = 1 << (e - 16)
                let shift_n = 1i32 << (e - 16);
                c = ((n as i32 * shift_n) << 1) as i16;
            } else {
                c = (c as i32 + n as i32) as i16;
            }
        }
        coefficient = c;
    } else {
        coefficient = m;
    }
    (coefficient, e)
}

/// ShiftR: right-shift a coefficient by E positions.
/// Matches snes9x DSP1_ShiftR: C * DSP1ROM[0x0031 + E] >> 15.
fn dsp1_shift_r(c: i16, e: i16) -> i16 {
    if e <= 0 {
        // DSP1ROM[0x0031] = 0x7FFF
        ((c as i32 * 0x7FFF) >> 15) as i16
    } else if e >= 15 {
        0
    } else {
        // DSP1ROM[0x0031 + e] = 32768 >> e = 0x4000 >> (e - 1)
        let multiplier = 0x4000i32 >> (e - 1);
        ((c as i32 * multiplier) >> 15) as i16
    }
}

/// Truncate coefficient + exponent to a plain 16-bit value.
/// Matches snes9x DSP1_Truncate exactly.
fn dsp1_truncate(c: i16, e: i16) -> i16 {
    if e > 0 {
        if c > 0 {
            return 32767;
        } else if c < 0 {
            return -32767;
        }
    } else if e < 0 {
        // Right-shift by |e|
        let shift = (-e) as u32;
        if shift >= 16 {
            return 0;
        }
        return ((c as i32) >> shift) as i16;
    }
    c
}

/// Inverse: compute 1/(C * 2^E), returning (iC, iE).
/// Matches snes9x DSP1_Inverse exactly using Newton's method with ROM lookup.
fn dsp1_inverse(coefficient: i16, exponent: i16) -> (i16, i16) {
    if coefficient == 0 {
        return (0x7FFF, 0x002F);
    }

    let sign: i32 = if coefficient < 0 { -1 } else { 1 };
    let mut c = (coefficient as i32).abs();
    if c > 32767 {
        c = 32767;
    }
    let mut e = exponent;

    // Normalize c to [0x4000, 0x7FFF]
    while c < 0x4000 {
        c <<= 1;
        e -= 1;
    }

    // Special case: exact power of 2
    if c == 0x4000 {
        if sign > 0 {
            return (0x7FFF, 1 - e);
        } else {
            // snes9x: iCoefficient = -0x4000, Exponent--, then iExponent = 1 - Exponent
            return (-0x4000, 2 - e);
        }
    }

    // Newton's method with DSP1ROM initial guess, matching snes9x exactly:
    //   i = DSP1ROM[((Coefficient - 0x4000) >> 7) + 0x0065]
    //   i = (i + (-i * (Coefficient * i >> 15) >> 15)) << 1  // iteration 1
    //   i = (i + (-i * (Coefficient * i >> 15) >> 15)) << 1  // iteration 2
    let idx = ((c - 0x4000) >> 7) as usize;
    let mut i = DSP1_INV_TABLE[idx] as i32;
    // Newton iteration 1
    i = (i + ((-i * (c * i >> 15)) >> 15)) << 1;
    // Newton iteration 2
    i = (i + ((-i * (c * i >> 15)) >> 15)) << 1;

    let ic = (i * sign) as i16;
    (ic, 1 - e)
}

/// Raster: compute per-scanline Mode 7 A/B/C/D coefficients.
/// Matches snes9x DSP1_Raster.
#[allow(clippy::too_many_arguments)]
fn dsp1_raster(
    vs: i16,
    sin_azs: i16,
    v_offset: i16,
    vplane_c: i16,
    vplane_e: i16,
    sec_azs_c2: i16,
    sec_azs_e2: i16,
    cos_aas: i16,
    sin_aas: i16,
    an: &mut i16,
    bn: &mut i16,
    cn: &mut i16,
    dn: &mut i16,
) {
    // Inverse of scanline depth
    let depth_val = ((vs as i32 * sin_azs as i32) >> 15) + v_offset as i32;

    let (mut c, mut e) = dsp1_inverse(depth_val as i16, 7);
    e += vplane_e;

    let c1 = (c as i32 * vplane_c as i32 >> 15) as i16;
    let e1 = e + sec_azs_e2;

    // An, Cn: normalize C1 with exponent E, truncate
    let mut nc = c1;
    let mut ne = e;
    dsp1_normalize(&mut nc, &mut ne);
    c = dsp1_truncate(nc, ne);

    *an = (c as i32 * cos_aas as i32 >> 15) as i16;
    *cn = (c as i32 * sin_aas as i32 >> 15) as i16;

    // Bn, Dn: apply secant correction, normalize with E1, truncate
    let c1_sec = (c1 as i32 * sec_azs_c2 as i32 >> 15) as i16;
    let mut nc2 = c1_sec;
    let mut ne2 = e1;
    dsp1_normalize(&mut nc2, &mut ne2);
    c = dsp1_truncate(nc2, ne2);

    *bn = (c as i32 * -(sin_aas as i32) >> 15) as i16;
    *dn = (c as i32 * cos_aas as i32 >> 15) as i16;
}

// ---------------------------------------------------------------------------
// Sin/Cos tables and lookup — matches snes9x exactly
// ---------------------------------------------------------------------------

/// 256-entry sin table from snes9x. Index i corresponds to sin(i * 360°/256).
/// The table covers the full circle in 256 steps.
static DSP1_SIN_TABLE: [i16; 256] = [
    0x0000, 0x0324, 0x0647, 0x096a, 0x0c8b, 0x0fab, 0x12c8, 0x15e2, 0x18f8, 0x1c0b, 0x1f19, 0x2223,
    0x2528, 0x2826, 0x2b1f, 0x2e11, 0x30fb, 0x33de, 0x36ba, 0x398c, 0x3c56, 0x3f17, 0x41ce, 0x447a,
    0x471c, 0x49b4, 0x4c3f, 0x4ebf, 0x5133, 0x539b, 0x55f5, 0x5842, 0x5a82, 0x5cb4, 0x5ed7, 0x60ec,
    0x62f2, 0x64e8, 0x66cf, 0x68a6, 0x6a6d, 0x6c24, 0x6dca, 0x6f5f, 0x70e2, 0x7255, 0x73b5, 0x7504,
    0x7641, 0x776c, 0x7884, 0x798a, 0x7a7d, 0x7b5d, 0x7c29, 0x7ce3, 0x7d8a, 0x7e1d, 0x7e9d, 0x7f09,
    0x7f62, 0x7fa7, 0x7fd8, 0x7ff6, 0x7fff, 0x7ff6, 0x7fd8, 0x7fa7, 0x7f62, 0x7f09, 0x7e9d, 0x7e1d,
    0x7d8a, 0x7ce3, 0x7c29, 0x7b5d, 0x7a7d, 0x798a, 0x7884, 0x776c, 0x7641, 0x7504, 0x73b5, 0x7255,
    0x70e2, 0x6f5f, 0x6dca, 0x6c24, 0x6a6d, 0x68a6, 0x66cf, 0x64e8, 0x62f2, 0x60ec, 0x5ed7, 0x5cb4,
    0x5a82, 0x5842, 0x55f5, 0x539b, 0x5133, 0x4ebf, 0x4c3f, 0x49b4, 0x471c, 0x447a, 0x41ce, 0x3f17,
    0x3c56, 0x398c, 0x36ba, 0x33de, 0x30fb, 0x2e11, 0x2b1f, 0x2826, 0x2528, 0x2223, 0x1f19, 0x1c0b,
    0x18f8, 0x15e2, 0x12c8, 0x0fab, 0x0c8b, 0x096a, 0x0647, 0x0324, -0x0000, -0x0324, -0x0647,
    -0x096a, -0x0c8b, -0x0fab, -0x12c8, -0x15e2, -0x18f8, -0x1c0b, -0x1f19, -0x2223, -0x2528,
    -0x2826, -0x2b1f, -0x2e11, -0x30fb, -0x33de, -0x36ba, -0x398c, -0x3c56, -0x3f17, -0x41ce,
    -0x447a, -0x471c, -0x49b4, -0x4c3f, -0x4ebf, -0x5133, -0x539b, -0x55f5, -0x5842, -0x5a82,
    -0x5cb4, -0x5ed7, -0x60ec, -0x62f2, -0x64e8, -0x66cf, -0x68a6, -0x6a6d, -0x6c24, -0x6dca,
    -0x6f5f, -0x70e2, -0x7255, -0x73b5, -0x7504, -0x7641, -0x776c, -0x7884, -0x798a, -0x7a7d,
    -0x7b5d, -0x7c29, -0x7ce3, -0x7d8a, -0x7e1d, -0x7e9d, -0x7f09, -0x7f62, -0x7fa7, -0x7fd8,
    -0x7ff6, -0x7fff, -0x7ff6, -0x7fd8, -0x7fa7, -0x7f62, -0x7f09, -0x7e9d, -0x7e1d, -0x7d8a,
    -0x7ce3, -0x7c29, -0x7b5d, -0x7a7d, -0x798a, -0x7884, -0x776c, -0x7641, -0x7504, -0x73b5,
    -0x7255, -0x70e2, -0x6f5f, -0x6dca, -0x6c24, -0x6a6d, -0x68a6, -0x66cf, -0x64e8, -0x62f2,
    -0x60ec, -0x5ed7, -0x5cb4, -0x5a82, -0x5842, -0x55f5, -0x539b, -0x5133, -0x4ebf, -0x4c3f,
    -0x49b4, -0x471c, -0x447a, -0x41ce, -0x3f17, -0x3c56, -0x398c, -0x36ba, -0x33de, -0x30fb,
    -0x2e11, -0x2b1f, -0x2826, -0x2528, -0x2223, -0x1f19, -0x1c0b, -0x18f8, -0x15e2, -0x12c8,
    -0x0fab, -0x0c8b, -0x096a, -0x0647, -0x0324,
];

/// Interpolation table for sub-sample sin/cos lookup.
static DSP1_MUL_TABLE: [i16; 256] = [
    0x0000, 0x0003, 0x0006, 0x0009, 0x000c, 0x000f, 0x0012, 0x0015, 0x0019, 0x001c, 0x001f, 0x0022,
    0x0025, 0x0028, 0x002b, 0x002f, 0x0032, 0x0035, 0x0038, 0x003b, 0x003e, 0x0041, 0x0045, 0x0048,
    0x004b, 0x004e, 0x0051, 0x0054, 0x0057, 0x005b, 0x005e, 0x0061, 0x0064, 0x0067, 0x006a, 0x006d,
    0x0071, 0x0074, 0x0077, 0x007a, 0x007d, 0x0080, 0x0083, 0x0087, 0x008a, 0x008d, 0x0090, 0x0093,
    0x0096, 0x0099, 0x009d, 0x00a0, 0x00a3, 0x00a6, 0x00a9, 0x00ac, 0x00af, 0x00b3, 0x00b6, 0x00b9,
    0x00bc, 0x00bf, 0x00c2, 0x00c5, 0x00c9, 0x00cc, 0x00cf, 0x00d2, 0x00d5, 0x00d8, 0x00db, 0x00df,
    0x00e2, 0x00e5, 0x00e8, 0x00eb, 0x00ee, 0x00f1, 0x00f5, 0x00f8, 0x00fb, 0x00fe, 0x0101, 0x0104,
    0x0107, 0x010b, 0x010e, 0x0111, 0x0114, 0x0117, 0x011a, 0x011d, 0x0121, 0x0124, 0x0127, 0x012a,
    0x012d, 0x0130, 0x0133, 0x0137, 0x013a, 0x013d, 0x0140, 0x0143, 0x0146, 0x0149, 0x014d, 0x0150,
    0x0153, 0x0156, 0x0159, 0x015c, 0x015f, 0x0163, 0x0166, 0x0169, 0x016c, 0x016f, 0x0172, 0x0175,
    0x0178, 0x017c, 0x017f, 0x0182, 0x0185, 0x0188, 0x018b, 0x018e, 0x0192, 0x0195, 0x0198, 0x019b,
    0x019e, 0x01a1, 0x01a4, 0x01a8, 0x01ab, 0x01ae, 0x01b1, 0x01b4, 0x01b7, 0x01ba, 0x01be, 0x01c1,
    0x01c4, 0x01c7, 0x01ca, 0x01cd, 0x01d0, 0x01d4, 0x01d7, 0x01da, 0x01dd, 0x01e0, 0x01e3, 0x01e6,
    0x01ea, 0x01ed, 0x01f0, 0x01f3, 0x01f6, 0x01f9, 0x01fc, 0x0200, 0x0203, 0x0206, 0x0209, 0x020c,
    0x020f, 0x0212, 0x0216, 0x0219, 0x021c, 0x021f, 0x0222, 0x0225, 0x0228, 0x022c, 0x022f, 0x0232,
    0x0235, 0x0238, 0x023b, 0x023e, 0x0242, 0x0245, 0x0248, 0x024b, 0x024e, 0x0251, 0x0254, 0x0258,
    0x025b, 0x025e, 0x0261, 0x0264, 0x0267, 0x026a, 0x026e, 0x0271, 0x0274, 0x0277, 0x027a, 0x027d,
    0x0280, 0x0284, 0x0287, 0x028a, 0x028d, 0x0290, 0x0293, 0x0296, 0x029a, 0x029d, 0x02a0, 0x02a3,
    0x02a6, 0x02a9, 0x02ac, 0x02b0, 0x02b3, 0x02b6, 0x02b9, 0x02bc, 0x02bf, 0x02c2, 0x02c6, 0x02c9,
    0x02cc, 0x02cf, 0x02d2, 0x02d5, 0x02d8, 0x02db, 0x02df, 0x02e2, 0x02e5, 0x02e8, 0x02eb, 0x02ee,
    0x02f1, 0x02f5, 0x02f8, 0x02fb, 0x02fe, 0x0301, 0x0304, 0x0307, 0x030b, 0x030e, 0x0311, 0x0314,
    0x0317, 0x031a, 0x031d, 0x0321,
];

/// Inverse lookup table from snes9x DSP1ROM (128 entries at offset 0x0065).
/// Used by Newton's method for computing 1/x.
static DSP1_INV_TABLE: [i16; 128] = [
    0x7fff, 0x7f02, 0x7e08, 0x7d12, 0x7c1f, 0x7b30, 0x7a45, 0x795d, 0x7878, 0x7797, 0x76ba, 0x75df,
    0x7507, 0x7433, 0x7361, 0x7293, 0x71c7, 0x70fe, 0x7038, 0x6f75, 0x6eb4, 0x6df6, 0x6d3a, 0x6c81,
    0x6bca, 0x6b16, 0x6a64, 0x69b4, 0x6907, 0x685b, 0x67b2, 0x670b, 0x6666, 0x65c4, 0x6523, 0x6484,
    0x63e7, 0x634c, 0x62b3, 0x621c, 0x6186, 0x60f2, 0x6060, 0x5fd0, 0x5f41, 0x5eb5, 0x5e29, 0x5d9f,
    0x5d17, 0x5c91, 0x5c0c, 0x5b88, 0x5b06, 0x5a85, 0x5a06, 0x5988, 0x590b, 0x5890, 0x5816, 0x579d,
    0x5726, 0x56b0, 0x563b, 0x55c8, 0x5555, 0x54e4, 0x5474, 0x5405, 0x5398, 0x532b, 0x52bf, 0x5255,
    0x51ec, 0x5183, 0x511c, 0x50b6, 0x5050, 0x4fec, 0x4f89, 0x4f26, 0x4ec5, 0x4e64, 0x4e05, 0x4da6,
    0x4d48, 0x4cec, 0x4c90, 0x4c34, 0x4bda, 0x4b81, 0x4b28, 0x4ad0, 0x4a79, 0x4a23, 0x49cd, 0x4979,
    0x4925, 0x48d1, 0x487f, 0x482d, 0x47dc, 0x478c, 0x473c, 0x46ed, 0x469f, 0x4651, 0x4604, 0x45b8,
    0x456c, 0x4521, 0x44d7, 0x448d, 0x4444, 0x43fc, 0x43b4, 0x436d, 0x4326, 0x42e0, 0x429a, 0x4255,
    0x4211, 0x41cd, 0x4189, 0x4146, 0x4104, 0x40c2, 0x4081, 0x4040,
];

/// Sin lookup matching snes9x DSP1_Sin exactly, with MulTable interpolation.
fn dsp1_sin(angle: i16) -> i16 {
    if angle < 0 {
        if angle == -32768 {
            return 0;
        }
        return -dsp1_sin(-angle);
    }
    let idx = (angle >> 8) as usize;
    let frac = (angle & 0xFF) as usize;
    let s = DSP1_SIN_TABLE[idx] as i32
        + (DSP1_MUL_TABLE[frac] as i32 * DSP1_SIN_TABLE[0x40 + idx] as i32 >> 15);
    s.min(32767) as i16
}

/// Cos lookup matching snes9x DSP1_Cos exactly.
fn dsp1_cos(angle: i16) -> i16 {
    let a = if angle < 0 {
        if angle == -32768 {
            return -32768i16;
        }
        -angle
    } else {
        angle
    };
    let idx = (a >> 8) as usize;
    let frac = (a & 0xFF) as usize;
    let s = DSP1_SIN_TABLE[0x40 + idx] as i32
        - (DSP1_MUL_TABLE[frac] as i32 * DSP1_SIN_TABLE[idx] as i32 >> 15);
    s.max(-32768) as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rom_dump_command_can_stream_full_declared_output() {
        let mut dsp = Dsp1::new(0x100000);

        dsp.write_dr(0x1f);
        dsp.write_dr(0x00);
        dsp.write_dr(0x00);

        assert_eq!(dsp.read_sr(), 0x80);
        for _ in 0..DSP1_OUTPUT_BUFFER_LEN {
            assert_eq!(dsp.read_dr(), 0x00);
        }
        assert_eq!(dsp.read_sr(), 0xc0);
    }
}
