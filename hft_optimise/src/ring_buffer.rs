use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Slot<T> {
    value: UnsafeCell<T>,
}

impl<T: Default> Slot<T> {
    fn new() -> Self {
        Self {
            value: UnsafeCell::new(T::default()),
        }
    }
}

// UnsafeCell is not Sync by default, but since SPSC guarantees only one writer
// and one reader access distinct memory slots based on atomically coordinated
// indices, we can safely implement Send and Sync.
unsafe impl<T> Sync for Slot<T> {}
unsafe impl<T> Send for Slot<T> {}

/// Cache line alignment wrapper (64 bytes) to prevent false sharing
/// when fields reside on the same CPU cache line.
#[repr(align(64))]
struct CacheAlignedAtomicUsize {
    value: AtomicUsize,
}

impl CacheAlignedAtomicUsize {
    fn new(val: usize) -> Self {
        Self {
            value: AtomicUsize::new(val),
        }
    }
}

/// A lock-free Single-Producer Single-Consumer (SPSC) Ring Buffer.
/// Size `N` must be a power of two.
#[repr(align(64))]
pub struct SpscRingBuffer<T, const N: usize> {
    buffer: Vec<Slot<T>>,
    head: CacheAlignedAtomicUsize, // Consumer index
    tail: CacheAlignedAtomicUsize, // Producer index
}

unsafe impl<T: Send, const N: usize> Send for SpscRingBuffer<T, N> {}
unsafe impl<T: Sync, const N: usize> Sync for SpscRingBuffer<T, N> {}

impl<T: Default + Copy, const N: usize> SpscRingBuffer<T, N> {
    pub fn new() -> Self {
        assert!(N.is_power_of_two(), "Buffer size must be a power of 2");
        let mut buffer = Vec::with_capacity(N);
        for _ in 0..N {
            buffer.push(Slot::new());
        }
        Self {
            buffer,
            head: CacheAlignedAtomicUsize::new(0),
            tail: CacheAlignedAtomicUsize::new(0),
        }
    }

    /// Enqueues an item into the buffer.
    /// Returns `true` on success, `false` if the buffer is full.
    /// Called only by the Producer thread.
    #[inline(always)]
    pub fn enqueue(&self, item: T) -> bool {
        let current_tail = self.tail.value.load(Ordering::Relaxed);
        let current_head = self.head.value.load(Ordering::Acquire); // Synchronize with Consumer's read

        if (current_tail - current_head) >= N {
            return false; // Buffer is Full
        }

        unsafe {
            let ptr = self.buffer[current_tail & (N - 1)].value.get();
            *ptr = item;
        }
        self.tail.value.store(current_tail + 1, Ordering::Release); // Make write visible to Consumer
        true
    }

    /// Dequeues an item from the buffer.
    /// Returns `true` on success, `false` if the buffer is empty.
    /// Called only by the Consumer thread.
    #[inline(always)]
    pub fn dequeue(&self, item: &mut T) -> bool {
        let current_head = self.head.value.load(Ordering::Relaxed);
        let current_tail = self.tail.value.load(Ordering::Acquire); // Synchronize with Producer's write

        if current_head == current_tail {
            return false; // Buffer is Empty
        }

        unsafe {
            let ptr = self.buffer[current_head & (N - 1)].value.get();
            *item = *ptr;
        }
        self.head.value.store(current_head + 1, Ordering::Release); // Signal slot is free to Producer
        true
    }

    /// Returns the number of items currently in the buffer.
    #[inline(always)]
    pub fn size(&self) -> usize {
        let head = self.head.value.load(Ordering::Relaxed);
        let tail = self.tail.value.load(Ordering::Relaxed);
        tail.wrapping_sub(head)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_fifo() {
        let queue = SpscRingBuffer::<i32, 4>::new();
        assert_eq!(queue.size(), 0);

        // Fill queue
        assert!(queue.enqueue(10));
        assert!(queue.enqueue(20));
        assert!(queue.enqueue(30));
        assert!(queue.enqueue(40));
        assert_eq!(queue.size(), 4);

        // Verify it returns false when full:
        assert!(!queue.enqueue(50));

        let mut val = 0;
        assert!(queue.dequeue(&mut val));
        assert_eq!(val, 10);
        assert_eq!(queue.size(), 3);

        assert!(queue.dequeue(&mut val));
        assert_eq!(val, 20);

        assert!(queue.enqueue(50));
        assert_eq!(queue.size(), 3);

        assert!(queue.dequeue(&mut val));
        assert_eq!(val, 30);
        assert!(queue.dequeue(&mut val));
        assert_eq!(val, 40);
        assert!(queue.dequeue(&mut val));
        assert_eq!(val, 50);

        assert_eq!(queue.size(), 0);
        assert!(!queue.dequeue(&mut val));
    }
}
