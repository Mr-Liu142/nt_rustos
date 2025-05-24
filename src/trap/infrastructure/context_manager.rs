// nt_rustos/src/trap/infrastructure/context_manager.rs

//! # Heap-based Context Manager
//!
//! Manages the lifecycle of contexts (e.g., processes) and ensures
//! automatic cleanup of associated resources like trap handlers.

use crate::trap::ds::RegistrarId;
use crate::trap::infrastructure::di::traits::{ContextManager, HandlerManager};
use alloc::sync::Arc;

/// Represents a context-aware object, like a process.
pub struct ManagedContext {
    id: u64, // e.g., Process ID
    handler_manager: Arc<dyn HandlerManager>,
}

impl ManagedContext {
    pub fn new(id: u64, handler_manager: Arc<dyn HandlerManager>) -> Self {
        Self { id, handler_manager }
    }
}

impl Drop for ManagedContext {
    /// When a `ManagedContext` is dropped (e.g., a process terminates),
    /// automatically unregister all trap handlers associated with it.
    fn drop(&mut self) {
        self.handler_manager.unregister_for_context(self.id);
    }
}

pub struct HeapContextManager {
    // In a real OS, this would hold a map of all managed contexts, e.g.,
    // contexts: Mutex<BTreeMap<u64, Arc<ManagedContext>>>,
}

impl HeapContextManager {
    pub fn new() -> Self {
        Self {}
    }
}

impl ContextManager for HeapContextManager {
    // Implementations for creating/destroying contexts would go here.
}