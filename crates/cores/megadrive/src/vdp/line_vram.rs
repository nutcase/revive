use super::{FRAME_HEIGHT, VRAM_SIZE};
use bincode::{Decode, Encode};
use std::{ops::Index, sync::Arc};

static ZERO: [u8; VRAM_SIZE] = [0; VRAM_SIZE];

/// Debug-only scanline history. No heap storage while unused; identical
/// adjacent captures share immutable bytes. The wire format remains the old
/// Vec<[u8; VRAM_SIZE]> so existing states work in both directions.
#[derive(Debug, Clone, Default)]
pub(super) struct LineVram {
    rows: Vec<Arc<[u8; VRAM_SIZE]>>,
    latest: Option<usize>,
}
impl LineVram {
    pub(super) fn get(&self, index: usize) -> Option<&[u8; VRAM_SIZE]> {
        (index < FRAME_HEIGHT).then(|| &self[index])
    }
    pub(super) fn first(&self) -> Option<&[u8; VRAM_SIZE]> {
        self.get(0)
    }

    pub(super) fn capture(&mut self, line: usize, vram: &[u8; VRAM_SIZE]) {
        if self.rows.is_empty() {
            let zero = Arc::new(ZERO);
            self.rows = vec![zero; FRAME_HEIGHT];
        }
        if let Some(previous) = self.latest
            && self.rows[previous].as_ref() == vram
        {
            self.rows[line] = Arc::clone(&self.rows[previous]);
            self.latest = Some(line);
            return;
        }
        Arc::make_mut(&mut self.rows[line]).copy_from_slice(vram);
        self.latest = Some(line);
    }
}
impl Index<usize> for LineVram {
    type Output = [u8; VRAM_SIZE];
    fn index(&self, index: usize) -> &Self::Output {
        assert!(index < FRAME_HEIGHT);
        self.rows.get(index).map_or(&ZERO, |row| row.as_ref())
    }
}
impl Encode for LineVram {
    fn encode<E: bincode::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        (FRAME_HEIGHT as u64).encode(encoder)?;
        for line in 0..FRAME_HEIGHT {
            self[line].encode(encoder)?;
        }
        Ok(())
    }
}
impl<Context> Decode<Context> for LineVram {
    fn decode<D: bincode::de::Decoder<Context = Context>>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        let len = u64::decode(decoder)?;
        if len != FRAME_HEIGHT as u64 {
            return Err(bincode::error::DecodeError::Other(
                "invalid scanline VRAM history length",
            ));
        }
        let mut out = Self::default();
        for line in 0..FRAME_HEIGHT {
            let row = <[u8; VRAM_SIZE]>::decode(decoder)?;
            if row != ZERO || !out.rows.is_empty() {
                out.capture(line, &row);
            }
        }
        Ok(out)
    }
}
bincode::impl_borrow_decode!(LineVram);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unused_and_shared_history_preserve_legacy_bytes() {
        let config = bincode::config::standard();
        let mut history = LineVram::default();
        assert!(history.rows.is_empty());
        let mut legacy = vec![ZERO; FRAME_HEIGHT];
        assert_eq!(
            bincode::encode_to_vec(&history, config).unwrap(),
            bincode::encode_to_vec(&legacy, config).unwrap()
        );
        for (line, row) in legacy.iter_mut().enumerate() {
            row[17] = (line / 8) as u8;
            history.capture(line, row);
        }
        assert!(Arc::ptr_eq(&history.rows[8], &history.rows[15]));
        assert!(!Arc::ptr_eq(&history.rows[7], &history.rows[8]));
        let bytes = bincode::encode_to_vec(&legacy, config).unwrap();
        assert_eq!(bytes, bincode::encode_to_vec(&history, config).unwrap());
        let (restored, consumed): (LineVram, usize) =
            bincode::decode_from_slice(&bytes, config).unwrap();
        assert_eq!(consumed, bytes.len());
        for line in 0..FRAME_HEIGHT {
            assert_eq!(restored[line], legacy[line]);
        }
        history.capture(0, &[42; VRAM_SIZE]);
        assert_eq!(restored[0], legacy[0]);
    }
}
