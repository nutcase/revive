use revive_core::{CoreInstance, SystemKind};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

struct FixtureDir(PathBuf);
impl Drop for FixtureDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn no_audio_reaches_every_non_snes_adapter_and_frames_still_advance() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = FixtureDir(
        std::env::temp_dir().join(format!("revive-audio-{}-{nonce}", std::process::id())),
    );
    fs::create_dir(&dir.0).unwrap();
    let mut nes = vec![0; 16 + 0x8000];
    nes[..6].copy_from_slice(&[b'N', b'E', b'S', 0x1A, 2, 0]);
    nes[16..19].copy_from_slice(&[0x4C, 0x00, 0x80]);
    nes[16 + 0x7FFC..16 + 0x7FFE].copy_from_slice(&[0x00, 0x80]);
    let mut gb = vec![0; 0x8000];
    gb[0x100..0x102].copy_from_slice(&[0x18, 0xFE]);
    gb[0x143] = 0x80;
    let mut gba = vec![0; 0x200];
    gba[..4].copy_from_slice(&0xEAFF_FFFEu32.to_le_bytes());
    let sega8 = vec![0x76; 0x8000]; // HALT
    let mut md = vec![0; 0x400];
    md[..4].copy_from_slice(&0x00FF_FF00u32.to_be_bytes());
    md[4..8].copy_from_slice(&0x0000_0200u32.to_be_bytes());
    md[0x100..0x104].copy_from_slice(b"SEGA");
    md[0x200..0x202].copy_from_slice(&[0x60, 0xFE]);
    let pce = vec![0x80, 0xFE]; // load_program supplies the reset vector
    for (system, name, rom) in [
        (SystemKind::Nes, "loop.nes", nes.as_slice()),
        (SystemKind::GameBoy, "loop.gb", gb.as_slice()),
        (SystemKind::GameBoyColor, "loop.gbc", gb.as_slice()),
        (SystemKind::GameBoyAdvance, "loop.gba", gba.as_slice()),
        (SystemKind::Sg1000, "loop.sg", sega8.as_slice()),
        (SystemKind::MasterSystem, "loop.sms", sega8.as_slice()),
        (SystemKind::MegaDrive, "loop.md", md.as_slice()),
        (SystemKind::Pce, "loop.bin", pce.as_slice()),
    ] {
        let path = dir.0.join(name);
        fs::write(&path, rom).unwrap();
        let mut core = CoreInstance::load_rom_with_audio(&path, Some(system), false).unwrap();
        let mut out = vec![123];
        for _ in 0..3 {
            core.step_frame()
                .unwrap_or_else(|e| panic!("{system:?}: {e}"));
            core.drain_audio_i16(&mut out);
            assert!(
                out.is_empty(),
                "{system:?} produced host PCM with audio disabled"
            );
            assert!(!core.frame().data.is_empty(), "{system:?}");
        }
    }
}
