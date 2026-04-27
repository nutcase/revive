use super::super::apu::Apu;
use super::super::consts::{Spc, REG_LEN};
use super::audio_dump::AudioDump;
use super::dsp_helpers;
use super::filter::Filter;
use super::ring_buffer::RingBuffer;
use super::voice::{ResamplingMode, Voice};

pub const SAMPLE_RATE: usize = 32000;
pub const BUFFER_LEN: usize = SAMPLE_RATE * 2;

const NUM_VOICES: usize = 8;

const REG_READONLY_MASK: u8 = 0x80;
const VOICE_INDEX_SHIFT: u8 = 4;
const VOICE_REG_MASK: u8 = 0x0f;
const VOICE_REG_WRITABLE_END: u8 = 0x0a;
const VOICE_REG_VOL_L: u8 = 0x00;
const VOICE_REG_VOL_R: u8 = 0x01;
const VOICE_REG_PITCH_L: u8 = 0x02;
const VOICE_REG_PITCH_H: u8 = 0x03;
const VOICE_REG_SOURCE: u8 = 0x04;
const VOICE_REG_ADSR0: u8 = 0x05;
const VOICE_REG_ADSR1: u8 = 0x06;
const VOICE_REG_GAIN: u8 = 0x07;
const VOICE_REG_ENVX: u8 = 0x08;
const VOICE_REG_OUTX: u8 = 0x09;
const VOICE_REG_FIR_COEF: u8 = 0x0f;

const REG_MVOL_L: u8 = 0x0c;
const REG_MVOL_R: u8 = 0x1c;
const REG_EVOL_L: u8 = 0x2c;
const REG_EVOL_R: u8 = 0x3c;
const REG_KON: u8 = 0x4c;
const REG_KOF: u8 = 0x5c;
const REG_FLG: u8 = 0x6c;
const REG_EFB: u8 = 0x0d;
const REG_PMON: u8 = 0x2d;
const REG_NON: u8 = 0x3d;
const REG_EON: u8 = 0x4d;
const REG_DIR: u8 = 0x5d;
const REG_ESA: u8 = 0x6d;
const REG_EDL: u8 = 0x7d;
const REG_ENDX: u8 = 0x7c;
const REG_KON_USIZE: usize = REG_KON as usize;
const REG_KOF_USIZE: usize = REG_KOF as usize;

const FLG_RESET: u8 = 0x80;
const FLG_MUTE: u8 = 0x40;
const FLG_ECHO_DISABLE: u8 = 0x20;
const FLG_NOISE_CLOCK_MASK: u8 = 0x1f;
const EDL_MASK: u8 = 0x0f;

const COUNTER_RANGE: i32 = 30720;
static COUNTER_RATES: [i32; 32] = [
    COUNTER_RANGE + 1, // Never fires
    2048,
    1536,
    1280,
    1024,
    768,
    640,
    512,
    384,
    320,
    256,
    192,
    160,
    128,
    96,
    80,
    64,
    48,
    40,
    32,
    24,
    20,
    16,
    12,
    10,
    8,
    6,
    5,
    4,
    3,
    2,
    1,
];

static COUNTER_OFFSETS: [i32; 32] = [
    1, 0, 1040, 536, 0, 1040, 536, 0, 1040, 536, 0, 1040, 536, 0, 1040, 536, 0, 1040, 536, 0, 1040,
    536, 0, 1040, 536, 0, 1040, 536, 0, 1040, 0, 0,
];

#[derive(Clone, Copy)]
struct VoiceMix {
    left: i32,
    right: i32,
    echo_left: i32,
    echo_right: i32,
}

#[derive(Clone, Copy)]
struct EchoInput {
    address: u32,
    left: i32,
    right: i32,
}

pub struct Dsp {
    emulator: *mut Apu,

    pub voices: Vec<Box<Voice>>,

    left_filter: Box<Filter>,
    right_filter: Box<Filter>,
    pub output_buffer: Box<RingBuffer>,

    vol_left: u8,
    vol_right: u8,
    echo_vol_left: u8,
    echo_vol_right: u8,
    noise_clock: u8,
    pub echo_write_enabled: bool,
    flg: u8,
    echo_feedback: u8,
    source_dir: u8,
    echo_start_address: u16,
    echo_delay: u8,
    kon: u8,
    kof: u8,
    pmon: u8,
    non: u8,
    eon: u8,
    endx: u8,

    counter: i32,

    cycles_since_last_flush: i32,
    pub is_flushing: bool,
    noise: i32,
    echo_pos: i32,
    echo_length: i32,

    resampling_mode: ResamplingMode,
    output_filter_enabled: bool,
    output_filter_left: i32,
    output_filter_right: i32,
    audio_dump: AudioDump,
}

impl Dsp {
    pub fn new(emulator: *mut Apu) -> Box<Dsp> {
        let resampling_mode = ResamplingMode::Gaussian;
        let mut ret = Box::new(Dsp {
            emulator: emulator,

            voices: Vec::with_capacity(NUM_VOICES),

            left_filter: Box::new(Filter::new()),
            right_filter: Box::new(Filter::new()),
            output_buffer: Box::new(RingBuffer::new()),

            vol_left: 0,
            vol_right: 0,
            echo_vol_left: 0,
            echo_vol_right: 0,
            noise_clock: 0,
            echo_write_enabled: false,
            flg: 0,
            echo_feedback: 0,
            source_dir: 0,
            echo_start_address: 0,
            echo_delay: 0,
            kon: 0,
            kof: 0,
            pmon: 0,
            non: 0,
            eon: 0,
            endx: 0,

            counter: 0,

            cycles_since_last_flush: 0,
            is_flushing: false,
            noise: 0,
            echo_pos: 0,
            echo_length: 0,

            resampling_mode: resampling_mode,
            output_filter_enabled: std::env::var("AUDIO_DSP_LOWPASS")
                .map(|v| v != "0" && !v.eq_ignore_ascii_case("false"))
                .unwrap_or(true),
            output_filter_left: 0,
            output_filter_right: 0,
            audio_dump: AudioDump::new_from_env(),
        });
        let ret_ptr = &mut *ret as *mut _;
        for i in 0..NUM_VOICES {
            ret.voices.push(Box::new(Voice::new(
                ret_ptr,
                emulator,
                resampling_mode,
                i as u8,
            )));
        }
        ret.reset();
        ret
    }

    #[inline]
    fn emulator(&self) -> &mut Apu {
        unsafe { &mut (*self.emulator) }
    }

    fn set_filter_coefficient(&mut self, index: i32, value: u8) {
        self.left_filter.coefficients[index as usize] = value;
        self.right_filter.coefficients[index as usize] = value;
    }

    pub fn reset(&mut self) {
        for voice in self.voices.iter_mut() {
            voice.reset();
        }

        self.left_filter.reset();
        self.right_filter.reset();
        self.output_buffer.reset();

        self.vol_left = 0;
        self.vol_right = 0;
        self.echo_vol_left = 0;
        self.echo_vol_right = 0;
        self.noise_clock = 0;
        self.echo_write_enabled = false;
        // Power-on state: reset+mute+echo disabled (matches SNES DSP reset behavior).
        self.flg = 0xE0;
        self.echo_feedback = 0;
        self.source_dir = 0;
        self.echo_start_address = 0;
        self.echo_delay = 0;
        self.kon = 0;
        self.kof = 0;
        self.pmon = 0;
        self.non = 0;
        self.eon = 0;
        self.endx = 0;

        self.counter = 0;

        self.cycles_since_last_flush = 0;
        self.is_flushing = false;
        self.noise = 0x4000;
        self.echo_pos = 0;
        self.echo_length = 0;

        self.set_resampling_mode(ResamplingMode::Gaussian);
        self.output_filter_left = 0;
        self.output_filter_right = 0;
    }

    fn filter_final_sample(&mut self, left: i16, right: i16) -> (i16, i16) {
        if !self.output_filter_enabled {
            return (left, right);
        }

        const SCALE_SHIFT: i32 = 8;
        const ALPHA_Q8: i32 = 224; // 0.875, a light SNES-style output low-pass.

        let left_target = (left as i32) << SCALE_SHIFT;
        let right_target = (right as i32) << SCALE_SHIFT;
        self.output_filter_left =
            advance_output_filter(self.output_filter_left, left_target, ALPHA_Q8);
        self.output_filter_right =
            advance_output_filter(self.output_filter_right, right_target, ALPHA_Q8);

        (
            dsp_helpers::clamp(self.output_filter_left >> SCALE_SHIFT) as i16,
            dsp_helpers::clamp(self.output_filter_right >> SCALE_SHIFT) as i16,
        )
    }

    fn finish_sample_tick(&mut self) {
        self.counter = (self.counter + 1) % COUNTER_RANGE;
        self.cycles_since_last_flush -= 64;
    }

    fn tick_noise(&mut self) {
        if !self.read_counter(self.noise_clock as i32) {
            let feedback = (self.noise << 13) ^ (self.noise << 14);
            self.noise = (feedback & 0x4000) ^ (self.noise >> 1);
        }
    }

    fn mix_voices(&mut self) -> VoiceMix {
        let are_any_voices_solod = self.voices.iter().any(|voice| voice.is_solod);
        let mut mix = VoiceMix {
            left: 0,
            right: 0,
            echo_left: 0,
            echo_right: 0,
        };
        let mut last_voice_out = 0;
        let audio_dump = &mut self.audio_dump;

        for (vi, voice) in self.voices.iter_mut().enumerate() {
            let output = voice.render_sample(last_voice_out, self.noise, are_any_voices_solod);

            if vi == 7 {
                audio_dump.push_voice7(output.left_out, output.right_out);
            }

            mix.left = dsp_helpers::clamp(mix.left + output.left_out);
            mix.right = dsp_helpers::clamp(mix.right + output.right_out);

            if voice.echo_on {
                mix.echo_left = dsp_helpers::clamp(mix.echo_left + output.left_out);
                mix.echo_right = dsp_helpers::clamp(mix.echo_right + output.right_out);
            }

            last_voice_out = output.last_voice_out;
        }

        mix
    }

    fn read_echo_input(&mut self) -> EchoInput {
        let address = echo_address(self.echo_start_address, self.echo_pos);
        let left = self.read_echo_word(address);
        let right = self.read_echo_word(address + 2);

        EchoInput {
            address,
            left: dsp_helpers::clamp(self.left_filter.next(left)),
            right: dsp_helpers::clamp(self.right_filter.next(right)),
        }
    }

    fn read_echo_word(&mut self, address: u32) -> i32 {
        let hi = self.emulator().read_u8(address + 1) as u16;
        let lo = self.emulator().read_u8(address) as u16;
        ((((hi << 8) | lo) as i16) & !1) as i32
    }

    fn mix_final_output(
        &mut self,
        dry_left: i32,
        dry_right: i32,
        echo_input: EchoInput,
        muted: bool,
    ) -> (i16, i16) {
        let mut left = dsp_helpers::clamp(
            dry_left + dsp_helpers::multiply_volume(echo_input.left, self.echo_vol_left),
        ) as i16;
        let mut right = dsp_helpers::clamp(
            dry_right + dsp_helpers::multiply_volume(echo_input.right, self.echo_vol_right),
        ) as i16;

        if muted {
            left = 0;
            right = 0;
            self.output_filter_left = 0;
            self.output_filter_right = 0;
        } else {
            (left, right) = self.filter_final_sample(left, right);
        }

        (left, right)
    }

    fn write_echo_output(&mut self, echo_input: EchoInput, voice_mix: VoiceMix) {
        if !self.echo_write_enabled {
            return;
        }

        let left = self.echo_feedback_sample(voice_mix.echo_left, echo_input.left);
        let right = self.echo_feedback_sample(voice_mix.echo_right, echo_input.right);

        self.emulator().write_u8(echo_input.address, left as u8);
        self.emulator()
            .write_u8(echo_input.address + 1, (left >> 8) as u8);
        self.emulator()
            .write_u8(echo_input.address + 2, right as u8);
        self.emulator()
            .write_u8(echo_input.address + 3, (right >> 8) as u8);
    }

    fn echo_feedback_sample(&self, voice_echo: i32, echo_input: i32) -> i32 {
        dsp_helpers::clamp(
            voice_echo
                + ((((echo_input * ((self.echo_feedback as i8) as i32)) >> 7) as i16) as i32),
        ) & !1
    }

    fn advance_echo_position(&mut self) {
        if self.echo_pos == 0 {
            self.echo_length = self.calculate_echo_length();
        }
        self.echo_pos += 4;
        if self.echo_pos >= self.echo_length {
            self.echo_pos = 0;
        }
    }

    pub(crate) fn set_endx_bit(&mut self, voice_index: u8) {
        self.endx |= 1u8 << (voice_index & 7);
    }

    pub fn resampling_mode(&self) -> ResamplingMode {
        self.resampling_mode
    }

    pub fn set_resampling_mode(&mut self, resampling_mode: ResamplingMode) {
        self.resampling_mode = resampling_mode;
        for voice in self.voices.iter_mut() {
            voice.resampling_mode = resampling_mode;
        }
    }

    pub fn set_state(&mut self, spc: &Spc) {
        for i in 0..REG_LEN {
            match i {
                REG_KON_USIZE | REG_KOF_USIZE => (), // Do nothing
                _ => {
                    self.set_register(i as u8, spc.regs[i as usize]);
                }
            }
        }

        self.set_kon(spc.regs[REG_KON as usize]);
    }

    pub fn set_state_from_regs(&mut self, regs: &[u8; REG_LEN]) {
        for i in 0..REG_LEN {
            match i {
                REG_KON_USIZE | REG_KOF_USIZE => (), // Do nothing
                _ => {
                    self.set_register(i as u8, regs[i as usize]);
                }
            }
        }

        self.set_kon(regs[REG_KON as usize]);
    }

    pub fn cycles_callback(&mut self, num_cycles: i32) {
        self.cycles_since_last_flush += num_cycles;
    }

    pub fn get_echo_start_address(&self) -> u16 {
        self.echo_start_address
    }

    pub fn calculate_echo_length(&self) -> i32 {
        (self.echo_delay as i32) * 0x800
    }

    pub fn flush(&mut self) {
        self.is_flushing = true;

        while self.cycles_since_last_flush >= 64 {
            let reset = (self.flg & FLG_RESET) != 0;
            let muted = (self.flg & FLG_MUTE) != 0 || reset;
            if reset {
                // When DSP reset is asserted, output silence and avoid advancing voices/echo.
                self.output_buffer.write_sample(0, 0);
                self.finish_sample_tick();
                continue;
            }

            self.tick_noise();

            let voice_mix = self.mix_voices();
            let dry_left = dsp_helpers::multiply_volume(voice_mix.left, self.vol_left);
            let dry_right = dsp_helpers::multiply_volume(voice_mix.right, self.vol_right);
            self.audio_dump.push_dry(dry_left, dry_right);

            let echo_input = self.read_echo_input();
            self.audio_dump.push_echo(
                dsp_helpers::multiply_volume(echo_input.left, self.echo_vol_left),
                dsp_helpers::multiply_volume(echo_input.right, self.echo_vol_right),
            );

            let (left_out, right_out) =
                self.mix_final_output(dry_left, dry_right, echo_input, muted);
            self.output_buffer.write_sample(left_out, right_out);
            self.audio_dump.push_main(left_out, right_out);

            self.write_echo_output(echo_input, voice_mix);
            self.advance_echo_position();
            self.finish_sample_tick();
        }

        self.is_flushing = false;
    }

    pub fn set_register(&mut self, address: u8, value: u8) {
        if (address & REG_READONLY_MASK) != 0 {
            return;
        }

        if !self.is_flushing {
            self.flush();
        }

        let voice_index = address >> VOICE_INDEX_SHIFT;
        let voice_address = address & VOICE_REG_MASK;
        if voice_address < VOICE_REG_WRITABLE_END {
            if voice_address < VOICE_REG_ENVX {
                let voice = &mut self.voices[voice_index as usize];
                match voice_address {
                    VOICE_REG_VOL_L => {
                        voice.vol_left = value;
                    }
                    VOICE_REG_VOL_R => {
                        voice.vol_right = value;
                    }
                    VOICE_REG_PITCH_L => {
                        voice.pitch_low = value;
                    }
                    VOICE_REG_PITCH_H => {
                        voice.set_pitch_high(value);
                    }
                    VOICE_REG_SOURCE => {
                        voice.source = value;
                    }
                    VOICE_REG_ADSR0 => {
                        voice.envelope.adsr0 = value;
                    }
                    VOICE_REG_ADSR1 => {
                        voice.envelope.adsr1 = value;
                    }
                    VOICE_REG_GAIN => {
                        voice.envelope.gain = value;
                    }
                    _ => (), // Do nothing
                }
            }
        } else if voice_address == VOICE_REG_FIR_COEF {
            self.set_filter_coefficient(voice_index as i32, value);
        } else {
            match address {
                REG_MVOL_L => {
                    self.vol_left = value;
                }
                REG_MVOL_R => {
                    self.vol_right = value;
                }
                REG_EVOL_L => {
                    self.echo_vol_left = value;
                }
                REG_EVOL_R => {
                    self.echo_vol_right = value;
                }
                REG_KON => {
                    self.kon = value;
                    self.set_kon(value);
                }
                REG_KOF => {
                    self.kof = value;
                    self.set_kof(value);
                }
                REG_FLG => {
                    self.set_flg(value);
                }

                REG_EFB => {
                    self.echo_feedback = value;
                }

                REG_PMON => {
                    self.pmon = value;
                    self.set_pmon(value);
                }
                REG_NON => {
                    self.non = value;
                    self.set_nov(value);
                }
                REG_EON => {
                    self.eon = value;
                    self.set_eon(value);
                }
                REG_DIR => {
                    self.source_dir = value;
                }
                REG_ESA => {
                    self.echo_start_address = (value as u16) << 8;
                }
                REG_EDL => {
                    self.echo_delay = value & EDL_MASK;
                }
                REG_ENDX => {
                    self.endx = 0;
                }

                _ => (), // Do nothing
            }
        }
    }

    pub fn get_register(&mut self, address: u8) -> u8 {
        if !self.is_flushing {
            self.flush();
        }

        if (address & REG_READONLY_MASK) != 0 {
            return 0;
        }

        let voice_index = (address >> VOICE_INDEX_SHIFT) as usize;
        let voice_address = address & VOICE_REG_MASK;
        if voice_address < VOICE_REG_WRITABLE_END && voice_index < self.voices.len() {
            let voice = &self.voices[voice_index];
            return match voice_address {
                VOICE_REG_VOL_L => voice.vol_left,
                VOICE_REG_VOL_R => voice.vol_right,
                VOICE_REG_PITCH_L => voice.pitch_low,
                VOICE_REG_PITCH_H => voice.pitch_high(),
                VOICE_REG_SOURCE => voice.source,
                VOICE_REG_ADSR0 => voice.envelope.adsr0,
                VOICE_REG_ADSR1 => voice.envelope.adsr1,
                VOICE_REG_GAIN => voice.envelope.gain,
                VOICE_REG_ENVX => ((voice.envelope.level >> 4) & 0xFF) as u8,
                VOICE_REG_OUTX => voice.outx(),
                _ => 0,
            };
        }
        if voice_address == VOICE_REG_FIR_COEF && voice_index < self.left_filter.coefficients.len()
        {
            return self.left_filter.coefficients[voice_index];
        }

        match address {
            REG_MVOL_L => self.vol_left,
            REG_MVOL_R => self.vol_right,
            REG_EVOL_L => self.echo_vol_left,
            REG_EVOL_R => self.echo_vol_right,
            REG_EFB => self.echo_feedback,
            REG_PMON => self.pmon,
            REG_NON => self.non,
            REG_EON => self.eon,
            REG_DIR => self.source_dir,
            REG_FLG => self.flg,
            REG_ESA => (self.echo_start_address >> 8) as u8,
            REG_ENDX => self.endx,
            REG_EDL => self.echo_delay & EDL_MASK,
            REG_KON => self.kon,
            REG_KOF => self.kof,
            _ => 0,
        }
    }

    pub fn read_counter(&self, rate: i32) -> bool {
        ((self.counter + COUNTER_OFFSETS[rate as usize]) % COUNTER_RATES[rate as usize]) != 0
    }

    pub fn read_source_dir_start_address(&self, index: i32) -> u32 {
        self.read_source_dir_address(index, 0)
    }

    pub fn read_source_dir_loop_address(&self, index: i32) -> u32 {
        self.read_source_dir_address(index, 2)
    }

    fn read_source_dir_address(&self, index: i32, offset: i32) -> u32 {
        let dir_address = (self.source_dir as i32) * 0x100;
        let entry_address = dir_address + index * 4;
        let mut ret = self
            .emulator()
            .read_u8((entry_address as u32) + (offset as u32)) as u32;
        ret |= (self
            .emulator()
            .read_u8((entry_address as u32) + (offset as u32) + 1) as u32)
            << 8;
        ret
    }

    fn set_kon(&mut self, voice_mask: u8) {
        for i in 0..NUM_VOICES {
            if ((voice_mask as usize) & (1 << i)) != 0 {
                // Trace all KON events
                if crate::debug_flags::trace_top_spc_cmd() {
                    use std::sync::atomic::{AtomicU32, Ordering};
                    static KON_CNT: AtomicU32 = AtomicU32::new(0);
                    let n = KON_CNT.fetch_add(1, Ordering::Relaxed);
                    if n < 500 {
                        let v = &self.voices[i];
                        let pitch = ((v.pitch_high() as u16) << 8) | (v.pitch_low as u16);
                        let dir_base = (self.source_dir as u32) * 0x100;
                        let entry = dir_base + (v.source as u32) * 4;
                        let s_lo = self.emulator().read_u8(entry) as u16;
                        let s_hi = self.emulator().read_u8(entry + 1) as u16;
                        let l_lo = self.emulator().read_u8(entry + 2) as u16;
                        let l_hi = self.emulator().read_u8(entry + 3) as u16;
                        let start = (s_hi << 8) | s_lo;
                        let lp = (l_hi << 8) | l_lo;
                        let mut brr = [0u8; 9];
                        for (offset, byte) in brr.iter_mut().enumerate() {
                            *byte = self.emulator().read_u8(u32::from(start) + offset as u32);
                        }
                        eprintln!(
                            "[DSP-KON] v{} src={} pitch={:04X} vol=({:02X},{:02X}) DIR={:02X} smp@{:04X} loop@{:04X} adsr=({:02X},{:02X}) gain={:02X} brr={:02X?}",
                            i, v.source, pitch, v.vol_left, v.vol_right,
                            self.source_dir, start, lp,
                            v.envelope.adsr0, v.envelope.adsr1, v.envelope.gain, brr
                        );
                    }
                }
                self.voices[i].key_on();
            }
        }
    }

    fn set_kof(&mut self, voice_mask: u8) {
        for i in 0..NUM_VOICES {
            if ((voice_mask as usize) & (1 << i)) != 0 {
                self.voices[i].key_off();
            }
        }
    }

    fn set_flg(&mut self, value: u8) {
        let prev = self.flg;
        if (value & FLG_RESET) != 0 && (prev & FLG_RESET) == 0 {
            // DSP reset bit: clear internal state on 0->1 transition.
            self.reset();
        }
        self.noise_clock = value & FLG_NOISE_CLOCK_MASK;
        self.echo_write_enabled = (value & FLG_ECHO_DISABLE) == 0;
        self.flg = value;
    }

    fn set_pmon(&mut self, voice_mask: u8) {
        for i in 1..NUM_VOICES {
            self.voices[i].pitch_mod = ((voice_mask as usize) & (1 << i)) != 0;
        }
    }

    fn set_nov(&mut self, voice_mask: u8) {
        for i in 0..NUM_VOICES {
            self.voices[i].noise_on = ((voice_mask as usize) & (1 << i)) != 0;
        }
    }

    fn set_eon(&mut self, voice_mask: u8) {
        for i in 0..NUM_VOICES {
            self.voices[i].echo_on = ((voice_mask as usize) & (1 << i)) != 0;
        }
    }

    pub fn dump_audio_wav(&self) {
        self.audio_dump.write_all();
    }
}

fn echo_address(start: u16, position: i32) -> u32 {
    start.wrapping_add(position as u16) as u32
}

fn advance_output_filter(current: i32, target: i32, alpha_q8: i32) -> i32 {
    let delta = (((target as i64) - (current as i64)) * (alpha_q8 as i64)) >> 8;
    let next = (current as i64) + delta;
    next.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echo_address_wraps_at_spc_ram_boundary() {
        assert_eq!(echo_address(0xF800, 0x0800), 0x0000);
        assert_eq!(echo_address(0xF800, 0x0804), 0x0004);
        assert_eq!(echo_address(0xFFFC, 0x0004), 0x0000);
    }

    #[test]
    fn output_filter_handles_full_scale_swings_without_overflow() {
        let mut dsp = Dsp::new(std::ptr::null_mut());

        let first = dsp.filter_final_sample(i16::MAX, i16::MIN);
        let second = dsp.filter_final_sample(i16::MIN, i16::MAX);

        assert!(first.0 > 0);
        assert!(first.1 < 0);
        assert!(second.0 < 0);
        assert!(second.1 > 0);
    }
}
