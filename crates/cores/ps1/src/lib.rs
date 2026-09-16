//! Safe, single-instance host for the vendored PCSX ReARMed HLE core.
//! The native core has process-global state. The lifetime lock and !Send/!Sync
//! marker keep all calls and borrowed buffers exclusive to their owning thread.
use std::{
    ffi::{c_char, c_void, CString},
    marker::PhantomData,
    path::Path,
    rc::Rc,
    sync::{Mutex, MutexGuard},
};

static INSTANCE: Mutex<()> = Mutex::new(());

unsafe extern "C" {
    fn revive_ps1_load(path: *const c_char, save_dir: *const c_char) -> bool;
    fn revive_ps1_close();
    fn revive_ps1_step() -> bool;
    fn revive_ps1_pixels() -> *const u8;
    fn revive_ps1_width() -> u32;
    fn revive_ps1_height() -> u32;
    fn revive_ps1_audio() -> *const i16;
    fn revive_ps1_audio_len() -> usize;
    fn revive_ps1_clear_audio();
    fn revive_ps1_audio_enabled(enabled: bool);
    fn revive_ps1_button(port: u32, id: u32, pressed: bool);
    fn revive_ps1_fps() -> f64;
    fn retro_get_memory_data(id: u32) -> *mut c_void;
    fn retro_get_memory_size(id: u32) -> usize;
    fn retro_serialize_size() -> usize;
    fn retro_serialize(data: *mut c_void, size: usize) -> bool;
    fn retro_unserialize(data: *const c_void, size: usize) -> bool;
}

pub struct Emulator {
    _lock: MutexGuard<'static, ()>,
    _thread: PhantomData<Rc<()>>,
    _path: CString,
    _save_directory: CString,
}
impl Emulator {
    pub fn load(path: &Path, save_directory: &Path) -> Result<Self, String> {
        let lock = INSTANCE
            .try_lock()
            .map_err(|_| "A PS1 core is already active in this process")?;
        let path = c_path(path)?;
        let save_directory = c_path(save_directory)?;
        // The lock is acquired before any access to the native global state.
        if !unsafe { revive_ps1_load(path.as_ptr(), save_directory.as_ptr()) } {
            return Err(
                "PS1 could not load the disc with HLE BIOS; check the CUE and its tracks".into(),
            );
        }
        Ok(Self {
            _lock: lock,
            _thread: PhantomData,
            _path: path,
            _save_directory: save_directory,
        })
    }
    pub fn step_frame(&mut self) -> Result<(), String> {
        if unsafe { revive_ps1_step() } {
            Ok(())
        } else {
            Err("PS1 requested shutdown".into())
        }
    }
    pub fn frame(&self) -> (&[u8], usize, usize) {
        unsafe {
            let w = revive_ps1_width() as usize;
            let h = revive_ps1_height() as usize;
            (
                std::slice::from_raw_parts(revive_ps1_pixels(), w * h * 4),
                w,
                h,
            )
        }
    }
    pub fn frame_rate_hz(&self) -> f64 {
        unsafe { revive_ps1_fps() }
    }
    pub fn set_audio_enabled(&mut self, enabled: bool) {
        unsafe { revive_ps1_audio_enabled(enabled) };
    }
    pub fn drain_audio(&mut self, out: &mut Vec<i16>) {
        out.clear();
        unsafe {
            out.extend_from_slice(std::slice::from_raw_parts(
                revive_ps1_audio(),
                revive_ps1_audio_len(),
            ));
            revive_ps1_clear_audio();
        }
    }
    /// Libretro digital joypad ids; ports are zero based.
    pub fn set_button(&mut self, port: u32, id: u32, pressed: bool) {
        unsafe { revive_ps1_button(port, id, pressed) };
    }
    fn memory(&self, id: u32) -> &[u8] {
        unsafe {
            let ptr = retro_get_memory_data(id).cast::<u8>();
            let len = retro_get_memory_size(id);
            if ptr.is_null() || len == 0 {
                &[]
            } else {
                std::slice::from_raw_parts(ptr, len)
            }
        }
    }
    fn memory_mut(&mut self, id: u32) -> &mut [u8] {
        unsafe {
            let ptr = retro_get_memory_data(id).cast::<u8>();
            let len = retro_get_memory_size(id);
            if ptr.is_null() || len == 0 {
                &mut []
            } else {
                std::slice::from_raw_parts_mut(ptr, len)
            }
        }
    }
    pub fn ram(&self) -> &[u8] {
        self.memory(2)
    }
    pub fn write_ram(&mut self, offset: usize, value: u8) -> bool {
        if let Some(byte) = self.memory_mut(2).get_mut(offset) {
            *byte = value;
            true
        } else {
            false
        }
    }
    pub fn memory_card(&self) -> &[u8] {
        self.memory(0)
    }
    pub fn load_memory_card(&mut self, data: &[u8]) -> Result<(), String> {
        let memory = self.memory_mut(0);
        if memory.len() != data.len() || memory.is_empty() {
            return Err("PS1 memory card must be exactly 128 KiB".into());
        }
        memory.copy_from_slice(data);
        Ok(())
    }
    pub fn save_state(&mut self) -> Result<Vec<u8>, String> {
        let mut data = vec![0; unsafe { retro_serialize_size() }];
        if unsafe { retro_serialize(data.as_mut_ptr().cast(), data.len()) } {
            Ok(data)
        } else {
            Err("PS1 state save failed".into())
        }
    }
    pub fn load_state(&mut self, data: &[u8]) -> Result<(), String> {
        // The upstream memory reader assumes a full native state buffer.
        if data.len() != unsafe { retro_serialize_size() } {
            return Err("Invalid PS1 state size".into());
        }
        if !unsafe { retro_unserialize(data.as_ptr().cast(), data.len()) } {
            return Err("PS1 state load failed".into());
        }
        unsafe {
            revive_ps1_clear_audio();
        }
        Ok(())
    }
}
impl Drop for Emulator {
    fn drop(&mut self) {
        unsafe { revive_ps1_close() };
    }
}
fn c_path(path: &Path) -> Result<CString, String> {
    let text = path.to_str().ok_or("PS1 requires a UTF-8 file path")?;
    CString::new(text).map_err(|_| "PS1 path contains a NUL byte".into())
}
