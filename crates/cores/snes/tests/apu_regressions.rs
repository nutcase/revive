use snes_emulator::audio::apu::Apu;

#[test]
fn apu_pending_cycles_saturates() {
    let mut apu = Apu::new();
    apu.add_cpu_cycles(u32::MAX - 8);
    apu.add_cpu_cycles(64);
    assert_eq!(apu.pending_cpu_cycles(), u32::MAX);
}

#[test]
fn apu_pending_cycles_sync_and_savestate_roundtrip() {
    let mut apu = Apu::new();
    apu.add_cpu_cycles(321);
    assert_eq!(apu.pending_cpu_cycles(), 321);

    // Save/load should preserve pending cycle debt.
    let st = apu.to_save_state();
    let mut apu2 = Apu::new();
    apu2.load_from_save_state(&st);
    assert_eq!(apu2.pending_cpu_cycles(), 321);

    // sync() should flush debt.
    apu2.sync();
    assert_eq!(apu2.pending_cpu_cycles(), 0);
}

#[test]
fn apu_port_sync_is_strict_during_boot_handshake() {
    let mut apu = Apu::new();
    // Force non-running handshake phase (Uploading) irrespective of env defaults.
    let mut st = apu.to_save_state();
    st.boot_state = 2;
    apu.load_from_save_state(&st);
    apu.add_cpu_cycles(1);
    apu.sync_for_port_access();
    assert_eq!(apu.pending_cpu_cycles(), 0);
}

#[test]
fn apu_port_read_sync_in_running_state_flushes_debt() {
    let mut apu = Apu::new();
    apu.load_and_start(&[], 0x0200, 0x0200); // Force Running state

    apu.add_cpu_cycles(1);
    apu.sync_for_port_access();
    assert_eq!(apu.pending_cpu_cycles(), 0);
}

#[test]
fn apu_port_write_sync_flushes_only() {
    let mut apu = Apu::new();
    apu.load_and_start(&[], 0x0200, 0x0200); // Force Running state

    // sync_for_port_write はフラッシュのみ行い SPC サイクル消費はしない。
    // マルチポート書き込み中に SPC が走るレースを防ぐため。
    apu.add_cpu_cycles(8);
    apu.sync_for_port_write();
    assert_eq!(apu.pending_cpu_cycles(), 8);
}

#[test]
fn apu_port_write_sync_flushes_only_during_boot() {
    let mut apu = Apu::new();
    // Force non-running handshake phase (Uploading).
    let mut st = apu.to_save_state();
    st.boot_state = 2;
    apu.load_from_save_state(&st);

    apu.add_cpu_cycles(1);
    apu.sync_for_port_write();
    assert_eq!(apu.pending_cpu_cycles(), 1);
}

#[test]
fn apu_sync_flushes_pending_port_writes_even_without_cycle_debt() {
    let mut apu = Apu::new();
    apu.load_and_start(&[], 0x0200, 0x0200); // Running

    // Queue CPU->APU port write with zero cycle debt.
    apu.write_port(0, 0x5A);
    let st_before = apu.to_save_state();
    assert_eq!(st_before.pending_cpu_cycles, 0);
    assert_eq!(st_before.pending_port_writes.len(), 1);

    // sync_for_port_access must still flush pending writes even when debt is 0.
    apu.sync_for_port_access();
    let st_after = apu.to_save_state();
    assert_eq!(st_after.pending_port_writes.len(), 0);
    assert_eq!(st_after.cpu_to_apu_ports[0], 0x5A);
}
