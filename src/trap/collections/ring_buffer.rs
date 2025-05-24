// nt_rustos/src/trap/collections/ring_buffer.rs

//! # Generic Ring Buffer
//!
//! A heap-allocated, generic ring buffer (or circular queue) implementation.
//! It provides a fixed-capacity buffer that overwrites the oldest elements
//! when full.

use alloc::vec::Vec;
use core::fmt;

/// A generic, circular buffer.
pub struct RingBuffer<T> {
    buffer: Vec<Option<T>>,
    capacity: usize,
    head: usize,
    tail: usize,
    count: usize,
}

impl<T: Clone> RingBuffer<T> {
    /// Creates a new `RingBuffer` with a specified capacity.
    /// The buffer is allocated on the heap.
    ///
    /// # Panics
    /// Panics if the capacity is 0.
    pub fn with_capacity(capacity: usize) -> Self {
        assert!(capacity > 0, "RingBuffer capacity cannot be zero");
        let mut buffer = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            buffer.push(None);
        }

        Self {
            buffer,
            capacity,
            head: 0,
            tail: 0,
            count: 0,
        }
    }

    /// Pushes an element into the buffer.
    /// If the buffer is full, the oldest element is overwritten.
    pub fn push(&mut self, item: T) {
        // Place the new item at the current head position.
        self.buffer[self.head] = Some(item);

        // Advance the head, wrapping around if necessary.
        self.head = (self.head + 1) % self.capacity;

        if self.is_full() {
            // If full, the tail also advances, overwriting the oldest item.
            self.tail = (self.tail + 1) % self.capacity;
        } else {
            // If not full, the count simply increases.
            self.count += 1;
        }
    }

    /// Removes and returns the oldest element from the buffer.
    /// Returns `None` if the buffer is empty.
    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }

        // Take the item from the tail position.
        let item = self.buffer[self.tail].take();

        // Advance the tail, wrapping around if necessary.
        self.tail = (self.tail + 1) % self.capacity;
        self.count -= 1;

        item
    }

    /// Returns a reference to the oldest element without removing it.
    pub fn front(&self) -> Option<&T> {
        if self.is_empty() {
            None
        } else {
            self.buffer[self.tail].as_ref()
        }
    }

    /// Returns a reference to the newest element without removing it.
    pub fn back(&self) -> Option<&T> {
        if self.is_empty() {
            None
        } else {
            // The newest element is at the position just before the current head.
            let index = (self.head + self.capacity - 1) % self.capacity;
            self.buffer[index].as_ref()
        }
    }

    /// Returns the number of elements currently in the buffer.
    pub fn len(&self) -> usize {
        self.count
    }

    /// Returns the total capacity of the buffer.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns `true` if the buffer contains no elements.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Returns `true` if the buffer is at full capacity.
    pub fn is_full(&self) -> bool {
        self.count == self.capacity
    }

    /// Clears the buffer, removing all elements.
    pub fn clear(&mut self) {
        for item in self.buffer.iter_mut() {
            item.take();
        }
        self.head = 0;
        self.tail = 0;
        self.count = 0;
    }

    /// Returns an iterator that yields references to the elements
    /// from oldest to newest.
    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            buffer: self,
            index: self.tail,
            remaining: self.count,
        }
    }
}

/// An iterator over the elements of a `RingBuffer`.
pub struct Iter<'a, T> {
    buffer: &'a RingBuffer<T>,
    index: usize,
    remaining: usize,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }

        let item = self.buffer.buffer[self.index].as_ref();
        self.index = (self.index + 1) % self.buffer.capacity;
        self.remaining -= 1;
        item
    }
}

impl<T: fmt::Debug> fmt::Debug for RingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}