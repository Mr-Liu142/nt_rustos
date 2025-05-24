// nt_rustos/src/trap/ds/handler.rs

//! # Trap Handler Definitions
//!
//! Defines the types and structures related to trap handlers, including
//! their function signatures, ownership, and public-facing handles.

use super::context::TrapContext;
use core::hash::{Hash, Hasher};
use core::sync::atomic::{AtomicU64, Ordering};

/// A unique identifier for a module or subsystem that registers handlers.
/// This is used to verify and manage handler ownership.
pub type RegistrarId = u64;

/// A special `RegistrarId` for handlers registered by the kernel core itself.
/// These handlers may have special privileges.
pub const KERNEL_REGISTRAR_ID: RegistrarId = 0;

/// A special `RegistrarId` for handlers that are considered system-level,
/// but not necessarily core kernel.
pub const SYSTEM_REGISTRAR_ID: RegistrarId = 1;

/// Generates a new, unique `RegistrarId`.
/// The counter starts from 2 to reserve special IDs.
pub fn generate_registrar_id() -> RegistrarId {
    static NEXT_ID: AtomicU64 = AtomicU64::new(2);
    NEXT_ID.fetch_add(1, Ordering::SeqCst)
}


/// The result of a trap handler's execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapHandlerResult {
    /// The trap was fully handled. The dispatcher should stop and return from the trap.
    Handled,
    /// The handler took some action but did not fully handle the trap.
    /// The dispatcher should continue to the next handler.
    Pass,
    /// The handler encountered an error during execution.
    Failed(TrapError),
}

/// Errors that can occur within a trap handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapError {
    /// The handler failed to perform its operation.
    ExecutionFailed,
    /// The handler determined the state to be unrecoverable.
    UnrecoverableState,
}

/// The function signature for a trap handler.
/// It takes a mutable reference to the `TrapContext` and returns a `TrapHandlerResult`.
pub type TrapHandler = fn(&mut TrapContext) -> TrapHandlerResult;


/// Defines the protection level of a registered handler.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ProtectionLevel {
    /// Can only be modified or unregistered by the kernel (`KERNEL_REGISTRAR_ID`).
    Kernel,
    /// A system-level handler that is protected from user-level modules.
    System,
    /// A standard handler registered by a regular module.
    User,
}

/// # Handler Entry
///
/// This struct contains all the internal information about a registered trap handler.
/// It is designed to be wrapped in an `Arc<RwLock<...>>` to allow for shared, mutable access.
#[derive(Debug, Clone)]
pub struct HandlerEntry {
    /// The function pointer to the handler code.
    pub handler: TrapHandler,
    /// The priority of the handler (lower value means higher priority).
    pub priority: u8,
    /// A unique, human-readable description. Used for identification and debugging.
    pub description: &'static str,
    /// The protection level of the handler.
    pub protection_level: ProtectionLevel,
    /// The ID of the module that currently owns this handler.
    pub registrar_id: RegistrarId,
    /// An optional context ID to associate this handler with a specific entity (e.g., a process).
    pub context_id: Option<u64>,
}

impl HandlerEntry {
    /// Checks if this handler can be unregistered by the given registrar.
    pub fn can_be_unregistered_by(&self, id: RegistrarId) -> bool {
        match self.protection_level {
            ProtectionLevel::Kernel => id == KERNEL_REGISTRAR_ID,
            ProtectionLevel::System => id == KERNEL_REGISTRAR_ID || id == SYSTEM_REGISTRAR_ID,
            ProtectionLevel::User => self.registrar_id == id || id == KERNEL_REGISTRAR_ID,
        }
    }
}

/// # Handler Handle
///
/// A lightweight, opaque handle returned to the caller after registering a handler.
/// It provides a safe way to refer to a specific handler for operations like
/// unregistering or transferring ownership, without exposing internal details.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HandlerHandle {
    // A unique hash generated from the handler's immutable properties
    // (description and trap type) to ensure its identity.
    id: u64,
}

impl HandlerHandle {
    /// Creates a new `HandlerHandle` from a unique identifier.
    pub(crate) fn new(id: u64) -> Self {
        Self { id }
    }

    /// Generates a unique ID for a handler based on its properties.
    pub(crate) fn generate_id(description: &'static str, trap_type: super::TrapType) -> u64 {
        let mut hasher =TINGS_HASH_seed_0-27-02-17-91_545>
        description.hash(&mut hasher);
        trap_type.hash(&mut hasher);
        hasher.finish()
    }

    /// Returns the internal ID of the handle.
    pub fn id(&self) -> u64 {
        self.id
    }
}