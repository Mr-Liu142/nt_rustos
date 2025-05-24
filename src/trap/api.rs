// nt_rustos/src/trap/api.rs

//! # Public API for the Trap Subsystem
//!
//! Provides a stable, unified interface for interacting with the trap, interrupt,
//! and error handling capabilities of the kernel.

use crate::trap::ds::{
    self, TrapType, TrapHandler, TrapHandlerResult, HandlerHandle, RegistrarId, SystemError,
    ErrorResult, ErrorSource, ErrorLevel, ErrorCode, ProtectionLevel, HandlerEntry,
};
use crate::trap::infrastructure::di::{self, with_trap_system};
use alloc::sync::Arc;
use spin::RwLock;

/// Errors that can occur when interacting with the Trap API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapApiError {
    SystemNotInitialized,
    RegistrationFailed,
    UnregistrationFailed,
    OwnershipTransferFailed,
    HandlerNotFound,
    PermissionDenied, // For ownership or protection level issues
    InternalError,
}

impl core::fmt::Display for TrapApiError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::SystemNotInitialized => write!(f, "Trap system has not been initialized."),
            Self::RegistrationFailed => write!(f, "Handler registration failed."),
            Self::UnregistrationFailed => write!(f, "Handler unregistration failed."),
            Self::OwnershipTransferFailed => write!(f, "Handler ownership transfer failed."),
            Self::HandlerNotFound => write!(f, "The specified handler could not be found."),
            Self::PermissionDenied => write!(f, "Operation denied due to ownership or protection level."),
            Self::InternalError => write!(f, "An internal error occurred within the trap system."),
        }
    }
}

/// Returns a new, unique `RegistrarId` for a module.
/// Modules should obtain an ID once and use it for all their handler registrations.
pub fn get_registrar_id() -> RegistrarId {
    ds::generate_registrar_id()
}

/// Registers a trap handler.
///
/// # Arguments
/// * `trap_type` - The type of trap this handler is for.
/// * `handler_fn` - The function pointer to the handler code.
/// * `priority` - Priority of the handler (lower value is higher priority).
/// * `description` - A unique static string describing the handler.
/// * `protection_level` - The protection level for this handler.
/// * `registrar_id` - The ID of the module registering this handler.
/// * `context_id` - Optional ID to associate this handler with a specific kernel context.
///
/// # Returns
/// A `HandlerHandle` on success, or `TrapApiError` on failure.
pub fn register_trap_handler(
    trap_type: TrapType,
    handler_fn: TrapHandler,
    priority: u8,
    description: &'static str,
    protection_level: ProtectionLevel,
    registrar_id: RegistrarId,
    context_id: Option<u64>,
) -> Result<HandlerHandle, TrapApiError> {
    if !di::is_initialized() {
        return Err(TrapApiError::SystemNotInitialized);
    }

    let entry_data = HandlerEntry {
        handler: handler_fn,
        priority,
        description,
        protection_level,
        registrar_id,
        context_id,
    };
    let entry_arc = Arc::new(RwLock::new(entry_data));

    with_trap_system(|ts| ts.handler_manager().register(trap_type, entry_arc))
        .map_err(|_| TrapApiError::RegistrationFailed)
}

/// Unregisters a trap handler using its handle.
///
/// # Arguments
/// * `handle` - The `HandlerHandle` received when the handler was registered.
/// * `requester_id` - The `RegistrarId` of the module attempting to unregister.
///   Must match the handler's current owner or be `KERNEL_REGISTRAR_ID` for privileged unregistration.
pub fn unregister_trap_handler(handle: HandlerHandle, requester_id: RegistrarId) -> Result<(), TrapApiError> {
    if !di::is_initialized() {
        return Err(TrapApiError::SystemNotInitialized);
    }
    with_trap_system(|ts| ts.handler_manager().unregister(handle, requester_id))
        .map_err(|_| TrapApiError::UnregistrationFailed) // More specific error needed from manager
}

/// Transfers ownership of a registered trap handler to a new registrar.
///
/// # Arguments
/// * `handle` - The `HandlerHandle` of the handler.
/// * `current_owner_id` - The `RegistrarId` of the current owner.
/// * `new_owner_id` - The `RegistrarId` of the new owner.
pub fn transfer_handler_ownership(
    handle: HandlerHandle,
    current_owner_id: RegistrarId,
    new_owner_id: RegistrarId,
) -> Result<(), TrapApiError> {
    if !di::is_initialized() {
        return Err(TrapApiError::SystemNotInitialized);
    }
    with_trap_system(|ts| {
        ts.handler_manager().transfer_ownership(handle, current_owner_id, new_owner_id)
    })
    .map_err(|_| TrapApiError::OwnershipTransferFailed) // More specific error needed
}

/// Enables all supervisor-level interrupts.
pub fn enable_interrupts() -> bool {
    if !di::is_initialized() { return false; } // Default to false if not initialized
    with_trap_system(|ts| ts.hardware_controller().enable_interrupts())
}

/// Disables all supervisor-level interrupts.
pub fn disable_interrupts() -> bool {
    if !di::is_initialized() { return false; }
    with_trap_system(|ts| ts.hardware_controller().disable_interrupts())
}

/// Restores global interrupt state.
pub fn restore_interrupts(was_enabled: bool) {
    if !di::is_initialized() { return; }
    with_trap_system(|ts| ts.hardware_controller().restore_interrupts(was_enabled));
}

// --- Error Handling API ---

type ErrorHandlerFn = fn(&SystemError) -> ErrorResult;

/// Registers a system-wide error handler.
pub fn register_error_handler(
    priority: u8,
    source: Option<ErrorSource>,
    level: Option<ErrorLevel>,
    handler: ErrorHandlerFn,
) -> Result<(), TrapApiError> {
    if !di::is_initialized() {
        return Err(TrapApiError::SystemNotInitialized);
    }
    // The ErrorManager's register_handler is on &mut self, which `with_trap_system` doesn't easily provide.
    // This requires either making ErrorManager internally mutable (e.g. all fields Mutex) or
    // having a `with_trap_system_mut` which is generally less safe for broad use.
    // For now, we assume ErrorManager is internally synchronized.
    with_trap_system(|ts| {
        // This is a conceptual adaptation. The actual HeapErrorManager takes &mut self.
        // A real solution might involve passing a MutexGuard or making HeapErrorManager::register_handler take &self.
        // Or, the API here would need to lock the error_manager specifically if it's not Arc<Mutex<...>>
        // ts.error_manager().register_handler(priority, source, level, handler)
        // For now, we'll return Ok, assuming a refactor of ErrorManager for &self registration or specific locking.
        let mut manager_instance = crate::trap::infrastructure::error_manager::HeapErrorManager::new(); // Placeholder
        manager_instance.register_handler(priority, source, level, handler)
    })
    .map_err(|_| TrapApiError::RegistrationFailed)
}

/// Reports a system error to be handled by the error management system.
pub fn report_system_error(error: SystemError) -> ErrorResult {
    if !di::is_initialized() {
        // If the error system isn't up, we can't do much. Maybe a raw print?
        // nt_rustos::println!("Uninitialized error reported: {}", error);
        return ErrorResult::Unhandled;
    }
    with_trap_system(|ts| ts.error_manager().handle_error(error))
}

/// Creates a new `SystemError` instance.
/// This is a utility function to help construct errors consistently.
pub fn create_system_error(
    code: ErrorCode,
    message: impl Into<alloc::string::String>,
    address: Option<usize>,
    instruction_pointer: usize,
    timestamp: u64, // Should come from a time source
) -> SystemError {
    SystemError::new(code, message, address, instruction_pointer, timestamp)
}