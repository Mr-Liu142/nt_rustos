// nt_rustos/src/trap/ds/mod.rs

//! # Trap Data Structures Module
//!
//! Defines the core data structures for the trap and error handling subsystem.
//! This includes contexts, trap types, error definitions, and handler-related structures.
//! All data structures in this module are designed to be used with heap allocation
//! via the `alloc` crate.

// The order of declaration matters for public re-export.
pub mod types;
pub mod context;
pub mod error;
pub mod handler;

// Re-export key types for convenient access by other modules.
pub use self::types::{
    TrapCause, TrapMode, TrapType,
    Interrupt, Exception
};

pub use self::context::{
    TrapContext, TaskContext
};

pub use self::error::{
    SystemError, ErrorCode, ErrorResult,
    ErrorSource, ErrorLevel, ErrorLogEntry
};

pub use self::handler::{
    TrapHandler, TrapHandlerResult, TrapError,
    HandlerEntry, HandlerHandle, ProtectionLevel,
    RegistrarId, SYSTEM_REGISTRAR_ID, KERNEL_REGISTRAR_ID
};