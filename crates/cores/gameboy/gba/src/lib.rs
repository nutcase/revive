mod bus;
mod cpu;
mod ppu;
pub mod state;
mod timer;

use bus::GbaBus;
use cpu::Arm7Tdmi;
use emulator_core::{ConsoleKind, EmuError, EmuResult, EmulatorCore, FrameResult, RomImage};
use ppu::GBA_FRAME_CYCLES;
use ppu::GbaPpu;
use std::sync::OnceLock;
use timer::GbaTimer;

pub const GBA_LCD_WIDTH: u32 = 240;
pub const GBA_LCD_HEIGHT: u32 = 160;
pub const GBA_KEY_A: u16 = 1 << 0;
pub const GBA_KEY_B: u16 = 1 << 1;
pub const GBA_KEY_SELECT: u16 = 1 << 2;
pub const GBA_KEY_START: u16 = 1 << 3;
pub const GBA_KEY_RIGHT: u16 = 1 << 4;
pub const GBA_KEY_LEFT: u16 = 1 << 5;
pub const GBA_KEY_UP: u16 = 1 << 6;
pub const GBA_KEY_DOWN: u16 = 1 << 7;
pub const GBA_KEY_R: u16 = 1 << 8;
pub const GBA_KEY_L: u16 = 1 << 9;

const REG_DISPCNT: u32 = 0x0400_0000;
#[cfg(test)]
const REG_BG2CNT: u32 = 0x0400_000C;
const REG_BG2PA: u32 = 0x0400_0020;
const REG_BG2PB: u32 = 0x0400_0022;
const REG_BG2PC: u32 = 0x0400_0024;
const REG_BG2PD: u32 = 0x0400_0026;
const REG_BG2X: u32 = 0x0400_0028;
const REG_BG2Y: u32 = 0x0400_002C;
const REG_BG3PA: u32 = 0x0400_0030;
const REG_BG3PB: u32 = 0x0400_0032;
const REG_BG3PC: u32 = 0x0400_0034;
const REG_BG3PD: u32 = 0x0400_0036;
const REG_BG3X: u32 = 0x0400_0038;
const REG_BG3Y: u32 = 0x0400_003C;
#[cfg(test)]
const REG_MOSAIC: u32 = 0x0400_004C;
const VRAM_BASE: u32 = 0x0600_0000;
const OAM_BASE: u32 = 0x0700_0000;
const OBJ_CHAR_BASE_TEXT: u32 = VRAM_BASE + 0x10_000;
const OBJ_CHAR_BASE_BITMAP: u32 = VRAM_BASE + 0x14_000;

const LAYER_BG0: u8 = 0;
const LAYER_BG1: u8 = 1;
const LAYER_BG2: u8 = 2;
const LAYER_BG3: u8 = 3;
const LAYER_OBJ: u8 = 4;
const LAYER_BD: u8 = 5;
const WINDOW_MASK_ALL: u8 = 0x3F;
const GBA_PIXEL_COUNT: usize = (GBA_LCD_WIDTH as usize) * (GBA_LCD_HEIGHT as usize);
const GBA_FRAME_RGBA8888_BYTES: usize = GBA_PIXEL_COUNT * 4;
static TRACE_SCANLINE_IO_ENABLED: OnceLock<bool> = OnceLock::new();

pub struct GbaFrameBuffer {
    pixels: Vec<u8>,
    second_pixels: Vec<u8>,
    bg_priorities: Vec<u8>,
    second_bg_priorities: Vec<u8>,
    layer_ids: Vec<u8>,
    second_layer_ids: Vec<u8>,
    window_masks: Vec<u8>,
    obj_semitrans: Vec<bool>,
    prev_pixels: Vec<u8>,
    frame_blend: bool,
    no_effects: bool,
    debug_blend_frames: u32,
}

impl Default for GbaFrameBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl GbaFrameBuffer {
    pub fn new() -> Self {
        Self {
            pixels: vec![0; GBA_FRAME_RGBA8888_BYTES],
            second_pixels: vec![0; GBA_FRAME_RGBA8888_BYTES],
            bg_priorities: vec![4; GBA_PIXEL_COUNT],
            second_bg_priorities: vec![4; GBA_PIXEL_COUNT],
            layer_ids: vec![LAYER_BD; GBA_PIXEL_COUNT],
            second_layer_ids: vec![LAYER_BD; GBA_PIXEL_COUNT],
            window_masks: vec![WINDOW_MASK_ALL; GBA_PIXEL_COUNT],
            obj_semitrans: vec![false; GBA_PIXEL_COUNT],
            prev_pixels: vec![0; GBA_FRAME_RGBA8888_BYTES],
            frame_blend: false,
            no_effects: false,
            debug_blend_frames: 0,
        }
    }

    pub fn set_frame_blend(&mut self, enabled: bool) {
        self.frame_blend = enabled;
    }

    pub fn set_no_effects(&mut self, enabled: bool) {
        self.no_effects = enabled;
    }

    pub fn set_debug_blend_frames(&mut self, frames: u32) {
        self.debug_blend_frames = frames;
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    fn prepare_for_render(&mut self) {
        self.bg_priorities.fill(4);
        self.second_bg_priorities.fill(4);
        self.layer_ids.fill(LAYER_BD);
        self.second_layer_ids.fill(LAYER_BD);
        self.window_masks.fill(WINDOW_MASK_ALL);
        self.obj_semitrans.fill(false);
    }
}

#[derive(Debug, Clone, Copy)]
struct TextBgLayer {
    bg: u8,
    cnt: u16,
    hofs: u32,
    vofs: u32,
    priority: u8,
}

#[derive(Debug, Clone, Copy)]
struct AffineBgLayer {
    bg: u8,
    cnt: u16,
    pa: i32,
    pb: i32,
    pc: i32,
    pd: i32,
    ref_x: i64,
    ref_y: i64,
    priority: u8,
}

#[derive(Debug, Clone, Copy)]
enum BgLayer {
    Text(TextBgLayer),
    Affine(AffineBgLayer),
}

#[derive(Debug, Clone, Copy)]
struct ObjPixel {
    obj_index: usize,
    color: u16,
    priority: u8,
    semi_transparent: bool,
}

#[derive(Debug, Clone, Copy, Default)]
struct ObjAttributes {
    attr0: u16,
    attr1: u16,
    attr2: u16,
}

#[derive(Debug, Clone, Copy, Default)]
struct ObjAffineParams {
    pa: i32,
    pb: i32,
    pc: i32,
    pd: i32,
}

#[derive(Debug, Clone, Copy)]
struct PixelCandidate {
    color: (u8, u8, u8),
    layer: u8,
    priority: u8,
    obj_index: usize,
    semi_transparent: bool,
}

#[derive(Debug, Clone, Copy)]
struct MosaicState {
    bg_h: u32,
    bg_v: u32,
    obj_h: u32,
    obj_v: u32,
}

impl MosaicState {
    fn from_register(value: u16) -> Self {
        Self {
            bg_h: u32::from(value & 0x000F) + 1,
            bg_v: u32::from((value >> 4) & 0x000F) + 1,
            obj_h: u32::from((value >> 8) & 0x000F) + 1,
            obj_v: u32::from((value >> 12) & 0x000F) + 1,
        }
    }
}

#[derive(Debug, Default)]
pub struct GbaEmulator {
    bus: GbaBus,
    cpu: Arm7Tdmi,
    ppu: GbaPpu,
    timer: GbaTimer,
    frame_number: u64,
    rom_loaded: bool,
}

impl GbaEmulator {
    pub fn new() -> Self {
        let mut emulator = Self::default();
        emulator.cpu.reset();
        emulator
    }

    pub fn load_bios(&mut self, bios: &[u8]) {
        self.bus.load_bios(bios);
    }

    pub fn set_keyinput_pressed_mask(&mut self, pressed_mask: u16) {
        self.bus.set_keyinput_pressed_mask(pressed_mask);
    }

    pub fn has_backup(&self) -> bool {
        self.bus.has_backup()
    }

    pub fn backup_data(&self) -> Option<Vec<u8>> {
        self.bus.backup_data()
    }

    pub fn load_backup_data(&mut self, data: &[u8]) {
        self.bus.load_backup_data(data);
    }

    pub fn take_audio_samples_i16(&mut self) -> Vec<i16> {
        self.bus.take_audio_samples()
    }

    pub fn take_audio_samples_i16_into(&mut self, out: &mut Vec<i16>) {
        self.bus.take_audio_samples_into(out);
    }

    pub fn debug_read16(&self, addr: u32) -> u16 {
        self.bus.read16(addr)
    }

    pub fn debug_read32(&self, addr: u32) -> u32 {
        self.bus.read32(addr)
    }

    pub fn debug_scanline_io_read16(&self, line: u32, io_offset: u32) -> u16 {
        self.bus.scanline_io_read16(line, io_offset)
    }

    pub fn debug_scanline_oam_read16(&self, line: u32, addr: u32) -> u16 {
        self.bus.scanline_oam_read16(line, addr)
    }

    pub fn debug_scanline_pram_read16(&self, line: u32, offset: u32) -> u16 {
        self.bus.scanline_pram_read16(line, offset)
    }

    pub fn debug_scanline_obj_vram_read8(&self, line: u32, addr: u32) -> u8 {
        self.bus.scanline_obj_vram_read8(line, addr)
    }

    pub fn debug_scanline_bg_bitmap_vram_read16(&self, line: u32, addr: u32) -> u16 {
        self.bus.scanline_bg_bitmap_vram_read16(line, addr)
    }

    pub fn debug_read_pram16(&self, offset: u32) -> u16 {
        self.bus.read_pram16(offset)
    }

    /// Hash the PRAM/VRAM/OAM render snapshots (what the renderer actually sees).
    pub fn debug_snapshot_hashes(&self) -> (u32, u32, u32) {
        let pram_h = state::crc32(&self.bus.pram_snapshot);
        let vram_h = state::crc32(&self.bus.vram_snapshot);
        let oam_h = state::crc32(&self.bus.oam_snapshot);
        (pram_h, vram_h, oam_h)
    }

    pub fn debug_cpu_pc(&self) -> u32 {
        self.cpu.program_counter()
    }

    pub fn debug_cpu_cpsr(&self) -> u32 {
        self.cpu.cpsr()
    }

    pub fn debug_cpu_reg(&self, index: usize) -> u32 {
        self.cpu.reg(index)
    }

    pub fn debug_audio_sample_rate_hz(&self) -> u32 {
        self.bus.current_audio_sample_rate_hz()
    }

    fn serialize_state_payload(&self) -> Vec<u8> {
        let mut w = state::StateWriter::new();
        self.bus.serialize_state(&mut w);
        self.cpu.serialize_state(&mut w);
        self.ppu.serialize_state(&mut w);
        self.timer.serialize_state(&mut w);
        w.write_u64(self.frame_number);
        w.into_vec()
    }

    pub fn save_state(&self) -> Vec<u8> {
        let payload = self.serialize_state_payload();
        let rom_crc = self.rom_crc32();
        let payload_len = payload.len() as u32;
        let mut out = Vec::with_capacity(16 + payload.len());
        out.extend_from_slice(b"GBAS");
        out.extend_from_slice(&1u32.to_le_bytes()); // version
        out.extend_from_slice(&rom_crc.to_le_bytes());
        out.extend_from_slice(&payload_len.to_le_bytes());
        out.extend_from_slice(&payload);
        out
    }

    pub fn load_state(&mut self, data: &[u8]) -> Result<(), &'static str> {
        if data.len() < 16 {
            return Err("state data too short");
        }
        if &data[0..4] != b"GBAS" {
            return Err("invalid state magic");
        }
        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        if version != 1 {
            return Err("unsupported state version");
        }
        let rom_crc = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
        if rom_crc != self.rom_crc32() {
            return Err("ROM CRC mismatch");
        }
        let payload_len = u32::from_le_bytes([data[12], data[13], data[14], data[15]]) as usize;
        if data.len() < 16 + payload_len {
            return Err("state data truncated");
        }
        let payload = &data[16..16 + payload_len];
        let current_payload_len = self.serialize_state_payload().len();
        let upgraded_payload =
            GbaBus::maybe_upgrade_legacy_state_payload(payload, current_payload_len);
        let payload = upgraded_payload.as_deref().unwrap_or(payload);
        let mut r = state::StateReader::new(payload);
        self.bus.deserialize_state(&mut r)?;
        self.cpu.deserialize_state(&mut r)?;
        self.ppu.deserialize_state(&mut r)?;
        self.timer.deserialize_state(&mut r)?;
        self.frame_number = r.read_u64()?;
        Ok(())
    }

    fn rom_crc32(&self) -> u32 {
        state::crc32(self.bus.rom_bytes())
    }

    pub fn frame_rgba8888(&self) -> Vec<u8> {
        let mut frame = GbaFrameBuffer::new();
        self.render_frame_rgba8888(&mut frame);
        frame.pixels
    }

    pub fn render_frame_rgba8888<'a>(&self, frame: &'a mut GbaFrameBuffer) -> &'a [u8] {
        frame.prepare_for_render();
        let mut obj_attrs = [ObjAttributes::default(); 128];
        let mut obj_affine = [ObjAffineParams::default(); 32];
        for y in 0..GBA_LCD_HEIGHT {
            self.build_obj_render_cache_for_line(y, &mut obj_attrs, &mut obj_affine);
            self.render_scanline(frame, y, &obj_attrs, &obj_affine);
        }
        if frame.debug_blend_frames > 0 {
            frame.debug_blend_frames -= 1;
            // Dump blend diagnostic for scanline 80, pixels x=0..80
            let y = 80u32;
            let bldcnt = self.bus.scanline_io_read16(y, 0x50);
            let bldalpha = self.bus.scanline_io_read16(y, 0x52);
            let dispcnt_snap = self.bus.scanline_io_read16(y, 0x00);
            let dispcnt_live = self.bus.read16(REG_DISPCNT);
            eprintln!(
                "[dbg] y=80 DISPCNT snap={:#06X} live={:#06X} BLDCNT={:#06X} BLDALPHA={:#06X}",
                dispcnt_snap, dispcnt_live, bldcnt, bldalpha
            );
            for x in [10u32, 20, 40, 60] {
                let idx = (y * GBA_LCD_WIDTH + x) as usize;
                let lid = frame.layer_ids[idx];
                let s_lid = frame.second_layer_ids[idx];
                let r = frame.pixels[idx * 4];
                let g = frame.pixels[idx * 4 + 1];
                let b = frame.pixels[idx * 4 + 2];
                let wmask = frame.window_masks[idx];
                eprintln!(
                    "[dbg]   x={x} layer={lid} second={s_lid} rgb=({r},{g},{b}) wmask={wmask:#04X}"
                );
            }
        }
        if frame.frame_blend {
            for i in 0..frame.pixels.len() {
                let cur = frame.pixels[i] as u16;
                let prev = frame.prev_pixels[i] as u16;
                frame.prev_pixels[i] = frame.pixels[i];
                frame.pixels[i] = ((cur + prev) / 2) as u8;
            }
        }
        frame.pixels()
    }

    /// Render a single scanline using the current render memory view.
    ///
    /// Called during the frame loop near the end of each visible scanline's
    /// draw period, immediately BEFORE HBlank DMA fires for that line.
    /// This keeps same-line HBlank DMA from leaking into the current render.
    /// The same path is reused by `frame_rgba8888()`, where the bus reads from
    /// the PRAM/VRAM/OAM snapshot captured at VBlank entry.
    fn render_scanline(
        &self,
        frame: &mut GbaFrameBuffer,
        y: u32,
        obj_attrs: &[ObjAttributes; 128],
        obj_affine: &[ObjAffineParams; 32],
    ) {
        let y_dispcnt = self.bus.scanline_io_read16(y, 0x00);
        let mosaic = MosaicState::from_register(self.bus.scanline_io_read16(y, 0x4C));
        let mode = y_dispcnt & 0x0007;

        if (y_dispcnt & (1 << 7)) != 0 {
            // Forced blank — white
            let base = (y * GBA_LCD_WIDTH) as usize;
            for x in 0..GBA_LCD_WIDTH as usize {
                write_pixel_rgba8888(&mut frame.pixels, base + x, 0x7FFF);
                write_pixel_rgba8888(&mut frame.second_pixels, base + x, 0x7FFF);
            }
            return;
        }

        // 1. Build window mask for this scanline
        self.build_window_masks_for_line(
            y,
            y_dispcnt,
            mosaic,
            &mut frame.window_masks,
            obj_attrs,
            obj_affine,
        );

        // 2. Render BG tiles for this scanline
        let backdrop = self.read_bg_palette_color_for_line(y, 0);
        match mode {
            0..=2 => self.render_tile_modes_for_line(
                y,
                y_dispcnt,
                mode,
                backdrop,
                &mut frame.pixels,
                &mut frame.bg_priorities,
                &mut frame.second_bg_priorities,
                &mut frame.layer_ids,
                &mut frame.second_pixels,
                &mut frame.second_layer_ids,
                &frame.window_masks,
                mosaic,
            ),
            3 => self.render_mode3_for_line(
                y,
                y_dispcnt,
                backdrop,
                &mut frame.pixels,
                &mut frame.bg_priorities,
                &mut frame.second_bg_priorities,
                &mut frame.layer_ids,
                &mut frame.second_pixels,
                &mut frame.second_layer_ids,
                &frame.window_masks,
                mosaic,
            ),
            4 => self.render_mode4_for_line(
                y,
                y_dispcnt,
                backdrop,
                &mut frame.pixels,
                &mut frame.bg_priorities,
                &mut frame.second_bg_priorities,
                &mut frame.layer_ids,
                &mut frame.second_pixels,
                &mut frame.second_layer_ids,
                &frame.window_masks,
                mosaic,
            ),
            5 => self.render_mode5_for_line(
                y,
                y_dispcnt,
                backdrop,
                &mut frame.pixels,
                &mut frame.bg_priorities,
                &mut frame.second_bg_priorities,
                &mut frame.layer_ids,
                &mut frame.second_pixels,
                &mut frame.second_layer_ids,
                &frame.window_masks,
                mosaic,
            ),
            _ => {
                let base = (y * GBA_LCD_WIDTH) as usize;
                for x in 0..GBA_LCD_WIDTH as usize {
                    write_pixel_rgba8888(&mut frame.pixels, base + x, backdrop);
                    write_pixel_rgba8888(&mut frame.second_pixels, base + x, backdrop);
                }
            }
        }

        // 3. Overlay OBJs for this scanline
        self.overlay_objects_for_line(
            y,
            y_dispcnt,
            &mut frame.bg_priorities,
            &mut frame.second_bg_priorities,
            &mut frame.layer_ids,
            &mut frame.pixels,
            &mut frame.second_pixels,
            &mut frame.second_layer_ids,
            &mut frame.obj_semitrans,
            &frame.window_masks,
            mosaic,
            obj_attrs,
            obj_affine,
        );

        // 4. Apply color effects for this scanline
        if !frame.no_effects {
            self.apply_color_effects_for_line(
                y,
                &mut frame.pixels,
                &frame.layer_ids,
                &frame.second_pixels,
                &frame.second_layer_ids,
                &frame.obj_semitrans,
                &frame.window_masks,
            );
        }
    }

    fn scanline_io_read32(&self, line: u32, io_offset: u32) -> u32 {
        u32::from(self.bus.scanline_io_read16(line, io_offset))
            | (u32::from(self.bus.scanline_io_read16(line, io_offset + 2)) << 16)
    }

    fn read_affine_bg_layer(&self, line: u32, bg: u8, cnt: u16) -> AffineBgLayer {
        let (pa_reg, pb_reg, pc_reg, pd_reg, x_reg, y_reg) = match bg {
            2 => (
                REG_BG2PA, REG_BG2PB, REG_BG2PC, REG_BG2PD, REG_BG2X, REG_BG2Y,
            ),
            3 => (
                REG_BG3PA, REG_BG3PB, REG_BG3PC, REG_BG3PD, REG_BG3X, REG_BG3Y,
            ),
            _ => unreachable!("affine background must be BG2 or BG3"),
        };

        let pa_offset = pa_reg - REG_DISPCNT;
        let pb_offset = pb_reg - REG_DISPCNT;
        let pc_offset = pc_reg - REG_DISPCNT;
        let pd_offset = pd_reg - REG_DISPCNT;
        let x_offset = x_reg - REG_DISPCNT;
        let y_offset = y_reg - REG_DISPCNT;

        let raw_x = self.scanline_io_read32(line, x_offset) & 0x0FFF_FFFF;
        let raw_y = self.scanline_io_read32(line, y_offset) & 0x0FFF_FFFF;
        let pa = i32::from(self.bus.scanline_io_read16(line, pa_offset) as i16);
        let pb = i32::from(self.bus.scanline_io_read16(line, pb_offset) as i16);
        let pc = i32::from(self.bus.scanline_io_read16(line, pc_offset) as i16);
        let pd = i32::from(self.bus.scanline_io_read16(line, pd_offset) as i16);

        AffineBgLayer {
            bg,
            cnt,
            pa,
            pb,
            pc,
            pd,
            ref_x: i64::from(sign_extend_u32(raw_x, 28)),
            ref_y: i64::from(sign_extend_u32(raw_y, 28)),
            priority: (cnt & 0x0003) as u8,
        }
    }

    fn sample_text_bg_color(&self, line: u32, layer: &TextBgLayer, x: u32, y: u32) -> Option<u16> {
        let size = ((layer.cnt >> 14) & 0x3) as u8;
        let (map_w, map_h) = text_bg_dimensions(size);

        let sx = (x + layer.hofs) % map_w;
        let sy = (y + layer.vofs) % map_h;
        let tile_x = sx / 8;
        let tile_y = sy / 8;

        let entry_addr =
            text_bg_map_entry_addr(((layer.cnt >> 8) & 0x1F) as u32, size, tile_x, tile_y);
        let entry = self.bus.scanline_bg_bitmap_vram_read16(line, entry_addr);

        let mut px = sx & 7;
        let mut py = sy & 7;
        if (entry & (1 << 10)) != 0 {
            px = 7 - px;
        }
        if (entry & (1 << 11)) != 0 {
            py = 7 - py;
        }

        let tile_index = (entry & 0x03FF) as u32;
        let char_base = ((u32::from(layer.cnt >> 2)) & 0x3) * 0x4000;
        let color_mode_8bpp = (layer.cnt & (1 << 7)) != 0;

        if color_mode_8bpp {
            let tile_addr = VRAM_BASE + char_base + tile_index * 64 + py * 8 + px;
            let index = self.bus.scanline_bg_bitmap_vram_read8(line, tile_addr);
            if index == 0 {
                return None;
            }
            Some(self.read_bg_palette_color_for_line(line, u16::from(index)))
        } else {
            let tile_addr = VRAM_BASE + char_base + tile_index * 32 + py * 4 + (px / 2);
            let byte = self.bus.scanline_bg_bitmap_vram_read8(line, tile_addr);
            let index = if (px & 1) == 0 {
                byte & 0x0F
            } else {
                byte >> 4
            };
            if index == 0 {
                return None;
            }

            let palette_bank = ((entry >> 12) & 0xF) as u16;
            let palette_index = palette_bank * 16 + u16::from(index);
            Some(self.read_bg_palette_color_for_line(line, palette_index))
        }
    }

    fn sample_affine_bg_color(
        &self,
        line: u32,
        layer: &AffineBgLayer,
        x: u32,
        y: u32,
    ) -> Option<u16> {
        let size = affine_bg_dimension(((layer.cnt >> 14) & 0x3) as u8);
        let map_w_tiles = size / 8;
        let wrap = (layer.cnt & (1 << 13)) != 0;

        let tex_x =
            layer.ref_x + i64::from(layer.pa) * i64::from(x) + i64::from(layer.pb) * i64::from(y);
        let tex_y =
            layer.ref_y + i64::from(layer.pc) * i64::from(x) + i64::from(layer.pd) * i64::from(y);

        let mut sx = (tex_x >> 8) as i32;
        let mut sy = (tex_y >> 8) as i32;
        if wrap {
            sx = sx.rem_euclid(size as i32);
            sy = sy.rem_euclid(size as i32);
        } else if sx < 0 || sy < 0 || sx >= size as i32 || sy >= size as i32 {
            return None;
        }

        let sx = sx as u32;
        let sy = sy as u32;
        let tile_x = sx / 8;
        let tile_y = sy / 8;
        let local_x = sx & 7;
        let local_y = sy & 7;

        let screen_base = ((layer.cnt >> 8) & 0x1F) as u32;
        let char_base = ((u32::from(layer.cnt >> 2)) & 0x3) * 0x4000;
        let map_addr = VRAM_BASE + screen_base * 0x800 + tile_y * map_w_tiles + tile_x;
        let tile_index = u32::from(self.bus.scanline_bg_bitmap_vram_read8(line, map_addr));
        let tile_addr = VRAM_BASE + char_base + tile_index * 64 + local_y * 8 + local_x;
        let palette_index = self.bus.scanline_bg_bitmap_vram_read8(line, tile_addr);
        if palette_index == 0 {
            return None;
        }

        Some(self.read_bg_palette_color_for_line(line, u16::from(palette_index)))
    }

    fn read_bg_palette_color_for_line(&self, line: u32, index: u16) -> u16 {
        self.bus.scanline_pram_read16(line, u32::from(index) * 2)
    }

    fn build_obj_render_cache_for_line(
        &self,
        line: u32,
        obj_attrs: &mut [ObjAttributes; 128],
        obj_affine: &mut [ObjAffineParams; 32],
    ) {
        for (obj_index, attrs) in obj_attrs.iter_mut().enumerate() {
            let base = OAM_BASE + (obj_index as u32) * 8;
            attrs.attr0 = self.bus.scanline_oam_read16(line, base);
            attrs.attr1 = self.bus.scanline_oam_read16(line, base + 2);
            attrs.attr2 = self.bus.scanline_oam_read16(line, base + 4);
        }
        for (affine_index, params) in obj_affine.iter_mut().enumerate() {
            let base = OAM_BASE + 0x06 + (affine_index as u32) * 32;
            params.pa = i32::from(self.bus.scanline_oam_read16(line, base) as i16);
            params.pb = i32::from(self.bus.scanline_oam_read16(line, base + 8) as i16);
            params.pc = i32::from(self.bus.scanline_oam_read16(line, base + 16) as i16);
            params.pd = i32::from(self.bus.scanline_oam_read16(line, base + 24) as i16);
        }
    }

    // ── Per-scanline rendering helpers ──────────────────────────────────
    //
    // These are used both by the realtime scanline renderer and the offline
    // `frame_rgba8888()` path so each visible line sees its own IO snapshot.

    fn build_window_masks_for_line(
        &self,
        y: u32,
        dispcnt: u16,
        mosaic: MosaicState,
        window_masks: &mut [u8],
        obj_attrs: &[ObjAttributes; 128],
        obj_affine: &[ObjAffineParams; 32],
    ) {
        let win0_enabled = (dispcnt & (1 << 13)) != 0;
        let win1_enabled = (dispcnt & (1 << 14)) != 0;
        let objwin_enabled = (dispcnt & (1 << 15)) != 0;

        if !win0_enabled && !win1_enabled && !objwin_enabled {
            let base = (y * GBA_LCD_WIDTH) as usize;
            window_masks[base..base + GBA_LCD_WIDTH as usize].fill(WINDOW_MASK_ALL);
            return;
        }

        let winin = self.bus.scanline_io_read16(y, 0x48);
        let winout = self.bus.scanline_io_read16(y, 0x4A);
        let win0_mask = (winin & 0x003F) as u8;
        let win1_mask = ((winin >> 8) & 0x003F) as u8;
        let outside_mask = (winout & 0x003F) as u8;
        let objwin_mask = ((winout >> 8) & 0x003F) as u8;

        let (win0_x1, win0_x2) = window_axis_bounds(self.bus.scanline_io_read16(y, 0x40));
        let (win0_y1, win0_y2) = window_axis_bounds(self.bus.scanline_io_read16(y, 0x44));
        let (win1_x1, win1_x2) = window_axis_bounds(self.bus.scanline_io_read16(y, 0x42));
        let (win1_y1, win1_y2) = window_axis_bounds(self.bus.scanline_io_read16(y, 0x46));

        for x in 0..GBA_LCD_WIDTH {
            let mut mask = outside_mask;
            if objwin_enabled
                && self.sample_objwin_hit(y, dispcnt, x, y, mosaic, obj_attrs, obj_affine)
            {
                mask = objwin_mask;
            }
            if win1_enabled && point_in_window(x, y, win1_x1, win1_x2, win1_y1, win1_y2) {
                mask = win1_mask;
            }
            if win0_enabled && point_in_window(x, y, win0_x1, win0_x2, win0_y1, win0_y2) {
                mask = win0_mask;
            }
            window_masks[(y * GBA_LCD_WIDTH + x) as usize] = mask;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_tile_modes_for_line(
        &self,
        y: u32,
        y_dispcnt: u16,
        mode: u16,
        backdrop: u16,
        pixels: &mut [u8],
        bg_priorities: &mut [u8],
        second_bg_priorities: &mut [u8],
        layer_ids: &mut [u8],
        second_pixels: &mut [u8],
        second_layer_ids: &mut [u8],
        window_masks: &[u8],
        mosaic: MosaicState,
    ) {
        let mode = (mode & 0x7) as u8;
        let mut layers = [None::<BgLayer>; 4];
        let mut layer_count = 0usize;
        for bg in 0..4u8 {
            if (y_dispcnt & (1 << (8 + bg))) == 0 {
                continue;
            }
            let cnt = self.bus.scanline_io_read16(y, 0x08 + u32::from(bg) * 2);
            let layer = match mode {
                0 => Some(BgLayer::Text(TextBgLayer {
                    bg,
                    cnt,
                    hofs: u32::from(
                        self.bus.scanline_io_read16(y, 0x10 + u32::from(bg) * 4) & 0x01FF,
                    ),
                    vofs: u32::from(
                        self.bus.scanline_io_read16(y, 0x12 + u32::from(bg) * 4) & 0x01FF,
                    ),
                    priority: (cnt & 0x0003) as u8,
                })),
                1 if bg <= 1 => Some(BgLayer::Text(TextBgLayer {
                    bg,
                    cnt,
                    hofs: u32::from(
                        self.bus.scanline_io_read16(y, 0x10 + u32::from(bg) * 4) & 0x01FF,
                    ),
                    vofs: u32::from(
                        self.bus.scanline_io_read16(y, 0x12 + u32::from(bg) * 4) & 0x01FF,
                    ),
                    priority: (cnt & 0x0003) as u8,
                })),
                1 if bg == 2 => Some(BgLayer::Affine(self.read_affine_bg_layer(y, bg, cnt))),
                2 if bg >= 2 => Some(BgLayer::Affine(self.read_affine_bg_layer(y, bg, cnt))),
                _ => None,
            };
            if let Some(l) = layer {
                layers[layer_count] = Some(l);
                layer_count += 1;
            }
        }

        // Insertion sort by (priority, bg_index)
        for i in 1..layer_count {
            let mut j = i;
            while j > 0 {
                let key_a = layer_sort_key(layers[j - 1].as_ref().unwrap());
                let key_b = layer_sort_key(layers[j].as_ref().unwrap());
                if key_a > key_b {
                    layers.swap(j - 1, j);
                    j -= 1;
                } else {
                    break;
                }
            }
        }

        for x in 0..GBA_LCD_WIDTH {
            let mut color = backdrop;
            let mut priority = 4u8;
            let mut layer_id = LAYER_BD;
            let mut second_color = backdrop;
            let mut second_priority = 4u8;
            let mut second_layer_id = LAYER_BD;
            let mut found_top = false;
            let index = (y * GBA_LCD_WIDTH + x) as usize;
            let window_mask = window_masks[index];
            for i in 0..layer_count {
                let layer = layers[i].as_ref().unwrap();
                if !window_allows_layer(window_mask, layer_id_for_bg(layer_bg_index(layer))) {
                    continue;
                }
                let mosaic_enabled = match layer {
                    BgLayer::Text(l) => (l.cnt & (1 << 6)) != 0,
                    BgLayer::Affine(l) => (l.cnt & (1 << 6)) != 0,
                };
                let sample_x = if mosaic_enabled {
                    x - (x % mosaic.bg_h)
                } else {
                    x
                };
                let sample_y = if mosaic_enabled {
                    y - (y % mosaic.bg_v)
                } else {
                    y
                };
                let sampled = match layer {
                    BgLayer::Text(l) => self.sample_text_bg_color(y, l, sample_x, sample_y),
                    BgLayer::Affine(l) => self.sample_affine_bg_color(y, l, sample_x, sample_y),
                };
                if let Some(layer_color) = sampled {
                    if !found_top {
                        color = layer_color;
                        priority = layer_priority(layer);
                        layer_id = layer_id_for_bg(layer_bg_index(layer));
                        found_top = true;
                    } else {
                        second_color = layer_color;
                        second_priority = layer_priority(layer);
                        second_layer_id = layer_id_for_bg(layer_bg_index(layer));
                        break;
                    }
                }
            }

            bg_priorities[index] = priority;
            second_bg_priorities[index] = second_priority;
            layer_ids[index] = layer_id;
            second_layer_ids[index] = second_layer_id;
            write_pixel_rgba8888(pixels, index, color);
            write_pixel_rgba8888(second_pixels, index, second_color);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_mode3_for_line(
        &self,
        y: u32,
        dispcnt: u16,
        backdrop: u16,
        pixels: &mut [u8],
        bg_priorities: &mut [u8],
        second_bg_priorities: &mut [u8],
        layer_ids: &mut [u8],
        second_pixels: &mut [u8],
        second_layer_ids: &mut [u8],
        window_masks: &[u8],
        mosaic: MosaicState,
    ) {
        let bg2cnt = self.bus.scanline_io_read16(y, 0x0C);
        let priority = (bg2cnt & 0x3) as u8;
        let mosaic_enabled = (bg2cnt & (1 << 6)) != 0;
        let bg2_enabled = (dispcnt & (1 << 10)) != 0;
        for x in 0..GBA_LCD_WIDTH {
            let index = (y * GBA_LCD_WIDTH + x) as usize;
            if !bg2_enabled {
                write_pixel_rgba8888(pixels, index, backdrop);
                write_pixel_rgba8888(second_pixels, index, backdrop);
                bg_priorities[index] = 4;
                second_bg_priorities[index] = 4;
                layer_ids[index] = LAYER_BD;
                second_layer_ids[index] = LAYER_BD;
                continue;
            }
            let sample_x = if mosaic_enabled {
                x - (x % mosaic.bg_h)
            } else {
                x
            };
            let sample_y = if mosaic_enabled {
                y - (y % mosaic.bg_v)
            } else {
                y
            };
            let sample_index = sample_y * GBA_LCD_WIDTH + sample_x;
            let (color, layer_id) = if window_allows_layer(window_masks[index], LAYER_BG2) {
                (
                    self.bus
                        .scanline_bg_bitmap_vram_read16(y, VRAM_BASE + sample_index * 2),
                    LAYER_BG2,
                )
            } else {
                (backdrop, LAYER_BD)
            };
            write_pixel_rgba8888(pixels, index, color);
            write_pixel_rgba8888(second_pixels, index, backdrop);
            bg_priorities[index] = if layer_id == LAYER_BG2 { priority } else { 4 };
            second_bg_priorities[index] = 4;
            layer_ids[index] = layer_id;
            second_layer_ids[index] = LAYER_BD;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_mode4_for_line(
        &self,
        y: u32,
        dispcnt: u16,
        backdrop: u16,
        pixels: &mut [u8],
        bg_priorities: &mut [u8],
        second_bg_priorities: &mut [u8],
        layer_ids: &mut [u8],
        second_pixels: &mut [u8],
        second_layer_ids: &mut [u8],
        window_masks: &[u8],
        mosaic: MosaicState,
    ) {
        let bg2cnt = self.bus.scanline_io_read16(y, 0x0C);
        let priority = (bg2cnt & 0x3) as u8;
        let mosaic_enabled = (bg2cnt & (1 << 6)) != 0;
        let bg2_enabled = (dispcnt & (1 << 10)) != 0;
        let page_offset = if (dispcnt & (1 << 4)) != 0 { 0xA000 } else { 0 };
        for x in 0..GBA_LCD_WIDTH {
            let index = (y * GBA_LCD_WIDTH + x) as usize;
            if !bg2_enabled {
                write_pixel_rgba8888(pixels, index, backdrop);
                write_pixel_rgba8888(second_pixels, index, backdrop);
                bg_priorities[index] = 4;
                second_bg_priorities[index] = 4;
                layer_ids[index] = LAYER_BD;
                second_layer_ids[index] = LAYER_BD;
                continue;
            }
            let sample_x = if mosaic_enabled {
                x - (x % mosaic.bg_h)
            } else {
                x
            };
            let sample_y = if mosaic_enabled {
                y - (y % mosaic.bg_v)
            } else {
                y
            };
            let sample_index = sample_y * GBA_LCD_WIDTH + sample_x;
            let (color, layer_id) = if window_allows_layer(window_masks[index], LAYER_BG2) {
                let pi = self
                    .bus
                    .scanline_bg_bitmap_vram_read8(y, VRAM_BASE + page_offset + sample_index);
                (
                    self.read_bg_palette_color_for_line(y, u16::from(pi)),
                    LAYER_BG2,
                )
            } else {
                (backdrop, LAYER_BD)
            };
            write_pixel_rgba8888(pixels, index, color);
            write_pixel_rgba8888(second_pixels, index, backdrop);
            bg_priorities[index] = if layer_id == LAYER_BG2 { priority } else { 4 };
            second_bg_priorities[index] = 4;
            layer_ids[index] = layer_id;
            second_layer_ids[index] = LAYER_BD;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_mode5_for_line(
        &self,
        y: u32,
        dispcnt: u16,
        backdrop: u16,
        pixels: &mut [u8],
        bg_priorities: &mut [u8],
        second_bg_priorities: &mut [u8],
        layer_ids: &mut [u8],
        second_pixels: &mut [u8],
        second_layer_ids: &mut [u8],
        window_masks: &[u8],
        mosaic: MosaicState,
    ) {
        let bg2cnt = self.bus.scanline_io_read16(y, 0x0C);
        let priority = (bg2cnt & 0x3) as u8;
        let mosaic_enabled = (bg2cnt & (1 << 6)) != 0;
        let bg2_enabled = (dispcnt & (1 << 10)) != 0;
        let page_offset = if (dispcnt & (1 << 4)) != 0 { 0xA000 } else { 0 };
        for x in 0..GBA_LCD_WIDTH {
            let sample_x = if mosaic_enabled {
                x - (x % mosaic.bg_h)
            } else {
                x
            };
            let sample_y = if mosaic_enabled {
                y - (y % mosaic.bg_v)
            } else {
                y
            };
            let (color, color_priority) = if bg2_enabled && sample_x < 160 && sample_y < 128 {
                let idx = sample_y * 160 + sample_x;
                (
                    self.bus
                        .scanline_bg_bitmap_vram_read16(y, VRAM_BASE + page_offset + idx * 2),
                    priority,
                )
            } else {
                (backdrop, 4)
            };
            let out_index = (y * GBA_LCD_WIDTH + x) as usize;
            let bg2_visible =
                color_priority < 4 && window_allows_layer(window_masks[out_index], LAYER_BG2);
            bg_priorities[out_index] = if bg2_visible { color_priority } else { 4 };
            second_bg_priorities[out_index] = 4;
            layer_ids[out_index] = if bg2_visible { LAYER_BG2 } else { LAYER_BD };
            let final_color = if bg2_visible { color } else { backdrop };
            write_pixel_rgba8888(pixels, out_index, final_color);
            write_pixel_rgba8888(second_pixels, out_index, backdrop);
            second_layer_ids[out_index] = LAYER_BD;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn overlay_objects_for_line(
        &self,
        y: u32,
        dispcnt: u16,
        bg_priorities: &mut [u8],
        second_bg_priorities: &mut [u8],
        layer_ids: &mut [u8],
        pixels: &mut [u8],
        second_pixels: &mut [u8],
        second_layer_ids: &mut [u8],
        obj_semitrans: &mut [bool],
        window_masks: &[u8],
        mosaic: MosaicState,
        obj_attrs: &[ObjAttributes; 128],
        obj_affine: &[ObjAffineParams; 32],
    ) {
        let bldcnt = self.bus.scanline_io_read16(y, 0x50);
        let second_mask = (bldcnt >> 8) & 0x003F;
        let (eva, evb) = read_blend_factors(self.bus.scanline_io_read16(y, 0x52));

        for x in 0..GBA_LCD_WIDTH {
            let index = (y * GBA_LCD_WIDTH + x) as usize;
            let mut top = PixelCandidate {
                color: read_pixel_rgb(pixels, index),
                layer: layer_ids[index],
                priority: bg_priorities[index],
                obj_index: usize::MAX,
                semi_transparent: false,
            };
            let mut second = PixelCandidate {
                color: read_pixel_rgb(second_pixels, index),
                layer: second_layer_ids[index],
                priority: second_bg_priorities[index],
                obj_index: usize::MAX,
                semi_transparent: false,
            };

            if (dispcnt & (1 << 12)) != 0 && window_allows_layer(window_masks[index], LAYER_OBJ) {
                let (obj_top, obj_second) =
                    self.sample_obj_pixels(y, dispcnt, x, y, mosaic, obj_attrs, obj_affine);
                if let Some(obj_top) = obj_top {
                    let candidate = PixelCandidate {
                        color: bgr555_to_rgb888(obj_top.color),
                        layer: LAYER_OBJ,
                        priority: obj_top.priority,
                        obj_index: obj_top.obj_index,
                        semi_transparent: obj_top.semi_transparent,
                    };
                    if pixel_candidate_in_front(&candidate, &top) {
                        second = top;
                        top = candidate;
                    } else if pixel_candidate_in_front(&candidate, &second) {
                        second = candidate;
                    }
                }
                if let Some(obj_second) = obj_second {
                    let candidate = PixelCandidate {
                        color: bgr555_to_rgb888(obj_second.color),
                        layer: LAYER_OBJ,
                        priority: obj_second.priority,
                        obj_index: obj_second.obj_index,
                        semi_transparent: obj_second.semi_transparent,
                    };
                    if pixel_candidate_in_front(&candidate, &top) {
                        second = top;
                        top = candidate;
                    } else if pixel_candidate_in_front(&candidate, &second) {
                        second = candidate;
                    }
                }
            }

            let mut top_rgb = top.color;
            if top.layer == LAYER_OBJ
                && top.semi_transparent
                && window_allows_special_effect(window_masks[index])
                && (second_mask & layer_bit(second.layer)) != 0
            {
                top_rgb = blend_rgb888(top_rgb, second.color, eva, evb);
            }

            write_pixel_rgb(pixels, index, top_rgb);
            write_pixel_rgb(second_pixels, index, second.color);
            bg_priorities[index] = top.priority;
            second_bg_priorities[index] = second.priority;
            layer_ids[index] = top.layer;
            second_layer_ids[index] = second.layer;
            obj_semitrans[index] = top.layer == LAYER_OBJ && top.semi_transparent;
        }
    }

    fn apply_color_effects_for_line(
        &self,
        y: u32,
        pixels: &mut [u8],
        layer_ids: &[u8],
        second_pixels: &[u8],
        second_layers: &[u8],
        obj_semitrans: &[bool],
        window_masks: &[u8],
    ) {
        let bldcnt = self.bus.scanline_io_read16(y, 0x50);
        let effect = ((bldcnt >> 6) & 0x3) as u8;
        if effect == 0 {
            return;
        }

        let first_mask = bldcnt & 0x003F;
        let second_mask = (bldcnt >> 8) & 0x003F;
        let (eva, evb) = read_blend_factors(self.bus.scanline_io_read16(y, 0x52));
        let ey = read_brightness_factor(self.bus.scanline_io_read16(y, 0x54));

        for x in 0..GBA_LCD_WIDTH {
            let index = (y * GBA_LCD_WIDTH + x) as usize;
            let wmask = window_masks[index];
            if !window_allows_special_effect(wmask) {
                continue;
            }
            let layer = layer_ids[index];
            let top_bit = layer_bit(layer);
            if (first_mask & top_bit) == 0 {
                continue;
            }
            if obj_semitrans[index] && layer == LAYER_OBJ {
                continue;
            }

            let rgb = read_pixel_rgb(pixels, index);
            let new_rgb = match effect {
                1 => {
                    let under_layer = second_layers[index];
                    if (second_mask & layer_bit(under_layer)) == 0 {
                        continue;
                    }
                    let under_rgb = read_pixel_rgb(second_pixels, index);
                    blend_rgb888(rgb, under_rgb, eva, evb)
                }
                2 => brighten_rgb888(rgb, ey),
                3 => darken_rgb888(rgb, ey),
                _ => continue,
            };
            write_pixel_rgb(pixels, index, new_rgb);
        }
    }

    fn sample_obj_pixels(
        &self,
        line: u32,
        dispcnt: u16,
        x: u32,
        y: u32,
        mosaic: MosaicState,
        obj_attrs: &[ObjAttributes; 128],
        obj_affine: &[ObjAffineParams; 32],
    ) -> (Option<ObjPixel>, Option<ObjPixel>) {
        let mode = dispcnt & 0x7;
        let one_d_mapping = (dispcnt & (1 << 6)) != 0;
        let obj_char_base = if mode >= 3 {
            OBJ_CHAR_BASE_BITMAP
        } else {
            OBJ_CHAR_BASE_TEXT
        };
        let obj_mosaic_h = mosaic.obj_h as i32;
        let obj_mosaic_v = mosaic.obj_v as i32;

        let mut best: Option<ObjPixel> = None;
        let mut second: Option<ObjPixel> = None;
        for (obj_index, attrs) in obj_attrs.iter().enumerate() {
            let attr0 = attrs.attr0;
            let attr1 = attrs.attr1;
            let attr2 = attrs.attr2;

            let affine = (attr0 & (1 << 8)) != 0;
            if !affine && (attr0 & (1 << 9)) != 0 {
                continue;
            }

            let obj_mode = ((attr0 >> 10) & 0x3) as u8;
            if obj_mode == 2 || obj_mode == 3 {
                continue;
            }
            let semi_transparent = obj_mode == 1;
            let mosaic_enabled = (attr0 & (1 << 12)) != 0;

            let shape = ((attr0 >> 14) & 0x3) as u8;
            let size = ((attr1 >> 14) & 0x3) as u8;
            let (width, height) = match obj_dimensions(shape, size) {
                Some(dim) => dim,
                None => continue,
            };

            let x_raw = (attr1 & 0x01FF) as i32;
            let y_raw = (attr0 & 0x00FF) as i32;
            let obj_x = if x_raw >= 240 { x_raw - 512 } else { x_raw };
            let obj_y = if y_raw >= 160 { y_raw - 256 } else { y_raw };

            let color_8bpp = (attr0 & (1 << 13)) != 0;
            let mut tile_index = (attr2 & 0x03FF) as u32;
            let tile_span = if color_8bpp { 2 } else { 1 };

            let (sx, sy) = if affine {
                let double_size = (attr0 & (1 << 9)) != 0;
                let draw_w = if double_size { width * 2 } else { width } as i32;
                let draw_h = if double_size { height * 2 } else { height } as i32;

                let mut rel_x = x as i32 - obj_x;
                let mut rel_y = y as i32 - obj_y;
                if rel_x < 0 || rel_y < 0 || rel_x >= draw_w || rel_y >= draw_h {
                    continue;
                }
                if mosaic_enabled {
                    rel_x -= rel_x % obj_mosaic_h;
                    rel_y -= rel_y % obj_mosaic_v;
                }

                let dx = rel_x - (draw_w / 2);
                let dy = rel_y - (draw_h / 2);
                let affine_index = ((attr1 >> 9) & 0x1F) as usize;
                let params = &obj_affine[affine_index];
                let sx_fp = i64::from(params.pa) * i64::from(dx)
                    + i64::from(params.pb) * i64::from(dy)
                    + i64::from((width as i32 * 256) / 2);
                let sy_fp = i64::from(params.pc) * i64::from(dx)
                    + i64::from(params.pd) * i64::from(dy)
                    + i64::from((height as i32 * 256) / 2);

                if sx_fp < 0
                    || sy_fp < 0
                    || sx_fp >= i64::from(width * 256)
                    || sy_fp >= i64::from(height * 256)
                {
                    continue;
                }

                ((sx_fp >> 8) as u32, (sy_fp >> 8) as u32)
            } else {
                let mut sx = x as i32 - obj_x;
                let mut sy = y as i32 - obj_y;
                if sx < 0 || sy < 0 || sx >= width as i32 || sy >= height as i32 {
                    continue;
                }
                if mosaic_enabled {
                    sx -= sx % obj_mosaic_h;
                    sy -= sy % obj_mosaic_v;
                }

                if (attr1 & (1 << 12)) != 0 {
                    sx = width as i32 - 1 - sx;
                }
                if (attr1 & (1 << 13)) != 0 {
                    sy = height as i32 - 1 - sy;
                }
                (sx as u32, sy as u32)
            };

            if color_8bpp {
                tile_index &= !1;
            }
            let tile_x = sx / 8;
            let tile_y = sy / 8;
            let local_x = sx & 7;
            let local_y = sy & 7;

            let row_step = if one_d_mapping {
                let tiles_w = (width / 8) as u32;
                tiles_w * tile_span
            } else {
                32
            };

            let tile_number = tile_index + tile_y * row_step + tile_x * tile_span;
            let tile_addr = obj_char_base + tile_number * 32;

            let color = if color_8bpp {
                let index = self
                    .bus
                    .scanline_obj_vram_read8(line, tile_addr + local_y * 8 + local_x);
                if index == 0 {
                    continue;
                }
                self.bus
                    .scanline_pram_read16(line, 0x200 + u32::from(index) * 2)
            } else {
                let byte = self
                    .bus
                    .scanline_obj_vram_read8(line, tile_addr + local_y * 4 + (local_x / 2));
                let index = if (local_x & 1) == 0 {
                    byte & 0x0F
                } else {
                    byte >> 4
                };
                if index == 0 {
                    continue;
                }
                let palette_bank = ((attr2 >> 12) & 0x0F) as u16;
                let palette_index = palette_bank * 16 + u16::from(index);
                self.bus
                    .scanline_pram_read16(line, 0x200 + u32::from(palette_index) * 2)
            };

            let candidate = ObjPixel {
                obj_index,
                color,
                priority: ((attr2 >> 10) & 0x3) as u8,
                semi_transparent,
            };
            if best
                .as_ref()
                .is_none_or(|current| obj_pixel_in_front(&candidate, current))
            {
                second = best;
                best = Some(candidate);
            } else if second
                .as_ref()
                .is_none_or(|current| obj_pixel_in_front(&candidate, current))
            {
                second = Some(candidate);
            }
        }

        (best, second)
    }

    fn sample_objwin_hit(
        &self,
        line: u32,
        dispcnt: u16,
        x: u32,
        y: u32,
        mosaic: MosaicState,
        obj_attrs: &[ObjAttributes; 128],
        obj_affine: &[ObjAffineParams; 32],
    ) -> bool {
        let mode = dispcnt & 0x7;
        let one_d_mapping = (dispcnt & (1 << 6)) != 0;
        let obj_char_base = if mode >= 3 {
            OBJ_CHAR_BASE_BITMAP
        } else {
            OBJ_CHAR_BASE_TEXT
        };
        let obj_mosaic_h = mosaic.obj_h as i32;
        let obj_mosaic_v = mosaic.obj_v as i32;

        for attrs in obj_attrs {
            let attr0 = attrs.attr0;
            let attr1 = attrs.attr1;
            let attr2 = attrs.attr2;

            let affine = (attr0 & (1 << 8)) != 0;
            if !affine && (attr0 & (1 << 9)) != 0 {
                continue;
            }

            let obj_mode = ((attr0 >> 10) & 0x3) as u8;
            if obj_mode != 2 {
                continue;
            }
            let mosaic_enabled = (attr0 & (1 << 12)) != 0;

            let shape = ((attr0 >> 14) & 0x3) as u8;
            let size = ((attr1 >> 14) & 0x3) as u8;
            let (width, height) = match obj_dimensions(shape, size) {
                Some(dim) => dim,
                None => continue,
            };

            let x_raw = (attr1 & 0x01FF) as i32;
            let y_raw = (attr0 & 0x00FF) as i32;
            let obj_x = if x_raw >= 240 { x_raw - 512 } else { x_raw };
            let obj_y = if y_raw >= 160 { y_raw - 256 } else { y_raw };

            let color_8bpp = (attr0 & (1 << 13)) != 0;
            let mut tile_index = (attr2 & 0x03FF) as u32;
            let tile_span = if color_8bpp { 2 } else { 1 };

            let (sx, sy) = if affine {
                let double_size = (attr0 & (1 << 9)) != 0;
                let draw_w = if double_size { width * 2 } else { width } as i32;
                let draw_h = if double_size { height * 2 } else { height } as i32;

                let mut rel_x = x as i32 - obj_x;
                let mut rel_y = y as i32 - obj_y;
                if rel_x < 0 || rel_y < 0 || rel_x >= draw_w || rel_y >= draw_h {
                    continue;
                }
                if mosaic_enabled {
                    rel_x -= rel_x % obj_mosaic_h;
                    rel_y -= rel_y % obj_mosaic_v;
                }

                let dx = rel_x - (draw_w / 2);
                let dy = rel_y - (draw_h / 2);
                let affine_index = ((attr1 >> 9) & 0x1F) as usize;
                let params = &obj_affine[affine_index];
                let sx_fp = i64::from(params.pa) * i64::from(dx)
                    + i64::from(params.pb) * i64::from(dy)
                    + i64::from((width as i32 * 256) / 2);
                let sy_fp = i64::from(params.pc) * i64::from(dx)
                    + i64::from(params.pd) * i64::from(dy)
                    + i64::from((height as i32 * 256) / 2);

                if sx_fp < 0
                    || sy_fp < 0
                    || sx_fp >= i64::from(width * 256)
                    || sy_fp >= i64::from(height * 256)
                {
                    continue;
                }

                ((sx_fp >> 8) as u32, (sy_fp >> 8) as u32)
            } else {
                let mut sx = x as i32 - obj_x;
                let mut sy = y as i32 - obj_y;
                if sx < 0 || sy < 0 || sx >= width as i32 || sy >= height as i32 {
                    continue;
                }
                if mosaic_enabled {
                    sx -= sx % obj_mosaic_h;
                    sy -= sy % obj_mosaic_v;
                }

                if (attr1 & (1 << 12)) != 0 {
                    sx = width as i32 - 1 - sx;
                }
                if (attr1 & (1 << 13)) != 0 {
                    sy = height as i32 - 1 - sy;
                }
                (sx as u32, sy as u32)
            };

            if color_8bpp {
                tile_index &= !1;
            }
            let tile_x = sx / 8;
            let tile_y = sy / 8;
            let local_x = sx & 7;
            let local_y = sy & 7;

            let row_step = if one_d_mapping {
                let tiles_w = (width / 8) as u32;
                tiles_w * tile_span
            } else {
                32
            };

            let tile_number = tile_index + tile_y * row_step + tile_x * tile_span;
            let tile_addr = obj_char_base + tile_number * 32;
            let pixel_nonzero = if color_8bpp {
                self.bus
                    .scanline_obj_vram_read8(line, tile_addr + local_y * 8 + local_x)
                    != 0
            } else {
                let byte = self
                    .bus
                    .scanline_obj_vram_read8(line, tile_addr + local_y * 4 + (local_x / 2));
                let index = if (local_x & 1) == 0 {
                    byte & 0x0F
                } else {
                    byte >> 4
                };
                index != 0
            };

            if pixel_nonzero {
                return true;
            }
        }

        false
    }
}

impl EmulatorCore for GbaEmulator {
    fn console_kind(&self) -> ConsoleKind {
        ConsoleKind::Gba
    }

    fn load_rom(&mut self, rom: RomImage) -> EmuResult<()> {
        self.bus.load_rom(rom.bytes());
        self.rom_loaded = true;
        self.reset();
        Ok(())
    }

    fn reset(&mut self) {
        self.bus.reset();
        self.cpu.reset_for_boot(self.bus.has_bios());
        self.ppu.reset();
        let _ = self.ppu.step(0, &mut self.bus);
        self.timer.reset();
        self.frame_number = 0;
    }

    fn step_frame(&mut self) -> EmuResult<FrameResult> {
        if !self.rom_loaded {
            return Err(EmuError::InvalidState("ROM is not loaded"));
        }

        // Snapshot line 0 before the CPU starts executing.  At this point
        // the registers reflect the VBlank handler's setup for this frame,
        // which is exactly what the renderer needs for scanline 0.
        self.snapshot_scanline_renderer_state(0);

        let mut pending_snapshot_line: Option<u32> = None;
        let mut cycles_this_frame = 0;
        while cycles_this_frame < GBA_FRAME_CYCLES {
            let step_cycles = self.cpu.step(&mut self.bus);
            self.timer.step(step_cycles, &mut self.bus);
            let result = self.ppu.step(step_cycles, &mut self.bus);
            cycles_this_frame += step_cycles;

            if result.vcounter_match_entered {
                Self::request_vcount_irq(&mut self.bus);
            }

            if let Some(line) = result.scanline_entered {
                if let Some(prev) = pending_snapshot_line.take() {
                    self.snapshot_scanline_renderer_state(prev as u16);
                }
                let line = u32::from(line);
                if line < GBA_LCD_HEIGHT {
                    pending_snapshot_line = Some(line);
                }
            }

            if result.hblank_entered {
                if let Some(line) = pending_snapshot_line.take() {
                    self.snapshot_scanline_renderer_state(line as u16);
                }
                Self::trigger_hblank_dma_and_irq(&mut self.bus);
            }

            if result.vblank_entered {
                if let Some(line) = pending_snapshot_line.take() {
                    self.snapshot_scanline_renderer_state(line as u16);
                }
                // Snapshot PRAM/VRAM/OAM before VBlank DMA/handler can
                // modify them for the next frame (used by frame_rgba8888).
                self.bus.snapshot_render_state();
                Self::trigger_vblank_dma_and_irq(&mut self.bus);
            }
            if result.frame_ready {
                break;
            }
        }

        self.frame_number += 1;
        Ok(FrameResult {
            cycles: cycles_this_frame,
            frame_number: self.frame_number,
        })
    }
}

impl GbaEmulator {
    fn snapshot_scanline_renderer_state(&mut self, line: u16) {
        self.bus.snapshot_scanline_io(line);
        self.bus.snapshot_scanline_pram(line);
        self.bus.snapshot_scanline_bg_bitmap_vram(line);
        self.bus.snapshot_scanline_obj_vram(line);
        self.bus.snapshot_scanline_oam(line);
    }

    fn request_vcount_irq(bus: &mut GbaBus) {
        let dispstat = bus.dispstat();
        if (dispstat & (1 << 5)) != 0 {
            bus.request_irq(bus::IRQ_VCOUNT);
        }
    }

    fn trigger_hblank_dma_and_irq(bus: &mut GbaBus) {
        let dispstat = bus.dispstat();
        bus.trigger_hblank_dma();
        if (dispstat & (1 << 4)) != 0 {
            bus.request_irq(bus::IRQ_HBLANK);
        }
    }

    fn trigger_vblank_dma_and_irq(bus: &mut GbaBus) {
        let dispstat = bus.dispstat();
        bus.trigger_vblank_dma();
        if (dispstat & (1 << 3)) != 0 {
            bus.request_irq(bus::IRQ_VBLANK);
        }
    }

    /// Step one frame with per-scanline rendering.
    ///
    /// Each visible scanline is rendered late in its draw period using the
    /// live VRAM/PRAM/OAM/IO state from just before HBlank. This keeps
    /// line-start IRQ updates visible while still preventing same-line
    /// HBlank DMA from leaking into the current render.
    pub fn step_frame_with_render(&mut self, frame: &mut GbaFrameBuffer) -> EmuResult<FrameResult> {
        if !self.rom_loaded {
            return Err(EmuError::InvalidState("ROM is not loaded"));
        }

        self.snapshot_scanline_renderer_state(0);
        // Ensure the renderer reads live PRAM/VRAM/OAM (no stale snapshot).
        self.bus.invalidate_render_snapshot();

        // Prepare frame buffers.
        frame.prepare_for_render();

        // Build the line 0 OBJ cache before rendering the first scanline.
        let mut obj_attrs = [ObjAttributes::default(); 128];
        let mut obj_affine = [ObjAffineParams::default(); 32];
        self.build_obj_render_cache_for_line(0, &mut obj_attrs, &mut obj_affine);

        // Render scanline 0 immediately — the VBlank handler has already
        // finished setting up registers for this frame.
        self.render_scanline(frame, 0, &obj_attrs, &obj_affine);

        // Track which visible scanline is waiting to be rendered.
        // We defer rendering until HBlank so line-start IRQ handlers have
        // the draw period to update registers/VRAM/OAM, but the scanline
        // still must be drawn before that same line's HBlank/VBlank DMA.
        let mut pending_render_line: Option<u32> = None;

        let mut cycles_this_frame = 0;
        while cycles_this_frame < GBA_FRAME_CYCLES {
            let step_cycles = self.cpu.step(&mut self.bus);
            self.timer.step(step_cycles, &mut self.bus);
            let result = self.ppu.step(step_cycles, &mut self.bus);
            cycles_this_frame += step_cycles;

            // VCount IRQs should become pending as soon as the line starts,
            // giving the CPU the rest of the draw period to react before
            // we render the scanline at HBlank.
            if result.vcounter_match_entered {
                Self::request_vcount_irq(&mut self.bus);
            }

            // Optional trace for scanline-timed window register debugging.
            if trace_scanline_io_enabled()
                && (result.vcounter_match_entered || result.scanline_entered.is_some())
            {
                let vcount = self.bus.read16(0x0400_0006);
                let win0h = self.bus.read16(0x0400_0040);
                let win0v = self.bus.read16(0x0400_0044);
                let win1h = self.bus.read16(0x0400_0042);
                let win1v = self.bus.read16(0x0400_0046);
                let dispstat = self.bus.read16(0x0400_0004);
                let lyc = (dispstat >> 8) & 0xFF;
                let dispcnt = self.bus.read16(0x0400_0000);
                let bldcnt = self.bus.read16(0x0400_0050);
                if result.vcounter_match_entered {
                    eprintln!(
                        "[dbg] VCOUNT_MATCH vcount={} lyc={} win0h={:#06X} win0v={:#06X} win1h={:#06X} win1v={:#06X} dispcnt={:#06X} bldcnt={:#06X}",
                        vcount, lyc, win0h, win0v, win1h, win1v, dispcnt, bldcnt
                    );
                }
                if let Some(line) = result.scanline_entered {
                    eprintln!(
                        "[dbg] SCANLINE_ENTER line={} lyc={} win0h={:#06X} win0v={:#06X} win1h={:#06X} win1v={:#06X} dispcnt={:#06X}",
                        line, lyc, win0h, win0v, win1h, win1v, dispcnt
                    );
                }
            }

            // When a new visible scanline starts, mark it for deferred render.
            if let Some(line) = result.scanline_entered {
                // If a previous line was pending (missed HBlank — shouldn't
                // normally happen), render it now with current IO state.
                if let Some(prev) = pending_render_line.take() {
                    self.snapshot_scanline_renderer_state(prev as u16);
                    self.build_obj_render_cache_for_line(prev, &mut obj_attrs, &mut obj_affine);
                    self.render_scanline(frame, prev, &obj_attrs, &obj_affine);
                }
                let l = u32::from(line);
                if l < 160 {
                    pending_render_line = Some(l);
                }
            }

            // Render the pending scanline at HBlank time.  By now any
            // VCount IRQ handler that fired at the start of this scanline
            // has had ~960 cycles to update IO registers.
            if result.hblank_entered {
                if let Some(line) = pending_render_line.take() {
                    self.snapshot_scanline_renderer_state(line as u16);
                    self.build_obj_render_cache_for_line(line, &mut obj_attrs, &mut obj_affine);
                    self.render_scanline(frame, line, &obj_attrs, &obj_affine);
                }

                Self::trigger_hblank_dma_and_irq(&mut self.bus);
            }

            if result.vblank_entered {
                // Flush any remaining pending scanline before frame blend.
                if let Some(line) = pending_render_line.take() {
                    self.snapshot_scanline_renderer_state(line as u16);
                    self.build_obj_render_cache_for_line(line, &mut obj_attrs, &mut obj_affine);
                    self.render_scanline(frame, line, &obj_attrs, &obj_affine);
                }

                Self::trigger_vblank_dma_and_irq(&mut self.bus);

                // Apply frame blending after all scanlines have been rendered.
                if frame.frame_blend {
                    for i in 0..frame.pixels.len() {
                        let cur = frame.pixels[i] as u16;
                        let prev = frame.prev_pixels[i] as u16;
                        frame.prev_pixels[i] = frame.pixels[i];
                        frame.pixels[i] = ((cur + prev) / 2) as u8;
                    }
                }
            }

            if result.frame_ready {
                break;
            }
        }

        self.frame_number += 1;
        Ok(FrameResult {
            cycles: cycles_this_frame,
            frame_number: self.frame_number,
        })
    }
}

fn bgr555_to_rgb888(color: u16) -> (u8, u8, u8) {
    let r5 = (color & 0x001F) as u8;
    let g5 = ((color >> 5) & 0x001F) as u8;
    let b5 = ((color >> 10) & 0x001F) as u8;
    (expand_5bit(r5), expand_5bit(g5), expand_5bit(b5))
}

fn expand_5bit(value: u8) -> u8 {
    (value << 3) | (value >> 2)
}

fn write_pixel_rgba8888(pixels: &mut [u8], index: usize, color: u16) {
    let (r, g, b) = bgr555_to_rgb888(color);
    let out = index * 4;
    pixels[out] = r;
    pixels[out + 1] = g;
    pixels[out + 2] = b;
    pixels[out + 3] = 0xFF;
}

fn read_pixel_rgb(pixels: &[u8], index: usize) -> (u8, u8, u8) {
    let out = index * 4;
    (pixels[out], pixels[out + 1], pixels[out + 2])
}

fn write_pixel_rgb(pixels: &mut [u8], index: usize, (r, g, b): (u8, u8, u8)) {
    let out = index * 4;
    pixels[out] = r;
    pixels[out + 1] = g;
    pixels[out + 2] = b;
    pixels[out + 3] = 0xFF;
}

fn read_blend_factors(value: u16) -> (u16, u16) {
    let eva = u16::min(value & 0x001F, 16);
    let evb = u16::min((value >> 8) & 0x001F, 16);
    (eva, evb)
}

fn read_brightness_factor(value: u16) -> u16 {
    u16::min(value & 0x001F, 16)
}

fn blend_rgb888(top: (u8, u8, u8), under: (u8, u8, u8), eva: u16, evb: u16) -> (u8, u8, u8) {
    let blend = |a: u8, b: u8| -> u8 {
        let mixed = (u16::from(a) * eva + u16::from(b) * evb) / 16;
        u16::min(mixed, 255) as u8
    };
    (
        blend(top.0, under.0),
        blend(top.1, under.1),
        blend(top.2, under.2),
    )
}

fn brighten_rgb888(color: (u8, u8, u8), ey: u16) -> (u8, u8, u8) {
    let brighten = |c: u8| -> u8 {
        let value = u16::from(c) + ((u16::from(255 - c) * ey) / 16);
        u16::min(value, 255) as u8
    };
    (brighten(color.0), brighten(color.1), brighten(color.2))
}

fn darken_rgb888(color: (u8, u8, u8), ey: u16) -> (u8, u8, u8) {
    let darken = |c: u8| -> u8 {
        let reduction = (u16::from(c) * ey) / 16;
        (u16::from(c).saturating_sub(reduction)) as u8
    };
    (darken(color.0), darken(color.1), darken(color.2))
}

fn layer_sort_key(layer: &BgLayer) -> (u8, u8) {
    match layer {
        BgLayer::Text(layer) => (layer.priority, layer.bg),
        BgLayer::Affine(layer) => (layer.priority, layer.bg),
    }
}

fn layer_priority(layer: &BgLayer) -> u8 {
    match layer {
        BgLayer::Text(layer) => layer.priority,
        BgLayer::Affine(layer) => layer.priority,
    }
}

fn layer_bg_index(layer: &BgLayer) -> u8 {
    match layer {
        BgLayer::Text(layer) => layer.bg,
        BgLayer::Affine(layer) => layer.bg,
    }
}

fn layer_id_for_bg(bg: u8) -> u8 {
    match bg {
        0 => LAYER_BG0,
        1 => LAYER_BG1,
        2 => LAYER_BG2,
        3 => LAYER_BG3,
        _ => LAYER_BD,
    }
}

fn layer_bit(layer: u8) -> u16 {
    if layer <= LAYER_BD { 1u16 << layer } else { 0 }
}

fn obj_pixel_in_front(candidate: &ObjPixel, other: &ObjPixel) -> bool {
    candidate.priority < other.priority
        || (candidate.priority == other.priority && candidate.obj_index < other.obj_index)
}

fn pixel_candidate_in_front(candidate: &PixelCandidate, other: &PixelCandidate) -> bool {
    if candidate.priority != other.priority {
        return candidate.priority < other.priority;
    }

    let candidate_is_obj = candidate.layer == LAYER_OBJ;
    let other_is_obj = other.layer == LAYER_OBJ;
    if candidate_is_obj != other_is_obj {
        return candidate_is_obj;
    }

    if candidate_is_obj && other_is_obj {
        return candidate.obj_index < other.obj_index;
    }

    candidate.layer < other.layer
}

fn window_allows_layer(window_mask: u8, layer: u8) -> bool {
    (u16::from(window_mask) & layer_bit(layer)) != 0
}

fn window_allows_special_effect(window_mask: u8) -> bool {
    window_allows_layer(window_mask, LAYER_BD)
}

fn window_axis_bounds(value: u16) -> (u32, u32) {
    (((value >> 8) & 0x00FF) as u32, (value & 0x00FF) as u32)
}

fn point_in_window(x: u32, y: u32, x1: u32, x2: u32, y1: u32, y2: u32) -> bool {
    in_window_axis(x, x1, x2) && in_window_axis(y, y1, y2)
}

fn in_window_axis(position: u32, start: u32, end: u32) -> bool {
    if start <= end {
        position >= start && position < end
    } else {
        position >= start || position < end
    }
}

fn text_bg_dimensions(size: u8) -> (u32, u32) {
    match size & 0x3 {
        0 => (256, 256),
        1 => (512, 256),
        2 => (256, 512),
        _ => (512, 512),
    }
}

fn affine_bg_dimension(size: u8) -> u32 {
    match size & 0x3 {
        0 => 128,
        1 => 256,
        2 => 512,
        _ => 1024,
    }
}

fn text_bg_map_entry_addr(screen_base_block: u32, size: u8, tile_x: u32, tile_y: u32) -> u32 {
    let map_base = VRAM_BASE + screen_base_block * 0x800;

    let (block, local_x, local_y) = match size & 0x3 {
        0 => (0, tile_x, tile_y),
        1 => (if tile_x >= 32 { 1 } else { 0 }, tile_x % 32, tile_y),
        2 => (if tile_y >= 32 { 1 } else { 0 }, tile_x, tile_y % 32),
        _ => (
            (if tile_x >= 32 { 1 } else { 0 }) + (if tile_y >= 32 { 2 } else { 0 }),
            tile_x % 32,
            tile_y % 32,
        ),
    };

    map_base + block * 0x800 + ((local_y * 32 + local_x) * 2)
}

fn obj_dimensions(shape: u8, size: u8) -> Option<(u32, u32)> {
    Some(match (shape & 0x3, size & 0x3) {
        (0, 0) => (8, 8),
        (0, 1) => (16, 16),
        (0, 2) => (32, 32),
        (0, 3) => (64, 64),
        (1, 0) => (16, 8),
        (1, 1) => (32, 8),
        (1, 2) => (32, 16),
        (1, 3) => (64, 32),
        (2, 0) => (8, 16),
        (2, 1) => (8, 32),
        (2, 2) => (16, 32),
        (2, 3) => (32, 64),
        _ => return None,
    })
}

fn sign_extend_u32(value: u32, bits: u8) -> i32 {
    let shift = 32_u32.saturating_sub(u32::from(bits));
    ((value << shift) as i32) >> shift
}

fn trace_scanline_io_enabled() -> bool {
    *TRACE_SCANLINE_IO_ENABLED.get_or_init(|| env_flag("GBA_TRACE_SCANLINE_IO"))
}

fn env_flag(name: &str) -> bool {
    let value = match std::env::var(name) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let lowered = value.trim().to_ascii_lowercase();
    !(lowered.is_empty()
        || lowered == "0"
        || lowered == "false"
        || lowered == "off"
        || lowered == "no")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRAM_BASE: u32 = 0x0500_0000;
    const REG_BG0CNT: u32 = 0x0400_0008;
    const REG_BG0HOFS: u32 = 0x0400_0010;
    const REG_WIN0H: u32 = 0x0400_0040;
    const REG_WIN1H: u32 = 0x0400_0042;
    const REG_WIN0V: u32 = 0x0400_0044;
    const REG_WIN1V: u32 = 0x0400_0046;
    const REG_WININ: u32 = 0x0400_0048;
    const REG_WINOUT: u32 = 0x0400_004A;
    const REG_BLDCNT: u32 = 0x0400_0050;
    const REG_BLDALPHA: u32 = 0x0400_0052;
    const REG_BLDY: u32 = 0x0400_0054;

    #[test]
    fn gba_reports_console_kind() {
        let emulator = GbaEmulator::new();
        assert_eq!(emulator.console_kind(), ConsoleKind::Gba);
    }

    #[test]
    fn gba_steps_frame_with_dummy_rom() {
        let mut emulator = GbaEmulator::new();
        let dummy_rom = RomImage::from_bytes(vec![0x00; 512]).expect("dummy ROM should be valid");
        emulator
            .load_rom(dummy_rom)
            .expect("ROM load should succeed");

        let frame = emulator.step_frame().expect("frame should step");
        assert!(frame.cycles > 0);
        assert_eq!(frame.frame_number, 1);
    }

    #[test]
    fn gba_frame_rgba8888_has_expected_size() {
        let mut emulator = GbaEmulator::new();
        let dummy_rom = RomImage::from_bytes(vec![0x00; 512]).expect("dummy ROM should be valid");
        emulator
            .load_rom(dummy_rom)
            .expect("ROM load should succeed");
        emulator.step_frame().expect("frame should step");

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels.len(), (GBA_LCD_WIDTH * GBA_LCD_HEIGHT * 4) as usize);
        assert_eq!(pixels[3], 0xFF);
    }

    #[test]
    fn gba_frame_rgba8888_renders_mode0_bg0_tile_pixel() {
        let mut emulator = GbaEmulator::new();
        let dummy_rom = RomImage::from_bytes(vec![0x00; 512]).expect("dummy ROM should be valid");
        emulator
            .load_rom(dummy_rom)
            .expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 0x0100); // mode 0 + BG0 enable
        emulator.bus.write16(REG_BG0CNT, 0x0000); // charbase 0, screenbase 0, 4bpp
        emulator.bus.write16(VRAM_BASE, 1); // tilemap entry (0,0) -> tile 1
        emulator.bus.write16(PRAM_BASE + 2, 0x001F); // palette color 1 = red
        emulator.bus.write8(VRAM_BASE + 0x20, 0x01); // tile 1, first byte => pixel (0,0)=1

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 0xFF);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0x00);
        assert_eq!(pixels[3], 0xFF);
    }

    #[test]
    fn gba_frame_rgba8888_applies_bg_mosaic_in_mode0() {
        let mut emulator = GbaEmulator::new();
        let dummy_rom = RomImage::from_bytes(vec![0x00; 512]).expect("dummy ROM should be valid");
        emulator
            .load_rom(dummy_rom)
            .expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 0x0100); // mode 0 + BG0 enable
        emulator.bus.write16(REG_BG0CNT, 1 << 6); // BG0 mosaic enable
        emulator.bus.write16(REG_MOSAIC, 0x0001); // BG mosaic H=2, V=1
        emulator.bus.write16(VRAM_BASE, 1); // tilemap entry (0,0) -> tile 1
        emulator.bus.write16(PRAM_BASE + 2, 0x001F); // palette 1 = red
        emulator.bus.write16(PRAM_BASE + 4, 0x03E0); // palette 2 = green
        emulator.bus.write8(VRAM_BASE + 0x20, 0x21); // tile 1: x0=index1, x1=index2

        let pixels = emulator.frame_rgba8888();
        // x=0
        assert_eq!(pixels[0], 0xFF);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0x00);
        // x=1 should replicate x=0 due BG mosaic.
        assert_eq!(pixels[4], 0xFF);
        assert_eq!(pixels[5], 0x00);
        assert_eq!(pixels[6], 0x00);
    }

    #[test]
    fn gba_frame_rgba8888_applies_bg_mosaic_in_mode3() {
        let mut emulator = GbaEmulator::new();
        let dummy_rom = RomImage::from_bytes(vec![0x00; 512]).expect("dummy ROM should be valid");
        emulator
            .load_rom(dummy_rom)
            .expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 0x0003 | (1 << 10)); // mode 3 + BG2 enable
        emulator.bus.write16(REG_BG2CNT, 1 << 6); // BG2 mosaic enable
        emulator.bus.write16(REG_MOSAIC, 0x0001); // BG mosaic H=2, V=1
        emulator.bus.write16(VRAM_BASE, 0x001F); // x0 = red
        emulator.bus.write16(VRAM_BASE + 2, 0x03E0); // x1 = green

        let pixels = emulator.frame_rgba8888();
        // x=0
        assert_eq!(pixels[0], 0xFF);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0x00);
        // x=1 should replicate x=0 due BG mosaic.
        assert_eq!(pixels[4], 0xFF);
        assert_eq!(pixels[5], 0x00);
        assert_eq!(pixels[6], 0x00);
    }

    #[test]
    fn gba_frame_rgba8888_applies_obj_mosaic() {
        let mut emulator = GbaEmulator::new();
        let dummy_rom = RomImage::from_bytes(vec![0x00; 512]).expect("dummy ROM should be valid");
        emulator
            .load_rom(dummy_rom)
            .expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 1 << 12); // OBJ enable
        emulator.bus.write16(REG_MOSAIC, 0x0100); // OBJ mosaic H=2, V=1
        emulator.bus.write16(PRAM_BASE + 0x200 + 2, 0x001F); // palette 1 = red
        emulator.bus.write16(PRAM_BASE + 0x200 + 4, 0x03E0); // palette 2 = green
        emulator.bus.write8(OBJ_CHAR_BASE_TEXT, 0x21); // tile 0: x0=index1, x1=index2

        emulator.bus.write16(OAM_BASE, 1 << 12); // attr0: y=0, mosaic, square, 4bpp
        emulator.bus.write16(OAM_BASE + 2, 0); // attr1: x=0, size=8x8
        emulator.bus.write16(OAM_BASE + 4, 0); // attr2: tile 0

        let pixels = emulator.frame_rgba8888();
        // x=0
        assert_eq!(pixels[0], 0xFF);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0x00);
        // x=1 should replicate x=0 due OBJ mosaic.
        assert_eq!(pixels[4], 0xFF);
        assert_eq!(pixels[5], 0x00);
        assert_eq!(pixels[6], 0x00);
    }

    #[test]
    fn gba_frame_rgba8888_renders_obj_over_backdrop() {
        let mut emulator = GbaEmulator::new();
        let dummy_rom = RomImage::from_bytes(vec![0x00; 512]).expect("dummy ROM should be valid");
        emulator
            .load_rom(dummy_rom)
            .expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 1 << 12); // OBJ enable
        emulator.bus.write16(PRAM_BASE + 0x200 + 2, 0x03E0); // OBJ palette index 1 = green
        emulator.bus.write8(OBJ_CHAR_BASE_TEXT, 0x01); // first OBJ tile pixel (0,0)=index1

        emulator.bus.write16(OAM_BASE, 0); // attr0: y=0, square, 4bpp
        emulator.bus.write16(OAM_BASE + 2, 0); // attr1: x=0, size=8x8
        emulator.bus.write16(OAM_BASE + 4, 0); // attr2: tile 0, priority 0, palette 0

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 0x00);
        assert_eq!(pixels[1], 0xFF);
        assert_eq!(pixels[2], 0x00);
        assert_eq!(pixels[3], 0xFF);
    }

    #[test]
    fn gba_frame_rgba8888_renders_mode1_bg2_affine_pixel() {
        let mut emulator = GbaEmulator::new();
        let dummy_rom = RomImage::from_bytes(vec![0x00; 512]).expect("dummy ROM should be valid");
        emulator
            .load_rom(dummy_rom)
            .expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 0x0001 | (1 << 10)); // mode 1 + BG2 enable
        emulator.bus.write16(REG_BG2CNT, 0x0000); // priority 0, charbase 0, screenbase 0
        emulator.bus.write16(REG_BG2PA, 0x0100); // affine identity
        emulator.bus.write16(REG_BG2PD, 0x0100);

        emulator.bus.write8(VRAM_BASE, 1); // map tile index at (0,0)
        emulator.bus.write8(VRAM_BASE + 0x40, 1); // tile 1, pixel (0,0)=palette 1
        emulator.bus.write16(PRAM_BASE + 2, 0x7C00); // palette 1 = blue

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 0x00);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0xFF);
        assert_eq!(pixels[3], 0xFF);
    }

    #[test]
    fn gba_frame_rgba8888_renders_mode2_bg3_affine_pixel() {
        let mut emulator = GbaEmulator::new();
        let dummy_rom = RomImage::from_bytes(vec![0x00; 512]).expect("dummy ROM should be valid");
        emulator
            .load_rom(dummy_rom)
            .expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 0x0002 | (1 << 11)); // mode 2 + BG3 enable
        emulator.bus.write16(REG_BG2CNT + 2, 0x0000); // BG3CNT
        emulator.bus.write16(REG_BG3PA, 0x0100); // affine identity
        emulator.bus.write16(REG_BG3PD, 0x0100);

        emulator.bus.write8(VRAM_BASE, 1); // map tile index at (0,0)
        emulator.bus.write8(VRAM_BASE + 0x40, 1); // tile 1, pixel (0,0)=palette 1
        emulator.bus.write16(PRAM_BASE + 2, 0x03E0); // palette 1 = green

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 0x00);
        assert_eq!(pixels[1], 0xFF);
        assert_eq!(pixels[2], 0x00);
        assert_eq!(pixels[3], 0xFF);
    }

    #[test]
    fn gba_frame_rgba8888_renders_affine_obj_over_backdrop() {
        let mut emulator = GbaEmulator::new();
        let dummy_rom = RomImage::from_bytes(vec![0x00; 512]).expect("dummy ROM should be valid");
        emulator
            .load_rom(dummy_rom)
            .expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 1 << 12); // OBJ enable
        emulator.bus.write16(PRAM_BASE + 0x200 + 2, 0x001F); // OBJ palette index 1 = red
        emulator.bus.write8(OBJ_CHAR_BASE_TEXT, 0x01); // tile 0 pixel (0,0)=index1

        emulator.bus.write16(OAM_BASE, 1 << 8); // attr0: y=0, affine, square, 4bpp
        emulator.bus.write16(OAM_BASE + 2, 0); // attr1: x=0, affine index 0, size=8x8
        emulator.bus.write16(OAM_BASE + 4, 0); // attr2: tile 0, priority 0, palette 0

        // Affine matrix #0 = identity.
        emulator.bus.write16(OAM_BASE + 0x06, 0x0100); // pa
        emulator.bus.write16(OAM_BASE + 0x0E, 0x0000); // pb
        emulator.bus.write16(OAM_BASE + 0x16, 0x0000); // pc
        emulator.bus.write16(OAM_BASE + 0x1E, 0x0100); // pd

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 0xFF);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0x00);
        assert_eq!(pixels[3], 0xFF);
    }

    #[test]
    fn gba_frame_rgba8888_blends_semitransparent_obj_with_backdrop() {
        let mut emulator = GbaEmulator::new();
        let dummy_rom = RomImage::from_bytes(vec![0x00; 512]).expect("dummy ROM should be valid");
        emulator
            .load_rom(dummy_rom)
            .expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 1 << 12); // OBJ enable
        for obj_index in 0..128u32 {
            emulator.bus.write16(OAM_BASE + obj_index * 8, 1 << 9); // hide unused OBJ
        }
        emulator.bus.write16(PRAM_BASE, 0x7C00); // backdrop blue
        emulator.bus.write16(PRAM_BASE + 0x200 + 2, 0x001F); // OBJ palette index 1 = red
        emulator.bus.write8(OBJ_CHAR_BASE_TEXT, 0x01); // tile 0 pixel (0,0)=index1

        emulator.bus.write16(REG_BLDCNT, (1 << 4) | (1 << 13)); // OBJ first target, backdrop second
        emulator.bus.write16(REG_BLDALPHA, 0x0808); // 50% / 50%

        emulator.bus.write16(OAM_BASE, 1 << 10); // attr0: y=0, semi-transparent OBJ
        emulator.bus.write16(OAM_BASE + 2, 0); // attr1: x=0, size=8x8
        emulator.bus.write16(OAM_BASE + 4, 0); // attr2: tile 0, priority 0, palette 0

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 127);
        assert_eq!(pixels[1], 0);
        assert_eq!(pixels[2], 127);
        assert_eq!(pixels[3], 0xFF);
    }

    #[test]
    fn gba_frame_rgba8888_applies_brighten_effect_to_bg() {
        let mut emulator = GbaEmulator::new();
        let dummy_rom = RomImage::from_bytes(vec![0x00; 512]).expect("dummy ROM should be valid");
        emulator
            .load_rom(dummy_rom)
            .expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 0x0100); // mode 0 + BG0 enable
        emulator.bus.write16(REG_BG0CNT, 0x0000); // charbase 0, screenbase 0, 4bpp
        emulator.bus.write16(VRAM_BASE, 1); // tilemap entry (0,0) -> tile 1
        emulator.bus.write16(PRAM_BASE + 2, 0x0010); // palette color 1 = medium red
        emulator.bus.write8(VRAM_BASE + 0x20, 0x01); // tile 1, first byte => pixel (0,0)=1

        emulator.bus.write16(REG_BLDCNT, 0x0081); // effect=brighten, BG0 first target
        emulator.bus.write16(REG_BLDY, 0x0008); // brighten by 8/16

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 193);
        assert_eq!(pixels[1], 127);
        assert_eq!(pixels[2], 127);
        assert_eq!(pixels[3], 0xFF);
    }

    #[test]
    fn gba_frame_rgba8888_blends_bg0_with_bg1_when_alpha_enabled() {
        let mut emulator = GbaEmulator::new();
        let dummy_rom = RomImage::from_bytes(vec![0x00; 512]).expect("dummy ROM should be valid");
        emulator
            .load_rom(dummy_rom)
            .expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 0x0300); // mode 0 + BG0/BG1 enable
        emulator.bus.write16(REG_BG0CNT, 0x0000); // BG0 priority 0, screenbase 0, 4bpp
        emulator.bus.write16(REG_BG0CNT + 2, 0x0101); // BG1 priority 1, screenbase 1, 4bpp

        emulator.bus.write16(VRAM_BASE, 1); // BG0 map entry -> tile 1
        emulator.bus.write16(VRAM_BASE + 0x800, 2); // BG1 map entry -> tile 2
        emulator.bus.write8(VRAM_BASE + 0x20, 0x01); // tile 1 pixel (0,0)=palette 1
        emulator.bus.write8(VRAM_BASE + 0x40, 0x02); // tile 2 pixel (0,0)=palette 2
        emulator.bus.write16(PRAM_BASE + 2, 0x001F); // palette 1 = red
        emulator.bus.write16(PRAM_BASE + 4, 0x7C00); // palette 2 = blue

        // Effect 1 (alpha): BG0 as first target, BG1 as second target.
        emulator
            .bus
            .write16(REG_BLDCNT, (1 << 0) | (1 << 6) | (1 << 9));
        emulator.bus.write16(REG_BLDALPHA, 0x0808); // 50% / 50%

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 127);
        assert_eq!(pixels[1], 0);
        assert_eq!(pixels[2], 127);
        assert_eq!(pixels[3], 0xFF);
    }

    #[test]
    fn gba_frame_rgba8888_blends_bg_with_obj_as_second_target() {
        let mut emulator = GbaEmulator::new();
        let dummy_rom = RomImage::from_bytes(vec![0x00; 512]).expect("dummy ROM should be valid");
        emulator
            .load_rom(dummy_rom)
            .expect("ROM load should succeed");

        // mode0 + BG0 + OBJ
        emulator.bus.write16(REG_DISPCNT, (1 << 8) | (1 << 12));
        for obj_index in 0..128u32 {
            emulator.bus.write16(OAM_BASE + obj_index * 8, 1 << 9); // hide unused OBJ
        }
        emulator.bus.write16(REG_BG0CNT, 0x0000); // BG0 priority 0

        // BG0 top pixel = red.
        emulator.bus.write16(VRAM_BASE, 1); // map entry -> tile 1
        emulator.bus.write8(VRAM_BASE + 0x20, 0x01); // tile 1 pixel -> palette 1
        emulator.bus.write16(PRAM_BASE + 2, 0x001F); // BG palette 1 = red

        // OBJ behind BG (priority 1), pixel = blue.
        emulator.bus.write16(PRAM_BASE + 0x200 + 2, 0x7C00); // OBJ palette 1 = blue
        emulator.bus.write8(OBJ_CHAR_BASE_TEXT, 0x01); // OBJ tile 0 pixel -> palette 1
        emulator.bus.write16(OAM_BASE, 0); // y=0
        emulator.bus.write16(OAM_BASE + 2, 0); // x=0, 8x8
        emulator.bus.write16(OAM_BASE + 4, 1 << 10); // priority 1

        // Alpha blend: BG0 first target, OBJ second target.
        emulator
            .bus
            .write16(REG_BLDCNT, (1 << 0) | (1 << 6) | (1 << 12));
        emulator.bus.write16(REG_BLDALPHA, 0x0808); // 50% / 50%

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 127);
        assert_eq!(pixels[1], 0);
        assert_eq!(pixels[2], 127);
        assert_eq!(pixels[3], 0xFF);
    }

    #[test]
    fn frame_rgba8888_latches_bg_priority_per_scanline() {
        let mut emulator = GbaEmulator::new();
        let nop_rom = RomImage::from_bytes(vec![0x00, 0x00, 0xA0, 0xE1].repeat(128))
            .expect("NOP ROM should be valid");
        emulator.load_rom(nop_rom).expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 0x0300); // mode 0 + BG0/BG1
        emulator.bus.write16(REG_BG0CNT, 0x0000); // BG0 priority 0, screenbase 0
        emulator.bus.write16(REG_BG0CNT + 2, 0x0101); // BG1 priority 1, screenbase 1

        emulator.bus.write16(VRAM_BASE, 1); // BG0 map -> tile 1
        emulator.bus.write16(VRAM_BASE + 0x800, 2); // BG1 map -> tile 2
        for row in 0..8u32 {
            emulator.bus.write8(VRAM_BASE + 0x20 + row * 4, 0x11); // tile 1 -> palette 1
            emulator.bus.write8(VRAM_BASE + 0x40 + row * 4, 0x22); // tile 2 -> palette 2
        }
        emulator.bus.write16(PRAM_BASE + 2, 0x001F); // red
        emulator.bus.write16(PRAM_BASE + 4, 0x7C00); // blue

        // At line 0 HBlank swap priorities so BG1 wins from line 1 onward.
        emulator.bus.write16(0x0200_0000, 0x0001); // BG0 priority 1
        emulator.bus.write16(0x0200_0002, 0x0100); // BG1 priority 0
        emulator.bus.write32(0x0400_00D4, 0x0200_0000);
        emulator.bus.write32(0x0400_00D8, REG_BG0CNT);
        emulator.bus.write16(0x0400_00DC, 2);
        emulator.bus.write16(0x0400_00DE, 0xA200); // enable + HBlank + repeat

        emulator.step_frame().expect("frame should step");

        assert_eq!(emulator.debug_scanline_io_read16(0, 0x08), 0x0000);
        assert_eq!(emulator.debug_scanline_io_read16(0, 0x0A), 0x0101);
        assert_eq!(emulator.debug_scanline_io_read16(1, 0x08), 0x0001);
        assert_eq!(emulator.debug_scanline_io_read16(1, 0x0A), 0x0100);

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 0xFF);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0x00);
        assert_eq!(pixels[3], 0xFF);

        let line1 = (GBA_LCD_WIDTH * 4) as usize;
        assert_eq!(pixels[line1], 0x00);
        assert_eq!(pixels[line1 + 1], 0x00);
        assert_eq!(pixels[line1 + 2], 0xFF);
        assert_eq!(pixels[line1 + 3], 0xFF);
    }

    #[test]
    fn frame_rgba8888_latches_blend_registers_per_scanline() {
        let mut emulator = GbaEmulator::new();
        let nop_rom = RomImage::from_bytes(vec![0x00, 0x00, 0xA0, 0xE1].repeat(128))
            .expect("NOP ROM should be valid");
        emulator.load_rom(nop_rom).expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 0x0300); // mode 0 + BG0/BG1
        emulator.bus.write16(REG_BG0CNT, 0x0000); // BG0 priority 0
        emulator.bus.write16(REG_BG0CNT + 2, 0x0101); // BG1 priority 1, screenbase 1

        emulator.bus.write16(VRAM_BASE, 1); // BG0 map -> tile 1
        emulator.bus.write16(VRAM_BASE + 0x800, 2); // BG1 map -> tile 2
        for row in 0..8u32 {
            emulator.bus.write8(VRAM_BASE + 0x20 + row * 4, 0x11); // tile 1 -> palette 1
            emulator.bus.write8(VRAM_BASE + 0x40 + row * 4, 0x22); // tile 2 -> palette 2
        }
        emulator.bus.write16(PRAM_BASE + 2, 0x001F); // red
        emulator.bus.write16(PRAM_BASE + 4, 0x7C00); // blue
        emulator.bus.write16(REG_BLDCNT, 0x0000);
        emulator.bus.write16(REG_BLDALPHA, 0x0000);

        // At line 0 HBlank enable alpha blend BG0(first) + BG1(second).
        emulator
            .bus
            .write16(0x0200_0000, (1 << 0) | (1 << 6) | (1 << 9));
        emulator.bus.write16(0x0200_0002, 0x0808);
        emulator.bus.write32(0x0400_00D4, 0x0200_0000);
        emulator.bus.write32(0x0400_00D8, REG_BLDCNT);
        emulator.bus.write16(0x0400_00DC, 2);
        emulator.bus.write16(0x0400_00DE, 0xA200); // enable + HBlank + repeat

        emulator.step_frame().expect("frame should step");

        assert_eq!(emulator.debug_scanline_io_read16(0, 0x50), 0x0000);
        assert_eq!(emulator.debug_scanline_io_read16(0, 0x52), 0x0000);
        assert_eq!(
            emulator.debug_scanline_io_read16(1, 0x50),
            (1 << 0) | (1 << 6) | (1 << 9)
        );
        assert_eq!(emulator.debug_scanline_io_read16(1, 0x52), 0x0808);

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 0xFF);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0x00);
        assert_eq!(pixels[3], 0xFF);

        let line1 = (GBA_LCD_WIDTH * 4) as usize;
        assert_eq!(pixels[line1], 127);
        assert_eq!(pixels[line1 + 1], 0);
        assert_eq!(pixels[line1 + 2], 127);
        assert_eq!(pixels[line1 + 3], 0xFF);
    }

    #[test]
    fn frame_rgba8888_latches_objwin_mask_per_scanline() {
        let mut emulator = GbaEmulator::new();
        let nop_rom = RomImage::from_bytes(vec![0x00, 0x00, 0xA0, 0xE1].repeat(128))
            .expect("NOP ROM should be valid");
        emulator.load_rom(nop_rom).expect("ROM load should succeed");

        emulator
            .bus
            .write16(REG_DISPCNT, (1 << 8) | (1 << 12) | (1 << 15)); // mode0 + BG0 + OBJ + OBJWIN
        emulator.bus.write16(REG_BG0CNT, 0x0000);
        for map_index in 0..(32u32 * 32) {
            emulator.bus.write16(VRAM_BASE + map_index * 2, 1);
        }
        for row in 0..8u32 {
            emulator.bus.write8(VRAM_BASE + 0x20 + row * 4, 0x11);
            emulator.bus.write8(OBJ_CHAR_BASE_TEXT + row * 4, 0x11);
        }
        emulator.bus.write16(PRAM_BASE + 2, 0x001F); // BG0 = red
        emulator.bus.write16(PRAM_BASE, 0x7C00); // backdrop = blue

        emulator.bus.write16(REG_WINOUT, 0x0000);

        emulator.bus.write16(OAM_BASE, 2 << 10); // OBJWIN sprite
        emulator.bus.write16(OAM_BASE + 2, 0x0000);
        emulator.bus.write16(OAM_BASE + 4, 0x0000);

        // Enable BG0 inside OBJWIN after line 0 so the change starts on line 1.
        emulator.bus.write16(0x0200_0000, 0x0100);
        emulator.bus.write32(0x0400_00D4, 0x0200_0000);
        emulator.bus.write32(0x0400_00D8, REG_WINOUT);
        emulator.bus.write16(0x0400_00DC, 1);
        emulator.bus.write16(0x0400_00DE, 0xA000); // enable + HBlank

        emulator.step_frame().expect("frame should step");

        assert_eq!(emulator.debug_scanline_io_read16(0, 0x4A), 0x0000);
        assert_eq!(emulator.debug_scanline_io_read16(1, 0x4A), 0x0100);

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 0x00);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0xFF);
        assert_eq!(pixels[3], 0xFF);

        let line1 = (GBA_LCD_WIDTH * 4) as usize;
        assert_eq!(pixels[line1], 0xFF);
        assert_eq!(pixels[line1 + 1], 0x00);
        assert_eq!(pixels[line1 + 2], 0x00);
        assert_eq!(pixels[line1 + 3], 0xFF);
    }

    #[test]
    fn frame_rgba8888_latches_objwin_bldy_per_scanline() {
        let mut emulator = GbaEmulator::new();
        let nop_rom = RomImage::from_bytes(vec![0x00, 0x00, 0xA0, 0xE1].repeat(128))
            .expect("NOP ROM should be valid");
        emulator.load_rom(nop_rom).expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, (1 << 8) | (1 << 15)); // mode0 + BG0 + OBJWIN
        emulator.bus.write16(REG_BG0CNT, 0x0000);
        for map_index in 0..(32u32 * 32) {
            emulator.bus.write16(VRAM_BASE + map_index * 2, 1);
        }
        for row in 0..8u32 {
            emulator.bus.write8(VRAM_BASE + 0x20 + row * 4, 0x11);
            emulator.bus.write8(OBJ_CHAR_BASE_TEXT + row * 4, 0x11);
        }
        emulator.bus.write16(PRAM_BASE + 2, 0x0010); // medium red
        emulator.bus.write16(REG_BLDCNT, 0x0081); // brighten BG0
        emulator.bus.write16(REG_BLDY, 0x0000);
        emulator
            .bus
            .write16(REG_WINOUT, (1 << 0) | ((1 << 0 | 1 << 5) << 8));

        emulator.bus.write16(OAM_BASE, 2 << 10); // OBJWIN sprite
        emulator.bus.write16(OAM_BASE + 2, 0x0000);
        emulator.bus.write16(OAM_BASE + 4, 0x0000);

        // Raise BLDY after line 0; only line 1 inside OBJWIN should brighten.
        emulator.bus.write16(0x0200_0000, 0x0008);
        emulator.bus.write32(0x0400_00D4, 0x0200_0000);
        emulator.bus.write32(0x0400_00D8, REG_BLDY);
        emulator.bus.write16(0x0400_00DC, 1);
        emulator.bus.write16(0x0400_00DE, 0xA000); // enable + HBlank

        emulator.step_frame().expect("frame should step");

        assert_eq!(emulator.debug_scanline_io_read16(0, 0x54), 0x0000);
        assert_eq!(emulator.debug_scanline_io_read16(1, 0x54), 0x0008);

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 132);
        assert_eq!(pixels[1], 0);
        assert_eq!(pixels[2], 0);
        assert_eq!(pixels[3], 0xFF);

        let line1 = (GBA_LCD_WIDTH * 4) as usize;
        assert_eq!(pixels[line1], 193);
        assert_eq!(pixels[line1 + 1], 127);
        assert_eq!(pixels[line1 + 2], 127);
        assert_eq!(pixels[line1 + 3], 0xFF);

        let outside_line1 = ((GBA_LCD_WIDTH + 20) * 4) as usize;
        assert_eq!(pixels[outside_line1], 132);
        assert_eq!(pixels[outside_line1 + 1], 0);
        assert_eq!(pixels[outside_line1 + 2], 0);
        assert_eq!(pixels[outside_line1 + 3], 0xFF);
    }

    #[test]
    fn frame_rgba8888_latches_blend_triplets_per_scanline() {
        let mut emulator = GbaEmulator::new();
        let nop_rom = RomImage::from_bytes(vec![0x00, 0x00, 0xA0, 0xE1].repeat(128))
            .expect("NOP ROM should be valid");
        emulator.load_rom(nop_rom).expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 0x0300); // mode0 + BG0/BG1
        emulator.bus.write16(REG_BG0CNT, 0x0000); // BG0 priority 0
        emulator.bus.write16(REG_BG0CNT + 2, 0x0101); // BG1 priority 1, screenbase 1

        emulator.bus.write16(VRAM_BASE, 1); // BG0 map -> tile 1
        emulator.bus.write16(VRAM_BASE + 0x800, 2); // BG1 map -> tile 2
        for row in 0..8u32 {
            emulator.bus.write8(VRAM_BASE + 0x20 + row * 4, 0x11); // tile 1 -> palette 1
            emulator.bus.write8(VRAM_BASE + 0x40 + row * 4, 0x22); // tile 2 -> palette 2
        }
        emulator.bus.write16(PRAM_BASE + 2, 0x001F); // red
        emulator.bus.write16(PRAM_BASE + 4, 0x7C00); // blue
        emulator.bus.write16(REG_BLDCNT, 0x0000);
        emulator.bus.write16(REG_BLDALPHA, 0x0000);
        emulator.bus.write16(REG_BLDY, 0x0000);

        let alpha_bldcnt = (1 << 0) | (1 << 6) | (1 << 9);
        let brighten_bldcnt = 0x0081;
        for line in 0..GBA_LCD_HEIGHT {
            let base = 0x0200_0000 + u32::from(line) * 6;
            let (bldcnt, bldalpha, bldy) = if line == 0 {
                (alpha_bldcnt, 0x0808, 0x0000)
            } else {
                (brighten_bldcnt, 0x0000, 0x0008)
            };
            emulator.bus.write16(base, bldcnt);
            emulator.bus.write16(base + 2, bldalpha);
            emulator.bus.write16(base + 4, bldy);
        }

        emulator.bus.write32(0x0400_00D4, 0x0200_0000);
        emulator.bus.write32(0x0400_00D8, REG_BLDCNT);
        emulator.bus.write16(0x0400_00DC, 3);
        emulator.bus.write16(0x0400_00DE, 0xA260); // enable + HBlank + repeat + dest increment/reload

        emulator.step_frame().expect("frame should step");

        assert_eq!(emulator.debug_scanline_io_read16(0, 0x50), 0x0000);
        assert_eq!(emulator.debug_scanline_io_read16(0, 0x52), 0x0000);
        assert_eq!(emulator.debug_scanline_io_read16(0, 0x54), 0x0000);
        assert_eq!(emulator.debug_scanline_io_read16(1, 0x50), alpha_bldcnt);
        assert_eq!(emulator.debug_scanline_io_read16(1, 0x52), 0x0808);
        assert_eq!(emulator.debug_scanline_io_read16(1, 0x54), 0x0000);
        assert_eq!(emulator.debug_scanline_io_read16(2, 0x50), brighten_bldcnt);
        assert_eq!(emulator.debug_scanline_io_read16(2, 0x52), 0x0000);
        assert_eq!(emulator.debug_scanline_io_read16(2, 0x54), 0x0008);

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 0xFF);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0x00);
        assert_eq!(pixels[3], 0xFF);

        let line1 = (GBA_LCD_WIDTH * 4) as usize;
        assert_eq!(pixels[line1], 127);
        assert_eq!(pixels[line1 + 1], 0);
        assert_eq!(pixels[line1 + 2], 127);
        assert_eq!(pixels[line1 + 3], 0xFF);

        let line2 = (2 * GBA_LCD_WIDTH * 4) as usize;
        assert_eq!(pixels[line2], 0xFF);
        assert_eq!(pixels[line2 + 1], 127);
        assert_eq!(pixels[line2 + 2], 127);
        assert_eq!(pixels[line2 + 3], 0xFF);
    }

    #[test]
    fn frame_rgba8888_latches_obj_priority_and_blend_per_scanline() {
        let mut emulator = GbaEmulator::new();
        let nop_rom = RomImage::from_bytes(vec![0x00, 0x00, 0xA0, 0xE1].repeat(128))
            .expect("NOP ROM should be valid");
        emulator.load_rom(nop_rom).expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 1 << 12); // OBJ only
        for obj_index in 0..128u32 {
            emulator.bus.write16(OAM_BASE + obj_index * 8, 1 << 9); // hide unused OBJ
        }
        emulator.bus.write16(PRAM_BASE + 0x200 + 2, 0x001F); // OBJ palette 1 = red
        emulator.bus.write16(PRAM_BASE + 0x200 + 4, 0x7C00); // OBJ palette 2 = blue
        for row in 0..8u32 {
            emulator.bus.write8(OBJ_CHAR_BASE_TEXT + row * 4, 0x11); // tile 0 -> palette 1
            emulator
                .bus
                .write8(OBJ_CHAR_BASE_TEXT + 0x20 + row * 4, 0x22); // tile 1 -> palette 2
        }

        // OBJ0: semi-transparent red, starts behind OBJ1.
        emulator.bus.write16(OAM_BASE, 1 << 10); // y=0, semi-transparent OBJ
        emulator.bus.write16(OAM_BASE + 2, 0x0000); // x=0, 8x8
        emulator.bus.write16(OAM_BASE + 4, 1 << 10); // tile 0, priority 1

        // OBJ1: normal blue, starts in front.
        emulator.bus.write16(OAM_BASE + 8, 0x0000); // y=0
        emulator.bus.write16(OAM_BASE + 10, 0x0000); // x=0
        emulator.bus.write16(OAM_BASE + 12, 1); // tile 1, priority 0

        emulator.bus.write16(REG_BLDCNT, 0x0000);
        emulator.bus.write16(REG_BLDALPHA, 0x0000);

        // At line 0 HBlank, move OBJ0 to priority 0 and enable OBJ alpha blending.
        emulator.bus.write16(0x0200_0000, 0x0000); // OBJ0 attr2 -> tile 0, priority 0
        emulator.bus.write32(0x0400_00D4, 0x0200_0000);
        emulator.bus.write32(0x0400_00D8, OAM_BASE + 4);
        emulator.bus.write16(0x0400_00DC, 1);
        emulator.bus.write16(0x0400_00DE, 0xA000); // enable + HBlank

        emulator.bus.write16(0x0200_0010, 1 << 12); // second target: OBJ
        emulator.bus.write16(0x0200_0012, 0x0808); // 50% / 50%
        emulator.bus.write32(0x0400_00C8, 0x0200_0010);
        emulator.bus.write32(0x0400_00CC, REG_BLDCNT);
        emulator.bus.write16(0x0400_00D0, 2);
        emulator.bus.write16(0x0400_00D2, 0xA000); // enable + HBlank

        emulator.step_frame().expect("frame should step");

        assert_eq!(emulator.debug_scanline_oam_read16(0, OAM_BASE + 4), 1 << 10);
        assert_eq!(emulator.debug_scanline_oam_read16(1, OAM_BASE + 4), 0x0000);
        assert_eq!(emulator.debug_scanline_io_read16(0, 0x50), 0x0000);
        assert_eq!(emulator.debug_scanline_io_read16(0, 0x52), 0x0000);
        assert_eq!(emulator.debug_scanline_io_read16(1, 0x50), 1 << 12);
        assert_eq!(emulator.debug_scanline_io_read16(1, 0x52), 0x0808);

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 0x00);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0xFF);
        assert_eq!(pixels[3], 0xFF);

        let line1 = (GBA_LCD_WIDTH * 4) as usize;
        assert_eq!(pixels[line1], 127);
        assert_eq!(pixels[line1 + 1], 0);
        assert_eq!(pixels[line1 + 2], 127);
        assert_eq!(pixels[line1 + 3], 0xFF);
    }

    #[test]
    fn frame_rgba8888_latches_bg_obj_priority_and_blend_per_scanline() {
        let mut emulator = GbaEmulator::new();
        let nop_rom = RomImage::from_bytes(vec![0x00, 0x00, 0xA0, 0xE1].repeat(128))
            .expect("NOP ROM should be valid");
        emulator.load_rom(nop_rom).expect("ROM load should succeed");

        emulator
            .bus
            .write16(REG_DISPCNT, (1 << 8) | (1 << 9) | (1 << 12)); // mode0 + BG0 + BG1 + OBJ
        for obj_index in 0..128u32 {
            emulator.bus.write16(OAM_BASE + obj_index * 8, 1 << 9); // hide unused OBJ
        }

        emulator.bus.write16(REG_BG0CNT, 0x0001); // BG0 priority 1, screenbase 0
        emulator.bus.write16(REG_BG0CNT + 2, 0x0100); // BG1 priority 0, screenbase 1
        emulator.bus.write16(VRAM_BASE, 1); // BG0 map -> tile 1
        emulator.bus.write16(VRAM_BASE + 0x800, 2); // BG1 map -> tile 2
        for row in 0..8u32 {
            emulator.bus.write8(VRAM_BASE + 0x20 + row * 4, 0x11); // tile 1 -> palette 1
            emulator.bus.write8(VRAM_BASE + 0x40 + row * 4, 0x22); // tile 2 -> palette 2
            emulator.bus.write8(OBJ_CHAR_BASE_TEXT + row * 4, 0x33); // tile 0 -> palette 3
        }
        emulator.bus.write16(PRAM_BASE + 2, 0x001F); // BG0 = red
        emulator.bus.write16(PRAM_BASE + 4, 0x7C00); // BG1 = blue
        emulator.bus.write16(PRAM_BASE + 0x200 + 6, 0x03E0); // OBJ palette 3 = green

        // Semi-transparent OBJ starts behind BG1.
        emulator.bus.write16(OAM_BASE, 1 << 10); // y=0, semi-transparent OBJ
        emulator.bus.write16(OAM_BASE + 2, 0x0000); // x=0
        emulator.bus.write16(OAM_BASE + 4, 0x0800); // tile 0, priority 2

        emulator.bus.write16(REG_BLDCNT, 0x0000);
        emulator.bus.write16(REG_BLDALPHA, 0x0000);

        // At line 0 HBlank:
        // - BG0 moves ahead of BG1
        // - OBJ moves ahead of BGs
        // - semi-transparent OBJ blends with BG0 underneath
        emulator.bus.write16(0x0200_0000, 0x0000); // BG0 priority 0
        emulator.bus.write32(0x0400_00D4, 0x0200_0000);
        emulator.bus.write32(0x0400_00D8, REG_BG0CNT);
        emulator.bus.write16(0x0400_00DC, 1);
        emulator.bus.write16(0x0400_00DE, 0xA000); // DMA3 enable + HBlank

        emulator.bus.write16(0x0200_0010, 0x0000); // OBJ attr2 -> priority 0
        emulator.bus.write32(0x0400_00C8, 0x0200_0010);
        emulator.bus.write32(0x0400_00CC, OAM_BASE + 4);
        emulator.bus.write16(0x0400_00D0, 1);
        emulator.bus.write16(0x0400_00D2, 0xA000); // DMA2 enable + HBlank

        emulator.bus.write16(0x0200_0020, 1 << 8); // BG0 as second target
        emulator.bus.write16(0x0200_0022, 0x0808); // 50% / 50%
        emulator.bus.write32(0x0400_00BC, 0x0200_0020);
        emulator.bus.write32(0x0400_00C0, REG_BLDCNT);
        emulator.bus.write16(0x0400_00C4, 2);
        emulator.bus.write16(0x0400_00C6, 0xA000); // DMA1 enable + HBlank

        emulator.step_frame().expect("frame should step");

        assert_eq!(emulator.debug_scanline_io_read16(0, 0x08), 0x0001);
        assert_eq!(emulator.debug_scanline_io_read16(1, 0x08), 0x0000);
        assert_eq!(emulator.debug_scanline_oam_read16(0, OAM_BASE + 4), 0x0800);
        assert_eq!(emulator.debug_scanline_oam_read16(1, OAM_BASE + 4), 0x0000);
        assert_eq!(emulator.debug_scanline_io_read16(0, 0x50), 0x0000);
        assert_eq!(emulator.debug_scanline_io_read16(0, 0x52), 0x0000);
        assert_eq!(emulator.debug_scanline_io_read16(1, 0x50), 1 << 8);
        assert_eq!(emulator.debug_scanline_io_read16(1, 0x52), 0x0808);

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 0x00);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0xFF);
        assert_eq!(pixels[3], 0xFF);

        let line1 = (GBA_LCD_WIDTH * 4) as usize;
        assert_eq!(pixels[line1], 127);
        assert_eq!(pixels[line1 + 1], 127);
        assert_eq!(pixels[line1 + 2], 0x00);
        assert_eq!(pixels[line1 + 3], 0xFF);
    }

    #[test]
    fn frame_rgba8888_latches_scroll_window_obj_blend_per_scanline() {
        let mut emulator = GbaEmulator::new();
        let nop_rom = RomImage::from_bytes(vec![0x00, 0x00, 0xA0, 0xE1].repeat(128))
            .expect("NOP ROM should be valid");
        emulator.load_rom(nop_rom).expect("ROM load should succeed");

        emulator
            .bus
            .write16(REG_DISPCNT, (1 << 8) | (1 << 12) | (1 << 13)); // mode0 + BG0 + OBJ + WIN0
        for obj_index in 0..128u32 {
            emulator.bus.write16(OAM_BASE + obj_index * 8, 1 << 9); // hide unused OBJ
        }

        emulator.bus.write16(REG_BG0CNT, 0x0000); // BG0 priority 0, screenbase 0
        emulator.bus.write16(VRAM_BASE, 1); // map entry x=0 -> tile 1
        emulator.bus.write16(VRAM_BASE + 2, 2); // map entry x=1 -> tile 2
        for row in 0..8u32 {
            emulator.bus.write8(VRAM_BASE + 0x20 + row * 4, 0x11); // tile 1 -> palette 1 (red)
            emulator.bus.write8(VRAM_BASE + 0x40 + row * 4, 0x22); // tile 2 -> palette 2 (blue)
            emulator.bus.write8(OBJ_CHAR_BASE_TEXT + row * 4, 0x33); // OBJ tile 0 -> palette 3 (green)
        }
        emulator.bus.write16(PRAM_BASE + 2, 0x001F); // BG0 palette 1 = red
        emulator.bus.write16(PRAM_BASE + 4, 0x7C00); // BG0 palette 2 = blue
        emulator.bus.write16(PRAM_BASE + 0x200 + 6, 0x03E0); // OBJ palette 3 = green

        emulator.bus.write16(REG_BG0HOFS, 0x0000);
        emulator.bus.write16(REG_WIN0H, 0x0000); // empty window on line 0
        emulator.bus.write16(REG_WIN0V, 0x00A0); // full height
        emulator
            .bus
            .write16(REG_WININ, (1 << 0) | (1 << 4) | (1 << 5)); // BG0 + OBJ + SFX in WIN0
        emulator.bus.write16(REG_WINOUT, 0x0000); // outside: nothing visible

        emulator.bus.write16(OAM_BASE, 1 << 10); // semi-transparent OBJ
        emulator.bus.write16(OAM_BASE + 2, 0x0000); // x=0
        emulator.bus.write16(OAM_BASE + 4, 2 << 10); // priority 2, hidden behind BG if window opens

        emulator.bus.write16(REG_BLDCNT, 0x0000);
        emulator.bus.write16(REG_BLDALPHA, 0x0000);

        // At line 0 HBlank:
        // - scroll BG0 so x=0 samples tile 2 (blue)
        // - open WIN0 over x=[0,8)
        // - move OBJ in front
        // - blend semi-transparent OBJ with BG0
        emulator.bus.write16(0x0200_0000, 0x0008);
        emulator.bus.write32(0x0400_00D4, 0x0200_0000);
        emulator.bus.write32(0x0400_00D8, REG_BG0HOFS);
        emulator.bus.write16(0x0400_00DC, 1);
        emulator.bus.write16(0x0400_00DE, 0xA000); // DMA3 enable + HBlank

        emulator.bus.write16(0x0200_0010, 0x0008);
        emulator.bus.write32(0x0400_00C8, 0x0200_0010);
        emulator.bus.write32(0x0400_00CC, REG_WIN0H);
        emulator.bus.write16(0x0400_00D0, 1);
        emulator.bus.write16(0x0400_00D2, 0xA000); // DMA2 enable + HBlank

        emulator.bus.write16(0x0200_0020, 0x0000); // OBJ priority 0
        emulator.bus.write32(0x0400_00BC, 0x0200_0020);
        emulator.bus.write32(0x0400_00C0, OAM_BASE + 4);
        emulator.bus.write16(0x0400_00C4, 1);
        emulator.bus.write16(0x0400_00C6, 0xA000); // DMA1 enable + HBlank

        emulator.bus.write16(0x0200_0030, 1 << 8); // BG0 second target
        emulator.bus.write16(0x0200_0032, 0x0808); // 50% / 50%
        emulator.bus.write32(0x0400_00B0, 0x0200_0030);
        emulator.bus.write32(0x0400_00B4, REG_BLDCNT);
        emulator.bus.write16(0x0400_00B8, 2);
        emulator.bus.write16(0x0400_00BA, 0xA000); // DMA0 enable + HBlank

        emulator.step_frame().expect("frame should step");

        assert_eq!(emulator.debug_scanline_io_read16(0, 0x10), 0x0000);
        assert_eq!(emulator.debug_scanline_io_read16(1, 0x10), 0x0008);
        assert_eq!(emulator.debug_scanline_io_read16(0, 0x40), 0x0000);
        assert_eq!(emulator.debug_scanline_io_read16(1, 0x40), 0x0008);
        assert_eq!(emulator.debug_scanline_oam_read16(0, OAM_BASE + 4), 2 << 10);
        assert_eq!(emulator.debug_scanline_oam_read16(1, OAM_BASE + 4), 0x0000);
        assert_eq!(emulator.debug_scanline_io_read16(0, 0x50), 0x0000);
        assert_eq!(emulator.debug_scanline_io_read16(0, 0x52), 0x0000);
        assert_eq!(emulator.debug_scanline_io_read16(1, 0x50), 1 << 8);
        assert_eq!(emulator.debug_scanline_io_read16(1, 0x52), 0x0808);

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 0x00);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0x00);
        assert_eq!(pixels[3], 0xFF);

        let line1 = (GBA_LCD_WIDTH * 4) as usize;
        assert_eq!(pixels[line1], 0x00);
        assert_eq!(pixels[line1 + 1], 127);
        assert_eq!(pixels[line1 + 2], 127);
        assert_eq!(pixels[line1 + 3], 0xFF);

        let line1_outside = ((GBA_LCD_WIDTH + 12) * 4) as usize;
        assert_eq!(pixels[line1_outside], 0x00);
        assert_eq!(pixels[line1_outside + 1], 0x00);
        assert_eq!(pixels[line1_outside + 2], 0x00);
        assert_eq!(pixels[line1_outside + 3], 0xFF);
    }

    #[test]
    fn gba_frame_rgba8888_semitrans_obj_blends_with_under_obj() {
        let mut emulator = GbaEmulator::new();
        let dummy_rom = RomImage::from_bytes(vec![0x00; 512]).expect("dummy ROM should be valid");
        emulator
            .load_rom(dummy_rom)
            .expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 1 << 12); // OBJ enable
        for obj_index in 0..128u32 {
            emulator.bus.write16(OAM_BASE + obj_index * 8, 1 << 9); // hide unused OBJ
        }
        emulator.bus.write16(PRAM_BASE + 0x200 + 2, 0x001F); // OBJ palette 1 = red
        emulator.bus.write16(PRAM_BASE + 0x200 + 4, 0x7C00); // OBJ palette 2 = blue
        emulator.bus.write8(OBJ_CHAR_BASE_TEXT, 0x01); // tile 0 pixel -> palette 1
        emulator.bus.write8(OBJ_CHAR_BASE_TEXT + 0x20, 0x02); // tile 1 pixel -> palette 2

        // Top OBJ: semi-transparent, priority 0, tile 0 (red).
        emulator.bus.write16(OAM_BASE, 1 << 10); // y=0, semi-transparent OBJ
        emulator.bus.write16(OAM_BASE + 2, 0); // x=0, 8x8
        emulator.bus.write16(OAM_BASE + 4, 0); // tile 0, priority 0

        // Under OBJ: normal, priority 1, tile 1 (blue).
        emulator.bus.write16(OAM_BASE + 8, 0); // y=0
        emulator.bus.write16(OAM_BASE + 10, 0); // x=0
        emulator.bus.write16(OAM_BASE + 12, 1 | (1 << 10)); // tile 1, priority 1

        // Semi-trans OBJ uses alpha factors and second-target mask.
        emulator.bus.write16(REG_BLDCNT, 1 << 12); // second target: OBJ
        emulator.bus.write16(REG_BLDALPHA, 0x0808); // 50% / 50%

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 127);
        assert_eq!(pixels[1], 0);
        assert_eq!(pixels[2], 127);
        assert_eq!(pixels[3], 0xFF);
    }

    #[test]
    fn gba_frame_rgba8888_win0_masks_bg0_outside_window() {
        let mut emulator = GbaEmulator::new();
        let dummy_rom = RomImage::from_bytes(vec![0x00; 512]).expect("dummy ROM should be valid");
        emulator
            .load_rom(dummy_rom)
            .expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 0x2100); // mode 0 + BG0 + WIN0
        emulator.bus.write16(REG_BG0CNT, 0x0000); // charbase 0, screenbase 0, 4bpp

        // Fill BG0 tilemap with tile 1 so the background is visible everywhere.
        for map_index in 0..(32u32 * 32) {
            emulator.bus.write16(VRAM_BASE + map_index * 2, 1);
        }
        emulator.bus.write8(VRAM_BASE + 0x20, 0x01); // tile 1 pixel (0,0)=palette 1
        emulator.bus.write16(PRAM_BASE + 2, 0x001F); // palette 1 = red
        emulator.bus.write16(PRAM_BASE, 0x7C00); // backdrop = blue

        // WIN0 covers x=[0,8), y=[0,8): BG0 visible only there. Outside shows backdrop.
        emulator.bus.write16(REG_WIN0H, 0x0008);
        emulator.bus.write16(REG_WIN0V, 0x0008);
        emulator.bus.write16(REG_WININ, 1 << 0); // WIN0: BG0 enable
        emulator.bus.write16(REG_WINOUT, 0); // Outside: no BG/OBJ

        let pixels = emulator.frame_rgba8888();
        // (0,0) is inside WIN0, so BG0 red appears.
        assert_eq!(pixels[0], 0xFF);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0x00);

        // (20,20) is outside WIN0, so backdrop blue appears.
        let outside = ((20 * GBA_LCD_WIDTH + 20) * 4) as usize;
        assert_eq!(pixels[outside], 0x00);
        assert_eq!(pixels[outside + 1], 0x00);
        assert_eq!(pixels[outside + 2], 0xFF);
        assert_eq!(pixels[outside + 3], 0xFF);
    }

    #[test]
    fn gba_frame_rgba8888_window_can_disable_special_effects() {
        let mut emulator = GbaEmulator::new();
        let dummy_rom = RomImage::from_bytes(vec![0x00; 512]).expect("dummy ROM should be valid");
        emulator
            .load_rom(dummy_rom)
            .expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 0x2100); // mode 0 + BG0 + WIN0
        emulator.bus.write16(REG_BG0CNT, 0x0000); // charbase 0, screenbase 0, 4bpp
        emulator.bus.write16(VRAM_BASE, 1); // tilemap entry (0,0) -> tile 1
        emulator.bus.write8(VRAM_BASE + 0x20, 0x01); // tile 1 pixel (0,0)=palette 1
        emulator.bus.write16(PRAM_BASE + 2, 0x0010); // palette 1 = medium red

        emulator.bus.write16(REG_BLDCNT, 0x0081); // effect=brighten, BG0 first target
        emulator.bus.write16(REG_BLDY, 0x0008); // brighten by 8/16

        // WIN0 covers full screen but disables special effects (bit5=0).
        emulator.bus.write16(REG_WIN0H, 0x00F0); // x=[0,240)
        emulator.bus.write16(REG_WIN0V, 0x00A0); // y=[0,160)
        emulator.bus.write16(REG_WININ, 1 << 0); // BG0 on, special effects off
        emulator.bus.write16(REG_WINOUT, 0);

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 132);
        assert_eq!(pixels[1], 0);
        assert_eq!(pixels[2], 0);
        assert_eq!(pixels[3], 0xFF);
    }

    #[test]
    fn gba_frame_rgba8888_obj_window_uses_winout_obj_mask() {
        let mut emulator = GbaEmulator::new();
        let dummy_rom = RomImage::from_bytes(vec![0x00; 512]).expect("dummy ROM should be valid");
        emulator
            .load_rom(dummy_rom)
            .expect("ROM load should succeed");

        // mode 0 + BG0 + OBJ + OBJWIN
        emulator
            .bus
            .write16(REG_DISPCNT, (1 << 8) | (1 << 12) | (1 << 15));
        emulator.bus.write16(REG_BG0CNT, 0x0000); // BG0 priority 0, 4bpp

        // BG0: fill map with tile 1.
        for map_index in 0..(32u32 * 32) {
            emulator.bus.write16(VRAM_BASE + map_index * 2, 1);
        }
        emulator.bus.write8(VRAM_BASE + 0x20, 0x01); // tile 1 pixel (0,0)=palette 1
        emulator.bus.write16(PRAM_BASE + 2, 0x001F); // palette 1 = red
        emulator.bus.write16(PRAM_BASE, 0x7C00); // backdrop = blue

        // Outside window: no BG/OBJ. OBJ window area: BG0 only.
        emulator.bus.write16(REG_WINOUT, 1 << 8);

        // OBJ window sprite at (0,0), 8x8, tile 0.
        emulator.bus.write8(OBJ_CHAR_BASE_TEXT, 0x01); // tile 0 pixel (0,0)=index1
        emulator.bus.write16(OAM_BASE, 2 << 10); // attr0: y=0, mode=OBJWIN, square, 4bpp
        emulator.bus.write16(OAM_BASE + 2, 0); // attr1: x=0, size=8x8
        emulator.bus.write16(OAM_BASE + 4, 0); // attr2: tile 0

        let pixels = emulator.frame_rgba8888();
        // Inside OBJ window -> BG0 visible (red).
        assert_eq!(pixels[0], 0xFF);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0x00);
        // Outside OBJ window -> backdrop (blue).
        let outside = ((20 * GBA_LCD_WIDTH + 20) * 4) as usize;
        assert_eq!(pixels[outside], 0x00);
        assert_eq!(pixels[outside + 1], 0x00);
        assert_eq!(pixels[outside + 2], 0xFF);
        assert_eq!(pixels[outside + 3], 0xFF);
    }

    #[test]
    fn gba_frame_rgba8888_win0_has_priority_over_win1() {
        let mut emulator = GbaEmulator::new();
        let dummy_rom = RomImage::from_bytes(vec![0x00; 512]).expect("dummy ROM should be valid");
        emulator
            .load_rom(dummy_rom)
            .expect("ROM load should succeed");

        emulator
            .bus
            .write16(REG_DISPCNT, (1 << 8) | (1 << 13) | (1 << 14)); // mode0 + BG0 + WIN0 + WIN1
        emulator.bus.write16(REG_BG0CNT, 0x0000); // BG0
        emulator.bus.write16(VRAM_BASE, 1); // map entry -> tile1
        emulator.bus.write8(VRAM_BASE + 0x20, 0x01); // tile1 pixel -> palette1
        emulator.bus.write16(PRAM_BASE + 2, 0x001F); // palette1 red
        emulator.bus.write16(PRAM_BASE, 0x7C00); // backdrop blue

        // Both windows cover full screen.
        emulator.bus.write16(REG_WIN0H, 0x00F0);
        emulator.bus.write16(REG_WIN0V, 0x00A0);
        emulator.bus.write16(REG_WIN1H, 0x00F0);
        emulator.bus.write16(REG_WIN1V, 0x00A0);

        // WIN0 allows BG0, WIN1 blocks BG0.
        emulator.bus.write16(REG_WININ, (1 << 0) | (0 << 8));
        emulator.bus.write16(REG_WINOUT, 0);

        let pixels = emulator.frame_rgba8888();
        // WIN0 should win over WIN1 in overlap => BG0 red is visible.
        assert_eq!(pixels[0], 0xFF);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0x00);
        assert_eq!(pixels[3], 0xFF);
    }

    #[test]
    fn gba_frame_rgba8888_objwin_can_enable_effect_only_inside_objwin() {
        let mut emulator = GbaEmulator::new();
        let dummy_rom = RomImage::from_bytes(vec![0x00; 512]).expect("dummy ROM should be valid");
        emulator
            .load_rom(dummy_rom)
            .expect("ROM load should succeed");

        // mode0 + BG0 + OBJWIN
        emulator.bus.write16(REG_DISPCNT, (1 << 8) | (1 << 15));
        emulator.bus.write16(REG_BG0CNT, 0x0000);
        for map_index in 0..(32u32 * 32) {
            emulator.bus.write16(VRAM_BASE + map_index * 2, 1);
        }
        emulator.bus.write8(VRAM_BASE + 0x20, 0x01);
        emulator.bus.write16(PRAM_BASE + 2, 0x0010); // medium red

        // Brighten BG0 when special effects are enabled.
        emulator.bus.write16(REG_BLDCNT, 0x0081);
        emulator.bus.write16(REG_BLDY, 0x0008);

        // Outside: BG0 visible, SFX off. OBJWIN: BG0 visible, SFX on.
        emulator
            .bus
            .write16(REG_WINOUT, (1 << 0) | ((1 << 0 | 1 << 5) << 8));

        // OBJ window sprite at (0,0), 8x8, mode=2.
        emulator.bus.write8(OBJ_CHAR_BASE_TEXT, 0x01);
        emulator.bus.write16(OAM_BASE, 2 << 10); // OBJWIN
        emulator.bus.write16(OAM_BASE + 2, 0);
        emulator.bus.write16(OAM_BASE + 4, 0);

        let pixels = emulator.frame_rgba8888();
        // Inside OBJWIN => brightened color.
        assert_eq!(pixels[0], 193);
        assert_eq!(pixels[1], 127);
        assert_eq!(pixels[2], 127);
        assert_eq!(pixels[3], 0xFF);

        // Outside OBJWIN => unmodified medium red.
        let outside = ((20 * GBA_LCD_WIDTH + 20) * 4) as usize;
        assert_eq!(pixels[outside], 132);
        assert_eq!(pixels[outside + 1], 0);
        assert_eq!(pixels[outside + 2], 0);
        assert_eq!(pixels[outside + 3], 0xFF);
    }

    #[test]
    fn tetris_worlds_gameplay_window_blend_pattern_darkens_inside_dumped_windows() {
        let mut emulator = GbaEmulator::new();
        let dummy_rom = RomImage::from_bytes(vec![0x00; 512]).expect("dummy ROM should be valid");
        emulator
            .load_rom(dummy_rom)
            .expect("ROM load should succeed");

        // Window/blend values captured from a Tetris Worlds gameplay state:
        // DISPCNT=0x7D40 BLDCNT=0x3FC5 BLDY=0x0008
        // WININ=0x3F3F WINOUT=0x001F WIN0H=0x50A0 WIN1H=0xA8FF WIN0V=0x00FF WIN1V=0x0065
        emulator.bus.write16(REG_DISPCNT, 0x7D40);
        for obj_index in 0..128u32 {
            emulator.bus.write16(OAM_BASE + obj_index * 8, 1 << 9); // hide OBJs
        }
        emulator.bus.write16(REG_BG0CNT, 0x0000);
        for map_index in 0..(32u32 * 32) {
            emulator.bus.write16(VRAM_BASE + map_index * 2, 1);
        }
        for row in 0..8u32 {
            emulator.bus.write8(VRAM_BASE + 0x20 + row * 4, 0x11);
        }
        emulator.bus.write16(PRAM_BASE + 2, 0x0010); // medium red -> 132

        emulator.bus.write16(REG_WININ, 0x3F3F);
        emulator.bus.write16(REG_WINOUT, 0x001F);
        emulator.bus.write16(REG_WIN0H, 0x50A0);
        emulator.bus.write16(REG_WIN1H, 0xA8FF);
        emulator.bus.write16(REG_WIN0V, 0x00FF);
        emulator.bus.write16(REG_WIN1V, 0x0065);
        emulator.bus.write16(REG_BLDCNT, 0x3FC5); // darken, first targets BG0/BG2
        emulator.bus.write16(REG_BLDY, 0x0008);

        let pixels = emulator.frame_rgba8888();

        // Outside both windows: no special effects, plain medium red.
        let outside = (10 * 4) as usize;
        assert_eq!(pixels[outside], 132);
        assert_eq!(pixels[outside + 1], 0);
        assert_eq!(pixels[outside + 2], 0);
        assert_eq!(pixels[outside + 3], 0xFF);

        // Inside WIN0 x=[80,160): darkened to half intensity.
        let win0 = (100 * 4) as usize;
        assert_eq!(pixels[win0], 66);
        assert_eq!(pixels[win0 + 1], 0);
        assert_eq!(pixels[win0 + 2], 0);
        assert_eq!(pixels[win0 + 3], 0xFF);

        // Inside WIN1 x=[168,255), y=[0,101): also darkened.
        let win1 = ((50 * GBA_LCD_WIDTH + 200) * 4) as usize;
        assert_eq!(pixels[win1], 66);
        assert_eq!(pixels[win1 + 1], 0);
        assert_eq!(pixels[win1 + 2], 0);
        assert_eq!(pixels[win1 + 3], 0xFF);

        // Below WIN1's vertical extent: effect disabled again.
        let below_win1 = ((120 * GBA_LCD_WIDTH + 200) * 4) as usize;
        assert_eq!(pixels[below_win1], 132);
        assert_eq!(pixels[below_win1 + 1], 0);
        assert_eq!(pixels[below_win1 + 2], 0);
        assert_eq!(pixels[below_win1 + 3], 0xFF);
    }

    #[test]
    fn tetris_worlds_gameplay_window_blend_pattern_keeps_obj_opaque_inside_windows() {
        let mut emulator = GbaEmulator::new();
        let dummy_rom = RomImage::from_bytes(vec![0x00; 512]).expect("dummy ROM should be valid");
        emulator
            .load_rom(dummy_rom)
            .expect("ROM load should succeed");

        // Same gameplay dump as above. This state had BGx scroll = 0, but
        // windows + darken were active while regular OBJ sprites were visible.
        emulator.bus.write16(REG_DISPCNT, 0x7D40);
        for obj_index in 0..128u32 {
            emulator.bus.write16(OAM_BASE + obj_index * 8, 1 << 9); // hide OBJs
        }

        emulator.bus.write16(REG_BG0CNT, 0x0000);
        for map_index in 0..(32u32 * 32) {
            emulator.bus.write16(VRAM_BASE + map_index * 2, 1);
        }
        for row in 0..8u32 {
            emulator.bus.write8(VRAM_BASE + 0x20 + row * 4, 0x11);
            emulator.bus.write8(OBJ_CHAR_BASE_TEXT + row * 4, 0x11);
        }
        emulator.bus.write16(PRAM_BASE + 2, 0x0010); // BG palette 1 = medium red
        emulator.bus.write16(PRAM_BASE + 0x200 + 2, 0x0200); // OBJ palette 1 = medium green

        // One OBJ outside any window, one inside WIN0.
        emulator.bus.write16(OAM_BASE, 20);
        emulator.bus.write16(OAM_BASE + 2, 16);
        emulator.bus.write16(OAM_BASE + 4, 0x0000);
        emulator.bus.write16(OAM_BASE + 8, 20);
        emulator.bus.write16(OAM_BASE + 10, 100);
        emulator.bus.write16(OAM_BASE + 12, 0x0000);

        emulator.bus.write16(REG_WININ, 0x3F3F);
        emulator.bus.write16(REG_WINOUT, 0x001F);
        emulator.bus.write16(REG_WIN0H, 0x50A0);
        emulator.bus.write16(REG_WIN1H, 0xA8FF);
        emulator.bus.write16(REG_WIN0V, 0x00FF);
        emulator.bus.write16(REG_WIN1V, 0x0065);
        emulator.bus.write16(REG_BLDCNT, 0x3FC5); // darken, first targets BG0/BG2 only
        emulator.bus.write16(REG_BLDY, 0x0008);

        let pixels = emulator.frame_rgba8888();

        // Outside windows, uncovered BG0 stays plain medium red.
        let outside_bg = (10 * 4) as usize;
        assert_eq!(pixels[outside_bg], 132);
        assert_eq!(pixels[outside_bg + 1], 0);
        assert_eq!(pixels[outside_bg + 2], 0);
        assert_eq!(pixels[outside_bg + 3], 0xFF);

        // Inside WIN0, uncovered BG0 is darkened.
        let win0_bg = (100 * 4) as usize;
        assert_eq!(pixels[win0_bg], 66);
        assert_eq!(pixels[win0_bg + 1], 0);
        assert_eq!(pixels[win0_bg + 2], 0);
        assert_eq!(pixels[win0_bg + 3], 0xFF);

        // OBJ pixels stay green both outside windows and inside WIN0 because
        // the real BLDCNT dump does not mark OBJ as a first target.
        let outside_obj = ((22 * GBA_LCD_WIDTH + 18) * 4) as usize;
        assert_eq!(pixels[outside_obj], 0);
        assert_eq!(pixels[outside_obj + 1], 132);
        assert_eq!(pixels[outside_obj + 2], 0);
        assert_eq!(pixels[outside_obj + 3], 0xFF);

        let win0_obj = ((22 * GBA_LCD_WIDTH + 102) * 4) as usize;
        assert_eq!(pixels[win0_obj], 0);
        assert_eq!(pixels[win0_obj + 1], 132);
        assert_eq!(pixels[win0_obj + 2], 0);
        assert_eq!(pixels[win0_obj + 3], 0xFF);
    }

    #[test]
    fn step_frame_with_render_applies_hblank_dma_to_next_line_only() {
        let mut emulator = GbaEmulator::new();
        let nop_rom = RomImage::from_bytes(vec![0x00, 0x00, 0xA0, 0xE1].repeat(128))
            .expect("NOP ROM should be valid");
        emulator.load_rom(nop_rom).expect("ROM load should succeed");

        // mode0 + BG0 + WIN0
        emulator.bus.write16(REG_DISPCNT, (1 << 8) | (1 << 13));
        emulator.bus.write16(REG_BG0CNT, 0x0000);

        // BG0 red everywhere, backdrop blue.
        for map_index in 0..(32u32 * 32) {
            emulator.bus.write16(VRAM_BASE + map_index * 2, 1);
        }
        emulator.bus.write8(VRAM_BASE + 0x20, 0x01);
        emulator.bus.write16(PRAM_BASE + 2, 0x001F);
        emulator.bus.write16(PRAM_BASE, 0x7C00);

        emulator.bus.write16(REG_WIN0H, 0x00F0);
        emulator.bus.write16(REG_WIN0V, 0x00A0);
        emulator.bus.write16(REG_WININ, 1 << 0);
        emulator.bus.write16(REG_WINOUT, 0);

        // HBlank DMA sequence for WIN0V:
        // - end of line 0: keep full-height window, so line 1 stays red
        // - end of line 1: shrink to y=[0,1), so line 2 turns blue
        emulator.bus.write16(0x0200_0000, 0x00A0);
        emulator.bus.write16(0x0200_0002, 0x0001);
        emulator.bus.write16(0x0200_0004, 0x0001);
        emulator.bus.write32(0x0400_00D4, 0x0200_0000);
        emulator.bus.write32(0x0400_00D8, REG_WIN0V);
        emulator.bus.write16(0x0400_00DC, 1);
        emulator.bus.write16(0x0400_00DE, 0xA240); // enable + HBlank + repeat + dest fixed

        let mut frame = GbaFrameBuffer::new();
        emulator
            .step_frame_with_render(&mut frame)
            .expect("frame render should succeed");

        let line1 = (GBA_LCD_WIDTH * 4) as usize;
        assert_eq!(frame.pixels()[line1], 0xFF);
        assert_eq!(frame.pixels()[line1 + 1], 0x00);
        assert_eq!(frame.pixels()[line1 + 2], 0x00);
        assert_eq!(frame.pixels()[line1 + 3], 0xFF);

        let line2 = (GBA_LCD_WIDTH * 2 * 4) as usize;
        assert_eq!(frame.pixels()[line2], 0x00);
        assert_eq!(frame.pixels()[line2 + 1], 0x00);
        assert_eq!(frame.pixels()[line2 + 2], 0xFF);
        assert_eq!(frame.pixels()[line2 + 3], 0xFF);
    }

    #[test]
    fn step_frame_latches_scanline_io_after_vcount_irq_updates() {
        let mut rom = vec![0; 0x200];
        let write_word = |rom: &mut [u8], offset: usize, word: u32| {
            rom[offset] = (word & 0xFF) as u8;
            rom[offset + 1] = ((word >> 8) & 0xFF) as u8;
            rom[offset + 2] = ((word >> 16) & 0xFF) as u8;
            rom[offset + 3] = ((word >> 24) & 0xFF) as u8;
        };
        write_word(&mut rom, 0x000, 0xEAFF_FFFE); // B .
        let irq_handler = [
            0xE3A0_1001, // MOV r1,#1         ; WIN0V = y=[0,1)
            0xE1C0_14B4, // STRH r1,[r0,#0x44]
            0xE3A0_1004, // MOV r1,#4         ; clear IF.VCOUNT
            0xE280_2C02, // ADD r2,r0,#0x200
            0xE1C2_10B2, // STRH r1,[r2,#2]
            0xE1A0_F00E, // MOV pc,lr
        ];
        for (index, word) in irq_handler.iter().enumerate() {
            write_word(&mut rom, 0x100 + index * 4, *word);
        }

        let mut emulator = GbaEmulator::new();
        emulator
            .load_rom(RomImage::from_bytes(rom).expect("ROM should be valid"))
            .expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, (1 << 8) | (1 << 13));
        emulator.bus.write16(REG_BG0CNT, 0x0000);
        for map_index in 0..(32u32 * 32) {
            emulator.bus.write16(VRAM_BASE + map_index * 2, 1);
        }
        emulator.bus.write8(VRAM_BASE + 0x20, 0x01);
        emulator.bus.write16(PRAM_BASE + 2, 0x001F);
        emulator.bus.write16(PRAM_BASE, 0x7C00);
        emulator.bus.write16(REG_WIN0H, 0x00F0);
        emulator.bus.write16(REG_WIN0V, 0x00A0);
        emulator.bus.write16(REG_WININ, 1 << 0);
        emulator.bus.write16(REG_WINOUT, 0);

        emulator.bus.write32(0x0300_7FFC, 0x0800_0100); // ARM no-BIOS IRQ handler
        emulator.bus.write16(0x0400_0004, (1 << 5) | (1 << 8)); // VCount IRQ, LYC=1
        emulator.bus.write16(0x0400_0200, bus::IRQ_VCOUNT);
        emulator.bus.write16(0x0400_0208, 1); // IME on

        emulator.step_frame().expect("frame should step");

        assert_eq!(emulator.debug_scanline_io_read16(0, 0x44), 0x00A0);
        assert_eq!(emulator.debug_scanline_io_read16(1, 0x44), 0x0001);

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 0xFF);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0x00);
        assert_eq!(pixels[3], 0xFF);

        let line1 = (GBA_LCD_WIDTH * 4) as usize;
        assert_eq!(pixels[line1], 0x00);
        assert_eq!(pixels[line1 + 1], 0x00);
        assert_eq!(pixels[line1 + 2], 0xFF);
        assert_eq!(pixels[line1 + 3], 0xFF);
    }

    #[test]
    fn step_frame_latches_vcount_irq_window_and_blend_updates() {
        let mut rom = vec![0; 0x200];
        let write_word = |rom: &mut [u8], offset: usize, word: u32| {
            rom[offset] = (word & 0xFF) as u8;
            rom[offset + 1] = ((word >> 8) & 0xFF) as u8;
            rom[offset + 2] = ((word >> 16) & 0xFF) as u8;
            rom[offset + 3] = ((word >> 24) & 0xFF) as u8;
        };
        write_word(&mut rom, 0x000, 0xEAFF_FFFE); // B .
        let irq_handler = [
            0xE3A0_1008, // MOV r1,#8          ; WIN0H = x=[0,8)
            0xE1C0_14B0, // STRH r1,[r0,#0x40]
            0xE1C0_15B4, // STRH r1,[r0,#0x54] ; BLDY = 8/16
            0xE3A0_1004, // MOV r1,#4          ; clear IF.VCOUNT
            0xE280_2C02, // ADD r2,r0,#0x200
            0xE1C2_10B2, // STRH r1,[r2,#2]
            0xE1A0_F00E, // MOV pc,lr
        ];
        for (index, word) in irq_handler.iter().enumerate() {
            write_word(&mut rom, 0x100 + index * 4, *word);
        }

        let mut emulator = GbaEmulator::new();
        emulator
            .load_rom(RomImage::from_bytes(rom).expect("ROM should be valid"))
            .expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, (1 << 8) | (1 << 13)); // mode0 + BG0 + WIN0
        emulator.bus.write16(REG_BG0CNT, 0x0000);
        for map_index in 0..(32u32 * 32) {
            emulator.bus.write16(VRAM_BASE + map_index * 2, 1);
        }
        for row in 0..8u32 {
            emulator.bus.write8(VRAM_BASE + 0x20 + row * 4, 0x11);
        }
        emulator.bus.write16(PRAM_BASE + 2, 0x0010); // medium red

        emulator.bus.write16(REG_WIN0H, 0x0000); // closed on line 0
        emulator.bus.write16(REG_WIN0V, 0x00A0); // full height
        emulator.bus.write16(REG_WININ, (1 << 0) | (1 << 5)); // BG0 + SFX in WIN0
        emulator.bus.write16(REG_WINOUT, 0x0000); // outside: nothing visible
        emulator.bus.write16(REG_BLDCNT, 0x0081); // brighten BG0
        emulator.bus.write16(REG_BLDY, 0x0000); // off until IRQ

        emulator.bus.write32(0x0300_7FFC, 0x0800_0100); // ARM no-BIOS IRQ handler
        emulator.bus.write16(0x0400_0004, (1 << 5) | (1 << 8)); // VCount IRQ, LYC=1
        emulator.bus.write16(0x0400_0200, bus::IRQ_VCOUNT);
        emulator.bus.write16(0x0400_0208, 1); // IME on

        emulator.step_frame().expect("frame should step");

        assert_eq!(emulator.debug_scanline_io_read16(0, 0x40), 0x0000);
        assert_eq!(emulator.debug_scanline_io_read16(1, 0x40), 0x0008);
        assert_eq!(emulator.debug_scanline_io_read16(0, 0x54), 0x0000);
        assert_eq!(emulator.debug_scanline_io_read16(1, 0x54), 0x0008);

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 0x00);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0x00);
        assert_eq!(pixels[3], 0xFF);

        let line1 = (GBA_LCD_WIDTH * 4) as usize;
        assert_eq!(pixels[line1], 193);
        assert_eq!(pixels[line1 + 1], 127);
        assert_eq!(pixels[line1 + 2], 127);
        assert_eq!(pixels[line1 + 3], 0xFF);

        let line1_outside = ((GBA_LCD_WIDTH + 12) * 4) as usize;
        assert_eq!(pixels[line1_outside], 0x00);
        assert_eq!(pixels[line1_outside + 1], 0x00);
        assert_eq!(pixels[line1_outside + 2], 0x00);
        assert_eq!(pixels[line1_outside + 3], 0xFF);
    }

    #[test]
    fn step_frame_latches_vcount_irq_objwin_and_blend_updates() {
        let mut rom = vec![0; 0x200];
        let write_word = |rom: &mut [u8], offset: usize, word: u32| {
            rom[offset] = (word & 0xFF) as u8;
            rom[offset + 1] = ((word >> 8) & 0xFF) as u8;
            rom[offset + 2] = ((word >> 16) & 0xFF) as u8;
            rom[offset + 3] = ((word >> 24) & 0xFF) as u8;
        };
        write_word(&mut rom, 0x000, 0xEAFF_FFFE); // B .
        let irq_handler = [
            0xE3A0_1C21, // MOV r1,#0x2100    ; OBJWIN: BG0 + SFX, outside unchanged later
            0xE281_1001, // ADD r1,r1,#1      ; WINOUT = 0x2101
            0xE1C0_14BA, // STRH r1,[r0,#0x4A]
            0xE3A0_1008, // MOV r1,#8         ; BLDY = 8/16
            0xE1C0_15B4, // STRH r1,[r0,#0x54]
            0xE3A0_1004, // MOV r1,#4         ; clear IF.VCOUNT
            0xE280_2C02, // ADD r2,r0,#0x200
            0xE1C2_10B2, // STRH r1,[r2,#2]
            0xE1A0_F00E, // MOV pc,lr
        ];
        for (index, word) in irq_handler.iter().enumerate() {
            write_word(&mut rom, 0x100 + index * 4, *word);
        }

        let mut emulator = GbaEmulator::new();
        emulator
            .load_rom(RomImage::from_bytes(rom).expect("ROM should be valid"))
            .expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, (1 << 8) | (1 << 15)); // mode0 + BG0 + OBJWIN
        emulator.bus.write16(REG_BG0CNT, 0x0000);
        for map_index in 0..(32u32 * 32) {
            emulator.bus.write16(VRAM_BASE + map_index * 2, 1);
        }
        for row in 0..8u32 {
            emulator.bus.write8(VRAM_BASE + 0x20 + row * 4, 0x11);
            emulator.bus.write8(OBJ_CHAR_BASE_TEXT + row * 4, 0x11);
        }
        emulator.bus.write16(PRAM_BASE + 2, 0x0010); // medium red

        // Outside: BG0 visible, SFX off. OBJWIN: nothing visible until IRQ updates WINOUT.
        emulator.bus.write16(REG_WINOUT, 0x0001);
        emulator.bus.write16(REG_BLDCNT, 0x0081); // brighten BG0
        emulator.bus.write16(REG_BLDY, 0x0000); // disabled until IRQ

        // OBJ window sprite covering x=[0,8), y=[0,8).
        emulator.bus.write16(OAM_BASE, 2 << 10); // OBJWIN
        emulator.bus.write16(OAM_BASE + 2, 0x0000);
        emulator.bus.write16(OAM_BASE + 4, 0x0000);

        emulator.bus.write32(0x0300_7FFC, 0x0800_0100); // ARM no-BIOS IRQ handler
        emulator.bus.write16(0x0400_0004, (1 << 5) | (1 << 8)); // VCount IRQ, LYC=1
        emulator.bus.write16(0x0400_0200, bus::IRQ_VCOUNT);
        emulator.bus.write16(0x0400_0208, 1); // IME on

        emulator.step_frame().expect("frame should step");

        assert_eq!(emulator.debug_scanline_io_read16(0, 0x4A), 0x0001);
        assert_eq!(emulator.debug_scanline_io_read16(1, 0x4A), 0x2101);
        assert_eq!(emulator.debug_scanline_io_read16(0, 0x54), 0x0000);
        assert_eq!(emulator.debug_scanline_io_read16(1, 0x54), 0x0008);

        let pixels = emulator.frame_rgba8888();
        // line 0 inside OBJWIN: OBJWIN mask blocks BG0 before IRQ reconfigures it.
        assert_eq!(pixels[0], 0x00);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0x00);
        assert_eq!(pixels[3], 0xFF);

        // line 0 outside OBJWIN remains plain medium red.
        let line0_outside = (20 * 4) as usize;
        assert_eq!(pixels[line0_outside], 132);
        assert_eq!(pixels[line0_outside + 1], 0);
        assert_eq!(pixels[line0_outside + 2], 0);
        assert_eq!(pixels[line0_outside + 3], 0xFF);

        // line 1 inside OBJWIN: BG0 visible and brightened.
        let line1 = (GBA_LCD_WIDTH * 4) as usize;
        assert_eq!(pixels[line1], 193);
        assert_eq!(pixels[line1 + 1], 127);
        assert_eq!(pixels[line1 + 2], 127);
        assert_eq!(pixels[line1 + 3], 0xFF);

        // line 1 outside OBJWIN: still plain medium red.
        let line1_outside = ((GBA_LCD_WIDTH + 20) * 4) as usize;
        assert_eq!(pixels[line1_outside], 132);
        assert_eq!(pixels[line1_outside + 1], 0);
        assert_eq!(pixels[line1_outside + 2], 0);
        assert_eq!(pixels[line1_outside + 3], 0xFF);
    }

    #[test]
    fn step_frame_latches_affine_bg_registers_per_scanline() {
        let mut rom = vec![0; 0x200];
        let write_word = |rom: &mut [u8], offset: usize, word: u32| {
            rom[offset] = (word & 0xFF) as u8;
            rom[offset + 1] = ((word >> 8) & 0xFF) as u8;
            rom[offset + 2] = ((word >> 16) & 0xFF) as u8;
            rom[offset + 3] = ((word >> 24) & 0xFF) as u8;
        };
        write_word(&mut rom, 0x000, 0xEAFF_FFFE); // B .
        let irq_handler = [
            0xE3A0_1000, // MOV r1,#0         ; BG2PA = 0
            0xE1C0_12B0, // STRH r1,[r0,#0x20]
            0xE3A0_1004, // MOV r1,#4         ; clear IF.VCOUNT
            0xE280_2C02, // ADD r2,r0,#0x200
            0xE1C2_10B2, // STRH r1,[r2,#2]
            0xE1A0_F00E, // MOV pc,lr
        ];
        for (index, word) in irq_handler.iter().enumerate() {
            write_word(&mut rom, 0x100 + index * 4, *word);
        }

        let mut emulator = GbaEmulator::new();
        emulator
            .load_rom(RomImage::from_bytes(rom).expect("ROM should be valid"))
            .expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 0x0001 | (1 << 10)); // mode 1 + BG2
        emulator.bus.write16(REG_BG2CNT, 0x0000);
        emulator.bus.write16(REG_BG2PA, 0x0100);
        emulator.bus.write16(REG_BG2PD, 0x0100);

        // Affine tilemap: tile 1 at x=[0,8), tile 2 at x=[8,16)
        emulator.bus.write16(VRAM_BASE, 0x0201);
        for byte in 0..64u32 {
            emulator.bus.write8(VRAM_BASE + 0x40 + byte, 1);
            emulator.bus.write8(VRAM_BASE + 0x80 + byte, 2);
        }
        emulator.bus.write16(PRAM_BASE + 2, 0x001F); // red
        emulator.bus.write16(PRAM_BASE + 4, 0x7C00); // blue

        emulator.bus.write32(0x0300_7FFC, 0x0800_0100); // ARM no-BIOS IRQ handler
        emulator.bus.write16(0x0400_0004, (1 << 5) | (1 << 8)); // VCount IRQ, LYC=1
        emulator.bus.write16(0x0400_0200, bus::IRQ_VCOUNT);
        emulator.bus.write16(0x0400_0208, 1); // IME on

        emulator.step_frame().expect("frame should step");

        assert_eq!(emulator.debug_scanline_io_read16(0, 0x20), 0x0100);
        assert_eq!(emulator.debug_scanline_io_read16(1, 0x20), 0x0000);

        let pixels = emulator.frame_rgba8888();
        let line0_x8 = (8 * 4) as usize;
        assert_eq!(pixels[line0_x8], 0x00);
        assert_eq!(pixels[line0_x8 + 1], 0x00);
        assert_eq!(pixels[line0_x8 + 2], 0xFF);
        assert_eq!(pixels[line0_x8 + 3], 0xFF);

        let line1_x8 = ((GBA_LCD_WIDTH + 8) * 4) as usize;
        assert_eq!(pixels[line1_x8], 0xFF);
        assert_eq!(pixels[line1_x8 + 1], 0x00);
        assert_eq!(pixels[line1_x8 + 2], 0x00);
        assert_eq!(pixels[line1_x8 + 3], 0xFF);
    }

    #[test]
    fn frame_rgba8888_latches_mode4_page_per_scanline() {
        let mut emulator = GbaEmulator::new();
        let nop_rom = RomImage::from_bytes(vec![0x00, 0x00, 0xA0, 0xE1].repeat(128))
            .expect("NOP ROM should be valid");
        emulator.load_rom(nop_rom).expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 0x0004 | (1 << 10)); // mode 4 + BG2, page 0
        emulator.bus.write16(REG_BG2CNT, 0x0000);
        emulator.bus.write16(PRAM_BASE + 2, 0x001F); // red
        emulator.bus.write16(PRAM_BASE + 4, 0x7C00); // blue
        for y in 0..=1u32 {
            emulator.bus.write8(VRAM_BASE + y * GBA_LCD_WIDTH, 1);
            emulator
                .bus
                .write8(VRAM_BASE + 0xA000 + y * GBA_LCD_WIDTH, 2);
        }

        // Switch to page 1 during line 0 HBlank so line 1 reads from page 1.
        emulator.bus.write16(0x0200_0000, 0x0014 | (1 << 10));
        emulator.bus.write32(0x0400_00D4, 0x0200_0000);
        emulator.bus.write32(0x0400_00D8, REG_DISPCNT);
        emulator.bus.write16(0x0400_00DC, 1);
        emulator.bus.write16(0x0400_00DE, 0xA240); // enable + HBlank + repeat + dest fixed

        emulator.step_frame().expect("frame should step");

        assert_eq!(emulator.debug_scanline_io_read16(0, 0x00), 0x0404);
        assert_eq!(emulator.debug_scanline_io_read16(1, 0x00), 0x0414);

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 0xFF);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0x00);
        assert_eq!(pixels[3], 0xFF);

        let line1 = (GBA_LCD_WIDTH * 4) as usize;
        assert_eq!(pixels[line1], 0x00);
        assert_eq!(pixels[line1 + 1], 0x00);
        assert_eq!(pixels[line1 + 2], 0xFF);
        assert_eq!(pixels[line1 + 3], 0xFF);
    }

    #[test]
    fn frame_rgba8888_latches_mode3_vram_per_scanline() {
        let mut emulator = GbaEmulator::new();
        let nop_rom = RomImage::from_bytes(vec![0x00, 0x00, 0xA0, 0xE1].repeat(128))
            .expect("NOP ROM should be valid");
        emulator.load_rom(nop_rom).expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 0x0003 | (1 << 10)); // mode 3 + BG2
        emulator.bus.write16(0x0600_0000, 0x001F); // line 0 x=0 = red

        // Overwrite the already-drawn line 0 pixel during line 0 HBlank.
        emulator.bus.write16(0x0200_0000, 0x7C00);
        emulator.bus.write32(0x0400_00D4, 0x0200_0000);
        emulator.bus.write32(0x0400_00D8, 0x0600_0000);
        emulator.bus.write16(0x0400_00DC, 1);
        emulator.bus.write16(0x0400_00DE, 0xA040); // enable + HBlank + dest fixed

        emulator.step_frame().expect("frame should step");

        assert_eq!(
            emulator.debug_scanline_bg_bitmap_vram_read16(0, 0x0600_0000),
            0x001F
        );
        assert_eq!(
            emulator.debug_scanline_bg_bitmap_vram_read16(1, 0x0600_0000),
            0x7C00
        );

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 0xFF);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0x00);
        assert_eq!(pixels[3], 0xFF);
    }

    #[test]
    fn frame_rgba8888_latches_mode4_pram_per_scanline() {
        let mut emulator = GbaEmulator::new();
        let nop_rom = RomImage::from_bytes(vec![0x00, 0x00, 0xA0, 0xE1].repeat(128))
            .expect("NOP ROM should be valid");
        emulator.load_rom(nop_rom).expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 0x0004 | (1 << 10)); // mode 4 + BG2
        emulator.bus.write16(PRAM_BASE + 2, 0x001F); // BG palette 1 = red
        emulator.bus.write8(VRAM_BASE, 1);
        emulator.bus.write8(VRAM_BASE + GBA_LCD_WIDTH, 1);

        // Recolor palette entry 1 during line 0 HBlank so line 1 becomes blue.
        emulator.bus.write16(0x0200_0000, 0x7C00);
        emulator.bus.write32(0x0400_00D4, 0x0200_0000);
        emulator.bus.write32(0x0400_00D8, PRAM_BASE + 2);
        emulator.bus.write16(0x0400_00DC, 1);
        emulator.bus.write16(0x0400_00DE, 0xA040); // enable + HBlank + dest fixed

        emulator.step_frame().expect("frame should step");

        assert_eq!(emulator.debug_scanline_pram_read16(0, 2), 0x001F);
        assert_eq!(emulator.debug_scanline_pram_read16(1, 2), 0x7C00);

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 0xFF);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0x00);
        assert_eq!(pixels[3], 0xFF);

        let line1 = (GBA_LCD_WIDTH * 4) as usize;
        assert_eq!(pixels[line1], 0x00);
        assert_eq!(pixels[line1 + 1], 0x00);
        assert_eq!(pixels[line1 + 2], 0xFF);
        assert_eq!(pixels[line1 + 3], 0xFF);
    }

    #[test]
    fn frame_rgba8888_latches_text_bg_tile_data_per_scanline() {
        let mut emulator = GbaEmulator::new();
        let nop_rom = RomImage::from_bytes(vec![0x00, 0x00, 0xA0, 0xE1].repeat(128))
            .expect("NOP ROM should be valid");
        emulator.load_rom(nop_rom).expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 1 << 8); // mode 0 + BG0
        emulator.bus.write16(0x0400_0008, 1 << 2); // BG0CNT: char base 1
        emulator.bus.write16(VRAM_BASE, 1); // tilemap entry -> tile 1
        emulator.bus.write16(PRAM_BASE + 2, 0x001F); // BG palette 1 = red
        emulator.bus.write16(PRAM_BASE + 4, 0x7C00); // BG palette 2 = blue
        for byte in 0..32u32 {
            emulator.bus.write8(VRAM_BASE + 0x4000 + 0x20 + byte, 0x11);
        }

        // Rewrite tile 1 during line 0 HBlank so line 1 becomes blue.
        emulator.bus.write16(0x0200_0000, 0x2222);
        emulator.bus.write32(0x0400_00D4, 0x0200_0000);
        emulator.bus.write32(0x0400_00D8, VRAM_BASE + 0x4000 + 0x20);
        emulator.bus.write16(0x0400_00DC, 16);
        emulator.bus.write16(0x0400_00DE, 0xA300); // enable + HBlank + repeat + source fixed

        emulator.step_frame().expect("frame should step");

        assert_eq!(
            emulator.debug_scanline_bg_bitmap_vram_read16(0, VRAM_BASE + 0x4000 + 0x20),
            0x1111
        );
        assert_eq!(
            emulator.debug_scanline_bg_bitmap_vram_read16(1, VRAM_BASE + 0x4000 + 0x20),
            0x2222
        );

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 0xFF);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0x00);
        assert_eq!(pixels[3], 0xFF);

        let line1 = (GBA_LCD_WIDTH * 4) as usize;
        assert_eq!(pixels[line1], 0x00);
        assert_eq!(pixels[line1 + 1], 0x00);
        assert_eq!(pixels[line1 + 2], 0xFF);
        assert_eq!(pixels[line1 + 3], 0xFF);
    }

    #[test]
    fn frame_rgba8888_latches_text_bg_map_entry_per_scanline() {
        let mut emulator = GbaEmulator::new();
        let nop_rom = RomImage::from_bytes(vec![0x00, 0x00, 0xA0, 0xE1].repeat(128))
            .expect("NOP ROM should be valid");
        emulator.load_rom(nop_rom).expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 1 << 8); // mode 0 + BG0
        emulator.bus.write16(0x0400_0008, 1 << 2); // BG0CNT: char base 1
        emulator.bus.write16(VRAM_BASE, 1); // tilemap entry -> tile 1
        emulator.bus.write16(PRAM_BASE + 2, 0x001F); // BG palette 1 = red
        emulator.bus.write16(PRAM_BASE + 4, 0x7C00); // BG palette 2 = blue
        for byte in 0..32u32 {
            emulator.bus.write8(VRAM_BASE + 0x4000 + 0x20 + byte, 0x11);
            emulator.bus.write8(VRAM_BASE + 0x4000 + 0x40 + byte, 0x22);
        }

        // Rewrite the first map entry during line 0 HBlank so line 1 uses tile 2.
        emulator.bus.write16(0x0200_0000, 0x0002);
        emulator.bus.write32(0x0400_00D4, 0x0200_0000);
        emulator.bus.write32(0x0400_00D8, VRAM_BASE);
        emulator.bus.write16(0x0400_00DC, 1);
        emulator.bus.write16(0x0400_00DE, 0xA100); // enable + HBlank + source fixed

        emulator.step_frame().expect("frame should step");

        assert_eq!(
            emulator.debug_scanline_bg_bitmap_vram_read16(0, VRAM_BASE),
            0x0001
        );
        assert_eq!(
            emulator.debug_scanline_bg_bitmap_vram_read16(1, VRAM_BASE),
            0x0002
        );

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 0xFF);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0x00);
        assert_eq!(pixels[3], 0xFF);

        let line1 = (GBA_LCD_WIDTH * 4) as usize;
        assert_eq!(pixels[line1], 0x00);
        assert_eq!(pixels[line1 + 1], 0x00);
        assert_eq!(pixels[line1 + 2], 0xFF);
        assert_eq!(pixels[line1 + 3], 0xFF);
    }

    #[test]
    fn frame_rgba8888_latches_affine_bg_palette_per_scanline() {
        let mut emulator = GbaEmulator::new();
        let nop_rom = RomImage::from_bytes(vec![0x00, 0x00, 0xA0, 0xE1].repeat(128))
            .expect("NOP ROM should be valid");
        emulator.load_rom(nop_rom).expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 0x0001 | (1 << 10)); // mode 1 + BG2
        emulator.bus.write16(0x0400_000C, 0x0000); // BG2CNT
        emulator.bus.write16(REG_BG2PA, 0x0100);
        emulator.bus.write16(REG_BG2PD, 0x0100);
        emulator.bus.write16(PRAM_BASE + 2, 0x001F); // BG palette 1 = red
        emulator.bus.write16(VRAM_BASE, 1); // affine map entry -> tile 1
        for byte in 0..64u32 {
            emulator.bus.write8(VRAM_BASE + 0x40 + byte, 1);
        }

        // Recolor palette entry 1 during line 0 HBlank so line 1 becomes blue.
        emulator.bus.write16(0x0200_0000, 0x7C00);
        emulator.bus.write32(0x0400_00D4, 0x0200_0000);
        emulator.bus.write32(0x0400_00D8, PRAM_BASE + 2);
        emulator.bus.write16(0x0400_00DC, 1);
        emulator.bus.write16(0x0400_00DE, 0xA040); // enable + HBlank + dest fixed

        emulator.step_frame().expect("frame should step");

        assert_eq!(emulator.debug_scanline_pram_read16(0, 2), 0x001F);
        assert_eq!(emulator.debug_scanline_pram_read16(1, 2), 0x7C00);

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 0xFF);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0x00);
        assert_eq!(pixels[3], 0xFF);

        let line1 = (GBA_LCD_WIDTH * 4) as usize;
        assert_eq!(pixels[line1], 0x00);
        assert_eq!(pixels[line1 + 1], 0x00);
        assert_eq!(pixels[line1 + 2], 0xFF);
        assert_eq!(pixels[line1 + 3], 0xFF);
    }

    #[test]
    fn frame_rgba8888_latches_affine_bg_map_entry_per_scanline() {
        let mut emulator = GbaEmulator::new();
        let nop_rom = RomImage::from_bytes(vec![0x00, 0x00, 0xA0, 0xE1].repeat(128))
            .expect("NOP ROM should be valid");
        emulator.load_rom(nop_rom).expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 0x0001 | (1 << 10)); // mode 1 + BG2
        emulator.bus.write16(0x0400_000C, 0x0000); // BG2CNT
        emulator.bus.write16(REG_BG2PA, 0x0100);
        emulator.bus.write16(REG_BG2PD, 0x0100);
        emulator.bus.write16(PRAM_BASE + 2, 0x001F); // BG palette 1 = red
        emulator.bus.write16(PRAM_BASE + 4, 0x7C00); // BG palette 2 = blue
        emulator.bus.write16(VRAM_BASE, 0x0001); // affine map entries 0..1 -> tile 1
        for byte in 0..64u32 {
            emulator.bus.write8(VRAM_BASE + 0x40 + byte, 1);
            emulator.bus.write8(VRAM_BASE + 0x80 + byte, 2);
        }

        // Rewrite the first affine map entry during line 0 HBlank so line 1 uses tile 2.
        emulator.bus.write16(0x0200_0000, 0x0202);
        emulator.bus.write32(0x0400_00D4, 0x0200_0000);
        emulator.bus.write32(0x0400_00D8, VRAM_BASE);
        emulator.bus.write16(0x0400_00DC, 1);
        emulator.bus.write16(0x0400_00DE, 0xA100); // enable + HBlank + source fixed

        emulator.step_frame().expect("frame should step");

        assert_eq!(
            emulator.debug_scanline_bg_bitmap_vram_read16(0, VRAM_BASE),
            0x0001
        );
        assert_eq!(
            emulator.debug_scanline_bg_bitmap_vram_read16(1, VRAM_BASE),
            0x0202
        );

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 0xFF);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0x00);
        assert_eq!(pixels[3], 0xFF);

        let line1 = (GBA_LCD_WIDTH * 4) as usize;
        assert_eq!(pixels[line1], 0x00);
        assert_eq!(pixels[line1 + 1], 0x00);
        assert_eq!(pixels[line1 + 2], 0xFF);
        assert_eq!(pixels[line1 + 3], 0xFF);
    }

    #[test]
    fn frame_rgba8888_latches_oam_per_scanline() {
        let mut emulator = GbaEmulator::new();
        let nop_rom = RomImage::from_bytes(vec![0x00, 0x00, 0xA0, 0xE1].repeat(128))
            .expect("NOP ROM should be valid");
        emulator.load_rom(nop_rom).expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 1 << 12); // OBJ only
        emulator.bus.write16(PRAM_BASE, 0x7C00); // blue backdrop
        emulator.bus.write16(PRAM_BASE + 0x200 + 2, 0x001F); // OBJ palette 1 = red
        for byte in 0..32u32 {
            emulator.bus.write8(OBJ_CHAR_BASE_TEXT + byte, 0x11);
        }

        // 8x8 sprite at (0,0), tile 0.
        emulator.bus.write16(OAM_BASE, 0x0000);
        emulator.bus.write16(OAM_BASE + 2, 0x0000);
        emulator.bus.write16(OAM_BASE + 4, 0x0000);

        // Move the sprite offscreen during line 0 HBlank so only line 0 sees it.
        emulator.bus.write16(0x0200_0000, 0x00F0);
        emulator.bus.write32(0x0400_00D4, 0x0200_0000);
        emulator.bus.write32(0x0400_00D8, OAM_BASE + 2);
        emulator.bus.write16(0x0400_00DC, 1);
        emulator.bus.write16(0x0400_00DE, 0xA040); // enable + HBlank + dest fixed

        emulator.step_frame().expect("frame should step");

        assert_eq!(emulator.debug_scanline_oam_read16(0, OAM_BASE + 2), 0x0000);
        assert_eq!(emulator.debug_scanline_oam_read16(1, OAM_BASE + 2), 0x00F0);

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 0xFF);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0x00);
        assert_eq!(pixels[3], 0xFF);

        let line1 = (GBA_LCD_WIDTH * 4) as usize;
        assert_eq!(pixels[line1], 0x00);
        assert_eq!(pixels[line1 + 1], 0x00);
        assert_eq!(pixels[line1 + 2], 0xFF);
        assert_eq!(pixels[line1 + 3], 0xFF);
    }

    #[test]
    fn frame_rgba8888_latches_obj_tile_data_per_scanline() {
        let mut emulator = GbaEmulator::new();
        let nop_rom = RomImage::from_bytes(vec![0x00, 0x00, 0xA0, 0xE1].repeat(128))
            .expect("NOP ROM should be valid");
        emulator.load_rom(nop_rom).expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 1 << 12); // OBJ only
        emulator.bus.write16(PRAM_BASE + 0x200 + 2, 0x001F); // OBJ palette 1 = red
        emulator.bus.write16(PRAM_BASE + 0x200 + 4, 0x7C00); // OBJ palette 2 = blue
        for byte in 0..32u32 {
            emulator.bus.write8(OBJ_CHAR_BASE_TEXT + byte, 0x11);
        }

        emulator.bus.write16(OAM_BASE, 0x0000);
        emulator.bus.write16(OAM_BASE + 2, 0x0000);
        emulator.bus.write16(OAM_BASE + 4, 0x0000);

        // Change the sprite tile data during line 0 HBlank so line 1 becomes blue.
        emulator.bus.write16(0x0200_0000, 0x2222);
        emulator.bus.write32(0x0400_00D4, 0x0200_0000);
        emulator.bus.write32(0x0400_00D8, OBJ_CHAR_BASE_TEXT);
        emulator.bus.write16(0x0400_00DC, 16);
        emulator.bus.write16(0x0400_00DE, 0xA300); // enable + HBlank + repeat + source fixed

        emulator.step_frame().expect("frame should step");

        assert_eq!(
            emulator.debug_scanline_obj_vram_read8(0, OBJ_CHAR_BASE_TEXT),
            0x11
        );
        assert_eq!(
            emulator.debug_scanline_obj_vram_read8(1, OBJ_CHAR_BASE_TEXT),
            0x22
        );

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 0xFF);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0x00);
        assert_eq!(pixels[3], 0xFF);

        let line1 = (GBA_LCD_WIDTH * 4) as usize;
        assert_eq!(pixels[line1], 0x00);
        assert_eq!(pixels[line1 + 1], 0x00);
        assert_eq!(pixels[line1 + 2], 0xFF);
        assert_eq!(pixels[line1 + 3], 0xFF);
    }

    #[test]
    fn frame_rgba8888_latches_obj_palette_per_scanline() {
        let mut emulator = GbaEmulator::new();
        let nop_rom = RomImage::from_bytes(vec![0x00, 0x00, 0xA0, 0xE1].repeat(128))
            .expect("NOP ROM should be valid");
        emulator.load_rom(nop_rom).expect("ROM load should succeed");

        emulator.bus.write16(REG_DISPCNT, 1 << 12); // OBJ only
        emulator.bus.write16(PRAM_BASE + 0x200 + 2, 0x001F); // OBJ palette 1 = red
        for byte in 0..32u32 {
            emulator.bus.write8(OBJ_CHAR_BASE_TEXT + byte, 0x11);
        }

        emulator.bus.write16(OAM_BASE, 0x0000);
        emulator.bus.write16(OAM_BASE + 2, 0x0000);
        emulator.bus.write16(OAM_BASE + 4, 0x0000);

        // Change OBJ palette 1 during line 0 HBlank so line 1 becomes blue.
        emulator.bus.write16(0x0200_0000, 0x7C00);
        emulator.bus.write32(0x0400_00D4, 0x0200_0000);
        emulator.bus.write32(0x0400_00D8, PRAM_BASE + 0x200 + 2);
        emulator.bus.write16(0x0400_00DC, 1);
        emulator.bus.write16(0x0400_00DE, 0xA040); // enable + HBlank + dest fixed

        emulator.step_frame().expect("frame should step");

        assert_eq!(emulator.debug_scanline_pram_read16(0, 0x200 + 2), 0x001F);
        assert_eq!(emulator.debug_scanline_pram_read16(1, 0x200 + 2), 0x7C00);

        let pixels = emulator.frame_rgba8888();
        assert_eq!(pixels[0], 0xFF);
        assert_eq!(pixels[1], 0x00);
        assert_eq!(pixels[2], 0x00);
        assert_eq!(pixels[3], 0xFF);

        let line1 = (GBA_LCD_WIDTH * 4) as usize;
        assert_eq!(pixels[line1], 0x00);
        assert_eq!(pixels[line1 + 1], 0x00);
        assert_eq!(pixels[line1 + 2], 0xFF);
        assert_eq!(pixels[line1 + 3], 0xFF);
    }

    fn make_test_rom() -> RomImage {
        RomImage::from_bytes(vec![0u8; 256]).expect("dummy ROM")
    }

    fn make_legacy_state_from_current(state_data: &[u8]) -> Vec<u8> {
        let legacy_payload = bus::strip_newer_scanline_snapshots_for_legacy_test(&state_data[16..]);
        let mut legacy_state = Vec::with_capacity(16 + legacy_payload.len());
        legacy_state.extend_from_slice(&state_data[..12]);
        legacy_state.extend_from_slice(&(legacy_payload.len() as u32).to_le_bytes());
        legacy_state.extend_from_slice(&legacy_payload);
        legacy_state
    }

    fn count_non_black_pixels(frame: &[u8]) -> usize {
        frame
            .chunks_exact(4)
            .filter(|px| px[0] != 0 || px[1] != 0 || px[2] != 0)
            .count()
    }

    #[test]
    fn save_state_roundtrip() {
        let mut emu = GbaEmulator::new();
        emu.load_rom(make_test_rom()).unwrap();
        emu.bus.write32(0x0200_0000, 0xDEAD_BEEF);
        emu.bus.write16(0x0400_0000, 0x0403);

        let state_data = emu.save_state();
        assert!(state_data.len() > 16);
        assert_eq!(&state_data[0..4], b"GBAS");

        let mut emu2 = GbaEmulator::new();
        emu2.load_rom(make_test_rom()).unwrap();
        emu2.load_state(&state_data)
            .expect("load_state should succeed");

        assert_eq!(emu2.bus.read32(0x0200_0000), 0xDEAD_BEEF);
        assert_eq!(emu2.bus.read16(0x0400_0000), 0x0403);
    }

    #[test]
    fn load_state_resizes_legacy_render_snapshots() {
        let mut emu = GbaEmulator::new();
        emu.load_rom(make_test_rom()).unwrap();
        emu.step_frame().unwrap();
        emu.bus.pram_snapshot.clear();
        emu.bus.vram_snapshot.clear();
        emu.bus.oam_snapshot.clear();

        let legacy_like_state = emu.save_state();

        let mut emu2 = GbaEmulator::new();
        emu2.load_rom(make_test_rom()).unwrap();
        emu2.load_state(&legacy_like_state)
            .expect("legacy-like state should load");

        assert_eq!(emu2.bus.pram_snapshot.len(), 0x400);
        assert_eq!(emu2.bus.vram_snapshot.len(), 0x18000);
        assert_eq!(emu2.bus.oam_snapshot.len(), 0x400);

        let pixels = emu2.frame_rgba8888();
        assert_eq!(pixels.len(), GBA_FRAME_RGBA8888_BYTES);
    }

    #[test]
    fn load_state_upgrades_legacy_payload_missing_scanline_snapshots() {
        let mut emu = GbaEmulator::new();
        emu.load_rom(make_test_rom()).unwrap();
        emu.bus.write32(0x0200_0000, 0xDEAD_BEEF);
        emu.bus.write16(0x0400_0000, 0x0403);
        emu.step_frame().unwrap();

        let expected_pc = emu.debug_cpu_pc();
        let expected_cpsr = emu.debug_cpu_cpsr();
        let expected_line0_dispcnt = emu.debug_scanline_io_read16(0, 0x00);
        let expected_snapshots = emu.debug_snapshot_hashes();

        let state_data = emu.save_state();
        let legacy_state = make_legacy_state_from_current(&state_data);

        let mut emu2 = GbaEmulator::new();
        emu2.load_rom(make_test_rom()).unwrap();
        emu2.load_state(&legacy_state)
            .expect("legacy payload should auto-upgrade");

        assert_eq!(emu2.bus.read32(0x0200_0000), 0xDEAD_BEEF);
        assert_eq!(emu2.bus.read16(0x0400_0000), 0x0403);
        assert_eq!(emu2.debug_cpu_pc(), expected_pc);
        assert_eq!(emu2.debug_cpu_cpsr(), expected_cpsr);
        assert_eq!(
            emu2.debug_scanline_io_read16(0, 0x00),
            expected_line0_dispcnt
        );
        assert_eq!(emu2.debug_snapshot_hashes(), expected_snapshots);
    }

    #[test]
    fn upgraded_legacy_payload_matches_current_state_after_stepping() {
        let mut emu = GbaEmulator::new();
        emu.load_rom(make_test_rom()).unwrap();
        emu.bus.write32(0x0200_0000, 0xDEAD_BEEF);
        emu.bus.write16(0x0400_0000, 0x0403);
        emu.step_frame().unwrap();

        let current_state = emu.save_state();
        let legacy_state = make_legacy_state_from_current(&current_state);

        let mut baseline = GbaEmulator::new();
        baseline.load_rom(make_test_rom()).unwrap();
        baseline
            .load_state(&current_state)
            .expect("current state should load");

        let mut upgraded = GbaEmulator::new();
        upgraded.load_rom(make_test_rom()).unwrap();
        upgraded
            .load_state(&legacy_state)
            .expect("legacy state should auto-upgrade");

        for _ in 0..8 {
            let baseline_frame = baseline.step_frame().expect("baseline frame should step");
            let upgraded_frame = upgraded.step_frame().expect("upgraded frame should step");
            assert_eq!(upgraded_frame.frame_number, baseline_frame.frame_number);
            assert_eq!(upgraded_frame.cycles, baseline_frame.cycles);
        }

        assert_eq!(upgraded.debug_cpu_pc(), baseline.debug_cpu_pc());
        assert_eq!(upgraded.debug_cpu_cpsr(), baseline.debug_cpu_cpsr());
        assert_eq!(
            upgraded.bus.read32(0x0200_0000),
            baseline.bus.read32(0x0200_0000)
        );
        assert_eq!(
            upgraded.debug_snapshot_hashes(),
            baseline.debug_snapshot_hashes()
        );
        assert_eq!(
            upgraded.debug_scanline_io_read16(0, 0x00),
            baseline.debug_scanline_io_read16(0, 0x00)
        );
    }

    #[test]
    #[ignore = "requires local Tetris Worlds ROM/state fixtures under roms/"]
    fn tetris_worlds_old_state_loads_and_runs_without_padding() {
        let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let rom_path = repo_root.join("roms/Tetris Worlds (Japan).gba");
        let state_path = repo_root.join("roms/Tetris Worlds (Japan).ss1");
        if !rom_path.exists() || !state_path.exists() {
            eprintln!(
                "skipping fixture test: missing {} or {}",
                rom_path.display(),
                state_path.display()
            );
            return;
        }

        let mut emu = GbaEmulator::new();
        emu.load_rom(RomImage::from_file(&rom_path).expect("fixture ROM should load"))
            .expect("emulator should accept fixture ROM");
        let state_data = std::fs::read(&state_path).expect("fixture state should read");
        emu.load_state(&state_data)
            .expect("old fixture state should auto-upgrade");

        let first_frame = emu.step_frame().expect("fixture frame should step");
        assert_eq!(first_frame.cycles, GBA_FRAME_CYCLES);
        assert_eq!(emu.debug_cpu_pc(), 0x000D_8784);
        assert_eq!(emu.debug_read16(REG_DISPCNT), 0x7D40);
        assert_eq!(emu.debug_read16(REG_BLDCNT), 0x3FC5);
        assert_eq!(emu.debug_scanline_io_read16(0, 0x4A), 0x001F);
        assert_eq!(emu.debug_scanline_io_read16(0, 0x40), 0x50A0);
        assert_eq!(count_non_black_pixels(&emu.frame_rgba8888()), 38_207);

        for _ in 0..3 {
            let frame = emu.step_frame().expect("fixture frame should step");
            assert_eq!(frame.cycles, GBA_FRAME_CYCLES);
        }

        let pixels = emu.frame_rgba8888();
        assert!(count_non_black_pixels(&pixels) > 30_000);
        assert_eq!(emu.debug_read16(REG_DISPCNT), 0x7D40);
    }

    #[test]
    fn save_state_rejects_invalid_magic() {
        let mut emu = GbaEmulator::new();
        emu.load_rom(make_test_rom()).unwrap();
        let mut data = emu.save_state();
        data[0] = b'X';
        assert_eq!(emu.load_state(&data), Err("invalid state magic"));
    }

    #[test]
    fn save_state_rejects_wrong_crc() {
        let mut emu = GbaEmulator::new();
        emu.load_rom(make_test_rom()).unwrap();
        let mut data = emu.save_state();
        data[8] ^= 0xFF;
        assert_eq!(emu.load_state(&data), Err("ROM CRC mismatch"));
    }

    #[test]
    fn save_state_rejects_truncated_data() {
        let mut emu = GbaEmulator::new();
        emu.load_rom(make_test_rom()).unwrap();
        let data = emu.save_state();
        let truncated = &data[..16];
        assert!(emu.load_state(truncated).is_err());
    }
}
