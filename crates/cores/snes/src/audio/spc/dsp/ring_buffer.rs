use super::dsp::BUFFER_LEN;

pub struct RingBuffer {
    left_buffer: Box<[i16; BUFFER_LEN]>,
    right_buffer: Box<[i16; BUFFER_LEN]>,
    write_pos: i32,
    read_pos: i32,
    sample_count: i32,
}

impl RingBuffer {
    pub fn new() -> RingBuffer {
        RingBuffer {
            left_buffer: Box::new([0; BUFFER_LEN]),
            right_buffer: Box::new([0; BUFFER_LEN]),
            write_pos: 0,
            read_pos: 0,
            sample_count: 0,
        }
    }

    pub fn reset(&mut self) {
        for i in 0..BUFFER_LEN {
            self.left_buffer[i] = 0;
            self.right_buffer[i] = 0;
        }
        self.write_pos = 0;
        self.read_pos = 0;
        self.sample_count = 0;
    }

    pub fn write_sample(&mut self, left: i16, right: i16) {
        // Prevent overflow: if the buffer is full, drop the oldest sample.
        // Without this, write_pos wraps and overwrites unread samples while sample_count keeps
        // growing, which will eventually corrupt reads.
        if self.sample_count >= (BUFFER_LEN as i32) {
            self.read_pos = (self.read_pos + 1) % (BUFFER_LEN as i32);
            self.sample_count = (BUFFER_LEN as i32) - 1;
        }
        self.left_buffer[self.write_pos as usize] = left;
        self.right_buffer[self.write_pos as usize] = right;
        self.write_pos = (self.write_pos + 1) % (BUFFER_LEN as i32);
        self.sample_count += 1;
    }

    pub fn read(&mut self, left: &mut [i16], right: &mut [i16], num_samples: i32) {
        for i in 0..num_samples {
            left[i as usize] = self.left_buffer[self.read_pos as usize];
            right[i as usize] = self.right_buffer[self.read_pos as usize];
            self.read_pos = (self.read_pos + 1) % (BUFFER_LEN as i32);
        }
        self.sample_count -= num_samples;
    }

    pub fn discard_oldest(&mut self, num_samples: i32) -> i32 {
        let count = num_samples.clamp(0, self.sample_count);
        if count == 0 {
            return 0;
        }
        self.read_pos = (self.read_pos + count) % (BUFFER_LEN as i32);
        self.sample_count -= count;
        count
    }

    pub fn get_sample_count(&self) -> i32 {
        self.sample_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discard_oldest_drops_from_read_side() {
        let mut ring = RingBuffer::new();
        for i in 0..4 {
            ring.write_sample(i, i + 10);
        }

        assert_eq!(ring.discard_oldest(2), 2);
        assert_eq!(ring.get_sample_count(), 2);

        let mut left = [0; 2];
        let mut right = [0; 2];
        ring.read(&mut left, &mut right, 2);

        assert_eq!(left, [2, 3]);
        assert_eq!(right, [12, 13]);
        assert_eq!(ring.get_sample_count(), 0);
    }

    #[test]
    fn discard_oldest_clamps_to_available_samples() {
        let mut ring = RingBuffer::new();
        ring.write_sample(7, 8);

        assert_eq!(ring.discard_oldest(10), 1);
        assert_eq!(ring.get_sample_count(), 0);
    }
}
