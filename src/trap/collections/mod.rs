// nt_rustos/src/trap/collections/mod.rs

//! # Kernel Collections Module
//!
//! Provides common, heap-allocated data structures for use within the kernel,
//! such as a generic ring buffer. These collections are designed to be safe
//! and efficient for kernel-level programming.

pub mod ring_buffer;

// Re-export the RingBuffer for easy access.
pub use self::ring_buffer::RingBuffer;