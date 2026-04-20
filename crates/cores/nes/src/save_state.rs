//! Save-state serialization support.
//!
//! Front-ends normally use [`crate::Nes::save_state`] and
//! [`crate::Nes::load_state`]. The exported [`SaveState`] type is intentionally
//! opaque so compatibility migrations can stay inside the core crate.

mod format;
mod io;
mod legacy;
mod types;

pub use types::SaveState;

#[cfg(test)]
mod tests;
