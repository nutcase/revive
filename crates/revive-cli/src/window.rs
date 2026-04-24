use sdl2::video::Window;

pub(crate) fn bring_window_to_front(window: &mut Window) {
    window.show();
    window.raise();
    platform_bring_window_to_front(window);
}

#[cfg(target_os = "macos")]
fn platform_bring_window_to_front(window: &Window) {
    macos_frontmost::activate_window(window);
}

#[cfg(not(target_os = "macos"))]
fn platform_bring_window_to_front(_window: &Window) {}

#[cfg(target_os = "macos")]
mod macos_frontmost {
    use std::ffi::{c_char, c_void, CString};

    use sdl2::video::Window;

    const SDL_SYSWM_COCOA: u32 = 4;
    const NS_APPLICATION_ACTIVATION_POLICY_REGULAR: isize = 0;
    const NS_APPLICATION_ACTIVATE_ALL_WINDOWS: usize = 1 << 0;
    const NS_APPLICATION_ACTIVATE_IGNORING_OTHER_APPS: usize = 1 << 1;

    #[repr(C)]
    union SdlSysWmInfoData {
        cocoa: CocoaInfo,
        dummy: [u8; 64],
        _align: [u64; 8],
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct CocoaInfo {
        window: *mut c_void,
    }

    #[repr(C)]
    struct SdlSysWmInfo {
        version: sdl2::sys::SDL_version,
        subsystem: u32,
        info: SdlSysWmInfoData,
    }

    #[link(name = "objc")]
    unsafe extern "C" {
        fn SDL_GetWindowWMInfo(
            window: *mut sdl2::sys::SDL_Window,
            info: *mut SdlSysWmInfo,
        ) -> sdl2::sys::SDL_bool;

        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        fn objc_msgSend();
    }

    pub fn activate_window(window: &Window) {
        let Some(ns_window) = ns_window(window) else {
            return;
        };
        unsafe {
            activate_application();
            send_void_no_args(ns_window, sel("makeMainWindow"));
            send_void_no_args(ns_window, sel("makeKeyWindow"));
            send_void(
                ns_window,
                sel("makeKeyAndOrderFront:"),
                std::ptr::null_mut(),
            );
            send_void_no_args(ns_window, sel("orderFrontRegardless"));
        }
    }

    fn ns_window(window: &Window) -> Option<*mut c_void> {
        unsafe {
            let mut info: SdlSysWmInfo = std::mem::zeroed();
            sdl2::sys::SDL_GetVersion(&mut info.version);
            if SDL_GetWindowWMInfo(window.raw(), &mut info) == sdl2::sys::SDL_bool::SDL_FALSE {
                return None;
            }
            if info.subsystem != SDL_SYSWM_COCOA {
                return None;
            }
            let ns_window = info.info.cocoa.window;
            (!ns_window.is_null()).then_some(ns_window)
        }
    }

    unsafe fn activate_application() {
        let ns_application = objc_getClass(cstr("NSApplication").as_ptr());
        if ns_application.is_null() {
            return;
        }
        let app = send_id(ns_application, sel("sharedApplication"));
        if app.is_null() {
            return;
        }
        let _ = send_isize_bool(
            app,
            sel("setActivationPolicy:"),
            NS_APPLICATION_ACTIVATION_POLICY_REGULAR,
        );
        send_bool(app, sel("activateIgnoringOtherApps:"), true);

        let ns_running_application = objc_getClass(cstr("NSRunningApplication").as_ptr());
        if ns_running_application.is_null() {
            return;
        }
        let running_app = send_id(ns_running_application, sel("currentApplication"));
        if running_app.is_null() {
            return;
        }
        let _ = send_usize_bool(
            running_app,
            sel("activateWithOptions:"),
            NS_APPLICATION_ACTIVATE_ALL_WINDOWS | NS_APPLICATION_ACTIVATE_IGNORING_OTHER_APPS,
        );
    }

    unsafe fn send_id(receiver: *mut c_void, selector: *mut c_void) -> *mut c_void {
        let send: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void =
            std::mem::transmute(objc_msgSend as *const ());
        send(receiver, selector)
    }

    unsafe fn send_bool(receiver: *mut c_void, selector: *mut c_void, value: bool) {
        let send: unsafe extern "C" fn(*mut c_void, *mut c_void, bool) =
            std::mem::transmute(objc_msgSend as *const ());
        send(receiver, selector, value);
    }

    unsafe fn send_isize_bool(receiver: *mut c_void, selector: *mut c_void, value: isize) -> bool {
        let send: unsafe extern "C" fn(*mut c_void, *mut c_void, isize) -> bool =
            std::mem::transmute(objc_msgSend as *const ());
        send(receiver, selector, value)
    }

    unsafe fn send_usize_bool(receiver: *mut c_void, selector: *mut c_void, value: usize) -> bool {
        let send: unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> bool =
            std::mem::transmute(objc_msgSend as *const ());
        send(receiver, selector, value)
    }

    unsafe fn send_void(receiver: *mut c_void, selector: *mut c_void, value: *mut c_void) {
        let send: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) =
            std::mem::transmute(objc_msgSend as *const ());
        send(receiver, selector, value);
    }

    unsafe fn send_void_no_args(receiver: *mut c_void, selector: *mut c_void) {
        let send: unsafe extern "C" fn(*mut c_void, *mut c_void) =
            std::mem::transmute(objc_msgSend as *const ());
        send(receiver, selector);
    }

    fn sel(name: &str) -> *mut c_void {
        unsafe { sel_registerName(cstr(name).as_ptr()) }
    }

    fn cstr(value: &str) -> CString {
        CString::new(value).expect("Objective-C selector/class names must not contain NUL")
    }
}
