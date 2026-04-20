#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

static QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);
static EXIT_CODE: AtomicI32 = AtomicI32::new(0);

pub fn should_quit() -> bool {
    QUIT_REQUESTED.load(Ordering::SeqCst)
}

pub fn request_quit() {
    QUIT_REQUESTED.store(true, Ordering::SeqCst);
}

pub fn clear_for_tests() {
    QUIT_REQUESTED.store(false, Ordering::SeqCst);
    EXIT_CODE.store(0, Ordering::SeqCst);
}

pub fn exit_code() -> i32 {
    EXIT_CODE.load(Ordering::SeqCst)
}

pub fn set_exit_code(code: i32) {
    if code == 0 {
        return;
    }
    // Keep the first non-zero code to preserve the original failure reason.
    let _ = EXIT_CODE.compare_exchange(0, code, Ordering::SeqCst, Ordering::SeqCst);
}

pub fn request_quit_with_code(code: i32) {
    set_exit_code(code);
    request_quit();
}

#[cfg(unix)]
pub fn install() {
    use std::os::raw::c_int;
    const SIGINT: c_int = 2;
    const SIGTERM: c_int = 15;

    extern "C" fn handler(_sig: c_int) {
        // Set a flag only; do not perform IO in signal context
        request_quit();
    }

    extern "C" {
        fn signal(sig: c_int, handler: extern "C" fn(c_int)) -> usize;
    }

    unsafe {
        // Best-effort; ignore returns
        let _ = signal(SIGINT, handler);
        let _ = signal(SIGTERM, handler);
    }
}

#[cfg(not(unix))]
pub fn install() {
    // Windows console Ctrl+C handler via SetConsoleCtrlHandler
    #[cfg(target_os = "windows")]
    unsafe {
        use std::ptr;
        type HandlerRoutine = extern "system" fn(u32) -> i32;
        extern "system" {
            fn SetConsoleCtrlHandler(handler: Option<HandlerRoutine>, add: i32) -> i32;
        }
        extern "system" fn handler(ctrl_type: u32) -> i32 {
            // Handle CTRL_C_EVENT(0), CTRL_CLOSE_EVENT(2) etc.
            let _ = ctrl_type; // unused detail
            request_quit();
            1 // handled
        }
        let _ = SetConsoleCtrlHandler(Some(handler), 1);
    }
    #[cfg(not(target_os = "windows"))]
    {
        // No-op fallback; periodic autosave still provides coverage
    }
}
