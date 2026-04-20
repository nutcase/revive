mod legacy_format;
mod u8_mapper;
mod v1;
mod v2;

pub(in crate::save_state) use legacy_format::LegacySaveState;
pub(in crate::save_state) use u8_mapper::{CartridgeStateU8Mapper, SaveStateRawU8Mapper};
#[cfg(test)]
pub(in crate::save_state) use v1::CartridgeStateV1;
pub(in crate::save_state) use v1::SaveStateV1;
pub(in crate::save_state) use v2::SaveStateV2;
