use crate::save_state::SaveState;

pub(super) fn load_serialized_save_state<T: serde::Serialize>(label: &str, state: &T) -> SaveState {
    let encoded = bincode::serialize(state).expect("serialize save state");
    let mut path = std::env::temp_dir();
    path.push(format!(
        "nes_{label}_state_{}.sav",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));

    std::fs::write(&path, encoded).expect("write save state");
    let decoded = SaveState::load_from_file(path.to_str().expect("utf-8 path"))
        .expect("load serialized save state");
    let _ = std::fs::remove_file(path);
    decoded
}
