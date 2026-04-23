use super::*;
use crate::psg::PSG_WAVE_SIZE;
use crate::psg::*;
use crate::vdc::{
    DMA_CTRL_IRQ_SATB, DMA_CTRL_IRQ_VRAM, DMA_CTRL_SATB_AUTO, FRAME_WIDTH, LINES_PER_FRAME,
    SPRITE_PATTERN_HEIGHT, SPRITE_PATTERN_WORDS, TILE_HEIGHT, TILE_WIDTH, VDC_BUSY_ACCESS_CYCLES,
    VDC_VBLANK_INTERVAL, VDC_VISIBLE_LINES, Vdc,
};

const PHI_CYCLES_PER_SAMPLE: u32 = MASTER_CLOCK_HZ / AUDIO_SAMPLE_RATE;

const VCE_ADDRESS_ADDR: u16 = 0x0402;
const VCE_ADDRESS_HIGH_ADDR: u16 = 0x0403;
const VCE_DATA_ADDR: u16 = 0x0404;
const VCE_DATA_HIGH_ADDR: u16 = 0x0405;
#[allow(dead_code)]
const PSG_ADDR_REG: u16 = 0x1C60;
#[allow(dead_code)]
const PSG_WRITE_REG: u16 = 0x1C61;
#[allow(dead_code)]
const PSG_READ_REG: u16 = 0x1C62;
#[allow(dead_code)]
const PSG_STATUS_REG: u16 = 0x1C63;
const TIMER_STD_BASE: u16 = 0x0C00;
const JOYPAD_BASE_ADDR: u16 = 0x1000;
const IRQ_TIMER_BASE: u16 = 0x1400;
const CPU_IRQ_MASK: u16 = 0x1402;
const CPU_IRQ_STATUS: u16 = 0x1403;
const VDC_CTRL_DISPLAY_FULL: u16 = VDC_CTRL_ENABLE_BACKGROUND
    | VDC_CTRL_ENABLE_BACKGROUND_LEGACY
    | VDC_CTRL_ENABLE_SPRITES
    | VDC_CTRL_ENABLE_SPRITES_LEGACY;

fn set_vdc_control(bus: &mut Bus, value: u16) {
    bus.write_st_port(0, 0x05);
    bus.write_st_port(1, (value & 0x00FF) as u8);
    bus.write_st_port(2, (value >> 8) as u8);
}

fn prepare_bus_for_zoom() -> Bus {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    set_vdc_control(&mut bus, VDC_CTRL_DISPLAY_FULL);

    const MAP_WIDTH: usize = 32;
    for col in 0..MAP_WIDTH {
        let tile_id = 0x100 + col;
        let palette_bank = (col & 0x0F) as usize;
        bus.vdc.vram[col] = ((tile_id as u16) & 0x07FF) | ((palette_bank as u16) << 12);
        let base = (tile_id * 16) & 0x7FFF;
        for row in 0..8 {
            bus.vdc.vram[(base + row) & 0x7FFF] = 0x00FF;
            bus.vdc.vram[(base + row + 8) & 0x7FFF] = 0x0000;
        }
    }

    for bank in 0..16 {
        let colour = (bank as u16) * 0x041;
        bus.vce.palette[(bank << 4) | 1] = colour;
    }

    bus
}

fn render_zoom_pair(zoom_x: u16) -> ([u32; FRAME_WIDTH], [u32; FRAME_WIDTH]) {
    let mut baseline = prepare_bus_for_zoom();
    baseline.render_frame_from_vram();
    let mut zoomed = prepare_bus_for_zoom();
    zoomed.vdc.set_zoom_for_test(zoom_x, 0x0010);
    zoomed.render_frame_from_vram();

    let mut base_line = [0u32; FRAME_WIDTH];
    let mut zoom_line = [0u32; FRAME_WIDTH];
    base_line.copy_from_slice(&baseline.framebuffer[0..FRAME_WIDTH]);
    zoom_line.copy_from_slice(&zoomed.framebuffer[0..FRAME_WIDTH]);
    (base_line, zoom_line)
}

fn prepare_bus_for_vertical_zoom() -> Bus {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    set_vdc_control(&mut bus, VDC_CTRL_DISPLAY_FULL);

    const MAP_WIDTH: usize = 32;
    for row in 0..32 {
        let tile_id = 0x200 + row * MAP_WIDTH;
        let palette_bank = (row & 0x0F) as usize;
        for col in 0..MAP_WIDTH {
            let idx = row * MAP_WIDTH + col;
            bus.vdc.vram[idx] = ((tile_id as u16) & 0x07FF) | ((palette_bank as u16) << 12);
        }
        let base = (tile_id * 16) & 0x7FFF;
        for line in 0..8 {
            bus.vdc.vram[(base + line) & 0x7FFF] = 0x00FF;
            bus.vdc.vram[(base + line + 8) & 0x7FFF] = 0x0000;
        }
    }

    for bank in 0..16 {
        let colour = 0x0100 | ((bank as u16) * 0x021);
        bus.vce.palette[(bank << 4) | 1] = colour;
    }

    bus
}

fn render_vertical_zoom_pair(zoom_y: u16) -> (Vec<u32>, Vec<u32>) {
    let mut baseline = prepare_bus_for_vertical_zoom();
    baseline.render_frame_from_vram();
    let mut zoomed = prepare_bus_for_vertical_zoom();
    zoomed.vdc.set_zoom_for_test(0x0010, zoom_y);
    zoomed.render_frame_from_vram();
    (baseline.framebuffer.clone(), zoomed.framebuffer.clone())
}

#[test]
fn load_and_bank_switch_rom() {
    let mut bus = Bus::new();
    bus.load(0x0000, &[0xAA, 0xBB]);
    assert_eq!(bus.read(0x0000), 0xAA);

    bus.load_rom_image(vec![0x10; PAGE_SIZE * 2]);
    bus.map_bank_to_rom(4, 1);
    assert_eq!(bus.read(0x8000), 0x10);

    bus.write(0x8000, 0x77); // ignored because ROM
    assert_eq!(bus.read(0x8000), 0x10);

    bus.map_bank_to_ram(4, 0);
    bus.write(0x8000, 0x12);
    assert_eq!(bus.read(0x8000), 0x12);
}

#[test]
fn large_hucard_mapper_switches_selectable_512k_window() {
    let mut rom = vec![0xFF; PAGE_SIZE * 320];
    for page in 0..320usize {
        let offset = page * PAGE_SIZE;
        rom[offset] = (page & 0xFF) as u8;
        rom[offset + 1] = (page >> 8) as u8;
    }

    let mut bus = Bus::new();
    bus.load_rom_image(rom);
    bus.enable_large_hucard_mapper();

    bus.set_mpr(4, 0x20);
    assert_eq!((bus.read(0x8001), bus.read(0x8000)), (0x00, 0x20));

    bus.set_mpr(4, 0x40);
    assert_eq!((bus.read(0x8001), bus.read(0x8000)), (0x00, 0x40));

    bus.set_mpr(0, 0x00);
    bus.write(0x1FF2, 0x00);
    assert_eq!((bus.read(0x8001), bus.read(0x8000)), (0x00, 0xC0));

    bus.write(0x1FF3, 0x00);
    bus.set_mpr(4, 0x7F);
    assert_eq!((bus.read(0x8001), bus.read(0x8000)), (0x01, 0x3F));
}

#[test]
fn mpr_mirrors_apply_across_high_page() {
    let mut bus = Bus::new();
    bus.load_rom_image(vec![0x55; PAGE_SIZE * 2]);

    // MPR registers at $FF80-$FFBF are only accessible when MPR7
    // maps to the hardware page ($FF).
    bus.set_mpr(7, 0xFF);

    // 0xFF95 mirrors MPR5
    bus.write(0xFF95, (bus.total_ram_pages() + 1) as u8);
    assert_eq!(bus.mpr(5), (bus.total_ram_pages() + 1) as u8);

    // ROM page 1 is filled with 0x55
    assert_eq!(bus.read(0xA000), 0x55);

    // Reading from a mirror location returns the same register value.
    assert_eq!(bus.read(0xFFAD), bus.mpr(5));
}

#[test]
fn io_port_reads_selected_joypad_nibble() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);
    bus.set_joypad_input(0x5A);

    // SEL=1 -> d-pad (lower nibble of input)
    bus.write(JOYPAD_BASE_ADDR, 0x01);
    assert_eq!(bus.read(JOYPAD_BASE_ADDR) & 0x0F, 0x0A);

    // SEL=0 -> buttons (upper nibble of input)
    bus.write(JOYPAD_BASE_ADDR, 0x00);
    assert_eq!(bus.read(JOYPAD_BASE_ADDR) & 0x0F, 0x05);
}

#[test]
fn st_ports_store_values() {
    let mut bus = Bus::new();
    bus.write_st_port(0, 0x12);
    bus.write_st_port(1, 0x34);
    bus.write_st_port(2, 0x56);
    assert_eq!(bus.st_port(0), 0x12);
    assert_eq!(bus.st_port(1), 0x34);
    assert_eq!(bus.st_port(2), 0x56);
}

#[test]
fn io_registers_round_trip_and_reset() {
    let mut bus = Bus::new();
    // Test I/O register round-trip via direct read_io/write_io
    assert_eq!(bus.read_io(HW_CPU_CTRL_BASE + 0x20), 0);
    assert_eq!(bus.read_io(HW_CPU_CTRL_BASE + 0x7F), 0);

    bus.write_io(HW_CPU_CTRL_BASE + 0x20, 0xAA);
    assert_eq!(bus.read_io(HW_CPU_CTRL_BASE + 0x20), 0xAA);
    bus.write_io(HW_CPU_CTRL_BASE + 0x7F, 0x55);
    assert_eq!(bus.read_io(HW_CPU_CTRL_BASE + 0x7F), 0x55);

    bus.write_io(HW_CPU_CTRL_BASE + 0x30, 0x42);
    assert_eq!(bus.read_io(HW_CPU_CTRL_BASE + 0x30), 0x42);

    bus.clear();
    assert_eq!(bus.read_io(HW_CPU_CTRL_BASE + 0x20), 0x00);
    assert_eq!(bus.read_io(HW_CPU_CTRL_BASE + 0x30), 0x00);
    assert_eq!(bus.read_io(HW_CPU_CTRL_BASE + 0x7F), 0x00);
}

#[test]
fn timer_borrow_sets_request_bit() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);
    bus.write(0x0C00, 0x02); // reload value
    bus.write(0x0C01, TIMER_CONTROL_START);

    let fired = bus.tick(1024u32 * 3, true);
    assert!(fired);
    assert_eq!(bus.read(0x1403) & IRQ_REQUEST_TIMER, IRQ_REQUEST_TIMER);

    bus.write(0x1403, IRQ_REQUEST_TIMER);
    assert_eq!(bus.read(0x1403) & IRQ_REQUEST_TIMER, 0);
}

#[test]
fn timer_accessible_via_standard_io_offset() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    bus.write(TIMER_STD_BASE, 0x02);
    bus.write(TIMER_STD_BASE + 1, TIMER_CONTROL_START);

    let fired = bus.tick(1024u32 * 3, true);
    assert!(fired);
    assert_eq!(
        bus.read(CPU_IRQ_STATUS) & IRQ_REQUEST_TIMER,
        IRQ_REQUEST_TIMER
    );

    bus.write(CPU_IRQ_STATUS, IRQ_REQUEST_TIMER);
    assert_eq!(bus.read(CPU_IRQ_STATUS) & IRQ_REQUEST_TIMER, 0);
}

#[test]
fn irq_registers_not_aliased_to_timer() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    bus.write(TIMER_STD_BASE, 0x05);
    bus.write(TIMER_STD_BASE + 1, TIMER_CONTROL_START);
    assert_eq!(bus.read(TIMER_STD_BASE + 1) & TIMER_CONTROL_START, 1);

    bus.write(IRQ_TIMER_BASE, 0xAA);
    bus.write(IRQ_TIMER_BASE + 1, 0x55);

    assert_eq!(bus.read(IRQ_TIMER_BASE), 0xAA);
    assert_eq!(bus.read(IRQ_TIMER_BASE + 1), 0x55);
    assert_eq!(bus.read(TIMER_STD_BASE + 1) & TIMER_CONTROL_START, 1);
}

#[test]
fn hardware_page_irq_registers_alias_cpu_space() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    bus.write(IRQ_TIMER_BASE + 0x02, 0xFF);
    assert_eq!(
        bus.read(CPU_IRQ_MASK),
        IRQ_DISABLE_IRQ2 | IRQ_DISABLE_IRQ1 | IRQ_DISABLE_TIMER
    );

    bus.write(CPU_IRQ_MASK, 0x00);
    bus.write(IRQ_TIMER_BASE + 0x03, IRQ_REQUEST_TIMER);
    assert_eq!(bus.read(CPU_IRQ_STATUS) & IRQ_REQUEST_TIMER, 0);
}

#[test]
fn cart_ram_banks_map_into_memory_space() {
    let mut bus = Bus::new();
    bus.configure_cart_ram(PAGE_SIZE * 2);

    let cart_base = 0x80u8;
    bus.set_mpr(2, cart_base);
    bus.write(0x4000, 0x5A);
    assert_eq!(bus.cart_ram[0], 0x5A);
    assert_eq!(bus.read(0x4000), 0x5A);

    bus.set_mpr(2, cart_base + 1);
    bus.write(0x4000, 0xCC);
    assert_eq!(bus.cart_ram[PAGE_SIZE], 0xCC);
    assert_eq!(bus.read(0x4000), 0xCC);

    bus.set_mpr(2, cart_base);
    assert_eq!(bus.read(0x4000), 0x5A);
}

#[test]
fn cart_ram_load_and_snapshot_round_trip() {
    let mut bus = Bus::new();
    bus.configure_cart_ram(PAGE_SIZE);
    let pattern = vec![0xAB; PAGE_SIZE];
    assert!(bus.load_cart_ram(&pattern).is_ok());
    assert_eq!(bus.cart_ram().unwrap()[0], 0xAB);
    let cart_base = 0x80u8;
    bus.set_mpr(2, cart_base);
    let cart_addr = 0x4000u16;
    assert_eq!(bus.read(cart_addr), 0xAB);

    if let Some(data) = bus.cart_ram_mut() {
        data.fill(0x11);
    }
    assert_eq!(bus.read(cart_addr), 0x11);
}

#[test]
fn bram_is_locked_until_unlocked_via_1807_and_relocked_by_1803_read() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);
    bus.set_mpr(2, 0xF7);
    let bram_addr = 0x4000u16;

    assert_eq!(bus.read(bram_addr), 0xFF);
    bus.write(bram_addr, 0x12);
    assert_eq!(bus.read(bram_addr), 0xFF);

    bus.write(0x1807, 0x80);
    assert!(bus.bram_unlocked());

    bus.write(bram_addr, 0x34);
    assert_eq!(bus.read(bram_addr), 0x34);

    assert_eq!(bus.read(0x1803), 0xFF);
    assert!(!bus.bram_unlocked());
    assert_eq!(bus.read(bram_addr), 0xFF);

    bus.write(0x1807, 0x80);
    assert_eq!(bus.read(bram_addr), 0x34);
}

#[test]
fn bram_default_image_starts_with_formatted_header() {
    let bus = Bus::new();
    assert_eq!(&bus.bram()[..BRAM_FORMAT_HEADER.len()], &BRAM_FORMAT_HEADER);
}

#[test]
fn bram_load_accepts_blank_legacy_2k_image_and_repairs_header() {
    let mut bus = Bus::new();
    let mut legacy = vec![0; 0x0800];
    legacy[0x20] = 0x5A;

    bus.load_bram(&legacy).unwrap();

    assert_eq!(&bus.bram()[..BRAM_FORMAT_HEADER.len()], &BRAM_FORMAT_HEADER);
    assert_eq!(bus.bram()[0x20], 0x5A);
}

#[test]
fn bram_load_accepts_full_f7_page_dump() {
    let mut bus = Bus::new();
    let mut dump = vec![0xCC; PAGE_SIZE];
    dump[..BRAM_FORMAT_HEADER.len()].copy_from_slice(&BRAM_FORMAT_HEADER);
    dump[0x07FF] = 0xA5;

    bus.load_bram(&dump).unwrap();

    assert_eq!(&bus.bram()[..BRAM_FORMAT_HEADER.len()], &BRAM_FORMAT_HEADER);
    assert_eq!(bus.bram()[0x07FF], 0xA5);
}

#[test]
fn bram_cd_status_space_reads_as_ff_without_cd_hardware() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    assert_eq!(bus.read(0x1800), 0xFF);
    assert_eq!(bus.read(0x1807), 0xFF);
    assert_eq!(bus.read(0x1BFF), 0xFF);
}

#[test]
fn bram_maps_only_first_2k_of_f7_page() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);
    bus.set_mpr(2, 0xF7);
    bus.write(0x1807, 0x80);

    bus.write(0x47FF, 0xAA);
    bus.write(0x4800, 0x55);

    assert_eq!(bus.read(0x47FF), 0xAA);
    assert_eq!(bus.read(0x4800), 0xFF);
    assert_eq!(bus.bram()[0x07FF], 0xAA);
}

#[test]
fn sprite_priority_respects_background_mask() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    set_vdc_control(&mut bus, VDC_CTRL_DISPLAY_FULL);
    bus.write_st_port(0, 0x09);
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x00);

    const BG_TILE_ID: usize = 200;
    const SPRITE_PATTERN_ID: usize = 201;
    const BG_PALETTE_BANK: usize = 1;
    const SPRITE_PALETTE_BANK: usize = 2;

    for entry in bus.vdc.vram.iter_mut().take(32 * 32) {
        *entry = ((BG_TILE_ID as u16) & 0x07FF) | ((BG_PALETTE_BANK as u16) << 12);
    }

    let bg_base = BG_TILE_ID * 16;
    for row in 0..8 {
        bus.vdc.vram[bg_base + row] = 0xFFFF;
        bus.vdc.vram[bg_base + 8 + row] = 0xFFFF;
    }

    write_constant_sprite_tile(&mut bus, SPRITE_PATTERN_ID, 0x01);

    bus.vce.palette[0x1F] = 0x001F;
    bus.vce.palette[0x121] = 0x03E0;

    bus.render_frame_from_vram();
    let bg_colour = bus.framebuffer[0];
    assert_ne!(bg_colour, 0);
    assert!(bus.bg_opaque[0]);

    let satb_index = 0;
    // SAT Y=64 puts sprite at screen row 0: screen Y = SAT_Y - 64
    let y_word = ((0 + 64) & 0x03FF) as u16;
    let x_word = ((0 + 32) & 0x03FF) as u16;
    bus.vdc.satb[satb_index] = y_word;
    bus.vdc.satb[satb_index + 1] = x_word;
    bus.vdc.satb[satb_index + 2] = (SPRITE_PATTERN_ID as u16) << 1;
    bus.vdc.satb[satb_index + 3] = SPRITE_PALETTE_BANK as u16;

    bus.render_frame_from_vram();
    assert_eq!(bus.framebuffer[0], bg_colour);

    bus.vdc.satb[satb_index + 3] |= 0x0080;
    bus.render_frame_from_vram();
    let sprite_colour = bus.vce.palette_rgb(0x121);
    assert_eq!(bus.framebuffer[0], sprite_colour);
}

fn write_constant_sprite_tile(bus: &mut Bus, pattern_index: usize, value: u8) {
    let base = (pattern_index * SPRITE_PATTERN_WORDS) & 0x7FFF;
    let plane0 = if value & 0x01 != 0 { 0xFFFF } else { 0x0000 };
    let plane1 = if value & 0x02 != 0 { 0xFFFF } else { 0x0000 };
    let plane2 = if value & 0x04 != 0 { 0xFFFF } else { 0x0000 };
    let plane3 = if value & 0x08 != 0 { 0xFFFF } else { 0x0000 };
    for row in 0..SPRITE_PATTERN_HEIGHT {
        bus.vdc.vram[(base + row) & 0x7FFF] = plane0;
        bus.vdc.vram[(base + 16 + row) & 0x7FFF] = plane1;
        bus.vdc.vram[(base + 32 + row) & 0x7FFF] = plane2;
        bus.vdc.vram[(base + 48 + row) & 0x7FFF] = plane3;
    }
}

#[test]
fn sprite_render_uses_frame_boundary_vram_snapshot() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    set_vdc_control(&mut bus, VDC_CTRL_DISPLAY_FULL);
    bus.vce.palette[0x101] = 0x0001;
    bus.vce.palette[0x102] = 0x0010;

    let sprite_y = 32;
    let sprite_x = 24;
    write_constant_sprite_tile(&mut bus, 0, 0x01);
    bus.vdc.satb[0] = ((sprite_y + 64) & 0x03FF) as u16;
    bus.vdc.satb[1] = ((sprite_x + 32) & 0x03FF) as u16;
    bus.vdc.satb[2] = 0x0000;
    bus.vdc.satb[3] = 0x0000;
    bus.capture_sprite_vram_snapshot();

    write_constant_sprite_tile(&mut bus, 0, 0x02);
    bus.render_frame_from_vram();

    let sprite_pixel = sprite_y as usize * FRAME_WIDTH + sprite_x as usize;
    assert_eq!(bus.framebuffer[sprite_pixel], bus.vce.palette_rgb(0x101));
}

#[test]
fn tick_captures_sprite_vram_snapshot_on_first_active_line() {
    let mut bus = Bus::new();
    bus.vdc.registers[0x0C] = 0x0F02;
    bus.vdc.registers[0x0D] = 0x00EF;
    bus.vdc.registers[0x0E] = 0x0003;
    let active_start = bus.vdc.vertical_window().active_start_line as u16;

    write_constant_sprite_tile(&mut bus, 0, 0x03);
    bus.vdc.scanline = active_start - 1;
    bus.vdc.in_vblank = false;
    bus.vdc.phi_scaled = VDC_VBLANK_INTERVAL as u64;

    bus.tick(1, true);

    assert_eq!(bus.vdc.scanline, active_start);
    assert_eq!(bus.sprite_vram_snapshot.0.len(), bus.vdc.vram.len());
    assert_eq!(
        &bus.sprite_vram_snapshot.0[..SPRITE_PATTERN_WORDS],
        &bus.vdc.vram[..SPRITE_PATTERN_WORDS]
    );
}

#[test]
fn sprites_render_when_background_disabled() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    bus.vce.palette[0x00] = 0x0000;
    bus.vce.palette[0x100] = 0x0000;
    bus.vce.palette[0x101] = 0x7C00;

    write_constant_sprite_tile(&mut bus, 0, 0x01);

    let sprite_y = 32;
    let sprite_x = 24;
    // SAT Y = screen_y + 64: screen Y = SAT_Y - 64
    bus.vdc.satb[0] = ((sprite_y + 64) & 0x03FF) as u16;
    bus.vdc.satb[1] = ((sprite_x + 32) & 0x03FF) as u16;
    bus.vdc.satb[2] = 0x0000;
    bus.vdc.satb[3] = 0x0000;

    bus.write_st_port(0, 0x05);
    bus.write_st_port(1, 0x40);
    bus.write_st_port(2, 0x00);

    bus.render_frame_from_vram();

    let background_colour = bus.vce.palette_rgb(0x00);
    assert_eq!(bus.framebuffer[0], background_colour);

    let sprite_index = sprite_y as usize * FRAME_WIDTH + sprite_x as usize;
    let sprite_colour = bus.vce.palette_rgb(0x101);
    assert_eq!(bus.framebuffer[sprite_index], sprite_colour);
    assert!(
        bus.framebuffer.iter().any(|&pixel| pixel == sprite_colour),
        "expected sprite colour to appear in framebuffer"
    );
}

#[test]
fn sprite_double_width_draws_all_columns() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    set_vdc_control(&mut bus, VDC_CTRL_DISPLAY_FULL);

    const BASE_PATTERN: usize = 0x200;
    bus.vce.palette[0x101] = 0x0111;
    bus.vce.palette[0x102] = 0x0222;
    write_constant_sprite_tile(&mut bus, BASE_PATTERN, 0x01);
    write_constant_sprite_tile(&mut bus, BASE_PATTERN + 1, 0x02);

    let sprite_base = 0;
    let sprite_y = 32;
    let sprite_x = 24;
    // SAT Y = screen_y + 64: screen Y = SAT_Y - 64
    bus.vdc.satb[sprite_base] = ((sprite_y + 64) & 0x03FF) as u16;
    bus.vdc.satb[sprite_base + 1] = ((sprite_x + 32) & 0x03FF) as u16;
    bus.vdc.satb[sprite_base + 2] = (BASE_PATTERN as u16) << 1;
    bus.vdc.satb[sprite_base + 3] = 0x0100 | 0x0080;

    bus.render_frame_from_vram();

    let row_start = sprite_y * FRAME_WIDTH + sprite_x;
    let left = bus.framebuffer[row_start];
    let right = bus.framebuffer[row_start + 16];
    assert_eq!(left, bus.vce.palette_rgb(0x101));
    assert_eq!(right, bus.vce.palette_rgb(0x102));
}

#[test]
fn sprite_scanline_overflow_sets_status() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    const TILE_ID: usize = 0x400;
    write_constant_sprite_tile(&mut bus, TILE_ID, 0x01);

    set_vdc_control(&mut bus, VDC_CTRL_DISPLAY_FULL);

    let y_pos = 48;
    for sprite in 0..17 {
        let base = sprite * 4;
        let x_pos = sprite as i32 * 8;
        bus.vdc.satb[base] = ((y_pos + 64) & 0x03FF) as u16;
        bus.vdc.satb[base + 1] = ((x_pos + 32) & 0x03FF) as u16;
        bus.vdc.satb[base + 2] = (TILE_ID as u16) << 1;
        bus.vdc.satb[base + 3] = 0x0000;
    }

    bus.render_frame_from_vram();
    let max_count = bus
        .sprite_line_counts_for_test()
        .iter()
        .copied()
        .max()
        .unwrap_or(0);
    assert_eq!(max_count, 16);
    assert_ne!(bus.vdc.status_bits() & VDC_STATUS_OR, 0);

    let overflow_sprite = 16 * 4;
    bus.vdc.satb[overflow_sprite] = 0;
    bus.vdc.satb[overflow_sprite + 1] = 0;
    bus.vdc.satb[overflow_sprite + 2] = 0;
    bus.vdc.satb[overflow_sprite + 3] = 0;

    bus.render_frame_from_vram();
    assert_eq!(bus.vdc.status_bits() & VDC_STATUS_OR, 0);
}

#[test]
fn sprite_size_scaling_plots_full_extent() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    set_vdc_control(&mut bus, VDC_CTRL_DISPLAY_FULL);

    const BASE_TILE: usize = 0x300;
    const WIDTH_UNITS: usize = 2;
    const HEIGHT_UNITS: usize = 2;
    const WIDTH_TILES: usize = WIDTH_UNITS;
    const HEIGHT_TILES: usize = HEIGHT_UNITS;

    for tile in 0..(WIDTH_TILES * HEIGHT_TILES) {
        write_constant_sprite_tile(&mut bus, BASE_TILE + tile, 0x0F);
    }

    let sprite_colour = 0x7C00;
    bus.vce.palette[0x12F] = sprite_colour;

    let x_pos = 40;
    let y_pos = 32;
    let satb_index = 0;
    // SAT Y = screen_y + 64: screen Y = SAT_Y - 64
    bus.vdc.satb[satb_index] = ((y_pos + 64) & 0x03FF) as u16;
    bus.vdc.satb[satb_index + 1] = ((x_pos + 32) & 0x03FF) as u16;
    bus.vdc.satb[satb_index + 2] = (BASE_TILE as u16) << 1;
    bus.vdc.satb[satb_index + 3] = 0x1000 | 0x0100 | 0x0002;

    bus.render_frame_from_vram();

    let colour = bus.vce.palette_rgb(0x12F);
    let idx = (y_pos + HEIGHT_UNITS * 16 - 1) * FRAME_WIDTH + (x_pos + WIDTH_UNITS * 16 - 1);
    assert_eq!(bus.framebuffer[idx], colour);
}

#[test]
fn sprite_quad_height_plots_bottom_row() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    const BASE_TILE: usize = 0x320;
    const TILES_WIDE: usize = 1;
    const TILES_HIGH: usize = 4;

    set_vdc_control(&mut bus, VDC_CTRL_DISPLAY_FULL);

    for cell_y in 0..TILES_HIGH {
        for cell_x in 0..TILES_WIDE {
            write_constant_sprite_tile(&mut bus, BASE_TILE + cell_y * 2 + cell_x, 0x0F);
        }
    }

    let sprite_colour = 0x03FF;
    bus.vce.palette[0x10F] = sprite_colour;

    let x_pos = 24;
    let y_pos = 40;
    let satb_index = 0;
    // SAT Y = screen_y + 64: screen Y = SAT_Y - 64
    bus.vdc.satb[satb_index] = ((y_pos + 64) & 0x03FF) as u16;
    bus.vdc.satb[satb_index + 1] = ((x_pos + 32) & 0x03FF) as u16;
    bus.vdc.satb[satb_index + 2] = (BASE_TILE as u16) << 1;
    bus.vdc.satb[satb_index + 3] = 0x2000;

    bus.render_frame_from_vram();

    let expected = bus.vce.palette_rgb(0x10F);
    let drawn_pixels = bus
        .framebuffer
        .iter()
        .filter(|&&pixel| pixel == expected)
        .count();
    assert!(drawn_pixels > 0);
    let top_row = &bus.framebuffer[y_pos * FRAME_WIDTH..(y_pos + 1) * FRAME_WIDTH];
    assert!(top_row.iter().any(|&pixel| pixel == expected));
    let bottom_row = &bus.framebuffer[(y_pos + 63) * FRAME_WIDTH..(y_pos + 64) * FRAME_WIDTH];
    assert!(bottom_row.iter().any(|&pixel| pixel == expected));
}

#[test]
fn sprite_quad_height_masks_pattern_bits_one_and_two() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    set_vdc_control(&mut bus, VDC_CTRL_DISPLAY_FULL);

    const MASKED_TILE: usize = 0x320;
    const UNMASKED_BY_OLD_CODE_TILE: usize = 0x322;
    const SAT_TILE: usize = 0x326;

    write_constant_sprite_tile(&mut bus, MASKED_TILE, 0x01);
    write_constant_sprite_tile(&mut bus, UNMASKED_BY_OLD_CODE_TILE, 0x02);
    bus.vce.palette[0x101] = 0x0001;
    bus.vce.palette[0x102] = 0x0010;

    let x_pos = 24;
    let y_pos = 40;
    let satb_index = 0;
    bus.vdc.satb[satb_index] = ((y_pos + 64) & 0x03FF) as u16;
    bus.vdc.satb[satb_index + 1] = ((x_pos + 32) & 0x03FF) as u16;
    bus.vdc.satb[satb_index + 2] = (SAT_TILE as u16) << 1;
    bus.vdc.satb[satb_index + 3] = 0x2000 | 0x0080;

    bus.render_frame_from_vram();

    let top_left_pixel = y_pos * FRAME_WIDTH + x_pos;
    assert_eq!(bus.framebuffer[top_left_pixel], bus.vce.palette_rgb(0x101));
}

#[test]
fn scroll_registers_latch_on_scanline_boundary() {
    let mut vdc = Vdc::new();
    let (x0, y0) = vdc.scroll_for_scanline();
    assert_eq!(x0, 0);
    assert_eq!(y0, 0);

    vdc.write_select(0x07);
    vdc.write_data_low(0x34);
    vdc.write_data_high(0x12);
    let (x1, y1) = vdc.scroll_for_scanline();
    assert_eq!(x1, 0x1234 & 0x03FF);
    assert_eq!(y1, 0);

    vdc.write_select(0x08);
    vdc.write_data_low(0x78);
    vdc.write_data_high(0x05);
    let (x2, y2) = vdc.scroll_for_scanline();
    assert_eq!(x2, x1);
    assert_eq!(y2, 0x0578 & 0x01FF);

    let (x3, y3) = vdc.scroll_for_scanline();
    assert_eq!(x3, x2);
    assert_eq!(y3, y2);
}

#[test]
fn scroll_writes_apply_on_next_visible_scanline() {
    let mut vdc = Vdc::new();
    vdc.advance_scanline_for_test();
    let (x0, _, _) = vdc.scroll_values_for_line(0);
    assert_eq!(x0, 0);

    vdc.write_select(0x07);
    vdc.write_data_low(0x34);
    vdc.write_data_high(0x12);

    let (x_still, _, _) = vdc.scroll_values_for_line(0);
    assert_eq!(x_still, 0);

    vdc.advance_scanline_for_test();
    let (x1, _, _) = vdc.scroll_values_for_line(1);
    assert_eq!(x1, 0x1234 & 0x03FF);

    let (x_now, _) = vdc.scroll_for_scanline();
    assert_eq!(x_now, 0x1234 & 0x03FF);
}

#[test]
fn rcr_isr_scroll_writes_apply_on_following_scanline() {
    let mut vdc = Vdc::new();
    vdc.in_vblank = false;
    vdc.scroll_x = 4;
    vdc.scroll_y = 0;
    vdc.registers[0x04] = 0x008C;
    vdc.registers[0x05] = 0x008C;
    vdc.render_control_latch = 0x008C;

    vdc.latch_line_state(10);

    vdc.scroll_x_pending = 51;
    vdc.scroll_x_dirty = true;
    vdc.scroll_y_pending = 77;
    vdc.scroll_y_dirty = true;
    vdc.registers[0x04] = 0x00CC;
    vdc.registers[0x05] = 0x00CC;
    vdc.render_control_latch = 0x00CC;
    vdc.consume_post_isr_scroll(10);

    assert_eq!(vdc.scroll_values_for_line(10), (4, 0, 0));
    assert_eq!(vdc.control_values_for_line(10), 0x008C);

    vdc.latch_line_state(11);
    // BYR writes during the active area take effect after the h-sync latch,
    // so the first visible line starts at new_BYR + 1.
    assert_eq!(vdc.scroll_values_for_line(11), (51, 77, 1));
    assert_eq!(vdc.control_values_for_line(11), 0x00CC);
}

#[test]
fn register_select_between_low_and_high_uses_current_ar() {
    let mut vdc = Vdc::new();

    vdc.write_select(0x07); // AR = BXR
    vdc.write_data_low(0x34); // BXR low byte committed immediately → BXR = 0x0034
    // On real HuC6270, changing AR between ST1 and ST2 means the
    // high-byte commit targets the NEW register, not the old one.
    // Per MAME, each register has its own data latch (m_vdc_data[]),
    // so the low byte 0x34 stays in BXR's latch, NOT BYR's.
    vdc.write_select(0x08); // AR = BYR
    vdc.write_data_high_direct(0x12); // commits vdc_data[8]=(0x12,0x00) to BYR

    // BXR only received the low-byte commit (0x0034).
    assert_eq!(vdc.registers[0x07], 0x0034);
    // BYR received (high=0x12, low=0x00 from its own latch), masked to 9 bits.
    // The 0x34 from BXR's ST1 does NOT leak to BYR's data latch.
    assert_eq!(vdc.registers[0x08], 0x1200 & 0x01FF);
}

#[test]
fn vdc_vertical_window_uses_vpr_vdw_vcr() {
    let mut vdc = Vdc::new();
    vdc.registers[0x0C] = 0x0F02;
    vdc.registers[0x0D] = 0x00EF;
    vdc.registers[0x0E] = 0x0003;

    let window = vdc.vertical_window();
    assert_eq!(window.active_start_line, 0x14);
    assert_eq!(window.active_line_count, 0x0F0);
    assert_eq!(window.post_active_overscan_lines, 6);
    assert_eq!(window.vblank_start_line, 260);
    assert_eq!(vdc.vblank_start_scanline(), 260);
}

#[test]
fn vdc_output_row_active_window_honours_vdw_vcr_gap() {
    let mut vdc = Vdc::new();
    vdc.registers[0x0C] = 0x0100;
    vdc.registers[0x0D] = 0x0003; // 4 active lines
    vdc.registers[0x0E] = 0x0002; // 5 overscan lines (VCR + 3)

    for row in 0..4 {
        assert!(
            vdc.output_row_in_active_window(row),
            "row {row} should be active in first display pass"
        );
    }
    for row in 4..13 {
        assert!(
            !vdc.output_row_in_active_window(row),
            "row {row} should be overscan"
        );
    }
    for row in 13..17 {
        assert!(
            vdc.output_row_in_active_window(row),
            "row {row} should be active after display-counter reset"
        );
    }
}

#[test]
fn vdc_vertical_timing_registers_ignore_writes_during_active_display() {
    let mut vdc = Vdc::new();
    vdc.registers[0x0C] = 0x0100;
    vdc.registers[0x0D] = 0x0003;
    vdc.registers[0x0E] = 0x0002;
    vdc.in_vblank = false;
    vdc.scanline = vdc.vertical_window().active_start_line as u16;

    vdc.write_select(0x0C);
    vdc.write_data_low(0x34);
    vdc.write_data_high_direct(0x12);
    vdc.write_select(0x0D);
    vdc.write_data_low(0x78);
    vdc.write_data_high_direct(0x56);
    vdc.write_select(0x0E);
    vdc.write_data_low(0xBC);
    vdc.write_data_high_direct(0x9A);

    assert_eq!(vdc.registers[0x0C], 0x0100);
    assert_eq!(vdc.registers[0x0D], 0x0003);
    assert_eq!(vdc.registers[0x0E], 0x0002);
}

#[test]
fn vdc_vertical_timing_registers_allow_initial_programming_during_active_display() {
    let mut vdc = Vdc::new();
    vdc.in_vblank = false;
    vdc.scanline = 108;

    vdc.write_select(0x0C);
    vdc.write_data_low(0x05);
    vdc.write_data_high_direct(0x1A);
    vdc.write_select(0x0D);
    vdc.write_data_low(0xCF);
    vdc.write_data_high_direct(0x00);
    vdc.write_select(0x0E);
    vdc.write_data_low(0x0A);
    vdc.write_data_high_direct(0x00);

    assert_eq!(vdc.registers[0x0C], 0x1A05);
    assert_eq!(vdc.registers[0x0D], 0x00CF);
    assert_eq!(vdc.registers[0x0E], 0x000A);
}

#[test]
fn vdc_vertical_timing_registers_update_outside_active_display() {
    let mut vdc = Vdc::new();
    vdc.in_vblank = true;

    vdc.write_select(0x0C);
    vdc.write_data_low(0x34);
    vdc.write_data_high_direct(0x12);
    vdc.write_select(0x0D);
    vdc.write_data_low(0x78);
    vdc.write_data_high_direct(0x56);
    vdc.write_select(0x0E);
    vdc.write_data_low(0xBC);
    vdc.write_data_high_direct(0x9A);

    assert_eq!(vdc.registers[0x0C], 0x1234);
    assert_eq!(vdc.registers[0x0D], 0x5678);
    assert_eq!(vdc.registers[0x0E], 0x9ABC);
}

#[test]
fn line_state_index_tracks_vpr_active_start() {
    let mut vdc = Vdc::new();
    vdc.registers[0x0C] = 0x0302; // VSW=2, VDS=3 => start line (2+1)+(3+2)=8
    vdc.registers[0x0D] = 0x0001;

    assert_eq!(vdc.line_state_index_for_frame_row(0), 8);
    assert_eq!(
        vdc.line_state_index_for_frame_row(239),
        (8 + 239) % (LINES_PER_FRAME as usize)
    );
}

#[test]
fn rcr_scanline_uses_active_start_offset() {
    let mut vdc = Vdc::new();
    // VPR: VSW=2 VDS=15 → active_start = vsw+1+vds+2 = 20
    vdc.registers[0x0C] = 0x0F02;
    vdc.registers[0x0D] = 0x00EF;
    vdc.registers[0x0E] = 0x0003;

    // target 0x40: counter = 0x40 at active_start → line = 20
    assert_eq!(vdc.rcr_scanline_for_target(0x0040), Some(20));
    // target 0x63: line = 20 + 35 = 55 (Kato-chan HUD split)
    assert_eq!(vdc.rcr_scanline_for_target(0x0063), Some(55));
    // target below 0x40: not reachable
    assert_eq!(vdc.rcr_scanline_for_target(0x0002), None);
}

#[test]
fn map_dimensions_follow_mwr_width_height_bits() {
    let mut vdc = Vdc::new();

    vdc.registers[0x09] = 0x0000;
    assert_eq!(vdc.map_dimensions(), (32, 32));

    vdc.registers[0x09] = 0x0010;
    assert_eq!(vdc.map_dimensions(), (64, 32));

    vdc.registers[0x09] = 0x0020;
    assert_eq!(vdc.map_dimensions(), (128, 32));

    vdc.registers[0x09] = 0x0030;
    assert_eq!(vdc.map_dimensions(), (128, 32));

    vdc.registers[0x09] = 0x0050;
    assert_eq!(vdc.map_dimensions(), (64, 64));
}

#[test]
fn map_entry_address_64x64_flat() {
    let mut vdc = Vdc::new();
    vdc.registers[0x09] = 0x0050; // 64x64

    // HuC6270 BAT uses flat row-major addressing (MAME/Mednafen):
    //   address = row * map_width + col
    assert_eq!(vdc.map_entry_address_for_test(0, 0), 0x0000);
    // Row 1, col 0: 1*64 = 64
    assert_eq!(vdc.map_entry_address_for_test(1, 0), 64);
    // Row 0, col 31: 31
    assert_eq!(vdc.map_entry_address_for_test(0, 31), 31);
    // Row 0, col 32: 32
    assert_eq!(vdc.map_entry_address_for_test(0, 32), 32);
    // Row 31, col 0: 31*64 = 1984
    assert_eq!(vdc.map_entry_address_for_test(31, 0), 31 * 64);
    // Row 32, col 0: 32*64 = 2048
    assert_eq!(vdc.map_entry_address_for_test(32, 0), 32 * 64);
    // Row 32, col 32: 32*64+32 = 2080
    assert_eq!(vdc.map_entry_address_for_test(32, 32), 32 * 64 + 32);
}

#[test]
fn bat_always_starts_at_vram_zero() {
    let mut vdc = Vdc::new();
    for mwr in [0x0000u16, 0x0010, 0x0050, 0x1150, 0xFF50] {
        vdc.registers[0x09] = mwr;
        assert_eq!(vdc.map_base_address(), 0, "mwr={mwr:04X}");
    }
}

#[test]
fn bg_disabled_when_cr_bit7_clear() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    // Top-left BAT cell points to a visible tile.
    bus.vdc.vram[0x0000] = 0x0001;
    for row in 0..8usize {
        bus.vdc.vram[0x0010 + row] = if row == 0 { 0x0080 } else { 0x0000 };
        bus.vdc.vram[0x0018 + row] = 0x0000;
    }
    bus.vce.palette[0x001] = 0x01FF;

    // BG bit (CR bit7) is clear, while increment bits 11-12 are set.
    set_vdc_control(&mut bus, VDC_CTRL_ENABLE_SPRITES_LEGACY | (0b11 << 11));

    bus.render_frame_from_vram();

    assert_eq!(bus.framebuffer[0], bus.vce.palette_rgb(0));
    assert!(!bus.bg_opaque[0]);
}

#[test]
fn tile_entry_zero_uses_tile_zero_pattern_data() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    // BAT (row0,col1) is entry value 0 => tile 0.
    bus.vdc.vram[0x0001] = 0x0000;
    // Tile 0 row 0 plane data (overlaps BAT area by hardware design).
    bus.vdc.vram[0x0000] = 0x0080;
    bus.vdc.vram[0x0008] = 0x0000;
    bus.vce.palette[0x001] = 0x01FF;

    set_vdc_control(&mut bus, VDC_CTRL_ENABLE_BACKGROUND_LEGACY);
    bus.render_frame_from_vram();

    assert_eq!(bus.framebuffer[8], bus.vce.palette_rgb(0x001));
    assert!(bus.bg_opaque[8]);
}

#[test]
fn renderer_honours_vertical_window_overscan_rows() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    // Configure visible/overscan colours.
    bus.write(VCE_ADDRESS_ADDR, 0x00);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
    bus.write(VCE_DATA_ADDR, 0x00);
    bus.write(VCE_DATA_HIGH_ADDR, 0x00);
    bus.write(VCE_ADDRESS_ADDR, 0x00);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x01);
    bus.write(VCE_DATA_ADDR, 0x3F);
    bus.write(VCE_DATA_HIGH_ADDR, 0x00);
    bus.write(VCE_ADDRESS_ADDR, 0x11);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
    bus.write(VCE_DATA_ADDR, 0x00);
    bus.write(VCE_DATA_HIGH_ADDR, 0x3F);

    // Single opaque tile in the top-left BAT cell (palette 1, tile 1).
    // Tile pixels = colour index 1 → VCE[0x11] = 0x003F (non-black).
    write_vram_word(&mut bus, 0x0000, 0x1001);
    for row in 0..8u16 {
        // Plane 0 lo=0xFF, plane 1 hi=0x00 → index bit 0 set, bit 1 clear = index 1
        write_vram_word(&mut bus, 0x0010 + row, 0x00FF);
        write_vram_word(&mut bus, 0x0018 + row, 0x0000);
    }

    set_vdc_control(&mut bus, VDC_CTRL_DISPLAY_FULL);
    bus.write_st_port(0, 0x0C);
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x01);
    bus.write_st_port(0, 0x0D);
    bus.write_st_port(1, 0x03); // 4 active lines
    bus.write_st_port(2, 0x00);
    bus.write_st_port(0, 0x0E);
    bus.write_st_port(1, 0x02); // 5 overscan lines
    bus.write_st_port(2, 0x00);

    bus.render_frame_from_vram();
    // VDW=3 → 4 active output rows (0..3), rows 4+ are overscan → black.
    let active_pixel = bus.framebuffer[0]; // first active line
    let overscan_pixel = bus.framebuffer[6 * FRAME_WIDTH]; // post-active overscan
    assert_ne!(
        active_pixel, 0x000000,
        "active line should render tile data"
    );
    assert_eq!(overscan_pixel, 0x000000, "overscan lines should be black");
}

#[test]
fn unprogrammed_vertical_timing_uses_default_visible_height() {
    let mut bus = Bus::new();
    bus.render_frame_from_vram();

    assert_eq!(bus.display_height(), 224);
    assert_eq!(
        bus.take_frame().map(|frame| frame.len()),
        Some(bus.display_width() * 224)
    );
}

#[test]
fn vce_palette_access_during_active_display_smears_previous_pixel_colour() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);
    set_vdc_control(
        &mut bus,
        VDC_CTRL_ENABLE_BACKGROUND | VDC_CTRL_ENABLE_BACKGROUND_LEGACY,
    );

    // 7 MHz dot clock => one low-speed CPU palette access smears two pixels.
    bus.vce.write_control_low(0x01);

    const TILE_ID: usize = 0x40;
    for col in 0..32usize {
        let palette_bank = if col < 8 { 0 } else { 1 };
        bus.vdc.vram[col] = (TILE_ID as u16) | ((palette_bank as u16) << 12);
    }
    for row in 0..8usize {
        bus.vdc.vram[TILE_ID * 16 + row] = 0x00FF;
        bus.vdc.vram[TILE_ID * 16 + row + 8] = 0x0000;
    }
    bus.vce.palette[0x001] = 0x0007;
    bus.vce.palette[0x011] = 0x01C0;

    bus.vdc.in_vblank = false;
    bus.vdc.scanline = 0;
    bus.vdc.phi_scaled =
        (VDC_VBLANK_INTERVAL as u64 * 64) / (bus.vdc.display_width_for_line(0) as u64);
    bus.set_cpu_high_speed_hint(false);
    bus.vce.set_address(0x20);

    let mut baseline = bus.clone();
    baseline.render_frame_from_vram();

    bus.write_io(VCE_DATA_ADDR as usize, 0x00);
    bus.render_frame_from_vram();

    let left = bus.vce_palette_rgb(0x001);
    let right = bus.vce_palette_rgb(0x011);
    let smear_x = bus.vdc.display_start_for_line(0) + 64;
    assert_eq!(baseline.framebuffer[smear_x], right);
    assert_eq!(bus.framebuffer[smear_x], left);
    assert_eq!(bus.framebuffer[smear_x + 1], right);
}

#[test]
fn vdc_horizontal_display_width_is_latched_per_line() {
    let mut vdc = Vdc::new();
    vdc.registers[0x0B] = 0x001F;
    vdc.latch_line_state(0);
    vdc.registers[0x0B] = 0x0027;
    vdc.latch_line_state(1);

    assert_eq!(vdc.display_width_for_line(0), 256);
    assert_eq!(vdc.display_width_for_line(1), 320);
}

#[test]
fn vdc_horizontal_display_start_is_latched_per_line() {
    let mut vdc = Vdc::new();
    vdc.registers[0x0A] = 0x0002;
    vdc.latch_line_state(0);
    vdc.registers[0x0A] = 0x0202;
    vdc.latch_line_state(1);

    assert_eq!(vdc.display_start_for_line(0), 0);
    assert_eq!(vdc.display_start_for_line(1), 16);
}

#[test]
fn vdc_horizontal_display_end_margin_is_latched_per_line() {
    let mut vdc = Vdc::new();
    vdc.registers[0x0B] = 0x001F;
    vdc.latch_line_state(0);
    vdc.registers[0x0B] = 0x021F;
    vdc.latch_line_state(1);

    assert_eq!(vdc.display_end_margin_for_line(0), 0);
    assert_eq!(vdc.display_end_margin_for_line(1), 16);
}

#[test]
fn render_frame_uses_maximum_latched_horizontal_width_and_preserves_overscan() {
    let mut bus = Bus::new();
    bus.vce.palette[0x000] = 0x0000;
    bus.vce.palette[0x100] = 0x01C0;

    bus.vdc.registers[0x0B] = 0x001F;
    bus.vdc.latch_line_state(0);
    bus.vdc.registers[0x0B] = 0x0027;
    bus.vdc.latch_line_state(1);

    bus.render_frame_from_vram();

    let background = bus.vce_palette_rgb(0x000);
    let overscan = bus.vce_palette_rgb(0x100);
    assert_eq!(bus.display_width(), 320);
    assert_eq!(bus.framebuffer[300], overscan);
    assert_eq!(bus.framebuffer[FRAME_WIDTH + 300], background);
}

#[test]
fn take_frame_includes_left_border_when_latched_horizontal_start_increases() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    set_vdc_control(
        &mut bus,
        VDC_CTRL_ENABLE_BACKGROUND | VDC_CTRL_ENABLE_BACKGROUND_LEGACY,
    );
    bus.vce.palette[0x000] = 0x0000;
    bus.vce.palette[0x100] = 0x01C0;
    bus.vce.palette[0x001] = 0x0007;

    const TILE_ID: usize = 0x40;
    bus.vdc.vram[0] = TILE_ID as u16;
    for row in 0..8usize {
        bus.vdc.vram[TILE_ID * 16 + row] = 0x00FF;
        bus.vdc.vram[TILE_ID * 16 + row + 8] = 0x0000;
    }

    bus.vdc.registers[0x0A] = 0x0002;
    bus.vdc.registers[0x0B] = 0x001F;
    bus.vdc.latch_line_state(0);
    bus.vdc.registers[0x0A] = 0x0202;
    bus.vdc.latch_line_state(1);

    bus.render_frame_from_vram();
    let frame = bus.take_frame().expect("expected frame");

    let overscan = bus.vce_palette_rgb(0x100);
    let tile_colour = bus.vce_palette_rgb(0x001);
    assert_eq!(bus.display_width(), 272);
    assert_eq!(frame[0], tile_colour);
    assert_eq!(frame[bus.display_width()], overscan);
    assert_eq!(frame[bus.display_width() + 16], tile_colour);
}

#[test]
fn sprites_follow_horizontal_display_start_offset() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    bus.vce.palette[0x00] = 0x0000;
    bus.vce.palette[0x100] = 0x0000;
    bus.vce.palette[0x101] = 0x7C00;
    write_constant_sprite_tile(&mut bus, 0, 0x01);

    set_vdc_control(
        &mut bus,
        VDC_CTRL_ENABLE_SPRITES | VDC_CTRL_ENABLE_SPRITES_LEGACY,
    );

    bus.vdc.registers[0x0A] = 0x0202;
    bus.vdc.registers[0x0B] = 0x001F;
    bus.vdc.latch_line_state(0);

    bus.vdc.satb[0] = ((0 + 64) & 0x03FF) as u16;
    bus.vdc.satb[1] = ((0 + 32) & 0x03FF) as u16;
    bus.vdc.satb[2] = 0x0000;
    bus.vdc.satb[3] = 0x0000;

    bus.render_frame_from_vram();

    let sprite_colour = bus.vce.palette_rgb(0x101);
    assert_eq!(bus.framebuffer[16], sprite_colour);
    let frame = bus.take_frame().expect("expected frame");
    assert_eq!(frame[0], sprite_colour);
}

#[test]
fn take_frame_excludes_right_timing_margin_when_latched_horizontal_end_increases() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    set_vdc_control(
        &mut bus,
        VDC_CTRL_ENABLE_BACKGROUND | VDC_CTRL_ENABLE_BACKGROUND_LEGACY,
    );
    bus.vce.palette[0x000] = 0x0000;
    bus.vce.palette[0x100] = 0x01C0;
    bus.vce.palette[0x001] = 0x0007;

    const TILE_ID: usize = 0x40;
    bus.vdc.vram[0] = TILE_ID as u16;
    for row in 0..8usize {
        bus.vdc.vram[TILE_ID * 16 + row] = 0x00FF;
        bus.vdc.vram[TILE_ID * 16 + row + 8] = 0x0000;
    }

    bus.vdc.registers[0x0A] = 0x0002;
    bus.vdc.registers[0x0B] = 0x001F;
    bus.vdc.latch_line_state(0);
    bus.vdc.registers[0x0B] = 0x021F;
    bus.vdc.latch_line_state(1);

    bus.render_frame_from_vram();
    let frame = bus.take_frame().expect("expected frame");

    assert_eq!(bus.display_width(), 256);
    assert_eq!(frame.len(), 256 * bus.display_height());
}

#[test]
fn background_horizontal_zoom_scales_source() {
    let mut baseline = prepare_bus_for_zoom();
    baseline.render_frame_from_vram();
    let base0 = baseline.framebuffer[0];
    let base8 = baseline.framebuffer[8];
    let base16 = baseline.framebuffer[16];
    assert_ne!(base0, base8);
    assert_ne!(base8, base16);

    let mut zoomed = prepare_bus_for_zoom();
    zoomed.vdc.set_zoom_for_test(0x08, 0x0010);
    zoomed.render_frame_from_vram();
    let zoom0 = zoomed.framebuffer[0];
    let zoom16 = zoomed.framebuffer[16];
    let zoom32 = zoomed.framebuffer[32];
    assert_eq!(zoom0, base0);
    assert_eq!(zoom16, base8);
    assert_eq!(zoom32, base16);
}

#[test]
fn background_horizontal_zoom_shrinks_source() {
    let (baseline, zoomed) = render_zoom_pair(0x18);
    assert_eq!(zoomed[0], baseline[0]);
    assert_eq!(zoomed[16], baseline[24]);
}

#[test]
fn background_horizontal_zoom_extreme_zoom_in() {
    let (baseline, zoomed) = render_zoom_pair(0x01);
    let colour = baseline[0];
    for x in 0..16 {
        assert_eq!(zoomed[x], colour);
    }
}

#[test]
fn background_horizontal_zoom_extreme_shrink() {
    let (baseline, zoomed) = render_zoom_pair(0x1F);
    assert_eq!(zoomed[0], baseline[0]);
    assert_eq!(zoomed[8], baseline[15]);
    assert_eq!(zoomed[16], baseline[31]);
}

#[test]
fn background_priority_bit_sets_bg_priority_mask() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    set_vdc_control(&mut bus, VDC_CTRL_DISPLAY_FULL);

    const TILE_ID: usize = 0x180;
    let tile_entry = (TILE_ID as u16) & 0x07FF;
    bus.vdc.vram[0] = tile_entry;
    bus.vdc.vram[1] = tile_entry | 0x0800;

    let tile_base = (TILE_ID * 16) & 0x7FFF;
    bus.vdc.vram[(tile_base) & 0x7FFF] = 0x0080;
    for row in 1..8 {
        bus.vdc.vram[(tile_base + row) & 0x7FFF] = 0;
    }
    for row in 0..8 {
        bus.vdc.vram[(tile_base + row + 8) & 0x7FFF] = 0;
    }

    bus.vce.palette[0x01] = 0x7C00;

    bus.render_frame_from_vram();
    let colour = bus.vce.palette_rgb(0x01);
    let bg = bus.vce.palette_rgb(0x00);

    assert_eq!(bus.framebuffer[0], colour);
    assert_eq!(bus.framebuffer[8], colour);
    assert_eq!(bus.framebuffer[1], bg);
    assert!(!bus.bg_priority[0]);
    assert!(bus.bg_priority[8]);
}

#[test]
fn background_priority_overrides_sprite_pixels() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    bus.write(VCE_ADDRESS_ADDR, 0x00);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
    bus.write(VCE_DATA_ADDR, 0x00);
    bus.write(VCE_DATA_HIGH_ADDR, 0x00);

    bus.write(VCE_ADDRESS_ADDR, 0x10);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
    bus.write(VCE_DATA_ADDR, 0x3F);
    bus.write(VCE_DATA_HIGH_ADDR, 0x00);

    bus.write(VCE_ADDRESS_ADDR, 0x20);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
    bus.write(VCE_DATA_ADDR, 0x00);
    bus.write(VCE_DATA_HIGH_ADDR, 0x3F);

    bus.write_st_port(0, 0x09);
    bus.write_st_port(1, 0x10);
    bus.write_st_port(2, 0x00);

    let tile_index = 0x0100u16;
    let priority_entry = tile_index | 0x1000 | 0x0800;
    let addr_priority = bus.vdc.map_entry_address_for_test(0, 0) as u16;
    write_vram_word(&mut bus, addr_priority, priority_entry);

    let addr_plain = bus.vdc.map_entry_address_for_test(0, 1) as u16;
    write_vram_word(&mut bus, addr_plain, tile_index | 0x1000);

    let tile_addr = tile_index * 16;
    write_vram_word(&mut bus, tile_addr, 0x0080);
    for offset in 1..16 {
        write_vram_word(&mut bus, tile_addr + offset as u16, 0x0000);
    }

    set_vdc_control(&mut bus, VDC_CTRL_DISPLAY_FULL);

    bus.write_st_port(0, 0x07);
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x00);
    bus.write_st_port(0, 0x08);
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x00);

    bus.render_frame_from_vram();
    assert!(bus.bg_priority[0]);
    assert!(!bus.bg_priority[8]);
}

#[test]
fn background_vertical_zoom_scales_source() {
    let (baseline, zoomed) = render_vertical_zoom_pair(0x08);
    assert_ne!(baseline[0], baseline[8 * FRAME_WIDTH]);
    assert_ne!(baseline[8 * FRAME_WIDTH], baseline[16 * FRAME_WIDTH]);
    assert_eq!(zoomed[0], baseline[0]);
    assert_eq!(zoomed[16 * FRAME_WIDTH], baseline[8 * FRAME_WIDTH]);
    assert_eq!(zoomed[32 * FRAME_WIDTH], baseline[16 * FRAME_WIDTH]);
}

#[test]
fn background_vertical_zoom_extreme_zoom_in() {
    let (baseline, zoomed) = render_vertical_zoom_pair(0x01);
    let colour = baseline[0];
    for y in 0..16 {
        assert_eq!(zoomed[y * FRAME_WIDTH], colour);
    }
}

#[test]
fn background_vertical_zoom_extreme_shrink() {
    let (baseline, zoomed) = render_vertical_zoom_pair(0x1F);
    assert_eq!(zoomed[0], baseline[0]);
    assert_eq!(zoomed[8 * FRAME_WIDTH], baseline[15 * FRAME_WIDTH]);
    assert_eq!(zoomed[16 * FRAME_WIDTH], baseline[31 * FRAME_WIDTH]);
}

#[test]
fn timer_disable_masks_irq_line() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);
    bus.write(0x0C00, 0x01);
    bus.write(0x1402, IRQ_DISABLE_TIMER);
    bus.write(0x0C01, TIMER_CONTROL_START);

    let fired = bus.tick(1024u32 * 2, true);
    assert!(!fired);
    assert_eq!(bus.read(0x1403) & IRQ_REQUEST_TIMER, IRQ_REQUEST_TIMER);

    bus.write(0x1402, 0x00);
    assert!(bus.tick(0, true));
    bus.write(0x1403, IRQ_REQUEST_TIMER);
    assert!(!bus.tick(0, true));
}

#[test]
fn timer_uses_slow_clock_divider() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);
    bus.write(0x0C00, 0x00);
    bus.write(0x0C01, TIMER_CONTROL_START);

    let fired = bus.tick(256u32, false);
    assert!(fired);
}

#[test]
fn hardware_page_routes_vdc_registers() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    bus.write_st_port(0, 0x05); // select control register
    bus.write_st_port(1, 0x08);
    bus.write_st_port(2, 0x00);

    assert_eq!(bus.vdc_register(5), Some(0x0008));
}

#[test]
fn io_space_mirror_routes_vdc_and_vce() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);
    bus.set_mpr(1, 0xFF);

    // VCE palette write via 0x2000-mirrored address space.
    bus.write(0x2402, 0x00); // address low
    bus.write(0x2403, 0x00); // address high
    bus.write(0x2404, 0x56); // data low
    bus.write(0x2405, 0x01); // data high (only bit 0 is meaningful)
    assert_eq!(bus.vce_palette_word(0x0000), 0x0156);

    // VDC register select/data via mirrored offsets inside 0x0000-0x03FF.
    bus.write(0x2201, 0x05); // select control register (odd address mirror)
    assert_eq!(bus.st_port(0), 0x05);

    // Use a higher-offset mirror (0x2202/0x2203) to exercise the 0x100-spaced mirrors.
    bus.write(0x2202, 0xAA); // low byte (ST1 mirror)
    bus.write(0x2203, 0x00); // high byte via ST2 mirror
    assert_eq!(bus.vdc_register(5), Some(0x00AA));
}

#[test]
fn hardware_page_status_read_clears_irq() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    // Enable VBlank interrupt and raise the status flag.
    bus.write_st_port(0, 0x05);
    bus.write_st_port(1, 0x08);
    bus.write_st_port(2, 0x00);
    bus.vdc_set_status_for_test(VDC_STATUS_VBL);
    assert_ne!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);

    let status = bus.read_io(0x00);
    assert!(status & VDC_STATUS_VBL != 0);
    assert_eq!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);
    assert_eq!(bus.read_io(0x00) & VDC_STATUS_VBL, 0);
}

#[test]
fn vce_palette_write_and_read_round_trip() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    // Select palette index 0x0010.
    bus.write(VCE_ADDRESS_ADDR, 0x10);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);

    bus.write(VCE_DATA_ADDR, 0x34);
    bus.write(VCE_DATA_HIGH_ADDR, 0x01);

    assert_eq!(bus.vce_palette_word(0x0010), 0x0134);

    // Reading back should return the stored value and advance the index.
    bus.write(VCE_ADDRESS_ADDR, 0x10);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
    let lo = bus.read(VCE_DATA_ADDR);
    let hi = bus.read(VCE_DATA_HIGH_ADDR);
    assert_eq!(lo, 0x34);
    assert_eq!(hi, 0xFF);
    assert_eq!(bus.vce_palette_word(0x0011), 0);
}

#[test]
fn vce_sequential_writes_auto_increment_index() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    bus.write(VCE_ADDRESS_ADDR, 0x00);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);

    for i in 0..4u16 {
        let value = 0x0100 | i;
        bus.write(VCE_DATA_ADDR, (value & 0x00FF) as u8);
        bus.write(VCE_DATA_HIGH_ADDR, (value >> 8) as u8);
    }

    assert_eq!(bus.vce_palette_word(0), 0x0100);
    assert_eq!(bus.vce_palette_word(1), 0x0101);
    assert_eq!(bus.vce_palette_word(2), 0x0102);
    assert_eq!(bus.vce_palette_word(3), 0x0103);
}

#[test]
fn vce_write_only_and_unused_ports_read_back_as_ff() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    bus.write(0x0400, 0x07);
    bus.write(0x0401, 0xAA);
    bus.write(VCE_ADDRESS_ADDR, 0x34);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x01);

    assert_eq!(bus.read(0x0400), 0xFF);
    assert_eq!(bus.read(0x0401), 0xFF);
    assert_eq!(bus.read(VCE_ADDRESS_ADDR), 0xFF);
    assert_eq!(bus.read(VCE_ADDRESS_HIGH_ADDR), 0xFF);
    assert_eq!(bus.read(0x0406), 0xFF);
    assert_eq!(bus.read(0x0407), 0xFF);
}

#[test]
fn vce_high_data_read_sets_unused_bits_high() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    bus.write(VCE_ADDRESS_ADDR, 0x22);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
    bus.write(VCE_DATA_ADDR, 0x55);
    bus.write(VCE_DATA_HIGH_ADDR, 0x00);
    bus.write(VCE_DATA_ADDR, 0x66);
    bus.write(VCE_DATA_HIGH_ADDR, 0x01);

    bus.write(VCE_ADDRESS_ADDR, 0x22);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
    assert_eq!(bus.read(VCE_DATA_ADDR), 0x55);
    assert_eq!(bus.read(VCE_DATA_HIGH_ADDR), 0xFE);
    assert_eq!(bus.read(VCE_DATA_ADDR), 0x66);
    assert_eq!(bus.read(VCE_DATA_HIGH_ADDR), 0xFF);
}

#[test]
fn hardware_page_psg_accesses_data_ports() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);
    // Use the standard PSG address range ($0800-$0BFF) which is within
    // the real I/O decode range.  The legacy $1C60 mirror is only
    // reachable via write_io/read_io_internal (offsets $1800+ now fall
    // through to ROM in the read path).
    bus.write(0x0800, 0x02); // channel select = 2
    bus.write(0x0805, 0x7F); // channel balance = 0x7F
    bus.write(0x0800, 0x02); // re-select channel 2
    assert_eq!(bus.read(0x0805), 0x7F);
    assert_eq!(bus.psg.channels[2].balance, 0x7F);
}

#[test]
fn hardware_page_psg_direct_register_map_is_available() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    bus.write(0x0800, 0x02); // channel select
    bus.write(0x0805, 0x7F); // channel balance for selected channel
    assert_eq!(bus.psg.channels[2].balance, 0x7F);
    assert_eq!(bus.read(0x0805), 0x7F);
}

#[test]
fn vce_palette_rgb_applies_brightness_and_channels() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    // Select palette index zero.
    bus.write(VCE_ADDRESS_ADDR, 0x00);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);

    // Write palette word with G=3, R=5, B=7 (raw layout: GGGRRRBBB).
    let raw = (0x3 << 6) | (0x5 << 3) | 0x7;
    bus.write(VCE_DATA_ADDR, (raw & 0xFF) as u8);
    bus.write(VCE_DATA_HIGH_ADDR, (raw >> 8) as u8);

    let rgb = bus.vce_palette_rgb(0);
    let r = ((rgb >> 16) & 0xFF) as u8;
    let g = ((rgb >> 8) & 0xFF) as u8;
    let b = (rgb & 0xFF) as u8;

    assert_eq!(r, (0x5 * 255 / 0x07) as u8);
    assert_eq!(g, (0x3 * 255 / 0x07) as u8);
    assert_eq!(b, 255);
}

#[cfg(test)]
fn write_vram_word(bus: &mut Bus, addr: u16, value: u16) {
    bus.write_st_port(0, 0x00);
    bus.write_st_port(1, (addr & 0x00FF) as u8);
    bus.write_st_port(2, ((addr >> 8) & 0x7F) as u8);
    bus.write_st_port(0, 0x02);
    bus.write_st_port(1, (value & 0x00FF) as u8);
    bus.write_st_port(2, (value >> 8) as u8);
}

#[cfg(test)]
fn fetch_frame(bus: &mut Bus, steps: u32) -> Vec<u32> {
    for _ in 0..(steps.saturating_mul(2)) {
        bus.tick(1, true);
        if let Some(frame) = bus.take_frame() {
            return frame;
        }
    }
    panic!("expected frame output");
}

#[test]
fn render_blank_frame_uses_palette_zero() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    // Write a vivid palette entry at index 0.
    bus.write(VCE_ADDRESS_ADDR, 0x00);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
    let raw_colour = 0x01FF; // full intensity (R=7,G=7,B=7)
    bus.write(VCE_DATA_ADDR, (raw_colour & 0x00FF) as u8);
    bus.write(VCE_DATA_HIGH_ADDR, (raw_colour >> 8) as u8);

    // Enable VBlank IRQ so tick processing advances display timing.
    bus.write_st_port(0, 0x05);
    bus.write_st_port(1, 0x08);
    bus.write_st_port(2, 0x00);

    // Run long enough to hit VBlank.
    let line_cycles = (VDC_VBLANK_INTERVAL + LINES_PER_FRAME as u32 - 1) / LINES_PER_FRAME as u32;
    let steps = line_cycles * (VDC_VISIBLE_LINES as u32 + 1);
    for _ in 0..steps {
        bus.tick(1, true);
    }

    let frame = bus.take_frame().expect("expected frame after VBlank");
    assert_eq!(frame.len(), bus.display_width() * bus.display_height());
    // With both BG and SPR disabled the VDC is in burst mode — no pixel
    // data is driven.  The display is black.
    assert!(frame.iter().all(|&pixel| pixel == 0xFF000000));
}

#[test]
fn render_frame_uses_vram_palette_indices() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    // Palette index 0 -> background colour.
    bus.write(VCE_ADDRESS_ADDR, 0x00);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
    bus.write(VCE_DATA_ADDR, 0x00);
    bus.write(VCE_DATA_HIGH_ADDR, 0x00);
    // Palette index 0x10 -> black, 0x11 -> bright red.
    bus.write(VCE_ADDRESS_ADDR, 0x10);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
    bus.write(VCE_DATA_ADDR, 0x00);
    bus.write(VCE_DATA_HIGH_ADDR, 0x00);
    bus.write(VCE_ADDRESS_ADDR, 0x11);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
    bus.write(VCE_DATA_ADDR, 0x38); // red max
    bus.write(VCE_DATA_HIGH_ADDR, 0x00);

    // Write tile map entry at VRAM 0 pointing to tile index 0x0100 with palette bank 1.
    let tile_index: u16 = 0x0100;
    let map_entry = tile_index | 0x1000;
    let map_addr = bus.vdc.map_entry_address_for_test(0, 0) as u16;
    write_vram_word(&mut bus, map_addr, map_entry);

    // Write a simple tile at tile index 0x0100: first pixel uses colour 1, others 0.
    let tile_addr = tile_index * 16;
    write_vram_word(&mut bus, tile_addr, 0x0080);
    for offset in 1..16 {
        let addr = tile_addr.wrapping_add(offset as u16);
        write_vram_word(&mut bus, addr, 0x0000);
    }

    // Enable background and configure scroll.
    bus.write_st_port(0, 0x09);
    bus.write_st_port(1, 0x10);
    bus.write_st_port(2, 0x00);
    bus.write_st_port(0, 0x07); // X scroll
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x00);
    bus.write_st_port(0, 0x08); // Y scroll
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x00);
    bus.write_st_port(0, 0x05);
    bus.write_st_port(1, 0x88);
    bus.write_st_port(2, 0x80);
    let line_cycles = (VDC_VBLANK_INTERVAL + LINES_PER_FRAME as u32 - 1) / LINES_PER_FRAME as u32;
    let steps = line_cycles * (VDC_VISIBLE_LINES as u32 + 1);
    for _ in 0..steps {
        bus.tick(1, true);
    }

    let frame = bus.take_frame().expect("expected frame");
    assert_eq!(frame.len(), bus.display_width() * bus.display_height());
    let colour1 = bus.vce_palette_rgb(0x11);
    let colour0 = bus.vce_palette_rgb(0x00);
    assert_eq!(frame[0], colour1);
    assert_eq!(frame[1], colour0);
}

#[test]
fn render_frame_respects_map_size_and_scroll() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    // Configure palette entries.
    bus.write(VCE_ADDRESS_ADDR, 0x00);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
    bus.write(VCE_DATA_ADDR, 0x00);
    bus.write(VCE_DATA_HIGH_ADDR, 0x00);
    bus.write(VCE_ADDRESS_ADDR, 0x10);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
    bus.write(VCE_DATA_ADDR, 0x00);
    bus.write(VCE_DATA_HIGH_ADDR, 0x00);
    bus.write(VCE_ADDRESS_ADDR, 0x11);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
    bus.write(VCE_DATA_ADDR, 0x38);
    bus.write(VCE_DATA_HIGH_ADDR, 0x00);

    // Configure virtual map to 64x32 and scroll so tile column 40 appears at x=0.
    bus.write_st_port(0, 0x09);
    bus.write_st_port(1, 0x10);
    bus.write_st_port(2, 0x00);
    let scroll_x = 40 * TILE_WIDTH as u16;
    bus.write_st_port(0, 0x07);
    bus.write_st_port(1, (scroll_x & 0xFF) as u8);
    bus.write_st_port(2, ((scroll_x >> 8) & 0x03) as u8);
    bus.write_st_port(0, 0x08);
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x00);

    // Write map entry for column 40 with palette bank 1.
    let tile_index: u16 = 0x0100;
    let map_entry = tile_index | 0x1000;
    let map_addr = bus.vdc.map_entry_address_for_test(0, 40) as u16;
    write_vram_word(&mut bus, map_addr, map_entry);

    // Tile pattern data for tile 0x0100.
    let tile_addr = tile_index * 16;
    write_vram_word(&mut bus, tile_addr, 0x0080);
    for offset in 1..16 {
        let addr = tile_addr.wrapping_add(offset as u16);
        write_vram_word(&mut bus, addr, 0x0000);
    }

    bus.write_st_port(0, 0x05);
    bus.write_st_port(1, 0x88);
    bus.write_st_port(2, 0x80);

    let line_cycles = (VDC_VBLANK_INTERVAL + LINES_PER_FRAME as u32 - 1) / LINES_PER_FRAME as u32;
    let steps = line_cycles * (VDC_VISIBLE_LINES as u32 + 1);
    for _ in 0..steps {
        bus.tick(1, true);
    }

    let frame = bus.take_frame().expect("expected frame");
    assert_eq!(frame.len(), bus.display_width() * bus.display_height());
    let colour1 = bus.vce_palette_rgb(0x11);
    let colour0 = bus.vce_palette_rgb(0x00);
    assert_eq!(frame[0], colour1);
    assert_eq!(frame[1], colour0);
}

#[test]
fn render_frame_honours_map_base_offset() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    bus.write(VCE_ADDRESS_ADDR, 0x00);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
    bus.write(VCE_DATA_ADDR, 0x00);
    bus.write(VCE_DATA_HIGH_ADDR, 0x00);
    bus.write(VCE_ADDRESS_ADDR, 0x11);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
    bus.write(VCE_DATA_ADDR, 0x38);
    bus.write(VCE_DATA_HIGH_ADDR, 0x00);

    bus.write_st_port(0, 0x09);
    bus.write_st_port(1, 0x50);
    bus.write_st_port(2, 0x0A);

    bus.write_st_port(0, 0x07);
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x00);
    bus.write_st_port(0, 0x08);
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x00);

    let tile_index: u16 = 0x0100;
    let map_entry = tile_index | 0x1000;
    let map_addr = bus.vdc.map_entry_address_for_test(0, 0) as u16;
    write_vram_word(&mut bus, map_addr, map_entry);

    let tile_addr = tile_index * 16;
    write_vram_word(&mut bus, tile_addr, 0x0080);
    for offset in 1..16 {
        let addr = tile_addr.wrapping_add(offset as u16);
        write_vram_word(&mut bus, addr, 0x0000);
    }

    set_vdc_control(&mut bus, VDC_CTRL_DISPLAY_FULL);

    let line_cycles = (VDC_VBLANK_INTERVAL + LINES_PER_FRAME as u32 - 1) / LINES_PER_FRAME as u32;
    let steps = line_cycles * (VDC_VISIBLE_LINES as u32 + 1);
    let frame = fetch_frame(&mut bus, steps);
    let colour = bus.vce_palette_rgb(0x11);
    assert_eq!(frame[0], colour);
}

#[test]
fn render_frame_respects_cg_mode_restricted_planes() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    // Configure palettes: index 0 = background, 0x14 = visible colour.
    bus.write(VCE_ADDRESS_ADDR, 0x00);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
    bus.write(VCE_DATA_ADDR, 0x00);
    bus.write(VCE_DATA_HIGH_ADDR, 0x00);
    bus.write(VCE_ADDRESS_ADDR, 0x14);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
    bus.write(VCE_DATA_ADDR, 0x38); // bright red
    bus.write(VCE_DATA_HIGH_ADDR, 0x00);

    // Map tile 0x0100 at origin using palette bank 1.
    let tile_index: u16 = 0x0100;
    let map_entry = tile_index | 0x1000;
    let map_addr = bus.vdc.map_entry_address_for_test(0, 0) as u16;
    write_vram_word(&mut bus, map_addr, map_entry);

    // Tile data: only plane2 bit set so colour index = 4.
    let tile_addr = tile_index * 16;
    write_vram_word(&mut bus, tile_addr, 0x0000);
    write_vram_word(&mut bus, tile_addr + 8, 0x0080);
    for offset in 1..16 {
        if offset == 8 {
            continue;
        }
        let addr = tile_addr.wrapping_add(offset as u16);
        write_vram_word(&mut bus, addr, 0x0000);
    }

    // Scroll to origin and enable background.
    bus.write_st_port(0, 0x07);
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x00);
    bus.write_st_port(0, 0x08);
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x00);

    // Use restricted CG mode with CM=0 (only CG0 valid).
    bus.write_st_port(0, 0x09);
    bus.write_st_port(1, 0x03);
    bus.write_st_port(2, 0x00);

    bus.write_st_port(0, 0x05);
    bus.write_st_port(1, 0x88);
    bus.write_st_port(2, 0x80);

    let line_cycles = (VDC_VBLANK_INTERVAL + LINES_PER_FRAME as u32 - 1) / LINES_PER_FRAME as u32;
    let steps = line_cycles * (VDC_VISIBLE_LINES as u32 + 1);
    let frame = fetch_frame(&mut bus, steps);
    let bg_colour = bus.vce_palette_rgb(0x00);
    assert_eq!(
        frame[0], bg_colour,
        "plane2 data should be ignored when CM=0"
    );

    // Switch to CM=1 and rerun a frame; plane2 data should now be visible.
    bus.write_st_port(0, 0x09);
    bus.write_st_port(1, 0x83);
    bus.write_st_port(2, 0x00);

    let frame_cm1 = fetch_frame(&mut bus, steps);
    let colour_plane2 = bus.vce_palette_rgb(0x14);
    assert_eq!(
        frame_cm1[0], colour_plane2,
        "plane2 data should produce colour when CM=1"
    );
}

#[test]
fn render_frame_wraps_horizontally_on_64x64_map() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    bus.write(VCE_ADDRESS_ADDR, 0x00);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
    bus.write(VCE_DATA_ADDR, 0x00);
    bus.write(VCE_DATA_HIGH_ADDR, 0x00);
    bus.write(VCE_ADDRESS_ADDR, 0x11);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
    bus.write(VCE_DATA_ADDR, 0x38);
    bus.write(VCE_DATA_HIGH_ADDR, 0x00);

    bus.write_st_port(0, 0x09);
    bus.write_st_port(1, 0x50);
    bus.write_st_port(2, 0x00);

    let tile_index: u16 = 0x0100;
    let map_entry = tile_index | 0x1000;
    let map_addr = bus.vdc.map_entry_address_for_test(0, 63) as u16;
    write_vram_word(&mut bus, map_addr, map_entry);

    let tile_addr = tile_index * 16;
    write_vram_word(&mut bus, tile_addr, 0x0080);
    for offset in 1..16 {
        let addr = tile_addr.wrapping_add(offset as u16);
        write_vram_word(&mut bus, addr, 0x0000);
    }

    let scroll_x = 63 * TILE_WIDTH as u16;
    bus.write_st_port(0, 0x07);
    bus.write_st_port(1, (scroll_x & 0xFF) as u8);
    bus.write_st_port(2, ((scroll_x >> 8) & 0x03) as u8);
    bus.write_st_port(0, 0x08);
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x00);

    bus.write_st_port(0, 0x05);
    bus.write_st_port(1, 0x88);
    bus.write_st_port(2, 0x80);

    let line_cycles = (VDC_VBLANK_INTERVAL + LINES_PER_FRAME as u32 - 1) / LINES_PER_FRAME as u32;
    let steps = line_cycles * (VDC_VISIBLE_LINES as u32 + 1);
    let frame = fetch_frame(&mut bus, steps);
    let expected = bus.vce_palette_rgb(0x11);
    assert_eq!(
        frame[0], expected,
        "scrolled column 63 should appear at x=0"
    );
    assert_eq!(
        frame[TILE_WIDTH],
        bus.vce_palette_rgb(0x00),
        "next column should wrap to column 0 background"
    );
}

#[test]
fn render_frame_wraps_vertically_on_64x64_map() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);

    bus.write(VCE_ADDRESS_ADDR, 0x00);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
    bus.write(VCE_DATA_ADDR, 0x00);
    bus.write(VCE_DATA_HIGH_ADDR, 0x00);
    bus.write(VCE_ADDRESS_ADDR, 0x11);
    bus.write(VCE_ADDRESS_HIGH_ADDR, 0x00);
    bus.write(VCE_DATA_ADDR, 0x38);
    bus.write(VCE_DATA_HIGH_ADDR, 0x00);

    bus.write_st_port(0, 0x09);
    bus.write_st_port(1, 0x50);
    bus.write_st_port(2, 0x00);

    let tile_index: u16 = 0x0100;
    let map_entry = tile_index | 0x1000;
    let map_addr = bus.vdc.map_entry_address_for_test(63, 0) as u16;
    write_vram_word(&mut bus, map_addr, map_entry);

    let tile_addr = tile_index * 16;
    write_vram_word(&mut bus, tile_addr, 0x0080);
    for offset in 1..16 {
        let addr = tile_addr.wrapping_add(offset as u16);
        write_vram_word(&mut bus, addr, 0x0000);
    }

    bus.write_st_port(0, 0x07);
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x00);
    let scroll_y = 63 * TILE_HEIGHT as u16;
    bus.write_st_port(0, 0x08);
    bus.write_st_port(1, (scroll_y & 0xFF) as u8);
    bus.write_st_port(2, ((scroll_y >> 8) & 0x01) as u8);

    bus.write_st_port(0, 0x05);
    bus.write_st_port(1, 0x88);
    bus.write_st_port(2, 0x80);

    let line_cycles = (VDC_VBLANK_INTERVAL + LINES_PER_FRAME as u32 - 1) / LINES_PER_FRAME as u32;
    let steps = line_cycles * (VDC_VISIBLE_LINES as u32 + 1);
    let frame = fetch_frame(&mut bus, steps);
    assert_eq!(
        frame[0],
        bus.vce_palette_rgb(0x11),
        "scrolled row 63 should appear at y=0"
    );
    assert_eq!(
        frame[bus.display_width() * TILE_HEIGHT],
        bus.vce_palette_rgb(0x00),
        "next row should wrap to row 0 background"
    );
}

#[test]
fn vdc_vblank_flag_clears_during_display() {
    let mut bus = Bus::new();
    bus.read_io(0x00); // clear initial flags.

    let mut seen_high = false;
    let mut saw_cleared_after = false;
    for _ in 0..(LINES_PER_FRAME as usize * 4) {
        bus.tick(500, true);
        let status = bus.read_io(0x00);
        if status & VDC_STATUS_VBL != 0 {
            seen_high = true;
        } else if seen_high {
            saw_cleared_after = true;
            break;
        }
    }
    assert!(seen_high, "VBlank status bit never asserted");
    assert!(
        saw_cleared_after,
        "VBlank status bit never cleared after asserting"
    );
}

#[test]
fn vdc_vblank_flag_returns_after_display() {
    let mut bus = Bus::new();
    bus.read_io(0x00); // clear initial flags.

    let mut phase = 0;
    let mut seen_second_high = false;
    for _ in 0..(LINES_PER_FRAME as usize * 4) {
        bus.tick(500, true);
        let status = bus.read_io(0x00);
        match phase {
            0 => {
                if status & VDC_STATUS_VBL != 0 {
                    phase = 1;
                }
            }
            1 => {
                if status & VDC_STATUS_VBL == 0 {
                    phase = 2;
                }
            }
            _ => {
                if status & VDC_STATUS_VBL != 0 {
                    seen_second_high = true;
                    break;
                }
            }
        }
    }
    assert!(
        seen_second_high,
        "VBlank status bit never asserted again after clearing"
    );
}

#[test]
fn vdc_tick_holds_on_first_frame_trigger_for_large_cycle_chunk() {
    let mut vdc = Vdc::new();
    vdc.scanline = 0;
    vdc.in_vblank = false;
    vdc.frame_trigger = false;
    vdc.scroll_line_valid.fill(false);
    let frame_cycles = VDC_VBLANK_INTERVAL;

    // One large chunk can cover more than a full frame worth of scanline steps.
    // We should stop at the first VBlank/frame trigger and preserve latched line state.
    let _ = vdc.tick(frame_cycles);
    assert!(vdc.frame_ready(), "expected frame trigger after large tick");
    // Frame trigger now fires at the last active scanline (one before VBlank),
    // so that mid-frame VRAM writes are captured by the batch renderer.
    assert_eq!(
        vdc.scanline,
        VDC_VISIBLE_LINES - 1,
        "scanline should stop one line before VBlank"
    );
    assert!(vdc.scroll_line_valid[1], "line 1 should be latched");
    assert!(
        vdc.scroll_line_valid[(VDC_VISIBLE_LINES - 1) as usize],
        "last active line should be latched"
    );
    assert!(
        !vdc.scroll_line_valid[VDC_VISIBLE_LINES as usize],
        "VBlank line should remain unlatched until frame is consumed"
    );
}

#[test]
fn vdc_register_write_sequence() {
    let mut bus = Bus::new();
    bus.write_st_port(0, 0x00); // MAWR
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x04);
    assert_eq!(bus.vdc_register(0), Some(0x0400));

    bus.write_st_port(0, 0x02); // VRAM data
    bus.write_st_port(0, 0x02); // select VRAM data port
    bus.write_st_port(1, 0x34);
    bus.write_st_port(2, 0x12);
    assert_eq!(bus.vdc_vram_word(0x0400), 0x1234);
    assert_eq!(bus.vdc_register(0), Some(0x0401));

    // Subsequent data write should auto-increment MAWR
    bus.write_st_port(1, 0x78);
    bus.write_st_port(2, 0x56);
    assert_eq!(bus.vdc_vram_word(0x0401), 0x5678);
    assert_eq!(bus.vdc_register(0), Some(0x0402));
}

#[test]
fn vdc_status_initial_vblank_and_clear() {
    let mut bus = Bus::new();
    let status = bus.read_io(0x00);
    assert!(status & VDC_STATUS_VBL != 0);
    let status_after = bus.read_io(0x00);
    assert_eq!(status_after & VDC_STATUS_VBL, 0);
}

#[test]
fn vdc_vblank_irq_raises_when_enabled() {
    let mut bus = Bus::new();
    bus.set_mpr(1, 0xFF);
    // Clear the initial VBlank state.
    bus.read_io(0x00);
    assert_eq!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);

    // Enable VBlank IRQ (bit 3 of control register).
    bus.write_st_port(0, 0x05);
    bus.write_st_port(1, 0x08);
    bus.write_st_port(2, 0x00);

    for _ in 0..400 {
        bus.tick(200, false);
    }

    assert!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1 != 0);
    let status = bus.read_io(0x00);
    assert!(status & VDC_STATUS_VBL != 0);
    bus.acknowledge_irq(IRQ_REQUEST_IRQ1);
    assert_eq!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);
}

#[test]
fn vdc_status_interrupt_respects_control() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);
    bus.set_mpr(1, 0xFF);

    // Enable VBlank IRQ (bit 3 of control register).
    bus.write_st_port(0, 0x05);
    bus.write_st_port(1, 0x08);
    bus.write_st_port(2, 0x00);

    bus.vdc_set_status_for_test(VDC_STATUS_VBL);
    assert_eq!(bus.read(0x1403) & IRQ_REQUEST_IRQ1, IRQ_REQUEST_IRQ1);

    let status = bus.read(0x2000);
    assert_eq!(status & VDC_STATUS_VBL, VDC_STATUS_VBL);
    bus.write(0x1403, IRQ_REQUEST_IRQ1);
    assert_eq!(bus.read(0x1403) & IRQ_REQUEST_IRQ1, 0);

    // Disable VBlank interrupt and ensure no IRQ is raised.
    bus.write_st_port(0, 0x05);
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x00);

    bus.vdc_set_status_for_test(VDC_STATUS_VBL);
    bus.write(0x1403, IRQ_REQUEST_IRQ1);
    bus.vdc_set_status_for_test(VDC_STATUS_VBL);
    assert_eq!(bus.read(0x1403) & IRQ_REQUEST_IRQ1, 0);
}

#[test]
fn vdc_vram_increment_uses_control_bits() {
    let mut bus = Bus::new();

    bus.write_st_port(0, 0x00); // MAWR = 0
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x00);

    // Set increment mode to 32 (INC field = 01b at bits 12..11).
    bus.write_st_port(0, 0x05);
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x08);

    bus.write_st_port(0, 0x02); // VRAM data write
    bus.write_st_port(1, 0xAA);
    bus.write_st_port(2, 0x55);
    assert_eq!(bus.vdc_vram_word(0x0000), 0x55AA);
    assert_eq!(bus.vdc_register(0), Some(0x0020));

    bus.write_st_port(1, 0xBB);
    bus.write_st_port(2, 0x66);
    assert_eq!(bus.vdc_vram_word(0x0020), 0x66BB);
    assert_eq!(bus.vdc_register(0), Some(0x0040));
}

#[test]
fn vdc_vram_reads_prefetch_and_increment() {
    let mut bus = Bus::new();
    bus.set_mpr(1, 0xFF);

    // Populate VRAM with two words.
    bus.write_st_port(0, 0x00); // MAWR = 0
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x00);
    bus.write_st_port(0, 0x02);
    bus.write_st_port(1, 0x34);
    bus.write_st_port(2, 0x12);
    bus.write_st_port(1, 0x78);
    bus.write_st_port(2, 0x56);

    assert_eq!(bus.vdc_vram_word(0x0000), 0x1234);
    assert_eq!(bus.vdc_vram_word(0x0001), 0x5678);
    assert_eq!(bus.vdc_register(0), Some(0x0002));

    // Point VRR to zero.
    bus.write_st_port(0, 0x01); // MARR
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x00);
    bus.write_st_port(0, 0x02); // select VRAM data register for reads

    let lo = bus.read(0x2002);
    assert_eq!(lo, 0x34);
    assert_eq!(bus.vdc_register(1), Some(0x0000));

    let hi = bus.read(0x2003);
    assert_eq!(hi, 0x12);
    assert_eq!(bus.vdc_register(1), Some(0x0001));

    let next_lo = bus.read(0x2002);
    assert_eq!(next_lo, 0x78);
    let next_hi = bus.read(0x2003);
    assert_eq!(next_hi, 0x56);
    assert_eq!(bus.vdc_register(1), Some(0x0002));
}

#[test]
fn vdc_data_low_port_always_returns_low_byte() {
    let mut bus = Bus::new();
    bus.set_mpr(1, 0xFF);

    write_vram_word(&mut bus, 0x0000, 0x1234);
    write_vram_word(&mut bus, 0x0001, 0x5678);

    bus.write_st_port(0, 0x01); // MARR
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x00);
    bus.write_st_port(0, 0x02); // data register

    let lo1 = bus.read(0x2002);
    let lo2 = bus.read(0x2002);
    assert_eq!(lo1, 0x34);
    assert_eq!(lo2, 0x34);
    assert_eq!(bus.vdc_register(1), Some(0x0000));

    let hi = bus.read(0x2003);
    assert_eq!(hi, 0x12);
    assert_eq!(bus.vdc_register(1), Some(0x0001));
}

#[test]
fn vdc_data_port_reads_selected_register_for_non_vram_index() {
    let mut bus = Bus::new();
    bus.set_mpr(1, 0xFF);

    // Write control register (R05) through normal data ports.
    bus.write_st_port(0, 0x05);
    bus.write_st_port(1, 0x5A);
    bus.write_st_port(2, 0x08);

    // Read back from data ports and ensure we get R05 contents, not VRAM.
    bus.write_st_port(0, 0x05);
    let lo = bus.read(0x2002);
    let hi = bus.read(0x2003);
    assert_eq!(lo, 0x5A);
    assert_eq!(hi, 0x08);
    assert_eq!(bus.vdc_register(0x05), Some(0x085A));

    // MARR should remain untouched by non-VRAM register reads.
    assert_eq!(bus.vdc_register(0x01), Some(0x0000));
}

#[test]
fn vdc_satb_dma_copies_sprite_table_and_sets_interrupt() {
    let mut bus = Bus::new();
    // Clear initial VBlank flag.
    bus.read_io(0x00);

    // Seed VRAM at $0200 with sprite attribute data.
    bus.write_st_port(0, 0x00); // MAWR
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x02);
    bus.write_st_port(0, 0x02); // VRAM data write
    for &word in &[0x1234u16, 0x5678, 0x9ABC, 0xDEF0] {
        bus.write_st_port(1, (word & 0x00FF) as u8);
        bus.write_st_port(2, (word >> 8) as u8);
    }

    // Enable SATB DMA IRQ and schedule a transfer from $0200.
    bus.write_st_port(0, 0x0F);
    bus.write_st_port(1, DMA_CTRL_IRQ_SATB as u8);
    bus.write_st_port(2, 0x00);
    bus.write_st_port(0, 0x13);
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x02);

    // Run enough cycles to hit the next VBlank and service the DMA.
    for _ in 0..4 {
        bus.tick(200_000, true);
    }

    assert_eq!(bus.vdc_satb_word(0), 0x1234);
    assert_eq!(bus.vdc_satb_word(1), 0x5678);
    assert_eq!(bus.vdc_satb_word(2), 0x9ABC);
    assert_eq!(bus.vdc_satb_word(3), 0xDEF0);

    assert_eq!(
        bus.pending_interrupts() & IRQ_REQUEST_IRQ1,
        IRQ_REQUEST_IRQ1
    );
    let status = bus.read_io(0x00);
    assert!(status & VDC_STATUS_DS != 0);
    bus.acknowledge_irq(IRQ_REQUEST_IRQ1);
    assert_eq!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);
}

#[test]
fn vdc_satb_dma_sets_ds_when_source_written() {
    let mut bus = Bus::new();
    bus.read_io(0x00); // clear initial DS/VBlank bits

    const SATB_SOURCE: u16 = 0x0200;
    let sample = [0xAAAAu16, 0xBBBB, 0xCCCC, 0xDDDD];

    // Populate VRAM at $0200 with sample sprite attributes.
    bus.write_st_port(0, 0x00); // MAWR
    bus.write_st_port(1, (SATB_SOURCE & 0x00FF) as u8);
    bus.write_st_port(2, (SATB_SOURCE >> 8) as u8);
    bus.write_st_port(0, 0x02); // VRAM data write
    for &word in &sample {
        bus.write_st_port(1, (word & 0x00FF) as u8);
        bus.write_st_port(2, (word >> 8) as u8);
    }

    // Writing the SATB source schedules a DMA for the next VBlank.
    bus.write_st_port(0, 0x13);
    bus.write_st_port(1, (SATB_SOURCE & 0x00FF) as u8);
    bus.write_st_port(2, (SATB_SOURCE >> 8) as u8);

    // DMA is deferred — SATB should NOT be updated yet.
    assert_eq!(
        bus.vdc_satb_word(0),
        0,
        "SATB should not update before VBlank"
    );

    // Advance past VBlank so the deferred DMA executes.
    for _ in 0..4 {
        bus.tick(200_000, true);
    }

    for (idx, &expected) in sample.iter().enumerate() {
        assert_eq!(
            bus.vdc_satb_word(idx),
            expected,
            "SATB entry {idx} did not match VRAM word"
        );
    }
    assert_ne!(bus.vdc_status_bits() & VDC_STATUS_DS, 0);
}

#[test]
fn vdc_cram_dma_transfers_palette_from_vram() {
    let mut bus = Bus::new();
    bus.read_io(0x00); // clear initial status bits

    const VRAM_BASE: u16 = 0x0500;
    let words = [0x0011u16, 0x2233, 0x4455, 0x6677];

    // Seed VRAM at $0500 with palette words.
    bus.write_st_port(0, 0x00); // MAWR
    bus.write_st_port(1, (VRAM_BASE & 0x00FF) as u8);
    bus.write_st_port(2, (VRAM_BASE >> 8) as u8);
    bus.write_st_port(0, 0x02);
    for &word in &words {
        bus.write_st_port(1, (word & 0x00FF) as u8);
        bus.write_st_port(2, (word >> 8) as u8);
    }

    // Point the VRAM read address at the same base for CRAM DMA.
    bus.write_st_port(0, 0x01); // MARR
    bus.write_st_port(1, (VRAM_BASE & 0x00FF) as u8);
    bus.write_st_port(2, (VRAM_BASE >> 8) as u8);

    // Request four words for the upcoming CRAM DMA.
    bus.vdc.registers[0x12] = 0x0004;
    // Schedule CRAM DMA directly (not a standard HuC6270 feature —
    // our emulator provides this as an internal utility).
    bus.vdc.schedule_cram_dma();

    // Tick through VBlank so the pending CRAM DMA executes.
    for _ in 0..4 {
        bus.tick(200_000, true);
    }

    for (idx, &expected) in words.iter().enumerate() {
        assert_eq!(
            bus.vce_palette_word(idx),
            expected,
            "palette entry {idx} did not match VRAM word"
        );
    }
    assert_eq!(bus.vdc_register(0x00), Some(VRAM_BASE + words.len() as u16));
    assert_eq!(bus.read_io(VCE_ADDRESS_ADDR as usize), 0xFF);
    assert_ne!(bus.vdc_status_bits() & VDC_STATUS_DV, 0);
}

#[test]
fn vdc_vram_dma_copies_words_and_raises_status() {
    let mut bus = Bus::new();
    bus.read_io(0x00); // clear initial VBlank

    const SOURCE: u16 = 0x0200;
    let words = [0x0AA0u16, 0x0BB1, 0x0CC2];
    for (index, &word) in words.iter().enumerate() {
        bus.vdc.vram[(SOURCE as usize + index) & 0x7FFF] = word;
    }

    // Configure VRAM DMA: enable IRQ, set source/destination, and trigger by writing LENR MSB.
    bus.write_st_port(0, 0x0F);
    bus.write_st_port(1, DMA_CTRL_IRQ_VRAM as u8);
    bus.write_st_port(2, 0x00);

    bus.write_st_port(0, 0x10);
    bus.write_st_port(1, (SOURCE & 0x00FF) as u8);
    bus.write_st_port(2, (SOURCE >> 8) as u8);

    bus.write_st_port(0, 0x11);
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x05);

    bus.write_st_port(0, 0x12);
    bus.write_st_port(1, (words.len() as u16 - 1) as u8);
    bus.write_st_port(2, 0x00);

    assert_eq!(bus.vdc_vram_word(0x0500), 0x0AA0);
    assert_eq!(bus.vdc_vram_word(0x0501), 0x0BB1);
    assert_eq!(bus.vdc_vram_word(0x0502), 0x0CC2);
    assert_eq!(
        bus.vdc_register(0x10),
        Some(SOURCE.wrapping_add(words.len() as u16))
    );
    assert_eq!(bus.vdc_register(0x11), Some(0x0503));
    assert_eq!(bus.vdc_register(0x12), Some(0xFFFF));

    assert_eq!(
        bus.pending_interrupts() & IRQ_REQUEST_IRQ1,
        IRQ_REQUEST_IRQ1
    );
    let status = bus.read_io(0x00);
    assert!(status & VDC_STATUS_DV != 0);
    bus.acknowledge_irq(IRQ_REQUEST_IRQ1);
    assert_eq!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);
}

#[test]
fn vdc_dma_status_clears_after_status_read() {
    let mut bus = Bus::new();
    bus.read_io(0x00); // clear initial VBlank

    // Configure VRAM DMA with IRQ enabled and execute a single-word copy.
    const SOURCE: u16 = 0x0100;
    bus.vdc.vram[SOURCE as usize & 0x7FFF] = 0xDEAD;

    bus.write_st_port(0, 0x0F);
    bus.write_st_port(1, DMA_CTRL_IRQ_VRAM as u8);
    bus.write_st_port(2, 0x00);
    bus.write_st_port(0, 0x10);
    bus.write_st_port(1, (SOURCE & 0x00FF) as u8);
    bus.write_st_port(2, (SOURCE >> 8) as u8);
    bus.write_st_port(0, 0x11);
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x02);
    bus.write_st_port(0, 0x12);
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x00);

    assert_eq!(
        bus.pending_interrupts() & IRQ_REQUEST_IRQ1,
        IRQ_REQUEST_IRQ1
    );
    let status = bus.read_io(0x00);
    assert!(status & VDC_STATUS_DV != 0);
    assert_eq!(bus.read_io(0x00) & VDC_STATUS_DV, 0);
}

#[test]
fn vdc_dma_status_clears_on_status_read() {
    let mut bus = Bus::new();
    bus.read_io(0x00);

    const SOURCE: u16 = 0x0400;
    bus.vdc.vram[SOURCE as usize & 0x7FFF] = 0x1234;

    // Trigger VRAM DMA.
    bus.write_st_port(0, 0x0F);
    bus.write_st_port(1, DMA_CTRL_IRQ_VRAM as u8);
    bus.write_st_port(2, 0x00);
    bus.write_st_port(0, 0x10);
    bus.write_st_port(1, (SOURCE & 0x00FF) as u8);
    bus.write_st_port(2, (SOURCE >> 8) as u8);
    bus.write_st_port(0, 0x11);
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x03);
    bus.write_st_port(0, 0x12);
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x00);

    assert_eq!(
        bus.pending_interrupts() & IRQ_REQUEST_IRQ1,
        IRQ_REQUEST_IRQ1
    );

    // Per MAME: writing DCR does NOT clear status flags.
    // DV survives the DCR write.
    bus.write_st_port(0, 0x0F);
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x00);
    assert_ne!(
        bus.read_io(0x00) & VDC_STATUS_DV,
        0,
        "DV should survive DCR write"
    );

    // Reading status clears DV and drops the IRQ.
    assert_eq!(
        bus.read_io(0x00) & VDC_STATUS_DV,
        0,
        "DV cleared after status read"
    );
    assert_eq!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);
}

#[test]
fn vdc_satb_auto_transfer_stops_when_disabled() {
    let mut bus = Bus::new();
    bus.read_io(0x00);

    // Seed VRAM at $0300 with initial sprite words.
    bus.write_st_port(0, 0x00); // MAWR
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x03);
    bus.write_st_port(0, 0x02);
    let first_words = [0xAAAAu16, 0xBBBB];
    for &word in &first_words {
        bus.write_st_port(1, (word & 0x00FF) as u8);
        bus.write_st_port(2, (word >> 8) as u8);
    }

    // Enable SATB DMA with auto-transfer and IRQs.
    bus.write_st_port(0, 0x0F);
    bus.write_st_port(1, (DMA_CTRL_IRQ_SATB | DMA_CTRL_SATB_AUTO) as u8);
    bus.write_st_port(2, 0x00);

    // Point SATB DMA at $0300.
    bus.write_st_port(0, 0x13);
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x03);

    // Run until VBlank triggers the auto SATB DMA.
    for _ in 0..4 {
        bus.tick(200_000, true);
    }

    assert_eq!(bus.vdc_satb_word(0), 0xAAAA);
    assert_eq!(bus.vdc_satb_word(1), 0xBBBB);

    // Acknowledge the interrupt by reading status (per real HW).
    bus.read_io(0x00);

    // Change VRAM words to a new pattern.
    bus.write_st_port(0, 0x00);
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x03);
    bus.write_st_port(0, 0x02);
    let second_words = [0xCCCCu16, 0xDDDD];
    for &word in &second_words {
        bus.write_st_port(1, (word & 0x00FF) as u8);
        bus.write_st_port(2, (word >> 8) as u8);
    }

    // Disable auto-transfer.
    bus.write_st_port(0, 0x0F);
    bus.write_st_port(1, DMA_CTRL_IRQ_SATB as u8);
    bus.write_st_port(2, 0x00);

    // Acknowledge any pending DS from the first DMA.
    bus.read_io(0x00);

    // Next frame should not pull new SATB data.
    for _ in 0..4 {
        bus.tick(200_000, true);
    }

    assert_eq!(bus.vdc_satb_word(0), 0xAAAA);
    assert_eq!(bus.vdc_satb_word(1), 0xBBBB);
    assert_eq!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);
}

#[test]
fn vdc_satb_auto_transfer_repeats_when_enabled() {
    let mut bus = Bus::new();
    bus.read_io(0x00);

    // Seed VRAM at $0300 with an initial pattern.
    bus.write_st_port(0, 0x00); // MAWR
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x03);
    bus.write_st_port(0, 0x02);
    let initial_words = [0x1111u16, 0x2222];
    for &word in &initial_words {
        bus.write_st_port(1, (word & 0x00FF) as u8);
        bus.write_st_port(2, (word >> 8) as u8);
    }

    // Enable SATB auto-transfer with IRQs.
    bus.write_st_port(0, 0x0F);
    bus.write_st_port(1, (DMA_CTRL_IRQ_SATB | DMA_CTRL_SATB_AUTO) as u8);
    bus.write_st_port(2, 0x00);

    // Point SATB DMA at $0300. DMA is deferred to next VBlank.
    bus.write_st_port(0, 0x13);
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x03);

    // Advance past VBlank so the deferred DMA executes.
    for _ in 0..4 {
        bus.tick(200_000, true);
    }

    assert_eq!(bus.vdc_satb_word(0), 0x1111);
    assert_eq!(bus.vdc_satb_word(1), 0x2222);
    assert_ne!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);

    // Acknowledge the initial interrupt and clear DS.
    bus.acknowledge_irq(IRQ_REQUEST_IRQ1);
    bus.read_io(0x00);

    // Overwrite VRAM with a new pattern; auto-transfer should pick it up on next VBlank.
    bus.write_st_port(0, 0x00);
    bus.write_st_port(1, 0x00);
    bus.write_st_port(2, 0x03);
    bus.write_st_port(0, 0x02);
    let updated_words = [0x3333u16, 0x4444];
    for &word in &updated_words {
        bus.write_st_port(1, (word & 0x00FF) as u8);
        bus.write_st_port(2, (word >> 8) as u8);
    }

    // Advance enough cycles to cover another frame; auto-transfer should fire.
    for _ in 0..4 {
        bus.tick(200_000, true);
    }

    assert_eq!(bus.vdc_satb_word(0), 0x3333);
    assert_eq!(bus.vdc_satb_word(1), 0x4444);
    assert_ne!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);
}

#[test]
fn vdc_rcr_irq_sets_irq1() {
    let mut bus = Bus::new();
    // Enable RCR interrupt (CR bit 2).
    bus.write_st_port(0, 0x05);
    bus.write_st_port(1, 0x04);
    bus.write_st_port(2, 0x00);
    // Set RCR target to 0x42 (valid: counter base 0x40 + 2 = scanline 3).
    bus.write_st_port(0, 0x06);
    bus.write_st_port(1, 0x42);
    bus.write_st_port(2, 0x00);

    assert_eq!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);
    for _ in 0..100_000 {
        if bus.pending_interrupts() & IRQ_REQUEST_IRQ1 != 0 {
            break;
        }
        bus.tick(1, true);
    }
    assert_ne!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);
    bus.acknowledge_irq(IRQ_REQUEST_IRQ1);
    assert_eq!(bus.pending_interrupts() & IRQ_REQUEST_IRQ1, 0);
}

#[test]
fn vdc_vblank_irq_fires_via_tick() {
    let mut bus = Bus::new();
    bus.set_mpr(0, 0xFF);
    bus.set_mpr(1, 0xFF);

    // Enable VBlank IRQ.
    bus.write_st_port(0, 0x05);
    bus.write_st_port(1, 0x08);
    bus.write_st_port(2, 0x00);

    // Clear any pending VBlank from power-on state.
    bus.read_io(0x00);
    bus.write(0x1403, IRQ_REQUEST_IRQ1);
    let line_cycles = (VDC_VBLANK_INTERVAL + LINES_PER_FRAME as u32 - 1) / LINES_PER_FRAME as u32;
    let visible_lines = VDC_VISIBLE_LINES as u32;
    let min_expected = line_cycles * visible_lines.saturating_sub(1);
    let max_expected = line_cycles * visible_lines.saturating_add(1);

    let mut trigger_iter = None;
    for iter in 0..(VDC_VBLANK_INTERVAL * 2) {
        if bus.tick(1, true) {
            trigger_iter = Some(iter);
            break;
        }
    }
    let trigger_iter = trigger_iter.expect("VBlank IRQ did not trigger within two frame intervals");
    assert!(
        trigger_iter >= min_expected && trigger_iter <= max_expected,
        "VBlank IRQ fired outside expected window: iter={trigger_iter}, min={min_expected}, max={max_expected}"
    );
    assert_ne!(bus.read(0x1403) & IRQ_REQUEST_IRQ1, 0);
    let status = bus.read(0x2000);
    assert!(status & VDC_STATUS_VBL != 0);
    bus.write(0x1403, IRQ_REQUEST_IRQ1);

    // Low-speed mode should need 4x cycles (fresh bus to reset accumulator).
    let mut slow_bus = Bus::new();
    slow_bus.set_mpr(0, 0xFF);
    slow_bus.set_mpr(1, 0xFF);
    slow_bus.write_st_port(0, 0x05);
    slow_bus.write_st_port(1, 0x08);
    slow_bus.write_st_port(2, 0x00);
    slow_bus.read_io(0x00);
    slow_bus.write(0x1403, IRQ_REQUEST_IRQ1);
    let mut trigger_iter_slow = None;
    for iter in 0..(max_expected * 2) {
        if slow_bus.tick(1, false) {
            trigger_iter_slow = Some(iter);
            break;
        }
    }
    let trigger_iter_slow =
        trigger_iter_slow.expect("VBlank IRQ (slow clock) did not trigger within window");
    let slow_phi = trigger_iter_slow * 4;
    assert!(
        slow_phi >= min_expected && slow_phi <= max_expected,
        "Slow-clock VBlank IRQ fired outside expected window: cycles={} min={} max={}",
        slow_phi,
        min_expected,
        max_expected
    );
    assert_ne!(slow_bus.read(0x1403) & IRQ_REQUEST_IRQ1, 0);
}

#[test]
fn vdc_rcr_flag_clears_on_status_read() {
    let mut bus = Bus::new();
    bus.set_mpr(1, 0xFF);
    // Enable RCR interrupt (CR bit 2) — required for the RR status flag
    // to be raised on raster counter match (per HuC6270 hardware).
    bus.write_st_port(0, 0x05);
    bus.write_st_port(1, 0x04);
    bus.write_st_port(2, 0x00);
    // Set RCR target to 0x42 (counter base 0x40 + 2 → scanline 3).
    bus.write_st_port(0, 0x06);
    bus.write_st_port(1, 0x42);
    bus.write_st_port(2, 0x00);

    let line_cycles = (VDC_VBLANK_INTERVAL + LINES_PER_FRAME as u32 - 1) / LINES_PER_FRAME as u32;
    // Advance past scanline 3 to trigger the RCR.
    for _ in 0..=4 {
        bus.tick(line_cycles, true);
    }

    let status = bus.read(0x2000);
    assert!(status & VDC_STATUS_RCR != 0);
    let status_after = bus.read(0x2000);
    assert_eq!(status_after & VDC_STATUS_RCR, 0);
}

#[test]
fn vdc_busy_flag_counts_down() {
    let mut bus = Bus::new();
    bus.set_mpr(1, 0xFF);
    bus.write_st_port(0, 0x02);
    bus.write_st_port(1, 0xAA);
    bus.write_st_port(2, 0x55);

    let status = bus.read(0x2000);
    assert!(status & VDC_STATUS_BUSY != 0);

    bus.tick(VDC_BUSY_ACCESS_CYCLES * 2, true);
    let cleared = bus.read(0x2000);
    assert_eq!(cleared & VDC_STATUS_BUSY, 0);
}

#[test]
fn psg_irq2_triggers_when_enabled() {
    let mut bus = Bus::new();
    bus.write_io(0x1C60, PSG_REG_TIMER_LO as u8);
    bus.write_io(0x1C61, 0x20);
    bus.write_io(0x1C60, PSG_REG_TIMER_HI as u8);
    bus.write_io(0x1C61, 0x00);
    bus.write_io(0x1C60, PSG_REG_TIMER_CTRL as u8);
    bus.write_io(0x1C61, PSG_CTRL_ENABLE | PSG_CTRL_IRQ_ENABLE);

    assert_eq!(bus.pending_interrupts() & IRQ_REQUEST_IRQ2, 0);
    for _ in 0..0x20 {
        bus.tick(1, true);
    }
    assert_ne!(bus.pending_interrupts() & IRQ_REQUEST_IRQ2, 0);
    bus.acknowledge_irq(IRQ_REQUEST_IRQ2);
    assert_eq!(bus.pending_interrupts() & IRQ_REQUEST_IRQ2, 0);
}

#[test]
fn psg_sample_uses_waveform_ram() {
    let mut bus = Bus::new();

    bus.write_io(0x1C60, 0x00);
    bus.write_io(0x1C61, 0x10);
    bus.write_io(0x1C61, 0x01);
    bus.write_io(0x1C61, 0x00);
    bus.write_io(0x1C61, 0x1F);

    bus.write_io(0x1C60, PSG_REG_COUNT as u8);
    bus.write_io(0x1C61, 0x1F);

    bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
    bus.write_io(0x1C61, PSG_CH_CTRL_KEY_ON | 0x1F);

    bus.write_io(0x1C60, PSG_REG_TIMER_LO as u8);
    bus.write_io(0x1C61, 0x20);
    bus.write_io(0x1C60, PSG_REG_TIMER_HI as u8);
    bus.write_io(0x1C61, 0x00);
    bus.write_io(0x1C60, PSG_REG_TIMER_CTRL as u8);
    bus.write_io(0x1C61, PSG_CTRL_ENABLE);

    for _ in 0..(PHI_CYCLES_PER_SAMPLE * 4) {
        bus.tick(1, true);
    }
    let samples = bus.take_audio_samples();
    assert!(samples.iter().any(|s| *s > 0));
}

#[test]
fn audio_diagnostics_track_generation_and_reset() {
    let mut bus = Bus::new();

    bus.write_io(0x1C60, PSG_REG_CH_SELECT as u8);
    bus.write_io(0x1C61, 0x00);
    bus.write_io(0x1C60, PSG_REG_MAIN_BALANCE as u8);
    bus.write_io(0x1C61, 0xFF);
    bus.write_io(0x1C60, PSG_REG_CH_BALANCE as u8);
    bus.write_io(0x1C61, 0xFF);
    bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
    bus.write_io(0x1C61, PSG_CH_CTRL_KEY_ON | PSG_CH_CTRL_DDA | 0x1F);
    bus.write_io(0x1C60, PSG_REG_WAVE_DATA as u8);
    bus.write_io(0x1C61, 0x1F);

    let initial = bus.audio_diagnostics();
    assert_eq!(initial.total_phi_cycles, 0);
    assert_eq!(initial.generated_samples, 0);
    assert_eq!(initial.drained_samples, 0);
    assert_eq!(initial.drain_calls, 0);
    assert_eq!(initial.pending_bus_samples, 0);

    bus.tick(MASTER_CLOCK_HZ, true);

    let generated = bus.audio_diagnostics();
    assert_eq!(generated.total_phi_cycles, MASTER_CLOCK_HZ as u64);
    assert_eq!(generated.generated_samples, AUDIO_SAMPLE_RATE as u64);
    assert_eq!(generated.drained_samples, 0);
    assert_eq!(generated.drain_calls, 0);
    assert_eq!(generated.pending_bus_samples, AUDIO_SAMPLE_RATE as usize);

    let drained = bus.take_audio_samples();
    assert_eq!(drained.len(), AUDIO_SAMPLE_RATE as usize);

    let after_drain = bus.audio_diagnostics();
    assert_eq!(after_drain.generated_samples, AUDIO_SAMPLE_RATE as u64);
    assert_eq!(after_drain.drained_samples, AUDIO_SAMPLE_RATE as u64);
    assert_eq!(after_drain.drain_calls, 1);
    assert_eq!(after_drain.pending_bus_samples, 0);

    bus.reset_audio_diagnostics();

    let reset = bus.audio_diagnostics();
    assert_eq!(reset.total_phi_cycles, 0);
    assert_eq!(reset.generated_samples, 0);
    assert_eq!(reset.drained_samples, 0);
    assert_eq!(reset.drain_calls, 0);
    assert_eq!(reset.pending_bus_samples, 0);
}

#[test]
fn disabling_video_output_clears_pending_frame_trigger() {
    let mut bus = Bus::new();
    bus.vdc.frame_trigger = true;
    bus.frame_ready = true;

    bus.set_video_output_enabled(false);
    bus.tick(0, true);

    assert!(!bus.vdc.frame_ready());
    assert!(!bus.frame_ready);
    assert!(bus.take_frame().is_none());
}

#[test]
fn post_load_fixup_invalidates_transient_render_caches() {
    let mut bus = Bus::new();
    bus.vdc.registers[0x0A] = 0x0202;
    bus.vdc.registers[0x0B] = 0x021F;
    bus.vdc.latch_line_state(0);
    bus.frame_ready = true;
    bus.audio_phi_accumulator = 1234;
    bus.audio_buffer.extend_from_slice(&[1, 2, 3, 4]);
    bus.video_output_enabled = super::types::TransientBool(false);

    assert!(bus.vdc_scroll_line_valid(0));
    assert_eq!(bus.vdc.horizontal_values_for_line(0), (0x0202, 0x021F));

    bus.post_load_fixup();

    assert!(!bus.vdc_scroll_line_valid(0));
    assert!(!bus.frame_ready);
    assert_eq!(bus.audio_phi_accumulator, 0);
    assert!(bus.audio_buffer.is_empty());
    assert!(*bus.video_output_enabled);
    assert_eq!(bus.vdc.horizontal_values_for_line(0), (0x0202, 0x021F));
}

#[test]
fn psg_dda_mode_outputs_direct_level() {
    let mut bus = Bus::new();

    bus.write_io(0x1C60, PSG_REG_CH_SELECT as u8);
    bus.write_io(0x1C61, 0x00);
    bus.write_io(0x1C60, PSG_REG_MAIN_BALANCE as u8);
    bus.write_io(0x1C61, 0xFF);
    bus.write_io(0x1C60, PSG_REG_CH_BALANCE as u8);
    bus.write_io(0x1C61, 0xFF);
    bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
    bus.write_io(0x1C61, PSG_CH_CTRL_KEY_ON | PSG_CH_CTRL_DDA | 0x1F);

    bus.write_io(0x1C60, PSG_REG_WAVE_DATA as u8);
    bus.write_io(0x1C61, 0x1F);
    let hi = bus.psg_sample();

    bus.write_io(0x1C60, PSG_REG_WAVE_DATA as u8);
    bus.write_io(0x1C61, 0x00);
    let lo = bus.psg_sample();

    assert!(hi > 0, "DDA high level should produce positive sample");
    assert!(lo < 0, "DDA low level should produce negative sample");
}

#[test]
fn psg_noise_channel_changes_sample_values() {
    let mut bus = Bus::new();

    bus.write_io(0x1C60, PSG_REG_CH_SELECT as u8);
    bus.write_io(0x1C61, 0x04); // channel 4 supports noise
    bus.write_io(0x1C60, PSG_REG_MAIN_BALANCE as u8);
    bus.write_io(0x1C61, 0xFF);
    bus.write_io(0x1C60, PSG_REG_CH_BALANCE as u8);
    bus.write_io(0x1C61, 0xFF);
    bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
    bus.write_io(0x1C61, PSG_CH_CTRL_KEY_ON | 0x1F);
    bus.write_io(0x1C60, PSG_REG_NOISE_CTRL as u8);
    bus.write_io(0x1C61, PSG_NOISE_ENABLE | 0x1F);

    let mut distinct = std::collections::BTreeSet::new();
    for _ in 0..2048 {
        distinct.insert(bus.psg_sample());
    }
    assert!(
        distinct.len() > 1,
        "noise channel should not output a constant level"
    );
}

#[test]
fn psg_balance_registers_affect_output_amplitude() {
    let mut bus = Bus::new();

    bus.write_io(0x1C60, PSG_REG_CH_SELECT as u8);
    bus.write_io(0x1C61, 0x00);
    bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
    bus.write_io(0x1C61, PSG_CH_CTRL_KEY_ON | PSG_CH_CTRL_DDA | 0x1F);
    bus.write_io(0x1C60, PSG_REG_WAVE_DATA as u8);
    bus.write_io(0x1C61, 0x1F);
    bus.write_io(0x1C60, PSG_REG_CH_BALANCE as u8);
    bus.write_io(0x1C61, 0xFF);

    bus.write_io(0x1C60, PSG_REG_MAIN_BALANCE as u8);
    bus.write_io(0x1C61, 0xFF);
    let full = bus.psg_sample().abs();

    bus.write_io(0x1C60, PSG_REG_MAIN_BALANCE as u8);
    bus.write_io(0x1C61, 0x11);
    let reduced = bus.psg_sample().abs();

    assert!(full > 0);
    assert!(reduced < full);
}

#[test]
fn psg_wave_writes_ignored_while_channel_enabled() {
    let mut bus = Bus::new();

    bus.write_io(0x1C60, PSG_REG_CH_SELECT as u8);
    bus.write_io(0x1C61, 0x00);
    bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
    bus.write_io(0x1C61, PSG_CH_CTRL_KEY_ON | 0x1F);
    bus.write_io(0x1C60, PSG_REG_WAVE_DATA as u8);
    bus.write_io(0x1C61, 0x1F);

    assert_eq!(bus.psg.waveform_ram[0], 0);
    assert_eq!(bus.psg.channels[0].wave_write_pos, 0);
}

#[test]
fn psg_clearing_dda_resets_wave_write_index() {
    let mut bus = Bus::new();

    bus.write_io(0x1C60, PSG_REG_CH_SELECT as u8);
    bus.write_io(0x1C61, 0x00);
    bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
    bus.write_io(0x1C61, 0x00);
    bus.write_io(0x1C60, PSG_REG_WAVE_DATA as u8);
    bus.write_io(0x1C61, 0x04);
    bus.write_io(0x1C60, PSG_REG_WAVE_DATA as u8);
    bus.write_io(0x1C61, 0x05);
    assert_eq!(bus.psg.channels[0].wave_write_pos, 2);

    bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
    bus.write_io(0x1C61, PSG_CH_CTRL_DDA);
    bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
    bus.write_io(0x1C61, 0x00);
    assert_eq!(bus.psg.channels[0].wave_write_pos, 0);

    bus.write_io(0x1C60, PSG_REG_WAVE_DATA as u8);
    bus.write_io(0x1C61, 0x1E);
    assert_eq!(bus.psg.waveform_ram[0], 0x1E);
}

#[test]
fn psg_wave_writes_allowed_with_dda_when_key_off() {
    let mut bus = Bus::new();

    bus.write_io(0x1C60, PSG_REG_CH_SELECT as u8);
    bus.write_io(0x1C61, 0x00);
    bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
    bus.write_io(0x1C61, PSG_CH_CTRL_DDA);
    bus.write_io(0x1C60, PSG_REG_WAVE_DATA as u8);
    bus.write_io(0x1C61, 0x1A);
    bus.write_io(0x1C60, PSG_REG_WAVE_DATA as u8);
    bus.write_io(0x1C61, 0x05);

    assert_eq!(bus.psg.waveform_ram[0], 0x1A);
    assert_eq!(bus.psg.waveform_ram[1], 0x05);
    assert_eq!(bus.psg.channels[0].wave_write_pos, 2);
}

#[test]
fn psg_frequency_divider_uses_inverse_pitch_relation() {
    fn transition_count_for_divider(divider: u16) -> usize {
        let mut bus = Bus::new();
        bus.write_io(0x1C60, PSG_REG_CH_SELECT as u8);
        bus.write_io(0x1C61, 0x00);
        bus.write_io(0x1C60, PSG_REG_MAIN_BALANCE as u8);
        bus.write_io(0x1C61, 0xFF);
        bus.write_io(0x1C60, PSG_REG_CH_BALANCE as u8);
        bus.write_io(0x1C61, 0xFF);
        bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
        bus.write_io(0x1C61, 0x00);
        bus.write_io(0x1C60, PSG_REG_WAVE_DATA as u8);
        for i in 0..PSG_WAVE_SIZE {
            bus.write_io(0x1C61, if i & 0x01 == 0 { 0x00 } else { 0x1F });
        }
        bus.write_io(0x1C60, PSG_REG_FREQ_LO as u8);
        bus.write_io(0x1C61, divider as u8);
        bus.write_io(0x1C60, PSG_REG_FREQ_HI as u8);
        bus.write_io(0x1C61, ((divider >> 8) as u8) & 0x0F);
        bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
        bus.write_io(0x1C61, PSG_CH_CTRL_KEY_ON | 0x1F);

        let mut transitions = 0usize;
        let mut prev = bus.psg_sample();
        for _ in 0..2048 {
            let sample = bus.psg_sample();
            if (sample >= 0) != (prev >= 0) {
                transitions += 1;
            }
            prev = sample;
        }
        transitions
    }

    let fast = transition_count_for_divider(0x0001);
    let slow = transition_count_for_divider(0x0FFF);
    assert!(
        fast > slow.saturating_mul(8),
        "expected divider 0x001 to run much faster than 0xFFF (fast={fast}, slow={slow})"
    );
}

#[test]
fn psg_lfo_halt_stops_modulator_phase_advance() {
    let mut bus = Bus::new();

    bus.write_io(0x1C60, PSG_REG_CH_SELECT as u8);
    bus.write_io(0x1C61, 0x01);
    bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
    bus.write_io(0x1C61, 0x00);
    bus.write_io(0x1C60, PSG_REG_WAVE_DATA as u8);
    for i in 0..PSG_WAVE_SIZE {
        bus.write_io(0x1C61, if i & 0x01 == 0 { 0x00 } else { 0x1F });
    }
    bus.write_io(0x1C60, PSG_REG_FREQ_LO as u8);
    bus.write_io(0x1C61, 0x01);
    bus.write_io(0x1C60, PSG_REG_FREQ_HI as u8);
    bus.write_io(0x1C61, 0x00);
    bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
    bus.write_io(0x1C61, PSG_CH_CTRL_KEY_ON | 0x1F);

    bus.write_io(0x1C60, PSG_REG_LFO_FREQ as u8);
    bus.write_io(0x1C61, 0x01);
    bus.write_io(0x1C60, PSG_REG_LFO_CTRL as u8);
    bus.write_io(0x1C61, 0x01);

    for _ in 0..256 {
        let _ = bus.psg_sample();
    }
    let running_pos = bus.psg.channels[1].wave_pos;
    assert_ne!(running_pos, 0, "active LFO should advance channel 1");

    bus.write_io(0x1C60, PSG_REG_LFO_CTRL as u8);
    bus.write_io(0x1C61, 0x81);
    let halted_pos = bus.psg.channels[1].wave_pos;
    for _ in 0..256 {
        let _ = bus.psg_sample();
    }
    assert_eq!(
        bus.psg.channels[1].wave_pos, halted_pos,
        "LFO halt bit should stop channel 1 phase advance"
    );
}

#[test]
fn psg_dc_blocker_keeps_centered_wave_mean_near_zero() {
    let mut bus = Bus::new();

    bus.write_io(0x1C60, PSG_REG_CH_SELECT as u8);
    bus.write_io(0x1C61, 0x00);
    bus.write_io(0x1C60, PSG_REG_MAIN_BALANCE as u8);
    bus.write_io(0x1C61, 0xFF);
    bus.write_io(0x1C60, PSG_REG_CH_BALANCE as u8);
    bus.write_io(0x1C61, 0xFF);
    bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
    bus.write_io(0x1C61, 0x00);
    bus.write_io(0x1C60, PSG_REG_WAVE_DATA as u8);
    for i in 0..PSG_WAVE_SIZE {
        bus.write_io(0x1C61, if i & 0x01 == 0 { 0x00 } else { 0x1F });
    }
    bus.write_io(0x1C60, PSG_REG_FREQ_LO as u8);
    bus.write_io(0x1C61, 0x10);
    bus.write_io(0x1C60, PSG_REG_FREQ_HI as u8);
    bus.write_io(0x1C61, 0x00);
    bus.write_io(0x1C60, PSG_REG_CH_CONTROL as u8);
    bus.write_io(0x1C61, PSG_CH_CTRL_KEY_ON | 0x1F);

    for _ in 0..1024 {
        let _ = bus.psg_sample();
    }

    let mut sum = 0i64;
    for _ in 0..4096 {
        sum += i64::from(bus.psg_sample());
    }
    let mean = sum / 4096;
    assert!(
        mean.abs() < 64,
        "DC blocker should keep centered waveform mean near zero (mean={mean})"
    );
}
