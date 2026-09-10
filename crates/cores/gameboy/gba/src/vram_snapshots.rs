use std::sync::Arc;

use crate::state::{StateReader, StateWriter};

/// Immutable scanline images. Unchanged VRAM shares storage across lines/frames.
/// State files still contain the original, fully expanded row bytes.
#[derive(Debug)]
pub(crate) struct VramSnapshots {
    rows: Vec<Arc<[u8]>>,
    latest: Option<usize>,
    dirty: bool,
}

impl VramSnapshots {
    pub(crate) fn new(row_size: usize, lines: usize) -> Self {
        let zero: Arc<[u8]> = vec![0; row_size].into();
        Self {
            rows: vec![zero; lines],
            latest: None,
            dirty: true,
        }
    }

    pub(crate) fn invalidate(&mut self) {
        self.dirty = true;
    }

    pub(crate) fn capture(&mut self, line: usize, bytes: &[u8]) {
        if !self.dirty {
            if let Some(previous) = self.latest {
                if previous != line {
                    self.rows[line] = Arc::clone(&self.rows[previous]);
                }
                self.latest = Some(line);
                return;
            }
        }
        // Reuse an unshared row on workloads that change VRAM every scanline.
        if let Some(row) = Arc::get_mut(&mut self.rows[line]) {
            row.copy_from_slice(bytes);
        } else {
            self.rows[line] = Arc::from(bytes);
        }
        self.latest = Some(line);
        self.dirty = false;
    }

    pub(crate) fn read(&self, line: usize, offset: usize) -> u8 {
        self.rows[line][offset]
    }

    pub(crate) fn serialize(&self, writer: &mut StateWriter) {
        for row in &self.rows {
            writer.write_slice(row);
        }
    }

    pub(crate) fn deserialize(&mut self, reader: &mut StateReader) -> Result<(), &'static str> {
        // Loaded snapshots need not match live VRAM, even on a successful load.
        self.latest = None;
        self.dirty = true;
        for row in &mut self.rows {
            reader.read_into_slice(Arc::make_mut(row))?;
        }
        Ok(())
    }

    pub(crate) fn reset(&mut self) {
        *self = Self::new(self.rows[0].len(), self.rows.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shares_unchanged_rows_and_preserves_history_after_writes() {
        let mut snapshots = VramSnapshots::new(4, 3);
        snapshots.capture(0, &[1; 4]);
        snapshots.capture(1, &[1; 4]);
        assert!(Arc::ptr_eq(&snapshots.rows[0], &snapshots.rows[1]));
        snapshots.invalidate();
        snapshots.capture(2, &[2; 4]);
        assert_eq!(snapshots.read(0, 0), 1);
        assert_eq!(snapshots.read(1, 0), 1);
        assert_eq!(snapshots.read(2, 0), 2);
        // Next frame overwrites line zero without changing the previous line one.
        snapshots.capture(0, &[2; 4]);
        assert_eq!(snapshots.read(1, 0), 1);
        assert!(Arc::ptr_eq(&snapshots.rows[0], &snapshots.rows[2]));
        let mut writer = StateWriter::new();
        snapshots.serialize(&mut writer);
        assert_eq!(writer.into_vec(), [2, 2, 2, 2, 1, 1, 1, 1, 2, 2, 2, 2]);
    }
}
