
use super::Z80;
use crate::audio::AudioBus;
use crate::cartridge::Cartridge;
use crate::input::IoBus;
use crate::vdp::Vdp;

fn dummy_cart() -> Cartridge {
    Cartridge::from_bytes(vec![0; 0x200]).expect("valid cart")
}

#[test]
fn bus_request_register_controls_halt_state() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    assert_eq!(z80.read_busreq_byte(), 0x01);

    z80.write_busreq_byte(0x01);
    assert_eq!(z80.read_busreq_byte(), 0x01);
    z80.step(16, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.read_busreq_byte(), 0x00);
    z80.write_busreq_byte(0x00);
    assert_eq!(z80.read_busreq_byte(), 0x01);
}

#[test]
fn reset_register_controls_run_state_and_cycles() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.step(100, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.cycles(), 0);

    z80.write_reset_byte(0x01); // release reset
    z80.step(100, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.cycles(), 46);

    z80.write_busreq_byte(0x01); // bus requested -> grant pending, still running
    z80.step(8, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.cycles(), 50);

    z80.step(8, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io); // grant reached at the end of this slice.
    assert_eq!(z80.cycles(), 54);

    z80.step(100, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io); // bus granted -> halt
    assert_eq!(z80.cycles(), 54);
}

#[test]
fn bus_grant_mid_slice_only_runs_until_grant_edge() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.write_busreq_byte(0x01);

    // BUSACK delay is 16 M68k cycles; a larger slice must not run beyond it.
    z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

    let expected = ((16u64 * super::Z80_CLOCK_HZ) / super::M68K_CLOCK_HZ) as u64;
    assert_eq!(z80.cycles(), expected);
    assert!(z80.bus_granted());
}

#[test]
fn m68k_ram_access_becomes_available_after_bus_grant_delay() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_busreq_byte(0x01);
    assert!(!z80.m68k_can_access_ram());

    z80.step(8, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert!(!z80.m68k_can_access_ram());

    z80.step(100, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert!(z80.m68k_can_access_ram());
}

#[test]
fn bus_granted_or_reset_still_advances_audio_busy_timer() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();

    audio.write_ym2612(0, 0x22);
    audio.write_ym2612(1, 0x0F);
    assert_ne!(audio.read_ym2612(0) & 0x80, 0);

    // While reset is asserted, Z80 CPU is halted, but YM time should still pass.
    let mut cleared = false;
    for _ in 0..8 {
        z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        if (audio.read_ym2612(0) & 0x80) == 0 {
            cleared = true;
            break;
        }
    }
    assert!(cleared);

    // Re-arm busy and verify BUSREQ-granted state also advances YM time.
    audio.write_ym2612(0, 0x22);
    audio.write_ym2612(1, 0x10);
    assert_ne!(audio.read_ym2612(0) & 0x80, 0);
    z80.write_reset_byte(0x01);
    z80.write_busreq_byte(0x01);
    z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert!(z80.bus_granted());
    let mut cleared = false;
    for _ in 0..8 {
        z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        if (audio.read_ym2612(0) & 0x80) == 0 {
            cleared = true;
            break;
        }
    }
    assert!(cleared);
}

#[test]
fn z80_ram_is_8kb_and_mirrored() {
    let mut z80 = Z80::new();
    z80.write_ram_u8(0x0001, 0x12);
    z80.write_ram_u8(0x2001, 0x34); // mirror of 0x0001

    assert_eq!(z80.read_ram_u8(0x0001), 0x34);
    assert_eq!(z80.read_ram_u8(0x2001), 0x34);
}

#[test]
fn executes_program_that_writes_ym2612_register() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // ld a,0x22 ; ld (0x4000),a ; ld a,0x0F ; ld (0x4001),a ; halt
    let program = [
        0x3E, 0x22, 0x32, 0x00, 0x40, 0x3E, 0x0F, 0x32, 0x01, 0x40, 0x76,
    ];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(400, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(audio.ym2612().register(0, 0x22), 0x0F);
}

#[test]
fn executes_program_that_writes_psg() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // ld a,0x9F ; ld (0x7F11),a ; halt
    let program = [0x3E, 0x9F, 0x32, 0x11, 0x7F, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(200, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(audio.psg().last_data(), 0x9F);
}

#[test]
fn cpl_and_rla_are_implemented() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // LD A,0x80 ; RLA ; CPL ; HALT
    let program = [0x3E, 0x80, 0x17, 0x2F, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.a, 0xFF);
    assert_ne!(z80.f & super::FLAG_C, 0);
    assert_ne!(z80.f & super::FLAG_N, 0);
    assert_ne!(z80.f & super::FLAG_H, 0);
}

#[test]
fn scf_and_ccf_update_halfcarry_and_subtract_flags() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // XOR A ; SCF ; CCF ; HALT
    let program = [0xAF, 0x37, 0x3F, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.f & super::FLAG_C, 0);
    // CCF should move old carry into H and clear N.
    assert_ne!(z80.f & super::FLAG_H, 0);
    assert_eq!(z80.f & super::FLAG_N, 0);
}

#[test]
fn scf_and_ccf_take_xy_from_a() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // LD A,0x28 ; SCF ; CCF ; HALT
    let program = [0x3E, 0x28, 0x37, 0x3F, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.f & super::FLAG_C, 0);
    assert_ne!(z80.f & super::FLAG_H, 0);
    assert_eq!(z80.f & super::FLAG_N, 0);
    assert_eq!(
        z80.f & (super::FLAG_X | super::FLAG_Y),
        super::FLAG_X | super::FLAG_Y
    );
}

#[test]
fn index_prefixed_sub_and_sbc_memory_are_implemented() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.ix = 0x0100;
    z80.a = 5;

    z80.write_ram_u8(0x0101, 1);
    z80.write_ram_u8(0x0102, 2);

    // SUB A,(IX+1) ; SBC A,(IX+2) ; HALT
    let program = [0xDD, 0x96, 0x01, 0xDD, 0x9E, 0x02, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.a, 2);
}

#[test]
fn ed_neg_sets_n_h_and_c_flags() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // LD A,0x01 ; ED 44 (NEG) ; HALT
    let program = [0x3E, 0x01, 0xED, 0x44, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.a, 0xFF);
    assert_ne!(z80.f & super::FLAG_N, 0);
    assert_ne!(z80.f & super::FLAG_H, 0);
    assert_ne!(z80.f & super::FLAG_C, 0);
    assert_eq!(
        z80.f & (super::FLAG_X | super::FLAG_Y),
        super::FLAG_X | super::FLAG_Y
    );

    z80.write_reset_byte(0x00);
    z80.write_reset_byte(0x01);
    // LD A,0x80 ; NEG ; HALT -> overflow sets PV
    let program = [0x3E, 0x80, 0xED, 0x44, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.a, 0x80);
    assert_ne!(z80.f & super::FLAG_PV, 0);
    assert_eq!(z80.f & (super::FLAG_X | super::FLAG_Y), 0);
}

#[test]
fn ed_in_b_sets_parity_and_preserves_carry() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // SCF ; LD BC,0x007F ; IN B,(C) ; HALT
    let program = [0x37, 0x01, 0x7F, 0x00, 0xED, 0x40, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.b, 0xFF);
    assert_ne!(z80.f & super::FLAG_C, 0);
    assert_ne!(z80.f & super::FLAG_S, 0);
    assert_ne!(z80.f & super::FLAG_PV, 0);
    assert_eq!(z80.f & super::FLAG_Z, 0);
    assert_eq!(z80.f & super::FLAG_N, 0);
    assert_eq!(z80.f & super::FLAG_H, 0);
}

#[test]
fn index_prefixed_cp_memory_sets_compare_flags() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.ix = 0x0100;
    z80.a = 0x10;
    z80.write_ram_u8(0x0101, 0x01);

    // CP (IX+1) ; HALT
    let program = [0xDD, 0xBE, 0x01, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    // 0x10 - 0x01 => N set, H set, C clear, Z clear.
    assert_ne!(z80.f & super::FLAG_N, 0);
    assert_ne!(z80.f & super::FLAG_H, 0);
    assert_eq!(z80.f & super::FLAG_C, 0);
    assert_eq!(z80.f & super::FLAG_Z, 0);
}

#[test]
fn bit_ix_d_uses_effective_address_high_for_xy_flags() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.ix = 0x2810;
    z80.write_ram_u8(0x0815, 0x00);

    // SCF ; BIT 0,(IX+5) ; HALT
    let program = [0x37, 0xDD, 0xCB, 0x05, 0x46, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_ne!(z80.f & super::FLAG_C, 0);
    assert_ne!(z80.f & super::FLAG_H, 0);
    assert_ne!(z80.f & super::FLAG_Z, 0);
    assert_ne!(z80.f & super::FLAG_PV, 0);
    assert_eq!(
        z80.f & (super::FLAG_X | super::FLAG_Y),
        super::FLAG_X | super::FLAG_Y
    );
}

#[test]
fn index_high_low_register_ops_and_alu_are_implemented() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // LD IX,1234 ; LD IXH,20 ; LD IXL,05 ; LD A,IXH ; ADD A,IXL ; AND IXH ; OR IXL ; HALT
    let program = [
        0xDD, 0x21, 0x34, 0x12, 0xDD, 0x26, 0x20, 0xDD, 0x2E, 0x05, 0xDD, 0x7C, 0xDD, 0x85, 0xDD,
        0xA4, 0xDD, 0xB5, 0x76,
    ];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.ix, 0x2005);
    assert_eq!(z80.a, 0x25);
    assert_eq!(z80.pc, program.len() as u16);
}

#[test]
fn index_displacement_h_l_forms_keep_plain_h_l_registers() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    z80.write_ram_u8(0x0124, 0x11);
    z80.write_ram_u8(0x0125, 0x22);
    z80.write_ram_u8(0x0133, 0xAA);
    z80.write_ram_u8(0x0134, 0xBB);

    // LD H,99 ; LD L,88 ; LD IX,0120
    // LD (IX+2),H ; LD (IX+3),L ; LD H,(IX+4) ; LD L,(IX+5)
    // LD H,33 ; LD L,44 ; LD IY,0130
    // LD (IY+1),H ; LD (IY+2),L ; LD H,(IY+3) ; LD L,(IY+4) ; HALT
    let program = [
        0x26, 0x99, 0x2E, 0x88, 0xDD, 0x21, 0x20, 0x01, 0xDD, 0x74, 0x02, 0xDD, 0x75, 0x03, 0xDD,
        0x66, 0x04, 0xDD, 0x6E, 0x05, 0x26, 0x33, 0x2E, 0x44, 0xFD, 0x21, 0x30, 0x01, 0xFD, 0x74,
        0x01, 0xFD, 0x75, 0x02, 0xFD, 0x66, 0x03, 0xFD, 0x6E, 0x04, 0x76,
    ];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(2048, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);

    // DD/FD + LD (IX/IY+d),H/L and LD H/L,(IX/IY+d) keep plain H/L semantics.
    assert_eq!(z80.read_ram_u8(0x0122), 0x99);
    assert_eq!(z80.read_ram_u8(0x0123), 0x88);
    assert_eq!(z80.read_ram_u8(0x0131), 0x33);
    assert_eq!(z80.read_ram_u8(0x0132), 0x44);

    // Final H/L come from memory loads.
    assert_eq!(z80.h, 0xAA);
    assert_eq!(z80.l, 0xBB);
    // IX/IY themselves are unchanged by these H/L memory forms.
    assert_eq!(z80.ix, 0x0120);
    assert_eq!(z80.iy, 0x0130);
}

#[test]
fn index_cb_bit_uses_20_cycles_while_res_set_use_23() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.ix = 0x0100;
    z80.write_ram_u8(0x0101, 0x01);

    let mut bus = super::Z80Bus {
        audio: &mut audio,
        cartridge: &cart,
        work_ram: &mut work_ram,
        vdp: &mut vdp,
        io: &mut io,
    };

    // BIT 0,(IX+1)
    let c_bit = z80.exec_index_cb(true, 1, 0x46, &mut bus);
    assert_eq!(c_bit, 20);
    assert_eq!(z80.read_ram_u8(0x0101), 0x01);

    // RES 0,(IX+1)
    let c_res = z80.exec_index_cb(true, 1, 0x86, &mut bus);
    assert_eq!(c_res, 23);
    assert_eq!(z80.read_ram_u8(0x0101), 0x00);

    // SET 0,(IX+1)
    let c_set = z80.exec_index_cb(true, 1, 0xC6, &mut bus);
    assert_eq!(c_set, 23);
    assert_eq!(z80.read_ram_u8(0x0101), 0x01);
}

#[test]
fn add_ix_iy_rr_update_halfcarry_and_carry_flags() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.ix = 0xFFFF;
    z80.set_bc(0x0001);
    z80.f = super::FLAG_S | super::FLAG_Z | super::FLAG_PV | super::FLAG_N;

    // ADD IX,BC ; HALT
    let ix_program = [0xDD, 0x09, 0x76];
    for (i, byte) in ix_program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.ix, 0x0000);
    assert_ne!(z80.f & super::FLAG_H, 0);
    assert_ne!(z80.f & super::FLAG_C, 0);
    assert_eq!(z80.f & super::FLAG_N, 0);
    assert_ne!(z80.f & super::FLAG_S, 0);
    assert_ne!(z80.f & super::FLAG_Z, 0);
    assert_ne!(z80.f & super::FLAG_PV, 0);

    z80.write_reset_byte(0x00);
    z80.write_reset_byte(0x01);
    z80.iy = 0x0FFF;
    z80.sp = 0x0001;

    // ADD IY,SP ; HALT
    let iy_program = [0xFD, 0x39, 0x76];
    for (i, byte) in iy_program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.iy, 0x1000);
    assert_ne!(z80.f & super::FLAG_H, 0);
    assert_eq!(z80.f & super::FLAG_C, 0);
}

#[test]
fn index_prefixed_halt_is_supported() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    z80.write_ram_u8(0x0000, 0xDD);
    z80.write_ram_u8(0x0001, 0x76);

    z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert!(z80.halted);
    assert_eq!(z80.pc, 0x0002);
}

#[test]
fn ed_ld_bc_mem_and_ld_mem_bc_are_implemented() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.set_bc(0x1234);

    // LD (0x0100),BC ; LD BC,(0x0100) ; HALT
    let program = [0xED, 0x43, 0x00, 0x01, 0xED, 0x4B, 0x00, 0x01, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.read_ram_u8(0x0100), 0x34);
    assert_eq!(z80.read_ram_u8(0x0101), 0x12);
    assert_eq!(z80.bc(), 0x1234);
}

#[test]
fn adc_immediate_and_adc_indexed_are_implemented() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.ix = 0x0100;
    z80.a = 1;
    z80.write_ram_u8(0x0105, 2);

    // SBC A,0x00 ; ADC A,(IX+5) ; HALT
    let program = [0xDE, 0x00, 0xDD, 0x8E, 0x05, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.a, 3);
}

#[test]
fn ed_adc_sbc_hl_rr_are_implemented() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.set_hl(0x1000);
    z80.set_bc(0x0001);
    z80.set_de(0x0002);
    z80.sp = 0x0003;

    // ADC HL,BC ; ADC HL,DE ; ADC HL,SP ; SBC HL,BC ; HALT
    let program = [0xED, 0x4A, 0xED, 0x5A, 0xED, 0x7A, 0xED, 0x42, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.hl(), 0x1005);
}

#[test]
fn ed_ldd_and_lddr_are_implemented() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.set_hl(0x0102);
    z80.set_de(0x0202);
    z80.set_bc(0x0003);
    z80.write_ram_u8(0x0100, 0x11);
    z80.write_ram_u8(0x0101, 0x22);
    z80.write_ram_u8(0x0102, 0x33);

    // LDD ; LDDR ; HALT
    let program = [0xED, 0xA8, 0xED, 0xB8, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.read_ram_u8(0x0202), 0x33);
    assert_eq!(z80.read_ram_u8(0x0201), 0x22);
    assert_eq!(z80.read_ram_u8(0x0200), 0x11);
    assert_eq!(z80.bc(), 0x0000);
    assert_eq!(z80.hl(), 0x00FF);
    assert_eq!(z80.de(), 0x01FF);
}

#[test]
fn ed_ldi_updates_block_transfer_flags() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.a = 0x20;
    z80.f = super::FLAG_S | super::FLAG_Z | super::FLAG_C | super::FLAG_H | super::FLAG_N;
    z80.set_hl(0x0100);
    z80.set_de(0x0200);
    z80.set_bc(0x0002);
    z80.write_ram_u8(0x0100, 0x11);

    // LDI ; HALT
    let program = [0xED, 0xA0, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.read_ram_u8(0x0200), 0x11);
    assert_eq!(z80.bc(), 0x0001);
    assert_ne!(z80.f & super::FLAG_S, 0);
    assert_ne!(z80.f & super::FLAG_Z, 0);
    assert_ne!(z80.f & super::FLAG_C, 0);
    assert_ne!(z80.f & super::FLAG_PV, 0);
    assert_eq!(z80.f & (super::FLAG_H | super::FLAG_N), 0);
}

#[test]
fn ed_cpi_uses_bc_for_pv_and_preserves_carry() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.a = 0x10;
    z80.f = super::FLAG_C;
    z80.set_hl(0x0100);
    z80.set_bc(0x0002);
    z80.write_ram_u8(0x0100, 0x01);

    // CPI ; HALT
    let program = [0xED, 0xA1, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.hl(), 0x0101);
    assert_eq!(z80.bc(), 0x0001);
    assert_ne!(z80.f & super::FLAG_C, 0);
    assert_ne!(z80.f & super::FLAG_N, 0);
    assert_ne!(z80.f & super::FLAG_H, 0);
    assert_ne!(z80.f & super::FLAG_PV, 0);
    assert_eq!(z80.f & (super::FLAG_Z | super::FLAG_S), 0);
}

#[test]
fn ed_cpi_uses_a_minus_mem_minus_h_for_xy_flags() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.a = 0x30;
    z80.set_hl(0x0100);
    z80.set_bc(0x0002);
    z80.write_ram_u8(0x0100, 0x08);

    // CPI ; HALT
    let program = [0xED, 0xA1, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.hl(), 0x0101);
    assert_eq!(z80.bc(), 0x0001);
    assert_ne!(z80.f & super::FLAG_N, 0);
    assert_ne!(z80.f & super::FLAG_H, 0);
    assert_ne!(z80.f & super::FLAG_PV, 0);
    // result=0x28, H=1 => undocumented XY come from 0x27.
    assert_eq!(z80.f & (super::FLAG_X | super::FLAG_Y), super::FLAG_Y);
}

#[test]
fn ed_cpir_repeats_until_match() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.a = 0x22;
    z80.set_hl(0x0100);
    z80.set_bc(0x0003);
    z80.write_ram_u8(0x0100, 0x10);
    z80.write_ram_u8(0x0101, 0x22);
    z80.write_ram_u8(0x0102, 0x33);

    // CPIR ; HALT
    let program = [0xED, 0xB1, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.hl(), 0x0102);
    assert_eq!(z80.bc(), 0x0001);
    assert_ne!(z80.f & super::FLAG_Z, 0);
}

#[test]
fn ed_cpir_clears_pv_when_bc_reaches_zero_without_match() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.a = 0x7E;
    z80.f = super::FLAG_C;
    z80.set_hl(0x0100);
    z80.set_bc(0x0002);
    z80.write_ram_u8(0x0100, 0x10);
    z80.write_ram_u8(0x0101, 0x20);

    // CPIR ; HALT
    let program = [0xED, 0xB1, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(1024, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.hl(), 0x0102);
    assert_eq!(z80.bc(), 0x0000);
    assert_ne!(z80.f & super::FLAG_C, 0);
    assert_ne!(z80.f & super::FLAG_N, 0);
    assert_eq!(z80.f & super::FLAG_PV, 0);
    assert_eq!(z80.f & super::FLAG_Z, 0);
}

#[test]
fn ed_cpdr_repeats_until_match() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.a = 0x22;
    z80.set_hl(0x0102);
    z80.set_bc(0x0003);
    z80.write_ram_u8(0x0100, 0x10);
    z80.write_ram_u8(0x0101, 0x22);
    z80.write_ram_u8(0x0102, 0x33);

    // CPDR ; HALT
    let program = [0xED, 0xB9, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.hl(), 0x0100);
    assert_eq!(z80.bc(), 0x0001);
    assert_ne!(z80.f & super::FLAG_Z, 0);
}

#[test]
fn out_immediate_writes_psg_port() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // LD A,0x9A ; OUT (0x7F),A ; HALT
    let program = [0x3E, 0x9A, 0xD3, 0x7F, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(audio.psg().last_data(), 0x9A);
}

#[test]
fn out_immediate_writes_ym2612_via_port_io() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // LD A,0x22 ; OUT (0x40),A ; LD A,0x0F ; OUT (0x41),A ; HALT
    let program = [0x3E, 0x22, 0xD3, 0x40, 0x3E, 0x0F, 0xD3, 0x41, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(320, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(audio.ym2612().register(0, 0x22), 0x0F);
}

#[test]
fn ed_out_c_a_writes_ym2612_via_port_io() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // LD BC,0x0040 ; LD A,0x22 ; OUT (C),A ; INC C ; LD A,0x0F ; OUT (C),A ; HALT
    let program = [
        0x01, 0x40, 0x00, 0x3E, 0x22, 0xED, 0x79, 0x0C, 0x3E, 0x0F, 0xED, 0x79, 0x76,
    ];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(audio.ym2612().register(0, 0x22), 0x0F);
}

#[test]
fn in_immediate_reads_ym2612_status_via_port_io() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // Preload YM timer-A status bit so IN can verify YM status routing robustly.
    audio.write_ym2612(0, 0x24);
    audio.write_ym2612(1, 0xFF);
    audio.write_ym2612(0, 0x25);
    audio.write_ym2612(1, 0x03);
    audio.write_ym2612(0, 0x27);
    audio.write_ym2612(1, 0x05);
    audio.step_z80_cycles(80);
    assert_ne!(audio.read_ym2612(0) & 0x01, 0);

    // IN A,(0x40) ; HALT
    let program = [0xDB, 0x40, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_ne!(z80.a & 0x01, 0);
}

#[test]
fn ed_in_c_a_reads_ym2612_status_via_port_io() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // Preload YM timer-A status bit so IN can verify YM status routing robustly.
    audio.write_ym2612(0, 0x24);
    audio.write_ym2612(1, 0xFF);
    audio.write_ym2612(0, 0x25);
    audio.write_ym2612(1, 0x03);
    audio.write_ym2612(0, 0x27);
    audio.write_ym2612(1, 0x05);
    audio.step_z80_cycles(80);
    assert_ne!(audio.read_ym2612(0) & 0x01, 0);

    // LD BC,0x0040 ; IN A,(C) ; HALT
    let program = [0x01, 0x40, 0x00, 0xED, 0x78, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_ne!(z80.a & 0x01, 0);
}

#[test]
fn ed_otir_repeats_and_writes_psg_port() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.set_hl(0x0100);
    z80.set_bc(0x027F); // B=2, C=0x7F
    z80.write_ram_u8(0x0100, 0x9A);
    z80.write_ram_u8(0x0101, 0x9B);

    // OTIR ; HALT
    let program = [0xED, 0xB3, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(audio.psg().last_data(), 0x9B);
    assert_eq!(z80.b, 0);
    assert_eq!(z80.hl(), 0x0102);
}

#[test]
fn ed_inir_reads_port_into_memory_until_b_zero() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.set_hl(0x0100);
    z80.set_bc(0x0200); // B=2

    // INIR ; HALT
    let program = [0xED, 0xB2, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.read_ram_u8(0x0100), 0xFF);
    assert_eq!(z80.read_ram_u8(0x0101), 0xFF);
    assert_eq!(z80.b, 0);
    assert_eq!(z80.hl(), 0x0102);
}

#[test]
fn ed_ini_updates_block_io_flags() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.set_hl(0x0100);
    z80.set_bc(0x0100); // B=1, C=0

    // INI ; HALT
    let program = [0xED, 0xA2, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.read_ram_u8(0x0100), 0xFF);
    assert_eq!(z80.b, 0);
    assert_eq!(z80.hl(), 0x0101);
    assert_ne!(z80.f & super::FLAG_Z, 0);
    assert_ne!(z80.f & super::FLAG_N, 0);
    assert_ne!(z80.f & super::FLAG_H, 0);
    assert_ne!(z80.f & super::FLAG_C, 0);
    assert_ne!(z80.f & super::FLAG_PV, 0);
    assert_eq!(z80.f & super::FLAG_S, 0);
}

#[test]
fn ed_outd_updates_block_io_flags() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.set_hl(0x0081);
    z80.set_bc(0x017F); // B=1, C=0x7F (PSG port)
    z80.write_ram_u8(0x0081, 0x80);

    // OUTD ; HALT
    let program = [0xED, 0xAB, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(audio.psg().last_data(), 0x80);
    assert_eq!(z80.b, 0);
    assert_eq!(z80.hl(), 0x0080);
    assert_ne!(z80.f & super::FLAG_Z, 0);
    assert_ne!(z80.f & super::FLAG_N, 0);
    assert_eq!(z80.f & super::FLAG_H, 0);
    assert_eq!(z80.f & super::FLAG_C, 0);
    assert_ne!(z80.f & super::FLAG_PV, 0);
    assert_eq!(z80.f & super::FLAG_S, 0);
}

#[test]
fn ed_outi_uses_c_plus_one_for_hc_calculation() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.set_hl(0x1200);
    z80.set_bc(0x01FE); // B=1, C=0xFE
    z80.write_ram_u8(0x1200, 0x02);

    // OUTI ; HALT
    let program = [0xED, 0xA3, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.b, 0);
    assert_eq!(z80.hl(), 0x1201);
    // (C+1)=0xFF; 0xFF + 0x02 overflows -> H and C set.
    assert_ne!(z80.f & super::FLAG_H, 0);
    assert_ne!(z80.f & super::FLAG_C, 0);
}

#[test]
fn ed_outd_uses_c_minus_one_for_hc_calculation() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.set_hl(0x1201);
    z80.set_bc(0x0100); // B=1, C=0x00
    z80.write_ram_u8(0x1201, 0x80);

    // OUTD ; HALT
    let program = [0xED, 0xAB, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.b, 0);
    assert_eq!(z80.hl(), 0x1200);
    // (C-1)=0xFF; 0xFF + 0x80 overflows -> H and C set.
    assert_ne!(z80.f & super::FLAG_H, 0);
    assert_ne!(z80.f & super::FLAG_C, 0);
}

#[test]
fn ed_ini_wraps_c_plus_one_for_hc_calculation() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.set_hl(0x0100);
    z80.set_bc(0x01FF); // B=1, C=0xFF (port returns 0xFF default)

    // INI ; HALT
    let program = [0xED, 0xA2, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.read_ram_u8(0x0100), 0xFF);
    assert_eq!(z80.b, 0);
    // (C+1) wraps to 0x00; 0x00 + 0xFF does not set carry/half-carry in this rule.
    assert_eq!(z80.f & (super::FLAG_H | super::FLAG_C), 0);
}

#[test]
fn ed_ind_wraps_c_minus_one_for_hc_calculation() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.set_hl(0x0100);
    z80.set_bc(0x0100); // B=1, C=0x00 (port returns 0xFF default)

    // IND ; HALT
    let program = [0xED, 0xAA, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.read_ram_u8(0x0100), 0xFF);
    assert_eq!(z80.b, 0);
    // (C-1) wraps to 0xFF; 0xFF + 0xFF sets both H and C in block I/O rules.
    assert_ne!(z80.f & super::FLAG_H, 0);
    assert_ne!(z80.f & super::FLAG_C, 0);
}

#[test]
fn ed_rld_and_rrd_transform_nibbles_between_a_and_hl() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.set_hl(0x0100);
    z80.a = 0xAB;
    z80.write_ram_u8(0x0100, 0xCD);

    // RLD ; RRD ; HALT
    let program = [0xED, 0x6F, 0xED, 0x67, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.a, 0xAB);
    assert_eq!(z80.read_ram_u8(0x0100), 0xCD);
}

#[test]
fn ed_ld_i_r_and_ld_a_i_r_are_implemented() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.f = super::FLAG_C;

    // LD A,0xA5 ; LD I,A ; XOR A ; LD A,I ; LD A,0x80 ; LD R,A ; LD A,R ; HALT
    let program = [
        0x3E, 0xA5, 0xED, 0x47, 0xAF, 0xED, 0x57, 0x3E, 0x80, 0xED, 0x4F, 0xED, 0x5F, 0x76,
    ];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.i_reg, 0xA5);
    assert_eq!(z80.a, 0x82);
}

#[test]
fn ed_ld_a_i_and_ld_a_r_update_xy_and_control_bits() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // LD A,I ; HALT
    z80.i_reg = 0x28;
    z80.f = super::FLAG_C | super::FLAG_H | super::FLAG_N;
    z80.iff2 = false;
    z80.write_ram_u8(0x0000, 0xED);
    z80.write_ram_u8(0x0001, 0x57);
    z80.write_ram_u8(0x0002, 0x76);

    z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.a, 0x28);
    assert_ne!(z80.f & super::FLAG_C, 0);
    assert_eq!(z80.f & (super::FLAG_H | super::FLAG_N), 0);
    assert_eq!(z80.f & super::FLAG_PV, 0);
    assert_eq!(
        z80.f & (super::FLAG_X | super::FLAG_Y),
        super::FLAG_X | super::FLAG_Y
    );

    // LD A,R ; HALT
    z80.write_reset_byte(0x00);
    z80.write_reset_byte(0x01);
    z80.r_reg = 0x26;
    z80.f = super::FLAG_C;
    z80.iff2 = true;
    z80.write_ram_u8(0x0000, 0xED);
    z80.write_ram_u8(0x0001, 0x5F);
    z80.write_ram_u8(0x0002, 0x76);
    let expected = z80.r_reg.wrapping_add(2);

    z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.a, expected);
    assert_ne!(z80.f & super::FLAG_C, 0);
    assert_eq!(z80.f & (super::FLAG_H | super::FLAG_N), 0);
    assert_ne!(z80.f & super::FLAG_PV, 0);
    assert_eq!(
        z80.f & (super::FLAG_X | super::FLAG_Y),
        expected & (super::FLAG_X | super::FLAG_Y)
    );
}

#[test]
fn refresh_counter_advances_on_opcode_fetches() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // LD A,0x12 ; NOP ; HALT
    z80.write_ram_u8(0x0000, 0x3E);
    z80.write_ram_u8(0x0001, 0x12);
    z80.write_ram_u8(0x0002, 0x00);
    z80.write_ram_u8(0x0003, 0x76);

    // 33 M68k cycles -> 15 Z80 cycles (exactly enough for this sequence).
    z80.step(33, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.r_reg & 0x7F, 3);
}

#[test]
fn refresh_counter_preserves_high_bit_during_opcode_fetch() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.r_reg = 0x80;

    // NOP ; NOP ; HALT
    z80.write_ram_u8(0x0000, 0x00);
    z80.write_ram_u8(0x0001, 0x00);
    z80.write_ram_u8(0x0002, 0x76);

    // 27 M68k cycles -> 12 Z80 cycles (exactly enough for this sequence).
    z80.step(27, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.r_reg, 0x83);
}

#[test]
fn halt_repeats_m1_and_advances_refresh_counter_without_advancing_pc() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // HALT
    z80.write_ram_u8(0x0000, 0x76);
    z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

    assert_eq!(z80.unknown_opcode_total(), 0);
    assert!(z80.halted);
    assert_eq!(z80.pc, 0x0001);
    // 128 M68k cycles grant 59 Z80 cycles:
    // first HALT fetch increments R once, then HALT M1 repeats for remaining cycles.
    assert_eq!(z80.r_reg & 0x7F, 14);
}

#[test]
fn halt_refresh_counter_wraps_low_7_bits_and_preserves_high_bit() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.r_reg = 0xFE;

    // HALT
    z80.write_ram_u8(0x0000, 0x76);
    z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

    assert_eq!(z80.unknown_opcode_total(), 0);
    assert!(z80.halted);
    assert_eq!(z80.pc, 0x0001);
    // Low 7 bits wrap, bit7 is preserved.
    assert_eq!(z80.r_reg, 0x85);
}

#[test]
fn ed_undefined_opcode_behaves_as_nop_and_does_not_increment_unknown() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.write_ram_u8(0x0000, 0xED);
    z80.write_ram_u8(0x0001, 0xFF);
    z80.write_ram_u8(0x0002, 0x76);

    z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert!(z80.halted);
    assert_eq!(z80.pc, 0x0003);
}

#[test]
fn all_ed_prefixed_opcodes_do_not_increment_unknown_counter() {
    for op in 0u16..=0xFF {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // ED op ; HALT
        z80.write_ram_u8(0x0000, 0xED);
        z80.write_ram_u8(0x0001, op as u8);
        z80.write_ram_u8(0x0002, 0x76);

        z80.step(96, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(
            z80.unknown_opcode_total(),
            0,
            "ED sub-opcode {:02X} should not be unknown",
            op
        );
    }
}

#[test]
fn all_base_opcodes_do_not_increment_unknown_counter() {
    for op in 0u16..=0xFF {
        let mut z80 = Z80::new();
        let mut audio = AudioBus::new();
        let cart = dummy_cart();
        let mut work_ram = [0u8; 0x10000];
        let mut vdp = Vdp::new();
        let mut io = IoBus::new();
        z80.write_reset_byte(0x01);

        // opcode ; filler for immediate/displacement consumers ; HALT
        z80.write_ram_u8(0x0000, op as u8);
        z80.write_ram_u8(0x0001, 0x00);
        z80.write_ram_u8(0x0002, 0x00);
        z80.write_ram_u8(0x0003, 0x00);
        z80.write_ram_u8(0x0004, 0x76);

        z80.step(160, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
        assert_eq!(
            z80.unknown_opcode_total(),
            0,
            "base opcode {:02X} should not be unknown",
            op
        );
    }
}

#[test]
fn all_dd_fd_prefixed_second_bytes_do_not_increment_unknown_counter() {
    for &prefix in &[0xDDu8, 0xFDu8] {
        for op in 0u16..=0xFF {
            let mut z80 = Z80::new();
            let mut audio = AudioBus::new();
            let cart = dummy_cart();
            let mut work_ram = [0u8; 0x10000];
            let mut vdp = Vdp::new();
            let mut io = IoBus::new();
            z80.write_reset_byte(0x01);

            // prefix op d op3 ; HALT
            // The trailing bytes satisfy forms that need displacement/immediate
            // (including prefix CB d op3) while remaining harmless otherwise.
            z80.write_ram_u8(0x0000, prefix);
            z80.write_ram_u8(0x0001, op as u8);
            z80.write_ram_u8(0x0002, 0x00);
            z80.write_ram_u8(0x0003, 0x00);
            z80.write_ram_u8(0x0004, 0x76);

            z80.step(160, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
            assert_eq!(
                z80.unknown_opcode_total(),
                0,
                "{:02X} sub-opcode {:02X} should not be unknown",
                prefix,
                op
            );
        }
    }
}

#[test]
fn inc_sp_opcode_is_implemented() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.sp = 0x1234;

    // INC SP ; HALT
    z80.write_ram_u8(0x0000, 0x33);
    z80.write_ram_u8(0x0001, 0x76);

    z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.sp, 0x1235);
}

#[test]
fn dd_prefix_before_inc_sp_is_ignored_and_executes_inc_sp() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.sp = 0x00FF;

    // DD ; INC SP ; HALT
    z80.write_ram_u8(0x0000, 0xDD);
    z80.write_ram_u8(0x0001, 0x33);
    z80.write_ram_u8(0x0002, 0x76);

    z80.step(96, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.sp, 0x0100);
    assert_eq!(z80.pc, 0x0003);
}

#[test]
fn dd_prefix_is_ignored_for_non_indexed_opcode() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // DD ; NOP ; HALT
    z80.write_ram_u8(0x0000, 0xDD);
    z80.write_ram_u8(0x0001, 0x00);
    z80.write_ram_u8(0x0002, 0x76);

    // 27 M68k cycles -> 12 Z80 cycles (exactly enough for DD NOP HALT).
    z80.step(27, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.r_reg & 0x7F, 3);
}

#[test]
fn dd_prefix_before_ed_executes_ed_opcode_normally() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.a = 0x5A;

    // DD ; ED 47 (LD I,A) ; HALT
    z80.write_ram_u8(0x0000, 0xDD);
    z80.write_ram_u8(0x0001, 0xED);
    z80.write_ram_u8(0x0002, 0x47);
    z80.write_ram_u8(0x0003, 0x76);

    z80.step(192, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.i_reg, 0x5A);
}

#[test]
fn repeated_dd_prefix_uses_last_prefix_and_does_not_mark_unknown() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // DD ; DD ; LD IX,0x1234 ; HALT
    let program = [0xDD, 0xDD, 0x21, 0x34, 0x12, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.ix, 0x1234);
}

#[test]
fn ed_ld_mem_sp_and_ld_sp_mem_are_implemented() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.sp = 0xBEEF;

    // LD (0x1234),SP ; LD SP,(0x1234) ; HALT
    let program = [0xED, 0x73, 0x34, 0x12, 0xED, 0x7B, 0x34, 0x12, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.read_ram_u8(0x1234), 0xEF);
    assert_eq!(z80.read_ram_u8(0x1235), 0xBE);
    assert_eq!(z80.sp, 0xBEEF);
}

#[test]
fn ed_ld_mem_hl_and_ld_hl_mem_are_implemented() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.set_hl(0xCAFE);

    // LD (0x1234),HL ; LD HL,(0x1234) ; HALT
    let program = [0xED, 0x63, 0x34, 0x12, 0xED, 0x6B, 0x34, 0x12, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.read_ram_u8(0x1234), 0xFE);
    assert_eq!(z80.read_ram_u8(0x1235), 0xCA);
    assert_eq!(z80.hl(), 0xCAFE);
}

#[test]
fn ed_retn_alias_restores_iff_and_returns() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.iff2 = true;
    z80.sp = 0x0100;
    z80.write_ram_u8(0x0100, 0x34);
    z80.write_ram_u8(0x0101, 0x12);

    // RETN alias: ED 55 ; HALT (at return target)
    z80.write_ram_u8(0x0000, 0xED);
    z80.write_ram_u8(0x0001, 0x55);
    z80.write_ram_u8(0x1234, 0x76);

    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.pc, 0x1235);
    assert!(z80.iff1);
}

#[test]
fn ed_neg_opcode_is_implemented() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.a = 0x01;
    // NEG ; HALT
    z80.write_ram_u8(0x0000, 0xED);
    z80.write_ram_u8(0x0001, 0x44);
    z80.write_ram_u8(0x0002, 0x76);

    z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.a, 0xFF);
    assert_eq!(z80.f & super::FLAG_Z, 0);
    assert_ne!(z80.f & super::FLAG_S, 0);
    assert_ne!(z80.f & super::FLAG_C, 0);
}

#[test]
fn or_a_updates_flags_and_is_not_unknown() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    // xor a ; or a ; halt
    z80.write_ram_u8(0x0000, 0xAF);
    z80.write_ram_u8(0x0001, 0xB7);
    z80.write_ram_u8(0x0002, 0x76);

    z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.a, 0);
    assert_ne!(z80.f & super::FLAG_Z, 0);
    assert_eq!(z80.f & super::FLAG_C, 0);
}

#[test]
fn djnz_and_ret_nz_execute_control_flow() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // ld b,3
    z80.write_ram_u8(0x0000, 0x06);
    z80.write_ram_u8(0x0001, 0x03);
    // ld a,0
    z80.write_ram_u8(0x0002, 0x3E);
    z80.write_ram_u8(0x0003, 0x00);
    // add a,1
    z80.write_ram_u8(0x0004, 0xC6);
    z80.write_ram_u8(0x0005, 0x01);
    // djnz -4 (to add a,1)
    z80.write_ram_u8(0x0006, 0x10);
    z80.write_ram_u8(0x0007, 0xFC);
    // call 0x0010
    z80.write_ram_u8(0x0008, 0xCD);
    z80.write_ram_u8(0x0009, 0x10);
    z80.write_ram_u8(0x000A, 0x00);
    // halt
    z80.write_ram_u8(0x000B, 0x76);
    // subroutine @0x0010: or a ; ret nz
    z80.write_ram_u8(0x0010, 0xB7);
    z80.write_ram_u8(0x0011, 0xC0);
    // halt (should not reach)
    z80.write_ram_u8(0x0012, 0x76);

    z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.a, 3);
    assert_eq!(z80.pc, 0x000C);
}

#[test]
fn conditional_call_c_and_ret_c_execute_control_flow() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // ld a,0 ; sub 1 ; call c,0x0010 ; halt
    z80.write_ram_u8(0x0000, 0x3E);
    z80.write_ram_u8(0x0001, 0x00);
    z80.write_ram_u8(0x0002, 0xD6);
    z80.write_ram_u8(0x0003, 0x01);
    z80.write_ram_u8(0x0004, 0xDC);
    z80.write_ram_u8(0x0005, 0x10);
    z80.write_ram_u8(0x0006, 0x00);
    z80.write_ram_u8(0x0007, 0x76);

    // subroutine @0x0010: ld b,0x42 ; ret c
    z80.write_ram_u8(0x0010, 0x06);
    z80.write_ram_u8(0x0011, 0x42);
    z80.write_ram_u8(0x0012, 0xD8);
    z80.write_ram_u8(0x0013, 0x76);

    z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.b, 0x42);
    assert_eq!(z80.pc, 0x0008);
}

#[test]
fn conditional_call_nc_and_call_p_are_implemented() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // xor a ; call nc,0x0010 ; call p,0x0020 ; halt
    z80.write_ram_u8(0x0000, 0xAF);
    z80.write_ram_u8(0x0001, 0xD4);
    z80.write_ram_u8(0x0002, 0x10);
    z80.write_ram_u8(0x0003, 0x00);
    z80.write_ram_u8(0x0004, 0xF4);
    z80.write_ram_u8(0x0005, 0x20);
    z80.write_ram_u8(0x0006, 0x00);
    z80.write_ram_u8(0x0007, 0x76);

    // @0x0010: ld b,0x11 ; ret
    z80.write_ram_u8(0x0010, 0x06);
    z80.write_ram_u8(0x0011, 0x11);
    z80.write_ram_u8(0x0012, 0xC9);
    // @0x0020: ld c,0x22 ; ret
    z80.write_ram_u8(0x0020, 0x0E);
    z80.write_ram_u8(0x0021, 0x22);
    z80.write_ram_u8(0x0022, 0xC9);

    z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.b, 0x11);
    assert_eq!(z80.c, 0x22);
    assert_eq!(z80.pc, 0x0008);
}

#[test]
fn parity_condition_jp_call_and_ret_are_implemented() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // Stack seed for POP AF (A=0x00, F=0x04 => PV=1).
    z80.sp = 0x0100;
    z80.write_ram_u8(0x0100, super::FLAG_PV);
    z80.write_ram_u8(0x0101, 0x00);

    // pop af ; call po,0x0030 ; call pe,0x0040 ; jp po,0x0010 ; jp pe,0x0020 ; halt
    let program = [
        0xF1, 0xE4, 0x30, 0x00, 0xEC, 0x40, 0x00, 0xE2, 0x10, 0x00, 0xEA, 0x20, 0x00, 0x76,
    ];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    // Should be skipped (JP PO / CALL PO not taken with PV=1).
    z80.write_ram_u8(0x0010, 0x76);
    z80.write_ram_u8(0x0030, 0x76);

    // JP PE target: execute payload and halt.
    z80.write_ram_u8(0x0020, 0x0E); // LD C,0x22
    z80.write_ram_u8(0x0021, 0x22);
    z80.write_ram_u8(0x0022, 0x76);

    // Subroutine @0x0040: RET PO (not taken) ; LD B,0x44 ; RET PE (taken)
    z80.write_ram_u8(0x0040, 0xE0);
    z80.write_ram_u8(0x0041, 0x06);
    z80.write_ram_u8(0x0042, 0x44);
    z80.write_ram_u8(0x0043, 0xE8);
    z80.write_ram_u8(0x0044, 0x76);

    z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.b, 0x44);
    assert_eq!(z80.c, 0x22);
    assert_eq!(z80.pc, 0x0023);
}

#[test]
fn add_and_sub_set_overflow_parity_flag() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // LD A,0x7F ; ADD A,0x01 ; HALT
    let add_prog = [0x3E, 0x7F, 0xC6, 0x01, 0x76];
    for (i, byte) in add_prog.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.a, 0x80);
    assert_ne!(z80.f & super::FLAG_PV, 0);
    assert_ne!(z80.f & super::FLAG_S, 0);
    assert_eq!(z80.f & super::FLAG_N, 0);

    z80.write_reset_byte(0x00);
    z80.write_reset_byte(0x01);
    // LD A,0x80 ; SUB 0x01 ; HALT
    let sub_prog = [0x3E, 0x80, 0xD6, 0x01, 0x76];
    for (i, byte) in sub_prog.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.a, 0x7F);
    assert_ne!(z80.f & super::FLAG_PV, 0);
    assert_ne!(z80.f & super::FLAG_N, 0);
    assert_eq!(z80.f & super::FLAG_S, 0);
}

#[test]
fn adc_and_sbc_with_carry_in_have_correct_overflow_flag() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // SCF ; LD A,0x00 ; ADC A,0x7F ; HALT
    // 0 + 127 + 1 = -128 (signed overflow set)
    let adc_prog = [0x37, 0x3E, 0x00, 0xCE, 0x7F, 0x76];
    for (i, byte) in adc_prog.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.a, 0x80);
    assert_ne!(z80.f & super::FLAG_PV, 0);
    assert_ne!(z80.f & super::FLAG_S, 0);

    z80.write_reset_byte(0x00);
    z80.write_reset_byte(0x01);

    // SCF ; LD A,0x00 ; SBC A,0x7F ; HALT
    // 0 - 127 - 1 = -128 (signed overflow clear)
    let sbc_prog = [0x37, 0x3E, 0x00, 0xDE, 0x7F, 0x76];
    for (i, byte) in sbc_prog.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.a, 0x80);
    assert_eq!(z80.f & super::FLAG_PV, 0);
    assert_ne!(z80.f & super::FLAG_S, 0);
}

#[test]
fn and_sets_halfcarry_and_parity_flags() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // LD A,0xF0 ; AND 0x0F ; HALT
    let prog = [0x3E, 0xF0, 0xE6, 0x0F, 0x76];
    for (i, byte) in prog.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.a, 0x00);
    assert_ne!(z80.f & super::FLAG_Z, 0);
    assert_ne!(z80.f & super::FLAG_H, 0);
    assert_ne!(z80.f & super::FLAG_PV, 0);
    assert_eq!(z80.f & (super::FLAG_N | super::FLAG_C), 0);
}

#[test]
fn inc_dec_and_bit_update_pv_related_flags() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // SCF ; LD B,0x7F ; INC B ; HALT
    let inc_prog = [0x37, 0x06, 0x7F, 0x04, 0x76];
    for (i, byte) in inc_prog.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.b, 0x80);
    assert_ne!(z80.f & super::FLAG_C, 0);
    assert_ne!(z80.f & super::FLAG_H, 0);
    assert_ne!(z80.f & super::FLAG_PV, 0);
    assert_eq!(z80.f & super::FLAG_N, 0);

    z80.write_reset_byte(0x00);
    z80.write_reset_byte(0x01);
    // SCF ; LD C,0x80 ; DEC C ; HALT
    let dec_prog = [0x37, 0x0E, 0x80, 0x0D, 0x76];
    for (i, byte) in dec_prog.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.c, 0x7F);
    assert_ne!(z80.f & super::FLAG_C, 0);
    assert_ne!(z80.f & super::FLAG_H, 0);
    assert_ne!(z80.f & super::FLAG_PV, 0);
    assert_ne!(z80.f & super::FLAG_N, 0);

    z80.write_reset_byte(0x00);
    z80.write_reset_byte(0x01);
    // SCF ; LD B,0x00 ; BIT 0,B ; HALT
    let bit_prog = [0x37, 0x06, 0x00, 0xCB, 0x40, 0x76];
    for (i, byte) in bit_prog.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_ne!(z80.f & super::FLAG_C, 0);
    assert_ne!(z80.f & super::FLAG_H, 0);
    assert_ne!(z80.f & super::FLAG_Z, 0);
    assert_ne!(z80.f & super::FLAG_PV, 0);
    assert_eq!(z80.f & super::FLAG_N, 0);
}

#[test]
fn bit_hl_uses_h_for_xy_flags() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.set_hl(0x2810);
    z80.write_ram_u8(0x0810, 0x00);

    // SCF ; BIT 0,(HL) ; HALT
    let prog = [0x37, 0xCB, 0x46, 0x76];
    for (i, byte) in prog.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_ne!(z80.f & super::FLAG_C, 0);
    assert_ne!(z80.f & super::FLAG_H, 0);
    assert_ne!(z80.f & super::FLAG_Z, 0);
    assert_ne!(z80.f & super::FLAG_PV, 0);
    assert_eq!(
        z80.f & (super::FLAG_X | super::FLAG_Y),
        super::FLAG_X | super::FLAG_Y
    );
}

#[test]
fn undocumented_xy_flags_follow_alu_and_bit_results() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // LD A,0 ; OR 0x28 ; HALT
    let or_prog = [0x3E, 0x00, 0xF6, 0x28, 0x76];
    for (i, byte) in or_prog.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.a, 0x28);
    assert_eq!(
        z80.f & (super::FLAG_X | super::FLAG_Y),
        super::FLAG_X | super::FLAG_Y
    );

    z80.write_reset_byte(0x00);
    z80.write_reset_byte(0x01);

    // SCF ; LD B,0x28 ; BIT 0,B ; HALT
    let bit_prog = [0x37, 0x06, 0x28, 0xCB, 0x40, 0x76];
    for (i, byte) in bit_prog.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_ne!(z80.f & super::FLAG_C, 0);
    assert_ne!(z80.f & super::FLAG_H, 0);
    assert_ne!(z80.f & super::FLAG_Z, 0);
    assert_ne!(z80.f & super::FLAG_PV, 0);
    assert_eq!(
        z80.f & (super::FLAG_X | super::FLAG_Y),
        super::FLAG_X | super::FLAG_Y
    );
}

#[test]
fn add_hl_sets_xy_from_result_high_byte() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // LD HL,0x1000 ; LD BC,0x1800 ; ADD HL,BC ; HALT  => HL=0x2800 (high=0x28)
    let prog = [0x21, 0x00, 0x10, 0x01, 0x00, 0x18, 0x09, 0x76];
    for (i, byte) in prog.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.hl(), 0x2800);
    assert_eq!(
        z80.f & (super::FLAG_X | super::FLAG_Y),
        super::FLAG_X | super::FLAG_Y
    );
}

#[test]
fn add_hl_preserves_szpv_and_sets_halfcarry() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // XOR A ; LD HL,0x0FFF ; LD BC,0x0001 ; ADD HL,BC ; HALT
    let prog = [0xAF, 0x21, 0xFF, 0x0F, 0x01, 0x01, 0x00, 0x09, 0x76];
    for (i, byte) in prog.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

    assert_eq!(z80.hl(), 0x1000);
    assert_ne!(z80.f & super::FLAG_Z, 0);
    assert_ne!(z80.f & super::FLAG_PV, 0);
    assert_ne!(z80.f & super::FLAG_H, 0);
    assert_eq!(z80.f & super::FLAG_N, 0);
    assert_eq!(z80.f & super::FLAG_C, 0);
}

#[test]
fn adc_hl_and_sbc_hl_set_extended_flags() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // SCF ; LD HL,0x7FFF ; LD BC,0 ; ADC HL,BC ; HALT
    let adc_prog = [0x37, 0x21, 0xFF, 0x7F, 0x01, 0x00, 0x00, 0xED, 0x4A, 0x76];
    for (i, byte) in adc_prog.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.hl(), 0x8000);
    assert_ne!(z80.f & super::FLAG_S, 0);
    assert_ne!(z80.f & super::FLAG_PV, 0);
    assert_ne!(z80.f & super::FLAG_H, 0);
    assert_eq!(z80.f & super::FLAG_N, 0);
    assert_eq!(z80.f & super::FLAG_C, 0);

    z80.write_reset_byte(0x00);
    z80.write_reset_byte(0x01);
    // SCF ; LD HL,0x8000 ; LD BC,0 ; SBC HL,BC ; HALT
    let sbc_prog = [0x37, 0x21, 0x00, 0x80, 0x01, 0x00, 0x00, 0xED, 0x42, 0x76];
    for (i, byte) in sbc_prog.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.hl(), 0x7FFF);
    assert_eq!(z80.f & super::FLAG_S, 0);
    assert_ne!(z80.f & super::FLAG_PV, 0);
    assert_ne!(z80.f & super::FLAG_H, 0);
    assert_ne!(z80.f & super::FLAG_N, 0);
    assert_eq!(z80.f & super::FLAG_C, 0);
}

#[test]
fn rotate_a_instructions_preserve_pv() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // XOR A (PV=1, Z=1) ; LD A,0x80 ; RLCA ; HALT
    let prog = [0xAF, 0x3E, 0x80, 0x07, 0x76];
    for (i, byte) in prog.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.a, 0x01);
    assert_ne!(z80.f & super::FLAG_C, 0);
    assert_ne!(z80.f & super::FLAG_PV, 0);
    assert_ne!(z80.f & super::FLAG_Z, 0);
}

#[test]
fn misc_flag_and_sp_ops_are_implemented() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // LD HL,0x1234 ; LD SP,HL ; LD A,0x09 ; ADD A,0x01 ; DAA ; SCF ; CCF ; HALT
    let program = [
        0x21, 0x34, 0x12, 0xF9, 0x3E, 0x09, 0xC6, 0x01, 0x27, 0x37, 0x3F, 0x76,
    ];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.sp, 0x1234);
    assert_eq!(z80.a, 0x10);
    assert_eq!(z80.f & super::FLAG_C, 0);
}

#[test]
fn daa_handles_subtraction_path() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // LD A,0x10 ; SUB 0x01 ; DAA ; HALT  => 0x09
    let program = [0x3E, 0x10, 0xD6, 0x01, 0x27, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.a, 0x09);
    assert_eq!(z80.f & super::FLAG_C, 0);
    assert_ne!(z80.f & super::FLAG_PV, 0);

    // LD A,0x00 ; SUB 0x01 ; DAA ; HALT => 0x99 with carry.
    z80.write_reset_byte(0x00);
    z80.write_reset_byte(0x01);
    let program = [0x3E, 0x00, 0xD6, 0x01, 0x27, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.a, 0x99);
    assert_ne!(z80.f & super::FLAG_C, 0);
    assert_ne!(z80.f & super::FLAG_PV, 0);
}

#[test]
fn daa_handles_addition_carry_and_xy_cases() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // 0x15 + 0x27 = 0x42 (BCD, no carry)
    let program = [0x3E, 0x15, 0xC6, 0x27, 0x27, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.a, 0x42);
    assert_eq!(z80.f & super::FLAG_C, 0);
    assert_eq!(z80.f & super::FLAG_N, 0);
    assert_eq!(z80.f & (super::FLAG_X | super::FLAG_Y), 0x00);

    // 0x99 + 0x01 = 0x00 with decimal carry.
    z80.write_reset_byte(0x00);
    z80.write_reset_byte(0x01);
    let program = [0x3E, 0x99, 0xC6, 0x01, 0x27, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.a, 0x00);
    assert_ne!(z80.f & super::FLAG_C, 0);
    assert_ne!(z80.f & super::FLAG_Z, 0);
    assert_ne!(z80.f & super::FLAG_PV, 0);
    assert_eq!(z80.f & (super::FLAG_X | super::FLAG_Y), 0x00);

    // 0x08 + 0x08 = 0x16: verifies H-driven adjust path.
    z80.write_reset_byte(0x00);
    z80.write_reset_byte(0x01);
    let program = [0x3E, 0x08, 0xC6, 0x08, 0x27, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.a, 0x16);
    assert_eq!(z80.f & super::FLAG_C, 0);
    assert_eq!(z80.f & super::FLAG_N, 0);
}

#[test]
fn index_ex_sp_and_jp_index_are_implemented() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.ix = 0x0200;
    z80.sp = 0x0100;
    z80.write_ram_u8(0x0100, 0x34);
    z80.write_ram_u8(0x0101, 0x12);

    // EX (SP),IX ; JP (IX)
    z80.write_ram_u8(0x0000, 0xDD);
    z80.write_ram_u8(0x0001, 0xE3);
    z80.write_ram_u8(0x0002, 0xDD);
    z80.write_ram_u8(0x0003, 0xE9);
    z80.write_ram_u8(0x1234, 0x76); // HALT at jump target

    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.ix, 0x1234);
    assert_eq!(z80.read_ram_u8(0x0100), 0x00);
    assert_eq!(z80.read_ram_u8(0x0101), 0x02);
    assert_eq!(z80.pc, 0x1235);
}

#[test]
fn pop_af_restores_accumulator_and_flags() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // Seed stack with AF value 0xAA45 and execute POP AF.
    z80.sp = 0x0100;
    z80.write_ram_u8(0x0100, 0x45);
    z80.write_ram_u8(0x0101, 0xAA);
    z80.write_ram_u8(0x0000, 0xF1);
    z80.write_ram_u8(0x0001, 0x76);

    z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.a, 0xAA);
    assert_eq!(
        z80.f & (super::FLAG_S | super::FLAG_Z | super::FLAG_PV | super::FLAG_C),
        0x45
    );
    assert_eq!(z80.sp, 0x0102);
}

#[test]
fn push_pop_bc_and_conditional_call_nz() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // ld bc,0x1234 ; push bc ; ld bc,0 ; pop bc ; call nz,0x0010 ; halt
    // 0x0010: and 0x0F ; sub 0x01 ; ret
    let program = [
        0x01, 0x34, 0x12, 0xC5, 0x01, 0x00, 0x00, 0xC1, 0xC4, 0x10, 0x00, 0x76, 0x00, 0x00, 0x00,
        0x00, 0xE6, 0x0F, 0xD6, 0x01, 0xC9,
    ];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }
    z80.a = 0x3C;
    z80.f = 0; // NZ true

    z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.bc(), 0x1234);
    assert_eq!(z80.a, 0x0B);
}

#[test]
fn bank_window_reads_from_68k_rom_space() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let mut rom = vec![0u8; 0x200];
    rom[0x0000] = 0xAB;
    let cart = Cartridge::from_bytes(rom).expect("valid cart");
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // ld a,(0x8000) ; halt
    z80.write_ram_u8(0x0000, 0x3A);
    z80.write_ram_u8(0x0001, 0x00);
    z80.write_ram_u8(0x0002, 0x80);
    z80.write_ram_u8(0x0003, 0x76);

    z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.a, 0xAB);
}

#[test]
fn bank_window_writes_to_68k_work_ram_space() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.bank_address = 0x00FF_0000;

    // ld a,0x5A ; ld (0x8000),a ; halt
    z80.write_ram_u8(0x0000, 0x3E);
    z80.write_ram_u8(0x0001, 0x5A);
    z80.write_ram_u8(0x0002, 0x32);
    z80.write_ram_u8(0x0003, 0x00);
    z80.write_ram_u8(0x0004, 0x80);
    z80.write_ram_u8(0x0005, 0x76);

    z80.step(160, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(work_ram[0], 0x5A);
}

#[test]
fn bank_window_reads_io_version_register() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.bank_address = 0x00A1_0000;

    // ld a,(0x8000) ; halt
    z80.write_ram_u8(0x0000, 0x3A);
    z80.write_ram_u8(0x0001, 0x00);
    z80.write_ram_u8(0x0002, 0x80);
    z80.write_ram_u8(0x0003, 0x76);

    z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.a, 0x20);
}

#[test]
fn bank_window_reads_vdp_hv_counter_bytes() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.bank_address = 0x00C0_0000;
    let expected = vdp.read_hv_counter();

    // ld a,(0x8008) ; ld b,a ; ld a,(0x8009) ; halt
    let program = [0x3A, 0x08, 0x80, 0x47, 0x3A, 0x09, 0x80, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(224, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.b, (expected >> 8) as u8);
    assert_eq!(z80.a, expected as u8);
}

#[test]
fn bank_window_reads_vdp_control_status_bytes() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.bank_address = 0x00C0_0000;
    let expected = vdp.read_control_port();

    // ld a,(0x8004) ; ld b,a ; ld a,(0x8005) ; halt
    let program = [0x3A, 0x04, 0x80, 0x47, 0x3A, 0x05, 0x80, 0x76];
    for (i, byte) in program.iter().enumerate() {
        z80.write_ram_u8(i as u16, *byte);
    }

    z80.step(224, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.b, (expected >> 8) as u8);
    assert_eq!(z80.a, expected as u8);
}

#[test]
fn bank_window_control_write_executes_pending_vdp_bus_dma() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.bank_address = 0x00C0_0000;
    work_ram[0] = 0x12;
    work_ram[1] = 0x34;

    let mut bus = super::Z80Bus {
        audio: &mut audio,
        cartridge: &cart,
        work_ram: &mut work_ram,
        vdp: &mut vdp,
        io: &mut io,
    };
    let mut write_control_word = |word: u16| {
        z80.write_68k_window(0x8004, (word >> 8) as u8, &mut bus);
        z80.write_68k_window(0x8005, word as u8, &mut bus);
    };

    // Enable DMA and setup one-word 68k-bus DMA from 0xFF0000 to VRAM 0x0000.
    write_control_word(0x8150);
    write_control_word(0x8F02);
    write_control_word(0x9301);
    write_control_word(0x9400);
    write_control_word(0x9500);
    write_control_word(0x9680);
    write_control_word(0x977F);

    // Set VRAM write command with DMA request bit.
    write_control_word(0x4000);
    write_control_word(0x0080);

    assert_eq!(bus.vdp.read_vram_u8(0x0000), 0x12);
    assert_eq!(bus.vdp.read_vram_u8(0x0001), 0x34);
}

#[test]
fn bank_window_vdp_data_byte_pair_commits_single_word() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.bank_address = 0x00C0_0000;

    let mut bus = super::Z80Bus {
        audio: &mut audio,
        cartridge: &cart,
        work_ram: &mut work_ram,
        vdp: &mut vdp,
        io: &mut io,
    };
    let mut write_control_word = |word: u16| {
        z80.write_68k_window(0x8004, (word >> 8) as u8, &mut bus);
        z80.write_68k_window(0x8005, word as u8, &mut bus);
    };

    // VRAM write at address 0.
    write_control_word(0x4000);
    write_control_word(0x0000);

    // Write one 16-bit data word through byte path.
    z80.write_68k_window(0x8000, 0x12, &mut bus);
    z80.write_68k_window(0x8001, 0x34, &mut bus);

    assert_eq!(bus.vdp.read_vram_u8(0x0000), 0x12);
    assert_eq!(bus.vdp.read_vram_u8(0x0001), 0x34);
    assert_eq!(bus.vdp.read_vram_u8(0x0002), 0x00);
    assert_eq!(bus.vdp.read_vram_u8(0x0003), 0x00);
}

#[test]
fn bank_window_can_write_psg_through_68k_bus_address() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.bank_address = 0x00C0_0000;

    // ld a,0x9A ; ld (0x8011),a ; halt
    z80.write_ram_u8(0x0000, 0x3E);
    z80.write_ram_u8(0x0001, 0x9A);
    z80.write_ram_u8(0x0002, 0x32);
    z80.write_ram_u8(0x0003, 0x11);
    z80.write_ram_u8(0x0004, 0x80);
    z80.write_ram_u8(0x0005, 0x76);

    z80.step(160, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(audio.psg().last_data(), 0x9A);
}

#[test]
fn bank_window_can_write_psg_through_68k_bus_mirror_addresses() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.bank_address = 0x00C0_0000;

    // ld a,0x9B ; ld (0x8013),a ; halt
    z80.write_ram_u8(0x0000, 0x3E);
    z80.write_ram_u8(0x0001, 0x9B);
    z80.write_ram_u8(0x0002, 0x32);
    z80.write_ram_u8(0x0003, 0x13);
    z80.write_ram_u8(0x0004, 0x80);
    z80.write_ram_u8(0x0005, 0x76);

    z80.step(160, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(audio.psg().last_data(), 0x9B);

    // Same via Dxxxxx mirror region.
    z80 = Z80::new();
    audio = AudioBus::new();
    z80.write_reset_byte(0x01);
    z80.bank_address = 0x00D0_0000;
    z80.write_ram_u8(0x0000, 0x3E);
    z80.write_ram_u8(0x0001, 0x9C);
    z80.write_ram_u8(0x0002, 0x32);
    z80.write_ram_u8(0x0003, 0x11);
    z80.write_ram_u8(0x0004, 0x80);
    z80.write_ram_u8(0x0005, 0x76);

    z80.step(160, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(audio.psg().last_data(), 0x9C);
}

#[test]
fn bank_register_uses_serial_bit_latch() {
    let mut z80 = Z80::new();
    for _ in 0..8 {
        z80.write_bank_register(1);
    }
    assert_eq!(z80.bank_address, 0x00FF_0000);
}

#[test]
fn maskable_interrupt_acknowledge_increments_refresh_counter() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.r_reg = 0xFF;
    z80.iff1 = true;
    z80.request_interrupt();

    // 28 M68k cycles grant exactly 13 Z80 cycles (IRQ acknowledge only).
    z80.step(28, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.pc, 0x0038);
    // R increments on acknowledge M1, low 7 bits wrap and bit7 stays unchanged.
    assert_eq!(z80.r_reg, 0x80);
}

#[test]
fn nmi_acknowledge_increments_refresh_counter() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);
    z80.r_reg = 0x7F;
    z80.iff1 = true;
    z80.request_nmi();

    // 24 M68k cycles grant exactly 11 Z80 cycles (NMI acknowledge only).
    z80.step(24, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.pc, 0x0066);
    // R increments on acknowledge M1 and wraps modulo 128.
    assert_eq!(z80.r_reg, 0x00);
}

#[test]
fn interrupt_requests_vector_to_0038_and_reti_returns() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // ei ; halt
    z80.write_ram_u8(0x0000, 0xFB);
    z80.write_ram_u8(0x0001, 0x76);
    z80.write_ram_u8(0x0002, 0x76);
    // IRQ vector @0x0038: ld a,0x42 ; reti
    z80.write_ram_u8(0x0038, 0x3E);
    z80.write_ram_u8(0x0039, 0x42);
    z80.write_ram_u8(0x003A, 0xED);
    z80.write_ram_u8(0x003B, 0x4D);

    z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.pc, 0x0002);

    z80.request_interrupt();
    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.a, 0x42);
    assert_eq!(z80.pc, 0x0003);
}

#[test]
fn ei_defers_maskable_irq_until_after_next_instruction() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // ei ; ld a,0x11 ; halt ; halt
    z80.write_ram_u8(0x0000, 0xFB);
    z80.write_ram_u8(0x0001, 0x3E);
    z80.write_ram_u8(0x0002, 0x11);
    z80.write_ram_u8(0x0003, 0x76);
    z80.write_ram_u8(0x0004, 0x76);

    // IRQ vector @0x0038: ld a,0x22 ; reti
    z80.write_ram_u8(0x0038, 0x3E);
    z80.write_ram_u8(0x0039, 0x22);
    z80.write_ram_u8(0x003A, 0xED);
    z80.write_ram_u8(0x003B, 0x4D);

    z80.request_interrupt();
    z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

    // If IRQ were taken immediately after EI, LD A,0x11 would run after RETI.
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.a, 0x22);
    assert_eq!(z80.pc, 0x0004);
}

#[test]
fn nmi_vectors_to_0066_even_when_maskable_irqs_are_disabled() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // di ; halt ; halt
    z80.write_ram_u8(0x0000, 0xF3);
    z80.write_ram_u8(0x0001, 0x76);
    z80.write_ram_u8(0x0002, 0x76);

    // NMI vector @0x0066: ld a,0x5A ; retn
    z80.write_ram_u8(0x0066, 0x3E);
    z80.write_ram_u8(0x0067, 0x5A);
    z80.write_ram_u8(0x0068, 0xED);
    z80.write_ram_u8(0x0069, 0x45);

    z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert!(z80.halted);
    assert_eq!(z80.pc, 0x0002);

    z80.request_nmi();
    z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.a, 0x5A);
    assert_eq!(z80.pc, 0x0003);
    assert!(z80.halted);
}

#[test]
fn nmi_latches_previous_iff1_into_iff2_and_retn_restores_it() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // halt ; halt
    z80.write_ram_u8(0x0000, 0x76);
    z80.write_ram_u8(0x0001, 0x76);

    // NMI handler @0x0066: retn
    z80.write_ram_u8(0x0066, 0xED);
    z80.write_ram_u8(0x0067, 0x45);

    // Set up a state where only IFF1 is enabled so NMI must copy it to IFF2.
    z80.iff1 = true;
    z80.iff2 = false;

    z80.step(64, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert!(z80.halted);
    assert_eq!(z80.pc, 0x0001);

    z80.request_nmi();
    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

    assert_eq!(z80.unknown_opcode_total(), 0);
    assert!(z80.iff1);
    assert!(z80.iff2);
    assert_eq!(z80.pc, 0x0002);
    assert!(z80.halted);
}

#[test]
fn im_opcodes_update_interrupt_mode_state() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // im 1 ; im 2 ; im 0 ; halt
    z80.write_ram_u8(0x0000, 0xED);
    z80.write_ram_u8(0x0001, 0x56);
    z80.write_ram_u8(0x0002, 0xED);
    z80.write_ram_u8(0x0003, 0x5E);
    z80.write_ram_u8(0x0004, 0xED);
    z80.write_ram_u8(0x0005, 0x46);
    z80.write_ram_u8(0x0006, 0x76);

    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.interrupt_mode, 0);
    assert_eq!(z80.unknown_opcode_total(), 0);
}

#[test]
fn interrupt_mode_0_uses_configured_rst_vector_opcode() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // im 0 ; ei ; halt ; halt
    z80.write_ram_u8(0x0000, 0xED);
    z80.write_ram_u8(0x0001, 0x46);
    z80.write_ram_u8(0x0002, 0xFB);
    z80.write_ram_u8(0x0003, 0x76);
    z80.write_ram_u8(0x0004, 0x76);

    // Handler @0x0028: ld a,0x66 ; reti
    z80.write_ram_u8(0x0028, 0x3E);
    z80.write_ram_u8(0x0029, 0x66);
    z80.write_ram_u8(0x002A, 0xED);
    z80.write_ram_u8(0x002B, 0x4D);

    z80.set_im0_interrupt_opcode(0xEF); // RST 28h
    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.interrupt_mode, 0);
    assert_eq!(z80.pc, 0x0004);

    z80.request_interrupt();
    z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.a, 0x66);
    assert_eq!(z80.pc, 0x0005);
}

#[test]
fn interrupt_mode_0_falls_back_to_rst38_for_non_rst_opcode() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // im 0 ; ei ; halt ; halt
    z80.write_ram_u8(0x0000, 0xED);
    z80.write_ram_u8(0x0001, 0x46);
    z80.write_ram_u8(0x0002, 0xFB);
    z80.write_ram_u8(0x0003, 0x76);
    z80.write_ram_u8(0x0004, 0x76);

    // Fallback handler @0x0038: ld a,0x44 ; reti
    z80.write_ram_u8(0x0038, 0x3E);
    z80.write_ram_u8(0x0039, 0x44);
    z80.write_ram_u8(0x003A, 0xED);
    z80.write_ram_u8(0x003B, 0x4D);

    z80.set_im0_interrupt_opcode(0x00); // Non-RST opcode
    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.interrupt_mode, 0);
    assert_eq!(z80.pc, 0x0004);

    z80.request_interrupt();
    z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);

    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.a, 0x44);
    assert_eq!(z80.pc, 0x0005);
}

#[test]
fn interrupt_mode_2_uses_i_register_vector_table() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // ld a,0x12 ; ld i,a ; im 2 ; ei ; halt ; halt
    z80.write_ram_u8(0x0000, 0x3E);
    z80.write_ram_u8(0x0001, 0x12);
    z80.write_ram_u8(0x0002, 0xED);
    z80.write_ram_u8(0x0003, 0x47);
    z80.write_ram_u8(0x0004, 0xED);
    z80.write_ram_u8(0x0005, 0x5E);
    z80.write_ram_u8(0x0006, 0xFB);
    z80.write_ram_u8(0x0007, 0x76);
    z80.write_ram_u8(0x0008, 0x76);

    // IM2 vector table at I:0x12FF -> 0x3456.
    z80.write_ram_u8(0x12FF, 0x56);
    z80.write_ram_u8(0x1300, 0x34);
    // Handler @0x3456: ld a,0x77 ; reti
    z80.write_ram_u8(0x3456, 0x3E);
    z80.write_ram_u8(0x3457, 0x77);
    z80.write_ram_u8(0x3458, 0xED);
    z80.write_ram_u8(0x3459, 0x4D);

    z80.step(256, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.interrupt_mode, 2);
    assert_eq!(z80.pc, 0x0008);

    z80.request_interrupt();
    z80.step(512, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.unknown_opcode_total(), 0);
    assert_eq!(z80.a, 0x77);
    assert_eq!(z80.pc, 0x0009);
}

#[test]
fn inc_de_opcode_updates_register_pair() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    // ld de,0x00FF ; inc de ; halt
    z80.write_ram_u8(0x0000, 0x11);
    z80.write_ram_u8(0x0001, 0xFF);
    z80.write_ram_u8(0x0002, 0x00);
    z80.write_ram_u8(0x0003, 0x13);
    z80.write_ram_u8(0x0004, 0x76);

    z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.de(), 0x0100);
}

#[test]
fn ldi_copies_byte_and_updates_pairs() {
    let mut z80 = Z80::new();
    let mut audio = AudioBus::new();
    let cart = dummy_cart();
    let mut work_ram = [0u8; 0x10000];
    let mut vdp = Vdp::new();
    let mut io = IoBus::new();
    z80.write_reset_byte(0x01);

    z80.set_hl(0x0100);
    z80.set_de(0x0200);
    z80.set_bc(0x0001);
    z80.write_ram_u8(0x0100, 0x5A);
    z80.write_ram_u8(0x0000, 0xED);
    z80.write_ram_u8(0x0001, 0xA0);
    z80.write_ram_u8(0x0002, 0x76);

    z80.step(128, &mut audio, &cart, &mut work_ram, &mut vdp, &mut io);
    assert_eq!(z80.read_ram_u8(0x0200), 0x5A);
    assert_eq!(z80.hl(), 0x0101);
    assert_eq!(z80.de(), 0x0201);
    assert_eq!(z80.bc(), 0x0000);
}
