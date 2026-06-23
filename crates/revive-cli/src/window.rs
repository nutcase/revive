use sdl3::video::Window;

pub(crate) fn bring_window_to_front(window: &mut Window) {
    let _ = window.show();
    let _ = window.raise();
    platform_bring_window_to_front();
}

#[cfg(target_os = "macos")]
fn platform_bring_window_to_front() {
    macos_frontmost::activate_application();
}

#[cfg(not(target_os = "macos"))]
fn platform_bring_window_to_front() {}

#[cfg(target_os = "macos")]
mod macos_frontmost {
    use std::ffi::{c_char, c_void, CString};

    const NS_APPLICATION_ACTIVATION_POLICY_REGULAR: isize = 0;
    const NS_APPLICATION_ACTIVATE_ALL_WINDOWS: usize = 1 << 0;
    const NS_APPLICATION_ACTIVATE_IGNORING_OTHER_APPS: usize = 1 << 1;

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        fn objc_msgSend();
    }

    pub fn activate_application() {
        unsafe {
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
    }

    unsafe fn send_id(target: *mut c_void, selector: *mut c_void) -> *mut c_void {
        let func: extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void =
            unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        func(target, selector)
    }

    unsafe fn send_bool(target: *mut c_void, selector: *mut c_void, value: bool) {
        let func: extern "C" fn(*mut c_void, *mut c_void, bool) =
            unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        func(target, selector, value);
    }

    unsafe fn send_isize_bool(target: *mut c_void, selector: *mut c_void, value: isize) -> bool {
        let func: extern "C" fn(*mut c_void, *mut c_void, isize) -> bool =
            unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        func(target, selector, value)
    }

    unsafe fn send_usize_bool(target: *mut c_void, selector: *mut c_void, value: usize) -> bool {
        let func: extern "C" fn(*mut c_void, *mut c_void, usize) -> bool =
            unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        func(target, selector, value)
    }

    fn sel(name: &str) -> *mut c_void {
        let name = cstr(name);
        unsafe { sel_registerName(name.as_ptr()) }
    }

    fn cstr(value: &str) -> CString {
        CString::new(value).expect("Objective-C selector names must not contain NUL")
    }
}
