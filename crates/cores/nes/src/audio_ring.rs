use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Lock-free single-producer single-consumer ring buffer for audio samples.
///
/// Capacity must be a power of 2. Usable slots = capacity - 1.
/// Producer calls `push_slice`, consumer calls `pop_slice`.
pub struct SpscRingBuffer {
    buffer: Box<[UnsafeCell<f32>]>,
    mask: usize,
    head: AtomicUsize, // read position  (consumer advances)
    tail: AtomicUsize, // write position (producer advances)
}

// Safety: only one thread writes (push_slice) and one reads (pop_slice).
// Atomic indices with Acquire/Release ordering provide the necessary
// happens-before guarantee for the data in `buffer`.
unsafe impl Send for SpscRingBuffer {}
unsafe impl Sync for SpscRingBuffer {}

impl SpscRingBuffer {
    /// Create a ring buffer with power-of-two capacity.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity.is_power_of_two() && capacity >= 2);
        let mut buf = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            buf.push(UnsafeCell::new(0.0));
        }
        Self {
            buffer: buf.into_boxed_slice(),
            mask: capacity - 1,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    /// Number of samples available to read.
    #[inline]
    pub fn len(&self) -> usize {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Relaxed);
        tail.wrapping_sub(head) & self.mask
    }

    #[inline]
    /// Return `true` when no samples are available to read.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Push a single sample. Returns `true` if written, `false` if full.
    #[inline]
    pub fn push_one(&self, sample: f32) -> bool {
        let tail = self.tail.load(Ordering::Relaxed);
        let next = (tail + 1) & self.mask;
        if next == self.head.load(Ordering::Acquire) {
            return false;
        }
        unsafe {
            *self.buffer[tail].get() = sample;
        }
        self.tail.store(next, Ordering::Release);
        true
    }

    /// Push samples into the buffer. Returns how many were actually written.
    /// If the buffer is full, remaining samples are silently dropped.
    pub fn push_slice(&self, samples: &[f32]) -> usize {
        let mut tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        let mut count = 0;

        for &sample in samples {
            let next = (tail + 1) & self.mask;
            if next == head {
                break;
            }
            unsafe {
                *self.buffer[tail].get() = sample;
            }
            tail = next;
            count += 1;
        }

        self.tail.store(tail, Ordering::Release);
        count
    }

    /// Pop samples into `out`. Returns how many were actually read.
    /// Unread slots in `out` are left untouched.
    pub fn pop_slice(&self, out: &mut [f32]) -> usize {
        let mut head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        let mut count = 0;

        for sample in out.iter_mut() {
            if head == tail {
                break;
            }
            *sample = unsafe { *self.buffer[head].get() };
            head = (head + 1) & self.mask;
            count += 1;
        }

        self.head.store(head, Ordering::Release);
        count
    }

    /// Discard up to `n` oldest samples without reading them.
    /// Only call from the consumer side.
    pub fn discard(&self, n: usize) {
        let head = self.head.load(Ordering::Relaxed);
        let available = self.len();
        let skip = n.min(available);
        self.head
            .store((head + skip) & self.mask, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_pop_basic() {
        let rb = SpscRingBuffer::new(8);
        assert_eq!(rb.len(), 0);

        let data = [1.0, 2.0, 3.0];
        assert_eq!(rb.push_slice(&data), 3);
        assert_eq!(rb.len(), 3);

        let mut out = [0.0f32; 4];
        assert_eq!(rb.pop_slice(&mut out), 3);
        assert_eq!(&out[..3], &[1.0, 2.0, 3.0]);
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn overflow_drops_new() {
        let rb = SpscRingBuffer::new(4); // usable = 3
        let data = [1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(rb.push_slice(&data), 3);
        assert_eq!(rb.len(), 3);

        let mut out = [0.0f32; 5];
        assert_eq!(rb.pop_slice(&mut out), 3);
        assert_eq!(&out[..3], &[1.0, 2.0, 3.0]);
    }

    #[test]
    fn wrap_around() {
        let rb = SpscRingBuffer::new(4); // usable = 3
        rb.push_slice(&[1.0, 2.0]);
        let mut out = [0.0f32; 2];
        rb.pop_slice(&mut out); // head advances past 0

        rb.push_slice(&[3.0, 4.0, 5.0]);
        assert_eq!(rb.len(), 3);
        let mut out2 = [0.0f32; 3];
        assert_eq!(rb.pop_slice(&mut out2), 3);
        assert_eq!(&out2, &[3.0, 4.0, 5.0]);
    }

    #[test]
    fn discard_works() {
        let rb = SpscRingBuffer::new(8);
        rb.push_slice(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        rb.discard(3);
        assert_eq!(rb.len(), 2);
        let mut out = [0.0f32; 2];
        rb.pop_slice(&mut out);
        assert_eq!(&out, &[4.0, 5.0]);
    }
}
