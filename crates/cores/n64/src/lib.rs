//! Thread-confined host for the bundled Mupen64Plus-Next software core.
use sha2::{Digest, Sha256};
use std::{
    ffi::{c_char, c_void, CString},
    marker::PhantomData,
    path::Path,
    rc::Rc,
    sync::{Mutex, MutexGuard},
};

static INSTANCE: Mutex<()> = Mutex::new(());
const STATE_MAGIC: &[u8; 8] = b"RVN64S01";
const STATE_HEADER: usize = 72;

macro_rules! api {
    ($($field:ident: $ty:ty ; $symbol:literal),* $(,)?) => {
        struct Api { $($field: $ty,)* }
        impl Api {
            unsafe fn load(lib: &libloading::Library) -> Result<Self, String> {
                Ok(Self { $($field: *lib.get::<$ty>(concat!($symbol, "\0").as_bytes()).map_err(|e| e.to_string())?,)* })
            }
        }
    }
}
api! {
    load: unsafe extern "C" fn(*const u8, usize, *const c_char) -> bool ; "revive_n64_load",
    close: unsafe extern "C" fn() ; "revive_n64_close",
    step: unsafe extern "C" fn() -> bool ; "revive_n64_step",
    pixels: unsafe extern "C" fn() -> *const u8 ; "revive_n64_pixels",
    width: unsafe extern "C" fn() -> u32 ; "revive_n64_width",
    height: unsafe extern "C" fn() -> u32 ; "revive_n64_height",
    audio: unsafe extern "C" fn() -> *const i16 ; "revive_n64_audio",
    audio_len: unsafe extern "C" fn() -> usize ; "revive_n64_audio_len",
    clear_audio: unsafe extern "C" fn() ; "revive_n64_clear_audio",
    audio_enabled: unsafe extern "C" fn(bool) ; "revive_n64_audio_enabled",
    button: unsafe extern "C" fn(u32, u32, bool) ; "revive_n64_button",
    axis: unsafe extern "C" fn(u32, i16, i16) ; "revive_n64_axis",
    fps: unsafe extern "C" fn() -> f64 ; "revive_n64_fps",
    memory: unsafe extern "C" fn(u32) -> *mut c_void ; "retro_get_memory_data",
    memory_size: unsafe extern "C" fn(u32) -> usize ; "retro_get_memory_size",
    state_size: unsafe extern "C" fn() -> usize ; "retro_serialize_size",
    serialize: unsafe extern "C" fn(*mut c_void, usize) -> bool ; "retro_serialize",
    unserialize: unsafe extern "C" fn(*const c_void, usize) -> bool ; "retro_unserialize",
}

pub struct Emulator {
    api: Api,
    // Drop unloads the core first, then the image, then its temporary directory.
    _library: libloading::Library,
    _directory: tempfile::TempDir,
    _lock: MutexGuard<'static, ()>,
    _thread: PhantomData<Rc<()>>,
    rom_hash: [u8; 32],
    ram: Vec<u8>,
}

/// Normalize cartridge dumps to big-endian (.z64) independently of their name.
/// Bounds and alignment are checked before passing any data to native code.
pub fn normalize_rom(mut bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    if !(0x1000..=64 * 1024 * 1024).contains(&bytes.len()) || !bytes.len().is_multiple_of(4) {
        return Err("N64 ROM must be 4 KiB–64 MiB and a multiple of four bytes".into());
    }
    match &bytes[..4] {
        [0x80, 0x37, 0x12, 0x40] => {}
        [0x37, 0x80, 0x40, 0x12] => {
            for pair in bytes.as_chunks_mut::<2>().0.iter_mut() {
                pair.swap(0, 1);
            }
        }
        [0x40, 0x12, 0x37, 0x80] => {
            for word in bytes.as_chunks_mut::<4>().0.iter_mut() {
                word.reverse();
            }
        }
        _ => return Err("Invalid N64 cartridge header".into()),
    }
    Ok(bytes)
}

impl Emulator {
    pub fn load(rom: Vec<u8>, save_directory: &Path) -> Result<Self, String> {
        let rom = normalize_rom(rom)?;
        let lock = INSTANCE
            .try_lock()
            .map_err(|_| "An N64 core is already active")?;
        std::fs::create_dir_all(save_directory).map_err(|e| e.to_string())?;
        let directory = tempfile::tempdir().map_err(|e| e.to_string())?;
        let library_path = directory.path().join(if cfg!(target_os = "macos") {
            "core.dylib"
        } else if cfg!(target_os = "windows") {
            "core.dll"
        } else {
            "core.so"
        });
        std::fs::write(&library_path, include_bytes!(env!("REVIVE_N64_LIBRARY")))
            .map_err(|e| e.to_string())?;
        let save_dir = std::fs::canonicalize(save_directory).map_err(|e| e.to_string())?;
        let save_dir = CString::new(
            save_dir
                .to_str()
                .ok_or("N64 save directory must be UTF-8")?,
        )
        .map_err(|e| e.to_string())?;
        // Only our build-time embedded image is loaded, with local symbol scope.
        let library =
            unsafe { libloading::Library::new(&library_path) }.map_err(|e| e.to_string())?;
        let api = unsafe { Api::load(&library)? };
        if !unsafe { (api.load)(rom.as_ptr(), rom.len(), save_dir.as_ptr()) } {
            return Err("N64 could not load the cartridge".into());
        }
        let mut emulator = Self {
            api,
            _library: library,
            _directory: directory,
            _lock: lock,
            _thread: PhantomData,
            rom_hash: Sha256::digest(&rom).into(),
            ram: vec![0; 8 * 1024 * 1024],
        };
        emulator.refresh_ram();
        Ok(emulator)
    }
    pub fn step_frame(&mut self) -> Result<(), String> {
        if !unsafe { (self.api.step)() } {
            return Err("N64 requested shutdown".into());
        }
        self.refresh_ram();
        Ok(())
    }
    pub fn frame(&self) -> (&[u8], usize, usize) {
        unsafe {
            let w = (self.api.width)() as usize;
            let h = (self.api.height)() as usize;
            (
                std::slice::from_raw_parts((self.api.pixels)(), w * h * 4),
                w,
                h,
            )
        }
    }
    pub fn frame_rate_hz(&self) -> f64 {
        unsafe { (self.api.fps)() }
    }
    pub fn set_audio_enabled(&mut self, enabled: bool) {
        unsafe { (self.api.audio_enabled)(enabled) };
    }
    pub fn drain_audio(&mut self, out: &mut Vec<i16>) {
        out.clear();
        unsafe {
            out.extend_from_slice(std::slice::from_raw_parts(
                (self.api.audio)(),
                (self.api.audio_len)(),
            ));
            (self.api.clear_audio)();
        }
    }
    pub fn set_button(&mut self, port: u32, id: u32, pressed: bool) {
        unsafe { (self.api.button)(port, id, pressed) };
    }
    /// SDL/libretro axis convention: negative Y points up; full range is i16.
    pub fn set_stick(&mut self, port: u32, x: i16, y: i16) {
        unsafe { (self.api.axis)(port, x, y) };
    }
    fn memory(&self, id: u32) -> &[u8] {
        unsafe {
            let ptr = (self.api.memory)(id).cast::<u8>();
            let len = (self.api.memory_size)(id);
            if ptr.is_null() || len == 0 {
                &[]
            } else {
                std::slice::from_raw_parts(ptr, len)
            }
        }
    }
    fn refresh_ram(&mut self) {
        // Mupen stores guest big-endian words in host order. Expose guest byte
        // addresses to the existing byte-based memory panel and cheat manager.
        unsafe {
            let ptr = (self.api.memory)(2).cast::<u8>();
            let len = (self.api.memory_size)(2).min(self.ram.len());
            if !ptr.is_null() {
                let native = std::slice::from_raw_parts(ptr, len);
                for (dst, src) in self
                    .ram
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(native.as_chunks::<4>().0.iter())
                {
                    dst.copy_from_slice(&[src[3], src[2], src[1], src[0]]);
                }
            }
        }
    }
    pub fn ram(&self) -> &[u8] {
        &self.ram
    }
    pub fn write_ram(&mut self, offset: usize, value: u8) -> bool {
        if offset >= self.ram.len() {
            return false;
        }
        unsafe {
            let ptr = (self.api.memory)(2).cast::<u8>();
            if ptr.is_null() || offset >= (self.api.memory_size)(2) {
                return false;
            }
            *ptr.add(offset ^ 3) = value;
        }
        self.ram[offset] = value;
        true
    }
    pub fn persistent_save(&self) -> &[u8] {
        self.memory(0)
    }
    pub fn load_persistent_save(&mut self, data: &[u8]) -> Result<(), String> {
        if data.len() != self.memory(0).len() || data.is_empty() {
            return Err("Invalid N64 cartridge/Controller Pak save size".into());
        }
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), (self.api.memory)(0).cast(), data.len());
        }
        Ok(())
    }
    fn raw_state(&mut self) -> Result<Vec<u8>, String> {
        let mut data = vec![0; unsafe { (self.api.state_size)() }];
        if unsafe { (self.api.serialize)(data.as_mut_ptr().cast(), data.len()) } {
            self.refresh_ram();
            Ok(data)
        } else {
            Err("N64 state save failed; run at least one frame first".into())
        }
    }
    /// Exact on-disk state size, including the Revive envelope.
    pub fn state_size(&self) -> usize {
        STATE_HEADER + unsafe { (self.api.state_size)() }
    }
    pub fn save_state(&mut self) -> Result<Vec<u8>, String> {
        let raw = self.raw_state()?;
        let mut data = Vec::with_capacity(STATE_HEADER + raw.len());
        data.extend_from_slice(STATE_MAGIC);
        data.extend_from_slice(&self.rom_hash);
        data.extend_from_slice(&Sha256::digest(&raw));
        data.extend_from_slice(&raw);
        Ok(data)
    }
    pub fn load_state(&mut self, data: &[u8]) -> Result<(), String> {
        let size = unsafe { (self.api.state_size)() };
        let raw = validate_state(data, &self.rom_hash, size)?;
        let backup = self.raw_state()?;
        // Require precisely this core's native format/version and current ROM.
        if raw[..44] != backup[..44] {
            return Err("N64 state uses an incompatible native format".into());
        }
        if !unsafe { (self.api.unserialize)(raw.as_ptr().cast(), raw.len()) } {
            if !unsafe { (self.api.unserialize)(backup.as_ptr().cast(), backup.len()) } {
                return Err("N64 state recovery failed; restart the game".into());
            }
            self.refresh_ram();
            return Err("N64 state load rejected; previous state restored".into());
        }
        unsafe {
            (self.api.clear_audio)();
        }
        self.refresh_ram();
        Ok(())
    }
}
impl Drop for Emulator {
    fn drop(&mut self) {
        unsafe {
            (self.api.close)();
        }
    }
}

fn validate_state<'a>(
    data: &'a [u8],
    rom_hash: &[u8; 32],
    size: usize,
) -> Result<&'a [u8], String> {
    if size < 44 || data.len() != STATE_HEADER + size || &data[..8] != STATE_MAGIC {
        return Err("Invalid N64 state size or format".into());
    }
    if &data[8..40] != rom_hash {
        return Err("N64 state belongs to a different ROM".into());
    }
    let raw = &data[STATE_HEADER..];
    if Sha256::digest(raw)[..] != data[40..STATE_HEADER] {
        return Err("N64 state checksum mismatch".into());
    }
    Ok(raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coroutine_preserves_host_floating_point_registers() {
        let _lock = INSTANCE.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-core");
        std::fs::write(&path, include_bytes!(env!("REVIVE_N64_LIBRARY"))).unwrap();
        unsafe {
            let lib = libloading::Library::new(path).unwrap();
            let test = lib
                .get::<unsafe extern "C" fn() -> bool>(b"revive_n64_test_coroutine\0")
                .unwrap();
            assert!(test());
        }
    }
    #[test]
    fn normalizes_all_three_formats_and_rejects_invalid_roms() {
        let mut base = vec![0; 4096];
        base[..8].copy_from_slice(&[0x80, 0x37, 0x12, 0x40, 1, 2, 3, 4]);
        let mut v = base.clone();
        for w in v.as_chunks_mut::<2>().0.iter_mut() {
            w.reverse();
        }
        let mut n = base.clone();
        for w in n.as_chunks_mut::<4>().0.iter_mut() {
            w.reverse();
        }
        assert_eq!(normalize_rom(v).unwrap(), base);
        assert_eq!(normalize_rom(n).unwrap(), base);
        assert_eq!(normalize_rom(base.clone()).unwrap(), base);
        assert!(normalize_rom(vec![]).is_err());
        assert!(normalize_rom(vec![0; 4096]).is_err());
        base.push(0);
        assert!(normalize_rom(base).is_err());
    }
    #[test]
    fn rejects_truncated_foreign_and_corrupt_states() {
        let hash = [7; 32];
        let raw = vec![0; 64];
        let mut state = Vec::from(STATE_MAGIC.as_slice());
        state.extend_from_slice(&hash);
        state.extend_from_slice(&Sha256::digest(&raw));
        state.extend_from_slice(&raw);
        assert!(validate_state(&state, &hash, 64).is_ok());
        assert!(validate_state(&state, &[8; 32], 64).is_err());
        assert!(validate_state(&state[..20], &hash, 64).is_err());
        state[80] ^= 1;
        assert!(validate_state(&state, &hash, 64).is_err());
    }
}
