use super::legacy::{LegacySaveState, SaveStateRawU8Mapper, SaveStateV1, SaveStateV2};
use super::types::SaveState;
use crate::Result;
use serde::{Deserialize, Serialize};

const SAVE_STATE_MAGIC: [u8; 4] = *b"NESS";

#[derive(Serialize, Deserialize)]
struct SaveStateFile {
    magic: [u8; 4],
    version: u16,
    state: SaveState,
}

#[derive(Deserialize)]
struct SaveStateFileU8Mapper {
    magic: [u8; 4],
    version: u16,
    state: SaveStateRawU8Mapper,
}

#[derive(Clone, Copy)]
pub(super) enum LoadedSaveStateFormat {
    CurrentWrapper(u16),
    CurrentWrapperU8Mapper(u16),
    CurrentRaw,
    CurrentRawU8Mapper,
    V2,
    V1,
    Legacy,
}

pub(super) fn encode_current(state: &SaveState) -> Result<Vec<u8>> {
    Ok(bincode::serialize(&SaveStateFile {
        magic: SAVE_STATE_MAGIC,
        version: SaveState::FORMAT_VERSION,
        state: state.clone(),
    })?)
}

pub(super) fn decode_any(data: &[u8]) -> Result<(SaveState, LoadedSaveStateFormat)> {
    if let Ok(file) = bincode::deserialize::<SaveStateFile>(data) {
        if file.magic == SAVE_STATE_MAGIC && file.version == SaveState::FORMAT_VERSION {
            return Ok((
                file.state,
                LoadedSaveStateFormat::CurrentWrapper(file.version),
            ));
        }
    }

    if let Ok(file) = bincode::deserialize::<SaveStateFileU8Mapper>(data) {
        if file.magic == SAVE_STATE_MAGIC {
            return Ok((
                file.state.into(),
                LoadedSaveStateFormat::CurrentWrapperU8Mapper(file.version),
            ));
        }
    }

    if let Ok(save_state) = bincode::deserialize::<SaveState>(data) {
        return Ok((save_state, LoadedSaveStateFormat::CurrentRaw));
    }

    if let Ok(save_state) = bincode::deserialize::<SaveStateRawU8Mapper>(data) {
        return Ok((save_state.into(), LoadedSaveStateFormat::CurrentRawU8Mapper));
    }

    if let Ok(v2) = bincode::deserialize::<SaveStateV2>(data) {
        return Ok((v2.into(), LoadedSaveStateFormat::V2));
    }

    if let Ok(v1) = bincode::deserialize::<SaveStateV1>(data) {
        return Ok((v1.into(), LoadedSaveStateFormat::V1));
    }

    let legacy = bincode::deserialize::<LegacySaveState>(data)?;
    Ok((legacy.into(), LoadedSaveStateFormat::Legacy))
}
