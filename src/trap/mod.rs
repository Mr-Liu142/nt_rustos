// nt_rustos/src/trap/mod.rs

//! # RISC-V Trap, Interrupt, and Error Handling Subsystem
//!
//! This module provides a comprehensive, heap-based, and dynamically configurable
//! system for managing traps (exceptions and interrupts) and system errors
//! for the `nt_rustos` kernel.

// Make submodules accessible within the trap crate.
mod collections;
mod ds;
mod infrastructure;
mod api;

// Publicly re-export the entire API module.
pub use self::api::*;

// Re-export key data structures that users of the API might need directly.
pub use self::ds::{
    TrapType, TrapMode, Interrupt, Exception, TrapCause, // Core trap types
    TrapContext, TaskContext,                           // Context structures
    TrapHandler, TrapHandlerResult, TrapError,           // Handler signatures and results
    HandlerHandle, ProtectionLevel, RegistrarId,         // Handler identification and security
    SystemError, ErrorCode, ErrorSource, ErrorLevel,     // Error structures
    ErrorResult,
    KERNEL_REGISTRAR_ID, SYSTEM_REGISTRAR_ID,           // Standard Registrar IDs
};


/// Initializes the entire trap subsystem.
///
/// This function must be called once during kernel startup, after the
/// heap allocator is available. It sets up the hardware trap vector,
/// initializes all necessary managers (for handlers, errors, contexts),
/// and registers default handlers for critical system traps.
///
/// # Arguments
/// * `mode` - The desired trap mode for the hardware trap vector (`stvec`).
///            Typically `TrapMode::Direct`.
///
/// # Panics
/// Panics if called more than once, or if internal initialization fails.
pub fn init(mode: TrapMode) {
    // Initialize the core trap system infrastructure via the DI module.
    // This sets up the global TrapSystem container and registers default handlers.
    infrastructure::di::initialize_trap_system(mode);

    // nt_rustos::println!("Trap subsystem fully initialized."); // Requires a println macro
}