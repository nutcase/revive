use super::*;

fn write_test_data_rom_byte(rom: &mut [u8], bank: u8, addr: u16, value: u8) {
    let bank = bank & 0x7F;
    let full_addr = ((bank as usize) << 16) | addr as usize;
    let offset = if (full_addr & 0xE0_0000) == 0x40_0000 {
        full_addr
    } else {
        ((full_addr & 0x3F_0000) >> 1) | (full_addr & 0x7FFF)
    };
    rom[offset % rom.len()] = value;
}

fn make_b301_packet_transform_state(r14_packet: u16) -> (SuperFx, Vec<u8>) {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];

    let program = [
        0x3D, 0xA0, 0x32, 0x3F, 0xDF, 0x60, 0x3E, 0xDF, 0x3D, 0xAE, 0x31, 0xEE, 0x11, 0xEF, 0xEE,
        0x21, 0x3D, 0xEF, 0xEE, 0xEE, 0xEE, 0x13, 0xEF, 0xEE, 0x23, 0x3D, 0xEF, 0xEE, 0x12, 0xEF,
        0xEE, 0x22, 0x3D, 0xEF, 0x3D, 0xA9, 0x16,
    ];
    let program_base = 0x01usize * 0x8000 + (0xB301usize & 0x7FFF);
    rom[program_base..program_base + program.len()].copy_from_slice(&program);

    write_test_data_rom_byte(&mut rom, 0x14, 0xD895, 0x00);
    write_test_data_rom_byte(&mut rom, 0x14, 0xD894, 0x20);
    write_test_data_rom_byte(&mut rom, 0x14, 0xD891, 0x06);
    write_test_data_rom_byte(&mut rom, 0x14, 0xD890, 0x02);

    write_test_data_rom_byte(&mut rom, 0x14, 0xD5A9, 0x00);
    write_test_data_rom_byte(&mut rom, 0x14, 0xD5A8, 0x0A);
    write_test_data_rom_byte(&mut rom, 0x14, 0xD5A5, 0x12);
    write_test_data_rom_byte(&mut rom, 0x14, 0xD5A4, 0x00);

    gsu.pbr = 0x01;
    gsu.rombr = 0x00;
    gsu.regs[0] = 0x0824;
    gsu.regs[1] = 0x5191;
    gsu.regs[2] = 0x5190;
    gsu.regs[3] = 0x0700;
    gsu.regs[4] = 0x0000;
    gsu.regs[5] = 0xC7F8;
    gsu.regs[6] = 0x0153;
    gsu.regs[7] = 0xB4B6;
    gsu.regs[8] = 0xB337;
    gsu.regs[9] = 0x2800;
    gsu.regs[10] = 0x0000;
    gsu.regs[11] = 0xB33E;
    gsu.regs[12] = 0x0000;
    gsu.regs[13] = 0xB3DE;
    gsu.write_reg(14, 0x6242);
    gsu.regs[15] = 0xB301;
    gsu.game_ram[0x0062] = (r14_packet & 0x00FF) as u8;
    gsu.game_ram[0x0063] = (r14_packet >> 8) as u8;
    gsu.game_ram[0x0064] = 0x14;
    gsu.game_ram[0x002C] = 0x00;
    gsu.game_ram[0x002D] = 0x40;
    gsu.debug_prepare_cpu_start(&rom);

    (gsu, rom)
}

#[test]
fn b301_packet_transform_consumes_d896_stream() {
    let (mut gsu, rom) = make_b301_packet_transform_state(0xD896);

    gsu.run_steps(&rom, 34);

    assert_eq!(gsu.regs[1], 0x2000);
    assert_eq!(gsu.regs[2], 0x0000);
    assert_eq!(gsu.regs[3], 0x0206);
    assert_eq!(gsu.regs[9], 0x4000);
    assert_eq!(gsu.regs[14], 0xD88E);
    assert_eq!(gsu.regs[15], 0xB327);
}

#[test]
fn b301_packet_transform_consumes_d5aa_stream() {
    let (mut gsu, rom) = make_b301_packet_transform_state(0xD5AA);

    gsu.run_steps(&rom, 34);

    assert_eq!(gsu.regs[1], 0x0A00);
    assert_eq!(gsu.regs[2], 0x0000);
    assert_eq!(gsu.regs[3], 0x0012);
    assert_eq!(gsu.regs[9], 0x4000);
    assert_eq!(gsu.regs[14], 0xD5A2);
    assert_eq!(gsu.regs[15], 0xB327);
}

#[test]
fn d4b4_success_tail_loads_record_fields_into_r9_r13_and_r6() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    let base = 0x01usize * 0x8000 + (0xD4B4usize & 0x7FFF);
    rom[base..base + 44].copy_from_slice(&[
        0xA2, 0x08, 0x22, 0x5B, 0x12, 0x42, 0x22, 0x19, 0x52, 0x90, 0x3D, 0xA4, 0x14, 0x12, 0x54,
        0xA0, 0x05, 0x5B, 0x3D, 0x40, 0x95, 0xA6, 0x0A, 0x26, 0x5B, 0x16, 0x46, 0x26, 0x1D, 0x56,
        0x90, 0x3D, 0xA4, 0x15, 0x16, 0x54, 0xA5, 0x01, 0x25, 0x5B, 0x15, 0x3D, 0x45, 0x00,
    ]);

    gsu.pbr = 0x01;
    gsu.rombr = 0x14;
    gsu.regs[0] = 0x0031;
    gsu.regs[1] = 0xE2DA;
    gsu.regs[2] = 0x004B;
    gsu.regs[3] = 0x0000;
    gsu.regs[4] = 0xE2D3;
    gsu.regs[5] = 0x004B;
    gsu.regs[6] = 0x00FC;
    gsu.regs[7] = 0x004B;
    gsu.regs[8] = 0x0003;
    gsu.regs[9] = 0x004A;
    gsu.regs[10] = 0x04C8;
    gsu.regs[11] = 0x1AD6;
    gsu.regs[12] = 0x0129;
    gsu.regs[13] = 0xD1B4;
    gsu.write_reg(14, 0x8A9D);
    gsu.regs[15] = 0xD4B4;
    gsu.game_ram[0x0028] = 0x2B;
    gsu.game_ram[0x0029] = 0x01;
    gsu.game_ram[0x002A] = 0x0A;
    gsu.game_ram[0x002B] = 0x61;
    gsu.game_ram[0x1ADB] = 0xF9;
    gsu.game_ram[0x1ADE] = 0x32;
    gsu.game_ram[0x1ADF] = 0x00;
    gsu.game_ram[0x1AE0] = 0xF9;
    gsu.game_ram[0x1AE1] = 0xFF;
    gsu.running = true;

    gsu.run_steps(&rom, 64);

    assert_eq!(gsu.regs[2], 0x018E);
    assert_eq!(gsu.regs[6], 0x60FC);
    assert_eq!(gsu.regs[9], 0x0032);
    assert_eq!(gsu.regs[13], 0xFFF9);
    assert_eq!(gsu.regs[15], 0xD4E1);
}

#[test]
fn d4b4_success_tail_rearms_match_word_after_loading_zero_cursor() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    let base = 0x01usize * 0x8000 + (0xD4B4usize & 0x7FFF);
    rom[base..base + 44].copy_from_slice(&[
        0xA2, 0x08, 0x22, 0x5B, 0x12, 0x42, 0x22, 0x19, 0x52, 0x90, 0x3D, 0xA4, 0x14, 0x12, 0x54,
        0xA0, 0x05, 0x5B, 0x3D, 0x40, 0x95, 0xA6, 0x0A, 0x26, 0x5B, 0x16, 0x46, 0x26, 0x1D, 0x56,
        0x90, 0x3D, 0xA4, 0x15, 0x16, 0x54, 0xA5, 0x01, 0x25, 0x5B, 0x15, 0x3D, 0x45, 0x00,
    ]);

    gsu.pbr = 0x01;
    gsu.rombr = 0x14;
    gsu.regs[0] = 0x0031;
    gsu.regs[1] = 0xE2DA;
    gsu.regs[2] = 0x004B;
    gsu.regs[3] = 0x0000;
    gsu.regs[4] = 0xE2D3;
    gsu.regs[5] = 0x004B;
    gsu.regs[6] = 0x00FC;
    gsu.regs[7] = 0x004B;
    gsu.regs[8] = 0x0003;
    gsu.regs[9] = 0x004A;
    gsu.regs[10] = 0x04C8;
    gsu.regs[11] = 0x1AD6;
    gsu.regs[12] = 0x0129;
    gsu.regs[13] = 0xD1B4;
    gsu.write_reg(14, 0x8A7B);
    gsu.regs[15] = 0xD4B4;
    gsu.game_ram[0x0028] = 0x2B;
    gsu.game_ram[0x0029] = 0x01;
    gsu.game_ram[0x002A] = 0x0A;
    gsu.game_ram[0x002B] = 0x61;
    gsu.game_ram[0x1ADB] = 0xF9;
    gsu.game_ram[0x1ADE] = 0x32;
    gsu.game_ram[0x1ADF] = 0x00;
    gsu.game_ram[0x1AE0] = 0x00;
    gsu.game_ram[0x1AE1] = 0x00;
    gsu.running = true;

    gsu.run_steps(&rom, 64);

    assert_eq!(gsu.regs[13], 0x0000);
    assert_eq!(gsu.read_ram_word(0x1AE0), 0xFFF9);
    assert_eq!(gsu.regs[15], 0xD4E1);
}

#[test]
fn d496_success_prelude_builds_continuation_fields_before_tail() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    let base = 0x01usize * 0x8000 + (0xD496usize & 0x7FFF);
    rom[base..base + 33].copy_from_slice(&[
        0x2B, 0x3E, 0x6C, 0xA0, 0x03, 0x5B, 0x3D, 0x40, 0x95, 0xA1, 0x06, 0x21, 0x5B, 0x11, 0x41,
        0x21, 0x13, 0x51, 0x90, 0x3D, 0xA4, 0x13, 0x11, 0x54, 0xA0, 0x04, 0x5B, 0x3D, 0x40, 0x95,
        0xA2, 0x08, 0x00,
    ]);

    gsu.pbr = 0x01;
    gsu.rombr = 0x14;
    gsu.regs[0] = 0x0000;
    gsu.regs[1] = 0x00FC;
    gsu.regs[2] = 0x004B;
    gsu.regs[3] = 0x2B14;
    gsu.regs[4] = 0x0006;
    gsu.regs[5] = 0x004B;
    gsu.regs[6] = 0x00FC;
    gsu.regs[7] = 0x004B;
    gsu.regs[8] = 0x0003;
    gsu.regs[9] = 0x004A;
    gsu.regs[10] = 0x04C8;
    gsu.regs[11] = 0x1AE2;
    gsu.regs[12] = 0x0129;
    gsu.regs[13] = 0xD1B4;
    gsu.write_reg(14, 0x8A7B);
    gsu.regs[15] = 0xD496;
    gsu.game_ram[0x0026] = 0xD3;
    gsu.game_ram[0x0027] = 0xE2;
    gsu.game_ram[0x1AD9] = 0x07;
    gsu.game_ram[0x1ADC] = 0x00;
    gsu.game_ram[0x1ADD] = 0x00;
    gsu.game_ram[0x1ADA] = 0x32;
    gsu.running = true;

    gsu.run_steps(&rom, 27);

    assert_eq!(gsu.regs[1], 0xE2DA);
    assert_eq!(gsu.regs[2], 0x0008);
    assert_eq!(gsu.regs[3], 0x0000);
    assert_eq!(gsu.regs[4], 0xE2D3);
    assert_eq!(gsu.regs[9], 0x004A);
    assert_eq!(gsu.regs[11], 0x1AD6);
    assert_eq!(gsu.regs[13], 0xD1B4);
    assert_eq!(gsu.regs[15], 0xD4B7);
}

#[test]
fn af70_continuation_stream_jumps_to_target_when_pointer_is_nonzero() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000..0x000D].copy_from_slice(&[
        0x11, // TO R1
        0x4A, // LDW [R10] -> R1
        0x11, // TO R1
        0x41, // LDW [R1] -> R1
        0x21, // WITH R1
        0xB1, // MOVES R1 -> R1 (updates Z)
        0x09, 0x05, // BEQ +5 (not taken)
        0x01, // NOP
        0xFF, 0x00, 0x90, // IWT R15,#9000
        0x01, // stale old-stream byte executes once before the transfer
    ]);
    rom[0x1000] = 0x00; // STOP at $9000

    gsu.pbr = 0x00;
    gsu.regs[10] = 0x04C4;
    gsu.regs[15] = 0x8000;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;
    gsu.write_ram_word(0x04C4, 0x887F);
    gsu.write_ram_word(0x887F, 0x29E3);

    gsu.run_steps(&rom, 11);

    assert_eq!(gsu.regs[1], 0x29E3);
    assert!(!gsu.running());
    assert_eq!(gsu.regs[15], 0x9002);
    assert_eq!(gsu.sfr & SFR_Z_BIT, 0);
}

#[test]
fn af70_continuation_stream_branches_locally_when_pointer_is_zero() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000..0x000E].copy_from_slice(&[
        0x11, // TO R1
        0x4A, // LDW [R10] -> R1
        0x11, // TO R1
        0x41, // LDW [R1] -> R1
        0x21, // WITH R1
        0xB1, // MOVES R1 -> R1 (updates Z)
        0x09, 0x05, // BEQ +5 (taken)
        0x01, // skipped NOP
        0xFF, 0x00, 0x90, // skipped IWT R15,#9000
        0x00, // padding at $800C
        0x00, // STOP at branch target $800D
    ]);

    gsu.pbr = 0x00;
    gsu.regs[10] = 0x04C4;
    gsu.regs[15] = 0x8000;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;
    gsu.write_ram_word(0x04C4, 0x887F);
    gsu.write_ram_word(0x887F, 0x0000);

    gsu.run_steps(&rom, 7);

    assert_eq!(gsu.regs[1], 0x0000);
    assert!(gsu.running());
    assert_eq!(gsu.regs[15], 0x800D);
    assert_ne!(gsu.sfr & SFR_Z_BIT, 0);
}

#[test]
fn d1d0_success_fragment_branches_after_writing_record_when_r8_is_not_one() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000..0x0012].copy_from_slice(&[
        0x53, // ADD R3 -> R7
        0xB5, // FROM R5
        0x37, // STW [R7], R5
        0xB6, // FROM R6
        0x3D, // ALT1
        0x33, // STB [R3], R6
        0xA0, 0x01, // IBT R0,#1
        0x3F, // ALT3
        0x68, // CMP R8
        0x08, 0x05, // BNE +5 (taken)
        0x01, // skipped NOP
        0xFF, 0xFD, 0xD3, // skipped IWT R15,#D3FD
        0x00, // padding at $8010
        0x00, // STOP at branch target $8011
    ]);

    gsu.pbr = 0x00;
    gsu.regs[3] = 0x1AD6;
    gsu.regs[5] = 0x004B;
    gsu.regs[6] = 0x00FC;
    gsu.regs[7] = 0x000C;
    gsu.regs[8] = 0x0003;
    gsu.regs[15] = 0x8000;
    gsu.src_reg = 7;
    gsu.dst_reg = 7;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;

    gsu.run_steps(&rom, 14);

    assert_eq!(gsu.regs[7], 0x1AE2);
    assert_eq!(gsu.read_ram_word(0x1AE2), 0x004B);
    assert_eq!(gsu.game_ram[0x1AD6], 0xFC);
    assert!(!gsu.running());
    assert_eq!(gsu.regs[15], 0x8013);
}

#[test]
fn d1d0_success_fragment_falls_through_to_iwt_when_r8_is_one() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000..0x0011].copy_from_slice(&[
        0x53, // ADD R3 -> R7
        0xB5, // FROM R5
        0x37, // STW [R7], R5
        0xB6, // FROM R6
        0x3D, // ALT1
        0x33, // STB [R3], R6
        0xA0, 0x01, // IBT R0,#1
        0x3F, // ALT3
        0x68, // CMP R8
        0x08, 0x05, // BNE +5 (not taken)
        0x01, // NOP
        0xFF, 0xFD, 0xD3, // IWT R15,#D3FD
        0xD0, // delay slot: INC R0
    ]);
    rom[0x53FD] = 0x00; // STOP at $D3FD

    gsu.pbr = 0x00;
    gsu.regs[3] = 0x1AD6;
    gsu.regs[5] = 0x004B;
    gsu.regs[6] = 0x00FC;
    gsu.regs[7] = 0x000C;
    gsu.regs[8] = 0x0001;
    gsu.regs[15] = 0x8000;
    gsu.src_reg = 7;
    gsu.dst_reg = 7;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;

    gsu.run_steps(&rom, 14);

    assert_eq!(gsu.regs[7], 0x1AE2);
    assert_eq!(gsu.read_ram_word(0x1AE2), 0x004B);
    assert_eq!(gsu.game_ram[0x1AD6], 0xFC);
    assert_eq!(gsu.regs[0], 0x0002);
    assert!(!gsu.running());
    assert_eq!(gsu.regs[15], 0xD3FF);
}

#[test]
fn d1d0_success_fragment_dispatches_to_d316_when_r8_is_three() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000..0x0027].copy_from_slice(&[
        0x53, // ADD R3 -> R7
        0xB5, // FROM R5
        0x37, // STW [R7], R5
        0xB6, // FROM R6
        0x3D, // ALT1
        0x33, // STB [R3], R6
        0xA0, 0x01, // IBT R0,#1
        0x3F, // ALT3
        0x68, // CMP R8
        0x08, 0x05, // BNE +5
        0x01, // NOP
        0xFF, 0xFD, 0xD3, // IWT R15,#D3FD
        0x01, // delay slot
        0xA0, 0x02, // IBT R0,#2
        0x3F, // ALT3
        0x68, // CMP R8
        0x08, 0x05, // BNE +5
        0x01, // NOP
        0xFF, 0x87, 0xD3, // IWT R15,#D387
        0x01, // delay slot
        0xA0, 0x03, // IBT R0,#3
        0x3F, // ALT3
        0x68, // CMP R8
        0x08, 0x05, // BNE +5 (not taken)
        0x01, // NOP
        0xFF, 0x16, 0xD3, // IWT R15,#D316
        0xD0, // delay slot: INC R0
    ]);
    rom[0x5316] = 0x00; // STOP at $D316

    gsu.pbr = 0x00;
    gsu.regs[3] = 0x1AD6;
    gsu.regs[5] = 0x004B;
    gsu.regs[6] = 0x00FC;
    gsu.regs[7] = 0x000C;
    gsu.regs[8] = 0x0003;
    gsu.regs[15] = 0x8000;
    gsu.src_reg = 7;
    gsu.dst_reg = 7;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;

    gsu.run_steps(&rom, 24);

    assert_eq!(gsu.regs[7], 0x1AE2);
    assert_eq!(gsu.read_ram_word(0x1AE2), 0x004B);
    assert_eq!(gsu.game_ram[0x1AD6], 0xFC);
    assert_eq!(gsu.regs[0], 0x0004);
    assert!(!gsu.running());
    assert_eq!(gsu.regs[15], 0xD318);
}

fn run_d316_success_fragment_sample(
    r2_start: u16,
    r3_start: u16,
    r4_start: u16,
    r9_start: u16,
    r12_start: u16,
    expected_r0: u16,
    expected_r2: u16,
    expected_r4: u16,
) {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    let base = 0x01usize * 0x8000;
    rom[base..base + 43].copy_from_slice(&[
        0x60, // SUB R0 -> R0
        0xA7, 0x01, // IBT R7,#1
        0x27, // WITH R7
        0x53, // ADD R3 -> R7
        0xB0, // FROM R0
        0x3D, // ALT1
        0x37, // STB [R7], R0
        0xB2, // FROM R2
        0x4D, // SWAP
        0x97, // ROR
        0x52, // ADD R2
        0x12, // TO R2
        0x3D, // ALT1
        0x52, // ADC R2
        0xA0, 0x0F, // IBT R0,#0F
        0x72, // AND R2
        0xA7, 0x07, // IBT R7,#7
        0x67, // SUB R7
        0xA7, 0x03, // IBT R7,#3
        0x27, // WITH R7
        0x53, // ADD R3 -> R7
        0xB0, // FROM R0
        0x3D, // ALT1
        0x37, // STB [R7], R0
        0x20, // WITH R0
        0xB0, // MOVES R0
        0x0A, 0x03, // BPL +3
        0x01, // NOP
        0x4F, // NOT
        0xD0, // INC R0
        0x20, // WITH R0
        0x14, // TO R4
        0xB2, // FROM R2
        0x4D, // SWAP
        0x97, // ROR
        0x52, // ADD R2
        0x12, // TO R2
        0x00, // STOP before the next ALT1/ADC stage
    ]);

    gsu.pbr = 0x01;
    gsu.regs[0] = 0x0003;
    gsu.regs[1] = 0x00FC;
    gsu.regs[2] = r2_start;
    gsu.regs[3] = r3_start;
    gsu.regs[4] = r4_start;
    gsu.regs[5] = 0x004B;
    gsu.regs[6] = 0x00FC;
    gsu.regs[7] = r3_start.wrapping_add(0x000C);
    gsu.regs[8] = 0x0003;
    gsu.regs[9] = r9_start;
    gsu.regs[10] = 0x04C8;
    gsu.regs[11] = 0xAD88;
    gsu.regs[12] = r12_start;
    gsu.regs[13] = 0xD1B4;
    gsu.regs[14] = 0x8A7B;
    gsu.regs[15] = 0x8000;
    gsu.src_reg = 0;
    gsu.dst_reg = 0;
    gsu.running = true;
    gsu.sfr = 0x0066 | SFR_GO_BIT;
    gsu.game_ram[r3_start as usize + 1] = 0xA2;
    gsu.game_ram[r3_start as usize + 3] = 0xFF;

    gsu.run_steps(&rom, 64);

    assert_eq!(gsu.regs[0], expected_r0);
    assert_eq!(gsu.regs[2], expected_r2);
    assert_eq!(gsu.regs[4], expected_r4);
    assert_eq!(gsu.regs[7], r3_start.wrapping_add(3));
    assert_eq!(gsu.game_ram[r3_start as usize + 1], 0x00);
    assert_eq!(gsu.game_ram[r3_start as usize + 3], expected_r4 as u8);
    assert!(!gsu.running());
    assert_eq!(gsu.regs[15], 0x802C);
}

#[test]
fn d316_success_fragment_matches_live_trace_for_88ed_sample() {
    run_d316_success_fragment_sample(
        0x88ED, 0x1AD6, 0x00FB, 0x004E, 0x0129, 0xD7E2, 0x889E, 0x0007,
    );
}

#[test]
fn d316_success_fragment_matches_live_trace_for_1a60_sample() {
    run_d316_success_fragment_sample(
        0x1A60, 0x1D3E, 0x000E, 0x004D, 0x00FD, 0xCB7F, 0x64CD, 0x0006,
    );
}

#[test]
fn d316_success_fragment_matches_live_trace_for_65a3_sample() {
    run_d316_success_fragment_sample(
        0x65A3, 0x1F0C, 0x000D, 0x004C, 0x00DC, 0x9906, 0x1CF8, 0x0001,
    );
}

#[test]
fn late_search_key_override_uses_match_word_when_raw_key_is_missing() {
    let prev = std::env::var_os("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_1AE2");
    std::env::set_var("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_1AE2", "1");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.regs[7] = 0x5A89;
    gsu.regs[15] = 0xD47A;
    gsu.write_ram_word(0x1AE0, 0xFFF9);
    gsu.write_ram_word(0x1AE2, 0x004B);
    gsu.write_ram_word(0x1AB8, 0x004B);

    gsu.maybe_force_starfox_late_search_key_from_match();
    assert_eq!(gsu.regs[7], 0x004B);

    if let Some(value) = prev {
        std::env::set_var("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_1AE2", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_1AE2");
    }
}

#[test]
fn parser_key_override_uses_match_word_when_ad46_writes_missing_head_key() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var_os("STARFOX_FORCE_PARSER_KEY_FROM_MATCH_WORD");
    std::env::set_var("STARFOX_FORCE_PARSER_KEY_FROM_MATCH_WORD", "1");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;
    gsu.current_exec_pc = 0xAD46;
    gsu.current_exec_opcode = 0xA0;
    gsu.write_ram_word(0x1AE0, 0xFFF9);
    gsu.write_ram_word(0x1AE2, 0x004B);
    gsu.write_ram_word(0x1AB8, 0x004B);

    gsu.write_ram_word(0x0136, 0x5ECF);

    assert_eq!(gsu.debug_read_ram_word_short(0x0136), 0x004B);

    if let Some(value) = prev {
        std::env::set_var("STARFOX_FORCE_PARSER_KEY_FROM_MATCH_WORD", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_PARSER_KEY_FROM_MATCH_WORD");
    }
}

#[test]
fn parser_key_override_can_promote_any_table_field_to_record_head() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var_os("STARFOX_FORCE_PARSER_KEY_FROM_ANY_TABLE_FIELD");
    std::env::set_var("STARFOX_FORCE_PARSER_KEY_FROM_ANY_TABLE_FIELD", "1");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;
    gsu.current_exec_pc = 0xAD46;
    gsu.current_exec_opcode = 0xA0;
    gsu.write_ram_word_short(0x1AB8, 0x004B);
    gsu.write_ram_word_short(0x1AB8 + 12, 0x5A89);

    gsu.write_ram_word_short(0x0136, 0x5A89);
    assert_eq!(gsu.debug_read_ram_word_short(0x0136), 0x004B);

    if let Some(value) = prev {
        std::env::set_var("STARFOX_FORCE_PARSER_KEY_FROM_ANY_TABLE_FIELD", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_PARSER_KEY_FROM_ANY_TABLE_FIELD");
    }
}

#[test]
fn late_search_key_override_can_promote_any_table_field_to_record_head() {
    let prev = std::env::var_os("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_ANY_TABLE_FIELD");
    std::env::set_var("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_ANY_TABLE_FIELD", "1");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.regs[7] = 0x5A89;
    gsu.regs[15] = 0xD47A;
    gsu.write_ram_word_short(0x1AB8, 0x004B);
    gsu.write_ram_word_short(0x1AB8 + 12, 0x5A89);

    gsu.maybe_force_starfox_late_search_key_from_match();
    assert_eq!(gsu.regs[7], 0x004B);

    if let Some(value) = prev {
        std::env::set_var("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_ANY_TABLE_FIELD", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_LATE_SEARCH_KEY_FROM_ANY_TABLE_FIELD");
    }
}

#[test]
fn continuation_ptr_override_redirects_887f_write_to_match_fragment() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var_os("STARFOX_FORCE_CONTINUATION_PTR_FROM_MATCH_FRAGMENT");
    std::env::set_var("STARFOX_FORCE_CONTINUATION_PTR_FROM_MATCH_FRAGMENT", "1");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;
    gsu.current_exec_pc = 0xB396;
    gsu.current_exec_opcode = 0x31;
    gsu.write_ram_word_short(0x888C, 0x4BFC);
    gsu.game_ram[0x021F] = 0x88;

    gsu.write_ram_byte(0x021E, 0x7F);

    assert_eq!(gsu.debug_read_ram_word_short(0x021E), 0x888D);

    if let Some(value) = prev {
        std::env::set_var("STARFOX_FORCE_CONTINUATION_PTR_FROM_MATCH_FRAGMENT", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_CONTINUATION_PTR_FROM_MATCH_FRAGMENT");
    }
}

#[test]
fn continuation_cursor_override_redirects_04c4_887f_to_match_fragment() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var_os("STARFOX_FORCE_CONTINUATION_CURSOR_FROM_MATCH_FRAGMENT");
    std::env::set_var("STARFOX_FORCE_CONTINUATION_CURSOR_FROM_MATCH_FRAGMENT", "1");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;
    gsu.current_exec_pc = 0xACAD;
    gsu.write_ram_word_short(0x888C, 0x4BFC);

    gsu.write_ram_word(0x04C4, 0x887F);

    assert_eq!(gsu.debug_read_ram_word_short(0x04C4), 0x888D);

    if let Some(value) = prev {
        std::env::set_var(
            "STARFOX_FORCE_CONTINUATION_CURSOR_FROM_MATCH_FRAGMENT",
            value,
        );
    } else {
        std::env::remove_var("STARFOX_FORCE_CONTINUATION_CURSOR_FROM_MATCH_FRAGMENT");
    }
}

#[test]
fn continuation_cursor_override_accepts_explicit_env_value() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var_os("STARFOX_FORCE_CONTINUATION_CURSOR_VALUE");
    std::env::set_var("STARFOX_FORCE_CONTINUATION_CURSOR_VALUE", "8890");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;
    gsu.current_exec_pc = 0xACAD;

    gsu.write_ram_word(0x04C4, 0x887F);

    assert_eq!(gsu.debug_read_ram_word_short(0x04C4), 0x8890);

    if let Some(value) = prev {
        std::env::set_var("STARFOX_FORCE_CONTINUATION_CURSOR_VALUE", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_CONTINUATION_CURSOR_VALUE");
    }
}

#[test]
fn continuation_cursor_override_can_null_stream_after_success_fragment() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var_os("STARFOX_NULL_CONTINUATION_AFTER_SUCCESS");
    std::env::set_var("STARFOX_NULL_CONTINUATION_AFTER_SUCCESS", "1");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;
    gsu.current_exec_pc = 0xACAD;
    gsu.write_ram_word_short(0x1AE0, 0xFFF9);
    gsu.write_ram_word_short(0x1AE2, 0x004B);
    gsu.write_ram_word_short(0x888C, 0x4BFC);

    assert_eq!(
        gsu.maybe_force_starfox_continuation_cursor_word(0x04C4, 0x29E3),
        0x0000
    );

    if let Some(value) = prev {
        std::env::set_var("STARFOX_NULL_CONTINUATION_AFTER_SUCCESS", value);
    } else {
        std::env::remove_var("STARFOX_NULL_CONTINUATION_AFTER_SUCCESS");
    }
}

#[test]
fn success_cursor_override_keeps_1ae0_armed_at_d1cc() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var_os("STARFOX_KEEP_SUCCESS_CURSOR_ARMED");
    std::env::set_var("STARFOX_KEEP_SUCCESS_CURSOR_ARMED", "1");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;
    gsu.current_exec_pc = 0xD1CC;

    gsu.write_ram_word(0x1AE0, 0x0000);

    assert_eq!(gsu.debug_read_ram_word_short(0x1AE0), 0xFFF9);

    if let Some(value) = prev {
        std::env::set_var("STARFOX_KEEP_SUCCESS_CURSOR_ARMED", value);
    } else {
        std::env::remove_var("STARFOX_KEEP_SUCCESS_CURSOR_ARMED");
    }
}

#[test]
fn success_branch_target_override_keeps_r13_at_d1b4() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var_os("STARFOX_KEEP_SUCCESS_BRANCH_TARGET");
    std::env::set_var("STARFOX_KEEP_SUCCESS_BRANCH_TARGET", "1");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;
    gsu.regs[13] = 0xD1B4;

    gsu.write_reg_exec(13, 0x0000, 0x1D, 0xD4D0);

    assert_eq!(gsu.regs[13], 0xD1B4);

    if let Some(value) = prev {
        std::env::set_var("STARFOX_KEEP_SUCCESS_BRANCH_TARGET", value);
    } else {
        std::env::remove_var("STARFOX_KEEP_SUCCESS_BRANCH_TARGET");
    }
}

#[test]
fn success_branch_target_override_can_redirect_tail_to_b196() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var_os("STARFOX_FORCE_SUCCESS_BRANCH_TO_B196");
    std::env::set_var("STARFOX_FORCE_SUCCESS_BRANCH_TO_B196", "1");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;
    gsu.regs[13] = 0xD1B4;

    gsu.write_reg_exec(13, 0x0000, 0x1D, 0xD4D0);

    assert_eq!(gsu.regs[13], 0xB196);

    if let Some(value) = prev {
        std::env::set_var("STARFOX_FORCE_SUCCESS_BRANCH_TO_B196", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_SUCCESS_BRANCH_TO_B196");
    }
}

#[test]
fn b30a_r14_seed_override_applies_only_on_matching_frame_and_pc() {
    let _guard = env_lock().lock().unwrap();
    let prev_value = std::env::var_os("STARFOX_FORCE_B30A_R14_VALUE");
    let prev_frame = std::env::var_os("STARFOX_FORCE_B30A_R14_FRAME");
    std::env::set_var("STARFOX_FORCE_B30A_R14_VALUE", "0x0000");
    std::env::set_var("STARFOX_FORCE_B30A_R14_FRAME", "163");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;

    super::set_trace_superfx_exec_frame(163);
    assert_eq!(
        gsu.maybe_force_starfox_b30a_r14_seed(14, 0xD896, 0xB30A),
        0x0000
    );
    assert_eq!(
        gsu.maybe_force_starfox_b30a_r14_seed(13, 0xD896, 0xB30A),
        0xD896
    );
    assert_eq!(
        gsu.maybe_force_starfox_b30a_r14_seed(14, 0xD896, 0xB30B),
        0xD896
    );

    super::set_trace_superfx_exec_frame(164);
    assert_eq!(
        gsu.maybe_force_starfox_b30a_r14_seed(14, 0xD896, 0xB30A),
        0xD896
    );

    if let Some(value) = prev_value {
        std::env::set_var("STARFOX_FORCE_B30A_R14_VALUE", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_B30A_R14_VALUE");
    }
    if let Some(value) = prev_frame {
        std::env::set_var("STARFOX_FORCE_B30A_R14_FRAME", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_B30A_R14_FRAME");
    }
}

#[test]
fn b380_r12_seed_override_applies_only_on_matching_frame_and_pc() {
    let _guard = env_lock().lock().unwrap();
    let prev_value = std::env::var_os("STARFOX_FORCE_B380_R12_VALUE");
    let prev_frame = std::env::var_os("STARFOX_FORCE_B380_R12_FRAME");
    std::env::set_var("STARFOX_FORCE_B380_R12_VALUE", "6");
    std::env::set_var("STARFOX_FORCE_B380_R12_FRAME", "163");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;

    super::set_trace_superfx_exec_frame(163);
    assert_eq!(
        gsu.maybe_force_starfox_b380_r12_seed(12, 0x0008, 0xB380),
        0x0006
    );
    assert_eq!(
        gsu.maybe_force_starfox_b380_r12_seed(11, 0x0008, 0xB380),
        0x0008
    );
    assert_eq!(
        gsu.maybe_force_starfox_b380_r12_seed(12, 0x0008, 0xB381),
        0x0008
    );

    super::set_trace_superfx_exec_frame(164);
    assert_eq!(
        gsu.maybe_force_starfox_b380_r12_seed(12, 0x0008, 0xB380),
        0x0008
    );

    if let Some(value) = prev_value {
        std::env::set_var("STARFOX_FORCE_B380_R12_VALUE", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_B380_R12_VALUE");
    }
    if let Some(value) = prev_frame {
        std::env::set_var("STARFOX_FORCE_B380_R12_FRAME", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_B380_R12_FRAME");
    }
}

#[test]
fn b384_preexec_live_state_override_applies_only_on_matching_frame_and_pc() {
    let _guard = env_lock().lock().unwrap();
    let prev_r12 = std::env::var_os("STARFOX_FORCE_B384_PREEXEC_R12_VALUE");
    let prev_r14 = std::env::var_os("STARFOX_FORCE_B384_PREEXEC_R14_VALUE");
    let prev_frame = std::env::var_os("STARFOX_FORCE_B384_PREEXEC_FRAME");
    std::env::set_var("STARFOX_FORCE_B384_PREEXEC_R12_VALUE", "6");
    std::env::set_var("STARFOX_FORCE_B384_PREEXEC_R14_VALUE", "0");
    std::env::set_var("STARFOX_FORCE_B384_PREEXEC_FRAME", "163");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;
    gsu.regs[12] = 0x0008;
    gsu.regs[14] = 0xCD92;

    super::set_trace_superfx_exec_frame(163);
    gsu.maybe_force_starfox_b384_preexec_live_state(0xB384);
    assert_eq!(gsu.regs[12], 0x0006);
    assert_eq!(gsu.regs[14], 0x0000);

    gsu.regs[12] = 0x0008;
    gsu.regs[14] = 0xCD92;
    gsu.maybe_force_starfox_b384_preexec_live_state(0xB388);
    assert_eq!(gsu.regs[12], 0x0006);
    assert_eq!(gsu.regs[14], 0x0000);

    gsu.regs[12] = 0x0008;
    gsu.regs[14] = 0xCD92;
    gsu.maybe_force_starfox_b384_preexec_live_state(0xB396);
    assert_eq!(gsu.regs[12], 0x0006);
    assert_eq!(gsu.regs[14], 0x0000);

    gsu.regs[12] = 0x0008;
    gsu.regs[14] = 0xCD92;
    gsu.maybe_force_starfox_b384_preexec_live_state(0xB397);
    assert_eq!(gsu.regs[12], 0x0008);
    assert_eq!(gsu.regs[14], 0xCD92);

    super::set_trace_superfx_exec_frame(164);
    gsu.maybe_force_starfox_b384_preexec_live_state(0xB384);
    assert_eq!(gsu.regs[12], 0x0008);
    assert_eq!(gsu.regs[14], 0xCD92);

    if let Some(value) = prev_r12 {
        std::env::set_var("STARFOX_FORCE_B384_PREEXEC_R12_VALUE", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_B384_PREEXEC_R12_VALUE");
    }
    if let Some(value) = prev_r14 {
        std::env::set_var("STARFOX_FORCE_B384_PREEXEC_R14_VALUE", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_B384_PREEXEC_R14_VALUE");
    }
    if let Some(value) = prev_frame {
        std::env::set_var("STARFOX_FORCE_B384_PREEXEC_FRAME", value);
    } else {
        std::env::remove_var("STARFOX_FORCE_B384_PREEXEC_FRAME");
    }
}

#[test]
fn success_context_override_keeps_r9_and_r13_when_success_tail_zeroes_them() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var_os("STARFOX_KEEP_SUCCESS_CONTEXT");
    std::env::set_var("STARFOX_KEEP_SUCCESS_CONTEXT", "1");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;
    gsu.regs[7] = 0x004B;
    gsu.regs[9] = 0x004A;
    gsu.regs[13] = 0xD1B4;

    gsu.write_reg_exec(9, 0x0000, 0x19, 0xD4BB);
    gsu.write_reg_exec(13, 0x0000, 0x1D, 0xD4D0);

    assert_eq!(gsu.regs[9], 0x004A);
    assert_eq!(gsu.regs[13], 0xD1B4);

    if let Some(value) = prev {
        std::env::set_var("STARFOX_KEEP_SUCCESS_CONTEXT", value);
    } else {
        std::env::remove_var("STARFOX_KEEP_SUCCESS_CONTEXT");
    }
}

#[test]
fn ac98_override_can_null_success_continuation_word_before_bad_parser_handoff() {
    let _guard = env_lock().lock().unwrap();
    let prev = std::env::var_os("STARFOX_NULL_AC98_AFTER_SUCCESS");
    std::env::set_var("STARFOX_NULL_AC98_AFTER_SUCCESS", "1");

    let mut gsu = SuperFx::new(0x20_0000);
    gsu.running = true;
    gsu.pbr = 0x01;
    gsu.current_exec_pbr = 0x01;
    gsu.write_ram_word_short(0x1AE2, 0x004B);
    gsu.write_ram_word_short(0x888C, 0x4BFC);

    gsu.write_reg_exec(1, 0x887F, 0xF1, 0xAC98);

    assert_eq!(gsu.regs[1], 0x0000);

    if let Some(value) = prev {
        std::env::set_var("STARFOX_NULL_AC98_AFTER_SUCCESS", value);
    } else {
        std::env::remove_var("STARFOX_NULL_AC98_AFTER_SUCCESS");
    }
}

#[test]
fn d31a_success_fragment_clears_record_plus_one_flag_byte() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000..0x0008].copy_from_slice(&[
        0xA7, 0x01, // IBT R7,#1
        0x27, // WITH R7
        0x53, // ADD R3 -> R7
        0xB0, // FROM R0
        0x3D, // ALT1
        0x37, // STB [R7], R0
        0x00, // STOP
    ]);

    gsu.pbr = 0x00;
    gsu.regs[0] = 0x0000;
    gsu.regs[3] = 0x1AD6;
    gsu.regs[7] = 0x000C;
    gsu.regs[15] = 0x8000;
    gsu.game_ram[0x1AD7] = 0xA2;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;

    for _ in 0..16 {
        if !gsu.running() {
            break;
        }
        gsu.run_steps(&rom, 1);
    }

    assert_eq!(gsu.regs[7], 0x1AD7);
    assert_eq!(gsu.game_ram[0x1AD7], 0x00);
    assert!(!gsu.running());
}

#[test]
fn d4d8_success_tail_sets_low_bit_on_zero_flag_byte() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000..0x0015].copy_from_slice(&[
        0xA5, 0x01, // IBT R5,#1
        0x25, // WITH R5
        0x5B, // ADD R11 -> R5
        0x15, // TO R5
        0x3D, // ALT1
        0x45, // LDB [R5] -> R5
        0xB5, // FROM R5
        0x3E, // ALT2
        0xC1, // OR #1 -> R0
        0xA4, 0x01, // IBT R4,#1
        0x24, // WITH R4
        0x5B, // ADD R11 -> R4
        0xB0, // FROM R0
        0x3D, // ALT1
        0x34, // STB [R4], R0
        0xB5, // FROM R5
        0x3E, // ALT2
        0x72, // AND #2 -> R0
        0x00, // STOP
    ]);

    gsu.pbr = 0x00;
    gsu.regs[1] = 0xE2DA;
    gsu.regs[2] = 0x007D;
    gsu.regs[4] = 0xFC4B;
    gsu.regs[5] = 0x0001;
    gsu.regs[6] = 0xFC44;
    gsu.regs[7] = 0x004B;
    gsu.regs[8] = 0x0003;
    gsu.regs[10] = 0x04C8;
    gsu.regs[11] = 0x1AD6;
    gsu.regs[12] = 0x0129;
    gsu.regs[14] = 0x8A7B;
    gsu.regs[15] = 0x8000;
    gsu.game_ram[0x1AD7] = 0x00;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;

    for _ in 0..32 {
        if !gsu.running() {
            break;
        }
        gsu.run_steps(&rom, 1);
    }

    assert_eq!(gsu.game_ram[0x1AD7], 0x01);
    assert_eq!(gsu.regs[0], 0x0000);
    assert!(!gsu.running());
}

#[test]
fn d4d8_success_tail_preserves_existing_flag_bits_before_setting_low_bit() {
    let mut gsu = SuperFx::new(0x20_0000);
    let mut rom = vec![0u8; 0x20_0000];
    rom[0x0000..0x0015].copy_from_slice(&[
        0xA5, 0x01, // IBT R5,#1
        0x25, // WITH R5
        0x5B, // ADD R11 -> R5
        0x15, // TO R5
        0x3D, // ALT1
        0x45, // LDB [R5] -> R5
        0xB5, // FROM R5
        0x3E, // ALT2
        0xC1, // OR #1 -> R0
        0xA4, 0x01, // IBT R4,#1
        0x24, // WITH R4
        0x5B, // ADD R11 -> R4
        0xB0, // FROM R0
        0x3D, // ALT1
        0x34, // STB [R4], R0
        0xB5, // FROM R5
        0x3E, // ALT2
        0x72, // AND #2 -> R0
        0x00, // STOP
    ]);

    gsu.pbr = 0x00;
    gsu.regs[1] = 0xE2DA;
    gsu.regs[2] = 0x007D;
    gsu.regs[4] = 0xFC4B;
    gsu.regs[5] = 0x0001;
    gsu.regs[6] = 0xFC44;
    gsu.regs[7] = 0x004B;
    gsu.regs[8] = 0x0003;
    gsu.regs[10] = 0x04C8;
    gsu.regs[11] = 0x1AD6;
    gsu.regs[12] = 0x0129;
    gsu.regs[14] = 0x8A7B;
    gsu.regs[15] = 0x8000;
    gsu.game_ram[0x1AD7] = 0xA2;
    gsu.running = true;
    gsu.sfr |= SFR_GO_BIT;

    for _ in 0..32 {
        if !gsu.running() {
            break;
        }
        gsu.run_steps(&rom, 1);
    }

    assert_eq!(gsu.game_ram[0x1AD7], 0xA3);
    assert_eq!(gsu.regs[0], 0x0002);
    assert!(!gsu.running());
}
