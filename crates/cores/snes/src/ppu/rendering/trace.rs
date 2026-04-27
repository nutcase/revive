use std::sync::OnceLock;

pub(super) fn env_presence_flag(name: &'static str) -> bool {
    if cfg!(test) {
        return std::env::var_os(name).is_some();
    }

    match name {
        "BYPASS_OPT" => {
            static VALUE: OnceLock<bool> = OnceLock::new();
            *VALUE.get_or_init(|| std::env::var_os("BYPASS_OPT").is_some())
        }
        _ => std::env::var_os(name).is_some(),
    }
}
